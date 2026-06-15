import { describe, expect, it } from 'vitest';
import {
  acpTimelineNoticeTitle,
  acpTimelineStatusTitle,
  acpTimelineTodoEntries,
  acpTimelineToolDisplayTitle,
  acpTimelineToolKindLabel,
  fallbackTimelineFromMessages,
  normalizeAcpTimelineToolStatus,
} from '../../../shared/acpTimeline';

describe('acpTimeline', () => {
  it('normalizes tool statuses from ACP/provider terms', () => {
    expect(normalizeAcpTimelineToolStatus('pending')).toBe('pending');
    expect(normalizeAcpTimelineToolStatus('in_progress')).toBe('running');
    expect(normalizeAcpTimelineToolStatus('success')).toBe('completed');
    expect(normalizeAcpTimelineToolStatus('errored')).toBe('failed');
    expect(normalizeAcpTimelineToolStatus(undefined)).toBe('unknown');
  });

  it('maps common tool kinds to compact labels', () => {
    expect(acpTimelineToolKindLabel('bash')).toBe('Run');
    expect(acpTimelineToolKindLabel('grep')).toBe('Search');
    expect(acpTimelineToolKindLabel('file_read')).toBe('Read');
    expect(acpTimelineToolKindLabel('apply_patch')).toBe('Edit');
    expect(acpTimelineToolKindLabel('todo_write')).toBe('Todo');
    expect(acpTimelineToolKindLabel('lsp_diagnostics')).toBe('Diagnostics');
  });

  it('uses title before kind label for tool display titles', () => {
    expect(acpTimelineToolDisplayTitle('Reading package.json', 'read')).toBe('Reading package.json');
    expect(acpTimelineToolDisplayTitle('', 'grep')).toBe('Search');
  });

  it('converts legacy messages to timeline messages', () => {
    expect(fallbackTimelineFromMessages([{ role: 'assistant', text: 'Hello', timestamp: 12 }])).toEqual([
      { id: 'legacy-message-0', type: 'message', role: 'assistant', text: 'Hello', timestamp: 12 },
    ]);
  });

  it('labels notice cards', () => {
    expect(acpTimelineNoticeTitle('stderr')).toBe('Process Output');
    expect(acpTimelineNoticeTitle('warning')).toBe('Warning');
    expect(acpTimelineNoticeTitle('error')).toBe('Error');
    expect(acpTimelineNoticeTitle('cancelled')).toBe('Cancelled');
  });

  it('extracts todo entries from Claude TodoWrite payloads', () => {
    expect(acpTimelineTodoEntries({
      type: 'tool_use',
      name: 'TodoWrite',
      input: {
        todos: [
          { content: 'Inspect ACP UI', status: 'completed', priority: 'high' },
          { content: 'Add copy button', status: 'in_progress' },
        ],
      },
    })).toEqual([
      { text: 'Inspect ACP UI', status: 'completed', priority: 'high' },
      { text: 'Add copy button', status: 'in_progress', priority: undefined },
    ]);
  });

  it('extracts todo entries from string payloads', () => {
    expect(acpTimelineTodoEntries('Review final diff')).toEqual([{ text: 'Review final diff' }]);
    expect(acpTimelineTodoEntries('{"todos":["One","Two"]}')).toEqual([{ text: 'One' }, { text: 'Two' }]);
  });

  it('labels status cards without inventing context metrics', () => {
    expect(acpTimelineStatusTitle('compact')).toBe('Context Compacting');
    expect(acpTimelineStatusTitle('context')).toBe('Context');
    expect(acpTimelineStatusTitle('terminal')).toBe('Terminal');
  });
});
