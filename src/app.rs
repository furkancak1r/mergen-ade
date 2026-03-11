use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use arboard::Clipboard;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Color32, Event, FontData, FontFamily, FontId, Id, Key, Layout, RichText, Sense,
    Stroke, TextWrapMode, Ui, Vec2, WidgetInfo, WidgetText, WidgetType,
};
use iconflow::{fonts as icon_fonts, try_icon, Pack, Size, Style};

use crate::config;
use crate::layout;
use crate::models::{
    AppConfig, AutoTileScope, LeftSidebarTab, MainVisibilityMode, ProjectRecord, ShellKind,
    TerminalKind,
};
use crate::terminal::{
    try_terminal_selection_snapshot, try_terminal_snapshots, TerminalColor, TerminalCursor,
    TerminalCursorShape, TerminalDimensions, TerminalRuntime, TerminalSelectionLine,
    TerminalSelectionSnapshot, TerminalSnapshot, TerminalUiEvent, TerminalUiEventKind,
};
use crate::title::{terminal_title_text, update_terminal_title};

const CELL_WIDTH_PX: f32 = 8.0;
const CELL_HEIGHT_PX: f32 = 16.0;
const TITLE_MAX_LEN: usize = 40;
const TERMINAL_EVENT_BUDGET: usize = 4096;
const TERMINAL_RETRY_MS: u64 = 8;
const TERMINAL_FALLBACK_REFRESH_MS: u64 = 16;
const CURSOR_BLINK_STEP_SECS: f64 = 0.6;
const CTRL_C_DOUBLE_PRESS_WINDOW_SECS: f64 = 0.75;
const POWERSHELL_CURSOR_ROW_STABLE_SECS: f64 = 0.06;
const CURSOR_BAR_WIDTH_PX: f32 = 2.0;
const CURSOR_UNDERLINE_HEIGHT_PX: f32 = 2.0;
const DIRECTORY_INDEX_MAX_DEPTH: usize = 8;
const DIRECTORY_INDEX_MAX_NODES: usize = 20_000;
const TERMINAL_OUTPUT_BG: Color32 = Color32::from_rgb(26, 30, 36);
const TERMINAL_HEADER_HEIGHT: f32 = 38.0;
const TERMINAL_HEADER_GAP: f32 = 6.0;
const TERMINAL_TILE_GAP_X: f32 = 0.0;
const TERMINAL_TILE_GAP_Y: f32 = 0.0;
const TERMINAL_PANE_INNER_MARGIN: f32 = 2.0;
const APP_BG: Color32 = Color32::from_rgb(14, 18, 24);
const SURFACE_BG: Color32 = Color32::from_rgb(22, 28, 38);
const SURFACE_BG_SOFT: Color32 = Color32::from_rgb(24, 38, 52);
const BORDER_COLOR: Color32 = Color32::from_rgb(46, 60, 78);
const ACCENT: Color32 = Color32::from_rgb(26, 179, 255);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(225, 233, 245);
const TEXT_MUTED: Color32 = Color32::from_rgb(148, 167, 191);
const PROJECT_EXPLORER_WIDTH: f32 = 320.0;
const ACTIVITY_RAIL_WIDTH: f32 = 48.0;
const CONTROL_ROW_HEIGHT: f32 = 28.0;
const TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH: f32 = 32.0;
const TOP_BAR_HEIGHT: f32 = 54.0;
const DIRECTORY_SEARCH_INPUT_ID: &str = "directory-search-input";
const SAVED_MESSAGE_DRAFT_INPUT_ID: &str = "saved-message-draft-input";
// Pill button palette
const BTN_BLUE: Color32 = Color32::from_rgb(16, 64, 112);
const BTN_BLUE_HOVER: Color32 = Color32::from_rgb(22, 88, 150);
const BTN_TEAL: Color32 = Color32::from_rgb(14, 68, 82);
const BTN_TEAL_HOVER: Color32 = Color32::from_rgb(20, 92, 110);
const BTN_SUBTLE: Color32 = Color32::from_rgb(20, 63, 92);
const BTN_SUBTLE_HOVER: Color32 = Color32::from_rgb(28, 85, 122);
const BTN_RED: Color32 = Color32::from_rgb(120, 30, 30);
const BTN_RED_HOVER: Color32 = Color32::from_rgb(160, 40, 40);
const BTN_ICON: Color32 = Color32::from_rgb(24, 70, 103);
const BTN_ICON_HOVER: Color32 = Color32::from_rgb(31, 98, 144);
const BTN_ICON_ACTIVE: Color32 = Color32::from_rgb(24, 118, 172);
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum AppIcon {
    ArrowClockwise,
    ChatText,
    CheckCircle,
    Clock,
    Copy,
    Download,
    Eye,
    EyeOff,
    Folder,
    FolderOpen,
    FolderPlus,
    Gear,
    GitBranch,
    List,
    Plus,
    Terminal,
    TerminalWindow,
    Trash,
    TreeView,
    X,
}

impl AppIcon {
    const ALL: [Self; 20] = [
        Self::ArrowClockwise,
        Self::ChatText,
        Self::CheckCircle,
        Self::Clock,
        Self::Copy,
        Self::Download,
        Self::Eye,
        Self::EyeOff,
        Self::Folder,
        Self::FolderOpen,
        Self::FolderPlus,
        Self::Gear,
        Self::GitBranch,
        Self::List,
        Self::Plus,
        Self::Terminal,
        Self::TerminalWindow,
        Self::Trash,
        Self::TreeView,
        Self::X,
    ];

    const fn lucide_name(self) -> &'static str {
        match self {
            Self::ArrowClockwise => "refresh-ccw",
            Self::ChatText => "message-square-text",
            Self::CheckCircle => "circle-check",
            Self::Clock => "clock",
            Self::Copy => "copy",
            Self::Download => "download",
            Self::Eye => "eye",
            Self::EyeOff => "eye-off",
            Self::Folder => "folder",
            Self::FolderOpen => "folder-open",
            Self::FolderPlus => "folder-plus",
            Self::Gear => "settings",
            Self::GitBranch => "git-branch",
            Self::List => "list",
            Self::Plus => "plus",
            Self::Terminal => "terminal",
            Self::TerminalWindow => "app-window",
            Self::Trash => "trash-2",
            Self::TreeView => "folder-tree",
            Self::X => "x",
        }
    }
}

impl fmt::Display for AppIcon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(icon_glyph(*self))
    }
}

fn icon_glyph(icon: AppIcon) -> &'static str {
    static GLYPH_CACHE: OnceLock<BTreeMap<AppIcon, String>> = OnceLock::new();
    let cache = GLYPH_CACHE.get_or_init(|| {
        let mut map = BTreeMap::new();
        for item in AppIcon::ALL {
            let glyph = try_icon(
                Pack::Lucide,
                item.lucide_name(),
                Style::Regular,
                Size::Regular,
            )
            .ok()
            .and_then(|entry| char::from_u32(entry.codepoint))
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| "?".to_owned());
            map.insert(item, glyph);
        }
        map
    });
    cache.get(&icon).map(String::as_str).unwrap_or("?")
}

mod icons {
    use super::AppIcon;

    pub const ARROW_CLOCKWISE: AppIcon = AppIcon::ArrowClockwise;
    pub const CHAT_TEXT: AppIcon = AppIcon::ChatText;
    pub const CHECK_CIRCLE: AppIcon = AppIcon::CheckCircle;
    pub const CLOCK: AppIcon = AppIcon::Clock;
    pub const COPY: AppIcon = AppIcon::Copy;
    pub const DOWNLOAD: AppIcon = AppIcon::Download;
    pub const EYE: AppIcon = AppIcon::Eye;
    pub const EYE_OFF: AppIcon = AppIcon::EyeOff;
    pub const FOLDER: AppIcon = AppIcon::Folder;
    pub const FOLDER_OPEN: AppIcon = AppIcon::FolderOpen;
    pub const FOLDER_PLUS: AppIcon = AppIcon::FolderPlus;
    pub const GEAR: AppIcon = AppIcon::Gear;
    pub const GIT_BRANCH: AppIcon = AppIcon::GitBranch;
    pub const LIST: AppIcon = AppIcon::List;
    pub const PLUS: AppIcon = AppIcon::Plus;
    pub const TERMINAL: AppIcon = AppIcon::Terminal;
    pub const TERMINAL_WINDOW: AppIcon = AppIcon::TerminalWindow;
    pub const TRASH: AppIcon = AppIcon::Trash;
    pub const TREE_VIEW: AppIcon = AppIcon::TreeView;
    pub const X: AppIcon = AppIcon::X;
}

pub struct AdeApp {
    config_path: PathBuf,
    config: AppConfig,
    config_load_error: Option<String>,
    config_save_requires_reload: bool,
    pending_config_changes: PendingConfigChanges,
    projects: BTreeMap<u64, ProjectRecord>,
    terminals: BTreeMap<u64, TerminalEntry>,
    next_project_id: u64,
    next_terminal_id: u64,
    selected_project: Option<u64>,
    active_terminal: Option<u64>,
    pending_ctrl_c: Option<PendingCtrlC>,
    buffered_terminal_input: Vec<Event>,
    buffered_terminal_navigation: Vec<TerminalNavigationDirection>,
    terminal_events_tx: Sender<TerminalUiEvent>,
    terminal_events_rx: Receiver<TerminalUiEvent>,
    show_settings_popup: bool,
    saved_message_drafts: BTreeMap<u64, String>,
    directory_search_query: String,
    status_line: String,
    layout_epoch: u64,
    theme_initialized: bool,
    #[cfg(target_os = "windows")]
    window_hwnd: Option<isize>,
    #[cfg(target_os = "windows")]
    window_layout_passes_remaining: u8,
    source_control_events_tx: Sender<SourceControlEvent>,
    source_control_events_rx: Receiver<SourceControlEvent>,
    source_control_state: BTreeMap<u64, SourceControlSnapshot>,
    directory_index_events_tx: Sender<DirectoryIndexEvent>,
    directory_index_events_rx: Receiver<DirectoryIndexEvent>,
    directory_index_state: BTreeMap<u64, DirectoryIndexSnapshot>,
    directory_index_generation: BTreeMap<u64, u64>,
}

