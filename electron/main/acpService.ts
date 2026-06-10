import { spawn, type ChildProcess } from 'child_process';
import path from 'path';
import fs from 'fs';
import { BrowserWindow } from 'electron';
import type { AcpChatSession, AcpConfigOption, AcpAvailableCommand, AcpChatMessage, OpenCodeModelConfig } from '../shared/types';
import { activeBuildModel, effectivePlanModel, effectivePlanEffort, mergeAcpKnownModels } from '../shared/types';
import { buildAcpPermissionResponse, firstAutoApproveOptionId, permissionRequestIdFromRpc, startupModeToModeId } from '../shared/acpProtocol';
import { loadConfig, saveConfig } from './config';
import { getOpencodeBinPath } from './opencode';

function buildAcpPromptText(text: string, attachments: string[]): string {
  if (attachments.length === 0) return text;
  const lines = attachments.map((a) => `- ${a}`);
  const attachmentBlock = `Attached file paths:\n${lines.join('\n')}`;
  if (!text.trim()) return attachmentBlock;
  return `${text}\n\n${attachmentBlock}`;
}

interface AcpSession {
  process: ChildProcess | null;
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
  sendRawRpc(session, req);
}

function sendRawRpc(session: AcpSession, req: unknown): void {
  if (!session.process) return;
  session.process.stdin?.write(JSON.stringify(req) + '\n');
}

function setSessionMode(session: AcpSession, modeId: string, forceRpc = false): void {
  if (!modeId) return;
  const changed = session.currentModeId !== modeId;
  session.currentModeId = modeId;
  if (session.sessionId && (forceRpc || changed)) {
    sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId: 'mode', value: modeId });
  }
  applyModeModelBinding(session, modeId);
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
  const initialModeId = startupModeToModeId(loadConfig().acpStartupMode);

  const session: AcpSession = {
    process: null as unknown as ChildProcess,
    chatId,
    projectId: opts.projectId,
    cwd: opts.cwd,
    mcpServers: opts.mcpServers,
    status: 'starting',
    messages: [],
    promptInput: '',
    attachments: [],
    configOptions: [],
    currentModeId: initialModeId,
    queuedPrompts: [],
  };

  sessions.set(chatId, session);

  try {
    // Ensure cwd is valid and absolute on Windows
    let cwd = opts.cwd;
    if (process.platform === 'win32' && cwd) {
      cwd = path.resolve(cwd);
      if (!fs.existsSync(cwd)) {
        throw new Error(`ACP spawn cwd does not exist: ${cwd}`);
      }
    }
    const proc = spawn(opencodeBin, ['acp'], {
      cwd: cwd || opts.cwd,
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });

    session.process = proc;

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

    proc.on('error', (err) => {
      session.status = 'error';
      const text = err.message;
      const msg: AcpChatMessage = { role: 'system', text: `ACP spawn error: ${text}`, timestamp: Date.now() };
      session.messages.push(msg);
      broadcast('acp:event', chatId, { type: 'error', text: `ACP spawn error: ${text}` });
    });

    // Send initialize
    sendRpc(session, 'initialize', {
      protocolVersion: 1,
      clientCapabilities: {
        fs: { readTextFile: false, writeTextFile: false },
        terminal: false,
      },
      clientInfo: { name: 'opencode-local-acp', title: 'OpenCode', version: '1.0.0' },
    });
  } catch (err) {
    session.status = 'error';
    const text = err instanceof Error ? err.message : String(err);
    const msg: AcpChatMessage = { role: 'system', text: `ACP spawn error: ${text}`, timestamp: Date.now() };
    session.messages.push(msg);
    broadcast('acp:event', chatId, { type: 'error', text: `ACP spawn error: ${text}` });
  }

  return chatId;
}

