# Repository Guidelines

> This file is a modular index. Detailed guidelines live under `.agents/`.

## Critical Rules - DO NOT BREAK

### Never Kill Running Mergen ADE Processes
- **ABSOLUTE RULE**: Never terminate, stop, or kill any running `mergen-ade` process, especially if it's running on the desktop.
- This applies to ALL automation, scripts, builds, and agent actions.
- If a new binary needs testing, ask the user to manually restart the application.
- The user explicitly owns the lifecycle of the desktop application instance.
- Violation of this rule can cause data loss, interrupted workflows, and broken user trust.

## Project Structure & Module Organization
- `electron/`: primary Electron application, with `main/`, `preload/`, `renderer/`, and `shared/` TypeScript code.
- `electron/BUILD.md`: detailed build configuration, troubleshooting, and macOS signing info.
- `.github/workflows/release.yml`: GitHub release pipeline for Electron Windows portable EXE and signed/notarized macOS ARM64 DMG assets.
- Build artifacts are in `electron/out/`, `electron/renderer/dist/`, and `electron/renderer/dist-electron/` (do not commit).
- **Do not watch the GitHub release workflow via CLI.** Once the release is triggered by pushing a version tag, the workflow runs asynchronously on GitHub Actions. There is no need to poll or watch it with `gh run watch` / `gh run list`; the user can track progress through the GitHub web interface if desired.

## Build, Test, and Development Commands
- `cd electron && npm ci`: install primary Electron dependencies.
- `cd electron && npx vitest run`: run Electron renderer/main/shared unit tests.
- `cd electron && npm run build`: build and package the primary Electron app. Produces single portable EXE at `electron/out/mergen-ade-<version>-windows-x64-portable.exe`.

## Coding Style & Naming Conventions
- TypeScript, UTF-8, LF/CRLF handled by Git.
- Keep modules focused; prefer small functions over large mixed-responsibility blocks.
- Naming: `camelCase` for functions/variables, `PascalCase` for types/interfaces, `SCREAMING_SNAKE_CASE` for constants.
- Keep UI controls visually lightweight; prefer minimal icon-first interactions over heavy bordered button chrome unless emphasis is required.

## Testing Guidelines
- Use Vitest for unit tests in the Electron codebase.
- Test behavior, not implementation details.
- Prefer descriptive test names like `wide_viewport_prefers_more_columns`.
- Minimum expectation for feature changes:
  1. Update/add tests in affected modules.
  2. Ensure `cd electron && npx vitest run` passes for Electron changes.

## Commit & Pull Request Guidelines
- Follow existing history style: short, imperative subject lines (examples: `Fix terminal input focus`, `Add release workflow`).
- Keep commits scoped to one concern when possible.
- PRs should include:
  1. What changed and why.
  2. Validation steps (`cd electron && npx vitest run`, manual run notes).
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

## Concurrent AI Sessions
- Sen (AI agent) çalışırken başka bir AI agent'ı da aynı anda çalışıyor olabilir.
- Eğer dosyalarda veya kodda başka birinin yaptığı değişiklikleri fark edersen, bu değişikliklere müdahale etme.
- Kendi işlemlerine devam et; başkasının yaptığı değişiklikleri değiştirme, silme veya üzerine yazma.
- Çakışma olursa kullanıcıya danış; tek başına karar verip başkasının işini bozma.

---

## Detailed Guidelines (modular)

All detailed implementation guidelines are split into topic files under `.agents/`:

- [.agents/core.md](.agents/core.md) — Critical rules, project structure, build commands, coding style, testing, commits, security, mojibake repair, cross-tool config
- [.agents/directory-indexing.md](.agents/directory-indexing.md) — Directory indexing performance, deferred loading, search, icons
- [.agents/ai-cli.md](.agents/ai-cli.md) — AI CLI integration (Droid, Codex, OpenCode, Claude), ACP protocol, ACP standby, ACP/terminal coexistence
- [.agents/terminal.md](.agents/terminal.md) — Hover/tooltip, terminal input/history, selection, terminal manager, saved messages, shortcuts
- [.agents/smart-input.md](.agents/smart-input.md) — OpenCode Smart Input (queue, dispatch, attachments, questions, auto-run)
- [.agents/worktree.md](.agents/worktree.md) — Git worktree integration, source control search
- [.agents/browser-panel.md](.agents/browser-panel.md) — Embedded browser panel (WebView2, lifecycle, project-scoped state)
- [.agents/browser-mcp.md](.agents/browser-mcp.md) — Browser MCP (z-order, performance, compact UI, tabs, single-binary, multi-terminal isolation, highlight, design inspect)
- [.agents/file-editor.md](.agents/file-editor.md) — File editor (scroll, selection, context menu, copy)
- [.agents/checklist.md](.agents/checklist.md) — Check-list panel
- [.agents/acp-ui.md](.agents/acp-ui.md) — OpenCode ACP UI (composer, capsule, attachments, model selector, mode toggle)
- [.agents/ui-overlay.md](.agents/ui-overlay.md) — Window close, clipboard paste, resizable panels, OS notifications, popup/overlay
- [.agents/claude-code.md](.agents/claude-code.md) — Claude Code configuration (Mimo, hooks, auth, Windows compat)
