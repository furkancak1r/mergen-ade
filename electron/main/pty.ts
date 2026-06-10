import { spawn, type IPty } from 'node-pty';
import { BrowserWindow } from 'electron';
import type { TerminalKind, ShellKind } from '../shared/types';
import { ANTHROPIC_ENV_VARS_TO_REMOVE, ShellKindCommand } from '../shared/types';
import { normalizeWindowsVerbatimPath } from './config';
import { getBrowserMcpToken, getHookServicePort } from './hookService';
import {
  MERGEN_BROWSER_MCP_PORT_ENV_VAR,
  MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR,
  MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR,
  MERGEN_BROWSER_MCP_TOKEN_ENV_VAR,
} from './browserMcpTools';

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
  opencodeLastHookEventSince?: number;
}

export function getTerminalState(terminalId: number): Pick<TerminalSession, 'pendingLineForTitle' | 'pendingInputForHistory' | 'recentInputs' | 'title' | 'aiStatus' | 'aiStatusReason'> | undefined {
  const s = sessions.get(terminalId);
  if (!s) return undefined;
  return {
    pendingLineForTitle: s.pendingLineForTitle,
    pendingInputForHistory: s.pendingInputForHistory,
    recentInputs: s.recentInputs,
    title: s.title,
    aiStatus: s.aiStatus,
    aiStatusReason: s.aiStatusReason,
  };
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
  // Set Mergen-specific env vars for hook plugin integration
  env['MERGEN_TERMINAL_ID'] = String(id);
  const hookPort = getHookServicePort();
  if (hookPort) {
    env['MERGEN_HOOK_PORT'] = String(hookPort);
    env[MERGEN_BROWSER_MCP_PORT_ENV_VAR] = String(hookPort);
    env[MERGEN_BROWSER_MCP_TOKEN_ENV_VAR] = getBrowserMcpToken();
    env[MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR] = String(id);
    env[MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR] = String(opts.projectId);
  }
  if (opts.env) {
    Object.assign(env, opts.env);
  }

  let pty: IPty;
  try {
    pty = spawn(shellCommand, shellArgs, {
      name: 'xterm-color',
      cols: opts.cols,
      rows: opts.rows,
      cwd: normalizeWindowsVerbatimPath(opts.cwd),
      env,
    });
  } catch (err) {
    console.error('Failed to spawn PTY:', err);
    throw err;
  }

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

  // OSC sequence parsing for Claude Code title detection
  let oscBuffer = '';
  pty.onData((data) => {
    // PTY output does not affect user input history buffers
    broadcast('pty:data', id, data);

    // Parse OSC 0/1/2 sequences for title changes (Claude Code detection)
    oscBuffer += data;
    // Process complete OSC sequences: ESC ] <num> ; <text> BEL  or  ESC ] <num> ; <text> ESC \
    const oscPattern = /\x1b\](0|1|2);([^\x07\x1b]*)\x07|\x1b\](0|1|2);([^\x07\x1b]*)\x1b\\/g;
    let match: RegExpExecArray | null;
    while ((match = oscPattern.exec(oscBuffer)) !== null) {
      const title = (match[2] || match[4] || '').trim();
      if (title) {
        // Detect Claude Code or Orca in title
        const lowerTitle = title.toLowerCase();
        if (lowerTitle.includes('claude') || lowerTitle.includes('orca')) {
          const s = sessions.get(id);
          if (s) {
            s.aiTool = 'claude';
            s.aiStatus = 'running';
            s.aiStatusReason = title;
          }
          broadcast('hook:status', id, {
            terminalId: id,
            tool: 'claude',
            status: 'running',
            reason: title,
            eventKind: 'title.update',
          });
        } else {
          // If previously detected as Claude and now title doesn't match, mark inactive
          const s = sessions.get(id);
          if (s && s.aiTool === 'claude') {
            s.aiTool = undefined;
            s.aiStatus = 'inactive';
            s.aiStatusReason = title;
            broadcast('hook:status', id, {
              terminalId: id,
              tool: 'claude',
              status: 'inactive',
              reason: title,
              eventKind: 'title.update',
            });
          }
        }
      }
    }
    // Trim processed buffer to avoid unbounded growth
    const lastBell = oscBuffer.lastIndexOf('\x07');
    const lastEsc = oscBuffer.lastIndexOf('\x1b');
    const trimPos = Math.max(lastBell, lastEsc);
    if (trimPos >= 0 && trimPos < oscBuffer.length - 1) {
      oscBuffer = oscBuffer.slice(trimPos);
    }
    if (oscBuffer.length > 4096) {
      oscBuffer = oscBuffer.slice(-1024);
    }
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
  // Filter out bracketed paste, CSI, and OSC sequences from input history
  const textForHistory = data
    .replace(/\x1b\[200~[\s\S]*?\x1b\[201~/g, '') // bracketed paste
    .replace(/\x1b\[[\x30-\x3f]*[\x20-\x2f]*[\x40-\x7e]/g, '') // CSI sequences (ECMA-48: catches SGR mouse, arrow keys, etc.)
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, ''); // OSC sequences
  let recentInputsChanged = false;
  let titleChanged = false;
  for (const ch of textForHistory) {
    if (ch === '\b' || ch === '\x7f') {
      // Char-safe backspace using Array.from for surrogate pairs
      const titleChars = Array.from(session.pendingLineForTitle);
      titleChars.pop();
      session.pendingLineForTitle = titleChars.join('');
      const histChars = Array.from(session.pendingInputForHistory);
      histChars.pop();
      session.pendingInputForHistory = histChars.join('');
    } else {
      const code = ch.charCodeAt(0);
      // Include printable chars, tabs, and newlines in history; skip other control chars
      if (code >= 0x20 || ch === '\r' || ch === '\n' || ch === '\t') {
        session.pendingInputForHistory += ch;
      }
      if (ch === '\r' || ch === '\n') {
        // On Enter, record history from full raw text and clear both buffers
        const historyLine = session.pendingInputForHistory;
        const trimmed = historyLine.trim();
        if (trimmed && !trimmed.startsWith('/')) {
          session.recentInputs.unshift(trimmed);
          if (session.recentInputs.length > 20) {
            session.recentInputs.pop();
          }
          recentInputsChanged = true;
        }
        session.pendingLineForTitle = '';
        session.pendingInputForHistory = '';
      } else if (code >= 0x20) {
        session.pendingLineForTitle += ch;
        if (session.pendingLineForTitle.length > 512) {
          // Char-safe truncation for title
          const chars = Array.from(session.pendingLineForTitle);
          session.pendingLineForTitle = chars.slice(-512).join('');
        }
        titleChanged = true;
      }
    }
  }

  if (recentInputsChanged || titleChanged) {
    session.title = session.pendingLineForTitle;
    broadcast('pty:state', session.id, {
      recentInputs: session.recentInputs,
      title: session.title,
    });
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
