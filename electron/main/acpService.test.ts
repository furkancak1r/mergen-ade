import { describe, expect, it, vi } from 'vitest';

const runClaudeCodexPlanMock = vi.hoisted(() => vi.fn());

vi.mock('electron', () => ({
  BrowserWindow: { getAllWindows: () => [] },
}));

vi.mock('./claudeCodexHook', () => ({
  runClaudeCodexPlan: runClaudeCodexPlanMock,
}));

import type { AcpTimelineToolItem } from '../shared/types';
import { claudeCodeArgsForMode, claudeCodePromptTextForMode, getAcpSession, killAcpChat, sendAcpPrompt, spawnAcpChat } from './acpService';

function isTimelineTool(item: unknown): item is AcpTimelineToolItem {
  return Boolean(item && typeof item === 'object' && (item as { type?: unknown }).type === 'tool');
}

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

  it('clears Claude adapter UI history without sending /clear as a visible prompt', async () => {
    const chatId = await spawnAcpChat({ projectId: 1, cwd: process.cwd(), mcpServers: [], tool: 'claude_acp' });
    try {
      const existing = getAcpSession(chatId);
      existing?.messages.push({ role: 'user', text: 'old prompt', timestamp: 1 });
      existing?.timeline.push({ id: 'old-message', type: 'message', role: 'user', text: 'old prompt', timestamp: 1 });

      sendAcpPrompt(chatId, '/clear', []);
      const session = getAcpSession(chatId);
      expect(session?.status).toBe('idle');
      expect(session?.title).toBe('Claude Code ACP');
      expect(session?.messages).toEqual([]);
      expect(session?.timeline).toEqual([]);
    } finally {
      killAcpChat(chatId);
    }
  });

  it('clears Codex adapter UI history without sending /clear as a visible prompt', async () => {
    const chatId = await spawnAcpChat({ projectId: 1, cwd: process.cwd(), mcpServers: [], tool: 'codex_acp' });
    try {
      const existing = getAcpSession(chatId);
      existing?.messages.push({ role: 'user', text: 'old prompt', timestamp: 1 });
      existing?.timeline.push({ id: 'old-message', type: 'message', role: 'user', text: 'old prompt', timestamp: 1 });

      sendAcpPrompt(chatId, '/clear', []);
      const session = getAcpSession(chatId);
      expect(session?.status).toBe('idle');
      expect(session?.title).toBe('Codex ACP');
      expect(session?.messages).toEqual([]);
      expect(session?.timeline).toEqual([]);
    } finally {
      killAcpChat(chatId);
    }
  });

  it('shows Codex Plan progress in Claude ACP while the planning hook is running', async () => {
    runClaudeCodexPlanMock.mockReturnValueOnce(new Promise(() => {}));
    const prompt = 'Fix ACP hook routing across renderer and main service with tests';
    const chatId = await spawnAcpChat({ projectId: 1, cwd: process.cwd(), mcpServers: [], tool: 'claude_acp' });
    try {
      sendAcpPrompt(chatId, prompt, [], 'codex_plan');

      const session = getAcpSession(chatId);
      const planningTool = session?.timeline.find((item): item is AcpTimelineToolItem => isTimelineTool(item) && item.kind === 'codex_plan');

      expect(runClaudeCodexPlanMock).toHaveBeenCalledWith({
        terminalId: 0,
        projectPath: process.cwd(),
        originalPrompt: prompt,
      });
      expect(planningTool).toMatchObject({
        type: 'tool',
        title: 'Running read-only Codex plan',
        kind: 'codex_plan',
        status: 'running',
      });
      expect(planningTool?.raw).toMatchObject({
        provider: 'Codex CLI',
        route: 'codex_plan',
        phase: 'planning',
      });
    } finally {
      killAcpChat(chatId);
      runClaudeCodexPlanMock.mockReset();
    }
  });
});