struct TerminalEntry {
    id: u64,
    project_id: u64,
    kind: TerminalKind,
    shell: ShellKind,
    title: String,
    full_title: String,
    pending_line_for_title: String,
    in_main_view: bool,
    dirty: bool,
    last_seqno: usize,
    last_cursor_row: Option<usize>,
    last_cursor_row_changed_at: Option<f64>,
    stable_input_cursor_row: Option<usize>,
    render_cache: TerminalSnapshot,
    selection: Option<TerminalSelection>,
    selection_snapshot: Option<TerminalSelectionSnapshot>,
    selection_drag_active: bool,
    snapshot_refresh_deferred: bool,
    exited: bool,
    runtime: TerminalRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingCtrlC {
    terminal_id: u64,
    expires_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSelectionPoint {
    row: usize,
    column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSelection {
    anchor: TerminalSelectionPoint,
    focus: TerminalSelectionPoint,
}

impl TerminalSelection {
    fn collapsed(point: TerminalSelectionPoint) -> Self {
        Self {
            anchor: point,
            focus: point,
        }
    }

    fn has_selection(&self) -> bool {
        self.anchor != self.focus
    }

    fn normalized(&self) -> (TerminalSelectionPoint, TerminalSelectionPoint) {
        if (self.anchor.row, self.anchor.column) <= (self.focus.row, self.focus.column) {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CtrlCAction {
    CopySelection,
    ArmInterrupt,
    SendInterrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalNavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Default)]
struct SourceControlSnapshot {
    branch: String,
    ahead: usize,
    behind: usize,
    files: Vec<SourceControlFile>,
    loading: bool,
    last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceControlFile {
    path: String,
    status: &'static str,
    staged: bool,
}

#[derive(Debug)]
struct SourceControlEvent {
    project_id: u64,
    snapshot: SourceControlSnapshot,
}

struct TerminalRenderModel {
    layout_job: LayoutJob,
    cursor_overlay: Option<TerminalCursorOverlay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalCursorOverlay {
    shape: TerminalCursorShape,
    row: usize,
    column: usize,
    width_columns: usize,
    color: Color32,
}

#[derive(Debug, Clone)]
struct DirectoryNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    children: Vec<DirectoryNode>,
}

#[derive(Debug, Clone)]
struct DirectoryIndexSnapshot {
    root: DirectoryNode,
    loading: bool,
    last_error: Option<String>,
    truncated: bool,
}

#[derive(Debug, Clone)]
struct DirectoryIndexEvent {
    project_id: u64,
    generation: u64,
    snapshot: DirectoryIndexSnapshot,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PendingConfigChanges {
    default_shell: bool,
    ui: bool,
    projects: bool,
    selection: bool,
}

impl AdeApp {
    pub fn bootstrap(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = config::config_path().unwrap_or_else(|_| PathBuf::from("config.toml"));
        let (mut config, config_load_error) = match config::load_config(&config_path) {
            Ok(config) => (config, None),
            Err(err) => (AppConfig::default(), Some(err.to_string())),
        };
        config.ui.show_project_explorer = true;
        config.ui.show_terminal_manager = true;
        config.ui.main_visibility_mode = MainVisibilityMode::Global;
        config.ui.project_filter_mode = false;
        #[cfg(target_os = "windows")]
        let window_hwnd = Self::extract_window_hwnd(cc);

        let projects = config
            .projects
            .iter()
            .cloned()
            .map(|project| (project.id, project))
            .collect::<BTreeMap<_, _>>();

        let next_project_id = projects.keys().last().copied().unwrap_or(0) + 1;
        let selected_project = config
            .ui
            .last_selected_project_id
            .filter(|project_id| projects.contains_key(project_id))
            .or_else(|| projects.keys().next().copied());

        let (terminal_events_tx, terminal_events_rx) = crossbeam_channel::unbounded();
        let (source_control_events_tx, source_control_events_rx) = crossbeam_channel::unbounded();
        let (directory_index_events_tx, directory_index_events_rx) = crossbeam_channel::unbounded();

        let app = Self {
            config_path,
            config,
            config_load_error: config_load_error.clone(),
            config_save_requires_reload: config_load_error.is_some(),
            pending_config_changes: PendingConfigChanges::default(),
            projects,
            terminals: BTreeMap::new(),
            next_project_id,
            next_terminal_id: 1,
            selected_project,
            active_terminal: None,
            pending_ctrl_c: None,
            buffered_terminal_input: Vec::new(),
            buffered_terminal_navigation: Vec::new(),
            terminal_events_tx,
            terminal_events_rx,
            show_settings_popup: false,
            saved_message_drafts: BTreeMap::new(),
            directory_search_query: String::new(),
            status_line: config_load_error
                .map(|err| format!("Config load error: {err}. Existing config preserved."))
                .unwrap_or_else(|| "Ready".to_owned()),
            layout_epoch: 0,
            theme_initialized: false,
            #[cfg(target_os = "windows")]
            window_hwnd,
            #[cfg(target_os = "windows")]
            window_layout_passes_remaining: 8,
            source_control_events_tx,
            source_control_events_rx,
            source_control_state: BTreeMap::new(),
            directory_index_events_tx,
            directory_index_events_rx,
            directory_index_state: BTreeMap::new(),
            directory_index_generation: BTreeMap::new(),
        };
        app
    }

    fn persist_config(&mut self) {
        let recovered_from_disk = self.config_load_error.is_some();
        let mut config_to_save = if self.config_save_requires_reload {
            match config::load_config(&self.config_path) {
                Ok(loaded_config) => {
                    self.config_load_error = None;
                    recover_config_state(
                        &self.config,
                        &self.projects,
                        self.selected_project,
                        loaded_config,
                        self.pending_config_changes,
                    )
                }
                Err(err) => {
                    let err = err.to_string();
                    self.config_load_error = Some(err.clone());
                    self.status_line =
                        format!("Config save skipped while config reload still fails: {err}");
                    return;
                }
            }
        } else {
            self.config.clone()
        };

        if !self.config_save_requires_reload {
            self.config.projects = self.projects.values().cloned().collect();
            self.config.ui.last_selected_project_id = self.selected_project;
            config_to_save = self.config.clone();
        }

        if let Err(err) = config::save_config(&self.config_path, &config_to_save) {
            self.status_line = format!("Config save error: {err}");
            return;
        }

        self.pending_config_changes = PendingConfigChanges::default();
        if recovered_from_disk {
            self.status_line = "Config recovered and changes saved.".to_owned();
        }
    }

    fn note_ui_config_changed(&mut self) {
        self.pending_config_changes.ui = true;
    }

    fn note_default_shell_changed(&mut self) {
        self.pending_config_changes.default_shell = true;
    }

    fn note_projects_changed(&mut self) {
        self.pending_config_changes.projects = true;
    }

    fn note_selection_changed(&mut self) {
        self.pending_config_changes.selection = true;
    }

    fn bump_layout_epoch(&mut self) {
        self.layout_epoch = self.layout_epoch.wrapping_add(1);
    }

    fn first_visible_terminal_for_main(&self) -> Option<u64> {
        self.terminals
            .iter()
            .filter_map(|(id, terminal)| self.terminal_visible_in_main(terminal).then_some(*id))
            .min()
    }

    fn apply_auto_tile_scope_to_open_terminals(&mut self) -> bool {
        let auto_tile_scope = self.config.ui.auto_tile_scope;
        let selected_project = self.selected_project;
        let mut changed = false;

        for terminal in self.terminals.values_mut() {
            let next_in_main_view = match auto_tile_scope {
                AutoTileScope::AllVisible => true,
                AutoTileScope::SelectedProjectOnly => {
                    selected_project.is_some_and(|project_id| terminal.project_id == project_id)
                }
            };

            if terminal.in_main_view != next_in_main_view {
                terminal.in_main_view = next_in_main_view;
                changed = true;
            }
        }

        let active_visible = self
            .active_terminal
            .and_then(|terminal_id| self.terminals.get(&terminal_id))
            .is_some_and(|terminal| self.terminal_visible_in_main(terminal));
        let next_active_terminal = if active_visible {
            self.active_terminal
        } else {
            self.first_visible_terminal_for_main()
        };
        if self.active_terminal != next_active_terminal {
            self.set_active_terminal(next_active_terminal);
            changed = true;
        }

        changed
    }

    fn apply_auto_tile_scope_and_refresh_layout(&mut self, ctx: &egui::Context) {
        if self.apply_auto_tile_scope_to_open_terminals() {
            self.bump_layout_epoch();
            ctx.request_repaint();
        }
    }

    fn apply_selected_project_auto_tile_scope_and_refresh_layout(&mut self, ctx: &egui::Context) {
        if self.config.ui.auto_tile_scope != AutoTileScope::SelectedProjectOnly {
            return;
        }

        self.apply_auto_tile_scope_and_refresh_layout(ctx);
    }

    fn terminal_visible_in_main(&self, terminal: &TerminalEntry) -> bool {
        terminal.in_main_view
    }

    fn add_project(&mut self, path: PathBuf) {
        if self.projects.values().any(|project| project.path == path) {
            self.status_line = "Project is already added".to_owned();
            return;
        }

        let name = path
            .file_name()
            .map(|segment| segment.to_string_lossy().to_string())
            .filter(|segment| !segment.trim().is_empty())
            .unwrap_or_else(|| path.display().to_string());

        let project = ProjectRecord {
            id: self.next_project_id,
            name,
            path,
            saved_messages: Vec::new(),
        };

        self.selected_project = Some(project.id);
        self.projects.insert(project.id, project);
        self.next_project_id += 1;
        if self.config.ui.auto_tile_scope == AutoTileScope::SelectedProjectOnly {
            let _ = self.apply_auto_tile_scope_to_open_terminals();
        }
        self.bump_layout_epoch();
        self.note_projects_changed();
        self.note_selection_changed();
        self.persist_config();
    }

    fn spawn_terminal_for_project(
        &mut self,
        ctx: &egui::Context,
        project_id: u64,
        kind: TerminalKind,
    ) {
        let Some(project) = self.projects.get(&project_id).cloned() else {
            return;
        };

        let shell = self.config.default_shell;

        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;

        let dimensions = TerminalDimensions::default();
        let runtime = match TerminalRuntime::spawn(
            terminal_id,
            shell,
            project.path.clone(),
            self.terminal_events_tx.clone(),
            ctx.clone(),
            dimensions,
        ) {
            Ok(runtime) => runtime,
            Err(err) => {
                self.status_line = format!("Failed to create terminal: {err}");
                return;
            }
        };

        let fallback_title = format!("Terminal {terminal_id}");
        let entry = TerminalEntry {
            id: terminal_id,
            project_id,
            kind,
            shell,
            title: fallback_title.clone(),
            full_title: fallback_title,
            pending_line_for_title: String::new(),
            in_main_view: true,
            dirty: true,
            last_seqno: runtime.latest_seqno(),
            last_cursor_row: None,
            last_cursor_row_changed_at: None,
            stable_input_cursor_row: None,
            render_cache: TerminalSnapshot::default(),
            selection: None,
            selection_snapshot: None,
            selection_drag_active: false,
            snapshot_refresh_deferred: false,
            exited: false,
            runtime,
        };

        self.terminals.insert(terminal_id, entry);
        self.set_active_terminal(Some(terminal_id));
        self.bump_layout_epoch();

        self.status_line = "Terminal created".to_owned();
    }

    fn process_terminal_events(&mut self, ctx: &egui::Context) {
        let mut dirty_ids = BTreeSet::new();
        let mut exited_ids = BTreeSet::new();
        let mut processed = 0usize;

        while processed < TERMINAL_EVENT_BUDGET {
            let Ok(event) = self.terminal_events_rx.try_recv() else {
                break;
            };
            processed += 1;

            match event.kind {
                TerminalUiEventKind::Wakeup => {
                    dirty_ids.insert(event.terminal_id);
                }
                TerminalUiEventKind::ChildExit | TerminalUiEventKind::Exit => {
                    exited_ids.insert(event.terminal_id);
                    dirty_ids.insert(event.terminal_id);
                }
            }
        }

        let mut changed = false;

        for terminal_id in dirty_ids {
            let Some(entry) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            entry.dirty = true;
            changed = true;
        }

        for terminal_id in exited_ids {
            let Some(entry) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            entry.exited = true;
            entry.dirty = true;
            changed = true;
        }

        if changed {
            ctx.request_repaint();
        }

        if !self.terminal_events_rx.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(1));
        }
    }

    fn process_source_control_events(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(event) = self.source_control_events_rx.try_recv() {
            self.source_control_state
                .insert(event.project_id, event.snapshot);
            changed = true;
        }
        if changed {
            ctx.request_repaint();
        }
    }

    fn request_source_control_refresh(&mut self, project_id: u64, run_fetch: bool) {
        let Some(project) = self.projects.get(&project_id).cloned() else {
            return;
        };

        self.source_control_state
            .entry(project_id)
            .and_modify(|snapshot| {
                snapshot.loading = true;
                snapshot.last_error = None;
            })
            .or_insert_with(|| SourceControlSnapshot {
                loading: true,
                ..SourceControlSnapshot::default()
            });

        let tx = self.source_control_events_tx.clone();
        std::thread::spawn(move || {
            let snapshot = collect_source_control_snapshot(&project.path, run_fetch);
            let _ = tx.send(SourceControlEvent {
                project_id,
                snapshot,
            });
        });
    }

    fn process_directory_index_events(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(event) = self.directory_index_events_rx.try_recv() {
            let latest_generation = self
                .directory_index_generation
                .get(&event.project_id)
                .copied()
                .unwrap_or(0);
            if event.generation != latest_generation {
                continue;
            }

            self.directory_index_state
                .insert(event.project_id, event.snapshot);
            changed = true;
        }
        if changed {
            ctx.request_repaint();
        }
        if !self.directory_index_events_rx.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(1));
        }
    }

    fn request_directory_index_refresh(&mut self, project_id: u64, force: bool) {
        let Some(project) = self.projects.get(&project_id).cloned() else {
            return;
        };

        if self
            .directory_index_state
            .get(&project_id)
            .is_some_and(|snapshot| snapshot.loading)
        {
            return;
        }

        if !force && self.directory_index_state.contains_key(&project_id) {
            return;
        }

        let generation = self
            .directory_index_generation
            .entry(project_id)
            .or_insert(0);
        *generation = generation.wrapping_add(1);
        let current_generation = *generation;

        self.directory_index_state
            .entry(project_id)
            .and_modify(|snapshot| {
                snapshot.loading = true;
                snapshot.last_error = None;
                snapshot.truncated = false;
            })
            .or_insert_with(|| DirectoryIndexSnapshot {
                root: build_directory_root_node(&project.path),
                loading: true,
                last_error: None,
                truncated: false,
            });

        let tx = self.directory_index_events_tx.clone();
        std::thread::spawn(move || {
            let snapshot = collect_directory_index_snapshot(&project.path);
            let _ = tx.send(DirectoryIndexEvent {
                project_id,
                generation: current_generation,
                snapshot,
            });
        });
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context, main_area_size: Vec2) {
        if self.ui_owns_keyboard(ctx) {
            return;
        }

        let mut changed = false;
        let directions = self.take_terminal_navigation_shortcuts(ctx);
        for direction in directions {
            let visible_ids = self.visible_terminal_ids_for_main();
            let grid =
                layout::compute_tile_grid(visible_ids.len(), main_area_size.x, main_area_size.y);
            let next_terminal = next_terminal_in_direction(
                self.active_terminal_accepts_input(),
                &visible_ids,
                grid,
                direction,
            );
            if let Some(next_terminal) = next_terminal {
                self.set_active_terminal(Some(next_terminal));
                changed = true;
            }
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn active_terminal_accepts_input(&self) -> Option<u64> {
        let active_terminal_id = self.active_terminal?;
        self.terminals
            .get(&active_terminal_id)
            .is_some_and(|terminal| self.terminal_visible_in_main(terminal) && !terminal.exited)
            .then_some(active_terminal_id)
    }

    fn should_capture_terminal_keyboard_state(
        active_terminal_accepts_input: bool,
        ui_owns_keyboard: bool,
    ) -> bool {
        active_terminal_accepts_input && !ui_owns_keyboard
    }

    fn directory_search_input_id() -> Id {
        Id::new(DIRECTORY_SEARCH_INPUT_ID)
    }

    fn saved_message_draft_input_id(project_id: u64) -> Id {
        Id::new((SAVED_MESSAGE_DRAFT_INPUT_ID, project_id))
    }

    fn text_input_has_focus(&self, ctx: &egui::Context) -> bool {
        if ctx.memory(|mem| mem.has_focus(Self::directory_search_input_id())) {
            return true;
        }

        self.selected_project.is_some_and(|project_id| {
            ctx.memory(|mem| mem.has_focus(Self::saved_message_draft_input_id(project_id)))
        })
    }

    fn surrender_ui_text_focus(&self, ctx: &egui::Context) {
        ctx.memory_mut(|mem| {
            mem.surrender_focus(Self::directory_search_input_id());
            if let Some(project_id) = self.selected_project {
                mem.surrender_focus(Self::saved_message_draft_input_id(project_id));
            }
        });
    }

    fn ui_owns_keyboard_state(
        text_input_has_focus: bool,
        popup_open: bool,
        context_menu_open: bool,
        show_settings_popup: bool,
        wants_keyboard_input: bool,
    ) -> bool {
        text_input_has_focus
            || popup_open
            || context_menu_open
            || (show_settings_popup && wants_keyboard_input)
    }

    fn ui_owns_keyboard(&self, ctx: &egui::Context) -> bool {
        Self::ui_owns_keyboard_state(
            self.text_input_has_focus(ctx),
            ctx.memory(|mem| mem.any_popup_open()),
            ctx.is_context_menu_open(),
            self.show_settings_popup,
            ctx.wants_keyboard_input(),
        )
    }

    fn should_capture_terminal_keyboard(&self, ctx: &egui::Context) -> bool {
        Self::should_capture_terminal_keyboard_state(
            self.active_terminal_accepts_input().is_some(),
            self.ui_owns_keyboard(ctx),
        )
    }

    fn event_is_blocked_ui_reverse_focus_traversal(event: &Event) -> bool {
        matches!(
            event,
            Event::Key {
                key: Key::Tab,
                pressed: true,
                modifiers,
                ..
            } if modifiers.shift
                && !modifiers.ctrl
                && !modifiers.alt
                && !modifiers.command
        )
    }

    fn partition_blocked_ui_reverse_focus_traversal_events(
        events: Vec<Event>,
    ) -> (Vec<Event>, Vec<Event>) {
        let mut blocked_events = Vec::new();
        let mut remaining_events = Vec::new();

        for event in events {
            if Self::event_is_blocked_ui_reverse_focus_traversal(&event) {
                blocked_events.push(event);
            } else {
                remaining_events.push(event);
            }
        }

        (blocked_events, remaining_events)
    }

    fn event_is_terminal_key(event: &Event) -> bool {
        matches!(event, Event::Key { .. })
    }

    fn event_terminal_navigation_direction(event: &Event) -> Option<TerminalNavigationDirection> {
        match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if modifiers.ctrl && !modifiers.alt && !modifiers.shift => match key {
                Key::ArrowLeft => Some(TerminalNavigationDirection::Left),
                Key::ArrowRight => Some(TerminalNavigationDirection::Right),
                Key::ArrowUp => Some(TerminalNavigationDirection::Up),
                Key::ArrowDown => Some(TerminalNavigationDirection::Down),
                _ => None,
            },
            _ => None,
        }
    }

    fn event_is_terminal_post_ui_input(event: &Event) -> bool {
        matches!(
            event,
            Event::Text(_) | Event::Paste(_) | Event::Copy | Event::Cut
        )
    }

    fn partition_terminal_key_events(events: Vec<Event>) -> (Vec<Event>, Vec<Event>) {
        let mut terminal_events = Vec::new();
        let mut remaining_events = Vec::new();

        for event in events {
            if Self::event_is_terminal_key(&event)
                && Self::event_terminal_navigation_direction(&event).is_none()
            {
                terminal_events.push(event);
            } else {
                remaining_events.push(event);
            }
        }

        (terminal_events, remaining_events)
    }

    fn partition_terminal_navigation_shortcuts(
        events: Vec<Event>,
    ) -> (Vec<TerminalNavigationDirection>, Vec<Event>) {
        let mut directions = Vec::new();
        let mut remaining_events = Vec::new();

        for event in events {
            if let Some(direction) = Self::event_terminal_navigation_direction(&event) {
                directions.push(direction);
            } else {
                remaining_events.push(event);
            }
        }

        (directions, remaining_events)
    }

    fn capture_active_terminal_input(&self, ctx: &egui::Context) -> Vec<Event> {
        if !self.should_capture_terminal_keyboard(ctx) {
            return Vec::new();
        }

        ctx.input_mut(|input| {
            let events = std::mem::take(&mut input.events);
            let (terminal_events, remaining_events) = Self::partition_terminal_key_events(events);
            input.events = remaining_events;
            terminal_events
        })
    }

    fn take_buffered_terminal_input(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.buffered_terminal_input)
    }

    fn take_buffered_terminal_navigation_shortcuts(&mut self) -> Vec<TerminalNavigationDirection> {
        std::mem::take(&mut self.buffered_terminal_navigation)
    }

    fn take_terminal_navigation_shortcuts(
        &mut self,
        ctx: &egui::Context,
    ) -> Vec<TerminalNavigationDirection> {
        let mut directions = self.take_buffered_terminal_navigation_shortcuts();
        directions.extend(ctx.input_mut(|input| {
            let events = std::mem::take(&mut input.events);
            let (directions, remaining_events) =
                Self::partition_terminal_navigation_shortcuts(events);
            input.events = remaining_events;
            directions
        }));
        directions
    }

    fn visible_terminal_ids_for_main(&self) -> Vec<u64> {
        let mut ids = self
            .terminals
            .iter()
            .filter_map(|(id, terminal)| self.terminal_visible_in_main(terminal).then_some(*id))
            .collect::<Vec<_>>();

        ids.sort_unstable();
        ids
    }

    fn route_active_terminal_input(&mut self, ctx: &egui::Context, events: Vec<Event>) {
        if self.ui_owns_keyboard(ctx) {
            self.pending_ctrl_c = None;
            return;
        }

        let now = ctx.input(|input| input.time);
        if self
            .pending_ctrl_c
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_ctrl_c = None;
        }

        let Some(active_terminal_id) = self.active_terminal_accepts_input() else {
            self.pending_ctrl_c = None;
            return;
        };
        let mut events = events;
        events.extend(
            ctx.input(|input| input.events.clone())
                .into_iter()
                .filter(Self::event_is_terminal_post_ui_input),
        );

        if events.is_empty() {
            return;
        }

        let Some(terminal) = self.terminals.get_mut(&active_terminal_id) else {
            self.pending_ctrl_c = None;
            return;
        };

        let mut outbound = Vec::new();
        let mut copied_selection = None;
        let mut armed_interrupt = false;

        for event in events {
            match event {
                Event::Copy => {
                    let has_selection = terminal
                        .selection
                        .as_ref()
                        .is_some_and(TerminalSelection::has_selection);
                    let (next_pending, action) = resolve_ctrl_c_action(
                        self.pending_ctrl_c,
                        active_terminal_id,
                        now,
                        has_selection,
                    );
                    self.pending_ctrl_c = next_pending;

                    match action {
                        CtrlCAction::CopySelection => {
                            copied_selection = Self::selected_terminal_text(terminal);
                            Self::clear_terminal_selection(terminal);
                            armed_interrupt = self.pending_ctrl_c.is_some();
                        }
                        CtrlCAction::ArmInterrupt => {
                            armed_interrupt = true;
                        }
                        CtrlCAction::SendInterrupt => {
                            Self::clear_terminal_selection(terminal);
                            outbound.push(0x03);
                        }
                    }
                }
                Event::Text(text) => {
                    if text.is_empty() {
                        continue;
                    }
                    self.pending_ctrl_c = None;
                    Self::clear_terminal_selection(terminal);
                    outbound.extend_from_slice(text.as_bytes());
                    Self::append_pending_line(&mut terminal.pending_line_for_title, &text);
                }
                Event::Paste(text) => {
                    if text.is_empty() {
                        continue;
                    }
                    self.pending_ctrl_c = None;
                    Self::clear_terminal_selection(terminal);
                    let text = Self::pasted_text(&text);
                    outbound.extend_from_slice(text.as_bytes());
                    Self::append_pending_line(&mut terminal.pending_line_for_title, &text);
                }
                Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } if pressed => {
                    if key == Key::Enter {
                        self.pending_ctrl_c = None;
                        Self::clear_terminal_selection(terminal);
                        outbound.push(b'\r');
                        let line = std::mem::take(&mut terminal.pending_line_for_title);
                        terminal.full_title = terminal_title_text(&line, terminal.id as usize);
                        terminal.title =
                            update_terminal_title(&line, terminal.id as usize, TITLE_MAX_LEN);
                        terminal.dirty = true;
                        continue;
                    }

                    if key == Key::Backspace {
                        self.pending_ctrl_c = None;
                        Self::clear_terminal_selection(terminal);
                        terminal.pending_line_for_title.pop();
                    }

                    if let Some(bytes) = Self::key_to_terminal_bytes(key, modifiers) {
                        self.pending_ctrl_c = None;
                        Self::clear_terminal_selection(terminal);
                        outbound.extend_from_slice(&bytes);
                    }
                }
                _ => {}
            }
        }

        if let Some(text) = copied_selection {
            ctx.copy_text(text);
            if armed_interrupt {
                self.status_line =
                    "Copied terminal selection. Press Ctrl+C again to interrupt".to_owned();
                ctx.request_repaint_after(Duration::from_secs_f64(CTRL_C_DOUBLE_PRESS_WINDOW_SECS));
            } else {
                self.status_line = "Copied terminal selection".to_owned();
                ctx.request_repaint();
            }
        } else if armed_interrupt {
            self.status_line = "Press Ctrl+C again to interrupt".to_owned();
            ctx.request_repaint_after(Duration::from_secs_f64(CTRL_C_DOUBLE_PRESS_WINDOW_SECS));
        }

        if !outbound.is_empty() {
            terminal.runtime.send_bytes(outbound);
            terminal.dirty = true;
            ctx.request_repaint();
        }
    }

    fn has_live_terminals(&self) -> bool {
        self.terminals.values().any(|terminal| !terminal.exited)
    }

    fn schedule_terminal_refresh(&self, ctx: &egui::Context) {
        if self.has_live_terminals() {
            ctx.request_repaint_after(Duration::from_millis(TERMINAL_FALLBACK_REFRESH_MS));
        }
    }

    fn ensure_theme_initialized(&mut self, ctx: &egui::Context) {
        if self.theme_initialized {
            return;
        }

        let mut fonts = egui::FontDefinitions::default();
        let fallback_fonts = fonts.font_data.keys().cloned().collect::<Vec<_>>();
        let icon_families = icon_fonts()
            .iter()
            .map(|asset| asset.family.to_owned())
            .collect::<Vec<_>>();
        for asset in icon_fonts() {
            fonts
                .font_data
                .insert(asset.family.to_owned(), FontData::from_static(asset.bytes));
            let family = fonts
                .families
                .entry(FontFamily::Name(asset.family.into()))
                .or_default();
            family.insert(0, asset.family.to_owned());
            for fallback in &fallback_fonts {
                if fallback != asset.family {
                    family.push(fallback.clone());
                }
            }
        }
        for ui_family in [FontFamily::Proportional, FontFamily::Monospace] {
            let family = fonts.families.entry(ui_family).or_default();
            for icon_family in icon_families.iter().rev() {
                if !family.iter().any(|name| name == icon_family) {
                    family.insert(0, icon_family.clone());
                }
            }
        }
        ctx.set_fonts(fonts);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.window_margin = egui::Margin::symmetric(12.0, 10.0);
        let mut scroll_style = egui::style::ScrollStyle::floating();
        // Keep scrollbars thin and low-contrast, even while hovered.
        scroll_style.bar_width = 3.2;
        scroll_style.floating_width = 1.2;
        scroll_style.handle_min_length = 16.0;
        scroll_style.active_background_opacity = 0.04;
        scroll_style.interact_background_opacity = 0.10;
        scroll_style.active_handle_opacity = 0.22;
        scroll_style.interact_handle_opacity = 0.38;
        style.spacing.scroll = scroll_style;
        style.visuals.window_rounding = 10.0.into();
        style.visuals.menu_rounding = 8.0.into();
        style.visuals.widgets.noninteractive.rounding = 7.0.into();
        style.visuals.widgets.inactive.rounding = 7.0.into();
        style.visuals.widgets.hovered.rounding = 7.0.into();
        style.visuals.widgets.active.rounding = 7.0.into();
        style.visuals.widgets.open.rounding = 7.0.into();

        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(TEXT_PRIMARY);
        visuals.panel_fill = SURFACE_BG;
        visuals.window_fill = SURFACE_BG;
        visuals.faint_bg_color = SURFACE_BG_SOFT;
        visuals.extreme_bg_color = Color32::from_rgb(18, 30, 44);
        visuals.code_bg_color = Color32::from_rgb(12, 16, 22);
        visuals.hyperlink_color = ACCENT;
        visuals.window_stroke = Stroke::new(1.0, BORDER_COLOR);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(24, 45, 66);
        visuals.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(22, 38, 56);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(55, 95, 128));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(26, 66, 98);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(23, 55, 83);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(76, 122, 162));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(33, 86, 128);
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(30, 76, 113);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::from_rgb(240, 247, 255));
        visuals.widgets.active.bg_fill = Color32::from_rgb(20, 112, 166);
        visuals.widgets.active.weak_bg_fill = Color32::from_rgb(18, 96, 145);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_rgb(244, 251, 255));
        visuals.widgets.open.bg_fill = Color32::from_rgb(28, 78, 118);
        visuals.widgets.open.weak_bg_fill = Color32::from_rgb(24, 64, 98);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, Color32::from_rgb(74, 126, 168));
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, Color32::from_rgb(235, 245, 255));
        visuals.selection.bg_fill = Color32::from_rgb(18, 93, 136);
        visuals.selection.stroke = Stroke::new(1.0, ACCENT);

        style.visuals = visuals;
        ctx.set_style(style);
        self.theme_initialized = true;
    }

    #[cfg(target_os = "windows")]
    fn extract_window_hwnd(cc: &eframe::CreationContext<'_>) -> Option<isize> {
        use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
        let Ok(window_handle) = cc.window_handle() else {
            return None;
        };
        let RawWindowHandle::Win32(handle) = window_handle.as_raw() else {
            return None;
        };

        Some(handle.hwnd.get())
    }

    #[cfg(target_os = "windows")]
    fn apply_initial_window_bounds(&mut self, ctx: &egui::Context) {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MAXIMIZE};

        if self.window_layout_passes_remaining == 0 {
            return;
        }
        let Some(hwnd_value) = self.window_hwnd else {
            self.window_layout_passes_remaining = 0;
            return;
        };

        let hwnd = hwnd_value as HWND;
        if hwnd.is_null() {
            self.window_layout_passes_remaining = 0;
            return;
        }

        unsafe {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }

        self.window_layout_passes_remaining = self.window_layout_passes_remaining.saturating_sub(1);
        if self.window_layout_passes_remaining > 0 {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn extract_window_hwnd(_cc: &eframe::CreationContext<'_>) -> Option<isize> {
        None
    }

    #[cfg(not(target_os = "windows"))]
    fn apply_initial_window_bounds(&mut self, _ctx: &egui::Context) {}

    fn append_pending_line(pending: &mut String, text: &str) {
        for ch in text.chars() {
            if ch == '\r' || ch == '\n' {
                pending.clear();
                continue;
            }
            pending.push(ch);
        }
    }

    fn pasted_text(text: &str) -> &str {
        text
    }

    fn key_to_terminal_bytes(key: Key, modifiers: egui::Modifiers) -> Option<Vec<u8>> {
        if modifiers.ctrl && !modifiers.alt {
            if let Some(ctrl) = Self::ctrl_key_to_byte(key) {
                return Some(vec![ctrl]);
            }
        }

        if modifiers.ctrl || modifiers.alt || modifiers.command {
            return None;
        }

        let sequence = match (key, modifiers.shift) {
            (Key::Backspace, _) => b"\x08".as_slice(),
            (Key::Tab, true) => b"\x1b[Z".as_slice(),
            (Key::Tab, false) => b"\t".as_slice(),
            (Key::Escape, _) => b"\x1b".as_slice(),
            (Key::ArrowUp, _) => b"\x1b[A".as_slice(),
            (Key::ArrowDown, _) => b"\x1b[B".as_slice(),
            (Key::ArrowRight, _) => b"\x1b[C".as_slice(),
            (Key::ArrowLeft, _) => b"\x1b[D".as_slice(),
            (Key::Home, _) => b"\x1b[H".as_slice(),
            (Key::End, _) => b"\x1b[F".as_slice(),
            (Key::PageUp, _) => b"\x1b[5~".as_slice(),
            (Key::PageDown, _) => b"\x1b[6~".as_slice(),
            (Key::Delete, _) => b"\x1b[3~".as_slice(),
            (Key::Insert, _) => b"\x1b[2~".as_slice(),
            _ => return None,
        };

        Some(sequence.to_vec())
    }

    fn ctrl_key_to_byte(key: Key) -> Option<u8> {
        match key {
            Key::A => Some(0x01),
            Key::B => Some(0x02),
            Key::C => Some(0x03),
            Key::D => Some(0x04),
            Key::E => Some(0x05),
            Key::F => Some(0x06),
            Key::G => Some(0x07),
            Key::H => Some(0x08),
            Key::I => Some(0x09),
            Key::J => Some(0x0A),
            Key::K => Some(0x0B),
            Key::L => Some(0x0C),
            Key::M => Some(0x0D),
            Key::N => Some(0x0E),
            Key::O => Some(0x0F),
            Key::P => Some(0x10),
            Key::Q => Some(0x11),
            Key::R => Some(0x12),
            Key::S => Some(0x13),
            Key::T => Some(0x14),
            Key::U => Some(0x15),
            Key::V => Some(0x16),
            Key::W => Some(0x17),
            Key::X => Some(0x18),
            Key::Y => Some(0x19),
            Key::Z => Some(0x1A),
            _ => None,
        }
    }

    fn close_terminal(&mut self, ctx: &egui::Context, terminal_id: u64) {
        let Some((title, close_result)) = self.terminals.get(&terminal_id).map(|terminal| {
            let close_result = terminal.runtime.terminate();
            (terminal.title.clone(), close_result)
        }) else {
            return;
        };

        self.terminals.remove(&terminal_id);
        self.status_line = match close_result {
            Ok(()) => format!("Closed {title}"),
            Err(err) => format!("Closed {title} (cleanup failed: {err})"),
        };
        self.pending_ctrl_c = None;

        let remaining_terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        self.set_active_terminal(next_active_terminal_after_close(
            self.active_terminal,
            terminal_id,
            &remaining_terminal_ids,
        ));
        self.bump_layout_epoch();
        ctx.request_repaint();
    }

    fn set_active_terminal(&mut self, terminal_id: Option<u64>) {
        if self.active_terminal == terminal_id {
            return;
        }

        self.active_terminal = terminal_id;
        self.pending_ctrl_c = None;
        self.clear_terminal_selections_except(terminal_id);
    }

    fn clear_terminal_selections_except(&mut self, keep_terminal_id: Option<u64>) {
        for (terminal_id, terminal) in &mut self.terminals {
            if Some(*terminal_id) != keep_terminal_id {
                Self::clear_terminal_selection(terminal);
            }
        }
    }

    fn clear_terminal_selection(terminal: &mut TerminalEntry) {
        terminal.selection = None;
        terminal.selection_snapshot = None;
        terminal.selection_drag_active = false;
    }

    fn should_defer_terminal_snapshot(selection: Option<&TerminalSelection>) -> bool {
        selection.is_some()
    }

    fn acknowledge_deferred_terminal_snapshot(
        dirty: &mut bool,
        snapshot_refresh_deferred: &mut bool,
    ) {
        *dirty = false;
        *snapshot_refresh_deferred = true;
    }

    fn apply_terminal_snapshot(
        terminal: &mut TerminalEntry,
        snapshot: TerminalSnapshot,
        selection_snapshot: TerminalSelectionSnapshot,
    ) {
        Self::apply_terminal_snapshot_parts(
            &mut terminal.render_cache,
            &mut terminal.dirty,
            &mut terminal.snapshot_refresh_deferred,
            &mut terminal.selection_snapshot,
            snapshot,
            selection_snapshot,
        );
    }

    fn apply_terminal_snapshot_parts(
        render_cache: &mut TerminalSnapshot,
        dirty: &mut bool,
        snapshot_refresh_deferred: &mut bool,
        selection_snapshot: &mut Option<TerminalSelectionSnapshot>,
        snapshot: TerminalSnapshot,
        next_selection_snapshot: TerminalSelectionSnapshot,
    ) {
        *render_cache = snapshot;
        *dirty = false;
        *snapshot_refresh_deferred = false;
        *selection_snapshot = Some(next_selection_snapshot);
    }

    fn ensure_terminal_selection_snapshot(terminal: &mut TerminalEntry) {
        if terminal.selection_snapshot.is_none() {
            terminal.selection_snapshot = try_terminal_selection_snapshot(&terminal.runtime);
        }
    }

    fn selected_terminal_text(terminal: &mut TerminalEntry) -> Option<String> {
        Self::ensure_terminal_selection_snapshot(terminal);
        terminal
            .selection_snapshot
            .as_ref()
            .and_then(|snapshot| terminal_selection_text(snapshot, terminal.selection.as_ref()))
    }

    fn send_pasted_text_to_terminal(&mut self, terminal_id: u64, text: &str) -> bool {
        let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
            self.status_line = "Target terminal not found".to_owned();
            return false;
        };

        if terminal.exited {
            self.status_line = format!("{} is exited", terminal.title);
            return false;
        }

        let text = Self::pasted_text(text);
        if text.is_empty() {
            return false;
        }

        terminal.runtime.send_bytes(text.as_bytes().to_vec());
        Self::append_pending_line(&mut terminal.pending_line_for_title, &text);
        Self::clear_terminal_selection(terminal);
        terminal.dirty = true;
        true
    }

    fn paste_clipboard_to_terminal(&mut self, terminal_id: u64) {
        self.pending_ctrl_c = None;

        let text = match Clipboard::new()
            .map_err(|err| err.to_string())
            .and_then(|mut clipboard| clipboard.get_text().map_err(|err| err.to_string()))
        {
            Ok(text) => text,
            Err(err) => {
                self.status_line = format!("Clipboard read failed: {err}");
                return;
            }
        };

        if self.send_pasted_text_to_terminal(terminal_id, &text) {
            self.status_line = "Pasted clipboard into terminal".to_owned();
        }
    }

    fn send_saved_message_to_terminal(&mut self, terminal_id: u64, message: &str) {
        self.pending_ctrl_c = None;
        let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
            self.status_line = "Target terminal not found".to_owned();
            return;
        };

        if terminal.exited {
            self.status_line = format!("{} is exited", terminal.title);
            return;
        }

        let destination_title = terminal.title.clone();
        let mut outbound = message.as_bytes().to_vec();
        outbound.push(b'\r');
        terminal.runtime.send_bytes(outbound);
        Self::clear_terminal_selection(terminal);
        Self::append_pending_line(&mut terminal.pending_line_for_title, message);
        let line = std::mem::take(&mut terminal.pending_line_for_title);
        terminal.full_title = terminal_title_text(&line, terminal.id as usize);
        terminal.title = update_terminal_title(&line, terminal.id as usize, TITLE_MAX_LEN);
        terminal.dirty = true;
        self.status_line = format!("Sent saved message to {}", destination_title);
    }

    fn finalize_pointer_selection_copy(
        pending_ctrl_c: &mut Option<PendingCtrlC>,
        status_line: &mut String,
    ) {
        *pending_ctrl_c = None;
        *status_line = "Copied terminal selection".to_owned();
    }

    fn preferred_terminal_for_project(&self, project_id: u64) -> Option<u64> {
        if let Some(active_terminal_id) = self.active_terminal {
            if self
                .terminals
                .get(&active_terminal_id)
                .is_some_and(|terminal| terminal.project_id == project_id && !terminal.exited)
            {
                return Some(active_terminal_id);
            }
        }

        self.terminals
            .iter()
            .find(|(_, terminal)| {
                terminal.project_id == project_id
                    && terminal.kind == TerminalKind::Foreground
                    && !terminal.exited
            })
            .map(|(terminal_id, _)| *terminal_id)
            .or_else(|| {
                self.terminals
                    .iter()
                    .find(|(_, terminal)| terminal.project_id == project_id && !terminal.exited)
                    .map(|(terminal_id, _)| *terminal_id)
            })
    }

    fn draw_top_bar(&mut self, ctx: &egui::Context) -> egui::Rect {
        egui::TopBottomPanel::top("top_bar")
            .exact_height(TOP_BAR_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(SURFACE_BG)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{}  Mergen ADE", icons::TERMINAL_WINDOW))
                            .strong()
                            .size(15.0)
                            .color(ACCENT),
                    );
                    ui.add_space(6.0);
                    let remaining_width = ui.available_size_before_wrap().x.max(0.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(remaining_width, 28.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            if styled_icon_button(
                                ui,
                                icons::GEAR,
                                BTN_SUBTLE,
                                BTN_SUBTLE_HOVER,
                                BTN_ICON_ACTIVE,
                                "Settings",
                            ) {
                                self.show_settings_popup = true;
                            }
                        },
                    );
                });
            })
            .response
            .rect
    }

    fn main_area_size_from_chrome(
        &self,
        content_rect: egui::Rect,
        top_bar_rect: egui::Rect,
        activity_rect: Option<egui::Rect>,
        explorer_rect: Option<egui::Rect>,
    ) -> Vec2 {
        let mut width = content_rect.width();
        let height = content_rect.height() - top_bar_rect.height();

        if let Some(activity_rect) = activity_rect {
            width -= activity_rect.width();
        }
        if let Some(explorer_rect) = explorer_rect {
            width -= explorer_rect.width();
        }

        egui::vec2(width.max(1.0), height.max(1.0))
    }

    fn draw_activity_rail(&mut self, ctx: &egui::Context) -> Option<egui::Rect> {
        if !self.config.ui.show_project_explorer {
            return None;
        }

        let response = egui::SidePanel::left("activity_rail")
            .resizable(false)
            .exact_width(ACTIVITY_RAIL_WIDTH)
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .fill(SURFACE_BG)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::same(8.0)),
            )
            .show(ctx, |ui| {
                let mut should_persist = false;
                let directory_active = self.config.ui.show_project_explorer
                    && self.config.ui.project_explorer_expanded
                    && self.config.ui.left_sidebar_tab == LeftSidebarTab::Directory;
                let source_control_active = self.config.ui.show_project_explorer
                    && self.config.ui.project_explorer_expanded
                    && self.config.ui.left_sidebar_tab == LeftSidebarTab::SourceControl;
                let terminal_manager_active = self.config.ui.show_project_explorer
                    && self.config.ui.project_explorer_expanded
                    && self.config.ui.left_sidebar_tab == LeftSidebarTab::TerminalManager;

                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);

                    if self.config.ui.show_project_explorer
                        && styled_icon_toggle(
                            ui,
                            directory_active,
                            icons::TREE_VIEW,
                            "Open Directory",
                        )
                    {
                        self.config.ui.show_project_explorer = true;
                        if directory_active {
                            self.config.ui.project_explorer_expanded = false;
                        } else {
                            self.config.ui.project_explorer_expanded = true;
                            self.config.ui.left_sidebar_tab = LeftSidebarTab::Directory;
                        }
                        should_persist = true;
                    }

                    if self.config.ui.show_project_explorer {
                        ui.add_space(6.0);
                        if styled_icon_toggle(
                            ui,
                            source_control_active,
                            icons::GIT_BRANCH,
                            "Open Source Control",
                        ) {
                            self.config.ui.show_project_explorer = true;
                            if source_control_active {
                                self.config.ui.project_explorer_expanded = false;
                            } else {
                                self.config.ui.project_explorer_expanded = true;
                                self.config.ui.left_sidebar_tab = LeftSidebarTab::SourceControl;
                            }
                            should_persist = true;
                        }
                    }

                    ui.add_space(6.0);
                    if styled_icon_toggle(
                        ui,
                        terminal_manager_active,
                        icons::TERMINAL_WINDOW,
                        "Open Terminal Manager",
                    ) {
                        self.config.ui.show_project_explorer = true;
                        if terminal_manager_active {
                            self.config.ui.project_explorer_expanded = false;
                        } else {
                            self.config.ui.project_explorer_expanded = true;
                            self.config.ui.left_sidebar_tab = LeftSidebarTab::TerminalManager;
                        }
                        should_persist = true;
                    }
                });

                if should_persist {
                    self.note_ui_config_changed();
                    self.persist_config();
                }
            });

        Some(response.response.rect)
    }

    fn draw_project_explorer(&mut self, ctx: &egui::Context) -> Option<egui::Rect> {
        if !self.config.ui.show_project_explorer {
            return None;
        }

        let response = egui::SidePanel::left("project_explorer")
            .resizable(false)
            .exact_width(PROJECT_EXPLORER_WIDTH)
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .fill(SURFACE_BG)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::same(10.0)),
            )
            .show_animated(ctx, self.config.ui.project_explorer_expanded, |ui| {
                let panel_right = ui.max_rect().right();
                ui.set_width(ui.max_rect().width());

                let (panel_icon, panel_title) = match self.config.ui.left_sidebar_tab {
                    LeftSidebarTab::Directory => (icons::TREE_VIEW, "Directory"),
                    LeftSidebarTab::SourceControl => (icons::GIT_BRANCH, "Source Control"),
                    LeftSidebarTab::TerminalManager => (icons::TERMINAL_WINDOW, "Terminal Manager"),
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{panel_icon} {panel_title}"))
                            .strong()
                            .size(15.0)
                            .color(TEXT_PRIMARY),
                    );
                    if self.config.ui.left_sidebar_tab == LeftSidebarTab::Directory {
                        let remaining_width = ui.available_size_before_wrap().x.max(0.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(remaining_width, CONTROL_ROW_HEIGHT),
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                if styled_icon_button(
                                    ui,
                                    icons::FOLDER_PLUS,
                                    BTN_TEAL,
                                    BTN_TEAL_HOVER,
                                    BTN_ICON_ACTIVE,
                                    "Add Project",
                                ) {
                                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                        self.add_project(path);
                                    }
                                }
                            },
                        );
                    }
                });
                ui.separator();

                match self.config.ui.left_sidebar_tab {
                    LeftSidebarTab::Directory => {
                        let project_rows = self
                            .projects
                            .iter()
                            .map(|(project_id, project)| {
                                (
                                    *project_id,
                                    project.name.clone(),
                                    project.path.clone(),
                                    project.path.display().to_string(),
                                )
                            })
                            .collect::<Vec<_>>();

                        let selected_project_label = self
                            .selected_project
                            .and_then(|selected_id| {
                                project_rows
                                    .iter()
                                    .find(|(project_id, _, _, _)| *project_id == selected_id)
                                    .map(|(_, project_name, _, _)| {
                                        format!("{} {}", icons::FOLDER_OPEN, project_name)
                                    })
                            })
                            .unwrap_or_else(|| "No project selected".to_owned());

                        let mut refresh_index = false;
                        let selected_project_details =
                            self.selected_project.and_then(|selected_id| {
                                project_rows
                                    .iter()
                                    .find(|(project_id, _, _, _)| *project_id == selected_id)
                                    .cloned()
                            });
                        let previous_selected_project = self.selected_project;
                        ui.label(RichText::new("Project").color(TEXT_MUTED));
                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size.y = CONTROL_ROW_HEIGHT;
                            ui.horizontal(|ui| {
                                let button_group_width =
                                    30.0 * 3.0 + ui.spacing().item_spacing.x * 2.0;
                                let combo_width =
                                    (ui.available_width() - button_group_width).clamp(96.0, 150.0);
                                with_minimal_button_chrome(ui, |ui| {
                                    egui::ComboBox::from_id_salt("directory-project-select")
                                        .selected_text(selected_project_label)
                                        .icon(paint_minimal_combo_icon)
                                        .width(combo_width)
                                        .show_ui(ui, |ui| {
                                            for (project_id, project_name, _, _) in &project_rows {
                                                ui.selectable_value(
                                                    &mut self.selected_project,
                                                    Some(*project_id),
                                                    format!("{} {}", icons::FOLDER, project_name),
                                                );
                                            }
                                        });
                                });

                                ui.add_enabled_ui(selected_project_details.is_some(), |ui| {
                                    if styled_icon_button(
                                        ui,
                                        icons::COPY,
                                        BTN_SUBTLE,
                                        BTN_SUBTLE_HOVER,
                                        BTN_ICON_ACTIVE,
                                        "Copy Path",
                                    ) {
                                        if let Some((_, project_name, _, project_path_text)) =
                                            selected_project_details.as_ref()
                                        {
                                            ui.ctx().copy_text(project_path_text.clone());
                                            self.status_line = format!(
                                                "Copied path for project '{}'",
                                                project_name
                                            );
                                        }
                                    }
                                    if styled_icon_button(
                                        ui,
                                        icons::FOLDER_OPEN,
                                        BTN_SUBTLE,
                                        BTN_SUBTLE_HOVER,
                                        BTN_ICON_ACTIVE,
                                        "Open in Folder",
                                    ) {
                                        if let Some((_, project_name, project_path, _)) =
                                            selected_project_details.as_ref()
                                        {
                                            match open_in_file_explorer(project_path, false) {
                                                Ok(()) => {
                                                    self.status_line = format!(
                                                        "Opened project '{}' in Explorer",
                                                        project_name
                                                    );
                                                }
                                                Err(err) => {
                                                    self.status_line =
                                                        format!("Open folder failed: {err}");
                                                }
                                            }
                                        }
                                    }
                                    if styled_icon_button(
                                        ui,
                                        icons::ARROW_CLOCKWISE,
                                        BTN_ICON,
                                        BTN_ICON_HOVER,
                                        BTN_ICON_ACTIVE,
                                        "Refresh Directory Index",
                                    ) {
                                        refresh_index = true;
                                    }
                                });
                            });
                        });
                        if self.selected_project != previous_selected_project {
                            self.apply_selected_project_auto_tile_scope_and_refresh_layout(ctx);
                            self.note_selection_changed();
                            self.persist_config();
                        }
                        ui.add_sized(
                            [ui.available_width(), CONTROL_ROW_HEIGHT],
                            egui::TextEdit::singleline(&mut self.directory_search_query)
                                .id(Self::directory_search_input_id())
                                .hint_text("Search files and folders")
                                .vertical_align(Align::Center),
                        );
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .id_salt("directory-tree-scroll")
                            .max_height(ui.available_height())
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let search_query =
                                    self.directory_search_query.trim().to_lowercase();
                                let search_query =
                                    (!search_query.is_empty()).then_some(search_query);
                                if let Some(project_id) = self.selected_project {
                                    if refresh_index {
                                        self.request_directory_index_refresh(project_id, true);
                                    }
                                    self.request_directory_index_refresh(project_id, false);

                                    let mut status_line_update = None;
                                    {
                                        let Some(snapshot) =
                                            self.directory_index_state.get(&project_id)
                                        else {
                                            ui.label(
                                                RichText::new("Indexing files...").color(TEXT_MUTED),
                                            );
                                            return;
                                        };

                                        ui.label(
                                            RichText::new(format!("{} Files", icons::FOLDER_OPEN))
                                                .color(TEXT_MUTED)
                                                .strong(),
                                        );

                                        if snapshot.loading {
                                            ui.label(
                                                RichText::new("Indexing files...").color(TEXT_MUTED),
                                            );
                                        }

                                        if let Some(error) = &snapshot.last_error {
                                            ui.colored_label(Color32::LIGHT_RED, error);
                                        } else if !snapshot.loading {
                                            let mut matching_directories = HashSet::new();
                                            if let Some(query) = search_query.as_deref() {
                                                let _ = collect_matching_directory_paths(
                                                    &snapshot.root,
                                                    query,
                                                    false,
                                                    &mut matching_directories,
                                                );
                                            }

                                            let has_results = draw_folder_tree(
                                                ui,
                                                &snapshot.root,
                                                &mut status_line_update,
                                                search_query.as_deref(),
                                                false,
                                                search_query.as_deref().map(|_| &matching_directories),
                                            );

                                            if search_query.is_some() && !has_results {
                                                ui.label(
                                                    RichText::new("No matching files or folders")
                                                        .color(TEXT_MUTED),
                                                );
                                            }
                                            if snapshot.truncated {
                                                ui.label(
                                                    RichText::new(
                                                        "Index truncated for performance. Refine search or refresh after narrowing scope.",
                                                    )
                                                    .color(TEXT_MUTED),
                                                );
                                            }
                                        }
                                    }

                                    if let Some(status_line) = status_line_update {
                                        self.status_line = status_line;
                                    }
                                } else {
                                    ui.label(RichText::new("No project selected").color(TEXT_MUTED));
                                }
                            });
                    }
                    LeftSidebarTab::SourceControl => {
                                let project_rows = self
                                    .projects
                                    .iter()
                                    .map(|(project_id, project)| {
                                        (*project_id, project.name.clone())
                                    })
                                    .collect::<Vec<_>>();

                                if project_rows.is_empty() {
                                    ui.label(RichText::new("No projects added").color(TEXT_MUTED));
                                    return;
                                }

                                let mut should_persist_selection = false;
                                if self.selected_project.is_some_and(|selected_id| {
                                    !project_rows
                                        .iter()
                                        .any(|(project_id, _)| *project_id == selected_id)
                                }) {
                                    self.selected_project = None;
                                    should_persist_selection = true;
                                }

                                let selected_project_label = self
                                    .selected_project
                                    .and_then(|selected_id| {
                                        project_rows
                                            .iter()
                                            .find(|(project_id, _)| *project_id == selected_id)
                                            .map(|(_, project_name)| {
                                                format!("{} {}", icons::FOLDER_OPEN, project_name)
                                            })
                                    })
                                    .unwrap_or_else(|| "No project selected".to_owned());

                                let selected_project_details =
                                    self.selected_project.and_then(|selected_id| {
                                        project_rows
                                            .iter()
                                            .find(|(project_id, _)| *project_id == selected_id)
                                            .cloned()
                                    });
                                let previous_selected_project = self.selected_project;
                                ui.label(RichText::new("Project").color(TEXT_MUTED));
                                let mut refresh_status = false;
                                let mut fetch_and_refresh = false;
                                let mut open_project_folder = false;
                                ui.scope(|ui| {
                                    ui.spacing_mut().interact_size.y = CONTROL_ROW_HEIGHT;
                                    ui.horizontal(|ui| {
                                        let button_group_width =
                                            30.0 * 3.0 + ui.spacing().item_spacing.x * 2.0;
                                        let combo_width = (ui.available_width()
                                            - button_group_width)
                                            .clamp(96.0, 150.0);
                                        with_minimal_button_chrome(ui, |ui| {
                                            egui::ComboBox::from_id_salt(
                                                "source-control-project-select",
                                            )
                                            .selected_text(selected_project_label)
                                            .icon(paint_minimal_combo_icon)
                                            .width(combo_width)
                                            .show_ui(ui, |ui| {
                                                for (project_id, project_name) in &project_rows {
                                                    ui.selectable_value(
                                                        &mut self.selected_project,
                                                        Some(*project_id),
                                                        format!(
                                                            "{} {}",
                                                            icons::FOLDER, project_name
                                                        ),
                                                    );
                                                }
                                            });
                                        });

                                        ui.add_enabled_ui(
                                            selected_project_details.is_some(),
                                            |ui| {
                                                if styled_icon_button(
                                                    ui,
                                                    icons::ARROW_CLOCKWISE,
                                                    BTN_ICON,
                                                    BTN_ICON_HOVER,
                                                    BTN_ICON_ACTIVE,
                                                    "Refresh Status",
                                                ) {
                                                    refresh_status = true;
                                                }
                                                if styled_icon_button(
                                                    ui,
                                                    icons::DOWNLOAD,
                                                    BTN_ICON,
                                                    BTN_ICON_HOVER,
                                                    BTN_ICON_ACTIVE,
                                                    "Fetch and Refresh",
                                                ) {
                                                    fetch_and_refresh = true;
                                                }
                                                if styled_icon_button(
                                                    ui,
                                                    icons::FOLDER_OPEN,
                                                    BTN_ICON,
                                                    BTN_ICON_HOVER,
                                                    BTN_ICON_ACTIVE,
                                                    "Open Project Folder",
                                                ) {
                                                    open_project_folder = true;
                                                }
                                            },
                                        );
                                    });
                                });
                                if self.selected_project != previous_selected_project {
                                    self.apply_selected_project_auto_tile_scope_and_refresh_layout(
                                        ctx,
                                    );
                                    should_persist_selection = true;
                                }
                                if should_persist_selection {
                                    self.note_selection_changed();
                                    self.persist_config();
                                }

                                let Some(project_id) = self.selected_project else {
                                    ui.label(
                                        RichText::new("No project selected").color(TEXT_MUTED),
                                    );
                                    return;
                                };
                                let Some(project) = self.projects.get(&project_id).cloned() else {
                                    ui.label(RichText::new("Project not found").color(TEXT_MUTED));
                                    return;
                                };

                                if !self.source_control_state.contains_key(&project_id) {
                                    self.request_source_control_refresh(project_id, false);
                                }

                                if refresh_status {
                                    self.request_source_control_refresh(project_id, false);
                                }
                                if fetch_and_refresh {
                                    self.request_source_control_refresh(project_id, true);
                                }
                                if open_project_folder {
                                    match open_in_file_explorer(&project.path, false) {
                                        Ok(()) => {
                                            self.status_line =
                                                "Opened project folder".to_owned();
                                        }
                                        Err(err) => {
                                            self.status_line =
                                                format!("Open folder failed: {err}");
                                        }
                                    }
                                }
                                ui.separator();

                                egui::ScrollArea::vertical()
                                    .id_salt("source-control-scroll")
                                    .max_height(ui.available_height())
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        let snapshot = self
                                            .source_control_state
                                            .entry(project_id)
                                            .or_insert_with(SourceControlSnapshot::default)
                                            .clone();

                                        if snapshot.loading {
                                            ui.label(
                                                RichText::new("Refreshing source control...")
                                                    .color(TEXT_MUTED),
                                            );
                                        }
                                        if let Some(error) = &snapshot.last_error {
                                            ui.colored_label(Color32::LIGHT_RED, error);
                                        } else {
                                            let mut branch_line = format!(
                                                "{} {}",
                                                icons::GIT_BRANCH,
                                                snapshot.branch
                                            );
                                            if snapshot.ahead > 0 || snapshot.behind > 0 {
                                                branch_line.push_str(&format!(
                                                    "  ahead:{} behind:{}",
                                                    snapshot.ahead, snapshot.behind
                                                ));
                                            }
                                            ui.label(RichText::new(branch_line).color(TEXT_MUTED));
                                        }

                                        ui.separator();
                                        if snapshot.files.is_empty()
                                            && snapshot.last_error.is_none()
                                            && !snapshot.loading
                                        {
                                            ui.label(
                                                RichText::new("Working tree is clean")
                                                    .color(TEXT_MUTED),
                                            );
                                        }

                                        for file in snapshot.files {
                                            let absolute = project.path.join(&file.path);
                                            ui.horizontal(|ui| {
                                                let status_icon = if file.staged {
                                                    icons::CHECK_CIRCLE
                                                } else {
                                                    icons::CLOCK
                                                };
                                                ui.label(
                                                    RichText::new(status_icon.to_string())
                                                        .color(TEXT_MUTED),
                                                );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} {}",
                                                    file.status, file.path
                                                ))
                                                .monospace()
                                                .small(),
                                            )
                                            .context_menu(|ui| {
                                                with_minimal_button_chrome(ui, |ui| {
                                                    if ui
                                                        .button(format!(
                                                            "{} Open in Folder",
                                                            icons::FOLDER_OPEN
                                                        ))
                                                        .clicked()
                                                    {
                                                        match open_in_file_explorer(&absolute, true)
                                                        {
                                                            Ok(()) => {
                                                                self.status_line =
                                                                    "Opened containing folder"
                                                                        .to_owned();
                                                            }
                                                            Err(err) => {
                                                                self.status_line = format!(
                                                                    "Open folder failed: {err}"
                                                                );
                                                            }
                                                        }
                                                        ui.close_menu();
                                                    }
                                                    if ui
                                                        .button(format!(
                                                            "{} Copy Relative Path",
                                                            icons::COPY
                                                        ))
                                                        .clicked()
                                                    {
                                                        ui.ctx().copy_text(file.path.clone());
                                                        self.status_line =
                                                            "Copied relative path".to_owned();
                                                        ui.close_menu();
                                                    }
                                                });
                                                });
                                            });
                                        }
                                    });
                    }
                    LeftSidebarTab::TerminalManager => {
                        self.draw_terminal_manager_contents(ctx, ui);
                    }
                }
                ui.expand_to_include_x(panel_right);
            });
        response.map(|inner| inner.response.rect)
    }

    fn draw_terminal_manager_contents(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let panel_right = ui.max_rect().right();
        ui.set_width(ui.max_rect().width());

        let mut project_ids = self.projects.keys().copied().collect::<Vec<_>>();
        project_ids.sort_unstable();

        for project_id in project_ids {
            if self.config.ui.project_filter_mode
                && self
                    .selected_project
                    .is_some_and(|selected| selected != project_id)
            {
                continue;
            }

            let Some(project_snapshot) = self.projects.get(&project_id).cloned() else {
                continue;
            };

            let project_path = project_snapshot.path.display().to_string();

            let header_label = format!("{} {}", icons::FOLDER_OPEN, project_snapshot.name);
            let header_id = ui.make_persistent_id(format!("project-group-{project_id}"));
            let mut header_state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                header_id,
                false,
            );
            let header_response =
                styled_flat_section_header(ui, &header_label, header_state.is_open());
            if header_response.clicked() {
                header_state.toggle(ui);
            }
            let foreground_count = self
                .terminals
                .values()
                .filter(|terminal| {
                    terminal.project_id == project_id && terminal.kind == TerminalKind::Foreground
                })
                .count();
            let background_count = self
                .terminals
                .values()
                .filter(|terminal| {
                    terminal.project_id == project_id && terminal.kind == TerminalKind::Background
                })
                .count();
            let _ = header_state.show_body_unindented(ui, |ui| {
                ui.horizontal(|ui| {
                    if styled_icon_button(
                        ui,
                        icons::TERMINAL,
                        BTN_BLUE,
                        BTN_BLUE_HOVER,
                        BTN_ICON_ACTIVE,
                        "New Foreground Terminal",
                    ) {
                        self.spawn_terminal_for_project(ctx, project_id, TerminalKind::Foreground);
                    }
                    if styled_icon_button(
                        ui,
                        icons::LIST,
                        BTN_TEAL,
                        BTN_TEAL_HOVER,
                        BTN_ICON_ACTIVE,
                        "New Background Terminal",
                    ) {
                        self.spawn_terminal_for_project(ctx, project_id, TerminalKind::Background);
                    }
                });

                if foreground_count > 0 {
                    ui.separator();
                    draw_meta_kicker(ui, icons::TERMINAL, "Foreground");
                    self.draw_terminal_rows(ctx, ui, project_id, TerminalKind::Foreground);
                }

                if background_count > 0 {
                    ui.separator();
                    draw_meta_kicker(ui, icons::LIST, "Background");
                    self.draw_terminal_rows(ctx, ui, project_id, TerminalKind::Background);
                }
            });

            header_response.context_menu(|ui| {
                with_minimal_button_chrome(ui, |ui| {
                    if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                        ui.ctx().copy_text(project_path.clone());
                        self.status_line =
                            format!("Copied path for project '{}'", project_snapshot.name);
                        ui.close_menu();
                    }
                    if ui
                        .button(format!("{} Open in Folder", icons::FOLDER_OPEN))
                        .clicked()
                    {
                        match open_in_file_explorer(&project_snapshot.path, false) {
                            Ok(()) => {
                                self.status_line = format!(
                                    "Opened project '{}' in Explorer",
                                    project_snapshot.name
                                );
                            }
                            Err(err) => {
                                self.status_line = format!("Open folder failed: {err}");
                            }
                        }
                        ui.close_menu();
                    }
                });
            });
            ui.add_space(4.0);
        }

        ui.expand_to_include_x(panel_right);
    }

    fn draw_terminal_rows(
        &mut self,
        ctx: &egui::Context,
        ui: &mut Ui,
        project_id: u64,
        kind: TerminalKind,
    ) {
        let ids = self
            .terminals
            .iter()
            .filter(|(_, terminal)| terminal.project_id == project_id && terminal.kind == kind)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let saved_messages = self
            .projects
            .get(&project_id)
            .map(|project| project.saved_messages.clone())
            .unwrap_or_default();
        let current_active = self.active_terminal;

        for terminal_id in ids {
            let mut set_active = false;
            let mut close_terminal = false;
            let mut visibility_changed = false;
            let mut send_message: Option<String> = None;
            let terminal_entry_id = {
                let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
                    continue;
                };
                let terminal_entry_id = terminal.id;
                let active = current_active == Some(terminal_entry_id);
                let label = terminal_display_label(&terminal.full_title, terminal.exited);
                let title_font = egui::TextStyle::Button.resolve(ui.style());
                let section_gap = ui.spacing().item_spacing.x;
                let actions_width = terminal_manager_actions_width(section_gap);
                let row_width = ui.available_width().max(0.0);
                let (row_label_width, row_actions_width) =
                    terminal_manager_row_widths(row_width, actions_width, section_gap);
                let row_height = ui.spacing().interact_size.y.max(CONTROL_ROW_HEIGHT);
                let (row_rect, _) =
                    ui.allocate_exact_size(egui::vec2(row_width, row_height), Sense::hover());

                if row_label_width > 0.0 {
                    let label_rect = egui::Rect::from_min_size(
                        row_rect.min,
                        egui::vec2(row_label_width, row_rect.height()),
                    );
                    let label_response = ui
                        .scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(label_rect)
                                .layout(Layout::left_to_right(Align::Center)),
                            |ui| {
                                let label_response =
                                    draw_truncated_selectable_label(ui, active, &label);
                                with_truncation_tooltip(
                                    ui,
                                    label_response,
                                    &label,
                                    &title_font,
                                    TEXT_PRIMARY,
                                )
                            },
                        )
                        .inner;
                    if label_response.clicked() {
                        set_active = true;
                    }
                }

                let actions_rect = egui::Rect::from_min_size(
                    egui::pos2(row_rect.right() - row_actions_width, row_rect.top()),
                    egui::vec2(row_actions_width, row_rect.height()),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(actions_rect)
                        .layout(Layout::right_to_left(Align::Center)),
                    |ui| {
                        if styled_icon_button(
                            ui,
                            icons::X,
                            BTN_RED,
                            BTN_RED_HOVER,
                            Color32::from_rgb(186, 58, 58),
                            "Close",
                        ) {
                            close_terminal = true;
                        }

                        let visibility_icon = if terminal.in_main_view {
                            icons::EYE
                        } else {
                            icons::EYE_OFF
                        };
                        let visibility_tooltip = if terminal.in_main_view {
                            "Hide from main area"
                        } else {
                            "Show in main area"
                        };
                        if styled_icon_toggle(
                            ui,
                            terminal.in_main_view,
                            visibility_icon,
                            visibility_tooltip,
                        ) {
                            terminal.in_main_view = !terminal.in_main_view;
                            visibility_changed = true;
                        }

                        let message_menu = with_minimal_button_chrome(ui, |ui| {
                            ui.menu_button(format!("{}", icons::CHAT_TEXT), |ui| {
                                with_minimal_button_chrome(ui, |ui| {
                                    if saved_messages.is_empty() {
                                        ui.label(
                                            RichText::new("No saved messages").color(TEXT_MUTED),
                                        );
                                        return;
                                    }

                                    for message in &saved_messages {
                                        if ui.button(message).clicked() {
                                            send_message = Some(message.clone());
                                            ui.close_menu();
                                        }
                                    }
                                });
                            })
                        });
                        message_menu.response.on_hover_text("Send saved message");
                    },
                );

                terminal_entry_id
            };

            if let Some(message) = send_message {
                self.send_saved_message_to_terminal(terminal_entry_id, &message);
            }

            if visibility_changed {
                self.bump_layout_epoch();
            }
            if set_active {
                self.set_active_terminal(Some(terminal_entry_id));
            }
            if close_terminal {
                self.close_terminal(ctx, terminal_entry_id);
            }
        }
    }

    fn draw_main_area(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(APP_BG))
            .show(ctx, |ui| {
                let visible_ids = self.visible_terminal_ids_for_main();

                if visible_ids.is_empty() {
                    let empty_state_rect = ui.available_rect_before_wrap();
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(empty_state_rect)
                            .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
                        |ui| {
                            ui.label(
                                RichText::new(format!("{}  No visible terminals", icons::TERMINAL))
                                    .size(20.0)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new("Select a project, then use New FG/New BG to start.")
                                    .color(TEXT_MUTED),
                            );
                        },
                    );
                    ui.allocate_rect(empty_state_rect, Sense::hover());
                    return;
                }

                let available = ui.available_size();
                if available.x < 160.0 || available.y < 120.0 {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Expand the window to render terminals")
                                .color(TEXT_MUTED),
                        );
                    });
                    return;
                }
                let grid = layout::compute_tile_grid(visible_ids.len(), available.x, available.y);

                let total_gap_x = TERMINAL_TILE_GAP_X * grid.cols.saturating_sub(1) as f32;
                let total_gap_y = TERMINAL_TILE_GAP_Y * grid.rows.saturating_sub(1) as f32;

                let pane_width = ((available.x - total_gap_x) / grid.cols as f32)
                    .floor()
                    .max(72.0);
                let pane_height = ((available.y - total_gap_y) / grid.rows as f32)
                    .floor()
                    .max(80.0);

                let origin = ui.cursor().min;

                // Use absolute rect positioning to bypass egui auto-layout entirely
                for row in 0..grid.rows {
                    for col in 0..grid.cols {
                        let index = row * grid.cols + col;
                        let Some(terminal_id) = visible_ids.get(index) else {
                            continue;
                        };

                        let x = origin.x + col as f32 * (pane_width + TERMINAL_TILE_GAP_X);
                        let y = origin.y + row as f32 * (pane_height + TERMINAL_TILE_GAP_Y);
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(x, y),
                            Vec2::new(pane_width, pane_height),
                        );

                        let inner_margin = TERMINAL_PANE_INNER_MARGIN;
                        let inner_size = Vec2::new(
                            (pane_width - inner_margin * 2.0).max(64.0),
                            (pane_height - inner_margin * 2.0).max(64.0),
                        );

                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rect)
                                .layout(Layout::top_down(Align::Min)),
                        );
                        child.set_clip_rect(rect);
                        child.spacing_mut().item_spacing = Vec2::ZERO;
                        egui::Frame::none()
                            .fill(SURFACE_BG)
                            .stroke(Stroke::new(1.0, BORDER_COLOR))
                            .rounding(10.0)
                            .inner_margin(egui::Margin::same(inner_margin))
                            .show(&mut child, |ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                self.draw_terminal_pane(ui, *terminal_id, inner_size);
                            });
                    }
                }

                // Reserve the full grid area so the CentralPanel knows the space is used
                let total_width = grid.cols as f32 * pane_width + total_gap_x;
                let total_height = grid.rows as f32 * pane_height + total_gap_y;
                ui.allocate_space(Vec2::new(total_width, total_height));
            });
    }

    fn draw_sidebar_seam_fix(
        &self,
        ctx: &egui::Context,
        explorer_rect: egui::Rect,
        terminal_rect: egui::Rect,
    ) {
        let top = explorer_rect.min.y.max(terminal_rect.min.y);
        let bottom = explorer_rect.max.y.min(terminal_rect.max.y);
        if bottom <= top {
            return;
        }

        let seam_left = explorer_rect.max.x.min(terminal_rect.min.x) - 1.0;
        let seam_right = explorer_rect.max.x.max(terminal_rect.min.x) + 1.0;
        let seam_rect =
            egui::Rect::from_min_max(egui::pos2(seam_left, top), egui::pos2(seam_right, bottom));

        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("sidebar-seam-fix"),
        ))
        .rect_filled(seam_rect, 0.0, SURFACE_BG);
    }

    fn draw_terminal_pane(&mut self, ui: &mut Ui, terminal_id: u64, pane_size: Vec2) {
        let project_name = self
            .terminals
            .get(&terminal_id)
            .and_then(|terminal| self.projects.get(&terminal.project_id))
            .map(|project| project.name.clone())
            .unwrap_or_else(|| "Unknown Project".to_owned());
        let is_active = self.active_terminal == Some(terminal_id);

        let (clicked, close_requested, copied_selection, paste_requested) = {
            let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
                return;
            };

            let mut close_requested = false;
            let mut pane_clicked = false;
            let mut copied_selection = None;
            let mut paste_requested = false;
            let header_fill = if is_active {
                Color32::from_rgb(24, 36, 50)
            } else {
                Color32::from_rgb(22, 32, 46)
            };
            let header_stroke = if is_active {
                Stroke::new(1.0, Color32::from_rgb(58, 72, 90))
            } else {
                Stroke::new(1.0, BORDER_COLOR)
            };
            let pane_width = pane_size.x.max(96.0);
            let pane_height = pane_size.y.max(124.0);

            let header_size = Vec2::new(pane_width, TERMINAL_HEADER_HEIGHT);
            ui.allocate_ui_with_layout(header_size, Layout::left_to_right(Align::Center), |ui| {
                ui.set_min_size(header_size);
                egui::Frame::none()
                    .fill(header_fill)
                    .stroke(header_stroke)
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_min_height(TERMINAL_HEADER_HEIGHT - 12.0);

                        let indicator_color = if is_active { ACCENT } else { TEXT_MUTED };
                        draw_terminal_header_dot(ui, indicator_color);
                        ui.add_space(4.0);
                        let title = terminal_display_label(&terminal.title, terminal.exited);
                        let title_font = egui::TextStyle::Body.resolve(ui.style());
                        let title_response = ui.add(
                            egui::Label::new(RichText::new(title).color(TEXT_PRIMARY))
                                .truncate()
                                .sense(Sense::click()),
                        );
                        let title_response = with_truncation_tooltip(
                            ui,
                            title_response,
                            &terminal.full_title,
                            &title_font,
                            TEXT_PRIMARY,
                        );
                        if title_response.clicked() {
                            pane_clicked = true;
                        }

                        ui.add_space(6.0);
                        draw_terminal_header_separator(ui);
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{} {}", icons::FOLDER, project_name))
                                    .color(TEXT_MUTED),
                            )
                            .truncate(),
                        );
                        ui.add_space(4.0);
                        draw_terminal_header_separator(ui);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(terminal.kind.label())
                                .small()
                                .strong()
                                .color(with_alpha(TEXT_MUTED, 230)),
                        );
                        if terminal.exited {
                            ui.colored_label(Color32::LIGHT_RED, "Exited");
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if styled_icon_button(
                                ui,
                                icons::X,
                                BTN_RED,
                                BTN_RED_HOVER,
                                Color32::from_rgb(186, 58, 58),
                                "Close",
                            ) {
                                close_requested = true;
                            }
                        });
                    });
            });
            if !close_requested {
                ui.add_space(TERMINAL_HEADER_GAP);

                let monospace = egui::TextStyle::Monospace;
                let font_id = monospace.resolve(ui.style());
                let char_width = ui
                    .fonts(|fonts| fonts.glyph_width(&font_id, 'W'))
                    .max(CELL_WIDTH_PX);
                let line_height = ui.text_style_height(&monospace).max(CELL_HEIGHT_PX);

                let output_height = (pane_height - TERMINAL_HEADER_HEIGHT - TERMINAL_HEADER_GAP)
                    .max(line_height * 2.0);
                let output_size = Vec2::new(pane_width, output_height);

                let cols = ((output_size.x / char_width).floor() as u16).max(8);
                let lines = ((output_size.y / line_height).floor() as u16).max(3);
                if output_size.x >= char_width * 8.0 && output_size.y >= line_height * 3.0 {
                    let resize_applied = terminal.runtime.resize(TerminalDimensions {
                        cols,
                        lines,
                        cell_width: char_width as u16,
                        cell_height: line_height as u16,
                    });
                    if !resize_applied {
                        ui.ctx()
                            .request_repaint_after(Duration::from_millis(TERMINAL_RETRY_MS));
                    }
                }

                let latest_seqno = terminal.runtime.latest_seqno();
                if latest_seqno > terminal.last_seqno {
                    terminal.last_seqno = latest_seqno;
                    terminal.dirty = true;
                }

                if terminal.dirty
                    || terminal.snapshot_refresh_deferred
                    || terminal.render_cache.lines.is_empty()
                {
                    if Self::should_defer_terminal_snapshot(terminal.selection.as_ref()) {
                        Self::acknowledge_deferred_terminal_snapshot(
                            &mut terminal.dirty,
                            &mut terminal.snapshot_refresh_deferred,
                        );
                        ui.ctx().request_repaint_after(Duration::from_millis(
                            TERMINAL_FALLBACK_REFRESH_MS,
                        ));
                    } else if let Some((snapshot, selection_snapshot)) =
                        try_terminal_snapshots(&terminal.runtime)
                    {
                        Self::apply_terminal_snapshot(terminal, snapshot, selection_snapshot);
                    } else {
                        ui.ctx()
                            .request_repaint_after(Duration::from_millis(TERMINAL_RETRY_MS));
                    }
                }

                let now = ui.ctx().input(|input| input.time);
                sync_terminal_cursor_row_state(terminal, now);

                ui.allocate_ui_with_layout(output_size, Layout::top_down(Align::Min), |ui| {
                    egui::Frame::none()
                        .fill(TERMINAL_OUTPUT_BG)
                        .stroke(Stroke::NONE)
                        .rounding(0.0)
                        .inner_margin(egui::Margin::same(0.0))
                        .outer_margin(egui::Margin::same(0.0))
                        .show(ui, |ui| {
                            ui.set_min_size(output_size);
                            egui::ScrollArea::vertical()
                                .id_salt(format!("term-output-{terminal_id}"))
                                .max_height(output_height)
                                .auto_shrink([false, false])
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    ui.set_width(output_size.x);
                                    ui.set_min_width(output_size.x);
                                    if terminal.render_cache.lines.is_empty() {
                                        let placeholder = WidgetText::from(
                                            RichText::new("Terminal is resizing...")
                                                .color(TEXT_MUTED),
                                        );
                                        let galley = placeholder.into_galley(
                                            ui,
                                            Some(TextWrapMode::Extend),
                                            output_size.x,
                                            egui::TextStyle::Monospace,
                                        );
                                        let (rect, response) = allocate_terminal_output_surface(
                                            ui,
                                            output_size,
                                            galley.size().y,
                                            Sense::click(),
                                        );
                                        ui.painter().galley(rect.min, galley, TEXT_MUTED);
                                        if response.clicked() {
                                            pane_clicked = true;
                                        }
                                        if response.secondary_clicked() {
                                            pane_clicked = true;
                                        }
                                        let can_paste = !terminal.exited;
                                        response.context_menu(|ui| {
                                            with_minimal_button_chrome(ui, |ui| {
                                                ui.add_enabled_ui(false, |ui| {
                                                    let _ =
                                                        ui.button(format!("{} Copy", icons::COPY));
                                                });
                                                ui.add_enabled_ui(can_paste, |ui| {
                                                    if ui
                                                        .button(format!(
                                                            "{} Paste",
                                                            icons::DOWNLOAD
                                                        ))
                                                        .clicked()
                                                    {
                                                        paste_requested = true;
                                                        ui.close_menu();
                                                    }
                                                });
                                            });
                                        });
                                    } else {
                                        let render = build_terminal_render(
                                            &terminal.render_cache,
                                            &font_id,
                                            terminal.exited,
                                            terminal.shell,
                                            terminal.stable_input_cursor_row,
                                            ui.ctx().input(|input| input.time),
                                        );
                                        let TerminalRenderModel {
                                            layout_job,
                                            cursor_overlay,
                                        } = render;
                                        let galley = ui.painter().layout_job(layout_job);
                                        let (rect, response) = allocate_terminal_output_surface(
                                            ui,
                                            output_size,
                                            galley.size().y,
                                            Sense::click_and_drag(),
                                        );
                                        ui.painter().galley(rect.min, galley, TEXT_PRIMARY);
                                        if response.drag_started_by(egui::PointerButton::Primary) {
                                            if let Some(point) =
                                                terminal_selection_point_from_pointer(
                                                    response.interact_pointer_pos(),
                                                    rect.min,
                                                    &terminal.render_cache,
                                                    char_width,
                                                    line_height,
                                                )
                                            {
                                                Self::ensure_terminal_selection_snapshot(terminal);
                                                terminal.selection =
                                                    Some(TerminalSelection::collapsed(point));
                                                terminal.selection_drag_active = true;
                                            }
                                        }
                                        if response.is_pointer_button_down_on()
                                            && ui.ctx().input(|input| input.pointer.primary_down())
                                        {
                                            pane_clicked = true;
                                            if terminal.selection.is_none() {
                                                if let Some(point) =
                                                    terminal_selection_point_from_pointer(
                                                        response.interact_pointer_pos(),
                                                        rect.min,
                                                        &terminal.render_cache,
                                                        char_width,
                                                        line_height,
                                                    )
                                                {
                                                    Self::ensure_terminal_selection_snapshot(
                                                        terminal,
                                                    );
                                                    terminal.selection =
                                                        Some(TerminalSelection::collapsed(point));
                                                    terminal.selection_drag_active = true;
                                                }
                                            }
                                        }
                                        if response.dragged_by(egui::PointerButton::Primary) {
                                            if let Some(point) =
                                                terminal_selection_point_from_pointer(
                                                    response.interact_pointer_pos(),
                                                    rect.min,
                                                    &terminal.render_cache,
                                                    char_width,
                                                    line_height,
                                                )
                                            {
                                                if terminal.selection.is_none() {
                                                    Self::ensure_terminal_selection_snapshot(
                                                        terminal,
                                                    );
                                                }
                                                let selection =
                                                    terminal.selection.get_or_insert_with(|| {
                                                        TerminalSelection::collapsed(point)
                                                    });
                                                selection.focus = point;
                                                terminal.selection_drag_active = true;
                                            }
                                        }
                                        if !ui.ctx().input(|input| input.pointer.primary_down()) {
                                            terminal.selection_drag_active = false;
                                        }
                                        if response.drag_stopped_by(egui::PointerButton::Primary)
                                            && terminal
                                                .selection
                                                .as_ref()
                                                .is_some_and(|selection| !selection.has_selection())
                                        {
                                            Self::clear_terminal_selection(terminal);
                                        } else if response
                                            .drag_stopped_by(egui::PointerButton::Primary)
                                        {
                                            terminal.selection_drag_active = false;
                                        }
                                        if response.clicked() {
                                            pane_clicked = true;
                                            Self::clear_terminal_selection(terminal);
                                        }
                                        if response.secondary_clicked() {
                                            pane_clicked = true;
                                        }

                                        if terminal.selection.is_some() {
                                            Self::ensure_terminal_selection_snapshot(terminal);
                                        }
                                        let can_copy = terminal
                                            .selection
                                            .as_ref()
                                            .is_some_and(TerminalSelection::has_selection)
                                            && terminal.selection_snapshot.is_some();
                                        let can_paste = !terminal.exited;
                                        let mut copy_requested = false;
                                        response.context_menu(|ui| {
                                            with_minimal_button_chrome(ui, |ui| {
                                                ui.add_enabled_ui(can_copy, |ui| {
                                                    if ui
                                                        .button(format!("{} Copy", icons::COPY))
                                                        .clicked()
                                                    {
                                                        copy_requested = true;
                                                        ui.close_menu();
                                                    }
                                                });
                                                ui.add_enabled_ui(can_paste, |ui| {
                                                    if ui
                                                        .button(format!(
                                                            "{} Paste",
                                                            icons::DOWNLOAD
                                                        ))
                                                        .clicked()
                                                    {
                                                        paste_requested = true;
                                                        ui.close_menu();
                                                    }
                                                });
                                            });
                                        });
                                        if copy_requested {
                                            copied_selection =
                                                Self::selected_terminal_text(terminal);
                                            Self::clear_terminal_selection(terminal);
                                        }
                                        paint_terminal_selection(
                                            ui,
                                            rect.min,
                                            &terminal.render_cache,
                                            terminal.selection.as_ref(),
                                            char_width,
                                            line_height,
                                        );
                                        if let Some(cursor_overlay) = cursor_overlay {
                                            paint_terminal_cursor(
                                                ui,
                                                rect.min,
                                                char_width,
                                                line_height,
                                                cursor_overlay,
                                            );
                                        }
                                    }
                                });
                        });
                });
            }

            (
                pane_clicked,
                close_requested,
                copied_selection,
                paste_requested,
            )
        };

        if close_requested {
            self.close_terminal(ui.ctx(), terminal_id);
            return;
        }

        if clicked {
            self.surrender_ui_text_focus(ui.ctx());
            self.pending_ctrl_c = None;
        }

        if clicked || copied_selection.is_some() || paste_requested {
            self.set_active_terminal(Some(terminal_id));
        }

        if let Some(text) = copied_selection {
            ui.ctx().copy_text(text);
            Self::finalize_pointer_selection_copy(&mut self.pending_ctrl_c, &mut self.status_line);
        }

        if paste_requested {
            self.paste_clipboard_to_terminal(terminal_id);
        }

        if clicked {
            ui.ctx().request_repaint();
        }
    }

    fn draw_settings_popup(&mut self, ctx: &egui::Context) {
        if !self.show_settings_popup {
            return;
        }

        let mut should_persist = false;
        let mut ui_config_changed = false;
        let mut default_shell_changed = false;
        let mut projects_changed = false;

        // Dark overlay backdrop
        egui::Area::new("settings_overlay".into())
            .fixed_pos(egui::pos2(0.0, 0.0))
            .order(egui::Order::Background)
            .interactable(true)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let response = ui.allocate_rect(screen, Sense::click());
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    Color32::from_rgba_premultiplied(0, 0, 0, 140),
                );
                if response.clicked() {
                    self.show_settings_popup = false;
                }
            });

        egui::Window::new(format!("{} Settings", icons::GEAR))
            .resizable(false)
            .collapsible(false)
            .movable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Application Settings")
                            .strong()
                            .size(16.0)
                            .color(TEXT_PRIMARY),
                    );
                    let remaining_width = ui.available_size_before_wrap().x.max(0.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(remaining_width, CONTROL_ROW_HEIGHT),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            if styled_icon_button(
                                ui,
                                icons::X,
                                BTN_SUBTLE,
                                BTN_SUBTLE_HOVER,
                                BTN_ICON_ACTIVE,
                                "Close",
                            ) {
                                self.show_settings_popup = false;
                            }
                        },
                    );
                });
                ui.separator();

                let mut filter_mode = self.config.ui.project_filter_mode;
                if ui
                    .checkbox(
                        &mut filter_mode,
                        "Filter Terminal Manager by Selected Project",
                    )
                    .changed()
                {
                    self.config.ui.project_filter_mode = filter_mode;
                    should_persist = true;
                    ui_config_changed = true;
                }

                ui.separator();

                self.config.ui.main_visibility_mode = MainVisibilityMode::Global;

                let previous_scope = self.config.ui.auto_tile_scope;
                egui::ComboBox::from_label("Auto Tile Scope")
                    .selected_text(self.config.ui.auto_tile_scope.label())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.config.ui.auto_tile_scope,
                            AutoTileScope::AllVisible,
                            AutoTileScope::AllVisible.label(),
                        );
                        ui.selectable_value(
                            &mut self.config.ui.auto_tile_scope,
                            AutoTileScope::SelectedProjectOnly,
                            AutoTileScope::SelectedProjectOnly.label(),
                        );
                    });
                if self.config.ui.auto_tile_scope != previous_scope {
                    self.apply_auto_tile_scope_and_refresh_layout(ui.ctx());
                    should_persist = true;
                    ui_config_changed = true;
                }

                let previous_shell = self.config.default_shell;
                egui::ComboBox::from_label("Default Shell")
                    .selected_text(self.config.default_shell.label())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.config.default_shell,
                            ShellKind::PowerShell,
                            ShellKind::PowerShell.label(),
                        );
                        ui.selectable_value(
                            &mut self.config.default_shell,
                            ShellKind::Cmd,
                            ShellKind::Cmd.label(),
                        );
                    });
                if self.config.default_shell != previous_shell {
                    should_persist = true;
                    default_shell_changed = true;
                }

                ui.separator();
                ui.label(
                    RichText::new(format!("{} Saved Messages", icons::CHAT_TEXT))
                        .strong()
                        .size(15.0)
                        .color(TEXT_PRIMARY),
                );

                let mut project_ids = self.projects.keys().copied().collect::<Vec<_>>();
                project_ids.sort_unstable();

                if project_ids.is_empty() {
                    ui.label(
                        RichText::new("Add a project to manage saved messages.").color(TEXT_MUTED),
                    );
                }

                for project_id in project_ids {
                    let Some(project_snapshot) = self.projects.get(&project_id).cloned() else {
                        continue;
                    };

                    let mut add_message: Option<String> = None;
                    let mut remove_message_index: Option<usize> = None;
                    let mut send_message_request: Option<String> = None;
                    let send_target_terminal = self.preferred_terminal_for_project(project_id);

                    egui::CollapsingHeader::new(format!(
                        "{} {}",
                        icons::FOLDER_OPEN,
                        project_snapshot.name
                    ))
                    .id_salt(format!("settings-saved-messages-{project_id}"))
                    .default_open(self.selected_project == Some(project_id))
                    .icon(paint_minimal_disclosure_icon)
                    .show(ui, |ui| {
                        if project_snapshot.saved_messages.is_empty() {
                            ui.label(
                                RichText::new("No saved messages for this project.")
                                    .color(TEXT_MUTED),
                            );
                        } else if send_target_terminal.is_none() {
                            ui.label(
                                RichText::new(
                                    "Open a live terminal in this project to send messages one by one.",
                                )
                                .color(TEXT_MUTED),
                            );
                        }

                        for (index, message) in project_snapshot.saved_messages.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let message_label = ui.add(
                                    egui::Label::new(RichText::new(message).monospace().small())
                                        .truncate(),
                                );
                                let _ = with_truncation_tooltip(
                                    ui,
                                    message_label,
                                    message,
                                    &egui::TextStyle::Monospace.resolve(ui.style()),
                                    TEXT_PRIMARY,
                                );

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if styled_icon_button(
                                        ui,
                                        icons::TRASH,
                                        BTN_RED,
                                        BTN_RED_HOVER,
                                        Color32::from_rgb(186, 58, 58),
                                        "Remove message",
                                    ) {
                                        remove_message_index = Some(index);
                                    }

                                    if let Some(_terminal_id) = send_target_terminal {
                                        if styled_icon_button(
                                            ui,
                                            icons::TERMINAL,
                                            BTN_BLUE,
                                            BTN_BLUE_HOVER,
                                            BTN_ICON_ACTIVE,
                                            "Send message",
                                        ) {
                                            send_message_request = Some(message.clone());
                                        }
                                    }
                                });
                            });
                        }

                        ui.horizontal(|ui| {
                            let draft = self.saved_message_drafts.entry(project_id).or_default();
                            ui.add(
                                egui::TextEdit::singleline(draft)
                                    .id(Self::saved_message_draft_input_id(project_id)),
                            );
                            if styled_icon_button(
                                ui,
                                icons::PLUS,
                                BTN_BLUE,
                                BTN_BLUE_HOVER,
                                BTN_ICON_ACTIVE,
                                "Add message",
                            ) {
                                let text = draft.trim();
                                if !text.is_empty() {
                                    add_message = Some(text.to_owned());
                                    draft.clear();
                                }
                            }
                        });
                    });

                    if let Some(project) = self.projects.get_mut(&project_id) {
                        if let Some(message) = add_message {
                            project.saved_messages.push(message);
                            should_persist = true;
                            projects_changed = true;
                        }
                        if let Some(index) = remove_message_index {
                            if index < project.saved_messages.len() {
                                project.saved_messages.remove(index);
                                should_persist = true;
                                projects_changed = true;
                            }
                        }
                    }

                    if let (Some(terminal_id), Some(message)) =
                        (send_target_terminal, send_message_request)
                    {
                        self.send_saved_message_to_terminal(terminal_id, &message);
                    }
                }

            });

        if should_persist {
            if ui_config_changed {
                self.note_ui_config_changed();
            }
            if default_shell_changed {
                self.note_default_shell_changed();
            }
            if projects_changed {
                self.note_projects_changed();
            }
            self.persist_config();
        }
    }
}

