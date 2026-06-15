import type { AcpStartupMode } from './types';
import { AcpStartupModeAsModeId } from './types';

export type JsonRpcId = string | number;

export interface AcpPermissionOptionLike {
  optionId?: string;
  id?: string;
  name?: string;
}

export function startupModeToModeId(mode: AcpStartupMode | undefined): string {
  return mode ? AcpStartupModeAsModeId(mode) : 'build';
}

export function jsonRpcIdToString(id: unknown): string {
  if (typeof id === 'string') return id;
  if (typeof id === 'number' && Number.isFinite(id)) return String(id);
  return '';
}

export function isJsonRpcId(id: unknown): id is JsonRpcId {
  return (typeof id === 'string' && id.length > 0) || (typeof id === 'number' && Number.isFinite(id));
}

export function createAcpRequestIdGenerator(start = 1): () => number {
  let next = Number.isFinite(start) ? Math.max(1, Math.floor(start)) : 1;
  return () => next++;
}

export function permissionRequestIdFromRpc(id: unknown): string {
  return jsonRpcIdToString(id);
}

export function permissionOptionId(option: AcpPermissionOptionLike | undefined): string {
  return option?.optionId || option?.id || '';
}

export function firstAutoApproveOptionId(options: AcpPermissionOptionLike[], enabled: boolean): string | undefined {
  if (!enabled) return undefined;
  const first = options.find((option) => permissionOptionId(option).length > 0);
  return first ? permissionOptionId(first) : undefined;
}

export function buildAcpPermissionResponse(requestId: JsonRpcId, optionId: string, rejected = false) {
  return {
    jsonrpc: '2.0',
    id: requestId,
    result: {
      outcome: rejected
        ? { outcome: 'cancelled' }
        : {
            outcome: 'selected',
            optionId,
          },
    },
  };
}

export function buildAcpQuestionResponse(requestId: JsonRpcId, answers: string[][], rejected = false) {
  return {
    jsonrpc: '2.0',
    id: requestId,
    result: rejected
      ? { rejected: true }
      : { answers },
  };
}

export function buildAcpCancelNotification(sessionId: string) {
  return {
    jsonrpc: '2.0',
    method: 'session/cancel',
    params: {
      sessionId,
    },
  };
}

export function stripAnsi(text: string): string {
  return text.replace(/[\u001B\u009B][[\]()#;?]*(?:(?:(?:[a-zA-Z\d]*(?:;[a-zA-Z\d]*)*)?\u0007)|(?:(?:\d{1,4}(?:;\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~]))/g, '');
}

export function isAcpCancelNoise(text: string): boolean {
  const lower = stripAnsi(text).toLowerCase();
  return lower.includes('session/cancel') || lower.includes('method not found') || lower.includes('-32601');
}

export function isAcpCancelUnsupported(text: string): boolean {
  const lower = stripAnsi(text).toLowerCase();
  return lower.includes('method not found') || lower.includes('-32601');
}

export function isAcpUnknownResponseWarning(text: string): boolean {
  return stripAnsi(text).toLowerCase().includes('got response to unknown request');
}

export function acpUnknownResponseWarningText(text: string): string {
  const clean = stripAnsi(text).trim();
  return clean
    ? `Ignored a stale ACP response after it was already handled. (${clean})`
    : 'Ignored a stale ACP response after it was already handled.';
}

export function isAcpJsonParseError(text: string): boolean {
  return stripAnsi(text).includes('ACP JSON parse error');
}

export function isAcpErrorFatalForSession(text: string, hasSessionId: boolean, status: string | undefined): boolean {
  if (isAcpJsonParseError(text)) return true;
  return !(hasSessionId || status === 'starting');
}

export function isAcpClearCommand(text: string, attachments: readonly unknown[] = []): boolean {
  return attachments.length === 0 && text.trim().toLowerCase() === '/clear';
}
