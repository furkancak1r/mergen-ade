import { spawn, type IPty } from 'node-pty';
import { BrowserWindow } from 'electron';
import type { TerminalKind, ShellKind } from '../shared/types';
import { ANTHROPIC_ENV_VARS_TO_REMOVE, ShellKindCommand } from '../shared/types';
import { normalizeWindowsVerbatimPath } from './config';

export interface PtyCreateOptions {
  shell: ShellKind;
  cwd: string;
  cols: number;
  rows: number;
  env?: Record<string, string>;
  terminalId?: number;
  projectId: number;
  kind: TerminalKind;
}

export interface TerminalSession {
  id: number;
  projectId: number;
  kind: TerminalKind;
  shell: ShellKind;
  cwd: string;
  cols: number;
  rows: number;
  pty: IPty;
  title: string;
  pendingLineForTitle: string;
  pendingInputForHistory: string;
  recentInputs: string[];
  aiTool?: string;
  aiStatus: string;
  aiStatusReason?: string;
  opencodeSessionActive: boolean;
  terminalOutputFocusOverride: boolean;
}

let nextId = 1;
const sessions = new Map<number, TerminalSession>();

export function getSession(terminalId: number): TerminalSession | undefined {
  return sessions.get(terminalId);
}

export function getAllSessions(): TerminalSession[] {
  return Array.from(sessions.values());
}

export function createTerminal(opts: PtyCreateOptions): number {
  const id = opts.terminalId ?? nextId++;
  const [shellCommand, shellArgs] = ShellKindCommand(opts.shell);

  const env: Record<string, string> = { ...process.env as Record<string, string> };
  for (const key of ANTHROPIC_ENV_VARS_TO_REMOVE) {
    delete env[key];
  }
  if (opts.env) {
    Object.assign(env, opts.env);
  }

  const pty = spawn(shellCommand, shellArgs, {
    name: 'xterm-color',
    cols: opts.cols,
    rows: opts.rows,
    cwd: normalizeWindowsVerbatimPath(opts.cwd),
    env,
  });

  const session: TerminalSession = {
    id,
    projectId: opts.projectId,
    kind: opts.kind,
    shell: opts.shell,
    cwd: opts.cwd,
    cols: opts.cols,
    rows: opts.rows,
    pty,
    title: '',
    pendingLineForTitle: '',
    pendingInputForHistory: '',
    recentInputs: [],
    aiStatus: 'inactive',
    opencodeSessionActive: false,
    terminalOutputFocusOverride: false,
  };

  sessions.set(id, session);

  pty.onData((data) => {
    // Update pending buffers for title/history tracking
    for (const ch of data) {
      const code = ch.charCodeAt(0);
      if (ch === '\r' || ch === '\n') {
        session.pendingLineForTitle = '';
      } else if (code >= 0x20 && code !== 0x7f) {
        session.pendingLineForTitle += ch;
        if (session.pendingLineForTitle.length > 512) {
          session.pendingLineForTitle = session.pendingLineForTitle.slice(-512);
        }
      }
    }
    session.pendingInputForHistory += data;

    broadcast('pty:data', id, data);
  });

  pty.onExit(({ exitCode }) => {
    broadcast('pty:exit', id, exitCode ?? 0);
    sessions.delete(id);
  });

  return id;
}

export function writeTerminal(terminalId: number, data: string): void {
  const session = sessions.get(terminalId);
  if (!session) return;

  // Track backspace and printable chars for pending buffers
  for (const ch of data) {
    if (ch === '\b' || ch === '\x7f') {
      session.pendingLineForTitle = session.pendingLineForTitle.slice(0, -1);
      session.pendingInputForHistory = session.pendingInputForHistory.slice(0, -1);
    } else {
      session.pendingInputForHistory += ch;
      const code = ch.charCodeAt(0);
      if (ch === '\r' || ch === '\n') {
        session.pendingLineForTitle = '';
      } else if (code >= 0x20) {
        session.pendingLineForTitle += ch;
        if (session.pendingLineForTitle.length > 512) {
          session.pendingLineForTitle = session.pendingLineForTitle.slice(-512);
        }
      }
    }
  }

  session.pty.write(data);
}

export function resizeTerminal(terminalId: number, cols: number, rows: number): void {
  const session = sessions.get(terminalId);
  if (!session) return;
  session.cols = cols;
  session.rows = rows;
  session.pty.resize(cols, rows);
}

export function killTerminal(terminalId: number, signal?: string): void {
  const session = sessions.get(terminalId);
  if (!session) return;
  if (signal) {
    session.pty.kill(signal);
  } else {
    session.pty.kill();
  }
  sessions.delete(terminalId);
}

function broadcast(channel: string, terminalId: number, data: unknown) {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(channel, terminalId, data);
    }
  }
}
