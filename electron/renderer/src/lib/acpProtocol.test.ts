import { describe, expect, it } from 'vitest';
import { AcpStartupMode } from '../../../shared/types';
import {
  buildAcpPermissionResponse,
  buildAcpQuestionResponse,
  firstAutoApproveOptionId,
  isJsonRpcId,
  jsonRpcIdToString,
  permissionRequestIdFromRpc,
  stripAnsi,
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

  it('accepts string and numeric JSON-RPC ids only', () => {
    expect(isJsonRpcId('req-1')).toBe(true);
    expect(isJsonRpcId(0)).toBe(true);
    expect(isJsonRpcId('')).toBe(false);
    expect(isJsonRpcId(null)).toBe(false);
  });

  it('uses only the JSON-RPC id for ACP permission responses', () => {
    expect(permissionRequestIdFromRpc('rpc-id')).toBe('rpc-id');
    expect(permissionRequestIdFromRpc(0)).toBe('0');
    expect(permissionRequestIdFromRpc(undefined)).toBe('');
  });

  it('selects the first concrete permission option only when auto approve is enabled', () => {
    expect(firstAutoApproveOptionId([{ optionId: '' }, { optionId: 'allow' }], true)).toBe('allow');
    expect(firstAutoApproveOptionId([{ optionId: 'allow' }], false)).toBeUndefined();
    expect(firstAutoApproveOptionId([], true)).toBeUndefined();
  });

  it('builds an ACP permission JSON-RPC response, not a method call', () => {
    expect(buildAcpPermissionResponse(42, 'allow')).toEqual({
      jsonrpc: '2.0',
      id: 42,
      result: {
        outcome: {
          outcome: 'selected',
          optionId: 'allow',
        },
      },
    });
  });

  it('builds an ACP question JSON-RPC response', () => {
    expect(buildAcpQuestionResponse(0, [['Yes']])).toEqual({
      jsonrpc: '2.0',
      id: 0,
      result: {
        answers: [['Yes']],
      },
    });
  });

  it('builds an ACP question rejection response', () => {
    expect(buildAcpQuestionResponse('req-1', [], true)).toEqual({
      jsonrpc: '2.0',
      id: 'req-1',
      result: {
        rejected: true,
      },
    });
  });

  it('strips ANSI control sequences from ACP stderr text', () => {
    expect(stripAnsi('\u001b[0m\u001b[31mGot response to unknown request 0\u001b[0m')).toBe('Got response to unknown request 0');
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
