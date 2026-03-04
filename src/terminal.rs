use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tattoy_wezterm_term::color::{ColorPalette, SrgbaTuple};
use tattoy_wezterm_term::config::TerminalConfiguration;
use tattoy_wezterm_term::{CellAttributes, Terminal, TerminalSize};

use crate::models::ShellKind;

const DEFAULT_SCROLLBACK: usize = 1000;
const IO_BUFFER_SIZE: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalStyle {
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStyledRun {
    pub text: String,
    pub style: TerminalStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalStyledLine {
    pub runs: Vec<TerminalStyledRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalSnapshot {
    pub lines: Vec<TerminalStyledLine>,
}

#[derive(Debug, Clone)]
pub struct TerminalUiEvent {
    pub terminal_id: u64,
    pub kind: TerminalUiEventKind,
}

#[derive(Debug, Clone)]
pub enum TerminalUiEventKind {
    Wakeup,
    Title(String),
    ResetTitle,
    PtyWrite(String),
    ChildExit,
    Exit,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalDimensions {
    pub cols: u16,
    pub lines: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl Default for TerminalDimensions {
    fn default() -> Self {
        Self {
            cols: 120,
            lines: 30,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

impl TerminalDimensions {
    fn to_pty_size(self) -> PtySize {
        PtySize {
            rows: self.lines.max(1),
            cols: self.cols.max(1),
            pixel_width: self.cell_width.saturating_mul(self.cols.max(1)),
            pixel_height: self.cell_height.saturating_mul(self.lines.max(1)),
        }
    }

    fn to_term_size(self) -> TerminalSize {
        TerminalSize {
            rows: self.lines.max(1) as usize,
            cols: self.cols.max(1) as usize,
            pixel_width: usize::from(self.cell_width.saturating_mul(self.cols.max(1))),
            pixel_height: usize::from(self.cell_height.saturating_mul(self.lines.max(1))),
            dpi: 96,
        }
    }
}

pub struct TerminalRuntime {
    term: Arc<Mutex<Terminal>>,
    command_tx: Sender<RuntimeCommand>,
    latest_seqno: Arc<AtomicUsize>,
    last_size: TerminalDimensions,
}

enum RuntimeCommand {
    Input(Vec<u8>),
    Resize(TerminalDimensions),
    Shutdown,
}

#[derive(Debug)]
struct AdeTerminalConfig;

impl TerminalConfiguration for AdeTerminalConfig {
    fn scrollback_size(&self) -> usize {
        DEFAULT_SCROLLBACK
    }

    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl SharedWriter {
    fn new(inner: Arc<Mutex<Box<dyn Write + Send>>>) -> Self {
        Self { inner }
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut writer = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "writer lock poisoned"))?;
        writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut writer = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "writer lock poisoned"))?;
        writer.flush()
    }
}

impl TerminalRuntime {
    pub fn spawn(
        terminal_id: u64,
        shell: ShellKind,
        working_directory: std::path::PathBuf,
        ui_event_tx: Sender<TerminalUiEvent>,
        repaint_ctx: eframe::egui::Context,
        dimensions: TerminalDimensions,
    ) -> io::Result<Self> {
        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(dimensions.to_pty_size())
            .map_err(io_error_from_anyhow)?;

        let (program, args) = shell.command();
        let mut command = CommandBuilder::new(program);
        command.args(args.iter().copied());
        command.cwd(working_directory);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("CLICOLOR", "1");
        command.env("CLICOLOR_FORCE", "1");
        command.env("FORCE_COLOR", "1");
        command.env("TERM_PROGRAM", "MergenADE");
        command.env("WT_SESSION", "MergenADE");
        command.env("ConEmuANSI", "ON");
        command.env("ANSICON", "1");

        let child = pty_pair
            .slave
            .spawn_command(command)
            .map_err(io_error_from_anyhow)?;

        let reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(io_error_from_anyhow)?;
        let writer = pty_pair
            .master
            .take_writer()
            .map_err(io_error_from_anyhow)?;
        let shared_writer = Arc::new(Mutex::new(writer));

        let mut terminal = Terminal::new(
            dimensions.to_term_size(),
            Arc::new(AdeTerminalConfig),
            "mergen-ade",
            env!("CARGO_PKG_VERSION"),
            Box::new(SharedWriter::new(shared_writer.clone())),
        );
        #[cfg(target_os = "windows")]
        terminal.enable_conpty_quirks();

        let latest_seqno = Arc::new(AtomicUsize::new(terminal.current_seqno()));
        let term = Arc::new(Mutex::new(terminal));
        let (command_tx, command_rx) = crossbeam_channel::unbounded();

        spawn_reader_thread(
            terminal_id,
            term.clone(),
            latest_seqno.clone(),
            reader,
            ui_event_tx.clone(),
            repaint_ctx.clone(),
        );
        spawn_io_thread(
            terminal_id,
            term.clone(),
            latest_seqno.clone(),
            pty_pair.master,
            shared_writer,
            command_rx,
            ui_event_tx.clone(),
            repaint_ctx.clone(),
        );
        spawn_child_waiter_thread(terminal_id, child, ui_event_tx, repaint_ctx);

        Ok(Self {
            term,
            command_tx,
            latest_seqno,
            last_size: dimensions,
        })
    }

    pub fn send_bytes(&self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }

        let _ = self.command_tx.send(RuntimeCommand::Input(bytes));
    }

    pub fn resize(&mut self, dimensions: TerminalDimensions) -> bool {
        if dimensions.cols == 0 || dimensions.lines == 0 {
            return false;
        }

        if self.last_size.cols == dimensions.cols && self.last_size.lines == dimensions.lines {
            return true;
        }

        self.last_size = dimensions;
        self.command_tx
            .send(RuntimeCommand::Resize(dimensions))
            .is_ok()
    }

    pub fn shutdown(&self) {
        let _ = self.command_tx.send(RuntimeCommand::Shutdown);
    }

    pub fn latest_seqno(&self) -> usize {
        self.latest_seqno.load(Ordering::Relaxed)
    }
}

fn spawn_reader_thread(
    terminal_id: u64,
    term: Arc<Mutex<Terminal>>,
    latest_seqno: Arc<AtomicUsize>,
    mut reader: Box<dyn Read + Send>,
    tx: Sender<TerminalUiEvent>,
    repaint_ctx: eframe::egui::Context,
) {
    thread::spawn(move || {
        let mut buffer = vec![0u8; IO_BUFFER_SIZE];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read_bytes) => {
                    if let Ok(mut terminal) = term.lock() {
                        terminal.advance_bytes(&buffer[..read_bytes]);
                        latest_seqno.store(terminal.current_seqno(), Ordering::Relaxed);
                    }
                    send_ui_event(terminal_id, TerminalUiEventKind::Wakeup, &tx, &repaint_ctx);
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    });
}

fn spawn_io_thread(
    terminal_id: u64,
    term: Arc<Mutex<Terminal>>,
    latest_seqno: Arc<AtomicUsize>,
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    command_rx: Receiver<RuntimeCommand>,
    tx: Sender<TerminalUiEvent>,
    repaint_ctx: eframe::egui::Context,
) {
    thread::spawn(move || {
        let master = master;

        while let Ok(command) = command_rx.recv() {
            match command {
                RuntimeCommand::Input(bytes) => {
                    let write_result = writer.lock().map_err(|_| {
                        io::Error::new(io::ErrorKind::BrokenPipe, "writer lock poisoned")
                    });

                    let Ok(mut writer_guard) = write_result else {
                        break;
                    };

                    if writer_guard.write_all(&bytes).is_err() {
                        break;
                    }
                    if writer_guard.flush().is_err() {
                        break;
                    }
                    send_ui_event(terminal_id, TerminalUiEventKind::Wakeup, &tx, &repaint_ctx);
                }
                RuntimeCommand::Resize(dimensions) => {
                    let _ = master.resize(dimensions.to_pty_size());
                    if let Ok(mut terminal) = term.lock() {
                        terminal.resize(dimensions.to_term_size());
                        latest_seqno.store(terminal.current_seqno(), Ordering::Relaxed);
                    }
                    send_ui_event(terminal_id, TerminalUiEventKind::Wakeup, &tx, &repaint_ctx);
                }
                RuntimeCommand::Shutdown => break,
            }
        }
    });
}

fn spawn_child_waiter_thread(
    terminal_id: u64,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    tx: Sender<TerminalUiEvent>,
    repaint_ctx: eframe::egui::Context,
) {
    thread::spawn(move || {
        let _ = child.wait();
        send_ui_event(
            terminal_id,
            TerminalUiEventKind::ChildExit,
            &tx,
            &repaint_ctx,
        );
        send_ui_event(terminal_id, TerminalUiEventKind::Exit, &tx, &repaint_ctx);
    });
}

fn send_ui_event(
    terminal_id: u64,
    kind: TerminalUiEventKind,
    tx: &Sender<TerminalUiEvent>,
    repaint_ctx: &eframe::egui::Context,
) {
    let _ = tx.send(TerminalUiEvent { terminal_id, kind });
    repaint_ctx.request_repaint();
}

pub fn try_terminal_snapshot(runtime: &TerminalRuntime) -> Option<TerminalSnapshot> {
    let terminal = runtime.term.lock().ok()?;
    Some(snapshot_from_terminal(&terminal))
}

fn snapshot_from_terminal(terminal: &Terminal) -> TerminalSnapshot {
    let palette = terminal.palette();
    let screen = terminal.screen();
    let rows = screen.physical_rows;
    let cols = screen.physical_cols;

    if rows == 0 || cols == 0 {
        return TerminalSnapshot::default();
    }

    let first_visible_row = screen.scrollback_rows().saturating_sub(rows);
    let default_style = default_style(&palette);
    let mut lines = Vec::with_capacity(rows);

    screen.for_each_phys_line(|row_index, line| {
        if row_index < first_visible_row || lines.len() >= rows {
            return;
        }

        let mut segments: Vec<(String, TerminalStyle)> = Vec::new();
        let mut cursor_col = 0usize;

        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                continue;
            }

            if col > cursor_col {
                push_segment(&mut segments, " ".repeat(col - cursor_col), default_style);
            }

            let style = resolve_style(cell.attrs(), &palette);
            let mut text = sanitize_cell_text(cell.str());
            if text.is_empty() {
                text.push(' ');
            }

            push_segment(&mut segments, text, style);
            cursor_col = (col + cell.width().max(1)).min(cols);
        }

        if cursor_col < cols {
            push_segment(&mut segments, " ".repeat(cols - cursor_col), default_style);
        }

        trim_trailing_default_spaces(&mut segments, default_style);

        let mut styled_line = TerminalStyledLine::default();
        for (text, style) in segments {
            if text.is_empty() {
                continue;
            }

            if let Some(last) = styled_line.runs.last_mut() {
                if last.style == style {
                    last.text.push_str(&text);
                    continue;
                }
            }

            styled_line.runs.push(TerminalStyledRun { text, style });
        }

        lines.push(styled_line);
    });

    while lines.len() < rows {
        lines.push(TerminalStyledLine::default());
    }

    TerminalSnapshot { lines }
}

fn push_segment(segments: &mut Vec<(String, TerminalStyle)>, text: String, style: TerminalStyle) {
    if text.is_empty() {
        return;
    }

    if let Some((previous_text, previous_style)) = segments.last_mut() {
        if *previous_style == style {
            previous_text.push_str(&text);
            return;
        }
    }

    segments.push((text, style));
}

fn trim_trailing_default_spaces(
    segments: &mut Vec<(String, TerminalStyle)>,
    default: TerminalStyle,
) {
    loop {
        let Some((text, style)) = segments.last_mut() else {
            break;
        };

        if *style != default {
            break;
        }

        let trimmed_len = text.trim_end_matches(' ').len();
        if trimmed_len == text.len() {
            break;
        }

        if trimmed_len == 0 {
            segments.pop();
            continue;
        }

        text.truncate(trimmed_len);
        break;
    }
}

fn resolve_style(attrs: &CellAttributes, palette: &ColorPalette) -> TerminalStyle {
    let mut fg = to_terminal_color(palette.resolve_fg(attrs.foreground()));
    let mut bg = to_terminal_color(palette.resolve_bg(attrs.background()));

    let intensity = attrs.intensity() as u8;
    if intensity == 1 {
        fg = brighten_color(fg);
    } else if intensity == 2 {
        fg = dim_color(fg);
    }

    if attrs.reverse() {
        std::mem::swap(&mut fg, &mut bg);
    }

    if attrs.invisible() {
        fg = bg;
    }

    TerminalStyle {
        fg,
        bg,
        italic: attrs.italic(),
        underline: (attrs.underline() as u8) != 0,
        strike: attrs.strikethrough(),
    }
}

fn default_style(palette: &ColorPalette) -> TerminalStyle {
    TerminalStyle {
        fg: to_terminal_color(palette.foreground),
        bg: to_terminal_color(palette.background),
        italic: false,
        underline: false,
        strike: false,
    }
}

fn to_terminal_color(color: SrgbaTuple) -> TerminalColor {
    TerminalColor {
        r: float_channel_to_u8(color.0),
        g: float_channel_to_u8(color.1),
        b: float_channel_to_u8(color.2),
    }
}

fn float_channel_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn brighten_color(color: TerminalColor) -> TerminalColor {
    TerminalColor {
        r: color.r.saturating_add((u16::from(255 - color.r) / 3) as u8),
        g: color.g.saturating_add((u16::from(255 - color.g) / 3) as u8),
        b: color.b.saturating_add((u16::from(255 - color.b) / 3) as u8),
    }
}

fn dim_color(color: TerminalColor) -> TerminalColor {
    TerminalColor {
        r: (u16::from(color.r) * 2 / 3) as u8,
        g: (u16::from(color.g) * 2 / 3) as u8,
        b: (u16::from(color.b) * 2 / 3) as u8,
    }
}

fn sanitize_cell_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch == '\0' || ch == '\u{fe0f}' || ch.is_control() {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

fn io_error_from_anyhow(err: impl std::fmt::Display) -> io::Error {
    io::Error::other(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        default_style, push_segment, sanitize_cell_text, snapshot_from_terminal,
        trim_trailing_default_spaces, AdeTerminalConfig, TerminalColor, TerminalStyle,
    };
    use std::sync::Arc;
    use tattoy_wezterm_term::color::ColorPalette;
    use tattoy_wezterm_term::{Terminal, TerminalSize};

    #[test]
    fn sanitize_cell_text_drops_control_chars() {
        let text = sanitize_cell_text("ab\u{0007}\0c");
        assert_eq!(text, "ab  c");
    }

    #[test]
    fn trimming_removes_only_default_trailing_spaces() {
        let style = TerminalStyle {
            fg: TerminalColor { r: 1, g: 2, b: 3 },
            bg: TerminalColor { r: 0, g: 0, b: 0 },
            italic: false,
            underline: false,
            strike: false,
        };
        let default = default_style(&ColorPalette::default());
        let mut segments = vec![
            ("abc".to_owned(), style),
            ("   ".to_owned(), default),
            ("  ".to_owned(), default),
        ];

        trim_trailing_default_spaces(&mut segments, default);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, "abc");
    }

    #[test]
    fn push_segment_merges_adjacent_styles() {
        let style = TerminalStyle {
            fg: TerminalColor {
                r: 10,
                g: 20,
                b: 30,
            },
            bg: TerminalColor { r: 0, g: 0, b: 0 },
            italic: false,
            underline: false,
            strike: false,
        };

        let mut segments = Vec::new();
        push_segment(&mut segments, "ab".to_owned(), style);
        push_segment(&mut segments, "cd".to_owned(), style);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, "abcd");
    }

    #[test]
    fn snapshot_preserves_ansi_foreground_color() {
        let size = TerminalSize {
            rows: 4,
            cols: 40,
            pixel_width: 320,
            pixel_height: 64,
            dpi: 96,
        };
        let mut terminal = Terminal::new(
            size,
            Arc::new(AdeTerminalConfig),
            "test",
            "0",
            Box::new(std::io::sink()),
        );
        terminal.advance_bytes(b"\x1b[31mRED\x1b[0m");

        let snapshot = snapshot_from_terminal(&terminal);
        let first_line = &snapshot.lines[0];
        let red_run = first_line
            .runs
            .iter()
            .find(|run| run.text.contains("RED"))
            .expect("expected RED run");

        assert!(red_run.style.fg.r > red_run.style.fg.g);
        assert!(red_run.style.fg.r > red_run.style.fg.b);
    }
}
