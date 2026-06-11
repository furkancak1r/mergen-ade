import { describe, it, expect } from 'vitest';
import { parseAcpLine, appendMentionsToInput, buildAcpPromptText, pathToMention, removeMentionFromInput } from './acpParser';

describe('acpParser', () => {
  it('parses initialized', () => {
    const result = parseAcpLine('{"method":"initialized"}');
    expect(result).toEqual({ type: 'initialized' });
  });

  it('parses session/new with sessionId and configOptions', () => {
    const line = JSON.stringify({
      method: 'session/new',
      result: { sessionId: 'abc123', configOptions: [{ id: 'model', name: 'Model', category: 'General', currentValue: 'sonnet', options: [] }] },
    });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('sessionCreated');
    expect(result?.sessionId).toBe('abc123');
    expect(result?.options).toHaveLength(1);
    expect(result?.options?.[0].id).toBe('model');
  });

  it('parses session/prompt response', () => {
    const line = JSON.stringify({ method: 'session/prompt', result: { text: 'Hello world' } });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('promptResponse');
    expect(result?.text).toBe('Hello world');
  });

  it('parses config_option_update with configOptions array', () => {
    const line = JSON.stringify({
      method: 'config_option_update',
      params: { configOptions: [{ id: 'effort', name: 'Effort', category: 'General', currentValue: 'low', options: [] }] },
    });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('configOptions');
    expect(result?.options?.[0].id).toBe('effort');
  });

  it('parses current_mode_update with currentModeId', () => {
    const line = JSON.stringify({ method: 'current_mode_update', params: { currentModeId: 'plan' } });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('modeUpdate');
    expect(result?.modeId).toBe('plan');
  });

  it('parses current_mode_update with legacy modeId', () => {
    const line = JSON.stringify({ method: 'current_mode_update', params: { modeId: 'build' } });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('modeUpdate');
    expect(result?.modeId).toBe('build');
  });

  it('parses available_commands_update with availableCommands', () => {
    const line = JSON.stringify({
      method: 'available_commands_update',
      params: { availableCommands: [{ name: 'test', description: 'Test command' }] },
    });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('commands');
    expect(result?.commands?.[0].name).toBe('test');
  });

  it('parses available_commands_update with legacy commands field', () => {
    const line = JSON.stringify({
      method: 'available_commands_update',
      params: { commands: [{ name: 'legacy', description: 'Legacy' }] },
    });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('commands');
    expect(result?.commands?.[0].name).toBe('legacy');
  });

  it('parses permission_request with string requestId', () => {
    const line = JSON.stringify({
      method: 'permission_request',
      params: { requestId: 'req-123', message: 'Allow edit?' },
    });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('permission');
    expect(result?.requestId).toBe('req-123');
    expect(result?.message).toBe('Allow edit?');
  });

  it('parses permission_request with numeric requestId', () => {
    const line = JSON.stringify({
      method: 'permission_request',
      params: { requestId: 42, message: 'Allow?' },
    });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('permission');
    expect(result?.requestId).toBe(42);
  });

  it('returns raw for unknown methods', () => {
    const line = JSON.stringify({ method: 'unknown_thing', params: {} });
    const result = parseAcpLine(line);
    expect(result?.type).toBe('raw');
  });

  it('returns undefined for non-JSON lines', () => {
    const result = parseAcpLine('not json');
    expect(result).toBeUndefined();
  });

  it('buildAcpPromptText includes attachments block', () => {
    const text = 'analyze this';
    const attachments = ['/a/b.ts', '/c/d.ts'];
    const result = buildAcpPromptText(text, attachments);
    expect(result).toBe('analyze this\n\nAttached file paths:\n/a/b.ts\n/c/d.ts');
  });

  it('buildAcpPromptText matches Rust attachment-only format', () => {
    expect(buildAcpPromptText('', ['/a/b.ts'])).toBe('Attached file paths:\n/a/b.ts');
  });

  it('buildAcpPromptText returns text only when no attachments', () => {
    expect(buildAcpPromptText('hello', [])).toBe('hello');
  });

  it('pathToMention matches Rust basename and mojibake repair behavior', () => {
    expect(pathToMention('C:/test/file.rs')).toBe('@file.rs');
    expect(pathToMention('/home/user/şema.rs')).toBe('@şema.rs');
    expect(pathToMention('C:/test/TÃ¼rkÃ§e.txt')).toBe('@Türkçe.txt');
  });

  it('appendMentionsToInput matches Rust spacing behavior', () => {
    expect(appendMentionsToInput('', ['C:/test/foo.rs'])).toBe('@foo.rs');
    expect(appendMentionsToInput('bunu incele', ['C:/test/foo.rs'])).toBe('bunu incele @foo.rs');
    expect(appendMentionsToInput('incele', ['C:/test/a.rs', 'D:/test/b.rs'])).toBe('incele @a.rs @b.rs');
    expect(appendMentionsToInput('incele', [])).toBe('incele');
  });

  it('removeMentionFromInput removes last exact match', () => {
    const input = 'hello @file.ts world @file.ts';
    const result = removeMentionFromInput(input, '@file.ts');
    expect(result).toBe('hello @file.ts world');
  });

  it('removeMentionFromInput keeps Rust first-position behavior', () => {
    expect(removeMentionFromInput('@file.ts world', '@file.ts')).toBe(' world');
  });

  it('removeMentionFromInput returns unchanged when mention not found', () => {
    expect(removeMentionFromInput('hello', '@missing')).toBe('hello');
  });
});
