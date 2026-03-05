use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Color32, Event, FontData, FontFamily, FontId, Key, Layout, RichText, Sense,
    Stroke, TextWrapMode, Ui, Vec2,
};
use iconflow::{fonts as icon_fonts, try_icon, Pack, Size, Style};

use crate::config;
use crate::layout;
use crate::models::{
    AppConfig, AutoTileScope, LeftSidebarTab, MainVisibilityMode, ProjectRecord, ShellKind,
    TerminalKind,
};
use crate::terminal::{
    try_terminal_snapshot, TerminalColor, TerminalDimensions, TerminalRuntime, TerminalSnapshot,
    TerminalUiEvent, TerminalUiEventKind,
};
use crate::title::update_terminal_title;

const CELL_WIDTH_PX: f32 = 8.0;
const CELL_HEIGHT_PX: f32 = 16.0;
const TITLE_MAX_LEN: usize = 40;
const TERMINAL_EVENT_BUDGET: usize = 4096;
const TERMINAL_RETRY_MS: u64 = 8;
const TERMINAL_FALLBACK_REFRESH_MS: u64 = 16;
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
    const ALL: [Self; 19] = [
        Self::ArrowClockwise,
        Self::ChatText,
        Self::CheckCircle,
        Self::Clock,
        Self::Copy,
        Self::Download,
        Self::Eye,
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
    projects: BTreeMap<u64, ProjectRecord>,
    terminals: BTreeMap<u64, TerminalEntry>,
    next_project_id: u64,
    next_terminal_id: u64,
    selected_project: Option<u64>,
    active_terminal: Option<u64>,
    terminal_events_tx: Sender<TerminalUiEvent>,
    terminal_events_rx: Receiver<TerminalUiEvent>,
    show_settings_popup: bool,
    saved_message_drafts: BTreeMap<u64, String>,
    status_line: String,
    layout_epoch: u64,
    theme_initialized: bool,
    source_control_events_tx: Sender<SourceControlEvent>,
    source_control_events_rx: Receiver<SourceControlEvent>,
    source_control_state: BTreeMap<u64, SourceControlSnapshot>,
}

struct TerminalEntry {
    id: u64,
    project_id: u64,
    kind: TerminalKind,
    title: String,
    pending_line_for_title: String,
    in_main_view: bool,
    dirty: bool,
    last_seqno: usize,
    render_cache: TerminalSnapshot,
    exited: bool,
    runtime: TerminalRuntime,
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

impl AdeApp {
    pub fn bootstrap() -> Self {
        let config_path = config::config_path().unwrap_or_else(|_| PathBuf::from("config.toml"));
        let mut config = config::load_config(&config_path).unwrap_or_default();
        config.ui.main_visibility_mode = MainVisibilityMode::Global;
        config.ui.project_filter_mode = false;

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

        Self {
            config_path,
            config,
            projects,
            terminals: BTreeMap::new(),
            next_project_id,
            next_terminal_id: 1,
            selected_project,
            active_terminal: None,
            terminal_events_tx,
            terminal_events_rx,
            show_settings_popup: false,
            saved_message_drafts: BTreeMap::new(),
            status_line: "Ready".to_owned(),
            layout_epoch: 0,
            theme_initialized: false,
            source_control_events_tx,
            source_control_events_rx,
            source_control_state: BTreeMap::new(),
        }
    }

    fn persist_config(&mut self) {
        self.config.projects = self.projects.values().cloned().collect();
        self.config.ui.last_selected_project_id = self.selected_project;

        if let Err(err) = config::save_config(&self.config_path, &self.config) {
            self.status_line = format!("Config save error: {err}");
        }
    }

    fn bump_layout_epoch(&mut self) {
        self.layout_epoch = self.layout_epoch.wrapping_add(1);
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
        self.bump_layout_epoch();
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
            title: fallback_title,
            pending_line_for_title: String::new(),
            in_main_view: true,
            dirty: true,
            last_seqno: runtime.latest_seqno(),
            render_cache: TerminalSnapshot::default(),
            exited: false,
            runtime,
        };

        self.active_terminal = Some(terminal_id);
        self.terminals.insert(terminal_id, entry);
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

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let _ = ctx;
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