function handleAcpLine(session: AcpSession, line: string): void {
  try {
    const msg = JSON.parse(line) as Record<string, unknown>;
    const method = msg.method as string | undefined;
    const result = msg.result as Record<string, unknown> | undefined;
    const id = msg.id as number | string | undefined;
    const error = msg.error as Record<string, unknown> | undefined;

    // Handle JSON-RPC error responses first
    if (error) {
      session.status = 'error';
      const text = (error.message as string) || JSON.stringify(error);
      const errMsg: AcpChatMessage = { role: 'system', text: `ACP error: ${text}`, timestamp: Date.now() };
      session.messages.push(errMsg);
      broadcast('acp:event', session.chatId, { type: 'error', text: `ACP error: ${text}` });
      return;
    }

    // Responses (have result + id)
    if (id !== undefined && result) {
      // Response to initialize -> send session/new
      if (result.protocolVersion !== undefined) {
        sendRpc(session, 'session/new', {
          cwd: session.cwd,
          mcpServers: session.mcpServers,
        });
        return;
      }

      // session/new response
      if (result.sessionId !== undefined) {
        session.sessionId = (result.sessionId as string) || undefined;
        session.status = 'idle';
        if (result.configOptions) {
          session.configOptions = result.configOptions as AcpConfigOption[];
          const modelOpt = session.configOptions.find((o) => o.id === 'model');
          if (modelOpt) session.currentModel = modelOpt.currentValue;
          const effortOpt = session.configOptions.find((o) => o.id === 'effort');
          if (effortOpt) session.currentEffort = effortOpt.currentValue;
          const modeOpt = session.configOptions.find((o) => o.id === 'mode');
          if (!session.currentModeId && modeOpt) session.currentModeId = modeOpt.currentValue;
          // Update known models from server options
          const modelOptions = session.configOptions.find((o) => o.id === 'model');
          if (modelOptions) {
            const config = loadConfig();
            const entries = modelOptions.options.map((o) => [o.value, o.label || o.value] as [string, string]);
            const changed = mergeAcpKnownModels(config.opencode, entries);
            if (changed) saveConfig(config);
          }
          broadcast('acp:event', session.chatId, { type: 'configOptions', options: session.configOptions });
        }
        // Apply startup/pending mode and model binding from Mergen config on session start.
        setSessionMode(session, session.currentModeId || startupModeToModeId(loadConfig().acpStartupMode), true);
        updateAcpStandbyStatus(session.chatId, 'idle', session.sessionId);
        broadcast('acp:event', session.chatId, { type: 'sessionCreated', sessionId: session.sessionId });
        // Flush any prompts queued while starting
        flushNextQueuedPrompt(session);
        return;
      }

      // session/prompt response (turn complete)
      if (result.stopReason !== undefined || result.text !== undefined) {
        session.status = 'idle';
        updateAcpStandbyStatus(session.chatId, 'idle');
        broadcast('acp:event', session.chatId, { type: 'promptResponse', stopReason: result.stopReason, text: result.text, queuedPrompts: session.queuedPrompts.length });
        // Flush queued prompts one at a time
        flushNextQueuedPrompt(session);
        return;
      }

      return;
    }

    // Requests from agent (have id and method, no result)
    if (id !== undefined && method) {
      if (method === 'session/request_permission') {
        const params = msg.params as Record<string, unknown>;
        const optionsRaw = (params.options as Array<Record<string, unknown>>) || [];
        const options = optionsRaw.map((o) => ({
          id: (o.optionId as string) || '',
          label: (o.name as string) || '',
        }));
        const requestId = permissionRequestIdFromRpc(id, params);
        const autoOptionId = firstAutoApproveOptionId(options, loadConfig().opencode.acpAutoApprovePermissions);
        if (requestId && autoOptionId) {
          sendRawRpc(session, buildAcpPermissionResponse(requestId, autoOptionId));
          session.status = 'running';
          updateAcpStandbyStatus(session.chatId, 'running');
          broadcast('acp:event', session.chatId, { type: 'permissionResponse', requestId, rejected: false, autoApproved: true, status: 'running', queuedPrompts: session.queuedPrompts.length });
        } else {
          session.status = 'permission';
          updateAcpStandbyStatus(session.chatId, 'permission');
          broadcast('acp:event', session.chatId, {
            type: 'permission',
            requestId,
            message: params.message,
            options,
            multiple: params.multiple,
            custom: params.custom,
            sessionId: session.sessionId,
            status: 'permission',
          });
        }
        return;
      }
      return;
    }

    // Notifications (no id, have method)
    if (method === 'session/update') {
      const params = msg.params as Record<string, unknown>;
      const update = (params.update as Record<string, unknown>) || {};
      const sessionUpdate = (update.sessionUpdate as string) || '';

      switch (sessionUpdate) {
        case 'agent_message_chunk': {
          const content = (update.content as Record<string, unknown>) || {};
          const text = (content.text as string) || '';
          const lastMsg = session.messages[session.messages.length - 1];
          if (lastMsg && lastMsg.role === 'assistant') {
            lastMsg.text += text;
          } else {
            session.messages.push({ role: 'assistant', text, timestamp: Date.now() });
          }
          broadcast('acp:event', session.chatId, { type: 'messageChunk', text, role: 'assistant' });
          return;
        }
        case 'user_message_chunk': {
          const content = (update.content as Record<string, unknown>) || {};
          const text = (content.text as string) || '';
          const lastMsg = session.messages[session.messages.length - 1];
          if (lastMsg && lastMsg.role === 'user') {
            lastMsg.text += text;
          } else {
            session.messages.push({ role: 'user', text, timestamp: Date.now() });
          }
          broadcast('acp:event', session.chatId, { type: 'messageChunk', text, role: 'user' });
          return;
        }
        case 'tool_call': {
          const toolCallId = (update.toolCallId as string) || '';
          const title = (update.title as string) || '';
          const kind = (update.kind as string) || '';
          const status = (update.status as string) || 'pending';
          session.messages.push({ role: 'system', text: `${title} (${kind})`, timestamp: Date.now() });
          broadcast('acp:event', session.chatId, { type: 'toolCall', toolCallId, title, kind, status });
          return;
        }
        case 'tool_call_update': {
          const toolCallId = (update.toolCallId as string) || '';
          const status = (update.status as string) || '';
          broadcast('acp:event', session.chatId, { type: 'toolCallUpdate', toolCallId, status });
          return;
        }
        case 'current_mode_update': {
          const modeId = (update.currentModeId as string) || (update.modeId as string);
          if (modeId) {
            session.currentModeId = modeId;
            applyModeModelBinding(session, modeId);
          }
          broadcast('acp:event', session.chatId, { type: 'modeUpdate', modeId });
          return;
        }
        case 'config_option_update': {
          const optionsRaw = update.configOptions as AcpConfigOption[] | undefined;
          if (optionsRaw) {
            session.configOptions = optionsRaw;
            const modelOpt = optionsRaw.find((o) => o.id === 'model');
            if (modelOpt) session.currentModel = modelOpt.currentValue;
            const effortOpt = optionsRaw.find((o) => o.id === 'effort');
            if (effortOpt) session.currentEffort = effortOpt.currentValue;
            const modeOpt = optionsRaw.find((o) => o.id === 'mode');
            if (modeOpt) session.currentModeId = modeOpt.currentValue;
            // Update known models from server options
            const modelOptions = optionsRaw.find((o) => o.id === 'model');
            if (modelOptions) {
              const config = loadConfig();
              const entries = modelOptions.options.map((o) => [o.value, o.label || o.value] as [string, string]);
              const changed = mergeAcpKnownModels(config.opencode, entries);
              if (changed) saveConfig(config);
            }
          } else {
            // Legacy fallback: single field format
            const category = (update.category as string) || '';
            const value = (update.value as string) || '';
            if (category) {
              const existing = session.configOptions.find((o) => o.id === category);
              if (existing) {
                existing.currentValue = value;
              } else {
                session.configOptions.push({ id: category, name: category, category, currentValue: value, options: [] });
              }
            }
          }
          broadcast('acp:event', session.chatId, { type: 'configOptions', options: session.configOptions });
          return;
        }
        case 'available_commands_update': {
          const commands = (update.availableCommands as AcpAvailableCommand[]) || (update.commands as AcpAvailableCommand[]) || [];
          session.availableCommands = commands;
          broadcast('acp:event', session.chatId, { type: 'commands', commands });
          return;
        }
        default:
          return;
      }
    }

    if (method) {
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
    setSessionMode(session, next.modeId);
  }

  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  const msg: AcpChatMessage = { role: 'user', text: fullText, timestamp: Date.now() };
  session.messages.push(msg);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, prompt: [{ type: 'text', text: fullText }] });
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
    setSessionMode(session, modeId);
  }

  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  const msg: AcpChatMessage = { role: 'user', text: fullText, timestamp: Date.now() };
  session.messages.push(msg);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, prompt: [{ type: 'text', text: fullText }] });
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
  if (!session) return;

  if (configId === 'mode') {
    setSessionMode(session, value, Boolean(session.sessionId));
    broadcast('acp:event', session.chatId, { type: 'modeUpdate', modeId: value });
    return;
  }

  if (!session.sessionId) return;

  sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId, value });

  if (configId === 'model') {
    session.currentModel = value;
  } else if (configId === 'effort') {
    session.currentEffort = value;
  }
}

export function sendAcpPermissionResponse(chatId: string, requestId: string | number, answers: string[], rejected: boolean): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  sendRawRpc(session, buildAcpPermissionResponse(requestId, answers[0] || '', rejected));
  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  broadcast('acp:event', chatId, { type: 'permissionResponse', requestId: String(requestId), rejected });
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
  if (session.process) {
    try {
      session.process.kill();
    } catch {
      // ignore
    }
  }
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
      if (oldSession.process) {
        try {
          oldSession.process.kill();
        } catch {
          // ignore
        }
      }
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
        if (session.process) {
          try {
            session.process.kill();
          } catch {
            // ignore
          }
        }
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
