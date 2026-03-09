use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
#[cfg(target_os = "windows")]
use std::os::windows::io::RawHandle;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tattoy_wezterm_surface::{CursorShape, CursorVisibility};
use tattoy_wezterm_term::color::{ColorPalette, SrgbaTuple};
use tattoy_wezterm_term::config::TerminalConfiguration;
use tattoy_wezterm_term::{CellAttributes, Terminal, TerminalSize};
#[cfg(all(target_os = "windows", test))]
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, ERROR_NO_MORE_FILES, FILETIME, HANDLE,
    INVALID_HANDLE_VALUE,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursor {
    pub x: usize,
    pub y: usize,
    pub shape: TerminalCursorShape,
    pub blinking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStyledRun {
    pub text: String,
    pub style: TerminalStyle,
    pub column: usize,
    pub display_width: usize,
}

impl TerminalStyledRun {
    fn blank(column: usize, display_width: usize, style: TerminalStyle) -> Self {
        Self {
            text: " ".repeat(display_width),
            style,
            column,
            display_width,
        }
    }

    fn is_blank(&self) -> bool {
        self.text.chars().all(|ch| ch == ' ')
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStyledCell {
    pub text: String,
    pub style: TerminalStyle,
    pub column: usize,
    pub display_width: usize,
}

impl TerminalStyledCell {
    fn blank(column: usize, style: TerminalStyle) -> Self {
        Self {
            text: " ".to_owned(),
            style,
            column,
            display_width: 1,
        }
    }

    pub fn covers_column(&self, column: usize) -> bool {
        let end_column = self.column.saturating_add(self.display_width.max(1));
        column >= self.column && column < end_column
    }

    pub fn rendered_text(&self) -> String {
        let mut rendered = if self.text.is_empty() {
            " ".to_owned()
        } else {
            self.text.clone()
        };
        if self.display_width > 1 {
            rendered.push_str(&" ".repeat(self.display_width - 1));
        }
        rendered
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalStyledLine {
    pub runs: Vec<TerminalStyledRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCursorLine {
    pub row: usize,
    pub cells: Vec<TerminalStyledCell>,
}

impl TerminalCursorLine {
    pub fn cell_covering_column(&self, column: usize) -> Option<&TerminalStyledCell> {
        self.cells.iter().find(|cell| cell.covers_column(column))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalSnapshot {
    pub lines: Vec<TerminalStyledLine>,
    pub cursor: Option<TerminalCursor>,
    pub cursor_line: Option<TerminalCursorLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalSelectionLine {
    pub width: usize,
    pub wraps_to_next: bool,
    pub cells: Vec<TerminalStyledCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalSelectionSnapshot {
    pub lines: Vec<TerminalSelectionLine>,
}

#[derive(Debug, Clone)]
pub struct TerminalUiEvent {
    pub terminal_id: u64,
    pub kind: TerminalUiEventKind,
}

#[derive(Debug, Clone)]
pub enum TerminalUiEventKind {
    Wakeup,
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
    child_killer: Arc<Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>>,
    child_pid: Option<u32>,
    child_creation_time: Option<u64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessSnapshotEntry {
    pid: u32,
    parent_pid: u32,
    creation_time: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootProcessTerminationPlan {
    VerifiedProcess(ProcessSnapshotEntry),
    DirectProcess(ProcessSnapshotEntry),
    FallbackToChildKiller,
    AlreadyExited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifiedProcessLookup {
    Verified(ProcessSnapshotEntry),
    Missing,
    Unverifiable,
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
        let child_killer = Arc::new(Mutex::new(child.clone_killer()));
        let child_pid = child.process_id();
        #[cfg(target_os = "windows")]
        let child_creation_time = child
            .as_raw_handle()
            .and_then(process_creation_time_from_raw_handle);
        #[cfg(not(target_os = "windows"))]
        let child_creation_time = None;

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
            child_killer,
            child_pid,
            child_creation_time,
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

    pub fn terminate(&self) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        let snapshot = snapshot_processes().ok();

        #[cfg(target_os = "windows")]
        if let Some(child_pid) = self.child_pid {
            if let Some(snapshot) = snapshot.as_deref() {
                if let Some(descendants) =
                    verified_process_tree_descendants(snapshot, child_pid, self.child_creation_time)
                {
                    best_effort_terminate_snapshot_entries(&descendants);
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            return finish_termination(&self.command_tx, self.kill_root_process_windows(snapshot));
        }

        #[cfg(not(target_os = "windows"))]
        finish_termination(&self.command_tx, self.kill_root_process())
    }

    pub fn latest_seqno(&self) -> usize {
        self.latest_seqno.load(Ordering::Relaxed)
    }

    fn kill_root_process_with_child_killer(&self) -> io::Result<()> {
        let mut child_killer = self
            .child_killer
            .lock()
            .map_err(|_| io::Error::other("child killer lock poisoned"))?;
        child_killer.kill()
    }

    #[cfg(not(target_os = "windows"))]
    fn kill_root_process(&self) -> io::Result<()> {
        self.kill_root_process_with_child_killer()
    }

    #[cfg(target_os = "windows")]
    fn kill_root_process_windows(
        &self,
        snapshot: Option<Vec<ProcessSnapshotEntry>>,
    ) -> io::Result<()> {
        let plan = root_process_termination_plan(
            snapshot.as_deref(),
            self.child_pid,
            self.child_creation_time,
        );

        let entry = match plan {
            RootProcessTerminationPlan::VerifiedProcess(entry)
            | RootProcessTerminationPlan::DirectProcess(entry) => entry,
            RootProcessTerminationPlan::FallbackToChildKiller => {
                return self.kill_root_process_with_child_killer();
            }
            RootProcessTerminationPlan::AlreadyExited => return Ok(()),
        };

        match terminate_snapshot_process(entry) {
            Ok(()) => Ok(()),
            Err(primary_err) => match self.kill_root_process_with_child_killer() {
                Ok(()) => Ok(()),
                Err(fallback_err)
                    if is_benign_process_exit_error(&primary_err)
                        || is_benign_process_exit_error(&fallback_err) =>
                {
                    Ok(())
                }
                Err(fallback_err) => Err(fallback_err),
            },
        }
    }
}

#[cfg(test)]
#[derive(Debug)]
struct NoopChildKiller;

#[cfg(test)]
impl portable_pty::ChildKiller for NoopChildKiller {
    fn kill(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(Self)
    }
}

#[cfg(test)]
pub(crate) fn test_terminal_runtime() -> TerminalRuntime {
    let dimensions = TerminalDimensions::default();
    let terminal = Terminal::new(
        dimensions.to_term_size(),
        Arc::new(AdeTerminalConfig),
        "test",
        "0",
        Box::new(std::io::sink()),
    );
    let latest_seqno = Arc::new(AtomicUsize::new(terminal.current_seqno()));

    TerminalRuntime {
        term: Arc::new(Mutex::new(terminal)),
        command_tx: crossbeam_channel::unbounded().0,
        latest_seqno,
        last_size: dimensions,
        child_killer: Arc::new(Mutex::new(Box::new(NoopChildKiller))),
        child_pid: None,
        child_creation_time: None,
    }
}

#[cfg(target_os = "windows")]
fn is_benign_process_exit_error(err: &io::Error) -> bool {
    matches!(err.kind(), io::ErrorKind::NotFound)
        || matches!(
            err.raw_os_error(),
            Some(code) if code == ERROR_INVALID_PARAMETER as i32
        )
}

fn root_process_termination_plan(
    snapshot: Option<&[ProcessSnapshotEntry]>,
    child_pid: Option<u32>,
    child_creation_time: Option<u64>,
) -> RootProcessTerminationPlan {
    let Some(child_pid) = child_pid else {
        return RootProcessTerminationPlan::FallbackToChildKiller;
    };
    let Some(child_creation_time) = child_creation_time else {
        return RootProcessTerminationPlan::FallbackToChildKiller;
    };

    if let Some(snapshot) = snapshot {
        return match verified_snapshot_root_process(snapshot, child_pid, child_creation_time) {
            VerifiedProcessLookup::Verified(entry) => {
                RootProcessTerminationPlan::VerifiedProcess(entry)
            }
            VerifiedProcessLookup::Missing => RootProcessTerminationPlan::AlreadyExited,
            VerifiedProcessLookup::Unverifiable => {
                RootProcessTerminationPlan::FallbackToChildKiller
            }
        };
    }

    RootProcessTerminationPlan::DirectProcess(ProcessSnapshotEntry {
        pid: child_pid,
        parent_pid: 0,
        creation_time: Some(child_creation_time),
    })
}

fn verified_process_entry(
    entries: &[ProcessSnapshotEntry],
    pid: u32,
    expected_creation_time: Option<u64>,
) -> Option<ProcessSnapshotEntry> {
    let expected_creation_time = expected_creation_time?;
    let entry = entries.iter().find(|entry| entry.pid == pid).copied()?;
    if entry.creation_time != Some(expected_creation_time) {
        return None;
    }
    Some(entry)
}

fn verified_snapshot_root_process(
    entries: &[ProcessSnapshotEntry],
    pid: u32,
    expected_creation_time: u64,
) -> VerifiedProcessLookup {
    let Some(entry) = entries.iter().find(|entry| entry.pid == pid).copied() else {
        return VerifiedProcessLookup::Missing;
    };

    match entry.creation_time {
        Some(actual_creation_time) if actual_creation_time == expected_creation_time => {
            VerifiedProcessLookup::Verified(entry)
        }
        Some(_) => VerifiedProcessLookup::Missing,
        None => VerifiedProcessLookup::Unverifiable,
    }
}

fn verified_process_tree_descendants(
    entries: &[ProcessSnapshotEntry],
    root_pid: u32,
    expected_root_creation_time: Option<u64>,
) -> Option<Vec<ProcessSnapshotEntry>> {
    let root_entry = verified_process_entry(entries, root_pid, expected_root_creation_time)?;
    let entries_by_pid = entries
        .iter()
        .copied()
        .map(|entry| (entry.pid, entry))
        .collect::<BTreeMap<_, _>>();
    if entries_by_pid.get(&root_pid) != Some(&root_entry) {
        return None;
    }

    let kill_order = process_tree_kill_order(entries, root_pid)?;
    Some(
        kill_order
            .into_iter()
            .filter(|pid| *pid != root_pid)
            .filter_map(|pid| entries_by_pid.get(&pid).copied())
            .collect(),
    )
}

#[cfg(target_os = "windows")]
fn snapshot_processes() -> io::Result<Vec<ProcessSnapshotEntry>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = WinHandle(snapshot);

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot.0, &mut entry) == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut entries = Vec::new();
        loop {
            entries.push(ProcessSnapshotEntry {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                creation_time: process_creation_time_by_pid(entry.th32ProcessID),
            });

            if Process32NextW(snapshot.0, &mut entry) == 0 {
                let err = GetLastError();
                if err == ERROR_NO_MORE_FILES {
                    break;
                }
                return Err(io::Error::from_raw_os_error(err as i32));
            }
        }

        Ok(entries)
    }
}

#[cfg(target_os = "windows")]
fn best_effort_terminate_snapshot_entries(entries: &[ProcessSnapshotEntry]) {
    best_effort_terminate_entries(entries, terminate_snapshot_process);
}

fn best_effort_terminate_entries<F>(entries: &[ProcessSnapshotEntry], mut terminate_entry: F)
where
    F: FnMut(ProcessSnapshotEntry) -> io::Result<()>,
{
    for entry in entries {
        let _ = terminate_entry(*entry);
    }
}

#[cfg(target_os = "windows")]
fn terminate_snapshot_process(entry: ProcessSnapshotEntry) -> io::Result<()> {
    let Some(expected_creation_time) = entry.creation_time else {
        return Ok(());
    };

    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            0,
            entry.pid,
        );
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = WinHandle(handle);

        let Some(actual_creation_time) = process_creation_time(handle.0) else {
            return Err(io::Error::last_os_error());
        };
        if actual_creation_time != expected_creation_time {
            return Ok(());
        }

        if TerminateProcess(handle.0, 1) == 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
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

fn finish_termination(
    command_tx: &Sender<RuntimeCommand>,
    kill_result: io::Result<()>,
) -> io::Result<()> {
    let _ = command_tx.send(RuntimeCommand::Shutdown);
    kill_result
}

pub fn try_terminal_snapshots(
    runtime: &TerminalRuntime,
) -> Option<(TerminalSnapshot, TerminalSelectionSnapshot)> {
    let terminal = runtime.term.lock().ok()?;
    Some(snapshots_from_terminal(&terminal))
}

pub fn try_terminal_selection_snapshot(
    runtime: &TerminalRuntime,
) -> Option<TerminalSelectionSnapshot> {
    try_terminal_snapshots(runtime).map(|(_, selection_snapshot)| selection_snapshot)
}

fn process_tree_kill_order(entries: &[ProcessSnapshotEntry], root_pid: u32) -> Option<Vec<u32>> {
    let mut pids = BTreeSet::new();
    let mut children_by_parent = BTreeMap::<u32, Vec<u32>>::new();

    for entry in entries {
        pids.insert(entry.pid);
        children_by_parent
            .entry(entry.parent_pid)
            .or_default()
            .push(entry.pid);
    }

    if !pids.contains(&root_pid) {
        return None;
    }

    let mut visited = BTreeSet::new();
    let mut depths = BTreeMap::new();
    let mut stack = vec![(root_pid, 0usize)];

    while let Some((pid, depth)) = stack.pop() {
        if !visited.insert(pid) {
            continue;
        }

        depths.insert(pid, depth);
        if let Some(children) = children_by_parent.get(&pid) {
            for &child_pid in children.iter().rev() {
                stack.push((child_pid, depth + 1));
            }
        }
    }

    let mut ordered = depths.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(pid, depth)| (Reverse(*depth), *pid));
    Some(ordered.into_iter().map(|(pid, _)| pid).collect())
}

#[cfg(target_os = "windows")]
fn process_creation_time_by_pid(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let handle = WinHandle(handle);
        process_creation_time(handle.0)
    }
}

#[cfg(target_os = "windows")]
fn process_creation_time_from_raw_handle(raw_handle: RawHandle) -> Option<u64> {
    process_creation_time(raw_handle as HANDLE)
}

#[cfg(target_os = "windows")]
fn process_creation_time(handle: HANDLE) -> Option<u64> {
    unsafe {
        let mut creation_time = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit_time = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel_time = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user_time = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };

        if GetProcessTimes(
            handle,
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        ) == 0
        {
            return None;
        }

        Some(
            (u64::from(creation_time.dwHighDateTime) << 32)
                | u64::from(creation_time.dwLowDateTime),
        )
    }
}

#[cfg(target_os = "windows")]
struct WinHandle(HANDLE);

#[cfg(target_os = "windows")]
impl Drop for WinHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn snapshot_from_terminal(terminal: &Terminal) -> TerminalSnapshot {
    let palette = terminal.palette();
    let screen = terminal.screen();
    let rows = screen.physical_rows;
    let cols = screen.physical_cols;

    if rows == 0 || cols == 0 {
        return TerminalSnapshot::default();
    }

    let total_rows = screen.scrollback_rows().max(rows);
    let viewport_top_row = total_rows.saturating_sub(rows);
    let default_style = default_style(&palette);
    let cursor = snapshot_cursor(terminal, rows, cols, viewport_top_row);
    let cursor_row = cursor.map(|cursor| cursor.y);
    let mut lines = Vec::with_capacity(total_rows);
    let mut cursor_line = None;

    screen.for_each_phys_line(|row_index, line| {
        if row_index >= total_rows {
            return;
        }

        while lines.len() < row_index {
            let snapshot_row = lines.len();
            let min_columns_to_keep = cursor_columns_to_keep(cursor, snapshot_row, cols);
            let (line, blank_cursor_line) =
                build_blank_line(default_style, min_columns_to_keep, snapshot_row, cursor_row);
            lines.push(line);
            if blank_cursor_line.is_some() {
                cursor_line = blank_cursor_line;
            }
        }

        let snapshot_row = lines.len();
        let min_columns_to_keep = cursor_columns_to_keep(cursor, snapshot_row, cols);
        let track_cursor_cells = cursor_row == Some(snapshot_row);
        let mut line_cells = track_cursor_cells.then(Vec::new);
        let mut runs = Vec::new();
        let mut next_column = 0usize;

        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                continue;
            }

            if col > next_column {
                push_blank_run(&mut runs, next_column, col - next_column, default_style);
                if let Some(cells) = line_cells.as_mut() {
                    append_blank_cells(cells, next_column, col - next_column, default_style);
                }
            }

            let style = resolve_style(cell.attrs(), &palette);
            let mut text = sanitize_cell_text(cell.str());
            if text.is_empty() {
                text.push(' ');
            }

            let display_width = cell.width().max(1).min(cols.saturating_sub(col));
            if display_width == 0 {
                continue;
            }

            let rendered_text = rendered_cell_text(&text, display_width);
            push_run(&mut runs, col, rendered_text, display_width, style);
            if let Some(cells) = line_cells.as_mut() {
                cells.push(TerminalStyledCell {
                    text,
                    style,
                    column: col,
                    display_width,
                });
            }
            next_column = col.saturating_add(display_width).min(cols);
        }

        if next_column < cols {
            push_blank_run(&mut runs, next_column, cols - next_column, default_style);
            if let Some(cells) = line_cells.as_mut() {
                append_blank_cells(cells, next_column, cols - next_column, default_style);
            }
        }

        trim_trailing_default_runs(&mut runs, default_style, min_columns_to_keep);
        if let Some(cells) = line_cells.as_mut() {
            trim_trailing_default_cells(cells, default_style, min_columns_to_keep);
        }

        lines.push(TerminalStyledLine { runs });

        if let Some(cells) = line_cells {
            cursor_line = Some(TerminalCursorLine {
                row: snapshot_row,
                cells,
            });
        }
    });

    while lines.len() < total_rows {
        let snapshot_row = lines.len();
        let min_columns_to_keep = cursor_columns_to_keep(cursor, snapshot_row, cols);
        let (line, blank_cursor_line) =
            build_blank_line(default_style, min_columns_to_keep, snapshot_row, cursor_row);
        lines.push(line);
        if blank_cursor_line.is_some() {
            cursor_line = blank_cursor_line;
        }
    }

    TerminalSnapshot {
        lines,
        cursor,
        cursor_line,
    }
}

fn selection_snapshot_from_terminal(terminal: &Terminal) -> TerminalSelectionSnapshot {
    let palette = terminal.palette();
    let screen = terminal.screen();
    let rows = screen.physical_rows;
    let cols = screen.physical_cols;
    let total_rows = screen.scrollback_rows().max(rows);
    let viewport_top_row = total_rows.saturating_sub(rows);
    let cursor = snapshot_cursor(terminal, rows, cols, viewport_top_row);
    let default_style = default_style(&palette);

    let mut lines = Vec::with_capacity(total_rows);

    screen.for_each_phys_line(|row_index, line| {
        if row_index >= total_rows {
            return;
        }

        while lines.len() < row_index {
            let snapshot_row = lines.len();
            lines.push(TerminalSelectionLine {
                width: cursor_columns_to_keep(cursor, snapshot_row, cols),
                wraps_to_next: false,
                cells: Vec::new(),
            });
        }

        let snapshot_row = lines.len();
        let min_columns_to_keep = cursor_columns_to_keep(cursor, snapshot_row, cols);
        let mut width = min_columns_to_keep;
        let mut cells = Vec::new();

        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                continue;
            }

            let style = resolve_style(cell.attrs(), &palette);
            let mut text = sanitize_cell_text(cell.str());
            if text.is_empty() {
                text.push(' ');
            }

            let display_width = cell.width().max(1).min(cols.saturating_sub(col));
            if display_width == 0 {
                continue;
            }

            width = width.max(col.saturating_add(display_width).min(cols));
            cells.push(TerminalStyledCell {
                text,
                style,
                column: col,
                display_width,
            });
        }

        trim_trailing_default_cells(&mut cells, default_style, min_columns_to_keep);
        lines.push(TerminalSelectionLine {
            width,
            wraps_to_next: line.last_cell_was_wrapped(),
            cells,
        });
    });

    while lines.len() < total_rows {
        let snapshot_row = lines.len();
        lines.push(TerminalSelectionLine {
            width: cursor_columns_to_keep(cursor, snapshot_row, cols),
            wraps_to_next: false,
            cells: Vec::new(),
        });
    }

    TerminalSelectionSnapshot { lines }
}

fn snapshots_from_terminal(terminal: &Terminal) -> (TerminalSnapshot, TerminalSelectionSnapshot) {
    (
        snapshot_from_terminal(terminal),
        selection_snapshot_from_terminal(terminal),
    )
}

fn snapshot_cursor(
    terminal: &Terminal,
    rows: usize,
    cols: usize,
    viewport_top_row: usize,
) -> Option<TerminalCursor> {
    if rows == 0 || cols == 0 {
        return None;
    }

    let cursor = terminal.cursor_pos();
    if cursor.visibility != CursorVisibility::Visible {
        return None;
    }

    let row = usize::try_from(cursor.y).ok()?;
    if row >= rows {
        return None;
    }

    let (shape, blinking) = map_cursor_shape(cursor.shape);
    Some(TerminalCursor {
        x: cursor.x.min(cols.saturating_sub(1)),
        y: viewport_top_row.saturating_add(row),
        shape,
        blinking,
    })
}

fn map_cursor_shape(shape: CursorShape) -> (TerminalCursorShape, bool) {
    match shape {
        CursorShape::Default => (TerminalCursorShape::Block, true),
        CursorShape::BlinkingBlock => (TerminalCursorShape::Block, true),
        CursorShape::SteadyBlock => (TerminalCursorShape::Block, false),
        CursorShape::BlinkingUnderline => (TerminalCursorShape::Underline, true),
        CursorShape::SteadyUnderline => (TerminalCursorShape::Underline, false),
        CursorShape::BlinkingBar => (TerminalCursorShape::Bar, true),
        CursorShape::SteadyBar => (TerminalCursorShape::Bar, false),
    }
}

fn cursor_columns_to_keep(
    cursor: Option<TerminalCursor>,
    visible_row: usize,
    cols: usize,
) -> usize {
    cursor
        .filter(|cursor| cursor.y == visible_row)
        .map_or(0, |cursor| cursor.x.saturating_add(1).min(cols))
}

fn build_blank_line(
    default_style: TerminalStyle,
    min_columns_to_keep: usize,
    visible_row: usize,
    cursor_row: Option<usize>,
) -> (TerminalStyledLine, Option<TerminalCursorLine>) {
    let mut runs = Vec::new();
    let mut cells = Vec::new();
    if min_columns_to_keep > 0 {
        push_blank_run(&mut runs, 0, min_columns_to_keep, default_style);
        append_blank_cells(&mut cells, 0, min_columns_to_keep, default_style);
    }

    let cursor_line = if cursor_row == Some(visible_row) {
        Some(TerminalCursorLine {
            row: visible_row,
            cells: cells.clone(),
        })
    } else {
        None
    };

    (TerminalStyledLine { runs }, cursor_line)
}

fn push_blank_run(
    runs: &mut Vec<TerminalStyledRun>,
    column: usize,
    count: usize,
    style: TerminalStyle,
) {
    if count == 0 {
        return;
    }
    if let Some(previous_run) = runs.last_mut() {
        let previous_end = previous_run
            .column
            .saturating_add(previous_run.display_width);
        if previous_run.style == style && previous_run.is_blank() && previous_end == column {
            previous_run.text.push_str(&" ".repeat(count));
            previous_run.display_width += count;
            return;
        }
    }
    runs.push(TerminalStyledRun::blank(column, count, style));
}

fn push_run(
    runs: &mut Vec<TerminalStyledRun>,
    column: usize,
    text: String,
    display_width: usize,
    style: TerminalStyle,
) {
    if display_width == 0 || text.is_empty() {
        return;
    }

    if let Some(previous_run) = runs.last_mut() {
        let previous_end = previous_run
            .column
            .saturating_add(previous_run.display_width);
        if previous_run.style == style && previous_end == column {
            previous_run.text.push_str(&text);
            previous_run.display_width += display_width;
            return;
        }
    }

    runs.push(TerminalStyledRun {
        text,
        style,
        column,
        display_width,
    });
}

fn append_blank_cells(
    cells: &mut Vec<TerminalStyledCell>,
    start_column: usize,
    count: usize,
    style: TerminalStyle,
) {
    for offset in 0..count {
        cells.push(TerminalStyledCell::blank(start_column + offset, style));
    }
}

fn trim_trailing_default_runs(
    runs: &mut Vec<TerminalStyledRun>,
    default: TerminalStyle,
    min_columns_to_keep: usize,
) {
    while let Some(run) = runs.last_mut() {
        let run_end = run.column.saturating_add(run.display_width.max(1));
        if run_end <= min_columns_to_keep {
            break;
        }

        if run.style != default || !run.is_blank() {
            break;
        }

        let keep_width = min_columns_to_keep.saturating_sub(run.column);
        if keep_width == 0 {
            runs.pop();
            continue;
        }

        run.text.truncate(keep_width);
        run.display_width = keep_width;
        break;
    }
}

fn trim_trailing_default_cells(
    cells: &mut Vec<TerminalStyledCell>,
    default: TerminalStyle,
    min_columns_to_keep: usize,
) {
    while let Some(cell) = cells.last() {
        let cell_end = cell.column.saturating_add(cell.display_width.max(1));
        if cell_end <= min_columns_to_keep {
            break;
        }

        if cell.style != default || cell.text != " " || cell.display_width != 1 {
            break;
        }

        cells.pop();
    }
}

fn rendered_cell_text(text: &str, display_width: usize) -> String {
    let mut rendered = if text.is_empty() {
        " ".to_owned()
    } else {
        text.to_owned()
    };
    if display_width > 1 {
        rendered.push_str(&" ".repeat(display_width - 1));
    }
    rendered
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
        best_effort_terminate_entries, default_style, finish_termination,
        is_benign_process_exit_error, process_tree_kill_order, root_process_termination_plan,
        sanitize_cell_text, selection_snapshot_from_terminal, snapshot_from_terminal,
        snapshots_from_terminal, trim_trailing_default_cells, verified_process_entry,
        verified_process_tree_descendants, verified_snapshot_root_process, AdeTerminalConfig,
        ProcessSnapshotEntry, RootProcessTerminationPlan, RuntimeCommand, TerminalColor,
        TerminalCursor, TerminalCursorLine, TerminalCursorShape, TerminalStyle, TerminalStyledCell,
        VerifiedProcessLookup,
    };
    use std::{io, sync::Arc};
    use tattoy_wezterm_term::color::ColorPalette;
    use tattoy_wezterm_term::{Terminal, TerminalSize};

    fn snapshot_entry(
        pid: u32,
        parent_pid: u32,
        creation_time: Option<u64>,
    ) -> ProcessSnapshotEntry {
        ProcessSnapshotEntry {
            pid,
            parent_pid,
            creation_time,
        }
    }

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
        let mut cells = vec![
            TerminalStyledCell {
                text: "x".to_owned(),
                style,
                column: 0,
                display_width: 1,
            },
            TerminalStyledCell::blank(1, default),
            TerminalStyledCell::blank(2, default),
        ];

        trim_trailing_default_cells(&mut cells, default, 1);

        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].column, 0);
    }

    #[test]
    fn trimming_preserves_columns_reserved_for_cursor() {
        let default = default_style(&ColorPalette::default());
        let mut cells = vec![
            TerminalStyledCell::blank(0, default),
            TerminalStyledCell::blank(1, default),
            TerminalStyledCell::blank(2, default),
        ];

        trim_trailing_default_cells(&mut cells, default, 2);

        assert_eq!(cells.len(), 2);
        assert_eq!(cells[1].column, 1);
    }

    #[test]
    fn wide_cells_pad_rendered_text_to_match_display_width() {
        let style = default_style(&ColorPalette::default());
        let cell = TerminalStyledCell {
            text: "\u{4f60}".to_owned(),
            style,
            column: 0,
            display_width: 2,
        };

        assert_eq!(cell.rendered_text(), "\u{4f60} ");
    }

    #[test]
    fn process_tree_kill_order_prefers_children_before_root() {
        let entries = vec![
            snapshot_entry(1, 0, None),
            snapshot_entry(2, 1, None),
            snapshot_entry(3, 1, None),
            snapshot_entry(4, 2, None),
            snapshot_entry(5, 2, None),
        ];

        assert_eq!(
            process_tree_kill_order(&entries, 1),
            Some(vec![4, 5, 2, 3, 1])
        );
    }

    #[test]
    fn process_tree_kill_order_excludes_unrelated_processes() {
        let entries = vec![
            snapshot_entry(10, 0, None),
            snapshot_entry(11, 10, None),
            snapshot_entry(20, 0, None),
            snapshot_entry(21, 20, None),
        ];

        assert_eq!(process_tree_kill_order(&entries, 10), Some(vec![11, 10]));
    }

    #[test]
    fn process_tree_kill_order_returns_root_when_it_has_no_children() {
        let entries = vec![snapshot_entry(42, 0, None)];

        assert_eq!(process_tree_kill_order(&entries, 42), Some(vec![42]));
    }

    #[test]
    fn process_tree_kill_order_returns_none_when_root_is_missing() {
        let entries = vec![snapshot_entry(7, 0, None)];

        assert_eq!(process_tree_kill_order(&entries, 99), None);
    }

    #[test]
    fn verified_process_tree_descendants_require_matching_root_creation_time() {
        let entries = vec![
            snapshot_entry(1, 0, Some(200)),
            snapshot_entry(2, 1, Some(300)),
        ];

        assert_eq!(
            verified_process_tree_descendants(&entries, 1, Some(100)),
            None
        );
    }

    #[test]
    fn verified_process_entry_requires_matching_creation_time() {
        let entries = vec![snapshot_entry(1, 0, Some(200))];

        assert_eq!(verified_process_entry(&entries, 1, Some(100)), None);
        assert_eq!(
            verified_process_entry(&entries, 1, Some(200)),
            Some(snapshot_entry(1, 0, Some(200)))
        );
    }

    #[test]
    fn verified_snapshot_root_process_falls_back_when_creation_time_is_unavailable() {
        let entries = vec![snapshot_entry(1, 0, None)];

        assert_eq!(
            verified_snapshot_root_process(&entries, 1, 200),
            VerifiedProcessLookup::Unverifiable
        );
    }

    #[test]
    fn root_process_termination_plan_falls_back_without_identity_metadata() {
        let entries = vec![snapshot_entry(1, 0, Some(200))];

        assert_eq!(
            root_process_termination_plan(Some(&entries), None, Some(200)),
            RootProcessTerminationPlan::FallbackToChildKiller
        );
        assert_eq!(
            root_process_termination_plan(Some(&entries), Some(1), None),
            RootProcessTerminationPlan::FallbackToChildKiller
        );
    }

    #[test]
    fn root_process_termination_plan_marks_missing_snapshot_entry_as_exited() {
        let entries = vec![snapshot_entry(2, 1, Some(300))];

        assert_eq!(
            root_process_termination_plan(Some(&entries), Some(1), Some(200)),
            RootProcessTerminationPlan::AlreadyExited
        );
    }

    #[test]
    fn root_process_termination_plan_falls_back_when_snapshot_entry_lacks_creation_time() {
        let entries = vec![snapshot_entry(1, 0, None)];

        assert_eq!(
            root_process_termination_plan(Some(&entries), Some(1), Some(200)),
            RootProcessTerminationPlan::FallbackToChildKiller
        );
    }

    #[test]
    fn root_process_termination_plan_uses_direct_process_without_snapshot() {
        assert_eq!(
            root_process_termination_plan(None, Some(1), Some(200)),
            RootProcessTerminationPlan::DirectProcess(snapshot_entry(1, 0, Some(200)))
        );
    }

    #[test]
    fn verified_process_tree_descendants_exclude_root_when_identity_matches() {
        let entries = vec![
            snapshot_entry(1, 0, Some(100)),
            snapshot_entry(2, 1, Some(200)),
            snapshot_entry(3, 1, Some(300)),
        ];

        assert_eq!(
            verified_process_tree_descendants(&entries, 1, Some(100)),
            Some(vec![
                snapshot_entry(2, 1, Some(200)),
                snapshot_entry(3, 1, Some(300))
            ])
        );
    }

    #[test]
    fn best_effort_terminate_entries_continues_after_error() {
        let entries = vec![
            snapshot_entry(2, 1, Some(200)),
            snapshot_entry(3, 1, Some(300)),
        ];
        let mut attempted = Vec::new();

        best_effort_terminate_entries(&entries, |entry| {
            attempted.push(entry.pid);
            if entry.pid == 2 {
                Err(io::Error::from_raw_os_error(5))
            } else {
                Ok(())
            }
        });

        assert_eq!(attempted, vec![2, 3]);
    }

    #[test]
    fn finish_termination_sends_shutdown_after_success() {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();

        assert!(finish_termination(&command_tx, Ok(())).is_ok());
        assert!(matches!(
            command_rx.try_recv(),
            Ok(RuntimeCommand::Shutdown)
        ));
    }

    #[test]
    fn finish_termination_sends_shutdown_after_failure() {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let err = io::Error::other("kill failed");

        let result = finish_termination(&command_tx, Err(err));

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
        assert!(matches!(
            command_rx.try_recv(),
            Ok(RuntimeCommand::Shutdown)
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn benign_process_exit_errors_are_treated_as_success_races() {
        assert!(is_benign_process_exit_error(&io::Error::from_raw_os_error(
            super::ERROR_INVALID_PARAMETER as i32
        )));
        assert!(is_benign_process_exit_error(&io::Error::new(
            io::ErrorKind::NotFound,
            "gone"
        )));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn non_benign_process_errors_still_surface() {
        assert!(!is_benign_process_exit_error(
            &io::Error::from_raw_os_error(super::ERROR_ACCESS_DENIED as i32)
        ));
        assert!(!is_benign_process_exit_error(
            &io::Error::from_raw_os_error(5_4321)
        ));
        assert!(!is_benign_process_exit_error(&io::Error::new(
            io::ErrorKind::PermissionDenied,
            "denied"
        )));
    }

    #[test]
    fn verified_process_tree_descendants_return_none_when_root_is_missing() {
        let entries = vec![snapshot_entry(2, 1, Some(200))];

        assert_eq!(
            verified_process_tree_descendants(&entries, 1, Some(100)),
            None
        );
    }

    #[test]
    fn snapshot_coalesces_adjacent_default_style_cells_into_single_run() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 4,
            cols: 10,
            pixel_width: 80,
            pixel_height: 64,
            dpi: 96,
        });
        terminal.advance_bytes(b"abc\x1b[?25l");

        let snapshot = snapshot_from_terminal(&terminal);

        assert_eq!(snapshot.lines[0].runs.len(), 1);
        assert_eq!(snapshot.lines[0].runs[0].text, "abc");
        assert_eq!(snapshot.lines[0].runs[0].display_width, 3);
    }

    #[test]
    fn snapshot_includes_scrollback_lines_in_history() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 2,
            cols: 12,
            pixel_width: 96,
            pixel_height: 32,
            dpi: 96,
        });
        terminal.advance_bytes(b"first\r\nsecond\r\nthird");

        let snapshot = snapshot_from_terminal(&terminal);

        assert_eq!(snapshot_line_text(&snapshot.lines[0]), "first");
        assert_eq!(snapshot_line_text(&snapshot.lines[1]), "second");
        assert_eq!(snapshot_line_text(&snapshot.lines[2]), "third");
    }

    #[test]
    fn selection_snapshot_marks_soft_wrapped_lines() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 2,
            cols: 5,
            pixel_width: 40,
            pixel_height: 32,
            dpi: 96,
        });
        terminal.advance_bytes(b"abcdef");

        let snapshot = selection_snapshot_from_terminal(&terminal);

        assert!(snapshot.lines[0].wraps_to_next);
        assert!(!snapshot.lines[1].wraps_to_next);
    }

    #[test]
    fn selection_snapshot_keeps_filler_rows_unwrapped() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 3,
            cols: 5,
            pixel_width: 40,
            pixel_height: 48,
            dpi: 96,
        });
        terminal.advance_bytes(b"abc");

        let snapshot = selection_snapshot_from_terminal(&terminal);

        assert!(!snapshot.lines[1].wraps_to_next);
        assert!(!snapshot.lines[2].wraps_to_next);
    }

    #[test]
    fn paired_snapshots_capture_same_terminal_state() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 2,
            cols: 5,
            pixel_width: 40,
            pixel_height: 32,
            dpi: 96,
        });
        terminal.advance_bytes(b"abcdef");

        let (render_snapshot, selection_snapshot) = snapshots_from_terminal(&terminal);

        assert_eq!(snapshot_line_text(&render_snapshot.lines[0]), "abcde");
        assert_eq!(snapshot_line_text(&render_snapshot.lines[1]), "f");
        assert!(selection_snapshot.lines[0].wraps_to_next);
        assert_eq!(selection_snapshot.lines[0].width, 5);
        assert_eq!(selection_snapshot.lines[1].cells[0].text, "f");
    }

    #[test]
    fn snapshot_offsets_cursor_row_by_scrollback_history() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 2,
            cols: 12,
            pixel_width: 96,
            pixel_height: 32,
            dpi: 96,
        });
        terminal.advance_bytes(b"first\r\nsecond\r\nthird");

        let snapshot = snapshot_from_terminal(&terminal);
        let cursor = snapshot.cursor.expect("expected cursor");

        assert_eq!(cursor.y, 2);
        assert_eq!(
            snapshot.cursor_line.as_ref().map(|line| line.row),
            Some(cursor.y)
        );
        assert_eq!(snapshot_line_text(&snapshot.lines[cursor.y]), "third");
    }

    #[test]
    fn snapshot_preserves_ansi_foreground_color() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 4,
            cols: 40,
            pixel_width: 320,
            pixel_height: 64,
            dpi: 96,
        });
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

    #[test]
    fn snapshot_preserves_cursor_position_and_shape() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 4,
            cols: 10,
            pixel_width: 80,
            pixel_height: 64,
            dpi: 96,
        });
        terminal.advance_bytes(b"\x1b[2;6H\x1b[6 q");

        let snapshot = snapshot_from_terminal(&terminal);

        assert_eq!(
            snapshot.cursor,
            Some(TerminalCursor {
                x: 5,
                y: 1,
                shape: TerminalCursorShape::Bar,
                blinking: false,
            })
        );
        assert!(snapshot
            .cursor_line
            .as_ref()
            .and_then(|line| line.cell_covering_column(5))
            .is_some());
    }

    #[test]
    fn snapshot_treats_default_cursor_shape_as_blinking_block() {
        let terminal = make_test_terminal(TerminalSize {
            rows: 4,
            cols: 10,
            pixel_width: 80,
            pixel_height: 64,
            dpi: 96,
        });

        let snapshot = snapshot_from_terminal(&terminal);

        assert_eq!(
            snapshot.cursor,
            Some(TerminalCursor {
                x: 0,
                y: 0,
                shape: TerminalCursorShape::Block,
                blinking: true,
            })
        );
    }

    #[test]
    fn snapshot_hides_cursor_when_terminal_requests_it() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 4,
            cols: 10,
            pixel_width: 80,
            pixel_height: 64,
            dpi: 96,
        });
        terminal.advance_bytes(b"\x1b[?25l");

        let snapshot = snapshot_from_terminal(&terminal);

        assert_eq!(snapshot.cursor, None);
    }

    #[test]
    fn snapshot_preserves_wide_cell_width() {
        let mut terminal = make_test_terminal(TerminalSize {
            rows: 4,
            cols: 10,
            pixel_width: 80,
            pixel_height: 64,
            dpi: 96,
        });
        terminal.advance_bytes("\u{4f60}".as_bytes());

        let snapshot = snapshot_from_terminal(&terminal);
        let first_run = &snapshot.lines[0].runs[0];

        assert_eq!(first_run.display_width, 2);
        assert_eq!(first_run.text, "\u{4f60} ");
    }

    #[test]
    fn cursor_line_preserves_cell_level_details_for_cursor_row() {
        let default = default_style(&ColorPalette::default());
        let cursor_line = TerminalCursorLine {
            row: 0,
            cells: vec![
                TerminalStyledCell::blank(0, default),
                TerminalStyledCell {
                    text: "\u{4f60}".to_owned(),
                    style: default,
                    column: 1,
                    display_width: 2,
                },
            ],
        };

        let cell = cursor_line
            .cell_covering_column(2)
            .expect("expected wide cell");
        assert_eq!(cell.column, 1);
        assert_eq!(cell.display_width, 2);
    }

    fn make_test_terminal(size: TerminalSize) -> Terminal {
        Terminal::new(
            size,
            Arc::new(AdeTerminalConfig),
            "test",
            "0",
            Box::new(std::io::sink()),
        )
    }

    fn snapshot_line_text(line: &super::TerminalStyledLine) -> String {
        line.runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }
}
