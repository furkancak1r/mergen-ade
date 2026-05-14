//! Web mode protocol types for Mergen ADE.
//!
//! Defines the JSON message envelope used over WebSocket and REST.
//! Terminal output is sent as binary WebSocket frames, not JSON:
//!   first 8 bytes = terminal_id (little-endian u64)
//!   remaining bytes = raw PTY data

use serde::{Deserialize, Serialize};

/// Top-level envelope sent from backend to frontend over WebSocket (JSON only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ServerMessage {
    Hello {
        version: String,
        auth_required: bool,
    },
    StateSnapshot {
        projects: Vec<WebProject>,
        terminals: Vec<WebTerminal>,
        active_terminal_id: Option<u64>,
        selected_project_id: Option<u64>,
    },
    StatePatch {
        updates: Vec<StatePatchUpdate>,
    },
    TerminalOutput {
        terminal_id: u64,
        data: Vec<u8>,
    },
    TerminalStatus {
        terminal_id: u64,
        title: String,
        exited: bool,
        ai_tool: Option<String>,
        ai_status: Option<String>,
    },
    DirectoryIndex {
        project_id: u64,
        root: WebDirectoryNode,
    },
    SourceControl {
        project_id: u64,
        branch: String,
        ahead: usize,
        behind: usize,
        files: Vec<WebSourceControlFile>,
    },
    BrowserEvent {
        scope: WebBrowserScope,
        event: WebBrowserEvent,
    },
    Error {
        message: String,
    },
}

/// Individual state patch entry for incremental updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum StatePatchUpdate {
    ProjectAdded { project: WebProject },
    ProjectRemoved { project_id: u64 },
    ProjectSelected { project_id: Option<u64> },
    TerminalAdded { terminal: WebTerminal },
    TerminalRemoved { terminal_id: u64 },
    TerminalUpdated { terminal: WebTerminal },
    ActiveTerminalChanged { terminal_id: Option<u64> },
    StatusLine { text: String },
}

/// Top-level envelope sent from frontend to backend over WebSocket (JSON only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ClientMessage {
    Auth { token: String },
    SpawnTerminal {
        project_id: u64,
        shell: String,
        terminal_kind: String, // "foreground" | "background"
    },
    TerminalInput {
        terminal_id: u64,
        data: Vec<u8>,
    },
    TerminalPaste {
        terminal_id: u64,
        text: String,
    },
    TerminalResize {
        terminal_id: u64,
        cols: u16,
        lines: u16,
    },
    CloseTerminal {
        terminal_id: u64,
    },
    SelectProject {
        project_id: u64,
    },
    AddProject {
        name: String,
        path: String,
    },
    RemoveProject {
        project_id: u64,
    },
    RequestDirectoryIndex {
        project_id: u64,
    },
    RequestSourceControl {
        project_id: u64,
    },
    BrowserNavigate {
        scope: WebBrowserScope,
        url: String,
    },
    BrowserAction {
        scope: WebBrowserScope,
        action: WebBrowserClientAction,
    },
    SendShortcut {
        terminal_id: u64,
        command: String,
    },
    SmartInputSubmit {
        terminal_id: u64,
        text: String,
        mode: String, // "steer_now" | "after_done"
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebProject {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub is_worktree: bool,
    pub repo_root: Option<String>,
    pub saved_messages: Vec<String>,
    pub foreground_saved_messages: Vec<String>,
    pub browser_last_url: Option<String>,
    pub checklist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebTerminal {
    pub id: u64,
    pub project_id: u64,
    pub kind: String,
    pub shell: String,
    pub title: String,
    pub exited: bool,
    pub in_main_view: bool,
    pub ai_tool: Option<String>,
    pub ai_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDirectoryNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_deferred: bool,
    pub children: Vec<WebDirectoryNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSourceControlFile {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebBrowserScope {
    pub project_id: u64,
    pub terminal_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum WebBrowserEvent {
    UrlChanged { url: String },
    LoadStarted { url: String },
    LoadFinished { url: String },
    Screenshot { format: String, base64: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum WebBrowserClientAction {
    Click { x: i32, y: i32 },
    Type { text: String },
    PressKey { key: String },
    Screenshot { full_page: bool },
    GoBack,
    GoForward,
    Reload,
    SetVideoPlaybackRate { rate: f64 },
}

/// REST API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Config response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub default_shell: String,
    pub launchers: Vec<WebLauncher>,
    pub shortcuts: Vec<WebShortcut>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebLauncher {
    pub id: String,
    pub display_name: String,
    pub command: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebShortcut {
    pub id: String,
    pub label: String,
    pub key: String,
    pub command: String,
    pub enabled: bool,
}

/// Encode a terminal_id + raw data into a single binary payload.
/// Format: [u64 terminal_id LE][data...]
pub fn encode_terminal_binary(terminal_id: u64, data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(&terminal_id.to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

/// Decode the terminal_id from the start of a binary payload.
pub fn decode_terminal_binary_header(data: &[u8]) -> Option<(u64, &[u8])> {
    if data.len() < 8 {
        return None;
    }
    let mut id_bytes = [0u8; 8];
    id_bytes.copy_from_slice(&data[..8]);
    let terminal_id = u64::from_le_bytes(id_bytes);
    Some((terminal_id, &data[8..]))
}