impl eframe::App for AdeApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let events = std::mem::take(&mut raw_input.events);
        let capture_keyboard = self.should_capture_terminal_keyboard(ctx);

        if capture_keyboard {
            let (terminal_events, remaining_events) = Self::partition_terminal_key_events(events);
            let (navigation_directions, remaining_events) =
                Self::partition_terminal_navigation_shortcuts(remaining_events);
            self.buffered_terminal_input.extend(terminal_events);
            self.buffered_terminal_navigation
                .extend(navigation_directions);
            raw_input.events = remaining_events;
            return;
        }

        let (_, remaining_events) =
            Self::partition_blocked_ui_reverse_focus_traversal_events(events);
        raw_input.events = remaining_events;
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_theme_initialized(ctx);
        self.apply_initial_window_bounds(ctx);
        self.process_terminal_events(ctx);
        self.process_source_control_events(ctx);
        self.process_directory_index_events(ctx);
        self.schedule_terminal_refresh(ctx);
        let mut terminal_events = self.take_buffered_terminal_input();
        terminal_events.extend(self.capture_active_terminal_input(ctx));
        let top_bar_rect = self.draw_top_bar(ctx);
        let activity_rect = self.draw_activity_rail(ctx);
        let explorer_rect = self.draw_project_explorer(ctx);
        let main_area_size = self.main_area_size_from_chrome(
            ctx.screen_rect(),
            top_bar_rect,
            activity_rect,
            explorer_rect,
        );
        self.handle_shortcuts(ctx, main_area_size);
        self.draw_main_area(ctx);
        if let (Some(activity_rect), Some(explorer_rect)) = (activity_rect, explorer_rect) {
            self.draw_sidebar_seam_fix(ctx, activity_rect, explorer_rect);
        }
        self.draw_settings_popup(ctx);

        self.route_active_terminal_input(ctx, terminal_events);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for terminal in self.terminals.values() {
            let _ = terminal.runtime.terminate();
        }

        self.persist_config();
    }
}

