import type { AppDiagnostics, AiCliTool, TerminalKind } from '../../../shared/types';

export interface ActiveTerminalDiagnostics {
  id: number;
  title: string;
  cwd: string;
  kind: TerminalKind;
  aiTool?: AiCliTool;
  aiStatus: string;
  aiStatusReason?: string;
  opencodeSessionActive: boolean;
  opencodeTransportStatus?: string;
  opencodeAttentionReason?: string;
  exited: boolean;
}

export type DiagnosticsSeverity = 'loading' | 'healthy' | 'warning';

export interface RuntimeOverview {
  severity: DiagnosticsSeverity;
  title: string;
  message: string;
}

export function runtimeOverview(
  diagnostics: AppDiagnostics | undefined,
  activeTerminal: ActiveTerminalDiagnostics | undefined,
): RuntimeOverview {
  if (!diagnostics) {
    return {
      severity: 'loading',
      title: 'Loading runtime diagnostics',
      message: 'Collecting Electron runtime, hook bridge, and terminal state.',
    };
  }

  if (diagnostics.hookServicePort <= 0) {
    return {
      severity: 'warning',
      title: 'Hook bridge is not listening',
      message: 'AI CLI status updates may not reach the UI until the hook service starts.',
    };
  }

  if (!diagnostics.codexHooksInstalled) {
    return {
      severity: 'warning',
      title: 'Codex hooks are not installed',
      message: 'Codex CLI sessions need hook configuration before permission and turn-complete status can be tracked.',
    };
  }

  if (activeTerminal?.exited) {
    return {
      severity: 'warning',
      title: 'Active terminal has exited',
      message: `Terminal ${activeTerminal.id} is no longer running.`,
    };
  }

  return {
    severity: 'healthy',
    title: 'Runtime integrations look healthy',
    message: activeTerminal
      ? `Active terminal ${activeTerminal.id} is ${activeTerminal.aiStatus}.`
      : 'No active terminal is selected, but the application bridge is available.',
  };
}

export function diagnosticsColor(severity: DiagnosticsSeverity): string {
  switch (severity) {
    case 'healthy':
      return '#64c38c';
    case 'warning':
      return '#dcaa3c';
    case 'loading':
      return '#888';
  }
}
