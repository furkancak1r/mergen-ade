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
- `cargo build --release`: default local production build using the repo target configuration.
- `cargo build --release --target x86_64-pc-windows-msvc`: supported Windows release binary (`target/x86_64-pc-windows-msvc/release/mergen-ade.exe`).
- `cargo build --release --target aarch64-apple-darwin`: native macOS build used by the release workflow before signing/notarization packaging.
- `cargo run --release`: run optimized build locally.
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
