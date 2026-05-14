//! Web server module for Mergen ADE.
//!
//! Provides a standalone `--web` mode that serves a web UI over HTTP/WebSocket
//! without launching the native eframe/egui desktop window.
//!
//! The web mode reuses existing business logic (config, projects, terminal runtime,
//! source control, directory indexing) and exposes it via a JSON/WebSocket protocol.
//!
//! Architecture note: `TerminalRuntime` is not `Send` because it wraps
//! `tattoy_wezterm_term::Terminal` inside `Arc<Mutex<_>>`. Therefore all
//! `TerminalRuntime` instances live in a dedicated background thread and are
//! accessed only through message passing.

use crate::config;
use crate::hooks::AiHookManager;
use crate::models::{AppConfig, ProjectRecord, ShellKind, TerminalKind};
use crate::terminal::{TerminalDimensions, TerminalRuntime, TerminalUiEvent, TerminalUiEventKind, try_terminal_snapshots};
use crate::web_protocol::*;
use crate::path_utils;

use axum::{
    extract::{ws::{Message as WsMessage, WebSocket, WebSocketUpgrade}, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use futures::stream::StreamExt;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

const WEB_SERVER_DEFAULT_PORT: u16 = 8765;
const WEB_AUTH_TOKEN_ENV_VAR: &str = "MERGEN_WEB_AUTH_TOKEN";
const WEB_PORT_ENV_VAR: &str = "MERGEN_WEB_PORT";
const TERMINAL_EVENT_QUEUE_CAPACITY: usize = 1024;
const TERMINAL_CMD_CAPACITY: usize = 256;

/// Embedded web UI assets. The `web-dist` folder must exist at compile time.
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

/// Run the web mode server. This blocks until the server shuts down.
pub fn run_web_mode() -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("tokio runtime failed: {e}"))?;
    rt.block_on(run_web_server())
}

async fn run_web_server() -> Result<(), String> {
    let token = web_auth_token();
    let port = web_server_port();

    // Build shared state (projects, config, etc. - all Send + Sync)
    let (shared_inner, events_rx) = SharedState::bootstrap(token.clone())?;
    let shared = Arc::new(Mutex::new(shared_inner));

    // Build terminal manager handle (owns the non-Send TerminalRuntimes in a bg thread)
    let tm_handle = TerminalManagerHandle::spawn(shared.clone(), events_rx);

    // Build router
    let app_state = AppState {
        shared: shared.clone(),
        tm_handle: tm_handle.clone(),
        token: token.clone(),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .route("/api/config", get(api_config_handler))
        .route("/api/projects", get(api_projects_handler))
        .route("/api/projects", post(api_add_project_handler))
        .route("/api/terminals", get(api_terminals_handler))
        .route("/api/terminals", post(api_spawn_terminal_handler))
        .route("/api/directory", get(api_directory_handler))
        .route("/api/source-control", get(api_source_control_handler))
        .route("/api/ws", get(ws_handler))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    log::info!("Mergen web mode listening on http://{}", addr);
    eprintln!("Mergen web mode: open http://{} in your browser", addr);
    if !token.is_empty() {
        eprintln!("Auth token: {}", token);
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("bind failed: {e}"))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))?;

    // Signal terminal manager to shut down
    let _ = tm_handle.shutdown_tx.send(());

    Ok(())
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------
#[derive(Clone)]
struct AppState {
    shared: Arc<Mutex<SharedState>>,
    tm_handle: TerminalManagerHandle,
    token: String,
}

// ---------------------------------------------------------------------------
// SharedState: everything that is Send + Sync (projects, config, metadata)
// ---------------------------------------------------------------------------
struct SharedState {
    token: String,
    config_path: PathBuf,
    config: AppConfig,
    projects: BTreeMap<u64, ProjectRecord>,
    next_project_id: u64,
    next_terminal_id: u64,
    selected_project: Option<u64>,
    active_terminal: Option<u64>,
    terminal_events_tx: Sender<TerminalUiEvent>,
    ai_hook_manager: Option<Arc<AiHookManager>>,
    broadcast_tx: broadcast::Sender<ServerMessage>,
}

