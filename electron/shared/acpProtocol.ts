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

export function permissionRequestIdFromRpc(id: unknown, params: Record<string, unknown>): string {
  return jsonRpcIdToString(id) || jsonRpcIdToString(params.requestId);
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
    id: String(requestId),
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
