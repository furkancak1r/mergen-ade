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
