# Claude Code Configuration Guidelines

## Claude Code Configuration

- **Config location**: All Fireworks/Kimi runtime settings, CLIProxyAPI planner env, and hooks are stored in **global** `~/.claude/settings.json` so they apply regardless of which directory Claude Code is launched from. Project-local `.claude/settings.local.json` can still override for specific projects, but the global file is the primary source of truth.
- **Build model**: All Claude Code Anthropic model overrides (`ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`) must point to Fireworks Kimi K2.6 Turbo router (`accounts/fireworks/routers/kimi-k2p6-turbo`).
- **Base URL**: `ANTHROPIC_BASE_URL` must be `https://api.fireworks.ai/inference`. Never use the local CLIProxyAPI endpoint for Claude Code runtime.
- **API key helper**: `apiKeyHelper` resolves the Fireworks API key from `FIREWORKS_API_KEY` env var or `~/.local/share/opencode/auth.json`. No key is hardcoded or committed.
- **Mandatory planning hook**: The `UserPromptSubmit` hook runs before `/goal`, `/plan`, and any prompt submitted while Claude Code is in plan mode (`permission_mode == "plan"`) to generate a mandatory implementation plan via local Docker CLIProxyAPI (`gpt-5.5` with `xhigh` reasoning effort). The plan is written to `.claude/state/last-cliproxy-plan.md` and injected into Claude Code context as `additionalContext`.
- **Planner endpoint**: CLIProxyAPI runs at `http://127.0.0.1:8317/v1`. It is only used for planning; all build/goal/evaluator traffic stays on Fireworks Kimi.
- **Plan failure behavior**: If CLIProxyAPI plan generation fails, the hook returns `{"decision":"block"}` with a short reason on stdout and exits `0`. Claude Code processes JSON decisions only on exit code 0; exit code 2 is treated as a raw blocking error.
- **Hook stdin preservation**: The hook script must capture Claude's JSON stdin to a temp file **before** invoking Python, because Python heredocs (`python - <<'PYEOF'`) consume stdin and would prevent `json.load(sys.stdin)` from reading the hook payload. The bash wrapper drains stdin with `cat > "$HOOK_INPUT_FILE"`, then passes the file path to Python via `HOOK_INPUT_FILE` env.
- **Windows hook invocation**: Use `"shell": "powershell"` in the hook config and invoke Git Bash via PowerShell call operator:
  ```json
  {
    "type": "command",
    "shell": "powershell",
    "command": "& 'C:/Program Files/Git/bin/bash.exe' 'C:/Users/.../.claude/hooks/cliproxy-plan-before-goal.sh'",
    "timeout": 300
  }
  ```
  Do **not** wrap the command in extra quotes or prefix with `bash.exe` directly in the `command` field, because Claude Code's hook runner on Windows may invoke the default shell again and misinterpret `bash.exe` as a script rather than an executable.
- **Auto-updater disable**: Set `"DISABLE_AUTOUPDATER": "1"` in the settings `env` block to suppress background auto-update checks. If the update footer still appears, also add `"DISABLE_UPDATES": "1"` to block all update paths including manual `claude update`.
- **Windows compatibility**: The `apiKeyHelper` must use a `.cmd` wrapper on Windows (`fireworks-firepass-key-helper.cmd`) that invokes the bash script via Git Bash, because Claude Code spawns commands through `cmd.exe` which does not recognize extension-less scripts.
- **Auth conflict**: `ANTHROPIC_AUTH_TOKEN` env var must be unset when using `apiKeyHelper`. If both are present, Claude Code warns and may behave unexpectedly. Remove the token from the user environment and current shell before starting Claude Code. In PowerShell:
  ```powershell
  [Environment]::SetEnvironmentVariable("ANTHROPIC_AUTH_TOKEN", $null, "User")
  Remove-Item Env:ANTHROPIC_AUTH_TOKEN -ErrorAction SilentlyContinue
  ```
- **Shell alias conflict**: If the shell profile defines `claude` as a wrapper (e.g., routing to MiniMax via `Invoke-ClaudeViaMiniMax`), the wrapper may set `ANTHROPIC_AUTH_TOKEN` on every invocation and override the project Fireworks config. The `claude` alias/function must be updated to clear stale Anthropic env vars before calling the real `claude.cmd`, while a separate alias (e.g., `cc`) can keep the wrapper behavior.
- **Mergen terminal sanitization**: Mergen must remove stale Anthropic env vars (`ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL`) from every spawned terminal via `CommandBuilder::env_remove`. Additionally, the Claude builtin launcher must send a sanitized command that bypasses shell aliases and directly invokes the real npm-installed `claude.cmd`.
- **Auto-update failures**: If Claude Code prints "Auto-update failed", reinstall the global npm package:
  ```powershell
  npm i -g @anthropic-ai/claude-code
  ```
  Then verify with `claude --version`. Persistent failures can be diagnosed with `claude doctor`.
- **Secret handling**: `~/.claude/settings.json`, `.claude/settings.local.json`, and `.claude/state/` are ignored by git (via `.gitignore`). No API keys or tokens are committed to the repository.