impl SharedState {
    fn bootstrap(token: String) -> Result<(Self, Receiver<TerminalUiEvent>), String> {
        let config_path = config::config_path().unwrap_or_else(|_| PathBuf::from("config.toml"));
        let (mut config, _config_load_error) = match config::load_config(&config_path) {
            Ok(c) => (c, None),
            Err(err) => (AppConfig::default(), Some(err.to_string())),
        };

        let blank_id = 0u64;
        let blank_exists = config.projects.iter().any(|p| p.id == blank_id);
        if !blank_exists {
            if let Some(path) = default_blank_project_path() {
                config.projects.push(ProjectRecord {
                    id: blank_id,
                    name: "Blank".to_owned(),
                    path,
                    saved_messages: Vec::new(),
                    ai_config: crate::hooks::ProjectAiConfig::default(),
                    checklist: Vec::new(),
                    browser_last_url: None,
                    foreground_saved_messages: Vec::new(),
                    repo_root: None,
                    is_worktree: false,
                });
            }
        }

        let projects: BTreeMap<u64, ProjectRecord> = config
            .projects
            .iter()
            .cloned()
            .map(|p| (p.id, p))
            .collect();

        let next_project_id = projects
            .keys()
            .filter(|&&id| id != blank_id)
            .last()
            .copied()
            .unwrap_or(1)
            + 1;

        let selected_project = config
            .ui
            .last_selected_project_id
            .filter(|id| projects.contains_key(id))
            .or_else(|| projects.values().find(|p| p.id != blank_id).map(|p| p.id));

        let (terminal_events_tx, terminal_events_rx) =
            bounded(TERMINAL_EVENT_QUEUE_CAPACITY);

        let (broadcast_tx, _broadcast_rx) = broadcast::channel::<ServerMessage>(256);
        let events_rx = terminal_events_rx; // moved to terminal manager thread

        let ai_hook_manager = if config.ai_hooks.global_enabled {
            Some(Arc::new(AiHookManager::new(config.ai_hooks.clone())))
        } else {
            None
        };

        Ok((Self {
            token,
            config_path,
            config,
            projects,
            next_project_id,
            next_terminal_id: 1,
            selected_project,
            active_terminal: None,
            terminal_events_tx,
            ai_hook_manager,
            broadcast_tx,
        }, events_rx))
    }

    fn add_project(&mut self, name: String, path: String) -> Result<WebProject, String> {
        let path_buf = PathBuf::from(&path);
        if !path_buf.exists() {
            return Err("Path does not exist".to_owned());
        }
        let id = self.next_project_id;
        self.next_project_id += 1;
        let record = ProjectRecord {
            id,
            name: name.clone(),
            path: path_buf,
            saved_messages: Vec::new(),
            ai_config: crate::hooks::ProjectAiConfig::default(),
            checklist: Vec::new(),
            browser_last_url: None,
            foreground_saved_messages: Vec::new(),
            repo_root: None,
            is_worktree: false,
        };
        let web = project_to_web(&record);
        self.projects.insert(id, record);
        let _ = self.broadcast_tx.send(ServerMessage::StatePatch {
            updates: vec![StatePatchUpdate::ProjectAdded { project: web.clone() }],
        });
        Ok(web)
    }

    fn remove_project(&mut self, project_id: u64) {
        self.projects.remove(&project_id);
        let _ = self.broadcast_tx.send(ServerMessage::StatePatch {
            updates: vec![StatePatchUpdate::ProjectRemoved { project_id }],
        });
    }

    fn build_snapshot(&self) -> ServerMessage {
        ServerMessage::StateSnapshot {
            projects: self.projects.values().map(project_to_web).collect(),
            terminals: vec![], // filled by terminal manager
            active_terminal_id: self.active_terminal,
            selected_project_id: self.selected_project,
        }
    }
}

fn project_to_web(p: &ProjectRecord) -> WebProject {
    WebProject {
        id: p.id,
        name: p.name.clone(),
        path: p.path.display().to_string(),
        is_worktree: p.is_worktree,
        repo_root: p.repo_root.as_ref().map(|r| r.display().to_string()),
        saved_messages: p.saved_messages.clone(),
        foreground_saved_messages: p.foreground_saved_messages.clone(),
        browser_last_url: p.browser_last_url.clone(),
        checklist: p.checklist.clone(),
    }
}

