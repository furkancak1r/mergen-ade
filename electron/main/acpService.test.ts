import { describe, expect, it, vi } from 'vitest';

vi.mock('electron', () => ({
  BrowserWindow: { getAllWindows: () => [] },
}));

import { claudeCodeArgsForMode, claudeCodePromptTextForMode } from './acpService';

describe('Claude Code ACP helpers', () => {
  it('uses Claude Code native plan permission mode without prompt injection', () => {
    expect(claudeCodeArgsForMode('plan')).toEqual([
      '--print',
      '--output-format',
      'stream-json',
      '--verbose',
      '--permission-mode',
      'plan',
    ]);
    expect(claudeCodePromptTextForMode('fix the bug', 'plan')).toBe('fix the bug');
    expect(claudeCodePromptTextForMode('fix the bug', 'plan')).not.toContain('PLAN MODE');
  });

  it('keeps build prompts in bypass permission mode', () => {
    expect(claudeCodeArgsForMode('build')).toEqual([
      '--print',
      '--output-format',
      'stream-json',
      '--verbose',
      '--permission-mode',
      'bypassPermissions',
    ]);
  });
});