    fn route_active_terminal_input(&mut self, ctx: &egui::Context) {
        if self.show_settings_popup && ctx.wants_keyboard_input() {
            return;
        }

        let Some(active_terminal_id) = self.active_terminal else {
            return;
        };

        let can_receive_input = self
            .terminals
            .get(&active_terminal_id)
            .is_some_and(|terminal| self.terminal_visible_in_main(terminal) && !terminal.exited);
        if !can_receive_input {
            return;
        }

        let events = ctx.input(|input| input.events.clone());
        if events.is_empty() {
            return;
        }

        let Some(terminal) = self.terminals.get_mut(&active_terminal_id) else {
            return;
        };

        let mut outbound = Vec::new();

        for event in events {
            match event {
                Event::Copy => {
                    outbound.push(0x03);
                }
                Event::Text(text) => {
                    if text.is_empty() {
                        continue;
                    }
                    outbound.extend_from_slice(text.as_bytes());
                    Self::append_pending_line(&mut terminal.pending_line_for_title, &text);
                }
                Event::Paste(text) => {
                    if text.is_empty() {
                        continue;
                    }
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
                        outbound.push(b'\r');
                        let line = std::mem::take(&mut terminal.pending_line_for_title);
                        terminal.title =
                            update_terminal_title(&line, terminal.id as usize, TITLE_MAX_LEN);
                        terminal.dirty = true;
                        continue;
                    }

                    if key == Key::Backspace {
                        terminal.pending_line_for_title.pop();
                    }

                    if let Some(bytes) = Self::key_to_terminal_bytes(key, modifiers) {
                        outbound.extend_from_slice(&bytes);
                    }
                }
                _ => {}
            }
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
        visuals.panel_fill = APP_BG;
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

    fn append_pending_line(pending: &mut String, text: &str) {
        for ch in text.chars() {
            if ch == '\r' || ch == '\n' {
                pending.clear();
                continue;
            }
            pending.push(ch);
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

    fn close_terminal(&mut self, terminal_id: u64) {
        let Some(terminal) = self.terminals.remove(&terminal_id) else {
            return;
        };

        terminal.runtime.shutdown();
        self.status_line = format!("Closed {}", terminal.title);

        if self.active_terminal == Some(terminal_id) {
            self.active_terminal = self.terminals.keys().next().copied();
        }
        self.bump_layout_epoch();
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

        terminal.runtime.send_bytes(message.as_bytes().to_vec());
        Self::append_pending_line(&mut terminal.pending_line_for_title, message);
        terminal.dirty = true;
        self.status_line = format!("Sent saved message to {}", terminal.title);
    }

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar")
            .exact_height(54.0)
            .frame(
                egui::Frame::none()
                    .fill(SURFACE_BG)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}  Mergen ADE", icons::TERMINAL_WINDOW))
                            .strong()
                            .size(15.0)
                            .color(ACCENT),
                    );
                    ui.add_space(6.0);

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
                });
            });
    }

    fn draw_project_explorer(&mut self, ctx: &egui::Context) {
        if !self.config.ui.show_project_explorer {
            return;
        }

        egui::SidePanel::left("project_explorer")
            .resizable(true)
            .min_width(220.0)
            .max_width(540.0)
            .default_width(250.0)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(SURFACE_BG)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        let previous_tab = self.config.ui.left_sidebar_tab;
                        ui.horizontal(|ui| {
                            if styled_icon_toggle(
                                ui,
                                self.config.ui.left_sidebar_tab == LeftSidebarTab::Directory,
                                icons::TREE_VIEW,
                                LeftSidebarTab::Directory.label(),
                            ) {
                                self.config.ui.left_sidebar_tab = LeftSidebarTab::Directory;
                            }
                            if styled_icon_toggle(
                                ui,
                                self.config.ui.left_sidebar_tab == LeftSidebarTab::SourceControl,
                                icons::GIT_BRANCH,
                                LeftSidebarTab::SourceControl.label(),
                            ) {
                                self.config.ui.left_sidebar_tab = LeftSidebarTab::SourceControl;
                            }
                            if self.config.ui.left_sidebar_tab == LeftSidebarTab::Directory
                                && styled_icon_button(
                                    ui,
                                    icons::FOLDER_PLUS,
                                    BTN_TEAL,
                                    BTN_TEAL_HOVER,
                                    BTN_ICON_ACTIVE,
                                    "Add Project",
                                )
                            {
                                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                    self.add_project(path);
                                }
                            }
                        });
                        if previous_tab != self.config.ui.left_sidebar_tab {
                            self.persist_config();
                        }
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
                                            .find(|(project_id, _, _, _)| {
                                                *project_id == selected_id
                                            })
                                            .map(|(_, project_name, _, _)| {
                                                format!("{} {}", icons::FOLDER_OPEN, project_name)
                                            })
                                    })
                                    .unwrap_or_else(|| "No project selected".to_owned());

                                let previous_selected_project = self.selected_project;
                                egui::ComboBox::from_label("Project")
                                    .selected_text(selected_project_label)
                                    .width(220.0)
                                    .show_ui(ui, |ui| {
                                        for (project_id, project_name, _, _) in &project_rows {
                                            ui.selectable_value(
                                                &mut self.selected_project,
                                                Some(*project_id),
                                                format!("{} {}", icons::FOLDER, project_name),
                                            );
                                        }
                                    });
                                if self.selected_project != previous_selected_project {
                                    self.persist_config();
                                }

                                if let Some(selected_id) = self.selected_project {
                                    if let Some((
                                        _,
                                        project_name,
                                        project_path,
                                        project_path_text,
                                    )) = project_rows
                                        .iter()
                                        .find(|(project_id, _, _, _)| *project_id == selected_id)
                                        .cloned()
                                    {
                                        ui.horizontal(|ui| {
                                            if styled_pill_button(
                                                ui,
                                                icons::COPY,
                                                "Copy Path",
                                                BTN_SUBTLE,
                                                BTN_SUBTLE_HOVER,
                                            ) {
                                                ui.ctx().copy_text(project_path_text.clone());
                                                self.status_line = format!(
                                                    "Copied path for project '{}'",
                                                    project_name
                                                );
                                            }
                                            if styled_pill_button(
                                                ui,
                                                icons::FOLDER_OPEN,
                                                "Open in Folder",
                                                BTN_SUBTLE,
                                                BTN_SUBTLE_HOVER,
                                            ) {
                                                match open_in_file_explorer(&project_path, false) {
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
                                        });
                                    }
                                }

                                ui.separator();

                                egui::ScrollArea::vertical()
                                    .id_salt("directory-tree-scroll")
                                    .max_height(ui.available_height())
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        if let Some(project_id) = self.selected_project {
                                            if let Some(project) = self.projects.get(&project_id) {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} Files",
                                                        icons::FOLDER_OPEN
                                                    ))
                                                    .color(TEXT_MUTED)
                                                    .strong(),
                                                );
                                                draw_folder_tree(
                                                    ui,
                                                    &project.path,
                                                    0,
                                                    8,
                                                    &mut self.status_line,
                                                );
                                            }
                                        } else {
                                            ui.label(
                                                RichText::new("No project selected")
                                                    .color(TEXT_MUTED),
                                            );
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

                                let previous_selected_project = self.selected_project;
                                egui::ComboBox::from_label("Project")
                                    .selected_text(selected_project_label)
                                    .width(220.0)
                                    .show_ui(ui, |ui| {
                                        for (project_id, project_name) in &project_rows {
                                            ui.selectable_value(
                                                &mut self.selected_project,
                                                Some(*project_id),
                                                format!("{} {}", icons::FOLDER, project_name),
                                            );
                                        }
                                    });
                                if self.selected_project != previous_selected_project {
                                    should_persist_selection = true;
                                }
                                if should_persist_selection {
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

                                ui.horizontal(|ui| {
                                    if styled_icon_button(
                                        ui,
                                        icons::ARROW_CLOCKWISE,
                                        BTN_ICON,
                                        BTN_ICON_HOVER,
                                        BTN_ICON_ACTIVE,
                                        "Refresh Status",
                                    ) {
                                        self.request_source_control_refresh(project_id, false);
                                    }
                                    if styled_icon_button(
                                        ui,
                                        icons::DOWNLOAD,
                                        BTN_ICON,
                                        BTN_ICON_HOVER,
                                        BTN_ICON_ACTIVE,
                                        "Fetch and Refresh",
                                    ) {
                                        self.request_source_control_refresh(project_id, true);
                                    }
                                    if styled_icon_button(
                                        ui,
                                        icons::FOLDER_OPEN,
                                        BTN_ICON,
                                        BTN_ICON_HOVER,
                                        BTN_ICON_ACTIVE,
                                        "Open Project Folder",
                                    ) {
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
                                });
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
                                        }
                                    });
                            }
                        }
                    });
            });
    }

    fn draw_terminal_manager(&mut self, ctx: &egui::Context) {
        if !self.config.ui.show_terminal_manager {
            return;
        }

        egui::SidePanel::left("terminal_manager")
            .resizable(true)
            .min_width(180.0)
            .max_width(620.0)
            .default_width(280.0)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(SURFACE_BG)
                    .stroke(Stroke::new(1.0, BORDER_COLOR))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} Terminal Manager", icons::TERMINAL_WINDOW))
                                .strong()
                                .size(15.0),
                        );
                        ui.separator();

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

                            let Some(project_snapshot) = self.projects.get(&project_id).cloned()
                            else {
                                continue;
                            };

                            let project_path = project_snapshot.path.display().to_string();

                            let header_label =
                                format!("{} {}", icons::FOLDER_OPEN, project_snapshot.name);
                            let header = egui::CollapsingHeader::new(header_label)
                                .id_salt(format!("project-group-{project_id}"))
                                .default_open(true)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if styled_icon_button(
                                            ui,
                                            icons::TERMINAL,
                                            BTN_BLUE,
                                            BTN_BLUE_HOVER,
                                            BTN_ICON_ACTIVE,
                                            "New Foreground Terminal",
                                        ) {
                                            self.spawn_terminal_for_project(
                                                ctx,
                                                project_id,
                                                TerminalKind::Foreground,
                                            );
                                        }
                                        if styled_icon_button(
                                            ui,
                                            icons::LIST,
                                            BTN_TEAL,
                                            BTN_TEAL_HOVER,
                                            BTN_ICON_ACTIVE,
                                            "New Background Terminal",
                                        ) {
                                            self.spawn_terminal_for_project(
                                                ctx,
                                                project_id,
                                                TerminalKind::Background,
                                            );
                                        }
                                    });

                                    ui.separator();
                                    ui.label(
                                        RichText::new(format!("{} Foreground", icons::TERMINAL))
                                            .strong()
                                            .color(TEXT_MUTED),
                                    );
                                    self.draw_terminal_rows(
                                        ui,
                                        project_id,
                                        TerminalKind::Foreground,
                                    );

                                    ui.separator();
                                    ui.label(
                                        RichText::new(format!("{} Background", icons::LIST))
                                            .strong()
                                            .color(TEXT_MUTED),
                                    );
                                    self.draw_terminal_rows(
                                        ui,
                                        project_id,
                                        TerminalKind::Background,
                                    );
                                });

                            header.header_response.context_menu(|ui| {
                                if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                                    ui.ctx().copy_text(project_path.clone());
                                    self.status_line = format!(
                                        "Copied path for project '{}'",
                                        project_snapshot.name
                                    );
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
                        }
                    });
            });
    }

    fn draw_terminal_rows(&mut self, ui: &mut Ui, project_id: u64, kind: TerminalKind) {
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

                ui.horizontal(|ui| {
                    let active = current_active == Some(terminal_entry_id);
                    let label = if terminal.exited {
                        format!("{} {} (Exited)", icons::TERMINAL, terminal.title)
                    } else {
                        format!("{} {}", icons::TERMINAL, terminal.title)
                    };

                    if ui.selectable_label(active, label).clicked() {
                        set_active = true;
                    }

                    let message_menu = ui.menu_button(format!("{}", icons::CHAT_TEXT), |ui| {
                        if saved_messages.is_empty() {
                            ui.label(RichText::new("No saved messages").color(TEXT_MUTED));
                            return;
                        }

                        for message in &saved_messages {
                            if ui.button(message).clicked() {
                                send_message = Some(message.clone());
                                ui.close_menu();
                            }
                        }
                    });
                    message_menu.response.on_hover_text("Send saved message");

                    if styled_icon_toggle(
                        ui,
                        terminal.in_main_view,
                        icons::EYE,
                        "Show in main area",
                    ) {
                        terminal.in_main_view = !terminal.in_main_view;
                        visibility_changed = true;
                    }
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
                });

                terminal_entry_id
            };

            if let Some(message) = send_message {
                self.send_saved_message_to_terminal(terminal_entry_id, &message);
            }

            if visibility_changed {
                self.bump_layout_epoch();
            }
            if set_active {
                self.active_terminal = Some(terminal_entry_id);
            }
            if close_terminal {
                self.close_terminal(terminal_entry_id);
            }
        }
    }

    fn draw_main_area(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let visible_ids = self.visible_terminal_ids_for_main();

            if visible_ids.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!("{}  No visible terminals", icons::TERMINAL))
                                .size(20.0)
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Select a project, then use New FG/New BG to start.")
                                .color(TEXT_MUTED),
                        );
                    });
                });
                return;
            }

            let available = ui.available_size();
            if available.x < 160.0 || available.y < 120.0 {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Expand the window to render terminals").color(TEXT_MUTED),
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

    fn draw_terminal_pane(&mut self, ui: &mut Ui, terminal_id: u64, pane_size: Vec2) {
        let project_name = self
            .terminals
            .get(&terminal_id)
            .and_then(|terminal| self.projects.get(&terminal.project_id))
            .map(|project| project.name.clone())
            .unwrap_or_else(|| "Unknown Project".to_owned());
        let is_active = self.active_terminal == Some(terminal_id);

        let (clicked, close_requested) = {
            let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
                return;
            };

            let mut close_requested = false;
            let mut pane_clicked = false;
            let kind_fill = match terminal.kind {
                TerminalKind::Foreground => Color32::from_rgb(18, 90, 140),
                TerminalKind::Background => Color32::from_rgb(110, 76, 20),
            };
            let header_fill = if is_active {
                Color32::from_rgb(28, 48, 68)
            } else {
                Color32::from_rgb(22, 32, 46)
            };
            let header_stroke = if is_active {
                Stroke::new(1.5, ACCENT)
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

                        let indicator = if is_active { "●" } else { "○" };
                        let indicator_color = if is_active { ACCENT } else { TEXT_MUTED };
                        ui.label(RichText::new(indicator).color(indicator_color).size(10.0));
                        let title = format!("{} {}", icons::TERMINAL, terminal.title);
                        let title_response = ui.add(
                            egui::Label::new(RichText::new(title).color(TEXT_PRIMARY))
                                .truncate()
                                .sense(Sense::click()),
                        );
                        if title_response.clicked() {
                            pane_clicked = true;
                        }

                        ui.add_space(6.0);
                        ui.label(RichText::new("│").color(BORDER_COLOR).size(12.0));
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{} {}", icons::FOLDER, project_name))
                                    .color(TEXT_MUTED),
                            )
                            .truncate(),
                        );
                        ui.add_space(4.0);
                        ui.label(RichText::new("│").color(BORDER_COLOR).size(12.0));
                        ui.add_space(4.0);
                        egui::Frame::none()
                            .fill(kind_fill)
                            .rounding(6.0)
                            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(terminal.kind.label())
                                        .small()
                                        .color(Color32::from_rgb(225, 243, 255)),
                                );
                            });
                        if terminal.exited {
                            ui.colored_label(Color32::LIGHT_RED, "Exited");
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if styled_pill_button(ui, icons::X, "Close", BTN_RED, BTN_RED_HOVER) {
                                close_requested = true;
                            }
                        });
                    });
            });
            ui.add_space(TERMINAL_HEADER_GAP);

            let monospace = egui::TextStyle::Monospace;
            let font_id = monospace.resolve(ui.style());
            let char_width = ui
                .fonts(|fonts| fonts.glyph_width(&font_id, 'W'))
                .max(CELL_WIDTH_PX);
            let line_height = ui.text_style_height(&monospace).max(CELL_HEIGHT_PX);

            let output_height =
                (pane_height - TERMINAL_HEADER_HEIGHT - TERMINAL_HEADER_GAP).max(line_height * 2.0);
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

            if terminal.dirty || terminal.render_cache.lines.is_empty() {
                if let Some(snapshot) = try_terminal_snapshot(&terminal.runtime) {
                    terminal.render_cache = snapshot;
                    terminal.dirty = false;
                } else {
                    ui.ctx()
                        .request_repaint_after(Duration::from_millis(TERMINAL_RETRY_MS));
                }
            }

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
                                if terminal.render_cache.lines.is_empty() {
                                    let response = ui.add(
                                        egui::Label::new(
                                            RichText::new("Terminal is resizing...")
                                                .color(TEXT_MUTED),
                                        )
                                        .sense(Sense::click()),
                                    );
                                    if response.clicked() {
                                        pane_clicked = true;
                                    }
                                } else {
                                    let render_job =
                                        build_terminal_layout_job(&terminal.render_cache, &font_id);
                                    let response = ui.add(
                                        egui::Label::new(render_job)
                                            .wrap_mode(TextWrapMode::Extend)
                                            .sense(Sense::click()),
                                    );
                                    if response.clicked() {
                                        pane_clicked = true;
                                    }
                                }
                            });
                    });
            });

            (pane_clicked, close_requested)
        };

        if close_requested {
            self.close_terminal(terminal_id);
            return;
        }

        if clicked {
            self.active_terminal = Some(terminal_id);
        }
    }

    fn draw_settings_popup(&mut self, ctx: &egui::Context) {
        if !self.show_settings_popup {
            return;
        }

        let mut should_persist = false;

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
                ui.label(
                    RichText::new("Application Settings")
                        .strong()
                        .size(16.0)
                        .color(TEXT_PRIMARY),
                );
                ui.separator();

                let mut show_explorer = self.config.ui.show_project_explorer;
                if ui
                    .checkbox(&mut show_explorer, "Show Project Explorer")
                    .changed()
                {
                    self.config.ui.show_project_explorer = show_explorer;
                    should_persist = true;
                }

                let mut show_terminal_mgr = self.config.ui.show_terminal_manager;
                if ui
                    .checkbox(&mut show_terminal_mgr, "Show Terminal Manager")
                    .changed()
                {
                    self.config.ui.show_terminal_manager = show_terminal_mgr;
                    should_persist = true;
                }

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
                    should_persist = true;
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

                    egui::CollapsingHeader::new(format!(
                        "{} {}",
                        icons::FOLDER_OPEN,
                        project_snapshot.name
                    ))
                    .id_salt(format!("settings-saved-messages-{project_id}"))
                    .default_open(self.selected_project == Some(project_id))
                    .show(ui, |ui| {
                        if project_snapshot.saved_messages.is_empty() {
                            ui.label(
                                RichText::new("No saved messages for this project.")
                                    .color(TEXT_MUTED),
                            );
                        }

                        for (index, message) in project_snapshot.saved_messages.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(message).monospace().small());
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
                            });
                        }

                        ui.horizontal(|ui| {
                            let draft = self.saved_message_drafts.entry(project_id).or_default();
                            ui.text_edit_singleline(draft);
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
                        }
                        if let Some(index) = remove_message_index {
                            if index < project.saved_messages.len() {
                                project.saved_messages.remove(index);
                                should_persist = true;
                            }
                        }
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if styled_pill_button(ui, icons::X, "Close", BTN_SUBTLE, BTN_SUBTLE_HOVER) {
                        self.show_settings_popup = false;
                    }
                });
            });

        if should_persist {
            self.persist_config();
        }
    }
}