fn recover_config_state(
    current_config: &AppConfig,
    current_projects: &BTreeMap<u64, ProjectRecord>,
    current_selected_project: Option<u64>,
    loaded_config: AppConfig,
    pending_config_changes: PendingConfigChanges,
) -> AppConfig {
    let mut config = loaded_config;

    if pending_config_changes.default_shell {
        config.default_shell = current_config.default_shell;
    }

    config.ui.show_project_explorer = current_config.ui.show_project_explorer;
    config.ui.show_terminal_manager = current_config.ui.show_terminal_manager;
    config.ui.main_visibility_mode = current_config.ui.main_visibility_mode;
    config.ui.project_filter_mode = current_config.ui.project_filter_mode;

    if pending_config_changes.ui {
        config.ui.project_explorer_expanded = current_config.ui.project_explorer_expanded;
        config.ui.terminal_manager_expanded = current_config.ui.terminal_manager_expanded;
        config.ui.auto_tile_scope = current_config.ui.auto_tile_scope;
        config.ui.left_sidebar_tab = current_config.ui.left_sidebar_tab;
    }

    let (projects, project_id_remap) = recover_project_records(
        &config.projects,
        current_projects,
        pending_config_changes.projects,
    );

    let selected_project = if pending_config_changes.selection {
        valid_selected_project(
            current_selected_project.map(|project_id| {
                project_id_remap
                    .get(&project_id)
                    .copied()
                    .unwrap_or(project_id)
            }),
            &projects,
        )
    } else {
        valid_selected_project(config.ui.last_selected_project_id, &projects)
    };

    config.projects = projects.values().cloned().collect();
    config.ui.last_selected_project_id = selected_project;

    config
}

