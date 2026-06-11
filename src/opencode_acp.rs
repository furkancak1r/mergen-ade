use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use serde_json::{json, Value as JsonValue};

pub const ACP_LOOP_WARNING_TOOL_CALLS: usize = 20;
pub const ACP_LOOP_LIMIT_TOOL_CALLS: usize = 32;

/// Which agent backend powers this ACP chat session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpBackend {
    OpenCode,
    ClaudeCode,
}

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
        option: AcpConfigOption,
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
pub struct AcpConfigOptionEntry {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AcpConfigOption {
    pub id: String,
    pub name: String,
    pub category: String,
    pub current_value: String,
    pub options: Vec<AcpConfigOptionEntry>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpQueuedPrompt {
    pub id: u64,
    pub draft_text: String,
    pub attachments: Vec<String>,
    pub prompt_text: String,
    pub mode_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpQueueDraftReturn {
    pub index: usize,
    pub original_prompt: AcpQueuedPrompt,
}

/// A single OpenCode ACP session with the OpenCode agent.
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
    pub config_options_struct: Vec<AcpConfigOption>,
    pub modes: Vec<AcpMode>,
    pub available_commands: Vec<AcpCommand>,
    pub messages: Vec<AcpChatMessage>,
    pub pending_permission: Option<AcpPendingPermission>,
    pub prompt_input: String,
    pub attachments: Vec<String>,
    pub is_running: bool,
    pub show_thread_selector: bool,
    pub selected_mode_id: Option<String>,
    pub queue: Vec<AcpQueuedPrompt>,
    pub next_queued_prompt_id: u64,
    pub queue_expanded: bool,
    pub queue_draft_return: Option<AcpQueueDraftReturn>,
    pub queue_scroll_to_end: bool,
    pub model_search_query: String,
    pub recent_inputs: Vec<String>,
    pub history_index: Option<usize>,
    pub history_draft: String,
    pub tool_calls_this_turn: usize,
    pub loop_warning_emitted: bool,
    pub loop_limit_emitted: bool,
    pub cancel_error_suppression_until: Option<Instant>,
    pub cancel_unsupported: bool,
    command_tx: crossbeam_channel::Sender<String>,
    #[allow(dead_code)]
    process: Option<Child>,
    #[allow(dead_code)]
    writer_thread: Option<thread::JoinHandle<()>>,
    #[allow(dead_code)]
    reader_thread: Option<thread::JoinHandle<()>>,
    #[allow(dead_code)]
    stderr_thread: Option<thread::JoinHandle<()>>,
    /// Which backend powers this session.
    pub backend: AcpBackend,
    /// Event sender for Claude Code per-turn process spawning.
    pub event_tx: Option<crossbeam_channel::Sender<AcpChatEvent>>,
    /// Shared handle to the current Claude Code child process (for kill support).
    pub claude_child: Option<Arc<Mutex<Option<Child>>>>,
}

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

impl AcpChatSession {
    /// Create a terminal-less Claude Code chat session. Claude Code does not
    /// use the JSON-RPC writer loop; prompts spawn one `claude --print` turn.
    pub fn new_claude_code(
        id: u64,
        project_id: u64,
        project_path: PathBuf,
        event_tx: crossbeam_channel::Sender<AcpChatEvent>,
    ) -> Self {
        let (command_tx, _command_rx) = crossbeam_channel::unbounded::<String>();
        let now = Instant::now();
        Self {
            id,
            project_id,
            project_path,
            title: "Claude Code".to_owned(),
            created_at: now,
            updated_at: now,
            status: AcpChatStatus::Idle,
            session_id: None,
            config_options: BTreeMap::new(),
            config_options_struct: Vec::new(),
            modes: Vec::new(),
            available_commands: Vec::new(),
            messages: Vec::new(),
            pending_permission: None,
            prompt_input: String::new(),
            attachments: Vec::new(),
            is_running: false,
            show_thread_selector: false,
            selected_mode_id: None,
            queue: Vec::new(),
            next_queued_prompt_id: 1,
            queue_expanded: true,
            queue_draft_return: None,
            queue_scroll_to_end: false,
            model_search_query: String::new(),
            recent_inputs: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            tool_calls_this_turn: 0,
            loop_warning_emitted: false,
            loop_limit_emitted: false,
            cancel_error_suppression_until: None,
            cancel_unsupported: false,
            command_tx,
            process: None,
            writer_thread: None,
            reader_thread: None,
            stderr_thread: None,
            backend: AcpBackend::ClaudeCode,
            event_tx: Some(event_tx),
            claude_child: None,
        }
    }

