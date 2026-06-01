use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Instant;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use serde_json::{json, Value as JsonValue};

/// Events sent from the ACP agent thread to the UI thread.
#[derive(Debug, Clone)]
pub enum AcpChatEvent {
    Connected {
        chat_id: u64,
    },
    SessionCreated {
        chat_id: u64,
        session_id: String,
    },
    AgentMessageChunk {
        chat_id: u64,
        text: String,
    },
    UserMessageChunk {
        chat_id: u64,
        text: String,
    },
    ToolCall {
        chat_id: u64,
        tool_call_id: String,
        title: String,
        kind: String,
        status: String,
    },
    ToolCallUpdate {
        chat_id: u64,
        tool_call_id: String,
        status: String,
        content: Option<String>,
    },
    Plan {
        chat_id: u64,
        entries: Vec<AcpPlanEntry>,
    },
    CurrentModeUpdate {
        chat_id: u64,
        mode_id: String,
    },
    ConfigOptionUpdate {
        chat_id: u64,
        category: String,
        value: String,
    },
    AvailableCommandsUpdate {
        chat_id: u64,
        commands: Vec<AcpCommand>,
    },
    PromptResponse {
        chat_id: u64,
        stop_reason: String,
    },
    PermissionRequest {
        chat_id: u64,
        request_id: String,
        options: Vec<AcpPermissionOption>,
        tool_call: AcpToolCallBrief,
    },
    Error {
        chat_id: u64,
        message: String,
    },
    Disconnected {
        chat_id: u64,
    },
}

#[derive(Debug, Clone)]
pub struct AcpPlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct AcpPermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct AcpToolCallBrief {
    pub tool_call_id: String,
    pub title: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub enum AcpChatMessage {
    User {
        text: String,
    },
    Agent {
        text: String,
    },
    ToolCall {
        id: String,
        title: String,
        kind: String,
        status: String,
        content: Option<String>,
    },
    System {
        text: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpChatStatus {
    Starting,
    Idle,
    Running,
    Permission,
    Error,
    Exited,
}

#[derive(Debug, Clone)]
pub struct AcpPendingPermission {
    pub request_id: String,
    pub options: Vec<AcpPermissionOption>,
    pub tool_call: AcpToolCallBrief,
}

#[derive(Debug, Clone)]
pub struct AcpConfigOption {
    pub category: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct AcpMode {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AcpCommand {
    pub name: String,
    pub description: String,
}

/// A single ACP chat session with the OpenCode agent.
#[derive(Debug)]
pub struct AcpChatSession {
    pub id: u64,
    pub project_id: u64,
    pub project_path: PathBuf,
    pub title: String,
    pub created_at: Instant,
    pub updated_at: Instant,
    pub status: AcpChatStatus,
    pub session_id: Option<String>,
    pub config_options: BTreeMap<String, String>,
    pub modes: Vec<AcpMode>,
    pub available_commands: Vec<AcpCommand>,
    pub messages: Vec<AcpChatMessage>,
    pub pending_permission: Option<AcpPendingPermission>,
    pub prompt_input: String,
    pub is_running: bool,
    pub show_thread_selector: bool,
    pub selected_mode_id: Option<String>,
    command_tx: crossbeam_channel::Sender<String>,
    #[allow(dead_code)]
    process: Option<Child>,
    #[allow(dead_code)]
    writer_thread: Option<thread::JoinHandle<()>>,
    #[allow(dead_code)]
    reader_thread: Option<thread::JoinHandle<()>>,
    #[allow(dead_code)]
    stderr_thread: Option<thread::JoinHandle<()>>,
}

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

impl AcpChatSession {
    /// Send a JSON-RPC request.
    pub fn send_request(&self, id: u64, method: &str, params: JsonValue) {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let _ = self.command_tx.send(msg.to_string());
    }

    /// Send a session/new request.
    pub fn send_session_new(&self) {
        let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let mut params = json!({
            "cwd": self.project_path.to_string_lossy().to_string(),
            "mcpServers": []
        });
        if let Some(ref mode_id) = self.selected_mode_id {
            params["modeId"] = json!(mode_id);
        }
        if let Some(model) = self.config_options.get("model") {
            params["model"] = json!(model);
        }
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/new",
            "params": params
        });
        let _ = self.command_tx.send(msg.to_string());
    }

    /// Send a session/prompt request.
    pub fn send_prompt(&self, text: &str) {
        let session_id = match self.session_id {
            Some(ref id) => id.clone(),
            None => return,
        };
        let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let mut params = json!({
            "sessionId": session_id,
            "prompt": [{ "type": "text", "text": text }]
        });
        if let Some(ref mode_id) = self.selected_mode_id {
            params["modeId"] = json!(mode_id);
        }
        if let Some(model) = self.config_options.get("model") {
            params["model"] = json!(model);
        }
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/prompt",
            "params": params
        });
        let _ = self.command_tx.send(msg.to_string());
    }

    /// Send a session/cancel notification.
    pub fn send_cancel(&self) {
        let session_id = self.session_id.clone().unwrap_or_default();
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "session/cancel",
            "params": {
                "sessionId": session_id
            }
        });
        let _ = self.command_tx.send(msg.to_string());
    }

    /// Respond to a permission request.
    pub fn send_permission_response(&self, request_id: &str, option_id: &str) {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "outcome": {
                    "outcome": "selected",
                    "optionId": option_id
                }
            }
        });
        let _ = self.command_tx.send(msg.to_string());
    }
}

