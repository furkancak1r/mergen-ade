# Repository Guidelines

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

## Terminal Input & History Invariants
- `pending_line_for_title` is for title/AI command detection only; it clears on newlines (\r or \n) since titles should reflect only the current logical line. This buffer is capped to `TERMINAL_PENDING_LINE_MAX_CHARS` (512) to prevent unbounded growth.
- `pending_input_for_history` preserves the complete raw input including multi-line content and Unicode characters (e.g., box-drawing characters like `┃`). This buffer is **NOT** capped—full prompts of any length must be preserved for history and rerun functionality.
- When Enter is pressed, history is recorded from `pending_input_for_history` (taking the full raw text), while title updates use `pending_line_for_title` (taking only the last logical line).
- Runtime `recent_inputs` (for tooltips and rerun) must be populated from the raw `history_line`, not the sanitized title candidate. This ensures multi-line prompts appear correctly in Terminal Manager and background rerun replays the full command.
- Backspace pops from both buffers to keep them in sync for single-character deletion.
- Always use char-safe truncation helpers (e.g., `capped_hover_text()`) for UI display of user text; never use byte-index slicing like `text[..60]` which can panic on multi-byte UTF-8 sequences or split Unicode code points.

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