    pub fn is_claude_code(&self) -> bool {
        self.backend == AcpBackend::ClaudeCode
    }

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
        let prompt_parts = vec![json!({ "type": "text", "text": text })];
        let params = json!({
            "sessionId": session_id,
            "prompt": prompt_parts
        });
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/prompt",
            "params": params
        });
        let _ = self.command_tx.send(msg.to_string());
    }

    /// Send a session/set_config_option request.
    pub fn send_set_config_option(&self, config_id: &str, value: &str) {
        let session_id = match self.session_id {
            Some(ref id) => id.clone(),
            None => return,
        };
        let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/set_config_option",
            "params": {
                "sessionId": session_id,
                "configId": config_id,
                "value": value
            }
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

    /// Look up a structured config option by id.
    pub fn config_option(&self, id: &str) -> Option<&AcpConfigOption> {
        self.config_options_struct.iter().find(|o| o.id == id)
    }

    /// Determine the active mode id using fallback sources when the config option
    /// has not yet arrived (e.g. during ACP startup).
    pub fn active_mode_id_or_default(&self) -> String {
        self.active_mode_id_or("build")
    }

    pub fn active_mode_id_or(&self, fallback: &str) -> String {
        if let Some(mode_opt) = self.config_option("mode") {
            mode_opt.current_value.clone()
        } else if let Some(mode_id) = self.config_options.get("mode") {
            mode_id.clone()
        } else if let Some(ref mode_id) = self.selected_mode_id {
            mode_id.clone()
        } else if let Some(mode) = self.modes.last() {
            mode.id.clone()
        } else {
            fallback.to_string()
        }
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

#[cfg(test)]
pub(crate) fn test_session_for_app(
    chat_id: u64,
    project_id: u64,
    session_id: Option<String>,
) -> (AcpChatSession, crossbeam_channel::Receiver<String>) {
    let (tx, rx) = crossbeam_channel::unbounded::<String>();
    let status = if session_id.is_some() {
        AcpChatStatus::Idle
    } else {
        AcpChatStatus::Starting
    };
    (
        AcpChatSession {
            id: chat_id,
            project_id,
            project_path: PathBuf::from("C:/test/project"),
            title: "OpenCode ACP".to_owned(),
            created_at: Instant::now(),
            updated_at: Instant::now(),
            status,
            session_id,
            config_options: BTreeMap::new(),
            config_options_struct: Vec::new(),
            modes: Vec::new(),
            available_commands: Vec::new(),
            messages: Vec::new(),
            pending_permission: None,
            prompt_input: String::new(),
            attachments: Vec::new(),
            is_running: false,
            show_thread_selector: false,
            selected_mode_id: None,
            queue: Vec::new(),
            next_queued_prompt_id: 1,
            queue_expanded: true,
            queue_draft_return: None,
            queue_scroll_to_end: false,
            model_search_query: String::new(),
            recent_inputs: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            tool_calls_this_turn: 0,
            loop_warning_emitted: false,
            loop_limit_emitted: false,
            cancel_error_suppression_until: None,
            cancel_unsupported: false,
            command_tx: tx,
            process: None,
            writer_thread: None,
            reader_thread: None,
            stderr_thread: None,
            backend: AcpBackend::OpenCode,
            event_tx: None,
            claude_child: None,
        },
        rx,
    )
}

/// Build the final prompt text from user text and attachment paths.
/// Only file paths are appended; file contents are never read or injected.
/// This keeps the AI context lean and lets the agent read files on demand.
pub fn build_acp_prompt_text(text: &str, attachments: &[String]) -> String {
    if attachments.is_empty() {
        text.to_string()
    } else {
        let att_text = format!("Attached file paths:\n{}", attachments.join("\n"));
        if text.is_empty() {
            att_text
        } else {
            format!("{}\n\n{}", text, att_text)
        }
    }
}

/// Extract the file name from a path and return as `@file_name`.
/// The file name is run through mojibake repair so Turkish characters
/// (and other CP1252-corrupted text) display correctly.
pub fn path_to_mention(path: &str) -> String {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    let file_name = crate::mojibake::repair_mojibake_display(file_name);
    format!("@{}", file_name)
}

/// Append attachment mentions to the composer input text.
/// If input is empty, returns the mentions directly.
/// Otherwise, appends a space followed by the mentions.
pub fn append_mentions_to_input(input: &str, paths: &[String]) -> String {
    if paths.is_empty() {
        return input.to_string();
    }
    let mentions: Vec<String> = paths.iter().map(|p| path_to_mention(p)).collect();
    let mentions_text = mentions.join(" ");
    if input.is_empty() {
        mentions_text
    } else {
        format!("{} {}", input, mentions_text)
    }
}

/// Remove the last occurrence of a mention derived from `path` from the input text.
/// Returns the original text if the mention is not found.
pub fn remove_mention_from_input(input: &str, path: &str) -> String {
    let mention = path_to_mention(path);
    // Try to find the last occurrence of the mention, optionally preceded by a space.
    if let Some(pos) = input.rfind(&mention) {
        let before = &input[..pos];
        let after = &input[pos + mention.len()..];
        // If mention is preceded by a space and not at the start, remove the preceding space too.
        if !before.is_empty() && before.ends_with(' ') {
            return format!("{}{}", &before[..before.len() - 1], after);
        }
        return format!("{}{}", before, after);
    }
    input.to_string()
}

/// Return a human-readable display name for a mode id.
/// Falls back to the raw id if the mode is unrecognized.
pub fn mode_display_name(mode_id: &str) -> String {
    match mode_id {
        "plan" => "Plan".to_string(),
        "build" => "Default".to_string(),
        _ => mode_id.to_string(),
    }
}

/// Whether the given mode id is considered "plan".
pub fn mode_is_plan(mode_id: &str) -> bool {
    mode_id == "plan"
}

/// Spawn a new OpenCode ACP session.
pub fn spawn_opencode_acp(
    chat_id: u64,
    project_id: u64,
    project_path: PathBuf,
    opencode_bin: Option<std::ffi::OsString>,
    runtime_defaults: Option<crate::opencode_config::OpenCodeRuntimeDefaults>,
    startup_mode_id: String,
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
    if let Some(defaults) = runtime_defaults.as_ref() {
        command.env("OPENCODE_CONFIG_CONTENT", defaults.to_env_content_string());
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
                "name": "opencode-local-acp",
                "title": "OpenCode",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });
    let _ = command_tx.send(init_msg.to_string());
    let now = Instant::now();
    let mut config_options = BTreeMap::new();
    config_options.insert("mode".to_owned(), startup_mode_id.clone());
    if let Some(defaults) = runtime_defaults.as_ref() {
        config_options.insert(
            "model".to_owned(),
            defaults.desired_model_for_mode(&startup_mode_id).to_owned(),
        );
        if mode_is_plan(&startup_mode_id) {
            config_options.insert("effort".to_owned(), defaults.plan_effort.clone());
        }
    }
    Ok(AcpChatSession {
        id: chat_id,
        project_id,
        project_path,
        title: "OpenCode ACP".to_owned(),
        created_at: now,
        updated_at: now,
        status: AcpChatStatus::Starting,
        messages: Vec::new(),
        pending_permission: None,
        prompt_input: String::new(),
        attachments: Vec::new(),
        is_running: false,
        session_id: None,
        config_options,
        config_options_struct: Vec::new(),
        modes: Vec::new(),
        available_commands: Vec::new(),
        queue: Vec::new(),
        next_queued_prompt_id: 1,
        queue_expanded: true,
        queue_draft_return: None,
        queue_scroll_to_end: false,
        show_thread_selector: false,
        selected_mode_id: Some(startup_mode_id),
        model_search_query: String::new(),
        recent_inputs: Vec::new(),
        history_index: None,
        history_draft: String::new(),
        tool_calls_this_turn: 0,
        loop_warning_emitted: false,
        loop_limit_emitted: false,
        cancel_error_suppression_until: None,
        cancel_unsupported: false,
        command_tx,
        process: Some(child),
        writer_thread: Some(writer_thread),
        reader_thread: Some(reader_thread),
        stderr_thread: Some(stderr_thread),
        backend: AcpBackend::OpenCode,
        event_tx: None,
        claude_child: None,
    })
}

fn parse_config_option(opt: &JsonValue) -> Option<AcpConfigOption> {
    let id = opt.get("id").and_then(JsonValue::as_str)?.to_string();
    let name = opt
        .get("name")
        .and_then(JsonValue::as_str)
        .unwrap_or(&id)
        .to_string();
    let category = opt
        .get("category")
        .and_then(JsonValue::as_str)
        .unwrap_or(&id)
        .to_string();
    let current_value = opt
        .get("currentValue")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();
    let options = opt
        .get("options")
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|o| {
                    let value = o.get("value").and_then(JsonValue::as_str)?.to_string();
                    let entry_name = o
                        .get("name")
                        .and_then(JsonValue::as_str)
                        .unwrap_or(&value)
                        .to_string();
                    let description = o
                        .get("description")
                        .and_then(JsonValue::as_str)
                        .map(|s| s.to_string());
                    Some(AcpConfigOptionEntry {
                        value,
                        name: entry_name,
                        description,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(AcpConfigOption {
        id,
        name,
        category,
        current_value,
        options,
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
                            if let Some(option) = parse_config_option(opt) {
                                let _ = event_tx.send(AcpChatEvent::ConfigOptionUpdate {
                                    chat_id,
                                    option: option.clone(),
                                });
                            }
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
                    // OpenCode ACP spec sends configOptions as an array of full option objects
                    if let Some(options) = update.get("configOptions").and_then(JsonValue::as_array)
                    {
                        for opt in options {
                            if let Some(option) = parse_config_option(opt) {
                                let _ = event_tx.send(AcpChatEvent::ConfigOptionUpdate {
                                    chat_id,
                                    option: option.clone(),
                                });
                            }
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
                            option: AcpConfigOption {
                                id: category.clone(),
                                name: category.clone(),
                                category: category.clone(),
                                current_value: value.clone(),
                                options: Vec::new(),
                            },
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

    /// Build a test-only ACP session without spawning a real process.
    fn test_acp_session(
        chat_id: u64,
        project_id: u64,
        status: AcpChatStatus,
        session_id: Option<String>,
    ) -> AcpChatSession {
        let (tx, _rx) = crossbeam_channel::unbounded::<String>();
        AcpChatSession {
            id: chat_id,
            project_id,
            project_path: PathBuf::from("C:/test/project"),
            title: "OpenCode ACP".to_owned(),
            created_at: Instant::now(),
            updated_at: Instant::now(),
            status,
            session_id: session_id.clone(),
            config_options: BTreeMap::new(),
            config_options_struct: Vec::new(),
            modes: Vec::new(),
            available_commands: Vec::new(),
            messages: Vec::new(),
            pending_permission: None,
            prompt_input: String::new(),
            attachments: Vec::new(),
            is_running: false,
            show_thread_selector: false,
            selected_mode_id: None,
            model_search_query: String::new(),
            recent_inputs: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            queue: Vec::new(),
            next_queued_prompt_id: 1,
            queue_expanded: true,
            queue_draft_return: None,
            queue_scroll_to_end: false,
            tool_calls_this_turn: 0,
            loop_warning_emitted: false,
            loop_limit_emitted: false,
            cancel_error_suppression_until: None,
            cancel_unsupported: false,
            command_tx: tx,
            process: None,
            writer_thread: None,
            reader_thread: None,
            stderr_thread: None,
            backend: AcpBackend::OpenCode,
            event_tx: None,
            claude_child: None,
        }
    }

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
            config_options_struct: Vec::new(),
            modes: Vec::new(),
            available_commands: Vec::new(),
            queue: Vec::new(),
            next_queued_prompt_id: 1,
            queue_expanded: true,
            queue_draft_return: None,
            queue_scroll_to_end: false,
            messages: Vec::new(),
            pending_permission: None,
            prompt_input: String::new(),
            attachments: Vec::new(),
            is_running: false,
            show_thread_selector: false,
            selected_mode_id: None,
            model_search_query: String::new(),
            recent_inputs: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            tool_calls_this_turn: 0,
            loop_warning_emitted: false,
            loop_limit_emitted: false,
            cancel_error_suppression_until: None,
            cancel_unsupported: false,
            command_tx: tx,
            process: None,
            writer_thread: None,
            reader_thread: None,
            stderr_thread: None,
            backend: AcpBackend::OpenCode,
            event_tx: None,
            claude_child: None,
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
    fn send_prompt_does_not_append_attachments() {
        let (mut session, rx) = test_session();
        session.session_id = Some("sess-abc".to_string());
        session.attachments = vec!["C:/test/file.txt".to_string()];
        session.send_prompt("hello");
        let msg = rx.recv().unwrap();
        let parsed: JsonValue = serde_json::from_str(&msg).unwrap();
        let prompt = parsed["params"]["prompt"].as_array().unwrap();
        assert_eq!(prompt.len(), 1);
        assert_eq!(prompt[0]["text"], "hello");
    }

    #[test]
    fn build_acp_prompt_text_with_text_and_attachments() {
        let attachments = vec![
            "C:/test/file.txt".to_string(),
            "D:/test/file2.png".to_string(),
        ];
        let result = build_acp_prompt_text("hello", &attachments);
        assert_eq!(
            result,
            "hello\n\nAttached file paths:\nC:/test/file.txt\nD:/test/file2.png"
        );
    }

    #[test]
    fn build_acp_prompt_text_with_attachments_only() {
        let attachments = vec!["C:/test/file.txt".to_string()];
        let result = build_acp_prompt_text("", &attachments);
        assert_eq!(result, "Attached file paths:\nC:/test/file.txt");
    }

    #[test]
    fn build_acp_prompt_text_with_text_only() {
        let result = build_acp_prompt_text("hello", &[]);
        assert_eq!(result, "hello");
    }

    #[test]
    fn build_acp_prompt_text_no_duplicate_attachments() {
        let attachments = vec!["C:/test/file.txt".to_string()];
        let result = build_acp_prompt_text("hello", &attachments);
        assert!(result.contains("hello"));
        assert!(result.contains("Attached file paths:"));
        assert!(result.contains("C:/test/file.txt"));
        // Should only contain the attachment path once
        let count = result.matches("C:/test/file.txt").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn path_to_mention_windows_path() {
        assert_eq!(path_to_mention("C:/test/file.rs"), "@file.rs");
    }

    #[test]
    fn path_to_mention_unicode_file_name() {
        assert_eq!(path_to_mention("/home/user/şema.rs"), "@şema.rs");
    }

    #[test]
    fn append_mentions_to_input_empty() {
        let paths = vec!["C:/test/foo.rs".to_string()];
        assert_eq!(append_mentions_to_input("", &paths), "@foo.rs");
    }

    #[test]
    fn append_mentions_to_input_existing() {
        let paths = vec!["C:/test/foo.rs".to_string()];
        assert_eq!(
            append_mentions_to_input("bunu incele", &paths),
            "bunu incele @foo.rs"
        );
    }

    #[test]
    fn append_mentions_to_input_multiple() {
        let paths = vec!["C:/test/a.rs".to_string(), "D:/test/b.rs".to_string()];
        assert_eq!(append_mentions_to_input("", &paths), "@a.rs @b.rs");
    }

    #[test]
    fn append_mentions_to_input_existing_multiple() {
        let paths = vec!["C:/test/a.rs".to_string(), "D:/test/b.rs".to_string()];
        assert_eq!(
            append_mentions_to_input("incele", &paths),
            "incele @a.rs @b.rs"
        );
    }

    #[test]
    fn remove_mention_from_input_found() {
        let result = remove_mention_from_input("bunu incele @foo.rs", "C:/test/foo.rs");
        assert_eq!(result, "bunu incele");
    }

    #[test]
    fn remove_mention_from_input_not_found() {
        let result = remove_mention_from_input("bunu incele", "C:/test/foo.rs");
        assert_eq!(result, "bunu incele");
    }

    #[test]
    fn remove_mention_from_input_first_position() {
        let result = remove_mention_from_input("@foo.rs bunu incele", "C:/test/foo.rs");
        assert_eq!(result, " bunu incele");
    }

    #[test]
    fn remove_mention_from_input_multiple() {
        let result = remove_mention_from_input("@a.rs @b.rs @a.rs", "C:/test/a.rs");
        assert_eq!(result, "@a.rs @b.rs");
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
                    { "id": "model", "name": "Model", "category": "model", "currentValue": "gpt-4", "options": [{ "value": "gpt-4", "name": "GPT-4" }] },
                    { "id": "mode", "name": "Session Mode", "category": "mode", "currentValue": "build", "options": [{ "value": "build", "name": "Build" }] }
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
            matches!(ev2, AcpChatEvent::ConfigOptionUpdate { option, .. } if option.id == "model" && option.current_value == "gpt-4")
        );
        let ev3 = rx.recv().unwrap();
        assert!(
            matches!(ev3, AcpChatEvent::ConfigOptionUpdate { option, .. } if option.id == "mode" && option.current_value == "build")
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
                        { "id": "model", "name": "Model", "category": "model", "currentValue": "gpt-4o", "options": [{ "value": "gpt-4o", "name": "GPT-4o" }] }
                    ]
                }
            }
        });
        parse_acp_message(1, msg, &tx);
        let ev = rx.recv().unwrap();
        assert!(
            matches!(ev, AcpChatEvent::ConfigOptionUpdate { option, .. } if option.id == "model" && option.current_value == "gpt-4o")
        );
    }

    #[test]
    fn send_set_config_option_sends_correct_json_rpc() {
        let (session, rx) = test_session();
        let mut session = session;
        session.session_id = Some("sess-abc".to_string());
        session.send_set_config_option("mode", "plan");
        let msg = rx.recv().unwrap();
        let parsed: JsonValue = serde_json::from_str(&msg).unwrap();
        assert_eq!(parsed["method"], "session/set_config_option");
        assert_eq!(parsed["params"]["sessionId"], "sess-abc");
        assert_eq!(parsed["params"]["configId"], "mode");
        assert_eq!(parsed["params"]["value"], "plan");
    }

    #[test]
    fn config_option_helper_returns_correct_option() {
        let (mut session, _rx) = test_session();
        session.config_options_struct.push(AcpConfigOption {
            id: "model".to_string(),
            name: "Model".to_string(),
            category: "model".to_string(),
            current_value: "gpt-4".to_string(),
            options: Vec::new(),
        });
        assert_eq!(
            session.config_option("model").unwrap().current_value,
            "gpt-4"
        );
        assert!(session.config_option("mode").is_none());
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
        let result = spawn_opencode_acp(
            1,
            1,
            PathBuf::from("test"),
            Some(bin),
            None,
            "build".to_string(),
            vec![],
            tx,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn active_mode_id_or_default_fallback_to_build() {
        let (session, _rx) = test_session();
        assert_eq!(session.active_mode_id_or_default(), "build");
    }

    #[test]
    fn active_mode_id_or_uses_supplied_startup_fallback() {
        let (session, _rx) = test_session();
        assert_eq!(session.active_mode_id_or("plan"), "plan");
    }

    #[test]
    fn active_mode_id_or_default_uses_selected_mode_id() {
        let (mut session, _rx) = test_session();
        session.selected_mode_id = Some("plan".to_string());
        assert_eq!(session.active_mode_id_or_default(), "plan");
    }

    #[test]
    fn active_mode_id_or_default_prefers_config_option() {
        let (mut session, _rx) = test_session();
        session.selected_mode_id = Some("plan".to_string());
        session.config_options_struct.push(AcpConfigOption {
            id: "mode".to_string(),
            name: "Mode".to_string(),
            category: "mode".to_string(),
            current_value: "build".to_string(),
            options: Vec::new(),
        });
        assert_eq!(session.active_mode_id_or_default(), "build");
    }

    #[test]
    fn active_mode_id_or_default_uses_last_mode() {
        let (mut session, _rx) = test_session();
        session.modes.push(AcpMode {
            id: "plan".to_string(),
            name: "Plan".to_string(),
        });
        assert_eq!(session.active_mode_id_or_default(), "plan");
    }

    #[test]
    fn active_mode_id_or_default_uses_startup_mode_preseed() {
        let (mut session, _rx) = test_session();
        // When selected_mode_id is preseed at spawn time it wins over the hardcoded fallback
        session.selected_mode_id = Some("plan".to_string());
        assert_eq!(session.active_mode_id_or_default(), "plan");
    }

    #[test]
    fn mode_display_name_plan() {
        assert_eq!(mode_display_name("plan"), "Plan");
    }

    #[test]
    fn mode_display_name_build() {
        assert_eq!(mode_display_name("build"), "Default");
    }

    #[test]
    fn mode_display_name_unknown() {
        assert_eq!(mode_display_name("custom"), "custom");
    }

    #[test]
    fn mode_is_plan_true() {
        assert!(mode_is_plan("plan"));
    }

    #[test]
    fn mode_is_plan_false() {
        assert!(!mode_is_plan("build"));
    }
}
