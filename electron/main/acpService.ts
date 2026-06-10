import { spawn, type ChildProcess } from 'child_process';
import { BrowserWindow } from 'electron';
import type { AcpChatSession, AcpConfigOption, AcpAvailableCommand, AcpChatMessage, OpenCodeModelConfig } from '../shared/types';
import { activeBuildModel, effectivePlanModel, effectivePlanEffort } from '../shared/types';
import { loadConfig } from './config';
import { getOpencodeBinPath } from './opencode';

function buildAcpPromptText(text: string, attachments: string[]): string {
  if (attachments.length === 0) return text;
  const lines = attachments.map((a) => `- ${a}`);
  const attachmentBlock = `Attached file paths:\n${lines.join('\n')}`;
  if (!text.trim()) return attachmentBlock;
  return `${text}\n\n${attachmentBlock}`;
}

interface AcpSession {
  process: ChildProcess;
  sessionId?: string;
  chatId: string;
  projectId: number;
  cwd: string;
  mcpServers: string[];
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

// Standby pool: per-project warmed session
interface AcpStandbyEntry {
  chatId: string;
  sessionId?: string;
  status: AcpChatSession['status'];
  projectId: number;
  retryCooldownUntil?: number;
}
const standbyPool = new Map<number, AcpStandbyEntry>();
const ACP_STANDBY_RETRY_COOLDOWN = 10000;

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

function applyModeModelBinding(session: AcpSession, modeId: string): void {
  if (!session.sessionId) return;
  const config = loadConfig();
  const modelConfig: OpenCodeModelConfig = config.opencode;
  if (!modelConfig.acpBindModelToMode) return;

  const targetModel = modeId === 'plan' ? effectivePlanModel(modelConfig) : activeBuildModel(modelConfig);
  const targetEffort = modeId === 'plan' ? effectivePlanEffort(modelConfig) : '';

  if (targetModel && targetModel !== session.currentModel) {
    sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId: 'model', value: targetModel });
    session.currentModel = targetModel;
  }
  if (targetEffort && targetEffort !== session.currentEffort) {
    sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId: 'effort', value: targetEffort });
    session.currentEffort = targetEffort;
  }
}

