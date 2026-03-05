### Known Issues & Fix Log

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

#### Release EXE fails to start: missing libunwind.dll {#release-exe-fails-to-start-missing-libunwind-dll}
- Date: 2026-03-05T07:57:34Z
- Context: main/Windows local PowerShell/cargo 1.93.1 (host x86_64-pc-windows-gnullvm)
- Error signature: `libunwind.dll bulamıyor`
- Symptoms/Impact: `target\\x86_64-pc-windows-gnullvm\\release\\mergen-ade.exe` açılırken uygulama başlatılamıyor.
- Root cause: LLVM-MinGW hedefinde gerekli çalışma zamanı DLL'i (`libunwind.dll`) EXE ile aynı klasöre taşınmadığı için dinamik bağlama başarısız oldu.
- Resolution: `x86_64-w64-mingw32\\bin\\libunwind.dll` dosyası release EXE klasörüne kopyalanarak çalışma doğrulandı (`RUNNING PID=37512`), commit/PR referansı henüz yok (local fix).
- Prevent recurrence:
  - Dağıtım paketine `libunwind.dll` dahil edildiğini build sonrası otomatik kontrol et.
  - Dağıtım doğrulamasında temiz makine/shell üzerinde EXE açılış testi çalıştır.
  - Gerekli DLL bağımlılıklarını `llvm-objdump -p <exe>` çıktısından CI adımıyla denetle.
- Files/Commands touched: `target\\x86_64-pc-windows-gnullvm\\release\\libunwind.dll`, `.toolchain\\llvm-mingw-20260224-ucrt-x86_64\\x86_64-w64-mingw32\\bin\\libunwind.dll`, `llvm-objdump -p`, `Copy-Item`, `Start-Process`
- References: local workspace fix (commit pending), baseline commit `0b3794b`