fn recover_project_records(
    loaded_projects: &[ProjectRecord],
    current_projects: &BTreeMap<u64, ProjectRecord>,
    merge_current_projects: bool,
) -> (BTreeMap<u64, ProjectRecord>, BTreeMap<u64, u64>) {
    let mut projects = loaded_projects
        .iter()
        .cloned()
        .map(|project| (project.id, project))
        .collect::<BTreeMap<_, _>>();
    let mut project_id_remap = BTreeMap::new();

    if !merge_current_projects {
        return (projects, project_id_remap);
    }

    let mut next_project_id = projects.keys().last().copied().unwrap_or(0) + 1;

    for current_project in current_projects.values() {
        if let Some((loaded_id, loaded_project)) =
            projects.iter().find_map(|(project_id, project)| {
                (project.path == current_project.path).then(|| (*project_id, project.clone()))
            })
        {
            let mut merged_project = loaded_project;
            merged_project.name = current_project.name.clone();
            merged_project.saved_messages = merge_saved_messages(
                &merged_project.saved_messages,
                &current_project.saved_messages,
            );
            projects.insert(loaded_id, merged_project);
            project_id_remap.insert(current_project.id, loaded_id);
            continue;
        }

        let target_project_id = if projects.contains_key(&current_project.id) {
            let assigned_id = next_project_id;
            next_project_id += 1;
            assigned_id
        } else {
            current_project.id
        };

        let mut project = current_project.clone();
        project.id = target_project_id;
        projects.insert(target_project_id, project);
        project_id_remap.insert(current_project.id, target_project_id);
        if target_project_id >= next_project_id {
            next_project_id = target_project_id + 1;
        }
    }

    (projects, project_id_remap)
}

fn merge_saved_messages(loaded_messages: &[String], current_messages: &[String]) -> Vec<String> {
    let mut merged_messages = Vec::with_capacity(loaded_messages.len() + current_messages.len());
    let mut seen_messages = HashSet::with_capacity(loaded_messages.len() + current_messages.len());

    for message in loaded_messages.iter().chain(current_messages.iter()) {
        if seen_messages.insert(message.clone()) {
            merged_messages.push(message.clone());
        }
    }

    merged_messages
}

fn valid_selected_project(
    selected_project: Option<u64>,
    projects: &BTreeMap<u64, ProjectRecord>,
) -> Option<u64> {
    selected_project.filter(|project_id| projects.contains_key(project_id))
}

fn collect_source_control_snapshot(project_path: &Path, run_fetch: bool) -> SourceControlSnapshot {
    let mut snapshot = SourceControlSnapshot {
        loading: false,
        ..SourceControlSnapshot::default()
    };

    if run_fetch {
        match run_git_command(project_path, &["fetch", "--all", "--prune"]) {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                snapshot.last_error = Some(if stderr.is_empty() {
                    "git fetch failed".to_owned()
                } else {
                    format!("Fetch failed: {stderr}")
                });
                return snapshot;
            }
            Err(err) => {
                snapshot.last_error = Some(format!("Fetch failed: {err}"));
                return snapshot;
            }
        }
    }

    let output = match run_git_command(project_path, &["status", "--porcelain", "--branch"]) {
        Ok(output) => output,
        Err(err) => {
            snapshot.last_error = Some(format!("Status failed: {err}"));
            return snapshot;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        snapshot.last_error = Some(if stderr.is_empty() {
            "Not a git repository".to_owned()
        } else {
            stderr
        });
        return snapshot;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            let (branch, ahead, behind) = parse_branch_header(header);
            snapshot.branch = branch;
            snapshot.ahead = ahead;
            snapshot.behind = behind;
            continue;
        }

        if line.len() < 3 {
            continue;
        }

        let code = &line[..2];
        let Some(path_part) = line.get(3..) else {
            continue;
        };
        let mut path = path_part.trim().to_owned();
        if let Some((_, new_path)) = path.split_once(" -> ") {
            path = new_path.trim().to_owned();
        }

        let bytes = code.as_bytes();
        if bytes.len() < 2 {
            continue;
        }
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let status_char = if x != ' ' && x != '?' { x } else { y };

        let status = match status_char {
            'M' => "Modified",
            'A' => "Added",
            'D' => "Deleted",
            'R' => "Renamed",
            'C' => "Copied",
            'U' => "Conflicted",
            '?' => "Untracked",
            '!' => "Ignored",
            _ => "Changed",
        };

        snapshot.files.push(SourceControlFile {
            path,
            status,
            staged: x != ' ' && x != '?',
        });
    }

    if snapshot.branch.is_empty() {
        snapshot.branch = "detached".to_owned();
    }

    snapshot
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    snapshot
}

fn run_git_command(project_path: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(project_path).args(args);
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.output()
}

fn parse_branch_header(header: &str) -> (String, usize, usize) {
    let (branch_part, tracking_part) = match header.split_once("...") {
        Some((branch, tail)) => (branch.trim().to_owned(), Some(tail)),
        None => {
            let branch = header.split_whitespace().next().unwrap_or("detached");
            (branch.to_owned(), None)
        }
    };

    let mut ahead = 0usize;
    let mut behind = 0usize;

    if let Some(tail) = tracking_part {
        if let Some(start) = tail.find('[') {
            if let Some(end) = tail[start..].find(']') {
                let flags = &tail[start + 1..start + end];
                for part in flags.split(',') {
                    let piece = part.trim();
                    if let Some(value) = piece.strip_prefix("ahead ") {
                        ahead = value.parse().unwrap_or(0);
                    } else if let Some(value) = piece.strip_prefix("behind ") {
                        behind = value.parse().unwrap_or(0);
                    }
                }
            }
        }
    }

    (branch_part, ahead, behind)
}

fn build_directory_root_node(path: &Path) -> DirectoryNode {
    let name = path
        .file_name()
        .map(|segment| segment.to_string_lossy().to_string())
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or_else(|| path.display().to_string());

    DirectoryNode {
        name,
        path: path.to_path_buf(),
        is_dir: true,
        children: Vec::new(),
    }
}

fn collect_directory_index_snapshot(project_path: &Path) -> DirectoryIndexSnapshot {
    let mut root = build_directory_root_node(project_path);
    let mut node_budget = DIRECTORY_INDEX_MAX_NODES;
    let mut truncated = false;

    let snapshot_error = match read_directory_children(
        project_path,
        0,
        DIRECTORY_INDEX_MAX_DEPTH,
        &mut node_budget,
        &mut truncated,
    ) {
        Ok(children) => {
            root.children = children;
            None
        }
        Err(err) => Some(format!("Directory index failed: {err}")),
    };

    DirectoryIndexSnapshot {
        root,
        loading: false,
        last_error: snapshot_error,
        truncated,
    }
}