/// Spawn a new OpenCode ACP chat session.
pub fn spawn_opencode_acp(
    chat_id: u64,
    project_id: u64,
    project_path: PathBuf,
    opencode_bin: Option<std::ffi::OsString>,
    build_model: Option<String>,
    browser_mcp_env: Vec<(String, String)>,
    event_tx: crossbeam_channel::Sender<AcpChatEvent>,
) -> std::io::Result<AcpChatSession> {
    let mut command =
        Command::new(opencode_bin.unwrap_or_else(|| std::ffi::OsString::from("opencode")));
    command.arg("acp");
    command.arg("--cwd");
    command.arg(&project_path);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(model) = build_model {
        command.env(
            "OPENCODE_CONFIG_CONTENT",
            json!({
                "agent": { "build": { "model": model } },
                "mode": { "build": { "model": model } }
            })
            .to_string(),
        );
    }
    for (k, v) in browser_mcp_env {
        command.env(k, v);
    }
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn()?;
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (command_tx, command_rx) = crossbeam_channel::unbounded::<String>();
    // Writer thread
    let writer_thread = {
        thread::spawn(move || {
            let mut writer = std::io::BufWriter::new(stdin);
            while let Ok(line) = command_rx.recv() {
                if writer.write_all(line.as_bytes()).is_err() {
                    break;
                }
                if writer.write_all(b"\n").is_err() {
                    break;
                }
                if writer.flush().is_err() {
                    break;
                }
            }
        })
    };
    // Reader thread
    let reader_thread = {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
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
                        let _ = event_tx.send(AcpChatEvent::Error {
                            chat_id,
                            message: format!("ACP JSON parse error: {e}"),
                        });
                        continue;
                    }
                };
                parse_acp_message(chat_id, msg, &event_tx);
            }
            let _ = event_tx.send(AcpChatEvent::Disconnected { chat_id });
        })
    };
    // Stderr reader thread — surface OpenCode errors in the chat UI
    let stderr_thread = {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                if line.trim().is_empty() {
                    continue;
                }
                let _ = event_tx.send(AcpChatEvent::Error {
                    chat_id,
                    message: line,
                });
            }
        })
    };
    // Send initialize immediately
    let init_msg = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false
            },
            "clientInfo": {
                "name": "mergen-ade",
                "title": "Mergen ADE",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });
    let _ = command_tx.send(init_msg.to_string());
    let now = Instant::now();
    Ok(AcpChatSession {
        id: chat_id,
        project_id,
        project_path,
        title: format!("Chat {chat_id}"),
        created_at: now,
        updated_at: now,
        status: AcpChatStatus::Starting,
        messages: Vec::new(),
        pending_permission: None,
        prompt_input: String::new(),
        is_running: false,
        session_id: None,
        config_options: BTreeMap::new(),
        modes: Vec::new(),
        available_commands: Vec::new(),
        show_thread_selector: false,
        selected_mode_id: None,
        command_tx,
        process: Some(child),
        writer_thread: Some(writer_thread),
        reader_thread: Some(reader_thread),
        stderr_thread: Some(stderr_thread),
    })
}

