import { describe, expect, it } from 'vitest';
import { AcpStartupMode } from '../../../shared/types';
import {
  acpUnknownResponseWarningText,
  buildAcpCancelNotification,
  buildAcpPermissionResponse,
  buildAcpQuestionResponse,
  createAcpRequestIdGenerator,
  firstAutoApproveOptionId,
  isAcpCancelNoise,
  isAcpCancelUnsupported,
  isAcpErrorFatalForSession,
  isAcpJsonParseError,
  isAcpUnknownResponseWarning,
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

  it('generates monotonic ACP request ids', () => {
    const nextId = createAcpRequestIdGenerator();
    expect([nextId(), nextId(), nextId()]).toEqual([1, 2, 3]);
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

  it('builds ACP cancel as a JSON-RPC notification without an id', () => {
    expect(buildAcpCancelNotification('sess-1')).toEqual({
      jsonrpc: '2.0',
      method: 'session/cancel',
      params: {
        sessionId: 'sess-1',
      },
    });
  });

  it('strips ANSI control sequences from ACP stderr text', () => {
    expect(stripAnsi('\u001b[0m\u001b[31mGot response to unknown request 0\u001b[0m')).toBe('Got response to unknown request 0');
  });

  it('classifies cancel stderr and error-response noise', () => {
    expect(isAcpCancelNoise('{"code":-32601,"message":"Method not found: session/cancel"}')).toBe(true);
    expect(isAcpCancelNoise('regular failure')).toBe(false);
  });

  it('classifies unsupported cancel errors separately from generic cancel noise', () => {
    expect(isAcpCancelUnsupported('{"code":-32601,"message":"Method not found: session/cancel"}')).toBe(true);
    expect(isAcpCancelUnsupported('Error handling notification for session/cancel')).toBe(false);
  });

  it('normalizes stale unknown-response ACP warnings', () => {
    const raw = '\u001b[31mGot response to unknown request 0\u001b[0m';
    expect(isAcpUnknownResponseWarning(raw)).toBe(true);
    expect(acpUnknownResponseWarningText(raw)).toBe('Ignored a stale ACP response after it was already handled. (Got response to unknown request 0)');
  });

  it('treats only ACP JSON parse errors as fatal once a session exists or is starting', () => {
    expect(isAcpJsonParseError('ACP JSON parse error: Unexpected token')).toBe(true);
    expect(isAcpErrorFatalForSession('Authentication failed', true, 'idle')).toBe(false);
    expect(isAcpErrorFatalForSession('Authentication failed', false, 'starting')).toBe(false);
    expect(isAcpErrorFatalForSession('Authentication failed', false, 'idle')).toBe(true);
    expect(isAcpErrorFatalForSession('ACP JSON parse error: Unexpected token', true, 'idle')).toBe(true);
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