impl eframe::App for AdeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_theme_initialized(ctx);
        self.process_terminal_events(ctx);
        self.process_source_control_events(ctx);
        self.schedule_terminal_refresh(ctx);
        self.handle_shortcuts(ctx);

        self.draw_top_bar(ctx);
        self.draw_project_explorer(ctx);
        self.draw_terminal_manager(ctx);
        self.draw_main_area(ctx);
        self.draw_settings_popup(ctx);

        self.route_active_terminal_input(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for terminal in self.terminals.values() {
            terminal.runtime.shutdown();
        }

        self.persist_config();
    }
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
    path: &Path,
    depth: usize,
    max_depth: usize,
    status_line: &mut String,
) {
    if depth > max_depth {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    let mut items = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<PathBuf>>();

    items.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    for item in items {
        let name = item
            .file_name()
            .map(|segment| segment.to_string_lossy().to_string())
            .unwrap_or_else(|| item.display().to_string());
        let item_path_text = item.display().to_string();

        if item.is_dir() {
            let header = egui::CollapsingHeader::new(name)
                .id_salt(item.display().to_string())
                .show(ui, |ui| {
                    draw_folder_tree(ui, &item, depth + 1, max_depth, status_line)
                });
            header.header_response.context_menu(|ui| {
                if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                    ui.ctx().copy_text(item_path_text.clone());
                    *status_line = format!("Copied path: {}", item_path_text);
                    ui.close_menu();
                }
            });
        } else {
            ui.label(name).context_menu(|ui| {
                if ui.button(format!("{} Copy Path", icons::COPY)).clicked() {
                    ui.ctx().copy_text(item_path_text.clone());
                    *status_line = format!("Copied path: {}", item_path_text);
                    ui.close_menu();
                }
            });
        }
    }
}