fn parse_acp_message(
    chat_id: u64,
    msg: JsonValue,
    event_tx: &crossbeam_channel::Sender<AcpChatEvent>,
) {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(JsonValue::as_str);
    let result = msg.get("result");
    let error = msg.get("error");
    if id.is_some() {
        if result.is_some() || error.is_some() {
            // Response
            if let Some(result) = result {
                if result.get("protocolVersion").is_some() {
                    let _ = event_tx.send(AcpChatEvent::Connected { chat_id });
                    return;
                }
                if result.get("sessionId").is_some() {
                    let session_id = result
                        .get("sessionId")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let _ = event_tx.send(AcpChatEvent::SessionCreated {
                        chat_id,
                        session_id,
                    });
                    // Parse configOptions from session/new response
                    if let Some(options) = result.get("configOptions").and_then(JsonValue::as_array)
                    {
                        for opt in options {
                            let category = opt
                                .get("configId")
                                .and_then(JsonValue::as_str)
                                .unwrap_or("")
                                .to_string();
                            let value = opt
                                .get("value")
                                .and_then(JsonValue::as_str)
                                .unwrap_or("")
                                .to_string();
                            let _ = event_tx.send(AcpChatEvent::ConfigOptionUpdate {
                                chat_id,
                                category: category.clone(),
                                value: value.clone(),
                            });
                        }
                    }
                    return;
                }
                if result.get("stopReason").is_some() {
                    let stop_reason = result
                        .get("stopReason")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let _ = event_tx.send(AcpChatEvent::PromptResponse {
                        chat_id,
                        stop_reason,
                    });
                    return;
                }
                return;
            }
            if let Some(error) = error {
                let message = error
                    .get("message")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("unknown error")
                    .to_string();
                let _ = event_tx.send(AcpChatEvent::Error { chat_id, message });
                return;
            }
        } else if let Some(method) = method {
            // Request from agent
            if method == "session/request_permission" {
                let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
                let request_id = id
                    .and_then(|v| {
                        v.as_u64()
                            .map(|n| n.to_string())
                            .or_else(|| v.as_str().map(|s| s.to_owned()))
                    })
                    .unwrap_or_default();
                let options = params
                    .get("options")
                    .and_then(JsonValue::as_array)
                    .map(|arr| {
                        arr.iter()
                            .map(|o| AcpPermissionOption {
                                option_id: o
                                    .get("optionId")
                                    .and_then(JsonValue::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                name: o
                                    .get("name")
                                    .and_then(JsonValue::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                kind: o
                                    .get("kind")
                                    .and_then(JsonValue::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let tool_call = params
                    .get("toolCall")
                    .and_then(JsonValue::as_object)
                    .map(|o| AcpToolCallBrief {
                        tool_call_id: o
                            .get("toolCallId")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("")
                            .to_string(),
                        title: o
                            .get("title")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("")
                            .to_string(),
                        kind: o
                            .get("kind")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                    .unwrap_or_default();
                let _ = event_tx.send(AcpChatEvent::PermissionRequest {
                    chat_id,
                    request_id,
                    options,
                    tool_call,
                });
                return;
            }
            return;
        }
    }
    // Notification
    if let Some(method) = method {
        if method == "session/update" {
            let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
            let update = params
                .get("update")
                .and_then(JsonValue::as_object)
                .cloned()
                .unwrap_or_default();
            let session_update = update
                .get("sessionUpdate")
                .and_then(JsonValue::as_str)
                .unwrap_or("");
            match session_update {
                "agent_message_chunk" => {
                    let text = update
                        .get("content")
                        .and_then(|c| c.get("text"))
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let _ = event_tx.send(AcpChatEvent::AgentMessageChunk { chat_id, text });
                }
                "tool_call" => {
                    let tool_call_id = update
                        .get("toolCallId")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let title = update
                        .get("title")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let kind = update
                        .get("kind")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let status = update
                        .get("status")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("pending")
                        .to_string();
                    let _ = event_tx.send(AcpChatEvent::ToolCall {
                        chat_id,
                        tool_call_id,
                        title,
                        kind,
                        status,
                    });
                }
                "tool_call_update" => {
                    let tool_call_id = update
                        .get("toolCallId")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let status = update
                        .get("status")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let content = update.get("content").and_then(|c| c.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|c| {
                                c.get("content")
                                    .and_then(|cc| cc.get("text"))
                                    .and_then(JsonValue::as_str)
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    });
                    let _ = event_tx.send(AcpChatEvent::ToolCallUpdate {
                        chat_id,
                        tool_call_id,
                        status,
                        content,
                    });
                }
                "plan" => {
                    let entries = update
                        .get("entries")
                        .and_then(JsonValue::as_array)
                        .map(|arr| {
                            arr.iter()
                                .map(|e| AcpPlanEntry {
                                    content: e
                                        .get("content")
                                        .and_then(JsonValue::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                    priority: e
                                        .get("priority")
                                        .and_then(JsonValue::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                    status: e
                                        .get("status")
                                        .and_then(JsonValue::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let _ = event_tx.send(AcpChatEvent::Plan { chat_id, entries });
                }
                "current_mode_update" => {
                    let mode_id = update
                        .get("currentModeId")
                        .and_then(JsonValue::as_str)
                        .or_else(|| update.get("modeId").and_then(JsonValue::as_str))
                        .unwrap_or("")
                        .to_string();
                    let _ = event_tx.send(AcpChatEvent::CurrentModeUpdate { chat_id, mode_id });
                }
                "user_message_chunk" => {
                    let text = update
                        .get("content")
                        .and_then(|c| c.get("text"))
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                        .to_string();
                    let _ = event_tx.send(AcpChatEvent::UserMessageChunk { chat_id, text });
                }
                "config_option_update" => {
                    // OpenCode ACP spec sends configOptions as an array of {configId, value}
                    if let Some(options) = update.get("configOptions").and_then(JsonValue::as_array)
                    {
                        for opt in options {
                            let category = opt
                                .get("configId")
                                .and_then(JsonValue::as_str)
                                .unwrap_or("")
                                .to_string();
                            let value = opt
                                .get("value")
                                .and_then(JsonValue::as_str)
                                .unwrap_or("")
                                .to_string();
                            let _ = event_tx.send(AcpChatEvent::ConfigOptionUpdate {
                                chat_id,
                                category: category.clone(),
                                value: value.clone(),
                            });
                        }
                    } else {
                        // Fallback for legacy single-field format
                        let category = update
                            .get("category")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("")
                            .to_string();
                        let value = update
                            .get("value")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("")
                            .to_string();
                        let _ = event_tx.send(AcpChatEvent::ConfigOptionUpdate {
                            chat_id,
                            category,
                            value,
                        });
                    }
                }
                "available_commands_update" => {
                    let commands = update
                        .get("availableCommands")
                        .or_else(|| update.get("commands"))
                        .and_then(JsonValue::as_array)
                        .map(|arr| {
                            arr.iter()
                                .map(|c| AcpCommand {
                                    name: c
                                        .get("name")
                                        .and_then(JsonValue::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                    description: c
                                        .get("description")
                                        .and_then(JsonValue::as_str)
                                        .unwrap_or("")
                                        .to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let _ =
                        event_tx.send(AcpChatEvent::AvailableCommandsUpdate { chat_id, commands });
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn test_session() -> (AcpChatSession, crossbeam_channel::Receiver<String>) {
        let (tx, rx) = crossbeam_channel::unbounded::<String>();
        let session = AcpChatSession {
            id: 1,
            project_id: 7,
            project_path: PathBuf::from("C:/test/project"),
            title: "Test".to_string(),
            created_at: Instant::now(),
            updated_at: Instant::now(),
            status: AcpChatStatus::Starting,
            session_id: None,
            config_options: BTreeMap::new(),
            modes: Vec::new(),
            available_commands: Vec::new(),
            messages: Vec::new(),
            pending_permission: None,
            prompt_input: String::new(),
            is_running: false,
            show_thread_selector: false,
            selected_mode_id: None,
            command_tx: tx,
            process: None,
            writer_thread: None,
            reader_thread: None,
            stderr_thread: None,
        };
        (session, rx)
    }

    #[test]
    fn send_session_new_includes_cwd_and_mcp_servers() {
        let (session, rx) = test_session();
        session.send_session_new();
        let msg = rx.recv().unwrap();
        let parsed: JsonValue = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed["method"], "session/new");
        let params = parsed["params"].as_object().unwrap();
        assert!(params.contains_key("cwd"));
        assert_eq!(params["cwd"], "C:/test/project");
        assert!(params.contains_key("mcpServers"));
        assert_eq!(params["mcpServers"], json!([]));
    }

    #[test]
    fn send_session_new_includes_mode_and_model_when_set() {
        let (session, rx) = test_session();
        let mut session = session;
        session.selected_mode_id = Some("build".to_string());
        session
            .config_options
            .insert("model".to_string(), "gpt-4".to_string());
        session.send_session_new();
        let msg = rx.recv().unwrap();
        let parsed: JsonValue = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed["params"]["modeId"], "build");
        assert_eq!(parsed["params"]["model"], "gpt-4");
    }

    #[test]
    fn send_prompt_blocked_when_session_id_missing() {
        let (session, rx) = test_session();
        session.send_prompt("hello");
        // Should not send anything because session_id is None
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn send_prompt_sends_when_session_id_present() {
        let (mut session, rx) = test_session();
        session.session_id = Some("sess-abc".to_string());
        session.send_prompt("hello");
        let msg = rx.recv().unwrap();
        let parsed: JsonValue = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed["method"], "session/prompt");
        assert_eq!(parsed["params"]["sessionId"], "sess-abc");
    }

    #[test]
    fn parse_session_new_response_with_config_options() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "sessionId": "sess-123",
                "configOptions": [
                    { "configId": "model", "value": "gpt-4" },
                    { "configId": "mode", "value": "build" }
                ]
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev1 = rx.recv().unwrap();
        assert!(
            matches!(ev1, AcpChatEvent::SessionCreated { session_id, .. } if session_id == "sess-123")
        );
        let ev2 = rx.recv().unwrap();
        assert!(
            matches!(ev2, AcpChatEvent::ConfigOptionUpdate { category, value, .. } if category == "model" && value == "gpt-4")
        );
        let ev3 = rx.recv().unwrap();
        assert!(
            matches!(ev3, AcpChatEvent::ConfigOptionUpdate { category, value, .. } if category == "mode" && value == "build")
        );
    }

    #[test]
    fn parse_current_mode_update_uses_current_mode_id() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "sess-123",
                "update": {
                    "sessionUpdate": "current_mode_update",
                    "currentModeId": "build"
                }
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev = rx.recv().unwrap();
        assert!(
            matches!(ev, AcpChatEvent::CurrentModeUpdate { mode_id, .. } if mode_id == "build")
        );
    }

    #[test]
    fn parse_available_commands_update_uses_available_commands() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "sess-123",
                "update": {
                    "sessionUpdate": "available_commands_update",
                    "availableCommands": [
                        { "name": "gt", "description": "Git push" }
                    ]
                }
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev = rx.recv().unwrap();
        assert!(
            matches!(ev, AcpChatEvent::AvailableCommandsUpdate { commands, .. } if commands.len() == 1 && commands[0].name == "gt")
        );
    }

    #[test]
    fn parse_config_option_update_uses_config_options_array() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "sess-123",
                "update": {
                    "sessionUpdate": "config_option_update",
                    "configOptions": [
                        { "configId": "model", "value": "gpt-4o" }
                    ]
                }
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev = rx.recv().unwrap();
        assert!(
            matches!(ev, AcpChatEvent::ConfigOptionUpdate { category, value, .. } if category == "model" && value == "gpt-4o")
        );
    }

    #[test]
    fn parse_permission_request_with_string_id() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": "req-abc",
            "method": "session/request_permission",
            "params": {
                "sessionId": "sess-123",
                "options": [
                    { "optionId": "once", "name": "Allow once", "kind": "allow_once" }
                ],
                "toolCall": {
                    "toolCallId": "tc-1",
                    "title": "edit",
                    "kind": "edit"
                }
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev = rx.recv().unwrap();
        assert!(
            matches!(ev, AcpChatEvent::PermissionRequest { request_id, .. } if request_id == "req-abc")
        );
    }

    #[test]
    fn parse_permission_request_with_numeric_id() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 42,
            "method": "session/request_permission",
            "params": {
                "sessionId": "sess-123",
                "options": [
                    { "optionId": "once", "name": "Allow once", "kind": "allow_once" }
                ],
                "toolCall": {
                    "toolCallId": "tc-1",
                    "title": "edit",
                    "kind": "edit"
                }
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev = rx.recv().unwrap();
        assert!(
            matches!(ev, AcpChatEvent::PermissionRequest { request_id, .. } if request_id == "42")
        );
    }

    #[test]
    fn acp_spawn_opencode_acp_uses_custom_bin_path() {
        let (tx, _rx) = crossbeam_channel::unbounded::<AcpChatEvent>();
        let path = if cfg!(windows) {
            std::env::var("SystemRoot").unwrap_or("C:\\Windows".to_string()) + "\\System32\\cmd.exe"
        } else {
            "/bin/sh".to_string()
        };
        let bin = std::ffi::OsString::from(path);
        let result = spawn_opencode_acp(1, 1, PathBuf::from("test"), Some(bin), None, vec![], tx);
        assert!(result.is_ok());
    }
}