export async function spawnAcpChat(opts: { projectId: number; cwd: string; mcpServers: string[] }): Promise<string> {
  const chatId = `acp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

  const opencodeBin = getOpencodeBinPath();
  const proc = spawn(opencodeBin, ['acp'], {
    cwd: opts.cwd,
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
  });

  const session: AcpSession = {
    process: proc,
    chatId,
    projectId: opts.projectId,
    cwd: opts.cwd,
    mcpServers: opts.mcpServers,
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
    const method = msg.method as string | undefined;
    const result = msg.result as Record<string, unknown> | undefined;
    const id = msg.id as number | string | undefined;

    // Handle responses (have result + id) and notifications (have method)
    // Response to initialize -> send session/new
    if ((method === 'initialized' && result) || (result && id !== undefined && !method && result.protocolVersion !== undefined)) {
      sendRpc(session, 'session/new', {
        cwd: session.cwd,
        mcpServers: session.mcpServers,
      });
    } else if (method === 'session/new' || (result && result.sessionId !== undefined)) {
      const resultObj = result || (msg as Record<string, unknown>);
      session.sessionId = (resultObj.sessionId as string) || undefined;
      session.status = 'idle';
      if (resultObj.configOptions) {
        session.configOptions = resultObj.configOptions as AcpConfigOption[];
      }
      updateAcpStandbyStatus(session.chatId, 'idle', session.sessionId);
      broadcast('acp:event', session.chatId, { type: 'sessionCreated', sessionId: session.sessionId });
    } else if (method === 'session/prompt' || (result && result.text !== undefined)) {
      const resultObj = result || (msg as Record<string, unknown>);
      const response: AcpChatMessage = {
        role: 'assistant',
        text: (resultObj.text as string) || '',
        timestamp: Date.now(),
      };
      session.messages.push(response);
      session.status = 'idle';
      updateAcpStandbyStatus(session.chatId, 'idle');
      broadcast('acp:event', session.chatId, { type: 'promptResponse', text: response.text, queuedPrompts: session.queuedPrompts.length });
      // Flush queued prompts one at a time
      flushNextQueuedPrompt(session);
    } else if (method === 'config_option_update') {
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
    } else if (method === 'current_mode_update') {
      const params = msg.params as Record<string, unknown>;
      const modeId = (params.currentModeId as string) || (params.modeId as string);
      if (modeId) {
        session.currentModeId = modeId;
        applyModeModelBinding(session, modeId);
      }
      broadcast('acp:event', session.chatId, { type: 'modeUpdate', modeId });
    } else if (method === 'available_commands_update') {
      const params = msg.params as Record<string, unknown>;
      const commands = (params.availableCommands as AcpAvailableCommand[]) || (params.commands as AcpAvailableCommand[]);
      if (commands) session.availableCommands = commands;
      broadcast('acp:event', session.chatId, { type: 'commands', commands });
    } else if (method === 'permission_request') {
      const params = msg.params as Record<string, unknown>;
      session.status = 'permission';
      updateAcpStandbyStatus(session.chatId, 'permission');
      broadcast('acp:event', session.chatId, {
        type: 'permission',
        requestId: params.requestId,
        message: params.message,
        options: params.options,
        multiple: params.multiple,
        custom: params.custom,
        sessionId: session.sessionId,
      });
    } else if (method) {
      // Unknown notification
      broadcast('acp:event', session.chatId, { type: 'raw', message: msg });
    }
  } catch {
    // ignore non-JSON lines
  }
}

function flushNextQueuedPrompt(session: AcpSession): void {
  if (session.queuedPrompts.length === 0) return;
  if (!session.sessionId) return;
  if (session.status === 'running' || session.status === 'permission') return;

  const next = session.queuedPrompts.shift()!;
  const fullText = buildAcpPromptText(next.text, next.attachments);

  // Apply queued prompt's mode before sending, if different from current
  if (next.modeId && next.modeId !== session.currentModeId) {
    sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId: 'mode', value: next.modeId });
    session.currentModeId = next.modeId;
    applyModeModelBinding(session, next.modeId);
  }

  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  const msg: AcpChatMessage = { role: 'user', text: fullText, timestamp: Date.now() };
  session.messages.push(msg);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, promptText: fullText });
  broadcast('acp:event', session.chatId, { type: 'promptSent', text: fullText, queuedPrompts: session.queuedPrompts.length });
}

export function sendAcpPrompt(chatId: string, promptText: string, attachments: string[], modeId?: string): void {
  const session = sessions.get(chatId);
  if (!session) return;

  const effectiveModeId = modeId || session.currentModeId || 'build';
  const fullText = buildAcpPromptText(promptText, attachments);

  // Queue when session is not ready or a turn is active
  const shouldQueue = !session.sessionId || session.status === 'starting' || session.status === 'session_created' || session.status === 'running' || session.status === 'permission';
  if (shouldQueue) {
    session.queuedPrompts.push({ text: promptText, attachments, modeId: effectiveModeId, finalPromptText: fullText });
    broadcast('acp:event', chatId, { type: 'queued', count: session.queuedPrompts.length, queuedPrompts: session.queuedPrompts.length });
    return;
  }

  // Apply mode if explicitly provided and different
  if (modeId && modeId !== session.currentModeId) {
    sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId: 'mode', value: modeId });
    session.currentModeId = modeId;
    applyModeModelBinding(session, modeId);
  }

  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  const msg: AcpChatMessage = { role: 'user', text: fullText, timestamp: Date.now() };
  session.messages.push(msg);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, promptText: fullText });
  broadcast('acp:event', chatId, { type: 'promptSent', text: fullText, queuedPrompts: session.queuedPrompts.length });
}

export function cancelAcpPrompt(chatId: string): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  session.cancelGraceUntil = Date.now() + 2000;
  sendRpc(session, 'session/cancel', { sessionId: session.sessionId });
  session.status = 'idle';
  updateAcpStandbyStatus(session.chatId, 'idle');
  broadcast('acp:event', chatId, { type: 'cancelled', queuedPrompts: session.queuedPrompts.length });
}

export function setAcpConfigOption(chatId: string, configId: string, value: string): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId, value });

  if (configId === 'mode') {
    session.currentModeId = value;
    applyModeModelBinding(session, value);
  }
}

export function sendAcpPermissionResponse(chatId: string, requestId: string | number, answers: string[], rejected: boolean): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  sendRpc(session, 'session/permission_response', {
    sessionId: session.sessionId,
    requestId: String(requestId),
    answers,
    rejected,
  });
  session.status = 'idle';
  updateAcpStandbyStatus(session.chatId, 'idle');
  broadcast('acp:event', chatId, { type: 'permissionResponse', requestId: String(requestId), rejected });
  // Flush queued prompts after permission response
  flushNextQueuedPrompt(session);
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

// Standby pool management
export async function warmAcpStandby(projectId: number, cwd: string): Promise<void> {
  const existing = standbyPool.get(projectId);
  const now = Date.now();
  if (existing) {
    // Already warming or warm
    if (existing.retryCooldownUntil && now < existing.retryCooldownUntil) {
      return; // In cooldown
    }
    if (existing.sessionId && (existing.status === 'idle' || existing.status === 'running' || existing.status === 'permission')) {
      return; // Already warm
    }
  }

  // Clear old entry before retry
  if (existing) {
    const oldSession = sessions.get(existing.chatId);
    if (oldSession) {
      oldSession.process.kill();
      sessions.delete(existing.chatId);
    }
    standbyPool.delete(projectId);
  }

  try {
    const chatId = await spawnAcpChat({ projectId, cwd, mcpServers: [] });
    const entry: AcpStandbyEntry = {
      chatId,
      projectId,
      status: 'starting',
    };
    standbyPool.set(projectId, entry);
  } catch {
    const entry: AcpStandbyEntry = {
      chatId: '',
      projectId,
      status: 'error',
      retryCooldownUntil: now + ACP_STANDBY_RETRY_COOLDOWN,
    };
    standbyPool.set(projectId, entry);
  }
}

export function getAcpStandby(projectId: number): AcpStandbyEntry | undefined {
  return standbyPool.get(projectId);
}

export function clearAcpStandby(projectId: number): void {
  const entry = standbyPool.get(projectId);
  if (entry) {
    if (entry.chatId) {
      const session = sessions.get(entry.chatId);
      if (session) {
        session.process.kill();
        sessions.delete(entry.chatId);
      }
    }
    standbyPool.delete(projectId);
  }
}

export function promoteAcpStandby(projectId: number, visibleChatId: string): AcpStandbyEntry | undefined {
  const entry = standbyPool.get(projectId);
  if (!entry) return undefined;
  if (entry.chatId === visibleChatId) return undefined; // Already visible
  if (!entry.sessionId) return undefined;
  if (entry.status !== 'idle' && entry.status !== 'running' && entry.status !== 'permission') return undefined;
  // Remove from standby pool since it's now promoted to visible
  standbyPool.delete(projectId);
  return entry;
}

export function updateAcpStandbyStatus(chatId: string, status: AcpChatSession['status'], sessionId?: string): void {
  for (const entry of standbyPool.values()) {
    if (entry.chatId === chatId) {
      entry.status = status;
      if (sessionId) entry.sessionId = sessionId;
      break;
    }
  }
}

export function clearAllAcpStandby(): void {
  for (const [projectId] of standbyPool) {
    clearAcpStandby(projectId);
  }
}
