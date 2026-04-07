### Known Issues & Fix Log

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