fn default_blank_project_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "Mergen", "MergenADE")
        .map(|d| d.config_dir().join("blank"))
}

// ---------------------------------------------------------------------------
// TerminalManager: owns all TerminalRuntime instances in a bg thread
// ---------------------------------------------------------------------------

enum TerminalCommand {
    Spawn {
        terminal_id: u64,
        project_id: u64,
        shell: ShellKind,
        kind: TerminalKind,
        working_directory: PathBuf,
        ui_event_tx: Sender<TerminalUiEvent>,
        broadcast_tx: broadcast::Sender<ServerMessage>,
    },
    Input { terminal_id: u64, data: Vec<u8> },
    Paste { terminal_id: u64, text: String },
    Resize { terminal_id: u64, dims: TerminalDimensions },
    Close { terminal_id: u64 },
    SendShortcut { terminal_id: u64, command: String },
    SmartInput { terminal_id: u64, text: String },
}

#[derive(Clone)]
struct TerminalManagerHandle {
    cmd_tx: Sender<TerminalCommand>,
    #[allow(dead_code)]
    shutdown_tx: Sender<()>,
}

impl TerminalManagerHandle {
    fn spawn(_shared: Arc<Mutex<SharedState>>, events_rx: Receiver<TerminalUiEvent>) -> Self {
        let (cmd_tx, cmd_rx) = bounded(TERMINAL_CMD_CAPACITY);
        let (shutdown_tx, shutdown_rx) = bounded(1);

        std::thread::spawn(move || {
            let mut runtimes: BTreeMap<u64, TerminalRuntime> = BTreeMap::new();
            let mut titles: BTreeMap<u64, String> = BTreeMap::new();
            let mut exited: BTreeSet<u64> = BTreeSet::new();
            let mut broadcast_tx: Option<broadcast::Sender<ServerMessage>> = None;

            loop {
                // Drain commands
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TerminalCommand::Spawn {
                            terminal_id,
                            project_id,
                            shell,
                            kind,
                            working_directory,
                            ui_event_tx,
                            broadcast_tx: btx,
                        } => {
                            broadcast_tx = Some(btx.clone());
                            let dims = TerminalDimensions::default();
                            match TerminalRuntime::spawn(
                                terminal_id,
                                project_id,
                                shell,
                                working_directory,
                                ui_event_tx.clone(),
                                eframe::egui::Context::default(),
                                dims,
                                None, // ai_hook_manager - passed via env if needed
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            ) {
                                Ok(runtime) => {
                                    titles.insert(terminal_id, format!("{:?} - {}", kind, shell.label()));
                                    runtimes.insert(terminal_id, runtime);
                                    let _ = btx.send(ServerMessage::StatePatch {
                                        updates: vec![StatePatchUpdate::TerminalAdded {
                                            terminal: WebTerminal {
                                                id: terminal_id,
                                                project_id,
                                                kind: format!("{:?}", kind).to_lowercase(),
                                                shell: shell.label().to_owned(),
                                                title: titles.get(&terminal_id).cloned().unwrap_or_default(),
                                                exited: false,
                                                in_main_view: kind == TerminalKind::Foreground,
                                                ai_tool: None,
                                                ai_status: None,
                                            },
                                        }],
                                    });
                                }
                                Err(e) => {
                                    let _ = btx.send(ServerMessage::Error {
                                        message: format!("Spawn failed: {e}"),
                                    });
                                }
                            }
                        }
                        TerminalCommand::Input { terminal_id, data } => {
                            if let Some(rt) = runtimes.get(&terminal_id) {
                                rt.send_bytes(data);
                            }
                        }
                        TerminalCommand::Paste { terminal_id, text } => {
                            if let Some(rt) = runtimes.get(&terminal_id) {
                                if let Some(bytes) = rt.capture_paste_bytes(&text) {
                                    rt.send_bytes(bytes);
                                }
                            }
                        }
                        TerminalCommand::Resize { terminal_id, dims } => {
                            if let Some(rt) = runtimes.get_mut(&terminal_id) {
                                let _ = rt.resize(dims);
                            }
                        }
                        TerminalCommand::Close { terminal_id } => {
                            if let Some(rt) = runtimes.remove(&terminal_id) {
                                let _ = rt.terminate();
                            }
                            exited.insert(terminal_id);
                            titles.remove(&terminal_id);
                            if let Some(btx) = &broadcast_tx {
                                let _ = btx.send(ServerMessage::StatePatch {
                                    updates: vec![StatePatchUpdate::TerminalRemoved { terminal_id }],
                                });
                            }
                        }
                        TerminalCommand::SendShortcut { terminal_id, command } => {
                            if let Some(rt) = runtimes.get(&terminal_id) {
                                if let Some(bytes) = rt.capture_paste_bytes(&command) {
                                    rt.send_bytes(bytes);
                                    rt.send_bytes(vec![b'\r']);
                                }
                            }
                        }
                        TerminalCommand::SmartInput { terminal_id, text } => {
                            if let Some(rt) = runtimes.get(&terminal_id) {
                                if let Some(bytes) = rt.capture_paste_bytes(&text) {
                                    rt.send_bytes(bytes);
                                    rt.send_bytes(vec![b'\r']);
                                }
                            }
                        }
                    }
                }

                // Drain terminal events
                while let Ok(ev) = events_rx.try_recv() {
                        match ev.kind {
                            TerminalUiEventKind::Wakeup => {
                                // Title may have changed; update clients via state patch
                                if let Some(rt) = runtimes.get(&ev.terminal_id) {
                                    if let Some((snapshot, _)) = try_terminal_snapshots(rt, false) {
                                        let title = if let Some(first_line) = snapshot.lines.first() {
                                            first_line.runs.iter().map(|r| r.text.as_str()).collect::<String>()
                                        } else {
                                            String::new()
                                        };
                                        let prev = titles.get(&ev.terminal_id).cloned().unwrap_or_default();
                                        if !title.is_empty() && title != prev {
                                            titles.insert(ev.terminal_id, title.clone());
                                            if let Some(btx) = &broadcast_tx {
                                                let _ = btx.send(ServerMessage::StatePatch {
                                                    updates: vec![StatePatchUpdate::TerminalUpdated {
                                                        terminal: WebTerminal {
                                                            id: ev.terminal_id,
                                                            project_id: 0,
                                                            kind: "foreground".to_owned(),
                                                            shell: "".to_owned(),
                                                            title,
                                                            exited: false,
                                                            in_main_view: false,
                                                            ai_tool: None,
                                                            ai_status: None,
                                                        },
                                                    }],
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            TerminalUiEventKind::RawOutput { terminal_id, bytes } => {
                                if let Some(btx) = &broadcast_tx {
                                    let _ = btx.send(ServerMessage::TerminalOutput {
                                        terminal_id,
                                        data: bytes,
                                    });
                                }
                            }
                            TerminalUiEventKind::ChildExit | TerminalUiEventKind::Exit => {
                                exited.insert(ev.terminal_id);
                                if let Some(btx) = &broadcast_tx {
                                    let _ = btx.send(ServerMessage::StatePatch {
                                        updates: vec![StatePatchUpdate::TerminalUpdated {
                                            terminal: WebTerminal {
                                                id: ev.terminal_id,
                                                project_id: 0,
                                                kind: "foreground".to_owned(),
                                                shell: "".to_owned(),
                                                title: titles.get(&ev.terminal_id).cloned().unwrap_or_default(),
                                                exited: true,
                                                in_main_view: false,
                                                ai_tool: None,
                                                ai_status: None,
                                            },
                                        }],
                                    });
                                }
                            }
                            _ => {}
                        }
                }

                if shutdown_rx.try_recv().is_ok() {
                    while let Some((_, rt)) = runtimes.pop_first() {
                        let _result: std::io::Result<()> = rt.terminate();
                    }
                    break;
                }

                std::thread::sleep(Duration::from_millis(16));
            }
        });

        Self { cmd_tx, shutdown_tx }
    }
}

// ---------------------------------------------------------------------------
// HTTP Handlers
// ---------------------------------------------------------------------------

async fn index_handler() -> impl IntoResponse {
    match WebAssets::get("index.html") {
        Some(asset) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html")
            .body(axum::body::Body::from(asset.data.into_owned()))
            .unwrap(),
        None => Html("<h1>Mergen ADE Web</h1><p>UI assets not found.</p>").into_response(),
    }
}

async fn health_handler() -> impl IntoResponse {
    Json(ApiResponse {
        success: true,
        data: Some(HealthResponse {
            status: "ok".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }),
        error: None,
    })
}

async fn api_config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let guard = state.shared.lock().unwrap();
    let shell = &guard.config.default_shell;
    Json(ApiResponse {
        success: true,
        data: Some(ConfigResponse {
            default_shell: shell.label().to_owned(),
            launchers: guard
                .config
                .launchers
                .iter()
                .map(|l| WebLauncher {
                    id: l.id.clone(),
                    display_name: l.display_name.clone(),
                    command: l.launch_command.clone(),
                    enabled: l.enabled,
                })
                .collect(),
            shortcuts: guard
                .config
                .terminal_shortcuts
                .iter()
                .map(|s| WebShortcut {
                    id: s.id.clone(),
                    label: s.label.clone(),
                    key: s.key.clone(),
                    command: s.command.clone(),
                    enabled: s.enabled,
                })
                .collect(),
        }),
        error: None,
    })
}

async fn api_projects_handler(State(state): State<AppState>) -> impl IntoResponse {
    let guard = state.shared.lock().unwrap();
    Json(ApiResponse {
        success: true,
        data: Some(guard.projects.values().map(project_to_web).collect::<Vec<_>>()),
        error: None,
    })
}

#[derive(Deserialize)]
struct AddProjectBody {
    name: String,
    path: String,
}

async fn api_add_project_handler(
    State(state): State<AppState>,
    Json(body): Json<AddProjectBody>,
) -> impl IntoResponse {
    let mut guard = state.shared.lock().unwrap();
    match guard.add_project(body.name, body.path) {
        Ok(p) => Json(ApiResponse {
            success: true,
            data: Some(p),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        }),
    }
}

async fn api_terminals_handler(State(_state): State<AppState>) -> impl IntoResponse {
    // Terminal list comes from the manager thread via state snapshot
    // For now return empty - clients should use WebSocket state snapshot
    Json(ApiResponse {
        success: true,
        data: Some(Vec::<WebTerminal>::new()),
        error: None,
    })
}

#[derive(Deserialize)]
struct SpawnTerminalBody {
    project_id: u64,
    shell: String,
    terminal_kind: String,
}

async fn api_spawn_terminal_handler(
    State(state): State<AppState>,
    Json(body): Json<SpawnTerminalBody>,
) -> impl IntoResponse {
    let shell = match body.shell.as_str() {
        "powershell" | "PowerShell" => ShellKind::PowerShell,
        "cmd" | "CMD" => ShellKind::Cmd,
        "zsh" => ShellKind::Zsh,
        _ => ShellKind::default_for_current_platform(),
    };
    let kind = if body.terminal_kind == "background" {
        TerminalKind::Background
    } else {
        TerminalKind::Foreground
    };

    let mut guard = state.shared.lock().unwrap();
    let terminal_id = guard.next_terminal_id;
    guard.next_terminal_id += 1;

    let project = match guard.projects.get(&body.project_id) {
        Some(p) => p,
        None => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Project not found".to_owned()),
            });
        }
    };

    let working_directory = path_utils::normalize_windows_verbatim_path_for_shell(&project.path);
    let ui_event_tx = guard.terminal_events_tx.clone();
    let broadcast_tx = guard.broadcast_tx.clone();
    drop(guard);

    let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Spawn {
        terminal_id,
        project_id: body.project_id,
        shell,
        kind,
        working_directory,
        ui_event_tx,
        broadcast_tx,
    });

