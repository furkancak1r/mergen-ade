import { AiCliAttentionKind, AiCliStatus, AiCliTool, type AiHookEvent } from './types';

export enum ClaudeTransportStatus {
  Working = 'working',
  Idle = 'idle',
  Permission = 'permission',
}

const CLAUDE_IDLE_PREFIX = '\u2733';

function isBrailleSpinner(char: string): boolean {
  const codePoint = char.codePointAt(0) ?? 0;
  return codePoint >= 0x2800 && codePoint <= 0x28ff;
}

function containsBrailleSpinner(title: string): boolean {
  return Array.from(title).some(isBrailleSpinner);
}

function containsAgentName(title: string): boolean {
  const lower = title.toLowerCase();
  return ['claude', 'codex', 'gemini', 'opencode', 'aider'].some((name) => lower.includes(name));
}

function containsAny(title: string, keywords: string[]): boolean {
  const lower = title.toLowerCase();
  return keywords.some((keyword) => lower.includes(keyword));
}

export function detectClaudeStatusFromTitle(title: string): ClaudeTransportStatus | undefined {
  if (title.length === 0) return undefined;

  if (title.startsWith(CLAUDE_IDLE_PREFIX) || title === CLAUDE_IDLE_PREFIX) {
    return ClaudeTransportStatus.Idle;
  }
  if (containsBrailleSpinner(title)) {
    return ClaudeTransportStatus.Working;
  }
  if (title.startsWith('. ')) {
    return ClaudeTransportStatus.Working;
  }
  if (title.startsWith('* ')) {
    return ClaudeTransportStatus.Idle;
  }

  if (containsAgentName(title)) {
    if (containsAny(title, ['action required', 'permission', 'waiting'])) {
      return ClaudeTransportStatus.Permission;
    }
    if (containsAny(title, ['ready', 'idle', 'done'])) {
      return ClaudeTransportStatus.Idle;
    }
    if (containsAny(title, ['working', 'thinking', 'running'])) {
      return ClaudeTransportStatus.Working;
    }
    if (title.toLowerCase().startsWith('claude')) {
      return ClaudeTransportStatus.Idle;
    }
  }

  const lower = title.toLowerCase();
  if (lower === 'cc' || lower.startsWith('cc ')) {
    return ClaudeTransportStatus.Idle;
  }

  return undefined;
}

export function isClaudeAgentTitle(title: string): boolean {
  if (title.length === 0) return false;

  if (title.startsWith(CLAUDE_IDLE_PREFIX) || title === CLAUDE_IDLE_PREFIX) return true;
  if (title.startsWith('. ') || title.startsWith('* ')) return true;
  if (containsBrailleSpinner(title)) return true;

  const lower = title.toLowerCase();
  if (lower.startsWith('claude')) return true;
  return lower === 'cc' || lower.startsWith('cc ');
}

export function claudeTitleHookEvent(terminalId: number, title: string): AiHookEvent | undefined {
  if (!isClaudeAgentTitle(title)) return undefined;
  const status = detectClaudeStatusFromTitle(title);
  if (!status) return undefined;

  if (status === ClaudeTransportStatus.Working) {
    return {
      terminalId,
      tool: AiCliTool.Claude,
      status: AiCliStatus.Running,
      reason: title,
      eventKind: 'title.update',
    };
  }

  return {
    terminalId,
    tool: AiCliTool.Claude,
    status: AiCliStatus.Attention,
    reason: title,
    attentionKind: status === ClaudeTransportStatus.Permission
      ? AiCliAttentionKind.Permission
      : AiCliAttentionKind.TurnComplete,
    eventKind: 'title.update',
  };
}