fn styled_pill_button(
    ui: &mut Ui,
    icon: AppIcon,
    label: &str,
    bg: Color32,
    hover_bg: Color32,
) -> bool {
    let text = format!("{} {}", icon, label);
    let frame_response = egui::Frame::none()
        .fill(bg)
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .color(Color32::from_rgb(230, 240, 255))
                        .size(13.0),
                )
                .sense(Sense::click()),
            )
        });

    let label_response = frame_response.inner;
    let is_hovered = label_response.hovered() || frame_response.response.hovered();

    // Repaint with hover color if hovered
    if is_hovered {
        ui.painter()
            .rect_filled(frame_response.response.rect, 8.0, hover_bg);
        // Re-draw text on top
        let text_pos = frame_response.response.rect.min + egui::vec2(10.0, 5.0);
        ui.painter().text(
            text_pos,
            egui::Align2::LEFT_TOP,
            format!("{} {}", icon, label),
            egui::FontId::proportional(13.0),
            Color32::from_rgb(230, 240, 255),
        );
    }

    label_response.clicked()
}

fn styled_icon_button(
    ui: &mut Ui,
    icon: AppIcon,
    bg: Color32,
    hover_bg: Color32,
    active_bg: Color32,
    tooltip: &str,
) -> bool {
    let button = egui::Button::new(
        RichText::new(format!("{icon}"))
            .size(15.0)
            .color(Color32::from_rgb(230, 240, 255)),
    )
    .fill(bg)
    .stroke(Stroke::new(1.0, BORDER_COLOR))
    .rounding(8.0)
    .min_size(egui::vec2(30.0, 28.0));
    let response = ui.add(button).on_hover_text(tooltip);

    if response.hovered() {
        ui.painter().rect_filled(response.rect, 8.0, hover_bg);
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{icon}"),
            egui::FontId::proportional(15.0),
            Color32::from_rgb(230, 240, 255),
        );
    } else if response.is_pointer_button_down_on() {
        ui.painter().rect_filled(response.rect, 8.0, active_bg);
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{icon}"),
            egui::FontId::proportional(15.0),
            Color32::from_rgb(230, 240, 255),
        );
    }

    response.clicked()
}

