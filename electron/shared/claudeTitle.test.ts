import { describe, expect, it } from 'vitest';
import { AiCliAttentionKind, AiCliStatus, AiCliTool } from './types';
import {
  ClaudeTransportStatus,
  claudeTitleHookEvent,
  detectClaudeStatusFromTitle,
  isClaudeAgentTitle,
} from './claudeTitle';

describe('claude title detection', () => {
  it('returns undefined for empty and non-agent titles', () => {
    expect(detectClaudeStatusFromTitle('')).toBeUndefined();
    expect(detectClaudeStatusFromTitle('bash')).toBeUndefined();
    expect(detectClaudeStatusFromTitle('vim myfile.ts')).toBeUndefined();
    expect(detectClaudeStatusFromTitle('cargo build')).toBeUndefined();
  });

  it('detects idle title prefixes like Rust', () => {
    expect(detectClaudeStatusFromTitle('\u2733 User acknowledgment and confirmation')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('\u2733 Claude Code')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('\u2733')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('* claude')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('* Waiting for input')).toBe(ClaudeTransportStatus.Idle);
  });

  it('detects working title prefixes like Rust', () => {
    expect(detectClaudeStatusFromTitle('\u280b Fixing the bug')).toBe(ClaudeTransportStatus.Working);
    expect(detectClaudeStatusFromTitle('\u2802 Claude Code')).toBe(ClaudeTransportStatus.Working);
    expect(detectClaudeStatusFromTitle('\u2810 User acknowledgment and confirmation')).toBe(ClaudeTransportStatus.Working);
    expect(detectClaudeStatusFromTitle('. claude')).toBe(ClaudeTransportStatus.Working);
    expect(detectClaudeStatusFromTitle('. Implementing feature')).toBe(ClaudeTransportStatus.Working);
  });

  it('detects permission, idle, and working keywords with Claude names', () => {
    expect(detectClaudeStatusFromTitle('Claude Code - action required')).toBe(ClaudeTransportStatus.Permission);
    expect(detectClaudeStatusFromTitle('claude - permission needed')).toBe(ClaudeTransportStatus.Permission);
    expect(detectClaudeStatusFromTitle('claude waiting for input')).toBe(ClaudeTransportStatus.Permission);
    expect(detectClaudeStatusFromTitle('claude ready')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('claude idle')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('claude done')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('claude working on task')).toBe(ClaudeTransportStatus.Working);
    expect(detectClaudeStatusFromTitle('claude thinking')).toBe(ClaudeTransportStatus.Working);
    expect(detectClaudeStatusFromTitle('claude running tests')).toBe(ClaudeTransportStatus.Working);
  });

  it('detects bare Claude and cc alias titles as idle', () => {
    expect(detectClaudeStatusFromTitle('claude')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('CLAUDE')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('cc')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('CC')).toBe(ClaudeTransportStatus.Idle);
    expect(detectClaudeStatusFromTitle('cc my task')).toBe(ClaudeTransportStatus.Idle);
  });

  it('scopes Claude agent title detection like Rust', () => {
    expect(isClaudeAgentTitle('\u2733 User acknowledgment')).toBe(true);
    expect(isClaudeAgentTitle('\u2802 Claude Code')).toBe(true);
    expect(isClaudeAgentTitle('. claude')).toBe(true);
    expect(isClaudeAgentTitle('* claude')).toBe(true);
    expect(isClaudeAgentTitle('claude ready')).toBe(true);
    expect(isClaudeAgentTitle('Claude Code - action required')).toBe(true);
    expect(isClaudeAgentTitle('cc')).toBe(true);
    expect(isClaudeAgentTitle('cc my task')).toBe(true);
    expect(isClaudeAgentTitle('bash')).toBe(false);
    expect(isClaudeAgentTitle('vim file.ts')).toBe(false);
    expect(isClaudeAgentTitle('codex')).toBe(false);
    expect(isClaudeAgentTitle('OpenCode working')).toBe(false);
  });

  it('maps Claude title statuses to hook events for renderer state', () => {
    expect(claudeTitleHookEvent(7, '\u280b Fixing')).toMatchObject({
      terminalId: 7,
      tool: AiCliTool.Claude,
      status: AiCliStatus.Running,
      reason: '\u280b Fixing',
    });
    expect(claudeTitleHookEvent(7, '\u2733 Done')).toMatchObject({
      terminalId: 7,
      tool: AiCliTool.Claude,
      status: AiCliStatus.Attention,
      attentionKind: AiCliAttentionKind.TurnComplete,
    });
    expect(claudeTitleHookEvent(7, 'claude permission needed')).toMatchObject({
      terminalId: 7,
      tool: AiCliTool.Claude,
      status: AiCliStatus.Attention,
      attentionKind: AiCliAttentionKind.Permission,
    });
    expect(claudeTitleHookEvent(7, 'bash')).toBeUndefined();
  });
});
