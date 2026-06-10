import { describe, expect, it } from 'vitest';
import { AcpStartupMode } from '../../../shared/types';
import {
  buildAcpPermissionResponse,
  firstAutoApproveOptionId,
  jsonRpcIdToString,
  permissionRequestIdFromRpc,
  startupModeToModeId,
} from '../../../shared/acpProtocol';

describe('acpProtocol', () => {
  it('maps startup mode to ACP mode id', () => {
    expect(startupModeToModeId(AcpStartupMode.Plan)).toBe('plan');
    expect(startupModeToModeId(AcpStartupMode.Build)).toBe('build');
    expect(startupModeToModeId(undefined)).toBe('build');
  });

  it('preserves JSON-RPC request ids as strings', () => {
    expect(jsonRpcIdToString('req-1')).toBe('req-1');
    expect(jsonRpcIdToString(42)).toBe('42');
    expect(jsonRpcIdToString(null)).toBe('');
  });

  it('prefers the JSON-RPC id over legacy params requestId', () => {
    expect(permissionRequestIdFromRpc('rpc-id', { requestId: 'param-id' })).toBe('rpc-id');
    expect(permissionRequestIdFromRpc(undefined, { requestId: 7 })).toBe('7');
  });

  it('selects the first concrete permission option only when auto approve is enabled', () => {
    expect(firstAutoApproveOptionId([{ optionId: '' }, { optionId: 'allow' }], true)).toBe('allow');
    expect(firstAutoApproveOptionId([{ optionId: 'allow' }], false)).toBeUndefined();
    expect(firstAutoApproveOptionId([], true)).toBeUndefined();
  });

  it('builds an ACP permission JSON-RPC response, not a method call', () => {
    expect(buildAcpPermissionResponse(42, 'allow')).toEqual({
      jsonrpc: '2.0',
      id: '42',
      result: {
        outcome: {
          outcome: 'selected',
          optionId: 'allow',
        },
      },
    });
  });

  it('builds an ACP permission cancellation response', () => {
    expect(buildAcpPermissionResponse('req-1', '', true)).toEqual({
      jsonrpc: '2.0',
      id: 'req-1',
      result: {
        outcome: {
          outcome: 'cancelled',
        },
      },
    });
  });
});
