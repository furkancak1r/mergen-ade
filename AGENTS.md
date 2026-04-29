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
- **Do not interpolate GitHub contexts directly**: In workflow `run:` steps, use environment variables instead of direct `${{ github.ref_name }}` interpolation to avoid shell injection risks (per Semgrep findings).
