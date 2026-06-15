import type {
  AcpChatMessage,
  AcpTimelineItem,
  AcpTimelineNoticeKind,
  AcpTimelineStatusKind,
  AcpTimelineToolStatus,
} from './types';

export interface AcpTimelineTodoEntry {
  text: string;
  status?: string;
  priority?: string;
}

export function normalizeAcpTimelineToolStatus(status: unknown): AcpTimelineToolStatus {
  const text = typeof status === 'string' ? status.trim().toLowerCase() : '';
  if (!text) return 'unknown';
  if (['pending', 'queued'].includes(text)) return 'pending';
  if (['running', 'started', 'in_progress', 'in-progress'].includes(text)) return 'running';
  if (['completed', 'complete', 'done', 'success', 'succeeded', 'finished'].includes(text)) return 'completed';
  if (['failed', 'failure', 'error', 'errored', 'cancelled', 'canceled'].includes(text)) return 'failed';
  return 'unknown';
}

export function acpTimelineToolKindLabel(kind: string | undefined): string {
  const normalized = (kind || '').trim().toLowerCase();
  if (!normalized) return 'Tool';

  if (normalized.includes('bash') || normalized.includes('shell') || normalized.includes('terminal')) return 'Run';
  if (normalized.includes('grep') || normalized.includes('glob') || normalized.includes('search')) return 'Search';
  if (normalized.includes('read')) return 'Read';
  if (normalized.includes('todo')) return 'Todo';
  if (normalized.includes('edit') || normalized.includes('patch') || normalized.includes('write')) return 'Edit';
  if (normalized.includes('web')) return 'Web';
  if (normalized.includes('lsp') || normalized.includes('diagnostic')) return 'Diagnostics';
  return normalized
    .split(/[_\s/-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ') || 'Tool';
}

export function acpTimelineToolDisplayTitle(title: string | undefined, kind: string | undefined): string {
  const trimmed = title?.trim();
  if (trimmed) return trimmed;
  return acpTimelineToolKindLabel(kind);
}

export function acpTimelineNoticeTitle(kind: AcpTimelineNoticeKind): string {
  switch (kind) {
    case 'stderr':
      return 'Process Output';
    case 'warning':
      return 'Warning';
    case 'error':
      return 'Error';
    case 'cancelled':
      return 'Cancelled';
  }
}

export function acpTimelineStatusTitle(kind: AcpTimelineStatusKind): string {
  switch (kind) {
    case 'compact':
      return 'Context Compacting';
    case 'context':
      return 'Context';
    case 'status':
      return 'Status';
    case 'cost':
      return 'Cost';
    case 'terminal':
      return 'Terminal';
    case 'info':
      return 'Info';
  }
}

export function acpTimelineTodoEntries(raw: unknown): AcpTimelineTodoEntry[] {
  const direct = extractTodoEntries(raw);
  if (direct.length > 0) return direct;

  if (raw && typeof raw === 'object') {
    const record = raw as Record<string, unknown>;
    for (const key of ['input', 'params', 'arguments', 'args', 'content']) {
      const nested = extractTodoEntries(record[key]);
      if (nested.length > 0) return nested;
    }
  }

  return [];
}

export function fallbackTimelineFromMessages(messages: readonly AcpChatMessage[] | undefined): AcpTimelineItem[] {
  return (messages ?? []).map((message, index) => ({
    id: `legacy-message-${index}`,
    type: 'message',
    role: message.role,
    text: message.text,
    timestamp: message.timestamp,
  }));
}

function extractTodoEntries(value: unknown): AcpTimelineTodoEntry[] {
  if (!value) return [];
  if (Array.isArray(value)) return value.map(todoEntryFromUnknown).filter((entry): entry is AcpTimelineTodoEntry => Boolean(entry));
  if (typeof value === 'string') return todoEntriesFromString(value);
  if (typeof value !== 'object') return [];

  const record = value as Record<string, unknown>;
  for (const key of ['todos', 'todo', 'items', 'tasks']) {
    const entries = extractTodoEntries(record[key]);
    if (entries.length > 0) return entries;
  }
  return [];
}

function todoEntriesFromString(value: string): AcpTimelineTodoEntry[] {
  const trimmed = value.trim();
  if (!trimmed) return [];
  try {
    return extractTodoEntries(JSON.parse(trimmed));
  } catch {
    return [{ text: trimmed }];
  }
}

function todoEntryFromUnknown(value: unknown): AcpTimelineTodoEntry | undefined {
  if (typeof value === 'string') {
    const text = value.trim();
    return text ? { text } : undefined;
  }
  if (!value || typeof value !== 'object') return undefined;

  const record = value as Record<string, unknown>;
  const text = stringValue(record.content) || stringValue(record.text) || stringValue(record.title) || stringValue(record.task);
  if (!text) return undefined;

  return {
    text,
    status: stringValue(record.status) || undefined,
    priority: stringValue(record.priority) || undefined,
  };
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
