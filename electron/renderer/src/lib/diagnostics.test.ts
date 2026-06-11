import { describe, expect, it } from 'vitest';
import type { AppDiagnostics } from '../../../shared/types';
import { AiCliStatus, AiCliTool, TerminalKind } from '../../../shared/types';
import { runtimeOverview, type ActiveTerminalDiagnostics } from './diagnostics';

function diagnostics(partial: Partial<AppDiagnostics> = {}): AppDiagnostics {
  return {
    appVersion: '0.1.40',
    platform: 'win32',
    arch: 'x64',
    electronVersion: '31.0.0',
    chromeVersion: '126.0.0',
    nodeVersion: '20.14.0',
    execPath: 'C:\\app\\mergen-ade.exe',
    cwd: 'C:\\repo',
    configPath: 'C:\\data\\config.json',
    legacyConfigPath: 'C:\\data\\config.toml',
    historyPath: 'C:\\data\\history.json',
    hookInboxDir: 'C:\\data\\hooks',
    hookServicePort: 4321,
    codexInboxDir: 'C:\\data\\codex',
    codexHooksPath: 'C:\\Users\\test\\.codex\\hooks.json',
    codexHooksInstalled: true,
    codexBridgePath: 'C:\\data\\bin\\mergen-codex-bridge.exe',
    codexBridgeInstalled: false,
    browserMcpCommand: ['mergen-ade.exe', '--browser-mcp-helper'],
    browserMcpSessionCount: 0,
    ...partial,
  };
}

function activeTerminal(partial: Partial<ActiveTerminalDiagnostics> = {}): ActiveTerminalDiagnostics {
  return {
    id: 7,
    title: 'OpenCode',
    cwd: 'C:\\repo',
    kind: TerminalKind.Foreground,
    aiTool: AiCliTool.OpenCode,
    aiStatus: AiCliStatus.Running,
    opencodeSessionActive: true,
    exited: false,
    ...partial,
  };
}

describe('diagnostics runtimeOverview', () => {
  it('reports loading before diagnostics arrive', () => {
    expect(runtimeOverview(undefined, undefined)).toMatchObject({
      severity: 'loading',
      title: 'Loading runtime diagnostics',
    });
  });

  it('warns when the hook service is not listening', () => {
    expect(runtimeOverview(diagnostics({ hookServicePort: 0 }), activeTerminal())).toMatchObject({
      severity: 'warning',
      title: 'Hook bridge is not listening',
    });
  });

  it('warns when Codex hooks are not installed', () => {
    expect(runtimeOverview(diagnostics({ codexHooksInstalled: false }), activeTerminal())).toMatchObject({
      severity: 'warning',
      title: 'Codex hooks are not installed',
    });
  });

  it('reports healthy runtime with active terminal state', () => {
    expect(runtimeOverview(diagnostics(), activeTerminal())).toEqual({
      severity: 'healthy',
      title: 'Runtime integrations look healthy',
      message: 'Active terminal 7 is running.',
    });
  });
});
