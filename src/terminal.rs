use std::borrow::Cow;
use std::io;
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::tty;
use crossbeam_channel::Sender;

use crate::models::ShellKind;

const DEFAULT_SCROLLBACK: usize = 1000;

#[derive(Clone, Copy)]
struct SnapshotCell {
    line: i32,
    col: usize,
    ch: char,
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

#[derive(Clone)]
struct TerminalEventProxy {
    terminal_id: u64,
    tx: Sender<TerminalUiEvent>,
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        let kind = match event {
            Event::Wakeup => TerminalUiEventKind::Wakeup,
            Event::Title(title) => TerminalUiEventKind::Title(title),
            Event::ResetTitle => TerminalUiEventKind::ResetTitle,
            Event::PtyWrite(text) => TerminalUiEventKind::PtyWrite(text),
            Event::ChildExit(_) => TerminalUiEventKind::ChildExit,
            Event::Exit => TerminalUiEventKind::Exit,
            _ => return,
        };

        let _ = self.tx.send(TerminalUiEvent {
            terminal_id: self.terminal_id,
            kind,
        });
    }
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
    pub fn to_window_size(self) -> WindowSize {
        WindowSize {
            num_lines: self.lines,
            num_cols: self.cols,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

impl Dimensions for TerminalDimensions {
    fn total_lines(&self) -> usize {
        self.lines as usize
    }

    fn screen_lines(&self) -> usize {
        self.lines as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}

pub struct TerminalRuntime {
    term: Arc<FairMutex<Term<TerminalEventProxy>>>,
    sender: EventLoopSender,
    last_size: TerminalDimensions,
}

impl TerminalRuntime {
    pub fn spawn(
        terminal_id: u64,
        shell: ShellKind,
        working_directory: std::path::PathBuf,
        ui_event_tx: Sender<TerminalUiEvent>,
        dimensions: TerminalDimensions,
    ) -> io::Result<Self> {
        let mut options = tty::Options::default();
        options.shell = Some(shell.to_shell());
        options.working_directory = Some(working_directory);
        #[cfg(target_os = "windows")]
        {
            options.escape_args = true;
        }

        let pty = tty::new(&options, dimensions.to_window_size(), 0)?;

        let proxy = TerminalEventProxy {
            terminal_id,
            tx: ui_event_tx,
        };

        let mut term_config = TermConfig::default();
        term_config.scrolling_history = DEFAULT_SCROLLBACK;

        let term = Arc::new(FairMutex::new(Term::new(term_config, &dimensions, proxy.clone())));
        let event_loop = EventLoop::new(term.clone(), proxy, pty, false, false)?;
        let sender = event_loop.channel();

        let _ = event_loop.spawn();

        Ok(Self {
            term,
            sender,
            last_size: dimensions,
        })
    }

    pub fn send_bytes(&self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }

        let _ = self.sender.send(Msg::Input(Cow::Owned(bytes)));
    }

    pub fn send_line(&self, line: &str) {
        let mut payload = line.as_bytes().to_vec();
        payload.push(b'\n');
        self.send_bytes(payload);
    }

    pub fn resize(&mut self, dimensions: TerminalDimensions) {
        if dimensions.cols == 0 || dimensions.lines == 0 {
            return;
        }

        if self.last_size.cols == dimensions.cols && self.last_size.lines == dimensions.lines {
            return;
        }

        {
            let mut term = self.term.lock();
            term.resize(dimensions);
        }

        let _ = self.sender.send(Msg::Resize(dimensions.to_window_size()));
        self.last_size = dimensions;
    }

    pub fn shutdown(&self) {
        let _ = self.sender.send(Msg::Shutdown);
    }
}

pub fn terminal_snapshot_text(runtime: &TerminalRuntime) -> String {
    let mut terminal = runtime.term.lock();
    let grid = terminal.grid();

    let rows = grid.screen_lines();
    let cols = grid.columns();
    let snapshot = render_snapshot_cells(
        rows,
        cols,
        grid.display_iter().map(|indexed| SnapshotCell {
            line: indexed.point.line.0,
            col: indexed.point.column.0,
            ch: indexed.cell.c,
        }),
    );

    terminal.reset_damage();
    snapshot
}

fn render_snapshot_cells(
    rows: usize,
    cols: usize,
    cells: impl IntoIterator<Item = SnapshotCell>,
) -> String {
    if rows == 0 || cols == 0 {
        return String::new();
    }

    let mut lines = vec![vec![' '; cols]; rows];

    for cell in cells {
        if cell.line < 0 {
            continue;
        }
        let line = cell.line as usize;
        if line >= rows || cell.col >= cols {
            continue;
        }

        lines[line][cell.col] = sanitize_cell_char(cell.ch);
    }

    let mut out = String::with_capacity(rows * (cols + 1));
    for (index, row) in lines.into_iter().enumerate() {
        let end = row
            .iter()
            .rposition(|ch| *ch != ' ')
            .map_or(0, |position| position + 1);
        for ch in row.into_iter().take(end) {
            out.push(ch);
        }

        if index + 1 < rows {
            out.push('\n');
        }
    }

    out
}

fn sanitize_cell_char(ch: char) -> char {
    if ch == '\0' || ch == '\u{fe0f}' || ch.is_control() {
        ' '
    } else {
        ch
    }
}

#[cfg(test)]
mod tests {
    use super::{render_snapshot_cells, SnapshotCell};

    #[test]
    fn places_text_by_column_index() {
        let output = render_snapshot_cells(
            2,
            8,
            [
                SnapshotCell {
                    line: 0,
                    col: 3,
                    ch: 'A',
                },
                SnapshotCell {
                    line: 0,
                    col: 4,
                    ch: 'B',
                },
                SnapshotCell {
                    line: 1,
                    col: 1,
                    ch: 'X',
                },
            ],
        );

        assert_eq!(output, "   AB\n X");
    }

    #[test]
    fn trims_only_trailing_spaces() {
        let output = render_snapshot_cells(
            1,
            6,
            [SnapshotCell {
                line: 0,
                col: 2,
                ch: 'Z',
            }],
        );

        assert_eq!(output, "  Z");
    }

    #[test]
    fn skips_out_of_bounds_cells() {
        let output = render_snapshot_cells(
            2,
            4,
            [
                SnapshotCell {
                    line: -1,
                    col: 0,
                    ch: 'A',
                },
                SnapshotCell {
                    line: 99,
                    col: 0,
                    ch: 'B',
                },
                SnapshotCell {
                    line: 0,
                    col: 99,
                    ch: 'C',
                },
                SnapshotCell {
                    line: 1,
                    col: 2,
                    ch: 'Q',
                },
            ],
        );

        assert_eq!(output, "\n  Q");
    }

    #[test]
    fn sanitizes_control_and_keeps_unicode() {
        let output = render_snapshot_cells(
            1,
            5,
            [
                SnapshotCell {
                    line: 0,
                    col: 0,
                    ch: '\u{0007}',
                },
                SnapshotCell {
                    line: 0,
                    col: 1,
                    ch: '\0',
                },
                SnapshotCell {
                    line: 0,
                    col: 2,
                    ch: '\u{fe0f}',
                },
                SnapshotCell {
                    line: 0,
                    col: 3,
                    ch: 'ş',
                },
            ],
        );

        assert_eq!(output, "   ş");
    }
}
