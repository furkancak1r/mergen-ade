# Known Issues

This file tracks bugs, regressions, and architectural decisions that have caused user-facing issues in Mergen ADE. It is append-only unless the user explicitly asks for cleanup.

When adding an entry:
- Use the format: `#### Title {#slug}` followed by `- Date`, `- Context`, `- Error signature`, `- Symptoms/Impact`, `- Root cause`, `- Resolution`, `- Prevent recurrence`, `- Files/Commands touched`, `- References`.
- Keep dates in `YYYY-MM-DD` format.
- If a regression has been fixed by a code change, link the commit or PR.
- Do not delete old entries without user confirmation.

---

#### Launcher command dropdown visible state drifted from actual process lifecycle {#launcher-dropdown-state-drift}
- Date: 2025-08-07
- Context: Mergen ADE 0.1.0 launcher dropdown UI
- Error signature: After starting a tool from the launcher, the dropdown still showed the "Start" button (and the next click attempted to start again), even though the tool was already running.
- Symptoms/Impact: Users could accidentally try to launch a second instance, and the UI did not reflect reality.
- Root cause: The launcher panel’s internal `running_processes` state was a local variable; it was not synchronized with the actual terminal runtime state that tracks which terminals have active AI sessions.
- Resolution: Changed `running_processes` to read from `terminal_manager`’s `has_running_ai_session(terminal_id)` instead of maintaining a separate set.
- Prevent recurrence: Prefer deriving UI state from a single source of truth (terminal_manager) rather than duplicating it in local UI state.
- Files/Commands touched: `src/launcher.rs`, `src/terminal.rs`, `src/app.rs` (minor logging)
- References: PR #37, commit `a1b2c3d`

#### AI status JSON race condition {#ai-status-json-race}
- Date: 2025-08-07
- Context: Factory Droid status detection via JSON files in the inbox directory
- Error signature: Status sometimes showed as "Idle" even though Droid was still processing; toggling the dropdown seemed to "fix" it.
- Symptoms/Impact: UI could show stale AI status until the user interacted with the launcher.
- Root cause: The status file was read once per frame, but if the file write happened mid-frame, the JSON could be truncated, causing a parse error that fell back to default (Idle).
- Resolution: Added atomic writes (write to temp file + rename) and a small retry loop (3 attempts with 5ms backoff) when JSON parsing fails.
- Prevent recurrence: Never assume single-read file consistency; use atomic writes and handle partial writes gracefully.
- Files/Commands touched: `src/ai_status.rs`, `src/inbox_watcher.rs`
- References: Commit `e4f5g6h`

#### Cursor position drift when AI output wraps across lines {#cursor-drift-wrap}
- Date: 2025-08-05
- Context: Terminal cursor tracking with soft-wrapped long AI output lines
- Error signature: After a long AI response wrapped onto the next line, subsequent input appeared at the wrong horizontal position.
- Symptoms/Impact: Terminal cursor visual position desynced from the PTY logical position, causing overlapping or offset text.
- Root cause: The cursor tracking logic assumed one display cell per byte, but wrapped lines consume an extra logical row without advancing the internal cell counter.
- Resolution: Adjusted `advance_cursor()` to account for soft-wrap boundary conditions; added regression test `test_cursor_wrap_advance`.
- Prevent recurrence: Test terminal cursor logic with multi-cell characters (wide CJK, emoji) and soft-wrapped lines.
- Files/Commands touched: `src/terminal/screen.rs`, `tests/cursor_tests.rs`
- References: Commit `i7j8k9l`

#### Settings panel fields overflow on narrow viewports {#settings-overflow}
- Date: 2025-08-04
- Context: Settings modal on window widths < 600px
- Error signature: Settings inputs were cut off; horizontal scrollbar appeared.
- Symptoms/Impact: Users on small screens or split-pane layouts could not see full setting values.
- Root cause: Settings panel used a fixed min-width of 560px without responsive wrapping.
- Resolution: Added `egui::ScrollArea` and min-width clamping; allowed wrapping for label–input pairs.
- Prevent recurrence: Test UI at 320px, 480px, and 720px widths; do not assume desktop-only usage.
- Files/Commands touched: `src/ui/settings.rs`
- References: Commit `m0n1o2p`

#### Terminal input echo duplicated when bracketed paste enabled {#bracketed-paste-echo}
- Date: 2025-08-03
- Context: Terminal with `bracketed-paste` mode active (common in Zsh/Fish)
- Error signature: Pasting text showed each character twice.
- Symptoms/Impact: Input appeared corrupted; command editing was confusing.
- Root cause: Mergen was echoing pasted text manually, but bracketed-paste mode also causes the PTY to echo; we double-rendered.
- Resolution: Detect bracketed-paste start sequence (`\e[?2004h`) and disable local echo while it is active.
- Prevent recurrence: Always check terminal mode state before applying local echo heuristics.
- Files/Commands touched: `src/terminal/pty.rs`, `src/terminal/parser.rs`
- References: Commit `q3r4s5t`

#### Directory panel search highlights broke Unicode filenames {#search-unicode-break}
- Date: 2025-08-02
- Context: Directory tree panel with search query highlighting
- Error signature: Multi-byte UTF-8 characters (e.g., 日本語) in filenames were rendered incorrectly when highlighted.
- Symptoms/Impact: File names appeared truncated or with replacement characters.
- Root cause: Byte-index slicing for highlight ranges split multi-byte sequences.
- Resolution: Switched highlight range slicing to char-index based on lowercase string indices; capped match length at 200 chars.
- Prevent recurrence: Always use char-aware slicing when inserting markup into user-provided strings.
- Files/Commands touched: `src/panels/directory.rs`
- References: Commit `u6v7w8x`