    Json(ApiResponse {
        success: true,
        data: Some(WebTerminal {
            id: terminal_id,
            project_id: body.project_id,
            kind: format!("{:?}", kind).to_lowercase(),
            shell: shell.label().to_owned(),
            title: format!("{:?} - {}", kind, shell.label()),
            exited: false,
            in_main_view: kind == TerminalKind::Foreground,
            ai_tool: None,
            ai_status: None,
        }),
        error: None,
    })
}

#[derive(Deserialize)]
struct DirectoryQuery {
    project_id: u64,
}

async fn api_directory_handler(
    State(state): State<AppState>,
    Query(query): Query<DirectoryQuery>,
) -> impl IntoResponse {
    let guard = state.shared.lock().unwrap();
    let project = match guard.projects.get(&query.project_id) {
        Some(p) => p,
        None => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Project not found".to_owned()),
            });
        }
    };
    let root = build_directory_node(&project.path, &project.path);
    Json(ApiResponse {
        success: true,
        data: Some(root),
        error: None,
    })
}

fn build_directory_node(path: &std::path::Path, project_root: &std::path::Path) -> WebDirectoryNode {
    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_owned();
    let is_dir = path.is_dir();
    let mut children = Vec::new();
    if is_dir {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.take(100) {
                if let Ok(entry) = entry {
                    let child_path = entry.path();
                    let rel = child_path.strip_prefix(project_root).unwrap_or(&child_path);
                    if rel.components().count() <= 2 {
                        children.push(build_directory_node(&child_path, project_root));
                    }
                }
            }
        }
    }
    children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    WebDirectoryNode {
        name,
        path: path.display().to_string(),
        is_dir,
        is_deferred: false,
        children,
    }
}

