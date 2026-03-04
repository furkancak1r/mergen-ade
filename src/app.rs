use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Color32, Event, FontId, Key, Layout, RichText, Sense, Stroke, TextWrapMode, Ui,
    Vec2,
};

use crate::config;
use crate::layout;
use crate::models::{AppConfig, AutoTileScope, ProjectRecord, ShellKind, TerminalKind};
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
const TERMINAL_OUTPUT_BG: Color32 = Color32::from_rgb(0, 0, 0);

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
    show_saved_messages_picker: bool,
    new_saved_message: String,
    status_line: String,
    layout_epoch: u64,
    repaint_pump_started: bool,
    repaint_pump_flag: Arc<AtomicBool>,
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

impl AdeApp {
    pub fn bootstrap() -> Self {
        let config_path = config::config_path().unwrap_or_else(|_| PathBuf::from("config.toml"));
        let config = config::load_config(&config_path).unwrap_or_default();

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
            show_saved_messages_picker: false,
            new_saved_message: String::new(),
            status_line: "Ready".to_owned(),
            layout_epoch: 0,
            repaint_pump_started: false,
            repaint_pump_flag: Arc::new(AtomicBool::new(true)),
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
        if !terminal.in_main_view {
            return false;
        }

        if self.config.ui.project_filter_mode
            && self
                .selected_project
                .is_some_and(|selected_project| selected_project != terminal.project_id)
        {
            return false;
        }

        true
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
            shell_override: None,
            saved_messages: Vec::new(),
        };