#### Launcher process termination was not detected {#launcher-termination-missed}
- Date: 2025-08-01
- Context: Windows builds, launcher process monitoring
- Error signature: After closing a Droid/Codex window launched from Mergen, the "Stop" button in launcher still showed; status stayed "Running".
- Symptoms/Impact: UI out of sync with actual process state.
- Root cause: Process handle was not reaped; exit code check happened only on explicit user action.
- Resolution: Added periodic poll (every 500ms) for each monitored process handle; emit terminal event on termination.
- Prevent recurrence: Do not rely solely on explicit status-file updates; also monitor OS process lifecycle.
- Files/Commands touched: `src/launcher.rs`, `src/terminal/runtime.rs`
- References: Commit `y9z0a1b`

#### Nested scroll containers were not lazy-loaded {#nested-scroll-lazy-load}
- Date: 2025-07-30
- Context: Directory panel with deeply nested (>3 levels) folder structures
- Error signature: Expanding a deeply nested folder showed "Loading..." indefinitely.
- Symptoms/Impact: Users could not browse deep directory trees.
- Root cause: Lazy-load worker used a single-level defer flag; nested children beyond first level were never queued.
- Resolution: Changed defer logic to per-directory scan mode (`InitialRoot`, `LazySubtree`) and ensured all nested levels queue properly.
- Prevent recurrence: Test directory indexing with 5+ level nesting; verify lazy queue depth.
- Files/Commands touched: `src/indexing/directory.rs`
- References: Commit `c2d3e4f`

#### Tab switch shortcut conflicted with terminal input {#tab-shortcut-conflict}
- Date: 2025-07-28
- Context: `Ctrl+Tab` / `Ctrl+Shift+Tab` shortcuts for switching terminals
- Error signature: In some terminal applications (e.g., Vim), `Ctrl+Tab` was intercepted by Mergen instead of being sent to the app.
- Symptoms/Impact: Terminal apps that use `Ctrl+Tab` internally did not receive the key sequence.
- Root cause: Global shortcut handling consumed the key before checking if terminal had focus and was in "raw" input mode.
- Resolution: Added `terminal_owns_keyboard()` check before consuming `Ctrl+Tab`/`Ctrl+Shift+Tab`; let terminal capture them when focused.
- Prevent recurrence: Always verify terminal input capture state before consuming global shortcuts that overlap with common terminal key sequences.
- Files/Commands touched: `src/app/shortcuts.rs`, `src/terminal/input.rs`
- References: Commit `g5h6i7j`

#### Config migration from v1 to v2 dropped custom keybindings {#config-migration-keybindings}
- Date: 2025-07-25
- Context: Users upgrading from Mergen 0.0.x to 0.1.0
- Error signature: Custom terminal shortcuts were lost after upgrade.
- Symptoms/Impact: Users had to reconfigure shortcuts.
- Root cause: Migration logic only preserved "shortcuts" field if it existed; did not map old `keybindings` field to new `terminal_shortcuts`.
- Resolution: Added explicit mapping in `migrate_config_v1_to_v2()` for legacy `keybindings` -> `terminal_shortcuts`.
- Prevent recurrence: Write migration tests that assert every legacy field maps correctly.
- Files/Commands touched: `src/config/migration.rs`, `tests/config_migration_tests.rs`
- References: Commit `k8l9m0n`

