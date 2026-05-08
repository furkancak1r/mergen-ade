# Repository Guidelines

## Critical Rules - DO NOT BREAK

### Never Kill Running Mergen ADE Processes
- **ABSOLUTE RULE**: Never terminate, stop, or kill any running `mergen-ade` process, especially if it's running on the desktop.
- This applies to ALL automation, scripts, builds, and agent actions.
- If a new binary needs testing, ask the user to manually restart the application.
- The user explicitly owns the lifecycle of the desktop application instance.
- Violation of this rule can cause data loss, interrupted workflows, and broken user trust.

## Project Structure & Module Organization
- `src/main.rs`: app entrypoint and native window startup.
- `src/app.rs`: UI composition (top bar, activity rail, collapsible side panels, terminal manager, main tiled panes) and app state flow.
- `src/terminal.rs`: terminal runtime, PTY integration, event forwarding, snapshot rendering data.
- `src/layout.rs`: auto-tiling grid math and related unit tests.
- `src/title.rs`: terminal title update/truncation logic and unit tests.
- `src/config.rs` + `src/models.rs`: persisted TOML config schema and load/save behavior.
- `.github/workflows/release.yml`: GitHub release pipeline for Windows ZIP and signed/notarized macOS ARM64 DMG assets.
- Build artifacts are in `target/` (do not commit).

## Build, Test, and Development Commands
- `cargo build --release`: default local production build using the repo MSVC target configuration.
- `cargo build --release --target x86_64-pc-windows-msvc`: **supported Windows release binary** (`target/x86_64-pc-windows-msvc/release/mergen-ade.exe`). Use this command to update the release executable.
- **To update the release executable**: Always use `cargo build --release --target x86_64-pc-windows-msvc` (not the default target) to ensure the binary is placed at the correct path.
- `cargo build --release --target x86_64-pc-windows-gnullvm`: optional local build via repo-local LLVM-MinGW linker.
- `cargo build --release --target aarch64-apple-darwin`: native macOS build used by the release workflow before signing/notarization packaging.
- `cargo run --release`: run optimized build locally using the same default target.
- `cargo test`: run unit tests (layout, title, terminal helpers).
- `cargo fmt`: format Rust sources before commit.

If `cargo` is not on PATH in PowerShell, use:
`$env:USERPROFILE\.cargo\bin\cargo.exe <command>`.

## Coding Style & Naming Conventions
- Rust 2021, 4-space indentation, UTF-8, LF/CRLF handled by Git.
- Keep modules focused; prefer small functions over large mixed-responsibility blocks.
- Naming: `snake_case` for functions/modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Avoid heavy dependencies; preserve the low-memory, native-first design.
- Keep UI controls visually lightweight; prefer minimal icon-first interactions over heavy bordered button chrome unless emphasis is required.
- Run `cargo fmt` after edits; keep warnings minimal and intentional.

## Testing Guidelines
- Use inline unit tests (`#[cfg(test)]`) in the same module where logic lives.
- Test behavior, not implementation details.
- Prefer descriptive test names like `wide_viewport_prefers_more_columns`.
- Minimum expectation for feature changes:
  1. Update/add tests in affected modules.
  2. Ensure `cargo test` passes locally.

## Commit & Pull Request Guidelines
- Follow existing history style: short, imperative subject lines (examples: `Fix terminal input focus`, `Add release workflow`).
- Keep commits scoped to one concern when possible.
- PRs should include:
  1. What changed and why.
  2. Validation steps (`cargo test`, manual run notes).
  3. UI screenshots/GIFs for visible behavior changes.
  4. Any platform-specific assumptions or limitations, especially Windows-first runtime behavior and macOS signing/notarization requirements.

## Security & Configuration Notes
- Do not commit local paths, secrets, or generated executables.
- Config is user-local via `ProjectDirs`; on Windows this maps under `%APPDATA%`. Treat it as runtime data, not source-controlled state.
- Official macOS releases require GitHub secrets for Apple signing and notarization: `APPLE_DEVELOPER_ID_APP_CERT_BASE64`, `APPLE_DEVELOPER_ID_APP_CERT_PASSWORD`, `APPLE_DEVELOPER_IDENTITY`, `APPLE_NOTARY_API_KEY_ID`, `APPLE_NOTARY_API_ISSUER_ID`, `APPLE_NOTARY_API_PRIVATE_KEY_BASE64`.
- Public repository status is acceptable for this flow because signing material stays in GitHub Actions secrets and the release workflow is tag-push based; do not write signing material into tracked files or logs.

## Known Issues Maintenance
- Keep `KNOWN_ISSUES.md` up to date whenever a bug is diagnosed and fixed or a recurring failure mode is identified.
- Treat `KNOWN_ISSUES.md` as append-only unless the user explicitly asks for a cleanup or rewrite; prefer adding a new dated entry over rewriting history.
- Record the symptom, root cause, resolution summary, and concrete references (commit/PR/issue) so later regressions can be traced quickly.

