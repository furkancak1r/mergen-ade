import { spawn, type ChildProcess } from 'child_process';
import { BrowserWindow } from 'electron';
import type { AcpChatSession, AcpConfigOption, AcpAvailableCommand, AcpChatMessage } from '../shared/types';

interface AcpSession {
  process: ChildProcess;
  sessionId?: string;
  chatId: string;
  projectId: number;
  cwd: string;
  status: AcpChatSession['status'];
  messages: AcpChatMessage[];
  promptInput: string;
  attachments: string[];
  configOptions: AcpConfigOption[];
  currentModeId?: string;
  currentModel?: string;
  currentEffort?: string;
  availableCommands?: AcpAvailableCommand[];
  queuedPrompts: { text: string; attachments: string[]; modeId: string; finalPromptText: string }[];
  partialStderr?: string;
  cancelGraceUntil?: number;
}

const sessions = new Map<string, AcpSession>();

function broadcast(channel: string, ...args: unknown[]) {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(channel, ...args);
    }
  }
}

function sendRpc(session: AcpSession, method: string, params: unknown): void {
  const req = { jsonrpc: '2.0', id: Date.now(), method, params };
  session.process.stdin?.write(JSON.stringify(req) + '\n');
}

export async function spawnAcpChat(opts: { projectId: number; cwd: string; mcpServers: string[] }): Promise<string> {
  const chatId = `acp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

  const proc = spawn('opencode', ['acp'], {
    cwd: opts.cwd,
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  });

  const session: AcpSession = {
    process: proc,
    chatId,
    projectId: opts.projectId,
    cwd: opts.cwd,
    status: 'starting',
    messages: [],
    promptInput: '',
    attachments: [],
    configOptions: [],
    queuedPrompts: [],
  };

  sessions.set(chatId, session);

  let buffer = '';
  proc.stdout?.on('data', (data) => {
    buffer += data.toString();
    let idx;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (line.trim()) {
        handleAcpLine(session, line);
      }
    }
  });

  proc.stderr?.on('data', (data) => {
    const text = data.toString();
    const now = Date.now();
    if (session.cancelGraceUntil && now < session.cancelGraceUntil) {
      // Suppress cancel-specific stderr noise
      if (text.includes('Method not found') || text.includes('session/cancel') || text.includes('-32601')) {
        return;
      }
    }
    session.partialStderr = (session.partialStderr || '') + text;
    const msg: AcpChatMessage = { role: 'system', text: `ACP stderr: ${text.trim()}`, timestamp: Date.now() };
    session.messages.push(msg);
    broadcast('acp:event', chatId, { type: 'error', text: text.trim() });
  });

  proc.on('exit', (code) => {
    session.status = 'error';
    broadcast('acp:event', chatId, { type: 'exit', code });
  });

  // Send initialize
  sendRpc(session, 'initialize', {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'opencode-local-acp', version: '1.0.0' },
  });

  return chatId;
}

function handleAcpLine(session: AcpSession, line: string): void {
  try {
    const msg = JSON.parse(line) as Record<string, unknown>;
    if (msg.method === 'initialized') {
      // Send session/new
      sendRpc(session, 'session/new', {
        cwd: session.cwd,
        mcpServers: [],
      });
    } else if (msg.method === 'session/new') {
      const result = msg.result as Record<string, unknown>;
      session.sessionId = (result.sessionId as string) || undefined;
      session.status = 'session_created';
      if (result.configOptions) {
        session.configOptions = result.configOptions as AcpConfigOption[];
      }
      broadcast('acp:event', session.chatId, { type: 'sessionCreated', sessionId: session.sessionId });
    } else if (msg.method === 'session/prompt') {
      const result = msg.result as Record<string, unknown>;
      const response: AcpChatMessage = {
        role: 'assistant',
        text: (result.text as string) || '',
        timestamp: Date.now(),
      };
      session.messages.push(response);
      session.status = 'idle';
      broadcast('acp:event', session.chatId, { type: 'promptResponse', text: response.text });
    } else if (msg.method === 'config_option_update') {
      const params = msg.params as Record<string, unknown>;
      const options = params.configOptions as AcpConfigOption[] | undefined;
      if (options) {
        session.configOptions = options;
        const modelOpt = options.find((o) => o.id === 'model');
        if (modelOpt) session.currentModel = modelOpt.currentValue;
        const effortOpt = options.find((o) => o.id === 'effort');
        if (effortOpt) session.currentEffort = effortOpt.currentValue;
      }
      broadcast('acp:event', session.chatId, { type: 'configOptions', options });
    } else if (msg.method === 'current_mode_update') {
      const params = msg.params as Record<string, unknown>;
      const modeId = (params.currentModeId as string) || (params.modeId as string);
      if (modeId) session.currentModeId = modeId;
      broadcast('acp:event', session.chatId, { type: 'modeUpdate', modeId });
    } else if (msg.method === 'available_commands_update') {
      const params = msg.params as Record<string, unknown>;
      const commands = (params.availableCommands as AcpAvailableCommand[]) || (params.commands as AcpAvailableCommand[]);
      if (commands) session.availableCommands = commands;
      broadcast('acp:event', session.chatId, { type: 'commands', commands });
    } else if (msg.method === 'permission_request') {
      const params = msg.params as Record<string, unknown>;
      session.status = 'permission';
      broadcast('acp:event', session.chatId, { type: 'permission', requestId: params.requestId, message: params.message });
    } else {
      // Unknown notification
      broadcast('acp:event', session.chatId, { type: 'raw', message: msg });
    }
  } catch {
    // ignore non-JSON lines
  }
}

export function sendAcpPrompt(chatId: string, promptText: string, attachments: string[]): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  if (session.status === 'running' || session.status === 'permission') {
    // Queue the prompt
    session.queuedPrompts.push({ text: promptText, attachments, modeId: session.currentModeId || 'build', finalPromptText: promptText });
    broadcast('acp:event', chatId, { type: 'queued', count: session.queuedPrompts.length });
    return;
  }

  session.status = 'running';
  const msg: AcpChatMessage = { role: 'user', text: promptText, timestamp: Date.now() };
  session.messages.push(msg);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, promptText, attachments });
  broadcast('acp:event', chatId, { type: 'promptSent', text: promptText });
}

export function cancelAcpPrompt(chatId: string): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  session.cancelGraceUntil = Date.now() + 2000;
  sendRpc(session, 'session/cancel', { sessionId: session.sessionId });
  session.status = 'idle';
  broadcast('acp:event', chatId, { type: 'cancelled' });
}

export function setAcpConfigOption(chatId: string, configId: string, value: string): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId, value });
}

export function getAcpSession(chatId: string): AcpChatSession | undefined {
  const session = sessions.get(chatId);
  if (!session) return undefined;
  return {
    sessionId: session.sessionId,
    status: session.status,
    messages: session.messages,
    promptInput: session.promptInput,
    attachments: session.attachments,
    configOptions: session.configOptions,
    currentModeId: session.currentModeId,
    currentModel: session.currentModel,
    currentEffort: session.currentEffort,
    availableCommands: session.availableCommands,
    queuedPrompts: session.queuedPrompts,
    partialStderr: session.partialStderr,
  };
}

export function killAcpChat(chatId: string): void {
  const session = sessions.get(chatId);
  if (!session) return;
  session.process.kill();
  sessions.delete(chatId);
}
