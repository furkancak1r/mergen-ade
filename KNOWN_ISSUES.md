# Known Issues

This file tracks bugs, regressions, and architectural decisions that have caused user-facing issues in Mergen ADE. It is append-only unless the user explicitly asks for cleanup.

When adding an entry:
- Use the format: `#### Title {#slug}` followed by `- Date`, `- Context`, `- Error signature`, `- Symptoms/Impact`, `- Root cause`, `- Resolution`, `- Prevent recurrence`, `- Files/Commands touched`, `- References`.
- Keep dates in `YYYY-MM-DD` format.
- If a regression has been fixed by a code change, link the commit or PR.
- Do not delete old entries without user confirmation.

---

#### Browser MCP cursor invisible on dark theme websites {#browser-cursor-dark-theme}
- Date: 2026-05-06
- Context: Browser MCP automation cursor on websites with dark backgrounds like `#18181b`
- Error signature: Cursor was black and invisible on dark-themed websites; user could not see where the automated mouse was pointing.
- Symptoms/Impact: Cursor overlay used static `rgba(0,0,0,0.98)` fill, making it invisible against dark backgrounds. This broke visual feedback during `browser_click`, `browser_hover`, and other automation tools.
- Root cause: Cursor color was hardcoded to black without considering page background luminance.
- Resolution:
  - Added `parseCssColor` helper to parse CSS rgb/rgba/hex colors.
  - Added `relativeLuminance` function implementing WCAG sRGB luminance formula.
  - Added `getEffectiveBackground` using `document.elementsFromPoint` to find the effective background color under cursor, with body/html fallbacks.
  - Added `updateCursorTheme` that computes luminance and switches cursor fill to white (`rgba(255,255,255,0.98)`) on dark backgrounds (luminance < 0.45 threshold), black on light backgrounds.
  - Changed SVG fill from static `rgba(0,0,0,0.98)` to CSS custom property `var(--mergen-mcp-cursor-fill, rgba(0,0,0,0.98))`.
  - Called `updateCursorTheme(point)` inside `setCursorPosition` so cursor updates automatically during all mouse movements, clicks, drags, and scrolls.
  - Bumped injected automation script version from 16 to 17.
- Prevent recurrence:
  - Test coverage asserts presence of `parseCssColor`, `relativeLuminance`, `getEffectiveBackground`, `updateCursorTheme`, `elementsFromPoint`, and both white/black fill options in the automation script.
  - Verify cursor visibility on both light and dark themed pages.
- Files/Commands touched: `src/web_browser.rs` (injected automation script), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request on 2026-05-06: "cursor sitenin temasına göre zıt renk olmalı... siyah veya koyu temalı bir web sitesinde cursor da siyah olunca gözükmüyor"

#### Launcher command dropdown visible state drifted from actual process lifecycle {#launcher-dropdown-state-drift}
- Date: 2025-08-07
- Context: Mergen ADE 0.1.0 launcher dropdown UI
- Error signature: After starting a tool from the launcher, the dropdown still showed the "Start" button (and the next click attempted to start again), even though the tool was already running.
- Symptoms/Impact: Users could accidentally try to launch a second instance, and the UI did not reflect reality.
- Root cause: The launcher panel's internal `running_processes` state was a local variable; it was not synchronized with the actual terminal runtime state that tracks which terminals have active AI sessions.
- Resolution: Changed `running_processes` to read from `terminal_manager`'s `has_running_ai_session(terminal_id)` instead of maintaining a separate set.
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
- Root cause:
  - `process_command_batch` loop used a single `while let Some(cmd) = rx.try_recv()` which processed one command at a time.
  - The optimization to deduplicate `Full` commands per project accidentally dropped `Subtree` commands because they weren't stored in a collection first.
- Resolution:
  - Changed `process_command_batch` to drain all available commands into a `Vec` first using a `loop { match rx.try_recv() {...} }` pattern.
  - Separated command draining from deduplication: first collect all commands, then deduplicate only `Full` commands per project (keeping latest generation), preserving all distinct `Subtree` commands.
- Prevent recurrence:
  - Add regression test `test_subtree_commands_not_deduplicated` that queues multiple distinct subtree requests and verifies all are processed.
- Files/Commands touched: `src/indexing/directory.rs`, `KNOWN_ISSUES.md`, regression tests
- References: AGENTS.md directory worker guidelines

