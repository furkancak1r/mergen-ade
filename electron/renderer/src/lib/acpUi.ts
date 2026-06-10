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

export interface AcpKimiProtectionBadge {
  label: 'Kimi protected' | 'Kimi unprotected';
  color: string;
}

export const ACP_QUEUED_PROMPT_MAX_VISIBLE_ROWS = 2;
export const ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS = 96;
export const ACP_CHAT_TITLE_MAX_CHARS = 72;
export const OPENCODE_ACP_LABEL = 'OpenCode ACP';
export const OPENCODE_ACP_OPEN_BUTTON_LABEL = '+ ACP';
export const OPENCODE_ACP_CLOSE_TOOLTIP = `Close ${OPENCODE_ACP_LABEL}`;

export function openCodeAcpPanelTitle(_projectName?: string): string {
  return OPENCODE_ACP_LABEL;
}

export function openCodeAcpWelcomeText(): string {
  return `Welcome to ${OPENCODE_ACP_LABEL}`;
}

export function acpChatTitleFromPrompt(promptText: string): string {
  const collapsed = promptText.split(/\s+/).filter(Boolean).join(' ').trim();
  if (!collapsed) return OPENCODE_ACP_LABEL;
  const chars = Array.from(collapsed);
  if (chars.length <= ACP_CHAT_TITLE_MAX_CHARS) return collapsed;
  return `${chars.slice(0, ACP_CHAT_TITLE_MAX_CHARS).join('')}...`;
}

interface AcpChatTitleState {
  messages?: readonly { role?: string }[];
  queuedPrompts?: readonly unknown[];
  title?: string;
}

export function acpChatHasStartedState(session: AcpChatTitleState | null | undefined): boolean {
  return Boolean(
    session
      && ((session.queuedPrompts?.length ?? 0) > 0
        || (session.messages?.some((message) => message.role === 'user') ?? false)),
  );
}

export function acpChatDisplayTitle(session: AcpChatTitleState | null | undefined): string {
  if (!acpChatHasStartedState(session)) return OPENCODE_ACP_LABEL;
  const title = session?.title?.trim();
  return title || OPENCODE_ACP_LABEL;
}

export function acpTerminalManagerRowLabel(session: AcpChatTitleState | null | undefined): string {
  const title = acpChatDisplayTitle(session);
  return acpChatHasStartedState(session) ? `${OPENCODE_ACP_LABEL} - ${title}` : title;
}

export function isAcpRunningStatus(status: AcpChatSession['status'] | undefined): boolean {
  return status === 'running' || status === 'permission';
}

export function acpStatusText(status: AcpChatSession['status'] | undefined): string {
  switch (status) {
    case 'starting':
    case 'connected':
    case 'session_created':
      return 'Starting...';
    case 'idle':
      return 'Idle';
    case 'running':
      return 'Running...';
    case 'permission':
      return 'Permission';
    case 'error':
      return 'Error';
    default:
      return 'Idle';
  }
}

export function acpHeaderStatusColor(status: AcpChatSession['status'] | undefined): string {
  switch (status) {
    case 'running':
      return 'rgb(100, 200, 100)';
    case 'permission':
      return 'rgb(255, 200, 100)';
    case 'error':
      return 'rgb(185, 45, 45)';
    default:
      return 'rgb(138, 138, 138)';
  }
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

export function acpComposerHintText(options: {
  welcomeCenter: boolean;
  sessionReady: boolean;
  activeMode?: string;
}): string {
  if (!options.sessionReady) return 'Waiting for session...';
  if (options.welcomeCenter) return 'Plan, Build, / for skills, @ for context';
  if (options.activeMode === 'plan') return 'Plan and design before coding...';
  return 'Type a message...';
}

export function opencodeModelHasKimiLoopRisk(model: string | undefined): boolean {
  const normalized = model?.toLowerCase() ?? '';
  return normalized.includes('kimi') || normalized.includes('k2p6');
}

export function acpKimiProtectionBadge(
  model: string | undefined,
  loopProtectionEnabled: boolean,
): AcpKimiProtectionBadge | undefined {
  if (!opencodeModelHasKimiLoopRisk(model)) return undefined;
  return loopProtectionEnabled
    ? { label: 'Kimi protected', color: 'rgb(100, 195, 140)' }
    : { label: 'Kimi unprotected', color: 'rgb(220, 170, 60)' };
}

function acpQueuedPromptPreviewText(text: string): string {
  const collapsed = text.split(/\s+/).filter(Boolean).join(' ');
  const chars = Array.from(collapsed);
  if (chars.length <= ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS) return collapsed;
  return `${chars.slice(0, ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS).join('')}...`;
}

export function queuedPromptPreview(prompt: { text?: string; finalPromptText?: string; attachments?: unknown[] }): string {
  const finalText = prompt.finalPromptText ? acpQueuedPromptPreviewText(prompt.finalPromptText) : '';
  if (finalText) return finalText;
  const directText = prompt.text ? acpQueuedPromptPreviewText(prompt.text) : '';
  if (directText) return directText;
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

export function acpQueuedPromptDraftEditBlockedMessage(options: {
  input: string;
  attachments?: unknown[];
  editingQueuedPrompt: boolean;
}): string | undefined {
  if (options.input.trim().length > 0 || (options.attachments?.length ?? 0) > 0 || options.editingQueuedPrompt) {
    return 'Input is not empty; send or clear it before editing queued message';
  }
  return undefined;
}

export function acpQueuedPromptPlanCount(prompts: readonly { modeId?: string }[]): number {
  return prompts.filter((prompt) => prompt.modeId === 'plan').length;
}

export function acpQueuedPromptHeaderLabel(queueCount: number, editingIndex?: number): string {
  if (editingIndex !== undefined) return `Editing queued #${editingIndex + 1}`;
  return `Queued ${Math.max(0, queueCount)}`;
}

export function acpQueuedPromptVisibleRowCount(queueCount: number, expanded: boolean): number {
  if (!expanded) return 0;
  return Math.min(Math.max(0, queueCount), ACP_QUEUED_PROMPT_MAX_VISIBLE_ROWS);
}

export function acpQueuedPromptIndexLabel(index: number): string {
  if (!Number.isInteger(index) || index < 0) return '';
  return `${index + 1}.`;
}

export function acpQueuedPromptAttachmentLabel(count: number): string | undefined {
  if (!Number.isInteger(count) || count <= 0) return undefined;
  return `${count} file`;
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

export function slashCommandItemsForComposer(
  commands: unknown,
  input: string,
  controlsEnabled: boolean,
  limit = 6,
): AcpSlashCommandItem[] {
  if (!controlsEnabled) return [];
  return slashCommandItemsForInput(commands, input, limit);
}
