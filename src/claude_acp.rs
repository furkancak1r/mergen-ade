use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const MIMO_CLAUDE_ENV: [(&str, &str); 7] = [
    (
        "ANTHROPIC_BASE_URL",
        "https://token-plan-sgp.xiaomimimo.com/anthropic",
    ),
    ("ANTHROPIC_MODEL", "mimo-v2.5-pro"),
    ("ANTHROPIC_SMALL_FAST_MODEL", "mimo-v2.5-pro"),
    ("ANTHROPIC_DEFAULT_SONNET_MODEL", "mimo-v2.5-pro"),
    ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "mimo-v2.5-pro"),
    ("ANTHROPIC_DEFAULT_OPUS_MODEL", "mimo-v2.5-pro"),
    ("CLAUDE_CODE_SUBAGENT_MODEL", "mimo-v2.5-pro"),
];

use serde_json::Value as JsonValue;

use crate::opencode_acp::{AcpChatEvent, AcpChatSession};

/// Resolve the Claude Code binary path.
pub fn claude_bin_path() -> std::ffi::OsString {
    if let Ok(path) = std::env::var("CLAUDE_BIN_PATH") {
        return std::ffi::OsString::from(path);
    }
    #[cfg(target_os = "windows")]
    {
        std::ffi::OsString::from("claude.cmd")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::ffi::OsString::from("claude")
    }
}

/// Create a new Claude Code ACP session without spawning a child process.
/// The process is spawned per-turn via `send_claude_code_prompt`.
pub fn spawn_claude_code(
    chat_id: u64,
    project_id: u64,
    project_path: PathBuf,
    event_tx: crossbeam_channel::Sender<AcpChatEvent>,
) -> AcpChatSession {
    AcpChatSession::new_claude_code(chat_id, project_id, project_path, event_tx)
}

/// Spawn a Claude Code process for a single turn and stream events back.
pub fn send_claude_code_prompt(
    chat_id: u64,
    project_path: &Path,
    session_id: Option<String>,
    prompt_text: &str,
    event_tx: crossbeam_channel::Sender<AcpChatEvent>,
    child_handle: Arc<Mutex<Option<Child>>>,
) -> std::io::Result<()> {
    let claude_bin = claude_bin_path();
    let mut command = Command::new(&claude_bin);
    command.arg("--print");
    command.arg("--output-format");
    command.arg("stream-json");
    command.arg("--verbose");
    command.arg("--permission-mode");
    command.arg("bypassPermissions");
    if let Some(ref sid) = session_id {
        command.arg("--session-id");
        command.arg(sid);
    }
    command.arg("--");
    command.arg(prompt_text);
    command.current_dir(project_path);
    configure_mimo_claude_env(&mut command);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    // stdin is not needed - prompt is passed as positional arg.

    // Store child handle for kill support.
    {
        let mut handle = child_handle.lock().unwrap();
        *handle = Some(child);
    }

    // Reader thread: parse NDJSON from stdout.
    let reader_event_tx = event_tx.clone();
    let reader_child_handle = child_handle.clone();
    let reader_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            let msg: JsonValue = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    let _ = reader_event_tx.send(AcpChatEvent::Error {
                        chat_id,
                        message: format!("Claude JSON parse error: {e}"),
                    });
                    continue;
                }
            };
            parse_claude_code_message(chat_id, msg, &reader_event_tx);
        }
        // Clear child handle when process exits.
        {
            let mut handle = reader_child_handle.lock().unwrap();
            *handle = None;
        }
        let _ = reader_event_tx.send(AcpChatEvent::Disconnected { chat_id });
    });

    // Stderr thread: surface errors.
    let stderr_event_tx = event_tx.clone();
    let stderr_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            // Filter out common non-error noise.
            if line.contains("SessionEnd hook")
                || line.contains("SessionStart hook")
                || line.contains("hook_started")
                || line.contains("hook_response")
            {
                continue;
            }
            let _ = stderr_event_tx.send(AcpChatEvent::Error {
                chat_id,
                message: line,
            });
        }
    });

    // Detach threads - they will run until the process exits.
    thread::spawn(move || {
        let _ = reader_thread.join();
        let _ = stderr_thread.join();
    });

    Ok(())
}