## Directory Indexing Performance Guidelines
- Directory tree indexing must never block the UI thread. Do not use blocking sends or synchronous recursive filesystem scans from egui rendering paths.
- **Initial indexing must be shallow**: only read the root directory's immediate children. All child directories must be deferred regardless of name; do not recursively scan any directory during initial project open.
- **Lazy subtree loading must be one-level-only**: when a deferred directory is expanded, load only its immediate children. Child directories discovered during this load must also be deferred.
- Use `DirectoryScanMode::InitialRoot` for the first project scan and `DirectoryScanMode::LazySubtree` for on-demand expansion. Do not use boolean `allow_defer` flags.
- Large/generated directories such as `.git`, `target`, `node_modules`, `.next`, `dist`, `build`, caches, and virtual environments are automatically deferred by the shallow scan behavior.
- Time budgets must be enforced as **hard stops** inside entry iteration and child construction loops, not just at function boundaries. Check `should_stop()` before every expensive operation.
- Prefer `DirEntry::file_type()` over `fs::symlink_metadata(path)` and `path.is_dir()` to minimize filesystem calls.
- Prefer fast partial snapshots over waiting for a complete tree. The Directory panel should become usable immediately.
- `partial_warning` should remain internal state only; do not display it in the Directory panel UI.
- Preserve symlink safeguards: never recursively descend into symlinked directories.
- The worker thread should drain stale commands and prefer the latest `Full` command per project to avoid processing outdated requests.
- **Deferred directories must use `DirectoryNode::is_deferred` as metadata**; do not add visible placeholder children for normal lazy-load state. Placeholder nodes (`directory_placeholder_node`) are only for exceptional/truncated states such as load failure, outside-project paths, or omitted items after hard limits.
- **Directory worker command draining must never silently drop distinct `Subtree` commands**. Use batch draining (`Vec<DirectoryIndexCommand>`) to preserve all subtree load requests. Only deduplicate Full commands per project (keep latest generation).
- **When the UI queues a subtree load, request a repaint** (`request_repaint_after`) to process worker events promptly without waiting for unrelated input.
- **`request_directory_subtree_load()` must report whether work was queued** (`bool`) and must clean up loading state (`directory_index_subtree_loading_by_project`) if command send fails, to prevent stuck loading indicators.
- **Directory search must progressively queue deferred directories** even when the folder name itself does not match the query; otherwise matches inside lazy-loaded folders can never be discovered.
- **Search-triggered directory loading must still use `DirectoryScanMode::LazySubtree`**; never perform synchronous recursive scans from the UI thread.
- **While deferred search loads are queued, in flight, or waiting for debounce, do not show final "No matching files or folders" feedback.** Instead show a "Searching folders..." indicator and schedule repaint to continue loading.
- **Cap search-triggered subtree queueing per frame** (`DIRECTORY_SEARCH_INITIAL_SUBTREE_REQUESTS_PER_FRAME` and `DIRECTORY_SEARCH_BACKGROUND_SUBTREE_REQUESTS_PER_FRAME`) to keep large projects responsive; defer additional directories in subsequent frames via repaint.
- **Debounce search-triggered deferred loading and self-wake**: Wait `DIRECTORY_SEARCH_DEFERRED_LOAD_DEBOUNCE_SECS` (250ms) after query stops changing before starting deep deferred loads. Schedule `request_repaint_after` for the remaining debounce duration so loading starts promptly without depending on unrelated input or project switching.
- **Minimum query length for deferred loading is character-based, not byte-based**: Use `query.chars().count()` against `DIRECTORY_SEARCH_MIN_DEFERRED_QUERY_CHARS` (2 characters) so Unicode searches respect the same minimum length threshold as ASCII.
- **Adaptive per-frame loading caps**: Use aggressive cap (`DIRECTORY_SEARCH_INITIAL_SUBTREE_REQUESTS_PER_FRAME` = 8) when no results exist yet, conservative cap (`DIRECTORY_SEARCH_BACKGROUND_SUBTREE_REQUESTS_PER_FRAME` = 2) when results already visible. This prioritizes finding first matches without overwhelming UI.
- **Hidden deferred queue for search**: Deferred directories whose names don't match the query should be loaded in background without being added to `matching_directories`. Only directories that actually contain matches should be expanded in UI; others load hidden.
- **Directory search results must update automatically as deferred subtree results arrive.** Compute visible paths from the current snapshot each frame; do not require explicit user action to refresh results.
- **Do not add "New results found" / "Update results" UI for directory search.** Users expect search results to appear automatically as background loading completes.
- **Do not conflate parent visibility with descendant visibility.** A parent directory shown because it contains a matching file must not force all sibling descendants visible. Only directories whose own names match the query should force-show descendants.
- **Directory search result highlighting must be char-safe.** Highlight matched query text with a high-contrast orange color (`DIRECTORY_SEARCH_MATCH_COLOR`) in file and folder names. Use `LayoutJob` with multiple `TextFormat` sections to apply highlighting. Always use byte ranges derived from lowercase string indices to avoid splitting multi-byte UTF-8 sequences. Preserve row ordering, lazy loading behavior, and tooltips while adding visual feedback.
- **Directory search query tracking must run before snapshot availability checks.** Update debounce/query state even while the selected project's directory index is missing, loading, or errored; do not wait for a fully loaded snapshot before arming search repaint/deferred-load logic.
- **Project selection changes must reset Directory search tracking, not the user's query text.** Preserve `directory_search_query`, but reset project-scoped debounce/last-query state so the same query re-runs for the newly selected project.
- **Directory search input focus must not be stolen by AI attention routing.** When Directory search owns keyboard focus, text input belongs to the search field until the user explicitly focuses a terminal.
- Add regression tests whenever directory indexing, deferred loading, or tree rendering behavior changes.

## Subagent Usage Policy
- For any non-trivial implementation, debugging, or review task, use subagents instead of running everything in a single agent.
- When work can be split safely, delegate independent parts in parallel (for example: `explorer` for discovery, `fast_code` for implementation, `test` for verification, `reviewer` for risk checks).
- Respect the configured concurrency limit and do not exceed 4 parallel subagent threads.
- Keep urgent critical-path edits local only when delegation would block progress; otherwise prefer delegation first.
- In handoff/final notes, summarize which subagents were used and what each one produced.