        self.selected_project = Some(project.id);
        self.projects.insert(project.id, project);
        self.next_project_id += 1;
        self.bump_layout_epoch();
        self.persist_config();
    }

    fn selected_project_shell(&self, project_id: u64) -> Option<ShellKind> {
        let project = self.projects.get(&project_id)?;
        Some(project.shell_override.unwrap_or(self.config.default_shell))
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

        let Some(shell) = self.selected_project_shell(project_id) else {
            return;
        };

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

        if self.config.ui.project_filter_mode {
            self.selected_project = Some(project_id);
            self.bump_layout_epoch();
        }

        self.status_line = "Terminal created".to_owned();
    }

    fn process_terminal_events(&mut self, ctx: &egui::Context) {
        let mut dirty_ids = BTreeSet::new();
        let mut exited_ids = BTreeSet::new();
        let mut title_updates: BTreeMap<u64, Option<String>> = BTreeMap::new();
        let mut pty_writes: Vec<(u64, String)> = Vec::new();
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
                TerminalUiEventKind::Title(title) => {
                    if !title.trim().is_empty() {
                        title_updates.insert(event.terminal_id, Some(title));
                    }
                }
                TerminalUiEventKind::ResetTitle => {
                    title_updates.insert(event.terminal_id, None);
                }
                TerminalUiEventKind::PtyWrite(payload) => {
                    pty_writes.push((event.terminal_id, payload));
                }
                TerminalUiEventKind::ChildExit | TerminalUiEventKind::Exit => {
                    exited_ids.insert(event.terminal_id);
                    dirty_ids.insert(event.terminal_id);
                }
            }
        }

        let mut changed = false;

        for (terminal_id, payload) in pty_writes {
            let Some(entry) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            entry.runtime.send_bytes(payload.into_bytes());
        }

        for terminal_id in dirty_ids {
            let Some(entry) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            entry.dirty = true;
            changed = true;
        }

        for (terminal_id, update) in title_updates {
            let Some(entry) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            entry.title = match update {
                Some(title) => title,
                None => format!("Terminal {}", entry.id),
            };
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

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let _ = ctx;
    }

    fn apply_auto_tile(&mut self, scope: AutoTileScope) {
        self.config.ui.auto_tile_scope = scope;

        for terminal in self.terminals.values_mut() {
            terminal.in_main_view = match scope {
                AutoTileScope::AllVisible => true,
                AutoTileScope::SelectedProjectOnly => self
                    .selected_project
                    .is_some_and(|project_id| project_id == terminal.project_id),
            };
        }

        self.bump_layout_epoch();
        self.status_line = format!("Auto Tile: {}", scope.label());
        self.persist_config();
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
        if (self.show_settings_popup || self.show_saved_messages_picker)
            && ctx.wants_keyboard_input()
        {
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

    fn ensure_repaint_pump(&mut self, ctx: &egui::Context) {
        if self.repaint_pump_started {
            return;
        }

        self.repaint_pump_started = true;
        let repaint_ctx = ctx.clone();
        let running = self.repaint_pump_flag.clone();

        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                repaint_ctx.request_repaint();
                std::thread::sleep(Duration::from_millis(TERMINAL_FALLBACK_REFRESH_MS));
            }
        });
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

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Add Project").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.add_project(path);
                    }
                }

                if ui.button("New Terminal").clicked() {
                    if let Some(project_id) = self.selected_project {
                        self.spawn_terminal_for_project(ctx, project_id, TerminalKind::Foreground);
                    }
                }

                if ui.button("Auto Tile").clicked() {
                    self.apply_auto_tile(self.config.ui.auto_tile_scope);
                }

                if ui.button("Saved Messages").clicked() {
                    self.show_saved_messages_picker = true;
                }

                if ui.button("Settings").clicked() {
                    self.show_settings_popup = true;
                }

                ui.separator();
                ui.label(format!("Status: {}", self.status_line));
            });
        });
    }

    fn draw_project_explorer(&mut self, ctx: &egui::Context) {
        if !self.config.ui.show_project_explorer {
            return;
        }

        egui::SidePanel::left("project_explorer")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Projects");
                ui.separator();

                let project_rows = self
                    .projects
                    .values()
                    .map(|project| {
                        (
                            project.id,
                            project.name.clone(),
                            project.path.display().to_string(),
                        )
                    })
                    .collect::<Vec<_>>();

                for (project_id, project_name, project_path) in project_rows {
                    let selected = self.selected_project == Some(project_id);
                    let response = ui.selectable_label(selected, &project_name);
                    if response.clicked() {
                        let previous_selected = self.selected_project;
                        self.selected_project = Some(project_id);
                        if self.config.ui.project_filter_mode
                            && previous_selected != self.selected_project
                        {
                            self.bump_layout_epoch();
                        }
                        self.persist_config();
                    }
                    response.context_menu(|ui| {
                        if ui.button("Copy Project Path").clicked() {
                            ui.ctx().copy_text(project_path.clone());
                            self.status_line =
                                format!("Copied path for project '{}'", project_name);
                            ui.close_menu();
                        }
                    });
                }

                ui.separator();

                if let Some(project_id) = self.selected_project {
                    if let Some(project) = self.projects.get(&project_id) {
                        ui.label("Project Explorer");

                        draw_folder_tree(ui, &project.path, 0, 8);
                    }
                } else {
                    ui.label("No project selected");
                }
            });
    }

    fn draw_terminal_manager(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("terminal_manager")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Terminal Manager");
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

                    let Some(project_snapshot) = self.projects.get(&project_id).cloned() else {
                        continue;
                    };

                    let mut next_shell_override = project_snapshot.shell_override;
                    let mut add_message: Option<String> = None;
                    let mut remove_message_index: Option<usize> = None;
                    let mut requested_persist = false;
                    let project_path = project_snapshot.path.display().to_string();

                    let header = egui::CollapsingHeader::new(project_snapshot.name.clone())
                        .id_salt(format!("project-group-{project_id}"))
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.button("New FG").clicked() {
                                    self.spawn_terminal_for_project(
                                        ctx,
                                        project_id,
                                        TerminalKind::Foreground,
                                    );
                                }
                                if ui.button("New BG").clicked() {
                                    self.spawn_terminal_for_project(
                                        ctx,
                                        project_id,
                                        TerminalKind::Background,
                                    );
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Shell Override:");
                                let mut current = project_snapshot
                                    .shell_override
                                    .unwrap_or(self.config.default_shell);
                                egui::ComboBox::from_id_salt(format!(
                                    "shell-override-{project_id}"
                                ))
                                .selected_text(
                                    project_snapshot
                                        .shell_override
                                        .map(|shell| shell.label().to_owned())
                                        .unwrap_or_else(|| {
                                            format!(
                                                "Global ({})",
                                                self.config.default_shell.label()
                                            )
                                        }),
                                )
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            project_snapshot.shell_override.is_none(),
                                            format!(
                                                "Global ({})",
                                                self.config.default_shell.label()
                                            ),
                                        )
                                        .clicked()
                                    {
                                        next_shell_override = None;
                                        requested_persist = true;
                                    }

                                    if ui
                                        .selectable_value(
                                            &mut current,
                                            ShellKind::PowerShell,
                                            ShellKind::PowerShell.label(),
                                        )
                                        .clicked()
                                    {
                                        next_shell_override = Some(ShellKind::PowerShell);
                                        requested_persist = true;
                                    }

                                    if ui
                                        .selectable_value(
                                            &mut current,
                                            ShellKind::Cmd,
                                            ShellKind::Cmd.label(),
                                        )
                                        .clicked()
                                    {
                                        next_shell_override = Some(ShellKind::Cmd);
                                        requested_persist = true;
                                    }
                                });
                            });

                            ui.separator();
                            ui.label(RichText::new("Foreground terminals").strong());
                            self.draw_terminal_rows(ui, project_id, TerminalKind::Foreground);

                            ui.separator();
                            ui.colored_label(Color32::LIGHT_BLUE, "Background terminals");
                            self.draw_terminal_rows(ui, project_id, TerminalKind::Background);

                            ui.separator();
                            ui.label("Saved messages");
                            for (index, message) in
                                project_snapshot.saved_messages.iter().enumerate()
                            {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(message).monospace().small());
                                    if ui.small_button("Remove").clicked() {
                                        remove_message_index = Some(index);
                                    }
                                });
                            }

                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.new_saved_message);
                                if ui.button("Add").clicked() {
                                    let text = self.new_saved_message.trim();
                                    if !text.is_empty() {
                                        add_message = Some(text.to_owned());
                                        self.new_saved_message.clear();
                                    }
                                }
                            });
                        });

                    header.header_response.context_menu(|ui| {
                        if ui.button("Copy Project Path").clicked() {
                            ui.ctx().copy_text(project_path.clone());
                            self.status_line =
                                format!("Copied path for project '{}'", project_snapshot.name);
                            ui.close_menu();
                        }
                    });

                    if let Some(project) = self.projects.get_mut(&project_id) {
                        project.shell_override = next_shell_override;
                        if let Some(message) = add_message {
                            project.saved_messages.push(message);
                            requested_persist = true;
                        }
                        if let Some(index) = remove_message_index {
                            if index < project.saved_messages.len() {
                                project.saved_messages.remove(index);
                                requested_persist = true;
                            }
                        }
                    }

                    if requested_persist {
                        self.persist_config();
                    }
                }
            });
    }

    fn draw_terminal_rows(&mut self, ui: &mut Ui, project_id: u64, kind: TerminalKind) {
        let ids = self
            .terminals
            .iter()
            .filter(|(_, terminal)| terminal.project_id == project_id && terminal.kind == kind)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let current_active = self.active_terminal;

        for terminal_id in ids {
            let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
                continue;
            };
            let terminal_entry_id = terminal.id;

            let mut set_active = false;
            let mut close_terminal = false;
            let mut visibility_changed = false;
            ui.horizontal(|ui| {
                let active = current_active == Some(terminal_entry_id);
                let label = if terminal.exited {
                    format!("{} (Exited)", terminal.title)
                } else {
                    terminal.title.clone()
                };

                if ui.selectable_label(active, label).clicked() {
                    set_active = true;
                }

                if ui.checkbox(&mut terminal.in_main_view, "Show").changed() {
                    visibility_changed = true;
                }
                if ui.small_button("Close").clicked() {
                    close_terminal = true;
                }
            });

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
                    ui.label("No visible terminals. Use 'New Terminal' and Auto Tile.");
                });
                return;
            }

            let available = ui.available_size();
            let grid = layout::compute_tile_grid(visible_ids.len(), available.x, available.y);
            let spacing = Vec2::new(8.0, 8.0);

            let pane_width = ((available.x - spacing.x * (grid.cols.saturating_sub(1) as f32))
                / grid.cols as f32)
                .max(140.0);
            let pane_height = ((available.y - spacing.y * (grid.rows.saturating_sub(1) as f32))
                / grid.rows as f32)
                .max(120.0);

            for row in 0..grid.rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing.x;
                    for col in 0..grid.cols {
                        let index = row * grid.cols + col;
                        let size = Vec2::new(pane_width, pane_height);
                        if let Some(terminal_id) = visible_ids.get(index) {
                            ui.allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    self.draw_terminal_pane(ui, *terminal_id, size);
                                });
                            });
                        } else {
                            ui.allocate_space(size);
                        }
                    }
                });
                if row + 1 < grid.rows {
                    ui.add_space(spacing.y);
                }
            }
        });
    }

    fn draw_terminal_pane(&mut self, ui: &mut Ui, terminal_id: u64, pane_size: Vec2) {
        let layout_epoch = self.layout_epoch;
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
            ui.horizontal_wrapped(|ui| {
                let title = format!("{} {}", if is_active { "*" } else { "." }, terminal.title);
                if ui.selectable_label(is_active, title).clicked() {
                    pane_clicked = true;
                }
                ui.separator();
                ui.label(project_name);
                ui.separator();
                ui.label(terminal.kind.label());
                if terminal.exited {
                    ui.colored_label(Color32::LIGHT_RED, "Exited");
                }
                if ui.small_button("Close").clicked() {
                    close_requested = true;
                }
            });

            let monospace = egui::TextStyle::Monospace;
            let font_id = monospace.resolve(ui.style());
            let char_width = ui
                .fonts(|fonts| fonts.glyph_width(&font_id, 'W'))
                .max(CELL_WIDTH_PX);
            let line_height = ui.text_style_height(&monospace).max(CELL_HEIGHT_PX);

            let pane_available = ui.available_size_before_wrap();
            let pane_width = pane_available.x.max((pane_size.x - 10.0).max(120.0));
            let pane_height = pane_available.y.max((pane_size.y - 10.0).max(120.0));

            let header_height = line_height + 10.0;
            let output_height = (pane_height - header_height - 8.0).max(line_height * 3.0);
            let output_size = Vec2::new(pane_width, output_height);

            let cols = ((output_size.x / char_width).floor() as u16).max(20);
            let lines = ((output_size.y / line_height).floor() as u16).max(6);
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
                            .id_salt(format!("term-output-{terminal_id}-{layout_epoch}"))
                            .max_height(output_height)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
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

        let mut keep_open = self.show_settings_popup;
        let mut should_persist = false;

        egui::Window::new("Settings")
            .open(&mut keep_open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Application Settings");
                ui.separator();

                let mut show_explorer = self.config.ui.show_project_explorer;
                if ui
                    .checkbox(&mut show_explorer, "Show Project Explorer")
                    .changed()
                {
                    self.config.ui.show_project_explorer = show_explorer;
                    should_persist = true;
                }

                let mut filter_mode = self.config.ui.project_filter_mode;
                if ui
                    .checkbox(&mut filter_mode, "Project Filter Mode")
                    .changed()
                {
                    self.config.ui.project_filter_mode = filter_mode;
                    self.bump_layout_epoch();
                    should_persist = true;
                }

                ui.separator();

                let previous_scope = self.config.ui.auto_tile_scope;
                egui::ComboBox::from_label("Auto Tile Scope")
                    .selected_text(self.config.ui.auto_tile_scope.label())
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
            });

        if should_persist {
            self.persist_config();
        }
        self.show_settings_popup = keep_open;
    }

    fn draw_saved_messages_picker(&mut self, ctx: &egui::Context) {
        if !self.show_saved_messages_picker {
            return;
        }

        let Some(project_id) = self.selected_project else {
            self.show_saved_messages_picker = false;
            return;
        };

        let Some(project) = self.projects.get(&project_id).cloned() else {
            self.show_saved_messages_picker = false;
            return;
        };

        let mut keep_open = self.show_saved_messages_picker;
        let mut should_close = false;
        egui::Window::new("Saved Messages")
            .open(&mut keep_open)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!("Project: {}", project.name));
                ui.label("Pick a message to insert into the active terminal.");
                ui.separator();

                for message in &project.saved_messages {
                    if ui.button(message).clicked() {
                        if let Some(active_terminal_id) = self.active_terminal {
                            if let Some(active_terminal) =
                                self.terminals.get_mut(&active_terminal_id)
                            {
                                active_terminal
                                    .runtime
                                    .send_bytes(message.as_bytes().to_vec());
                                Self::append_pending_line(
                                    &mut active_terminal.pending_line_for_title,
                                    message,
                                );
                            }
                        }
                        should_close = true;
                    }
                }

                if project.saved_messages.is_empty() {
                    ui.label("No saved messages for this project.");
                }
            });

        if should_close {
            keep_open = false;
        }
        self.show_saved_messages_picker = keep_open;
    }
}

impl eframe::App for AdeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_repaint_pump(ctx);
        self.process_terminal_events(ctx);
        self.schedule_terminal_refresh(ctx);
        self.handle_shortcuts(ctx);

        self.draw_top_bar(ctx);
        self.draw_project_explorer(ctx);
        self.draw_terminal_manager(ctx);
        self.draw_main_area(ctx);
        self.draw_settings_popup(ctx);
        self.draw_saved_messages_picker(ctx);

        self.route_active_terminal_input(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.repaint_pump_flag.store(false, Ordering::Relaxed);
        for terminal in self.terminals.values() {
            terminal.runtime.shutdown();
        }

        self.persist_config();
    }
}

fn draw_folder_tree(ui: &mut Ui, path: &Path, depth: usize, max_depth: usize) {
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

        if item.is_dir() {
            egui::CollapsingHeader::new(name)
                .id_salt(item.display().to_string())
                .show(ui, |ui| draw_folder_tree(ui, &item, depth + 1, max_depth));
        } else {
            ui.label(name);
        }
    }
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
                background: to_egui_color(run.style.bg),
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

#[cfg(test)]
mod tests {
    use super::AdeApp;
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
}
