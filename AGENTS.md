# Repository Guidelines

## Project Structure & Module Organization
- `src/main.rs`: app entrypoint and native window startup.
- `src/app.rs`: UI composition (top bar, sidebars, terminal manager, main tiled panes) and app state flow.
- `src/terminal.rs`: terminal runtime, PTY integration, event forwarding, snapshot rendering data.
- `src/layout.rs`: auto-tiling grid math and related unit tests.
- `src/title.rs`: terminal title update/truncation logic and unit tests.
- `src/config.rs` + `src/models.rs`: persisted TOML config schema and load/save behavior.
- `.github/workflows/release.yml`: Windows release build/publish pipeline.
- Build artifacts are in `target/` (do not commit).

## Build, Test, and Development Commands
- `cargo build --release`: production build (`target/x86_64-pc-windows-gnullvm/release/mergen-ade.exe`).
- `cargo run --release`: run optimized build locally.
- `cargo test`: run unit tests (layout, title, terminal helpers).
- `cargo fmt`: format Rust sources before commit.

If `cargo` is not on PATH in PowerShell, use:
`$env:USERPROFILE\.cargo\bin\cargo.exe <command>`.

## Mandatory EXE Refresh After Every Change
- After any code update, always regenerate the release EXE before handing off work.
- Canonical EXE path: `target/x86_64-pc-windows-gnullvm/release/mergen-ade.exe`.
- Required runtime DLL for local launch: `target/x86_64-pc-windows-gnullvm/release/libunwind.dll`.
- Update steps (PowerShell):
  1. `cargo clean` (optional but recommended for suspicious stale outputs).
  2. `cargo build --release`.
  3. Copy runtime DLL if missing: `Copy-Item .\\.toolchain\\llvm-mingw-20260224-ucrt-x86_64\\x86_64-w64-mingw32\\bin\\libunwind.dll .\\target\\x86_64-pc-windows-gnullvm\\release\\libunwind.dll -Force`.
  4. Verify timestamps: `Get-Item .\\target\\x86_64-pc-windows-gnullvm\\release\\mergen-ade.exe, .\\target\\x86_64-pc-windows-gnullvm\\release\\libunwind.dll`.
  5. Launch and smoke-test: `Start-Process .\\target\\x86_64-pc-windows-gnullvm\\release\\mergen-ade.exe`.

## Coding Style & Naming Conventions
- Rust 2021, 4-space indentation, UTF-8, LF/CRLF handled by Git.
- Keep modules focused; prefer small functions over large mixed-responsibility blocks.
- Naming: `snake_case` for functions/modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Avoid heavy dependencies; preserve the low-memory, native-first design.
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
  4. Any Windows-specific assumptions or limitations.

## Security & Configuration Notes
- Do not commit local paths, secrets, or generated executables.
- Config is user-local in `%APPDATA%` via `ProjectDirs`; treat it as runtime data, not source-controlled state.

## Subagent Usage Policy
- For any non-trivial implementation, debugging, or review task, use subagents instead of running everything in a single agent.
- When work can be split safely, delegate independent parts in parallel (for example: `explorer` for discovery, `fast_code` for implementation, `test` for verification, `reviewer` for risk checks).
- Respect the configured concurrency limit and do not exceed 4 parallel subagent threads.
- Keep urgent critical-path edits local only when delegation would block progress; otherwise prefer delegation first.
- In handoff/final notes, summarize which subagents were used and what each one produced.
