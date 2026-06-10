export interface SmartInputTask {
  text: string;
  attachments: { path: string }[];
  modeId?: string;
}

export const SMART_INPUT_AUTO_DISPATCH_SETTLE_MS = 300;

export function isStaleOpencodeCompletion(
  opencodePromptSubmitSince: number | undefined,
  now: number,
): boolean {
  if (!opencodePromptSubmitSince) return false;
  return now - opencodePromptSubmitSince < SMART_INPUT_AUTO_DISPATCH_SETTLE_MS;
}

export function canAutoDispatch(
  queue: SmartInputTask[],
  opencodeTransportStatus: string | undefined,
  aiStatus: string,
  aiStatusReason: string | undefined,
  opencodePendingQuestion: boolean,
  opencodePromptSubmitSince: number | undefined,
  opencodeThoughtLoopBlocked: boolean,
  opencodeLoopLimitEmitted: boolean,
  now: number,
): boolean {
  if (queue.length === 0) return false;
  if (opencodePendingQuestion) return false;
  if (opencodeThoughtLoopBlocked) return false;
  if (opencodeLoopLimitEmitted) return false;

  const isIdle = opencodeTransportStatus === 'Idle';
  const hasTurnComplete = aiStatus === 'attention' && aiStatusReason === 'TurnComplete';
  if (!isIdle && !hasTurnComplete) return false;

  if (isStaleOpencodeCompletion(opencodePromptSubmitSince, now)) return false;

  return true;
}

export function canAutoDispatchClaude(
  queue: SmartInputTask[],
  aiStatus: string,
  aiAttentionKind: string | undefined,
  promptSubmitSince: number | undefined,
  now: number,
): boolean {
  if (queue.length === 0) return false;
  if (aiStatus !== 'attention' || aiAttentionKind !== 'turn_complete') return false;
  if (isStaleOpencodeCompletion(promptSubmitSince, now)) return false;
  return true;
}

export function shouldShowSmartInputFooter(
  terminalKind: string,
  aiTool: string | undefined,
  aiStatus: string | undefined,
  opencodeSessionActive: boolean,
): boolean {
  if (terminalKind !== 'foreground') return false;
  if (aiTool === 'opencode') return opencodeSessionActive;
  if (aiTool === 'claude') return aiStatus === 'running' || aiStatus === 'attention';
  return false;
}

export function smartInputFooterHeight(
  visibleTaskRows: number,
  expanded: boolean,
  hasTasks: boolean,
  draftHeight: number,
  userHeight: number | undefined,
  safeMin: number,
  maxFooter: number,
): number {
  const computed = safeMin + (expanded ? draftHeight : 0);
  const desired = userHeight ?? computed;
  return Math.max(safeMin, Math.min(desired, maxFooter));
}

export function selectionEdgeAutoscrollDelta(
  pointerY: number,
  viewportTop: number,
  viewportBottom: number,
  lineHeight: number,
): number {
  const edgeZone = lineHeight * 2;
  if (pointerY < viewportTop + edgeZone) {
    const distance = viewportTop + edgeZone - pointerY;
    const speed = Math.min(8, Math.max(1, Math.ceil(distance / lineHeight)));
    return -speed * lineHeight;
  }
  if (pointerY > viewportBottom - edgeZone) {
    const distance = pointerY - (viewportBottom - edgeZone);
    const speed = Math.min(8, Math.max(1, Math.ceil(distance / lineHeight)));
    return speed * lineHeight;
  }
  return 0;
}
