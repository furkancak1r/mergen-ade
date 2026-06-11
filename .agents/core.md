# Core Project Rules

## Critical Rules - DO NOT BREAK

### Never Kill Running Mergen ADE Processes
- **ABSOLUTE RULE**: Never terminate, stop, or kill any running `mergen-ade` process, especially if it's running on the desktop.
- This applies to ALL automation, scripts, builds, and agent actions.
- If a new binary needs testing, ask the user to manually restart the application.
- The user explicitly owns the lifecycle of the desktop application instance.
- Violation of this rule can cause data loss, interrupted workflows, and broken user trust.

## Project Structure & Module Organization
- `electron/`: primary Electron application, with `main/`, `preload/`, `renderer/`, and `shared/` TypeScript code.
- `.github/workflows/release.yml`: GitHub release pipeline for Electron Windows ZIP and signed/notarized macOS ARM64 DMG assets.
- Build artifacts are in `electron/out/`, `electron/renderer/dist/`, and `electron/renderer/dist-electron/` (do not commit).
- **Do not watch the GitHub release workflow via CLI.** Once the release is triggered by pushing a version tag, the workflow runs asynchronously on GitHub Actions. There is no need to poll or watch it with `gh run watch` / `gh run list`; the user can track progress through the GitHub web interface if desired.

## Build, Test, and Development Commands
- `cd electron && npm ci`: install primary Electron dependencies.
- `cd electron && npx vitest run`: run Electron renderer/main/shared unit tests.
- `cd electron && npm run build`: build and package the primary Electron app.

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

## Runtime Mojibake (Character Encoding) Repair

- **Two repair modes**: `repair_mojibake_path` (disk-aware, returns repaired path only when it actually exists) vs `repair_mojibake_display` (always repairs for user-facing text).
- **Paths use disk-existence gating**: Use `repair_mojibake_path` for filesystem operations (paths, directory nodes, project records). The repaired path is only returned when it exists on disk to prevent false repair of genuinely oddly-named paths.
- **Display text uses unconditional repair**: Use `repair_mojibake_display` for user-facing strings (branch labels, file names, tooltips, attachment mentions). Always apply recovery regardless of disk existence.
- **CP1252→UTF-8 decode chain**: `repair_mojibake` iterates up to 5 rounds of treating bytes as CP1252 then decoding as UTF-8. Bail when repair yields no change or produces invalid UTF-8.
- **Coverage points**: Add mojibake repair at every text boundary where user-visible strings enter the system:
  - Directory node names (`build_directory_root_node`, `build_directory_node`, `build_directory_node_from_entry`)
  - File editor `display_name` in `open_file_in_editor`
  - ACP attachment paths and `path_to_mention` for `@file_name` mentions
  - Smart Input image attachment paths
  - Clipboard image paths (both HDROP and text-fallback)
  - Worktree `display_label()` and `discover_worktrees()` path repair
  - Config-level project records via `repair_mojibake_in_projects`
- **Non-UTF-8 path segments**: `repair_mojibake_path` converts paths to strings via `to_string_lossy()`, replacing non-UTF-8 segments with `U+FFFD`. This is a known limitation unlikely to matter in practice (Windows paths with NTFS are generally UTF-16 clean).

## Cross-Tool Project Configuration
- When a project is used in both Mergen and Zed, keep terminal commands in sync.
- Mergen `ProjectRecord::saved_messages` should be mirrored as `.zed/tasks.json` entries so the same commands are available in Zed's task picker.
- Zed tasks use `$ZED_WORKTREE_ROOT` for `cwd` and should be placed in the project root under `.zed/tasks.json`.
- MCP servers configured in OpenCode (`~/.config/opencode/opencode.json`) should also be registered in Zed (`context_servers` in Zed settings) so agents in both environments share the same tool set.
- Do not add Mergen-specific MCPs (e.g., `mergen-browser`) to Zed; use standard MCPs such as Playwright for browser automation in Zed.
- **Custom agent commands / skills**: When using OpenCode via ACP in Zed, custom slash commands should be defined as OpenCode commands (`~/.config/opencode/commands/*.md`) with frontmatter (`name`, `description`). The custom ACP adapter forwards these to Zed as `available_commands_update` so they appear when typing `/` in the Agent Panel.
- **Zed skills directory**: For Zed-native agent skill discovery, place `SKILL.md` files under `.zed/skills/<skill-name>/SKILL.md`. This mirrors the open SKILL.md standard and keeps project-specific agent instructions version-controlled.