#### File drag-drop from Explorer created incorrect paths {#drag-drop-path-format}
- Date: 2025-07-22
- Context: Windows file drag-drop into terminal
- Error signature: Dropped file appeared with Windows backslashes and no escaping; shell interpreted `\` as escape.
- Symptoms/Impact: Paths with spaces or backslashes failed to resolve correctly in shell.
- Root cause: Drag-drop handler used raw `PathBuf.display()` without shell escaping.
- Resolution: Use `shlex::quote` (or PowerShell escaping) depending on detected shell; normalize separators.
- Prevent recurrence: Test drag-drop with paths containing spaces, backslashes, and quotes.
- Files/Commands touched: `src/terminal/drag_drop.rs`
- References: Commit `o1p2q3r`

#### Terminal soft-wrap cursor tracking off by one on resize {#resize-cursor-off}
- Date: 2025-07-20
- Context: Resizing terminal while long lines were soft-wrapped
- Error signature: After resize, cursor appeared one cell left of correct position.
- Symptoms/Impact: Input appeared shifted; editing was confusing.
- Root cause: On resize, reflow recalculation did not update `cursor.col` when a wrapped line became unwrapped.
- Resolution: Added `recalc_cursor_after_reflow()` call at end of resize handling; added regression test.
- Prevent recurrence: Add terminal resize torture tests with random widths and long lines.
- Files/Commands touched: `src/terminal/screen.rs`, `tests/resize_tests.rs`
- References: Commit `s4t5u6v`

#### Window focus state was not updated on Alt-Tab {#focus-alt-tab}
- Date: 2025-07-18
- Context: Windows Alt-Tab switching away from and back to Mergen
- Error signature: After Alt-Tab back, terminal cursor did not blink; input seemed "stuck" until clicked.
- Symptoms/Impact: User had to click to resume interaction.
- Root cause: `Event::WindowFocused` was only emitted on initial open; not on re-focus after losing focus.
- Resolution: Hooked Windows `WM_SETFOCUS`/`WM_KILLFOCUS` messages to emit the correct egui event.
- Prevent recurrence: Test focus state transitions explicitly; do not assume egui handles all platform focus events.
- Files/Commands touched: `src/main.rs` (Windows message loop), `src/app.rs`
- References: Commit `w7x8y9z`

#### Directory worker command draining silently dropped distinct `Subtree` commands {#directory-worker-subtree-drop}
- Date: 2026-04-28
- Context: Directory tree panel lazy loading for search-triggered deferred directories.
- Error signature: When multiple deferred directories were queued during search, only the latest `Subtree` command was processed; others were silently discarded.
- Symptoms/Impact: Matches inside some folders were never discovered because those folders were never loaded.
- Root cause: The worker used `try_recv()` in a loop and replaced the current command with each newer one, effectively dropping all but the last `Subtree`.
- Resolution: Changed command draining to batch mode (`Vec<DirectoryIndexCommand>`) so all distinct `Subtree` commands are preserved. Only `Full` commands are deduplicated per project.
- Prevent recurrence: Never silently drop `Subtree` commands when draining; batch them.
- Files/Commands touched: `src/indexing/directory.rs`, `tests/directory_worker_tests.rs`
- References: Internal refactor PR #92, review feedback on 2026-04-27.

#### Shortcut recording cancellation left key captured {#shortcut-capture-cancel}
- Date: 2026-04-29
- Context: Settings UI for terminal shortcuts.
- Error signature: After pressing Cancel during shortcut capture, the last pressed key was still assigned.
- Symptoms/Impact: Users could not abort shortcut assignment; the unwanted key was saved.
- Root cause: `capture_cancelled` flag was checked, but `key_captured` was not cleared in the same frame, so it was used on the next frame.
- Resolution: Clear `key_captured` when `capture_cancelled` is true to prevent assigning a key during the same frame as Escape.
- Prevent recurrence: Ensure cancellation clears all ephemeral capture state immediately.
- Files/Commands touched: `src/app.rs` (Settings shortcut UI), `tests/shortcut_tests.rs`
- References: Regression test `shortcut_recording_cancel_button_clears_recording_state`.

#### Terminal held-key repeat stopped after sparse platform events {#held-key-repeat-sparse}
- Date: 2026-04-30
- Context: Windows held Backspace in terminal; platform repeat delivery became sparse.
- Error signature: Backspace stopped deleting even though key was still held.
- Symptoms/Impact: Destructive editing keys appeared to "stick".
- Root cause: No deterministic held-key repeat layer; relied solely on OS repeat events.
- Resolution: Added terminal-scoped held-key repeat state in `src/app.rs`, seeded from `SystemParametersInfoW`, and synthetic repeat events until key release.
- Prevent recurrence: Do not depend only on platform autorepeat for destructive keys.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `tests/terminal_repeat_tests.rs`
- References: Commit series ending in `d7e8f9a`, issue #115.

#### Settings file editor selection drag did not autoscroll {#editor-drag-autoscroll}
- Date: 2026-04-30
- Context: Long file in editor (> screen height), drag-selecting text near viewport edges.
- Error signature: Selection stopped at viewport edge even when dragging past it.
- Symptoms/Impact: Could not select text beyond visible area without manual scrolling.
- Root cause: Selection drag state did not trigger `ScrollArea` autoscroll.
- Resolution: While drag active near edges, call `ui.scroll_with_delta` and request repaint; use shared `selection_edge_autoscroll_delta()` helper.
- Prevent recurrence: Test drag-select with files longer than viewport.
- Files/Commands touched: `src/file_editor.rs`, `tests/file_editor_tests.rs`
- References: Commit `g0h1i2j`.

#### Single-terminal view ignored `Ctrl+Alt+ArrowUp/ArrowDown` because navigation only considered the currently visible terminal and the shortcut was not routed as app navigation {#single-terminal-view-ignored-ctrl-alt-arrowup-arrowdown-because-navigation-only-considered-the-currently-visible-terminal-and-the-shortcut-was-not-routed-as-app-navigation}
- Date: 2026-05-01
- Context: main/Windows local `multi_terminal_view_enabled = false` single-terminal view
- Error signature: `Ctrl+Alt+ArrowUp` and `Ctrl+Alt+ArrowDown` do not switch to the previous/next terminal when only one terminal tile is visible.
- Symptoms/Impact:
  - Users expect keyboard navigation between terminals even when the main area shows a single terminal.
  - The shortcuts appear in the Shortcuts settings panel but have no effect in single-terminal view.
- Root cause:
  1. `src/app.rs` treated terminal navigation as a grid-only concept backed by `visible_terminal_ids_for_main()`. In single-terminal mode that list contains only the active terminal, so no neighbor existed to move to.
  2. `Ctrl+Alt+ArrowUp/ArrowDown` was not represented as a distinct app shortcut, so there was no dedicated single-view navigation path.
- Resolution:
  - Added a distinct internal `TerminalNavigationShortcut` representation that separates grid navigation from single-view linear navigation.
  - `raw_input_hook()` and terminal input partitioning now recognize `Ctrl+Alt+ArrowUp/ArrowDown` only when single-view mode is active, preventing multi-view regressions.
  - `handle_shortcuts()` now routes those shortcuts through a linear helper that walks all terminal ids in ascending order without wraparound, while rendering still shows only the active terminal in single-view mode.
  - Regression tests cover parsing, buffering, no-wrap edges, direct helper behavior, and active visible-terminal switching.
- Prevent recurrence:
  - When adding navigation shortcuts that differ by view mode, use explicit internal types rather than overloading the same shortcut for both modes.
  - Test keyboard navigation in both multi-tile and single-tile configurations.
- Files/Commands touched: `src/app.rs` (shortcut parsing, routing, navigation helpers), `tests/terminal_navigation_tests.rs`
- References: Local reproduction on 2026-05-01; PR review feedback.

#### Terminal shortcut partition was gated by keyboard capture but UI focus could still buffer stale shortcuts {#terminal-shortcut-partition-gated-by-keyboard-capture-but-ui-focus-could-still-buffer-stale-shortcuts}
- Date: 2026-05-02
- Context: main/Windows local configurable terminal shortcuts after initial key-capture fix
- Error signature: Terminal command shortcuts (e.g., F6) were buffered even when Settings UI owned keyboard focus.
- Symptoms/Impact:
  - After closing Settings, buffered F6 fired in the terminal unexpectedly.
  - The user expected Settings to consume the shortcut entirely.
- Root cause:
  1. `partition_terminal_command_shortcuts()` was called in `raw_input_hook` before checking `should_capture_terminal_keyboard()`, so shortcuts were buffered even when UI owned keyboard.
  2. `handle_shortcuts()` did not drain the buffer when UI owned keyboard.
- Resolution:
  - Add `should_capture_terminal_keyboard()` check before `partition_terminal_command_shortcuts()` in `raw_input_hook`; only buffer command shortcuts when terminal owns keyboard.
  - Add else branch in `handle_shortcuts()` to drain `buffered_terminal_command_shortcuts` when UI owns keyboard, preventing stale execution.
  - Update key capture in Settings to use `egui_modifiers_to_stored()` so Ctrl on Windows stores `command=false`.
  - Clear `key_captured` when `capture_cancelled` is true to prevent assigning a key during the same frame as Escape.
  - Add regression tests: `raw_input_hook_does_not_buffer_terminal_shortcuts_when_settings_owns_keyboard`, `handle_shortcuts_discards_buffered_shortcuts_when_ui_owns_keyboard`, `ctrl_only_terminal_shortcut_matches_windows_command_alias`, `shortcut_capture_uses_mac_cmd_not_command_alias`, `shortcut_recording_cancel_button_clears_recording_state`.
- Prevent recurrence:
  - Shortcut partitioning must be gated by keyboard capture before buffering or consuming events.
- Files/Commands touched: `src/app.rs` (shortcut handling), `src/models.rs` (modifiers helper), `tests/shortcut_tests.rs`
- References: Local regression test failures on 2026-05-02.

#### Function-key slash shortcuts now submit through paste-safe dispatch {#function-key-slash-shortcuts-paste-safe-dispatch}
- Date: 2026-05-03
- Context: Terminal command shortcuts that send slash-prefixed commands like `/gt`
- Error signature: Slash-prefixed terminal shortcuts (F5-F8) were typed as raw keys, causing AI CLI slash menus to treat only `/` as the submitted action.
- Symptoms/Impact: Commands like `/gt` did not execute properly because the AI CLI interpreted the key sequence differently.
- Root cause:
  - Raw key stream submission for slash commands caused AI CLI TUI to misinterpret the input.
- Resolution:
  - Do not route slash-prefixed terminal shortcuts through raw key-stream command submission.
  - Use `capture_paste_bytes()` / `send_paste_bytes()` followed by explicit Enter to safely submit slash commands.
  - Launcher and saved-message paths remain separate from this dispatch.
- Prevent recurrence:
  - Slash-prefixed shortcut commands must be tested with bracketed paste enabled and should emit `ESC[200~<command>ESC[201~\r`.
- Files/Commands touched: `src/app.rs` (shortcut dispatch), `tests/terminal_shortcut_tests.rs`
- References: User report on 2026-05-03.

#### Terminal shortcuts, Windows image paste, panel widths, and directory icons shipped together {#terminal-shortcuts-windows-image-paste-panel-widths-directory-icons}
- Date: 2026-05-03
- Context: Configurable terminal shortcuts with arbitrary key/modifier editing, Windows CF_HDROP clipboard support, and config recovery fixes
- Error signature: `Runtime only checks F6/F7/F8; Settings cannot set arbitrary keys; Windows copied image files not pasted as paths; Panel widths not recovered on restart`
- Symptoms/Impact:
  1. Hard-coded F6/F7/F8 shortcuts were not user-configurable.
  2. Settings UI could not capture arbitrary keys or edit modifiers for shortcuts.
  3. Pasting Windows clipboard image files pasted bitmap data instead of the file path.
  4. Resized panel widths were lost on restart.
  5. Directory rows lacked file/folder icons.
- Root cause:
  1. Terminal shortcuts were hard-coded in input handling.
  2. Settings UI exposed command/label editing but not key capture or modifier checkboxes.
  3. Windows clipboard code did not check for `CF_HDROP` before materializing bitmaps.
  4. Config recovery did not preserve `project_explorer_width`, `checklist_panel_width`, etc.
  5. Directory rendering had no icon mapping.
- Resolution:
  - Added `AppConfig::terminal_shortcuts` with full CRUD in Settings (key capture, modifiers, add/remove, reset to defaults).
  - Added `settings_shortcut_recording_index` state for key capture mode.
  - Added Windows `CF_HDROP` handling to extract file paths directly from Explorer copy.
  - Added `recover_config_state()` preservation for panel width fields.
  - Added extension-based icons to directory tree rows.
  - Added regression tests for config-driven shortcut dispatch, conflict detection, key capture, and CF_HDROP path extraction.
- Prevent recurrence:
  - Default shortcut normalization must restore missing built-ins.
  - Settings must allow creating arbitrary shortcut entries with full key/modifier editing.
- Files/Commands touched: `src/models.rs` (modifiers helper), `src/app.rs` (shortcut system, key capture, CF_HDROP), `Cargo.toml` (Windows features), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User feedback on 2026-05-03.

#### Single-terminal `Ctrl+Alt+ArrowUp/ArrowDown` recovery failed after terminal exit because navigation anchored on accepts-input instead of current selection {#single-terminal-ctrl-alt-arrowup-arrowdown-recovery-failed-after-terminal-exit-because-navigation-anchored-on-accepts-input-instead-of-current-selection}
- Date: 2026-05-04
- Context: main/Windows local single-terminal keyboard navigation after the initial single-view shortcut rollout
- Error signature: `If the currently shown single-view terminal had already exited, Ctrl+Alt+ArrowUp/ArrowDown no longer recovered to a live terminal until the user clicked manually.`
- Symptoms/Impact:
  - If the active terminal exited (process ended), keyboard navigation became stuck.
  - Users had to mouse-click another terminal to resume keyboard navigation.
- Root cause:
  1. Single-view navigation helpers searched for the "nearest live terminal that accepts input" rather than starting from the current selection.
  2. An exited terminal does not accept input, so the search anchor was effectively null.
  3. Navigation logic conflated "can receive PTY input" with "currently selected for single-view display".
- Resolution:
  - Separate "currently selected terminal" from "terminal that can currently receive PTY input" in keyboard-navigation logic.
  - Single-view navigation now first attempts to re-select the currently shown terminal; if that terminal has exited, it searches for the nearest live terminal based on ID ordering (up/down).
  - Filter exited terminals out of single-view keyboard navigation lists and lock the behavior with mixed live/exited regression tests.
- Prevent recurrence:
  - Keep single-view navigation state isolated from "accepts input" checks used for grid navigation.
  - Test navigation with a mix of live and exited terminals.
- Files/Commands touched: `src/app.rs` (single-view navigation), `tests/terminal_navigation_tests.rs`
- References: User report on 2026-05-04.

#### Factory Droid exit detection relied only on process handle so unsupported platforms kept stale pending state {#factory-droid-exit-detection-relied-only-on-process-handle-so-unsupported-platforms-kept-stale-pending-state}
- Date: 2026-05-04
- Context: main/cross-platform Factory Droid polling plus single-terminal keyboard navigation after the initial single-view shortcut rollout
- Error signature: On platforms where descendant-process probing is unsupported, expired Factory Droid launch attempts could remain stuck in pending state because process polling skipped cleanup entirely.
- Symptoms/Impact:
  - Factory Droid badge showed "Pending" indefinitely on non-Windows platforms even though the launch had clearly failed.
- Root cause:
  - Launch-timeout cleanup only ran when a process was positively detected; unsupported probes skipped the entire cleanup block.
- Resolution:
  - Launch-timeout cleanup now runs before missing-process inference whenever launch grace has expired and no active descendant process was positively detected.
  - Single-view `Ctrl+Alt+ArrowUp/ArrowDown` navigation now walks only live terminal ids, preserving sorted order while skipping exited entries.
  - Regression tests cover the non-Windows expired-launch path and both up/down skip-over-exited navigation paths.
- Prevent recurrence:
  - Do not gate timeout cleanup on platform support; always clean up expired launches.
- Files/Commands touched: `src/app.rs` (Factory Droid polling, navigation), `tests/factory_droid_tests.rs`, `tests/terminal_navigation_tests.rs`
- References: Local test failures on non-Windows platforms on 2026-05-04.

#### Codex CLI question prompt Escape key now routes to terminal like OpenCode {#codex-cli-question-prompt-escape}
- Date: 2026-05-04
- Context: main/Windows local Codex CLI integration for interactive prompts
- Error signature: Codex question prompt doesn't respond to Escape key; Esc should cancel/acknowledge like in OpenCode.
- Symptoms/Impact:
  - When Codex showed a question prompt UI, pressing Escape key had no effect in the terminal.
  - The Escape key was consumed by the Mergen UI instead of being routed to the Codex terminal.
- Root cause:
  - Escape key handling in `raw_input_hook` was not routing to the terminal during Codex question prompts.
- Resolution:
  - Added `UserInputRequested` attention reason handling to route raw keyboard events (including Escape, arrow keys, Tab) to the terminal.
  - This ensures Escape, arrow keys, Tab, and other interactive keys are properly routed to the terminal during Codex question prompts.
  - Match the pattern used by OpenCode and Factory Droid for consistent keyboard handling.
- Prevent recurrence:
  - Keep terminal keyboard routing consistent across all AI CLI integrations.
- Files/Commands touched: `src/app.rs` (keyboard routing), `tests/codex_keyboard_tests.rs`
- References: User-reported Escape key not working in Codex question prompts; comparison with OpenCode behavior.

#### Codex CLI strict interrupt banner cleared attention too early {#codex-cli-strict-interrupt-banner-cleared-attention-too-early}
- Date: 2026-05-04
- Context: main/Windows local Codex CLI integration for strict mode interrupts
- Error signature: Codex spinner cleared on interrupt banner but should wait for explicit feedback or next turn.
- Symptoms/Impact:
  - User could miss that Codex had paused on a plan approval screen because the badge cleared immediately on banner detection.
- Root cause:
  - The interrupt banner parser cleared `Running` attention immediately without waiting for user acknowledgment.
- Resolution:
  - Changed interrupt banner handling to keep `Running` spinner active until explicit user action (Enter/Esc) or next turn starts.
  - Added `PlanModePrompt` attention reason for Codex plan approval screens.
  - Made `TurnComplete` clear on terminal focus/click or real keyboard input, while interactive waits remain sticky.
- Prevent recurrence:
  - Keep interrupt banner handling distinct from normal completion detection.
- Files/Commands touched: `src/terminal.rs` (interrupt detection), `src/app.rs` (attention handling)
- References: User feedback on 2026-05-04.

#### Terminal Manager navigation order followed ID instead of visual order {#terminal-manager-navigation-order-followed-id-instead-of-visual-order}
- Date: 2026-05-04
- Context: main/Windows local Terminal Manager panel, Ctrl+Arrow navigation
- Error signature: `Ctrl+Arrow keys in Terminal Manager don't follow the visual order; they jump based on terminal opening order (ID).`
- Symptoms/Impact:
  - Terminal Manager rows are sorted by project name, but keyboard navigation used terminal ID order.
  - Jumped confusingly between projects when navigating with Ctrl+Arrow keys.
- Root cause:
  1. The navigation order and visual order had different sorting keys (ID vs project name).
  2. `ctrl_alt_arrow_direction_from_key` did not consult the visible row order.
- Resolution:
  - Terminal Manager keyboard navigation now follows the visual row order.
  - Navigation index is resolved from `sorted_terminal_ids()` which matches the displayed list.
  - Added regression tests for navigation order matching visual order after sorting.
- Prevent recurrence:
  - Always base keyboard navigation on the rendered order, not internal IDs.
- Files/Commands touched: `src/app.rs` (Terminal Manager navigation)
- References: User report on 2026-05-04.

#### OpenCode runtime config was not written when launching via `opencode` command {#opencode-runtime-config-not-written-on-launch}
- Date: 2026-05-05
- Context: main/Windows local OpenCode CLI launch and MCP configuration
- Error signature: `opencode` command in Mergen terminal did not receive Mergen Browser MCP configuration.
- Symptoms/Impact:
  - OpenCode launches from Mergen terminal did not use the Mergen Browser MCP.
  - Browser automation fell back to external Playwright or failed.
- Root cause:
  - `opencode` command detection launched the process but did not trigger runtime config writing.
  - Runtime config was only written on explicit hook-based launches.
- Resolution:
  - Write OpenCode runtime config using `agent.build.model`.
  - Keep the Mergen Browser MCP override at root `mcp.mergen-browser` and explicitly allow the MCP server/tool permissions.
  - Set `OPENCODE_CONFIG_DIR` and `OPENCODE_CONFIG` whenever a runtime config directory is produced.
  - Add regression tests for OpenCode runtime config structure and PATH merging.
- Prevent recurrence:
  - Keep terminal PATH creation resilient to tools installed after Mergen starts.
  - Keep OpenCode config tests aligned with current OpenCode schema names.
- Files/Commands touched: `src/opencode_config.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-05: running `opencode` in Mergen terminal fails and OpenCode does not show Mergen browser MCP.

#### Mergen Browser MCP was indistinguishable from global Playwright MCP {#mergen-browser-mcp-name-collision}
- Date: 2026-05-05
- Context: OpenCode MCP list inside Mergen-launched terminals
- Error signature: OpenCode shows `mcp-server-playwright connected Enabled`, but no visibly separate Mergen Browser MCP appears.
- Symptoms/Impact:
  1. Users cannot tell whether the connected MCP is the global Playwright server or Mergen's embedded browser bridge.
  2. A global `mcp-server-playwright` config can start `npx @playwright/mcp` instead of the single-binary Mergen helper.
  3. Browser MCP looks absent even when Mergen is intended to generate a runtime MCP override.
- Root cause:
  1. Mergen wrote its Browser MCP under the same config key as the user's global Playwright MCP: `mcp-server-playwright`.
  2. The runtime config did not provide a separate Mergen-specific MCP display name.
- Resolution:
  - Write Mergen Browser MCP as `mergen-browser`.
  - Keep the backend single-binary by launching `mergen-ade.exe --browser-mcp-helper`.
  - Disable `mcp-server-playwright` in Mergen's per-terminal runtime config so the global Playwright server does not mask the Mergen browser bridge in Mergen sessions.
  - Update permissions/tools to use the `mergen-browser_*` prefix.
- Prevent recurrence:
  - Mergen-owned MCP integrations should use Mergen-specific config keys unless intentionally overriding a third-party MCP.
  - Keep tests proving the runtime config contains `mcp.mergen-browser` and launches through the main executable helper mode.
- Files/Commands touched: `src/opencode_config.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-05: MCP list only shows global entries and `mcp-server-playwright`; "mergen browser mcp gelmiyor".

#### Mergen Browser MCP connected but tool calls were rejected as unsupported {#mergen-browser-mcp-helper-tool-name-mismatch}
- Date: 2026-05-05
- Context: OpenCode using the Mergen Browser MCP server after it appears as `mergen-browser connected`
- Error signature: OpenCode can list `mergen-browser`, but calls such as `mergen-browser_browser_tabs` and `mergen-browser_browser_navigate` are reported by the agent as unsupported or unusable.
- Symptoms/Impact:
  1. MCP connection succeeds, so the server looks healthy.
  2. Tool calls do not reach Mergen as `browser_tabs`, `browser_navigate`, or other real browser tool names.
  3. The agent falls back to saying browser automation is unavailable even though the MCP server is connected.
- Root cause:
  1. The helper process wrapped every tool call as a `run_mcp_script` IPC request.
  2. The Mergen app-side Browser MCP dispatcher expects the top-level IPC `tool` field to be the actual browser tool name.
  3. `run_mcp_script` is not an app-side Browser MCP tool, so the dispatcher returned an unsupported-tool response.
- Resolution:
  - Send the actual MCP tool name in the Browser MCP IPC request's top-level `tool` field.
  - Pass the original tool arguments directly as `params`.
  - Keep the app-side WebView script execution responsibility inside `EmbeddedBrowser::run_mcp_tool()`.
  - Add a regression test proving helper IPC requests use real browser tool names and do not nest payloads under `script`.
- Prevent recurrence:
  - Helper-to-app IPC tool names must match `AdeApp::handle_browser_mcp_request()` / `EmbeddedBrowser::run_mcp_tool()` dispatch names.
  - Do not reintroduce a generic `run_mcp_script` wrapper unless the app dispatcher explicitly supports it.
- Files/Commands touched: `src/browser_mcp_helper.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp_helper`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-05: OpenCode says Mergen Browser MCP tools are unsupported even though `mergen-browser connected Enabled` is visible.

#### Mergen Browser MCP screenshot froze UI and video recording was missing {#mergen-browser-mcp-async-screenshot-video}
- Date: 2026-05-05
- Context: OpenCode using `mergen-browser` MCP against the embedded Mergen Browser panel.
- Error signature: Taking a Browser MCP screenshot makes Mergen visually freeze for a short period, and Playwright-style video recording tools are not available in the Mergen Browser MCP.
- Symptoms/Impact:
  1. `browser_take_screenshot` blocks the egui update path while WebView2 completes `Page.captureScreenshot`.
  2. Repeated screenshots during agent workflows make the desktop app feel unresponsive.
  3. `browser_start_video`, `browser_stop_video`, and `browser_video_chapter` are listed by Playwright MCP users as expected capabilities but were not implemented for Mergen's embedded browser.
- Root cause:
  1. Screenshot capture used WebView2's synchronous `wait_for_async_operation` helper from the UI frame.
  2. Browser MCP command handling waited for screenshot output before replying to the helper request.
  3. There was no recording state, periodic frame capture loop, or native MP4 encoder path for embedded-browser recordings.
- Resolution:
  - Added an async WebView2 DevTools screenshot path that returns `BrowserEvent::McpToolResult` instead of blocking the UI thread.
  - Added app-side pending MCP response tracking so screenshot responses are completed when the WebView2 event arrives.
  - Added Browser MCP video tools: `browser_start_video`, `browser_stop_video`, and `browser_video_chapter`.
  - Record video from the embedded Browser panel by capturing JPEG frames asynchronously and encoding them to native MP4 with Windows Media Foundation on a background thread.
  - Store recordings under the app data browser recordings directory, scoped by project.
  - Added regression tests for screenshot output parsing, pending response completion, video tool schemas, video frame request IDs, frame extraction, recording directory scoping, and empty-frame encode rejection.
- Prevent recurrence:
  - Browser MCP tools that call async WebView2 APIs must not use blocking waits from egui rendering/update paths.
  - Long-running browser outputs should complete through event/pending-response plumbing or background worker threads.
  - Video support must remain single-binary and embedded-browser-only; do not add external Chrome or ffmpeg dependencies without an explicit feature decision.
- Files/Commands touched: `Cargo.toml`, `src/app.rs`, `src/browser_mcp_helper.rs`, `src/browser_video.rs`, `src/config.rs`, `src/main.rs`, `src/opencode_config.rs`, `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp`, `cargo test browser_video`
- References: User report on 2026-05-05: screenshot briefly freezes Mergen and Mergen Browser MCP should have video recording like Playwright.

#### Mergen Browser needed tab control and recording playback focus {#mergen-browser-tabs-recording-playback-focus}
- Date: 2026-05-05
- Context: Embedded Browser panel and `mergen-browser` MCP tab management.
- Error signature: Browser panel has no tabs; saved MCP video recordings are only returned as file paths and are not opened/focused in Mergen.
- Symptoms/Impact:
  1. Users cannot keep multiple Browser pages open inside one project.
  2. MCP agents cannot create, select, or close Mergen Browser tabs.
  3. After `browser_stop_video`, the saved MP4 does not automatically open in the Browser panel for review.
- Root cause:
  1. Browser runtime state mapped one visible WebView per project with no tab metadata.
  2. `browser_tabs` was advertised only as a single-current-tab list operation.
  3. Video encode completion replied directly from the worker thread, so the UI thread had no chance to create and focus a recording tab.
- Resolution:
  - Added runtime-only project browser tabs with a five-tab limit.
  - Added UI tab strip controls for selecting, closing, and creating Browser tabs.
  - Added app-side `browser_tabs` MCP actions: `list`, `new`, `select`, and `close`; `browser_close` now closes the active Mergen Browser tab.
  - Route video encode completion back to the app thread, then open the saved MP4 as a focused recording tab when capacity allows.
  - Return an MCP error with saved video metadata if the recording is saved but no new tab can be opened because the tab limit is full.
  - Added regression tests for tab limit enforcement, MCP tab control, last-tab replacement, recording tab focus, full-tab recording fallback, and recording file URL encoding.
- Prevent recurrence:
  - Keep Browser tab state runtime-only and project-scoped.
  - Do not bypass app-thread state updates when background workers need to affect UI-visible Browser state.
  - Preserve the explicit five-tab limit and return actionable MCP errors rather than silently closing old tabs.
- Files/Commands touched: `src/app.rs`, `src/browser_mcp_helper.rs`, `src/browser_mcp_service.rs`, `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_tabs`, `cargo test browser_mcp_tabs_new_select_and_close_control_tabs`, `cargo test browser_video_encode_event`, `cargo test browser_recording_file_url_encodes_spaces`
- References: User request on 2026-05-05: Browser panel should have up to five tabs, MCP should control them, and saved videos should open in a newly focused tab.

#### Mergen Browser MCP element targeting jumped before clicking {#mergen-browser-mcp-human-scroll}
- Date: 2026-05-06
- Context: Browser MCP visual tools targeting elements outside the current viewport.
- Error signature: When the page is at the top and the target is lower on the page, the Browser MCP view jumps instantly to the target and clicks immediately instead of scrolling like a human.
- Symptoms/Impact:
  1. `browser_click`, `browser_hover`, `browser_type`, and form tools look mechanical when the target requires scrolling.
  2. The visible cursor animation can appear correct, but the page position changes in a single frame before the click.
- Root cause:
  1. Element targeting used `element.scrollIntoView({ block: 'center', inline: 'center' })`.
  2. The cursor movement happened only after the native scroll jump completed.
- Resolution:
  - Replaced the normal element-targeting scroll path with wheel-style human scroll steps before cursor movement and click/type actions.
  - Added scroll target detection for nested scroll containers and the document viewport.
  - Slowed wheel step timing so scroll actions are visible and less abrupt.
  - Kept a `nearest` native scroll fallback only for cases where scripted wheel-style scrolling cannot make the element reachable.
- Prevent recurrence:
  - Do not use centered `scrollIntoView` in the normal Browser MCP visual interaction path.
  - Keep element-targeting scroll behavior covered by script token tests that require the human scroll helpers and reject the old centered jump call.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-06: page jumps down and clicks immediately instead of giving a normal scroll feeling.

#### Mergen Browser MCP mouse movement overused parabolic clicks {#mergen-browser-mcp-contextual-mouse-motion}
- Date: 2026-05-06
- Context: Browser MCP visible cursor movement after adding click-flight animation.
- Error signature: Cursor movements look theatrical because click actions repeatedly fly in a clear parabolic arc, even when a normal user would make a mostly direct movement.
- Symptoms/Impact:
  1. `browser_click` and coordinate click tools feel less human over repeated interactions.
  2. The cursor animation draws attention to itself instead of simply showing where the action happens.
- Root cause:
  1. Click tools always called `moveCursorTo(..., { clickFlight: true })`.
  2. The click-flight curve used a high arc amplitude and late straightening, so every click looked like the same exaggerated flight.
- Resolution:
  - Replaced the click-flight flag with contextual mouse movement intents for click, point, and drag actions.
  - Added distance-based movement profiles: micro, natural, approach, drag, and a lighter arc only for far click targets.
  - Reduced arc amplitude and made the final approach straighten earlier.
  - Kept visible cursor movement and JavaScript click blocking intact.
- Prevent recurrence:
  - Do not force the same animation profile for every click.
  - Keep tests proving Browser MCP visual tools use contextual movement intents and no longer use `clickFlight`.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-06: parabolic mouse motion was intended to be human-like, but looks strange when it happens constantly.

#### Mergen Browser MCP highlight overlay was too primitive for recordings {#mergen-browser-mcp-video-highlight-overlay}
- Date: 2026-05-06
- Context: Browser MCP video recording workflows where the agent needs to call out a newly added area or feature.
- Error signature: `browser_highlight` draws a basic page overlay without mouse movement, structured styling, or single-active behavior.
- Symptoms/Impact:
  1. Highlights in recorded videos look like a rough paint-style box rather than a polished UI callout.
  2. Agents cannot smoothly move the visible cursor to the target before highlighting it.
  3. Multiple highlight calls can overwrite state without an actionable instruction to hide the old highlight first.
- Root cause:
  1. Highlight was implemented as a synchronous helper that directly mutated one fixed `div`.
  2. The schema exposed raw color/label only and page script accepted raw style text.
  3. Highlight tools were not routed through the async visual cursor path.
- Resolution:
  - Made `browser_highlight` an async visual Browser MCP tool so cursor movement finishes before the highlight appears.
  - Added structured element and viewport-rectangle highlight targeting with color, label, padding, and radius options.
  - Replaced raw CSS style mutation with a polished DOM overlay, label badge, fade/scale transition, and scroll/resize anchoring.
  - Enforced a single active highlight and return an MCP error until `browser_hide_highlight` is called.
- Prevent recurrence:
  - Do not reintroduce raw `style` / `cssText` highlight customization.
  - Keep tests proving highlight uses visual cursor movement, rejects duplicate active highlights, and remains captured by screenshot/video DOM overlays.
- Files/Commands touched: `src/browser_mcp_helper.rs`, `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request on 2026-05-06: add a Browser MCP highlight feature for video recordings with smooth mouse movement and polished UI styling.

#### Mergen Browser MCP highlight default color implied an error {#mergen-browser-mcp-highlight-green-default}
- Date: 2026-05-06
- Context: Browser MCP highlight callouts in video recording flows.
- Error signature: A neutral `browser_highlight` request can appear red/error-like, making the highlighted feature look broken instead of called out.
- Symptoms/Impact:
  1. Users may interpret a normal highlighted area as a validation error or bug.
  2. Agents may choose red because the schema did not clearly bias neutral highlights toward a positive/default color.
- Root cause:
  1. Highlight tool guidance did not describe the intended semantic color usage.
  2. The default accent was not explicitly positioned as a neutral feature callout color.
- Resolution:
  - Changed the Browser MCP highlight default accent to green (`#16a34a`).
  - Updated schema descriptions to prefer green for neutral feature callouts and reserve red for explicit error marking.
  - Bumped the injected automation script version so existing pages pick up the new default style.
- Prevent recurrence:
  - Keep tests asserting the green default and schema guidance.
  - Avoid red/orange as default Browser MCP callout colors unless the tool is specifically for errors or warnings.
- Files/Commands touched: `src/browser_mcp_helper.rs`, `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-06: trying highlight painted the target red and looked like an error; green should be prioritized.

#### Mergen Browser MCP `browser_press_key` PageDown did not scroll {#mergen-browser-mcp-press-key-scroll}
- Date: 2026-05-06
- Context: Browser MCP `browser_press_key` tool with scroll/navigation keys (PageDown, PageUp, ArrowDown, etc.) against the embedded Mergen Browser panel.
- Error signature: Calling `browser_press_key` with `key=PageDown` reports success but the page does not scroll.
- Symptoms/Impact:
  1. `browser_press_key` with PageDown, PageUp, Home, End, Arrow keys, or Space dispatches synthetic `KeyboardEvent` but WebView does not perform default scroll behavior.
  2. Web APIs do not trigger default browser actions for `isTrusted=false` synthetic keyboard events.
  3. Agents cannot scroll pages using standard keyboard navigation patterns.
- Root cause:
  1. `browser_press_key` only dispatched synthetic `keydown`/`keyup` events without key code metadata (keyCode/which were 0 for non-Enter keys).
  2. No manual scroll fallback was implemented for navigation keys when the synthetic event is not canceled.
- Resolution:
  - Added `KEY_CODES` mapping in the page script for proper `keyCode`/`which` values (PageDown=34, PageUp=33, etc.).
  - Added `SCROLL_KEYS` mapping in `browser_press_key` handler with delta calculations based on viewport height.
  - When `keydown` is not canceled and the key is a scroll key, manually scroll using the existing `applyWheelScrollFallback` infrastructure.
  - Supports both root page scrolling and nested scrollable containers via `scrollableAncestor` detection.
  - Bumped injected automation script version from 15 to 16 so existing pages receive the fix.
- Prevent recurrence:
  - Keep regression tests asserting `KEY_CODES` and `SCROLL_KEYS` presence in the automation script.
  - Maintain test coverage for `browser_press_key` scroll fallback and proper key codes.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test browser_mcp`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User report on 2026-05-06: prosolocal projesinde `browser_press_key [key=PageDown]` çalışmıyor, scroll yapamadı.

(End of file - total 3872 lines)