fn read_directory_children(
    path: &Path,
    depth: usize,
    max_depth: usize,
    node_budget: &mut usize,
    truncated: &mut bool,
) -> Result<Vec<DirectoryNode>, String> {
    if depth >= max_depth || *node_budget == 0 {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(path).map_err(|err| err.to_string())?;
    let mut children_paths = entries
        .filter_map(|entry| entry.ok().map(|dir_entry| dir_entry.path()))
        .collect::<Vec<PathBuf>>();
    children_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    let mut children = Vec::new();
    for child_path in children_paths {
        if *node_budget == 0 {
            *truncated = true;
            break;
        }
        if let Some(node) =
            build_directory_node(&child_path, depth + 1, max_depth, node_budget, truncated)
        {
            children.push(node);
        }
    }

    Ok(children)
}

fn build_directory_node(
    path: &Path,
    depth: usize,
    max_depth: usize,
    node_budget: &mut usize,
    truncated: &mut bool,
) -> Option<DirectoryNode> {
    if *node_budget == 0 {
        *truncated = true;
        return None;
    }
    *node_budget -= 1;

    let name = path
        .file_name()
        .map(|segment| segment.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let is_dir = path.is_dir();

    let mut node = DirectoryNode {
        name,
        path: path.to_path_buf(),
        is_dir,
        children: Vec::new(),
    };

    if is_dir && depth < max_depth {
        if let Ok(children) =
            read_directory_children(path, depth, max_depth, node_budget, truncated)
        {
            node.children = children;
        }
    }

    Some(node)
}

fn open_in_file_explorer(path: &Path, select_file: bool) -> Result<(), String> {
    let mut command = Command::new("explorer.exe");
    if select_file {
        command.arg("/select,").arg(path);
    } else {
        command.arg(path);
    }

    match command.spawn() {
        Ok(_) => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn draw_folder_tree(
    ui: &mut Ui,
    root: &DirectoryNode,
    status_line_update: &mut Option<String>,
    search_query: Option<&str>,
    force_show_all_descendants: bool,
    matching_directories: Option<&HashSet<PathBuf>>,
) -> bool {
    let mut rendered_any = false;
    for item in &root.children {
        let item_name_lower = item.name.to_lowercase();
        let item_matches = search_query.is_some_and(|query| item_name_lower.contains(query));

        if item.is_dir {
            let should_show_dir = match search_query {
                Some(_) => matching_directories.is_some_and(|dirs| dirs.contains(&item.path)),
                None => true,
            };
            if !should_show_dir {
                continue;
            }
            rendered_any = true;

            let show_all_descendants =
                force_show_all_descendants || search_query.is_some() && item_matches;
            let header = egui::CollapsingHeader::new(item.name.clone())
                .id_salt(item.path.display().to_string())
                .open(search_query.map(|_| true))
                .icon(paint_minimal_disclosure_icon)
                .show(ui, |ui| {
                    let _ = draw_folder_tree(
                        ui,
                        item,
                        status_line_update,
                        search_query,
                        show_all_descendants,
                        matching_directories,
                    );
                });
            header.header_response.context_menu(|ui| {
                with_minimal_button_chrome(ui, |ui| {
                    if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                        let item_path_text = item.path.display().to_string();
                        ui.ctx().copy_text(item_path_text.clone());
                        *status_line_update = Some(format!("Copied path: {}", item_path_text));
                        ui.close_menu();
                    }
                });
            });
        } else {
            let should_show_file = match search_query {
                Some(_) => force_show_all_descendants || item_matches,
                None => true,
            };
            if !should_show_file {
                continue;
            }
            rendered_any = true;

            ui.label(item.name.clone()).context_menu(|ui| {
                with_minimal_button_chrome(ui, |ui| {
                    if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                        let item_path_text = item.path.display().to_string();
                        ui.ctx().copy_text(item_path_text.clone());
                        *status_line_update = Some(format!("Copied path: {}", item_path_text));
                        ui.close_menu();
                    }
                });
            });
        }
    }

    rendered_any
}

fn collect_matching_directory_paths(
    root: &DirectoryNode,
    query: &str,
    include_self: bool,
    matching_directories: &mut HashSet<PathBuf>,
) -> bool {
    let mut has_match = include_self && root.name.to_lowercase().contains(query);

    for child in &root.children {
        if child.is_dir {
            if collect_matching_directory_paths(child, query, true, matching_directories) {
                has_match = true;
            }
        } else if child.name.to_lowercase().contains(query) {
            has_match = true;
        }
    }

    if root.is_dir && has_match {
        matching_directories.insert(root.path.clone());
    }

    has_match
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    let [r, g, b, _] = color.to_array();
    Color32::from_rgba_premultiplied(r, g, b, alpha)
}

fn next_active_terminal_after_close(
    active_terminal: Option<u64>,
    closed_terminal_id: u64,
    remaining_terminal_ids: &[u64],
) -> Option<u64> {
    if active_terminal == Some(closed_terminal_id) {
        remaining_terminal_ids.first().copied()
    } else {
        active_terminal
    }
}

fn next_terminal_in_direction(
    active_terminal: Option<u64>,
    visible_terminal_ids: &[u64],
    grid: layout::TileGrid,
    direction: TerminalNavigationDirection,
) -> Option<u64> {
    if visible_terminal_ids.is_empty() || grid.cols == 0 || grid.rows == 0 {
        return None;
    }

    let active_terminal = active_terminal?;
    let active_index = visible_terminal_ids
        .iter()
        .position(|terminal_id| *terminal_id == active_terminal)?;
    let row = active_index / grid.cols;
    let column = active_index % grid.cols;

    let next_index = match direction {
        TerminalNavigationDirection::Left if column > 0 => Some(active_index - 1),
        TerminalNavigationDirection::Right => {
            let candidate = active_index + 1;
            (column + 1 < grid.cols && candidate < visible_terminal_ids.len()).then_some(candidate)
        }
        TerminalNavigationDirection::Up if row > 0 => Some(active_index - grid.cols),
        TerminalNavigationDirection::Down => {
            let candidate = active_index + grid.cols;
            (candidate < visible_terminal_ids.len()).then_some(candidate)
        }
        _ => None,
    }?;

    visible_terminal_ids.get(next_index).copied()
}

fn terminal_display_label(title: &str, exited: bool) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        if exited {
            "Terminal (Exited)".to_owned()
        } else {
            "Terminal".to_owned()
        }
    } else if exited {
        format!("{trimmed} (Exited)")
    } else {
        trimmed.to_owned()
    }
}

fn terminal_manager_actions_width(section_gap: f32) -> f32 {
    (CONTROL_ROW_HEIGHT * 2.0) + TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH + (section_gap * 2.0)
}

fn terminal_manager_row_widths(
    total_width: f32,
    preferred_actions_width: f32,
    section_gap: f32,
) -> (f32, f32) {
    let total_width = total_width.max(0.0);
    let preferred_actions_width = preferred_actions_width.max(0.0).min(total_width);
    let label_width = (total_width - preferred_actions_width - section_gap.max(0.0)).max(0.0);
    let actions_width = if label_width > 0.0 {
        preferred_actions_width
    } else {
        total_width
    };
    (label_width, actions_width)
}

fn draw_truncated_selectable_label(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let button_padding = ui.spacing().button_padding;
    let available_width = ui.available_width().max(0.0);
    let wrap_width = (available_width - (button_padding.x * 2.0)).max(0.0);
    let galley = WidgetText::from(text.to_owned()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        wrap_width,
        egui::TextStyle::Button,
    );
    let galley_text = galley.text().to_owned();
    let desired_size = egui::vec2(available_width, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(
            WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            &galley_text,
        )
    });

    if ui.is_rect_visible(response.rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        if selected || response.hovered() || response.highlighted() || response.has_focus() {
            let rect = rect.expand(visuals.expansion);
            ui.painter().rect(
                rect,
                visuals.rounding,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
            );
        }

        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), rect.shrink2(button_padding))
            .min;
        ui.painter().galley(text_pos, galley, visuals.text_color());
    }

    response
}

fn capped_hover_text(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            result.push('…');
            break;
        }
        result.push(ch);
    }
    result
}

fn with_truncation_tooltip(
    _ui: &Ui,
    response: egui::Response,
    text: &str,
    _font_id: &FontId,
    _color: Color32,
) -> egui::Response {
    if !text.trim().is_empty() {
        response.on_hover_text(capped_hover_text(text, 500))
    } else {
        response
    }
}

fn draw_terminal_header_dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 12.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
}

fn draw_terminal_header_separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(6.0, 14.0), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top()),
            egui::pos2(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, with_alpha(BORDER_COLOR, 180)),
    );
}

fn lerp_pos(a: egui::Pos2, b: egui::Pos2, t: f32) -> egui::Pos2 {
    egui::pos2(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn paint_minimal_disclosure_icon(ui: &mut Ui, openness: f32, response: &egui::Response) {
    let rect = response.rect;
    let center = rect.center();
    let stroke_color = if response.hovered() {
        Color32::from_rgb(244, 249, 255)
    } else {
        with_alpha(TEXT_MUTED, 210)
    };
    let stroke = Stroke::new(1.6, stroke_color);

    let closed = [
        egui::pos2(center.x - 2.0, center.y - 5.0),
        egui::pos2(center.x + 2.5, center.y),
        egui::pos2(center.x - 2.0, center.y + 5.0),
    ];
    let open = [
        egui::pos2(center.x - 5.0, center.y - 2.0),
        egui::pos2(center.x, center.y + 2.5),
        egui::pos2(center.x + 5.0, center.y - 2.0),
    ];

    let p0 = lerp_pos(closed[0], open[0], openness);
    let p1 = lerp_pos(closed[1], open[1], openness);
    let p2 = lerp_pos(closed[2], open[2], openness);

    ui.painter().line_segment([p0, p1], stroke);
    ui.painter().line_segment([p1, p2], stroke);
}

fn paint_minimal_combo_icon(
    ui: &Ui,
    rect: egui::Rect,
    visuals: &egui::style::WidgetVisuals,
    is_open: bool,
    _above_or_below: egui::AboveOrBelow,
) {
    let center = rect.center();
    let stroke = Stroke::new(1.6, visuals.fg_stroke.color);
    let top = if is_open {
        egui::pos2(center.x - 4.0, center.y + 1.5)
    } else {
        egui::pos2(center.x - 4.0, center.y - 1.5)
    };
    let mid = if is_open {
        egui::pos2(center.x, center.y - 2.5)
    } else {
        egui::pos2(center.x, center.y + 2.5)
    };
    let bottom = if is_open {
        egui::pos2(center.x + 4.0, center.y + 1.5)
    } else {
        egui::pos2(center.x + 4.0, center.y - 1.5)
    };

    ui.painter().line_segment([top, mid], stroke);
    ui.painter().line_segment([mid, bottom], stroke);
}

fn with_minimal_button_chrome<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        let style = ui.style_mut();
        style.spacing.button_padding = egui::vec2(8.0, 5.0);
        let hover_fill = with_alpha(BTN_ICON_HOVER, 110);

        style.visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, with_alpha(TEXT_PRIMARY, 190));

        style.visuals.widgets.hovered.bg_fill = hover_fill;
        style.visuals.widgets.hovered.weak_bg_fill = hover_fill;
        style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        style.visuals.widgets.hovered.fg_stroke =
            Stroke::new(1.0, Color32::from_rgb(244, 249, 255));

        style.visuals.widgets.active.bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.active.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.active.bg_stroke = Stroke::NONE;
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_rgb(244, 249, 255));

        style.visuals.widgets.open.bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.open.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.open.bg_stroke = Stroke::NONE;
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, Color32::from_rgb(244, 249, 255));

        add_contents(ui)
    })
    .inner
}

fn styled_flat_section_header(ui: &mut Ui, label: &str, open: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), CONTROL_ROW_HEIGHT),
        Sense::click(),
    );

    let text_color = if response.is_pointer_button_down_on() {
        Color32::from_rgb(244, 249, 255)
    } else if open {
        with_alpha(TEXT_PRIMARY, 232)
    } else if response.hovered() {
        with_alpha(TEXT_PRIMARY, 214)
    } else {
        with_alpha(TEXT_MUTED, 220)
    };
    ui.painter().text(
        egui::pos2(rect.left() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        text_color,
    );

    response
}

fn draw_meta_kicker(ui: &mut Ui, icon: AppIcon, label: &str) {
    let text = format!("{icon} {label}");
    ui.label(
        RichText::new(text)
            .small()
            .strong()
            .color(with_alpha(TEXT_MUTED, 230)),
    );
}

fn styled_icon_button(
    ui: &mut Ui,
    icon: AppIcon,
    _bg: Color32,
    _hover_bg: Color32,
    _active_bg: Color32,
    tooltip: &str,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(CONTROL_ROW_HEIGHT, CONTROL_ROW_HEIGHT),
        Sense::click(),
    );
    let response = response.on_hover_text(tooltip);

    if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(1.0), 8.0, with_alpha(BTN_ICON_HOVER, 110));
    }

    let icon_color = if response.is_pointer_button_down_on() || response.hovered() {
        Color32::from_rgb(244, 249, 255)
    } else {
        with_alpha(TEXT_PRIMARY, 178)
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{icon}"),
        egui::FontId::proportional(15.0),
        icon_color,
    );

    response.clicked()
}

fn styled_icon_toggle(ui: &mut Ui, selected: bool, icon: AppIcon, tooltip: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(CONTROL_ROW_HEIGHT, CONTROL_ROW_HEIGHT),
        Sense::click(),
    );
    let response = response.on_hover_text(tooltip);

    if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink(1.0), 8.0, with_alpha(BTN_ICON_HOVER, 110));
    }

    let icon_color = if selected || response.hovered() || response.is_pointer_button_down_on() {
        Color32::from_rgb(244, 249, 255)
    } else {
        with_alpha(TEXT_PRIMARY, 170)
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{icon}"),
        egui::FontId::proportional(14.0),
        icon_color,
    );

    response.clicked()
}

fn resolve_ctrl_c_action(
    pending: Option<PendingCtrlC>,
    terminal_id: u64,
    time_seconds: f64,
    has_selection: bool,
) -> (Option<PendingCtrlC>, CtrlCAction) {
    if pending.is_some_and(|pending| {
        pending.terminal_id == terminal_id && time_seconds <= pending.expires_at
    }) {
        return (None, CtrlCAction::SendInterrupt);
    }

    if has_selection {
        return (
            Some(PendingCtrlC {
                terminal_id,
                expires_at: time_seconds + CTRL_C_DOUBLE_PRESS_WINDOW_SECS,
            }),
            CtrlCAction::CopySelection,
        );
    }

    (
        Some(PendingCtrlC {
            terminal_id,
            expires_at: time_seconds + CTRL_C_DOUBLE_PRESS_WINDOW_SECS,
        }),
        CtrlCAction::ArmInterrupt,
    )
}

fn terminal_selection_point_from_pointer(
    pointer_pos: Option<egui::Pos2>,
    origin: egui::Pos2,
    snapshot: &TerminalSnapshot,
    char_width: f32,
    line_height: f32,
) -> Option<TerminalSelectionPoint> {
    let pointer_pos = pointer_pos?;
    if snapshot.lines.is_empty() || char_width <= 0.0 || line_height <= 0.0 {
        return None;
    }

    let max_row = snapshot.lines.len().saturating_sub(1);
    let row = (((pointer_pos.y - origin.y).max(0.0) / line_height).floor() as usize).min(max_row);
    let line_width = terminal_snapshot_line_width(&snapshot.lines[row]);
    let column =
        (((pointer_pos.x - origin.x).max(0.0) / char_width).floor() as usize).min(line_width);

    Some(TerminalSelectionPoint { row, column })
}

fn terminal_output_surface_size(output_size: Vec2, content_height: f32) -> Vec2 {
    egui::vec2(
        output_size.x.max(0.0),
        output_size.y.max(content_height.max(0.0)),
    )
}

fn allocate_terminal_output_surface(
    ui: &mut Ui,
    output_size: Vec2,
    content_height: f32,
    sense: Sense,
) -> (egui::Rect, egui::Response) {
    ui.allocate_exact_size(
        terminal_output_surface_size(output_size, content_height),
        sense,
    )
}

fn terminal_selection_text(
    snapshot: &TerminalSelectionSnapshot,
    selection: Option<&TerminalSelection>,
) -> Option<String> {
    let selection = selection?;
    if !selection.has_selection() || snapshot.lines.is_empty() {
        return None;
    }

    let (start, end) = selection.normalized();
    let last_row = snapshot.lines.len().saturating_sub(1);
    let start_row = start.row.min(last_row);
    let end_row = end.row.min(last_row);

    let mut rendered = String::new();
    for row in start_row..=end_row {
        let line = &snapshot.lines[row];
        let line_width = terminal_selection_line_width(line);
        let start_column = if row == start_row {
            start.column.min(line_width)
        } else {
            0
        };
        let end_column = if row == end_row {
            end.column.min(line_width)
        } else {
            line_width
        };
        if row > start_row && !snapshot.lines[row - 1].wraps_to_next {
            rendered.push('\n');
        }
        rendered.push_str(&slice_terminal_line_columns(line, start_column, end_column));
    }

    Some(rendered)
}

fn terminal_snapshot_line_width(line: &crate::terminal::TerminalStyledLine) -> usize {
    line.runs
        .last()
        .map(|run| run.column.saturating_add(run.display_width.max(1)))
        .unwrap_or(0)
}

fn terminal_selection_line_width(line: &TerminalSelectionLine) -> usize {
    line.width
}

fn slice_terminal_line_columns(line: &TerminalSelectionLine, start: usize, end: usize) -> String {
    if end <= start {
        return String::new();
    }

    let mut rendered = String::new();
    let mut column = start;

    for cell in &line.cells {
        let cell_end = cell.column.saturating_add(cell.display_width.max(1));
        if cell_end <= start {
            continue;
        }
        if cell.column >= end {
            break;
        }

        if cell.column > column {
            rendered.push_str(&" ".repeat(cell.column.min(end).saturating_sub(column)));
        }

        rendered.push_str(&cell.rendered_text());
        column = cell_end;
    }

    if column < end {
        rendered.push_str(&" ".repeat(end - column));
    }

    rendered
}

fn paint_terminal_selection(
    ui: &mut Ui,
    origin: egui::Pos2,
    snapshot: &TerminalSnapshot,
    selection: Option<&TerminalSelection>,
    char_width: f32,
    line_height: f32,
) {
    if snapshot.lines.is_empty() {
        return;
    }

    let Some(selection) = selection.filter(|selection| selection.has_selection()) else {
        return;
    };

    let (start, end) = selection.normalized();
    let fill = with_alpha(ui.visuals().selection.bg_fill, 92);

    for row in start.row..=end.row.min(snapshot.lines.len().saturating_sub(1)) {
        let line_width = terminal_snapshot_line_width(&snapshot.lines[row]);
        let start_column = if row == start.row {
            start.column.min(line_width)
        } else {
            0
        };
        let end_column = if row == end.row {
            end.column.min(line_width)
        } else {
            line_width
        };

        if end_column <= start_column {
            continue;
        }

        let rect = egui::Rect::from_min_size(
            egui::pos2(
                origin.x + start_column as f32 * char_width,
                origin.y + row as f32 * line_height,
            ),
            egui::vec2((end_column - start_column) as f32 * char_width, line_height),
        );
        ui.painter().rect_filled(rect, 2.0, fill);
    }
}

fn build_terminal_render(
    snapshot: &TerminalSnapshot,
    font_id: &FontId,
    terminal_exited: bool,
    shell: ShellKind,
    stable_input_cursor_row: Option<usize>,
    time_seconds: f64,
) -> TerminalRenderModel {
    let visible_cursor = visible_terminal_cursor(
        snapshot.cursor,
        terminal_exited,
        shell,
        stable_input_cursor_row,
        time_seconds,
    );
    let cursor_overlay =
        visible_cursor.and_then(|cursor| build_terminal_cursor_overlay(snapshot, cursor));
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;

    for (line_index, line) in snapshot.lines.iter().enumerate() {
        let block_cursor = visible_cursor
            .filter(|cursor| cursor.y == line_index && cursor.shape == TerminalCursorShape::Block);

        if let (Some(cursor), Some(cursor_line)) = (block_cursor, snapshot.cursor_line.as_ref()) {
            if cursor_line.row == line_index {
                for cell in &cursor_line.cells {
                    let style = if cell.covers_column(cursor.x) {
                        invert_terminal_style(cell.style)
                    } else {
                        cell.style
                    };
                    append_terminal_text(&mut job, &cell.rendered_text(), style, font_id);
                }
            } else {
                for run in &line.runs {
                    append_terminal_text(&mut job, &run.text, run.style, font_id);
                }
            }
        } else {
            for run in &line.runs {
                append_terminal_text(&mut job, &run.text, run.style, font_id);
            }
        }

        if line_index + 1 < snapshot.lines.len() {
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    ..TextFormat::default()
                },
            );
        }
    }

    TerminalRenderModel {
        layout_job: job,
        cursor_overlay,
    }
}

fn build_terminal_cursor_overlay(
    snapshot: &TerminalSnapshot,
    cursor: TerminalCursor,
) -> Option<TerminalCursorOverlay> {
    if cursor.shape == TerminalCursorShape::Block {
        return None;
    }

    let color = snapshot
        .cursor_line
        .as_ref()
        .filter(|line| line.row == cursor.y)
        .and_then(|line| line.cell_covering_column(cursor.x))
        .map(|cell| to_egui_color(cell.style.fg))
        .unwrap_or(TEXT_PRIMARY);

    Some(TerminalCursorOverlay {
        shape: cursor.shape,
        row: cursor.y,
        column: cursor.x,
        width_columns: 1,
        color,
    })
}

fn visible_terminal_cursor(
    cursor: Option<TerminalCursor>,
    terminal_exited: bool,
    shell: ShellKind,
    stable_input_cursor_row: Option<usize>,
    time_seconds: f64,
) -> Option<TerminalCursor> {
    cursor.filter(|cursor| {
        !terminal_exited
            && !cursor_hidden_by_row_filter(shell, stable_input_cursor_row, cursor.y)
            && (!cursor.blinking || terminal_cursor_blink_phase_visible(time_seconds))
    })
}

fn terminal_cursor_blink_phase_visible(time_seconds: f64) -> bool {
    ((time_seconds / CURSOR_BLINK_STEP_SECS).floor() as u64) % 2 == 0
}

fn cursor_hidden_by_row_filter(
    shell: ShellKind,
    stable_input_cursor_row: Option<usize>,
    cursor_row: usize,
) -> bool {
    shell == ShellKind::PowerShell && stable_input_cursor_row != Some(cursor_row)
}

fn sync_terminal_cursor_row_state(terminal: &mut TerminalEntry, time_seconds: f64) {
    let current_cursor_row = terminal.render_cache.cursor.map(|cursor| cursor.y);

    if terminal.shell != ShellKind::PowerShell {
        terminal.last_cursor_row = current_cursor_row;
        terminal.last_cursor_row_changed_at = None;
        terminal.stable_input_cursor_row = current_cursor_row;
        return;
    }

    update_stable_cursor_row(
        &mut terminal.last_cursor_row,
        &mut terminal.last_cursor_row_changed_at,
        &mut terminal.stable_input_cursor_row,
        current_cursor_row,
        time_seconds,
    );
}

fn update_stable_cursor_row(
    last_cursor_row: &mut Option<usize>,
    last_cursor_row_changed_at: &mut Option<f64>,
    stable_input_cursor_row: &mut Option<usize>,
    current_cursor_row: Option<usize>,
    time_seconds: f64,
) {
    if current_cursor_row != *last_cursor_row {
        *last_cursor_row = current_cursor_row;
        *last_cursor_row_changed_at = Some(time_seconds);
    }

    let Some(current_cursor_row) = current_cursor_row else {
        *stable_input_cursor_row = None;
        return;
    };

    if *stable_input_cursor_row == Some(current_cursor_row) {
        return;
    }

    if last_cursor_row_changed_at.is_some_and(|changed_at| {
        time_seconds >= changed_at
            && (time_seconds - changed_at) >= POWERSHELL_CURSOR_ROW_STABLE_SECS
    }) {
        *stable_input_cursor_row = Some(current_cursor_row);
    }
}

fn invert_terminal_style(style: crate::terminal::TerminalStyle) -> crate::terminal::TerminalStyle {
    crate::terminal::TerminalStyle {
        fg: style.bg,
        bg: style.fg,
        ..style
    }
}

fn append_terminal_text(
    job: &mut LayoutJob,
    text: &str,
    style: crate::terminal::TerminalStyle,
    font_id: &FontId,
) {
    let fg = to_egui_color(style.fg);
    let mut format = TextFormat {
        font_id: font_id.clone(),
        color: fg,
        background: normalize_terminal_background(style.bg),
        italics: style.italic,
        ..TextFormat::default()
    };

    if style.underline {
        format.underline = Stroke::new(1.0, fg);
    }
    if style.strike {
        format.strikethrough = Stroke::new(1.0, fg);
    }

    job.append(text, 0.0, format);
}