fn styled_icon_toggle(ui: &mut Ui, selected: bool, icon: AppIcon, tooltip: &str) -> bool {
    let (fill, stroke) = if selected {
        (Color32::from_rgb(28, 108, 158), Stroke::new(1.0, ACCENT))
    } else {
        (BTN_ICON, Stroke::new(1.0, Color32::from_rgb(64, 104, 138)))
    };
    let button = egui::Button::new(
        RichText::new(format!("{icon}"))
            .size(14.0)
            .color(Color32::from_rgb(236, 244, 255)),
    )
    .fill(fill)
    .stroke(stroke)
    .rounding(8.0)
    .min_size(egui::vec2(30.0, 28.0));
    ui.add(button).on_hover_text(tooltip).clicked()
}

fn build_terminal_layout_job(snapshot: &TerminalSnapshot, font_id: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;

    for (line_index, line) in snapshot.lines.iter().enumerate() {
        for run in &line.runs {
            let fg = to_egui_color(run.style.fg);
            let mut format = TextFormat {
                font_id: font_id.clone(),
                color: fg,
                background: normalize_terminal_background(run.style.bg),
                italics: run.style.italic,
                ..TextFormat::default()
            };

            if run.style.underline {
                format.underline = Stroke::new(1.0, fg);
            }
            if run.style.strike {
                format.strikethrough = Stroke::new(1.0, fg);
            }

            job.append(&run.text, 0.0, format);
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

    job
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
    use super::{normalize_terminal_background, parse_branch_header, AdeApp, TERMINAL_OUTPUT_BG};
    use crate::terminal::TerminalColor;
    use eframe::egui::{Key, Modifiers};

    #[test]
    fn maps_navigation_keys_to_escape_sequences() {
        let up = AdeApp::key_to_terminal_bytes(Key::ArrowUp, Modifiers::default());
        let delete = AdeApp::key_to_terminal_bytes(Key::Delete, Modifiers::default());

        assert_eq!(up, Some(b"\x1b[A".to_vec()));
        assert_eq!(delete, Some(b"\x1b[3~".to_vec()));
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
    fn pending_line_keeps_last_logical_line() {
        let mut pending = String::new();

        AdeApp::append_pending_line(&mut pending, "echo first");
        AdeApp::append_pending_line(&mut pending, "\nnext");

        assert_eq!(pending, "next");
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
}
