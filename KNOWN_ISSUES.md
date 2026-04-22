### Known Issues & Fix Log

#### Windows wgpu renderer now supports DX12 with GL fallback and preflight probe {#windows-wgpu-dx12-gl-fallback-preflight}
- Date: 2026-04-17
- Context: Windows systems where wgpu DX12 backend fails to create surface (e.g., remote desktop, software rendering)
- Error signature: `WGPU error: Failed to create surface for any enabled backend: {}` and fallback to glow taking multiple seconds with ERROR logs visible to user.
- Symptoms/Impact: Startup showed ERROR-level log from eframe before successfully falling back to glow (OpenGL). This was visually jarring and confusing for users.
- Root cause:
  1. `setup_windows_wgpu_env_defaults()` was setting `WGPU_BACKEND=dx12` only, with no fallback backend.
  2. On systems where DX12 cannot create a surface (e.g., software rendering, remote desktop), wgpu would fail completely.
  3. eframe logs renderer initialization errors at ERROR level before returning Err to caller.
  4. The fallback to glow only happened after the full wgpu attempt failed, producing visible ERROR logs.
- Resolution:
  - Changed `WGPU_BACKEND` default from `dx12` to `dx12,gl` to allow wgpu to try OpenGL backend if DX12 fails.
  - Added `preflight_probe_wgpu()` function that creates a temporary wgpu Instance and enumerates adapters before attempting full renderer initialization.
  - Updated `RendererMode::Auto` to run preflight probe first; if it fails (no adapters available), skip directly to glow without attempting full wgpu initialization.
  - This prevents the ERROR log spam from eframe's `run_native` when we already know wgpu won't work.
- Prevent recurrence:
  - Always provide multiple backend options in `WGPU_BACKEND` when possible, or at least GL as fallback.
  - Use preflight probes for expensive initialization paths that can fail gracefully.
  - Log preflight failures at WARN level (not ERROR) since fallback is expected behavior.
- Files/Commands touched: `src/main.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo run`

#### Windows wgpu renderer defaults to DX12 and disables debug validation to suppress Vulkan warnings {#windows-wgpu-dx12-default-no-validation}
- Date: 2026-04-17
- Context: Windows debug builds with wgpu renderer (cargo run)
- Error signature: `wgpu_hal::vulkan::instance] InstanceFlags::VALIDATION requested, but unable to find layer: VK_LAYER_KHRONOS_validation` and `Unrecognized present mode SHARED_DEMAND_REFRESH`
- Symptoms/Impact: Startup console spam with multiple WARN-level messages from wgpu_hal::vulkan module every time the app launched in debug mode.
- Root cause:
  1. `wgpu` by default selects Vulkan backend on Windows when available.
  2. Debug builds enable `InstanceFlags::VALIDATION` by default, but the validation layer is not available on most Windows systems.
  3. Vulkan backend logs warnings about unrecognized present modes (SHARED_DEMAND_REFRESH, SHARED_CONTINUOUS_REFRESH).
- Resolution:
  - Added `setup_windows_wgpu_env_defaults()` that sets `WGPU_BACKEND=dx12` on Windows when not already set by user.
  - For debug builds, also sets `WGPU_VALIDATION=0` to suppress validation layer warnings.
  - Added log module filters for `wgpu_hal::vulkan`, `wgpu_hal::vulkan::conv`, `wgpu_hal::vulkan::instance` at Error level as last-resort suppression.
- Prevent recurrence:
  - User can still override with explicit `WGPU_BACKEND` or `WGPU_VALIDATION` environment variables.
  - Log filters only suppress WARN-level noise; actual errors will still be logged.
- Files/Commands touched: `src/main.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo run`

#### Codex CLI integration is now strictly hook-only (notify/visible/title methods removed) {#codex-cli-hook-only-integration}
- Date: 2026-04-17
- Context: Windows and cross-platform Codex CLI integration
- Error signature: `Codex CLI previously used multiple signal sources (notify, visible UI, title, hooks) causing inconsistent state tracking and complexity.`
- Symptoms/Impact: The previous multi-source approach led to:
  - Race conditions between notify and visible UI signals
  - Platform-specific behavior differences (Windows vs macOS/Linux)
  - Complex state machine with fallback chains that were hard to reason about
  - Tests that depended on specific visible text patterns that could change between Codex versions
- Root cause:
  1. Mergen tried to support all possible signal sources: BEL notifications, visible UI text detection, title patterns, and hooks.
  2. Each source had different semantics and timing, leading to conflicting state updates.
  3. Windows upstream disabled hooks, forcing reliance on less reliable notify/visible methods.
  4. Maintenance burden of keeping visible text parsers in sync with Codex TUI changes.
- Resolution:
  - **Switched to strict hook-only integration** following the pattern used by OpenCode.
  - `UserPromptSubmit` hook → `Running` state (spinner appears)
  - `Stop` hook → `Attention`/`Idle` state (pulse appears)
  - Removed all notify/TUI-based notification handling (`CODEX_NOTIFICATION_METHOD`, `CODEX_*_EVENT` constants).
  - Removed all visible UI detection for Codex (`PendingVisibleCodexStatus`, `detect_visible_codex_status_with_end()`).
  - Removed title-based detection for Codex (`detect_codex_status_from_title()`, `is_codex_agent_title()`).
  - Removed `CustomNotifyHook` and `HooksEnabledNotifyLimited` enum variants.
  - Updated `CodexIntegrationStatus` to only track hook-based states: `EnabledHealthy`, `HooksConfiguredUnverified`, `NeedsSetup`, `ConfigReadError`.
  - Updated `patch_codex_config_file()` to configure `hooks.json` instead of notify commands.
  - Updated all tests to use hook event format instead of notify/visible patterns.
  - Fixed `update_from_title()` in hooks.rs to return `None` for Codex (enforcing hook-only).
- Prevent recurrence:
  - Keep AI CLI integrations simple and single-source. Choose one reliable method and stick to it.
  - Hooks are more reliable than visible text parsing because they're explicit events, not inferred from TUI rendering.
  - When hooks are unavailable on a platform, prefer explicit launch/prompt detection over brittle text matching.
  - Document the hook-only requirement in AGENTS.md and integration settings.
- Files/Commands touched: `src/codex.rs`, `src/app.rs`, `src/terminal.rs`, `src/hooks.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: AGENTS.md AI CLI Integration section updated with hook-only specification.
- Date: 2026-04-17
- Context: native Windows Codex CLI launched from `cargo run` inside Mergen
- Error signature: `Spinner starts when a Codex prompt is sent, but never stops or turns into pulse after the turn finishes.`
- Symptoms/Impact: On native Windows, Mergen showed the Codex spinner as soon as the user submitted a prompt, but the badge stayed in `Running` even after Codex returned to the idle composer.
- Root cause:
  1. Mergen only treated the explicit visible text `your turn is complete ... turn complete` as Codex completion.
  2. Codex `v0.121.0` on Windows often finishes by re-rendering an empty `›` prompt plus the model footer instead of printing that explicit completion banner.
  3. `submitted_codex_prompt` still called the legacy `apply_codex_status(Running, ...)` path, so `codex_normalized_status` never became `Working` for prompt-submit-driven turns.
  4. Even when a later completion signal arrived, pulse arming depended on a previous normalized `Working -> Idle` transition, so the state machine could miss the pulse.
- Resolution:
  - Switched Codex prompt-submit handling to `apply_codex_transport_status(CodexTransportStatus::Working, ...)` in both live input and saved-message paths.
  - Added a guarded visible completion fallback that detects the empty prompt return pattern (`›` followed by model footer) without treating a typed user prompt as completion.
  - Ignored `codex-turn-complete` visible chunks unless the terminal was already in a real working state (`normalized Working`, `Running`, or `prompt_submit_since`).
  - Added regression tests for empty prompt return, typed-prompt false positives, and working/non-working turn-complete chunk handling.
- Prevent recurrence:
  - Do not assume Codex Windows prints the same completion text as OpenCode.
  - Keep Codex on the normalized transport-state path for all submit/start transitions.
  - Guard visible completion parsers with prior working-state evidence so startup chrome and echoed prompts cannot trigger false pulse.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test codex`, `cargo test`
- References: Local transcript from Codex `v0.121.0` showing idle re-entry as an empty `›` prompt with `gpt-5.4 medium` footer rather than an explicit `turn complete` banner.
#### Codex CLI managed bridge now prevents stale wiring when Mergen binary moves {#codex-cli-managed-bridge-prevents-stale-wiring}
- Date: 2026-04-16
- Context: main/Windows Codex CLI integration when Mergen is installed in different locations
- Error signature: `Codex CLI hooks work initially but stop producing events after Mergen is moved or updated; spinner starts but never stops.`
- Symptoms/Impact: Codex CLI sessions would show the spinner when work started (via launch detection or BEL), but hook events (`UserPromptSubmit`, `Stop`) would never arrive. The `~/.codex/config.toml` and `~/.codex/hooks.json` files pointed to stale paths from previous Mergen installations (e.g., `target/debug/mergen-ade.exe` or `C:/Users/.../Desktop/mergen-ade.exe`), while the actual running Mergen was at a different location. The spinner would start via fallback paths but never receive the `Stop` hook to transition to idle/pulse.
- Root cause:
  1. Mergen configured `~/.codex/config.toml` and `~/.codex/hooks.json` with the current executable's absolute path.
  2. When the user moved Mergen or updated via a new release binary, the Codex config files still referenced the old path.
  3. Codex CLI tried to invoke the hook commands at the old path, which either didn't exist or was an older version that didn't handle the events properly.
  4. Hook events silently failed because the executable at the configured path couldn't be found or didn't process the `--codex-hook` argument.
  5. The health check (`inspect_codex_cli_integration`) only checked for hook markers, not that the hooks targeted the actual running Mergen binary.
- Resolution:
  - Introduced a **managed bridge pattern** with a fixed installation location at `%APPDATA%\Mergen\MergenADE\bin\mergen-codex-bridge.exe`.
  - Added `config::codex_bridge_path()` that returns this fixed path regardless of where the actual Mergen binary is installed.
  - Implemented `codex::ensure_codex_bridge_installed()` that copies the current executable to the bridge location if:
    - Bridge doesn't exist
    - Bridge is older (modification time) than current executable
    - Bridge has different file size
  - Updated `enable_codex_cli_integration()` and `inspect_codex_cli_integration()` to use the bridge path instead of the current executable path.
  - Updated `patch_codex_config_file()` and `update_codex_hooks_json()` to generate commands targeting the bridge path.
  - Strengthened `check_codex_hooks_json()` to verify hooks contain the bridge path, not just the marker strings.
  - Added startup self-heal in `AdeApp::bootstrap()` that ensures bridge is installed before any Codex operations.
  - Added bridge diagnostics in the Settings panel showing:
    - Bridge status (Installed/Not installed)
    - Wiring mismatch warning when hooks point to a different path
  - Added `BridgeInstallFailed` outcome variant for error handling.
  - Added tests:
    - `install_codex_bridge_copies_executable_to_bridge_path`
    - `check_codex_hooks_json_requires_bridge_path_match`
- Prevent recurrence:
  - Always use a fixed, managed location for hook/notify executables rather than dynamic paths that change with installation location.
  - Health checks should verify both the existence of configuration AND that it targets the expected executable path.
  - Startup should auto-repair (self-heal) managed installations before user interaction.
  - Provide visible diagnostics for bridge/wiring status in Settings.
- Files/Commands touched: `src/config.rs`, `src/codex.rs`, `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test codex`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: 2026-04-16 live system diagnosis showing `~/.codex/config.toml` pointing to stale `target/debug/mergen-ade.exe` while running binary was at `Desktop/mergen-ade.exe`

#### Codex hook failures in mixed AI terminals were caused by shared tool-hint env collisions {#codex-hook-failures-shared-tool-hint-env-collisions}
- Date: 2026-04-16
- Context: main/Windows Codex and OpenCode sessions launched from the same Mergen terminal environment
- Error signature: `UserPromptSubmit hook (failed) error: hook exited with code 1`
- Symptoms/Impact: Codex showed hook failure banners instead of spinner/attention updates when launched from terminals that also carried OpenCode env setup. `Stop` hooks failed the same way, so the session never produced reliable state updates.
- Root cause:
  1. `TerminalRuntime::spawn()` injects both Codex and OpenCode env vars into the terminal process.
  2. Both integrations reuse the shared `MERGEN_AI_TOOL_HINT` variable.
  3. OpenCode env injection runs after Codex, leaving `MERGEN_AI_TOOL_HINT=opencode` in mixed terminals.
  4. Codex `write_codex_notify_event()` and `write_codex_hook_event()` treated that shared hint as authoritative and returned `Unexpected tool hint: opencode`, causing the hook command to exit with code 1.
- Resolution:
  - Made Codex and OpenCode treat the shared tool-hint env var as advisory only instead of a hard validation gate.
  - Added a Codex regression test that reproduces the real mixed-terminal failure (`MERGEN_AI_TOOL_HINT=opencode`) and verifies hooks still write inbox events.
  - Added an OpenCode regression test that verifies notify events still write successfully with a mismatched hint.
- Prevent recurrence:
  - Do not use a single shared env var as a hard gate for multiple AI CLIs that can run in the same terminal lifetime.
  - Use tool-specific tokens and inbox paths as the routing source of truth when more than one tool can coexist.
  - Keep at least one direct CLI-mode regression (`--codex-hook`, notify writer) for cross-tool env contamination cases.
- Files/Commands touched: `src/codex.rs`, `src/opencode.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test codex`, `cargo test opencode`, `cargo run -- --codex-hook UserPromptSubmit`
- References: Follow-up to the earlier 2026-04-16 Windows Codex spinner investigation; local repro showed `Failed to process Codex hook event: Unexpected tool hint: opencode`.
#### Codex CLI Windows spinner now shows immediately via launch/prompt fallback paths {#codex-cli-windows-spinner-immediate-via-fallback}
- Date: 2026-04-16
- Context: main/Windows native Codex CLI sessions (not WSL)
- Error signature: `Codex CLI on Windows showed no spinner during work; hooks are disabled upstream so only BEL notifications triggered state changes.`
- Symptoms/Impact: Windows native Codex CLI sessions showed no visual feedback (no spinner) when Codex was working because hooks are disabled on Windows upstream. Only process exit or BEL notifications would clear the idle state. The initial prompt submission and launch phases had no visual indication.
- Root cause:
  1. OpenAI Codex upstream explicitly disables hooks on Windows per documentation: "Hooks are currently disabled on Windows" (developers.openai.com/codex/hooks).
  2. Mergen's `mark_codex_launch_pending()` set `AiCliStatus::Inactive` which meant no spinner appeared during launch detection phase.
  3. `ai_badge_visual()` ignored the `_codex_attention_pending` parameter, causing Codex attention states (Permission/Idle) to not show the correct pulse/solid visual.
  4. `submitted_codex_prompt` handling only checked `current_codex_status.source` without considering `codex_launch_pending_since`, so the first prompt after launch did not trigger Running state.
  5. `should_show_codex_enable_button()` did not show the enable button when hooks were not runtime verified but integration was otherwise healthy.
- Resolution:
  - Changed `mark_codex_launch_pending()` to set `AiCliStatus::Running` instead of `Inactive`, so spinner appears immediately when launch is detected.
  - Fixed `ai_badge_visual()` to use `_codex_attention_pending` parameter for determining pulse vs solid visuals in Codex attention states.
  - Extended `submitted_codex_prompt` handler with `codex_launch_pending_since.is_some()` condition to transition to Running on first prompt when launch pending.
  - Updated Windows settings text to describe "fallback paths" (launch detection, prompt, notify/BEL) instead of implying hooks work.
  - Fixed `should_show_codex_enable_button()` to show enable button when `EnabledHealthy { hooks_runtime_verified: false }`.
