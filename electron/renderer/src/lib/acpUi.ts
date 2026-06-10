import type { AcpChatSession, AcpConfigOption } from '../../../shared/types';

export interface AcpEventLike {
  type?: string;
  status?: AcpChatSession['status'];
  queuedPrompts?: number;
  count?: number;
}

export interface AcpActivityState {
  running: boolean;
  hasQueuedPrompts: boolean;
}

export interface AcpCommandLike {
  id?: unknown;
  name?: unknown;
  description?: unknown;
}

export interface AcpSlashCommandItem {
  hint: string;
  description: string;
}

export const OPENCODE_ACP_LABEL = 'OpenCode ACP';
export const OPENCODE_ACP_OPEN_BUTTON_LABEL = '+ ACP';
export const OPENCODE_ACP_CLOSE_TOOLTIP = `Close ${OPENCODE_ACP_LABEL}`;

export function openCodeAcpPanelTitle(projectName: string): string {
  return `${OPENCODE_ACP_LABEL} - ${projectName}`;
}

export function openCodeAcpWelcomeText(): string {
  return `Welcome to ${OPENCODE_ACP_LABEL}`;
}

export function isAcpRunningStatus(status: AcpChatSession['status'] | undefined): boolean {
  return status === 'running' || status === 'permission';
}

export function nextAcpActivityState(previous: AcpActivityState, event: AcpEventLike): AcpActivityState {
  let running = previous.running;
  if (event.status !== undefined) {
    running = isAcpRunningStatus(event.status);
  } else if (event.type === 'promptSent' || event.type === 'permission') {
    running = true;
  } else if (event.type === 'promptResponse' || event.type === 'cancelled' || event.type === 'exit' || event.type === 'error') {
    running = false;
  }

  const queuedCount = event.queuedPrompts ?? event.count;
  const hasQueuedPrompts = queuedCount === undefined ? previous.hasQueuedPrompts : queuedCount > 0;

  return { running, hasQueuedPrompts };
}

export function shouldShowAcpWelcome(messages: unknown[] | undefined, queuedPrompts: unknown[] | undefined): boolean {
  return (messages?.length ?? 0) === 0 && (queuedPrompts?.length ?? 0) === 0;
}

export function optionValues(option: AcpConfigOption | undefined): AcpConfigOption['options'] {
  return option?.options ?? [];
}

export function hasConfigSelectorOptions(modelOptions: AcpConfigOption | undefined, effortOptions: AcpConfigOption | undefined): boolean {
  return optionValues(modelOptions).length > 0 || optionValues(effortOptions).length > 0;
}

export function actionControlsEnabled(session: Pick<AcpChatSession, 'sessionId'> | null | undefined): boolean {
  return Boolean(session?.sessionId);
}

export function acpModeUiLabel(modeId: string | undefined): string | undefined {
  if (modeId === 'plan') return 'Plan';
  if (!modeId || modeId === 'build') return undefined;
  return modeId;
}

export function queuedPromptPreview(prompt: { text?: string; finalPromptText?: string; attachments?: unknown[] }): string {
  const directText = prompt.text?.trim();
  if (directText) return directText;
  const finalText = prompt.finalPromptText?.trim();
  if (finalText) return finalText;
  return (prompt.attachments?.length ?? 0) > 0 ? '(Attachment)' : '(Empty prompt)';
}

export function moveQueuedPromptToFront<T>(queue: readonly T[], index: number): T[] {
  if (!Number.isInteger(index) || index < 0 || index >= queue.length) return [...queue];
  const next = [...queue];
  const [item] = next.splice(index, 1);
  next.unshift(item);
  return next;
}

export function removeQueuedPromptAt<T>(queue: readonly T[], index: number): T[] {
  if (!Number.isInteger(index) || index < 0 || index >= queue.length) return [...queue];
  return queue.filter((_, itemIndex) => itemIndex !== index);
}

function commandText(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

export function slashCommandHint(command: AcpCommandLike): string | undefined {
  const raw = commandText(command.id) || commandText(command.name);
  const token = raw.replace(/^\/+/, '').trim();
  if (!token || /\s/.test(token)) return undefined;
  return `/${token}`;
}

export function slashCommandItems(commands: unknown, query: string, limit = 6): AcpSlashCommandItem[] {
  if (!Array.isArray(commands)) return [];
  const normalizedQuery = query.trim().replace(/^\/+/, '').toLowerCase();
  const items: AcpSlashCommandItem[] = [];
  const seen = new Set<string>();

  for (const command of commands) {
    if (!command || typeof command !== 'object') continue;
    const candidate = command as AcpCommandLike;
    const hint = slashCommandHint(candidate);
    if (!hint) continue;

    const idText = hint.slice(1).toLowerCase();
    const nameText = commandText(candidate.name).replace(/^\/+/, '').toLowerCase();
    if (normalizedQuery && !idText.startsWith(normalizedQuery) && !nameText.startsWith(normalizedQuery)) {
      continue;
    }
    if (seen.has(hint)) continue;

    seen.add(hint);
    items.push({
      hint,
      description: commandText(candidate.description),
    });
    if (items.length >= limit) break;
  }

  return items;
}

export function slashCommandHints(commands: unknown, query: string, limit = 6): string[] {
  return slashCommandItems(commands, query, limit).map((item) => item.hint);
}

export function slashCommandItemsForInput(commands: unknown, input: string, limit = 6): AcpSlashCommandItem[] {
  const query = input.trim();
  if (!query.startsWith('/')) return [];
  return slashCommandItems(commands, query.slice(1), limit);
}
