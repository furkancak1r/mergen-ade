### Known Issues & Fix Log

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