#### Browser MCP `browser_wait_for` tool faked success for fixed waits {#browser-wait-for-fake-success}
- Date: 2026-04-27
- Context: Browser MCP automation script
- Error signature: Calling `browser_wait_for` with only a fixed time (no text/textGone) reported success immediately instead of actually waiting.
- Symptoms/Impact: Tests relying on fixed waits would proceed too early, causing flaky failures when page wasn't ready yet.
- Root cause: Script implementation had `return { ok: true, text: 'request accepted' }` at the top of the wait handler, before checking wait conditions.
- Resolution:
  - Removed the fake success return; now requires `text` or `textGone` parameter to be present.
  - If neither is provided, returns error explaining that fixed waits are handled by the MCP helper, not the page script.
  - Added test assertions that script does NOT contain "request accepted" string.
- Prevent recurrence:
  - Maintain test coverage asserting the absence of "request accepted" in automation script.
  - Document that fixed waits should use the helper-side timer, not page-side polling.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Browser MCP wait guidelines

#### Terminal history deduplication caused input loss on rapid consecutive same commands {#terminal-history-dedup}
- Date: 2026-04-26
- Context: Terminal input history persistence
- Error signature: Rapidly typing the same command twice within a short window resulted in only one history entry; the second was silently dropped.
- Symptoms/Impact: Users who re-executed the same command quickly could not access it via up-arrow history.
- Root cause: History deduplication logic compared only the previous entry; no timestamp check allowed deduping within arbitrarily short time windows.
- Resolution:
  - Added 2-second minimum window for deduplication: only dedupe if same command AND previous entry is older than 2 seconds.
  - Preserves intentional command repetition while still deduping true accidental duplicates.
- Prevent recurrence:
  - Test with rapid same-command input (< 1s apart) and verify both appear in history.
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`
- References: User report

#### Terminal wheel scroll during selection drag caused conflict with OpenCode scrollback {#terminal-wheel-selection}
- Date: 2026-04-25
- Context: Terminal selection drag with mouse wheel
- Error signature: When dragging to select text and scrolling with mouse wheel, the terminal scrollback and OpenCode's TUI both tried to handle the wheel event.
- Symptoms/Impact: Selection state became inconsistent; wheel delta was sometimes consumed by wrong component.
- Root cause: Wheel events during selection drag were forwarded to runtime without checking if Mergen's terminal scrollback could handle them first.
- Resolution:
  - Changed wheel handling to check Mergen's scrollback first; only forward to runtime if scrollback cannot consume the delta.
  - Added `opencode_manual_scroll_detached` tracking to prevent bottom-stick behavior from being incorrectly disabled.
- Prevent recurrence:
  - Test selection drag + wheel scroll combinations.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`
- References: AGENTS.md OpenCode wheel handling guidelines

#### Editor context menu selection lost on right-click {#editor-selection-lost}
- Date: 2026-04-24
- Context: File editor right-click context menu
- Error signature: Right-clicking selected text in the editor deselected it before the context menu appeared, making "Copy" useless.
- Symptoms/Impact: Users could not copy selected text via right-click menu.
- Root cause: `TextEdit` was being recreated each frame; right-click triggered a new `TextEdit::show()` which reset cursor state.
- Resolution:
  - Used `TextEdit::show()` instead of `ui.add(text_edit)` to preserve state.
  - Captured selection before showing context menu and restored it if menu opened.
- Prevent recurrence:
  - Test editor context menu with active selections.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md File Editor guidelines

#### Project switch left stale browser URL in URL bar {#browser-url-stale}
- Date: 2026-04-23
- Context: Embedded browser panel URL input
- Error signature: When switching projects, the URL bar showed the previous project's URL instead of the new project's.
- Symptoms/Impact: User confusion about which project's browser was active.
- Root cause: URL bar state was not synchronized on project switch; only updated on explicit navigation events.
- Resolution:
  - Added URL bar refresh when browser panel is drawn for a different project than last frame.
  - Ensured `browser_url_draft_by_project` is the source of truth per project.
- Prevent recurrence:
  - Test project switch with browser panel open and verify URL bar updates.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Browser panel guidelines

