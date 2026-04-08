use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write;
use std::ops::Range;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arboard::Clipboard;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Color32, Event, FontData, FontFamily, FontId, Galley, Id, Key, Layout, RichText,
    Sense, Stroke, TextWrapMode, Ui, Vec2, WidgetInfo, WidgetText, WidgetType,
};
use iconflow::{fonts as icon_fonts, try_icon, Pack, Size, Style};
use serde::{Deserialize, Serialize};
use tattoy_wezterm_surface::hyperlink::{
    Rule, CLOSING_PARENTHESIS_HYPERLINK_PATTERN, GENERIC_HYPERLINK_PATTERN,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETKEYBOARDDELAY, SPI_GETKEYBOARDSPEED,
};

use crate::codex::{self, CodexEnableOutcome, CodexNotifyInboxEvent};
use crate::config;
use crate::hooks::{AiCliSession, AiCliStatus, AiCliTool, AiHookManager};
use crate::layout;
use crate::models::{
    AppConfig, LeftSidebarTab, MainVisibilityMode, ProjectRecord, ShellKind, TerminalKind,
    TerminalManagerFilter,
};
use crate::terminal::{
    try_terminal_selection_snapshot, try_terminal_snapshots, TerminalColor, TerminalCursor,
    TerminalCursorShape, TerminalDimensions, TerminalRuntime, TerminalSelectionLine,
    TerminalSelectionSnapshot, TerminalSnapshot, TerminalUiEvent, TerminalUiEventKind,
    TerminalWheelEvent, TrackedProcessIdentity, WheelDirection,
};
use crate::title::{terminal_title_candidate, update_terminal_title};

const TITLE_MAX_LEN: usize = 40;
const TERMINAL_EVENT_BUDGET: usize = 4096;
const TERMINAL_RETRY_MS: u64 = 8;
const TERMINAL_FALLBACK_REFRESH_MS: u64 = 16;
const FACTORY_DROID_HOOK_POLL_MS: u64 = 75;
const FACTORY_DROID_PROCESS_POLL_MS: u64 = 75;
const FACTORY_DROID_LAUNCH_GRACE_MS: u64 = 5_000;
const FACTORY_DROID_TRAILING_OUTPUT_GRACE_MS: u64 = 750;
const CODEX_NOTIFY_POLL_MS: u64 = 75;
const CODEX_PROCESS_POLL_MS: u64 = 75;
const CODEX_LAUNCH_GRACE_MS: u64 = 5_000;
const CODEX_TRAILING_OUTPUT_GRACE_MS: u64 = 750;
const CURSOR_BLINK_STEP_SECS: f64 = 0.6;
const TERMINAL_COPY_TOAST_SECS: f64 = 1.75;
const TERMINAL_COPY_FEEDBACK_TEXT: &str = "Copied terminal selection";
const POWERSHELL_CURSOR_ROW_STABLE_SECS: f64 = 0.06;
const TERMINAL_CHAR_WIDTH_SAMPLE_CELLS: usize = 64;
const TERMINAL_FONT_FAMILY_NAME: &str = "terminal-mono";
const CURSOR_BAR_WIDTH_PX: f32 = 2.0;
const CURSOR_UNDERLINE_HEIGHT_PX: f32 = 2.0;
const TERMINAL_HELD_KEY_REPEAT_FALLBACK_INITIAL_DELAY_SECS: f64 = 0.5;
const TERMINAL_HELD_KEY_REPEAT_FALLBACK_INTERVAL_SECS: f64 = 1.0 / 30.0;
const TERMINAL_HELD_KEY_REPEAT_MAX_SYNTHETIC_EVENTS_PER_FRAME: usize = 16;

// Embedded Nerd Font for terminal icon support
const NERD_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/CaskaydiaCoveNerdFont-Regular.ttf");
const NERD_FONT_NAME: &str = "caskaydia-cove-nerd";
const DIRECTORY_INDEX_LOADING_ANIMATION_STEP_SECS: f64 = 0.25;
const SOURCE_CONTROL_PRIORITY_REFRESH_SECS: f64 = 5.0;
const SOURCE_CONTROL_BACKGROUND_REFRESH_SECS: f64 = 20.0;
const SOURCE_CONTROL_POLL_TICK_MS: u64 = 250;
const SOURCE_CONTROL_TOOLTIP_FILE_LIMIT: usize = 12;
const DIRECTORY_ENTRY_TOOLTIP_MAX_CHARS: usize = 500;
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
const PROJECT_EXPLORER_WIDTH: f32 = 352.0;
const ACTIVITY_RAIL_WIDTH: f32 = 48.0;
const CONTROL_ROW_HEIGHT: f32 = 28.0;
const SIDEBAR_ROW_LEADING_INSET: f32 = 6.0;
const TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH: f32 = 32.0;
const SOURCE_CONTROL_FILE_ICON_WIDTH: f32 = 16.0;
const SOURCE_CONTROL_FILE_ICON_GAP: f32 = 6.0;
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
#[cfg(target_os = "windows")]
const WINDOWS_TERMINAL_FONT_CANDIDATES: [(&str, &str); 2] = [
    ("terminal-cascadia-mono", "CascadiaMono.ttf"),
    ("terminal-consolas", "consola.ttf"),
];

static FACTORY_DROID_INBOX_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);
static CODEX_NOTIFY_INBOX_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FactoryDroidHookInboxState {
    offset: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CodexNotifyInboxState {
    offset: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct FactoryDroidHookInboxEvent {
    terminal_id: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    inbox_token: Option<String>,
    hook_event_name: String,
    status: String,
    #[serde(default)]
    notification_kind: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    timestamp_utc: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactoryDroidStatusSource {
    PromptSubmit,
    PtyHookEvent,
    PtyStop,
    PtyNotification,
    TerminalTitle,
    Inbox,
}

impl FactoryDroidStatusSource {
    const fn label(self) -> &'static str {
        match self {
            Self::PromptSubmit => "prompt_submit",
            Self::PtyHookEvent => "pty_hook_event",
            Self::PtyStop => "pty_stop",
            Self::PtyNotification => "pty_notification",
            Self::TerminalTitle => "terminal_title",
            Self::Inbox => "inbox",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexCliStatusSource {
    PromptSubmit,
    Notify,
    Bell,
}

impl CodexCliStatusSource {
    const fn label(self) -> &'static str {
        match self {
            Self::PromptSubmit => "prompt_submit",
            Self::Notify => "notify",
            Self::Bell => "bell",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FactoryDroidTransportDiagnostics {
    hooks_enabled: bool,
    executable_path: PathBuf,
    hooks_runtime_dir: Option<PathBuf>,
    hooks_runtime_error: Option<String>,
    active_session: Option<bool>,
    process_state: Option<String>,
    last_status_source: Option<FactoryDroidStatusSource>,
}

impl FactoryDroidTransportDiagnostics {
    const PRIMARY_TRANSPORT_LABEL: &'static str = "PTY/process (primary)";
    const FALLBACK_TRANSPORT_LABEL: &'static str = "Inbox JSONL (fallback)";

    fn runtime_status_text(&self) -> String {
        if !self.hooks_enabled {
            "Disabled".to_owned()
        } else if let Some(dir) = &self.hooks_runtime_dir {
            format!("Ready: {}", dir.display())
        } else if let Some(err) = &self.hooks_runtime_error {
            format!("Unavailable: {err}")
        } else {
            "Unavailable: unknown error".to_owned()
        }
    }

    fn active_session_text(&self) -> &'static str {
        match self.active_session {
            Some(true) => "Yes",
            Some(false) => "No",
            None => "No active terminal",
        }
    }

    fn process_state_text(&self) -> &str {
        self.process_state
            .as_deref()
            .unwrap_or("No active terminal")
    }

    fn last_status_source_text(&self) -> &'static str {
        self.last_status_source
            .map(FactoryDroidStatusSource::label)
            .unwrap_or("none")
    }

    fn warning_message(&self) -> Option<String> {
        self.hooks_enabled.then_some(()).and(
            self.hooks_runtime_error
                .as_ref()
                .map(|err| format!("Factory Droid inbox fallback unavailable: {err}")),
        )
    }
}

pub struct AdeApp {
    config_path: PathBuf,
    current_executable_path: PathBuf,
    factory_droid_hooks_dir: Option<PathBuf>,
    factory_droid_hooks_dir_error: Option<String>,
    factory_droid_hook_inboxes: BTreeMap<u64, FactoryDroidHookInboxState>,
    factory_droid_hook_last_poll_at: Option<Instant>,
    factory_droid_process_last_poll_at: Option<Instant>,
    codex_cli_runtime_dir: Option<PathBuf>,
    codex_cli_runtime_dir_error: Option<String>,
    codex_notify_inboxes: BTreeMap<u64, CodexNotifyInboxState>,
    codex_notify_last_poll_at: Option<Instant>,
    codex_process_last_poll_at: Option<Instant>,
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
    buffered_terminal_input: Vec<Event>,
    buffered_terminal_navigation: Vec<TerminalNavigationShortcut>,
    terminal_held_key_repeat: Option<TerminalHeldKeyRepeat>,
    allow_attention_terminal_input_routing_once: bool,
    pending_terminal_pastes: Vec<PendingTerminalPaste>,
    terminal_events_tx: Sender<TerminalUiEvent>,
    terminal_events_rx: Receiver<TerminalUiEvent>,
    ai_hook_manager: Option<Arc<AiHookManager>>,
    show_settings_popup: bool,
    settings_diagnostics_expanded: bool,
    saved_message_drafts: BTreeMap<u64, String>,
    directory_search_query: String,
    directory_pending_tree_open_state_by_project: BTreeMap<u64, bool>,
    status_line: String,
    copy_toast: Option<TransientToast>,
    layout_epoch: u64,
    theme_initialized: bool,
    #[cfg(target_os = "windows")]
    window_hwnd: Option<isize>,
    #[cfg(target_os = "windows")]
    window_layout_passes_remaining: u8,
    source_control_commands_tx: Sender<SourceControlCommand>,
    source_control_events_rx: Receiver<SourceControlEvent>,
    source_control_state: BTreeMap<u64, SourceControlSnapshot>,
    source_control_refresh_state: BTreeMap<u64, SourceControlRefreshState>,
    source_control_worker_busy: bool,
    source_control_last_auto_refresh_project: Option<u64>,
    directory_index_events_tx: Sender<DirectoryIndexEvent>,
    directory_index_events_rx: Receiver<DirectoryIndexEvent>,
    directory_index_state: BTreeMap<u64, DirectoryIndexSnapshot>,
    directory_tree_has_collapsed_cache_by_project: BTreeMap<u64, bool>,
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
    recent_inputs: VecDeque<String>,
    in_main_view: bool,
    dirty: bool,
    last_seqno: usize,
    last_cursor_row: Option<usize>,
    last_cursor_row_changed_at: Option<f64>,
    stable_input_cursor_row: Option<usize>,
    render_cache: TerminalSnapshot,
    selection: Option<TerminalSelection>,
    selection_snapshot: Option<TerminalSelectionSnapshot>,
    pending_link_click: Option<PendingTerminalLinkClick>,
    selection_drag_active: bool,
    snapshot_refresh_deferred: bool,
    exited: bool,
    runtime: TerminalRuntime,
    ai_session: AiCliSession,
    factory_droid_inbox_token: Option<String>,
    codex_notify_inbox_token: Option<String>,
    factory_droid_launch_pending_since: Option<Instant>,
    factory_droid_session_active: bool,
    factory_droid_last_process_seen_at: Option<Instant>,
    factory_droid_process_missing_since: Option<Instant>,
    factory_droid_last_status_source: Option<FactoryDroidStatusSource>,
    codex_launch_pending_since: Option<Instant>,
    codex_launch_process_baseline: Option<Vec<TrackedProcessIdentity>>,
    codex_session_active: bool,
    codex_process_identity: Option<TrackedProcessIdentity>,
    codex_last_process_seen_at: Option<Instant>,
    codex_process_missing_since: Option<Instant>,
    codex_last_status_source: Option<CodexCliStatusSource>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TerminalHeldKeyRepeat {
    terminal_id: u64,
    key: Key,
    modifiers: egui::Modifiers,
    first_pressed_at: f64,
    last_repeat_at: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TerminalHeldKeyRepeatTiming {
    initial_delay_secs: f64,
    interval_secs: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct TransientToast {
    message: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingTerminalLinkClick {
    anchor: TerminalSelectionPoint,
    url: String,
}

#[derive(Debug)]
struct PendingTerminalPaste {
    terminal_id: u64,
    text: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalLinkSegment {
    row: usize,
    start_column: usize,
    end_column: usize,
    byte_range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct TerminalLogicalLine {
    text: String,
    segments: Vec<TerminalLinkSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CtrlCAction {
    CopySelection,
    SendInterrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalSecondaryClickAction {
    OpenCopyMenu,
    PasteImmediately,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalNavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalNavigationShortcut {
    Grid(TerminalNavigationDirection),
    SingleViewLinear(TerminalNavigationDirection),
    SingleViewFilter(TerminalNavigationDirection),
}

#[derive(Debug, Clone, Default)]
struct SourceControlSnapshot {
    branch: String,
    ahead: usize,
    behind: usize,
    files: Vec<SourceControlFile>,
    added_lines: Option<usize>,
    removed_lines: Option<usize>,
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

#[derive(Debug)]
struct SourceControlCommand {
    project_id: u64,
    project_path: PathBuf,
    run_fetch: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct SourceControlRefreshState {
    queued: bool,
    queued_manual: bool,
    queued_fetch: bool,
    in_flight: bool,
    last_completed_at: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceControlDispatchPriority {
    ManualFetch,
    ManualStatus,
    PriorityAuto,
    BackgroundAuto,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceControlBadgeState {
    Clean,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalManagerDiffSummaryState {
    Unknown,
    Loading,
    Ready,
    Error,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TerminalManagerDiffSummaryModel {
    state: TerminalManagerDiffSummaryState,
    added_lines: usize,
    removed_lines: usize,
    tooltip_lines: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalManagerDiffSummaryVisual {
    Totals {
        added_text: String,
        removed_text: String,
        added_color: Color32,
        removed_color: Color32,
        separator_color: Color32,
    },
    Placeholder {
        text: &'static str,
        color: Color32,
    },
}

#[allow(dead_code)]
struct AiBadgeModel {
    tool: Option<AiCliTool>,
    status: AiCliStatus,
    tooltip_lines: Vec<String>,
}

impl AiBadgeModel {
    fn from_session(session: &AiCliSession) -> Self {
        let tool = session.tool;
        let status = session.status;
        let tooltip_lines = if let Some(t) = tool {
            vec![status.tooltip(t)]
        } else {
            vec!["AI: Not detected".to_string()]
        };
        Self {
            tool,
            status,
            tooltip_lines,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiBadgeVisual {
    Spinner(Color32),
    Pulse(Color32),
}

fn ai_badge_visual(status: AiCliStatus) -> Option<AiBadgeVisual> {
    match status {
        AiCliStatus::Inactive => None,
        AiCliStatus::Running => Some(AiBadgeVisual::Spinner(Color32::from_rgb(76, 209, 114))),
        AiCliStatus::Attention => Some(AiBadgeVisual::Pulse(Color32::from_rgb(46, 130, 255))),
    }
}

fn draw_ai_badge(ui: &mut Ui, badge: &AiBadgeModel) -> egui::Response {
    let Some(visual) = ai_badge_visual(badge.status) else {
        return ui.allocate_at_least(egui::vec2(0.0, 0.0), Sense::hover()).1;
    };

    let (rect, response) = ui.allocate_at_least(egui::vec2(16.0, 16.0), Sense::hover());

    if ui.is_rect_visible(rect) {
        match visual {
            AiBadgeVisual::Spinner(color) => {
                egui::Spinner::new().color(color).paint_at(ui, rect);
            }
            AiBadgeVisual::Pulse(color) => {
                let time_seconds = ui.ctx().input(|i| i.time);
                let center = rect.center();

                // Pulse animasyonu (büyüyüp küçülen daire)
                let pulse = ((time_seconds * 4.0).sin() + 1.0) * 0.25 + 2.5; // 2.5 - 5 arası yarıçap
                let radius = pulse as f32;
                let alpha = ((time_seconds * 4.0).sin() * 0.3 + 0.7) * 255.0;
                let color =
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha as u8);

                ui.painter().circle(
                    center,
                    radius,
                    color,
                    egui::Stroke::new(0.0, Color32::TRANSPARENT),
                );

                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
        }
    }

    response.on_hover_ui(|ui| {
        for line in &badge.tooltip_lines {
            ui.label(line);
        }
    })
}

struct TerminalStatusBadgeLayout {
    #[cfg(test)]
    ai_rect: Option<egui::Rect>,
}

struct TerminalManagerTitleSummaryLayout {
    #[cfg(test)]
    title_rect: egui::Rect,
    #[cfg(test)]
    diff_summary_rect: egui::Rect,
}

fn draw_terminal_status_badges(ui: &mut Ui, ai_badge: &AiBadgeModel) -> TerminalStatusBadgeLayout {
    let ai_response = ai_badge_visual(ai_badge.status).map(|_| draw_ai_badge(ui, ai_badge));
    #[cfg(test)]
    let ai_rect = ai_response.as_ref().map(|response| response.rect);
    if ai_response.is_some() {
        ui.add_space(4.0);
    }

    TerminalStatusBadgeLayout {
        #[cfg(test)]
        ai_rect,
    }
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
    fn ai_hook_manager_from_config(config: &AppConfig) -> Option<Arc<AiHookManager>> {
        config
            .ai_hooks
            .global_enabled
            .then(|| Arc::new(AiHookManager::new(config.ai_hooks.clone())))
    }

    fn factory_droid_hook_runtime_state(
        config: &AppConfig,
    ) -> (Option<Arc<AiHookManager>>, Option<PathBuf>, Option<String>) {
        let ai_hook_manager = Self::ai_hook_manager_from_config(config);
        if ai_hook_manager.is_none() {
            return (None, None, None);
        }

        match config::factory_droid_hook_runtime_dir() {
            Ok(dir) => (ai_hook_manager, Some(dir), None),
            Err(err) => {
                log::warn!("Factory Droid inbox runtime directory unavailable: {err}");
                (ai_hook_manager, None, Some(err.to_string()))
            }
        }
    }

    fn codex_cli_runtime_state(
        config: &AppConfig,
    ) -> (Option<Arc<AiHookManager>>, Option<PathBuf>, Option<String>) {
        let ai_hook_manager = Self::ai_hook_manager_from_config(config);
        if ai_hook_manager.is_none() {
            return (None, None, None);
        }

        match config::codex_cli_runtime_dir() {
            Ok(dir) => (ai_hook_manager, Some(dir), None),
            Err(err) => {
                log::warn!("Codex CLI runtime directory unavailable: {err}");
                (ai_hook_manager, None, Some(err.to_string()))
            }
        }
    }

    fn next_factory_droid_inbox_token(terminal_id: u64) -> String {
        let counter = FACTORY_DROID_INBOX_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        format!(
            "{terminal_id:016x}-{:08x}-{timestamp_nanos:032x}-{counter:016x}",
            std::process::id()
        )
    }

    fn next_codex_notify_inbox_token(terminal_id: u64) -> String {
        let counter = CODEX_NOTIFY_INBOX_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        format!(
            "{terminal_id:016x}-{:08x}-{timestamp_nanos:032x}-{counter:016x}",
            std::process::id()
        )
    }

    fn enable_codex_cli_integration(&mut self, ctx: &egui::Context) {
        match codex::enable_codex_cli_integration(&self.current_executable_path) {
            Ok(CodexEnableOutcome::MissingInstall) => {
                self.status_line =
                    "Codex CLI was not found. Install it with npm, then run `codex login`."
                        .to_owned();
                ctx.open_url(egui::OpenUrl::new_tab(codex::codex_setup_url()));
            }
            Ok(CodexEnableOutcome::NeedsLogin) => {
                self.status_line =
                    "Codex CLI is installed but not signed in. Run `codex login`, then try again."
                        .to_owned();
            }
            Ok(CodexEnableOutcome::CustomNotifyHookPreserved { path }) => {
                self.status_line = format!(
                    "Codex CLI kept the existing custom notify hook and only refreshed TUI notification settings; turn-complete tracking may stay limited until notify is routed through Mergen: {}",
                    path.display()
                );
            }
            Ok(CodexEnableOutcome::ConfigUpdated { path, updated }) => {
                self.status_line = if updated {
                    format!(
                        "Codex CLI integration enabled with Mergen turn-complete notify routing and BEL-backed TUI notifications: {}",
                        path.display()
                    )
                } else {
                    format!(
                        "Codex CLI integration already configured with Mergen turn-complete notify routing and BEL-backed TUI notifications: {}",
                        path.display()
                    )
                };
            }
            Err(err) => {
                self.status_line = format!("Failed to enable Codex CLI integration: {err}");
            }
        }
        ctx.request_repaint();
    }

    #[cfg(not(test))]
    fn prepare_codex_cli_integration_for_launch(&mut self) {
        let Ok(path) = codex::user_codex_config_path() else {
            return;
        };

        match codex::patch_codex_config_file(&path, &self.current_executable_path) {
            Ok(codex::CodexConfigPatchOutcome::Updated) => {
                self.status_line = format!(
                    "Codex CLI launch prepared Mergen turn-complete notifications in {}",
                    path.display()
                );
            }
            Ok(codex::CodexConfigPatchOutcome::Unchanged) => {}
            Ok(codex::CodexConfigPatchOutcome::CustomNotifyHookPreserved) => {
                self.status_line = format!(
                    "Codex CLI still uses a custom notify hook in {}; Mergen can track launch/running state, but turn-complete attention may stay limited until notify is routed through Mergen.",
                    path.display()
                );
            }
            Err(err) => {
                self.status_line =
                    format!("Failed to prepare Codex CLI notifications for launch: {err}");
            }
        }
    }

    #[cfg(test)]
    fn prepare_codex_cli_integration_for_launch(&mut self) {}

    pub fn bootstrap(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = config::config_path().unwrap_or_else(|_| PathBuf::from("config.toml"));
        let (mut config, config_load_error) = match config::load_config(&config_path) {
            Ok(config) => (config, None),
            Err(err) => (AppConfig::default(), Some(err.to_string())),
        };
        config.ui.show_project_explorer = true;
        config.ui.show_terminal_manager = true;
        config.ui.main_visibility_mode = MainVisibilityMode::Global;
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
        let current_executable_path =
            std::env::current_exe().unwrap_or_else(|_| PathBuf::from("unknown"));
        let (ai_hook_manager, factory_droid_hooks_dir, factory_droid_hooks_dir_error) =
            Self::factory_droid_hook_runtime_state(&config);
        let (_, codex_cli_runtime_dir, codex_cli_runtime_dir_error) =
            Self::codex_cli_runtime_state(&config);

        let (terminal_events_tx, terminal_events_rx) = crossbeam_channel::unbounded();
        let (source_control_commands_tx, source_control_commands_rx) =
            crossbeam_channel::unbounded();
        let (source_control_events_tx, source_control_events_rx) = crossbeam_channel::unbounded();
        let (directory_index_events_tx, directory_index_events_rx) = crossbeam_channel::unbounded();
        spawn_source_control_worker(source_control_commands_rx, source_control_events_tx.clone());

        let app = Self {
            config_path,
            current_executable_path,
            factory_droid_hooks_dir,
            factory_droid_hooks_dir_error: factory_droid_hooks_dir_error.clone(),
            factory_droid_hook_inboxes: BTreeMap::new(),
            factory_droid_hook_last_poll_at: None,
            factory_droid_process_last_poll_at: None,
            codex_cli_runtime_dir,
            codex_cli_runtime_dir_error: codex_cli_runtime_dir_error.clone(),
            codex_notify_inboxes: BTreeMap::new(),
            codex_notify_last_poll_at: None,
            codex_process_last_poll_at: None,
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
            buffered_terminal_input: Vec::new(),
            buffered_terminal_navigation: Vec::new(),
            terminal_held_key_repeat: None,
            allow_attention_terminal_input_routing_once: false,
            pending_terminal_pastes: Vec::new(),
            terminal_events_tx,
            terminal_events_rx,
            ai_hook_manager,
            show_settings_popup: false,
            settings_diagnostics_expanded: false,
            saved_message_drafts: BTreeMap::new(),
            directory_search_query: String::new(),
            directory_pending_tree_open_state_by_project: BTreeMap::new(),
            status_line: config_load_error
                .map(|err| format!("Config load error: {err}. Existing config preserved."))
                .or_else(|| {
                    factory_droid_hooks_dir_error
                        .as_ref()
                        .map(|err| format!("Factory Droid inbox fallback unavailable: {err}"))
                })
                .or_else(|| {
                    codex_cli_runtime_dir_error
                        .as_ref()
                        .map(|err| format!("Codex CLI runtime unavailable: {err}"))
                })
                .unwrap_or_else(|| "Ready".to_owned()),
            copy_toast: None,
            layout_epoch: 0,
            theme_initialized: false,
            #[cfg(target_os = "windows")]
            window_hwnd,
            #[cfg(target_os = "windows")]
            window_layout_passes_remaining: 8,
            source_control_commands_tx,
            source_control_events_rx,
            source_control_state: BTreeMap::new(),
            source_control_refresh_state: BTreeMap::new(),
            source_control_worker_busy: false,
            source_control_last_auto_refresh_project: None,
            directory_index_events_tx,
            directory_index_events_rx,
            directory_index_state: BTreeMap::new(),
            directory_tree_has_collapsed_cache_by_project: BTreeMap::new(),
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

    fn single_terminal_id_for_main(&self) -> Option<u64> {
        self.active_terminal
            .filter(|terminal_id| self.terminals.contains_key(terminal_id))
            .or_else(|| self.terminals.keys().next().copied())
    }

    fn first_visible_terminal_for_main(&self) -> Option<u64> {
        if self.config.ui.multi_terminal_view_enabled {
            self.terminals
                .iter()
                .find_map(|(id, terminal)| self.terminal_visible_in_main(terminal).then_some(*id))
        } else {
            self.single_terminal_id_for_main()
        }
    }

    fn terminal_visible_in_main(&self, terminal: &TerminalEntry) -> bool {
        if self.config.ui.multi_terminal_view_enabled {
            terminal.in_main_view
        } else {
            self.single_terminal_id_for_main() == Some(terminal.id)
        }
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
            ai_config: crate::hooks::ProjectAiConfig::default(),
        };

        self.selected_project = Some(project.id);
        self.projects.insert(project.id, project);
        self.next_project_id += 1;
        self.bump_layout_epoch();
        self.note_projects_changed();
        self.note_selection_changed();
        self.persist_config();
    }

    fn remove_project(&mut self, ctx: &egui::Context, project_id: u64) {
        let Some(project) = self.projects.remove(&project_id) else {
            return;
        };

        let terminal_ids = self
            .terminals
            .iter()
            .filter_map(|(terminal_id, terminal)| {
                (terminal.project_id == project_id).then_some(*terminal_id)
            })
            .collect::<Vec<_>>();
        let mut close_failures = 0usize;
        for terminal_id in terminal_ids {
            if self.terminals.contains_key(&terminal_id) {
                if self
                    .terminals
                    .get(&terminal_id)
                    .is_some_and(|terminal| terminal.runtime.terminate().is_err())
                {
                    close_failures += 1;
                }
                self.clear_factory_droid_state(terminal_id);
                self.reset_factory_droid_hook_inbox(terminal_id);
                self.clear_codex_state(terminal_id);
                self.reset_codex_notify_inbox(terminal_id);
                self.terminals.remove(&terminal_id);
            }
        }

        let next_active_terminal = self
            .active_terminal
            .filter(|terminal_id| self.terminals.contains_key(terminal_id))
            .or_else(|| self.first_visible_terminal_for_main());
        self.set_active_terminal(ctx, next_active_terminal);

        if self.selected_project == Some(project_id) {
            self.selected_project = self.projects.keys().copied().next();
            self.note_selection_changed();
        }

        self.source_control_state.remove(&project_id);
        self.source_control_refresh_state.remove(&project_id);
        if self.source_control_last_auto_refresh_project == Some(project_id) {
            self.source_control_last_auto_refresh_project = None;
        }
        self.directory_index_state.remove(&project_id);
        self.directory_index_generation.remove(&project_id);
        self.directory_tree_has_collapsed_cache_by_project
            .remove(&project_id);
        self.saved_message_drafts.remove(&project_id);
        self.directory_pending_tree_open_state_by_project
            .remove(&project_id);

        self.bump_layout_epoch();
        if close_failures == 0 {
            self.status_line = format!("Removed project '{}'", project.name);
        } else {
            self.status_line = format!(
                "Removed project '{}' ({close_failures} terminal(s) failed to close cleanly)",
                project.name
            );
        }
        self.note_projects_changed();
        self.persist_config();
        ctx.request_repaint();
    }

    fn spawn_terminal_for_project(
        &mut self,
        ctx: &egui::Context,
        project_id: u64,
        kind: TerminalKind,
    ) -> bool {
        let Some(project) = self.projects.get(&project_id).cloned() else {
            return false;
        };

        let shell = self.config.default_shell;

        let terminal_id = self.next_terminal_id;
        self.next_terminal_id += 1;
        self.reset_factory_droid_hook_inbox(terminal_id);
        self.reset_codex_notify_inbox(terminal_id);
        let factory_droid_inbox_token = self
            .ai_hook_manager
            .as_ref()
            .map(|_| Self::next_factory_droid_inbox_token(terminal_id));
        let codex_notify_inbox_token = self
            .ai_hook_manager
            .as_ref()
            .map(|_| Self::next_codex_notify_inbox_token(terminal_id));

        let dimensions = TerminalDimensions::default();
        let runtime = match TerminalRuntime::spawn(
            terminal_id,
            shell,
            project.path.clone(),
            self.terminal_events_tx.clone(),
            ctx.clone(),
            dimensions,
            self.ai_hook_manager.clone(),
            self.factory_droid_hooks_dir.clone(),
            factory_droid_inbox_token.clone(),
            self.codex_cli_runtime_dir.clone(),
            codex_notify_inbox_token.clone(),
        ) {
            Ok(runtime) => runtime,
            Err(err) => {
                self.status_line = format!("Failed to create terminal: {err}");
                return false;
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
            recent_inputs: VecDeque::new(),
            in_main_view: true,
            dirty: true,
            last_seqno: runtime.latest_seqno(),
            last_cursor_row: None,
            last_cursor_row_changed_at: None,
            stable_input_cursor_row: None,
            render_cache: TerminalSnapshot::default(),
            selection: None,
            selection_snapshot: None,
            pending_link_click: None,
            selection_drag_active: false,
            snapshot_refresh_deferred: false,
            exited: false,
            runtime,
            ai_session: AiCliSession::default(),
            factory_droid_inbox_token,
            codex_notify_inbox_token,
            factory_droid_launch_pending_since: None,
            factory_droid_session_active: false,
            factory_droid_last_process_seen_at: None,
            factory_droid_process_missing_since: None,
            factory_droid_last_status_source: None,
            codex_launch_pending_since: None,
            codex_launch_process_baseline: None,
            codex_session_active: false,
            codex_process_identity: None,
            codex_last_process_seen_at: None,
            codex_process_missing_since: None,
            codex_last_status_source: None,
        };

        self.terminals.insert(terminal_id, entry);
        self.set_active_terminal(ctx, Some(terminal_id));
        self.bump_layout_epoch();

        self.status_line = "Terminal created".to_owned();
        true
    }

    fn factory_droid_hook_inbox_path_for_dir(dir: &Path, terminal_id: u64) -> PathBuf {
        dir.join(format!("{terminal_id}.jsonl"))
    }

    fn codex_notify_inbox_path_for_dir(dir: &Path, terminal_id: u64, inbox_token: &str) -> PathBuf {
        codex::codex_notify_inbox_path_for_dir(dir, terminal_id, inbox_token)
    }

    fn factory_droid_hook_inbox_path(&self, terminal_id: u64) -> Option<PathBuf> {
        self.factory_droid_hooks_dir
            .as_deref()
            .map(|dir| Self::factory_droid_hook_inbox_path_for_dir(dir, terminal_id))
    }

    fn codex_notify_inbox_path(&self, terminal_id: u64) -> Option<PathBuf> {
        let inbox_token = self
            .terminals
            .get(&terminal_id)?
            .codex_notify_inbox_token
            .as_deref()?;
        self.codex_cli_runtime_dir
            .as_deref()
            .map(|dir| Self::codex_notify_inbox_path_for_dir(dir, terminal_id, inbox_token))
    }

    fn launch_command_stem(line: &str) -> Option<&str> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let command = match trimmed.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let after_quote = &trimmed[quote.len_utf8()..];
                match after_quote
                    .char_indices()
                    .find_map(|(offset, ch)| (ch == quote).then_some(offset))
                {
                    Some(offset) => &after_quote[..offset],
                    None => after_quote,
                }
            }
            Some(_) => trimmed.split_whitespace().next().unwrap_or_default(),
            None => return None,
        };
        if command.is_empty() {
            return None;
        }

        Path::new(command)
            .file_stem()
            .and_then(|stem| stem.to_str())
    }

    fn is_factory_droid_launch_command(line: &str) -> bool {
        Self::launch_command_stem(line).is_some_and(|stem| {
            stem.eq_ignore_ascii_case("droid") || stem.eq_ignore_ascii_case("factory")
        })
    }

    fn factory_droid_attention_source_from_chunk(chunk: &str) -> Option<FactoryDroidStatusSource> {
        let lower = chunk.to_ascii_lowercase();
        // Match "HOOKS  Stop" (visible script output) and also the raw
        // [droid-hook:event=Stop] / [factory-droid-hook:event=Stop] markers
        // that may appear in the PTY stream before the visible text.
        if lower.contains("hooks  stop")
            || lower.contains("hooks stop")
            || lower.contains("[droid-hook:event=stop]")
            || lower.contains("[factory-droid-hook:event=stop]")
        {
            return Some(FactoryDroidStatusSource::PtyStop);
        }

        if lower.contains("needs your permission") || lower.contains("waiting for your input") {
            return Some(FactoryDroidStatusSource::PtyNotification);
        }

        None
    }

    fn factory_droid_process_state_text(terminal: &TerminalEntry) -> &'static str {
        if terminal.factory_droid_session_active {
            "active"
        } else if terminal.factory_droid_process_missing_since.is_some() {
            "missing (grace)"
        } else {
            "none"
        }
    }

    fn mark_factory_droid_launch_pending(&mut self, terminal_id: u64) -> bool {
        if self.ai_hook_manager.is_none() {
            return false;
        }

        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        if let Some(manager) = &self.ai_hook_manager {
            manager.set_tool(terminal_id, AiCliTool::FactoryDroid);
        }

        let mut changed = false;
        if entry.ai_session.tool != Some(AiCliTool::FactoryDroid) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            changed = true;
        }
        if entry.ai_session.status != AiCliStatus::Inactive {
            entry.ai_session.status = AiCliStatus::Inactive;
            changed = true;
        }
        if entry.factory_droid_launch_pending_since.is_none() {
            changed = true;
        }
        entry.factory_droid_launch_pending_since = Some(Instant::now());
        entry.factory_droid_session_active = false;
        entry.factory_droid_last_process_seen_at = None;
        entry.factory_droid_process_missing_since = None;
        entry.dirty = true;
        changed
    }

    fn clear_factory_droid_state(&mut self, terminal_id: u64) -> bool {
        if let Some(manager) = &self.ai_hook_manager {
            manager.reset_session(terminal_id);
        }

        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let changed = entry.ai_session.tool == Some(AiCliTool::FactoryDroid)
            || entry.ai_session.status != AiCliStatus::Inactive
            || entry.factory_droid_launch_pending_since.is_some()
            || entry.factory_droid_session_active;

        entry.ai_session = AiCliSession::default();
        entry.factory_droid_launch_pending_since = None;
        entry.factory_droid_session_active = false;
        entry.factory_droid_last_process_seen_at = None;
        entry.factory_droid_process_missing_since = None;
        entry.factory_droid_last_status_source = None;
        entry.dirty = true;
        changed
    }

    fn note_factory_droid_session_active(&mut self, terminal_id: u64) -> bool {
        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        if let Some(manager) = &self.ai_hook_manager {
            manager.set_tool(terminal_id, AiCliTool::FactoryDroid);
        }

        let mut changed = false;
        if entry.ai_session.tool != Some(AiCliTool::FactoryDroid) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            changed = true;
        }
        if !entry.factory_droid_session_active {
            entry.factory_droid_session_active = true;
            changed = true;
        }
        if entry.factory_droid_process_missing_since.take().is_some() {
            changed = true;
        }
        if entry.factory_droid_launch_pending_since.take().is_some() {
            changed = true;
        }
        entry.factory_droid_last_process_seen_at = Some(Instant::now());
        entry.dirty = true;
        changed
    }

    fn note_factory_droid_process_missing(&mut self, terminal_id: u64) -> bool {
        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let mut changed = false;
        if entry.factory_droid_session_active {
            entry.factory_droid_session_active = false;
            changed = true;
        }
        if entry.factory_droid_process_missing_since.is_none() {
            entry.factory_droid_process_missing_since = Some(Instant::now());
            changed = true;
        }
        entry.dirty = true;
        changed
    }

    fn factory_droid_trailing_grace_elapsed(entry: &TerminalEntry) -> bool {
        entry
            .factory_droid_process_missing_since
            .is_some_and(|missing_since| {
                missing_since.elapsed()
                    >= Duration::from_millis(FACTORY_DROID_TRAILING_OUTPUT_GRACE_MS)
            })
    }

    fn apply_factory_droid_status(
        &mut self,
        terminal_id: u64,
        status: AiCliStatus,
        source: FactoryDroidStatusSource,
    ) -> bool {
        let Some(manager) = self.ai_hook_manager.as_ref().cloned() else {
            return false;
        };

        manager.set_tool(terminal_id, AiCliTool::FactoryDroid);

        let update = match status {
            AiCliStatus::Running => manager.ai_activity_started(terminal_id),
            AiCliStatus::Attention => manager.ai_waiting_for_user(terminal_id),
            AiCliStatus::Inactive => None,
        };

        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let mut changed = false;
        if entry.ai_session.tool != Some(AiCliTool::FactoryDroid) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            changed = true;
        }
        if let Some((tool, next_status)) = update {
            if entry.ai_session.tool != Some(tool) || entry.ai_session.status != next_status {
                entry.ai_session.tool = Some(tool);
                entry.ai_session.status = next_status;
                changed = true;
            }
        } else if entry.ai_session.status != status {
            entry.ai_session.status = status;
            changed = true;
        }
        if entry.factory_droid_last_status_source != Some(source) {
            entry.factory_droid_last_status_source = Some(source);
            changed = true;
        }
        if !entry.factory_droid_session_active {
            entry.factory_droid_session_active = true;
            changed = true;
        }
        if entry.factory_droid_process_missing_since.take().is_some() {
            changed = true;
        }
        entry.factory_droid_last_process_seen_at = Some(Instant::now());
        if entry.factory_droid_launch_pending_since.take().is_some() {
            changed = true;
        }
        entry.dirty = true;
        changed
    }

    fn reset_factory_droid_hook_inbox(&mut self, terminal_id: u64) {
        self.factory_droid_hook_inboxes.remove(&terminal_id);

        if let Some(path) = self.factory_droid_hook_inbox_path(terminal_id) {
            let _ = fs::remove_file(path);
        }
    }

    fn poll_factory_droid_processes(&mut self, ctx: &egui::Context) {
        if self.ai_hook_manager.is_none() || self.terminals.is_empty() {
            return;
        }

        if self
            .factory_droid_process_last_poll_at
            .is_some_and(|last_poll| {
                last_poll.elapsed() < Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS)
            })
        {
            return;
        }

        self.factory_droid_process_last_poll_at = Some(Instant::now());

        let mut changed = false;
        let terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        for terminal_id in terminal_ids {
            let Some(entry) = self.terminals.get(&terminal_id) else {
                continue;
            };
            let process_state = if entry.exited {
                Some(false)
            } else {
                entry.runtime.has_factory_droid_descendant_process()
            };
            let launch_expired =
                entry
                    .factory_droid_launch_pending_since
                    .is_some_and(|started_at| {
                        started_at.elapsed() >= Duration::from_millis(FACTORY_DROID_LAUNCH_GRACE_MS)
                    });
            let session_active = entry.factory_droid_session_active;
            let is_candidate = entry.factory_droid_launch_pending_since.is_some();
            let tool_is_factory = entry.ai_session.tool == Some(AiCliTool::FactoryDroid);
            let status = entry.ai_session.status;

            if is_candidate && launch_expired && process_state != Some(true) {
                changed |= self.clear_factory_droid_state(terminal_id);
                continue;
            }

            match process_state {
                Some(true) => {
                    changed |= self.note_factory_droid_session_active(terminal_id);
                    continue;
                }
                Some(false) => {
                    if tool_is_factory {
                        changed |= self.note_factory_droid_process_missing(terminal_id);

                        match status {
                            AiCliStatus::Attention => {}
                            AiCliStatus::Running => {
                                if self
                                    .terminals
                                    .get(&terminal_id)
                                    .is_some_and(Self::factory_droid_trailing_grace_elapsed)
                                {
                                    changed |= self.clear_factory_droid_state(terminal_id);
                                }
                            }
                            AiCliStatus::Inactive => {
                                changed |= self.clear_factory_droid_state(terminal_id);
                            }
                        }
                    } else if session_active {
                        changed |= self.clear_factory_droid_state(terminal_id);
                    }
                }
                None => {
                    continue;
                }
            }
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn poll_factory_droid_hook_inboxes(&mut self, ctx: &egui::Context) {
        if self.ai_hook_manager.is_none()
            || self.factory_droid_hooks_dir.is_none()
            || self.terminals.is_empty()
        {
            return;
        }

        if self
            .factory_droid_hook_last_poll_at
            .is_some_and(|last_poll| {
                last_poll.elapsed() < Duration::from_millis(FACTORY_DROID_HOOK_POLL_MS)
            })
        {
            return;
        }

        self.factory_droid_hook_last_poll_at = Some(Instant::now());

        let terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        let mut changed = false;
        for terminal_id in terminal_ids {
            changed |= self.process_factory_droid_hook_inbox(terminal_id);
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn process_factory_droid_hook_inbox(&mut self, terminal_id: u64) -> bool {
        let Some(path) = self.factory_droid_hook_inbox_path(terminal_id) else {
            return false;
        };

        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.factory_droid_hook_inboxes
                    .entry(terminal_id)
                    .or_default()
                    .offset = 0;
                return false;
            }
            Err(err) => {
                log::warn!(
                    "Failed to read Factory Droid hook inbox for terminal {terminal_id} at {}: {err}",
                    path.display()
                );
                return false;
            }
        };

        let previous_offset = self
            .factory_droid_hook_inboxes
            .get(&terminal_id)
            .map(|state| state.offset)
            .unwrap_or(0);
        let start = if previous_offset as usize <= bytes.len() {
            previous_offset as usize
        } else {
            0
        };
        let unread = &bytes[start..];
        let Some(last_newline) = unread.iter().rposition(|byte| *byte == b'\n') else {
            if previous_offset as usize > bytes.len() {
                self.factory_droid_hook_inboxes
                    .entry(terminal_id)
                    .or_default()
                    .offset = 0;
            }
            return false;
        };

        let processed_end = start + last_newline + 1;
        let mut changed = false;
        for line in bytes[start..processed_end].split(|byte| *byte == b'\n') {
            if line.is_empty() {
                continue;
            }

            match serde_json::from_slice::<FactoryDroidHookInboxEvent>(line) {
                Ok(event) => {
                    changed |= self.apply_factory_droid_hook_inbox_event(terminal_id, &event);
                }
                Err(err) => {
                    log::warn!(
                        "Ignoring malformed Factory Droid hook inbox event for terminal {terminal_id} at {}: {err}",
                        path.display()
                    );
                }
            }
        }

        self.factory_droid_hook_inboxes
            .entry(terminal_id)
            .or_default()
            .offset = processed_end as u64;

        changed
    }

    fn apply_factory_droid_hook_inbox_event(
        &mut self,
        terminal_id: u64,
        event: &FactoryDroidHookInboxEvent,
    ) -> bool {
        if event.terminal_id != terminal_id.to_string() {
            return false;
        }

        let Some(expected_inbox_token) = self
            .terminals
            .get(&terminal_id)
            .and_then(|terminal| terminal.factory_droid_inbox_token.as_deref())
        else {
            return false;
        };

        if event.inbox_token.as_deref() != Some(expected_inbox_token) {
            return false;
        }

        match event.status.as_str() {
            "running" => self.apply_factory_droid_status(
                terminal_id,
                AiCliStatus::Running,
                FactoryDroidStatusSource::Inbox,
            ),
            "attention" => self.apply_factory_droid_status(
                terminal_id,
                AiCliStatus::Attention,
                FactoryDroidStatusSource::Inbox,
            ),
            _ => false,
        }
    }

    fn is_codex_launch_command(line: &str) -> bool {
        Self::launch_command_stem(line).is_some_and(|stem| stem.eq_ignore_ascii_case("codex"))
    }

    fn codex_attention_source_from_chunk(chunk: &str) -> Option<CodexCliStatusSource> {
        chunk
            .contains("[bell]")
            .then_some(CodexCliStatusSource::Bell)
    }

    fn mark_codex_launch_pending(
        &mut self,
        terminal_id: u64,
        baseline: Option<Vec<TrackedProcessIdentity>>,
    ) -> bool {
        if self.ai_hook_manager.is_none() {
            return false;
        }

        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        if let Some(manager) = &self.ai_hook_manager {
            manager.set_tool(terminal_id, AiCliTool::CodexCli);
        }

        let mut changed = false;
        if entry.ai_session.tool != Some(AiCliTool::CodexCli) {
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            changed = true;
        }
        if entry.ai_session.status != AiCliStatus::Inactive {
            entry.ai_session.status = AiCliStatus::Inactive;
            changed = true;
        }
        if entry.codex_launch_pending_since.is_none() {
            changed = true;
        }
        entry.codex_launch_pending_since = Some(Instant::now());
        if entry.codex_launch_process_baseline != baseline {
            changed = true;
        }
        entry.codex_launch_process_baseline = baseline;
        entry.codex_session_active = false;
        entry.codex_process_identity = None;
        entry.codex_last_process_seen_at = None;
        entry.codex_process_missing_since = None;
        entry.dirty = true;
        changed
    }

    fn clear_codex_state(&mut self, terminal_id: u64) -> bool {
        if let Some(manager) = &self.ai_hook_manager {
            manager.reset_session(terminal_id);
        }

        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let changed = entry.ai_session.tool == Some(AiCliTool::CodexCli)
            || entry.codex_launch_pending_since.is_some()
            || entry.codex_launch_process_baseline.is_some()
            || entry.codex_session_active
            || entry.codex_process_identity.is_some()
            || entry.codex_process_missing_since.is_some()
            || entry.codex_last_status_source.is_some();

        if entry.ai_session.tool == Some(AiCliTool::CodexCli) {
            entry.ai_session = AiCliSession::default();
        }
        entry.codex_launch_pending_since = None;
        entry.codex_launch_process_baseline = None;
        entry.codex_session_active = false;
        entry.codex_process_identity = None;
        entry.codex_last_process_seen_at = None;
        entry.codex_process_missing_since = None;
        entry.codex_last_status_source = None;
        entry.dirty = true;
        changed
    }

    fn clear_codex_process_tracking(&mut self, terminal_id: u64) -> bool {
        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let changed = entry.codex_session_active
            || entry.codex_launch_process_baseline.is_some()
            || entry.codex_process_identity.is_some()
            || entry.codex_last_process_seen_at.is_some()
            || entry.codex_process_missing_since.is_some();

        entry.codex_launch_process_baseline = None;
        entry.codex_session_active = false;
        entry.codex_process_identity = None;
        entry.codex_last_process_seen_at = None;
        entry.codex_process_missing_since = None;
        entry.dirty = true;
        changed
    }

    fn note_codex_session_active(
        &mut self,
        terminal_id: u64,
        identity: TrackedProcessIdentity,
    ) -> bool {
        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        if let Some(manager) = &self.ai_hook_manager {
            manager.set_tool(terminal_id, AiCliTool::CodexCli);
        }

        let mut changed = false;
        if entry.ai_session.tool != Some(AiCliTool::CodexCli) {
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            changed = true;
        }
        if !entry.codex_session_active {
            entry.codex_session_active = true;
            changed = true;
        }
        if entry.codex_process_identity != Some(identity) {
            entry.codex_process_identity = Some(identity);
            changed = true;
        }
        if entry.codex_launch_process_baseline.take().is_some() {
            changed = true;
        }
        if entry.codex_process_missing_since.take().is_some() {
            changed = true;
        }
        if entry.codex_launch_pending_since.take().is_some() {
            changed = true;
        }
        entry.codex_last_process_seen_at = Some(Instant::now());
        entry.dirty = true;
        changed
    }

    fn note_codex_process_missing(&mut self, terminal_id: u64) -> bool {
        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let mut changed = false;
        if entry.codex_session_active {
            entry.codex_session_active = false;
            changed = true;
        }
        if entry.codex_process_missing_since.is_none() {
            entry.codex_process_missing_since = Some(Instant::now());
            changed = true;
        }
        entry.dirty = true;
        changed
    }

    fn codex_trailing_grace_elapsed(entry: &TerminalEntry) -> bool {
        entry
            .codex_process_missing_since
            .is_some_and(|missing_since| {
                missing_since.elapsed() >= Duration::from_millis(CODEX_TRAILING_OUTPUT_GRACE_MS)
            })
    }

    fn apply_codex_status(
        &mut self,
        terminal_id: u64,
        status: AiCliStatus,
        source: CodexCliStatusSource,
    ) -> bool {
        let Some(manager) = self.ai_hook_manager.as_ref().cloned() else {
            return false;
        };

        manager.set_tool(terminal_id, AiCliTool::CodexCli);

        let update = match status {
            AiCliStatus::Running => manager.ai_activity_started(terminal_id),
            AiCliStatus::Attention => manager.ai_waiting_for_user(terminal_id),
            AiCliStatus::Inactive => None,
        };

        let Some(entry) = self.terminals.get_mut(&terminal_id) else {
            return false;
        };

        let should_dedupe_attention = status == AiCliStatus::Attention
            && entry.ai_session.tool == Some(AiCliTool::CodexCli)
            && entry.ai_session.status == AiCliStatus::Attention
            && matches!(
                entry.codex_last_status_source,
                Some(CodexCliStatusSource::Notify | CodexCliStatusSource::Bell)
            )
            && matches!(
                source,
                CodexCliStatusSource::Notify | CodexCliStatusSource::Bell
            );
        if should_dedupe_attention {
            return false;
        }

        let mut changed = false;
        if entry.ai_session.tool != Some(AiCliTool::CodexCli) {
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            changed = true;
        }
        if let Some((tool, next_status)) = update {
            if entry.ai_session.tool != Some(tool) || entry.ai_session.status != next_status {
                entry.ai_session.tool = Some(tool);
                entry.ai_session.status = next_status;
                changed = true;
            }
        } else if entry.ai_session.status != status {
            entry.ai_session.status = status;
            changed = true;
        }
        if entry.codex_last_status_source != Some(source) {
            entry.codex_last_status_source = Some(source);
            changed = true;
        }
        if entry.codex_launch_process_baseline.take().is_some() {
            changed = true;
        }
        if !entry.codex_session_active {
            entry.codex_session_active = true;
            changed = true;
        }
        if entry.codex_process_missing_since.take().is_some() {
            changed = true;
        }
        entry.codex_last_process_seen_at = Some(Instant::now());
        if entry.codex_launch_pending_since.take().is_some() {
            changed = true;
        }
        entry.dirty = true;
        changed
    }

    fn reset_codex_notify_inbox(&mut self, terminal_id: u64) {
        self.codex_notify_inboxes.remove(&terminal_id);

        if let Some(path) = self.codex_notify_inbox_path(terminal_id) {
            let _ = fs::remove_file(path);
        }
    }

    fn poll_codex_processes(&mut self, ctx: &egui::Context) {
        if self.ai_hook_manager.is_none() || self.terminals.is_empty() {
            return;
        }

        if self.codex_process_last_poll_at.is_some_and(|last_poll| {
            last_poll.elapsed() < Duration::from_millis(CODEX_PROCESS_POLL_MS)
        }) {
            return;
        }

        self.codex_process_last_poll_at = Some(Instant::now());

        let mut changed = false;
        let terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        for terminal_id in terminal_ids {
            let Some((
                process_identity,
                recovered_baseline,
                launch_expired,
                session_active,
                is_candidate,
                tool_is_codex,
                status,
                exited,
            )) = self.terminals.get(&terminal_id).map(|entry| {
                let mut recovered_baseline = None;
                let process_identity = if entry.exited {
                    Some(None)
                } else if let Some(identity) = entry.codex_process_identity {
                    entry
                        .runtime
                        .tracked_codex_process_present(identity)
                        .map(|present| present.then_some(identity))
                } else if entry.codex_launch_pending_since.is_some() {
                    if let Some(baseline) = entry.codex_launch_process_baseline.as_deref() {
                        entry.runtime.detect_new_codex_descendant_process(baseline)
                    } else {
                        recovered_baseline = entry.runtime.snapshot_codex_descendant_processes();
                        match recovered_baseline.as_ref() {
                            Some(baseline) if baseline.is_empty() => Some(None),
                            _ => None,
                        }
                    }
                } else {
                    None
                };
                let launch_expired = entry.codex_launch_pending_since.is_some_and(|started_at| {
                    started_at.elapsed() >= Duration::from_millis(CODEX_LAUNCH_GRACE_MS)
                });
                (
                    process_identity,
                    recovered_baseline,
                    launch_expired,
                    entry.codex_session_active,
                    entry.codex_launch_pending_since.is_some(),
                    entry.ai_session.tool == Some(AiCliTool::CodexCli),
                    entry.ai_session.status,
                    entry.exited,
                )
            })
            else {
                continue;
            };

            if let Some(baseline) = recovered_baseline {
                if let Some(entry) = self.terminals.get_mut(&terminal_id) {
                    if entry.codex_launch_process_baseline.is_none() {
                        entry.codex_launch_process_baseline = Some(baseline);
                        entry.dirty = true;
                        changed = true;
                    }
                }
            }

            if is_candidate && !launch_expired && process_identity == Some(None) && !exited {
                continue;
            }

            if is_candidate && launch_expired && process_identity == Some(None) {
                changed |= self.clear_codex_state(terminal_id);
                continue;
            }

            match process_identity {
                Some(Some(identity)) => {
                    changed |= self.note_codex_session_active(terminal_id, identity);
                    continue;
                }
                Some(None) => {
                    if tool_is_codex {
                        changed |= self.note_codex_process_missing(terminal_id);

                        match status {
                            AiCliStatus::Attention => {
                                if self
                                    .terminals
                                    .get(&terminal_id)
                                    .is_some_and(Self::codex_trailing_grace_elapsed)
                                {
                                    changed |= self.clear_codex_process_tracking(terminal_id);
                                }
                            }
                            AiCliStatus::Running => {
                                if self
                                    .terminals
                                    .get(&terminal_id)
                                    .is_some_and(Self::codex_trailing_grace_elapsed)
                                {
                                    changed |= self.clear_codex_state(terminal_id);
                                }
                            }
                            AiCliStatus::Inactive => {
                                changed |= self.clear_codex_state(terminal_id);
                            }
                        }
                    } else if session_active {
                        changed |= self.clear_codex_state(terminal_id);
                    }
                }
                None => {
                    continue;
                }
            }
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn poll_codex_notify_inboxes(&mut self, ctx: &egui::Context) {
        if self.ai_hook_manager.is_none()
            || self.codex_cli_runtime_dir.is_none()
            || self.terminals.is_empty()
        {
            return;
        }

        if self.codex_notify_last_poll_at.is_some_and(|last_poll| {
            last_poll.elapsed() < Duration::from_millis(CODEX_NOTIFY_POLL_MS)
        }) {
            return;
        }

        self.codex_notify_last_poll_at = Some(Instant::now());

        let terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        let mut changed = false;
        for terminal_id in terminal_ids {
            changed |= self.process_codex_notify_inbox(terminal_id);
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn process_codex_notify_inbox(&mut self, terminal_id: u64) -> bool {
        let Some(path) = self.codex_notify_inbox_path(terminal_id) else {
            return false;
        };

        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.codex_notify_inboxes
                    .entry(terminal_id)
                    .or_default()
                    .offset = 0;
                return false;
            }
            Err(err) => {
                log::warn!(
                    "Failed to read Codex notify inbox for terminal {terminal_id} at {}: {err}",
                    path.display()
                );
                return false;
            }
        };

        let previous_offset = self
            .codex_notify_inboxes
            .get(&terminal_id)
            .map(|state| state.offset)
            .unwrap_or(0);
        let start = if previous_offset as usize <= bytes.len() {
            previous_offset as usize
        } else {
            0
        };
        let unread = &bytes[start..];
        let Some(last_newline) = unread.iter().rposition(|byte| *byte == b'\n') else {
            if previous_offset as usize > bytes.len() {
                self.codex_notify_inboxes
                    .entry(terminal_id)
                    .or_default()
                    .offset = 0;
            }
            return false;
        };

        let processed_end = start + last_newline + 1;
        let mut changed = false;
        for line in bytes[start..processed_end].split(|byte| *byte == b'\n') {
            if line.is_empty() {
                continue;
            }

            match serde_json::from_slice::<CodexNotifyInboxEvent>(line) {
                Ok(event) => {
                    changed |= self.apply_codex_notify_inbox_event(terminal_id, &event);
                }
                Err(err) => {
                    log::warn!(
                        "Ignoring malformed Codex notify inbox event for terminal {terminal_id} at {}: {err}",
                        path.display()
                    );
                }
            }
        }

        self.codex_notify_inboxes
            .entry(terminal_id)
            .or_default()
            .offset = processed_end as u64;

        changed
    }

    fn apply_codex_notify_inbox_event(
        &mut self,
        terminal_id: u64,
        event: &CodexNotifyInboxEvent,
    ) -> bool {
        let expected_inbox_token = self
            .terminals
            .get(&terminal_id)
            .and_then(|entry| entry.codex_notify_inbox_token.as_deref());

        if event.terminal_id != terminal_id.to_string() {
            return false;
        }
        if !event.tool.eq_ignore_ascii_case("codex") {
            return false;
        }
        if event.inbox_token.as_deref() != expected_inbox_token {
            return false;
        }

        match event.status.as_str() {
            "attention" => self.apply_codex_status(
                terminal_id,
                AiCliStatus::Attention,
                CodexCliStatusSource::Notify,
            ),
            _ => false,
        }
    }

    fn terminal_count_for_project_kind(&self, project_id: u64, kind: TerminalKind) -> usize {
        terminal_ids_for_project_kind(&self.terminals, project_id, kind).len()
    }

    #[cfg(test)]
    fn should_auto_open_project_terminal_group(spawn_succeeded: bool) -> bool {
        spawn_succeeded
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
                TerminalUiEventKind::AiStatusChange {
                    terminal_id,
                    tool,
                    status,
                    event: _,
                    from_title,
                } => {
                    if tool == Some(AiCliTool::FactoryDroid) {
                        let source = if from_title {
                            FactoryDroidStatusSource::TerminalTitle
                        } else {
                            FactoryDroidStatusSource::PtyHookEvent
                        };
                        if self.apply_factory_droid_status(terminal_id, status, source) {
                            dirty_ids.insert(terminal_id);
                        }
                    } else if tool == Some(AiCliTool::CodexCli) {
                        if let Some(entry) = self.terminals.get_mut(&terminal_id) {
                            entry.ai_session.tool = Some(AiCliTool::CodexCli);
                            entry.ai_session.status = status;
                            dirty_ids.insert(terminal_id);
                        }
                    } else if let Some(entry) = self.terminals.get_mut(&terminal_id) {
                        if tool.is_some() {
                            entry.ai_session.tool = tool;
                        }
                        entry.ai_session.status = status;
                        dirty_ids.insert(terminal_id);
                    }
                }
                TerminalUiEventKind::AiRawChunk { terminal_id, chunk } => {
                    let should_apply = self.terminals.get(&terminal_id).is_some_and(|entry| {
                        !entry.exited
                            && (entry.factory_droid_session_active
                                || entry.factory_droid_launch_pending_since.is_some()
                                || entry.ai_session.tool == Some(AiCliTool::FactoryDroid))
                    });

                    if should_apply {
                        if let Some(source) =
                            Self::factory_droid_attention_source_from_chunk(&chunk)
                        {
                            if self.apply_factory_droid_status(
                                terminal_id,
                                AiCliStatus::Attention,
                                source,
                            ) {
                                dirty_ids.insert(terminal_id);
                            }
                        }
                    }

                    let should_apply_codex =
                        self.terminals.get(&terminal_id).is_some_and(|entry| {
                            !entry.exited
                                && (entry.codex_session_active
                                    || entry.codex_launch_pending_since.is_some()
                                    || entry.ai_session.tool == Some(AiCliTool::CodexCli))
                        });

                    if should_apply_codex {
                        if let Some(source) = Self::codex_attention_source_from_chunk(&chunk) {
                            if self.apply_codex_status(terminal_id, AiCliStatus::Attention, source)
                            {
                                dirty_ids.insert(terminal_id);
                            }
                        }
                    }
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

        for &terminal_id in &exited_ids {
            let Some(entry) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            entry.exited = true;
            entry.dirty = true;
            changed = true;
        }

        for terminal_id in exited_ids {
            changed |= self.clear_factory_droid_state(terminal_id);
            changed |= self.clear_codex_state(terminal_id);
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
        let completed_at = ctx.input(|input| input.time);
        while let Ok(event) = self.source_control_events_rx.try_recv() {
            if let Some(state) = self.source_control_refresh_state.get_mut(&event.project_id) {
                state.in_flight = false;
                state.last_completed_at = Some(completed_at);
            }
            self.source_control_worker_busy = false;
            let merged_snapshot = merge_source_control_refresh_result(
                self.source_control_state.get(&event.project_id),
                event.snapshot,
            );
            self.source_control_state
                .insert(event.project_id, merged_snapshot);
            changed = true;
        }
        if changed {
            ctx.request_repaint();
        }
    }

    fn request_source_control_refresh(&mut self, project_id: u64, run_fetch: bool, manual: bool) {
        if !self.projects.contains_key(&project_id) {
            return;
        }

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

        let state = self
            .source_control_refresh_state
            .entry(project_id)
            .or_default();
        state.queued = true;
        state.queued_manual |= manual;
        state.queued_fetch |= run_fetch;
    }

    fn source_control_refresh_priority(&self, project_id: u64) -> SourceControlDispatchPriority {
        let Some(state) = self.source_control_refresh_state.get(&project_id) else {
            return if self.is_priority_source_control_project(project_id) {
                SourceControlDispatchPriority::PriorityAuto
            } else {
                SourceControlDispatchPriority::BackgroundAuto
            };
        };
        if state.queued_fetch {
            SourceControlDispatchPriority::ManualFetch
        } else if state.queued_manual {
            SourceControlDispatchPriority::ManualStatus
        } else if self.is_priority_source_control_project(project_id) {
            SourceControlDispatchPriority::PriorityAuto
        } else {
            SourceControlDispatchPriority::BackgroundAuto
        }
    }

    fn is_priority_source_control_project(&self, project_id: u64) -> bool {
        self.selected_project == Some(project_id)
            || self
                .terminals
                .values()
                .any(|terminal| terminal.project_id == project_id && !terminal.exited)
    }

    fn source_control_refresh_interval_secs(&self, project_id: u64) -> f64 {
        if self.is_priority_source_control_project(project_id) {
            SOURCE_CONTROL_PRIORITY_REFRESH_SECS
        } else {
            SOURCE_CONTROL_BACKGROUND_REFRESH_SECS
        }
    }

    fn source_control_next_due_at(&self, project_id: u64) -> f64 {
        self.source_control_refresh_state
            .get(&project_id)
            .and_then(|state| state.last_completed_at)
            .map(|time| time + self.source_control_refresh_interval_secs(project_id))
            .unwrap_or(0.0)
    }

    fn source_control_rotation_key(&self, project_id: u64) -> u64 {
        match self.source_control_last_auto_refresh_project {
            Some(last) if project_id > last => project_id,
            Some(_) => project_id.saturating_add(u64::MAX / 2),
            None => project_id,
        }
    }

    fn next_due_auto_source_control_project(&self, now: f64) -> Option<u64> {
        let mut priority_due = Vec::new();
        let mut background_due = Vec::new();

        for project_id in self.projects.keys().copied() {
            let state = self
                .source_control_refresh_state
                .get(&project_id)
                .copied()
                .unwrap_or_default();
            if state.queued || state.in_flight {
                continue;
            }

            if self.source_control_next_due_at(project_id) > now {
                continue;
            }

            if self.is_priority_source_control_project(project_id) {
                priority_due.push(project_id);
            } else {
                background_due.push(project_id);
            }
        }

        priority_due.sort_by_key(|project_id| self.source_control_rotation_key(*project_id));
        background_due.sort_by_key(|project_id| self.source_control_rotation_key(*project_id));
        priority_due
            .into_iter()
            .next()
            .or_else(|| background_due.into_iter().next())
    }

    fn source_control_has_queued_requests(&self) -> bool {
        self.source_control_refresh_state
            .values()
            .any(|state| state.queued)
    }

    fn prune_source_control_state(&mut self) {
        self.source_control_state
            .retain(|project_id, _| self.projects.contains_key(project_id));
        self.source_control_refresh_state
            .retain(|project_id, _| self.projects.contains_key(project_id));
        if self
            .source_control_last_auto_refresh_project
            .is_some_and(|project_id| !self.projects.contains_key(&project_id))
        {
            self.source_control_last_auto_refresh_project = None;
        }
    }

    fn dispatch_next_source_control_request(&mut self) {
        if self.source_control_worker_busy {
            return;
        }

        let mut candidates = self
            .projects
            .keys()
            .copied()
            .filter(|project_id| {
                self.source_control_refresh_state
                    .get(project_id)
                    .is_some_and(|state| state.queued)
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|project_id| {
            (
                self.source_control_refresh_priority(*project_id),
                self.source_control_rotation_key(*project_id),
            )
        });

        let Some(project_id) = candidates.into_iter().next() else {
            return;
        };
        let Some(project) = self.projects.get(&project_id).cloned() else {
            return;
        };
        let Some(state) = self.source_control_refresh_state.get_mut(&project_id) else {
            return;
        };

        let run_fetch = state.queued_fetch;
        state.queued = false;
        state.queued_manual = false;
        state.queued_fetch = false;
        state.in_flight = true;
        self.source_control_worker_busy = true;

        let _ = self.source_control_commands_tx.send(SourceControlCommand {
            project_id,
            project_path: project.path,
            run_fetch,
        });
    }

    fn schedule_source_control_refresh(&mut self, ctx: &egui::Context) {
        self.prune_source_control_state();
        let now = ctx.input(|input| input.time);

        if !self.source_control_has_queued_requests() {
            if let Some(project_id) = self.next_due_auto_source_control_project(now) {
                self.request_source_control_refresh(project_id, false, false);
                self.source_control_last_auto_refresh_project = Some(project_id);
            }
        }

        self.dispatch_next_source_control_request();

        if self.source_control_worker_busy || self.source_control_has_queued_requests() {
            ctx.request_repaint_after(Duration::from_millis(SOURCE_CONTROL_POLL_TICK_MS));
            return;
        }

        let mut next_due = None::<f64>;
        for project_id in self.projects.keys().copied() {
            let state = self
                .source_control_refresh_state
                .get(&project_id)
                .copied()
                .unwrap_or_default();
            if state.in_flight || state.queued {
                continue;
            }

            let due_at = self.source_control_next_due_at(project_id);
            next_due = Some(match next_due {
                Some(existing) => existing.min(due_at),
                None => due_at,
            });
        }

        let Some(next_due) = next_due else {
            return;
        };
        let delay_secs = (next_due - now).max(0.0);
        ctx.request_repaint_after(Duration::from_secs_f64(delay_secs));
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
            self.directory_tree_has_collapsed_cache_by_project
                .remove(&event.project_id);
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
            })
            .or_insert_with(|| DirectoryIndexSnapshot {
                root: build_directory_root_node(&project.path),
                loading: true,
                last_error: None,
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

    fn cached_directory_tree_has_collapsed_folders(
        &mut self,
        project_id: u64,
        ctx: &egui::Context,
    ) -> bool {
        if let Some(cached) = self
            .directory_tree_has_collapsed_cache_by_project
            .get(&project_id)
            .copied()
        {
            return cached;
        }

        let collapsed = self
            .directory_index_state
            .get(&project_id)
            .map(|snapshot| {
                if snapshot.loading || snapshot.last_error.is_some() {
                    true
                } else {
                    directory_tree_has_collapsed_folders(ctx, &snapshot.root)
                }
            })
            .unwrap_or(true);

        self.directory_tree_has_collapsed_cache_by_project
            .insert(project_id, collapsed);
        collapsed
    }

    fn invalidate_directory_tree_collapsed_cache(&mut self, project_id: u64) {
        self.directory_tree_has_collapsed_cache_by_project
            .remove(&project_id);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context, main_area_size: Vec2) {
        if self.ui_owns_keyboard(ctx) {
            return;
        }

        let mut changed = false;
        let shortcuts = self.take_terminal_navigation_shortcuts(ctx);
        for shortcut in shortcuts {
            match shortcut {
                TerminalNavigationShortcut::Grid(direction) => {
                    let visible_ids = self.visible_terminal_ids_for_main();
                    let grid = layout::compute_tile_grid(
                        visible_ids.len(),
                        main_area_size.x,
                        main_area_size.y,
                    );
                    if let Some(next_terminal) = next_terminal_in_direction(
                        self.active_terminal_accepts_input(),
                        &visible_ids,
                        grid,
                        direction,
                    ) {
                        self.set_active_terminal(ctx, Some(next_terminal));
                        changed = true;
                    }
                }
                TerminalNavigationShortcut::SingleViewLinear(direction)
                    if !self.config.ui.multi_terminal_view_enabled =>
                {
                    let terminal_ids = self.terminal_ids_for_single_view_navigation();
                    if let Some(next_terminal) = next_terminal_in_linear_direction(
                        self.single_view_navigation_anchor(),
                        &terminal_ids,
                        |terminal_id| {
                            self.terminals
                                .get(&terminal_id)
                                .is_some_and(|terminal| !terminal.exited)
                        },
                        direction,
                    ) {
                        self.set_active_terminal(ctx, Some(next_terminal));
                        changed = true;
                    }
                }
                TerminalNavigationShortcut::SingleViewFilter(direction)
                    if !self.config.ui.multi_terminal_view_enabled =>
                {
                    self.cycle_terminal_manager_filter(direction, ctx);
                    changed = true;
                }
                _ => {}
            }
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn cycle_terminal_manager_filter(
        &mut self,
        direction: TerminalNavigationDirection,
        _ctx: &egui::Context,
    ) {
        use crate::models::TerminalManagerFilter;
        let next_filter = match (self.config.ui.terminal_manager_filter, direction) {
            (TerminalManagerFilter::Foreground, TerminalNavigationDirection::Right) => {
                TerminalManagerFilter::Background
            }
            (TerminalManagerFilter::Background, TerminalNavigationDirection::Left) => {
                TerminalManagerFilter::Foreground
            }
            _ => return,
        };
        self.config.ui.terminal_manager_filter = next_filter;
        self.note_ui_config_changed();
        self.persist_config();
    }

    fn active_terminal_accepts_input(&self) -> Option<u64> {
        let active_terminal_id = if self.config.ui.multi_terminal_view_enabled {
            self.active_terminal?
        } else {
            self.single_terminal_id_for_main()?
        };
        self.terminals
            .get(&active_terminal_id)
            .is_some_and(|terminal| self.terminal_visible_in_main(terminal) && !terminal.exited)
            .then_some(active_terminal_id)
    }

    fn single_view_navigation_anchor(&self) -> Option<u64> {
        self.single_terminal_id_for_main()
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

    fn toggle_main_visibility_mode(&mut self) {
        self.config.ui.main_visibility_mode = match self.config.ui.main_visibility_mode {
            MainVisibilityMode::Global => MainVisibilityMode::SelectedProject,
            MainVisibilityMode::SelectedProject => MainVisibilityMode::Global,
        };
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

    fn event_is_terminal_input(event: &Event) -> bool {
        matches!(
            event,
            Event::Key { .. } | Event::Text(_) | Event::Paste(_) | Event::Copy | Event::Cut
        )
    }

    fn event_is_terminal_text_entry(event: &Event) -> bool {
        match event {
            Event::Text(text) => !text.is_empty(),
            Event::Paste(text) => !text.is_empty(),
            Event::Key {
                key: Key::Enter | Key::Backspace,
                pressed: true,
                ..
            } => true,
            _ => false,
        }
    }

    fn should_steal_attention_terminal_input(&self, ctx: &egui::Context, events: &[Event]) -> bool {
        if self.ai_hook_manager.is_none() {
            return false;
        }

        if !self.text_input_has_focus(ctx) {
            return false;
        }

        if ctx.memory(|mem| mem.any_popup_open()) || ctx.is_context_menu_open() {
            return false;
        }

        if self.show_settings_popup && ctx.wants_keyboard_input() {
            return false;
        }

        let Some(active_terminal_id) = self.active_terminal_accepts_input() else {
            return false;
        };

        let Some(terminal) = self.terminals.get(&active_terminal_id) else {
            return false;
        };

        terminal.ai_session.status == AiCliStatus::Attention
            && events.iter().any(Self::event_is_terminal_text_entry)
    }

    fn event_terminal_navigation_shortcut(event: &Event) -> Option<TerminalNavigationShortcut> {
        match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if modifiers.ctrl && !modifiers.alt && !modifiers.shift => match key {
                Key::ArrowLeft => Some(TerminalNavigationShortcut::Grid(
                    TerminalNavigationDirection::Left,
                )),
                Key::ArrowRight => Some(TerminalNavigationShortcut::Grid(
                    TerminalNavigationDirection::Right,
                )),
                Key::ArrowUp => Some(TerminalNavigationShortcut::Grid(
                    TerminalNavigationDirection::Up,
                )),
                Key::ArrowDown => Some(TerminalNavigationShortcut::Grid(
                    TerminalNavigationDirection::Down,
                )),
                _ => None,
            },
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if modifiers.ctrl && modifiers.alt && !modifiers.shift => match key {
                Key::ArrowUp => Some(TerminalNavigationShortcut::SingleViewLinear(
                    TerminalNavigationDirection::Up,
                )),
                Key::ArrowDown => Some(TerminalNavigationShortcut::SingleViewLinear(
                    TerminalNavigationDirection::Down,
                )),
                _ => None,
            },
            _ => None,
        }
    }

    fn active_terminal_navigation_shortcut(
        event: &Event,
        single_view_shortcuts_enabled: bool,
    ) -> Option<TerminalNavigationShortcut> {
        match Self::event_terminal_navigation_shortcut(event) {
            Some(TerminalNavigationShortcut::SingleViewLinear(_))
                if !single_view_shortcuts_enabled =>
            {
                None
            }
            Some(TerminalNavigationShortcut::Grid(
                direction @ (TerminalNavigationDirection::Up | TerminalNavigationDirection::Down),
            )) if single_view_shortcuts_enabled => {
                Some(TerminalNavigationShortcut::SingleViewLinear(direction))
            }
            Some(TerminalNavigationShortcut::Grid(
                direction
                @ (TerminalNavigationDirection::Left | TerminalNavigationDirection::Right),
            )) if single_view_shortcuts_enabled => {
                Some(TerminalNavigationShortcut::SingleViewFilter(direction))
            }
            shortcut => shortcut,
        }
    }

    fn event_is_alt_m_shortcut(event: &Event, global_modifiers: egui::Modifiers) -> bool {
        match event {
            Event::Key {
                key: Key::M,
                pressed: true,
                modifiers,
                ..
            } if (modifiers.alt || global_modifiers.alt)
                && !modifiers.ctrl
                && !global_modifiers.ctrl
                && !modifiers.shift
                && !global_modifiers.shift
                && !modifiers.command
                && !global_modifiers.command =>
            {
                true
            }
            Event::Text(ref text) if text == "m" && global_modifiers.alt => true,
            _ => false,
        }
    }

    fn partition_alt_m_shortcut(
        events: Vec<Event>,
        global_modifiers: egui::Modifiers,
    ) -> (Vec<Event>, Vec<Event>) {
        let mut alt_m_events = Vec::new();
        let mut remaining_events = Vec::new();

        for event in events {
            if Self::event_is_alt_m_shortcut(&event, global_modifiers) {
                alt_m_events.push(event);
            } else {
                remaining_events.push(event);
            }
        }

        (alt_m_events, remaining_events)
    }

    fn partition_terminal_input_events(
        events: Vec<Event>,
        single_view_shortcuts_enabled: bool,
    ) -> (Vec<Event>, Vec<Event>) {
        let mut terminal_events = Vec::new();
        let mut remaining_events = Vec::new();

        for event in events {
            if Self::event_is_terminal_input(&event)
                && Self::active_terminal_navigation_shortcut(&event, single_view_shortcuts_enabled)
                    .is_none()
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
        single_view_shortcuts_enabled: bool,
    ) -> (Vec<TerminalNavigationShortcut>, Vec<Event>) {
        let mut shortcuts = Vec::new();
        let mut remaining_events = Vec::new();

        for event in events {
            if let Some(shortcut) =
                Self::active_terminal_navigation_shortcut(&event, single_view_shortcuts_enabled)
            {
                shortcuts.push(shortcut);
            } else {
                remaining_events.push(event);
            }
        }

        (shortcuts, remaining_events)
    }

    fn is_repeatable_terminal_key(key: Key, modifiers: egui::Modifiers) -> bool {
        matches!(key, Key::Backspace | Key::Delete)
            && Self::key_to_terminal_bytes(key, modifiers).is_some()
    }

    fn clear_terminal_held_key_repeat(&mut self) {
        self.terminal_held_key_repeat = None;
    }

    fn clear_terminal_held_key_repeat_for_terminal(&mut self, terminal_id: u64) {
        if self
            .terminal_held_key_repeat
            .is_some_and(|state| state.terminal_id == terminal_id)
        {
            self.clear_terminal_held_key_repeat();
        }
    }

    fn terminal_held_key_repeat_timing() -> TerminalHeldKeyRepeatTiming {
        #[cfg(target_os = "windows")]
        {
            static TIMING: OnceLock<TerminalHeldKeyRepeatTiming> = OnceLock::new();
            *TIMING.get_or_init(Self::load_windows_terminal_held_key_repeat_timing)
        }

        #[cfg(not(target_os = "windows"))]
        {
            TerminalHeldKeyRepeatTiming {
                initial_delay_secs: TERMINAL_HELD_KEY_REPEAT_FALLBACK_INITIAL_DELAY_SECS,
                interval_secs: TERMINAL_HELD_KEY_REPEAT_FALLBACK_INTERVAL_SECS,
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn load_windows_terminal_held_key_repeat_timing() -> TerminalHeldKeyRepeatTiming {
        let fallback = TerminalHeldKeyRepeatTiming {
            initial_delay_secs: TERMINAL_HELD_KEY_REPEAT_FALLBACK_INITIAL_DELAY_SECS,
            interval_secs: TERMINAL_HELD_KEY_REPEAT_FALLBACK_INTERVAL_SECS,
        };

        let mut keyboard_delay = 0u32;
        let mut keyboard_speed = 0u32;
        let delay_ok = unsafe {
            SystemParametersInfoW(
                SPI_GETKEYBOARDDELAY,
                0,
                (&mut keyboard_delay as *mut u32).cast(),
                0,
            )
        } != 0;
        let speed_ok = unsafe {
            SystemParametersInfoW(
                SPI_GETKEYBOARDSPEED,
                0,
                (&mut keyboard_speed as *mut u32).cast(),
                0,
            )
        } != 0;
        if !delay_ok || !speed_ok {
            return fallback;
        }

        keyboard_delay = keyboard_delay.min(3);
        keyboard_speed = keyboard_speed.min(31);

        let repeats_per_second = 2.5 + (f64::from(keyboard_speed) * (27.5 / 31.0));
        if repeats_per_second <= 0.0 {
            return fallback;
        }

        TerminalHeldKeyRepeatTiming {
            initial_delay_secs: 0.25 * f64::from(keyboard_delay + 1),
            interval_secs: (1.0 / repeats_per_second).max(0.001),
        }
    }

    fn due_terminal_held_key_repeat_events(
        &mut self,
        active_terminal_id: u64,
        now: f64,
    ) -> Vec<Event> {
        let Some(state) = self.terminal_held_key_repeat.as_mut() else {
            return Vec::new();
        };
        if state.terminal_id != active_terminal_id {
            self.clear_terminal_held_key_repeat();
            return Vec::new();
        }

        let timing = Self::terminal_held_key_repeat_timing();
        let mut due_events = Vec::new();
        let mut next_repeat_at = state
            .last_repeat_at
            .map(|last_repeat_at| last_repeat_at + timing.interval_secs)
            .unwrap_or(state.first_pressed_at + timing.initial_delay_secs);

        while now + f64::EPSILON >= next_repeat_at
            && due_events.len() < TERMINAL_HELD_KEY_REPEAT_MAX_SYNTHETIC_EVENTS_PER_FRAME
        {
            state.last_repeat_at = Some(next_repeat_at);
            due_events.push(Event::Key {
                key: state.key,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers: state.modifiers,
            });
            next_repeat_at += timing.interval_secs;
        }

        due_events
    }

    fn preprocess_terminal_input_with_held_repeat(
        &mut self,
        ctx: &egui::Context,
        events: Vec<Event>,
    ) -> Vec<Event> {
        let active_terminal_id = self.active_terminal_accepts_input();
        let should_capture_terminal_keyboard = self.should_capture_terminal_keyboard(ctx);
        let now = ctx.input(|input| input.time);

        self.preprocess_terminal_input_with_held_repeat_state(
            active_terminal_id,
            should_capture_terminal_keyboard,
            now,
            events,
        )
    }

    fn preprocess_terminal_input_with_held_repeat_state(
        &mut self,
        active_terminal_id: Option<u64>,
        should_capture_terminal_keyboard: bool,
        now: f64,
        events: Vec<Event>,
    ) -> Vec<Event> {
        if !should_capture_terminal_keyboard {
            self.clear_terminal_held_key_repeat();
            return events;
        }

        let Some(active_terminal_id) = active_terminal_id else {
            self.clear_terminal_held_key_repeat();
            return events;
        };

        if self
            .terminal_held_key_repeat
            .is_some_and(|state| state.terminal_id != active_terminal_id)
        {
            self.clear_terminal_held_key_repeat();
        }

        let mut processed_events = Vec::with_capacity(events.len() + 1);
        for event in events {
            match event {
                Event::Key {
                    key,
                    physical_key,
                    pressed,
                    repeat,
                    modifiers,
                } if Self::is_repeatable_terminal_key(key, modifiers) => {
                    if pressed {
                        let is_existing_repeat =
                            self.terminal_held_key_repeat.is_some_and(|state| {
                                state.terminal_id == active_terminal_id
                                    && state.key == key
                                    && state.modifiers == modifiers
                            });
                        if is_existing_repeat {
                            continue;
                        }

                        self.terminal_held_key_repeat = Some(TerminalHeldKeyRepeat {
                            terminal_id: active_terminal_id,
                            key,
                            modifiers,
                            first_pressed_at: now,
                            last_repeat_at: None,
                        });
                    } else if self.terminal_held_key_repeat.is_some_and(|state| {
                        state.terminal_id == active_terminal_id
                            && state.key == key
                            && state.modifiers == modifiers
                    }) {
                        self.clear_terminal_held_key_repeat();
                    }

                    processed_events.push(Event::Key {
                        key,
                        physical_key,
                        pressed,
                        repeat,
                        modifiers,
                    });
                }
                Event::Key { pressed: true, .. }
                | Event::Text(_)
                | Event::Paste(_)
                | Event::Copy
                | Event::Cut => {
                    self.clear_terminal_held_key_repeat();
                    processed_events.push(event);
                }
                _ => {
                    processed_events.push(event);
                }
            }
        }

        processed_events.extend(self.due_terminal_held_key_repeat_events(active_terminal_id, now));
        processed_events
    }

    fn capture_active_terminal_input(&self, ctx: &egui::Context) -> Vec<Event> {
        if !self.should_capture_terminal_keyboard(ctx) {
            return Vec::new();
        }

        let single_view_shortcuts_enabled = !self.config.ui.multi_terminal_view_enabled;
        ctx.input_mut(|input| {
            let events = std::mem::take(&mut input.events);
            let (terminal_events, remaining_events) =
                Self::partition_terminal_input_events(events, single_view_shortcuts_enabled);
            input.events = remaining_events;
            terminal_events
        })
    }

    fn take_buffered_terminal_input(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.buffered_terminal_input)
    }

    fn take_buffered_terminal_navigation_shortcuts(&mut self) -> Vec<TerminalNavigationShortcut> {
        std::mem::take(&mut self.buffered_terminal_navigation)
    }

    fn take_terminal_navigation_shortcuts(
        &mut self,
        ctx: &egui::Context,
    ) -> Vec<TerminalNavigationShortcut> {
        let single_view_shortcuts_enabled = !self.config.ui.multi_terminal_view_enabled;
        let mut shortcuts = self.take_buffered_terminal_navigation_shortcuts();
        shortcuts.extend(ctx.input_mut(|input| {
            let events = std::mem::take(&mut input.events);
            let (shortcuts, remaining_events) = Self::partition_terminal_navigation_shortcuts(
                events,
                single_view_shortcuts_enabled,
            );
            input.events = remaining_events;
            shortcuts
        }));
        shortcuts
    }

    fn visible_terminal_ids_for_main(&self) -> Vec<u64> {
        if !self.config.ui.multi_terminal_view_enabled {
            return self.single_terminal_id_for_main().into_iter().collect();
        }

        let mut ids = self
            .terminals
            .iter()
            .filter_map(|(id, terminal)| self.terminal_visible_in_main(terminal).then_some(*id))
            .collect::<Vec<_>>();

        ids.sort_unstable();
        ids
    }

    fn terminal_ids_for_single_view_navigation(&self) -> Vec<u64> {
        if self.projects.is_empty() {
            return self.terminals.keys().copied().collect();
        }

        let kind = self.config.ui.terminal_manager_filter.terminal_kind();
        let mut result = Vec::new();

        let mut project_ids = self.projects.keys().copied().collect::<Vec<_>>();
        project_ids.sort_unstable();

        for project_id in project_ids {
            let ids = terminal_ids_for_project_kind(&self.terminals, project_id, kind);
            result.extend(ids);
        }

        result
    }

    fn route_active_terminal_input(&mut self, ctx: &egui::Context, events: Vec<Event>) {
        let allow_attention_override =
            std::mem::take(&mut self.allow_attention_terminal_input_routing_once);

        if self.ui_owns_keyboard(ctx) && !allow_attention_override {
            return;
        }

        let Some(active_terminal_id) = self.active_terminal_accepts_input() else {
            return;
        };

        if events.is_empty() {
            return;
        }

        let mut outbound = Vec::new();
        let mut copied_selection = None;
        let mut last_key_was_alt_m = false;
        let mut launched_factory_droid = false;
        let mut launched_codex_cli = false;
        let mut codex_launch_baseline = None;
        let mut submitted_factory_prompt = false;
        let mut submitted_codex_prompt = false;
        let mut sent_terminal_input = false;
        {
            let Some(terminal) = self.terminals.get_mut(&active_terminal_id) else {
                return;
            };

            for event in events {
                match event {
                    Event::Copy => {
                        let copied_text = terminal
                            .selection
                            .as_ref()
                            .is_some_and(TerminalSelection::has_selection)
                            .then(|| Self::selected_terminal_text(terminal))
                            .flatten();
                        let action = resolve_ctrl_c_action(copied_text.is_some());

                        match action {
                            CtrlCAction::CopySelection => {
                                copied_selection = copied_text;
                                Self::clear_terminal_selection(terminal);
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
                        // Suppress duplicate Text("m") that follows Alt+M key event
                        if last_key_was_alt_m && text == "m" {
                            last_key_was_alt_m = false;
                            continue;
                        }
                        Self::clear_terminal_selection(terminal);
                        outbound.extend_from_slice(text.as_bytes());
                        sent_terminal_input = true;
                        Self::append_pending_line(&mut terminal.pending_line_for_title, &text);
                    }
                    Event::Paste(text) => {
                        if text.is_empty() {
                            continue;
                        }
                        Self::clear_terminal_selection(terminal);
                        Self::flush_terminal_outbound(terminal, ctx, &mut outbound);
                        Self::deliver_pasted_text_to_terminal(terminal, &text, ctx);
                        sent_terminal_input = true;
                    }
                    Event::Key {
                        key,
                        pressed,
                        modifiers,
                        ..
                    } if pressed => {
                        last_key_was_alt_m = false;

                        if key == Key::Enter {
                            Self::clear_terminal_selection(terminal);
                            outbound.push(b'\r');
                            sent_terminal_input = true;
                            let line = std::mem::take(&mut terminal.pending_line_for_title);
                            if Self::is_factory_droid_launch_command(&line) {
                                launched_factory_droid = true;
                            }
                            if Self::is_codex_launch_command(&line) {
                                launched_codex_cli = true;
                                codex_launch_baseline =
                                    terminal.runtime.snapshot_codex_descendant_processes();
                            }
                            let sanitized_line = terminal_title_candidate(&line);
                            if terminal.factory_droid_session_active
                                && !launched_factory_droid
                                && !launched_codex_cli
                                && sanitized_line
                                    .as_ref()
                                    .is_some_and(|candidate| !candidate.trim().is_empty())
                            {
                                submitted_factory_prompt = true;
                            }
                            if terminal.codex_session_active
                                && !launched_factory_droid
                                && !launched_codex_cli
                                && sanitized_line
                                    .as_ref()
                                    .is_some_and(|candidate| !candidate.trim().is_empty())
                            {
                                submitted_codex_prompt = true;
                            }
                            if let Some(sanitized) = sanitized_line {
                                terminal.full_title = sanitized.clone();
                                terminal.title = update_terminal_title(
                                    &sanitized,
                                    terminal.id as usize,
                                    TITLE_MAX_LEN,
                                );
                            }
                            terminal.dirty = true;
                            continue;
                        }

                        if key == Key::Backspace {
                            Self::clear_terminal_selection(terminal);
                            terminal.pending_line_for_title.pop();
                        }

                        // Encode Alt+M as ESC+m for terminal
                        if modifiers.alt && key == Key::M && !modifiers.ctrl && !modifiers.command {
                            Self::clear_terminal_selection(terminal);
                            outbound.push(b'\x1b');
                            outbound.push(b'm');
                            sent_terminal_input = true;
                            last_key_was_alt_m = true;
                            continue;
                        }

                        if let Some(bytes) = Self::key_to_terminal_bytes(key, modifiers) {
                            Self::clear_terminal_selection(terminal);
                            outbound.extend_from_slice(&bytes);
                            sent_terminal_input = true;
                        }
                    }
                    _ => {}
                }
            }

            Self::flush_terminal_outbound(terminal, ctx, &mut outbound);
        }

        if launched_factory_droid {
            self.clear_codex_state(active_terminal_id);
            self.mark_factory_droid_launch_pending(active_terminal_id);
        }
        if launched_codex_cli {
            self.clear_factory_droid_state(active_terminal_id);
            self.prepare_codex_cli_integration_for_launch();
            self.mark_codex_launch_pending(active_terminal_id, codex_launch_baseline);
        }

        if let Some(ref text) = copied_selection {
            ctx.copy_text(text.clone());
            self.show_terminal_copy_feedback(ctx);
        }

        // Clear AI attention status when user types input
        if let Some(manager) = &self.ai_hook_manager {
            let has_copied = copied_selection.is_some();
            if sent_terminal_input || has_copied {
                if let Some((tool, status)) = manager.user_interacted(active_terminal_id) {
                    let should_clear_sticky_codex = self
                        .terminals
                        .get(&active_terminal_id)
                        .is_some_and(|entry| {
                            tool == AiCliTool::CodexCli
                                && status == AiCliStatus::Inactive
                                && entry.ai_session.tool == Some(AiCliTool::CodexCli)
                                && !entry.codex_session_active
                                && entry.codex_launch_pending_since.is_none()
                                && entry.codex_process_identity.is_none()
                        });
                    if should_clear_sticky_codex {
                        self.clear_codex_state(active_terminal_id);
                    } else if let Some(entry) = self.terminals.get_mut(&active_terminal_id) {
                        entry.ai_session.tool = Some(tool);
                        entry.ai_session.status = status;
                    }
                    ctx.request_repaint();
                }
            }
        }

        if submitted_factory_prompt {
            if self.apply_factory_droid_status(
                active_terminal_id,
                AiCliStatus::Running,
                FactoryDroidStatusSource::PromptSubmit,
            ) {
                ctx.request_repaint();
            }
        }
        if submitted_codex_prompt {
            if self.apply_codex_status(
                active_terminal_id,
                AiCliStatus::Running,
                CodexCliStatusSource::PromptSubmit,
            ) {
                ctx.request_repaint();
            }
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
        configure_terminal_font_family(&mut fonts);
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

    fn flush_terminal_outbound(
        terminal: &mut TerminalEntry,
        ctx: &egui::Context,
        outbound: &mut Vec<u8>,
    ) {
        if outbound.is_empty() {
            return;
        }

        terminal.runtime.send_bytes(std::mem::take(outbound));
        terminal.dirty = true;
        ctx.request_repaint();
    }

    fn deliver_pasted_bytes_to_terminal(
        terminal: &mut TerminalEntry,
        text: &str,
        paste_bytes: Vec<u8>,
        ctx: &egui::Context,
    ) {
        Self::clear_terminal_selection(terminal);
        terminal.runtime.send_paste_bytes(paste_bytes);
        Self::append_pending_line(&mut terminal.pending_line_for_title, text);
        terminal.dirty = true;
        ctx.request_repaint();
    }

    fn deliver_pasted_text_to_terminal(
        terminal: &mut TerminalEntry,
        text: &str,
        ctx: &egui::Context,
    ) {
        let Some(paste_bytes) = terminal.runtime.capture_paste_bytes(text) else {
            return;
        };

        Self::deliver_pasted_bytes_to_terminal(terminal, text, paste_bytes, ctx);
    }

    fn queue_pasted_text_to_terminal(&mut self, terminal_id: u64, text: &str) -> bool {
        let Some(terminal) = self.terminals.get(&terminal_id) else {
            self.status_line = "Target terminal not found".to_owned();
            return false;
        };

        if terminal.exited {
            self.status_line = format!("{} is exited", terminal.title);
            return false;
        }

        if text.is_empty() {
            return false;
        }

        let Some(paste_bytes) = terminal.runtime.capture_paste_bytes(text) else {
            self.status_line = "Paste failed".to_owned();
            return false;
        };

        self.pending_terminal_pastes.push(PendingTerminalPaste {
            terminal_id,
            text: text.to_owned(),
            bytes: paste_bytes,
        });
        true
    }

    fn flush_pending_terminal_pastes(&mut self, ctx: &egui::Context) {
        let pending_pastes = std::mem::take(&mut self.pending_terminal_pastes);
        if pending_pastes.is_empty() {
            return;
        }

        for paste in pending_pastes {
            let Some(terminal) = self.terminals.get_mut(&paste.terminal_id) else {
                self.status_line = "Target terminal not found".to_owned();
                continue;
            };

            if terminal.exited {
                self.status_line = format!("{} is exited", terminal.title);
                continue;
            }

            Self::deliver_pasted_bytes_to_terminal(terminal, &paste.text, paste.bytes, ctx);
        }
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
            (Key::Backspace, _) => b"\x7f".as_slice(),
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

        self.clear_terminal_held_key_repeat_for_terminal(terminal_id);
        self.clear_factory_droid_state(terminal_id);
        self.reset_factory_droid_hook_inbox(terminal_id);
        self.clear_codex_state(terminal_id);
        self.reset_codex_notify_inbox(terminal_id);
        self.terminals.remove(&terminal_id);
        self.status_line = match close_result {
            Ok(()) => format!("Closed {title}"),
            Err(err) => format!("Closed {title} (cleanup failed: {err})"),
        };

        let remaining_terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        self.set_active_terminal(
            ctx,
            next_active_terminal_after_close(
                self.active_terminal,
                terminal_id,
                &remaining_terminal_ids,
            ),
        );
        self.bump_layout_epoch();
        ctx.request_repaint();
    }

    fn set_active_terminal(&mut self, ctx: &egui::Context, terminal_id: Option<u64>) {
        if self.active_terminal == terminal_id {
            if let Some(terminal_id) = terminal_id {
                self.acknowledge_terminal_attention(terminal_id);
            }
            ctx.request_repaint();
            return;
        }

        if let Some(terminal_id) = terminal_id {
            self.acknowledge_terminal_attention(terminal_id);
        }

        self.active_terminal = terminal_id;
        self.clear_terminal_held_key_repeat();
        self.clear_terminal_selections_except(terminal_id);

        // Repaint to update AI badge state
        ctx.request_repaint();
    }

    fn acknowledge_terminal_attention(&mut self, terminal_id: u64) {
        let Some(manager) = &self.ai_hook_manager else {
            return;
        };

        if let Some((tool, status)) = manager.user_interacted(terminal_id) {
            if let Some(entry) = self.terminals.get_mut(&terminal_id) {
                entry.ai_session.tool = Some(tool);
                entry.ai_session.status = status;
            }
        }
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
        terminal.selection_drag_active = false;
    }

    fn clear_collapsed_terminal_selection(terminal: &mut TerminalEntry) {
        if terminal
            .selection
            .as_ref()
            .is_some_and(|selection| !selection.has_selection())
        {
            Self::clear_terminal_selection(terminal);
        }
    }

    fn clear_pending_terminal_link_click(terminal: &mut TerminalEntry) {
        terminal.pending_link_click = None;
    }

    fn begin_terminal_primary_interaction(
        terminal: &mut TerminalEntry,
        point: TerminalSelectionPoint,
        link_under_pointer: Option<String>,
    ) {
        terminal.selection = Some(TerminalSelection::collapsed(point));
        terminal.selection_drag_active = true;
        if let Some(url) = link_under_pointer {
            terminal.pending_link_click = Some(PendingTerminalLinkClick { anchor: point, url });
        } else {
            Self::clear_pending_terminal_link_click(terminal);
        }
    }

    fn update_terminal_primary_drag(terminal: &mut TerminalEntry, point: TerminalSelectionPoint) {
        if terminal
            .pending_link_click
            .as_ref()
            .is_some_and(|pending| pending.anchor == point)
        {
            return;
        }

        let anchor = terminal
            .pending_link_click
            .take()
            .map(|pending| pending.anchor)
            .or_else(|| {
                terminal
                    .selection
                    .as_ref()
                    .map(|selection| selection.anchor)
            })
            .unwrap_or(point);
        let selection = terminal
            .selection
            .get_or_insert_with(|| TerminalSelection::collapsed(anchor));
        selection.focus = point;
        terminal.selection_drag_active = true;
    }

    fn take_terminal_primary_clicked_link(
        terminal: &mut TerminalEntry,
        link_under_pointer: Option<&str>,
        link_activation_active: bool,
    ) -> Option<String> {
        let pending = terminal.pending_link_click.take()?;
        if link_activation_active && link_under_pointer == Some(pending.url.as_str()) {
            Self::clear_collapsed_terminal_selection(terminal);
            terminal.selection_drag_active = false;
            return Some(pending.url);
        }

        terminal.selection_drag_active = false;
        None
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

    fn paste_clipboard_to_terminal(&mut self, terminal_id: u64) {
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

        if self.queue_pasted_text_to_terminal(terminal_id, &text) {
            self.status_line = "Pasted clipboard into terminal".to_owned();
        }
    }

    fn send_saved_message_to_terminal(&mut self, terminal_id: u64, message: &str) {
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
        if let Some(sanitized) = terminal_title_candidate(&line) {
            terminal.full_title = sanitized;
            terminal.title = update_terminal_title(&line, terminal.id as usize, TITLE_MAX_LEN);
        }

        Self::push_recent_input(&mut terminal.recent_inputs, message);

        terminal.dirty = true;
        self.status_line = format!("Sent saved message to {}", destination_title);
    }

    const RECENT_INPUTS_MAX: usize = 4;

    fn push_recent_input(recent_inputs: &mut VecDeque<String>, message: &str) {
        if message.trim().is_empty() {
            return;
        }
        recent_inputs.push_front(message.to_owned());
        while recent_inputs.len() > Self::RECENT_INPUTS_MAX {
            recent_inputs.pop_back();
        }
    }

    #[allow(dead_code)]
    fn send_ai_command_to_project_terminal(&mut self, project_id: u64, command: &str) {
        let target_terminal_id = self
            .active_terminal
            .filter(|tid| {
                self.terminals
                    .get(tid)
                    .is_some_and(|t| t.project_id == project_id && !t.exited)
            })
            .or_else(|| {
                self.terminals
                    .iter()
                    .filter(|(_, t)| t.project_id == project_id && !t.exited)
                    .map(|(&id, _)| id)
                    .next()
            });

        let Some(target_terminal_id) = target_terminal_id else {
            self.status_line = "No active terminal for AI command".to_owned();
            return;
        };

        let Some(terminal) = self.terminals.get_mut(&target_terminal_id) else {
            return;
        };

        let mut outbound = command.as_bytes().to_vec();
        outbound.push(b'\r');
        terminal.runtime.send_bytes(outbound);
        Self::clear_terminal_selection(terminal);
        Self::append_pending_line(&mut terminal.pending_line_for_title, command);
        let line = std::mem::take(&mut terminal.pending_line_for_title);
        if let Some(sanitized) = terminal_title_candidate(&line) {
            terminal.full_title = sanitized;
            terminal.title = update_terminal_title(&line, terminal.id as usize, TITLE_MAX_LEN);
        }

        terminal.dirty = true;
        self.status_line = format!("Sent '{}' to {}", command, terminal.title);
    }

    fn finalize_pointer_selection_copy(&mut self, ctx: &egui::Context) {
        self.show_terminal_copy_feedback(ctx);
    }

    fn show_terminal_copy_feedback(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        self.status_line = TERMINAL_COPY_FEEDBACK_TEXT.to_owned();
        self.copy_toast = Some(TransientToast {
            message: TERMINAL_COPY_FEEDBACK_TEXT.to_owned(),
            expires_at: now + TERMINAL_COPY_TOAST_SECS,
        });
        ctx.request_repaint();
        ctx.request_repaint_after(Duration::from_secs_f64(TERMINAL_COPY_TOAST_SECS));
    }

    fn active_copy_toast_message(copy_toast: Option<&TransientToast>, now: f64) -> Option<&str> {
        copy_toast.and_then(|toast| (now <= toast.expires_at).then_some(toast.message.as_str()))
    }

    fn draw_copy_toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        let Some(message) =
            Self::active_copy_toast_message(self.copy_toast.as_ref(), now).map(str::to_owned)
        else {
            self.copy_toast = None;
            return;
        };
        let remaining = self
            .copy_toast
            .as_ref()
            .map(|toast| (toast.expires_at - now).max(0.0))
            .unwrap_or_default();
        ctx.request_repaint_after(Duration::from_secs_f64(remaining));

        egui::Area::new(Id::new("terminal_copy_toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-18.0, -18.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(SURFACE_BG_SOFT)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(message)
                                .color(TEXT_PRIMARY)
                                .strong()
                                .size(13.0),
                        );
                    });
            });
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

    fn factory_droid_transport_diagnostics(&self) -> FactoryDroidTransportDiagnostics {
        let active_terminal = self
            .active_terminal
            .and_then(|terminal_id| self.terminals.get(&terminal_id));
        FactoryDroidTransportDiagnostics {
            hooks_enabled: self.ai_hook_manager.is_some(),
            executable_path: self.current_executable_path.clone(),
            hooks_runtime_dir: self.factory_droid_hooks_dir.clone(),
            hooks_runtime_error: self.factory_droid_hooks_dir_error.clone(),
            active_session: active_terminal.map(|terminal| terminal.factory_droid_session_active),
            process_state: active_terminal
                .map(Self::factory_droid_process_state_text)
                .map(str::to_owned),
            last_status_source: active_terminal
                .and_then(|terminal| terminal.factory_droid_last_status_source),
        }
    }

    fn draw_settings_diagnostic_row(ui: &mut Ui, label: &str, value: &str, value_color: Color32) {
        ui.label(RichText::new(label).strong().color(TEXT_PRIMARY));
        ui.add(
            egui::Label::new(RichText::new(value).monospace().small().color(value_color))
                .truncate(),
        )
        .on_hover_text(value);
        ui.add_space(4.0);
    }

    fn open_settings_popup(&mut self) {
        self.show_settings_popup = true;
        self.settings_diagnostics_expanded = false;
    }

    fn draw_settings_diagnostics_section(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Diagnostics", icons::EYE))
                    .strong()
                    .size(15.0)
                    .color(TEXT_PRIMARY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let toggle_label = if self.settings_diagnostics_expanded {
                    "Hide"
                } else {
                    "Show"
                };
                if ui.button(toggle_label).clicked() {
                    self.settings_diagnostics_expanded = !self.settings_diagnostics_expanded;
                }
            });
        });

        if !self.settings_diagnostics_expanded {
            return;
        }

        let diagnostics = self.factory_droid_transport_diagnostics();
        egui::Frame::none()
            .fill(with_alpha(SURFACE_BG, 216))
            .stroke(Stroke::new(1.0, BORDER_COLOR))
            .rounding(10.0)
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings-diagnostics-scroll")
                    .max_height(220.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(
                                "Factory Droid status uses PTY/process detection first. Inbox JSONL remains a best-effort fallback.",
                            )
                            .color(TEXT_MUTED)
                            .small(),
                        );
                        ui.add_space(4.0);
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Executable Path",
                            &diagnostics.executable_path.display().to_string(),
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Factory Droid Primary",
                            FactoryDroidTransportDiagnostics::PRIMARY_TRANSPORT_LABEL,
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Factory Droid Fallback",
                            FactoryDroidTransportDiagnostics::FALLBACK_TRANSPORT_LABEL,
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Factory Droid Inbox",
                            &diagnostics.runtime_status_text(),
                            if diagnostics.hooks_runtime_dir.is_some() {
                                Color32::from_rgb(114, 209, 152)
                            } else {
                                Color32::from_rgb(232, 184, 76)
                            },
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Droid Session Active",
                            diagnostics.active_session_text(),
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Factory Droid Process State",
                            diagnostics.process_state_text(),
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Last Status Source",
                            diagnostics.last_status_source_text(),
                            TEXT_PRIMARY,
                        );
                        if let Some(warning_message) = diagnostics.warning_message() {
                            ui.label(
                                RichText::new(warning_message)
                                    .small()
                                    .color(Color32::from_rgb(232, 184, 76)),
                            );
                        }

                        ui.separator();
                        let active_terminal = self
                            .active_terminal
                            .and_then(|terminal_id| self.terminals.get(&terminal_id));
                        let codex_runtime_text = if let Some(dir) = &self.codex_cli_runtime_dir {
                            format!("Ready: {}", dir.display())
                        } else if let Some(err) = &self.codex_cli_runtime_dir_error {
                            format!("Unavailable: {err}")
                        } else {
                            "Unavailable: unknown error".to_owned()
                        };
                        let codex_config_text = match codex::user_codex_config_path() {
                            Ok(path) => path.display().to_string(),
                            Err(err) => format!("Unavailable: {err}"),
                        };
                        let codex_session_text = match active_terminal {
                            Some(terminal) if terminal.codex_session_active => "Yes",
                            Some(_) => "No",
                            None => "No active terminal",
                        };
                        let codex_process_text = match active_terminal {
                            Some(terminal) if terminal.exited => "terminal exited".to_owned(),
                            Some(terminal) if terminal.codex_session_active => {
                                "session active".to_owned()
                            }
                            Some(terminal) if terminal.codex_launch_pending_since.is_some() => {
                                "launch pending".to_owned()
                            }
                            Some(terminal) if terminal.codex_process_missing_since.is_some() => {
                                "awaiting trailing output".to_owned()
                            }
                            Some(terminal)
                                if terminal.ai_session.tool == Some(AiCliTool::CodexCli)
                                    && terminal.ai_session.status == AiCliStatus::Attention =>
                            {
                                "attention needed".to_owned()
                            }
                            Some(_) => "idle".to_owned(),
                            None => "No active terminal".to_owned(),
                        };
                        let codex_last_status_source = active_terminal
                            .and_then(|terminal| terminal.codex_last_status_source)
                            .map(CodexCliStatusSource::label)
                            .unwrap_or("none");
                        ui.label(
                            RichText::new("Codex CLI")
                                .strong()
                                .size(15.0)
                                .color(TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(
                                "Windows support in Codex CLI is still experimental. For now Mergen only wires native Windows sessions; WSL bridging stays out of scope in this release.",
                            )
                            .color(Color32::from_rgb(232, 184, 76))
                            .small(),
                        );
                        ui.label(
                            RichText::new(
                                "Official Codex hooks are currently disabled on native Windows, so Mergen relies on Codex notify for turn-complete detection and uses BEL-backed TUI notifications only as a supplemental signal.",
                            )
                            .color(TEXT_MUTED)
                            .small(),
                        );
                        ui.add_space(4.0);
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Codex Config",
                            &codex_config_text,
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Codex Inbox",
                            &codex_runtime_text,
                            if self.codex_cli_runtime_dir.is_some() {
                                Color32::from_rgb(114, 209, 152)
                            } else {
                                Color32::from_rgb(232, 184, 76)
                            },
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Codex Session Active",
                            codex_session_text,
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Codex Process State",
                            &codex_process_text,
                            TEXT_PRIMARY,
                        );
                        Self::draw_settings_diagnostic_row(
                            ui,
                            "Last Codex Source",
                            codex_last_status_source,
                            TEXT_PRIMARY,
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Enable Codex CLI integration").clicked() {
                                self.enable_codex_cli_integration(ctx);
                            }
                            if ui.button("Open Codex setup docs").clicked() {
                                ctx.open_url(egui::OpenUrl::new_tab(codex::codex_setup_url()));
                            }
                        });
                    });
            });
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
                            let diagnostics = self.factory_droid_transport_diagnostics();
                            if styled_icon_button(
                                ui,
                                icons::GEAR,
                                BTN_SUBTLE,
                                BTN_SUBTLE_HOVER,
                                BTN_ICON_ACTIVE,
                                "Settings",
                            ) {
                                self.open_settings_popup();
                            }

                            if let Some(warning_message) = diagnostics.warning_message() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Factory Droid inbox fallback unavailable")
                                        .small()
                                        .strong()
                                        .color(Color32::from_rgb(232, 184, 76)),
                                )
                                .on_hover_text(warning_message);
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
                        let mut remove_selected_project = false;
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
                                    30.0 * 5.0 + ui.spacing().item_spacing.x * 4.0;
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
                                    let search_active =
                                        !self.directory_search_query.trim().is_empty();
                                    ui.add_enabled_ui(!search_active, |ui| {
                                        let open_all = selected_project_details
                                            .as_ref()
                                            .map(|(project_id, _, _, _)| {
                                                self.cached_directory_tree_has_collapsed_folders(
                                                    *project_id,
                                                    ui.ctx(),
                                                )
                                            })
                                            .unwrap_or(true);
                                        if styled_icon_button(
                                            ui,
                                            if open_all {
                                                icons::EYE
                                            } else {
                                                icons::EYE_OFF
                                            },
                                            BTN_ICON,
                                            BTN_ICON_HOVER,
                                            BTN_ICON_ACTIVE,
                                            if open_all {
                                                "Expand All Folders"
                                            } else {
                                                "Collapse All Folders"
                                            },
                                        ) {
                                            if let Some((project_id, _, _, _)) =
                                                selected_project_details.as_ref()
                                            {
                                                self.directory_tree_has_collapsed_cache_by_project
                                                    .insert(*project_id, open_all);
                                                self.directory_pending_tree_open_state_by_project
                                                    .insert(*project_id, open_all);
                                            }
                                        }
                                    });
                                    if styled_icon_button(
                                        ui,
                                        icons::TRASH,
                                        BTN_RED,
                                        BTN_RED_HOVER,
                                        Color32::from_rgb(186, 58, 58),
                                        "Remove Project",
                                    ) {
                                        remove_selected_project = true;
                                    }
                                });
                            });
                        });
                        if remove_selected_project {
                            if let Some((project_id, _, _, _)) = selected_project_details {
                                self.remove_project(ctx, project_id);
                                return;
                            }
                        }
                        if self.selected_project != previous_selected_project {
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
                                                RichText::new(directory_index_loading_label(
                                                    ui.ctx().input(|input| input.time),
                                                ))
                                                .color(TEXT_MUTED),
                                            );
                                            ui.ctx().request_repaint_after(
                                                Duration::from_secs_f64(
                                                    DIRECTORY_INDEX_LOADING_ANIMATION_STEP_SECS,
                                                ),
                                            );
                                            return;
                                        };

                                        ui.label(
                                            RichText::new(format!("{} Files", icons::FOLDER_OPEN))
                                                .color(TEXT_MUTED)
                                                .strong(),
                                        );

                                        let pending_open_all = search_query
                                            .is_none()
                                            .then(|| {
                                                self.directory_pending_tree_open_state_by_project
                                                    .get(&project_id)
                                                    .copied()
                                            })
                                            .flatten();

                                        if snapshot.loading {
                                            ui.label(
                                                RichText::new(directory_index_loading_label(
                                                    ui.ctx().input(|input| input.time),
                                                ))
                                                .color(TEXT_MUTED),
                                            );
                                            ui.ctx().request_repaint_after(
                                                Duration::from_secs_f64(
                                                    DIRECTORY_INDEX_LOADING_ANIMATION_STEP_SECS,
                                                ),
                                            );
                                        }

                                        if let Some(error) = &snapshot.last_error {
                                            ui.colored_label(Color32::LIGHT_RED, error);
                                            if pending_open_all.is_some() {
                                                self.directory_pending_tree_open_state_by_project
                                                    .remove(&project_id);
                                                status_line_update = Some(
                                                    "Could not update folder visibility because directory index is unavailable"
                                                        .to_owned(),
                                                );
                                            }
                                        } else if !snapshot.loading {
                                            if let Some(open_all) = pending_open_all {
                                                apply_directory_tree_open_state(
                                                    ui.ctx(),
                                                    &snapshot.root,
                                                    open_all,
                                                );
                                                self.directory_pending_tree_open_state_by_project
                                                    .remove(&project_id);
                                                self.directory_tree_has_collapsed_cache_by_project
                                                    .insert(project_id, !open_all);
                                                status_line_update = Some(if open_all {
                                                    "Expanded all folders".to_owned()
                                                } else {
                                                    "Collapsed all folders".to_owned()
                                                });
                                            }

                                            let mut matching_directories = HashSet::new();
                                            if let Some(query) = search_query.as_deref() {
                                                let _ = collect_matching_directory_paths(
                                                    &snapshot.root,
                                                    query,
                                                    false,
                                                    &mut matching_directories,
                                                );
                                            }

                                            let (has_results, folder_state_changed) =
                                                draw_folder_tree(
                                                ui,
                                                &snapshot.root,
                                                &mut status_line_update,
                                                search_query.as_deref(),
                                                false,
                                                search_query
                                                    .as_deref()
                                                    .map(|_| &matching_directories),
                                            );

                                            if folder_state_changed {
                                                self.invalidate_directory_tree_collapsed_cache(
                                                    project_id,
                                                );
                                            }

                                            if search_query.is_some() && !has_results {
                                                ui.label(
                                                    RichText::new("No matching files or folders")
                                                        .color(TEXT_MUTED),
                                                );
                                            }
                                        }
                                    }

                                    if let Some(status_line) = status_line_update {
                                        self.status_line = status_line;
                                    }
                                } else {
                                    ui.label(
                                        RichText::new("No project selected").color(TEXT_MUTED),
                                    );
                                }
                            });
                    }
                    LeftSidebarTab::SourceControl => {
                        let project_rows = self
                            .projects
                            .iter()
                            .map(|(project_id, project)| (*project_id, project.name.clone()))
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
                                let combo_width =
                                    (ui.available_width() - button_group_width).clamp(96.0, 150.0);
                                with_minimal_button_chrome(ui, |ui| {
                                    egui::ComboBox::from_id_salt("source-control-project-select")
                                        .selected_text(selected_project_label)
                                        .icon(paint_minimal_combo_icon)
                                        .width(combo_width)
                                        .show_ui(ui, |ui| {
                                            for (project_id, project_name) in &project_rows {
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
                                });
                            });
                        });
                        if self.selected_project != previous_selected_project {
                            should_persist_selection = true;
                        }
                        if should_persist_selection {
                            self.note_selection_changed();
                            self.persist_config();
                        }

                        let Some(project_id) = self.selected_project else {
                            ui.label(RichText::new("No project selected").color(TEXT_MUTED));
                            return;
                        };
                        let Some(project) = self.projects.get(&project_id).cloned() else {
                            ui.label(RichText::new("Project not found").color(TEXT_MUTED));
                            return;
                        };

                        if !self.source_control_state.contains_key(&project_id) {
                            self.request_source_control_refresh(project_id, false, false);
                        }

                        if refresh_status {
                            self.request_source_control_refresh(project_id, false, true);
                        }
                        if fetch_and_refresh {
                            self.request_source_control_refresh(project_id, true, true);
                        }
                        if open_project_folder {
                            match open_in_file_explorer(&project.path, false) {
                                Ok(()) => {
                                    self.status_line = "Opened project folder".to_owned();
                                }
                                Err(err) => {
                                    self.status_line = format!("Open folder failed: {err}");
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
                                    draw_sidebar_text_row(
                                        ui,
                                        RichText::new("Refreshing source control...")
                                            .color(TEXT_MUTED),
                                        TEXT_MUTED,
                                        "Refreshing source control...",
                                    );
                                }
                                if let Some(error) = &snapshot.last_error {
                                    draw_sidebar_text_row(
                                        ui,
                                        RichText::new(error).color(Color32::LIGHT_RED),
                                        Color32::LIGHT_RED,
                                        error,
                                    );
                                }
                                if let Some(branch_line) = source_control_branch_line(&snapshot) {
                                    let branch_line = format!("{} {}", icons::GIT_BRANCH, branch_line);
                                    draw_sidebar_text_row(
                                        ui,
                                        RichText::new(&branch_line).color(TEXT_MUTED),
                                        TEXT_MUTED,
                                        &branch_line,
                                    );
                                }

                                ui.separator();
                                if !source_control_snapshot_has_display_data(&snapshot)
                                    && snapshot.last_error.is_none()
                                    && !snapshot.loading
                                {
                                    draw_sidebar_text_row(
                                        ui,
                                        RichText::new("Status pending").color(TEXT_MUTED),
                                        TEXT_MUTED,
                                        "Status pending",
                                    );
                                } else if source_control_snapshot_has_display_data(&snapshot)
                                    && snapshot.files.is_empty()
                                    && snapshot.last_error.is_none()
                                    && !snapshot.loading
                                {
                                    draw_sidebar_text_row(
                                        ui,
                                        RichText::new("Working tree is clean").color(TEXT_MUTED),
                                        TEXT_MUTED,
                                        "Working tree is clean",
                                    );
                                }

                                for file in snapshot.files {
                                    let absolute = project.path.join(&file.path);
                                    let status_icon = if file.staged {
                                        icons::CHECK_CIRCLE
                                    } else {
                                        icons::CLOCK
                                    };
                                    let file_line = format!("{} {}", file.status, file.path);
                                    draw_source_control_file_row(ui, status_icon, &file_line)
                                        .context_menu(|ui| {
                                            with_minimal_button_chrome(ui, |ui| {
                                                if ui
                                                    .button(format!(
                                                        "{} Open in Folder",
                                                        icons::FOLDER_OPEN
                                                    ))
                                                    .clicked()
                                                {
                                                    match open_in_file_explorer(&absolute, true) {
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
        let mut selected_filter = self.config.ui.terminal_manager_filter;

        if draw_terminal_manager_filter_tabs(ui, &mut selected_filter) {
            self.config.ui.terminal_manager_filter = selected_filter;
            self.note_ui_config_changed();
            self.persist_config();
        }
        ui.add_space(8.0);

        let mut project_ids = self.projects.keys().copied().collect::<Vec<_>>();
        project_ids.sort_unstable();
        let visible_kind = self.config.ui.terminal_manager_filter.terminal_kind();

        for project_id in project_ids {
            let Some(project_snapshot) = self.projects.get(&project_id).cloned() else {
                continue;
            };

            let project_path = project_snapshot.path.display().to_string();
            let project_diff_summary =
                terminal_manager_diff_summary_model(self.source_control_state.get(&project_id));

            let header_id = ui.make_persistent_id(format!("project-group-{project_id}"));
            let mut header_state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                header_id,
                false,
            );
            let header_open = header_state.is_open();
            let visible_count = self.terminal_count_for_project_kind(project_id, visible_kind);
            let has_children = visible_count > 0;
            let (header_response, spawn_clicked, header_clicked) = draw_project_group_header(
                ui,
                &project_snapshot.name,
                header_open,
                has_children,
                visible_kind,
                &project_diff_summary,
            );
            let spawn_succeeded =
                spawn_clicked && self.spawn_terminal_for_project(ctx, project_id, visible_kind);
            if header_clicked && has_children {
                header_state.toggle(ui);
                header_state.store(ui.ctx());
            }
            if spawn_succeeded {
                header_state.set_open(true);
                header_state.store(ui.ctx());
            }
            if has_children {
                let _ = header_state.show_body_unindented(ui, |ui| {
                    ui.indent(Id::new(("terminal-manager-body", project_id)), |ui| {
                        ui.add_space(4.0);
                        self.draw_terminal_rows(ctx, ui, project_id, visible_kind);
                    });
                });
            }

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
        let ids = terminal_ids_for_project_kind(&self.terminals, project_id, kind);
        let saved_messages = self
            .projects
            .get(&project_id)
            .map(|project| project.saved_messages.clone())
            .unwrap_or_default();
        let current_active = self.active_terminal;
        let show_visibility_toggle = self.config.ui.multi_terminal_view_enabled;

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
                let section_gap = ui.spacing().item_spacing.x;
                let actions_width =
                    terminal_manager_actions_width(section_gap, show_visibility_toggle);
                let row_width = ui.available_width().max(0.0);
                let (row_label_width, row_actions_width) =
                    terminal_manager_row_widths(row_width, actions_width, section_gap);
                let row_height = ui.spacing().interact_size.y.max(CONTROL_ROW_HEIGHT);
                let (row_rect, row_response) =
                    ui.allocate_exact_size(egui::vec2(row_width, row_height), Sense::click());
                let hover_text = if terminal.exited {
                    format!("{} (Exited)", label)
                } else {
                    let base_text = label.clone();
                    if terminal.recent_inputs.is_empty() {
                        base_text
                    } else {
                        format!(
                            "{}\n\n{}",
                            base_text,
                            recent_inputs_tooltip_text(&terminal.recent_inputs)
                        )
                    }
                };
                let row_response = row_response.on_hover_text(hover_text);
                let row_chrome = terminal_manager_row_chrome(active, row_response.hovered());

                if ui.is_rect_visible(row_rect) {
                    let paint_rect = row_rect.shrink2(egui::vec2(1.0, 1.0));
                    if let Some(fill) = row_chrome.fill {
                        ui.painter().rect_filled(paint_rect, 8.0, fill);
                    }
                    if row_chrome.stroke != Stroke::NONE {
                        ui.painter().rect_stroke(paint_rect, 8.0, row_chrome.stroke);
                    }
                }

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
                                ui.add_space(SIDEBAR_ROW_LEADING_INSET);

                                let ai_badge = AiBadgeModel::from_session(&terminal.ai_session);
                                let ai_response = ai_badge_visual(ai_badge.status)
                                    .map(|_| draw_ai_badge(ui, &ai_badge));
                                if ai_response.is_some() {
                                    ui.add_space(4.0);
                                }

                                let text_color = if terminal.exited {
                                    with_alpha(TEXT_MUTED, 160)
                                } else {
                                    row_chrome.title_color
                                };
                                let title_font = egui::TextStyle::Body.resolve(ui.style());
                                let title_response = ui.add(
                                    egui::Label::new(RichText::new(&label).color(text_color))
                                        .truncate()
                                        .sense(Sense::click()),
                                );
                                let title_response = with_truncation_tooltip(
                                    ui,
                                    title_response,
                                    &label,
                                    &title_font,
                                    text_color,
                                    row_label_width,
                                );
                                // Add recent inputs tooltip
                                if !terminal.recent_inputs.is_empty() {
                                    title_response.clone().on_hover_text(
                                        recent_inputs_tooltip_text(&terminal.recent_inputs),
                                    );
                                }

                                match ai_response {
                                    Some(response) => response.union(title_response),
                                    None => title_response,
                                }
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

                        if show_visibility_toggle {
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
                self.set_active_terminal(ctx, Some(terminal_entry_id));
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
            .map(|terminal| {
                self.projects
                    .get(&terminal.project_id)
                    .map(|project| project.name.clone())
                    .unwrap_or_else(|| "Unknown Project".to_owned())
            })
            .unwrap_or_else(|| "Unknown Project".to_owned());
        let is_active = self.active_terminal == Some(terminal_id);

        let (clicked, close_requested, copied_selection, paste_requested, link_to_open) = {
            let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
                return;
            };

            let mut close_requested = false;
            let mut pane_clicked = false;
            let mut copied_selection = None;
            let mut paste_requested = false;
            let mut link_to_open = None;
            let header_chrome = terminal_header_chrome(is_active);
            let pane_width = pane_size.x.max(96.0);
            let pane_height = pane_size.y.max(124.0);
            let pane_right = force_terminal_pane_width(ui, pane_width);

            let header_size = Vec2::new(pane_width, TERMINAL_HEADER_HEIGHT);
            let _header_response = ui.allocate_ui_with_layout(
                header_size,
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_size(header_size);
                    egui::Frame::none()
                        .fill(header_chrome.fill)
                        .stroke(header_chrome.stroke)
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_min_height(TERMINAL_HEADER_HEIGHT - 12.0);

                            let ai_badge = AiBadgeModel::from_session(&terminal.ai_session);
                            draw_terminal_status_badges(ui, &ai_badge);

                            let title = terminal_display_label(&terminal.title, terminal.exited);
                            let title_font = egui::TextStyle::Body.resolve(ui.style());
                            let title_response = ui.add(
                                egui::Label::new(
                                    RichText::new(title)
                                        .color(header_chrome.title_color)
                                        .strong(),
                                )
                                .truncate()
                                .sense(Sense::click()),
                            );
                            let title_response = with_truncation_tooltip(
                                ui,
                                title_response,
                                &terminal.full_title,
                                &title_font,
                                TEXT_PRIMARY,
                                ui.available_width(),
                            );
                            if title_response.clicked() {
                                pane_clicked = true;
                            }

                            ui.add_space(8.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{}  {}", icons::FOLDER, project_name))
                                        .small()
                                        .color(header_chrome.detail_color),
                                )
                                .truncate(),
                            );
                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(terminal.kind.label())
                                        .small()
                                        .color(header_chrome.detail_color),
                                )
                                .truncate(),
                            );

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
                },
            );
            if !close_requested {
                ui.add_space(TERMINAL_HEADER_GAP);

                let font_id = terminal_font_id(ui.style());
                let char_width = terminal_char_width(ui, &font_id);
                let line_height = terminal_line_height(ui, &font_id);

                let output_height = (pane_height - TERMINAL_HEADER_HEIGHT - TERMINAL_HEADER_GAP)
                    .max(line_height * 2.0);
                let output_size = Vec2::new(pane_width, output_height);

                let (cols, lines) = terminal_grid_dimensions(output_size, char_width, line_height);
                if output_size.x >= char_width * 8.0 && output_size.y >= line_height * 3.0 {
                    let resize_applied = terminal.runtime.resize(TerminalDimensions {
                        cols,
                        lines,
                        pixel_width: output_size.x.round().clamp(1.0, u16::MAX as f32) as u16,
                        pixel_height: output_size.y.round().clamp(1.0, u16::MAX as f32) as u16,
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

                let viewport_size = terminal_output_viewport_size(output_size);
                let (viewport_rect, _) = ui.allocate_exact_size(viewport_size, Sense::hover());
                ui.painter()
                    .rect_filled(viewport_rect, 0.0, TERMINAL_OUTPUT_BG);

                let mut output_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(viewport_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                output_ui.set_clip_rect(viewport_rect);
                output_ui.set_min_size(viewport_size);

                egui::ScrollArea::vertical()
                    .id_salt(format!("term-output-{terminal_id}"))
                    .max_height(output_height)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(&mut output_ui, |ui| {
                        ui.set_width(output_size.x);
                        ui.set_min_width(output_size.x);
                        ui.set_min_height(output_size.y);
                        if terminal.render_cache.lines.is_empty() {
                            let mut layout_job = LayoutJob::default();
                            layout_job.wrap.max_width = f32::INFINITY;
                            layout_job.append(
                                "Terminal is resizing...",
                                0.0,
                                TextFormat {
                                    font_id: font_id.clone(),
                                    color: TEXT_MUTED,
                                    ..TextFormat::default()
                                },
                            );
                            let galley = ui.painter().layout_job(layout_job);
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
                            if response.secondary_clicked() && can_paste {
                                paste_requested = true;
                            }
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
                            let (primary_pressed, primary_down, primary_released) =
                                ui.ctx().input(|input| {
                                    (
                                        input.pointer.primary_pressed(),
                                        input.pointer.primary_down(),
                                        input.pointer.primary_released(),
                                    )
                                });
                            let link_activation_modifiers_active = ui
                                .ctx()
                                .input(|input| terminal_link_activation_modifiers(input.modifiers));
                            let primary_drag_started =
                                response.drag_started_by(egui::PointerButton::Primary);
                            let primary_dragged = response.dragged_by(egui::PointerButton::Primary);
                            let primary_clicked = response.clicked_by(egui::PointerButton::Primary);
                            let primary_pressed_on_terminal =
                                response.is_pointer_button_down_on() && primary_pressed;
                            let should_resolve_link = should_resolve_terminal_link(
                                link_activation_modifiers_active,
                                primary_pressed_on_terminal,
                                terminal.pending_link_click.is_some(),
                            );
                            let should_track_pointer =
                                primary_drag_started || primary_dragged || should_resolve_link;
                            if should_track_pointer || terminal.selection.is_some() {
                                Self::ensure_terminal_selection_snapshot(terminal);
                            }
                            let pointer_pos =
                                response.hover_pos().or(response.interact_pointer_pos());
                            let pointer_point = should_track_pointer
                                .then(|| {
                                    terminal.selection_snapshot.as_ref().and_then(
                                        |selection_snapshot| {
                                            terminal_selection_point_from_pointer(
                                                pointer_pos,
                                                rect.min,
                                                selection_snapshot,
                                                char_width,
                                                &galley,
                                            )
                                        },
                                    )
                                })
                                .flatten();
                            let link_under_pointer = should_resolve_link
                                .then(|| {
                                    pointer_point.and_then(|point| {
                                        terminal.selection_snapshot.as_ref().and_then(
                                            |selection_snapshot| {
                                                terminal_link_at_point(selection_snapshot, point)
                                            },
                                        )
                                    })
                                })
                                .flatten();
                            let link_activation_active =
                                link_activation_modifiers_active && link_under_pointer.is_some();
                            let response = if link_activation_active {
                                response.on_hover_cursor(egui::CursorIcon::PointingHand)
                            } else {
                                response
                            };
                            ui.painter().galley(rect.min, galley.clone(), TEXT_PRIMARY);
                            if response.is_pointer_button_down_on() && primary_down {
                                pane_clicked = true;
                            }
                            if primary_pressed_on_terminal {
                                if let Some(point) = pointer_point {
                                    Self::begin_terminal_primary_interaction(
                                        terminal,
                                        point,
                                        link_under_pointer.clone(),
                                    );
                                }
                            }
                            if primary_drag_started {
                                if let Some(point) = pointer_point {
                                    Self::update_terminal_primary_drag(terminal, point);
                                }
                            }
                            if primary_dragged {
                                if let Some(point) = pointer_point {
                                    Self::update_terminal_primary_drag(terminal, point);
                                }
                            }
                            if !primary_down {
                                terminal.selection_drag_active = false;
                            }
                            if response.drag_stopped_by(egui::PointerButton::Primary)
                                && terminal
                                    .selection
                                    .as_ref()
                                    .is_some_and(|selection| !selection.has_selection())
                            {
                                Self::clear_terminal_selection(terminal);
                            } else if response.drag_stopped_by(egui::PointerButton::Primary) {
                                terminal.selection_drag_active = false;
                            }
                            if primary_clicked {
                                pane_clicked = true;
                                link_to_open = Self::take_terminal_primary_clicked_link(
                                    terminal,
                                    link_under_pointer.as_deref(),
                                    link_activation_active,
                                );
                                if link_to_open.is_none() {
                                    Self::clear_terminal_selection(terminal);
                                }
                            } else if response.clicked() {
                                pane_clicked = true;
                                Self::clear_terminal_selection(terminal);
                            }
                            if primary_released {
                                Self::clear_pending_terminal_link_click(terminal);
                            }

                            let has_selection = terminal
                                .selection
                                .as_ref()
                                .is_some_and(TerminalSelection::has_selection);
                            if has_selection {
                                Self::ensure_terminal_selection_snapshot(terminal);
                            }
                            let can_copy = has_selection && terminal.selection_snapshot.is_some();
                            let can_paste = !terminal.exited;
                            let mut copy_requested = false;
                            let secondary_click_action = if response.secondary_clicked() {
                                pane_clicked = true;
                                terminal_secondary_click_action(has_selection, can_paste)
                            } else {
                                TerminalSecondaryClickAction::None
                            };
                            if matches!(
                                secondary_click_action,
                                TerminalSecondaryClickAction::PasteImmediately
                            ) {
                                paste_requested = true;
                            }
                            if has_selection {
                                response.context_menu(|ui| {
                                    with_minimal_button_chrome(ui, |ui| {
                                        ui.add_enabled_ui(can_copy, |ui| {
                                            if ui.button(format!("{} Copy", icons::COPY)).clicked()
                                            {
                                                copy_requested = true;
                                                ui.close_menu();
                                            }
                                        });
                                    });
                                });
                            }
                            if copy_requested {
                                copied_selection = Self::selected_terminal_text(terminal);
                                Self::clear_terminal_selection(terminal);
                            }
                            let empty_selection_snapshot = TerminalSelectionSnapshot::default();
                            let selection_snapshot = terminal
                                .selection_snapshot
                                .as_ref()
                                .unwrap_or(&empty_selection_snapshot);
                            paint_terminal_selection(
                                ui,
                                rect.min,
                                selection_snapshot,
                                terminal.selection.as_ref(),
                                char_width,
                                &galley,
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
                            let wheel_delta = ui.ctx().input(|input| input.smooth_scroll_delta);
                            if wheel_delta != Vec2::ZERO {
                                if let Some(pointer_pos) =
                                    ui.ctx().input(|input| input.pointer.interact_pos())
                                {
                                    if rect.contains(pointer_pos)
                                        && terminal.runtime.is_mouse_reporting_active()
                                    {
                                        let direction = if wheel_delta.y > 0.0 {
                                            WheelDirection::Up
                                        } else if wheel_delta.y < 0.0 {
                                            WheelDirection::Down
                                        } else if wheel_delta.x > 0.0 {
                                            WheelDirection::Right
                                        } else {
                                            WheelDirection::Left
                                        };
                                        let cell_x = ((pointer_pos.x - rect.min.x) / char_width)
                                            .floor()
                                            as usize;
                                        let cell_y = ((pointer_pos.y - rect.min.y) / line_height)
                                            .floor()
                                            as usize;
                                        terminal.runtime.send_mouse_wheel(TerminalWheelEvent {
                                            direction,
                                            x: cell_x,
                                            y: cell_y,
                                            x_pixel_offset: 0,
                                            y_pixel_offset: 0,
                                        });
                                        ui.ctx().input_mut(|input| {
                                            input.smooth_scroll_delta = Vec2::ZERO;
                                        });
                                    }
                                }
                            }
                        }
                    });
                ui.expand_to_include_x(pane_right);
            }

            (
                pane_clicked,
                close_requested,
                copied_selection,
                paste_requested,
                link_to_open,
            )
        };

        if close_requested {
            self.close_terminal(ui.ctx(), terminal_id);
            return;
        }

        if clicked {
            self.surrender_ui_text_focus(ui.ctx());
        }

        if clicked || copied_selection.is_some() || paste_requested {
            self.set_active_terminal(ui.ctx(), Some(terminal_id));
        }

        if let Some(text) = copied_selection {
            ui.ctx().copy_text(text);
            self.finalize_pointer_selection_copy(ui.ctx());
        }

        if paste_requested {
            self.paste_clipboard_to_terminal(terminal_id);
        }

        if let Some(url) = link_to_open {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
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

        let mut open = self.show_settings_popup;
        if !open {
            return;
        }

        egui::Window::new(format!("{} Settings", icons::GEAR))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .movable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(380.0)
            .max_height((ctx.screen_rect().height() - 80.0).max(360.0))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Application Settings")
                        .strong()
                        .size(16.0)
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                let previous_shell = self.config.default_shell;
                egui::ComboBox::from_label("Default Shell")
                    .selected_text(self.config.default_shell.label())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for shell in ShellKind::available_for_current_platform() {
                            ui.selectable_value(
                                &mut self.config.default_shell,
                                *shell,
                                shell.label(),
                            );
                        }
                    });
                if self.config.default_shell != previous_shell {
                    should_persist = true;
                    default_shell_changed = true;
                }

                let previous_multi_terminal_view_enabled =
                    self.config.ui.multi_terminal_view_enabled;
                let multi_terminal_toggle = ui.checkbox(
                    &mut self.config.ui.multi_terminal_view_enabled,
                    "Show multiple terminals at once",
                );
                multi_terminal_toggle.on_hover_text(
                    "When disabled, only the active terminal stays visible in the main area.",
                );
                if self.config.ui.multi_terminal_view_enabled != previous_multi_terminal_view_enabled
                {
                    should_persist = true;
                    ui_config_changed = true;
                    self.bump_layout_epoch();
                    if !self.config.ui.multi_terminal_view_enabled {
                        self.set_active_terminal(ctx, self.single_terminal_id_for_main());
                    } else {
                        ctx.request_repaint();
                    }
                }

                ui.separator();
                self.draw_settings_diagnostics_section(ctx, ui);

                ui.separator();
                ui.label(
                    RichText::new(format!("{} Saved Messages", icons::CHAT_TEXT))
                        .strong()
                        .size(15.0)
                        .color(TEXT_PRIMARY),
                );

                let mut project_ids = self.projects.keys().copied().collect::<Vec<_>>();
                project_ids.sort_unstable();

                egui::ScrollArea::vertical()
                    .id_salt("settings-saved-messages-scroll")
                    .max_height(ui.available_height().max(180.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if project_ids.is_empty() {
                            ui.label(
                                RichText::new("Add a project to manage saved messages.")
                                    .color(TEXT_MUTED),
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

                                for (index, message) in project_snapshot.saved_messages.iter().enumerate()
                                {
                                    ui.horizontal(|ui| {
                                        let message_label = ui.add(
                                            egui::Label::new(
                                                RichText::new(message).monospace().small(),
                                            )
                                            .truncate(),
                                        );
                                        let _ = with_truncation_tooltip(
                                            ui,
                                            message_label,
                                            message,
                                            &egui::TextStyle::Monospace.resolve(ui.style()),
                                            TEXT_PRIMARY,
                                            ui.available_width(),
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

            });

        self.show_settings_popup = open;

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
        let global_modifiers = raw_input.modifiers;
        let single_view_shortcuts_enabled = !self.config.ui.multi_terminal_view_enabled;

        let capture_keyboard = self.should_capture_terminal_keyboard(ctx);

        // Only handle Alt+M shortcut when terminal is NOT capturing keyboard
        if !capture_keyboard {
            let (alt_m_events, remaining_events) =
                Self::partition_alt_m_shortcut(events, global_modifiers);
            if !alt_m_events.is_empty() {
                self.toggle_main_visibility_mode();
                raw_input.events = remaining_events;
                return;
            }

            if self.should_steal_attention_terminal_input(ctx, &remaining_events) {
                self.surrender_ui_text_focus(ctx);
                self.allow_attention_terminal_input_routing_once = true;
                let (terminal_events, remaining_events) = Self::partition_terminal_input_events(
                    remaining_events,
                    single_view_shortcuts_enabled,
                );
                let (navigation_shortcuts, remaining_events) =
                    Self::partition_terminal_navigation_shortcuts(
                        remaining_events,
                        single_view_shortcuts_enabled,
                    );
                self.buffered_terminal_input.extend(terminal_events);
                self.buffered_terminal_navigation
                    .extend(navigation_shortcuts);
                raw_input.events = remaining_events;
                return;
            }

            // Terminal capture not active — fall through to normal UI event processing
            let (_, remaining_events) =
                Self::partition_blocked_ui_reverse_focus_traversal_events(remaining_events);
            raw_input.events = remaining_events;
            return;
        }

        // Terminal is capturing keyboard — let Alt+M through to terminal
        let (terminal_events, remaining_events) =
            Self::partition_terminal_input_events(events, single_view_shortcuts_enabled);
        let (navigation_shortcuts, remaining_events) =
            Self::partition_terminal_navigation_shortcuts(
                remaining_events,
                single_view_shortcuts_enabled,
            );
        self.buffered_terminal_input.extend(terminal_events);
        self.buffered_terminal_navigation
            .extend(navigation_shortcuts);
        raw_input.events = remaining_events;
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_theme_initialized(ctx);
        self.apply_initial_window_bounds(ctx);
        self.process_terminal_events(ctx);
        self.poll_factory_droid_hook_inboxes(ctx);
        self.poll_factory_droid_processes(ctx);
        self.poll_codex_notify_inboxes(ctx);
        self.poll_codex_processes(ctx);
        self.process_source_control_events(ctx);
        self.process_directory_index_events(ctx);
        self.schedule_source_control_refresh(ctx);
        self.schedule_terminal_refresh(ctx);
        let mut terminal_events = self.take_buffered_terminal_input();
        terminal_events.extend(self.capture_active_terminal_input(ctx));
        terminal_events = self.preprocess_terminal_input_with_held_repeat(ctx, terminal_events);
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
        self.flush_pending_terminal_pastes(ctx);
        self.draw_copy_toast(ctx);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for terminal in self.terminals.values() {
            let _ = terminal.runtime.terminate();
        }
        let terminal_ids = self.terminals.keys().copied().collect::<Vec<_>>();
        for terminal_id in terminal_ids {
            self.reset_factory_droid_hook_inbox(terminal_id);
            self.reset_codex_notify_inbox(terminal_id);
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

    if pending_config_changes.ui {
        config.ui.project_explorer_expanded = current_config.ui.project_explorer_expanded;
        config.ui.terminal_manager_expanded = current_config.ui.terminal_manager_expanded;
        config.ui.multi_terminal_view_enabled = current_config.ui.multi_terminal_view_enabled;
        config.ui.terminal_manager_filter = current_config.ui.terminal_manager_filter;
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

fn spawn_source_control_worker(rx: Receiver<SourceControlCommand>, tx: Sender<SourceControlEvent>) {
    std::thread::spawn(move || {
        while let Ok(command) = rx.recv() {
            let snapshot =
                collect_source_control_snapshot(&command.project_path, command.run_fetch);
            if tx
                .send(SourceControlEvent {
                    project_id: command.project_id,
                    snapshot,
                })
                .is_err()
            {
                break;
            }
        }
    });
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

    if let Some((added_lines, removed_lines)) = collect_source_control_line_totals(project_path) {
        snapshot.added_lines = Some(added_lines);
        snapshot.removed_lines = Some(removed_lines);
    }

    snapshot
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextLineEncoding {
    Bytes,
    Utf16Le,
    Utf16Be,
}

fn collect_source_control_line_totals(project_path: &Path) -> Option<(usize, usize)> {
    let diff_base = resolve_git_tracked_diff_base(project_path).ok()?;
    let (tracked_added, tracked_removed) =
        run_git_numstat_totals(project_path, &["diff", diff_base.as_str(), "--numstat"]).ok()?;
    let untracked_added = count_untracked_text_file_lines(project_path).ok()?;

    Some((
        tracked_added.saturating_add(untracked_added),
        tracked_removed,
    ))
}

fn resolve_git_tracked_diff_base(project_path: &Path) -> std::io::Result<String> {
    let output = run_git_command(project_path, &["rev-parse", "--verify", "HEAD"])?;
    if output.status.success() {
        Ok("HEAD".to_owned())
    } else {
        git_empty_tree_oid(project_path)
    }
}

fn git_empty_tree_oid(project_path: &Path) -> std::io::Result<String> {
    let args = ["hash-object", "-t", "tree", "--stdin"];
    let output = run_git_command_with_input(project_path, &args, Some(&[]))?;
    if !output.status.success() {
        return Err(git_command_status_error(&args, &output.stderr));
    }

    let oid = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if oid.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "git hash-object returned an empty tree id",
        ));
    }

    Ok(oid)
}

fn run_git_numstat_totals(project_path: &Path, args: &[&str]) -> std::io::Result<(usize, usize)> {
    let output = run_git_command(project_path, args)?;
    if !output.status.success() {
        return Err(git_command_status_error(args, &output.stderr));
    }

    Ok(parse_git_numstat_totals(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_git_numstat_totals(stdout: &str) -> (usize, usize) {
    let mut added_lines = 0usize;
    let mut removed_lines = 0usize;

    for line in stdout.lines() {
        let mut columns = line.splitn(3, '\t');
        let Some(added_text) = columns.next() else {
            continue;
        };
        let Some(removed_text) = columns.next() else {
            continue;
        };

        let Ok(added) = added_text.parse::<usize>() else {
            continue;
        };
        let Ok(removed) = removed_text.parse::<usize>() else {
            continue;
        };

        added_lines = added_lines.saturating_add(added);
        removed_lines = removed_lines.saturating_add(removed);
    }

    (added_lines, removed_lines)
}

fn count_untracked_text_file_lines(project_path: &Path) -> std::io::Result<usize> {
    let args = ["ls-files", "--others", "--exclude-standard", "-z"];
    let output = run_git_command(project_path, &args)?;
    if !output.status.success() {
        return Err(git_command_status_error(&args, &output.stderr));
    }

    Ok(parse_git_path_list(&output.stdout)
        .into_iter()
        .filter_map(|path| count_text_file_lines(&project_path.join(path)))
        .fold(0usize, usize::saturating_add))
}

fn parse_git_path_list(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
        .collect()
}

fn count_text_file_lines(path: &Path) -> Option<usize> {
    if !path.is_file() {
        return None;
    }

    let bytes = fs::read(path).ok()?;
    count_text_line_bytes(&bytes)
}

fn count_text_line_bytes(bytes: &[u8]) -> Option<usize> {
    let (encoding, offset) = detect_text_line_encoding(bytes)?;
    let data = &bytes[offset..];

    match encoding {
        TextLineEncoding::Bytes => Some(count_byte_lines(data, b'\n')),
        TextLineEncoding::Utf16Le => count_utf16_lines(data, true),
        TextLineEncoding::Utf16Be => count_utf16_lines(data, false),
    }
}

fn detect_text_line_encoding(bytes: &[u8]) -> Option<(TextLineEncoding, usize)> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Some((TextLineEncoding::Utf16Le, 2));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Some((TextLineEncoding::Utf16Be, 2));
    }
    if bytes.contains(&0) {
        return detect_bomless_utf16_encoding(bytes).map(|encoding| (encoding, 0));
    }

    Some((TextLineEncoding::Bytes, 0))
}

fn detect_bomless_utf16_encoding(bytes: &[u8]) -> Option<TextLineEncoding> {
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return None;
    }

    let mut little_endian_pairs = 0usize;
    let mut big_endian_pairs = 0usize;
    for chunk in bytes.chunks_exact(2) {
        match (chunk[0] == 0, chunk[1] == 0) {
            (false, true) => {
                little_endian_pairs = little_endian_pairs.saturating_add(1);
            }
            (true, false) => {
                big_endian_pairs = big_endian_pairs.saturating_add(1);
            }
            _ => return None,
        }
    }

    if little_endian_pairs > 0 && big_endian_pairs == 0 {
        Some(TextLineEncoding::Utf16Le)
    } else if big_endian_pairs > 0 && little_endian_pairs == 0 {
        Some(TextLineEncoding::Utf16Be)
    } else {
        None
    }
}

fn count_byte_lines(bytes: &[u8], newline_byte: u8) -> usize {
    if bytes.is_empty() {
        return 0;
    }

    let line_breaks = bytes.iter().filter(|byte| **byte == newline_byte).count();
    if bytes.last() == Some(&newline_byte) {
        line_breaks
    } else {
        line_breaks.saturating_add(1)
    }
}

fn count_utf16_lines(bytes: &[u8], little_endian: bool) -> Option<usize> {
    if bytes.is_empty() {
        return Some(0);
    }
    if bytes.len() % 2 != 0 {
        return None;
    }

    let mut line_breaks = 0usize;
    let mut last_code_unit = None;
    for chunk in bytes.chunks_exact(2) {
        let code_unit = if little_endian {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        };
        if code_unit == 0x000A {
            line_breaks = line_breaks.saturating_add(1);
        }
        last_code_unit = Some(code_unit);
    }

    if last_code_unit == Some(0x000A) {
        Some(line_breaks)
    } else {
        Some(line_breaks.saturating_add(1))
    }
}

fn git_command_status_error(args: &[&str], stderr: &[u8]) -> std::io::Error {
    let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
    let message = if stderr.is_empty() {
        format!("git {} failed", args.join(" "))
    } else {
        stderr
    };
    std::io::Error::new(std::io::ErrorKind::Other, message)
}

fn run_git_command(project_path: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    run_git_command_with_input(project_path, args, None)
}

fn run_git_command_with_input(
    project_path: &Path,
    args: &[&str],
    stdin_bytes: Option<&[u8]>,
) -> std::io::Result<std::process::Output> {
    let mut command = Command::new("git");
    command.arg("-C").arg(project_path).args(args);
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    if let Some(stdin_bytes) = stdin_bytes {
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_bytes)?;
        }
        child.wait_with_output()
    } else {
        command.output()
    }
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

    let snapshot_error = match read_directory_children(project_path) {
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
    }
}

fn read_directory_children(path: &Path) -> Result<Vec<DirectoryNode>, String> {
    let entries = fs::read_dir(path).map_err(|err| err.to_string())?;
    let mut children_paths = entries
        .filter_map(|entry| entry.ok().map(|dir_entry| dir_entry.path()))
        .collect::<Vec<PathBuf>>();
    children_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    let mut children = Vec::new();
    for child_path in children_paths {
        if let Some(node) = build_directory_node(&child_path) {
            children.push(node);
        }
    }

    Ok(children)
}

fn build_directory_node(path: &Path) -> Option<DirectoryNode> {
    let name = path
        .file_name()
        .map(|segment| segment.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let file_type = fs::symlink_metadata(path).ok()?.file_type();
    let is_symlink = file_type.is_symlink();
    let is_dir = path.is_dir();

    let mut node = DirectoryNode {
        name,
        path: path.to_path_buf(),
        is_dir,
        children: Vec::new(),
    };

    if should_descend_into_directory(is_dir, is_symlink) {
        if let Ok(children) = read_directory_children(path) {
            node.children = children;
        }
    }

    Some(node)
}

fn should_descend_into_directory(is_dir: bool, is_symlink: bool) -> bool {
    is_dir && !is_symlink
}

fn directory_index_loading_label(time_secs: f64) -> String {
    let phase = ((time_secs / DIRECTORY_INDEX_LOADING_ANIMATION_STEP_SECS).floor() as usize) % 4;
    format!("Indexing files{}", ".".repeat(phase))
}

fn open_in_file_explorer(path: &Path, select_file: bool) -> Result<(), String> {
    let (program, args) = file_explorer_command(path, select_file);
    let mut command = Command::new(program);
    command.args(args);

    match command.spawn() {
        Ok(_) => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn file_explorer_command(path: &Path, select_file: bool) -> (&'static str, Vec<OsString>) {
    #[cfg(target_os = "windows")]
    {
        let args = if select_file {
            vec![OsString::from("/select,"), path.as_os_str().to_owned()]
        } else {
            vec![path.as_os_str().to_owned()]
        };
        ("explorer.exe", args)
    }

    #[cfg(target_os = "macos")]
    {
        let mut args = Vec::new();
        if select_file {
            args.push(OsString::from("-R"));
        }
        args.push(path.as_os_str().to_owned());
        ("open", args)
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let target = if select_file {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        ("xdg-open", vec![target.as_os_str().to_owned()])
    }
}

fn default_app_open_command(path: &Path) -> (&'static str, Vec<OsString>) {
    #[cfg(target_os = "windows")]
    {
        (
            "cmd",
            vec![
                OsString::from("/C"),
                OsString::from("start"),
                OsString::from(""),
                path.as_os_str().to_owned(),
            ],
        )
    }

    #[cfg(target_os = "macos")]
    {
        ("open", vec![path.as_os_str().to_owned()])
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        ("xdg-open", vec![path.as_os_str().to_owned()])
    }
}

fn open_path_with_default_app(path: &Path) -> Result<(), String> {
    let (program, args) = default_app_open_command(path);
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

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
) -> (bool, bool) {
    let mut rendered_any = false;
    let mut folder_state_changed = false;
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
            let mut header_state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                directory_tree_folder_state_id(&item.path),
                false,
            );
            let search_active = search_query.is_some();
            let initial_open_state = (!search_active).then(|| header_state.is_open());
            let previous_open_state = search_active.then(|| header_state.is_open());
            if search_active {
                header_state.set_open(true);
            }

            let (_, header_response, _) = header_state
                .show_header(ui, |ui| draw_directory_folder_row(ui, &item.name))
                .body(|ui| {
                    let (_, child_state_changed) = draw_folder_tree(
                        ui,
                        item,
                        status_line_update,
                        search_query,
                        show_all_descendants,
                        matching_directories,
                    );
                    folder_state_changed |= child_state_changed;
                });
            if search_active {
                if let Some(previous_open_state) = previous_open_state {
                    let mut restore_state =
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            directory_tree_folder_state_id(&item.path),
                            false,
                        );
                    restore_state.set_open(previous_open_state);
                    restore_state.store(ui.ctx());
                }
            }

            if !search_active && header_response.inner.clicked() {
                let mut clicked_state =
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        directory_tree_folder_state_id(&item.path),
                        false,
                    );
                clicked_state.toggle(ui);
                clicked_state.store(ui.ctx());
            }

            if let Some(initial_open_state) = initial_open_state {
                let current_open_state =
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        directory_tree_folder_state_id(&item.path),
                        false,
                    )
                    .is_open();
                if current_open_state != initial_open_state {
                    folder_state_changed = true;
                }
            }
            header_response.response.context_menu(|ui| {
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

            let response = draw_directory_file_row(ui, &item.name);
            let double_clicked = response.double_clicked();
            response.context_menu(|ui| {
                with_minimal_button_chrome(ui, |ui| {
                    if ui.button("Open").clicked() {
                        match open_path_with_default_app(&item.path) {
                            Ok(()) => {
                                *status_line_update = Some(format!("Opened file: {}", item.name));
                            }
                            Err(err) => {
                                *status_line_update = Some(format!("Open file failed: {err}"));
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .button(format!("{} Reveal in Folder", icons::FOLDER_OPEN))
                        .clicked()
                    {
                        match open_in_file_explorer(&item.path, true) {
                            Ok(()) => {
                                *status_line_update =
                                    Some(format!("Revealed file in folder: {}", item.name));
                            }
                            Err(err) => {
                                *status_line_update = Some(format!("Open folder failed: {err}"));
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                        let item_path_text = item.path.display().to_string();
                        ui.ctx().copy_text(item_path_text.clone());
                        *status_line_update = Some(format!("Copied path: {}", item_path_text));
                        ui.close_menu();
                    }
                });
            });

            if double_clicked {
                match open_path_with_default_app(&item.path) {
                    Ok(()) => {
                        *status_line_update = Some(format!("Opened file: {}", item.name));
                    }
                    Err(err) => {
                        *status_line_update = Some(format!("Open file failed: {err}"));
                    }
                }
            }
        }
    }

    (rendered_any, folder_state_changed)
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

fn apply_directory_tree_open_state(ctx: &egui::Context, root: &DirectoryNode, open: bool) {
    for item in &root.children {
        if !item.is_dir {
            continue;
        }

        let header_id = directory_tree_folder_state_id(&item.path);
        let mut header_state =
            egui::collapsing_header::CollapsingState::load_with_default_open(ctx, header_id, open);
        header_state.set_open(open);
        header_state.store(ctx);

        apply_directory_tree_open_state(ctx, item, open);
    }
}

fn directory_tree_has_collapsed_folders(ctx: &egui::Context, root: &DirectoryNode) -> bool {
    for item in &root.children {
        if !item.is_dir {
            continue;
        }

        let header_state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ctx,
            directory_tree_folder_state_id(&item.path),
            false,
        );
        if !header_state.is_open() || directory_tree_has_collapsed_folders(ctx, item) {
            return true;
        }
    }

    false
}

fn directory_tree_folder_state_id(path: &Path) -> Id {
    Id::new(("directory-tree-folder", path.display().to_string()))
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

fn next_terminal_in_linear_direction(
    active_terminal: Option<u64>,
    terminal_ids: &[u64],
    is_selectable: impl Fn(u64) -> bool,
    direction: TerminalNavigationDirection,
) -> Option<u64> {
    let active_terminal = active_terminal?;
    let active_index = terminal_ids
        .iter()
        .position(|terminal_id| *terminal_id == active_terminal)?;

    match direction {
        TerminalNavigationDirection::Up => terminal_ids[..active_index]
            .iter()
            .rev()
            .copied()
            .find(|terminal_id| is_selectable(*terminal_id)),
        TerminalNavigationDirection::Down => terminal_ids[active_index + 1..]
            .iter()
            .copied()
            .find(|terminal_id| is_selectable(*terminal_id)),
        _ => None,
    }
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

fn terminal_manager_actions_width(section_gap: f32, show_visibility_toggle: bool) -> f32 {
    let visibility_width = if show_visibility_toggle {
        CONTROL_ROW_HEIGHT + section_gap
    } else {
        0.0
    };
    CONTROL_ROW_HEIGHT + TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH + section_gap + visibility_width
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

fn sidebar_row_content_rect(rect: egui::Rect, button_padding: Vec2) -> egui::Rect {
    let mut content_rect = rect.shrink2(button_padding);
    content_rect.min.x = (content_rect.min.x + SIDEBAR_ROW_LEADING_INSET).min(content_rect.max.x);
    content_rect
}

fn sidebar_row_wrap_width(available_width: f32, button_padding: Vec2) -> f32 {
    (available_width - (button_padding.x * 2.0) - SIDEBAR_ROW_LEADING_INSET).max(0.0)
}

fn sidebar_row_desired_height(ui: &Ui, content_height: f32, button_padding: Vec2) -> f32 {
    ui.spacing()
        .interact_size
        .y
        .max(content_height + (button_padding.y * 2.0))
}

fn directory_row_text_position(
    rect: egui::Rect,
    button_padding: Vec2,
    galley_size: Vec2,
) -> egui::Pos2 {
    let content_rect = sidebar_row_content_rect(rect, button_padding);
    egui::pos2(
        content_rect.min.x,
        content_rect.center().y - (galley_size.y * 0.5),
    )
}

fn draw_directory_file_row(ui: &mut Ui, text: &str) -> egui::Response {
    let button_padding = ui.spacing().button_padding;
    let available_width = ui.available_width().max(0.0);
    let wrap_width = sidebar_row_wrap_width(available_width, button_padding);
    let galley = WidgetText::from(text.to_owned()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        wrap_width,
        egui::TextStyle::Body,
    );
    let desired_height = sidebar_row_desired_height(ui, galley.size().y, button_padding);
    let desired_size = egui::vec2(available_width, desired_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if ui.is_rect_visible(rect) {
        if let Some(fill) = directory_file_row_hover_fill(
            response.hovered() || response.highlighted() || response.has_focus(),
        ) {
            ui.painter()
                .rect_filled(rect.shrink2(egui::vec2(1.0, 1.0)), 8.0, fill);
        }

        let text_pos = directory_row_text_position(rect, button_padding, galley.size());
        ui.painter()
            .galley(text_pos, galley, ui.visuals().text_color());
    }

    let font_id = egui::TextStyle::Body.resolve(ui.style());
    with_truncation_tooltip(
        ui,
        response,
        text,
        &font_id,
        ui.visuals().text_color(),
        wrap_width,
    )
}

fn draw_directory_folder_row(ui: &mut Ui, text: &str) -> egui::Response {
    let button_padding = ui.spacing().button_padding;
    let available_width = ui.available_width().max(0.0);
    let wrap_width = sidebar_row_wrap_width(available_width, button_padding);
    let galley = WidgetText::from(text.to_owned()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        wrap_width,
        egui::TextStyle::Body,
    );
    let desired_height = sidebar_row_desired_height(ui, galley.size().y, button_padding);
    let desired_size = egui::vec2(available_width, desired_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if ui.is_rect_visible(rect) {
        let text_pos = directory_row_text_position(rect, button_padding, galley.size());
        ui.painter()
            .galley(text_pos, galley, ui.visuals().text_color());
    }

    let font_id = egui::TextStyle::Body.resolve(ui.style());
    with_truncation_tooltip(
        ui,
        response,
        text,
        &font_id,
        ui.visuals().text_color(),
        wrap_width,
    )
}

fn draw_sidebar_text_row<T>(
    ui: &mut Ui,
    text: T,
    fallback_color: Color32,
    tooltip: &str,
) -> egui::Response
where
    T: Into<WidgetText>,
{
    let button_padding = ui.spacing().button_padding;
    let available_width = ui.available_width().max(0.0);
    let wrap_width = sidebar_row_wrap_width(available_width, button_padding);
    let galley = text.into().into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        wrap_width,
        egui::TextStyle::Body,
    );
    let desired_height = sidebar_row_desired_height(ui, galley.size().y, button_padding);
    let desired_size = egui::vec2(available_width, desired_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let text_pos = directory_row_text_position(rect, button_padding, galley.size());
        ui.painter().galley(text_pos, galley, fallback_color);
    }

    let font_id = egui::TextStyle::Body.resolve(ui.style());
    with_truncation_tooltip(ui, response, tooltip, &font_id, fallback_color, wrap_width)
}

fn draw_source_control_file_row(ui: &mut Ui, status_icon: AppIcon, text: &str) -> egui::Response {
    let button_padding = ui.spacing().button_padding;
    let available_width = ui.available_width().max(0.0);
    let wrap_width = (sidebar_row_wrap_width(available_width, button_padding)
        - SOURCE_CONTROL_FILE_ICON_WIDTH
        - SOURCE_CONTROL_FILE_ICON_GAP)
        .max(0.0);
    let galley = WidgetText::from(RichText::new(text).monospace().small()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        wrap_width,
        egui::TextStyle::Small,
    );
    let desired_height = sidebar_row_desired_height(ui, galley.size().y, button_padding);
    let desired_size = egui::vec2(available_width, desired_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if ui.is_rect_visible(rect) {
        if let Some(fill) = directory_file_row_hover_fill(
            response.hovered() || response.highlighted() || response.has_focus(),
        ) {
            ui.painter()
                .rect_filled(rect.shrink2(egui::vec2(1.0, 1.0)), 8.0, fill);
        }

        let content_rect = sidebar_row_content_rect(rect, button_padding);
        let icon_rect = egui::Rect::from_min_size(
            content_rect.min,
            egui::vec2(
                SOURCE_CONTROL_FILE_ICON_WIDTH.min(content_rect.width()),
                content_rect.height(),
            ),
        );
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            status_icon.to_string(),
            egui::FontId::proportional(12.0),
            TEXT_MUTED,
        );

        let text_rect = egui::Rect::from_min_max(
            egui::pos2(
                (icon_rect.max.x + SOURCE_CONTROL_FILE_ICON_GAP).min(content_rect.max.x),
                content_rect.min.y,
            ),
            content_rect.max,
        );
        let text_pos = egui::pos2(
            text_rect.min.x,
            content_rect.center().y - (galley.size().y * 0.5),
        );
        ui.painter()
            .galley(text_pos, galley, ui.visuals().text_color());
    }

    let font_id = egui::TextStyle::Small.resolve(ui.style());
    with_truncation_tooltip(
        ui,
        response,
        text,
        &font_id,
        ui.visuals().text_color(),
        wrap_width,
    )
}

fn directory_file_row_hover_fill(is_hovered: bool) -> Option<Color32> {
    is_hovered.then(|| with_alpha(BTN_ICON_HOVER, 110))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerminalHeaderChrome {
    fill: Color32,
    stroke: Stroke,
    title_color: Color32,
    detail_color: Color32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerminalManagerRowChrome {
    fill: Option<Color32>,
    stroke: Stroke,
    title_color: Color32,
}

fn terminal_header_chrome(is_active: bool) -> TerminalHeaderChrome {
    if is_active {
        TerminalHeaderChrome {
            fill: Color32::from_rgb(28, 52, 72),
            stroke: Stroke::NONE,
            title_color: Color32::from_rgb(244, 251, 255),
            detail_color: with_alpha(TEXT_MUTED, 238),
        }
    } else {
        TerminalHeaderChrome {
            fill: Color32::from_rgb(22, 32, 46),
            stroke: Stroke::new(1.0, BORDER_COLOR),
            title_color: TEXT_PRIMARY,
            detail_color: with_alpha(TEXT_MUTED, 230),
        }
    }
}

fn terminal_manager_row_chrome(is_active: bool, is_hovered: bool) -> TerminalManagerRowChrome {
    if is_active {
        TerminalManagerRowChrome {
            fill: Some(Color32::from_rgb(24, 48, 68)),
            stroke: Stroke::new(1.0, with_alpha(ACCENT, 220)),
            title_color: Color32::from_rgb(244, 251, 255),
        }
    } else if is_hovered {
        TerminalManagerRowChrome {
            fill: Some(with_alpha(SURFACE_BG_SOFT, 180)),
            stroke: Stroke::new(1.0, with_alpha(BORDER_COLOR, 210)),
            title_color: with_alpha(TEXT_PRIMARY, 230),
        }
    } else {
        TerminalManagerRowChrome {
            fill: None,
            stroke: Stroke::NONE,
            title_color: with_alpha(TEXT_PRIMARY, 210),
        }
    }
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

const RECENT_INPUTS_HOVER_MAX_CHARS: usize = 100;

fn recent_inputs_tooltip_text(recent_inputs: &VecDeque<String>) -> String {
    if recent_inputs.is_empty() {
        return String::new();
    }

    let mut lines = Vec::with_capacity(recent_inputs.len() + 2);

    lines.push("─ Recent Inputs ─".to_owned());
    for (i, input) in recent_inputs.iter().enumerate() {
        let truncated = capped_hover_text(input, RECENT_INPUTS_HOVER_MAX_CHARS);
        lines.push(format!("{}: {}", i + 1, truncated));
    }

    lines.join("\n")
}

fn with_truncation_tooltip(
    ui: &Ui,
    response: egui::Response,
    text: &str,
    _font_id: &FontId,
    _color: Color32,
    available_width: f32,
) -> egui::Response {
    if text.trim().is_empty() {
        return response;
    }

    let galley = WidgetText::from(text.to_owned()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        available_width,
        egui::TextStyle::Body,
    );

    if galley.size().x > available_width {
        response.on_hover_text(capped_hover_text(text, DIRECTORY_ENTRY_TOOLTIP_MAX_CHARS))
    } else {
        response
    }
}

#[allow(dead_code)]
fn source_control_badge_color(state: SourceControlBadgeState) -> Color32 {
    match state {
        SourceControlBadgeState::Clean => Color32::from_rgb(94, 196, 130),
        SourceControlBadgeState::Error => Color32::from_rgb(224, 92, 92),
    }
}

fn source_control_snapshot_has_display_data(snapshot: &SourceControlSnapshot) -> bool {
    !snapshot.branch.is_empty()
        || snapshot.ahead > 0
        || snapshot.behind > 0
        || !snapshot.files.is_empty()
        || snapshot.added_lines.is_some()
        || snapshot.removed_lines.is_some()
}

fn source_control_branch_line(snapshot: &SourceControlSnapshot) -> Option<String> {
    if snapshot.branch.is_empty() {
        return None;
    }

    let mut branch_line = snapshot.branch.clone();
    if snapshot.ahead > 0 || snapshot.behind > 0 {
        branch_line.push_str(&format!(
            "  ahead:{} behind:{}",
            snapshot.ahead, snapshot.behind
        ));
    }
    Some(branch_line)
}

fn merge_source_control_refresh_result(
    current: Option<&SourceControlSnapshot>,
    incoming: SourceControlSnapshot,
) -> SourceControlSnapshot {
    if incoming.last_error.is_none() {
        return incoming;
    }

    match current {
        Some(current) if source_control_snapshot_has_display_data(current) => {
            let mut merged = current.clone();
            merged.loading = incoming.loading;
            merged.last_error = incoming.last_error;
            merged
        }
        _ => incoming,
    }
}

fn terminal_manager_diff_summary_model(
    snapshot: Option<&SourceControlSnapshot>,
) -> TerminalManagerDiffSummaryModel {
    match snapshot {
        Some(snapshot) => match (snapshot.added_lines, snapshot.removed_lines) {
            (Some(added_lines), Some(removed_lines)) => TerminalManagerDiffSummaryModel {
                state: TerminalManagerDiffSummaryState::Ready,
                added_lines,
                removed_lines,
                tooltip_lines: source_control_tooltip_lines(
                    snapshot,
                    SOURCE_CONTROL_TOOLTIP_FILE_LIMIT,
                ),
            },
            _ if snapshot.loading => TerminalManagerDiffSummaryModel {
                state: TerminalManagerDiffSummaryState::Loading,
                added_lines: 0,
                removed_lines: 0,
                tooltip_lines: source_control_tooltip_lines(
                    snapshot,
                    SOURCE_CONTROL_TOOLTIP_FILE_LIMIT,
                ),
            },
            _ if snapshot.last_error.is_some() => TerminalManagerDiffSummaryModel {
                state: TerminalManagerDiffSummaryState::Error,
                added_lines: 0,
                removed_lines: 0,
                tooltip_lines: source_control_tooltip_lines(
                    snapshot,
                    SOURCE_CONTROL_TOOLTIP_FILE_LIMIT,
                ),
            },
            _ => TerminalManagerDiffSummaryModel {
                state: TerminalManagerDiffSummaryState::Unknown,
                added_lines: 0,
                removed_lines: 0,
                tooltip_lines: source_control_tooltip_lines(
                    snapshot,
                    SOURCE_CONTROL_TOOLTIP_FILE_LIMIT,
                ),
            },
        },
        None => TerminalManagerDiffSummaryModel {
            state: TerminalManagerDiffSummaryState::Unknown,
            added_lines: 0,
            removed_lines: 0,
            tooltip_lines: vec!["Status pending".to_owned()],
        },
    }
}

#[allow(dead_code)]
fn terminal_manager_diff_summary_visual(
    summary: &TerminalManagerDiffSummaryModel,
) -> TerminalManagerDiffSummaryVisual {
    match summary.state {
        TerminalManagerDiffSummaryState::Ready => TerminalManagerDiffSummaryVisual::Totals {
            added_text: format!("+{}", summary.added_lines),
            removed_text: format!("-{}", summary.removed_lines),
            added_color: source_control_badge_color(SourceControlBadgeState::Clean),
            removed_color: source_control_badge_color(SourceControlBadgeState::Error),
            separator_color: TEXT_MUTED,
        },
        TerminalManagerDiffSummaryState::Loading => TerminalManagerDiffSummaryVisual::Placeholder {
            text: "...",
            color: TEXT_MUTED,
        },
        TerminalManagerDiffSummaryState::Unknown | TerminalManagerDiffSummaryState::Error => {
            TerminalManagerDiffSummaryVisual::Placeholder {
                text: "--",
                color: TEXT_MUTED,
            }
        }
    }
}

fn source_control_tooltip_lines(snapshot: &SourceControlSnapshot, max_files: usize) -> Vec<String> {
    let has_display_data = source_control_snapshot_has_display_data(snapshot);
    let mut lines = Vec::with_capacity(max_files.saturating_add(3));

    if snapshot.loading {
        lines.push("Refreshing source control...".to_owned());
    }
    if let Some(error) = &snapshot.last_error {
        lines.push(error.clone());
    }
    if !has_display_data {
        if lines.is_empty() {
            lines.push("Status pending".to_owned());
        }
        return lines;
    }

    if let Some(branch_line) = source_control_branch_line(snapshot) {
        lines.push(branch_line);
    }

    if snapshot.files.is_empty() {
        if snapshot.last_error.is_none() && !snapshot.loading {
            lines.push("Working tree is clean".to_owned());
        }
        return lines;
    }

    for file in snapshot.files.iter().take(max_files) {
        let staged = if file.staged { " [staged]" } else { "" };
        lines.push(format!("{}{}: {}", file.status, staged, file.path));
    }

    if snapshot.files.len() > max_files {
        lines.push(format!("+{} more", snapshot.files.len() - max_files));
    }

    lines
}

#[allow(dead_code)]
fn terminal_manager_diff_summary_layout_job(
    ui: &Ui,
    summary: &TerminalManagerDiffSummaryModel,
) -> LayoutJob {
    let visual = terminal_manager_diff_summary_visual(summary);
    let mut layout_job = LayoutJob::default();
    layout_job.wrap.max_width = f32::INFINITY;
    let font_id = egui::TextStyle::Small.resolve(ui.style());

    match visual {
        TerminalManagerDiffSummaryVisual::Totals {
            added_text,
            removed_text,
            added_color,
            removed_color,
            separator_color,
        } => {
            layout_job.append(
                &added_text,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: added_color,
                    ..TextFormat::default()
                },
            );
            layout_job.append(
                " ",
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: separator_color,
                    ..TextFormat::default()
                },
            );
            layout_job.append(
                &removed_text,
                0.0,
                TextFormat {
                    font_id,
                    color: removed_color,
                    ..TextFormat::default()
                },
            );
        }
        TerminalManagerDiffSummaryVisual::Placeholder { text, color } => {
            layout_job.append(
                text,
                0.0,
                TextFormat {
                    font_id,
                    color,
                    ..TextFormat::default()
                },
            );
        }
    }

    layout_job
}

#[allow(dead_code)]
fn draw_terminal_manager_diff_summary(
    ui: &mut Ui,
    summary: &TerminalManagerDiffSummaryModel,
) -> egui::Response {
    let response = ui.add(
        egui::Label::new(WidgetText::from(terminal_manager_diff_summary_layout_job(
            ui, summary,
        )))
        .sense(Sense::hover()),
    );

    response.on_hover_ui(|ui| {
        for line in &summary.tooltip_lines {
            ui.label(line);
        }
    })
}

fn draw_terminal_manager_title_and_diff_summary(
    ui: &mut Ui,
    title: &str,
    text_color: Color32,
    is_active: bool,
    row_height: f32,
    _diff_summary: &TerminalManagerDiffSummaryModel,
) -> TerminalManagerTitleSummaryLayout {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width().max(0.0), row_height),
        Layout::left_to_right(Align::Center),
        |ui| {
            let title_label = egui::Label::new(RichText::new(title).color(text_color).strong())
                .truncate()
                .sense(Sense::hover());
            let title_response = ui.add(title_label);
            {
                let info = WidgetInfo::selected(
                    WidgetType::SelectableLabel,
                    ui.is_enabled(),
                    is_active,
                    title,
                );
                title_response.widget_info(|| info.clone());
            }
            title_response
                .clone()
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(title);

            ui.add_space(6.0);

            TerminalManagerTitleSummaryLayout {
                #[cfg(test)]
                title_rect: title_response.rect,
                #[cfg(test)]
                diff_summary_rect: draw_terminal_manager_diff_summary(ui, _diff_summary).rect,
            }
        },
    )
    .inner
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

fn project_group_header_actions_width(_section_gap: f32) -> f32 {
    CONTROL_ROW_HEIGHT
}

fn project_group_header_action_spec(
    action_kind: TerminalKind,
) -> (AppIcon, Color32, Color32, &'static str) {
    match action_kind {
        TerminalKind::Foreground => (
            icons::TERMINAL,
            BTN_BLUE,
            BTN_BLUE_HOVER,
            "New Foreground Terminal",
        ),
        TerminalKind::Background => (
            icons::LIST,
            BTN_TEAL,
            BTN_TEAL_HOVER,
            "New Background Terminal",
        ),
    }
}

fn terminal_manager_filter_color(filter: TerminalManagerFilter) -> Color32 {
    match filter {
        TerminalManagerFilter::Foreground => ACCENT,
        TerminalManagerFilter::Background => Color32::from_rgb(100, 180, 160),
    }
}

fn draw_terminal_manager_filter_tabs(
    ui: &mut Ui,
    selected_filter: &mut TerminalManagerFilter,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.add_space(SIDEBAR_ROW_LEADING_INSET);

        for filter in [
            TerminalManagerFilter::Foreground,
            TerminalManagerFilter::Background,
        ] {
            let is_selected = *selected_filter == filter;
            let response = ui
                .add(
                    egui::Label::new(RichText::new(filter.label()).strong().color(
                        if is_selected {
                            terminal_manager_filter_color(filter)
                        } else {
                            with_alpha(TEXT_MUTED, 180)
                        },
                    ))
                    .sense(Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if is_selected {
                let underline_rect = egui::Rect::from_min_max(
                    egui::pos2(response.rect.left(), response.rect.bottom() + 2.0),
                    egui::pos2(response.rect.right(), response.rect.bottom() + 4.0),
                );
                ui.painter().rect_filled(
                    underline_rect,
                    2.0,
                    terminal_manager_filter_color(filter),
                );
            }

            if response.clicked() && !is_selected {
                *selected_filter = filter;
                changed = true;
            }

            ui.add_space(12.0);
        }
    });

    changed
}

fn terminal_ids_for_project_kind(
    terminals: &BTreeMap<u64, TerminalEntry>,
    project_id: u64,
    kind: TerminalKind,
) -> Vec<u64> {
    terminals
        .iter()
        .filter(|(_, terminal)| terminal.project_id == project_id && terminal.kind == kind)
        .map(|(id, _)| *id)
        .collect()
}

fn project_group_header_row_layout(total_width: f32, section_gap: f32) -> (f32, f32) {
    let total_width = total_width.max(0.0);
    let section_gap = section_gap.max(0.0);
    let actions_width = project_group_header_actions_width(section_gap).min(total_width);
    let label_width = (total_width - actions_width - section_gap).max(0.0);
    (label_width, actions_width)
}

fn draw_project_group_header(
    ui: &mut Ui,
    project_name: &str,
    open: bool,
    can_expand: bool,
    action_kind: TerminalKind,
    diff_summary: &TerminalManagerDiffSummaryModel,
) -> (egui::Response, bool, bool) {
    let row_width = ui.available_width();
    let section_gap = ui.spacing().item_spacing.x;
    let (label_width, actions_width) = project_group_header_row_layout(row_width, section_gap);
    let row_height = CONTROL_ROW_HEIGHT;
    let (row_rect, response) =
        ui.allocate_exact_size(egui::vec2(row_width, row_height), Sense::click());

    let response = if can_expand {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };

    let text_color = if open {
        TEXT_PRIMARY
    } else if response.hovered() && can_expand {
        with_alpha(TEXT_PRIMARY, 180)
    } else if response.hovered() {
        with_alpha(TEXT_MUTED, 180)
    } else {
        with_alpha(TEXT_MUTED, 180)
    };

    let mut spawn_clicked = false;

    let label_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(label_width, row_height));
    if label_width > 0.0 && ui.is_rect_visible(label_rect) {
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(label_rect)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                let label = format!("{} {}", icons::FOLDER_OPEN, project_name);
                let _ = draw_terminal_manager_title_and_diff_summary(
                    ui,
                    &label,
                    text_color,
                    open,
                    row_height,
                    diff_summary,
                );
            },
        );
    }

    let actions_rect = egui::Rect::from_min_size(
        egui::pos2(row_rect.right() - actions_width, row_rect.top()),
        egui::vec2(actions_width, row_height),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(actions_rect)
            .layout(Layout::right_to_left(Align::Center)),
        |ui| {
            let (icon, bg, hover_bg, tooltip) = project_group_header_action_spec(action_kind);
            if styled_icon_button(ui, icon, bg, hover_bg, BTN_ICON_ACTIVE, tooltip) {
                spawn_clicked = true;
            }
        },
    );

    let header_clicked = response.clicked() && !spawn_clicked;

    (response, spawn_clicked, header_clicked)
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
    let response = response
        .on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand);

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
    let response = response
        .on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand);

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

fn resolve_ctrl_c_action(can_copy_selection: bool) -> CtrlCAction {
    if can_copy_selection {
        CtrlCAction::CopySelection
    } else {
        CtrlCAction::SendInterrupt
    }
}

fn terminal_secondary_click_action(
    has_selection: bool,
    can_paste: bool,
) -> TerminalSecondaryClickAction {
    if has_selection {
        TerminalSecondaryClickAction::OpenCopyMenu
    } else if can_paste {
        TerminalSecondaryClickAction::PasteImmediately
    } else {
        TerminalSecondaryClickAction::None
    }
}

fn terminal_link_activation_modifiers(modifiers: egui::Modifiers) -> bool {
    (modifiers.ctrl || modifiers.command) && !modifiers.alt && !modifiers.shift
}

fn terminal_link_rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES
        .get_or_init(|| {
            vec![
                Rule::new(CLOSING_PARENTHESIS_HYPERLINK_PATTERN, "$0")
                    .expect("valid closing-parenthesis terminal hyperlink regex"),
                Rule::new(GENERIC_HYPERLINK_PATTERN, "$0")
                    .expect("valid generic terminal hyperlink regex"),
            ]
        })
        .as_slice()
}

fn terminal_link_at_point(
    snapshot: &TerminalSelectionSnapshot,
    point: TerminalSelectionPoint,
) -> Option<String> {
    terminal_explicit_link_at_point(snapshot, point)
        .or_else(|| terminal_text_link_at_point(snapshot, point))
}

fn terminal_explicit_link_at_point(
    snapshot: &TerminalSelectionSnapshot,
    point: TerminalSelectionPoint,
) -> Option<String> {
    terminal_selection_hyperlink_at_point(snapshot, point)
        .filter(|uri| terminal_link_uri_allowed(uri))
        .map(str::to_owned)
}

fn terminal_text_link_at_point(
    snapshot: &TerminalSelectionSnapshot,
    point: TerminalSelectionPoint,
) -> Option<String> {
    let logical_line = terminal_logical_line(snapshot, point.row)?;
    let byte_index = terminal_logical_line_byte_index(&logical_line, point)?;
    Rule::match_hyperlinks(&logical_line.text, terminal_link_rules())
        .into_iter()
        .find(|matched| {
            matched.range.contains(&byte_index) && terminal_link_uri_allowed(matched.link.uri())
        })
        .map(|matched| matched.link.uri().to_owned())
}

fn terminal_link_uri_allowed(uri: &str) -> bool {
    terminal_link_has_ascii_case_insensitive_prefix(uri, "http://")
        || terminal_link_has_ascii_case_insensitive_prefix(uri, "https://")
}

fn terminal_link_has_ascii_case_insensitive_prefix(uri: &str, prefix: &str) -> bool {
    uri.get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

fn terminal_selection_hyperlink_at_point(
    snapshot: &TerminalSelectionSnapshot,
    point: TerminalSelectionPoint,
) -> Option<&str> {
    snapshot
        .lines
        .get(point.row)?
        .hyperlinks
        .iter()
        .find(|hyperlink| {
            point.column >= hyperlink.start_column && point.column < hyperlink.end_column
        })
        .map(|hyperlink| hyperlink.uri.as_str())
}

fn should_resolve_terminal_link(
    link_activation_modifiers_active: bool,
    primary_pressed_on_terminal: bool,
    has_pending_link_click: bool,
) -> bool {
    link_activation_modifiers_active || primary_pressed_on_terminal || has_pending_link_click
}

fn terminal_logical_line(
    snapshot: &TerminalSelectionSnapshot,
    row: usize,
) -> Option<TerminalLogicalLine> {
    let (start_row, end_row) = terminal_logical_line_row_range(snapshot, row)?;
    let mut logical_line = TerminalLogicalLine::default();

    for row in start_row..=end_row {
        append_terminal_logical_line_row(
            &mut logical_line.text,
            &mut logical_line.segments,
            row,
            &snapshot.lines[row],
        );
    }

    Some(logical_line)
}

fn terminal_logical_line_row_range(
    snapshot: &TerminalSelectionSnapshot,
    row: usize,
) -> Option<(usize, usize)> {
    if row >= snapshot.lines.len() {
        return None;
    }

    let mut start_row = row;
    while start_row > 0 && snapshot.lines[start_row - 1].wraps_to_next {
        start_row -= 1;
    }

    let mut end_row = row;
    while end_row + 1 < snapshot.lines.len() && snapshot.lines[end_row].wraps_to_next {
        end_row += 1;
    }

    Some((start_row, end_row))
}

fn append_terminal_logical_line_row(
    text: &mut String,
    segments: &mut Vec<TerminalLinkSegment>,
    row: usize,
    line: &TerminalSelectionLine,
) {
    let mut column = 0;

    for cell in &line.cells {
        if cell.column > column {
            append_terminal_logical_segment(
                text,
                segments,
                row,
                column,
                cell.column.min(line.width),
                &" ".repeat(cell.column.min(line.width).saturating_sub(column)),
            );
            column = cell.column.min(line.width);
        }

        let end_column = cell
            .column
            .saturating_add(cell.display_width.max(1))
            .min(line.width);
        if end_column <= cell.column {
            continue;
        }

        append_terminal_logical_segment(
            text,
            segments,
            row,
            cell.column,
            end_column,
            &cell.rendered_text(),
        );
        column = end_column;
    }

    if column < line.width {
        append_terminal_logical_segment(
            text,
            segments,
            row,
            column,
            line.width,
            &" ".repeat(line.width - column),
        );
    }
}

fn append_terminal_logical_segment(
    text: &mut String,
    segments: &mut Vec<TerminalLinkSegment>,
    row: usize,
    start_column: usize,
    end_column: usize,
    segment_text: &str,
) {
    if end_column <= start_column || segment_text.is_empty() {
        return;
    }

    let byte_start = text.len();
    text.push_str(segment_text);
    segments.push(TerminalLinkSegment {
        row,
        start_column,
        end_column,
        byte_range: byte_start..text.len(),
    });
}

fn terminal_logical_line_byte_index(
    logical_line: &TerminalLogicalLine,
    point: TerminalSelectionPoint,
) -> Option<usize> {
    let segment = logical_line.segments.iter().find(|segment| {
        segment.row == point.row
            && point.column >= segment.start_column
            && point.column < segment.end_column
    })?;
    let segment_text = &logical_line.text[segment.byte_range.clone()];
    let byte_offset =
        byte_index_for_display_column(segment_text, point.column - segment.start_column);
    Some(segment.byte_range.start + byte_offset)
}

fn byte_index_for_display_column(text: &str, column_offset: usize) -> usize {
    if column_offset == 0 {
        return 0;
    }

    let mut seen_columns = 0;
    for (byte_index, ch) in text.char_indices() {
        if seen_columns >= column_offset {
            return byte_index;
        }

        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width == 0 {
            continue;
        }

        let next_columns = seen_columns.saturating_add(width);
        if next_columns > column_offset {
            return byte_index;
        }

        seen_columns = next_columns;
    }

    text.len()
}

fn terminal_selection_point_from_pointer(
    pointer_pos: Option<egui::Pos2>,
    origin: egui::Pos2,
    snapshot: &TerminalSelectionSnapshot,
    char_width: f32,
    galley: &Galley,
) -> Option<TerminalSelectionPoint> {
    let pointer_pos = pointer_pos?;
    if snapshot.lines.is_empty() || galley.rows.is_empty() || char_width <= 0.0 {
        return None;
    }

    let max_row = snapshot
        .lines
        .len()
        .saturating_sub(1)
        .min(galley.rows.len().saturating_sub(1));
    let cursor = galley.cursor_from_pos(pointer_pos - origin);
    let row = cursor.rcursor.row.min(max_row);
    let line_width = terminal_selection_line_width(&snapshot.lines[row]);
    let column =
        (((pointer_pos.x - origin.x).max(0.0) / char_width).floor() as usize).min(line_width);

    Some(TerminalSelectionPoint { row, column })
}

fn terminal_selection_row_rect(
    galley: &Galley,
    origin: egui::Pos2,
    row: usize,
) -> Option<egui::Rect> {
    galley
        .rows
        .get(row)
        .map(|galley_row| galley_row.rect.translate(origin.to_vec2()))
}

fn terminal_cell_metric(metric: f32) -> f32 {
    if metric.is_finite() {
        metric.max(1.0)
    } else {
        1.0
    }
}

fn average_terminal_cell_width(sample_width: f32, sample_cells: usize) -> f32 {
    let sample_cells = sample_cells.max(1) as f32;
    terminal_cell_metric(sample_width / sample_cells)
}

fn terminal_font_family() -> FontFamily {
    FontFamily::Name(TERMINAL_FONT_FAMILY_NAME.into())
}

fn terminal_font_id(style: &egui::Style) -> FontId {
    let mut font_id = egui::TextStyle::Monospace.resolve(style);
    font_id.family = terminal_font_family();
    font_id
}

fn terminal_line_height(ui: &Ui, font_id: &FontId) -> f32 {
    terminal_cell_metric(ui.fonts(|fonts| fonts.row_height(font_id)))
}

fn terminal_char_width(ui: &Ui, font_id: &FontId) -> f32 {
    let sample_width = ui.fonts(|fonts| {
        let mut layout_job = LayoutJob::default();
        layout_job.wrap.max_width = f32::INFINITY;
        layout_job.append(
            &"W".repeat(TERMINAL_CHAR_WIDTH_SAMPLE_CELLS),
            0.0,
            TextFormat {
                font_id: font_id.clone(),
                ..TextFormat::default()
            },
        );
        fonts.layout_job(layout_job).size().x
    });
    average_terminal_cell_width(sample_width, TERMINAL_CHAR_WIDTH_SAMPLE_CELLS)
}

fn configure_terminal_font_family(fonts: &mut egui::FontDefinitions) {
    let fallback_font_names = load_terminal_fallback_font_names(fonts);
    install_terminal_font_family(fonts, &fallback_font_names);
}

fn install_terminal_font_family(fonts: &mut egui::FontDefinitions, fallback_font_names: &[String]) {
    let icon_font_names = icon_fonts()
        .iter()
        .map(|asset| asset.family.to_owned())
        .collect::<HashSet<_>>();
    let mut terminal_family = Vec::new();
    let mut seen_font_names = HashSet::new();

    for font_name in fallback_font_names.iter().chain(
        fonts
            .families
            .get(&FontFamily::Monospace)
            .into_iter()
            .flatten(),
    ) {
        if icon_font_names.contains(font_name) || !fonts.font_data.contains_key(font_name) {
            continue;
        }

        if seen_font_names.insert(font_name.clone()) {
            terminal_family.push(font_name.clone());
        }
    }

    fonts
        .families
        .insert(terminal_font_family(), terminal_family);
}

#[cfg(target_os = "windows")]
fn windows_terminal_font_candidates() -> &'static [(&'static str, &'static str)] {
    &WINDOWS_TERMINAL_FONT_CANDIDATES
}

#[cfg(target_os = "windows")]
fn load_terminal_fallback_font_names(fonts: &mut egui::FontDefinitions) -> Vec<String> {
    let fonts_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .map(|windir| windir.join("Fonts"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\Fonts"));
    let mut loaded_font_names = Vec::new();

    // Load embedded Nerd Font for terminal icon support
    fonts.font_data.insert(
        NERD_FONT_NAME.to_owned(),
        FontData::from_owned(NERD_FONT_DATA.to_vec()),
    );
    loaded_font_names.push(NERD_FONT_NAME.to_owned());

    for (font_name, file_name) in windows_terminal_font_candidates() {
        let font_path = fonts_dir.join(file_name);
        let Ok(font_bytes) = fs::read(&font_path) else {
            continue;
        };

        fonts
            .font_data
            .insert((*font_name).to_owned(), FontData::from_owned(font_bytes));
        loaded_font_names.push((*font_name).to_owned());
    }

    loaded_font_names
}

#[cfg(not(target_os = "windows"))]
fn load_terminal_fallback_font_names(_fonts: &mut egui::FontDefinitions) -> Vec<String> {
    Vec::new()
}

fn terminal_grid_dimensions(output_size: Vec2, char_width: f32, line_height: f32) -> (u16, u16) {
    let char_width = terminal_cell_metric(char_width);
    let line_height = terminal_cell_metric(line_height);
    let cols = ((output_size.x.max(0.0) / char_width).floor() as u16).max(8);
    let lines = ((output_size.y.max(0.0) / line_height).floor() as u16).max(3);
    (cols, lines)
}

fn force_terminal_pane_width(ui: &mut Ui, pane_width: f32) -> f32 {
    let pane_width = pane_width.max(0.0);
    let pane_right = ui.max_rect().left() + pane_width;
    ui.set_width(pane_width);
    ui.set_min_width(pane_width);
    ui.expand_to_include_x(pane_right);
    pane_right
}

fn terminal_output_viewport_size(output_size: Vec2) -> Vec2 {
    egui::vec2(output_size.x.max(0.0), output_size.y.max(0.0))
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
    snapshot: &TerminalSelectionSnapshot,
    selection: Option<&TerminalSelection>,
    char_width: f32,
    galley: &Galley,
) {
    if snapshot.lines.is_empty() || galley.rows.is_empty() {
        return;
    }

    let Some(selection) = selection.filter(|selection| selection.has_selection()) else {
        return;
    };

    let (start, end) = selection.normalized();
    let fill = with_alpha(ui.visuals().selection.bg_fill, 92);

    let max_row = end
        .row
        .min(snapshot.lines.len().saturating_sub(1))
        .min(galley.rows.len().saturating_sub(1));

    for row in start.row.min(max_row)..=max_row {
        let line_width = terminal_selection_line_width(&snapshot.lines[row]);
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

        let Some(row_rect) = terminal_selection_row_rect(galley, origin, row) else {
            continue;
        };
        let rect = egui::Rect::from_min_size(
            egui::pos2(origin.x + start_column as f32 * char_width, row_rect.top()),
            egui::vec2(
                (end_column - start_column) as f32 * char_width,
                row_rect.height(),
            ),
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
        } else if let Some(cursor_line) = snapshot.cursor_line.as_ref() {
            if cursor_line.row == line_index {
                for cell in &cursor_line.cells {
                    append_terminal_text(&mut job, &cell.rendered_text(), cell.style, font_id);
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
        ai_badge_visual, average_terminal_cell_width, build_terminal_cursor_overlay,
        build_terminal_render, collect_source_control_line_totals, collect_source_control_snapshot,
        configure_terminal_font_family, count_text_line_bytes, cursor_hidden_by_row_filter,
        default_app_open_command, draw_ai_badge, draw_terminal_manager_title_and_diff_summary,
        draw_terminal_status_badges, force_terminal_pane_width, install_terminal_font_family,
        merge_source_control_refresh_result, next_active_terminal_after_close,
        next_terminal_in_direction, next_terminal_in_linear_direction,
        normalize_terminal_background, parse_branch_header, parse_git_numstat_totals,
        recent_inputs_tooltip_text, recover_config_state, resolve_ctrl_c_action,
        should_resolve_terminal_link, source_control_badge_color, source_control_tooltip_lines,
        terminal_cell_metric, terminal_cursor_blink_phase_visible, terminal_cursor_overlay_rect,
        terminal_font_family, terminal_font_id, terminal_grid_dimensions, terminal_line_height,
        terminal_link_activation_modifiers, terminal_link_at_point, terminal_logical_line,
        terminal_logical_line_byte_index, terminal_manager_actions_width,
        terminal_manager_diff_summary_model, terminal_manager_diff_summary_visual,
        terminal_manager_row_chrome, terminal_manager_row_widths, terminal_output_surface_size,
        terminal_output_viewport_size, terminal_secondary_click_action,
        terminal_selection_point_from_pointer, terminal_selection_text, to_egui_color,
        update_stable_cursor_row, visible_terminal_cursor, AdeApp, AiBadgeModel, AiBadgeVisual,
        CodexCliStatusSource, CtrlCAction, DirectoryIndexSnapshot, DirectoryNode,
        FactoryDroidHookInboxEvent, FactoryDroidStatusSource, FactoryDroidTransportDiagnostics,
        PendingConfigChanges, PendingTerminalLinkClick, SourceControlBadgeState, SourceControlFile,
        SourceControlRefreshState, SourceControlSnapshot, TerminalCursorOverlay, TerminalEntry,
        TerminalManagerDiffSummaryVisual, TerminalNavigationDirection, TerminalNavigationShortcut,
        TerminalSecondaryClickAction, TerminalSelection, TerminalSelectionPoint, TransientToast,
        CODEX_LAUNCH_GRACE_MS, CODEX_PROCESS_POLL_MS, CODEX_TRAILING_OUTPUT_GRACE_MS,
        FACTORY_DROID_HOOK_POLL_MS, FACTORY_DROID_PROCESS_POLL_MS,
        FACTORY_DROID_TRAILING_OUTPUT_GRACE_MS, SOURCE_CONTROL_TOOLTIP_FILE_LIMIT,
        TERMINAL_COPY_FEEDBACK_TEXT, TERMINAL_COPY_TOAST_SECS, TERMINAL_OUTPUT_BG,
    };
    use crate::codex::CodexNotifyInboxEvent;
    use crate::hooks::{
        AiCliSession, AiCliStatus, AiCliTool, AiHookManager, AiHooksConfig, ProjectAiConfig,
    };
    use crate::layout;
    use crate::models::{
        AppConfig, MainVisibilityMode, ProjectRecord, ShellKind, TerminalKind,
        TerminalManagerFilter,
    };
    use crate::terminal::{
        test_terminal_runtime, test_terminal_runtime_with_capture, TerminalColor, TerminalCursor,
        TerminalCursorLine, TerminalCursorShape, TerminalRuntime, TerminalSelectionHyperlink,
        TerminalSelectionLine, TerminalSelectionSnapshot, TerminalSnapshot, TerminalStyle,
        TerminalStyledCell, TerminalStyledLine, TerminalStyledRun, TerminalUiEvent,
        TerminalUiEventKind, TrackedProcessIdentity,
    };
    use eframe::egui::text::{LayoutJob, TextFormat};
    use eframe::egui::{
        self, pos2, Color32, Context, Event, FontDefinitions, FontFamily, Galley, Id, Key,
        Modifiers, RawInput, Stroke,
    };
    use std::collections::{BTreeMap, VecDeque};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn maps_navigation_keys_to_escape_sequences() {
        let up = AdeApp::key_to_terminal_bytes(Key::ArrowUp, Modifiers::default());
        let delete = AdeApp::key_to_terminal_bytes(Key::Delete, Modifiers::default());

        assert_eq!(up, Some(b"\x1b[A".to_vec()));
        assert_eq!(delete, Some(b"\x1b[3~".to_vec()));
    }

    #[test]
    fn maps_backspace_to_delete_byte() {
        let backspace = AdeApp::key_to_terminal_bytes(Key::Backspace, Modifiers::default());
        assert_eq!(backspace, Some(b"\x7f".to_vec()));
    }

    #[test]
    fn first_backspace_press_arms_terminal_held_key_repeat() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));

        let processed = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            1.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );

        assert_eq!(
            processed,
            vec![test_repeatable_key_event(Key::Backspace, true)]
        );
        let held = app
            .terminal_held_key_repeat
            .expect("held repeat should be armed");
        assert_eq!(held.terminal_id, 1);
        assert_eq!(held.key, Key::Backspace);
        assert_eq!(held.modifiers, Modifiers::default());
        assert_eq!(held.first_pressed_at, 1.0);
        assert_eq!(held.last_repeat_at, None);
    }

    #[test]
    fn repeated_backspace_press_is_suppressed_while_key_is_held() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));

        let first = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );
        let repeated = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.1,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );

        assert_eq!(first, vec![test_repeatable_key_event(Key::Backspace, true)]);
        assert!(repeated.is_empty());
        assert!(app.terminal_held_key_repeat.is_some());
    }

    #[test]
    fn held_backspace_synthesizes_repeat_events_after_delay() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let timing = AdeApp::terminal_held_key_repeat_timing();

        app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );
        let repeated = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            timing.initial_delay_secs + (timing.interval_secs * 2.5),
            Vec::new(),
        );

        assert_eq!(repeated.len(), 3);
        assert!(repeated.iter().all(
            |event| *event == test_repeatable_key_event_with_repeat(Key::Backspace, true, true)
        ));
        assert_eq!(
            app.terminal_held_key_repeat
                .expect("held repeat should remain armed")
                .last_repeat_at,
            Some(timing.initial_delay_secs + (timing.interval_secs * 2.0))
        );
    }

    #[test]
    fn backspace_release_clears_held_repeat_and_stops_synthesis() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let timing = AdeApp::terminal_held_key_repeat_timing();

        app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );
        let released = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.2,
            vec![test_repeatable_key_event(Key::Backspace, false)],
        );
        let after_release = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            timing.initial_delay_secs + (timing.interval_secs * 4.0),
            Vec::new(),
        );

        assert_eq!(
            released,
            vec![test_repeatable_key_event(Key::Backspace, false)]
        );
        assert!(after_release.is_empty());
        assert!(app.terminal_held_key_repeat.is_none());
    }

    #[test]
    fn losing_terminal_capture_clears_held_repeat_state() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));

        app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );
        let processed =
            app.preprocess_terminal_input_with_held_repeat_state(Some(1), false, 0.1, Vec::new());

        assert!(processed.is_empty());
        assert!(app.terminal_held_key_repeat.is_none());
    }

    #[test]
    fn changing_active_terminal_clears_held_repeat_state() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );

        app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );
        app.set_active_terminal(&ctx, Some(2));

        assert!(app.terminal_held_key_repeat.is_none());
    }

    #[test]
    fn held_backspace_repeat_writes_multiple_delete_bytes_to_terminal() {
        let ctx = Context::default();
        let (runtime, capture) = test_terminal_runtime_with_capture();
        let mut app = test_app(
            [(1, test_terminal_entry_with_runtime(1, 7, runtime))],
            Some(1),
        );
        let timing = AdeApp::terminal_held_key_repeat_timing();

        let initial = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            0.0,
            vec![test_repeatable_key_event(Key::Backspace, true)],
        );
        app.route_active_terminal_input(&ctx, initial);
        capture.drain();

        let repeated = app.preprocess_terminal_input_with_held_repeat_state(
            Some(1),
            true,
            timing.initial_delay_secs + (timing.interval_secs * 2.5),
            Vec::new(),
        );
        app.route_active_terminal_input(&ctx, repeated);
        capture.drain();

        assert_eq!(capture.bytes(), vec![0x7f, 0x7f, 0x7f, 0x7f]);
    }

    #[test]
    fn partitions_terminal_input_events_out_of_ui_stream() {
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

        let (terminal_events, remaining_events) = AdeApp::partition_terminal_input_events(
            vec![
                focus_event.clone(),
                shift_tab.clone(),
                plain_tab.clone(),
                Event::Text("git status".to_owned()),
            ],
            true,
        );

        assert_eq!(
            terminal_events,
            vec![shift_tab, plain_tab, Event::Text("git status".to_owned())]
        );
        assert_eq!(remaining_events, vec![focus_event]);
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
            AdeApp::partition_terminal_input_events(vec![ctrl_right.clone()], true);

        assert!(terminal_events.is_empty());
        assert_eq!(remaining_events, vec![ctrl_right]);
    }

    #[test]
    fn ctrl_vertical_arrow_shortcuts_stay_out_of_terminal_stream_in_single_view() {
        let ctrl_down = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };

        let (terminal_events, remaining_events) =
            AdeApp::partition_terminal_input_events(vec![ctrl_down.clone()], true);

        assert!(terminal_events.is_empty());
        assert_eq!(remaining_events, vec![ctrl_down]);
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
            AdeApp::partition_terminal_input_events(vec![ctrl_shift_right.clone()], true);

        assert_eq!(terminal_events, vec![ctrl_shift_right]);
        assert!(remaining_events.is_empty());
    }

    #[test]
    fn ctrl_alt_arrow_remains_terminal_input_when_single_view_shortcuts_disabled() {
        let ctrl_alt_down = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
        };

        let (terminal_events, remaining_events) =
            AdeApp::partition_terminal_input_events(vec![ctrl_alt_down.clone()], false);

        assert_eq!(terminal_events, vec![ctrl_alt_down]);
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

        assert_eq!(captured.len(), 2);
        assert!(matches!(
            &captured[0],
            Event::Key {
                key: Key::Tab,
                pressed: true,
                ..
            }
        ));
        assert!(matches!(&captured[1], Event::Text(t) if t == "echo hi"));

        let remaining_events = ctx.input(|input| input.events.clone());
        assert_eq!(remaining_events, vec![Event::PointerMoved(pos2(4.0, 8.0))]);
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
    fn opening_settings_popup_resets_diagnostics_to_collapsed() {
        let mut app = test_app([], None);
        app.settings_diagnostics_expanded = true;

        app.open_settings_popup();

        assert!(app.show_settings_popup);
        assert!(!app.settings_diagnostics_expanded);
    }

    #[test]
    fn route_active_terminal_input_combines_keys_and_text_from_single_event_list() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("git status".to_owned()),
                Event::Key {
                    key: Key::ArrowUp,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.pending_line_for_title, "git status");
        assert!(terminal.dirty);
    }

    #[test]
    fn queued_paste_flushes_after_buffered_text_input() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.buffered_terminal_input = vec![Event::Text("before".to_owned())];

        assert!(app.queue_pasted_text_to_terminal(1, "paste"));

        let buffered_events = app.take_buffered_terminal_input();
        app.route_active_terminal_input(&ctx, buffered_events);
        app.flush_pending_terminal_pastes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.pending_line_for_title, "beforepaste");
        assert!(app.pending_terminal_pastes.is_empty());
    }

    #[test]
    fn queued_paste_captures_terminal_state_before_flush() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));

        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            terminal
                .runtime
                .advance_terminal_bytes_for_test(b"\x1b[?2004l");
        }

        assert!(app.queue_pasted_text_to_terminal(1, "paste"));

        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            terminal
                .runtime
                .advance_terminal_bytes_for_test(b"\x1b[?2004h");
        }

        let paste = app.pending_terminal_pastes.first().expect("queued paste");
        assert_eq!(paste.bytes, b"paste".to_vec());
        assert_eq!(paste.text, "paste");
    }

    #[test]
    fn enter_on_empty_pending_line_keeps_previous_title() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let terminal = app.terminals.get_mut(&1).expect("terminal 1");
        terminal.title = "git status".to_owned();
        terminal.full_title = "git status".to_owned();

        app.route_active_terminal_input(
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.title, "git status");
        assert_eq!(terminal.full_title, "git status");
        assert!(terminal.pending_line_for_title.is_empty());
        assert!(terminal.dirty);
    }

    #[test]
    fn empty_saved_message_keeps_previous_title() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let terminal = app.terminals.get_mut(&1).expect("terminal 1");
        terminal.title = "git status".to_owned();
        terminal.full_title = "git status".to_owned();

        app.send_saved_message_to_terminal(1, "   ");

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.title, "git status");
        assert_eq!(terminal.full_title, "git status");
        assert!(terminal.pending_line_for_title.is_empty());
        assert!(terminal.dirty);
    }

    #[test]
    fn enter_on_dollar_prefix_keeps_previous_title() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let terminal = app.terminals.get_mut(&1).expect("terminal 1");
        terminal.title = "git status".to_owned();
        terminal.full_title = "git status".to_owned();

        app.route_active_terminal_input(&ctx, vec![Event::Text("$ git status".to_owned())]);
        app.route_active_terminal_input(
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.title, "git status");
        assert_eq!(terminal.full_title, "git status");
    }

    #[test]
    fn saved_message_dollar_prefix_keeps_previous_title() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let terminal = app.terminals.get_mut(&1).expect("terminal 1");
        terminal.title = "git status".to_owned();
        terminal.full_title = "git status".to_owned();

        app.send_saved_message_to_terminal(1, "$ git status");

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.title, "git status");
        assert_eq!(terminal.full_title, "git status");
        assert!(terminal.pending_line_for_title.is_empty());
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
        app.buffered_terminal_navigation = vec![TerminalNavigationShortcut::Grid(
            TerminalNavigationDirection::Right,
        )];

        let shortcuts = app.take_buffered_terminal_navigation_shortcuts();

        assert_eq!(
            shortcuts,
            vec![TerminalNavigationShortcut::Grid(
                TerminalNavigationDirection::Right,
            )]
        );
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
        app.config.ui.multi_terminal_view_enabled = true;
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
    fn ctrl_c_without_selection_sends_interrupt_immediately() {
        let action = resolve_ctrl_c_action(false);

        assert_eq!(action, CtrlCAction::SendInterrupt);
    }

    #[test]
    fn ctrl_c_with_selection_copies_selection() {
        let action = resolve_ctrl_c_action(true);
        assert_eq!(action, CtrlCAction::CopySelection);
    }

    #[test]
    fn ctrl_c_interrupts_after_selection_is_cleared() {
        let action = resolve_ctrl_c_action(false);

        assert_eq!(action, CtrlCAction::SendInterrupt);
    }

    #[test]
    fn secondary_click_without_selection_pastes_immediately() {
        let action = terminal_secondary_click_action(false, true);

        assert_eq!(action, TerminalSecondaryClickAction::PasteImmediately);
    }

    #[test]
    fn secondary_click_with_selection_opens_copy_menu() {
        let action = terminal_secondary_click_action(true, true);

        assert_eq!(action, TerminalSecondaryClickAction::OpenCopyMenu);
    }

    #[test]
    fn secondary_click_on_exited_terminal_without_selection_does_nothing() {
        let action = terminal_secondary_click_action(false, false);

        assert_eq!(action, TerminalSecondaryClickAction::None);
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
    fn show_terminal_copy_feedback_sets_status_line_and_toast() {
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let ctx = Context::default();
        ctx.input_mut(|input| input.time = 2.5);

        app.show_terminal_copy_feedback(&ctx);

        assert_eq!(app.status_line, TERMINAL_COPY_FEEDBACK_TEXT);
        let toast = app.copy_toast.as_ref().expect("expected toast");
        assert_eq!(toast.message, TERMINAL_COPY_FEEDBACK_TEXT);
        assert!((toast.expires_at - (2.5 + TERMINAL_COPY_TOAST_SECS)).abs() < f64::EPSILON);
    }

    #[test]
    fn active_copy_toast_message_hides_expired_toast() {
        let toast = TransientToast {
            message: TERMINAL_COPY_FEEDBACK_TEXT.to_owned(),
            expires_at: 4.0,
        };

        assert_eq!(
            AdeApp::active_copy_toast_message(Some(&toast), 3.5),
            Some(TERMINAL_COPY_FEEDBACK_TEXT)
        );
        assert_eq!(AdeApp::active_copy_toast_message(Some(&toast), 4.1), None);
        assert_eq!(AdeApp::active_copy_toast_message(None, 0.0), None);
    }

    #[test]
    fn clearing_terminal_selection_preserves_cached_selection_snapshot() {
        let mut terminal = test_terminal_entry(1, 7);
        terminal.selection = Some(TerminalSelection {
            anchor: TerminalSelectionPoint { row: 1, column: 0 },
            focus: TerminalSelectionPoint { row: 1, column: 4 },
        });
        terminal.selection_snapshot = Some(TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("keep", 0, 4)], 4)],
        });
        terminal.selection_drag_active = true;

        AdeApp::clear_terminal_selection(&mut terminal);

        assert_eq!(terminal.selection, None);
        assert!(!terminal.selection_drag_active);
        assert_eq!(
            terminal.selection_snapshot,
            Some(TerminalSelectionSnapshot {
                lines: vec![test_selection_line(&[("keep", 0, 4)], 4)],
            })
        );
    }

    #[test]
    fn beginning_primary_interaction_tracks_link_candidate_with_collapsed_selection() {
        let mut terminal = test_terminal_entry(1, 7);
        let anchor = TerminalSelectionPoint { row: 0, column: 3 };
        let url = "https://example.com/docs".to_owned();

        AdeApp::begin_terminal_primary_interaction(&mut terminal, anchor, Some(url.clone()));

        assert_eq!(
            terminal.pending_link_click,
            Some(PendingTerminalLinkClick { anchor, url })
        );
        assert_eq!(
            terminal.selection,
            Some(TerminalSelection::collapsed(anchor))
        );
        assert!(terminal.selection_drag_active);
    }

    #[test]
    fn primary_drag_keeps_link_candidate_when_pointer_stays_on_anchor_cell() {
        let mut terminal = test_terminal_entry(1, 7);
        let anchor = TerminalSelectionPoint { row: 0, column: 2 };
        let url = "https://example.com/docs".to_owned();

        AdeApp::begin_terminal_primary_interaction(&mut terminal, anchor, Some(url.clone()));
        AdeApp::update_terminal_primary_drag(&mut terminal, anchor);

        assert_eq!(
            terminal.pending_link_click,
            Some(PendingTerminalLinkClick { anchor, url })
        );
        assert_eq!(
            terminal.selection,
            Some(TerminalSelection::collapsed(anchor))
        );
        assert!(terminal.selection_drag_active);
    }

    #[test]
    fn primary_drag_uses_pending_link_anchor_for_selection() {
        let mut terminal = test_terminal_entry(1, 7);
        let anchor = TerminalSelectionPoint { row: 0, column: 2 };
        let focus = TerminalSelectionPoint { row: 1, column: 5 };

        AdeApp::begin_terminal_primary_interaction(
            &mut terminal,
            anchor,
            Some("https://example.com/docs".to_owned()),
        );
        AdeApp::update_terminal_primary_drag(&mut terminal, focus);

        assert_eq!(terminal.pending_link_click, None);
        assert_eq!(
            terminal.selection,
            Some(TerminalSelection { anchor, focus })
        );
        assert!(terminal.selection_drag_active);
    }

    #[test]
    fn beginning_primary_interaction_replaces_existing_selection() {
        let mut terminal = test_terminal_entry(1, 7);
        let point = TerminalSelectionPoint { row: 2, column: 4 };
        terminal.selection = Some(TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 1 },
            focus: TerminalSelectionPoint { row: 1, column: 5 },
        });

        AdeApp::begin_terminal_primary_interaction(&mut terminal, point, None);

        assert_eq!(
            terminal.selection,
            Some(TerminalSelection::collapsed(point))
        );
        assert_eq!(terminal.pending_link_click, None);
        assert!(terminal.selection_drag_active);
    }

    #[test]
    fn plain_primary_drag_restarts_selection_from_new_press_point() {
        let mut terminal = test_terminal_entry(1, 7);
        let anchor = TerminalSelectionPoint { row: 2, column: 4 };
        let focus = TerminalSelectionPoint { row: 3, column: 6 };
        terminal.selection = Some(TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 1 },
            focus: TerminalSelectionPoint { row: 1, column: 5 },
        });

        AdeApp::begin_terminal_primary_interaction(&mut terminal, anchor, None);
        AdeApp::update_terminal_primary_drag(&mut terminal, focus);

        assert_eq!(
            terminal.selection,
            Some(TerminalSelection { anchor, focus })
        );
        assert_eq!(terminal.pending_link_click, None);
        assert!(terminal.selection_drag_active);
    }

    #[test]
    fn link_candidate_can_open_after_modifier_added_before_release() {
        let mut terminal = test_terminal_entry(1, 7);
        let anchor = TerminalSelectionPoint { row: 2, column: 4 };
        let url = "https://example.com/docs".to_owned();

        AdeApp::begin_terminal_primary_interaction(&mut terminal, anchor, Some(url.clone()));

        let opened =
            AdeApp::take_terminal_primary_clicked_link(&mut terminal, Some(url.as_str()), true);

        assert_eq!(opened.as_deref(), Some(url.as_str()));
        assert_eq!(terminal.pending_link_click, None);
        assert_eq!(terminal.selection, None);
        assert!(!terminal.selection_drag_active);
    }

    #[test]
    fn clicked_link_opens_same_url_and_clears_collapsed_selection() {
        let mut terminal = test_terminal_entry(1, 7);
        let point = TerminalSelectionPoint { row: 2, column: 4 };
        let url = "https://example.com/docs".to_owned();
        terminal.selection = Some(TerminalSelection::collapsed(point));
        terminal.selection_drag_active = true;
        terminal.pending_link_click = Some(PendingTerminalLinkClick {
            anchor: point,
            url: url.clone(),
        });

        let opened =
            AdeApp::take_terminal_primary_clicked_link(&mut terminal, Some(url.as_str()), true);

        assert_eq!(opened.as_deref(), Some(url.as_str()));
        assert_eq!(terminal.pending_link_click, None);
        assert_eq!(terminal.selection, None);
        assert!(!terminal.selection_drag_active);
    }

    #[test]
    fn clicked_link_preserves_existing_selection_range() {
        let mut terminal = test_terminal_entry(1, 7);
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 1 },
            focus: TerminalSelectionPoint { row: 0, column: 5 },
        };
        let url = "https://example.com/docs".to_owned();
        terminal.selection = Some(selection);
        terminal.pending_link_click = Some(PendingTerminalLinkClick {
            anchor: TerminalSelectionPoint { row: 0, column: 2 },
            url: url.clone(),
        });

        let opened =
            AdeApp::take_terminal_primary_clicked_link(&mut terminal, Some(url.as_str()), true);

        assert_eq!(opened.as_deref(), Some(url.as_str()));
        assert_eq!(terminal.selection, Some(selection));
        assert!(!terminal.selection_drag_active);
    }

    #[test]
    fn plain_text_press_does_not_open_link_on_release() {
        let mut terminal = test_terminal_entry(1, 7);
        let point = TerminalSelectionPoint { row: 0, column: 2 };

        AdeApp::begin_terminal_primary_interaction(&mut terminal, point, None);

        let opened = AdeApp::take_terminal_primary_clicked_link(
            &mut terminal,
            Some("https://example.com/docs"),
            true,
        );
        if opened.is_none() {
            AdeApp::clear_terminal_selection(&mut terminal);
        }

        assert_eq!(opened, None);
        assert_eq!(terminal.selection, None);
        assert!(!terminal.selection_drag_active);
    }

    #[test]
    fn clicked_link_ignores_different_release_url() {
        let mut terminal = test_terminal_entry(1, 7);
        terminal.pending_link_click = Some(PendingTerminalLinkClick {
            anchor: TerminalSelectionPoint { row: 0, column: 2 },
            url: "https://example.com/docs".to_owned(),
        });
        terminal.selection_drag_active = true;

        let opened = AdeApp::take_terminal_primary_clicked_link(
            &mut terminal,
            Some("https://example.com/other"),
            true,
        );

        assert_eq!(opened, None);
        assert_eq!(terminal.pending_link_click, None);
        assert!(!terminal.selection_drag_active);
    }

    #[test]
    fn selected_terminal_text_uses_cached_selection_snapshot() {
        let mut terminal = test_terminal_entry(1, 7);
        terminal.selection = Some(TerminalSelection {
            anchor: TerminalSelectionPoint { row: 1, column: 0 },
            focus: TerminalSelectionPoint { row: 1, column: 4 },
        });
        terminal.selection_snapshot = Some(TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line(&[("wrong", 0, 5)], 5),
                test_selection_line(&[("pick", 0, 4)], 4),
            ],
        });

        let text = AdeApp::selected_terminal_text(&mut terminal)
            .expect("cached selection snapshot should be used");

        assert_eq!(text, "pick");
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
                hyperlinks: Vec::new(),
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
                hyperlinks: Vec::new(),
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
    fn terminal_selection_text_preserves_trailing_blank_columns() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("a", 0, 1)], 5)],
            ..TerminalSelectionSnapshot::default()
        };
        let selection = TerminalSelection {
            anchor: TerminalSelectionPoint { row: 0, column: 0 },
            focus: TerminalSelectionPoint { row: 0, column: 5 },
        };

        let text = terminal_selection_text(&snapshot, Some(&selection))
            .expect("selection should produce text");

        assert_eq!(text, "a    ");
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
    fn selection_point_from_pointer_uses_full_selection_width() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("a", 0, 1)], 5)],
        };
        let galley = test_selection_galley("a");

        let point = terminal_selection_point_from_pointer(
            Some(pos2(4.2, galley.rows[0].rect.center().y)),
            pos2(0.0, 0.0),
            &snapshot,
            1.0,
            &galley,
        )
        .expect("expected selection point");

        assert_eq!(point, TerminalSelectionPoint { row: 0, column: 4 });
    }

    #[test]
    fn selection_point_from_pointer_supports_empty_lines() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[], 5), test_selection_line(&[], 5)],
        };
        let galley = test_selection_galley("\n");

        let point = terminal_selection_point_from_pointer(
            Some(pos2(3.4, galley.rows[1].rect.center().y)),
            pos2(0.0, 0.0),
            &snapshot,
            1.0,
            &galley,
        )
        .expect("expected selection point");

        assert_eq!(point, TerminalSelectionPoint { row: 1, column: 3 });
    }

    #[test]
    fn selection_point_from_pointer_uses_galley_row_geometry() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line(&[("top", 0, 3)], 3),
                test_selection_line(&[("middle", 0, 6)], 6),
                test_selection_line(&[("bottom", 0, 6)], 6),
            ],
        };
        let galley = test_selection_galley("top\nmiddle\nbottom");

        let point = terminal_selection_point_from_pointer(
            Some(pos2(0.2, galley.rows[1].rect.center().y)),
            pos2(0.0, 0.0),
            &snapshot,
            1.0,
            &galley,
        )
        .expect("expected selection point");

        assert_eq!(point, TerminalSelectionPoint { row: 1, column: 0 });
    }

    #[test]
    fn terminal_link_activation_requires_command_alias() {
        assert!(!terminal_link_activation_modifiers(Modifiers::default()));
        assert!(terminal_link_activation_modifiers(Modifiers {
            ctrl: true,
            ..Modifiers::default()
        }));
        assert!(terminal_link_activation_modifiers(Modifiers {
            command: true,
            ..Modifiers::default()
        }));
        assert!(!terminal_link_activation_modifiers(Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        }));
        assert!(!should_resolve_terminal_link(false, false, false));
        assert!(should_resolve_terminal_link(true, false, false));
        assert!(should_resolve_terminal_link(false, true, false));
        assert!(should_resolve_terminal_link(false, false, true));
    }

    #[test]
    fn terminal_logical_line_flattens_soft_wrapped_rows_for_byte_mapping() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line_with_wrap(&[("https://example.", 0, 16)], 16, true),
                test_selection_line(&[("com/path", 0, 8)], 8),
            ],
        };

        let logical_line = terminal_logical_line(&snapshot, 1).expect("expected logical line");
        let byte_index = terminal_logical_line_byte_index(
            &logical_line,
            TerminalSelectionPoint { row: 1, column: 3 },
        )
        .expect("expected byte index");

        assert_eq!(logical_line.text, "https://example.com/path");
        assert_eq!(byte_index, 19);
    }

    #[test]
    fn terminal_logical_line_byte_index_accounts_for_wide_prefix_columns() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(
                &[("你好https://example.com", 0, 22)],
                22,
            )],
        };

        let logical_line = terminal_logical_line(&snapshot, 0).expect("expected logical line");
        let byte_index = terminal_logical_line_byte_index(
            &logical_line,
            TerminalSelectionPoint { row: 0, column: 4 },
        )
        .expect("expected byte index");

        assert_eq!(byte_index, logical_line.text.find("https").unwrap());
    }

    #[test]
    fn terminal_link_at_point_matches_wrapped_http_url() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line_with_wrap(&[("https://example.", 0, 16)], 16, true),
                test_selection_line(&[("com/path", 0, 8)], 8),
            ],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 1, column: 2 });

        assert_eq!(link.as_deref(), Some("https://example.com/path"));
    }

    #[test]
    fn terminal_link_at_point_prefers_explicit_hyperlink_metadata() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line_with_wrap_and_links(
                &[("https://shown.example", 0, 20)],
                20,
                false,
                &[(0, 20, "https://target.example/docs")],
            )],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 0, column: 5 });

        assert_eq!(link.as_deref(), Some("https://target.example/docs"));
    }

    #[test]
    fn terminal_link_at_point_matches_wrapped_explicit_hyperlink() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![
                test_selection_line_with_wrap_and_links(
                    &[("shown-", 0, 6)],
                    6,
                    true,
                    &[(0, 6, "https://target.example/docs")],
                ),
                test_selection_line_with_wrap_and_links(
                    &[("docs", 0, 4)],
                    4,
                    false,
                    &[(0, 4, "https://target.example/docs")],
                ),
            ],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 1, column: 2 });

        assert_eq!(link.as_deref(), Some("https://target.example/docs"));
    }

    #[test]
    fn terminal_link_at_point_rejects_non_http_explicit_hyperlinks() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line_with_wrap_and_links(
                &[("docs", 0, 4)],
                4,
                false,
                &[(0, 4, "file://server/share")],
            )],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 0, column: 1 });

        assert_eq!(link, None);
    }

    #[test]
    fn terminal_link_at_point_accepts_mixed_case_explicit_http_scheme() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line_with_wrap_and_links(
                &[("docs", 0, 4)],
                4,
                false,
                &[(0, 4, "HTTPS://target.example/docs")],
            )],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 0, column: 2 });

        assert_eq!(link.as_deref(), Some("HTTPS://target.example/docs"));
    }

    #[test]
    fn terminal_link_at_point_accepts_mixed_case_plain_text_http_scheme() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(
                &[("HTTPS://example.com/path", 0, 24)],
                24,
            )],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 0, column: 10 });

        assert_eq!(link.as_deref(), Some("HTTPS://example.com/path"));
    }

    #[test]
    fn terminal_link_at_point_ignores_non_http_plain_text_schemes() {
        let snapshot = TerminalSelectionSnapshot {
            lines: vec![test_selection_line(&[("file://server/share", 0, 19)], 19)],
        };

        let link = terminal_link_at_point(&snapshot, TerminalSelectionPoint { row: 0, column: 5 });

        assert_eq!(link, None);
    }

    #[test]
    fn terminal_manager_row_reserves_gap_and_actions_width() {
        let actions_width = terminal_manager_actions_width(8.0, true);
        let (label_width, actions_area_width) =
            terminal_manager_row_widths(160.0, actions_width, 8.0);

        assert_eq!(actions_area_width, actions_width);
        assert!((label_width - 48.0).abs() < f32::EPSILON);
    }

    #[test]
    fn terminal_manager_row_gives_actions_full_width_when_space_is_tight() {
        let actions_width = terminal_manager_actions_width(8.0, true);
        let (label_width, actions_area_width) =
            terminal_manager_row_widths(70.0, actions_width, 8.0);

        assert_eq!(label_width, 0.0);
        assert_eq!(actions_area_width, 70.0);
    }

    #[test]
    fn terminal_manager_row_actions_shrink_when_visibility_toggle_is_hidden() {
        let actions_width = terminal_manager_actions_width(8.0, false);

        assert_eq!(
            actions_width,
            super::CONTROL_ROW_HEIGHT + super::TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH + 8.0
        );
    }

    #[test]
    fn project_group_title_layout_keeps_diff_summary_right_aligned() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let mut snapshot =
            test_source_control_snapshot("main", &[("src/app.rs", "Modified", false)]);
        snapshot.added_lines = Some(24);
        snapshot.removed_lines = Some(7);
        let diff_summary = terminal_manager_diff_summary_model(Some(&snapshot));

        let mut observed = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(180.0, super::CONTROL_ROW_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let layout = draw_terminal_manager_title_and_diff_summary(
                            ui,
                            &format!(
                                "{} {}",
                                super::icons::FOLDER_OPEN,
                                "project name that should truncate"
                            ),
                            super::TEXT_PRIMARY,
                            false,
                            super::CONTROL_ROW_HEIGHT,
                            &diff_summary,
                        );
                        observed = Some((layout.title_rect, layout.diff_summary_rect));
                    },
                );
            });
        });

        let (title_rect, diff_summary_rect) = observed.expect("expected title layout");
        assert!(title_rect.max.x <= diff_summary_rect.min.x);
    }

    #[test]
    fn project_group_header_row_reserves_space_for_single_inline_action() {
        let (label_width, actions_width) = super::project_group_header_row_layout(160.0, 8.0);

        assert_eq!(
            actions_width,
            super::project_group_header_actions_width(8.0)
        );
        assert!((label_width - 124.0).abs() < f32::EPSILON);
    }

    #[test]
    fn project_group_header_row_keeps_label_space_when_one_button_fits() {
        let (label_width, actions_width) = super::project_group_header_row_layout(40.0, 8.0);

        assert_eq!(label_width, 4.0);
        assert_eq!(actions_width, super::CONTROL_ROW_HEIGHT);
    }

    #[test]
    fn project_group_header_actions_width_matches_single_button() {
        assert_eq!(
            super::project_group_header_actions_width(8.0),
            super::CONTROL_ROW_HEIGHT
        );
    }

    #[test]
    fn project_group_header_action_spec_matches_selected_kind() {
        let foreground = super::project_group_header_action_spec(TerminalKind::Foreground);
        let background = super::project_group_header_action_spec(TerminalKind::Background);

        assert_eq!(foreground.0, super::icons::TERMINAL);
        assert_eq!(foreground.3, "New Foreground Terminal");
        assert_eq!(background.0, super::icons::LIST);
        assert_eq!(background.3, "New Background Terminal");
    }

    #[test]
    fn successful_foreground_spawn_auto_opens_terminal_group() {
        assert!(super::AdeApp::should_auto_open_project_terminal_group(true));
    }

    #[test]
    fn successful_background_spawn_auto_opens_terminal_group() {
        assert!(super::AdeApp::should_auto_open_project_terminal_group(true));
    }

    #[test]
    fn failed_spawn_does_not_auto_open_terminal_group() {
        assert!(!super::AdeApp::should_auto_open_project_terminal_group(
            false
        ));
    }

    #[test]
    fn terminal_manager_filter_defaults_to_foreground() {
        assert_eq!(
            TerminalManagerFilter::default(),
            TerminalManagerFilter::Foreground
        );
    }

    #[test]
    fn terminal_ids_for_project_kind_only_return_matching_terminals() {
        let terminals = BTreeMap::from([
            (
                1,
                test_terminal_entry_with_kind(1, 7, TerminalKind::Foreground),
            ),
            (
                2,
                test_terminal_entry_with_kind(2, 7, TerminalKind::Background),
            ),
            (
                3,
                test_terminal_entry_with_kind(3, 8, TerminalKind::Foreground),
            ),
            (
                4,
                test_terminal_entry_with_kind(4, 7, TerminalKind::Foreground),
            ),
        ]);

        assert_eq!(
            super::terminal_ids_for_project_kind(&terminals, 7, TerminalKind::Foreground),
            vec![1, 4]
        );
        assert_eq!(
            super::terminal_ids_for_project_kind(&terminals, 7, TerminalKind::Background),
            vec![2]
        );
    }

    #[test]
    fn source_control_tooltip_lines_cap_changed_files_and_show_overflow() {
        let files = (0..15)
            .map(|index| (format!("src/file-{index}.rs"), "Modified", index % 2 == 0))
            .collect::<Vec<_>>();
        let snapshot = SourceControlSnapshot {
            branch: "main".to_owned(),
            ahead: 2,
            behind: 1,
            files: files
                .iter()
                .map(|(path, status, staged)| SourceControlFile {
                    path: path.clone(),
                    status,
                    staged: *staged,
                })
                .collect(),
            added_lines: Some(0),
            removed_lines: Some(0),
            loading: false,
            last_error: None,
        };

        let lines = source_control_tooltip_lines(&snapshot, 12);

        assert_eq!(
            lines.first().map(String::as_str),
            Some("main  ahead:2 behind:1")
        );
        assert_eq!(lines.len(), 14);
        assert!(
            lines
                .iter()
                .any(|line| line == "Modified [staged]: src/file-0.rs"),
            "expected staged file entry"
        );
        assert_eq!(lines.last().map(String::as_str), Some("+3 more"));
    }

    #[test]
    fn source_control_tooltip_lines_keep_existing_data_visible_during_loading() {
        let mut snapshot =
            test_source_control_snapshot("main", &[("src/app.rs", "Modified", true)]);
        snapshot.loading = true;

        let lines = source_control_tooltip_lines(&snapshot, SOURCE_CONTROL_TOOLTIP_FILE_LIMIT);

        assert_eq!(
            lines,
            vec![
                "Refreshing source control...".to_owned(),
                "main".to_owned(),
                "Modified [staged]: src/app.rs".to_owned(),
            ]
        );
    }

    #[test]
    fn source_control_tooltip_lines_keep_existing_data_visible_during_error() {
        let mut snapshot =
            test_source_control_snapshot("main", &[("src/app.rs", "Modified", false)]);
        snapshot.last_error = Some("git status failed".to_owned());

        let lines = source_control_tooltip_lines(&snapshot, SOURCE_CONTROL_TOOLTIP_FILE_LIMIT);

        assert_eq!(
            lines,
            vec![
                "git status failed".to_owned(),
                "main".to_owned(),
                "Modified: src/app.rs".to_owned(),
            ]
        );
    }

    #[test]
    fn parse_git_numstat_totals_skips_binary_rows() {
        let totals = parse_git_numstat_totals(
            "2\t1\tsrc/app.rs\n-\t-\tassets/logo.png\n4\t0\tsrc/layout.rs\n",
        );

        assert_eq!(totals, (6, 1));
    }

    #[test]
    fn count_text_line_bytes_supports_utf8_and_utf16_variants() {
        assert_eq!(count_text_line_bytes(b"alpha\nbeta\n"), Some(2));
        assert_eq!(
            count_text_line_bytes(&[0xFF, 0xFE, b'a', 0, b'\r', 0, b'\n', 0, b'b', 0]),
            Some(2)
        );
        assert_eq!(
            count_text_line_bytes(&[0xFE, 0xFF, 0, b'a', 0, b'\n', 0, b'b']),
            Some(2)
        );
        assert_eq!(
            count_text_line_bytes(&[b'a', 0, b'\n', 0, b'b', 0]),
            Some(2)
        );
    }

    #[test]
    fn count_text_line_bytes_skips_binary_content_with_embedded_nuls() {
        assert_eq!(count_text_line_bytes(&[0_u8, 159, 146, 150]), None);
    }

    #[test]
    fn source_control_line_totals_use_head_diff_without_double_counting() {
        let temp_dir = TestTempDir::new("source-control-head-diff");
        init_test_git_repo(&temp_dir);

        fs::write(temp_dir.path.join("tracked.txt"), "old\n").expect("write tracked file");
        assert_git_success(&temp_dir.path, &["add", "tracked.txt"]);
        assert_git_success(&temp_dir.path, &["commit", "--quiet", "-m", "init"]);

        fs::write(temp_dir.path.join("tracked.txt"), "mid\n").expect("write staged change");
        assert_git_success(&temp_dir.path, &["add", "tracked.txt"]);
        fs::write(temp_dir.path.join("tracked.txt"), "new\n").expect("write unstaged change");

        assert_eq!(
            collect_source_control_line_totals(&temp_dir.path),
            Some((1, 1))
        );
    }

    #[test]
    fn source_control_line_totals_use_empty_tree_for_unborn_repos() {
        let temp_dir = TestTempDir::new("source-control-empty-tree");
        init_test_git_repo(&temp_dir);

        fs::write(temp_dir.path.join("tracked.txt"), "one\ntwo\n").expect("write staged file");
        assert_git_success(&temp_dir.path, &["add", "tracked.txt"]);
        fs::write(temp_dir.path.join("tracked.txt"), "one\ntwo\nthree\n")
            .expect("write working tree update");

        assert_eq!(
            collect_source_control_line_totals(&temp_dir.path),
            Some((3, 0))
        );
    }

    #[test]
    fn source_control_snapshot_counts_untracked_files_inside_collapsed_directory() {
        let temp_dir = TestTempDir::new("source-control-untracked-directory");
        init_test_git_repo(&temp_dir);
        assert_git_success(
            &temp_dir.path,
            &["commit", "--allow-empty", "--quiet", "-m", "init"],
        );

        fs::create_dir_all(temp_dir.path.join("nested/deep")).expect("create nested directory");
        fs::write(temp_dir.path.join("nested/deep/a.txt"), "one\ntwo\n")
            .expect("write nested text file");
        fs::write(temp_dir.path.join("nested/b.txt"), "three").expect("write sibling text file");

        let snapshot = collect_source_control_snapshot(&temp_dir.path, false);

        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.files[0].path, "nested/");
        assert_eq!(snapshot.added_lines, Some(3));
        assert_eq!(snapshot.removed_lines, Some(0));
    }

    #[test]
    fn terminal_manager_diff_summary_visuals_cover_ready_loading_and_error_states() {
        let clean =
            terminal_manager_diff_summary_model(Some(&test_source_control_snapshot("main", &[])));
        let loading_with_totals =
            terminal_manager_diff_summary_model(Some(&SourceControlSnapshot {
                loading: true,
                ..test_source_control_snapshot("main", &[])
            }));
        let error_with_totals = terminal_manager_diff_summary_model(Some(&SourceControlSnapshot {
            last_error: Some("git status failed".to_owned()),
            ..test_source_control_snapshot("main", &[])
        }));
        let loading_without_totals =
            terminal_manager_diff_summary_model(Some(&SourceControlSnapshot {
                loading: true,
                ..SourceControlSnapshot::default()
            }));
        let error_without_totals =
            terminal_manager_diff_summary_model(Some(&SourceControlSnapshot {
                last_error: Some("git status failed".to_owned()),
                ..SourceControlSnapshot::default()
            }));

        assert_eq!(
            terminal_manager_diff_summary_visual(&clean),
            TerminalManagerDiffSummaryVisual::Totals {
                added_text: "+0".to_owned(),
                removed_text: "-0".to_owned(),
                added_color: source_control_badge_color(SourceControlBadgeState::Clean),
                removed_color: source_control_badge_color(SourceControlBadgeState::Error),
                separator_color: super::TEXT_MUTED,
            }
        );
        assert_eq!(
            terminal_manager_diff_summary_visual(&loading_with_totals),
            TerminalManagerDiffSummaryVisual::Totals {
                added_text: "+0".to_owned(),
                removed_text: "-0".to_owned(),
                added_color: source_control_badge_color(SourceControlBadgeState::Clean),
                removed_color: source_control_badge_color(SourceControlBadgeState::Error),
                separator_color: super::TEXT_MUTED,
            }
        );
        assert_eq!(
            terminal_manager_diff_summary_visual(&error_with_totals),
            TerminalManagerDiffSummaryVisual::Totals {
                added_text: "+0".to_owned(),
                removed_text: "-0".to_owned(),
                added_color: source_control_badge_color(SourceControlBadgeState::Clean),
                removed_color: source_control_badge_color(SourceControlBadgeState::Error),
                separator_color: super::TEXT_MUTED,
            }
        );
        assert_eq!(
            terminal_manager_diff_summary_visual(&loading_without_totals),
            TerminalManagerDiffSummaryVisual::Placeholder {
                text: "...",
                color: super::TEXT_MUTED,
            }
        );
        assert_eq!(
            terminal_manager_diff_summary_visual(&error_without_totals),
            TerminalManagerDiffSummaryVisual::Placeholder {
                text: "--",
                color: super::TEXT_MUTED,
            }
        );
    }

    #[test]
    fn request_source_control_refresh_marks_loading_without_clearing_snapshot_data() {
        let mut app = test_app([], None);
        app.projects = BTreeMap::from([(7, test_project(7, "Repo", "C:/repo", &[]))]);
        app.source_control_state.insert(
            7,
            SourceControlSnapshot {
                branch: "main".to_owned(),
                ahead: 2,
                behind: 1,
                files: vec![SourceControlFile {
                    path: "src/app.rs".to_owned(),
                    status: "Modified",
                    staged: false,
                }],
                added_lines: Some(5),
                removed_lines: Some(2),
                loading: false,
                last_error: Some("old error".to_owned()),
            },
        );

        app.request_source_control_refresh(7, false, true);

        let snapshot = app
            .source_control_state
            .get(&7)
            .expect("expected source control snapshot");
        assert!(snapshot.loading);
        assert_eq!(snapshot.last_error, None);
        assert_eq!(snapshot.branch, "main");
        assert_eq!(snapshot.ahead, 2);
        assert_eq!(snapshot.behind, 1);
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.added_lines, Some(5));
        assert_eq!(snapshot.removed_lines, Some(2));
    }

    #[test]
    fn merge_source_control_refresh_result_preserves_existing_data_on_error() {
        let current = SourceControlSnapshot {
            branch: "main".to_owned(),
            ahead: 1,
            behind: 0,
            files: vec![SourceControlFile {
                path: "src/app.rs".to_owned(),
                status: "Modified",
                staged: true,
            }],
            added_lines: Some(8),
            removed_lines: Some(3),
            loading: true,
            last_error: None,
        };
        let incoming = SourceControlSnapshot {
            last_error: Some("git status failed".to_owned()),
            ..SourceControlSnapshot::default()
        };

        let merged = merge_source_control_refresh_result(Some(&current), incoming);

        assert_eq!(merged.branch, "main");
        assert_eq!(merged.ahead, 1);
        assert_eq!(merged.behind, 0);
        assert_eq!(merged.files.len(), 1);
        assert_eq!(merged.added_lines, Some(8));
        assert_eq!(merged.removed_lines, Some(3));
        assert!(!merged.loading);
        assert_eq!(merged.last_error.as_deref(), Some("git status failed"));
    }

    #[test]
    fn next_due_auto_source_control_project_prefers_selected_or_live_terminal_projects() {
        let mut app = test_app([(3, test_terminal_entry(3, 3))], None);
        app.projects = BTreeMap::from([
            (1, test_project(1, "Idle", "C:/idle", &[])),
            (2, test_project(2, "Selected", "C:/selected", &[])),
            (3, test_project(3, "Live", "C:/live", &[])),
        ]);
        app.selected_project = Some(2);
        app.source_control_refresh_state
            .insert(1, SourceControlRefreshState::default());
        app.source_control_refresh_state
            .insert(2, SourceControlRefreshState::default());
        app.source_control_refresh_state
            .insert(3, SourceControlRefreshState::default());

        assert_eq!(app.next_due_auto_source_control_project(0.0), Some(2));
    }

    #[test]
    fn next_due_auto_source_control_project_rotates_after_last_auto_refresh() {
        let mut app = test_app([], None);
        app.projects = BTreeMap::from([
            (1, test_project(1, "One", "C:/one", &[])),
            (2, test_project(2, "Two", "C:/two", &[])),
            (3, test_project(3, "Three", "C:/three", &[])),
        ]);
        app.source_control_last_auto_refresh_project = Some(2);

        assert_eq!(app.next_due_auto_source_control_project(0.0), Some(3));
    }

    #[test]
    fn manual_fetch_upgrades_pending_source_control_refresh() {
        let mut app = test_app([], None);
        app.projects = BTreeMap::from([(7, test_project(7, "Demo", "C:/demo", &[]))]);

        app.request_source_control_refresh(7, false, false);
        app.request_source_control_refresh(7, true, true);

        let state = app
            .source_control_refresh_state
            .get(&7)
            .copied()
            .expect("expected source control refresh state");
        assert!(state.queued);
        assert!(state.queued_manual);
        assert!(state.queued_fetch);
        assert!(app
            .source_control_state
            .get(&7)
            .is_some_and(|snapshot| snapshot.loading));
    }

    #[test]
    fn prune_source_control_state_removes_queued_entries_for_deleted_projects() {
        let mut app = test_app([], None);
        app.projects = BTreeMap::from([(1, test_project(1, "Keep", "C:/keep", &[]))]);
        app.source_control_refresh_state.insert(
            7,
            SourceControlRefreshState {
                queued: true,
                ..SourceControlRefreshState::default()
            },
        );
        app.source_control_state
            .insert(7, test_source_control_snapshot("main", &[]));
        app.source_control_last_auto_refresh_project = Some(7);

        app.prune_source_control_state();

        assert!(!app.source_control_refresh_state.contains_key(&7));
        assert!(!app.source_control_state.contains_key(&7));
        assert_eq!(app.source_control_last_auto_refresh_project, None);
    }

    #[test]
    fn remove_project_cleans_related_state_and_project_terminals() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 9)),
            ],
            Some(1),
        );
        app.projects = BTreeMap::from([
            (7, test_project(7, "Remove", "C:/remove", &[])),
            (9, test_project(9, "Keep", "C:/keep", &[])),
        ]);
        app.selected_project = Some(7);
        app.source_control_state
            .insert(7, test_source_control_snapshot("main", &[]));
        app.source_control_refresh_state.insert(
            7,
            SourceControlRefreshState {
                queued: true,
                ..SourceControlRefreshState::default()
            },
        );
        app.source_control_last_auto_refresh_project = Some(7);
        app.directory_index_state.insert(
            7,
            DirectoryIndexSnapshot {
                root: DirectoryNode {
                    name: "remove".to_owned(),
                    path: PathBuf::from("C:/remove"),
                    is_dir: true,
                    children: Vec::new(),
                },
                loading: false,
                last_error: None,
            },
        );
        app.directory_index_generation.insert(7, 2);
        app.saved_message_drafts.insert(7, "draft".to_owned());
        app.config_load_error = Some("simulated config reload error".to_owned());
        app.config_save_requires_reload = true;
        app.config_path = PathBuf::from("C:/path/that/does/not/exist/config.toml");

        app.remove_project(&ctx, 7);

        assert!(!app.projects.contains_key(&7));
        assert_eq!(app.selected_project, Some(9));
        assert!(!app.terminals.contains_key(&1));
        assert!(app.terminals.contains_key(&2));
        assert_eq!(app.active_terminal, Some(2));
        assert!(!app.source_control_state.contains_key(&7));
        assert!(!app.source_control_refresh_state.contains_key(&7));
        assert_eq!(app.source_control_last_auto_refresh_project, None);
        assert!(!app.directory_index_state.contains_key(&7));
        assert!(!app.directory_index_generation.contains_key(&7));
        assert!(!app.saved_message_drafts.contains_key(&7));
    }

    #[test]
    fn remove_project_ignores_unknown_project_id() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.projects = BTreeMap::from([(7, test_project(7, "Keep", "C:/keep", &[]))]);
        app.selected_project = Some(7);

        app.remove_project(&ctx, 99);

        assert_eq!(app.projects.len(), 1);
        assert!(app.projects.contains_key(&7));
        assert!(app.terminals.contains_key(&1));
        assert_eq!(app.selected_project, Some(7));
        assert_eq!(app.active_terminal, Some(1));
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
    fn next_terminal_in_linear_direction_moves_between_neighbors() {
        let ids = [1, 2, 3];

        let up = next_terminal_in_linear_direction(
            Some(3),
            &ids,
            |_| true,
            TerminalNavigationDirection::Up,
        );
        let down = next_terminal_in_linear_direction(
            Some(1),
            &ids,
            |_| true,
            TerminalNavigationDirection::Down,
        );

        assert_eq!(up, Some(2));
        assert_eq!(down, Some(2));
    }

    #[test]
    fn next_terminal_in_linear_direction_stops_at_edges() {
        let ids = [1, 2, 3];

        assert_eq!(
            next_terminal_in_linear_direction(
                Some(1),
                &ids,
                |_| true,
                TerminalNavigationDirection::Up
            ),
            None
        );
        assert_eq!(
            next_terminal_in_linear_direction(
                Some(3),
                &ids,
                |_| true,
                TerminalNavigationDirection::Down,
            ),
            None
        );
    }

    #[test]
    fn next_terminal_in_linear_direction_skips_non_selectable_neighbors() {
        let ids = [1, 2, 3, 4];

        assert_eq!(
            next_terminal_in_linear_direction(
                Some(1),
                &ids,
                |terminal_id| terminal_id != 2,
                TerminalNavigationDirection::Down,
            ),
            Some(3)
        );
        assert_eq!(
            next_terminal_in_linear_direction(
                Some(4),
                &ids,
                |terminal_id| terminal_id != 3,
                TerminalNavigationDirection::Up,
            ),
            Some(2)
        );
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
        app.config.ui.multi_terminal_view_enabled = true;

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
    }

    #[test]
    fn handle_shortcuts_moves_single_view_terminal_with_ctrl_down() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowDown,
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
                (3, test_terminal_entry(3, 7)),
            ],
            Some(1),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
    }

    #[test]
    fn handle_shortcuts_moves_single_view_terminal_with_ctrl_up() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowUp,
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
                (3, test_terminal_entry(3, 7)),
            ],
            Some(3),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
    }

    #[test]
    fn handle_shortcuts_moves_single_view_terminal_with_ctrl_alt_down() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(1),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
        assert!(app.terminals.contains_key(&1));
        assert!(app.terminals.contains_key(&2));
    }

    #[test]
    fn handle_shortcuts_moves_single_view_terminal_with_ctrl_alt_up() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(3),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
    }

    #[test]
    fn handle_shortcuts_skips_exited_single_view_terminal_with_ctrl_alt_down() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut exited_terminal = test_terminal_entry(2, 7);
        exited_terminal.exited = true;
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, exited_terminal),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(1),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(3));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![3]);
        assert_eq!(app.active_terminal_accepts_input(), Some(3));
    }

    #[test]
    fn handle_shortcuts_skips_exited_single_view_terminal_with_ctrl_alt_up() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut exited_terminal = test_terminal_entry(2, 7);
        exited_terminal.exited = true;
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, exited_terminal),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(3),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(1));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![1]);
        assert_eq!(app.active_terminal_accepts_input(), Some(1));
    }

    #[test]
    fn handle_shortcuts_recovers_from_exited_active_single_view_terminal_with_ctrl_alt_down() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut exited_terminal = test_terminal_entry(2, 7);
        exited_terminal.exited = true;
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, exited_terminal),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(2),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(3));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![3]);
        assert_eq!(app.active_terminal_accepts_input(), Some(3));
    }

    #[test]
    fn handle_shortcuts_recovers_from_exited_active_single_view_terminal_with_ctrl_alt_up() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut exited_terminal = test_terminal_entry(2, 7);
        exited_terminal.exited = true;
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, exited_terminal),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(2),
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(1));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![1]);
        assert_eq!(app.active_terminal_accepts_input(), Some(1));
    }

    #[test]
    fn handle_shortcuts_keeps_single_view_terminal_at_navigation_edges() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(1),
        );
        app.buffered_terminal_navigation = vec![TerminalNavigationShortcut::SingleViewLinear(
            TerminalNavigationDirection::Up,
        )];

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(1));

        app.set_active_terminal(&ctx, Some(3));
        app.buffered_terminal_navigation = vec![TerminalNavigationShortcut::SingleViewLinear(
            TerminalNavigationDirection::Down,
        )];

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(3));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![3]);
    }

    #[test]
    fn handle_shortcuts_keeps_exited_single_view_terminal_when_no_live_neighbor_exists() {
        let ctx = Context::default();
        let mut exited_terminal = test_terminal_entry(2, 7);
        exited_terminal.exited = true;
        let mut app = test_app(
            [(1, test_terminal_entry(1, 7)), (2, exited_terminal)],
            Some(2),
        );
        app.buffered_terminal_navigation = vec![TerminalNavigationShortcut::SingleViewLinear(
            TerminalNavigationDirection::Down,
        )];

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
        assert_eq!(app.active_terminal_accepts_input(), None);
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
        app.config.ui.multi_terminal_view_enabled = true;
        app.buffered_terminal_navigation = vec![TerminalNavigationShortcut::Grid(
            TerminalNavigationDirection::Right,
        )];

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
    fn handle_shortcuts_cycles_filter_with_ctrl_right_in_single_view() {
        let ctx = Context::default();
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

        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        assert_eq!(
            app.config.ui.terminal_manager_filter,
            TerminalManagerFilter::Foreground
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(
            app.config.ui.terminal_manager_filter,
            TerminalManagerFilter::Background
        );
    }

    #[test]
    fn handle_shortcuts_cycles_filter_with_ctrl_left_in_single_view() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowLeft,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.config.ui.terminal_manager_filter = TerminalManagerFilter::Background;

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(
            app.config.ui.terminal_manager_filter,
            TerminalManagerFilter::Foreground
        );
    }

    #[test]
    fn handle_shortcuts_ctrl_left_does_not_change_filter_when_already_at_foreground() {
        let ctx = Context::default();
        ctx.input_mut(|input| {
            input.events = vec![Event::Key {
                key: Key::ArrowLeft,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
            }];
        });

        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        assert_eq!(
            app.config.ui.terminal_manager_filter,
            TerminalManagerFilter::Foreground
        );

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(
            app.config.ui.terminal_manager_filter,
            TerminalManagerFilter::Foreground
        );
    }

    #[test]
    fn handle_shortcuts_ctrl_right_does_not_change_filter_when_already_at_background() {
        let ctx = Context::default();
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

        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.config.ui.terminal_manager_filter = TerminalManagerFilter::Background;

        app.handle_shortcuts(&ctx, egui::vec2(1200.0, 800.0));

        assert_eq!(
            app.config.ui.terminal_manager_filter,
            TerminalManagerFilter::Background
        );
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
    fn raw_input_hook_buffers_ctrl_arrow_for_active_terminal_in_multi_view() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        app.config.ui.multi_terminal_view_enabled = true;
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
            vec![TerminalNavigationShortcut::Grid(
                TerminalNavigationDirection::Right,
            )]
        );
    }

    #[test]
    fn raw_input_hook_buffers_ctrl_horizontal_arrow_for_filter_in_single_view() {
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
            vec![TerminalNavigationShortcut::SingleViewFilter(
                TerminalNavigationDirection::Right,
            )]
        );
    }

    #[test]
    fn raw_input_hook_buffers_ctrl_vertical_arrow_for_single_view_navigation() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let ctrl_down = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };
        let mut raw_input = RawInput {
            events: vec![ctrl_down],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(
            app.buffered_terminal_navigation,
            vec![TerminalNavigationShortcut::SingleViewLinear(
                TerminalNavigationDirection::Down,
            )]
        );
    }

    #[test]
    fn surrender_ui_text_focus_allows_ctrl_horizontal_arrow_buffering_for_filter() {
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
            vec![TerminalNavigationShortcut::SingleViewFilter(
                TerminalNavigationDirection::Right,
            )]
        );
    }

    #[test]
    fn raw_input_hook_buffers_ctrl_alt_arrow_for_single_view_navigation() {
        let ctx = Context::default();
        let mut app = test_app([(1, test_terminal_entry(1, 7))], Some(1));
        let ctrl_alt_down = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
        };
        let mut raw_input = RawInput {
            events: vec![ctrl_alt_down],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(
            app.buffered_terminal_navigation,
            vec![TerminalNavigationShortcut::SingleViewLinear(
                TerminalNavigationDirection::Down,
            )]
        );
    }

    #[test]
    fn raw_input_hook_steals_directory_search_text_for_attention_terminal() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_ai_attention(&mut app, 1);
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));

        let mut raw_input = RawInput {
            events: vec![Event::Text("hello".to_owned())],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(
            app.buffered_terminal_input,
            vec![Event::Text("hello".to_owned())]
        );
        assert!(!ctx.memory(|mem| mem.has_focus(AdeApp::directory_search_input_id())));

        let buffered_events = app.take_buffered_terminal_input();
        app.route_active_terminal_input(&ctx, buffered_events);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert_eq!(terminal.pending_line_for_title, "hello");
    }

    #[test]
    fn raw_input_hook_steals_saved_message_text_for_attention_terminal() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.selected_project = Some(7);
        seed_ai_attention(&mut app, 1);
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::saved_message_draft_input_id(7)));

        let mut raw_input = RawInput {
            events: vec![Event::Text("reply".to_owned())],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert!(raw_input.events.is_empty());
        assert_eq!(
            app.buffered_terminal_input,
            vec![Event::Text("reply".to_owned())]
        );
        assert!(!ctx.memory(|mem| mem.has_focus(AdeApp::saved_message_draft_input_id(7))));
    }

    #[test]
    fn raw_input_hook_keeps_text_input_focus_when_terminal_is_not_attention() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        ctx.memory_mut(|mem| mem.request_focus(AdeApp::directory_search_input_id()));

        let mut raw_input = RawInput {
            events: vec![Event::Text("hello".to_owned())],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert_eq!(raw_input.events, vec![Event::Text("hello".to_owned())]);
        assert!(app.buffered_terminal_input.is_empty());
        assert!(ctx.memory(|mem| mem.has_focus(AdeApp::directory_search_input_id())));
    }

    #[test]
    fn raw_input_hook_preserves_popup_keyboard_ownership_during_attention() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_ai_attention(&mut app, 1);
        ctx.memory_mut(|mem| {
            mem.request_focus(AdeApp::directory_search_input_id());
            mem.open_popup(Id::new("test-popup"));
        });

        let mut raw_input = RawInput {
            events: vec![Event::Text("hello".to_owned())],
            ..RawInput::default()
        };

        <AdeApp as eframe::App>::raw_input_hook(&mut app, &ctx, &mut raw_input);

        assert_eq!(raw_input.events, vec![Event::Text("hello".to_owned())]);
        assert!(app.buffered_terminal_input.is_empty());
        assert!(ctx.memory(|mem| mem.has_focus(AdeApp::directory_search_input_id())));
    }

    #[test]
    fn same_terminal_focus_clears_attention() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_ai_attention(&mut app, 1);

        app.set_active_terminal(&ctx, Some(1));

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert_eq!(app.active_terminal, Some(1));
    }

    #[test]
    fn switching_terminals_preserves_previous_attention() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );
        seed_ai_attention(&mut app, 1);
        seed_ai_attention(&mut app, 2);

        app.set_active_terminal(&ctx, Some(2));

        let terminal_one = app.terminals.get(&1).expect("terminal 1");
        let terminal_two = app.terminals.get(&2).expect("terminal 2");
        assert_eq!(terminal_one.ai_session.status, AiCliStatus::Attention);
        assert_eq!(terminal_two.ai_session.status, AiCliStatus::Inactive);
        assert_eq!(app.active_terminal, Some(2));
    }

    #[test]
    fn single_terminal_mode_shows_only_active_terminal() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(2),
        );
        app.terminals.get_mut(&2).expect("terminal 2").in_main_view = false;

        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
    }

    #[test]
    fn single_terminal_mode_falls_back_to_first_terminal_when_active_terminal_missing() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            None,
        );
        app.terminals.get_mut(&1).expect("terminal 1").in_main_view = false;
        app.terminals.get_mut(&2).expect("terminal 2").in_main_view = false;

        assert_eq!(app.visible_terminal_ids_for_main(), vec![1]);
    }

    #[test]
    fn multi_terminal_mode_restores_previous_visible_set_after_single_mode() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
                (3, test_terminal_entry(3, 7)),
            ],
            Some(3),
        );
        app.config.ui.multi_terminal_view_enabled = true;
        app.terminals.get_mut(&2).expect("terminal 2").in_main_view = false;

        assert_eq!(app.visible_terminal_ids_for_main(), vec![1, 3]);

        app.config.ui.multi_terminal_view_enabled = false;
        assert_eq!(app.visible_terminal_ids_for_main(), vec![3]);

        app.config.ui.multi_terminal_view_enabled = true;
        assert_eq!(app.visible_terminal_ids_for_main(), vec![1, 3]);
    }

    #[test]
    fn single_terminal_mode_switches_visible_terminal_without_closing_previous_session() {
        let ctx = Context::default();
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );

        app.set_active_terminal(&ctx, Some(2));

        assert_eq!(app.visible_terminal_ids_for_main(), vec![2]);
        assert!(app.terminals.contains_key(&1));
        assert!(app.terminals.contains_key(&2));
    }

    #[test]
    fn ai_status_change_event_updates_badge_without_debug_ui_state() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiStatusChange {
                    terminal_id: 1,
                    tool: Some(AiCliTool::FactoryDroid),
                    status: AiCliStatus::Running,
                    event: None,
                    from_title: false,
                },
            })
            .expect("send ai status change");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PtyHookEvent)
        );
    }

    #[test]
    fn ai_status_change_event_from_title_records_terminal_title_source() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiStatusChange {
                    terminal_id: 1,
                    tool: Some(AiCliTool::FactoryDroid),
                    status: AiCliStatus::Attention,
                    event: None,
                    from_title: true,
                },
            })
            .expect("send ai title-derived status change");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::TerminalTitle)
        );
    }

    #[test]
    fn ai_raw_chunk_event_is_ignored_without_affecting_status() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "factory-droid-hook:test".to_owned(),
                },
            })
            .expect("send ai raw chunk");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
    }

    #[test]
    fn ai_raw_chunk_event_sets_attention_for_active_factory_droid_session() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.factory_droid_session_active = true;
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "HOOKS  Stop\n └─ Script: mergen-ade-droid-status.ps1".to_owned(),
                },
            })
            .expect("send ai raw stop chunk");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PtyStop)
        );
    }

    #[test]
    fn factory_droid_stop_chunk_matches_official_hook_marker() {
        // Test that [droid-hook:event=Stop] raw marker triggers attention.
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.factory_droid_session_active = true;
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "[droid-hook:event=Stop]".to_owned(),
                },
            })
            .expect("send droid-hook stop chunk");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PtyStop)
        );
    }

    #[test]
    fn factory_droid_stop_chunk_matches_factory_droid_hook_marker() {
        // Test that [factory-droid-hook:event=Stop] raw marker triggers attention.
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.factory_droid_session_active = true;
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "[factory-droid-hook:event=Stop]".to_owned(),
                },
            })
            .expect("send factory-droid-hook stop chunk");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PtyStop)
        );
    }

    #[test]
    fn factory_droid_hook_inbox_poll_updates_terminal_status() {
        let ctx = Context::default();
        let hook_dir = TestFactoryDroidHookDir::new();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.factory_droid_hooks_dir = Some(hook_dir.path.clone());

        write_test_factory_droid_hook_events(
            &hook_dir.path,
            1,
            &[test_factory_droid_hook_event(
                1,
                &test_factory_droid_inbox_token(1),
                "running",
            )],
        );

        app.poll_factory_droid_hook_inboxes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
        assert!(terminal.factory_droid_session_active);
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::Inbox)
        );

        let inbox_path = AdeApp::factory_droid_hook_inbox_path_for_dir(&hook_dir.path, 1);
        let mut inbox_file = fs::OpenOptions::new()
            .append(true)
            .open(&inbox_path)
            .expect("open hook inbox for append");
        use std::io::Write as _;
        writeln!(
            inbox_file,
            "{}",
            serde_json::to_string(&test_factory_droid_hook_event(
                1,
                &test_factory_droid_inbox_token(1),
                "attention",
            ))
            .expect("serialize attention event")
        )
        .expect("append attention event");

        app.factory_droid_hook_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_HOOK_POLL_MS + 1));
        app.poll_factory_droid_hook_inboxes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
    }

    #[test]
    fn factory_droid_hook_inbox_ignores_partial_trailing_lines_until_completed() {
        let hook_dir = TestFactoryDroidHookDir::new();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.factory_droid_hooks_dir = Some(hook_dir.path.clone());

        let inbox_path = AdeApp::factory_droid_hook_inbox_path_for_dir(&hook_dir.path, 1);
        let partial = serde_json::to_string(&test_factory_droid_hook_event(
            1,
            &test_factory_droid_inbox_token(1),
            "running",
        ))
        .expect("serialize running event");
        fs::write(&inbox_path, &partial).expect("write partial hook inbox");

        assert!(!app.process_factory_droid_hook_inbox(1));
        assert_eq!(
            app.terminals.get(&1).expect("terminal 1").ai_session.status,
            AiCliStatus::Inactive
        );

        fs::write(&inbox_path, format!("{partial}\n")).expect("complete hook inbox line");

        assert!(app.process_factory_droid_hook_inbox(1));
        assert_eq!(
            app.terminals.get(&1).expect("terminal 1").ai_session.status,
            AiCliStatus::Running
        );
    }

    #[test]
    fn factory_droid_hook_inbox_ignores_stale_token_records() {
        let ctx = Context::default();
        let hook_dir = TestFactoryDroidHookDir::new();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.factory_droid_hooks_dir = Some(hook_dir.path.clone());

        write_test_factory_droid_hook_events(
            &hook_dir.path,
            1,
            &[test_factory_droid_hook_event(
                1,
                "stale-inbox-token",
                "attention",
            )],
        );

        app.poll_factory_droid_hook_inboxes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert_eq!(terminal.factory_droid_last_status_source, None);
    }

    #[test]
    fn route_active_terminal_input_sets_running_when_factory_droid_prompt_is_submitted() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.factory_droid_session_active = true;
        }

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("write a changelog".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
        assert!(terminal.pending_line_for_title.is_empty());
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PromptSubmit)
        );
    }

    #[test]
    fn route_active_terminal_input_marks_factory_droid_launch_pending_without_running() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("droid".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.factory_droid_launch_pending_since.is_some());
        assert!(!terminal.factory_droid_session_active);
    }

    #[test]
    fn factory_droid_process_poll_marks_session_active_for_matching_terminal_only() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );
        app.terminals
            .get_mut(&1)
            .expect("terminal 1")
            .runtime
            .set_factory_droid_process_active_for_test(Some(true));
        app.terminals
            .get_mut(&2)
            .expect("terminal 2")
            .runtime
            .set_factory_droid_process_active_for_test(Some(false));

        app.poll_factory_droid_processes(&ctx);

        let terminal_one = app.terminals.get(&1).expect("terminal 1");
        let terminal_two = app.terminals.get(&2).expect("terminal 2");
        assert!(terminal_one.factory_droid_session_active);
        assert_eq!(terminal_one.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert!(!terminal_two.factory_droid_session_active);
        assert_eq!(terminal_two.ai_session.tool, None);
    }

    #[test]
    fn factory_droid_process_poll_clears_stale_session_before_shell_commands_resume() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_factory_droid_process_active_for_test(Some(true));
        }
        app.poll_factory_droid_processes(&ctx);

        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_factory_droid_process_active_for_test(Some(false));
        }
        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let cleared_terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(cleared_terminal.ai_session.tool, None);
        assert_eq!(cleared_terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!cleared_terminal.factory_droid_session_active);

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("git status".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
    }

    #[test]
    fn factory_droid_stop_chunk_applies_before_process_cleanup_in_update_order() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Running;
            entry.factory_droid_session_active = true;
            entry
                .runtime
                .set_factory_droid_process_active_for_test(Some(false));
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "HOOKS  Stop".to_owned(),
                },
            })
            .expect("send stop chunk");

        app.process_terminal_events(&ctx);
        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_some());
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PtyStop)
        );
    }

    #[test]
    fn factory_droid_stop_chunk_sets_attention_after_process_exit_race() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Running;
            entry.factory_droid_session_active = true;
            entry
                .runtime
                .set_factory_droid_process_active_for_test(Some(false));
        }

        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_some());

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "HOOKS  Stop".to_owned(),
                },
            })
            .expect("send stop chunk");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert!(terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_none());
        assert_eq!(
            terminal.factory_droid_last_status_source,
            Some(FactoryDroidStatusSource::PtyStop)
        );
    }

    #[test]
    fn factory_droid_process_poll_keeps_attention_after_process_exit() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Attention;
            entry.factory_droid_session_active = true;
            entry
                .runtime
                .set_factory_droid_process_active_for_test(Some(false));
        }

        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
            assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
            assert!(!terminal.factory_droid_session_active);
            assert!(terminal.factory_droid_process_missing_since.is_some());
        }

        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.factory_droid_process_missing_since = Some(
                Instant::now() - Duration::from_millis(FACTORY_DROID_TRAILING_OUTPUT_GRACE_MS + 25),
            );
        }
        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_some());
    }

    #[test]
    fn factory_droid_process_poll_clears_running_after_trailing_grace() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Running;
            entry.factory_droid_session_active = true;
            entry
                .runtime
                .set_factory_droid_process_active_for_test(Some(false));
            entry.factory_droid_process_missing_since = Some(
                Instant::now() - Duration::from_millis(FACTORY_DROID_TRAILING_OUTPUT_GRACE_MS + 25),
            );
        }

        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_none());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn factory_droid_process_poll_keeps_running_state_when_process_probe_is_unsupported() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Running;
            entry.factory_droid_session_active = true;
            entry.factory_droid_process_missing_since = Some(
                Instant::now() - Duration::from_millis(FACTORY_DROID_TRAILING_OUTPUT_GRACE_MS + 25),
            );
        }

        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
        assert!(terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_some());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn factory_droid_process_poll_clears_expired_launch_when_process_probe_is_unsupported() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Inactive;
            entry.factory_droid_launch_pending_since =
                Some(Instant::now() - Duration::from_millis(FACTORY_DROID_LAUNCH_GRACE_MS + 25));
        }

        app.factory_droid_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(FACTORY_DROID_PROCESS_POLL_MS + 1));
        app.poll_factory_droid_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.factory_droid_launch_pending_since.is_none());
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_process_missing_since.is_none());
    }

    #[test]
    fn process_terminal_events_clears_factory_droid_state_on_exit() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Attention;
            entry.factory_droid_session_active = true;
            entry.factory_droid_launch_pending_since = Some(Instant::now());
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::Exit,
            })
            .expect("send exit event");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert!(terminal.exited);
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_launch_pending_since.is_none());
    }

    #[test]
    fn codex_launch_detection_matches_expected_commands() {
        assert!(AdeApp::is_codex_launch_command("codex"));
        assert!(AdeApp::is_codex_launch_command("codex --help"));
        assert!(AdeApp::is_codex_launch_command(
            r#""C:\Program Files\OpenAI\codex.exe" --version"#
        ));
        assert!(!AdeApp::is_codex_launch_command("codex-helper"));
        assert!(!AdeApp::is_codex_launch_command("npm exec codex"));
        assert!(!AdeApp::is_codex_launch_command("git codex"));
    }

    #[test]
    fn factory_droid_launch_detection_matches_expected_commands() {
        assert!(AdeApp::is_factory_droid_launch_command("droid"));
        assert!(AdeApp::is_factory_droid_launch_command("factory --help"));
        assert!(AdeApp::is_factory_droid_launch_command(
            r#""C:\Program Files\Factory Droid\droid.exe" --version"#
        ));
        assert!(!AdeApp::is_factory_droid_launch_command("droid-helper"));
        assert!(!AdeApp::is_factory_droid_launch_command("npm exec droid"));
        assert!(!AdeApp::is_factory_droid_launch_command("git factory"));
    }

    #[test]
    fn route_active_terminal_input_marks_codex_launch_pending_without_running() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("codex".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_some());
        assert!(!terminal.codex_session_active);
    }

    #[test]
    fn route_active_terminal_input_captures_codex_baseline_before_queuing_launch_bytes() {
        let ctx = Context::default();
        let (runtime, _capture) = test_terminal_runtime_with_capture();
        let mut app = test_app_with_ai_hooks(
            [(1, test_terminal_entry_with_runtime(1, 7, runtime))],
            Some(1),
        );
        let fast_start_identity = TrackedProcessIdentity {
            pid: 5010,
            creation_time: Some(6010),
        };

        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_codex_descendant_processes_for_test(Some(vec![]));
            entry
                .runtime
                .queue_codex_descendant_processes_after_next_input_for_test(vec![(
                    fast_start_identity,
                    "node.exe",
                )]);
        }

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("codex".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        assert_eq!(
            app.terminals
                .get(&1)
                .expect("terminal 1")
                .codex_launch_process_baseline,
            Some(Vec::new())
        );

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert!(terminal.codex_session_active);
        assert_eq!(terminal.codex_process_identity, Some(fast_start_identity));
        assert!(terminal.codex_launch_pending_since.is_none());
    }

    #[test]
    fn route_active_terminal_input_switches_from_factory_droid_to_codex_launch() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_ai_attention(&mut app, 1);

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("codex".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_some());
        assert!(!terminal.codex_session_active);
        assert!(!terminal.factory_droid_session_active);
        assert!(terminal.factory_droid_launch_pending_since.is_none());
        assert_eq!(terminal.factory_droid_last_status_source, None);
    }

    #[test]
    fn route_active_terminal_input_switches_from_codex_to_factory_droid_launch() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_codex_attention(&mut app, 1);

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("droid".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.factory_droid_launch_pending_since.is_some());
        assert!(!terminal.factory_droid_session_active);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_launch_pending_since.is_none());
        assert_eq!(terminal.codex_last_status_source, None);
    }

    #[test]
    fn codex_status_lifecycle_transitions_running_attention_idle_running_and_inactive() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        let baseline_identity = TrackedProcessIdentity {
            pid: 5007,
            creation_time: Some(6007),
        };
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_codex_descendant_processes_for_test(Some(vec![(
                    baseline_identity,
                    "node.exe",
                )]));
        }

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("codex".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );
        app.terminals
            .get_mut(&1)
            .expect("terminal 1")
            .runtime
            .set_codex_process_active_for_test(Some(true));
        app.poll_codex_processes(&ctx);

        app.route_active_terminal_input(
            &ctx,
            vec![
                Event::Text("summarize this diff".to_owned()),
                Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::default(),
                },
            ],
        );
        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
            assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
            assert_eq!(
                terminal.codex_last_status_source,
                Some(CodexCliStatusSource::PromptSubmit)
            );
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "[bell]".to_owned(),
                },
            })
            .expect("send codex bell chunk");
        app.process_terminal_events(&ctx);
        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
            assert_eq!(
                terminal.codex_last_status_source,
                Some(CodexCliStatusSource::Bell)
            );
        }

        app.route_active_terminal_input(&ctx, vec![Event::Text("continue".to_owned())]);
        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
            assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
            assert_eq!(terminal.pending_line_for_title, "continue");
        }

        app.route_active_terminal_input(
            &ctx,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::default(),
            }],
        );
        {
            let terminal = app.terminals.get(&1).expect("terminal 1");
            assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
            assert_eq!(
                terminal.codex_last_status_source,
                Some(CodexCliStatusSource::PromptSubmit)
            );
        }

        {
            let terminal = app.terminals.get_mut(&1).expect("terminal 1");
            terminal
                .runtime
                .set_codex_process_active_for_test(Some(false));
            terminal.codex_process_missing_since =
                Some(Instant::now() - Duration::from_millis(CODEX_TRAILING_OUTPUT_GRACE_MS + 25));
        }
        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_launch_pending_since.is_none());
        assert!(terminal.codex_process_missing_since.is_none());
    }

    #[test]
    fn codex_launch_pending_survives_early_false_probe() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            entry.ai_session.status = AiCliStatus::Inactive;
            entry.codex_launch_pending_since = Some(Instant::now());
            entry.codex_launch_process_baseline = Some(Vec::new());
            entry.runtime.set_codex_process_active_for_test(Some(false));
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_some());
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_process_missing_since.is_none());
    }

    #[test]
    fn codex_launch_pending_latches_detected_node_identity() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        let baseline_identity = TrackedProcessIdentity {
            pid: 5001,
            creation_time: Some(6001),
        };
        let node_identity = TrackedProcessIdentity {
            pid: 5002,
            creation_time: Some(6002),
        };
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_codex_descendant_processes_for_test(Some(vec![(
                    baseline_identity,
                    "node.exe",
                )]));
        }
        assert!(app.mark_codex_launch_pending(1, snapshot_codex_launch_baseline(&app, 1)));
        app.terminals
            .get_mut(&1)
            .expect("terminal 1")
            .runtime
            .set_codex_descendant_processes_for_test(Some(vec![
                (baseline_identity, "node.exe"),
                (node_identity, "node.exe"),
            ]));

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert!(terminal.codex_session_active);
        assert_eq!(terminal.codex_process_identity, Some(node_identity));
        assert!(terminal.codex_launch_pending_since.is_none());
    }

    #[test]
    fn codex_launch_pending_ignores_preexisting_node_descendant() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        let preexisting_node_identity = TrackedProcessIdentity {
            pid: 5002,
            creation_time: Some(6002),
        };
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_codex_descendant_processes_for_test(Some(vec![(
                    preexisting_node_identity,
                    "node.exe",
                )]));
        }
        assert!(app.mark_codex_launch_pending(1, snapshot_codex_launch_baseline(&app, 1)));

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_some());
        assert!(!terminal.codex_session_active);
        assert_eq!(
            terminal.codex_launch_process_baseline,
            Some(vec![preexisting_node_identity])
        );
    }

    #[test]
    fn codex_launch_pending_with_unavailable_baseline_does_not_latch_existing_descendant() {
        let ctx = Context::default();
        let preexisting_node_identity = TrackedProcessIdentity {
            pid: 5005,
            creation_time: Some(6005),
        };
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.runtime.set_codex_process_probe_unavailable_for_test();
        }
        assert!(app.mark_codex_launch_pending(1, snapshot_codex_launch_baseline(&app, 1)));
        assert_eq!(
            app.terminals
                .get(&1)
                .expect("terminal 1")
                .codex_launch_process_baseline,
            None
        );
        app.terminals
            .get_mut(&1)
            .expect("terminal 1")
            .runtime
            .set_codex_descendant_processes_for_test(Some(vec![(
                preexisting_node_identity,
                "node.exe",
            )]));

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_some());
        assert_eq!(
            terminal.codex_launch_process_baseline,
            Some(vec![preexisting_node_identity])
        );
        assert!(!terminal.codex_session_active);
        assert_eq!(terminal.codex_process_identity, None);
    }

    #[test]
    fn codex_launch_pending_recovers_empty_baseline_after_unavailable_probe() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.runtime.set_codex_process_probe_unavailable_for_test();
        }
        assert!(app.mark_codex_launch_pending(1, snapshot_codex_launch_baseline(&app, 1)));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.codex_launch_pending_since =
                Some(Instant::now() - Duration::from_millis(CODEX_LAUNCH_GRACE_MS + 25));
            entry
                .runtime
                .set_codex_descendant_processes_for_test(Some(vec![]));
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_none());
        assert!(terminal.codex_launch_process_baseline.is_none());
        assert!(!terminal.codex_session_active);
    }

    #[test]
    fn codex_tracked_identity_ignores_replacement_node_process() {
        let ctx = Context::default();
        let tracked_identity = TrackedProcessIdentity {
            pid: 5003,
            creation_time: Some(6003),
        };
        let replacement_identity = TrackedProcessIdentity {
            pid: 5004,
            creation_time: Some(6004),
        };
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            entry.ai_session.status = AiCliStatus::Running;
            entry.codex_session_active = true;
            entry.codex_process_identity = Some(tracked_identity);
            entry
                .runtime
                .set_codex_process_identity_for_test(Some(replacement_identity));
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert!(!terminal.codex_session_active);
        assert_eq!(terminal.codex_process_identity, Some(tracked_identity));
        assert!(terminal.codex_process_missing_since.is_some());
    }

    #[test]
    fn codex_tracked_identity_survives_temporary_probe_unavailability() {
        let ctx = Context::default();
        let tracked_identity = TrackedProcessIdentity {
            pid: 5006,
            creation_time: Some(6006),
        };
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            entry.ai_session.status = AiCliStatus::Running;
            entry.codex_session_active = true;
            entry.codex_process_identity = Some(tracked_identity);
            entry.runtime.set_codex_process_probe_unavailable_for_test();
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Running);
        assert!(terminal.codex_session_active);
        assert_eq!(terminal.codex_process_identity, Some(tracked_identity));
        assert!(terminal.codex_process_missing_since.is_none());
    }

    #[test]
    fn codex_launch_pending_clears_after_grace_when_process_is_still_missing() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            entry.ai_session.status = AiCliStatus::Inactive;
            entry.codex_launch_pending_since =
                Some(Instant::now() - Duration::from_millis(CODEX_LAUNCH_GRACE_MS + 25));
            entry.codex_launch_process_baseline = Some(Vec::new());
            entry.runtime.set_codex_process_active_for_test(Some(false));
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_launch_pending_since.is_none());
        assert!(terminal.codex_process_missing_since.is_none());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn codex_process_poll_keeps_expired_launch_when_process_probe_is_unsupported() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            entry.ai_session.status = AiCliStatus::Inactive;
            entry.codex_launch_pending_since =
                Some(Instant::now() - Duration::from_millis(CODEX_LAUNCH_GRACE_MS + 25));
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(terminal.codex_launch_pending_since.is_some());
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_process_missing_since.is_none());
    }

    #[test]
    fn codex_notify_inbox_poll_updates_attention() {
        let ctx = Context::default();
        let temp_dir = TestTempDir::new("codex-notify-inbox");
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.codex_cli_runtime_dir = Some(temp_dir.path.clone());

        write_test_codex_notify_events(
            &temp_dir.path,
            1,
            &test_codex_inbox_token(1),
            &[test_codex_notify_event(
                1,
                &test_codex_inbox_token(1),
                "attention",
            )],
        );

        app.poll_codex_notify_inboxes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert!(terminal.codex_session_active);
        assert_eq!(
            terminal.codex_last_status_source,
            Some(CodexCliStatusSource::Notify)
        );
    }

    #[test]
    fn codex_notify_inbox_ignores_events_from_other_token() {
        let ctx = Context::default();
        let temp_dir = TestTempDir::new("codex-notify-inbox-other-token");
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.codex_cli_runtime_dir = Some(temp_dir.path.clone());

        write_test_codex_notify_events(
            &temp_dir.path,
            1,
            "other-codex-token",
            &[test_codex_notify_event(1, "other-codex-token", "attention")],
        );

        app.poll_codex_notify_inboxes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.codex_session_active);
    }

    #[test]
    fn reset_codex_notify_inbox_removes_only_current_tokenized_file() {
        let temp_dir = TestTempDir::new("codex-notify-reset");
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.codex_cli_runtime_dir = Some(temp_dir.path.clone());

        let current_path =
            AdeApp::codex_notify_inbox_path_for_dir(&temp_dir.path, 1, &test_codex_inbox_token(1));
        let other_path = AdeApp::codex_notify_inbox_path_for_dir(&temp_dir.path, 1, "other-token");
        fs::write(&current_path, "current\n").expect("write current inbox");
        fs::write(&other_path, "other\n").expect("write other inbox");

        app.reset_codex_notify_inbox(1);

        assert!(!current_path.exists());
        assert!(other_path.exists());
    }

    #[test]
    fn codex_attention_stays_visible_after_process_exit_grace() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_codex_attention(&mut app, 1);
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.runtime.set_codex_process_identity_for_test(None);
            entry.codex_process_missing_since =
                Some(Instant::now() - Duration::from_millis(CODEX_TRAILING_OUTPUT_GRACE_MS + 25));
        }

        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_process_missing_since.is_none());
        assert!(terminal.codex_process_identity.is_none());
        assert_eq!(
            terminal.codex_last_status_source,
            Some(CodexCliStatusSource::Notify)
        );
    }

    #[test]
    fn sticky_codex_attention_is_cleared_by_user_interaction() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_codex_attention(&mut app, 1);
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.runtime.set_codex_process_identity_for_test(None);
            entry.codex_process_missing_since =
                Some(Instant::now() - Duration::from_millis(CODEX_TRAILING_OUTPUT_GRACE_MS + 25));
        }
        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        app.route_active_terminal_input(&ctx, vec![Event::Text("continue".to_owned())]);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_process_identity.is_none());
        assert!(terminal.codex_process_missing_since.is_none());
        assert_eq!(terminal.codex_last_status_source, None);
    }

    #[test]
    fn sticky_codex_attention_does_not_reprobe_unrelated_node_after_grace() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_codex_attention(&mut app, 1);
        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry.runtime.set_codex_process_identity_for_test(None);
            entry.codex_process_missing_since =
                Some(Instant::now() - Duration::from_millis(CODEX_TRAILING_OUTPUT_GRACE_MS + 25));
        }
        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        {
            let entry = app.terminals.get_mut(&1).expect("terminal 1");
            entry
                .runtime
                .set_codex_process_identity_for_test(Some(TrackedProcessIdentity {
                    pid: 5009,
                    creation_time: Some(6009),
                }));
        }
        app.codex_process_last_poll_at =
            Some(Instant::now() - Duration::from_millis(CODEX_PROCESS_POLL_MS + 1));
        app.poll_codex_processes(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.tool, Some(AiCliTool::CodexCli));
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_process_identity.is_none());
        assert!(terminal.codex_process_missing_since.is_none());
    }

    #[test]
    fn codex_bell_is_deduped_after_notify_attention() {
        let ctx = Context::default();
        let temp_dir = TestTempDir::new("codex-notify-bell-dedupe");
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        app.codex_cli_runtime_dir = Some(temp_dir.path.clone());

        write_test_codex_notify_events(
            &temp_dir.path,
            1,
            &test_codex_inbox_token(1),
            &[test_codex_notify_event(
                1,
                &test_codex_inbox_token(1),
                "attention",
            )],
        );
        app.poll_codex_notify_inboxes(&ctx);

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::AiRawChunk {
                    terminal_id: 1,
                    chunk: "[bell]".to_owned(),
                },
            })
            .expect("send codex bell chunk");
        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert_eq!(terminal.ai_session.status, AiCliStatus::Attention);
        assert_eq!(
            terminal.codex_last_status_source,
            Some(CodexCliStatusSource::Notify)
        );
    }

    #[test]
    fn process_terminal_events_clears_codex_state_on_exit() {
        let ctx = Context::default();
        let mut app = test_app_with_ai_hooks([(1, test_terminal_entry(1, 7))], Some(1));
        seed_codex_attention(&mut app, 1);
        if let Some(entry) = app.terminals.get_mut(&1) {
            entry.codex_launch_pending_since = Some(Instant::now());
        }

        app.terminal_events_tx
            .send(TerminalUiEvent {
                terminal_id: 1,
                kind: TerminalUiEventKind::Exit,
            })
            .expect("send exit event");

        app.process_terminal_events(&ctx);

        let terminal = app.terminals.get(&1).expect("terminal 1");
        assert!(terminal.exited);
        assert_eq!(terminal.ai_session.tool, None);
        assert_eq!(terminal.ai_session.status, AiCliStatus::Inactive);
        assert!(!terminal.codex_session_active);
        assert!(terminal.codex_launch_pending_since.is_none());
    }

    #[test]
    fn event_terminal_navigation_shortcut_accepts_egui_command_alias_for_ctrl() {
        let shortcut = AdeApp::event_terminal_navigation_shortcut(&Event::Key {
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

        assert_eq!(
            shortcut,
            Some(TerminalNavigationShortcut::Grid(
                TerminalNavigationDirection::Down,
            ))
        );
    }

    #[test]
    fn event_terminal_navigation_shortcut_recognizes_ctrl_alt_up_down() {
        let down = AdeApp::event_terminal_navigation_shortcut(&Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
        });
        let up = AdeApp::event_terminal_navigation_shortcut(&Event::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                ..Modifiers::default()
            },
        });

        assert_eq!(
            down,
            Some(TerminalNavigationShortcut::SingleViewLinear(
                TerminalNavigationDirection::Down,
            ))
        );
        assert_eq!(
            up,
            Some(TerminalNavigationShortcut::SingleViewLinear(
                TerminalNavigationDirection::Up,
            ))
        );
    }

    #[test]
    fn active_terminal_navigation_shortcut_maps_ctrl_vertical_arrows_by_view_mode() {
        let ctrl_down = Event::Key {
            key: Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };

        assert_eq!(
            AdeApp::active_terminal_navigation_shortcut(&ctrl_down, true),
            Some(TerminalNavigationShortcut::SingleViewLinear(
                TerminalNavigationDirection::Down,
            ))
        );
        assert_eq!(
            AdeApp::active_terminal_navigation_shortcut(&ctrl_down, false),
            Some(TerminalNavigationShortcut::Grid(
                TerminalNavigationDirection::Down,
            ))
        );
    }

    #[test]
    fn active_terminal_navigation_shortcut_maps_ctrl_horizontal_arrows_to_filter_in_single_view() {
        let ctrl_left = Event::Key {
            key: Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };
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

        assert_eq!(
            AdeApp::active_terminal_navigation_shortcut(&ctrl_left, true),
            Some(TerminalNavigationShortcut::SingleViewFilter(
                TerminalNavigationDirection::Left,
            ))
        );
        assert_eq!(
            AdeApp::active_terminal_navigation_shortcut(&ctrl_right, true),
            Some(TerminalNavigationShortcut::SingleViewFilter(
                TerminalNavigationDirection::Right,
            ))
        );

        assert_eq!(
            AdeApp::active_terminal_navigation_shortcut(&ctrl_left, false),
            Some(TerminalNavigationShortcut::Grid(
                TerminalNavigationDirection::Left,
            ))
        );
        assert_eq!(
            AdeApp::active_terminal_navigation_shortcut(&ctrl_right, false),
            Some(TerminalNavigationShortcut::Grid(
                TerminalNavigationDirection::Right,
            ))
        );
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
    fn close_terminal_removes_entry_and_preserves_expected_app_state() {
        let mut app = test_app(
            [
                (1, test_terminal_entry(1, 7)),
                (2, test_terminal_entry(2, 7)),
            ],
            Some(1),
        );

        let ctx = eframe::egui::Context::default();
        app.close_terminal(&ctx, 1);

        assert!(!app.terminals.contains_key(&1));
        assert_eq!(app.active_terminal, Some(2));
        assert_eq!(app.layout_epoch, 1);
        assert_eq!(app.status_line, "Closed Terminal 1");
    }

    #[test]
    fn recovered_config_preserves_loaded_settings_until_session_changes_them() {
        let loaded_project = test_project(7, "Loaded", "C:/loaded/demo", &[]);
        let loaded_config = AppConfig {
            default_shell: ShellKind::Cmd,
            ui: crate::models::UiConfig {
                project_explorer_expanded: false,
                multi_terminal_view_enabled: true,
                terminal_manager_filter: TerminalManagerFilter::Background,
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
        assert!(!recovered.ui.project_explorer_expanded);
        assert!(recovered.ui.multi_terminal_view_enabled);
        assert_eq!(
            recovered.ui.terminal_manager_filter,
            TerminalManagerFilter::Background
        );
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
    fn recovered_config_uses_session_multi_terminal_setting_when_ui_changed() {
        let loaded_config = AppConfig {
            ui: crate::models::UiConfig {
                multi_terminal_view_enabled: true,
                terminal_manager_filter: TerminalManagerFilter::Background,
                ..crate::models::UiConfig::default()
            },
            ..AppConfig::default()
        };
        let mut current_config = boot_failed_current_config();
        current_config.ui.multi_terminal_view_enabled = false;
        current_config.ui.terminal_manager_filter = TerminalManagerFilter::Foreground;

        let recovered = recover_config_state(
            &current_config,
            &BTreeMap::new(),
            None,
            loaded_config,
            PendingConfigChanges {
                ui: true,
                ..PendingConfigChanges::default()
            },
        );

        assert!(!recovered.ui.multi_terminal_view_enabled);
        assert_eq!(
            recovered.ui.terminal_manager_filter,
            TerminalManagerFilter::Foreground
        );
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

        assert_eq!(recovered.default_shell, ShellKind::default());
    }

    fn boot_failed_current_config() -> AppConfig {
        AppConfig {
            ui: crate::models::UiConfig {
                show_project_explorer: true,
                show_terminal_manager: true,
                main_visibility_mode: MainVisibilityMode::Global,
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
        test_selection_line_with_wrap_and_links(segments, width, wraps_to_next, &[])
    }

    fn test_selection_line_with_wrap_and_links(
        segments: &[(&str, usize, usize)],
        width: usize,
        wraps_to_next: bool,
        hyperlinks: &[(usize, usize, &str)],
    ) -> TerminalSelectionLine {
        let style = test_terminal_style();
        TerminalSelectionLine {
            width,
            wraps_to_next,
            hyperlinks: hyperlinks
                .iter()
                .map(|(column, display_width, uri)| TerminalSelectionHyperlink {
                    start_column: *column,
                    end_column: column + display_width,
                    uri: (*uri).to_owned(),
                })
                .collect(),
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

    fn test_selection_galley(text: &str) -> Arc<Galley> {
        let ctx = Context::default();
        let mut fonts = FontDefinitions::default();
        configure_terminal_font_family(&mut fonts);
        ctx.set_fonts(fonts);
        let _ = ctx.run(RawInput::default(), |_ctx| {});
        let mut layout_job = LayoutJob::default();
        layout_job.wrap.max_width = f32::INFINITY;
        layout_job.append(
            text,
            0.0,
            TextFormat {
                font_id: terminal_font_id(&egui::Style::default()),
                ..TextFormat::default()
            },
        );
        ctx.fonts(|fonts| fonts.layout_job(layout_job))
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
            ai_config: ProjectAiConfig::default(),
        }
    }

    fn test_source_control_snapshot(
        branch: &str,
        files: &[(&str, &'static str, bool)],
    ) -> SourceControlSnapshot {
        SourceControlSnapshot {
            branch: branch.to_owned(),
            ahead: 0,
            behind: 0,
            files: files
                .iter()
                .map(|(path, status, staged)| SourceControlFile {
                    path: (*path).to_owned(),
                    status,
                    staged: *staged,
                })
                .collect(),
            added_lines: Some(0),
            removed_lines: Some(0),
            loading: false,
            last_error: None,
        }
    }

    fn test_terminal_entry(id: u64, project_id: u64) -> TerminalEntry {
        test_terminal_entry_with_runtime(id, project_id, test_terminal_runtime())
    }

    fn test_terminal_entry_with_kind(
        id: u64,
        project_id: u64,
        kind: TerminalKind,
    ) -> TerminalEntry {
        let mut terminal = test_terminal_entry(id, project_id);
        terminal.kind = kind;
        terminal
    }

    fn test_terminal_entry_with_runtime(
        id: u64,
        project_id: u64,
        runtime: TerminalRuntime,
    ) -> TerminalEntry {
        TerminalEntry {
            id,
            project_id,
            kind: TerminalKind::Foreground,
            shell: ShellKind::PowerShell,
            title: format!("Terminal {id}"),
            full_title: format!("Terminal {id}"),
            pending_line_for_title: String::new(),
            recent_inputs: VecDeque::new(),
            in_main_view: true,
            dirty: false,
            last_seqno: 0,
            last_cursor_row: None,
            last_cursor_row_changed_at: None,
            stable_input_cursor_row: None,
            render_cache: TerminalSnapshot::default(),
            selection: None,
            selection_snapshot: None,
            pending_link_click: None,
            selection_drag_active: false,
            snapshot_refresh_deferred: false,
            exited: false,
            runtime,
            ai_session: AiCliSession::default(),
            factory_droid_inbox_token: Some(test_factory_droid_inbox_token(id)),
            codex_notify_inbox_token: Some(test_codex_inbox_token(id)),
            factory_droid_launch_pending_since: None,
            factory_droid_session_active: false,
            factory_droid_last_process_seen_at: None,
            factory_droid_process_missing_since: None,
            factory_droid_last_status_source: None,
            codex_launch_pending_since: None,
            codex_launch_process_baseline: None,
            codex_session_active: false,
            codex_process_identity: None,
            codex_last_process_seen_at: None,
            codex_process_missing_since: None,
            codex_last_status_source: None,
        }
    }

    fn test_repeatable_key_event(key: Key, pressed: bool) -> Event {
        test_repeatable_key_event_with_repeat(key, pressed, false)
    }

    fn test_repeatable_key_event_with_repeat(key: Key, pressed: bool, repeat: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat,
            modifiers: Modifiers::default(),
        }
    }

    fn test_app(
        terminals: impl IntoIterator<Item = (u64, TerminalEntry)>,
        active_terminal: Option<u64>,
    ) -> AdeApp {
        let (terminal_events_tx, terminal_events_rx) = crossbeam_channel::unbounded();
        let (source_control_commands_tx, _source_control_commands_rx) =
            crossbeam_channel::unbounded();
        let (_source_control_events_tx, source_control_events_rx) = crossbeam_channel::unbounded();
        let (directory_index_events_tx, directory_index_events_rx) = crossbeam_channel::unbounded();

        AdeApp {
            config_path: PathBuf::new(),
            current_executable_path: PathBuf::from(r"C:\tests\mergen-ade.exe"),
            factory_droid_hooks_dir: None,
            factory_droid_hooks_dir_error: None,
            factory_droid_hook_inboxes: BTreeMap::new(),
            factory_droid_hook_last_poll_at: None,
            factory_droid_process_last_poll_at: None,
            codex_cli_runtime_dir: None,
            codex_cli_runtime_dir_error: None,
            codex_notify_inboxes: BTreeMap::new(),
            codex_notify_last_poll_at: None,
            codex_process_last_poll_at: None,
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
            buffered_terminal_input: Vec::new(),
            buffered_terminal_navigation: Vec::new(),
            terminal_held_key_repeat: None,
            allow_attention_terminal_input_routing_once: false,
            pending_terminal_pastes: Vec::new(),
            terminal_events_tx,
            terminal_events_rx,
            show_settings_popup: false,
            settings_diagnostics_expanded: false,
            saved_message_drafts: BTreeMap::new(),
            directory_search_query: String::new(),
            directory_pending_tree_open_state_by_project: BTreeMap::new(),
            status_line: "Ready".to_owned(),
            copy_toast: None,
            layout_epoch: 0,
            theme_initialized: false,
            #[cfg(target_os = "windows")]
            window_hwnd: None,
            #[cfg(target_os = "windows")]
            window_layout_passes_remaining: 0,
            source_control_commands_tx,
            source_control_events_rx,
            source_control_state: BTreeMap::new(),
            source_control_refresh_state: BTreeMap::new(),
            source_control_worker_busy: false,
            source_control_last_auto_refresh_project: None,
            directory_index_events_tx,
            directory_index_events_rx,
            directory_index_state: BTreeMap::new(),
            directory_tree_has_collapsed_cache_by_project: BTreeMap::new(),
            directory_index_generation: BTreeMap::new(),
            ai_hook_manager: None,
        }
    }

    fn test_app_with_ai_hooks(
        terminals: impl IntoIterator<Item = (u64, TerminalEntry)>,
        active_terminal: Option<u64>,
    ) -> AdeApp {
        let mut app = test_app(terminals, active_terminal);
        app.ai_hook_manager = Some(Arc::new(AiHookManager::new(
            AiHooksConfig::with_factory_droid_defaults(),
        )));
        app
    }

    fn snapshot_codex_launch_baseline(
        app: &AdeApp,
        terminal_id: u64,
    ) -> Option<Vec<TrackedProcessIdentity>> {
        app.terminals
            .get(&terminal_id)
            .expect("terminal")
            .runtime
            .snapshot_codex_descendant_processes()
    }

    fn seed_ai_attention(app: &mut AdeApp, terminal_id: u64) {
        let Some(manager) = app.ai_hook_manager.as_ref().cloned() else {
            panic!("expected ai hook manager");
        };

        manager.set_tool(terminal_id, AiCliTool::FactoryDroid);
        let _ = manager.ai_waiting_for_user(terminal_id);

        if let Some(entry) = app.terminals.get_mut(&terminal_id) {
            entry.ai_session.tool = Some(AiCliTool::FactoryDroid);
            entry.ai_session.status = AiCliStatus::Attention;
            entry.factory_droid_session_active = true;
            entry.factory_droid_last_status_source =
                Some(FactoryDroidStatusSource::PtyNotification);
        }
    }

    fn seed_codex_attention(app: &mut AdeApp, terminal_id: u64) {
        let Some(manager) = app.ai_hook_manager.as_ref().cloned() else {
            panic!("expected ai hook manager");
        };

        manager.set_tool(terminal_id, AiCliTool::CodexCli);
        let _ = manager.ai_waiting_for_user(terminal_id);

        if let Some(entry) = app.terminals.get_mut(&terminal_id) {
            entry.ai_session.tool = Some(AiCliTool::CodexCli);
            entry.ai_session.status = AiCliStatus::Attention;
            entry.codex_session_active = true;
            entry.codex_process_identity = Some(TrackedProcessIdentity {
                pid: 7001,
                creation_time: Some(8001),
            });
            entry.codex_last_status_source = Some(CodexCliStatusSource::Notify);
        }
    }

    fn test_factory_droid_inbox_token(terminal_id: u64) -> String {
        format!("test-inbox-token-{terminal_id}")
    }

    fn test_codex_inbox_token(terminal_id: u64) -> String {
        format!("test-codex-inbox-token-{terminal_id}")
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let unique_suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mergen-ade-{prefix}-{}-{unique_suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp test dir");
            Self { path }
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn assert_git_success(project_path: &Path, args: &[&str]) -> String {
        let output = super::run_git_command(project_path, args).expect("run git command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn init_test_git_repo(temp_dir: &TestTempDir) {
        assert_git_success(&temp_dir.path, &["init", "--quiet"]);
        assert_git_success(&temp_dir.path, &["config", "user.name", "Test User"]);
        assert_git_success(
            &temp_dir.path,
            &["config", "user.email", "test@example.com"],
        );
    }

    fn test_factory_droid_hook_event(
        terminal_id: u64,
        inbox_token: &str,
        status: &str,
    ) -> FactoryDroidHookInboxEvent {
        FactoryDroidHookInboxEvent {
            terminal_id: terminal_id.to_string(),
            session_id: Some("session-1".to_owned()),
            inbox_token: Some(inbox_token.to_owned()),
            hook_event_name: match status {
                "running" => "UserPromptSubmit".to_owned(),
                _ => "Stop".to_owned(),
            },
            status: status.to_owned(),
            notification_kind: (status == "attention").then(|| "idle_prompt".to_owned()),
            message: Some("Droid is waiting for your input".to_owned()),
            timestamp_utc: Some("2026-04-06T00:00:00Z".to_owned()),
        }
    }

    struct TestFactoryDroidHookDir {
        path: PathBuf,
    }

    impl TestFactoryDroidHookDir {
        fn new() -> Self {
            let unique_suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mergen-ade-factory-droid-hook-tests-{}-{unique_suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp hook dir");
            Self { path }
        }
    }

    impl Drop for TestFactoryDroidHookDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_test_factory_droid_hook_events(
        dir: &std::path::Path,
        terminal_id: u64,
        events: &[FactoryDroidHookInboxEvent],
    ) {
        let path = AdeApp::factory_droid_hook_inbox_path_for_dir(dir, terminal_id);
        let mut payload = String::new();
        for event in events {
            payload.push_str(
                &serde_json::to_string(event).expect("serialize factory droid hook event"),
            );
            payload.push('\n');
        }
        fs::write(path, payload).expect("write hook inbox");
    }

    fn test_codex_notify_event(
        terminal_id: u64,
        inbox_token: &str,
        status: &str,
    ) -> CodexNotifyInboxEvent {
        CodexNotifyInboxEvent {
            terminal_id: terminal_id.to_string(),
            tool: "codex".to_owned(),
            status: status.to_owned(),
            inbox_token: Some(inbox_token.to_owned()),
            event_kind: Some(match status {
                "attention" => "agent-turn-complete".to_owned(),
                _ => "unknown".to_owned(),
            }),
            raw_json: format!("{{\"event\":\"{status}\"}}"),
            timestamp_utc: "2026-04-06T00:00:00Z".to_owned(),
        }
    }

    fn write_test_codex_notify_events(
        dir: &std::path::Path,
        terminal_id: u64,
        inbox_token: &str,
        events: &[CodexNotifyInboxEvent],
    ) {
        let path = AdeApp::codex_notify_inbox_path_for_dir(dir, terminal_id, inbox_token);
        let mut payload = String::new();
        for event in events {
            payload.push_str(&serde_json::to_string(event).expect("serialize codex notify event"));
            payload.push('\n');
        }
        fs::write(path, payload).expect("write codex notify inbox");
    }

    #[test]
    fn pending_line_keeps_last_logical_line() {
        let mut pending = String::new();

        AdeApp::append_pending_line(&mut pending, "echo first");
        AdeApp::append_pending_line(&mut pending, "\nnext");

        assert_eq!(pending, "next");
    }

    #[test]
    fn append_pending_line_keeps_only_last_pasted_line() {
        let mut pending = String::new();

        AdeApp::append_pending_line(&mut pending, "first\r\n\r\nthird");

        assert_eq!(pending, "third");
    }

    #[test]
    fn push_recent_input_keeps_last_4() {
        let mut recent_inputs = VecDeque::new();

        AdeApp::push_recent_input(&mut recent_inputs, "first");
        assert_eq!(recent_inputs.len(), 1);
        assert_eq!(recent_inputs[0], "first");

        AdeApp::push_recent_input(&mut recent_inputs, "second");
        assert_eq!(recent_inputs.len(), 2);
        assert_eq!(recent_inputs[0], "second");
        assert_eq!(recent_inputs[1], "first");

        AdeApp::push_recent_input(&mut recent_inputs, "third");
        AdeApp::push_recent_input(&mut recent_inputs, "fourth");
        assert_eq!(recent_inputs.len(), 4);

        AdeApp::push_recent_input(&mut recent_inputs, "fifth");
        assert_eq!(recent_inputs.len(), 4);
        assert_eq!(recent_inputs[0], "fifth");
        assert_eq!(recent_inputs[3], "second");
    }

    #[test]
    fn push_recent_input_ignores_empty_messages() {
        let mut recent_inputs = VecDeque::new();

        AdeApp::push_recent_input(&mut recent_inputs, "");
        assert!(recent_inputs.is_empty());

        AdeApp::push_recent_input(&mut recent_inputs, "   ");
        assert!(recent_inputs.is_empty());
    }

    #[test]
    fn recent_inputs_tooltip_text_shows_all_inputs() {
        let mut recent_inputs = VecDeque::new();
        // push_front: newest first
        recent_inputs.push_front("msg1".to_owned());
        recent_inputs.push_front("msg2".to_owned());

        let tooltip = recent_inputs_tooltip_text(&recent_inputs);

        // msg2 is newest (index 0, number 1)
        assert!(tooltip.contains("1: msg2"));
        // msg1 is second newest (index 1, number 2)
        assert!(tooltip.contains("2: msg1"));
        assert!(tooltip.contains("─ Recent Inputs ─"));
    }

    #[test]
    fn recent_inputs_tooltip_text_returns_empty_for_no_inputs() {
        let recent_inputs: VecDeque<String> = VecDeque::new();
        let tooltip = recent_inputs_tooltip_text(&recent_inputs);
        assert!(tooltip.is_empty());
    }

    #[test]
    fn terminal_cell_metric_keeps_fractional_font_measurement() {
        assert_eq!(terminal_cell_metric(7.25), 7.25);
    }

    #[test]
    fn terminal_cell_metric_falls_back_for_invalid_measurement() {
        assert_eq!(terminal_cell_metric(0.0), 1.0);
        assert_eq!(terminal_cell_metric(f32::NAN), 1.0);
    }

    #[test]
    fn average_terminal_cell_width_preserves_fractional_width() {
        let width = average_terminal_cell_width(464.0, 64);

        assert_eq!(width, 7.25);
    }

    #[test]
    fn average_terminal_cell_width_falls_back_when_sample_is_invalid() {
        assert_eq!(average_terminal_cell_width(0.0, 64), 1.0);
        assert_eq!(average_terminal_cell_width(f32::NAN, 64), 1.0);
    }

    #[test]
    fn terminal_font_id_uses_dedicated_named_family() {
        let font_id = terminal_font_id(&egui::Style::default());

        assert_eq!(font_id.family, terminal_font_family());
    }

    #[test]
    fn terminal_font_family_prioritizes_fallbacks_and_excludes_icons() {
        let mut fonts = FontDefinitions::default();
        let seed_font = fonts
            .font_data
            .values()
            .next()
            .cloned()
            .expect("expected default font data");
        let fallback_font_names = vec![
            "terminal-cascadia-mono".to_owned(),
            "terminal-consolas".to_owned(),
        ];

        for font_name in &fallback_font_names {
            fonts.font_data.insert(font_name.clone(), seed_font.clone());
        }

        let icon_font_names = super::icon_fonts()
            .iter()
            .map(|asset| asset.family.to_owned())
            .collect::<Vec<_>>();
        for font_name in &icon_font_names {
            fonts.font_data.insert(font_name.clone(), seed_font.clone());
        }
        if let Some(monospace_family) = fonts.families.get_mut(&FontFamily::Monospace) {
            for font_name in icon_font_names.iter().rev() {
                monospace_family.insert(0, font_name.clone());
            }
        }

        install_terminal_font_family(&mut fonts, &fallback_font_names);

        let terminal_family = fonts
            .families
            .get(&terminal_font_family())
            .expect("expected terminal family");
        assert_eq!(
            &terminal_family[..fallback_font_names.len()],
            fallback_font_names.as_slice()
        );
        assert!(icon_font_names
            .iter()
            .all(|font_name| !terminal_family.contains(font_name)));
        assert!(terminal_family.iter().any(|font_name| font_name == "Hack"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_terminal_font_candidates_stay_in_priority_order() {
        assert_eq!(
            super::windows_terminal_font_candidates(),
            &[
                ("terminal-cascadia-mono", "CascadiaMono.ttf"),
                ("terminal-consolas", "consola.ttf"),
            ]
        );
    }

    #[test]
    fn configure_terminal_font_family_installs_named_family() {
        let mut fonts = FontDefinitions::default();

        configure_terminal_font_family(&mut fonts);

        assert!(fonts.families.contains_key(&terminal_font_family()));
    }

    #[test]
    fn terminal_line_height_uses_terminal_font_family_metrics() {
        let ctx = Context::default();
        let mut fonts = FontDefinitions::default();
        configure_terminal_font_family(&mut fonts);
        ctx.set_fonts(fonts);

        let mut observed = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let font_id = terminal_font_id(ui.style());
                observed = Some(terminal_line_height(ui, &font_id));
            });
        });

        assert!(observed.is_some_and(|height| height >= 1.0));
    }

    #[test]
    fn directory_index_loading_label_cycles_dot_animation() {
        assert_eq!(super::directory_index_loading_label(0.0), "Indexing files");
        assert_eq!(
            super::directory_index_loading_label(0.25),
            "Indexing files."
        );
        assert_eq!(
            super::directory_index_loading_label(0.50),
            "Indexing files.."
        );
        assert_eq!(
            super::directory_index_loading_label(0.75),
            "Indexing files..."
        );
        assert_eq!(super::directory_index_loading_label(1.0), "Indexing files");
    }

    #[test]
    fn symlinked_directories_do_not_recurse() {
        assert!(super::should_descend_into_directory(true, false));
        assert!(!super::should_descend_into_directory(true, true));
        assert!(!super::should_descend_into_directory(false, false));
    }

    #[test]
    fn terminal_grid_dimensions_use_measured_cell_metrics_without_old_floor() {
        let (cols, lines) = terminal_grid_dimensions(egui::vec2(912.0, 544.0), 7.25, 14.0);

        assert_eq!(cols, 125);
        assert_eq!(lines, 38);
    }

    #[test]
    fn terminal_grid_dimensions_expand_when_average_width_is_narrower() {
        let (cols, _) = terminal_grid_dimensions(egui::vec2(912.0, 544.0), 7.0, 14.0);

        assert_eq!(cols, 130);
    }

    #[test]
    fn force_terminal_pane_width_expands_ui_to_requested_right_edge() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let mut observed = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(360.0, 220.0));
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                egui::Frame::none()
                    .inner_margin(egui::Margin::same(2.0))
                    .show(&mut child, |ui| {
                        let pane_right = force_terminal_pane_width(ui, 320.0);
                        observed = Some((pane_right, ui.min_rect().right()));
                    });
            });
        });

        let (pane_right, min_right) = observed.expect("pane width was not observed");
        assert!(min_right >= pane_right);
    }

    #[test]
    fn directory_file_row_uses_full_available_width() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let mut observed_width = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(320.0, 80.0));
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let response = super::draw_directory_file_row(&mut child, "src/app.rs");
                observed_width = Some(response.rect.width());
            });
        });

        let observed_width = observed_width.expect("directory row width was not observed");
        assert_eq!(observed_width, 320.0);
    }

    #[test]
    fn directory_folder_row_uses_full_available_width() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let mut observed_width = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(320.0, 80.0));
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Center)),
                );
                let response = super::draw_directory_folder_row(&mut child, "src");
                observed_width = Some(response.rect.width());
            });
        });

        let observed_width = observed_width.expect("directory row width was not observed");
        assert_eq!(observed_width, 320.0);
    }

    #[test]
    fn source_control_file_row_uses_full_available_width() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let mut observed_width = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(320.0, 80.0));
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Center)),
                );
                let response = super::draw_source_control_file_row(
                    &mut child,
                    super::icons::CHECK_CIRCLE,
                    "Modified src/app.rs",
                );
                observed_width = Some(response.rect.width());
            });
        });

        let observed_width = observed_width.expect("source control row width was not observed");
        assert_eq!(observed_width, 320.0);
    }

    #[test]
    fn sidebar_row_wrap_width_reserves_shared_leading_inset() {
        let wrap_width = super::sidebar_row_wrap_width(160.0, egui::vec2(8.0, 4.0));

        assert_eq!(
            wrap_width,
            160.0 - (8.0 * 2.0) - super::SIDEBAR_ROW_LEADING_INSET
        );
    }

    #[test]
    fn directory_row_text_position_left_aligns_with_padding() {
        let rect = egui::Rect::from_min_size(pos2(32.0, 10.0), egui::vec2(240.0, 28.0));
        let button_padding = egui::vec2(8.0, 4.0);
        let galley_size = egui::vec2(56.0, 12.0);

        let text_pos = super::directory_row_text_position(rect, button_padding, galley_size);
        let content_rect = rect.shrink2(button_padding);

        assert_eq!(
            text_pos.x,
            content_rect.min.x + super::SIDEBAR_ROW_LEADING_INSET
        );
        assert_eq!(text_pos.y, content_rect.center().y - (galley_size.y * 0.5));
    }

    #[test]
    fn directory_file_row_hover_fill_is_translucent_and_only_present_on_hover() {
        assert_eq!(super::directory_file_row_hover_fill(false), None);
        assert_eq!(
            super::directory_file_row_hover_fill(true),
            Some(super::with_alpha(super::BTN_ICON_HOVER, 110))
        );
    }

    #[test]
    fn terminal_header_chrome_emphasizes_active_terminal() {
        let chrome = super::terminal_header_chrome(true);

        assert_eq!(chrome.fill, Color32::from_rgb(28, 52, 72));
        assert_eq!(chrome.stroke, Stroke::NONE);
        assert_eq!(chrome.title_color, Color32::from_rgb(244, 251, 255));
        assert_eq!(
            chrome.detail_color,
            super::with_alpha(super::TEXT_MUTED, 238)
        );
    }

    #[test]
    fn terminal_header_chrome_keeps_inactive_terminal_subtle() {
        let chrome = super::terminal_header_chrome(false);

        assert_eq!(chrome.fill, Color32::from_rgb(22, 32, 46));
        assert_eq!(chrome.stroke, Stroke::new(1.0, super::BORDER_COLOR));
        assert_eq!(chrome.title_color, super::TEXT_PRIMARY);
        assert_eq!(
            chrome.detail_color,
            super::with_alpha(super::TEXT_MUTED, 230)
        );
    }

    #[test]
    fn terminal_manager_row_chrome_emphasizes_active_terminal() {
        let chrome = terminal_manager_row_chrome(true, false);

        assert_eq!(chrome.fill, Some(Color32::from_rgb(24, 48, 68)));
        assert_eq!(
            chrome.stroke,
            Stroke::new(1.0, super::with_alpha(super::ACCENT, 220))
        );
        assert_eq!(chrome.title_color, Color32::from_rgb(244, 251, 255));
    }

    #[test]
    fn terminal_manager_row_chrome_keeps_inactive_terminal_subtle() {
        let chrome = terminal_manager_row_chrome(false, false);

        assert_eq!(chrome.fill, None);
        assert_eq!(chrome.stroke, Stroke::NONE);
        assert_eq!(
            chrome.title_color,
            super::with_alpha(super::TEXT_PRIMARY, 210)
        );
    }

    #[test]
    fn default_app_open_command_matches_platform_convention() {
        let path = PathBuf::from("C:\\temp\\notes.txt");
        let (program, args) = default_app_open_command(&path);
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        #[cfg(target_os = "windows")]
        {
            assert_eq!(program, "cmd");
            assert_eq!(args, vec!["/C", "start", "", "C:\\temp\\notes.txt"]);
        }

        #[cfg(target_os = "macos")]
        {
            assert_eq!(program, "open");
            assert_eq!(args, vec!["C:\\temp\\notes.txt"]);
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            assert_eq!(program, "xdg-open");
            assert_eq!(args, vec!["C:\\temp\\notes.txt"]);
        }
    }

    #[test]
    fn file_explorer_command_matches_platform_convention() {
        let path = PathBuf::from("/tmp/notes.txt");
        let (program, args) = super::file_explorer_command(&path, true);
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        #[cfg(target_os = "windows")]
        {
            assert_eq!(program, "explorer.exe");
            assert_eq!(args, vec!["/select,", "/tmp/notes.txt"]);
        }

        #[cfg(target_os = "macos")]
        {
            assert_eq!(program, "open");
            assert_eq!(args, vec!["-R", "/tmp/notes.txt"]);
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            assert_eq!(program, "xdg-open");
            assert_eq!(args, vec!["/tmp"]);
        }
    }

    #[test]
    fn terminal_output_viewport_size_matches_requested_output() {
        let viewport = terminal_output_viewport_size(egui::vec2(912.0, 544.0));

        assert_eq!(viewport, egui::vec2(912.0, 544.0));
    }

    #[test]
    fn terminal_output_surface_size_keeps_viewport_width_for_short_content() {
        let surface = terminal_output_surface_size(egui::vec2(912.0, 544.0), 120.0);

        assert_eq!(surface, egui::vec2(912.0, 544.0));
    }

    #[test]
    fn terminal_output_surface_size_grows_only_for_tall_content() {
        let surface = terminal_output_surface_size(egui::vec2(912.0, 544.0), 700.0);

        assert_eq!(surface, egui::vec2(912.0, 700.0));
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
            &terminal_font_id(&egui::Style::default()),
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

    #[test]
    fn ai_badge_visuals_match_status() {
        assert_eq!(
            ai_badge_visual(AiCliStatus::Running),
            Some(AiBadgeVisual::Spinner(Color32::from_rgb(76, 209, 114)))
        );
        assert_eq!(
            ai_badge_visual(AiCliStatus::Attention),
            Some(AiBadgeVisual::Pulse(Color32::from_rgb(46, 130, 255)))
        );
        assert_eq!(ai_badge_visual(AiCliStatus::Inactive), None);
    }

    #[test]
    fn draw_ai_badge_hides_inactive_and_reserves_space_for_active_states() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let running_badge = AiBadgeModel {
            tool: Some(AiCliTool::FactoryDroid),
            status: AiCliStatus::Running,
            tooltip_lines: vec!["Factory Droid - Working...".to_string()],
        };
        let attention_badge = AiBadgeModel {
            tool: Some(AiCliTool::FactoryDroid),
            status: AiCliStatus::Attention,
            tooltip_lines: vec!["Factory Droid - Waiting for you...".to_string()],
        };
        let inactive_badge = AiBadgeModel {
            tool: Some(AiCliTool::FactoryDroid),
            status: AiCliStatus::Inactive,
            tooltip_lines: vec!["Factory Droid - Idle".to_string()],
        };

        let mut running_size = None;
        let mut attention_size = None;
        let mut inactive_size = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                running_size = Some(draw_ai_badge(ui, &running_badge).rect.size());
                attention_size = Some(draw_ai_badge(ui, &attention_badge).rect.size());
                inactive_size = Some(draw_ai_badge(ui, &inactive_badge).rect.size());
            });
        });

        assert_eq!(running_size, Some(egui::vec2(16.0, 16.0)));
        assert_eq!(attention_size, Some(egui::vec2(16.0, 16.0)));
        assert_eq!(inactive_size, Some(egui::vec2(0.0, 0.0)));
    }

    #[test]
    fn draw_terminal_status_badges_places_ai_before_following_content_when_active() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let running_badge = AiBadgeModel {
            tool: Some(AiCliTool::FactoryDroid),
            status: AiCliStatus::Running,
            tooltip_lines: vec!["Factory Droid - Working...".to_string()],
        };

        let mut observed = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let layout = draw_terminal_status_badges(ui, &running_badge);
                    let title_rect = ui.label("terminal title").rect;
                    observed = Some((
                        layout.ai_rect.expect("expected visible ai badge"),
                        title_rect,
                    ));
                });
            });
        });

        let (ai_rect, title_rect) = observed.expect("expected badge layout");
        assert!(ai_rect.center().x < title_rect.min.x);
    }

    #[test]
    fn draw_terminal_status_badges_does_not_leave_leading_gap_when_ai_is_inactive() {
        let ctx = Context::default();
        ctx.set_fonts(FontDefinitions::default());

        let inactive_badge = AiBadgeModel {
            tool: Some(AiCliTool::FactoryDroid),
            status: AiCliStatus::Inactive,
            tooltip_lines: vec!["Factory Droid - Idle".to_string()],
        };

        let mut observed = None;
        let mut direct_x = None;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let layout = draw_terminal_status_badges(ui, &inactive_badge);
                    observed = Some((layout.ai_rect, ui.label("terminal title").rect.min.x));
                });
            });
        });
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    direct_x = Some(ui.label("terminal title").rect.min.x);
                });
            });
        });

        let (ai_rect, title_x) = observed.expect("expected badge layout");
        assert_eq!(ai_rect, None);
        assert!((title_x - direct_x.expect("expected direct title")).abs() < 0.5);
    }

    #[test]
    fn factory_droid_transport_diagnostics_report_ready_runtime_dir() {
        let diagnostics = FactoryDroidTransportDiagnostics {
            hooks_enabled: true,
            executable_path: PathBuf::from(r"C:\Users\furkan.cakir\Desktop\mergen-ade-new.exe"),
            hooks_runtime_dir: Some(PathBuf::from(
                r"C:\Users\furkan.cakir\AppData\Roaming\Mergen\MergenADE\config\runtime\factory-droid-hooks",
            )),
            hooks_runtime_error: None,
            active_session: Some(true),
            process_state: Some("active".to_owned()),
            last_status_source: Some(FactoryDroidStatusSource::PromptSubmit),
        };

        assert_eq!(
            diagnostics.runtime_status_text(),
            "Ready: C:\\Users\\furkan.cakir\\AppData\\Roaming\\Mergen\\MergenADE\\config\\runtime\\factory-droid-hooks"
        );
        assert_eq!(diagnostics.active_session_text(), "Yes");
        assert_eq!(diagnostics.process_state_text(), "active");
        assert_eq!(diagnostics.last_status_source_text(), "prompt_submit");
        assert_eq!(diagnostics.warning_message(), None);
    }

    #[test]
    fn factory_droid_transport_diagnostics_warn_when_runtime_dir_is_unavailable() {
        let diagnostics = FactoryDroidTransportDiagnostics {
            hooks_enabled: true,
            executable_path: PathBuf::from(r"C:\Users\furkan.cakir\Desktop\mergen-ade-new.exe"),
            hooks_runtime_dir: None,
            hooks_runtime_error: Some("Access denied".to_owned()),
            active_session: Some(false),
            process_state: Some("missing (grace)".to_owned()),
            last_status_source: None,
        };

        assert_eq!(
            diagnostics.runtime_status_text(),
            "Unavailable: Access denied"
        );
        assert_eq!(diagnostics.active_session_text(), "No");
        assert_eq!(diagnostics.process_state_text(), "missing (grace)");
        assert_eq!(diagnostics.last_status_source_text(), "none");
        assert_eq!(
            diagnostics.warning_message(),
            Some("Factory Droid inbox fallback unavailable: Access denied".to_owned())
        );
    }

    #[test]
    fn factory_droid_transport_diagnostics_hide_warnings_when_hooks_are_disabled() {
        let diagnostics = FactoryDroidTransportDiagnostics {
            hooks_enabled: false,
            executable_path: PathBuf::from(r"C:\Users\furkan.cakir\Desktop\mergen-ade-new.exe"),
            hooks_runtime_dir: None,
            hooks_runtime_error: Some("Access denied".to_owned()),
            active_session: None,
            process_state: None,
            last_status_source: None,
        };

        assert_eq!(diagnostics.runtime_status_text(), "Disabled");
        assert_eq!(diagnostics.warning_message(), None);
    }

    #[test]
    fn disabled_ai_hooks_do_not_create_manager() {
        let mut config = AppConfig::default();
        config.ai_hooks.global_enabled = false;

        assert!(AdeApp::ai_hook_manager_from_config(&config).is_none());
    }
}