fn paint_terminal_cursor(
    ui: &mut Ui,
    origin: egui::Pos2,
    char_width: f32,
    line_height: f32,
    overlay: TerminalCursorOverlay,
) {
    if overlay.shape == TerminalCursorShape::Block {
        return;
    }

    let rect = terminal_cursor_overlay_rect(origin, char_width, line_height, overlay);
    ui.painter().rect_filled(rect, 0.0, overlay.color);
}

fn terminal_cursor_overlay_rect(
    origin: egui::Pos2,
    char_width: f32,
    line_height: f32,
    overlay: TerminalCursorOverlay,
) -> egui::Rect {
    let x = origin.x + (overlay.column as f32 * char_width);
    let y = origin.y + (overlay.row as f32 * line_height);
    let width = (overlay.width_columns.max(1) as f32 * char_width).max(1.0);

    match overlay.shape {
        TerminalCursorShape::Bar => egui::Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(CURSOR_BAR_WIDTH_PX.min(width), line_height),
        ),
        TerminalCursorShape::Underline => {
            let height = CURSOR_UNDERLINE_HEIGHT_PX.min(line_height.max(1.0));
            egui::Rect::from_min_size(
                egui::pos2(x, y + line_height - height),
                egui::vec2(width, height),
            )
        }
        TerminalCursorShape::Block => {
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, line_height))
        }
    }
}

