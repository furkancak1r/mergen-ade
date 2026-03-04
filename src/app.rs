use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Align, Color32, Key, Layout, RichText, TextWrapMode, Ui, Vec2};

use crate::config;
use crate::layout;
use crate::models::{AppConfig, AutoTileScope, ProjectRecord, ShellKind, TerminalKind};
use crate::terminal::{terminal_snapshot_text, TerminalDimensions, TerminalRuntime, TerminalUiEvent, TerminalUiEventKind};
use crate::title::update_terminal_title;

const CELL_WIDTH_PX: f32 = 8.0;
const CELL_HEIGHT_PX: f32 = 16.0;
const TITLE_MAX_LEN: usize = 40;

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
}

struct TerminalEntry {
    id: u64,
    project_id: u64,
    kind: TerminalKind,
    title: String,
    input_buffer: String,
    in_main_view: bool,
    dirty: bool,
    render_cache: String,
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

    fn spawn_terminal_for_project(&mut self, project_id: u64, kind: TerminalKind) {
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
            dimensions,
        ) {
            Ok(runtime) => runtime,
            Err(err) => {
                self.status_line = format!("Failed to create terminal: {err}");
                return;
            },
        };

        let fallback_title = format!("Terminal {terminal_id}");
        let entry = TerminalEntry {
            id: terminal_id,
            project_id,
            kind,
            title: fallback_title,
            input_buffer: String::new(),
            in_main_view: true,
            dirty: true,
            render_cache: String::new(),
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
        while let Ok(event) = self.terminal_events_rx.try_recv() {
            let Some(entry) = self.terminals.get_mut(&event.terminal_id) else {
                continue;
            };

            match event.kind {
                TerminalUiEventKind::Wakeup => {
                    entry.dirty = true;
                    ctx.request_repaint();
                },
                TerminalUiEventKind::Title(title) => {
                    if !title.trim().is_empty() {
                        entry.title = title;
                    }
                },
                TerminalUiEventKind::ResetTitle => {
                    entry.title = format!("Terminal {}", entry.id);
                },
                TerminalUiEventKind::PtyWrite(payload) => {
                    entry.runtime.send_bytes(payload.into_bytes());
                },
                TerminalUiEventKind::ChildExit | TerminalUiEventKind::Exit => {
                    entry.exited = true;
                    entry.dirty = true;
                },
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let mut do_toggle_explorer = false;
        let mut do_toggle_filter = false;
        let mut do_new_terminal = false;
        let mut do_auto_tile_all = false;
        let mut do_auto_tile_selected = false;
        let mut do_saved_picker = false;
        let mut do_next_terminal = false;
        let mut do_prev_terminal = false;

        ctx.input(|input| {
            let command = input.modifiers.ctrl;
            if command && input.key_pressed(Key::B) {
                do_toggle_explorer = true;
            }
            if command && input.modifiers.shift && input.key_pressed(Key::F) {
                do_toggle_filter = true;
            }
            if command && input.modifiers.shift && input.key_pressed(Key::T) {
                do_new_terminal = true;
            }
            if command && input.modifiers.shift && input.key_pressed(Key::G) {
                do_auto_tile_all = true;
            }
            if command && input.modifiers.alt && input.key_pressed(Key::G) {
                do_auto_tile_selected = true;
            }
            if command && input.modifiers.shift && input.key_pressed(Key::P) {
                do_saved_picker = true;
            }
            if command && input.key_pressed(Key::Tab) {
                if input.modifiers.shift {
                    do_prev_terminal = true;
                } else {
                    do_next_terminal = true;
                }
            }
        });

        if do_toggle_explorer {
            self.config.ui.show_project_explorer = !self.config.ui.show_project_explorer;
            self.persist_config();
        }

        if do_toggle_filter {
            self.config.ui.project_filter_mode = !self.config.ui.project_filter_mode;
            self.bump_layout_epoch();
            self.persist_config();
        }

        if do_new_terminal {
            if let Some(project_id) = self.selected_project {
                self.spawn_terminal_for_project(project_id, TerminalKind::Foreground);
            }
        }

        if do_auto_tile_all {
            self.apply_auto_tile(AutoTileScope::AllVisible);
        }

        if do_auto_tile_selected {
            self.apply_auto_tile(AutoTileScope::SelectedProjectOnly);
        }

        if do_saved_picker {
            self.show_saved_messages_picker = true;
        }

        if do_next_terminal {
            self.cycle_active_terminal(true);
        }

        if do_prev_terminal {
            self.cycle_active_terminal(false);
        }
    }

    fn cycle_active_terminal(&mut self, forward: bool) {
        let mut ids = self.terminals.keys().copied().collect::<Vec<_>>();
        if ids.is_empty() {
            self.active_terminal = None;
            return;
        }

        ids.sort_unstable();

        let current_index = self
            .active_terminal
            .and_then(|active| ids.iter().position(|id| *id == active))
            .unwrap_or(0);

        let next_index = if forward {
            (current_index + 1) % ids.len()
        } else if current_index == 0 {
            ids.len() - 1
        } else {
            current_index - 1
        };

        self.active_terminal = ids.get(next_index).copied();
    }

    fn apply_auto_tile(&mut self, scope: AutoTileScope) {
        self.config.ui.auto_tile_scope = scope;

        for terminal in self.terminals.values_mut() {
            terminal.in_main_view = match scope {
                AutoTileScope::AllVisible => true,
                AutoTileScope::SelectedProjectOnly => {
                    self.selected_project.is_some_and(|project_id| project_id == terminal.project_id)
                },
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
            .filter_map(|(id, terminal)| {
                self.terminal_visible_in_main(terminal).then_some(*id)
            })
            .collect::<Vec<_>>();

        ids.sort_unstable();
        ids
    }

    fn submit_terminal_line(&mut self, terminal_id: u64) {
        let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
            return;
        };

        let line = std::mem::take(&mut terminal.input_buffer);
        terminal.runtime.send_line(&line);

        terminal.title = update_terminal_title(&line, terminal.id as usize, TITLE_MAX_LEN);
        terminal.dirty = true;
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
                        self.spawn_terminal_for_project(project_id, TerminalKind::Foreground);
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
                        if self.config.ui.project_filter_mode && previous_selected != self.selected_project {
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
                        && self.selected_project.is_some_and(|selected| selected != project_id)
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
                                    self.spawn_terminal_for_project(project_id, TerminalKind::Foreground);
                                }
                                if ui.button("New BG").clicked() {
                                    self.spawn_terminal_for_project(project_id, TerminalKind::Background);
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Shell Override:");
                                let mut current =
                                    project_snapshot.shell_override.unwrap_or(self.config.default_shell);
                                egui::ComboBox::from_id_salt(format!("shell-override-{project_id}"))
                                    .selected_text(
                                        project_snapshot
                                            .shell_override
                                            .map(|shell| shell.label().to_owned())
                                            .unwrap_or_else(|| format!("Global ({})", self.config.default_shell.label())),
                                    )
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                project_snapshot.shell_override.is_none(),
                                                format!("Global ({})", self.config.default_shell.label()),
                                            )
                                            .clicked()
                                        {
                                            next_shell_override = None;
                                            requested_persist = true;
                                        }

                                        if ui
                                            .selectable_value(&mut current, ShellKind::PowerShell, ShellKind::PowerShell.label())
                                            .clicked()
                                        {
                                            next_shell_override = Some(ShellKind::PowerShell);
                                            requested_persist = true;
                                        }

                                        if ui
                                            .selectable_value(&mut current, ShellKind::Cmd, ShellKind::Cmd.label())
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
                            for (index, message) in project_snapshot.saved_messages.iter().enumerate() {
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

        let (clicked, enter_pressed, close_requested) = {
            let Some(terminal) = self.terminals.get_mut(&terminal_id) else {
                return;
            };

            let mut close_requested = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(if is_active { "*" } else { "." }).strong());
                ui.label(RichText::new(terminal.title.clone()).strong());
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
            let input_height = line_height + 12.0;
            let output_height =
                (pane_height - header_height - input_height - 12.0).max(line_height * 3.0);
            let output_size = Vec2::new(pane_width, output_height);

            let cols = ((output_size.x / char_width).floor() as u16).max(20);
            let lines = ((output_size.y / line_height).floor() as u16).max(6);
            terminal.runtime.resize(TerminalDimensions {
                cols,
                lines,
                cell_width: char_width as u16,
                cell_height: line_height as u16,
            });

            if terminal.dirty {
                terminal.render_cache = terminal_snapshot_text(&terminal.runtime);
                terminal.dirty = false;
            }

            ui.allocate_ui_with_layout(output_size, Layout::top_down(Align::Min), |ui| {
                ui.set_min_size(output_size);
                egui::ScrollArea::vertical()
                    .id_salt(format!("term-output-{terminal_id}-{layout_epoch}"))
                    .max_height(output_height)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(terminal.render_cache.clone()).monospace())
                                .wrap_mode(TextWrapMode::Extend),
                        );
                    });
                });

            let response = ui.add_sized(
                [pane_width, input_height],
                egui::TextEdit::singleline(&mut terminal.input_buffer)
                    .desired_width(pane_width)
                    .hint_text("Type command and press Enter"),
            );

            if is_active && !response.has_focus() {
                response.request_focus();
            }

            (
                response.clicked(),
                response.has_focus() && ui.input(|input| input.key_pressed(Key::Enter)),
                close_requested,
            )
        };

        if close_requested {
            self.close_terminal(terminal_id);
            return;
        }

        if clicked {
            self.active_terminal = Some(terminal_id);
        }
        if enter_pressed {
            self.submit_terminal_line(terminal_id);
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
                if ui.checkbox(&mut show_explorer, "Show Project Explorer").changed() {
                    self.config.ui.show_project_explorer = show_explorer;
                    should_persist = true;
                }

                let mut filter_mode = self.config.ui.project_filter_mode;
                if ui.checkbox(&mut filter_mode, "Project Filter Mode").changed() {
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
                ui.label("Pick a message to insert into active terminal input buffer.");
                ui.separator();

                for message in &project.saved_messages {
                    if ui.button(message).clicked() {
                        if let Some(active_terminal_id) = self.active_terminal {
                            if let Some(active_terminal) = self.terminals.get_mut(&active_terminal_id) {
                                active_terminal.input_buffer.push_str(message);
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
        self.process_terminal_events(ctx);
        self.handle_shortcuts(ctx);

        self.draw_top_bar(ctx);
        self.draw_project_explorer(ctx);
        self.draw_terminal_manager(ctx);
        self.draw_main_area(ctx);
        self.draw_settings_popup(ctx);
        self.draw_saved_messages_picker(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
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
