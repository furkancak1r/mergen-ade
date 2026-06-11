# Claude Code Configuration Guidelines

## Claude Code Configuration

- **Config location**: Claude Code runtime settings are stored in **global** `~/.claude/settings.json` so they apply regardless of which directory Claude Code is launched from. Project-local `.claude/settings.local.json` can still override for specific projects, but the global file is the primary source of truth.
- **Build model**: Claude Code Anthropic model overrides (`ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`) must point to `mimo-v2.5-pro`.
- **Base URL**: `ANTHROPIC_BASE_URL` must be `https://token-plan-sgp.xiaomimimo.com/anthropic`. Do not route Claude Code runtime through Fireworks/Kimi or MiniMax wrappers.
- **API key helper**: `apiKeyHelper` resolves the Mimo API key from `MIMO_API_KEY` or the local desktop `key.txt` fallback. No key is hardcoded or committed.
- **Hook cleanup**: Keep the Codex Claude Code plugin configuration when present, but remove stale Orca and Emdash command hooks from `~/.claude/settings.json`; those hooks can route events to unavailable sidecars and break Claude startup.
- **Auto-updater disable**: Set `"DISABLE_AUTOUPDATER": "1"` in the settings `env` block to suppress background auto-update checks. If the update footer still appears, also add `"DISABLE_UPDATES": "1"` to block all update paths including manual `claude update`.
- **Windows compatibility**: The `apiKeyHelper` must use a `.cmd` wrapper on Windows (`mimo-key-helper.cmd`) because Claude Code spawns commands through `cmd.exe` which does not recognize extension-less scripts.
- **Auth conflict**: `ANTHROPIC_AUTH_TOKEN` env var must be unset when using `apiKeyHelper`. If both are present, Claude Code warns and may behave unexpectedly. Remove the token from the user environment and current shell before starting Claude Code. In PowerShell:
  ```powershell
  [Environment]::SetEnvironmentVariable("ANTHROPIC_AUTH_TOKEN", $null, "User")
  Remove-Item Env:ANTHROPIC_AUTH_TOKEN -ErrorAction SilentlyContinue
  ```
- **Shell alias conflict**: If the shell profile or `cc.cmd` defines a wrapper (for example MiniMax), that wrapper may set `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`, or `ANTHROPIC_MODEL` on every invocation and override the Mimo config. The Claude builtin launcher must bypass aliases/wrappers and directly invoke the real npm-installed `claude.cmd`.
- **Mergen terminal sanitization**: Mergen must remove stale Anthropic env vars (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`) from every spawned terminal via `CommandBuilder::env_remove`. Additionally, the Claude builtin launcher must send a sanitized command that bypasses shell aliases and directly invokes the real npm-installed `claude.cmd`.
- **Auto-update failures**: If Claude Code prints "Auto-update failed", reinstall the global npm package:
  ```powershell
  npm i -g @anthropic-ai/claude-code
  ```
  Then verify with `claude --version`. Persistent failures can be diagnosed with `claude doctor`.
- **Secret handling**: `~/.claude/settings.json`, `.claude/settings.local.json`, and `.claude/state/` are ignored by git (via `.gitignore`). No API keys or tokens are committed to the repository.
