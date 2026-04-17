# Codex CLI Windows Hooks Research

**Date:** 2026-04-17  
**Purpose:** Document upstream Codex CLI hooks support status and rationale for strict hook-only tracking in Mergen ADE

---

## Executive Summary

Codex CLI hooks are **officially supported on Windows** as of PR #17268 (merged 2026-04-09) and release 0.120.0. The previous assumption that hooks are "unsupported on native Windows" was incorrect and based on outdated documentation.

Mergen ADE's Codex integration must now move to a **strict hook-only model**, removing all fallback mechanisms (notify, visible UI parsing, title detection, process polling).

---

## Upstream Status

### Historical Context

| Period | Status | Notes |
|--------|--------|-------|
| Pre-0.120.0 | Windows hooks gated | Upstream docs stated "Hooks are currently disabled on Windows" |
| 2026-04-09 | PR #17268 merged | "remove windows gate that disables hooks" |
| 0.120.0+ | Fully supported | Hooks work on native Windows via `cmd.exe /C` execution |

### Technical Details

From upstream `codex-rs/hooks/src/engine/command_runner.rs`:

```rust
// On Windows, commands are run via cmd.exe /C to ensure proper shell execution
// This enables hooks to work on native Windows without MSYS2/MinGW dependencies
```

Hook commands are executed through the standard Windows command shell, making them compatible with native Windows binaries like Mergen ADE.

---

## Strict Hook-Only Model for Codex

### Rationale

1. **Upstream now guarantees hook delivery** - No need for unreliable fallback mechanisms
2. **Simpler state machine** - Only two events: `UserPromptSubmit` (start) and `Stop` (end)
3. **Consistent cross-platform behavior** - Same code path on Windows, macOS, and Linux
4. **Reduced maintenance burden** - No need to maintain multiple signal sources

### Allowed Sources

| Source | Status | Rationale |
|--------|--------|-----------|
| `UserPromptSubmit` hook | ✅ Required | Spinner starts (working state) |
| `Stop` hook | ✅ Required | Spinner stops (attention/idle state) |

### Removed Sources

| Source | Status | Reason for Removal |
|--------|--------|-------------------|
| `--codex-notify` events | ❌ Removed | Superseded by hooks |
| Visible UI/TUI text parsing | ❌ Removed | Unreliable, hook-only is sufficient |
| Terminal title detection | ❌ Removed | Superseded by hooks |
| BEL notification parsing | ❌ Removed | Superseded by hooks |
| Process polling | ❌ Removed | Superseded by hooks |
| Prompt submit fallback | ❌ Removed | Proper hook now available |
| Launch detection spinner start | ❌ Removed | Wait for actual hook event |

---

## Hook Event Semantics

```
UserPromptSubmit → Status: Running (spinner starts)
Stop             → Status: Attention/Idle (spinner stops, pulse if needed)
```

### State Machine

```
Inactive ──UserPromptSubmit──► Running ──Stop──► Attention ──[ack]──► Inactive
                                      │
                                      └─[new prompt]──► Running
```

The `Stop` event is intentionally generic—it signals the end of a turn without specifying why. The UI should transition to an attention state (pulse) that requires user acknowledgment.

---

## Implementation Notes

### Config Changes

`~/.codex/hooks.json` (managed by Mergen):

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "<bridge> --codex-hook UserPromptSubmit"
      }]
    }],
    "Stop": [{
      "hooks": [{
        "type": "command",
        "command": "<bridge> --codex-hook Stop"
      }]
    }]
  }
}
```

### Environment Variables

No changes required. Existing env vars continue to work:

- `MERGEN_TERMINAL_ID`
- `MERGEN_AI_INBOX_DIR`
- `MERGEN_AI_TOOL_HINT`
- `MERGEN_ADE_CODEX_INBOX_TOKEN`

### CLI Arguments

- `--codex-hook <event>`: Kept (primary mechanism)
- `--codex-notify`: Deprecated, to be removed in future cleanup

---

## Migration Checklist

- [x] Remove `CodexCliStatusSource::Notify`, `::VisibleUi`, `::TerminalTitle`, `::PromptSubmit`
- [x] Simplify to `CodexCliStatusSource::Hook` only
- [x] Remove `PendingVisibleCodexStatus` from terminal.rs
- [x] Remove `detect_codex_status_from_title()` and `is_codex_agent_title()` from hooks.rs
- [x] Update tooltip handling to generic "waiting" message
- [x] Update AGENTS.md with hook-only policy
- [ ] Eventually remove `--codex-notify` CLI argument entirely

---

## References

1. OpenAI Codex CLI Repository: `openai/codex`
2. PR #17268: "remove windows gate that disables hooks"
3. Changelog entry: `Codex CLI 0.120.0`
4. Upstream docs: `https://developers.openai.com/codex/cli/hooks/`
5. Mergen ADE AGENTS.md: AI CLI integration policy

---

## Related Files

- `src/codex.rs` - Hook handling, config management
- `src/app.rs` - Status source enum, state machine
- `src/terminal.rs` - Visible text detection (being removed)
- `src/hooks.rs` - Title-based detection (being removed)
- `src/main.rs` - CLI argument parsing
- `AGENTS.md` - Policy documentation