fn configure_mimo_claude_env(command: &mut Command) {
    command.env_remove("ANTHROPIC_AUTH_TOKEN");
    command.env_remove("ANTHROPIC_API_KEY");
    for (key, value) in MIMO_CLAUDE_ENV {
        command.env(key, value);
    }
}

/// Parse a single Claude Code stream-json message and emit AcpChatEvents.
fn parse_claude_code_message(
    chat_id: u64,
    msg: JsonValue,
    event_tx: &crossbeam_channel::Sender<AcpChatEvent>,
) {
    let msg_type = msg.get("type").and_then(JsonValue::as_str).unwrap_or("");

    match msg_type {
        "system" => {
            let subtype = msg.get("subtype").and_then(JsonValue::as_str).unwrap_or("");
            if subtype == "init" {
                let session_id = msg
                    .get("session_id")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_string();
                let _ = event_tx.send(AcpChatEvent::Connected { chat_id });
                let _ = event_tx.send(AcpChatEvent::SessionCreated {
                    chat_id,
                    session_id,
                });
            }
            // Other system subtypes (hook_started, hook_response) are ignored.
        }
        "assistant" => {
            if let Some(message) = msg.get("message") {
                if let Some(content) = message.get("content").and_then(JsonValue::as_array) {
                    for block in content {
                        let block_type =
                            block.get("type").and_then(JsonValue::as_str).unwrap_or("");
                        match block_type {
                            "text" => {
                                if let Some(text) = block.get("text").and_then(JsonValue::as_str) {
                                    let _ = event_tx.send(AcpChatEvent::AgentMessageChunk {
                                        chat_id,
                                        text: text.to_string(),
                                    });
                                }
                            }
                            "thinking" => {
                                // Show thinking blocks as agent messages with a prefix.
                                if let Some(thinking) =
                                    block.get("thinking").and_then(JsonValue::as_str)
                                {
                                    if !thinking.is_empty() {
                                        let _ = event_tx.send(AcpChatEvent::AgentMessageChunk {
                                            chat_id,
                                            text: format!("[Thinking] {}", thinking),
                                        });
                                    }
                                }
                            }
                            "tool_use" => {
                                let tool_call_id = block
                                    .get("id")
                                    .and_then(JsonValue::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let title = block
                                    .get("name")
                                    .and_then(JsonValue::as_str)
                                    .unwrap_or("tool")
                                    .to_string();
                                let _ = event_tx.send(AcpChatEvent::ToolCall {
                                    chat_id,
                                    tool_call_id,
                                    title,
                                    kind: "tool_use".to_string(),
                                    status: "running".to_string(),
                                });
                            }
                            "tool_result" => {
                                let tool_call_id = block
                                    .get("tool_use_id")
                                    .and_then(JsonValue::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let is_error = block
                                    .get("is_error")
                                    .and_then(JsonValue::as_bool)
                                    .unwrap_or(false);
                                let status = if is_error { "error" } else { "completed" };
                                let content = block.get("content").and_then(|c| {
                                    if c.is_string() {
                                        c.as_str().map(|s| s.to_string())
                                    } else {
                                        Some(c.to_string())
                                    }
                                });
                                let _ = event_tx.send(AcpChatEvent::ToolCallUpdate {
                                    chat_id,
                                    tool_call_id,
                                    status: status.to_string(),
                                    content,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        "result" => {
            let is_error = msg
                .get("is_error")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);
            if is_error {
                let errors = msg
                    .get("errors")
                    .and_then(JsonValue::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| "Unknown error".to_string());
                let _ = event_tx.send(AcpChatEvent::Error {
                    chat_id,
                    message: errors,
                });
            }
            let stop_reason = msg
                .get("stop_reason")
                .and_then(JsonValue::as_str)
                .unwrap_or("end_turn")
                .to_string();
            let _ = event_tx.send(AcpChatEvent::PromptResponse {
                chat_id,
                stop_reason,
            });
        }
        _ => {
            // Unknown message type - ignore.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opencode_acp::{AcpBackend, AcpChatStatus};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn parse_system_init_emits_connected_and_session_created() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "type": "system",
            "subtype": "init",
            "session_id": "test-session-id",
            "cwd": "/test/project",
            "model": "claude-sonnet-4-6",
            "tools": ["Bash", "Read", "Write"]
        });
        parse_claude_code_message(42, msg, &tx);
        let event1 = rx.recv().unwrap();
        assert!(matches!(event1, AcpChatEvent::Connected { chat_id: 42 }));
        let event2 = rx.recv().unwrap();
        match event2 {
            AcpChatEvent::SessionCreated {
                chat_id,
                session_id,
            } => {
                assert_eq!(chat_id, 42);
                assert_eq!(session_id, "test-session-id");
            }
            _ => panic!("Expected SessionCreated"),
        }
    }

    #[test]
    fn parse_assistant_text_emits_agent_message_chunk() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "type": "assistant",
            "message": {
                "id": "msg_123",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-6",
                "content": [
                    { "type": "text", "text": "Hello world" }
                ]
            }
        });
        parse_claude_code_message(1, msg, &tx);
        let event = rx.recv().unwrap();
        match event {
            AcpChatEvent::AgentMessageChunk { chat_id, text } => {
                assert_eq!(chat_id, 1);
                assert_eq!(text, "Hello world");
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn parse_assistant_tool_use_emits_tool_call() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_123",
                        "name": "Bash",
                        "input": { "command": "ls" }
                    }
                ]
            }
        });
        parse_claude_code_message(5, msg, &tx);
        let event = rx.recv().unwrap();
        match event {
            AcpChatEvent::ToolCall {
                chat_id,
                tool_call_id,
                title,
                kind,
                status,
            } => {
                assert_eq!(chat_id, 5);
                assert_eq!(tool_call_id, "toolu_123");
                assert_eq!(title, "Bash");
                assert_eq!(kind, "tool_use");
                assert_eq!(status, "running");
            }
            _ => panic!("Expected ToolCall"),
        }
    }

    #[test]
    fn parse_result_success_emits_prompt_response() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "stop_reason": "end_turn",
            "duration_ms": 5000,
            "total_cost_usd": 0.01
        });
        parse_claude_code_message(3, msg, &tx);
        let event = rx.recv().unwrap();
        match event {
            AcpChatEvent::PromptResponse {
                chat_id,
                stop_reason,
            } => {
                assert_eq!(chat_id, 3);
                assert_eq!(stop_reason, "end_turn");
            }
            _ => panic!("Expected PromptResponse"),
        }
    }

    #[test]
    fn parse_result_error_emits_error_and_prompt_response() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "type": "result",
            "is_error": true,
            "stop_reason": "end_turn",
            "errors": ["Budget exceeded"]
        });
        parse_claude_code_message(7, msg, &tx);
        let event1 = rx.recv().unwrap();
        assert!(matches!(event1, AcpChatEvent::Error { chat_id: 7, .. }));
        let event2 = rx.recv().unwrap();
        assert!(matches!(
            event2,
            AcpChatEvent::PromptResponse { chat_id: 7, .. }
        ));
    }

    #[test]
    fn parse_unknown_type_is_ignored() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({ "type": "unknown_event", "data": "..." });
        parse_claude_code_message(1, msg, &tx);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn claude_session_has_claude_code_backend() {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let session = spawn_claude_code(1, 7, PathBuf::from("/test"), tx);
        assert!(session.is_claude_code());
        assert_eq!(session.backend, AcpBackend::ClaudeCode);
        assert_eq!(session.status, AcpChatStatus::Idle);
        assert!(session.session_id.is_none());
    }

    #[test]
    fn claude_command_env_forces_mimo_and_removes_auth_overrides() {
        let mut command = Command::new("claude");
        configure_mimo_claude_env(&mut command);

        let envs: BTreeMap<String, Option<String>> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().to_string(),
                    value.map(|value| value.to_string_lossy().to_string()),
                )
            })
            .collect();

        assert_eq!(
            envs.get("ANTHROPIC_BASE_URL")
                .and_then(|value| value.as_deref()),
            Some("https://token-plan-sgp.xiaomimimo.com/anthropic")
        );
        assert_eq!(
            envs.get("ANTHROPIC_MODEL")
                .and_then(|value| value.as_deref()),
            Some("mimo-v2.5-pro")
        );
        assert_eq!(envs.get("ANTHROPIC_AUTH_TOKEN"), Some(&None));
        assert_eq!(envs.get("ANTHROPIC_API_KEY"), Some(&None));
    }
}
