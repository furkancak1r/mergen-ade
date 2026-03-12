### Known Issues & Fix Log

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