- Prevent recurrence:
  - Always verify upstream platform support before relying on hook-based integrations.
  - Provide multiple independent signal paths (launch detection, prompt tracking, notify/BEL, hooks when available) for critical UI feedback like spinners.
  - Test `codex` tagged tests after any AI CLI integration changes: `cargo test codex`.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test codex`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: OpenAI Codex documentation "Hooks are currently disabled on Windows"

#### Multi-terminal and OpenCode performance optimizations implemented {#multi-terminal-opencode-performance}
- Date: 2026-04-15
- Context: main/Windows 5+ terminal performance with OpenCode sessions
- Error signature: `OpenCode sometimes shows black screen or opens slowly when 5+ terminals are active.`
- Symptoms/Impact: When opening 5+ terminals with OpenCode, some terminals would show "Terminal is resizing..." placeholder for extended periods (appearing as black screen), and OpenCode launch would feel sluggish. Process polling for multiple terminals caused redundant Windows syscall overhead.
- Root cause: Several inefficiencies compounded under multi-terminal load:
  1. Frame budget was not a hard limit: `skip_full_snapshot` only triggered when `render_cache.lines.is_empty()`, allowing dirty terminals with cached content to still consume budget slots, starving background terminals.
  2. Selection snapshot was always created: `try_terminal_snapshots()` created both normal and selection snapshots unconditionally, even though selection is only needed for copy/hover operations.
  3. Synchronous process probe at launch: `mark_opencode_launch_pending()` and `mark_codex_launch_pending()` captured process baseline snapshots synchronously during terminal creation, blocking the UI thread.
  4. Per-terminal process snapshots: Each terminal's process polling called `snapshot_processes()` independently, causing O(n²) Windows Toolhelp32 snapshot overhead when polling n terminals.
- Resolution:
  - Hardened frame budget logic: `skip_full_snapshot` now applies whenever `budget_exhausted && !is_active_or_visible && !render_cache.lines.is_empty()`, properly deferring all background terminals after the budget is consumed (src/app.rs:11978-12001).
  - Lazy selection snapshot: `try_terminal_snapshot()` returns only the normal snapshot; selection snapshot is created on-demand only when `terminal.selection.is_some()` (src/app.rs:12020-12025, src/terminal.rs:2963-2980).
  - Async launch baseline: Removed synchronous `snapshot_opencode_descendant_processes()` and `snapshot_codex_descendant_processes()` calls from launch path; baseline is now captured on first poll tick (src/app.rs:3046-3061).
  - Process snapshot caching: Added `snapshot_processes_cached()` with thread-local 50ms TTL cache in src/terminal.rs:1718-1750. Multiple process probes within the same tick reuse the same Windows process snapshot, reducing overhead from O(n) syscalls to O(1) per poll tick.
  - Updated all process tracking methods in TerminalRuntime to use `snapshot_processes_cached()` (src/terminal.rs:740, 779, 812, 832, 861, 884).
- Prevent recurrence:
  - When adding new AI CLI integrations, avoid synchronous process probes during terminal creation.
  - Keep snapshot budget limits hard - active/visible terminals should always have priority.
  - Use cached process snapshots for any operation that probes multiple terminals in a loop.
  - Lazy-load expensive secondary data (selection snapshots, detailed process info) only when actually needed.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: Performance analysis of 5+ terminal scenarios with simultaneous OpenCode sessions.

#### Codex terminal scroll now works properly with full scrollback and correct mouse wheel handling {#codex-terminal-scroll-now-works-properly}
- Date: 2026-04-15
- Context: main/Windows terminal scroll behavior when Codex CLI TUI is active
- Error signature: `Codex terminal scrolls down unnecessarily when starting; cannot scroll up as far as desired; mouse wheel feels stuck.`
- Symptoms/Impact: When opening Codex CLI, the terminal viewport would jump too far down and users couldn't scroll up to see previous content. The mouse wheel would also feel "stuck" when trying to scroll through the transcript, as if the scroll was being intercepted by the TUI application even when just reviewing history.
- Root cause: Two separate issues combined to create poor scroll UX:
  1. Mouse wheel capture was too aggressive: `is_mouse_reporting_active()` in `src/terminal.rs:659-665` returned true whenever either `is_mouse_grabbed()` OR `is_alt_screen_active()` was true. Codex (and many TUIs) enable the alternate screen on startup, which immediately caused all mouse wheel events to be forwarded to the PTY instead of scrolling the transcript view. This prevented users from reviewing scrollback history using the mouse wheel.
  2. Scrollback history was artificially truncated: `MAX_SNAPSHOT_ROWS` was set to 500 while `DEFAULT_SCROLLBACK` was 1000. This meant the terminal backend kept 1000 lines of history but the UI only rendered 500, so users could only see half their scrollback even when scrolling manually.
- Resolution:
  - Narrowed `is_mouse_reporting_active()` to only check `is_mouse_grabbed()` (line 664). Now mouse wheel events are only forwarded to the PTY when the application has explicitly requested mouse control (e.g., via `\x1b[?1000h`), not merely because alt-screen is active.
  - Increased `MAX_SNAPSHOT_ROWS` from 500 to 1000 (line 57), matching `DEFAULT_SCROLLBACK`. Users can now scroll through their full scrollback history in the UI.
- Prevent recurrence:
  - When implementing mouse event forwarding, distinguish between "mouse reporting is enabled" (application wants mouse events) and "alternate screen is active" (application is a TUI). These are orthogonal concerns.
  - Keep UI scrollback limits (`MAX_SNAPSHOT_ROWS`) in sync with backend limits (`DEFAULT_SCROLLBACK`) so users can access all their history.
  - Test mouse wheel behavior with both TUI applications (Codex, vim) and transcript-heavy terminals (long build outputs).
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: This fix addresses the "terminal scrolls down unnecessarily" and "cannot scroll up as far as desired" symptoms reported after the previous activation scroll fix (2026-04-13 entry below).

#### Codex CLI hooks integration now enables spinner on Windows {#codex-cli-hooks-integration-now-enables-spinner-on-windows}
- Date: 2026-04-15
- Context: main/Windows Codex CLI integration via hooks (Codex CLI 0.120.0+)
- Error signature: `Codex CLI on Windows had no hooks support; spinner could only be triggered by process detection and BEL notifications.`
- Symptoms/Impact: Codex CLI sessions on Windows showed no visual feedback (spinner) during work because hooks were disabled. The only completion signal was BEL notification or process exit detection, which was less reliable than hook-based events.
- Root cause: 
  - Codex CLI historically disabled hooks on Windows (gated behind platform checks).
  - Codex CLI 0.120.0 removed the Windows gate (PR #17268), making hooks available on Windows.
  - Mergen ADE's Codex integration only used notify-based inbox events and title-based detection.
  - No `CodexCliStatusSource::Hook` variant existed to handle hook-based status changes.
- Resolution:
  - Updated `patch_codex_config_file()` in `src/codex.rs` to set `[features].codex_hooks = true` in Codex config.
  - Added `update_codex_hooks_json()` to manage `~/.codex/hooks.json` with Mergen's hook handlers:
    - `UserPromptSubmit` -> writes `running` inbox event (spinner starts)
    - `Stop` -> writes `attention` inbox event (spinner stops, pulse/idle shown)
  - Added `handle_codex_hook_from_env()` to process hook events from Codex CLI and write to inbox.
  - Added `--codex-hook` CLI mode in `src/main.rs` for Codex to call Mergen when hooks fire.
  - Added `CodexCliStatusSource::Hook` variant in `src/app.rs` with proper state machine handling.
  - Updated `apply_codex_notify_inbox_event()` to map `user-prompt-submit` to Working and `agent-turn-complete` to Idle.
  - Updated Settings diagnostics text to reflect hooks+notify format.
  - Added comprehensive tests for hook event handling and config patching.
- Prevent recurrence:
  - When Codex CLI releases new platform support, update Mergen's integration to leverage it.
  - Keep hook-based and notify-based paths as independent signal sources for redundancy.
  - Test hook events explicitly in unit tests to verify inbox writing behavior.
- Files/Commands touched: `src/codex.rs`, `src/main.rs`, `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: Codex CLI 0.120.0 release notes (PR #17268 "remove windows gate that disables hooks")

#### Renderer backend selection now supports wgpu with glow fallback {#renderer-backend-wgpu-glow-fallback}
- Date: 2026-04-15
- Context: main/Windows rendering backend stability
- Error signature: `Application crashes in igxelpicd64.dll (Intel OpenGL driver) during extended overnight sessions; no fallback to safer rendering backend.`
- Symptoms/Impact: Mergen ADE would crash when left open overnight on systems with Intel Iris Xe Graphics using OpenGL backend. Crash dumps showed faults in `igxelpicd64.dll` with exception `0xc0000005`. No automatic recovery or fallback mechanism existed.
- Root cause: 
  - The application was compiled with only the `glow` (OpenGL) backend enabled in `eframe`.
  - Intel's OpenGL driver (`igxelpicd64.dll` v32.0.101.5542) showed instability during extended GPU-intensive sessions with continuous repaint cycles (16ms fallback refresh).
  - No mechanism existed to automatically fall back to a more stable backend when the primary backend failed.
- Resolution:
  - Added `wgpu` feature alongside existing `glow` in `Cargo.toml` to enable dual-backend support.
  - Implemented `RendererMode` enum with three modes: `Auto`, `Wgpu`, `Glow`.
  - Created `MERGEN_RENDERER` environment variable for user control (`auto`, `wgpu`, `glow`).
  - In `Auto` mode: attempts `wgpu` first (better stability on Intel), falls back to `glow` on failure.
  - Extracted `NativeOptions` construction to `build_native_options()` helper for DRY code.
  - Extracted app creator closure to `make_app_creator()` function to allow multiple launch attempts.
  - Added startup logging to track which renderer was selected and whether fallback occurred.
- Prevent recurrence:
  - Always test renderer changes on target hardware (especially Intel integrated graphics).
  - Monitor `WER` (Windows Error Reporting) logs for `igxelpicd64.dll` or similar driver crashes.
  - When adding new GPU-intensive features, consider testing with both `wgpu` and `glow` backends.
  - Document renderer override environment variable in user-facing documentation.
- Files/Commands touched: `Cargo.toml`, `src/main.rs`, `cargo build --release --target x86_64-pc-windows-msvc`

#### Codex terminal scrolls too far down on activation when in bottom row {#codex-terminal-scroll-issue-fixed}
- Date: 2026-04-13
- Context: main/Windows terminal manager with Codex CLI sessions in bottom-row tiles
- Error signature: `Codex terminal opens too far scrolled down; user needs to scroll up to see the prompt.`
- Symptoms/Impact: When activating a Codex terminal (especially in bottom row tiles with less height), the viewport would open scrolled too far down, making the prompt invisible. Users had to manually scroll up to see their cursor/input line.
- Root cause: Coordinate system mismatch in truncated snapshots. When terminal scrollback exceeds `MAX_SNAPSHOT_ROWS` (500), the snapshot truncates to a window of recent rows. The cursor `y` coordinate was stored as an **absolute** screen row (including scrollback), while `snapshot.lines` became **relative** to the truncation start. This caused `terminal_activation_scroll_offset()` to calculate an offset targeting a row far outside the visible snapshot bounds, resulting in the viewport being clamped to the bottom.
  - `snapshot_from_terminal()` in `src/terminal.rs:3056` called `snapshot_cursor()` which returned absolute coordinates.
  - `terminal_activation_scroll_offset()` in `src/app.rs:15765` used `snapshot.cursor.map(|c| c.y)` without clamping against `snapshot.lines.len()`.
  - This was more noticeable for Codex because Codex produces more output during startup, hitting the truncation threshold more easily.
- Resolution:
  - Fixed `snapshot_cursor()` in `src/terminal.rs:3268` to accept `snapshot_start_row` parameter and return cursor coordinates **relative to the truncated snapshot window** instead of absolute screen coordinates.
  - Updated call sites in `snapshot_from_terminal()` (line 3056) and `selection_snapshot_from_terminal()` (line 3180) to pass `snapshot_start_row`.
  - Added defensive clamping in `terminal_activation_scroll_offset()` in `src/app.rs:15781-15782` to ensure `target_row` never exceeds `snapshot.lines.len() - 1`.
  - Added regression test `snapshot_cursor_row_is_relative_to_truncated_window` in `src/terminal.rs:5721` to verify cursor coordinates are relative after truncation.
  - Added regression test `terminal_activation_scroll_offset_clamps_out_of_bounds_cursor` in `src/app.rs:19762` to verify activation scroll clamps out-of-bounds cursor rows.
- Prevent recurrence:
  - When working with truncated snapshots, always ensure cursor coordinates are in the same coordinate space as `snapshot.lines` (0-based relative to the truncated window).
  - Activation scroll offset calculations should always clamp against visible bounds to avoid scrolling beyond content.
  - When `MAX_SNAPSHOT_ROWS` truncation is active, any code comparing cursor row to line indices must account for the truncation offset.
- Files/Commands touched: `src/terminal.rs`, `src/app.rs`, `cargo test`, `cargo fmt`

#### Claude Code title-based detection and spinner/pulse now supported {#claude-code-title-based-detection-and-spinner-pulse-now-supported}
- Date: 2026-04-12T00:00:00Z
- Context: main/Windows/macOS local Claude Code CLI integration via terminal title-based detection (Orca-compatible)
- Error signature: `Claude Code sessions were not detected; the badge showed no spinner during work and no pulse/idle state when complete; the UI treated Claude as "Not detected" even when running.`
- Symptoms/Impact: Users running `claude` or `cc` in Mergen ADE terminals saw no AI activity badge. There was no visual feedback when Claude was working (spinner), when it needed permission (pulse), or when it completed (idle). AGENTS.md explicitly stated "Claude Code, `cc`, and other AI CLI integrations are not supported."
- Root cause: The codebase lacked Claude-specific runtime integration. `AiCliTool` enum did not include Claude, hook/title detection only recognized Factory Droid and OpenCode patterns, and the UI badge logic had no Claude branch. The `update_from_title` function used only simple substring matching that couldn't handle Claude's OSC title conventions (✳ prefix, braille spinner, ./* prefixes).
- Resolution:
  - Added `Claude` variant to `AiCliTool` enum in `src/hooks.rs`.
  - Implemented Orca-compatible Claude title detection in `src/hooks.rs`:
    - `ClaudeTransportStatus` enum (Working, Idle, Permission) for semantic state differentiation
    - `ClaudeAttentionReason` enum for UI tooltip context
    - `detect_claude_status_from_title()` recognizes Claude Code title patterns:
      - ✳ (U+2733) prefix = idle
      - Braille patterns (U+2800-U+28FF) = working
      - ". " prefix = working
      - "* " prefix = idle
      - "claude" + permission/permission/waiting keywords = permission
      - "claude" + ready/idle/done keywords = idle
      - Bare "claude" = idle
    - `is_claude_agent_title()` for fast pre-filtering
    - `clear_claude_working_indicators()` for stale title cleanup
  - Updated `update_from_title()` in `src/hooks.rs` to prioritize Claude title detection over other tools, allowing tool override when a title clearly indicates Claude.
  - Added Claude state fields to `TerminalEntry` in `src/app.rs`:
    - `claude_normalized_status: Option<ClaudeTransportStatus>`
    - `claude_attention_reason: Option<ClaudeAttentionReason>`
    - Timestamps for evidence-based state resolution
  - Added `ClaudeStatusSource` enum and `apply_claude_status()` function in `src/app.rs` for state machine management.
  - Updated `process_terminal_events()` in `src/app.rs` to handle Claude `AiStatusChange` events.
  - Updated badge rendering:
    - `AiBadgeModel` now includes `claude_normalized_status`
    - `ai_badge_visual()` maps Claude states to visuals: Working=spinner, Idle=solid dot, Permission=pulse
    - `ai_badge_tooltip_lines()` handles Claude attention reasons
    - `draw_ai_badge()` receives Claude normalized status
  - Added launch detection for `claude` and `cc` commands with state clearing to ensure clean transitions from other tools.
  - Added `clear_claude_state()` for exit cleanup.
  - Added comprehensive tests for Claude detection:
    - `detect_claude_status_from_title` tests for all patterns
    - `is_claude_agent_title` tests
    - `update_from_title` tests for Claude idle, working, permission, and tool override scenarios
    - `ai_badge_visuals_match_status` tests for Claude state mapping
- Prevent recurrence:
  - Title-based detection (Orca pattern) should be preferred over hook-based detection for tools that set OSC titles.
  - New AI CLI integrations require: (1) tool enum variant, (2) detection logic, (3) state machine integration, (4) UI badge mapping, (5) cleanup paths, (6) comprehensive tests.
  - When AGENTS.md says a tool is "not supported," either remove the launcher entirely or implement full runtime integration—don't leave a launcher that can't function.
- Files/Commands touched: `src/hooks.rs`, `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`, `cargo build --release`
- References: 2026-04-12 user request for Orca-compatible Claude spinner/pulse logic; Claude title patterns from `orca/src/shared/agent-detection.ts`

#### OpenCode spinner now correctly transitions to pulse when work completes {#opencode-spinner-now-correctly-transitions-to-pulse-when-work-completes}
- Date: 2026-04-12T00:00:00Z
- Context: main/Windows/macOS local OpenCode CLI integration when a turn finishes
- Error signature: `OpenCode spinner kept spinning after the turn was complete; the badge never switched to the pulse state indicating the turn was done.`
- Symptoms/Impact: Users could not visually tell when OpenCode had finished its work because the running spinner would keep spinning indefinitely. The only completion signal was the literal text "turn complete" appearing in the terminal, but this often didn't trigger the state transition due to parsing issues or race conditions.
- Root cause: OpenCode relied on a single fragile completion signal (literal "turn complete" text in visible UI). There was no hook-based or notify-based attention path like Codex and Factory Droid have. The hook detection for OpenCode was broken because `detection_commands` was empty and the parser didn't recognize `[opencode-hook:*]` markers. Process polling could only detect process exit, not completion with the process still alive.
- Resolution:
  - Fixed hook detection in `src/hooks.rs` by modifying `parse_hook_event` to directly detect the tool from the hook prefix rather than relying on `detection_commands`.
  - Added `[opencode-hook:*]` prefix recognition in `src/terminal.rs`'s `complete_official_hook_end_offset` function.
  - Created a new `src/opencode.rs` module with notify/inbox support similar to `src/codex.rs`, providing:
    - `opencode_env_pairs()` for terminal spawn environment variables
    - `write_opencode_notify_event()` for CLI notify mode
    - `read_opencode_notify_inbox()` for polling-based event reading
    - Event kind constants: `OPENCODE_TURN_COMPLETE_EVENT`, `OPENCODE_QUESTION_PROMPT_EVENT`, etc.
  - Added OpenCode runtime directory configuration in `src/config.rs` (`opencode_cli_runtime_dir`).
  - Added `poll_opencode_notify_inboxes()` in `src/app.rs` to periodically check for notify events.
  - Strengthened the visible UI parser in `src/terminal.rs` with multi-marker detection:
    - Primary: "turn complete" literal
    - Secondary: "Build" pattern with timing (e.g., "1.5s") and model info
    - Tertiary: "completed" with follow-up prompt indicators
  - Updated `OpenCodeStatusSource` enum to include `Notify` and `Hook` variants.
  - Updated `apply_opencode_status` to record hook attention timestamps.
  - Updated `resolve_opencode_status` resolver priority: Hook/Notify now has highest priority, then visible UI, then title-based signals.
  - Updated `process_terminal_events` to accept hook events (from_title=false) for OpenCode.
- Prevent recurrence:
  - Always provide multiple independent completion signal paths (hook/notify + visible UI + title) for AI CLI integrations.
  - Test hook event handling explicitly in integration tests.
  - For new AI CLI integrations, follow the Codex/Factory pattern: implement both notify-based and visible-UI-based attention signals.
- Files/Commands touched: `src/app.rs`, `src/hooks.rs`, `src/terminal.rs`, `src/config.rs`, `src/opencode.rs` (new), `src/main.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: 2026-04-12 user-reported OpenCode spinner persistence issue; regression test `opencode_pty_hook_event_is_accepted` (updated from the old `opencode_pty_hook_event_is_ignored`)

#### macOS Command key now works as a primary shortcut modifier alongside Ctrl {#macos-command-key-now-works-as-a-primary-shortcut-modifier-alongside-ctrl}
- Date: 2026-04-11T00:00:00Z
- Context: keyboard shortcut handling on macOS vs Windows/Linux
- Error signature: `On macOS, pressing Cmd+Arrow, Cmd+Alt+Arrow, or Cmd+letter (e.g. Cmd+C, Cmd+G) had no effect. Only the physical Ctrl key was recognized for app navigation, terminal control bytes, and Factory Droid interactive entry.`
- Symptoms/Impact: macOS users had to use the Control key (not the natural Command key) for all Ctrl-equivalent shortcuts: terminal navigation, control-byte generation (like Ctrl+C interrupt), and Factory Droid attention gating. This broke standard macOS keyboard expectations where Cmd is the primary action modifier.
- Root cause: All shortcut matching used bare `modifiers.ctrl` checks without considering `modifiers.command` as an equivalent primary modifier. Only terminal link activation (`Ctrl/Cmd+Click`) already handled both via `modifiers.ctrl || modifiers.command`. Navigation parsing, control-byte generation, and Factory Droid interactive entry all missed the `command` branch.
- Resolution:
  - Added a `primary_shortcut_modifier(modifiers)` helper that returns `modifiers.ctrl || modifiers.command`, consistent with egui's cross-platform `Modifiers::command` convention.
  - Applied the helper to `event_terminal_navigation_shortcut` (Cmd+Arrow, Cmd+Alt+Arrow), `key_to_terminal_bytes` (Cmd+A–Z control bytes), and `event_is_factory_droid_interactive_entry` (Cmd+G).
  - The second guard in `key_to_terminal_bytes` that suppressed all modifier combos now uses `primary_shortcut_modifier || alt` instead of the old `ctrl || alt || command` tri-check, preserving the Cmd-only passthrough for control-byte generation while still rejecting Cmd+Alt (which should not generate a control byte).
  - Extended terminal link activation to use the shared helper for consistency.
  - Added explicit Command-only test coverage: `maps_command_letters_to_control_bytes`, `event_terminal_navigation_shortcut_accepts_command_only_for_navigation`, `event_terminal_navigation_shortcut_recognizes_command_alt_up_down`, `primary_shortcut_modifier_accepts_ctrl_or_command`, `event_is_factory_droid_interactive_entry_accepts_command_only`, `command_arrow_shortcuts_stay_out_of_terminal_stream`, `command_vertical_arrow_shortcuts_stay_out_of_terminal_stream_in_single_view`, `handle_shortcuts_moves_active_terminal_with_command_arrow`, `raw_input_hook_buffers_command_arrow_for_active_terminal_in_multi_view`, `raw_input_hook_buffers_command_horizontal_arrow_for_filter_in_single_view`, `raw_input_hook_buffers_command_vertical_arrow_for_single_view_navigation`.
  - Fixed a pre-existing issue where `is_benign_process_exit_error` import in `terminal.rs` tests was not gated for Windows, preventing test compilation on non-Windows targets.
- Prevent recurrence:
  - When adding new keyboard shortcuts that use Ctrl on Windows/Linux, always use `primary_shortcut_modifier()` instead of bare `modifiers.ctrl` so macOS Command key works automatically.
  - Keep the `terminal_link_activation_modifiers` function as-is since it already delegates to `primary_shortcut_modifier`.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo check --target x86_64-pc-windows-gnullvm`

#### Terminal switches could reopen the transcript at a blank bottom offset instead of the live prompt {#terminal-switches-could-reopen-the-transcript-at-a-blank-bottom-offset-instead-of-the-live-prompt}
- Date: 2026-04-10T00:00:00Z
- Context: main terminal pane scroll behavior while switching the active terminal
- Error signature: `After changing terminals, the viewport could land at the bottom of the rendered transcript with no useful text visible until the user manually scrolled back up.`
- Symptoms/Impact: Terminal changes intermittently reopened a pane in a visually empty lower region, making the current prompt/output appear lost until the user scrolled upward.
- Root cause: The terminal `ScrollArea` persisted per-terminal scroll state and stayed bottom-sticky for any non-empty transcript, but activation did not issue a one-shot realignment to the current prompt/cursor row.
- Resolution:
  - Added an activation-only scroll-alignment flag on terminal entries.
  - On first render after activation, the terminal pane now disables bottom stickiness and applies a vertical offset that targets the stable input cursor row, falling back to the live cursor row or last non-empty line.
  - Once content has been aligned, normal bottom-follow behavior resumes for ongoing terminal output.
- Prevent recurrence:
  - When restoring terminal-scoped scroll state, explicitly decide activation behavior instead of inheriting sticky log-view defaults.
  - Keep pure helper coverage for prompt-target selection and offset clamping so viewport regressions do not depend on ad hoc manual testing.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Crash-resistance hardening now keeps startup and config persistence alive under unwind-safe panic handling {#crash-resistance-hardening-now-keeps-startup-and-config-persistence-alive-under-unwind-safe-panic-handling}
- Date: 2026-04-09T00:00:00Z
- Context: main startup path and config persistence on Windows and non-Windows targets
- Error signature: `A startup icon decode failure or config write failure could become a hard process exit, and release panic handling was configured to abort instead of allowing a crash shield to recover.`
- Symptoms/Impact: A corrupt generated icon, a panic inside the UI lifecycle, or a partially failed config replace could terminate Mergen ADE instead of degrading safely.
- Root cause: Release builds used `panic = "abort"`, startup treated the embedded icon as mandatory, and config persistence relied on non-atomic replace behavior that could delete the last good file before the new one landed.
- Resolution:
  - Switched release panic handling to unwind so a crash shield can catch and degrade instead of aborting immediately.
  - Made app icon decode optional and logged the failure instead of treating it as fatal.
  - Reworked config persistence to write to a unique temp file and replace the target atomically, preserving the old config if replace fails.
- Prevent recurrence:
  - Any startup-only asset or persistence path should fail soft and log, not panic.
  - When changing config save logic, keep the previous file intact until the replacement has succeeded.
- Files/Commands touched: `Cargo.toml`, `src/main.rs`, `src/config.rs`, `KNOWN_ISSUES.md`

#### Settings Layout control now uses the same inline width treatment as the setting above it {#settings-layout-control-now-uses-the-same-inline-width-treatment-as-the-setting-above-it}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal general section
- Error signature: `The Layout section control rendered as a narrow left-aligned checkbox, so it looked visually smaller than the Default shell control above it even though both lived in full-width cards.`
- Symptoms/Impact: The Layout area read as cramped and inconsistent with the rest of Settings, especially next to the wider Default shell row directly above it.
- Root cause: The card shell was already full width, but the Layout setting content used a bare checkbox without the same inline control-width and right-aligned row treatment used by the upper setting.
- Resolution:
  - Reworked the Layout control row so it uses the same inline-width control treatment as Default shell.
  - Added a shared inline control width helper for General settings controls.
- Prevent recurrence:
  - When multiple controls share a card family inside Settings General, keep their inline control widths and row alignment consistent rather than mixing bare widgets with framed rows.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings Codex CLI enable action now stays hidden when Mergen notify wiring is already healthy and shows visible feedback when used {#settings-codex-cli-enable-action-now-stays-hidden-when-mergen-notify-wiring-is-already-healthy-and-shows-visible-feedback-when-used}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal diagnostics section
- Error signature: `Enable Codex CLI integration was clickable even when ~/.codex/config.toml already matched Mergen notify routing, and clicking it only updated an internal status_line that was not visible in the current UI.`
- Symptoms/Impact: Users could click the button and perceive that nothing happened, even though the config patch path was running or already idempotent. The Runtime Overview copy also kept implying that Codex could be enabled from here even when the integration was already healthy.
- Root cause: The Settings diagnostics UI had no explicit Codex integration health inspection. It rendered the enable button unconditionally and routed the result only through a non-rendered status line instead of a visible feedback surface.
- Resolution:
  - Added a Codex config inspection helper that distinguishes healthy Mergen notify wiring, missing setup, preserved custom notify hooks, and unreadable config states.
  - Hid the enable button when the current Codex config is already healthy for the active Mergen executable.
  - Reused the transient toast surface for Codex integration feedback and added inline Runtime Overview status text that reflects the actual Codex state.
- Prevent recurrence:
  - Any settings action that mutates external tool configuration should expose a visible success/failure surface instead of relying only on background status text.
  - Optional setup buttons in Settings should be gated by an explicit health inspection so already-satisfied actions do not remain visible.
- Files/Commands touched: `src/app.rs`, `src/codex.rs`, `KNOWN_ISSUES.md`

#### Settings Technical Details accordion no longer sits flush against the diagnostics scroll boundary when collapsed {#settings-technical-details-accordion-no-longer-sits-flush-against-the-diagnostics-scroll-boundary-when-collapsed}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal diagnostics section
- Error signature: `When Technical Details was collapsed, the accordion card ended immediately at the diagnostics section boundary, so the row could read as if the card was visually cut off.`
- Symptoms/Impact: The collapsed card looked unfinished at the bottom of the scroll area, especially when it was the last visible element in the Diagnostics section.
- Root cause: The diagnostics section ended directly after rendering the Technical Details surface frame, with no trailing spacer to give the collapsed accordion any breathing room against the scroll viewport edge.
- Resolution:
  - Added a dedicated bottom gap after the Technical Details accordion card.
  - Kept the accordion/header behavior unchanged and fixed only the section spacing.
- Prevent recurrence:
  - When a framed accordion is the last element inside a settings scroll section, leave explicit trailing space so the card chrome does not read as clipped by the viewport.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings Technical Details section now expands from a triangle header instead of a separate button row {#settings-technical-details-section-now-expands-from-a-triangle-header-instead-of-a-separate-button-row}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal diagnostics section
- Error signature: `The Technical Details block used a separate Expanded/Collapsed label and Show details/Hide details buttons, so it did not behave like the rest of the accordion-style settings UI.`
- Symptoms/Impact: Diagnostics looked inconsistent with Saved Messages, the card spent vertical space on toggle chrome instead of content, and the open/close affordance felt more like a form control than a section accordion.
- Root cause: Technical Details reused the generic settings card shell and layered button-driven expanded state inside the body, rather than treating the card header itself as the collapsible control.
- Resolution:
  - Replaced the button row with a header-level accordion for the Technical Details card.
  - Reused the shared settings disclosure painter so Diagnostics and Saved Messages use the same triangle behavior.
  - Moved the old description/help text into the accordion body so the collapsed card is just the clickable header.
- Prevent recurrence:
  - When a Settings section is primarily about revealing more detail, prefer a header-driven accordion over embedding extra Show/Hide buttons in the body.
  - Keep disclosure triangle behavior shared across Settings accordions even when the row layouts differ.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message draft input now reads as an actual field again without leaving the theme palette {#settings-saved-message-draft-input-now-reads-as-an-actual-field-again-without-leaving-the-theme-palette}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `After the Saved Messages draft input was recolored to match the theme black, it blended too far into the surrounding card and no longer read clearly as an editable field.`
- Symptoms/Impact: The add-message row looked flat, users could miss that the left area was an input, and the section lost a basic affordance cue even though the color family was technically correct.
- Root cause: The local text-edit fill and border were too close to the surrounding Saved Messages section surface, so the field no longer had enough separation to read as a control.
- Resolution:
  - Shifted the draft input fill to the darker shared theme black (`SURFACE_BG`) instead of the softer section surface.
  - Strengthened the idle and focused border tones with neutral greys so the field stays obvious without introducing a blue accent.
  - Updated the local text-edit chrome tests to assert the new field contrast values.
- Prevent recurrence:
  - When aligning a text field to the theme, preserve a small but explicit fill/stroke separation from the containing card.
  - Treat "theme-consistent" and "discoverable as input" as two separate constraints and test both at the helper level.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message draft input now overrides egui's actual text-edit background and focus stroke {#settings-saved-message-draft-input-now-overrides-eguis-actual-text-edit-background-and-focus-stroke}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `The Saved Messages add-message input still rendered with a near-pure black background even after the local widget fill override was updated.`
- Symptoms/Impact: The draft field continued to look darker than the surrounding Settings theme surface, and its focused outline could still appear harsher than intended.
- Root cause: egui's `TextEdit` frame uses `visuals.extreme_bg_color` for the background and `visuals.selection.stroke` for the focused border, so overriding only `widgets.*.bg_fill` and `bg_stroke` did not affect the actual focused text-edit chrome.
- Resolution:
  - Extended the local Saved Messages text-edit scope to override `visuals.extreme_bg_color` with the section-matched surface fill.
  - Bound the local focused outline to the same muted theme stroke via `visuals.selection.stroke`.
  - Kept the existing widget-state overrides so hover/open states stay visually consistent.
- Prevent recurrence:
  - When skinning egui text inputs, verify whether the widget reads from `widgets.*` or `visuals.extreme_bg_color` before assuming the frame fill changed.
  - Treat focused text-edit borders separately from generic widget borders because egui routes them through `selection.stroke`.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message draft input no longer falls back to the bright focused text-edit chrome {#settings-saved-message-draft-input-no-longer-falls-back-to-the-bright-focused-text-edit-chrome}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `The Saved Messages add-message input still switched to a harsh focused background/border once it received focus, despite the section-specific theme override.`
- Symptoms/Impact: The draft input looked visually detached from the surrounding Saved Messages surface, especially while focused, because it picked up a darker default fill and a brighter border than the rest of the section.
- Root cause: The local Settings text-edit chrome helper only overrode the inactive, hovered, and active widget visuals; the focused/open text-edit state still used egui's default `widgets.open` styling.
- Resolution:
  - Extended the local Settings text-edit chrome model with a dedicated focus stroke.
  - Applied the Saved Messages surface fill and muted border to the `widgets.open` state as well, so focus no longer changes the input background family.
  - Updated the regression test to assert the focused chrome stays on the same theme.
- Prevent recurrence:
  - When locally skinning an egui `TextEdit`, override `inactive`, `hovered`, `active`, and `open` together.
  - Keep focus-state styling in the same helper struct so the focused border cannot drift separately from the base field chrome.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message disclosure triangle now sits lower inside the accordion row {#settings-saved-message-disclosure-triangle-now-sits-lower-inside-the-accordion-row}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `The Saved Messages accordion triangle still looked too high even after the folder icon and title were centered, so the disclosure glyph felt visually detached from the row.`
- Symptoms/Impact: The header text and folder icon read correctly, but the triangle remained slightly high relative to the visual center of the card, which made the accordion look off-balance.
- Root cause: egui's built-in `show_header(...)` disclosure button uses its own compact icon slot and does not expose a vertical offset for the triangle inside the taller custom header row.
- Resolution:
  - Replaced the built-in Saved Messages `show_header(...)` toggle with a custom disclosure row that still uses the same persistent `CollapsingState`.
  - Drew the triangle manually with a dedicated downward offset while preserving the existing open/close animation and body indentation.
  - Added regression coverage for the header layout and disclosure icon vertical offset.
- Prevent recurrence:
  - When a Settings accordion needs icon-level alignment control, avoid the default `show_header(...)` button and render the disclosure slot explicitly.
  - Keep the disclosure offset in a named constant so future header tweaks do not reintroduce the high triangle.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved messages accordions now sit slightly lower under the section intro {#settings-saved-messages-accordions-now-sit-slightly-lower-under-the-section-intro}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `The Saved Messages accordion list sat too close to the section intro copy, so the first project card started higher than the intended breathing room.`
- Symptoms/Impact: The section opened feeling a bit cramped at the top, and the first accordion card visually collided with the introductory sentence.
- Root cause: The vertical gap between the intro text and the first Saved Messages accordion used a smaller fixed spacer than the rest of the section now needs.
- Resolution:
  - Increased the top spacer between the intro copy and the accordion list.
  - Promoted that spacing to a named Settings constant and added a regression test.
- Prevent recurrence:
  - Keep section-level vertical rhythm in named spacing constants instead of scattered literals.
  - Add a small test when a Settings spacing tweak is meant to be kept stable.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message project headers now keep the accordion caret, folder icon, and title aligned {#settings-saved-message-project-headers-now-keep-the-accordion-caret-folder-icon-and-title-aligned}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `Saved Messages project accordions showed a visually off-center folder icon/title row, so the caret, folder glyph, and header copy did not read as one aligned control.`
- Symptoms/Impact: The accordion header looked slightly crooked, especially next to the built-in disclosure triangle, and longer project names risked making the row feel uneven.
- Root cause: The project header rendered the folder icon and project title as one combined text label, which let the icon glyph inherit text baseline behavior instead of sitting in its own centered slot.
- Resolution:
  - Split the header into dedicated icon, title, and right-count lanes inside the existing collapsing header row.
  - Centered the folder icon in its own control-height slot and added truncate handling for long project names.
  - Added layout regression tests for normal and narrow header widths.
- Prevent recurrence:
  - Keep accordion header icons in explicit slots instead of embedding them into a shared text string.
  - Back Saved Messages header geometry with tests whenever row composition changes.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message draft input now uses the section theme surface {#settings-saved-message-draft-input-now-uses-the-section-theme-surface}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages draft input
- Error signature: `The add-new-message input field used the generic app text-edit fill instead of the Saved Messages section surface tone.`
- Symptoms/Impact: The draft input looked slightly off-theme compared with the surrounding Saved Messages card and section backgrounds, so the add-message row felt visually detached.
- Root cause: The saved-message draft field used the global `TextEdit` visuals, whose inactive fill is brighter than the Saved Messages section surface, and there was no local style override for that input.
- Resolution:
  - Added a local Settings text-edit chrome helper for the saved-message draft field.
  - Rebound the input background to the shared `SURFACE_BG_SOFT` surface color and kept a subtle border for field definition.
  - Added a regression test for the local chrome helper.
- Prevent recurrence:
  - When a Settings input is meant to visually merge with its section card, prefer a local text-edit chrome override instead of changing the global text-edit palette.
  - Keep theme-surface styling in a helper so future Settings inputs can reuse the same treatment.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved message actions no longer sit inside the text box {#settings-saved-message-actions-no-longer-sit-inside-the-text-box}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `Saved message action icons were rendered inside the message box and could end up below the text instead of clearly separated on the right.`
- Symptoms/Impact: The trash action felt visually attached to the text body, long messages pushed the action row downward, and the control target was harder to scan because it did not stay in a stable right-hand action lane.
- Root cause: Each saved message card rendered both the text and action icons inside the same framed surface, so the action layout participated in the text card's internal flow.
- Resolution:
  - Split each saved message row into a left text frame and a separate right action column.
  - Kept the message box dedicated to text only and reserved explicit width for the external action lane.
  - Replaced the old narrow-card action-stack test with a text-width reservation test.
- Prevent recurrence:
  - Keep per-row actions in a dedicated action lane when the row body contains multi-line text.
  - Back row geometry with helper tests so text width always reserves space for right-side controls.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved messages now render as a single-column list without overflow-prone wrapping {#settings-saved-messages-now-render-as-a-single-column-list-without-overflow-prone-wrapping}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `Saved messages were laid out side by side, which made the section harder to scan and increased the chance of cramped cards or horizontal overflow pressure.`
- Symptoms/Impact: Multiple saved messages could appear next to each other in wrapped rows instead of a clean top-to-bottom list, and long messages had less predictable room because each card width was clamped separately.
- Root cause: The Settings section rendered saved messages with `horizontal_wrapped()` and a per-card max-width clamp, so cards flowed into multiple columns whenever there was enough width.
- Resolution:
  - Replaced the wrapped horizontal layout with a vertical single-column list.
  - Switched saved-message cards to use the full available content width so long entries wrap inside one card instead of competing for row space.
  - Updated the width regression test to cover the full-width card behavior.
- Prevent recurrence:
  - Keep saved-message entries in a single-column reading layout unless there is a dedicated design change for dense grid presentation.
  - Tie saved-message card width directly to the available content width when overflow safety matters.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings saved-messages copy no longer mixed in prompt terminology {#settings-saved-messages-copy-no-longer-mixed-in-prompt-terminology}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal saved messages section
- Error signature: `The Settings section still mixed "Prompts" and "prompt" wording even though the feature is presented as Saved Messages elsewhere.`
- Symptoms/Impact: The navigation label, section description, helper copy, input hint text, and chip tooltips used inconsistent terminology, which made the Saved Messages feature feel split across two names.
- Root cause: Several user-facing strings in the Settings saved messages section were left on older prompt-based copy even after the feature model and section title standardized on saved messages.
- Resolution:
  - Renamed the Settings navigation label from `Prompts` to `Saved Messages`.
  - Updated Settings helper text, counters, empty states, hints, and chip action tooltips to use saved-messages wording consistently.
  - Updated the related navigation label regression test.
- Prevent recurrence:
  - Keep all user-facing copy for the saved messages feature aligned to one canonical name.
  - When renaming a feature surface, audit navigation labels, helper text, empty states, and action tooltips together.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings navigation chrome was simplified to a text-only list {#settings-navigation-chrome-was-simplified-to-a-text-only-list}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal left navigation
- Error signature: `The left Settings menu still looked like a boxed control list instead of a minimal stacked set of section labels.`
- Symptoms/Impact: Even after color cleanup, the Settings navigation still felt heavier than the rest of the modal because it kept a framed panel, row chrome, and icon-driven emphasis.
- Root cause: The left navigation continued to render each section with custom chrome helpers and the panel itself still used the shared settings surface frame, so the menu read as a separate bordered widget instead of lightweight navigation text.
- Resolution:
  - Removed the left navigation panel frame and row chrome.
  - Dropped the section icons from the left menu and rendered the navigation as stacked text labels only.
  - Kept selection and hover feedback text-only via subtle color and type-size emphasis.
- Prevent recurrence:
  - Keep auxiliary navigation surfaces visually lighter than the main content cards unless there is a strong interaction reason not to.
  - Prefer text-only state cues before reintroducing panel chrome or per-row containers in Settings navigation.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings navigation icon and label alignment was tightened {#settings-navigation-icon-and-label-alignment-was-tightened}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal left navigation
- Error signature: `Settings navigation labels sat too close to their icons and the rows relied on ad-hoc coordinates instead of a shared alignment layout.`
- Symptoms/Impact: The left Settings menu looked cramped, the gap between icon and text was inconsistent to the eye, and future spacing tweaks would have required changing magic numbers in the paint path.
- Root cause: `draw_settings_navigation()` painted the icon from a hard-coded center point and then started the label from an offset of that center, so the visible gap depended on icon glyph width rather than a fixed aligned slot.
- Resolution:
  - Replaced the ad-hoc positioning with a dedicated Settings navigation row layout helper.
  - Introduced explicit leading inset, icon slot width, and icon-to-label gap constants so rows share one alignment system.
  - Added a regression test covering the computed icon and text positions.
- Prevent recurrence:
  - Keep Settings navigation spacing driven by named layout constants instead of inline offsets.
  - Add or update pure layout tests whenever Settings row geometry changes.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Settings navigation selected state no longer used a blue accent {#settings-navigation-selected-state-no-longer-used-a-blue-accent}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal section navigation
- Error signature: `The selected Settings section painted a blue highlight that clashed with the rest of the dark terminal-first theme.`
- Symptoms/Impact: Switching between General, Prompts, and Diagnostics made the active row stand out with a saturated blue tint that looked inconsistent beside the neutral terminal and panel chrome.
- Root cause: `draw_settings_navigation()` used the shared `BTN_ICON_ACTIVE` action color for the selected row fill and stroke even though the rest of the Settings modal and terminal surfaces used neutral grays for active state emphasis.
- Resolution:
  - Replaced the selected Settings navigation chrome with a neutral active treatment that matches the existing terminal manager row styling.
  - Added regression coverage for both selected and hover Settings navigation chrome states.
- Prevent recurrence:
  - Keep Settings navigation selection styles aligned with neutral theme surface constants instead of reusing action/accent colors meant for buttons.
  - When adjusting Settings chrome, cover active and hover states with focused unit tests so theme regressions are caught early.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`

#### Terminal default background now follows the app surface theme {#terminal-default-background-now-follows-the-app-surface-theme}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local terminal output surface and default cell background rendering
- Error signature: `New terminal panes kept a flatter pure-black background that did not match the rest of the theme surface.`
- Symptoms/Impact: Empty prompt space, resize fill, and default-background terminal cells looked visually detached from the surrounding pane chrome even though the app already used a slightly lighter dark surface tone elsewhere.
- Root cause: The terminal viewport fill and near-black background normalization path were both pinned to a standalone pure-black constant instead of the shared theme surface color, so default terminal background rendering diverged from the app theme.
- Resolution:
  - Rebound the terminal output background constant to the shared `SURFACE_BG` theme color.
  - Kept ANSI or explicitly colored terminal cell backgrounds unchanged by preserving the existing passthrough behavior for non-default backgrounds.
  - Updated the regression test to assert that normalized default terminal backgrounds resolve to the same themed surface color as the viewport.
- Prevent recurrence:
  - Keep terminal default-background mapping tied to shared theme surface constants instead of duplicating raw RGB values.
  - When adjusting terminal render colors, verify both the viewport fill and per-cell default-background normalization path together.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`

#### Terminal header source control badge and Terminal Manager row chrome were simplified {#terminal-header-source-control-badge-and-terminal-manager-row-chrome-were-simplified}
- Date: 2026-04-08T00:00:00Z
- Context: main/Windows local terminal header chrome + Terminal Manager project and terminal rows
- Error signature: `Terminal headers still showed a source control icon, Terminal Manager repeated +/- diff totals on every terminal row, and terminal rows started with extra terminal icons before the text.`
- Symptoms/Impact: Source control status was duplicated across surfaces, Terminal Manager rows looked visually noisy, and per-terminal diff totals obscured the project-level git state the user actually wanted to scan.
- Root cause: The main terminal header still rendered the old source-control badge path, Terminal Manager diff summaries were attached to each terminal title instead of the project group label, and terminal rows still painted a static terminal icon ahead of the text even after the AI badge became the primary status marker.
- Resolution:
  - Removed the source control badge from the terminal header and left only the AI badge/status path there.
  - Moved the Terminal Manager `+/-` diff summary to the project group header so git totals are shown once per project.
  - Removed the leading terminal icon from Terminal Manager terminal rows so rows now begin directly with AI state and text.
- Prevent recurrence:
  - Keep project-level source control summaries attached to project headers instead of duplicating them on every child terminal row.
  - Avoid parallel status indicators in terminal chrome unless they provide distinct information.
  - When simplifying row chrome, update both layout code and regression tests together so removed icons do not leave dead spacing.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`

#### Terminal Manager filter actions, single-view Ctrl navigation, and Settings diagnostics layout were corrected {#terminal-manager-filter-actions-single-view-ctrl-navigation-and-settings-diagnostics-layout-were-corrected}
- Date: 2026-04-08T00:00:00Z
- Context: main/Windows local Terminal Manager project actions, single-terminal keyboard navigation, and Settings modal layout
- Error signature: `Terminal Manager always showed separate foreground/background buttons, Ctrl+Up/Down did nothing while only one terminal was visible, and diagnostics could expand the Settings modal until Saved Messages became cramped.`
- Symptoms/Impact: The Terminal Manager action row did not reflect the selected foreground/background filter, single-view users could not cycle terminals with the expected `Ctrl+Up/Down` shortcut, and the diagnostics block consumed most of the Settings window height when expanded.
- Root cause: The project header action row was hard-wired for two inline spawn buttons instead of being driven by the selected filter, plain `Ctrl+Up/Down` stayed on the grid-navigation path even when only one terminal was visible, and diagnostics were rendered as an unbounded linear block in the same flow as the rest of the Settings content.
- Resolution:
  - Terminal Manager project headers now render one inline spawn button that matches the active foreground/background filter.
  - Plain `Ctrl+Up/Down` now routes through single-view linear terminal navigation when `Show multiple terminals at once` is off, while the existing multi-terminal grid navigation remains intact.
  - Settings diagnostics now default to collapsed and render inside a dedicated capped scroll area so Saved Messages keeps usable space.
- Prevent recurrence:
  - Keep filter-driven actions bound to the same state that determines which terminal rows are visible.
  - Handle shortcut reinterpretation at the contextual routing layer so raw input buffering, terminal capture, and final navigation stay aligned.
  - When a settings subsection can grow significantly, give it its own bounded scroll container before adding more controls beneath it.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`

#### Source Control refresh no longer replaced preserved branch/diff data with placeholders {#source-control-refresh-no-longer-replaced-preserved-branch-diff-data-with-placeholders}
- Date: 2026-04-08T00:00:00Z
- Context: main/Windows local source-control sidebar + Terminal Manager diff summary during refresh/error states
- Error signature: `Refreshing source control replaced the last visible branch/diff snapshot with '-' or placeholder UI even before new data arrived.`
- Symptoms/Impact: While `Refresh Status` or `Fetch + Refresh` was running, terminal chrome and related source-control surfaces could temporarily lose the last known branch/diff/file state and show placeholder text instead. Refresh errors could also wipe the previous successful snapshot from the UI.
- Root cause: Source-control worker results were written back as full snapshot replacements, and tooltip/diff-summary rendering treated `loading` or `last_error` as mutually exclusive with existing snapshot data instead of as status overlays on top of preserved data.
- Resolution: Source-control refresh now preserves the last successful branch/diff/file snapshot during loading and refresh failures, merges error results onto existing data, and keeps tooltip/sidebar rendering focused on showing loading/error banners above preserved branch/file context instead of dropping to placeholders when totals already exist.
- Prevent recurrence:
  - Treat source-control `loading` and `last_error` as freshness metadata layered on top of snapshot data, not as reasons to discard the last successful snapshot.
  - When a refresh can fail independently from the last known state, merge status fields into cached data instead of replacing the entire model.
  - Keep regression tests around preserved diff totals and tooltip contents for loading/error states.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`

#### Factory Droid badge behavior fixed: detection vs hook events {#factory-droid-badge-behavior-fixed}
- Date: 2026-04-06T00:00:00Z
- Context: main/Windows terminal title bar AI status badge
- Error signature: `Badge turned green when typing 'droid' instead of waiting for UserPromptSubmit`
- Symptoms/Impact: Badge would glow green immediately when user typed `droid` command, but user wanted badge to only turn green when `UserPromptSubmit` hook event arrived (new prompt submitted).
- Root cause: `detect_tool()` was setting `status = Running` immediately upon `droid` detection, which was an over-correction from a previous fix that had made the badge invisible.
- Resolution:
  - `detect_tool()` now only sets `tool = FactoryDroid`, status stays `Inactive`
  - Badge only turns green (Running) when `UserPromptSubmit` hook event is received
  - Badge turns yellow (Attention) when `Stop` hook event is received
  - `parse_hook_event()` supports Droid CLI formats: `[droid-hook:event=X]`, `[factory-droid-hook:event=X]`, standalone word-boundary names
  - Added `request_repaint_after(100ms)` to `draw_ai_badge()` for animation continuity
- Prevent recurrence:
  - Detection should only identify the tool, not trigger UI state
  - Hook events should drive all UI state changes
- Files/Commands touched: `src/hooks.rs`, `src/app.rs`, `cargo test`, `cargo build --release`

#### Multiline paste in opencode CLI submitted blank lines as live Enter keys {#multiline-paste-in-opencode-cli-submitted-blank-lines-as-live-enter-keys}
- Date: 2026-04-01T00:00:00Z
- Context: main/Windows local terminal paste path with opencode CLI/readline-style TUIs
- Error signature: `Pasting text with blank lines into opencode CLI caused the terminal to submit early instead of treating the paste as one block.`
- Symptoms/Impact: Multiline clipboard content, especially with empty lines, was delivered as raw input bytes, so apps that expected bracketed paste interpreted embedded newlines as immediate Enter presses and broke the pasted command or prompt state.
- Root cause: `src/app.rs` deferred paste payload construction until the I/O thread, so bracketed-paste state could change between user action and write; the earlier raw-byte route also bypassed the terminal model's tracked bracketed-paste state and newline canonicalization logic from `tattoy-wezterm-term`.
- Resolution: Local workspace fix snapshots paste bytes at request time in `src/terminal.rs` before queuing the runtime command, and `src/app.rs` now flushes pending typed bytes before queueing paste to preserve input ordering and keep later terminal mode changes from altering the payload.
- Prevent recurrence:
  - Keep paste delivery on a dedicated runtime path instead of merging it into generic keyboard byte streams.
  - Cover both bracketed and non-bracketed paste behavior with regression tests at the terminal runtime layer.
  - When a TUI paste bug mentions blank lines or premature submit, verify whether DECSET 2004 state is being honored before changing newline normalization.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test --target-dir target_test_paste`

#### Plain cargo release builds now refresh the same MSVC EXE as cargo run {#plain-cargo-release-builds-now-refresh-the-same-msvc-exe-as-cargo-run}
- Date: 2026-03-30T00:00:00Z
- Context: main/Windows local cargo workflows
- Error signature: `cargo run` showed current behavior, but contributors could still be pointed at a different executable path when the repo default target and release target disagreed.
- Symptoms/Impact: The MSVC release EXE path was not guaranteed to reflect the same build configuration as the plain local cargo flow, so developers had to remember which target triple produced the binary they were testing.
- Root cause: The repository default target was still set to `x86_64-pc-windows-gnullvm`, which split the common local build path from the portable MSVC EXE path.
- Resolution: Switched the repo default build target to `x86_64-pc-windows-msvc` so `cargo build --release` and `cargo run --release` both refresh the same MSVC output path, while keeping `x86_64-pc-windows-gnullvm` available only as an explicit alternative target.
- Prevent recurrence:
  - Keep the repo default target aligned with the executable path contributors are expected to run.
  - Treat gnullvm as an explicit opt-in build target, not the default local path.
  - Update build docs and regression tests whenever the default target changes.
- Files/Commands touched: `.cargo\config.toml`, `AGENTS.md`, `README.md`, `scripts\__tests__\build-release.tests.ps1`, `cargo build --release`

#### Empty project terminal-group clicks did not reopen the project body {#empty-project-terminal-group-clicks-did-not-reopen-the-project-body}
- Date: 2026-03-30T00:00:00Z
- Context: main/Windows local terminal manager project-group headers
- Error signature: Clicking `New Foreground Terminal` or `New Background Terminal` on an empty project spawned the terminal but left the project group collapsed.
- Symptoms/Impact: The terminal existed but stayed hidden until the user manually expanded the project section, making the button feel unresponsive.
- Root cause: The render path decided whether to open the collapsing header only after mutating terminal state, so it lost the true pre-click empty-state signal.
- Resolution: Open the project group after any successful inline foreground or background spawn, so the newly created terminal is visible whether the project was empty or already had terminals.
- Prevent recurrence:
  - Treat inline spawn success as the visibility signal for the project group.
  - Keep the auto-open decision localized to the inline spawn path.
  - Add unit tests that cover successful and failed inline spawn behavior.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`

#### Source Control panel and terminal chrome could show stale git status until manual refresh {#source-control-panel-and-terminal-chrome-could-show-stale-git-status-until-manual-refresh}
- Date: 2026-03-11T00:00:00Z
- Context: main/Windows local source-control sidebar + terminal chrome status UX
- Error signature: `Source Control`, terminal headers, and Terminal Manager rows only refreshed git state on first open or explicit button clicks.
- Symptoms/Impact: Changed files, clean/dirty state, and branch indicators could remain stale across projects until the user manually pressed refresh, and there was no lightweight shared status signal in terminal chrome.
- Root cause: Each source-control refresh spawned an ad hoc thread from the UI path, there was no central scheduler for background status updates, and terminal surfaces did not consume shared project-level git snapshots.
- Resolution: Replaced per-refresh thread spawning with one shared source-control worker plus priority round-robin background scheduling, kept manual refresh/fetch buttons, and reused the same per-project snapshot cache for Source Control, terminal headers, and Terminal Manager git badges with lazy hover details.
- Prevent recurrence:
  - Keep source-control refresh orchestration centralized instead of spawning UI-driven one-off worker threads.
  - Reuse project-level git snapshots across all surfaces that visualize repository state.
  - Keep automatic background refresh limited to `git status`; leave `git fetch` manual unless a deliberate product change requires otherwise.
  - Verify selected project priority and tooltip truncation with unit tests whenever source-control UI is changed.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`

#### Portable release flow switched to single EXE MSVC output {#portable-release-flow-switched-to-single-exe-msvc-output}
- Date: 2026-03-09T00:00:00Z
- Context: main/Windows release packaging refresh
- Error signature: Previous release path produced extra EXEs that were not portable across Windows machines.
- Symptoms/Impact: Copying the wrong EXE could fail on another PC or leave users running stale legacy artifacts.
- Root cause: The repository still carried legacy Windows release paths instead of one canonical portable output.
- Resolution: Windows release flow now targets only `target\\x86_64-pc-windows-msvc\\release\\mergen-ade.exe`. Plain local `cargo` development remains on the repo's `gnullvm` host flow, including direct toolchain `cargo.exe` launches that bypass the rustup shim, while the release script uses an explicit MSVC toolchain for the portable artifact and removes stale legacy EXEs during release generation.
- Prevent recurrence:
  - Use `powershell -ExecutionPolicy Bypass -File .\\scripts\\build-release.ps1` for release builds.
  - Keep plain local `cargo` on the repo `gnullvm` flow with the repo-local linker stanza intact, and use `scripts\\build-release.ps1` for the MSVC portable release.
  - Keep the Rust MSVC toolchain installed and make sure Visual Studio Build Tools plus the Windows SDK are present for release builds.
  - Keep CI packaging aligned with the MSVC portable artifact only.
  - Do not distribute or reintroduce alternate Windows EXE output paths.
- Files/Commands touched: `.cargo\\config.toml`, `Cargo.toml`, `rust-toolchain.toml`, `scripts\\build-release.ps1`, `.github\\workflows\\release.yml`, `README.md`

#### Duplicate collapse arrows created noisy left chrome {#duplicate-collapse-arrows-created-noisy-left-chrome}
- Date: 2026-03-06T09:00:00Z
- Context: main/Windows local UI shell refresh
- Error signature: Both collapsible left panels could show their own narrow collapsed strip with separate arrow controls.
- Symptoms/Impact: When `Project Explorer` and `Terminal Manager` were both collapsed, the left edge showed multiple tiny arrow targets and felt visually noisy and outdated.
- Root cause: Each panel owned its own collapse affordance instead of sharing one navigation surface.
- Resolution: Replaced per-panel arrow strips with a single left activity rail that toggles both panels and keeps the shell closer to a modern editor layout.
- Prevent recurrence:
  - Prefer one shared navigation/toggle surface for adjacent collapsible panels.
  - Avoid duplicating narrow collapsed placeholders for sibling panes.
  - Review collapsed-state screenshots before accepting UI shell changes.
- Files/Commands touched: `src/app.rs`, `src/models.rs`, `cargo check`

#### Release binary wrong output location (root vs target/release) {#release-binary-wrong-output-location-root-vs-target-release}
- Date: 2026-03-04T14:20:34Z
- Context: main/Windows local PowerShell/cargo 1.93.1
- Error signature: Expected updated binary under `target/release/mergen-ade.exe`, but an extra root-level `mergen-ade.exe` was produced.
- Symptoms/Impact: Contributors can run a stale or unintended executable from repo root and think the latest fix is missing.
- Root cause: Release artifact handling copied the binary to repository root instead of treating `target/release` as the single source of truth.
- Resolution: Build flow was corrected to update `target/release/mergen-ade.exe` only and remove the root copy (`mergen-ade.exe`) in local workspace (commit pending).
- Prevent recurrence:
  - Always verify artifact path with `Get-Item target\\release\\mergen-ade.exe` after `cargo build --release`.
  - Do not copy release artifacts to repository root.
  - Add/keep CI checks and release notes explicitly referencing `target/release` output path.
- Files/Commands touched: `target/release/mergen-ade.exe`, `mergen-ade.exe` (removed), `cargo build --release`, `cmd /c del /f /q mergen-ade.exe`
- References: commit pending in local workspace; recent baseline commits `3eee74b`, `559605d`

#### Terminal geçmişi kaydırılamıyordu {#terminal-gecmisi-kaydirilamiyordu}
- Date: 2026-03-06T16:09:54Z
- Context: main/Windows local/cargo 1.93.1, rustc 1.93.1
- Error signature: `ScrollArea görünüyordu ama TerminalSnapshot yalnızca görünür satırları topladığı için scrollback geçmişi render edilmiyordu.`
- Symptoms/Impact: Terminal panelinde fare tekeri ve scrollbar görünse bile eski çıktı satırlarına çıkılamıyor, uzun komut geçmişi kaybolmuş gibi davranıyordu.
- Root cause: Terminal snapshot üretimi fiziksel viewport ile sınırlıydı ve scrollback satırları ile imleç ofseti render modeline hiç taşınmıyordu.
- Resolution: Scrollback satırlarını ve imleç ofsetini snapshot'a dahil eden düzeltme `2e332c7` commit'i ile eklendi.
- Prevent recurrence:
  - Terminal snapshot testlerinde scrollback ve cursor ofset senaryolarını zorunlu tut.
  - UI'da scrollbar görmek ile gerçekte geçmiş satırların render edildiğini ayrı ayrı doğrula.
  - Render modelinde viewport-relative ve absolute row indekslerini karıştırma.
- Files/Commands touched: `src/terminal.rs`, `cargo fmt`, `cargo test`
- References: commit `2e332c7` - https://github.com/furkancak1r/mergen-ade/commit/2e332c73898bb54b972ae9b9f3774409da1f0927

#### Terminal selection copied the row above the highlight {#terminal-selection-copied-the-row-above-the-highlight}
- Date: 2026-03-11T13:35:12Z
- Context: main/Windows local/egui 0.29.1, cargo 1.93.1
- Error signature: `Selected status rows were highlighted correctly, but clipboard content came back as "Merhaba. Nasıl yardımcı olayım?" from the row above.`
- Symptoms/Impact: Terminal users could drag-select one visual row and get a different row in the clipboard, making copy unreliable even when spaces and highlight looked correct.
- Root cause: Selection hit-testing and highlight placement used manual `line_height` row math instead of the real `egui::Galley` row geometry, so visual rows and copied rows diverged vertically.
- Resolution: Local workspace fix after baseline commit `d8e16b6` switched terminal selection hit-testing/highlighting to `Galley` row rects and kept cached selection snapshots aligned with copy output; validated with `cargo test` (134 passed).
- Prevent recurrence:
  - Base terminal row hit-testing on `Galley.rows[*].rect` or equivalent rendered row geometry, not estimated line spacing.
  - Keep regression tests that assert pointer-to-row mapping for empty rows, multi-line galleys, and full-width selections.
  - Re-check screenshot-backed copy bugs by comparing highlighted rows with actual clipboard output before closing the issue.
- Files/Commands touched: `src/app.rs`, `cargo fmt`, `cargo test`, `view_image`
- References: commit pending in local workspace after `d8e16b6`


#### Terminal selection copied the row above the visual highlight {#terminal-selection-copied-the-row-above-the-visual-highlight}
- Date: 2026-03-11T13:36:00Z
- Context: main/Windows local PowerShell/mergen-ade 0.1.0, eframe 0.29
- Error signature: `Seçili alt durum satırları kopyalanırken panoya "Merhaba. Nasıl yardımcı olayım?" gidiyordu.`
- Symptoms/Impact: Kullanıcı terminalde alttaki satırları mavi highlight ile seçse bile panoya bir üst satır kopyalanıyordu; görsel seçim ile gerçek copy sonucu ayrışıyordu.
- Root cause: Terminal seçim hit-test'i ve highlight'ı sentetik `line_height * row` hesabıyla yapılıyor, `egui` metni gerçek `Galley.rows[*].rect` geometrisiyle çizdiği için satır eşlemesi kayıyordu.
- Resolution: Dikey seçim eşlemesi `Galley` row geometrisine taşındı ve regression testleri eklendi; düzeltme yerel çalışma alanında HEAD `d8e16b6` üstünde commit bekliyor.
- Prevent recurrence:
  - Pointer-to-row eşlemesini manuel satır yüksekliğiyle değil gerçek `Galley` row rect'leriyle yap.
  - Görsel highlight ile panoya giden metni aynı geometri kaynağına bağlayan regression testlerini zorunlu tut.
  - Ekran görüntüsüyle doğrulanan seçim/kopya sapmalarını issue log'una kaydetmeden kapatılmış sayma.
- Files/Commands touched: `src/app.rs`, `cargo fmt`, `cargo test`
- References: HEAD `d8e16b6` (`Terminal sağ kenarındaki ölü alanı kaldır`), local workspace fix commit pending

#### Full-screen TUI left a right-edge gray strip {#full-screen-tui-left-a-right-edge-gray-strip}
- Date: 2026-03-11T14:12:24Z
- Context: main/Windows local/eframe 0.29, cargo test (146 passed)
- Error signature: `opencode` full-screen view filled vertically, but a gray/black strip remained on the right edge inside the terminal pane.
- Symptoms/Impact: Full-screen TUI content appeared narrower than the available pane, leaving unused right-side columns and making the terminal look partially undersized.
- Root cause: Horizontal terminal sizing used an overstated single-glyph width estimate, which underreported `cols` to the PTY and stopped TUI rendering before the pane's right edge.
- Resolution: Local workspace fix after HEAD `50d162a` changed horizontal cell measurement in `src/app.rs` to a multi-cell no-wrap galley average, kept pane-width forcing in place, and validated with `cargo test` (146 passed); commit pending.
- Prevent recurrence:
  - Measure terminal column width from averaged multi-cell layout output instead of a single glyph width.
  - Keep regression tests that prove narrower valid horizontal metrics increase reported `cols`.
  - When a right-edge strip remains, compare screenshot pixel colors against `TERMINAL_OUTPUT_BG` and `SURFACE_BG` before changing pane layout.
- Files/Commands touched: `src/app.rs`, `cargo fmt`, `cargo test`, `view_image`, `git log -1`
- References: HEAD `50d162a` (`Terminal seçim ve kopyalama hizasını düzelt, bilinen sorun kaydını ekle`), local workspace fix commit pending

#### Ctrl+C required a second press to interrupt {#ctrl-c-required-a-second-press-to-interrupt}
- Date: 2026-03-12T00:00:00Z
- Context: main/Windows local/egui terminal input routing
- Error signature: `Terminalde Ctrl+C ilk basista interrupt gondermiyor, ancak ikinci basista etkili oluyordu.`
- Symptoms/Impact: Aktif terminalde calisan komutlar standart terminal beklentisinin aksine tek `Ctrl+C` ile durmuyor, kullanici interrupt icin ayni kisayola ikinci kez basmak zorunda kaliyordu.
- Root cause: `src/app.rs` icindeki `Event::Copy` isleyicisi ve `pending_ctrl_c` durumu, secim yokken bile ilk `Ctrl+C` basisini sadece armed-interrupt durumuna cevirip gercek `0x03` gonderimini ikinci basisa birakiyordu.
- Resolution: Yerel calisma alanindaki duzeltme, cift-basis `pending_ctrl_c` akisini kaldirdi; artik secim varsa `Ctrl+C` secimi kopyaliyor, secim yoksa ilk basista dogrudan `0x03` gonderiyor. Ilgili testler yeni davranisa gore guncellendi.
- Prevent recurrence:
  - Terminal kisayollarinda secim-kopya davranisi ile interrupt davranisini ayri testlerle kilitle.
  - Kullaniciya gosterilen status mesajlarini gercek giris semantigiyle birebir uyumlu tut; "again" tipi akislar icin zaman pencereli state ekleniyorsa ayrica regression test yaz.
  - Terminal copy yolu secim uretemediginde olayi yutma; guvenli varsayilan olarak interrupt yolunu acik birak.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace change on 2026-03-12; commit pending

#### Ctrl+C required a second press to interrupt {#ctrl-c-required-a-second-press-to-interrupt-2}
- Date: 2026-03-12T05:41:44Z
- Context: main/Windows local/cargo 1.93.1, rustc unavailable on PATH
- Error signature: `Ctrl+C` did not interrupt on the first press; a second press was required to send `0x03`.
- Symptoms/Impact: Running terminal commands did not stop with a single `Ctrl+C`, which broke standard shell interrupt expectations and delayed command cancellation.
- Root cause: The terminal input path in `src/app.rs` consumed the first `Ctrl+C` into a timed `pending_ctrl_c` armed state instead of forwarding the control byte immediately when no selection existed.
- Resolution: Local workspace fix removed the double-press interrupt flow so `Ctrl+C` now copies only when there is an active selection and otherwise sends `0x03` on the first press; validated with `cargo test`, commit pending after `6ad2a25`.
- Prevent recurrence:
  - Keep resolver-level tests that lock copy-vs-interrupt behavior for both selected and unselected terminal states.
  - Do not add time-windowed terminal shortcut state that swallows standard shell control bytes without an explicit product requirement.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `git log -1`
- References: commit `6ad2a25` baseline (`Source control otomatik yenilemeyi ve terminal git rozetlerini ekle`); local workspace fix commit pending

#### Terminal child processes could survive app shutdown {#terminal-child-processes-could-survive-app-shutdown}
- Date: 2026-03-12T09:10:00Z
- Context: main/Windows local/portable-pty 0.9, windows-sys 0.59
- Error signature: `Closing or force-killing mergen-ade.exe could leave terminal child processes running in the background.`
- Symptoms/Impact: Long-running commands started from integrated terminals could outlive the app window, leaving shells or child tools consuming resources after the UI was gone.
- Root cause: Terminal cleanup relied on best-effort terminate calls during normal exit and had no crash-resilient OS-level process containment boundary.
- Resolution: Local workspace fix moved terminal children into per-runtime Windows Job Objects with `KILL_ON_JOB_CLOSE`, added bounded graceful shutdown, and kept process-tree termination as a fallback; validated with `cargo fmt` and `cargo test`, commit pending after `58e0593`.
- Prevent recurrence:
  - Treat terminal spawn as failed if crash-safe process containment cannot be established.
  - Keep shutdown tests that assert writer disconnect and no-op job fallback behavior.
  - Re-check crash and forced-exit behavior with a real long-running child process before release.
- Files/Commands touched: `src/terminal.rs`, `Cargo.toml`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: commit `58e0593` (`Düzelt terminal kopyalama bildirimini ve sağ tık yapıştırma davranışını`), local workspace fix commit pending
#### Windows job containment regressed terminal startup and exit cleanup {#windows-job-containment-regressed-terminal-startup-and-exit-cleanup}
- Date: 2026-03-12T10:20:00Z
- Context: main/Windows local/portable-pty 0.9, windows-sys 0.59
- Error signature: `AssignProcessToJobObject` denied terminal startup in inherited job sessions, and `WaitForSingleObject` on a stale borrowed child handle could surface false cleanup errors.
- Symptoms/Impact: Integrated terminals could fail to open under debuggers or launchers that already placed the app inside a job, and closing an already-exited terminal could incorrectly report cleanup failure.
- Root cause: The first containment pass made job attachment a hard spawn requirement and reused a raw child handle after ownership had moved to the waiter thread.
- Resolution: Follow-up local workspace fix made job containment best-effort with warning-only fallback, duplicated the child process handle for owned wait checks, and preserved process-tree cleanup when no job handle is available; validated with `cargo fmt` and `cargo test`, commit pending after the local containment change.
- Prevent recurrence:
  - Never make crash-hardening setup a terminal spawn blocker unless the product explicitly prefers failed startup over degraded cleanup.
  - When a background waiter owns the original child handle, duplicate any handle needed for later shutdown or liveness checks.
  - Add regression tests for inherited-job startup fallback and already-exited terminal cleanup paths.
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: review on local workspace after commit `58e0593`; follow-up fix commit pending

#### Inherited CARGO_TARGET_DIR left the repo MSVC release EXE stale {#inherited-cargo-target-dir-left-the-repo-msvc-release-exe-stale}
- Date: 2026-03-12T12:30:00Z
- Context: main/Windows local/PowerShell with `CARGO_TARGET_DIR=C:\zt`, build-release.ps1
- Error signature: `powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1` reported success, but `target\x86_64-pc-windows-msvc\release\mergen-ade.exe` still contained older runtime strings such as `Press Ctrl+C again to interrupt`.
- Symptoms/Impact: `cargo run` showed the latest behavior while the repo-path MSVC release EXE behaved like an older build, so manual launches and release packaging could pick up a stale binary.
- Root cause: The release script inherited `CARGO_TARGET_DIR`, so Cargo cleaned and built under the overridden target directory while script validation still read the repo-local `target\...` EXE path.
- Resolution: Local workspace fix pins `CARGO_TARGET_DIR` to the repo-local `target` directory inside `scripts/build-release.ps1`, keeps clean-before-build and hash validation on that path, and adds regression coverage in the PowerShell script tests.
- Prevent recurrence:
  - Release scripts that promise a concrete output path must set `CARGO_TARGET_DIR` explicitly instead of inheriting ambient shell overrides.
  - Validate a rebuilt EXE by checking for current runtime strings or a changed hash at the exact advertised output path.
  - Keep a regression test that asserts target-dir pinning happens before `cargo clean` and `cargo build`.
- Files/Commands touched: `scripts/build-release.ps1`, `scripts/__tests__/build-release.tests.ps1`, `KNOWN_ISSUES.md`, `powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1`
- References: local workspace diagnosis on 2026-03-12; commit pending

#### Repo-path MSVC release EXE lagged behind cargo run {#repo-path-msvc-release-exe-lagged-behind-cargo-run}
- Date: 2026-03-12T12:45:00Z
- Context: main/Windows local PowerShell/`CARGO_TARGET_DIR=C:\zt`, `cargo.cmd`, `build-release.ps1`
- Error signature: `Overriding inherited CARGO_TARGET_DIR for portable release build: C:\zt -> C:\Users\...\Mergen-ADE\target`
- Symptoms/Impact: `cargo run` showed current terminal and source-control behavior, but `target\x86_64-pc-windows-msvc\release\mergen-ade.exe` still launched an older build until the release pipeline was corrected.
- Root cause: Ambient `CARGO_TARGET_DIR` redirected MSVC clean/build outputs away from the repo tree, so the repo-path EXE the user launched remained stale even when release builds reported success.
- Resolution: Local workspace fix pinned `CARGO_TARGET_DIR` to the repo-local `target` directory in `scripts/build-release.ps1`, reran the PowerShell regression tests, and rebuilt the repo-path MSVC EXE with SHA-256 `E223287474106525A7035FF71A40F21E02C26371A31E37990963EB9C9265B677`; commit pending after `58e0593`.
- Prevent recurrence:
  - Emit a clear log line whenever the release script overrides an inherited target directory.
  - Verify the exact advertised EXE path after release builds by checking current runtime strings or a fresh hash.
  - Keep script tests that lock repo-local target pinning before `cargo clean` and `cargo build`.
- Files/Commands touched: `KNOWN_ISSUES.md`, `scripts/build-release.ps1`, `scripts/__tests__/build-release.tests.ps1`, `powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\build-release.tests.ps1`, `powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1`
- References: commit `58e0593` (`Düzelt terminal kopyalama bildirimini ve sağ tık yapıştırma davranışını`); local workspace release-script follow-up fix pending

#### macOS release packaging would have shipped a broken app experience {#macos-release-packaging-would-have-shipped-a-broken-app-experience}
- Date: 2026-03-12T13:30:00Z
- Context: main/local cross-platform release workflow review
- Error signature: `A future macOS DMG could build, but the app would still try to spawn Windows shells and open Explorer.`
- Symptoms/Impact: A published macOS asset would have launched into a partially unusable app: default terminal startup could fail because `powershell.exe`/`cmd.exe` do not exist on macOS, and file reveal actions would fail because `explorer.exe` is Windows-only.
- Root cause: The repo was Windows-first in both CI and runtime assumptions. `ShellKind` only modeled Windows shells, and `open_in_file_explorer` hard-coded `explorer.exe` without platform branching.
- Resolution: Local workspace fix added platform-aware shell defaults and shell normalization, switched macOS to `zsh`, made file reveal/open commands platform-specific, and reworked GitHub Releases into artifact-based Windows-plus-best-effort-macOS packaging with an unsigned ARM64 DMG path.
- Prevent recurrence:
  - Do not publish a new platform artifact unless the app's default runtime path is valid on that platform.
  - Keep pure command-construction tests for platform-specific shell and explorer/open behavior.
  - Keep optional release jobs artifact-based so experimental platform packaging can fail without blocking the primary release asset.
- Files/Commands touched: `src/models.rs`, `src/config.rs`, `src/app.rs`, `.github/workflows/release.yml`, `scripts/package-macos-release.sh`, `README.md`, `KNOWN_ISSUES.md`
- References: local workspace change on 2026-03-12; commit pending

#### macOS DMG release path skipped before packaging started {#macos-dmg-release-path-skipped-before-packaging-started}
- Date: 2026-03-12T14:10:00Z
- Context: main/local GitHub Actions release run `22999299197`, macos-15-arm64 runner
- Error signature: `error: target triple in channel name 'stable-x86_64-pc-windows-gnullvm'`
- Symptoms/Impact: The tagged `v0.1.3` release published only the Windows ZIP. The macOS job completed early, skipped `Package unsigned DMG`, and never uploaded a DMG artifact.
- Root cause: `rust-toolchain.toml` pinned the repo to the Windows-specific channel name `stable-x86_64-pc-windows-gnullvm`. On the macOS runner, both `dtolnay/rust-toolchain@stable` and `cargo build --target aarch64-apple-darwin` still consulted that repo override and failed before the DMG packaging script could run.
- Resolution: Local workspace fix switches the repo toolchain channel to host-agnostic `stable`, makes the macOS build invoke `cargo +stable build --target aarch64-apple-darwin`, and changes the release workflow so official tagged releases now require both the Windows ZIP and macOS DMG to succeed before publishing.
- Prevent recurrence:
  - Keep repo-level Rust toolchain names host-agnostic when CI must run on multiple operating systems.
  - Explicitly invoke `cargo +stable` or another host-valid toolchain in cross-platform workflow steps when the repo keeps platform-specific target defaults elsewhere.
  - Do not allow official release publish jobs to proceed after a skipped macOS packaging path if the release promise includes a DMG artifact.
- Files/Commands touched: `rust-toolchain.toml`, `.github/workflows/release.yml`, `README.md`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `gh run view 22999299197 --job 66779525438 --log`
- References: GitHub Actions run `22999299197` for tag `v0.1.3`; local fix commit pending

#### macOS DMG release path restored for official tags {#macos-dmg-release-path-restored-for-official-tags}
- Date: 2026-03-12T12:01:42Z
- Context: main/GitHub Actions release run `23000428561` on `macos-15-arm64` and `windows-latest`/Rust stable 1.94.0
- Error signature: `Package unsigned DMG`
- Symptoms/Impact: After the fix, the `v0.1.4` release produced both `mergen-ade-v0.1.4-macos-arm64.dmg` and `mergen-ade-v0.1.4-windows-x64-portable.zip` instead of silently publishing a Windows-only release.
- Root cause: The prior Windows-specific repo toolchain override was removed and the macOS workflow now builds with a host-valid stable toolchain before packaging the `.app` into a DMG.
- Resolution: Fixed by commit `2cc883d` (`macOS release toolchain kilidini kaldır ve DMG yayınını zorunlu yap`), validated by successful GitHub release run `23000428561` and published tag `v0.1.4`.
- Prevent recurrence:
  - Keep official release workflows fail-fast when a promised platform artifact cannot be produced.
  - Re-check release asset lists after each tagged run to confirm both DMG and ZIP uploads.
  - Avoid repo-level Rust channel names that encode a single host triple unless every CI runner matches that host.
- Files/Commands touched: `rust-toolchain.toml`, `.github/workflows/release.yml`, `README.md`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `gh run watch 23000428561 --exit-status`, `gh release view v0.1.4 --json assets,url,name`
- References: commit `2cc883d`; release `https://github.com/furkancak1r/mergen-ade/releases/tag/v0.1.4`; run `https://github.com/furkancak1r/mergen-ade/actions/runs/23000428561`

#### macOS notarized release flow replaced the damaged DMG experience {#macos-notarized-release-flow-replaced-the-damaged-dmg-experience}
- Date: 2026-03-12T13:06:02Z
- Context: main/local release workflow hardening for GitHub Actions macOS runner and Apple Developer notarization
- Error signature: `"<app>" is damaged and can't be opened. You should move it to the Trash.`
- Symptoms/Impact: The published macOS DMG could download successfully but still be blocked by Gatekeeper on a clean Mac, making the official release effectively unusable for normal end users.
- Root cause: The release pipeline packaged an unsigned, unstapled macOS app and DMG, so Gatekeeper treated the downloaded artifact as untrusted and potentially tampered with.
- Resolution: Local workspace fix updates the macOS release flow to import a Developer ID Application certificate from GitHub secrets, sign the `.app`, notarize the DMG with `notarytool` via App Store Connect API key, staple the results, and fail the release if any Apple verification step fails.
- Prevent recurrence:
  - Never publish an official macOS DMG without successful `codesign`, `notarytool`, `stapler`, and `spctl` verification in CI.
  - Keep Apple signing material only in GitHub Actions secrets; do not commit or echo certificate or API key contents.
  - Upload notarization diagnostics on failure so rejected submissions can be debugged before the next tag.
- Files/Commands touched: `.github/workflows/release.yml`, `scripts/package-macos-release.sh`, `README.md`, `AGENTS.md`, `KNOWN_ISSUES.md`
- References: release `https://github.com/furkancak1r/mergen-ade/releases/tag/v0.1.4`; run `https://github.com/furkancak1r/mergen-ade/actions/runs/23000428561`

#### Pre-notarization spctl check rejected the signed app bundle {#pre-notarization-spctl-check-rejected-the-signed-app-bundle}
- Date: 2026-03-12T14:48:05Z
- Context: main/local macOS release rerun after PKCS#12 import fix, GitHub Actions run `23005915477`
- Error signature: `Mergen ADE.app: rejected` / `source=Unnotarized Developer ID`
- Symptoms/Impact: After PKCS#12 import was fixed, the macOS job still failed before notarization, so `v0.1.5` could not publish a DMG even though signing credentials were valid.
- Root cause: `scripts/package-macos-release.sh` ran `spctl` against the signed `.app` before `notarytool` submission, but Gatekeeper assessment at that point correctly sees an unnotarized Developer ID app and rejects it.
- Resolution: Local workspace fix removes the pre-notarization `spctl` app check, keeps `codesign --verify` before notarization, and leaves the final Gatekeeper-style `spctl --type open` validation on the stapled DMG after notarization.
- Prevent recurrence:
  - Use `codesign --verify` for pre-notarization signature checks and reserve `spctl` for post-notarization validation.
  - Keep the final Gatekeeper assessment on the distribution artifact that users download, not on a still-unnotarized intermediate app bundle.
  - Treat each failed release rerun as a new diagnostic data point and append the exact Apple rejection string for future regressions.
- Files/Commands touched: `scripts/package-macos-release.sh`, `README.md`, `KNOWN_ISSUES.md`, `gh run view 23005915477 --log-failed`
- References: run `https://github.com/furkancak1r/mergen-ade/actions/runs/23005915477`; failed macOS job in attempt 3 for tag `v0.1.5`

#### Headless spctl DMG assessment blocked a notarized release in CI {#headless-spctl-dmg-assessment-blocked-a-notarized-release-in-ci}
- Date: 2026-03-13T05:12:34Z
- Context: main/local release fix after `v0.1.6` GitHub Actions run `23008045783` on `macos-15-arm64`
- Error signature: `mergen-ade-v0.1.6-macos-arm64.dmg: rejected` / `source=Insufficient Context`
- Symptoms/Impact: The macOS release job completed signing, notarization, stapling, and `stapler validate`, but still failed at the last CI gate, so the notarized DMG never uploaded and `v0.1.6` was not published.
- Root cause: `spctl -a -vv --type open` on a GitHub-hosted headless runner required runtime context that the CI environment did not provide, so it returned a false-negative even after Apple notarization had already been accepted.
- Resolution: Local workspace fix removes the blocking headless `spctl --type open` DMG gate from CI, keeps `notarytool` acceptance and `stapler validate` as release blockers, and documents the runner-context limitation.
- Prevent recurrence:
  - Do not make headless `spctl --type open` a blocking publish gate when notarization and stapler validation have already passed.
  - Treat `notarytool Accepted` plus `stapler validate` as the canonical CI release signal for DMG trust.
  - Reserve end-user Gatekeeper behavior checks for manual download testing on a real macOS desktop context.
- Files/Commands touched: `scripts/package-macos-release.sh`, `README.md`, `KNOWN_ISSUES.md`, `gh run view 23008045783 --job 66844593832 --log-failed`, `gh run download 23008045783 -n macos-notarization-diagnostics`
- References: run `https://github.com/furkancak1r/mergen-ade/actions/runs/23008045783`; failed tag `v0.1.6`; notary diagnostics artifact `macos-notarization-diagnostics`

#### cargo build --release did not refresh the repo-path MSVC EXE {#cargo-build-release-did-not-refresh-the-repo-path-msvc-exe}
- Date: 2026-03-18T00:00:00Z
- Context: main/Windows local PowerShell, default cargo target selection
- Error signature: `cargo run` reflected the latest code, but `target\x86_64-pc-windows-msvc\release\mergen-ade.exe` stayed stale after plain `cargo build --release`.
- Symptoms/Impact: Contributors expected `cargo build --release` to refresh the MSVC EXE and launched an older binary from the repo target path.
- Root cause: `/.cargo/config.toml` defaulted to `x86_64-pc-windows-gnullvm`, so plain release builds updated the gnullvm output while the MSVC path only changed with an explicit `--target x86_64-pc-windows-msvc` build or release script.
- Resolution: Switched the repo default build target to `x86_64-pc-windows-msvc`, updated build documentation to match, and kept gnullvm available as an explicit optional target.
- Prevent recurrence:
  - Keep the default target and documented default output path aligned.
  - When troubleshooting stale binaries, confirm which target triple the last build used.
  - Reserve gnullvm builds for explicit `--target x86_64-pc-windows-gnullvm` invocations.
- Files/Commands touched: `.cargo\config.toml`, `AGENTS.md`, `README.md`, `KNOWN_ISSUES.md`, `cargo build --release`
- References: local workspace change on 2026-03-18; commit pending

#### Expand/Collapse All action drifted from real folder open state {#expand-collapse-all-action-drifted-from-real-folder-open-state}
- Date: 2026-03-18T00:00:00Z
- Context: main/Windows local directory tree toolbar behavior
- Error signature: Toolbar action text could show `Collapse All Folders` after folders were manually collapsed, and clicking it had no visible effect.
- Symptoms/Impact: The remediation control felt misleading because button intent followed prior toolbar clicks instead of the current folder tree state.
- Root cause: `src/app.rs` derived the next action from cached per-project toggle intent (`directory_toggle_next_collapses_by_project`) rather than reading actual `CollapsingState` values from the tree.
- Resolution: Removed cached toggle-intent state, derived action label/intent from live folder header open state, and kept pending apply behavior for explicit bulk operations.
- Prevent recurrence:
  - Derive bulk tree actions from current UI state, not from last-click memory.
  - Keep toolbar labels/action text and executable behavior tied to the same source of truth.
  - Re-check manual folder toggles before accepting tree toolbar changes.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo check`
- References: local workspace fix on 2026-03-18; commit pending

#### Default MSVC local target broke contributor builds without Visual Studio toolchain {#default-msvc-local-target-broke-contributor-builds-without-visual-studio-toolchain}
- Date: 2026-03-18T00:00:00Z
- Context: main/Windows local contributor onboarding and plain cargo workflows
- Error signature: `cargo build --release` / `cargo run --release` failed before linking on machines that only had the repo-local LLVM-MinGW setup.
- Symptoms/Impact: Contributors who previously relied on the repo-local gnullvm linker could no longer run default local builds unless MSVC Build Tools and Windows SDK were preconfigured in shell environment.
- Root cause: `/.cargo/config.toml` default target was switched from `x86_64-pc-windows-gnullvm` to `x86_64-pc-windows-msvc`, making default local cargo flows depend on MSVC prerequisites.
- Resolution: Restored default target to `x86_64-pc-windows-gnullvm`, kept MSVC as explicit release target, and re-aligned docs/tests with the gnullvm default local flow.
- Prevent recurrence:
  - Keep plain local `cargo` defaults aligned with the lowest-friction contributor toolchain.
  - Treat MSVC release output as explicit (`--target x86_64-pc-windows-msvc`) or script-driven (`scripts/build-release.ps1`).
  - Update release tests and docs in the same change whenever default target behavior changes.
- Files/Commands touched: `.cargo\config.toml`, `scripts\__tests__\build-release.tests.ps1`, `AGENTS.md`, `README.md`, `KNOWN_ISSUES.md`, `cargo check`, `powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\build-release.tests.ps1`
- References: local workspace fix on 2026-03-18; commit pending

#### Directory tree toolbar and row truncation introduced hot-path repaint overhead {#directory-tree-toolbar-and-row-truncation-introduced-hot-path-repaint-overhead}
- Date: 2026-03-18T00:00:00Z
- Context: main/Windows local project explorer performance under continuous repaint
- Error signature: Explorer toolbar state check traversed entire directory trees each frame, and row truncation repeatedly re-laid out text per visible entry.
- Symptoms/Impact: Large repositories showed noticeable explorer stalls and degraded scrolling responsiveness while terminal activity and loading animations kept the pane repainting.
- Root cause: `src/app.rs` computed bulk action state with a full `directory_tree_has_collapsed_folders` traversal on every repaint, and truncation logic performed multiple galley layouts per row (full-width check + binary search passes).
- Resolution: Added per-project collapsed-state caching with explicit invalidation on index updates and manual folder toggles, and simplified directory row rendering to a single `TextWrapMode::Truncate` galley layout per row.
- Prevent recurrence:
  - Avoid O(total_directories) scans in per-frame UI paths; use cache + targeted invalidation.
  - Keep explorer row rendering to one text layout pass per row where possible.
  - Treat directory tree repaint-heavy views as performance-sensitive in code review.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo check`
- References: local workspace fix on 2026-03-18; commit pending

#### Directory tree folder labels drifted away from the disclosure triangle {#directory-tree-folder-labels-drifted-away-from-the-disclosure-triangle}
- Date: 2026-03-24T00:00:00Z
- Context: main/Windows local project explorer directory tree rows
- Error signature: Folder rows rendered with a visible gap after the disclosure triangle and the folder label appeared centered within the remaining row width.
- Symptoms/Impact: The project explorer hierarchy became harder to scan because the folder name looked detached from its expand/collapse affordance.
- Root cause: `src/app.rs` positioned directory row text with the parent `Ui` layout alignment, so `CollapsingHeader` header layout influenced folder-label placement instead of keeping it anchored to the left edge of the row content area.
- Resolution: Added a shared left-anchored directory row text-position helper, applied it to both folder and file rows, added regression tests for full-width folder rows and left-aligned text placement, and released the fix in `v0.1.8`.
- Prevent recurrence:
  - Keep directory tree row text placement independent of parent `Ui` alignment.
  - Share folder/file row alignment logic so fixes land in one place.
  - Add regression coverage whenever `CollapsingHeader`-backed row layout changes.
- Files/Commands touched: `src/app.rs`, `Cargo.toml`, `KNOWN_ISSUES.md`, `cargo test`
- References: local workspace fix on 2026-03-24; release `https://github.com/furkancak1r/mergen-ade/releases/tag/v0.1.8`; commit pending

#### Droid interactive spinner glyphs rendered as static boxes in integrated terminal {#droid-interactive-spinner-glyphs-rendered-as-static-boxes-in-integrated-terminal}
- Date: 2026-03-25T00:00:00Z
- Context: main/Windows local `droid` interactive mode inside Mergen-ADE
- Error signature: Fresh `droid` sessions showed a static square/box where the normal animated dots/spinner should appear, and the terminal looked like it was constantly refreshing without visible animation.
- Symptoms/Impact: Droid interactive mode looked visually broken even in new sessions, making progress indicators unreadable and exaggerating repaint churn.
- Root cause: `src/app.rs` rendered terminal content with the generic egui monospace family backed only by bundled default fonts, so Droid's braille-style spinner frames lacked glyph coverage and collapsed into the same fallback box each frame.
- Resolution: Added a dedicated terminal font family, prioritized Windows terminal fallbacks (`Cascadia Mono`, `Consolas`, `Segoe UI Symbol`) ahead of the bundled egui monospace fonts, switched terminal measurement/rendering to that family, and added regression coverage for terminal font ordering and icon-font exclusion.
- Prevent recurrence:
  - Keep terminal font fallback configuration separate from the app UI monospace family.
  - Measure terminal cell width and row height with the exact font family used for terminal painting.
  - Re-check fresh-session TUI glyphs such as braille spinners before blaming repaint scheduling.
  - Treat classic `powershell.exe` command parsing issues such as `&&` failures as a separate shell-compatibility follow-up, not as evidence that spinner animation bytes are missing.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace fix on 2026-03-25; Droid local logs in `%USERPROFILE%\.factory\logs\droid-log-single.log`

#### Droid braille spinner glyphs collapsed into tofu boxes {#droid-braille-spinner-glyphs-collapsed-into-tofu-boxes}
- Date: 2026-03-25T13:15:00Z
- Context: main/Windows local/`droid` 0.85.0/`eframe` 0.29.1/`cargo test` (175 passed)
- Error signature: `Fresh droid sessions showed a static square/box where the animated spinner dots should appear.`
- Symptoms/Impact: Droid interactive mode looked like it was constantly refreshing without visible animation, so progress indicators were unreadable even in new sessions.
- Root cause: The integrated terminal used the bundled egui monospace font stack without a terminal-specific Windows fallback chain, so Droid's braille spinner frames rendered as the same missing-glyph box.
- Resolution: Local workspace fix after `392d377` added a dedicated terminal font family, loaded Windows fallbacks (`Cascadia Mono`, `Consolas`, `Segoe UI Symbol`), switched terminal measurement/rendering to that family, and validated the change with `cargo test`.
- Prevent recurrence:
  - Keep a terminal-only font family instead of sharing the generic app monospace family.
  - Measure terminal width and line height from the same font family used to paint terminal content.
  - Re-check fresh-session TUI glyph coverage before attributing animation failures to repaint timing.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: commit `392d377` (`Fix directory tree row alignment`); local workspace fix pending

#### Terminal symbol fallback misaligned the Windows grid {#terminal-symbol-fallback-misaligned-the-windows-grid}
- Date: 2026-03-25T14:00:00Z
- Context: main/Windows local terminal font fallback follow-up after the Droid glyph fix
- Error signature: Terminal box-drawing and symbol-heavy output could render with cursor/selection drift even though the pane still measured columns from a fixed-width font.
- Symptoms/Impact: Windows terminal panes could show misaligned cursor overlays, incorrect selection rectangles, and shifted grid columns when output resolved through the newly added symbol fallback.
- Root cause: The dedicated terminal family inserted `Segoe UI Symbol` into the primary Windows fallback chain, but that font is proportional for several glyphs while terminal measurement, hit-testing, and cursor placement still assume fixed-width cells.
- Resolution: Removed `Segoe UI Symbol` from the Windows terminal fallback candidates, kept the dedicated terminal family on fixed-width fonts only, updated the Windows candidate-order regression test, and revalidated with `cargo test`.
- Prevent recurrence:
  - Do not add proportional fonts to terminal rendering fallback chains.
  - Keep terminal measurement and terminal paint paths locked to the same fixed-width family.
  - Re-check cursor and selection alignment whenever terminal font fallback coverage changes.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace fix on 2026-03-25; follow-up to the Droid glyph fallback change, commit pending

#### Terminal links ignored Ctrl+Click activation in integrated panes {#terminal-links-ignored-ctrl-click-activation-in-integrated-panes}
- Date: 2026-03-25T15:30:00Z
- Context: main/Windows local integrated terminal link activation
- Error signature: Visible terminal URLs and OSC8 hyperlinks stayed inert when clicked inside the pane.
- Symptoms/Impact: Operators could not open links directly from terminal output, and plain left-click kept starting or clearing selection instead of activating the target.
- Root cause: `src/app.rs` treated every primary click in terminal output as selection/focus input, while `src/terminal.rs` snapshot cells discarded hyperlink metadata and the app had no wrapped-line URL hit-testing for plain `http/https` text.
- Resolution: Carried hyperlink URIs into terminal cell snapshots, added wrapped logical-line URL resolution for plain `http/https` links, gated link activation behind `Ctrl+Click`/primary-command click so selection behavior stays intact, and added regression coverage for modifier detection plus explicit and wrapped-link resolution.
- Prevent recurrence:
  - Keep terminal pointer hit-testing separate from selection drag behavior when new interactive terminal affordances are added.
  - Preserve cell-level terminal metadata that the UI may need later instead of collapsing it during snapshot generation.
  - Re-test soft-wrapped terminal output whenever click-target resolution depends on logical line reconstruction.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace fix on 2026-03-25; commit pending

#### Terminal Ctrl+Click links accepted unsafe schemes and rejected mixed-case HTTP(S) {#terminal-ctrl-click-links-accepted-unsafe-schemes-and-rejected-mixed-case-https}
- Date: 2026-03-25T16:00:00Z
- Context: main/Windows local integrated terminal hyperlink follow-up
- Error signature: `Ctrl+Click could open explicit OSC8 links with non-web schemes, while plain-text URLs such as HTTPS://example.com stayed inert.`
- Symptoms/Impact: Untrusted terminal output could hand `file:`, `mailto:`, or custom-scheme targets to the OS/browser opener, and valid mixed-case HTTP(S) links failed to open even though they looked clickable.
- Root cause: The explicit hyperlink path in `src/app.rs` forwarded cell metadata directly to `open_url` without the `http/https` allowlist used for plain text, and that plain-text allowlist compared schemes case-sensitively.
- Resolution: Follow-up local workspace fix applies one shared ASCII-case-insensitive `http/https` allowlist to both explicit OSC8 hyperlinks and plain-text URL matches, with regression coverage for rejected non-web schemes plus accepted mixed-case HTTP(S).
- Prevent recurrence:
  - Route every terminal link source through the same URI allowlist before calling the platform opener.
  - Treat URI schemes as case-insensitive when validating terminal links.
  - Keep regression tests for explicit OSC8 metadata and plain-text wrapped links in the same suite.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace follow-up fix on 2026-03-25; commit pending

#### Terminal Ctrl+Click link gesture could leave a stale deferred selection {#terminal-ctrl-click-link-gesture-could-leave-a-stale-deferred-selection}
- Date: 2026-03-25T17:00:00Z
- Context: main/Windows local integrated terminal hyperlink follow-up after Ctrl+Click activation shipped
- Error signature: `Pressing Ctrl/Cmd after mouse-down on a terminal link could open the URL but leave terminal output visually stuck until another click.`
- Symptoms/Impact: Link activation worked, but some clicks left a hidden collapsed selection behind, so terminal snapshot refresh stayed deferred and the pane appeared frozen even though the PTY was still running.
- Root cause: `src/app.rs` reused the text-selection state machine for link clicks, created collapsed selection state on primary press, and only cleared it in the normal click/drag-stop path; when the gesture switched into link activation before release, that cleanup path was skipped.
- Resolution: Added dedicated pending link-click state for terminal presses, converted only real drags into text selection anchored at the original press point, required the same resolved URL on press/release for link open, and added regression tests covering modifier-toggle open, drag fallback, preserved existing selections, and mismatched release targets.
- Prevent recurrence:
  - Treat interactive terminal link gestures as their own transient state instead of piggybacking on collapsed text selection.
  - Clear pending link state on primary release even when the click does not open a link.
  - Add helper-level tests for pointer-state transitions whenever terminal click handling mixes selection and activation behaviors.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace follow-up fix on 2026-03-25; commit pending

#### Factory Droid title badge stayed dark because Factory hooks were unregistered and title-only signals were ignored before tool detection {#factory-droid-title-badge-stayed-dark-because-factory-hooks-were-unregistered-and-title-only-signals-were-ignored-before-tool-detection}
- Date: 2026-04-06T00:00:00Z
- Context: main/Windows local Factory Droid sessions inside the integrated terminal
- Error signature: `Factory Droid green/yellow badge did not react even though the user wanted running = green pulse and waiting/completed = yellow pulse until terminal focus acknowledgement.`
- Symptoms/Impact: The terminal header indicator stayed inactive because Mergen-ADE only reacted after prior tool detection, while the local Factory setup had no registered hook entries and an old unsupported `~/.claude/hooks` experiment was writing only console-title changes.
- Root cause: `~/.factory/settings.json` had no `hooks` registrations, the legacy `~/.claude/hooks/on-working.ps1` / `on-stop.ps1` files were unsupported for Factory, and `src/hooks.rs` rejected the first title-based `[Working...]` / `[Idle]` transition unless `session.tool` had already been set by an official hook marker.
- Resolution: Mergen-ADE now seeds `FactoryDroid` status directly from official title patterns, keeps partial hook markers buffered until the closing bracket arrives, adds a repo-owned Factory hook script plus idempotent installer, and installs user-wide Factory `UserPromptSubmit` / `Notification` / `Stop` hooks that emit official `factory-droid-hook:*` markers and `[Working...]` / `[Idle]` title updates.
- Prevent recurrence:
  - Keep Factory hook registration in `~/.factory/settings.json`; do not rely on unsupported `~/.claude/hooks/*` files.
  - Avoid writing `UserPromptSubmit` markers through ordinary hook stdout paths that would pollute Droid prompt context.
  - Keep title-based detection able to seed tool state when official markers are missing or delayed.
  - Require a closing `]` before parsing buffered hook markers from chunked PTY output.
- Files/Commands touched: `src/hooks.rs`, `scripts/factory-droid-status-hook.ps1`, `scripts/install-factory-droid-hooks.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `cargo test`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-factory-droid-hooks.ps1`
- References: Factory docs reviewed on 2026-04-06 (`https://docs.factory.ai/reference/hooks-reference`, `https://docs.factory.ai/cli/configuration/hooks-guide`, `https://docs.factory.ai/guides/hooks/notifications`); local Factory log evidence in `%USERPROFILE%\.factory\logs\droid-log-single.log`

#### AI attention badge cleared on terminal switches instead of only on the selected terminal {#ai-attention-badge-cleared-on-terminal-switches-instead-of-only-on-the-selected-terminal}
- Date: 2026-04-06T00:00:00Z
- Context: main/Windows local terminal header and terminal-manager selection flow
- Error signature: Selecting a different terminal cleared the previous terminal's yellow AI attention state, and same-terminal clicks/focus changes did not reliably acknowledge attention.
- Symptoms/Impact: The badge could disappear as soon as the user changed focus to another terminal, so attention state no longer meant "this terminal still needs a click/focus acknowledgment."
- Root cause: `src/app.rs` treated `set_active_terminal()` as a global attention reset and only cleared status for the previously active terminal, instead of acknowledging the terminal that was actually clicked or selected.
- Resolution: Reworked `src/app.rs` so `set_active_terminal()` acknowledges attention on the target terminal only, preserves other terminals' yellow state when switching away, and keeps copy/paste/typing paths clearing attention through the existing interaction flow. Added regression tests for same-terminal acknowledgement and for leaving another terminal's yellow state intact.
- Prevent recurrence:
  - Never clear attention on the terminal being abandoned just because focus moved elsewhere.
  - Route all click/focus/manager selection acknowledgments through one helper so the UI and tests stay aligned.
  - Keep interaction-driven clears limited to the active terminal's own user action paths.
- Files/Commands touched: `src/app.rs`, `src/hooks.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local workspace fix on 2026-04-06; commit pending

#### Factory Droid Windows hook commands failed because the installer persisted a quoted launcher string {#factory-droid-windows-hook-commands-failed-because-the-installer-persisted-a-quoted-launcher-string}
- Date: 2026-04-06T16:45:00Z
- Context: main/Windows local Factory Droid user-wide hook installation under `%USERPROFILE%\.factory\settings.json`
- Error signature: `HOOKS Stop ... '\"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe\"' is not recognized as an internal or external command`
- Symptoms/Impact: Factory registered the hook entry but failed before launching the PowerShell script, so Mergen-ADE never received the intended running/attention marker or title updates from `UserPromptSubmit` / `Notification` / `Stop`.
- Root cause: `scripts/install-factory-droid-hooks.ps1` wrote a launcher command that wrapped `powershell.exe` and the managed hook path in quotes. Factory's Windows hook runner forwarded that command shape literally, so `cmd` treated the quoted executable token as the program name instead of launching PowerShell.
- Resolution: The installer now emits one canonical Windows launcher command with an unquoted executable token and a quoted absolute script path, migrates any existing `mergen-ade-droid-status.ps1` hook entries to that canonical command instead of duplicating them, and verifies the installed command executes successfully through `cmd /c`.
- Prevent recurrence:
  - Keep the managed Factory Droid hook command canonicalized by the installer; do not hand-edit quoted variants into `%USERPROFILE%\.factory\settings.json`.
  - Re-run the installer when the managed hook path changes so legacy/broken entries are normalized instead of accumulating.
  - Restart Droid or accept the change from `/hooks` after editing hook settings because Factory snapshots hooks at session start.
- Files/Commands touched: `scripts/install-factory-droid-hooks.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-factory-droid-hooks.ps1`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: Factory docs reviewed on 2026-04-06 (`https://docs.factory.ai/reference/hooks-reference`); local failure reproduced from Droid transcript output on Windows

#### Factory Droid Hooks menu crashed because the installer serialized managed hook events as objects instead of arrays {#factory-droid-hooks-menu-crashed-because-the-installer-serialized-managed-hook-events-as-objects-instead-of-arrays}
- Date: 2026-04-06T17:15:00Z
- Context: main/Windows local Factory Droid `/hooks` and terminal startup after the first Windows launcher fix
- Error signature: `ERROR (D.hooks?.[G]||[]).reduce is not a function` in `src/components/hooks/HooksMenu.tsx`
- Symptoms/Impact: Droid could start and the hook command itself was valid, but opening the Hooks UI crashed because `~/.factory/settings.json` stored `UserPromptSubmit`, `Notification`, and `Stop` as `{ hooks: [...] }` instead of `[ { hooks: [...] } ]`.
- Root cause: `Normalize-FactoryHookEventEntries()` returned a one-element collection through the PowerShell pipeline, which unwrapped the array to a scalar `PSCustomObject` before `Merge-FactoryHookSettings()` assigned it. `ConvertTo-Json` then persisted an object-shaped event value, and the Hooks UI assumed an array and called `.reduce(...)`.
- Resolution: The installer now preserves managed event lists as arrays at both the normalization return boundary and the settings assignment point, validates the serialized JSON shape after writing, and adds regression tests that inspect the raw written `settings.json` instead of masking object-vs-array bugs with `@(...)`.
- Prevent recurrence:
  - Validate the persisted JSON contract, not just the in-memory PowerShell object graph.
  - Keep managed hook events serialized as arrays even when there is only one hook entry.
  - Seed tests with the malformed object-shaped form so future migrations prove the Hooks UI contract stays intact.
- Files/Commands touched: `scripts/install-factory-droid-hooks.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-factory-droid-hooks.ps1`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: Droid Hooks UI crash observed locally on 2026-04-06 after the Windows launcher quoting fix; current contract verified against `%USERPROFILE%\.factory\settings.json`

#### Factory Droid badge transport stayed dark because Factory hook output does not reliably flow through the PTY stream {#factory-droid-badge-transport-stayed-dark-because-factory-hook-output-does-not-reliably-flow-through-the-pty-stream}
- Date: 2026-04-06T18:30:00Z
- Context: main/Windows local Factory Droid sessions inside Mergen-ADE integrated terminals after hook registration and installer fixes
- Error signature: Hooks appeared in Droid and the terminal transcript showed `HOOKS Stop`, but the Mergen-ADE green/yellow badge still never changed state.
- Symptoms/Impact: The hook runner was active, yet `UserPromptSubmit`, `Notification`, and `Stop` signals did not reach the badge pipeline because `src/terminal.rs` only updates AI status from PTY bytes and OSC title bytes observed by the integrated terminal reader.
- Root cause: Factory hook stdout/stderr semantics do not provide one PTY-visible channel for all needed events. `UserPromptSubmit` output is special-cased by Factory, `Notification` output is not guaranteed to be transcript-visible, and writing to `CONOUT$` or console title APIs bypassed Mergen-ADE's PTY reader entirely.
- Resolution: Replaced the PTY marker/title transport with a terminal-scoped JSONL inbox under the Mergen-ADE app-data runtime directory. Each spawned terminal now injects `MERGEN_ADE_TERMINAL_ID` and `MERGEN_ADE_FACTORY_DROID_HOOKS_DIR`, the Factory hook script appends one quiet JSONL record per actionable event, and `src/app.rs` polls those inbox files to drive `Running` and `Attention`. A local Enter-submit fallback now also sets `Running` immediately for already-tagged Factory Droid terminals.
- Prevent recurrence:
  - Do not rely on Factory hook stdout, stderr, or `CONOUT$` writes as the primary badge transport.
  - Keep hook delivery scoped by terminal id so concurrent Droid terminals cannot cross-talk.
  - Treat `UserPromptSubmit` / `Notification` / `Stop` as app-runtime events, not transcript markers.
- Files/Commands touched: `src/app.rs`, `src/config.rs`, `src/hooks.rs`, `src/terminal.rs`, `scripts/factory-droid-status-hook.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `cargo test`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: Factory docs rechecked on 2026-04-06 (`https://docs.factory.ai/reference/hooks-reference`); local Droid transcript showed registered hooks without badge updates

#### Factory Droid badge fixes appeared broken when the user relaunched a stale Desktop binary instead of the rebuilt release {#factory-droid-badge-fixes-appeared-broken-when-the-user-relaunched-a-stale-desktop-binary-instead-of-the-rebuilt-release}
- Date: 2026-04-06T19:00:00Z
- Context: main/Windows local manual launch flow using `C:\Users\furkan.cakir\Desktop\mergen-ade-new.exe`
- Error signature: The integrated terminal still showed no green/yellow Factory Droid indicator even after the inbox-based hook transport shipped and the global Factory hooks were installed correctly.
- Symptoms/Impact: The repo built successfully and the user-wide hook script was current, but the visible app behavior stayed old because the user continued launching an older side-loaded Desktop executable instead of the freshly built release binary from the repo.
- Root cause: The running process path pointed at `C:\Users\furkan.cakir\Desktop\mergen-ade-new.exe`, whose hash differed from `target\x86_64-pc-windows-msvc\release\mergen-ade.exe`. That stale launcher binary did not include the latest Factory Droid transport and diagnostics changes.
- Resolution: Added in-app diagnostics for the current executable path and Factory Droid inbox runtime status, plus a visible top-bar warning when the inbox runtime directory is unavailable. Operationally, the Desktop launcher must be replaced with the current release build before testing Droid badge behavior.
- Prevent recurrence:
  - Always update the actual launcher binary the user runs, not just the repo build output.
  - Surface the current executable path in Settings so stale side-loaded binaries are immediately visible.
  - Verify the Factory Droid inbox runtime path shown by the app before diagnosing hook behavior.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: local process inspection on 2026-04-06 showed the running app was `C:\Users\furkan.cakir\Desktop\mergen-ade-new.exe` while the current repo release binary lived under `target\x86_64-pc-windows-msvc\release\mergen-ade.exe`

#### Factory Droid inbox transport was not reliable because Droid hooks did not consistently inherit Mergen-specific env vars {#factory-droid-inbox-transport-was-not-reliable-because-droid-hooks-did-not-consistently-inherit-mergen-specific-env-vars}
- Date: 2026-04-06T19:20:00Z
- Context: main/Windows local Factory Droid sessions after the inbox-based hook transport and launcher refresh were already in place
- Error signature: Settings showed `Inbox JSONL (Factory Droid hooks)` as ready and Droid showed `HOOKS Stop`, but `%APPDATA%\Mergen\MergenADE\config\runtime\factory-droid-hooks` stayed empty during real sessions.
- Symptoms/Impact: The hook script worked in direct PowerShell tests with `MERGEN_ADE_TERMINAL_ID` and `MERGEN_ADE_FACTORY_DROID_HOOKS_DIR`, yet real Droid hook executions still produced no JSONL files, so the badge pipeline never saw `Running` or `Attention`.
- Root cause: Mergen injected custom `MERGEN_ADE_*` env vars into the integrated shell child, but Factory's hook subprocess did not reliably inherit those env vars in real runs. Factory docs only guarantee hook stdin JSON and documented Droid env like `FACTORY_PROJECT_DIR`, not arbitrary terminal-local env propagation. This made the inbox transport a best-effort path instead of a dependable primary signal.
- Resolution: Pivoted the primary Factory Droid badge transport to PTY/process detection. Mergen now treats descendant `droid.exe`/`factory.exe` processes as the authoritative session boundary, marks `Running` from prompt submission inside an active Droid session, and marks `Attention` from visible PTY text like `HOOKS Stop`, permission prompts, and idle prompts. Inbox JSONL remains as fallback only.
- Prevent recurrence:
  - Do not make badge correctness depend on undocumented hook env inheritance.
  - Use process-descendant checks to prove a terminal is actually hosting a Droid session before turning prompt submits into green activity.
  - Treat hook inbox files as optional enrichment, not the sole transport.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, direct hook smoke tests, `droid --help`, `droid exec --help`, `droid --debug exec ...`
- References: Factory docs rechecked on 2026-04-06 (`https://docs.factory.ai/reference/hooks-reference`, `https://docs.factory.ai/cli/configuration/hooks-guide`); local direct script invocation wrote JSONL while real Droid hook execution did not

#### Factory Droid `Stop` PTY events could be dropped when the Droid process exited before the next UI frame {#factory-droid-stop-pty-events-could-be-dropped-when-the-droid-process-exited-before-the-next-ui-frame}
- Date: 2026-04-07T00:00:00Z
- Context: main/Windows local Factory Droid sessions after the PTY/process-primary badge pivot
- Error signature: Green `Running` pulse started correctly after prompt submit, Droid visibly printed `HOOKS Stop`, but the badge never transitioned to yellow `Attention`.
- Symptoms/Impact: Mergen detected active Droid sessions and prompt submits, yet completion and waiting-state PTY text could be ignored if the descendant Droid process disappeared immediately before the next `update()` cycle. This left the badge stuck in green or cleared it before the user saw the stop state.
- Root cause: `update()` previously polled descendant Droid processes before draining PTY terminal events. When `droid.exe` exited, `poll_factory_droid_processes()` cleared the Factory Droid session immediately, so the trailing `HOOKS Stop`, permission, or input-wait PTY chunks arriving in the same frame were no longer associated with an active Droid session.
- Resolution: Reordered the main loop to process PTY terminal events before process polling, added a 750 ms trailing-output grace window for missing Droid processes, and preserved `Attention` until user interaction instead of clearing it as soon as the process tree vanished. Added regressions for update-order stop delivery, post-exit stop chunks, attention persistence, and stale-running cleanup after grace expiry.
- Prevent recurrence:
  - Process PTY-delivered Factory Droid status before descendant-process cleanup.
  - Keep a short trailing-output grace window so process exit and terminal transcript delivery do not race.
  - Do not auto-clear `Attention` just because the Droid process has already exited.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: local reproduction on 2026-04-07 from a real Droid session where `HOOKS Stop` was visible but the badge stayed green; reviewer confirmation from subagent `Ampere`

#### Factory Droid visible `HOOKS Stop` text could still be missed when PTY output split the phrase across multiple reads {#factory-droid-visible-hooks-stop-text-could-still-be-missed-when-pty-output-split-the-phrase-across-multiple-reads}
- Date: 2026-04-07T00:30:00Z
- Context: main/Windows local Factory Droid sessions after the stop-race fix was already in place
- Error signature: Droid transcript visibly showed `HOOKS  Stop`, but the badge stayed green even though the process-exit race and trailing grace logic were already fixed.
- Symptoms/Impact: The app-side state machine was ready to turn visible stop/wait text into `Attention`, but the PTY reader only looked for those phrases inside the current `read()` chunk. If `HOOKS  Stop`, `needs your permission`, or `waiting for your input` was split across PTY reads, the transcript rendered the full line while no `AiRawChunk` event was emitted.
- Root cause: `src/terminal.rs` used stateless visible-text detection via `official_ai_debug_chunk(&text)` on one PTY chunk at a time. Unlike OSC title parsing, there was no rolling buffer for visible Factory Droid status text, no ANSI normalization, and no CRLF normalization for this path.
- Resolution: Added a bounded rolling visible-status parser in `src/terminal.rs` that normalizes ANSI/CRLF, carries text across PTY reads, detects split `HOOKS Stop` / permission / input-wait phrases once, and emits a single canonical `AiRawChunk` when the full phrase is assembled.
- Prevent recurrence:
  - Keep visible Factory Droid status detection stateful across PTY reads.
  - Normalize ANSI escape sequences and CRLF before matching visible status phrases.
  - Bound the rolling parser buffer and regression-test split-read, duplicate-emission, and trim-boundary cases.
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: local reproduction on 2026-04-07 from a real Droid session showing `HOOKS  Stop`; reviewer confirmation from subagent `McClintock`

#### Factory Droid Windows hook launcher printed a PowerShell banner and `Yolda geçersiz karakterler var` because the managed `-File` path was quoted {#factory-droid-windows-hook-launcher-printed-a-powershell-banner-and-yolda-gecersiz-karakterler-var-because-the-managed-file-path-was-quoted}
- Date: 2026-04-07T00:45:00Z
- Context: main/Windows local Factory Droid sessions after the badge transport fixes were already working
- Error signature: Droid showed a hook warning block with `Windows PowerShell`, `Install the latest PowerShell...`, and `Processing -File '"C:\Users\furkan.cakir\.factory\hooks\mergen-ade-droid-status.ps1"' failed: Yolda geçersiz karakterler var`.
- Symptoms/Impact: `Stop` still reached Droid, but each managed hook invocation leaked noisy banner text and an invalid-path warning into the transcript because PowerShell received an extra-quoted `-File` argument.
- Root cause: The managed hook command in `%USERPROFILE%\.factory\settings.json` was persisted as `powershell.exe ... -File "C:\...\mergen-ade-droid-status.ps1"`. Factory's Windows hook runner re-quoted that token, so PowerShell saw `-File '"C:\...\ps1"'`, rejected it as an invalid path, and printed its normal Windows PowerShell startup banner because `-NoLogo` was not present.
- Resolution: The installer now normalizes every managed Factory Droid hook command to `powershell.exe -NoLogo -NonInteractive -NoProfile -ExecutionPolicy Bypass -File C:\...\mergen-ade-droid-status.ps1`, migrates existing quoted commands in place, rejects whitespace-containing managed script paths on Windows, and regression-tests the quote-free `cmd /c` execution path.
- Prevent recurrence:
  - Keep the managed Windows hook command quote-free after `-File`.
  - Include `-NoLogo -NonInteractive` in the canonical hook launcher to suppress banner noise.
  - Fail fast when the managed hook script path contains whitespace instead of persisting a command that Factory will re-quote incorrectly.
- Files/Commands touched: `scripts/install-factory-droid-hooks.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-factory-droid-hooks.ps1`, `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: local Droid transcript captured on 2026-04-07 showing the PowerShell banner and invalid `-File` path error

#### Factory Droid yellow `Attention` badge sometimes required a click because typed input was blocked by UI focus and the routed-input clear check looked at an already-flushed buffer {#factory-droid-yellow-attention-badge-sometimes-required-a-click-because-typed-input-was-blocked-by-ui-focus-and-the-routed-input-clear-check-looked-at-an-already-flushed-buffer}
- Date: 2026-04-07T01:00:00Z
- Context: main/Windows local Factory Droid sessions after green/yellow badge signaling was otherwise working
- Error signature: Yellow `Attention` badge cleared on clicking the active terminal, but sometimes stayed yellow when the user started typing until they clicked first.
- Symptoms/Impact: When a repo UI text field still owned keyboard focus, the first typed character never reached the active terminal, so the badge did not clear. Even when terminal text did reach `route_active_terminal_input()`, the attention-clear check could still miss it because it looked at `outbound.is_empty()` after `flush_terminal_outbound()` had already drained the buffer.
- Root cause: Two conditions combined. First, `raw_input_hook()` left keyboard ownership with directory search or saved-message draft inputs unless the user clicked the terminal. Second, `route_active_terminal_input()` used the post-flush `outbound` buffer to decide whether terminal interaction happened, so real typed input could fail to call `manager.user_interacted(...)`.
- Resolution: Added an attention-specific one-frame keyboard-routing override that surrenders app text-input focus and buffers the first terminal text-entry event for the active terminal, while still preserving popup/context-menu/modal ownership. Separately, `route_active_terminal_input()` now tracks terminal interaction with a dedicated latch instead of checking the already-flushed `outbound` buffer.
- Prevent recurrence:
  - Do not infer user interaction from a buffer after it has been drained into the PTY writer.
  - Keep attention-specific keyboard stealing scoped to active-terminal `Attention` sessions and app text-input focus only.
  - Preserve popup, context-menu, and settings-modal keyboard ownership even when a terminal is waiting.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local user reproduction on 2026-04-07 where yellow cleared only after clicking; reviewer confirmation from subagent `Hooke`

#### Factory Droid hook disablement still left the hook runtime active because bootstrap created a fallback manager {#factory-droid-hook-disablement-still-left-the-hook-runtime-active-because-bootstrap-created-a-fallback-manager}
- Date: 2026-04-07T02:00:00Z
- Context: main/Windows local Factory Droid integration with `ai_hooks.global_enabled = false`
- Error signature: Disabling AI hooks in config still left Factory Droid badge state and hook-runtime behavior active.
- Symptoms/Impact: Users could turn hooks off in config yet Mergen-ADE still created a hook manager, still exposed Factory Droid runtime diagnostics, and could still react to Factory-specific status transitions instead of fully disabling the integration.
- Root cause: `src/app.rs` previously treated the disabled branch as "use Factory defaults," so bootstrap still constructed an `AiHookManager` and downstream logic still had a live hook runtime to poll and route through.
- Resolution: Hook bootstrap is now authoritative on `ai_hooks.global_enabled`: disabled config returns `None` for the manager and runtime directory, terminal spawn skips Factory-specific env injection, launch-pending/input-steal/inbox-polling paths are gated on manager presence, and PTY/title Factory Droid status changes now flow through the shared status helper so diagnostics record the real source when hooks are enabled.
- Prevent recurrence:
  - Treat disabled hook config as an absent runtime, not as a request to fall back to Factory defaults.
  - Gate every Factory Droid-specific runtime path on manager presence instead of re-deriving "enabled" from partial state.
  - Keep status-source diagnostics routed through the shared Factory Droid state helper so transport reporting stays consistent.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test`
- References: 2026-04-07 code review finding for `src/app.rs`; local regression tests `disabled_ai_hooks_do_not_create_manager`, `ai_status_change_event_updates_badge_without_debug_ui_state`, `ai_status_change_event_from_title_records_terminal_title_source`

#### Factory Droid Windows managed hook launcher could not be installed under profile paths with spaces because the persisted command relied on `-File` quoting {#factory-droid-windows-managed-hook-launcher-could-not-be-installed-under-profile-paths-with-spaces-because-the-persisted-command-relied-on-file-quoting}
- Date: 2026-04-07T02:15:00Z
- Context: main/Windows local Factory Droid hook installation under `%USERPROFILE%\.factory\hooks`
- Error signature: Managed hook installation failed for Windows users whose home/profile path contained spaces.
- Symptoms/Impact: The hook script lives under `%USERPROFILE%\.factory\hooks`, so the installer could reject otherwise normal Windows profile paths and leave Factory Droid hook registration unusable on affected machines.
- Root cause: The managed launcher contract still depended on a `powershell.exe ... -File <path>` command shape that Factory/cmd would re-quote inconsistently on Windows. The installer tried to avoid the quoting failure by rejecting whitespace instead of making the launcher path-safe.
- Resolution: The installer now persists one canonical `powershell.exe ... -EncodedCommand <base64>` launcher that bootstraps the managed script path inside PowerShell, recognizes and migrates both legacy `-File` and encoded managed commands, and keeps spaces and `%` characters working in the installed script path. Regression tests now exercise whitespace-containing and percent-containing home directories.
- Prevent recurrence:
  - Keep the persisted managed launcher in encoded-command form instead of depending on shell-level path quoting.
  - Normalize legacy managed entries in place so reinstalling collapses old `-File` variants and duplicates.
  - Test installer behavior with realistic Windows profile paths, including spaces and `%`.
- Files/Commands touched: `scripts/install-factory-droid-hooks.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: 2026-04-07 code review finding for `scripts/install-factory-droid-hooks.ps1`; local PowerShell regression tests for encoded launcher canonicalization and space-containing home dirs

#### Factory Droid inbox JSONL records could replay into a new terminal after app restart because acceptance keyed only on terminal id {#factory-droid-inbox-jsonl-records-could-replay-into-a-new-terminal-after-app-restart-because-acceptance-keyed-only-on-terminal-id}
- Date: 2026-04-07T02:30:00Z
- Context: main/Windows local Factory Droid inbox transport after terminal ids began restarting from `1` on each app launch
- Error signature: Delayed inbox writes from an older Droid session could mark a new terminal `Running` or `Attention` after restart.
- Symptoms/Impact: Mergen-ADE reused low terminal ids across launches, so a stale JSONL append targeting `1.jsonl` could be accepted by a freshly spawned terminal with the same id even though the old Droid session was gone.
- Root cause: `src/app.rs` previously accepted inbox events by filename/terminal id alone. Because terminal ids are app-local counters rather than durable session identities, old hook writes were indistinguishable from current-terminal writes.
- Resolution: Each spawned terminal now gets a fresh app-generated inbox token, the terminal runtime injects it through `MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN`, the hook script writes that token into each JSONL record, and the app accepts inbox events only when both `terminal_id` and `inbox_token` match the currently running terminal entry. `session_id` remains informational metadata only.
- Prevent recurrence:
  - Use a per-terminal-instance token for inbox delivery identity; do not rely on restartable terminal ids alone.
  - Keep hook env propagation and JSONL schema changes regression-tested together so transport identity stays end-to-end.
  - Treat Factory `session_id` as metadata rather than the sole acceptance key because one terminal can host multiple Droid sessions over time.
- Files/Commands touched: `src/app.rs`, `src/hooks.rs`, `src/terminal.rs`, `scripts/factory-droid-status-hook.ps1`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `KNOWN_ISSUES.md`, `cargo test`, `powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: 2026-04-07 code review finding for `src/app.rs`; local regression tests `factory_droid_hook_inbox_ignores_stale_token_records` and PowerShell inbox-token preservation coverage

#### Held `Backspace` in the integrated terminal could stop deleting because terminal routing depended on platform repeat events instead of a stable held-key repeat path {#held-backspace-in-the-integrated-terminal-could-stop-deleting-because-terminal-routing-depended-on-platform-repeat-events-instead-of-a-stable-held-key-repeat-path}
- Date: 2026-04-07T00:00:00Z
- Context: main/Windows local integrated terminal text editing in Mergen-ADE
- Error signature: `Backspace'a uzun basinca bir sure sonra silmeyi birakiyor; normal terminal gibi kesintisiz silmiyor.`
- Symptoms/Impact: While editing shell input inside the embedded terminal, holding `Backspace` could delete a few characters and then stall until the user released and pressed again. This made integrated terminal editing feel inconsistent with standard Windows terminals.
- Root cause: The app forwarded raw `Event::Key` presses to the PTY but had no deterministic held-key repeat layer of its own. Once platform repeat delivery became sparse or stopped reaching the routed event list, `Backspace` no longer generated additional `0x7f` bytes even though the key was still physically held.
- Resolution: Added terminal-scoped held-key repeat state in `src/app.rs`, keyed by active terminal plus key/modifiers, and preprocess terminal events before routing so duplicate OS repeat presses are suppressed while synthetic repeat presses are emitted frame-by-frame until release. On Windows the repeat timing is seeded from `SystemParametersInfoW(SPI_GETKEYBOARDDELAY/SPI_GETKEYBOARDSPEED)` with a safe fallback; repeat state is cleared on terminal switch/close or whenever the terminal stops owning keyboard capture. Regression tests now cover arming, duplicate suppression, timed repeat synthesis, release/capture-loss cleanup, active-terminal cleanup, and PTY byte output via a test capture runtime in `src/terminal.rs`.
- Prevent recurrence:
  - Keep held-key repeat state scoped to active terminal input routing; do not hide this logic in unrelated UI focus paths.
  - Do not depend on platform autorepeat events alone for destructive terminal editing keys like `Backspace` and `Delete`.
  - Preserve a byte-capture test path so terminal input regressions can assert PTY output directly instead of inferring behavior from UI state.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local user reproduction on 2026-04-07; regression tests `first_backspace_press_arms_terminal_held_key_repeat`, `held_backspace_synthesizes_repeat_events_after_delay`, and `held_backspace_repeat_writes_multiple_delete_bytes_to_terminal`

#### Terminal render path emitted fake backslash warnings for normal Windows prompt lines because a temporary snapshot debug logger was left enabled {#terminal-render-path-emitted-fake-backslash-warnings-for-normal-windows-prompt-lines-because-a-temporary-snapshot-debug-logger-was-left-enabled}
- Date: 2026-04-07T08:24:42Z
- Context: main/Windows local integrated terminal rendering while PowerShell prompt lines included standard `C:\...` paths
- Error signature: Repeated `[WARN  mergen_ade::app] DEBUG backslash in snapshot` lines appeared while the terminal showed ordinary Windows paths such as `PS C:\Users\...`
- Symptoms/Impact: The app logged noisy warnings during normal terminal rendering even though the snapshot content was valid. This obscured real problems and made backslashes in standard Windows prompt output look like rendering errors.
- Root cause: `src/app.rs` contained a temporary render-path debug block that scanned every terminal snapshot line for `\` and emitted a `warn!` whenever one was found. Because Windows prompt paths legitimately contain backslashes, the logger produced false-positive warnings on healthy output.
- Resolution: Removed the temporary `DEBUG backslash in snapshot` warning block from the terminal render path and kept the existing snapshot tests as the guardrail for real OSC/ST backslash leakage behavior.
- Prevent recurrence:
  - Do not treat plain `\` characters as a warning condition in Windows terminal output.
  - Keep temporary snapshot diagnostics out of the render hot path unless they are gated behind an explicit debug-only mechanism.
  - Rely on targeted snapshot tests for OSC/ST leakage instead of broad runtime warning heuristics.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test backslash`, `cargo test`
- References: local user reproduction on 2026-04-07 with repeated `DEBUG backslash in snapshot` warnings for `PS C:\Users\...`; regression tests `st_terminated_osc_does_not_leak_backslash_in_snapshot`, `bell_terminated_osc_does_not_leak_backslash_in_snapshot`, and `plain_text_with_backslash_renders_correctly`

#### Single-terminal view ignored `Ctrl+Alt+ArrowUp/ArrowDown` because navigation only considered the currently visible terminal and the shortcut was not routed as app navigation {#single-terminal-view-ignored-ctrl-alt-arrowup-arrowdown-because-navigation-only-considered-the-currently-visible-terminal-and-the-shortcut-was-not-routed-as-app-navigation}
- Date: 2026-04-07T00:00:00Z
- Context: main/Windows local keyboard navigation while `multi_terminal_view_enabled = false`
- Error signature: In single-terminal view, `Ctrl+Alt+Yukarı/Aşağı` did nothing even when multiple terminal sessions were open.
- Symptoms/Impact: Users could not switch between open terminals from the keyboard while the main area showed only one terminal. Existing grid navigation only operated on the visible tile set, which collapses to a single terminal in single-view mode.
- Root cause: `src/app.rs` treated terminal navigation as a grid-only concept backed by `visible_terminal_ids_for_main()`. In single-terminal mode that list contains only the active terminal, so no neighbor existed to move to. At the same time, `Ctrl+Alt+ArrowUp/ArrowDown` was not represented as a distinct app shortcut, so there was no dedicated single-view navigation path.
- Resolution: Added a distinct internal `TerminalNavigationShortcut` representation that separates grid navigation from single-view linear navigation. `raw_input_hook()` and terminal input partitioning now recognize `Ctrl+Alt+ArrowUp/ArrowDown` only when single-view mode is active, preventing multi-view regressions. `handle_shortcuts()` now routes those shortcuts through a linear helper that walks all terminal ids in ascending order without wraparound, while rendering still shows only the active terminal in single-view mode. Regression tests cover parsing, buffering, no-wrap edges, direct helper behavior, and active visible-terminal switching.
- Prevent recurrence:
  - Keep shortcut parsing mode-aware when a key combination is intended for only one layout mode.
  - Do not reuse visible-tile navigation lists for single-view terminal switching; single-view needs its own navigable terminal list.
  - Keep `raw_input_hook()` and `handle_shortcuts()` covered together so shortcut interception and active-terminal changes stay in sync.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: local user report on 2026-04-07; regression tests `event_terminal_navigation_shortcut_recognizes_ctrl_alt_up_down`, `raw_input_hook_buffers_ctrl_alt_arrow_for_single_view_navigation`, and `handle_shortcuts_moves_single_view_terminal_with_ctrl_alt_down`

#### Factory Droid launch timeout cleanup and single-view terminal navigation could get stuck on unsupported process probes or exited terminals {#factory-droid-launch-timeout-cleanup-and-single-view-terminal-navigation-could-get-stuck-on-unsupported-process-probes-or-exited-terminals}
- Date: 2026-04-07T09:00:00Z
- Context: main/cross-platform Factory Droid polling plus single-terminal keyboard navigation after the initial single-view shortcut rollout
- Error signature: `Factory Droid launch-pending state never cleared on non-Windows, and Ctrl+Alt+Arrow navigation in single-view could land on an exited terminal.`
- Symptoms/Impact: On platforms where descendant-process probing is unsupported, expired Factory Droid launch attempts could remain stuck in pending state because process polling skipped cleanup entirely. Separately, single-view keyboard navigation could activate an exited terminal entry, leaving the main terminal view on a dead session that no longer accepted input.
- Root cause: `src/app.rs` treated `has_factory_droid_descendant_process() == None` as a full early-exit from process polling, so launch-timeout cleanup never ran on those platforms. The same file built single-view navigation candidates from all terminal ids instead of filtering out exited terminals.
- Resolution: Launch-timeout cleanup now runs before missing-process inference whenever launch grace has expired and no active descendant process was positively detected, so unsupported probes still clear stale pending Factory Droid state without fabricating a missing-process signal. Single-view `Ctrl+Alt+ArrowUp/ArrowDown` navigation now walks only live terminal ids, preserving sorted order while skipping exited entries. Regression tests cover the non-Windows expired-launch path and both up/down skip-over-exited navigation paths.
- Prevent recurrence:
  - Keep launch-pending timeout cleanup independent from platform-specific descendant-process probing support.
  - Only infer missing Factory Droid processes from explicit negative probes, not from unsupported probes.
  - Filter exited terminals out of single-view keyboard navigation lists and lock the behavior with mixed live/exited regression tests.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-07 review findings for `src/app.rs`; regression tests `factory_droid_process_poll_clears_expired_launch_when_process_probe_is_unsupported`, `handle_shortcuts_skips_exited_single_view_terminal_with_ctrl_alt_down`, and `handle_shortcuts_skips_exited_single_view_terminal_with_ctrl_alt_up`

#### Mixed Factory Droid hook/title/status reads could apply signals out of byte order because hook parsing consumed the whole PTY tail instead of the hook boundary {#mixed-factory-droid-hook-title-status-reads-could-apply-signals-out-of-byte-order-because-hook-parsing-consumed-the-whole-pty-tail-instead-of-the-hook-boundary}
- Date: 2026-04-07T10:00:00Z
- Context: main/Windows local Factory Droid PTY parsing when official hook markers shared a read with later OSC titles or visible status text
- Error signature: `A UserPromptSubmit hook followed by later Idle/title or HOOKS Stop bytes in the same PTY read could be applied in the wrong order.`
- Symptoms/Impact: The AI badge and session state could briefly or permanently land on the wrong state for mixed reads. In the worst case, later `Idle`/attention bytes were sorted ahead of or collapsed into the earlier hook event, so the final state no longer reflected the actual byte order emitted by Factory Droid.
- Root cause: `src/hooks.rs` treated any tail containing a complete official hook as fully consumed, so `AiHookTransition.text_offset` pointed at the end of the whole PTY tail rather than the closing `]`. Separately, `src/terminal.rs` emitted an "official raw chunk" containing trailing bytes after the earliest hook/title boundary, allowing later `HOOKS Stop` text in the same read to be interpreted too early.
- Resolution: Hook splitting now stops at the first complete official hook boundary and keeps trailing bytes buffered for the same pass, including back-to-back official hook markers without newlines. The terminal reader now trims official raw chunks to the earliest complete hook/title span so later visible-status text is emitted by its own later signal. Regression tests cover trailing-byte buffering, back-to-back hook markers, hook-plus-title without newline, hook-plus-`HOOKS Stop` without newline, and clamped offsets with trailing non-ASCII bytes.
- Prevent recurrence:
  - Treat official hook boundaries as exact parsing spans, not as permission to consume the rest of the PTY tail.
  - Keep offset-based signal ordering tests for mixed hook/title/visible-status reads without relying on newline separators.
  - Bound debug/raw signal payloads to the actual signal span when downstream logic also interprets those chunks.
- Files/Commands touched: `src/hooks.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-07 review finding for `src/hooks.rs`; regression tests `extract_complete_lines_with_end_offsets_keeps_trailing_bytes_available`, `update_with_text_offsets_processes_back_to_back_hook_markers_without_newlines`, `collect_ai_read_signals_orders_hook_before_later_title_signal_without_newline`, and `collect_ai_read_signals_orders_hook_before_later_visible_attention`

#### Single-view `Ctrl+Alt+ArrowUp/ArrowDown` recovery could fail after the active terminal exited because navigation anchored on "accepts input" instead of the current selection {#single-view-ctrlaltarrowupctrlaltarrowdown-recovery-could-fail-after-the-active-terminal-exited-because-navigation-anchored-on-accepts-input-instead-of-the-current-selection}
- Date: 2026-04-07T10:05:00Z
- Context: main/Windows local single-terminal keyboard navigation after the initial single-view shortcut rollout
- Error signature: `If the currently shown single-view terminal had already exited, Ctrl+Alt+ArrowUp/ArrowDown no longer recovered to a live terminal until the user clicked manually.`
- Symptoms/Impact: Single-view mode could stay stuck on a dead terminal entry that no longer accepted input. Keyboard-only users lost the intended recovery path because the navigation shortcut resolved its anchor from `active_terminal_accepts_input()`, which returns `None` for exited terminals.
- Root cause: `src/app.rs` built single-view linear navigation from only live terminal ids while also using the "accepts input" terminal as the active anchor. Once the selected terminal exited, the anchor disappeared from the navigation list and the shortcut had no starting position to scan from.
- Resolution: Single-view navigation now anchors on the current selected terminal if it still exists, even when it has exited, walks the full sorted terminal order, and skips exited terminals while scanning in the requested direction. Regression tests cover recovery from an exited active terminal in both directions, skip-over-exited neighbors, and no-op behavior when no live neighbor exists.
- Prevent recurrence:
  - Separate "currently selected terminal" from "terminal that can currently receive PTY input" in keyboard-navigation logic.
  - Build single-view navigation from full terminal ordering, then filter/select live targets during directional scanning.
  - Keep exited-active recovery tests alongside edge/no-op tests so future shortcut changes do not reintroduce the dead-end.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-07 review finding for `src/app.rs`; regression tests `handle_shortcuts_recovers_from_exited_active_single_view_terminal_with_ctrl_alt_down`, `handle_shortcuts_recovers_from_exited_active_single_view_terminal_with_ctrl_alt_up`, and `handle_shortcuts_keeps_exited_single_view_terminal_when_no_live_neighbor_exists`

#### Push to `origin/main` could fail after local agent/build artifacts were committed into an unpushed history slice {#push-to-originmain-could-fail-after-local-agentbuild-artifacts-were-committed-into-an-unpushed-history-slice}
- Date: 2026-04-07T13:20:00Z
- Context: main/Windows local git push recovery after several unpushed commits accidentally included local tool output and alternate Cargo target artifacts
- Error signature: `git push` disconnected with `Read from remote host ssh.github.com: Connection reset by peer`, `send-pack: unexpected disconnect while reading sideband packet`, and `fatal: the remote end hung up unexpectedly`.
- Symptoms/Impact: Publishing local work could fail even though SSH authentication itself succeeded. The outgoing history slice carried thousands of non-source files such as `.firecrawl/*`, `.claude/settings.local.json`, and `target_test/*`, inflating the object set and polluting the repo with machine-local artifacts.
- Root cause: `.gitignore` excluded only `/target/` and missed alternate Cargo output directories plus repo-local agent scratch directories, so a broad local commit captured generated build output and local research/config artifacts into the unpushed commit chain.
- Resolution: Rebuilt a clean branch from `origin/main`, replayed only the real source changes, dropped `.claude/`, `.firecrawl/`, and `target_test/` from the replayed history, and added ignore rules for those paths so future commits keep them local-only.
- Prevent recurrence:
  - Ignore all alternate local Cargo target directories used for ad hoc test runs, not just `/target/`.
  - Keep repo-local agent scratch/config directories such as `.claude/` and `.firecrawl/` out of version control unless the project explicitly standardizes them.
  - When a broad "save pending changes" commit includes unusually large file counts, inspect the path list before pushing.
- Files/Commands touched: `.gitignore`, `KNOWN_ISSUES.md`, `git cherry-pick --no-commit`, `git restore --source=HEAD --staged --worktree .claude .firecrawl target_test`
- References: local push recovery on 2026-04-07 after `git diff --name-only origin/main..HEAD` exposed `target_test/`, `.firecrawl/`, and `.claude/settings.local.json` in the unpushed history

#### Factory Droid `Stop` could stay green when the global managed hook install drifted behind Mergen's inbox-token contract {#factory-droid-stop-could-stay-green-when-the-global-managed-hook-install-drifted-behind-mergens-inbox-token-contract}
- Date: 2026-04-08T00:00:00Z
- Context: main/Windows local Factory Droid sessions after the per-terminal inbox-token guard had already shipped
- Error signature: `Droid finished, global Stop hook ran, but the badge stayed green instead of switching to yellow Attention.`
- Symptoms/Impact: `UserPromptSubmit` still turned the badge green, yet `Stop` could be silently dropped. The app consumed the JSONL inbox line, advanced the file offset, and gave no visible reason why the end-of-response signal was ignored.
- Root cause: `%USERPROFILE%\.factory\hooks\mergen-ade-droid-status.ps1` had drifted behind the repo copy and no longer wrote `inbox_token`, while `%USERPROFILE%\.factory\settings.json` still contained a legacy managed `-File` launcher. Mergen correctly required both `terminal_id` and `inbox_token`, so stale global hook output was rejected as unsafe cross-session input.
- Resolution: Added managed-hook diagnostics in `src/app.rs` that inspect the user-global Factory settings and installed hook copy, warn when a legacy `-File` launcher or missing `inbox_token` support is detected, and log explicit reasons when inbox events are ignored for terminal-id or token mismatches. The installer-backed repair path remains the supported fix, and regression coverage now verifies the installer refreshes the copied hook script from the repo source.
- Prevent recurrence:
  - Keep `%USERPROFILE%\.factory\hooks\mergen-ade-droid-status.ps1` installer-managed; do not rely on an older manual copy.
  - Keep managed Factory hook commands on the canonical installer shape; do not hand-edit them back to `-File`.
  - When Diagnostics shows `Factory Droid hook repair needed`, rerun `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-factory-droid-hooks.ps1` from the repo root and restart Droid.
- Files/Commands touched: `src/app.rs`, `scripts/__tests__/factory-droid-hooks.tests.ps1`, `README.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `powershell -ExecutionPolicy Bypass -File .\scripts\__tests__\factory-droid-hooks.tests.ps1`
- References: local 2026-04-08 investigation of `%USERPROFILE%\.factory\settings.json`, `%USERPROFILE%\.factory\hooks\mergen-ade-droid-status.ps1`, and `%APPDATA%\mergen\MergenADE\config\runtime\factory-droid-hooks\*.jsonl`; Factory docs reviewed at `https://docs.factory.ai/reference/hooks-reference` and `https://docs.factory.ai/cli/configuration/settings`

#### Terminal Manager filter tabs only switched on label text clicks instead of the full slot area {#terminal-manager-filter-tabs-only-switched-on-label-text-clicks-instead-of-the-full-slot-area}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Terminal Manager foreground/background filter tabs
- Error signature: `Clicking empty space inside the Foreground or Background filter box did nothing unless the pointer was directly on the text label.`
- Symptoms/Impact: Terminal Manager filter switching felt inconsistent because the visible per-filter box implied a larger hit area than the code actually accepted. Users had to aim precisely at the label text instead of being able to click anywhere inside the owning slot.
- Root cause: `src/app.rs` attached `Sense::click()` only to the centered `Label` widget's `label_rect`, while the surrounding slot rectangle was layout-only and never registered pointer interaction.
- Resolution: The filter layout now retains both slot rects and label rects, the full slot rect handles hover/click interaction, the label remains centered and visual-only, and regression tests now cover blank-space clicks inside both filter slots plus the selected-slot no-op case.
- Prevent recurrence:
  - Bind Terminal Manager tab hit-testing to the same slot geometry used for layout, not just the text bounds.
  - Keep underline rendering tied to label bounds so click-area expansions do not accidentally change the visual design.
  - Add pointer-event regression tests whenever a control's visible affordance is larger than its text or icon content.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Terminal Manager filter hit-area bug; regression tests `terminal_manager_filter_clicking_blank_space_in_background_slot_switches_selection`, `terminal_manager_filter_clicking_blank_space_in_foreground_slot_switches_selection`, and `terminal_manager_filter_clicking_selected_slot_blank_space_is_no_op`

#### Terminal Manager filter follow-up fix restored blank-space clicks but broke direct label clicks {#terminal-manager-filter-follow-up-fix-restored-blank-space-clicks-but-broke-direct-label-clicks}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Terminal Manager foreground/background filter tabs after the initial slot hit-area expansion
- Error signature: `After blank-space clicks were fixed, clicking directly on the Foreground or Background text stopped switching the filter.`
- Symptoms/Impact: The visible tab text itself no longer acted like part of the tab control, so the filter only switched when the user clicked around the text instead of on it.
- Root cause: `src/app.rs` split interaction and painting into two overlapping widgets: the slot used `ui.interact(..., Sense::click())`, but the centered label was added afterward as a separate non-clickable widget on top of the text area, so text hits no longer flowed through the slot click response.
- Resolution: The filter tabs now keep the full-slot click response, make the label explicitly clickable and non-selectable, and merge the slot and label responses so both blank space and text activate the same filter behavior. Regression coverage now includes background/foreground label clicks and selected-label no-op behavior.
- Prevent recurrence:
  - When a control is visually composed from multiple overlapping widgets, merge their interaction responses instead of assuming the background response will catch topmost text hits.
  - Mark tab-like labels as non-selectable unless text selection is the intended UX.
  - Keep both blank-space and direct-text click regression tests for tab controls that mix layout rects with overlaid labels.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 follow-up user report after the slot-hit-area fix; regression tests `terminal_manager_filter_clicking_background_label_text_switches_selection`, `terminal_manager_filter_clicking_foreground_label_text_switches_selection`, and `terminal_manager_filter_clicking_selected_label_text_is_no_op`

#### Empty Terminal Manager project headers showed a hand cursor over non-expandable titles {#empty-terminal-manager-project-headers-showed-a-hand-cursor-over-non-expandable-titles}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Terminal Manager project headers when the selected foreground/background filter had no terminals under a project
- Error signature: `Hovering a project title still showed the hand cursor even though there was nothing to expand.`
- Symptoms/Impact: Empty project rows looked actionable in the wrong place. Users saw a link/button-style cursor over the title area, but clicking there could not expand anything because no terminal rows existed yet.
- Root cause: `src/app.rs` correctly limited the row-level header cursor to expandable projects, but `draw_terminal_manager_title_and_diff_summary` independently forced `PointingHand` on the overlaid title label, reintroducing a fake affordance on top of non-expandable rows.
- Resolution: The title helper now applies the hand cursor only when the project row can actually expand, the title label is rendered as non-selectable so empty rows fall back to the default cursor instead of text-selection affordances, and the inline spawn/action button keeps its own pointing-hand cursor. Regression tests now cover empty-title default cursor, expandable-title pointing hand, and empty-row action-button pointing hand.
- Prevent recurrence:
  - Keep project-header cursor semantics driven by actual available actions, not by shared title rendering helpers.
  - When a row mixes non-clickable text and clickable inline actions, test their cursor outputs separately.
  - Treat hover cursor changes as UX behavior that deserves regression coverage, especially when overlapping widgets are involved.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Terminal Manager empty-project cursor bug; regression tests `empty_project_title_hover_keeps_default_cursor`, `expandable_project_title_hover_uses_pointing_hand`, and `empty_project_action_button_still_uses_pointing_hand`

#### Terminal Manager terminal rows only activated on title text instead of the whole left row area {#terminal-manager-terminal-rows-only-activated-on-title-text-instead-of-the-whole-left-row-area}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Terminal Manager terminal rows with short labels
- Error signature: `Clicking the blank space inside a terminal row did nothing unless the pointer was directly on the terminal title text.`
- Symptoms/Impact: Short terminal titles were unnecessarily hard to target. The row chrome visually implied that the whole terminal row was selectable, but users had to click directly on the label text instead of anywhere in the row's left content area.
- Root cause: `src/app.rs` allocated a full-row clickable response for hover styling, but activation only listened to the overlaid title-label response. The empty space between a short title and the action buttons never participated in terminal selection.
- Resolution: Terminal rows now define a dedicated non-action selection rect that covers the title area plus the blank gap before the action cluster, merge that response with the title response, keep the hand cursor aligned to the same left-side hit area, and explicitly prevent right-side action-button clicks from also activating the terminal. Regression tests now cover blank-space selection, direct title selection, blank-space hover cursor, and action-button no-op activation behavior.
- Prevent recurrence:
  - Keep terminal-row hit-testing tied to the visible left content slot, not just the text widget.
  - When rows mix selection and inline actions, gate activation so action clicks cannot trigger both behaviors.
  - Add pointer regression tests whenever row affordance is visually wider than the label itself.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Terminal Manager terminal-row hit-area bug; regression tests `terminal_manager_row_clicking_blank_space_selects_terminal`, `terminal_manager_row_clicking_title_text_selects_terminal`, `terminal_manager_row_action_button_click_does_not_activate_terminal`, and `terminal_manager_row_hovering_blank_space_uses_pointing_hand`

#### Settings modal width could overflow the viewport on narrow windows {#settings-modal-width-could-overflow-the-viewport-on-narrow-windows}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal on narrow app windows and smaller laptop displays
- Error signature: `Settings opened too wide and extended past the visible screen area.`
- Symptoms/Impact: The Settings modal could render partially off-screen, making navigation and diagnostics awkward or inaccessible on narrower viewports.
- Root cause: `src/app.rs` sized the modal from `screen_rect - 48px` alone, kept a forced side-by-side nav/content layout, and left diagnostics/actions in always-wide layouts. The fixed window width did not account for window chrome margins, and narrow content could still pressure the modal horizontally.
- Resolution: The Settings modal now subtracts the current egui window margins when computing its fixed size, switches to a stacked navigation-above-content layout on narrow widths, collapses diagnostics to a single column when space is tight, and stacks Codex setup actions vertically instead of forcing a wide row. Regression tests now cover size calculation, layout breakpoints, and a narrow-viewport modal bounds check.
- Prevent recurrence:
  - Include current egui window margins in viewport-constrained modal size calculations.
  - When a modal has a sidebar-style navigation, add an explicit narrow-width stacked layout instead of relying on content shrinkage.
  - Add viewport-bounds tests for fixed-size popups that mix navigation, scroll areas, and wide diagnostic content.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Settings overflow bug; regression tests `settings_window_size_accounts_for_window_margin_on_small_screens`, `settings_popup_layout_switches_to_stacked_when_space_is_narrow`, `settings_diagnostics_layout_switches_to_single_column_when_space_is_narrow`, and `settings_popup_stays_within_narrow_viewport`

#### Settings modal body inherited the wrong layout and collapsed content into the middle of the window {#settings-modal-body-inherited-the-wrong-layout-and-collapsed-content-into-the-middle-of-the-window}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal after the narrow-window width clamp fix
- Error signature: `Settings no longer overflowed the viewport, but the inner content collapsed into the middle of the modal and left a large empty area above it.`
- Symptoms/Impact: The modal frame fit on screen, but the navigation and section controls looked broken: buttons lined up across the middle instead of top-to-bottom, the right pane appeared mostly empty, and Saved Messages became brittle on narrow widths.
- Root cause: `src/app.rs` bounded the modal body correctly but rendered the split layout with `body_ui.horizontal(...)`, so both inner frames inherited a left-to-right layout from the parent row. At the same time `draw_settings_section_panel()` forced a full-height section UI before measuring its own header and scroll area, which inflated blank space and pushed the actual content away from the top.
- Resolution: The settings nav and content frames now render inside explicit `top_down(Align::Min)` child layouts, the section panel no longer hard-sets its height before measuring content, the scroll area only consumes the remaining height after the section header, and Saved Messages switches to compact stacked rows on narrower widths. Regression coverage now includes the compact Saved Messages breakpoint in addition to the existing settings viewport/layout tests.
- Prevent recurrence:
  - Do not rely on `Frame::show(...)` inheriting the “right” layout when the parent UI is horizontal; explicitly set the child layout for each panel.
  - Avoid `ui.set_height(...)` on section containers before titles, descriptions, and scroll areas have been measured.
  - Add compact-row fallbacks for settings subsections that mix long text with inline action buttons.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 screenshot-backed user report after the initial settings width fix; regression tests `settings_saved_messages_layout_switches_to_compact_rows_when_space_is_narrow` and `settings_popup_stays_within_narrow_viewport`

#### Settings Prompts used stretched full-width rows and cramped text instead of compact wrapped prompt chips {#settings-prompts-used-stretched-full-width-rows-and-cramped-text-instead-of-compact-wrapped-prompt-chips}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Settings modal, `Prompts` section after the settings width/body fixes
- Error signature: `Saved prompts looked too wide, used undersized text, and still nested an extra manage-prompts disclosure instead of behaving like a compact accordion list.`
- Symptoms/Impact: The saved prompt area felt heavy and wasted horizontal space. Prompt text was harder to scan than necessary, long entries looked like stretched rows instead of reusable snippets, and the extra nested disclosure layer made the section feel more complex than it needed to be.
- Root cause: `src/app.rs` still rendered each saved prompt as a full-width row with truncation-first message labels and a secondary `Manage prompts` accordion inside an already grouped project card. The layout logic was width-threshold-based, so prompt rows stayed stretched even when a natural wrapped size would have fit better.
- Resolution: The `Prompts` section now uses project-level accordions directly, removes the extra `Manage prompts` wrapper, renders each saved prompt as a wrapped chip that sizes to its content up to a capped width, and bumps the prompt text size for readability while keeping add/remove/send behavior deferred until after render. Regression coverage now includes chip-width clamping, selected-project default-open behavior, and a long-prompt viewport bounds check.
- Prevent recurrence:
  - Prefer content-sized wrapped chips over full-width rows for reusable snippet/prompt collections.
  - Avoid stacking multiple disclosure layers when the outer project grouping can own the accordion interaction.
  - Add viewport-bounds coverage for long prompt text whenever Settings layout changes touch wrapped content.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported `Settings > Prompts` follow-up; regression tests `settings_saved_message_chip_width_clamps_to_available_space`, `settings_saved_messages_selected_project_starts_open`, and `settings_saved_messages_long_prompt_keeps_popup_within_viewport`

#### Codex running badge disappeared off-surface and Codex wait states looked like active work {#codex-running-badge-disappeared-off-surface-and-codex-wait-states-looked-like-active-work}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Codex CLI integration, especially when leaving the chat surface or returning to it mid-run
- Error signature: `The Codex spinner vanished when the chat surface changed, and plan/question wait states still looked like ongoing work.`
- Symptoms/Impact: Users could lose the only visible running indicator by switching away from the local terminal surface, then return later with no persistent signal that Codex was still working. When Codex stopped to ask a question or request plan approval, the UI still showed a spinner, so the interaction looked active instead of blocked on user input.
- Root cause: `src/app.rs` only rendered AI badges on terminal-local surfaces and the spinner path did not request periodic repaints, so the running indicator depended on those widgets staying mounted. Codex attention also reused the generic interaction-clears-attention path, so focus/click/chat navigation could acknowledge it without an actual reply. `src/codex.rs` only classified a narrow subset of notify payloads, which made attention-worthy Codex callbacks too easy to drop into an unhelpful fallback state.
- Resolution: The app now renders a persistent global AI badge in the activity rail while keeping the existing local badges, forces spinner repaints the same way pulse badges already did, and promotes Codex wait states to `Attention` with reason-aware pulse tooltips. Codex attention is no longer cleared by focus, click, or plain typing; it clears only after a real non-empty reply is committed with `Enter` or an equivalent saved-message send. Notify parsing now extracts `event`/`type`/`kind` names, maps approval and user-input requests explicitly, and treats any remaining routed Codex notify callback as attention via an `unknown-notify` fallback.
- Prevent recurrence:
  - Keep at least one always-mounted AI status surface for long-running integrations instead of relying only on terminal-local widgets.
  - Separate `Running` and `Waiting for user` states in the model and visuals; do not overload the spinner for blocked interactions.
  - For Codex attention, clear on actual submitted input, not generic focus or click events.
  - Preserve an attention fallback for notify payloads that reach Mergen even when the upstream event schema evolves.
- Files/Commands touched: `src/app.rs`, `src/codex.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Codex badge persistence and wait-state confusion; official Codex docs for `notify` payload routing and TUI notifications (`https://developers.openai.com/codex/config-reference#configtoml`, `https://developers.openai.com/codex/app-server#toolrequestuserinput`)

#### Codex question prompts could stay on spinner when notify was missing, and plan prompts were missing from the notify allow-list {#codex-question-prompts-could-stay-on-spinner-when-notify-was-missing-and-plan-prompts-were-missing-from-the-notify-allow-list}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Codex CLI integration with native TUI question prompts and plan mode approvals
- Error signature: `Codex showed Question 1/1 or waited on a plan decision, but Mergen kept the running spinner instead of switching to pulse.`
- Symptoms/Impact: Users could miss that Codex had already stopped for input. Question prompts triggered via the TUI footer (`tab to add notes | enter to submit answer | esc to interrupt`) looked like ongoing work when the upstream notify callback was absent, and plan-mode waits were easier to miss because `plan-mode-prompt` was not part of Mergen’s required Codex notification set.
- Root cause: `src/codex.rs` only required `agent-turn-complete`, `approval-requested`, and `user-input-requested` in `tui.notifications`, so plan-mode prompt events were not explicitly requested from Codex. Separately, `src/app.rs` only upgraded Codex raw PTY chunks to attention on `[bell]`, even though `src/terminal.rs` already had the right bounded-buffer, ANSI-normalized parsing pattern for visible wait-state chrome. That left native Codex question screens invisible to Mergen whenever notify did not arrive.
- Resolution: Mergen now adds `plan-mode-prompt` to the required Codex notification list and maps it to a dedicated Codex attention reason. The terminal reader also gained a Codex-specific visible prompt parser that detects the question UI chrome through a bounded ANSI-normalized buffer and emits a `codex-question-prompt` raw chunk only while a Codex session is active. `src/app.rs` promotes that fallback to `Attention` with a `VisibleUi` source, keeps `Notify` higher priority than the fallback, and treats interactive Codex attention reasons as satisfiable by an empty `Enter` submit so selection-based approvals/questions return to `Running` instead of sticking on pulse.
- Prevent recurrence:
  - Keep Codex `tui.notifications` in sync with all known user-blocking wait states, not just turn-complete and direct approvals.
  - Reuse the terminal-layer visible-status parser pattern for AI CLIs that render blocked UI directly in the PTY.
  - For selection-based CLI prompts, test both non-empty text replies and empty `Enter` submits.
- Files/Commands touched: `src/app.rs`, `src/codex.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 screenshot-backed Codex question prompt report; regression tests `codex_notify_inbox_maps_plan_mode_prompt_to_attention`, `visible_codex_question_prompt_sets_attention_reason`, `interactive_codex_attention_empty_enter_restores_running`, `sticky_interactive_codex_attention_is_cleared_by_empty_enter`, and `collect_ai_read_signals_emits_visible_codex_question_prompt_for_codex_sessions`

#### Codex interactive pulse could remain after the Codex child process had already exited {#codex-interactive-pulse-could-remain-after-the-codex-child-process-had-already-exited}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Codex CLI integration after `request_user_input`, approval waits, plan-mode waits, or other attention states that outlive the child-process check
- Error signature: `Mergen still showed a Codex pulse after the Codex session had already returned to the shell prompt or been interrupted with Ctrl+C.`
- Symptoms/Impact: Users could see a pulse that looked like Codex was still blocked on them even though the actual Codex child process was already gone. This was especially misleading after interactive question-tool flows because the answer screen could finish, the shell prompt could return, and Mergen still kept the pulse alive until the user sent more input.
- Root cause: `src/app.rs` treated every Codex `Attention` status the same when the tracked Codex child process disappeared after trailing grace. That branch only called `clear_codex_process_tracking()`, which intentionally preserved the sticky attention state regardless of whether the reason was a durable `TurnComplete` reminder or a process-bound interactive wait such as `UserInputRequested`, `ApprovalRequested`, or `PlanModePrompt`.
- Resolution: Codex attention cleanup on process disappearance is now reason-aware. After trailing grace, `TurnComplete` remains sticky, but interactive and generic attention reasons are fully cleared with the rest of the Codex session state. This covers notify-driven waits, visible-ui question prompts, and generic fallback attention that no longer has a live Codex child behind it.
- Prevent recurrence:
  - Distinguish sticky “you may continue later” reminders from process-bound “I am blocked right now” waits before preserving attention across process loss.
  - Add explicit process-exit regressions for each Codex attention reason instead of relying on one generic sticky-attention test.
  - Keep visible-ui and notify-backed Codex waits aligned so session exit clears both consistently.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Codex question-tool pulse persisting after shell prompt return; regression tests `codex_attention_stays_visible_after_process_exit_grace`, `notify_user_input_codex_attention_clears_after_process_exit_grace`, `approval_requested_codex_attention_clears_after_process_exit_grace`, `plan_mode_codex_attention_clears_after_process_exit_grace`, `unknown_codex_attention_clears_after_process_exit_grace`, and `visible_codex_question_prompt_clears_after_process_exit_grace`

#### Codex spinner could keep running after an `Esc` interrupt even though the active turn had already stopped {#codex-spinner-could-keep-running-after-an-esc-interrupt-even-though-the-active-turn-had-already-stopped}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Codex CLI integration during an in-flight turn that gets interrupted from the Codex TUI with `Esc`
- Error signature: `Codex showed the "Conversation interrupted ... /feedback" banner, but Mergen kept the running spinner visible.`
- Symptoms/Impact: Users could stop the current Codex turn, see the shell/TUI remain alive and ready for follow-up input, but still get a spinner that implied active work was continuing. This was especially confusing because the interrupt banner is a terminal-local UI state, not a process exit.
- Root cause: Mergen had no Codex-specific visible-ui detector for the interrupt banner, so the only post-submit running signal (`PromptSubmit`) remained in place until some later notify, question prompt, or process lifecycle change overwrote it. The existing visible parser only recognized the question prompt footer and intentionally ignored generic text.
- Resolution: The terminal-layer Codex visible-ui parser now recognizes the interrupt banner through a conservative multi-marker match (`conversation interrupted` plus the banner-specific follow-up/help text) and emits a dedicated raw chunk. `src/app.rs` maps that chunk to `AiCliStatus::Inactive` with `VisibleUi` as the source, which hides the spinner immediately without clearing the live Codex session or process tracking. Follow-up prompts and later question screens still work in the same session.
- Prevent recurrence:
  - Treat “turn stopped but session still alive” as a first-class Codex UI state distinct from both process exit and user-blocking attention.
  - Keep the interrupt parser stricter than the question parser; never key off the word `interrupt` alone because question prompts already include `esc to interrupt`.
  - Cover both `Running -> Inactive` and `Attention -> Inactive` interruption paths in regressions.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported `Esc` interrupt spinner persistence; regression tests `pending_visible_codex_status_detects_split_interrupted_banner_across_reads`, `running_codex_interrupt_banner_sets_inactive_without_clearing_session`, `attention_codex_interrupt_banner_sets_inactive_without_clearing_session`, and `codex_question_prompt_can_return_after_interrupt_banner_without_relaunch`

#### Factory Droid `Ask User` screens could keep the running spinner instead of switching to pulse {#factory-droid-ask-user-screens-could-keep-the-running-spinner-instead-of-switching-to-pulse}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows native Factory Droid integration when Droid renders the interactive `Ask User` TUI with `Q1`, selectable options, and `Enter Select / ESC cancel`
- Error signature: `Factory Droid showed an Ask User prompt, but Mergen kept the running spinner instead of a pulse and could clear that wait state too early on focus/input changes.`
- Symptoms/Impact: Users could mistake a blocked Droid question for ongoing work. Even after the `Ask User` screen appeared, changing focus back to the terminal or sending navigation keys like `Tab` could make the attention indicator disappear before a real answer or cancel action happened.
- Root cause: `src/terminal.rs` only recognized older visible Droid wait markers such as `HOOKS Stop`, `needs your permission`, and `waiting for your input`; the newer `Ask User / Q1 / Enter Select / ESC cancel` chrome was invisible to Mergen. Separately, `src/app.rs` treated every Factory Droid attention as generic, so focus acknowledgement and any terminal interaction could clear the state immediately.
- Resolution: Mergen now detects the visible `Ask User` UI through a conservative multi-marker parser in `src/terminal.rs` and emits a dedicated `droid-ask-user-prompt` raw chunk. `src/app.rs` maps that chunk to a dedicated Factory Droid attention reason, keeps that reason sticky across terminal focus changes, routes `Tab` and other prompt controls back into the terminal when UI text inputs own keyboard focus, restores `Running` only on real submit, clears to `Inactive` on `ESC` cancel, and drops the sticky pulse when the Droid session/process exits.
- Prevent recurrence:
  - Treat interactive visible Droid question screens as a first-class attention reason, not as a generic `Waiting for you` state.
  - Keep `Ask User` detection conservative by requiring multiple stable TUI markers instead of matching the title alone.
  - Test focus acknowledgement, empty-enter submit, `ESC` cancel, and process-exit cleanup independently for sticky interactive Droid waits.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo test`
- References: 2026-04-09 user-reported Factory Droid `Ask User` screenshot; regression tests `pending_visible_factory_status_detects_split_ask_user_prompt_across_reads`, `visible_droid_ask_user_chunk_sets_attention_reason`, `same_terminal_focus_keeps_factory_droid_ask_user_attention`, `factory_droid_ask_user_empty_enter_restores_running`, `factory_droid_ask_user_escape_clears_attention_without_running`, and `factory_droid_ask_user_process_exit_clears_attention`

#### Codex plan-mode prompts could stay on the running spinner, and completed-turn pulse reminders could remain sticky after acknowledgement {#codex-plan-mode-prompts-could-stay-on-the-running-spinner-and-completed-turn-pulse-reminders-could-remain-sticky-after-acknowledgement}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows local Codex CLI integration when Codex enters the visible `Implement this plan?` confirmation UI or when a finished turn leaves a `TurnComplete` pulse reminder behind
- Error signature: `Codex showed the plan approval prompt but Mergen sometimes kept the running spinner, and after work was done the pulse could keep blinking even after focusing/clicking/typing in the terminal.`
- Symptoms/Impact: Users could miss that Codex had paused on a plan approval screen because the badge still looked like active work. Separately, once a turn was already complete, the reminder pulse could stay visible longer than intended and keep demanding attention even after the user had clearly acknowledged the terminal with focus or keyboard input.
- Root cause: `src/terminal.rs` had no visible-ui detector for the native Codex plan approval chrome, so if the notify path was absent or late the prior `Running` state remained untouched. In `src/app.rs`, focus/input acknowledgement treated every Codex attention reason as sticky, so the durable `TurnComplete` reminder was preserved by the same policy used for genuinely interactive waits such as plan approval or question prompts.
- Resolution: Mergen now recognizes the visible `Implement this plan? / Yes, implement this plan / No, stay in Plan mode / Press enter to confirm or esc to go back` UI and maps it to `Attention + PlanModePrompt`, which forces the spinner to switch to pulse even without a matching notify event. Codex acknowledgement is also reason-aware: `TurnComplete` clears on terminal focus/click or real keyboard input, while interactive waits like `PlanModePrompt`, `ApprovalRequested`, and `UserInputRequested` remain sticky until a true reply/submit path happens.
- Prevent recurrence:
  - Keep visible-ui fallbacks for Codex waits whenever the CLI surfaces stable terminal chrome that may race with or outlive notify delivery.
  - Separate completion reminders from active user-blocking waits before deciding whether focus/input acknowledgement should clear a pulse.
  - Cover both live-session and already-exited-session `TurnComplete` acknowledgement paths in regressions.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Codex `Implement this plan?` spinner persistence and sticky completion pulse; regression tests `pending_visible_codex_status_detects_split_plan_mode_prompt_across_reads`, `visible_codex_plan_mode_prompt_sets_attention_reason`, `generic_notify_does_not_downgrade_visible_codex_plan_mode_prompt`, `turn_complete_codex_text_without_enter_keeps_live_session_but_clears_attention`, `sticky_turn_complete_codex_attention_text_without_enter_clears_state`, `turn_complete_codex_attention_same_terminal_focus_acknowledges`, and `plan_mode_codex_attention_same_terminal_focus_does_not_acknowledge`

#### Factory Droid spec-approval screens could keep the running spinner instead of switching to pulse {#factory-droid-spec-approval-screens-could-keep-the-running-spinner-instead-of-switching-to-pulse}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows native Factory Droid integration when Droid renders the visible `Propose Specification / Specification for approval` screen with numbered choices like `Proceed with the proposal`, `Proceed with comment`, and `No and explain why`
- Error signature: `Factory Droid showed the specification approval UI, but Mergen kept the running spinner instead of a pulse.`
- Symptoms/Impact: Users could miss that Droid had stopped for approval because the badge still implied ongoing work. If they interacted from a focused search/input field with prompt controls like `Ctrl+G`, that input could also risk clearing the wait state too early unless the screen was modeled as a real interactive attention reason.
- Root cause: `src/terminal.rs` only recognized the older visible `Ask User` question chrome and legacy stop/permission markers; it had no detector for the newer `Propose Specification / Specification for approval / Will save to: / Proceed with the proposal ... / ESC Cancel` screen. `src/app.rs` also only had one explicit interactive Droid attention reason (`AskUser`), so the approval UI had no dedicated sticky pulse behavior or tooltip even if its chrome had been detected.
- Resolution: Mergen now detects the visible Factory Droid specification approval chrome through a conservative multi-marker parser and emits a dedicated `droid-spec-approval-prompt` raw chunk. `src/app.rs` maps that chunk to a new `FactoryDroidAttentionReason::SpecificationApproval`, keeps it sticky across terminal focus changes, routes prompt-control input like `Ctrl+G` back into the terminal when UI text fields own focus, treats empty `Enter` as a real selection submit, clears on `ESC` cancel, and drops the pulse when the Droid session/process exits.
- Prevent recurrence:
  - Model visually distinct interactive Droid wait states as explicit reasons instead of collapsing everything into `AskUser` or generic attention.
  - Keep the spec-approval parser conservative by requiring multiple stable approval-screen markers rather than matching the title alone.
  - Cover split-read parsing, focus persistence, prompt-control input routing, and process-exit cleanup in regressions for each interactive Droid reason.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Factory Droid `Propose Specification` spinner persistence; regression tests `pending_visible_factory_status_detects_split_spec_approval_prompt_across_reads`, `visible_droid_spec_approval_chunk_sets_attention_reason`, `raw_input_hook_steals_ctrl_g_for_factory_droid_spec_approval_terminal`, `same_terminal_focus_keeps_factory_droid_spec_approval_attention`, `factory_droid_spec_approval_empty_enter_restores_running`, `factory_droid_spec_approval_escape_clears_attention_without_running`, and `factory_droid_spec_approval_process_exit_clears_attention`

#### Factory Droid spec-approval pulse could still miss live approval screens when the footer used arrow glyphs, and raw status chunks could drop under queue pressure {#factory-droid-spec-approval-pulse-could-still-miss-live-approval-screens-when-the-footer-used-arrow-glyphs-and-raw-status-chunks-could-drop-under-queue-pressure}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows native Factory Droid integration while Droid shows `Propose Specification / Specification for approval` with the live footer variant `↑/↓ Navigate • Enter Select • 1-4 Quick select • ctrl-g to edit plan • ESC Cancel`
- Error signature: `The Droid approval screen was visible, but Mergen sometimes kept the running spinner instead of switching to pulse.`
- Symptoms/Impact: Even after the earlier spec-approval fix, the live Droid approval screen could still stay on the spinner because the parser failed to recognize the footer variant actually rendered by Droid. Users also saw intermittent behavior because, under UI event queue pressure, the detected raw status chunk could be dropped before app state consumed it.
- Root cause: `src/terminal.rs` matched the footer too narrowly by requiring the literal substring `/ Navigate`, while the real UI used arrow glyphs and bullet separators. Separately, `send_ui_event()` treated every `AiRawChunk` as best-effort and silently dropped it on a full bounded queue, including stateful chunks like `droid-spec-approval-prompt`.
- Resolution: The spec-approval detector now matches semantic footer tokens instead of the exact ASCII `/ Navigate` chrome, so both the old and the live `↑/↓ Navigate` footer variants are accepted. `send_ui_event()` also now keeps `[bell]` best-effort but delivers stateful `AiRawChunk` events reliably, preventing prompt-state transitions from being lost under queue pressure.
- Prevent recurrence:
  - Prefer semantic marker matching for visible AI prompt chrome when the surrounding UI may use Unicode glyphs or alternate separators.
  - Keep noisy telemetry chunks like `[bell]` lossy, but never drop raw chunks that are required to transition visible wait-state UI.
  - Regress both live-footer parsing and queue-pressure delivery for any new visible prompt detector.
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-provided Factory Droid screenshot showing `↑/↓ Navigate • Enter Select • 1-4 Quick select • ctrl-g to edit plan • ESC Cancel`; regression tests `pending_visible_factory_status_detects_split_spec_approval_prompt_across_reads`, `pending_visible_factory_status_detects_spec_approval_prompt_with_ansi_and_crlf`, `collect_ai_read_signals_emits_visible_droid_spec_approval_prompt`, `send_ui_event_drops_bell_raw_chunk_when_queue_is_full`, and `send_ui_event_blocks_stateful_ai_raw_chunk_until_queue_has_capacity`

#### Factory Droid interactive prompt pulse could still be overwritten by later `Running` title or hook updates {#factory-droid-interactive-prompt-pulse-could-still-be-overwritten-by-later-running-title-or-hook-updates}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows native Factory Droid integration after an interactive visible wait state such as `Ask User` or `Propose Specification / Specification for approval` has already been recognized
- Error signature: `The interactive Droid prompt was visible, but a later running update switched the badge back to the spinner.`
- Symptoms/Impact: Even when Mergen successfully detected the visible Droid question or approval UI and entered pulse/attention state, the badge could flip back to the running spinner if Droid later emitted a `[Working...]` terminal title, a hook-derived running status, or an inbox-delivered running update. Users then still saw an active-work indicator instead of a blocked-on-user pulse.
- Root cause: `src/app.rs` had no Codex-style precedence guard for Factory Droid interactive attention. `apply_factory_droid_status()` accepted later `Running` updates from `TerminalTitle`, `PtyHookEvent`, and `Inbox` even while `FactoryDroidAttentionReason::AskUser` or `FactoryDroidAttentionReason::SpecificationApproval` was active, so the existing interactive wait state could be overwritten.
- Resolution: Mergen now preserves interactive Factory Droid attention against later `Running` updates from terminal-title, PTY-hook, and inbox sources. Real user submit/cancel paths still clear the pulse as before, but background `Running` telemetry no longer downgrades a visible `Ask User` or specification approval prompt.
- Prevent recurrence:
  - Treat interactive visible Droid wait states as higher-precedence than title/hook running telemetry until a real submit, cancel, or process exit occurs.
  - Keep regressions for both `Ask User` and specification approval flows whenever Factory Droid status precedence changes.
  - Include a control test showing that generic non-interactive Droid attention can still return to `Running`.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported “still spinner” regression after live Droid spec-approval detection; regression tests `terminal_title_running_does_not_downgrade_factory_droid_ask_user_attention`, `terminal_title_running_does_not_downgrade_factory_droid_spec_approval_attention`, `pty_hook_running_does_not_downgrade_factory_droid_spec_approval_attention`, `inbox_running_does_not_downgrade_factory_droid_spec_approval_attention`, and `terminal_title_running_can_downgrade_generic_factory_attention`

#### Factory Droid spec-approval pulse could still be missed on long approval screens, and active-process polling could clear sticky approval state {#factory-droid-spec-approval-pulse-could-still-be-missed-on-long-approval-screens-and-active-process-polling-could-clear-sticky-approval-state}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows native Factory Droid integration while Droid shows `Propose Specification / Specification for approval` and keeps the Droid process alive during the approval step
- Error signature: `The approval screen was visible, but Mergen still showed the running spinner instead of the approval pulse.`
- Symptoms/Impact: Users could land on a real Droid specification approval screen and still see a spinner if the approval chrome arrived across a long PTY split or if a later `[Working...]` title update arrived after process polling had already stripped the sticky approval reason. The badge then implied ongoing work instead of a blocked approval state.
- Root cause: `src/terminal.rs` kept only 512 characters of pending visible Droid status, so long `Propose Specification` screens could lose the header before the footer arrived in a later PTY read. The approval matcher also required intermediate footer chrome more strictly than necessary. Separately, `src/app.rs` cleared `factory_droid_attention_reason` whenever process polling confirmed the Droid process was still alive, which removed the sticky `SpecificationApproval` guard and let later running title updates downgrade the badge back to the spinner.
- Resolution: Mergen now keeps a larger visible-status window for Droid prompt parsing, recognizes approval screens from the stable header plus ordered choices and `Enter Select`/`ESC Cancel` footer markers, and preserves interactive Droid attention reasons when process polling only confirms that the process is still active.
- Prevent recurrence:
  - Size visible AI prompt buffers to cover at least a full terminal-screen worth of chrome, not just short hook lines.
  - Keep interactive approval reasons sticky across liveness polls so later running telemetry cannot erase them.
  - Regress both long split-read approval parsing and process-poll-plus-running-title sequences for Droid approval flows.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Factory Droid approval spinner persistence after `Propose Specification`; regression tests `pending_visible_factory_status_detects_spec_approval_prompt_with_minimal_footer`, `pending_visible_factory_status_detects_split_spec_approval_prompt_with_long_save_path`, and `factory_droid_process_poll_preserves_spec_approval_attention_before_running_title_update`

#### Factory Droid spec-approval pulse could still miss long specs after the header scrolled out of the PTY tail buffer {#factory-droid-spec-approval-pulse-could-still-miss-long-specs-after-the-header-scrolled-out-of-the-pty-tail-buffer}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows native Factory Droid integration while Droid renders a long specification body before the bottom approval choices stay visible
- Error signature: `The spec approval footer was on screen, but Mergen still kept the running spinner.`
- Symptoms/Impact: Even after the earlier spec-approval fixes, very long specs could still leave the badge on spinner because the top `Propose Specification / Specification for approval` header had already fallen out of the rolling PTY tail by the time the bottom choice block was rendered. Users saw the visible approval menu but not the pulse state.
- Root cause: `src/terminal.rs` still required the top approval markers for the only positive `droid-spec-approval-prompt` match. The rolling visible-status buffer held only the latest tail of PTY text, so long approval bodies could evict the header before the numbered choice/footer block arrived. With no emitted raw chunk, `src/app.rs` never entered `SpecificationApproval`.
- Resolution: Mergen now keeps the full-header matcher, but adds a footer-signature fallback that recognizes the stable numbered approval choices plus `Enter Select` and `ESC Cancel` even when the top header/body has already scrolled out of the tail buffer. Existing sticky-attention behavior continues to prevent passive running telemetry from switching the pulse back to spinner afterward.
- Prevent recurrence:
  - Treat long approval bodies as normal and avoid requiring top-of-screen markers when the bottom interactive menu is the only stable visible chrome left.
  - Regress footer-only approval detection and oversized-body PTY tail eviction separately.
  - Keep spinner-to-pulse regressions tied to emitted `droid-spec-approval-prompt` signals, not just app-side state updates.
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-provided Factory Droid approval screen showing the bottom numbered choice menu; regression tests `pending_visible_factory_status_detects_spec_approval_prompt_from_footer_only` and `pending_visible_factory_status_detects_spec_approval_prompt_after_oversized_body`

#### Foreground terminal creation could still open an empty shell instead of a chosen launcher {#foreground-terminal-creation-could-still-open-an-empty-shell-instead-of-a-chosen-launcher}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows terminal manager foreground creation flow after Codex/Droid support had been added but before launcher profiles existed
- Error signature: `Clicking the foreground new-terminal control opened a blank shell with no launcher selection.`
- Symptoms/Impact: Users had to open a foreground terminal first and then manually type `codex`, `claude`, `droid`, `opencode`, or a custom alias. That added friction, made alias customization impossible, and prevented the foreground create action from matching the intended launcher-first workflow.
- Root cause: `src/app.rs` only had a single direct foreground spawn path through `draw_project_group_header()` and `spawn_terminal_for_project()`. There was no persisted launcher catalog in config, no settings editor for launcher aliases, and no launcher-aware spawn path that could inject a configured command on terminal creation.
- Resolution: Mergen now stores a launcher catalog in config, exposes a dedicated Settings > Launchers editor, opens a launcher menu for foreground creation, auto-submits the selected command, and keeps Codex/Droid launch-pending detection working when users replace the default command with an alias such as `cc`.
- Prevent recurrence:
  - Keep foreground creation launcher-driven unless a deliberate raw-shell option is added later.
  - Regress config recovery for pending launcher edits and alias-based Codex/Droid detection.
  - Keep background creation behavior covered separately so launcher-only rules do not accidentally change raw background shells.
- Files/Commands touched: `src/app.rs`, `src/config.rs`, `src/models.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user request for Codex/Claude/Droid/OpenCode foreground launcher profiles with editable aliases; regression tests `route_active_terminal_input_uses_codex_launcher_alias_from_settings`, `recover_config_state_preserves_pending_launcher_changes`, `missing_launchers_field_restores_default_builtins`, and `save_and_load_preserves_launcher_command_edits`

#### Codex and Droid launcher icons could still show the wrong brand variants after the first branded-icon rollout {#codex-and-droid-launcher-icons-could-still-show-the-wrong-brand-variants-after-the-first-branded-icon-rollout}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows launcher menu and Settings > Launchers after built-in branded assets were introduced for Codex, Claude, Droid, and OpenCode
- Error signature: `Claude and OpenCode looked correct, but Codex was not purple and Droid did not use the expected Factory symbol.`
- Symptoms/Impact: The built-in launcher menu looked inconsistent with the actual products. Codex appeared as the wrong brand treatment, and Droid used a weaker/non-canonical mark, which made the built-in choices feel unfinished even though launcher behavior itself worked.
- Root cause: The first asset pass used the wrong source variants for Codex and Droid. Codex was not backed by the official purple Codex product icon, and Droid was not using the Factory `mobile-nav-logo` symbol from the official site.
- Resolution: Mergen now sources Codex from the official transparent Codex product app icon asset and renders Droid from the official Factory `mobile-nav-logo` SVG. The monogram fallback palette was also updated so decode failures still resemble the intended brands more closely.
- Prevent recurrence:
  - Validate product-specific icon variants with the user when a company brand page exposes both parent-brand and product-brand assets.
  - Prefer official vector/site symbols over favicons when a built-in launcher is meant to represent a specific product.
  - Keep launcher icon review separate from launcher behavior review so branding regressions are caught early.
- Files/Commands touched: `assets/launcher-icons`, `build.rs`, `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Codex purple-icon mismatch and Droid symbol mismatch after the branded launcher rollout

#### Droid launcher icon could still use the wrong color variant after switching to the correct Factory symbol {#droid-launcher-icon-could-still-use-the-wrong-color-variant-after-switching-to-the-correct-factory-symbol}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows launcher menu and Settings > Launchers after Droid switched from a non-canonical asset to the Factory `mobile-nav-logo` symbol
- Error signature: `The Droid symbol shape was correct, but the icon still appeared orange instead of white.`
- Symptoms/Impact: The built-in Droid launcher still looked off against the rest of the dark launcher UI because the chosen symbol color did not match the expected monochrome treatment. Users could read it as a partially fixed icon rollout rather than a finished branded surface.
- Root cause: The adopted `mobile-nav-logo` SVG was pinned to an orange fill instead of the intended white-on-dark treatment for this surface, and the Droid fallback badge palette still reinforced the orange variant.
- Resolution: Mergen now renders the Droid symbol as white on transparent background and updates the Droid fallback badge palette to a neutral dark slot with white foreground treatment.
- Prevent recurrence:
  - Confirm both shape and color variant when locking a product icon for a dark themed launcher surface.
  - Keep fallback badge colors aligned with the chosen asset treatment so decode failures do not reintroduce a rejected variant.
  - Re-review launcher icons in both the menu and settings rows after brand-asset swaps.
- Files/Commands touched: `assets/launcher-icons/droid.svg`, `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported Droid white-icon mismatch after the Factory symbol fix

#### Foreground launcher menu rows could still show the default cursor even though the whole row was clickable {#foreground-launcher-menu-rows-could-still-show-the-default-cursor-even-though-the-whole-row-was-clickable}
- Date: 2026-04-09T00:00:00Z
- Context: main/Windows foreground launcher creation menu after launcher profiles replaced empty-shell foreground creation
- Error signature: `Hovering a launcher row did not show a pointer cursor, even though clicking anywhere on the row selected the launcher.`
- Symptoms/Impact: The launcher menu felt less obviously interactive because the row behaved like a button but the cursor still looked passive. Users had weaker hover affordance on the most common foreground-terminal creation path.
- Root cause: `src/app.rs` made each launcher row clickable with `ui.interact(..., Sense::click())`, but did not attach hover cursor behavior to that row response.
- Resolution: Mergen now applies `PointingHand` to each clickable launcher row in the foreground launcher menu, matching the rest of the app's clickable list surfaces.
- Prevent recurrence:
  - Whenever a whole list row is clickable through `ui.interact`, pair it with explicit hover cursor behavior.
  - Re-check cursor affordance after converting button-based flows into full-row click targets.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-09 user-reported launcher menu hover cursor mismatch

#### Codex could flip from running spinner to pulse on a stray BEL or title-driven idle update {#codex-could-flip-from-running-spinner-to-pulse-on-a-stray-bel-or-title-driven-idle-update}
- Date: 2026-04-10T00:00:00Z
- Context: main/Windows local Codex CLI integration while an active Codex turn is still running
- Error signature: `Codex was still working, but the Mergen badge switched from spinner to pulse.`
- Symptoms/Impact: Users could misread an active Codex turn as blocked on input, especially when no real prompt or approval screen was visible. This made the activity rail and terminal badges unreliable during long-running Codex work.
- Root cause: `src/app.rs` treated any Codex raw PTY chunk containing `[bell]` as `Attention`, even though `src/terminal.rs` emits `[bell]` for every non-title BEL and not only for actionable Codex waits. Separately, the generic `AiStatusChange` branch accepted title-derived Codex status changes directly, so a transient idle-like title update could also overwrite a live running session.
- Resolution: Codex now ignores bare BEL chunks for badge-state transitions and also ignores title-derived `AiStatusChange` updates. Codex pulse states remain driven by explicit visible Codex wait UI, notify inbox events, and interrupt-banner handling instead of generic terminal noise.
- Prevent recurrence:
  - Keep generic BEL handling lossy for Codex unless it is paired with a concrete wait-state reason.
  - Do not let title-derived Codex status updates override explicit process- and prompt-driven state.
  - Add regression coverage for non-authoritative terminal noise during active Codex runs.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-10 user-reported Codex spinner-to-pulse regression; regression tests `codex_running_spinner_is_not_downgraded_by_bell_chunk` and `codex_title_attention_update_does_not_override_active_running_session`

#### Ctrl+V and right-click paste not working properly in OpenCode sessions {#ctrl-v-and-right-click-paste-not-working-properly-in-opencode-sessions}
- Date: 2026-04-13T00:00:00Z
- Context: main/Windows local terminal paste path with OpenCode CLI/readline-style TUIs
- Error signature: `Ctrl+V ve sag tik yapistir opencode uzerinde calismiyor, diger terminallerde calisiyordu.`
- Symptoms/Impact: OpenCode gibi readline-tabanli TUI oturumlarinda Ctrl+V ve sag tik yapistirma calismiyordu. Bu terminalde kontrol karakterleri (^V) veya canli newline akisi olarak iletiliyordu.
- Root cause: `src/app.rs` icindeki `key_to_terminal_bytes()` fonksiyonu Ctrl+V'yi her zaman kontrol karakteri `0x16` (`^V`) olarak terminal'e gonderiyordu. Ancak semantic `Event::Paste` olayi ayni batch icinde de geldiginde, bu cift yapistirma veya karisik davranisla sonuclaniyordu. Ayrica sag tik secim menusunde secim varken sadece Copy secenegi gorunuyordu.
- Resolution: 
  1. `normalize_terminal_clipboard_events()` fonksiyonu eklendi. Ayni input batch'i icinde `Event::Paste` varsa, ham `Ctrl+V`/`Cmd+V`/`Shift+Insert` tuslarini filtreleyerek cift yapistirma onleniyor.
  2. Semantic paste olayi yoksa, ham kontrol karakteri davranisi korunuyor (geriye uyumluluk).
  3. Sag tik secim menusune `Paste` secenegi eklendi; artik secim varken de Copy+Paste birlikte erisilebilir.
  4. `icons::PASTE` iconu ve `AppIcon::Paste` eklendi.
- Prevent recurrence:
  - Klavye kisayollarinin semantik olaylarla cakismasini onlemek icin batch-bazli normalizasyon kullan.
  - Paste yolunu bracketed paste uzerinden koru; karisikliga neden olan key stream birlestirmelerinden kacin.
  - Sag tik menude secim varken de Paste erisimini koru.
  - Yeni paste davranisi icin platform farkliliklarini (Ctrl vs Cmd) test et.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-13 user-reported paste issue; regression tests `normalize_terminal_clipboard_events_*` (7 tests)

#### Check-list panel empty-state behavior fixed to avoid unnecessary open-on-startup {#check-list-panel-empty-state-behavior-fixed-to-avoid-unnecessary-open-on-startup}
- Date: 2026-04-22T00:00:00Z
- Context: main/Windows startup UX and checklist panel auto-collapse
- Error signature: `Check-list sagdaki panel icerisinde veri yoksa otomatik olarak acik gelmesin ve mergen ade kapanip acilsa bile icindeki verilerin korunmasi lazim`
- Symptoms/Impact: Checklist panel was opening automatically on every startup even when empty, creating visual noise. However, checklist data was already persisted correctly across restarts.
- Root cause: The `checklist_panel_expanded` UI flag was persisted and restored without checking whether any checklist items actually existed. The data persistence (checklist items per project) was already working correctly.
- Resolution: 
  1. Added `has_any_checklist_items()` helper to detect if any project has checklist items.
  2. On startup (`bootstrap()`), collapse the panel if no checklist items exist.
  3. In config recovery (`recover_config_state()`), also collapse if empty after merge.
  4. When the last checklist item is removed (both from history popup and side panel), auto-collapse the panel immediately.
  5. Adding items does NOT auto-open the panel; user must explicitly toggle it.
- Prevent recurrence:
  - Always validate persisted UI state against actual data before applying on startup.
  - Keep data persistence (checklist items) separate from view state (panel open/closed).
  - Add regression tests for both startup collapse and runtime auto-close behaviors.
- Files/Commands touched: `src/app.rs`, `src/config.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: 2026-04-22 user request; regression tests `recover_config_collapses_empty_checklist_panel`, `recover_config_keeps_checklist_panel_when_items_exist`, `checklist_panel_closes_when_last_item_removed`, `save_and_load_preserves_project_checklist`