#[derive(Deserialize)]
struct SourceControlQuery {
    project_id: u64,
}

async fn api_source_control_handler(
    State(state): State<AppState>,
    Query(query): Query<SourceControlQuery>,
) -> impl IntoResponse {
    let guard = state.shared.lock().unwrap();
    let project = match guard.projects.get(&query.project_id) {
        Some(p) => p,
        None => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Project not found".to_owned()),
            });
        }
    };

    let (branch, files) = match get_git_status(&project.path) {
        Ok(status) => status,
        Err(_) => (String::new(), Vec::new()),
    };

    Json(ApiResponse {
        success: true,
        data: Some(SourceControlResponse { branch, files }),
        error: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceControlResponse {
    branch: String,
    files: Vec<crate::web_protocol::WebSourceControlFile>,
}

fn get_git_status(path: &std::path::Path) -> Result<(String, Vec<crate::web_protocol::WebSourceControlFile>), String> {
    use std::process::Command;

    let output = Command::new("git")
        .args(["status", "--porcelain", "-b"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("git status failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut branch = String::new();
    let mut files = Vec::new();

    for line in stdout.lines() {
        if line.starts_with("## ") {
            branch = line[3..].split("...").next().unwrap_or("").to_owned();
        } else if line.len() >= 3 {
            let status = match &line[..2] {
                " M" | "M " | "MM" => "modified",
                " A" | "A " | "AM" => "added",
                " D" | "D " | "DM" => "deleted",
                "??" => "untracked",
                _ => "other",
            };
            files.push(WebSourceControlFile {
                path: line[3..].to_owned(),
                status: status.to_owned(),
            });
        }
    }

    Ok((branch, files))
}

// ---------------------------------------------------------------------------
// WebSocket Handler
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let token_ok = query.token.as_ref() == Some(&state.token);
    ws.on_upgrade(move |socket| handle_socket(socket, state, token_ok))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, token_ok: bool) {
    let (mut broadcast_rx, hello_msg, snap_msg) = {
        let guard = state.shared.lock().unwrap();
        let rx = guard.broadcast_tx.subscribe();
        let hello = serde_json::to_string(&ServerMessage::Hello {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            auth_required: true,
        })
        .unwrap_or_default();
        let snap = serde_json::to_string(&guard.build_snapshot()).unwrap_or_default();
        (rx, hello, snap)
    };

    if !token_ok {
        let msg = serde_json::to_string(&ServerMessage::Error {
            message: "Invalid or missing auth token".to_owned(),
        })
        .unwrap_or_default();
        let _ = socket.send(WsMessage::Text(msg)).await;
        let _ = socket.close().await;
        return;
    }

    // Send hello + snapshot
    let _ = socket.send(WsMessage::Text(hello_msg)).await;
    let _ = socket.send(WsMessage::Text(snap_msg)).await;

    loop {
        tokio::select! {
            biased;
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok(msg) => {
                        let json = serde_json::to_string(&msg).unwrap_or_default();
                        if socket.send(WsMessage::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            maybe_msg = socket.next() => {
                match maybe_msg {
                    Some(Ok(msg)) => {
                        match msg {
                            WsMessage::Text(text) => {
                                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                    handle_client_message(&state, client_msg).await;
                                }
                            }
                            WsMessage::Binary(data) => {
                                if let Some((terminal_id, payload)) = decode_terminal_binary_header(&data) {
                                    let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Input {
                                        terminal_id,
                                        data: payload.to_vec(),
                                    });
                                }
                            }
                            WsMessage::Close(_) => break,
                            _ => {}
                        }
                    }
                    _ => break,
                }
            }
        }
    }
}

async fn handle_client_message(state: &AppState, msg: ClientMessage) {
    match msg {
        ClientMessage::SpawnTerminal {
            project_id,
            shell,
            terminal_kind: kind,
        } => {
            let mut guard = state.shared.lock().unwrap();
            let terminal_id = guard.next_terminal_id;
            guard.next_terminal_id += 1;
            let project = match guard.projects.get(&project_id) {
                Some(p) => p.clone(),
                None => return,
            };
            let working_directory = path_utils::normalize_windows_verbatim_path_for_shell(&project.path);
            let shell_kind = match shell.as_str() {
                "powershell" | "PowerShell" => ShellKind::PowerShell,
                "cmd" | "CMD" => ShellKind::Cmd,
                "zsh" => ShellKind::Zsh,
                _ => ShellKind::default_for_current_platform(),
            };
            let term_kind = if kind == "background" {
                TerminalKind::Background
            } else {
                TerminalKind::Foreground
            };
            let ui_event_tx = guard.terminal_events_tx.clone();
            let broadcast_tx = guard.broadcast_tx.clone();
            drop(guard);

            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Spawn {
                terminal_id,
                project_id,
                shell: shell_kind,
                kind: term_kind,
                working_directory,
                ui_event_tx,
                broadcast_tx,
            });
        }
        ClientMessage::TerminalInput { terminal_id, data } => {
            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Input {
                terminal_id,
                data,
            });
        }
        ClientMessage::TerminalPaste { terminal_id, text } => {
            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Paste {
                terminal_id,
                text,
            });
        }
        ClientMessage::TerminalResize {
            terminal_id,
            cols,
            lines,
        } => {
            let dims = TerminalDimensions {
                cols,
                lines,
                pixel_width: (cols as u32 * 8).max(1) as u16,
                pixel_height: (lines as u32 * 16).max(1) as u16,
            };
            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Resize {
                terminal_id,
                dims,
            });
        }
        ClientMessage::CloseTerminal { terminal_id } => {
            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::Close { terminal_id });
        }
        ClientMessage::SelectProject { project_id } => {
            let mut guard = state.shared.lock().unwrap();
            guard.selected_project = Some(project_id);
            let _ = guard.broadcast_tx.send(ServerMessage::StatePatch {
                updates: vec![StatePatchUpdate::ProjectSelected {
                    project_id: Some(project_id),
                }],
            });
        }
        ClientMessage::AddProject { name, path } => {
            let mut guard = state.shared.lock().unwrap();
            let _ = guard.add_project(name, path);
        }
        ClientMessage::RemoveProject { project_id } => {
            {
                let mut guard = state.shared.lock().unwrap();
                guard.remove_project(project_id);
            }
            // Also close associated terminals
            // Note: this is a simplified cleanup
        }
        ClientMessage::SendShortcut {
            terminal_id,
            command,
        } => {
            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::SendShortcut {
                terminal_id,
                command,
            });
        }
        ClientMessage::SmartInputSubmit {
            terminal_id,
            text,
            mode: _,
        } => {
            let _ = state.tm_handle.cmd_tx.send(TerminalCommand::SmartInput {
                terminal_id,
                text,
            });
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Static file handler
// ---------------------------------------------------------------------------

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(asset) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", mime.as_ref())
            .body(axum::body::Body::from(asset.data.into_owned()))
            .unwrap();
    }

    if let Some(asset) = WebAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html")
            .body(axum::body::Body::from(asset.data.into_owned()))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(axum::body::Body::from("Not found"))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn web_auth_token() -> String {
    std::env::var(WEB_AUTH_TOKEN_ENV_VAR).unwrap_or_else(|_| {
        let random: u64 = rand::random();
        format!("{:x}", random)
    })
}

fn web_server_port() -> u16 {
    std::env::var(WEB_PORT_ENV_VAR)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(WEB_SERVER_DEFAULT_PORT)
}