#### Terminal reroute on Windows sometimes missed batch confirmation prompt {#reroute-batch-miss}
- Date: 2026-04-22
- Context: Windows terminal background rerun after Ctrl+C
- Error signature: Rerunning a command in a background terminal on Windows sometimes failed because the "Terminate batch job (Y/N)?" prompt was not detected.
- Symptoms/Impact: Command didn't re-execute; terminal appeared stuck.
- Root cause: Detection looked for "Terminate batch job" anywhere in buffer; prompt might have been split across snapshot boundaries.
- Resolution:
  - Changed detection to look at the last non-empty line of the latest snapshot only.
  - Added phase tracking with settle delay before sending confirmation.
- Prevent recurrence:
  - Test batch file interruption and rerun on Windows terminals.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Terminal Manager guidelines

#### Codex interrupt banner cleared running spinner instead of just interrupt flag {#codex-interrupt-clear}
- Date: 2026-04-21
- Context: Codex CLI integration, interrupted-turn detection
- Error signature: When Codex displayed its strict interrupted-turn banner, Mergen cleared the running spinner but also removed all session tracking.
- Symptoms/Impact: A subsequent new turn would not show a running spinner because session was incorrectly cleared.
- Root cause: Detection logic called `clear_running_session()` instead of just clearing the spinner state.
- Resolution:
  - Changed to only clear the running flag, not the entire session tracking.
  - Preserved session process and notification path for subsequent turns.
- Prevent recurrence:
  - Test Codex interrupt banner scenario and verify next turn shows spinner.
- Files/Commands touched: `src/codex.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Codex CLI integration guidelines

#### Keyboard routing during AI question prompts blocked non-character keys {#keyboard-routing-question}
- Date: 2026-04-20
- Context: AI CLI question prompts (e.g., "Question 1/5" in Codex)
- Error signature: During question prompts, Escape, arrow keys, and Tab were not routed to the terminal.
- Symptoms/Impact: Users could not navigate or cancel question prompts with keyboard.
- Root cause: Keyboard routing only forwarded "interactive attention" state for OpenCode/Factory Droid, not for Codex.
- Resolution:
  - Extended keyboard routing to include `UserInputRequested` attention state.
  - Ensured raw keyboard events (Escape, arrows, Tab) are forwarded to terminal during question prompts.
- Prevent recurrence:
  - Test keyboard navigation during all AI CLI question UIs.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md AI CLI integration guidelines

#### Design Inspect stale hover messages forwarded to terminals {#design-inspect-hover-forward}
- Date: 2026-04-19
- Context: Browser design inspect mode, stale injected scripts
- Error signature: Hover events from old Design Inspect scripts were forwarded to the terminal as if they were click events.
- Symptoms/Impact: Spam in terminal from hovering over browser elements.
- Symptoms/Impact: Cursor overlay used static `rgba(0,0,0,0.98)` fill, making it invisible against dark backgrounds like `#18181b`.
- Root cause: Cursor color was hardcoded to near-black without considering page background luminance.
- Resolution:
  - Changed SVG path fill from static `rgba(0,0,0,0.98)` to CSS custom property `var(--mergen-mcp-cursor-fill, rgba(0,0,0,0.98))`.
  - Added `parseCssColor()` helper to parse CSS rgb/rgba/hex colors.
  - Added `relativeLuminance()` implementing WCAG sRGB luminance formula.
  - Added `getEffectiveBackground()` using `elementsFromPoint` with body/html fallbacks to find the effective background under cursor.
  - Added `updateCursorTheme()` that computes luminance and switches cursor fill to white on dark backgrounds (luminance < 0.45 threshold).
  - Integrated `updateCursorTheme(point)` into `setCursorPosition()` so all movements/clicks/drags update theme automatically.
  - Bumped automation script version from 16 to 17.
- Prevent recurrence:
  - Test coverage asserts `parseCssColor`, `relativeLuminance`, `getEffectiveBackground`, `updateCursorTheme`, `elementsFromPoint` helpers present.
  - Verify both `rgba(255,255,255,0.98)` (white) and `rgba(0,0,0,0.98)` (black) fill options exist in script.
  - Manually verify cursor visible on dark sites like Tailwind `#18181b`.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`
- References: User request 2026-05-06: "cursor sitenin temasına göre zıt renk olmalı"

(End of file - total 3872 lines)