## AI CLI Integration
- **Supported AI tools:** `Factory Droid`, `Codex CLI`, `OpenCode`, and `Claude Code` are supported. Codex CLI uses hook-only integration.
- **Factory Droid hook format:** Only `droid-hook:*` and `factory-droid-hook:*` format patterns are recognized for Factory Droid. The `claude-hook:*` format is not supported.
- **Factory Droid detection commands:** Only `droid` and `factory` trigger Factory Droid session detection. Do not add `cc`, `claude`, or other AI CLI commands.
- **OpenCode detection commands:** `opencode` triggers OpenCode session detection. OpenCode is tracked through explicit launch detection, process-based status, visible UI/title parsing, notify/inbox status paths, and the Mergen-owned `mergen-opencode-status.js` plugin. If OpenCode hangs at `Loading plugins`, inspect MCP startup load before disabling the plugin path.
- **OpenCode scrollback should prioritize Mergen scrolling.** When OpenCode is active, mouse wheel events over terminal output should scroll Mergen's terminal scrollback, not be forwarded to OpenCode's TUI.
- **OpenCode wheel uses Mergen-first fallback behavior.** When OpenCode is active, mouse wheel over terminal output must first be offered to Mergen's terminal `ScrollArea`. Only if Mergen does not consume the wheel delta because scrollback cannot move should the wheel be forwarded to OpenCode's runtime TUI.
- **OpenCode wheel handling must not affect other TUIs.** Preserve runtime mouse-wheel forwarding for non-OpenCode mouse-reporting applications.
- **Wheel hit-testing must use hover fallback.** Terminal wheel handling should use hover position before falling back to interaction position so passive wheel scrolling works.
- **OpenCode manual scroll detach must reflect actual Mergen scroll.** Set `opencode_manual_scroll_detached` only after Mergen consumes the wheel scroll; runtime fallback wheel events must not disable bottom-stick behavior.
- **Terminal wheel handling must yield to UI overlays.** When Settings popup, exit confirmation popup, Terminal Manager history popup, or foreground message popup is open, terminal wheel handling must be disabled to allow wheel events to reach the overlay's ScrollArea. Use `terminal_output_mouse_wheel_enabled()` helper to check overlay state before processing wheel events in `draw_terminal_pane()`.
- **Codex CLI integration:** Codex uses strict hook-only integration with narrow visible-state exceptions. Mergen configures `hooks.json` with `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, and `Stop` events that route to the Mergen bridge. The status transitions are:
  - `UserPromptSubmit`, `PreToolUse`, and `PostToolUse` hooks → Running (gray spinner)
  - `PermissionRequest` hook → `ApprovalRequested` / amber pulse (waiting for user)
  - `Stop` hook → short debounce, then `TurnComplete` / green pulse unless follow-up work arrives first
  - **Question prompt exception:** When Codex shows a question UI ("Question X/Y" with "enter to submit answer"), a narrow visible detection triggers `UserInputRequested` attention with amber pulse. This handles the case where the turn is waiting for user input but the Stop hook hasn't fired yet.
  - **Interrupt banner exception:** When Codex renders the strict interrupted-turn banner (`conversation interrupted` plus `/feedback`), narrow visible detection clears the running spinner without clearing the live Codex session/process tracking.
  - No notify, BEL, or title fallback is used for Codex.
  - **Routing:** Uses tool-specific `MERGEN_ADE_CODEX_INBOX_DIR` env var to avoid collisions with OpenCode's shared `MERGEN_AI_INBOX_DIR`.
  - **Hook configuration:** Managed hooks intentionally omit `statusMessage` to prevent Codex from displaying status text (e.g., "Running Stop hook: Mergen: session stopped") in the terminal.
  - **Redraw filtering:** The `CodexRedrawFilter` only suppresses `ED3` (scrollback erase) during synchronized update blocks. Cursor positioning sequences (`ESC[H`, `ESC[1;1H`, `ESC[f`) and `ED2` (erase display) are preserved to prevent diagonal/stair-step rendering artifacts.
  - **Keyboard routing:** During question prompts (`UserInputRequested` attention), raw keyboard events (including Escape, arrow keys, Tab) are routed to the terminal using the same mechanism as OpenCode/Factory Droid interactive attention states.
- **Claude Code integration:** Claude uses title-based detection (Orca-compatible). Title patterns are detected via OSC title updates.
- **UI labels:** Use "Droid", "Factory Droid", "Codex CLI", "OpenCode", and "Claude" terminology. Do not use "claude" lowercase references in user-facing text.
- **Event triggers:** Factory Droid uses `UserPromptSubmit` → Running (green pulse) and `Stop`/notification → Attention (yellow pulse). Codex CLI uses hook-only: prompt/tool hooks → Running, `PermissionRequest` → amber attention, debounced `Stop` → turn-complete attention, and the strict interrupt banner → clear running spinner. OpenCode uses explicit launch detection plus process-based tracking for spinner/pulse status. Claude uses title-based detection.
- **Plan mode skill restriction:** If you are Codex, OpenCode, or Droid, do not use the plan mode skill from Claude Code's configuration. The plan mode skill is exclusively for `cc` (Claude Code) sessions only.

## File Editor Guidelines
- Long editor content must be wrapped in a stable-id `ScrollArea` (e.g., `FILE_EDITOR_SCROLL_ID`).
- `TextEdit::multiline` must allocate enough rows for the full line count (`max(visible_rows, line_count)`), not only the visible viewport.
- Preserve `FILE_EDITOR_INPUT_ID` focus isolation so editor input never routes to terminal capture.
- Dirty detection must compare against `saved_text` (taking snapshots before mutable borrow to avoid borrow issues).
- Add regression tests for scroll behavior with files longer than screen height.
- Drag-selecting text in the file editor must continue tracking pointer position when the pointer leaves the editor viewport.
- While file editor text selection drag is active near viewport edges, the editor `ScrollArea` must autoscroll with `ui.scroll_with_delta` and request repaint.
- File editor selection drag state must be runtime-only (`FileEditorState.selection_drag_active`) and reset when opening, closing, or navigating between files.
- Use `input.pointer.interact_pos()` as the pointer fallback for file editor drag autoscroll; do not rely only on `TextEdit` hover state.
- Use shared `selection_edge_autoscroll_delta()` helper for consistent edge autoscroll behavior across terminal and file editor.
- Add regression tests for file editor selection drag state transitions and edge autoscroll delta behavior.
- Editor header buttons must use high-contrast `editor_header_icon_button()` helper to ensure visibility against dark surfaces.
- Editor has separate "open" (has active buffer) and "visible" (shown in main area) states.
- Use `FileEditorState::is_visible()` for main area rendering, `is_open()` for buffer existence checks.
- Call `FileEditorState::hide()` when switching to terminal; call `close()` when truly closing.
- `set_active_terminal()` must hide the editor before early-return checks to ensure terminal click always switches from editor.
- File editor selection-aware context menus must use `TextEdit::show()`/`TextEditOutput`, not plain `ui.add(text_edit)`, so cursor state and selection data remain available.
- Right-clicking `egui::TextEdit` can collapse the active selection before menu handling; capture the pre-click `TextEditState` selection and restore non-empty selections when opening editor copy menus.
- Copying selected editor text must use character cursor ranges (`CCursorRange`/`CursorRange`) and char-safe slicing; never byte-index slice editor text.
- Defer clipboard feedback that mutates `AdeApp` state until after the active editor buffer mutable borrow ends.
- Detect editor copy requests using `Event::Copy` (generated when TextEdit handles the copy shortcut) rather than just raw key detection, with `Ctrl+C`/`Command+C` as fallback. This ensures copy feedback works regardless of whether TextEdit consumed the shortcut.
- Add regression tests for editor context menu selection preservation, Unicode-safe selected-text extraction, and empty-selection behavior.
- Test both `Event::Copy` and raw `Ctrl+C`/`Command+C` paths for editor copy feedback.

## Terminal Input & History Invariants
- `pending_line_for_title` is for title/AI command detection only; it clears on newlines (\r or \n) since titles should reflect only the current logical line. This buffer is capped to `TERMINAL_PENDING_LINE_MAX_CHARS` (512) to prevent unbounded growth.
- `pending_input_for_history` preserves the complete raw input including multi-line content and Unicode characters (e.g., box-drawing characters like `┃`). This buffer is **NOT** capped—full prompts of any length must be preserved for history and rerun functionality.
- When Enter is pressed, history is recorded from `pending_input_for_history` (taking the full raw text), while title updates use `pending_line_for_title` (taking only the last logical line).
- Runtime `recent_inputs` (for tooltips and rerun) must be populated from the raw `history_line`, not the sanitized title candidate. This ensures multi-line prompts appear correctly in Terminal Manager and background rerun replays the full command.
- Backspace pops from both buffers to keep them in sync for single-character deletion.
- Always use char-safe truncation helpers (e.g., `capped_hover_text()`) for UI display of user text; never use byte-index slicing like `text[..60]` which can panic on multi-byte UTF-8 sequences or split Unicode code points.

## Terminal Selection Guidelines
- Drag selection must continue tracking pointer position even when the pointer leaves the terminal viewport.
- While selection drag is active near the viewport edges, terminal output must autoscroll with `ui.scroll_with_delta` and request repaint.
- Selection autoscroll must detach prompt scroll anchoring (`detach_terminal_prompt_scroll_anchor_on_manual_scroll`) and must not be overridden by `stick_to_bottom`.
- Use `input.pointer.interact_pos()` as a fallback when `response.hover_pos()` returns `None` during selection drag.
- Autoscroll speed must vary based on pointer distance from viewport edge (slower near edge, faster when farther away, clamped to 1-8 lines per frame).
- Add regression tests for edge autoscroll direction, speed, and zero delta inside the safe zone.

## Concurrent AI Sessions
- Sen (AI agent) çalışırken başka bir AI agent'ı da aynı anda çalışıyor olabilir.
- Eğer dosyalarda veya kodda başka birinin yaptığı değişiklikleri fark edersen, bu değişikliklere müdahale etme.
- Kendi işlemlerine devam et; başkasının yaptığı değişiklikleri değiştirme, silme veya üzerine yazma.
- Çakışma olursa kullanıcıya danış; tek başına karar verip başkasının işini bozma.

## Terminal Manager & Input History Guidelines
- **Background terminals use runtime-only input history**: Do not persist background terminal inputs to `history.json`. They use `recent_inputs` (runtime-only) for the rerun/interrupt button.
- **Foreground terminals persist input history**: Foreground terminal inputs are recorded to persistent history and shown in the global Input History panel.
- **Background rerun/interrupt behavior**: The background Terminal Manager row shows a refresh (rerun) button that becomes an X (interrupt) button when `AiCliStatus::Running` is detected. When clicked while running, it sends `0x03` (Ctrl+C) to the terminal; when idle, it reruns the most recent command from `recent_inputs`.
- **Rerun must replay the full stored command**: Background rerun sends the complete `recent_inputs[0]` content (including multi-line content and Unicode), not a trimmed or title-only version. The full raw input is preserved specifically for this purpose.
- **Background rerun uses explicit phases/timestamps**: Pending reruns track state with `pending_rerun_phase` (`InterruptSent`, `BatchConfirmSent`) and `pending_rerun_since` timestamps. Always request repaint while waiting so prompt handling does not depend on unrelated UI input.
- **Windows batch confirmation prompt detection**: On Windows PowerShell/CMD shells, after sending `Ctrl+C`, check the latest runtime snapshot for `Terminate batch job (Y/N)?` on the last non-empty line. If detected, send automatic `y\r` confirmation, wait for `PENDING_RERUN_BATCH_CONFIRM_SETTLE_MS`, then replay the command.
- **Internal confirmation input must not be recorded**: The automatic `y` sent to confirm batch termination is internal control input and must never be added to `recent_inputs` or persisted history.
- **Do not interpolate GitHub contexts directly**: In workflow `run:` steps, use environment variables instead of direct `${{ github.ref_name }}` interpolation to avoid shell injection risks (per Semgrep findings).

## Terminal Manager Saved Messages Guidelines
- **Two separate message systems**: Terminal Manager has distinct saved message systems for foreground and background terminals:
  - **Background saved messages** (`ProjectRecord::saved_messages`): Reusable snippets that persist across sessions. Sent via the message button in Terminal Manager rows for background terminals.
  - **Foreground saved messages** (`ProjectRecord::foreground_saved_messages`): Dynamic task queue for foreground terminals. Clicking a message sends it to the terminal AND removes it from the queue.
- **Foreground message queue UI**: The foreground message menu shows the current queue with edit/delete actions for each item, plus an "Add New" button at the bottom.
- **Foreground task menu icon color indicates queue state**: The CHAT_TEXT icon in the Terminal Manager row for foreground terminals must change color based on queue state:
  - Empty queue: `with_alpha(TEXT_PRIMARY, 190)` (white/gray muted)
  - Has items: `Color32::from_rgb(100, 200, 100)` (green)
  - Only the icon text color changes; button background remains transparent.
- **Fixed action button positioning**: Message row layout must reserve fixed width for action buttons (edit/delete) on the right side so they don't shift based on prompt text length. Use a fixed menu width (e.g., 160px) - do NOT use `ui.available_width()` inside egui menus as it can be unbounded. Calculate `action_button_width = CONTROL_ROW_HEIGHT * 2 + 4px gap`. Message button width = `menu_width - action_button_width - spacing`. Set `ui.set_min_width()` and `ui.set_max_width()` on each row to enforce the fixed width. This ensures consistent right-aligned action buttons regardless of message content length.
- **Add/Edit popup**: Clicking "Add New" or the edit button opens a popup with a multiline `TextEdit::multiline` input. The popup uses `Order::Foreground` to render above other UI layers.
- **Popup layout**: Text area should fill available space dynamically with minimum 280px height. Calculate as `available_height - button_row_height - 16px`. Buttons positioned at bottom with 8px gap (not 24px) to maximize text editing area and minimize unused space below buttons.
- **Popup text input key handling**: Use `Ctrl+Enter` for newline insertion and plain `Enter` alone to submit/save the task. Configure `TextEdit::multiline` with `.return_key(egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Enter))` to set Ctrl+Enter as the newline shortcut. Detect plain Enter in `raw_input_hook()` using `partition_foreground_message_popup_submit()` helper to reliably capture the event before egui consumes it. Do NOT rely on `ui.input()` or `response.has_focus()` in the draw function for Enter detection as focus state may be unreliable.
- **Focus management**: The popup input automatically requests focus on open. Terminal keyboard capture is disabled while the popup is open (via `text_input_has_focus_extended()` check).
- **Attention routing must not steal from popup**: When the foreground message popup is open, `should_steal_attention_terminal_input()` must return false to prevent AI attention terminals from stealing text input. The popup's text input focus takes priority over terminal attention input routing.
- **Browser overlay yield**: The foreground message popup is included in `embedded_browser_should_yield_to_ui_layer()` to ensure native WebView is hidden while the popup is open.
- **Persistence**: Foreground saved messages are persisted to config TOML in `ProjectRecord::foreground_saved_messages` and survive application restarts and version updates.
- **Send-and-remove behavior**: When a foreground message is sent to a terminal, it is automatically removed from the queue. This makes the foreground queue work as a task list that depletes as tasks are executed.
- **Double Enter for confirmation**: When sending a saved message (foreground or background), send the command followed by Enter (`\r`), then schedule a second Enter after `SECOND_ENTER_DELAY_MS` (1000ms) delay. This ensures AI CLI tools and shells process the command and any confirmation prompts. Use `schedule_second_enter_for_terminal()` helper and `process_pending_second_enters()` in the update loop. The same mechanism is used for terminal shortcuts.
- **Edit/Delete actions**: Each foreground queue item has edit (pencil) and delete (trash) buttons. Edit opens the popup pre-filled with the message content. Delete removes immediately without confirmation.
- **Empty queue handling**: When the foreground queue is empty, the menu shows "No tasks in queue" text and only the "Add New" button is available.

## Window Close Confirmation Guidelines
- **Window close confirmation must not early-return before rendering.** When intercepting a close request (`ViewportCommand::CancelClose`), do not use `return` to exit the update function early. The confirmation popup should be rendered in the same frame by setting the state flag and allowing the normal render path to continue.
- **Avoid `request_repaint()` after showing the confirmation popup.** Since the popup will be drawn later in the same update cycle by `draw_exit_confirm_popup()`, an explicit repaint request is unnecessary and can cause visual flicker.
- **Popup overlay should use appropriate layer order.** Modal confirmation dialogs should render above the main UI surface; use `egui::Order::Foreground` or appropriate z-ordering for overlay backdrops to ensure the modal appears on top without obscuring the underlying content during the same-frame transition.

## Embedded Browser Panel Guidelines
- **Browser panel is a right-side project tool panel.** It must be mutually exclusive with the Check-list panel; only one right panel can be open at a time.
- **Browser state is project-scoped.** Persist only `ProjectRecord::browser_last_url`; do not persist browsing history, cookies, or runtime state in Mergen config.
- **Terminal links route to project browser.** When user clicks an `http://` or `https://` link in a terminal, it should open in that terminal's project browser panel rather than externally.
- **URL input uses persistent draft state, not local variable.** Browser URL text input must use `browser_url_draft_by_project: BTreeMap<u64, String>` to persist typed text across frames. Never use a local `let mut url_input` variable that resets each frame.
- **URL normalization is mandatory.** Use `normalize_browser_url()` to ensure all URLs have valid schemes. Allow only `http://` and `https://`; reject `file:`, `data:`, `javascript:`, and other unsupported schemes.
- **Localhost detection uses http.** Automatically use `http://` for `localhost`, `127.0.0.1`, `0.0.0.0`, and `[::1]`; use `https://` for all other domains.
- **Right panel width accounting.** Update `main_area_size_from_chrome()` to accept a generic `right_panel_rect` parameter that works for both Check-list and Browser panels.
- **Activity rail toggle icon.** Use `icons::GLOBE` for the browser panel toggle button in the activity rail.
- **Config recovery sanitizes legacy browser state.** Ensure `recover_config_state()` resets `browser_panel_expanded` to `false` (it is legacy-only) and `recover_project_records()` preserves `browser_last_url` when merging. Browser panel open state is runtime-only and must never be persisted.
- **Add regression tests for browser behavior.** Test URL normalization (domain vs localhost, scheme handling, rejection of unsupported schemes), panel mutual exclusivity, URL draft persistence across frames, and config persistence.
- **Platform-specific WebView2 integration.** Use `web_browser::EmbeddedBrowser` facade with target-gated implementation: Windows uses `webview2-com` + `windows` crates; non-Windows returns `BrowserStatus::Unsupported`.
- **Browser bounds synchronization.** Call `sync_embedded_browser()` after UI render to update native WebView position using `browser_bounds_from_egui_rect()` for physical pixel conversion.
- **Browser lifecycle management.** Initialize in `AdeApp::new()`, shutdown in `on_exit()`, and sync position via `pending_browser_rect` set during `draw_browser_panel()`.
- **No placeholder text in browser panel.** The browser panel must render actual WebView content or a neutral empty state; never show placeholder text like "(Embedded browser coming...)".
- **Browser panel navigation must be embedded-only.** Browser panel Go actions and terminal HTTP/HTTPS links must never call `ctx.open_url()` or launch the system/default browser. External browser behavior must not be exposed from the browser panel unless explicitly requested for a separate feature.
- **WebView2 must be created before navigation.** `sync_embedded_browser()` must call `EmbeddedBrowser::ensure_created(window_hwnd)` when the browser panel is visible before syncing bounds or navigating pending URLs.
- **WebView2 async creation must pump messages.** Do not block WebView2 environment/controller creation with raw `recv()`. Use `webview2_com::wait_with_pump()` or an equivalent UI-message-pumping wait so WebView2 callbacks can fire.
- **Use WebView2 setter APIs for native state.** Use `ICoreWebView2Controller::SetBounds()` and `SetIsVisible()` for positioning/visibility. `Bounds()` and `IsVisible()` are getters and must not be used for sync.
- **Browser hide must not depend on a fresh panel rect.** When `browser_panel_expanded` is false, hide the native WebView even if `pending_browser_rect` is `None`.
- **Browser failures must be visible.** If WebView2 creation or navigation fails, render a clear in-panel error state instead of silently doing nothing or falling back to an external browser.
- **Browser panel follows active terminal project.** The browser panel shows the active terminal's project, not the manually selected project. When the active terminal changes, the browser panel automatically switches to that terminal's project.
- **Browser panel open/closed state is project-scoped and runtime-only.** Track which projects have the browser open in `browser_panel_open_projects: BTreeSet<u64>`. The visible panel follows the active terminal project: if the active project has browser open, the panel shows; if not, it hides. This state is not persisted across sessions; only `browser_last_url` is persisted.
- **`UiConfig::browser_panel_expanded` is legacy-only and must not be used.** This field exists for backward compatibility and must be treated as always `false`. Do not mirror runtime Browser open state into this config field. Do not use it to determine panel visibility. Sanitize it to `false` during config recovery and on app startup.
- **Active terminal switches must recompute visible browser panel.** When switching terminals, check `is_active_browser_panel_open()` to determine visibility based on the new active project's runtime open state. If the new project has browser open, the panel appears; if closed, it hides. Do not write Browser open state into persisted UI config.
- **A project that has not opened browser must not inherit another project's open panel.** When switching from a project with browser open to a project without browser open, the panel should close for the new project.
- **Returning to a project whose browser was open in the current session should restore it.** When switching back to a project that previously had browser open (in `browser_panel_open_projects`), the panel should reopen automatically with that project's browser state preserved.
- **Project browsers are isolated and persistent.** Each project has its own browser instance (`embedded_browsers_by_project`). Switching projects hides the previous browser but preserves its state; returning to that project shows the same browser state.
- **Browser instances are lazily created.** A browser instance is only created when first needed (when the panel opens for that project). This avoids unnecessary WebView2 resource usage for projects never viewed in the browser.
- **Native WebView must yield to egui overlays.** WebView2 renders as a native child surface above egui paint layers, so hide project WebViews while Settings, exit confirmation, egui popups/context menus, or Terminal Manager history popups are open; restore visibility when the overlay closes.
- **Modal browser-overlap fixes must not destroy browser state.** Hide/show native WebViews with `EmbeddedBrowser::hide()` / `show()` only; do not shutdown, recreate, clear navigation state, or change `browser_last_url` just because a modal opened.
- **Settings and exit confirmation modals must render in foreground order.** Use `egui::Order::Foreground` for modal backdrops and windows; do not rely on `Order::Background` for modal overlays.
- **Native WebView source changes must update the URL input.** Drain `EmbeddedBrowser::drain_events()` before rendering the browser panel, and apply `BrowserEvent::UrlChanged` to both `browser_url_draft_by_project` and `ProjectRecord::browser_last_url`.
- **Observed browser URLs remain scheme-gated.** Persist and display observed WebView URLs only when they are non-empty `http://` or `https://` URLs; ignore transient or unsupported sources such as `about:blank`, `file:`, `data:`, and `javascript:`.
- **WebView2 event handlers are part of browser lifecycle.** Register `SourceChanged` when the WebView2 instance is created, keep its event token with the browser instance, and remove it during shutdown before dropping native WebView resources.
- **Browser MCP tokens must be terminal-scoped capabilities.** Do not expose one global Browser MCP token to every terminal; each token must resolve server-side to the terminal/project it was issued for.
- **Browser MCP project selection must be derived from authenticated terminal ownership.** Client-supplied `terminal_id` and `project_id` are claims only; reject mismatches and never use them to choose another project.
- **Browser MCP must fail closed without active-project fallback.** Terminal-originated MCP commands must never fall back to `active_browser_project_id()`, `selected_project`, or other mutable UI state when authorization data is missing or stale.
- **Browser MCP waits must not fake success.** `browser_wait_for` may report success only after the requested duration has elapsed or the requested text/textGone condition is true.
- **Browser MCP waits must not block egui update paths.** Fixed waits and polling should run in the MCP helper or another non-UI pending flow, not inside `AdeApp::process_browser_mcp_commands()` or WebView script execution that blocks the UI frame.
- **Native WebView focus must yield to terminal activation.** When a terminal is activated—including re-selecting the same terminal—clear app text-input focus via `surrender_ui_text_focus()` and restore the host window focus via `SetFocus(hwnd)` on Windows. This ensures browser URL input and native WebView2 keyboard focus do not block terminal input capture.

## Browser Panel WebView Z-Order Guidelines
- **Never use native-popup menus (menu_button) over the WebView content area.** Native WebView2 renders as a child window above egui's immediate-mode rendering. Popups like `ui.menu_button` that extend over the WebView area will appear BEHIND the WebView and be unusable.
- **Use inline dual buttons instead of menus for WebView toolbar actions.** For screenshot and similar toolbar actions that need multiple options, render side-by-side buttons within a single bordered frame directly in the toolbar (e.g., `[ Full page | Visible area ]`). This avoids all menu/popup complexity and keeps controls in the egui layer above WebView.
- **WebView must yield during MODAL overlay interactions only.** When actual modal overlays (dropdown menus, context menus, popups) are active in the browser panel, hide the WebView via `SetIsVisible(false)` so the overlay appears above it. Simple hover tooltips on toolbar buttons/tabs do NOT trigger WebView hiding—these render safely in the egui layer above WebView without needing native hide/show cycles.
- **Toolbar hover does NOT trigger WebView hide.** The toolbar buttons and tab strip render in egui's paint layer above the native WebView. Hover tooltips on these controls remain visible without requiring `browser_panel_overlay_active` to be set or WebView to be hidden. This prevents the black/white flicker bug where hovering toolbar caused WebView content to disappear.
- **Toolbar tooltips must appear above buttons.** Standard egui tooltips appear below widgets by default, which causes them to overlap the WebView content area and potentially get obscured. Use `browser_toolbar_icon_button()`, `browser_toolbar_toggle_button()`, or `show_tooltip_above()` helpers to position tooltips above toolbar buttons. The gap is controlled by `BROWSER_TOOLBAR_TOOLTIP_GAP` constant.
- **Use grace period for smooth modal overlay transitions.** When modal overlay closes, keep WebView hidden briefly (150ms via `BROWSER_OVERLAY_GRACE_PERIOD_MS`) to prevent flickering when moving mouse between controls.
- **No per-project menu state needed.** With inline buttons instead of togglable menus, no runtime state tracking (like `browser_screenshot_menu_open_by_project`) is required—buttons are always visible and clickable.

## Browser Panel Performance Guidelines
- **Cache native WebView2 state to avoid redundant COM calls.** The `EmbeddedBrowser` struct maintains `cached_visible: Option<bool>` and `cached_bounds: Option<BrowserBounds>` to track the last applied native state. `set_visible_internal()` and `sync_position_internal()` check these caches before calling WebView2's `SetIsVisible()` or `SetBounds()`.
- **Sync bounds before showing the browser.** In `sync_embedded_browser()`, always call `browser.sync_position(&bounds)` before `browser.show()`. This prevents the browser from becoming visible at wrong/old dimensions, which can cause white flicker.
- **Reset cached state on shutdown.** The `shutdown()` method must clear `cached_visible` and `cached_bounds` to ensure clean state when the browser is recreated.
- **Idempotent native operations prevent scroll flicker.** During scroll operations, egui may re-layout the browser panel every frame. Without caching, repeated `SetBounds()` calls to WebView2 cause the child window to invalidate and repaint, producing white/blank flicker artifacts.

## Browser Panel Compact UI Guidelines
- **Browser panel UI must minimize vertical chrome to maximize WebView space.** The panel should allocate ~60-80px total for all UI chrome (tabs + toolbar), leaving the rest for the embedded browser content.
- **Avoid separate header rows for titles or project names.** The browser panel header should not have a dedicated "Browser" title row or separate project name display; use the activity rail and terminal context to indicate the active project.
- **Tabs must stay on a single row using horizontal scroll.** Use `ScrollArea::horizontal()` around the tab strip to prevent tabs from wrapping to multiple lines. This keeps tab height predictable (22px) regardless of panel width.
- **Place add tab button inside ScrollArea next to last tab.** The add tab (+) button should be rendered inside the `ScrollArea::horizontal()` block, immediately after the tabs loop. This ensures the button stays visually connected to the last tab and scrolls with the tab strip. Button width should be ~28px with 14px icon.
- **Combine URL input and action buttons into one compact toolbar row.** Place the URL input field on the left taking available width, followed by icon-only buttons (Go, Clear, Design Inspect, Screenshot) on the same row with minimal 4px spacing.
- **Use reduced padding and margins throughout.** Inner margins should be 6px (not 10px); spacing between UI sections should be 4-6px (not 8-16px).
- **Reduce tab dimensions for compactness.** Tab height should be 22px (not 26px); tab close button should be 16px (not 18px); tab font should be 11px (not 12px).
- **Preserve all functionality in compact layout.** URL editing with double-click to select all, Enter to submit, tab switching/closing, and all toolbar buttons must remain fully functional. Do not add custom right-click context menus to URL input; rely on standard keyboard shortcuts and native egui behavior.
- **Maintain minimum URL input width.** The URL field should have a minimum width of 100px to remain usable even at narrow panel widths.
- **Use scrollable tabs at max tab limit.** With 5 tabs (BROWSER_MAX_TABS_PER_PROJECT), horizontal scrolling must work smoothly without clipping tab content.

## Browser Tab Lifecycle Guidelines
- **Closing the last tab leaves the browser empty (no auto-recreate).** When the last browser tab is closed via the X button or MCP `close` action, the browser enters an empty state with zero tabs. The tab state maps are cleaned up (`active_browser_tab_by_scope`, `browser_tabs_by_scope`, `browser_url_draft_by_scope` removed for the scope), and any active/inactive WebViews are shut down. Do not automatically recreate a new empty tab.
- **The (+) Add Tab button creates the first tab when none exist.** When the browser panel is in an empty state (no tabs), clicking the "+" button in the tab strip creates the first tab. If `browser_last_url` has a saved URL, the first tab is created with that URL pre-filled and navigation is triggered automatically.
- **Opening browser panel with saved URL auto-creates first tab.** When `draw_browser_panel()` detects that the browser is opening and `browser_last_url` exists but no tabs exist, it automatically creates the first tab with the saved URL and triggers navigation. The user does not need to press Enter or click Go.
- **URL input is empty when no tabs exist.** When the browser panel has no tabs, the URL input field should be empty (not auto-filled with `browser_last_url`). This ensures a clean state for the empty browser. The draft is only populated with `browser_last_url` when at least one tab exists.
- **Explicit tab creation only.** Do not call `ensure_browser_tab_state()` from `draw_browser_panel()`, `add_browser_tab()`, or `close_browser_tab()` to auto-create tabs. Tabs should only be created explicitly via:
  - User clicking the "+" button (`add_browser_tab` with `None` URL)
  - Auto-creation on panel open with saved URL
  - MCP `new` action
  - Video recording completion opening a recording tab
- **Cleanup on last tab close.** When `close_browser_tab()` closes the last remaining tab, it must:
  1. Remove `active_browser_tab_by_scope` entry for the scope
  2. Remove `browser_tabs_by_scope` entry for the scope
  3. Remove `browser_url_draft_by_scope` entry for the scope
  4. Remove all inactive browsers for the scope from `inactive_browser_tab_browsers`
  5. Shut down the active WebView if the closed tab was active

## Browser MCP Single-Binary Guidelines
- **Browser MCP helper runs inside the main executable.** Browser MCP functionality must run via `mergen-ade(.exe) --browser-mcp-helper`, not as a separate sidecar binary.
- **Do not ship a separate `mergen-browser-mcp(.exe)` binary.** Release ZIP/DMG must contain only the main Mergen executable; sidecar binaries are unsupported and must be removed.
- **Do not place Browser MCP helper code under `src/bin/`.** Code under `src/bin/` creates implicit Cargo binary targets that produce separate release executables. Place helper code in `src/browser_mcp_helper.rs` as a regular module.
- **OpenCode runtime config must use the helper-mode argument.** The MCP command array must be `[current_exe, "--browser-mcp-helper", "--caps=devtools,vision,network,storage"]`.
- **Release builds must target only `--bin mergen-ade`.** Do not use `--bins` in release workflows; it builds all binary targets including stale sidecars.
- **Clean stale `mergen-browser-mcp(.exe)` artifacts.** Release scripts must remove any existing sidecar executable from previous builds to prevent accidental packaging.
- **Helper mode runs headless before GUI initialization.** When `--browser-mcp-helper` is detected, run the MCP JSON-RPC loop and exit; skip all eframe/egui initialization, wgpu setup, and window creation.
- **Helper mode uses stdio pipes.** The helper reads JSON-RPC requests from stdin and writes responses to stdout; GUI subsystem executables on Windows still support stdio redirection via pipes.

## Browser MCP Multi-Terminal Isolation Guidelines
- **Browser instances must be terminal-scoped for MCP isolation.** Each terminal using the Browser MCP must have its own isolated WebView2 instance (`BrowserScopeKey::Terminal`) to prevent session conflicts when multiple AI agents control browsers in the same project simultaneously.
- **BrowserScopeKey enum distinguishes project vs terminal scope.** Use `BrowserScopeKey::Project(pid)` for legacy UI-initiated browser usage; use `BrowserScopeKey::Terminal { project_id, terminal_id }` for MCP-originated browser commands.
- **Terminal-scoped browsers use isolated profile directories.** Terminal browsers store their WebView2 user data in `webview2/projects/{project_id}/terminals/{terminal_id}/` (via `browser_user_data_dir_path_for_terminal()`), ensuring separate cookies, localStorage, and session state per terminal.
- **Project browsers remain for UI-initiated navigation.** When users click terminal HTTP links or manually open the browser panel, continue using project-scoped browsers (`BrowserScopeKey::Project`) to preserve the existing single-browser-per-project user experience.
- **Browser MCP commands always resolve to terminal scope.** The `resolve_browser_mcp_scope()` function returns `BrowserScopeKey::Terminal` based on authenticated `auth_scope.terminal_id`, ensuring MCP commands never share browser state between different terminal sessions.
- **Session ID validation prevents cross-session contamination.** Browser MCP requests must include the `session_id` from the auth scope; mismatch between request and auth scope session ID rejects the request to prevent session hopping attacks.
- **Browser state maps use BrowserScopeKey instead of raw project_id.** All browser state (tabs, URL drafts, embedded browser instances, design inspect state, video recordings) is keyed by `BrowserScopeKey` to support both project and terminal scopes uniformly.
- **UI panel shows active terminal's browser when available.** The browser panel displays the terminal-scoped browser when the active terminal has one open, falling back to project-scoped browser for terminals without terminal-specific browsers.
- **Terminal browser cleanup occurs on terminal close.** When a terminal exits, its terminal-scoped browser state (tabs, WebView instance, recordings) must be cleaned up to prevent resource leaks.
- **Terminal-scoped browser URLs are not persisted.** Unlike project-scoped browsers that persist `browser_last_url` to config, terminal-scoped browser URLs are runtime-only and do not survive application restart.

## Terminal Shortcut Guidelines
- **Terminal shortcuts are user-configurable.** Store custom terminal command shortcuts in `AppConfig::terminal_shortcuts`; do not hard-code new terminal command shortcuts directly in input handling.
- **Runtime shortcut matching must inspect all enabled entries in `AppConfig::terminal_shortcuts`.** Never hard-code only F6/F7/F8; match any enabled shortcut from config with its key and modifiers.
- **Default terminal shortcuts must remain available.** Defaults are `F5 -> /gt (GitHub Push)`, `F6 -> /prepare-fix-plan`, `F11 -> /implement-plan`, and `F7 -> /review-guard`, and missing config fields should recover these defaults. Legacy configs with old IDs (`semgrep-check`) or keys (`F7` for implement-plan, `F8` for review-guard) are automatically migrated to new defaults while preserving user customizations.
- **Shortcuts send commands to the active terminal through a paste-safe path.** A triggered terminal shortcut must submit the configured command with `TerminalRuntime::capture_paste_bytes()` / `send_paste_bytes()`, then send Enter as a separate runtime input. Do not type slash-prefixed shortcut commands as raw key streams because AI CLI slash menus can treat only `/` as the submitted action.
- **Launcher and saved-message paths remain separate from shortcut dispatch.** Launcher commands may keep using raw command submission for AI launch detection; terminal shortcut dispatch must not reuse launcher submission if doing so bypasses bracketed paste safety.
- **Shortcut regression tests must cover bracketed paste.** Slash-prefixed shortcut commands must be tested with bracketed paste enabled and should emit `ESC[200~<command>ESC[201~\r`.
- **Default shortcut normalization must restore missing built-ins.** Existing configs that predate a new built-in shortcut must have missing default entries restored on load while preserving user edits and disabled states for existing default entries.
- **Settings must allow creating arbitrary shortcut entries.** Support key capture, modifier editing (Ctrl/Alt/Shift/Cmd), label editing, command editing, add/remove, and reset to defaults.
- **Text input focus must block terminal shortcuts.** Do not trigger command shortcuts while settings fields, file editor, directory search, browser URL input, or other text inputs own keyboard focus.
- **Shortcut key events must be consumed before PTY routing.** Handle command shortcuts in `raw_input_hook` before `partition_terminal_input_events` so matched events are removed from the stream.
- **Duplicate enabled shortcut combos must block ambiguous execution.** When multiple enabled shortcuts share the same key/modifier combination, detect the conflict, show a warning status, and execute none of them.
- **Shortcut conflicts must be visible in Settings.** Display duplicate key combination warnings in the Shortcuts settings panel with specific combo details.
- **Shortcut partitioning must be gated by keyboard capture before buffering.** In `raw_input_hook`, check `should_capture_terminal_keyboard()` before calling `partition_terminal_command_shortcuts()`. Only buffer shortcuts when the terminal owns the keyboard; when the UI owns the keyboard (Settings open), leave events for the UI.
- **Buffered terminal command shortcuts must be drained when UI gains focus.** In `handle_shortcuts()`, if `ui_owns_keyboard` is true, drain `buffered_terminal_command_shortcuts` without executing to prevent stale shortcuts from firing after Settings closes.
- **`ShortcutModifiers::command` means physical macOS Command key only.** Use `egui::Modifiers::mac_cmd` (physical Command key) when storing and matching shortcuts, not `egui::Modifiers::command` which is a cross-platform alias that equals Ctrl on Windows/Linux.
- **Normalize captured modifiers using `egui_modifiers_to_stored()`.** Convert egui modifiers to stored representation using `mac_cmd` for the `command` field to ensure Ctrl on Windows/Linux doesn't incorrectly set `command=true`.
- **Backward compatibility for stored shortcuts on non-macOS.** When matching on Windows/Linux, treat `ctrl=true, command=true` as legacy Ctrl-only state because old captures may have set `command=true` due to the egui alias bug. Do not degrade `command=true, ctrl=false` entries to plain-key shortcuts; those command-only shortcuts are unpressable on non-macOS and must not execute.
- **Shortcut recording cancellation must clear runtime state immediately.** Both Escape key and Cancel button must set `settings_shortcut_recording_index = None` in the same frame. When cancelling, discard any key captured during that frame to prevent unwanted assignment.
- **Shortcuts send a second Enter after a short delay for confirmation.** After sending the command with the first Enter, schedule a second Enter after `SHORTCUT_SECOND_ENTER_DELAY_MS` (250ms) via `pending_shortcut_second_enter`. This ensures AI CLI tools and shells process the command and any confirmation prompts that may appear. Use `process_pending_shortcut_second_enters()` in the update loop to handle the delayed sends.

## Clipboard Paste Guidelines
- **Terminal paste should preserve text fallback.** Text clipboard paste must continue to use the existing queued paste path.
- **Clipboard images paste as paths.** If the clipboard contains an image file path, paste the image path into the terminal instead of image bytes.
- **On Windows, copied image files from Explorer must be read from CF_HDROP before bitmap materialization.** This preserves the original file path and avoids creating duplicate saved images.
- **Prefer the original copied image file path over saving a duplicate bitmap.** When CF_HDROP provides an image file path, use that path directly instead of materializing a new screenshot.
- **Bitmap clipboard images must be materialized.** If the clipboard contains bitmap/image data without a file path, save it to a user-accessible screenshots folder and paste the saved file path.
- **Do not block normal paste on image failures.** If image extraction or saving fails, fall back to text clipboard paste when text exists; otherwise show a clear status-line error.
- **Generated image paths must be terminal-safe.** Normalize generated paths consistently and avoid control characters in filenames.
- **Clipboard image path normalization must reject control characters and produce terminal-safe paths.**
- **CF_HDROP handle must be used directly without GlobalLock.** The handle returned by `GetClipboardData(CF_HDROP)` is an `HDROP` handle; pass it directly to `DragQueryFileW`. Do not call `GlobalLock` on it - that returns a pointer to `DROPFILES`, not an `HDROP` handle.
- **Clipboard close must be guaranteed on all return paths.** Use a scope guard or RAII pattern to ensure `CloseClipboard()` is called after successful `OpenClipboard()`, even on early returns or errors.

## Resizable Panel Guidelines
- **Side panels should be horizontally resizable.** Project Explorer, Check-list, and Browser panels should allow mouse-driven width resizing while keeping full-height SidePanel behavior.
- **Panel widths are persisted UI config.** Store user-resized widths in `UiConfig` and clamp them to safe min/max ranges.
- **Do not make settings popups resizable.** Modal/pop-up windows such as Settings must keep their fixed sizing unless explicitly redesigned.
- **Avoid per-frame config writes.** Persist resized panel widths only when width changes meaningfully to prevent excessive disk writes.
- **Config recovery must preserve persisted panel width fields.** Ensure `recover_config_state()` preserves `project_explorer_width`, `checklist_panel_width`, and `browser_panel_width` when `pending_config_changes.ui` is true.

## Directory Icons Guidelines
- **Directory rows must include stable icons.** File and folder rows should render IDE-like icons without changing lazy loading, search filtering, or row ordering behavior.
- **Directory file icons are extension-based only.** Do not add blocking metadata reads just to choose icons; use the existing `DirectoryNode` path/name data.
- **Directory search highlighting must remain char-safe with icons.** Adding icons must not alter UTF-8-safe match highlighting or split multi-byte characters.

## Browser Design Inspect Guidelines

- **Design Inspect delivery is click-only.** Hover and pointer movement may update the highlight overlay but must never send context to the terminal.
- **Design Inspect clicks must block page actions.** While inspect mode is enabled, selecting an element must prevent normal page click behavior such as link navigation, button handlers, and form submission.
- **Design Inspect auto-disables after successful delivery.** When a user clicks a page element and the design inspect info is successfully queued to the terminal, the mode is automatically disabled to prevent accidental duplicate clicks. Users must re-enable design inspect via the toolbar button to select another element.
- **Browser events must use selection semantics.** Use click/selection event names such as `DesignElementClicked`; do not reintroduce terminal forwarding from hover events.
- **Stale hover messages must fail closed.** Ignore `type: "hover"` design-inspect messages from old injected scripts instead of forwarding them to terminals.
- **Bump the injected script version when Design Inspect behavior changes.** This prevents an existing `window.__mergenDesignInspect` implementation from short-circuiting around newer behavior.
- **Add regression tests for Design Inspect behavior.** Cover click parsing, hover rejection, duplicate click dedupe, stale URL gating, iframe page URL gating, auto-disable after delivery, and user-facing enable/status copy.
