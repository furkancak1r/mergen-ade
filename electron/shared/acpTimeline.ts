import type {
  AcpChatMessage,
  AcpTimelineItem,
  AcpTimelineNoticeKind,
  AcpTimelineToolStatus,
} from './types';

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

export function fallbackTimelineFromMessages(messages: readonly AcpChatMessage[] | undefined): AcpTimelineItem[] {
  return (messages ?? []).map((message, index) => ({
    id: `legacy-message-${index}`,
    type: 'message',
    role: message.role,
    text: message.text,
    timestamp: message.timestamp,
  }));
}