fn to_egui_color(color: TerminalColor) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn normalize_terminal_background(color: TerminalColor) -> Color32 {
    let mapped = to_egui_color(color);
    if color.r <= 6 && color.g <= 6 && color.b <= 6 {
        TERMINAL_OUTPUT_BG
    } else {
        mapped
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_terminal_cursor_overlay, build_terminal_render, cursor_hidden_by_row_filter,
        next_active_terminal_after_close, next_terminal_in_direction,
        normalize_terminal_background, parse_branch_header, recover_config_state,
        resolve_ctrl_c_action, terminal_cursor_blink_phase_visible, terminal_cursor_overlay_rect,
        terminal_manager_actions_width, terminal_manager_row_widths, terminal_output_surface_size,
        terminal_selection_text, to_egui_color, update_stable_cursor_row, visible_terminal_cursor,
        AdeApp, CtrlCAction, PendingConfigChanges, PendingCtrlC, TerminalCursorOverlay,
        TerminalEntry, TerminalNavigationDirection, TerminalSelection, TerminalSelectionPoint,
        TERMINAL_OUTPUT_BG,
    };
    use crate::layout;
    use crate::models::{
        AppConfig, AutoTileScope, MainVisibilityMode, ProjectRecord, ShellKind, TerminalKind,
    };
    use crate::terminal::{
        test_terminal_runtime, TerminalColor, TerminalCursor, TerminalCursorLine,
        TerminalCursorShape, TerminalSelectionLine, TerminalSelectionSnapshot, TerminalSnapshot,
        TerminalStyle, TerminalStyledCell, TerminalStyledLine, TerminalStyledRun,
    };
    use eframe::egui::{
        self, pos2, Color32, Context, Event, FontFamily, FontId, Id, Key, Modifiers, RawInput,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn maps_navigation_keys_to_escape_sequences() {
        let up = AdeApp::key_to_terminal_bytes(Key::ArrowUp, Modifiers::default());
        let delete = AdeApp::key_to_terminal_bytes(Key::Delete, Modifiers::default());

        assert_eq!(up, Some(b"\x1b[A".to_vec()));
        assert_eq!(delete, Some(b"\x1b[3~".to_vec()));
    }

    #[test]
    fn partitions_terminal_key_events_out_of_ui_stream() {
        let shift_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        };
        let plain_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };
        let focus_event = Event::WindowFocused(true);

        let (terminal_events, remaining_events) = AdeApp::partition_terminal_key_events(vec![
            focus_event.clone(),
            shift_tab.clone(),
            plain_tab.clone(),
            Event::Text("git status".to_owned()),
        ]);

        assert_eq!(terminal_events, vec![shift_tab, plain_tab]);
        assert_eq!(
            remaining_events,
            vec![focus_event, Event::Text("git status".to_owned())]
        );
    }

    #[test]
    fn partitions_blocked_reverse_focus_events_out_of_raw_input() {
        let shift_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        };
        let plain_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };
        let ctrl_shift_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        };

        let (blocked_events, remaining_events) =
            AdeApp::partition_blocked_ui_reverse_focus_traversal_events(vec![
                shift_tab.clone(),
                plain_tab.clone(),
                ctrl_shift_tab.clone(),
            ]);

        assert_eq!(blocked_events, vec![shift_tab]);
        assert_eq!(remaining_events, vec![plain_tab, ctrl_shift_tab]);
    }

    #[test]
    fn ctrl_arrow_shortcuts_stay_out_of_terminal_stream() {
        let ctrl_right = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::default()
            },
        };

        let (terminal_events, remaining_events) =
            AdeApp::partition_terminal_key_events(vec![ctrl_right.clone()]);

        assert!(terminal_events.is_empty());
        assert_eq!(remaining_events, vec![ctrl_right]);
    }

    #[test]
    fn ctrl_shift_arrow_remains_terminal_input() {
        let ctrl_shift_right = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        };

        let (terminal_events, remaining_events) =
            AdeApp::partition_terminal_key_events(vec![ctrl_shift_right.clone()]);

        assert_eq!(terminal_events, vec![ctrl_shift_right]);
        assert!(remaining_events.is_empty());
    }

    #[test]
    fn ui_keyboard_ownership_blocks_terminal_capture() {
        assert!(AdeApp::ui_owns_keyboard_state(
            true, false, false, false, false
        ));
        assert!(AdeApp::ui_owns_keyboard_state(
            false, true, false, false, false
        ));
        assert!(AdeApp::ui_owns_keyboard_state(
            false, false, true, false, false
        ));
        assert!(AdeApp::ui_owns_keyboard_state(
            false, false, false, true, true
        ));
        assert!(!AdeApp::ui_owns_keyboard_state(
            false, false, false, false, false
        ));

        assert!(!AdeApp::should_capture_terminal_keyboard_state(true, true));
        assert!(AdeApp::should_capture_terminal_keyboard_state(true, false));
        assert!(!AdeApp::should_capture_terminal_keyboard_state(
            false, false
        ));
    }

    #[test]
    fn maps_ctrl_letters_to_control_bytes() {
        let modifiers = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };

        let ctrl_c = AdeApp::key_to_terminal_bytes(Key::C, modifiers);
        let ctrl_z = AdeApp::key_to_terminal_bytes(Key::Z, modifiers);

        assert_eq!(ctrl_c, Some(vec![0x03]));
        assert_eq!(ctrl_z, Some(vec![0x1a]));
    }

    #[test]
    fn capture_active_terminal_input_removes_keyboard_events_from_egui_queue() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![
                Event::Key {
                    key: Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    },
                },
                Event::PointerMoved(pos2(4.0, 8.0)),
                Event::Text("echo hi".to_owned()),
            ];
        });

        let app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let captured = app.capture_active_terminal_input(&ctx);

        assert_eq!(captured.len(), 1);
        assert!(matches!(
            &captured[0],
            Event::Key {
                key: Key::Tab,
                pressed: true,
                ..
            }
        ));

        let remaining_events = ctx.input(|input| input.events.clone());
        assert_eq!(
            remaining_events,
            vec![
                Event::PointerMoved(pos2(4.0, 8.0)),
                Event::Text("echo hi".to_owned())
            ]
        );
    }

    #[test]
    fn capture_active_terminal_input_leaves_ctrl_arrow_for_app_shortcuts() {
        let ctx = Context::default();
        let ctrl_right = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };
        ctx.input_mut(|input| {
            input.events = vec![ctrl_right.clone()];
        });

        let app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let captured = app.capture_active_terminal_input(&ctx);

        assert!(captured.is_empty());
        assert_eq!(ctx.input(|input| input.events.clone()), vec![ctrl_right]);
    }

    #[test]
    fn focused_directory_search_blocks_terminal_capture() {
        let ctx = Context::default();
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }];
        });

        let app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let captured = app.capture_active_terminal_input(&ctx);

        assert!(captured.is_empty());
        assert_eq!(ctx.input(|input| input.events.len()), 1);
    }

    #[test]
    fn surrender_ui_text_focus_clears_directory_search_focus() {
        let ctx = Context::default();
        let app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));

        app.surrender_ui_text_focus(&ctx);

        assert!(!ctx.memory(|mem| mem.has_focus(AdeApp::directory_search_input_id())));
    }

    #[test]
    fn focused_saved_message_draft_blocks_terminal_capture() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.selected_project = Some(7);
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::saved_message_draft_input_id(7)));
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }];
        });

        let captured = app.capture_active_terminal_input(&ctx);

        assert!(captured.is_empty());
        assert_eq!(ctx.input(|input| input.events.len()), 1);
    }

    #[test]
    fn surrender_ui_text_focus_clears_saved_message_draft_focus() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.selected_project = Some(7);
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::saved_message_draft_input_id(7)));

        app.surrender_ui_text_focus(&ctx);

        assert!(!ctx.memory(|mem| mem.has_focus(AdeApp::saved_message_draft_input_id(7))));
    }

    #[test]
    fn open_popup_blocks_terminal_capture() {
        let ctx = Context::default();
        let app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.memory_mut(|mem| mem.open_popup(Id::new("test-popup")));
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }];
        });

        let captured = app.capture_active_terminal_input(&ctx);

        assert!(captured.is_empty());
        assert_eq!(ctx.input(|input| input.events.len()), 1);
    }

    #[test]
    fn route_active_terminal_input_combines_buffered_keys_and_post_ui_text_events() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.input_mut(|input| {
            input.events = vec![Event::Text("git status".to_owned())];
        });

        app.route_active_terminal_input(
            &ctx,
            vec![Event::Key {
                key: Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.pending_line_for_title, "git status");
        assert!(terminal.dirty);
    }

    #[test]
    fn take_buffered_terminal_input_drains_pre_egui_events() {
        let shift_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        };
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.buffered_terminal_input = vec![shift_tab.clone()];

        let buffered_events = app.take_buffered_terminal_input();

        assert_eq!(buffered_events, vec![shift_tab]);
        assert!(app.buffered_terminal_input.is_empty());
    }

    #[test]
    fn take_buffered_terminal_navigation_shortcuts_drains_pre_egui_shortcuts() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.buffered_terminal_navigation = vec![TerminalNavigationDirection::Right];

        let directions = app.take_buffered_terminal_navigation_shortcuts();

        assert_eq!(directions, vec![TerminalNavigationDirection::Right]);
        assert!(app.buffered_terminal_navigation.is_empty());
    }

    #[test]
    fn handle_shortcuts_moves_to_visual_neighbor() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 7)),
                (4, test_terminal_entry(4, 7)),
            ],
            Some(1),
        );
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }];
        });

        app.handle_shortcuts(&ctx, egui::vec2(1600.0, 900.0));

        assert_eq!(app.active_terminal, Some(2));
        assert!(ctx.input(|input| input.events.is_empty()));
    }

    #[test]
    fn handle_shortcuts_ignores_shortcuts_when_ui_owns_keyboard() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }];
        });

        app.handle_shortcuts(&ctx, egui::vec2(1600.0, 900.0));

        assert_eq!(app.active_terminal, Some(1));
        assert_eq!(ctx.input(|input| input.events.len()), 1);
    }

    #[test]
    fn first_ctrl_c_arms_interrupt_without_sending_signal() {
        let (pending, action) = resolve_ctrl_c_action(None, 9, 2.0, false);

        assert_eq!(action, CtrlCAction::ArmInterrupt);
        let pending = pending.expect("expected pending ctrl+c state");
        assert_eq!(pending.terminal_id, 9);
        assert!((pending.expires_at - 2.75).abs() < f64::EPSILON);
    }

    #[test]
    fn second_ctrl_c_within_window_sends_interrupt() {
        let pending = Some(PendingCtrlC {
            terminal_id: 9,
            expires_at: 2.75,
        });
        let (next_pending, action) = resolve_ctrl_c_action(pending, 9, 2.4, false);

        assert_eq!(action, CtrlCAction::SendInterrupt);
        assert_eq!(next_pending, None);
    }

    #[test]
    fn ctrl_c_copies_selection_and_arms_interrupt_on_first_press() {
        let (next_pending, action) = resolve_ctrl_c_action(None, 9, 2.0, true);

        assert_eq!(action, CtrlCAction::CopySelection);
        let pending = next_pending.expect("expected pending ctrl+c state");
        assert_eq!(pending.terminal_id, 9);
        assert!((pending.expires_at - 2.75).abs() < f64::EPSILON);
    }

    #[test]
    fn ctrl_c_sends_interrupt_on_second_press_even_when_selection_exists() {
        let pending = Some(PendingCtrlC {
            terminal_id: 9,
            expires_at: 2.75,
        });
        let (next_pending, action) = resolve_ctrl_c_action(pending, 9, 2.4, true);

        assert_eq!(action, CtrlCAction::SendInterrupt);
        assert_eq!(next_pending, None);
    }

    #[test]
    fn should_defer_terminal_snapshot_while_selection_exists() {
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 1 },
            focus: TerminalSelectionPoint { row: 0, column: 4 },
        };

        assert!(AdeApp::should_defer_terminal_snapshot(Some(&selection)));
        assert!(!AdeApp::should_defer_terminal_snapshot(None));
    }

    #[test]
    fn deferred_terminal_snapshot_clears_dirty_latch_and_marks_refresh_pending() {
        let mut dirty = true;
        let mut snapshot_refresh_deferred = false;

        AdeApp::acknowledge_deferred_terminal_snapshot(&mut dirty, &mut snapshot_refresh_deferred);

        assert!(!dirty);
        assert!(snapshot_refresh_deferred);
    }

    #[test]
    fn pointer_copy_clears_pending_ctrl_c_state() {
        let mut pending_ctrl_c = Some(PendingCtrlC {
            terminal_id: 9,
            expires_at: 2.75,
        });
        let mut status_line = String::new();

        AdeApp::finalize_pointer_selection_copy(&mut pending_ctrl_c, &mut status_line);

        assert_eq!(pending_ctrl_c, None);
        assert_eq!(status_line, "Copied terminal selection");
    }

    #[test]
    fn applying_terminal_snapshot_replaces_cached_selection_snapshot() {
        let snapshot = TerminalSnapshot {
            lines: vec![TerminalStyledLine {
                runs: vec![TerminalStyledRun {
                    text: "next".to_owned(),
                    style: test_terminal_style(),
                    column: 0,
                    display_width: 4,
                }],
            }],
            ..TerminalSnapshot::default()
        };
        let mut render_cache = TerminalSnapshot::default();
        let mut dirty = true;
        let mut snapshot_refresh_deferred = true;
        let mut selection_snapshot = Some(TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("stale", 0, 5)], 5)],
        });
        let next_selection_snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("next", 0, 4)], 4)],
        };

        AdeApp::apply_terminal_snapshot_parts(
            &mut render_cache,
            &mut dirty,
            &mut snapshot_refresh_deferred,
            &mut selection_snapshot,
            snapshot.clone(),
            next_selection_snapshot.clone(),
        );

        assert_eq!(render_cache, snapshot);
        assert!(!dirty);
        assert!(!snapshot_refresh_deferred);
        assert_eq!(selection_snapshot, Some(next_selection_snapshot));
    }

    #[test]
    fn terminal_selection_text_joins_multiple_lines() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line(&[("echo test", 0, 9)], 9),
                test_selection_line(&[("next line", 0, 9)], 9),
            ],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 5 },
            focus: TerminalSelectionPoint { row: 1, column: 4 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "test\nnext");
    }

    #[test]
    fn terminal_selection_text_preserves_wide_cell_padding() {
        let style = test_terminal_style();
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![TerminalSelectionLine {
                width: 3,
                wraps_to_next: false,
                cells: vec![
                    TerminalStyledCell {
                        text: "你".to_owned(),
                        style,
                        column: 0,
                        display_width: 2,
                    },
                    TerminalStyledCell {
                        text: "x".to_owned(),
                        style,
                        column: 2,
                        display_width: 1,
                    },
                ],
            }],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 0 },
            focus: TerminalSelectionPoint { row: 0, column: 2 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "你 ");
    }

    #[test]
    fn terminal_selection_text_keeps_wide_character_when_drag_starts_mid_cell() {
        let style = test_terminal_style();
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![TerminalSelectionLine {
                width: 2,
                wraps_to_next: false,
                cells: vec![TerminalStyledCell {
                    text: "你".to_owned(),
                    style,
                    column: 0,
                    display_width: 2,
                }],
            }],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 1 },
            focus: TerminalSelectionPoint { row: 0, column: 2 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "你 ");
    }

    #[test]
    fn terminal_selection_text_reconstructs_internal_blank_columns() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("a", 0, 1), ("b", 2, 1)], 3)],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 0 },
            focus: TerminalSelectionPoint { row: 0, column: 3 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "a b");
    }

    #[test]
    fn terminal_selection_text_joins_soft_wrapped_rows_without_newline() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line_with_wrap(&[("hello", 0, 5)], 5, true),
                test_selection_line(&[(" world", 0, 6)], 6),
            ],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 0 },
            focus: TerminalSelectionPoint { row: 1, column: 6 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "hello world");
    }

    #[test]
    fn terminal_selection_text_inserts_newline_after_wrapped_logical_line_ends() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line_with_wrap(&[("hello", 0, 5)], 5, true),
                test_selection_line(&[(" world", 0, 6)], 6),
                test_selection_line(&[("next", 0, 4)], 4),
            ],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 0 },
            focus: TerminalSelectionPoint { row: 2, column: 4 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "hello world\nnext");
    }

    #[test]
    fn terminal_manager_row_reserves_gap_and_actions_width() {
        let actions_width = terminal_manager_actions_width(8.0);
        let (label_width, actions_area_width) =
            terminal_manager_row_widths(160.0, actions_width, 8.0);

        assert_eq!(actions_area_width, actions_width);
        assert!((label_width - 48.0).abs() < f32::EPSILON);
    }

    #[test]
    fn terminal_manager_row_gives_actions_full_width_when_space_is_tight() {
        let actions_width = terminal_manager_actions_width(8.0);
        let (label_width, actions_area_width) =
            terminal_manager_row_widths(70.0, actions_width, 8.0);

        assert_eq!(label_width, 0.0);
        assert_eq!(actions_area_width, 70.0);
    }

    #[test]
    fn terminal_output_surface_size_preserves_full_output_width() {
        let size = terminal_output_surface_size(egui::vec2(320.0, 180.0), 64.0);

        assert_eq!(size.x, 320.0);
        assert_eq!(size.y, 180.0);
    }

    #[test]
    fn terminal_output_surface_size_expands_for_taller_content() {
        let size = terminal_output_surface_size(egui::vec2(320.0, 180.0), 260.0);

        assert_eq!(size.x, 320.0);
        assert_eq!(size.y, 260.0);
    }

    #[test]
    fn closing_active_terminal_selects_first_remaining_terminal() {
        let next = next_active_terminal_after_close(Some(7), 7, &[3, 5]);

        assert_eq!(next, Some(3));
    }

    #[test]
    fn closing_inactive_terminal_keeps_current_active_terminal() {
        let next = next_active_terminal_after_close(Some(9), 7, &[3, 9]);

        assert_eq!(next, Some(9));
    }

    #[test]
    fn next_terminal_in_direction_moves_between_visual_neighbors() {
        let visible_ids = [1, 2, 3, 4];
        let grid = crate::layout::TileGrid { rows: 2, cols: 2 };

        let right = next_terminal_in_direction(
            Some(1),
            &visible_ids,
            grid,
            TerminalNavigationDirection::Right,
        );
        let down = next_terminal_in_direction(
            Some(1),
            &visible_ids,
            grid,
            TerminalNavigationDirection::Down,
        );

        assert_eq!(right, Some(2));
        assert_eq!(down, Some(3));
    }

    #[test]
    fn next_terminal_in_direction_blocks_moves_into_missing_last_row_cells() {
        let visible_ids = [1, 2, 3];
        let grid = crate::layout::TileGrid { rows: 2, cols: 2 };

        let blocked = next_terminal_in_direction(
            Some(2),
            &visible_ids,
            grid,
            TerminalNavigationDirection::Down,
        );

        assert_eq!(blocked, None);
    }

    #[test]
    fn next_terminal_in_direction_uses_visual_neighbors() {
        let grid = layout::TileGrid { rows: 2, cols: 2 };

        assert_eq!(
            next_terminal_in_direction(
                Some(1),
                &[1, 2, 3, 4],
                grid,
                TerminalNavigationDirection::Right,
            ),
            Some(2)
        );
        assert_eq!(
            next_terminal_in_direction(
                Some(1),
                &[1, 2, 3, 4],
                grid,
                TerminalNavigationDirection::Down,
            ),
            Some(3)
        );
        assert_eq!(
            next_terminal_in_direction(
                Some(4),
                &[1, 2, 3, 4],
                grid,
                TerminalNavigationDirection::Left,
            ),
            Some(3)
        );
        assert_eq!(
            next_terminal_in_direction(
                Some(4),
                &[1, 2, 3, 4],
                grid,
                TerminalNavigationDirection::Up,
            ),
            Some(2)
        );
    }

    #[test]
    fn next_terminal_in_direction_ignores_edges_and_empty_cells() {
        let grid = layout::TileGrid { rows: 2, cols: 2 };

        assert_eq!(
            next_terminal_in_direction(
                Some(1),
                &[1, 2, 3],
                grid,
                TerminalNavigationDirection::Left,
            ),
            None
        );
        assert_eq!(
            next_terminal_in_direction(
                Some(2),
                &[1, 2, 3],
                grid,
                TerminalNavigationDirection::Down,
            ),
            None
        );
        assert_eq!(
            next_terminal_in_direction(
                Some(3),
                &[1, 2, 3],
                grid,
                TerminalNavigationDirection::Right,
            ),
            None
        );
    }

    #[test]
    fn handle_shortcuts_moves_active_terminal_with_ctrl_arrow() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    command: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 7)),
                (4, test_terminal_entry(4, 7)),
            ],
            Some(1),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
    }

    #[test]
    fn handle_shortcuts_uses_buffered_navigation_shortcuts() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );
        app.buffered_terminal_navigation = vec![TerminalNavigationDirection::Right];

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
        assert!(app.buffered_terminal_navigation.is_empty());
    }

    #[test]
    fn handle_shortcuts_respects_ui_keyboard_ownership() {
        let ctx = Context::default();
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(1));
    }

    #[test]
    fn raw_input_hook_filters_shift_tab_when_terminal_wont_capture_keyboard() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));
        let mut raw_input = RawInput {
            events: vec![Event::Key {
                key: Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
            }],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
    }

    #[test]
    fn raw_input_hook_buffers_shift_tab_for_active_terminal() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let shift_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        };
        let mut raw_input = RawInput {
            events: vec![shift_tab.clone()],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(app.buffered_terminal_input, vec![shift_tab]);
    }

    #[test]
    fn raw_input_hook_buffers_plain_tab_for_active_terminal() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let plain_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };
        let mut raw_input = RawInput {
            events: vec![plain_tab.clone()],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(app.buffered_terminal_input, vec![plain_tab]);
    }

    #[test]
    fn raw_input_hook_buffers_ctrl_arrow_for_active_terminal() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let ctrl_right = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::default()
            },
        };
        let mut raw_input = RawInput {
            events: vec![ctrl_right],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(
            app.buffered_terminal_navigation,
            vec![TerminalNavigationDirection::Right]
        );
    }

    #[test]
    fn surrender_ui_text_focus_allows_ctrl_arrow_buffering_after_terminal_click() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));
        assert!(!app.should_capture_terminal_keyboard(&ctx));

        app.surrender_ui_text_focus(&ctx);

        let mut raw_input = RawInput {
            events: vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    command: true,
                    ..Modifiers::default()
                },
            }],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(
            app.buffered_terminal_navigation,
            vec![TerminalNavigationDirection::Right]
        );
    }

    #[test]
    fn event_terminal_navigation_direction_accepts_egui_command_alias_for_ctrl() {
        let direction = AdeApp::event_terminal_navigation_direction(&Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::default()
            },
        });

        assert_eq!(direction, Some(TerminalNavigationDirection::Down));
    }

    #[test]
    fn raw_input_hook_keeps_ctrl_shift_tab_available() {
        let ctx = Context::default();
        let mut app = test_app([], None);
        let ctrl_shift_tab = Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
        };
        let mut raw_input = RawInput {
            events: vec![ctrl_shift_tab.clone()],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert_eq!(raw_input.events, vec![ctrl_shift_tab]);
    }

    #[test]
    fn raw_input_hook_leaves_ctrl_arrow_when_ui_owns_keyboard() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));
        let ctrl_right = Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };
        let mut raw_input = RawInput {
            events: vec![ctrl_right.clone()],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert_eq!(raw_input.events, vec![ctrl_right]);
        assert!(app.buffered_terminal_navigation.is_empty());
    }

    #[test]
    fn close_terminal_removes_entry_and_clears_pending_interrupt_state() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );
        app.pending_ctrl_c = Some(PendingCtrlC {
            terminal_id: 1,
            expires_at: 4.0,
        });

        let ctx = eframe::egui::Context::default();
        app.close_terminal(&ctx, 1);

        assert!(!app.terminals.contains_key(&1));
        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.pending_ctrl_c, None);
        assert_eq!(app.layout_epoch, 1);
        assert_eq!(app.status_line, "Closed Terminal 1");
    }

    #[test]
    fn auto_tile_scope_selected_project_only_rewrites_existing_terminal_visibility() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 9)),
            ],
            Some(3),
        );
        app.config.ui.auto_tile_scope = AutoTileScope::SelectedProjectOnly;
        app.selected_project = Some(7);

        let changed = app.apply_auto_tile_scope_to_open_terminals();

        assert!(changed);
        assert!(app
            .terminals
            .get(&1)
            .is_some_and(|terminal| terminal.in_main_view));
        assert!(app
            .terminals
            .get(&2)
            .is_some_and(|terminal| terminal.in_main_view));
        assert!(app
            .terminals
            .get(&3)
            .is_some_and(|terminal| !terminal.in_main_view));
        assert_eq!(app.active_terminal, Some(1));
    }

    #[test]
    fn auto_tile_scope_all_visible_restores_all_open_terminals() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 9)),
            ],
            Some(1),
        );
        app.config.ui.auto_tile_scope = AutoTileScope::SelectedProjectOnly;
        app.selected_project = Some(7);
        let _ = app.apply_auto_tile_scope_to_open_terminals();

        app.config.ui.auto_tile_scope = AutoTileScope::AllVisible;

        let changed = app.apply_auto_tile_scope_to_open_terminals();

        assert!(changed);
        assert!(app
            .terminals
            .get(&1)
            .is_some_and(|terminal| terminal.in_main_view));
        assert!(app
            .terminals
            .get(&2)
            .is_some_and(|terminal| terminal.in_main_view));
        assert_eq!(app.active_terminal, Some(1));
    }

    #[test]
    fn selected_project_change_does_not_reshow_hidden_terminals_in_all_visible_mode() {
        let ctx = eframe::egui::Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 9)),
            ],
            Some(1),
        );
        app.config.ui.auto_tile_scope = AutoTileScope::AllVisible;
        app.selected_project = Some(7);
        app.terminals.get_mut(&2).expect("terminal 2").in_main_view = false;

        app.selected_project = Some(9);
        app.apply_selected_project_auto_tile_scope_and_refresh_layout(&ctx);

        assert!(app
            .terminals
            .get(&1)
            .is_some_and(|terminal| terminal.in_main_view));
        assert!(app
            .terminals
            .get(&2)
            .is_some_and(|terminal| !terminal.in_main_view));
        assert_eq!(app.active_terminal, Some(1));
        assert_eq!(app.layout_epoch, 0);
    }

    #[test]
    fn auto_tile_scope_selected_project_only_hides_all_terminals_without_selection() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 9)),
            ],
            Some(2),
        );
        app.config.ui.auto_tile_scope = AutoTileScope::SelectedProjectOnly;
        app.selected_project = None;

        let changed = app.apply_auto_tile_scope_to_open_terminals();

        assert!(changed);
        assert!(app
            .terminals
            .values()
            .all(|terminal| !terminal.in_main_view));
        assert_eq!(app.active_terminal, None);
    }

    #[test]
    fn auto_tile_scope_keeps_active_terminal_when_it_remains_visible() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 9)),
            ],
            Some(2),
        );
        app.config.ui.auto_tile_scope = AutoTileScope::SelectedProjectOnly;
        app.selected_project = Some(7);

        let changed = app.apply_auto_tile_scope_to_open_terminals();

        assert!(changed);
        assert_eq!(app.active_terminal, Some(2));
    }

    #[test]
    fn recovered_config_preserves_loaded_settings_until_session_changes_them() {
        let loaded_project = test_project(7, "Loaded", "C:/loaded/demo", &[]);
        let loaded_config = AppConfig {
            default_shell: ShellKind::Cmd,
            ui: crate::models::UiConfig {
                auto_tile_scope: AutoTileScope::SelectedProjectOnly,
                project_explorer_expanded: false,
                last_selected_project_id: Some(loaded_project.id),
                ..boot_failed_current_config().ui
            },
            projects: vec![loaded_project.clone()],
            ..AppConfig::default()
        };

        let recovered = recover_config_state(
            &boot_failed_current_config(),
            &BTreeMap::new(),
            None,
            loaded_config,
            PendingConfigChanges::default(),
        );

        assert_eq!(recovered.default_shell, ShellKind::Cmd);
        assert_eq!(
            recovered.ui.auto_tile_scope,
            AutoTileScope::SelectedProjectOnly
        );
        assert!(!recovered.ui.project_explorer_expanded);
        assert_eq!(
            recovered.ui.last_selected_project_id,
            Some(loaded_project.id)
        );
        assert_eq!(recovered.projects.len(), 1);
        assert_eq!(recovered.projects[0].id, loaded_project.id);
        assert_eq!(recovered.projects[0].name, loaded_project.name);
        assert_eq!(recovered.projects[0].path, loaded_project.path);
    }

    #[test]
    fn recovered_config_keeps_loaded_projects_when_added_project_reuses_hidden_id() {
        let loaded_project = test_project(1, "Loaded", "C:/loaded/demo", &[]);
        let current_project = test_project(1, "Added", "C:/added/demo", &[]);
        let loaded_config = AppConfig {
            projects: vec![loaded_project.clone()],
            ..AppConfig::default()
        };
        let current_projects = BTreeMap::from([(current_project.id, current_project.clone())]);

        let recovered = recover_config_state(
            &boot_failed_current_config(),
            &current_projects,
            Some(current_project.id),
            loaded_config,
            PendingConfigChanges {
                projects: true,
                selection: true,
                ..PendingConfigChanges::default()
            },
        );

        assert_eq!(recovered.projects.len(), 2);
        assert_eq!(recovered.ui.last_selected_project_id, Some(2));
        assert_eq!(recovered.projects[0].id, loaded_project.id);
        assert_eq!(recovered.projects[0].path, loaded_project.path);
        assert_eq!(recovered.projects[1].id, 2);
        assert_eq!(recovered.projects[1].name, current_project.name);
        assert_eq!(recovered.projects[1].path, current_project.path);
    }

    #[test]
    fn recovered_config_merges_duplicate_project_paths_and_saved_messages() {
        let loaded_project = test_project(5, "Loaded", "C:/shared/demo", &["existing"]);
        let current_project = test_project(1, "Added", "C:/shared/demo", &["new"]);
        let loaded_config = AppConfig {
            projects: vec![loaded_project.clone()],
            ..AppConfig::default()
        };
        let current_projects = BTreeMap::from([(current_project.id, current_project)]);

        let recovered = recover_config_state(
            &boot_failed_current_config(),
            &current_projects,
            Some(1),
            loaded_config,
            PendingConfigChanges {
                projects: true,
                selection: true,
                ..PendingConfigChanges::default()
            },
        );

        assert_eq!(recovered.projects.len(), 1);
        assert_eq!(recovered.ui.last_selected_project_id, Some(5));
        assert_eq!(
            recovered.projects[0].saved_messages,
            vec!["existing".to_owned(), "new".to_owned()]
        );
    }

    #[test]
    fn recovered_config_only_overrides_loaded_shell_when_shell_changed_in_session() {
        let loaded_config = AppConfig {
            default_shell: ShellKind::Cmd,
            ui: crate::models::UiConfig {
                auto_tile_scope: AutoTileScope::SelectedProjectOnly,
                ..boot_failed_current_config().ui
            },
            ..AppConfig::default()
        };

        let recovered = recover_config_state(
            &boot_failed_current_config(),
            &BTreeMap::new(),
            None,
            loaded_config,
            PendingConfigChanges {
                default_shell: true,
                ..PendingConfigChanges::default()
            },
        );

        assert_eq!(recovered.default_shell, ShellKind::PowerShell);
        assert_eq!(
            recovered.ui.auto_tile_scope,
            AutoTileScope::SelectedProjectOnly
        );
    }

    fn boot_failed_current_config() -> AppConfig {
        AppConfig {
            ui: crate::models::UiConfig {
                show_project_explorer: true,
                show_terminal_manager: true,
                main_visibility_mode: MainVisibilityMode::Global,
                project_filter_mode: false,
                ..crate::models::UiConfig::default()
            },
            ..AppConfig::default()
        }
    }

    fn test_terminal_style() -> TerminalStyle {
        TerminalStyle {
            fg: TerminalColor {
                r: 220,
                g: 220,
                b: 220,
            },
            bg: TerminalColor {
                r: 20,
                g: 24,
                b: 30,
            },
            italic: false,
            underline: false,
            strike: false,
        }
    }

    fn test_selection_line(
        segments: &[(&str, usize, usize)],
        width: usize,
    ) -> TerminalSelectionLine {
        test_selection_line_with_wrap(segments, width, false)
    }

    fn test_selection_line_with_wrap(
        segments: &[(&str, usize, usize)],
        width: usize,
        wraps_to_next: bool,
    ) -> TerminalSelectionLine {
        let style = test_terminal_style();
        TerminalSelectionLine {
            width,
            wraps_to_next,
            cells: segments
                .iter()
                .flat_map(|(text, column, display_width)| {
                    let char_count = text.chars().count();
                    if *display_width == char_count {
                        text.chars()
                            .enumerate()
                            .map(move |(offset, ch)| TerminalStyledCell {
                                text: ch.to_string(),
                                style,
                                column: *column + offset,
                                display_width: 1,
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![TerminalStyledCell {
                            text: (*text).to_owned(),
                            style,
                            column: *column,
                            display_width: *display_width,
                        }]
                    }
                })
                .collect(),
        }
    }

    fn test_project(id: u64, name: &str, path: &str, saved_messages: &[&str]) -> ProjectRecord {
        ProjectRecord {
            id,
            name: name.to_owned(),
            path: PathBuf::from(path),
            saved_messages: saved_messages
                .iter()
                .map(|message| (*message).to_owned())
                .collect(),
        }
    }

    fn test_terminal_entry(id: u64, project_id: u64) -> TerminalEntry {
        TerminalEntry {
            id,
            project_id,
            kind: TerminalKind::Foreground,
            shell: ShellKind::PowerShell,
            title: format!("Terminal {id}"),
            full_title: format!("Terminal {id}"),
            pending_line_for_title: String::new(),
            in_main_view: true,
            dirty: false,
            last_seqno: 0,
            last_cursor_row: None,
            last_cursor_row_changed_at: None,
            stable_input_cursor_row: None,
            render_cache: TerminalSnapshot::default(),
            selection: None,
            selection_snapshot: None,
            selection_drag_active: false,
            snapshot_refresh_deferred: false,
            exited: false,
            runtime: test_terminal_runtime(),
        }
    }

    fn test_app(
        terminals: impl IntoIterator<Item = (u64, TerminalEntry)>,
        active_terminal: Option<u64>,
    ) -> AdeApp {
        let (terminal_events_tx, terminal_events_rx) = crossbeam_channel::unbounded();
        let (source_control_events_tx, source_control_events_rx) = crossbeam_channel::unbounded();
        let (directory_index_events_tx, directory_index_events_rx) = crossbeam_channel::unbounded();

        AdeApp {
            config_path: PathBuf::new(),
            config: AppConfig::default(),
            config_load_error: None,
            config_save_requires_reload: false,
            pending_config_changes: PendingConfigChanges::default(),
            projects: BTreeMap::new(),
            terminals: terminals.into_iter().collect(),
            next_project_id: 1,
            next_terminal_id: 3,
            selected_project: None,
            active_terminal,
            pending_ctrl_c: None,
            buffered_terminal_input: Vec::new(),
            buffered_terminal_navigation: Vec::new(),
            terminal_events_tx,
            terminal_events_rx,
            show_settings_popup: false,
            saved_message_drafts: BTreeMap::new(),
            directory_search_query: String::new(),
            status_line: "Ready".to_owned(),
            layout_epoch: 0,
            theme_initialized: false,
            #[cfg(target_os = "windows")]
            window_hwnd: None,
            #[cfg(target_os = "windows")]
            window_layout_passes_remaining: 0,
            source_control_events_tx,
            source_control_events_rx,
            source_control_state: BTreeMap::new(),
            directory_index_events_tx,
            directory_index_events_rx,
            directory_index_state: BTreeMap::new(),
            directory_index_generation: BTreeMap::new(),
        }
    }

    #[test]
    fn pending_line_keeps_last_logical_line() {
        let mut pending = String::new();

        AdeApp::append_pending_line(&mut pending, "echo first");
        AdeApp::append_pending_line(&mut pending, "\nnext");

        assert_eq!(pending, "next");
    }

    #[test]
    fn pasted_text_preserves_windows_newlines() {
        let pasted = AdeApp::pasted_text("first\r\nsecond\rthird");

        assert_eq!(pasted, "first\r\nsecond\rthird");
    }

    #[test]
    fn pasted_text_preserves_unix_newlines() {
        let pasted = AdeApp::pasted_text("first\nsecond");

        assert_eq!(pasted, "first\nsecond");
    }

    #[test]
    fn parse_branch_header_extracts_ahead_behind_counts() {
        let (branch, ahead, behind) = parse_branch_header("main...origin/main [ahead 2, behind 1]");
        assert_eq!(branch, "main");
        assert_eq!(ahead, 2);
        assert_eq!(behind, 1);
    }

    #[test]
    fn normalizes_near_black_terminal_background() {
        let normalized = normalize_terminal_background(TerminalColor { r: 0, g: 0, b: 0 });
        assert_eq!(normalized, TERMINAL_OUTPUT_BG);
    }

    #[test]
    fn keeps_non_black_terminal_background() {
        let normalized = normalize_terminal_background(TerminalColor {
            r: 32,
            g: 80,
            b: 120,
        });
        assert_eq!(normalized.r(), 32);
        assert_eq!(normalized.g(), 80);
        assert_eq!(normalized.b(), 120);
    }

    #[test]
    fn block_cursor_swaps_foreground_and_background_colors() {
        let style = sample_style();
        let snapshot = TerminalSnapshot {
            lines: vec![TerminalStyledLine {
                runs: vec![TerminalStyledRun {
                    text: "A".to_owned(),
                    style,
                    column: 0,
                    display_width: 1,
                }],
            }],
            cursor: Some(TerminalCursor {
                x: 0,
                y: 0,
                shape: TerminalCursorShape::Block,
                blinking: false,
            }),
            cursor_line: Some(TerminalCursorLine {
                row: 0,
                cells: vec![TerminalStyledCell {
                    text: "A".to_owned(),
                    style,
                    column: 0,
                    display_width: 1,
                }],
            }),
        };

        let render = build_terminal_render(
            &snapshot,
            &FontId::new(14.0, FontFamily::Monospace),
            false,
            ShellKind::PowerShell,
            Some(0),
            0.0,
        );
        let section = &render.layout_job.sections[0];

        assert!(render.cursor_overlay.is_none());
        assert_eq!(section.format.color, to_egui_color(style.bg));
        assert_eq!(section.format.background, to_egui_color(style.fg));
    }

    #[test]
    fn underline_cursor_overlay_rect_uses_single_cursor_column() {
        let rect = terminal_cursor_overlay_rect(
            pos2(10.0, 20.0),
            8.0,
            16.0,
            TerminalCursorOverlay {
                shape: TerminalCursorShape::Underline,
                row: 1,
                column: 3,
                width_columns: 1,
                color: Color32::WHITE,
            },
        );

        assert_eq!(rect.min, pos2(34.0, 50.0));
        assert_eq!(rect.width(), 8.0);
        assert_eq!(rect.height(), 2.0);
    }

    #[test]
    fn bar_cursor_overlay_rect_uses_terminal_origin() {
        let rect = terminal_cursor_overlay_rect(
            pos2(4.0, 6.0),
            8.0,
            16.0,
            TerminalCursorOverlay {
                shape: TerminalCursorShape::Bar,
                row: 2,
                column: 1,
                width_columns: 1,
                color: Color32::WHITE,
            },
        );

        assert_eq!(rect.min, pos2(12.0, 38.0));
        assert_eq!(rect.width(), 2.0);
        assert_eq!(rect.height(), 16.0);
    }

    #[test]
    fn blinking_cursor_toggles_visibility_by_half_second_steps() {
        assert!(terminal_cursor_blink_phase_visible(0.0));
        assert!(!terminal_cursor_blink_phase_visible(0.61));
        assert!(terminal_cursor_blink_phase_visible(1.21));
    }

    #[test]
    fn steady_cursor_stays_visible_across_blink_phases() {
        let cursor = TerminalCursor {
            x: 0,
            y: 0,
            shape: TerminalCursorShape::Block,
            blinking: false,
        };

        assert_eq!(
            visible_terminal_cursor(Some(cursor), false, ShellKind::PowerShell, Some(0), 0.0),
            Some(cursor)
        );
        assert_eq!(
            visible_terminal_cursor(Some(cursor), false, ShellKind::PowerShell, Some(0), 0.61),
            Some(cursor)
        );
        assert_eq!(
            visible_terminal_cursor(Some(cursor), false, ShellKind::PowerShell, Some(0), 1.21),
            Some(cursor)
        );
    }

    #[test]
    fn powershell_cursor_is_hidden_when_row_differs_from_stable_row() {
        assert!(cursor_hidden_by_row_filter(
            ShellKind::PowerShell,
            Some(4),
            3,
        ));
    }

    #[test]
    fn powershell_cursor_reappears_when_row_matches_stable_row() {
        let cursor = TerminalCursor {
            x: 0,
            y: 0,
            shape: TerminalCursorShape::Block,
            blinking: false,
        };

        assert_eq!(
            visible_terminal_cursor(Some(cursor), false, ShellKind::PowerShell, Some(0), 0.0),
            Some(cursor)
        );
    }

    #[test]
    fn cmd_cursor_is_not_hidden_by_row_filter() {
        let cursor = TerminalCursor {
            x: 0,
            y: 0,
            shape: TerminalCursorShape::Block,
            blinking: false,
        };

        assert_eq!(
            visible_terminal_cursor(Some(cursor), false, ShellKind::Cmd, Some(1), 0.0),
            Some(cursor)
        );
    }

    #[test]
    fn stable_cursor_row_updates_only_after_row_is_stable() {
        let mut last_cursor_row = None;
        let mut last_cursor_row_changed_at = None;
        let mut stable_input_cursor_row = None;

        update_stable_cursor_row(
            &mut last_cursor_row,
            &mut last_cursor_row_changed_at,
            &mut stable_input_cursor_row,
            Some(5),
            0.0,
        );
        assert_eq!(stable_input_cursor_row, None);

        update_stable_cursor_row(
            &mut last_cursor_row,
            &mut last_cursor_row_changed_at,
            &mut stable_input_cursor_row,
            Some(5),
            0.03,
        );
        assert_eq!(stable_input_cursor_row, None);

        update_stable_cursor_row(
            &mut last_cursor_row,
            &mut last_cursor_row_changed_at,
            &mut stable_input_cursor_row,
            Some(5),
            0.07,
        );
        assert_eq!(stable_input_cursor_row, Some(5));
    }

    #[test]
    fn stable_cursor_row_keeps_previous_input_row_during_transient_jump() {
        let mut last_cursor_row = Some(5);
        let mut last_cursor_row_changed_at = Some(0.0);
        let mut stable_input_cursor_row = Some(5);

        update_stable_cursor_row(
            &mut last_cursor_row,
            &mut last_cursor_row_changed_at,
            &mut stable_input_cursor_row,
            Some(4),
            0.08,
        );
        assert_eq!(stable_input_cursor_row, Some(5));

        update_stable_cursor_row(
            &mut last_cursor_row,
            &mut last_cursor_row_changed_at,
            &mut stable_input_cursor_row,
            Some(5),
            0.09,
        );
        assert_eq!(stable_input_cursor_row, Some(5));
    }

    #[test]
    fn non_block_cursor_overlay_anchors_to_cursor_column_on_wide_cell() {
        let style = sample_style();
        let snapshot = TerminalSnapshot {
            lines: vec![TerminalStyledLine {
                runs: vec![TerminalStyledRun {
                    text: "\u{4f60} ".to_owned(),
                    style,
                    column: 0,
                    display_width: 2,
                }],
            }],
            cursor: Some(TerminalCursor {
                x: 1,
                y: 0,
                shape: TerminalCursorShape::Underline,
                blinking: false,
            }),
            cursor_line: Some(TerminalCursorLine {
                row: 0,
                cells: vec![TerminalStyledCell {
                    text: "\u{4f60}".to_owned(),
                    style,
                    column: 0,
                    display_width: 2,
                }],
            }),
        };

        let overlay =
            build_terminal_cursor_overlay(&snapshot, snapshot.cursor.expect("expected cursor"))
                .expect("expected overlay");

        assert_eq!(overlay.column, 1);
        assert_eq!(overlay.width_columns, 1);
        assert_eq!(overlay.color, to_egui_color(style.fg));
    }

    fn sample_style() -> TerminalStyle {
        TerminalStyle {
            fg: TerminalColor {
                r: 26,
                g: 179,
                b: 255,
            },
            bg: TerminalColor {
                r: 12,
                g: 18,
                b: 28,
            },
            italic: false,
            underline: false,
            strike: false,
        }
    }
}
