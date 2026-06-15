import { describe, expect, it } from 'vitest';
import { codexExecJsonArgs, parseCodexExecJsonLine } from './codex';

describe('codex helpers', () => {
  it('builds JSON exec args with non-interactive approval policy', () => {
    expect(codexExecJsonArgs('C:/repo')).toEqual([
      '-a',
      'never',
      '-C',
      'C:/repo',
      'exec',
      '--json',
      '--sandbox',
      'workspace-write',
      '--skip-git-repo-check',
      '-',
    ]);
  });

  it('parses Codex assistant message events', () => {
    const parsed = parseCodexExecJsonLine(JSON.stringify({
      type: 'item.completed',
      item: { id: 'item_0', type: 'agent_message', text: 'READY' },
    }));

    expect(parsed).toEqual({ kind: 'assistant_message', text: 'READY' });
  });

  it('parses Codex command tool events', () => {
    const started = parseCodexExecJsonLine(JSON.stringify({
      type: 'item.started',
      item: { id: 'call_1', type: 'command_execution', command: 'npm test' },
    }));
    const completed = parseCodexExecJsonLine(JSON.stringify({
      type: 'item.completed',
      item: { id: 'call_1', type: 'command_execution', command: 'npm test' },
    }));

    expect(started).toMatchObject({
      kind: 'tool',
      id: 'call_1',
      title: 'npm test',
      toolKind: 'bash',
      status: 'running',
    });
    expect(completed).toMatchObject({
      kind: 'tool',
      id: 'call_1',
      title: 'npm test',
      toolKind: 'bash',
      status: 'completed',
    });
  });

  it('ignores non-json lines from Codex stderr-style output', () => {
    expect(parseCodexExecJsonLine('WARN not json')).toBeUndefined();
  });
});
