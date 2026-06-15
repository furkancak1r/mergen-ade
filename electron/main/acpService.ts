import { spawn, type ChildProcess } from 'child_process';
import path from 'path';
import fs from 'fs';
import os from 'os';
import { BrowserWindow } from 'electron';
import type {
  AcpChatSession,
  AcpConfigOption,
  AcpAvailableCommand,
  AcpChatMessage,
  AcpTimelineChangeSummaryFile,
  AcpTimelineItem,
  AcpTimelineNoticeKind,
  AcpTimelineStatusKind,
  GitFileDiff,
  OpenCodeModelConfig,
  OpenCodeQuestion,
  OpenCodeQuestionOption,
  QueuedAcpPrompt,
  SourceControlFile,
} from '../shared/types';
import { activeBuildModel, effectivePlanModel, effectivePlanEffort, mergeAcpKnownModels } from '../shared/types';
import { normalizeAcpTimelineToolStatus } from '../shared/acpTimeline';
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
  isAcpUnknownResponseWarning,
  isJsonRpcId,
  permissionRequestIdFromRpc,
  stripAnsi,
  startupModeToModeId,
  type JsonRpcId,
} from '../shared/acpProtocol';
import { loadConfig, saveConfig } from './config';
import { getOpencodeBinPath } from './opencode';
import { codexExecJsonArgs, getCodexBinPath, parseCodexExecJsonLine } from './codex';
import { getGitFileDiff, getGitStatus } from './worktree';

/**
 * Load slash commands from SKILL.md files in .zed/skills/
 */
function loadSlashCommandsFromSkills(projectPath: string): AcpAvailableCommand[] {
  try {
    const skillsBaseDir = path.join(projectPath, '.zed', 'skills');
    if (!fs.existsSync(skillsBaseDir)) return [];

    const entries = fs.readdirSync(skillsBaseDir, { withFileTypes: true });
    const commands: AcpAvailableCommand[] = [];

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      const skillFile = path.join(skillsBaseDir, entry.name, 'SKILL.md');
      try {
        const content = fs.readFileSync(skillFile, 'utf-8');
        // Parse frontmatter
        const frontmatterMatch = content.match(/^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/);
        if (!frontmatterMatch) continue;

        const frontmatter = frontmatterMatch[1];
        const nameMatch = frontmatter.match(/^name:\s*(.+)$/m);
        const descMatch = frontmatter.match(/^description:\s*(.+)$/m);

        if (nameMatch) {
          const name = nameMatch[1].trim();
          commands.push({
            id: `/${name}`,
            name: `/${name}`,
            description: descMatch ? descMatch[1].trim() : '',
          });
        }
      } catch {
        // Skip directories without SKILL.md or parse errors
      }
    }

    return commands;
  } catch {
    return [];
  }
}

function defaultAcpSlashCommands(): AcpAvailableCommand[] {
  return [
    { id: '/help', name: '/help', description: 'Show help and available commands' },
    { id: '/clear', name: '/clear', description: 'Clear conversation history' },
    { id: '/compact', name: '/compact', description: 'Compact conversation to save context' },
    { id: '/config', name: '/config', description: 'View/change configuration' },
    { id: '/cost', name: '/cost', description: 'Show token usage and cost' },
    { id: '/doctor', name: '/doctor', description: 'Check ACP health' },
    { id: '/init', name: '/init', description: 'Initialize project memory' },
    { id: '/memory', name: '/memory', description: 'Edit project memory' },
    { id: '/model', name: '/model', description: 'Switch AI model' },
    { id: '/permissions', name: '/permissions', description: 'Manage tool permissions' },
    { id: '/review', name: '/review', description: 'Request a code review' },
    { id: '/status', name: '/status', description: 'Show current status' },
    { id: '/terminal-setup', name: '/terminal-setup', description: 'Configure terminal integration' },
    { id: '/mcp', name: '/mcp', description: 'Manage MCP servers' },
  ];
}

function mergeAcpSlashCommands(...groups: AcpAvailableCommand[][]): AcpAvailableCommand[] {
  const merged: AcpAvailableCommand[] = [];
  const seen = new Set<string>();
  for (const group of groups) {
    for (const command of group) {
      const id = slashCommandKey(command.id || command.name);
      if (!id || seen.has(id)) continue;
      seen.add(id);
      merged.push(command);
    }
  }
  return merged;
}

function slashCommandKey(value: string | undefined): string {
  const token = (value || '').trim().replace(/^\/+/, '').toLowerCase();
  return token && !/\s/.test(token) ? `/${token}` : '';
}

function buildAcpPromptText(text: string, attachments: string[]): string {
  if (attachments.length === 0) return text;
  const attachmentBlock = `Attached file paths:\n${attachments.join('\n')}`;
  if (text.length === 0) return attachmentBlock;
  return `${text}\n\n${attachmentBlock}`;
}

function acpLabelForTool(tool?: AcpSession['tool']): string {
  if (tool === 'claude_acp') return 'Claude Code ACP';
  if (tool === 'codex_acp') return 'Codex ACP';
  return 'OpenCode ACP';
}

function acpChatTitleFromPrompt(promptText: string, tool?: AcpSession['tool']): string {
  const collapsed = promptText.split(/\s+/).filter(Boolean).join(' ').trim();
  if (!collapsed) return acpLabelForTool(tool);
  const chars = Array.from(collapsed);
  if (chars.length <= 72) return collapsed;
  return `${chars.slice(0, 72).join('')}...`;
}

function updateAcpChatTitleFromPrompt(session: AcpSession, promptText: string): void {
  if (!session.title || session.title === 'OpenCode ACP' || session.title === 'Claude Code ACP' || session.title === 'Codex ACP') {
    session.title = acpChatTitleFromPrompt(promptText, session.tool);
  }
}

type AcpPendingInteraction =
  | { kind: 'permission'; rpcId: JsonRpcId; optionIds: Set<string> }
  | { kind: 'question'; rpcId: JsonRpcId; questionCount: number };

interface AcpSession {
  process: ChildProcess | null;
  sessionId?: string;
  chatId: string;
  projectId: number;
  cwd: string;
  mcpServers: string[];
  status: AcpChatSession['status'];
  title: string;
  timeline: AcpTimelineItem[];
  messages: AcpChatMessage[];
  promptInput: string;
  attachments: string[];
  configOptions: AcpConfigOption[];
  currentModeId?: string;
  currentModel?: string;
  currentEffort?: string;
  availableCommands?: AcpAvailableCommand[];
  queuedPrompts: QueuedAcpPrompt[];
  partialStderr?: string;
  cancelGraceUntil?: number;
  cancelUnsupported?: boolean;
  pendingInteractions: Map<string, AcpPendingInteraction>;
  nextInteractionId: number;
  tool?: 'opencode' | 'claude_acp' | 'codex_acp';
  nextTimelineId: number;
  lastChangeSummarySignature?: string;
  changeSummaryInFlight?: boolean;
}

const sessions = new Map<string, AcpSession>();
const nextAcpRequestId = createAcpRequestIdGenerator();

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
  const req = { jsonrpc: '2.0', id: nextAcpRequestId(), method, params };
  sendRawRpc(session, req);
}

function sendRawRpc(session: AcpSession, req: unknown): void {
  if (!session.process) return;
  session.process.stdin?.write(JSON.stringify(req) + '\n');
}

function appendAcpSystemMessage(session: AcpSession, text: string): void {
  session.messages.push({ role: 'system', text, timestamp: Date.now() });
}

function createTimelineId(session: AcpSession, prefix: string): string {
  return `${prefix}-${Date.now()}-${session.nextTimelineId++}`;
}

function appendAcpTimelineMessage(
  session: AcpSession,
  role: AcpChatMessage['role'],
  text: string,
  timestamp = Date.now(),
): void {
  const msg: AcpChatMessage = { role, text, timestamp };
  session.messages.push(msg);
  session.timeline.push({
    id: createTimelineId(session, `${role}-message`),
    type: 'message',
    role,
    text,
    timestamp,
  });
}

function appendAcpTimelineMessageChunk(
  session: AcpSession,
  role: AcpChatMessage['role'],
  text: string,
  timestamp = Date.now(),
): void {
  const lastMsg = session.messages[session.messages.length - 1];
  if (lastMsg && lastMsg.role === role) {
    lastMsg.text += text;
  } else {
    session.messages.push({ role, text, timestamp });
  }

  const lastTimeline = session.timeline[session.timeline.length - 1];
  if (lastTimeline && lastTimeline.type === 'message' && lastTimeline.role === role) {
    lastTimeline.text += text;
  } else {
    session.timeline.push({
      id: createTimelineId(session, `${role}-message`),
      type: 'message',
      role,
      text,
      timestamp,
    });
  }
}

function appendAcpTimelineThinkingChunk(
  session: AcpSession,
  text: string,
  timestamp = Date.now(),
): void {
  const lastTimeline = session.timeline[session.timeline.length - 1];
  if (lastTimeline && lastTimeline.type === 'thinking') {
    lastTimeline.text += text;
  } else {
    session.timeline.push({
      id: createTimelineId(session, 'thinking'),
      type: 'thinking',
      text,
      timestamp,
    });
  }
}

function appendAcpTimelineNotice(
  session: AcpSession,
  kind: AcpTimelineNoticeKind,
  text: string,
  timestamp = Date.now(),
): void {
  session.timeline.push({
    id: createTimelineId(session, kind),
    type: 'notice',
    kind,
    text,
    timestamp,
  });
}

function appendAcpTimelineStatus(
  session: AcpSession,
  kind: AcpTimelineStatusKind,
  title: string,
  text: string,
  timestamp = Date.now(),
): void {
  const trimmed = text.trim();
  if (!trimmed) return;
  const item: AcpTimelineItem = {
    id: createTimelineId(session, `${kind}-status`),
    type: 'status',
    kind,
    title,
    text: trimmed,
    timestamp,
  };
  session.timeline.push(item);
  broadcast('acp:event', session.chatId, { type: 'timelineItem', item });
}

function scheduleAcpChangeSummary(session: AcpSession): void {
  if (session.changeSummaryInFlight) return;
  session.changeSummaryInFlight = true;
  void appendAcpChangeSummary(session)
    .finally(() => {
      session.changeSummaryInFlight = false;
    });
}

async function appendAcpChangeSummary(session: AcpSession): Promise<void> {
  const status = await getGitStatus(session.cwd, false);
  if (status.error || status.files.length === 0) return;

  const visibleFiles = status.files.slice(0, 12);
  const diffs = await Promise.all(visibleFiles.map(async (file) => {
    try {
      return await getGitFileDiff(session.cwd, file.path);
    } catch (error) {
      return {
        status: 'error',
        filePath: file.path,
        patch: '',
        addedLines: 0,
        removedLines: 0,
        binary: false,
        error: error instanceof Error ? error.message : String(error),
      } satisfies GitFileDiff;
    }
  }));

  const files = visibleFiles.map((file, index) => acpChangeSummaryFile(file, diffs[index]));
  const signature = acpChangeSummarySignature(status.files, files);
  if (!signature || signature === session.lastChangeSummarySignature) return;

  session.lastChangeSummarySignature = signature;
  const totals = files.reduce((sum, file) => ({
    addedLines: sum.addedLines + file.addedLines,
    removedLines: sum.removedLines + file.removedLines,
  }), { addedLines: 0, removedLines: 0 });
  const item: AcpTimelineItem = {
    id: createTimelineId(session, 'change-summary'),
    type: 'change_summary',
    files,
    totalFiles: status.files.length,
    addedLines: totals.addedLines,
    removedLines: totals.removedLines,
    signature,
    timestamp: Date.now(),
  };
  session.timeline.push(item);
  broadcast('acp:event', session.chatId, { type: 'timelineItem', item });
}

function acpChangeSummaryFile(file: SourceControlFile, diff: GitFileDiff | undefined): AcpTimelineChangeSummaryFile {
  return {
    path: file.path,
    status: file.status,
    staged: file.staged,
    addedLines: diff?.status === 'ready' ? diff.addedLines : 0,
    removedLines: diff?.status === 'ready' ? diff.removedLines : 0,
    binary: diff?.status === 'ready' ? diff.binary : false,
    error: diff?.status === 'error' ? diff.error || 'Diff unavailable' : undefined,
  };
}

function acpChangeSummarySignature(
  statusFiles: readonly SourceControlFile[],
  visibleFiles: readonly AcpTimelineChangeSummaryFile[],
): string {
  const statusPart = statusFiles
    .map((file) => `${file.path}:${file.status}:${file.staged ? 'staged' : 'unstaged'}`)
    .join('|');
  const diffPart = visibleFiles
    .map((file) => `${file.path}:${file.addedLines}:${file.removedLines}:${file.binary ? 'binary' : 'text'}:${file.error || ''}`)
    .join('|');
  return `${statusFiles.length}::${statusPart}::${diffPart}`;
}

function isAcpFileModifyingToolKind(kind: string | undefined): boolean {
  const normalized = (kind || '').trim().toLowerCase();
  if (!normalized) return false;
  return normalized.includes('edit') || normalized.includes('patch') || normalized.includes('write')
    || normalized.includes('bash') || normalized.includes('shell') || normalized.includes('terminal');
}

function isTerminalToolStatus(status: unknown): boolean {
  const normalized = normalizeAcpTimelineToolStatus(status);
  return normalized === 'completed' || normalized === 'failed';
}

function appendAcpStatusFromProtocolUpdate(session: AcpSession, value: unknown): void {
  const text = acpProtocolStatusText(value);
  if (!text) return;
  const lower = text.toLowerCase();
  const kind: AcpTimelineStatusKind = lower.includes('compact')
    ? 'compact'
    : lower.includes('context')
      ? 'context'
      : lower.includes('cost') || lower.includes('token')
        ? 'cost'
        : lower.includes('terminal')
          ? 'terminal'
          : lower.includes('status')
            ? 'status'
            : 'info';
  if (kind === 'info') return;
  appendAcpTimelineStatus(session, kind, acpProtocolStatusTitle(kind), text);
}

function acpProtocolStatusTitle(kind: AcpTimelineStatusKind): string {
  switch (kind) {
    case 'compact':
      return 'Context Compacting';
    case 'context':
      return 'Context';
    case 'status':
      return 'Status';
    case 'cost':
      return 'Cost';
    case 'terminal':
      return 'Terminal';
    case 'info':
      return 'Info';
  }
}

function acpProtocolStatusText(value: unknown): string {
  if (!value) return '';
  if (typeof value === 'string') return value.trim();
  if (typeof value !== 'object') return '';

  const record = value as Record<string, unknown>;
  const direct = firstStringValue(record, ['message', 'text', 'summary', 'status', 'description']);
  if (direct) return direct;

  const json = JSON.stringify(value);
  return /compact|context|cost|token|terminal|status/i.test(json) ? json : '';
}

function firstStringValue(record: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return '';
}

function upsertAcpTimelineTool(
  session: AcpSession,
  options: { toolCallId: string; title: string; kind: string; status: unknown; raw?: unknown },
): void {
  const now = Date.now();
  const status = normalizeAcpTimelineToolStatus(options.status);
  const existing = session.timeline.find((item) => (
    item.type === 'tool'
    && item.toolCallId.length > 0
    && item.toolCallId === options.toolCallId
  ));
  if (existing?.type === 'tool') {
    existing.title = options.title || existing.title;
    existing.kind = options.kind || existing.kind;
    existing.status = status;
    existing.updatedAt = now;
    existing.raw = options.raw ?? existing.raw;
    return;
  }

  session.timeline.push({
    id: createTimelineId(session, 'tool'),
    type: 'tool',
    toolCallId: options.toolCallId,
    title: options.title,
    kind: options.kind,
    status,
    startedAt: now,
    updatedAt: now,
    raw: options.raw,
  });
}

function updateAcpTimelineToolStatus(session: AcpSession, toolCallId: string, status: unknown): AcpTimelineItem | undefined {
  const now = Date.now();
  const normalized = normalizeAcpTimelineToolStatus(status);
  const existing = session.timeline.find((item) => item.type === 'tool' && item.toolCallId === toolCallId);
  if (existing?.type === 'tool') {
    existing.status = normalized;
    existing.updatedAt = now;
    return existing;
  }
  return undefined;
}

function appendAcpTimelinePermission(
  session: AcpSession,
  options: {
    interactionKind: 'permission' | 'question';
    requestId: string;
    header: string;
    question: string;
    options: OpenCodeQuestionOption[];
  },
): void {
  session.timeline.push({
    id: createTimelineId(session, options.interactionKind),
    type: 'permission',
    interactionKind: options.interactionKind,
    requestId: options.requestId,
    header: options.header,
    question: options.question,
    options: options.options,
    status: 'pending',
    timestamp: Date.now(),
  });
}

function resolveAcpTimelinePermission(session: AcpSession, requestId: string, rejected: boolean): void {
  const item = session.timeline.find((candidate) => (
    candidate.type === 'permission'
    && candidate.requestId === requestId
    && candidate.status === 'pending'
  ));
  if (item?.type === 'permission') {
    item.status = rejected ? 'rejected' : 'answered';
  }
}

function broadcastAcpWarning(session: AcpSession, text: string): void {
  const clean = stripAnsi(text).trim();
  if (!clean) return;
  appendAcpSystemMessage(session, `ACP warning: ${clean}`);
  appendAcpTimelineNotice(session, 'warning', clean);
  broadcast('acp:event', session.chatId, { type: 'warning', text: clean });
}

function createPendingInteraction(session: AcpSession, interaction: AcpPendingInteraction): string {
  const token = `${interaction.kind}-${Date.now()}-${session.nextInteractionId++}`;
  session.pendingInteractions.set(token, interaction);
  return token;
}

function clearPendingInteractions(session: AcpSession): void {
  session.pendingInteractions.clear();
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

export async function spawnAcpChat(opts: { projectId: number; cwd: string; mcpServers: string[]; tool?: string }): Promise<string> {
  const chatId = `acp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const isClaudeCode = opts.tool === 'claude_acp';
  const isCodex = opts.tool === 'codex_acp';
  const tool: AcpSession['tool'] = isClaudeCode ? 'claude_acp' : isCodex ? 'codex_acp' : 'opencode';

  const session: AcpSession = {
    process: null as unknown as ChildProcess,
    chatId,
    projectId: opts.projectId,
    cwd: opts.cwd,
    mcpServers: opts.mcpServers,
    status: 'starting',
    title: acpLabelForTool(tool),
    timeline: [],
    messages: [],
    promptInput: '',
    attachments: [],
    configOptions: [],
    currentModeId: startupModeToModeId(loadConfig().acpStartupMode),
    queuedPrompts: [],
    pendingInteractions: new Map(),
    nextInteractionId: 1,
    tool,
    nextTimelineId: 1,
  };

  sessions.set(chatId, session);

  if (isCodex) {
    session.sessionId = `codex-${Date.now()}`;
    session.status = 'idle';
    session.currentModel = 'Codex default';
    session.configOptions = [
      {
        id: 'model',
        name: 'Model',
        category: 'model',
        currentValue: session.currentModel,
        options: [],
      },
    ];
    session.availableCommands = mergeAcpSlashCommands(
      loadSlashCommandsFromSkills(opts.cwd || process.cwd()),
      defaultAcpSlashCommands(),
    );

    broadcast('acp:event', chatId, { type: 'sessionCreated', sessionId: session.sessionId });
    broadcast('acp:event', chatId, { type: 'configOptions', options: session.configOptions });
    broadcast('acp:event', chatId, { type: 'commands', commands: session.availableCommands });
    return chatId;
  }

  // Claude Code: fake handshake, session ready immediately
  if (isClaudeCode) {
    session.sessionId = `claude-${Date.now()}`;
    session.status = 'idle';

    // Populate configOptions with the current model so the UI selector is not empty
    const currentModel = process.env.ANTHROPIC_MODEL || 'mimo-v2.5-pro';
    session.currentModel = currentModel;
    session.configOptions = [
      {
        id: 'model',
        name: 'Model',
        category: 'model',
        currentValue: currentModel,
        options: [
          { value: 'mimo-v2.5-pro', label: 'mimo-v2.5-pro' },
          { value: 'claude-sonnet-4-6', label: 'claude-sonnet-4-6' },
          { value: 'claude-opus-4-7', label: 'claude-opus-4-7' },
          { value: 'claude-haiku-4-5-20251001', label: 'claude-haiku-4-5' },
        ],
      },
    ];

    // Load custom commands from .zed/skills/
    const customCommands = loadSlashCommandsFromSkills(opts.cwd || process.cwd());

    // Merge: custom commands first, then built-in commands
    session.availableCommands = mergeAcpSlashCommands(customCommands, defaultAcpSlashCommands());

    broadcast('acp:event', chatId, { type: 'sessionCreated', sessionId: session.sessionId });
    broadcast('acp:event', chatId, { type: 'configOptions', options: session.configOptions });
    broadcast('acp:event', chatId, { type: 'commands', commands: session.availableCommands });
    return chatId;
  }

  // OpenCode: spawn acp process
  const opencodeBin = getOpencodeBinPath();

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
      env: {
        ...process.env,
        OPENCODE_ENABLE_QUESTION_TOOL: '1',
      },
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
      const rawText = data.toString();
      const text = stripAnsi(rawText).trim();
      const now = Date.now();
      if (session.cancelGraceUntil && now < session.cancelGraceUntil) {
        // Suppress cancel-specific stderr noise
        if (isAcpCancelNoise(rawText)) {
          if (isAcpCancelUnsupported(rawText)) {
            session.cancelUnsupported = true;
          }
          return;
        }
      }
      if (!text) return;
      session.partialStderr = (session.partialStderr || '') + text + '\n';
      const staleResponseWarning = isAcpUnknownResponseWarning(text);
      const prefix = staleResponseWarning ? 'ACP warning' : 'ACP stderr';
      const message = staleResponseWarning ? acpUnknownResponseWarningText(text) : text;
      appendAcpSystemMessage(session, `${prefix}: ${message}`);
      appendAcpTimelineNotice(session, staleResponseWarning ? 'warning' : 'stderr', message);
      broadcast('acp:event', chatId, { type: staleResponseWarning ? 'warning' : 'stderr', text: message });
    });

    proc.on('exit', (code) => {
      session.status = 'error';
      clearPendingInteractions(session);
      appendAcpTimelineNotice(session, 'error', `ACP process exited with code ${code ?? 'unknown'}`);
      broadcast('acp:event', chatId, { type: 'exit', code });
    });

    proc.on('error', (err) => {
      session.status = 'error';
      clearPendingInteractions(session);
      const text = err.message;
      appendAcpSystemMessage(session, `ACP spawn error: ${text}`);
      appendAcpTimelineNotice(session, 'error', `ACP spawn error: ${text}`);
      broadcast('acp:event', chatId, { type: 'error', text: `ACP spawn error: ${text}` });
    });

    // Send initialize
    sendRpc(session, 'initialize', {
      protocolVersion: 1,
      clientCapabilities: {
        fs: { readTextFile: false, writeTextFile: false },
        terminal: false,
        _meta: {
          'opencode/question': { version: 1 },
        },
      },
      clientInfo: { name: 'opencode-local-acp', title: 'OpenCode', version: '1.0.0' },
    });
  } catch (err) {
    session.status = 'error';
    clearPendingInteractions(session);
    const text = err instanceof Error ? err.message : String(err);
    appendAcpSystemMessage(session, `ACP spawn error: ${text}`);
    appendAcpTimelineNotice(session, 'error', `ACP spawn error: ${text}`);
    broadcast('acp:event', chatId, { type: 'error', text: `ACP spawn error: ${text}` });
  }

  return chatId;
}

function handleAcpLine(session: AcpSession, line: string): void {
  try {
    const msg = JSON.parse(line) as Record<string, unknown>;
    const method = msg.method as string | undefined;
    const result = msg.result as Record<string, unknown> | undefined;
    const id = msg.id;
    const error = msg.error as Record<string, unknown> | undefined;

    // Handle JSON-RPC error responses first
    if (error) {
      const text = (error.message as string) || JSON.stringify(error);
      const now = Date.now();
      if (session.cancelGraceUntil && now < session.cancelGraceUntil && isAcpCancelNoise(text)) {
        if (isAcpCancelUnsupported(text)) {
          session.cancelUnsupported = true;
        }
        return;
      }
      if (isAcpUnknownResponseWarning(text)) {
        const message = acpUnknownResponseWarningText(text);
        appendAcpSystemMessage(session, `ACP warning: ${message}`);
        appendAcpTimelineNotice(session, 'warning', message);
        broadcast('acp:event', session.chatId, { type: 'warning', text: message });
        return;
      }
      if (!isAcpErrorFatalForSession(text, Boolean(session.sessionId), session.status)) {
        appendAcpSystemMessage(session, `ACP warning: ${text}`);
        appendAcpTimelineNotice(session, 'warning', text);
        broadcast('acp:event', session.chatId, { type: 'warning', text });
        return;
      }
      session.status = 'error';
      clearPendingInteractions(session);
      appendAcpSystemMessage(session, `ACP error: ${text}`);
      appendAcpTimelineNotice(session, 'error', `ACP error: ${text}`);
      broadcast('acp:event', session.chatId, { type: 'error', text: `ACP error: ${text}` });
      return;
    }

    // Responses (have result + id)
    if (isJsonRpcId(id) && result) {
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
        session.availableCommands = mergeAcpSlashCommands(
          loadSlashCommandsFromSkills(session.cwd || process.cwd()),
          session.availableCommands ?? [],
          defaultAcpSlashCommands(),
        );
        // Apply startup/pending mode and model binding from Mergen config on session start.
        setSessionMode(session, session.currentModeId || startupModeToModeId(loadConfig().acpStartupMode), true);
        updateAcpStandbyStatus(session.chatId, 'idle', session.sessionId);
        broadcast('acp:event', session.chatId, { type: 'sessionCreated', sessionId: session.sessionId });
        broadcast('acp:event', session.chatId, { type: 'commands', commands: session.availableCommands });
        // Flush any prompts queued while starting
        flushNextQueuedPrompt(session);
        return;
      }

      // session/prompt response (turn complete)
      if (result.stopReason !== undefined || result.text !== undefined) {
        session.status = 'idle';
        clearPendingInteractions(session);
        updateAcpStandbyStatus(session.chatId, 'idle');
        scheduleAcpChangeSummary(session);
        broadcast('acp:event', session.chatId, { type: 'promptResponse', stopReason: result.stopReason, text: result.text, queuedPrompts: session.queuedPrompts.length });
        // Flush queued prompts one at a time
        flushNextQueuedPrompt(session);
        return;
      }

      return;
    }

    // Requests from agent (have id and method, no result)
    if (isJsonRpcId(id) && method) {
      if (method === 'session/request_permission') {
        const params = msg.params as Record<string, unknown>;
        const optionsRaw = (params.options as Array<Record<string, unknown>>) || [];
        const options = optionsRaw.map((o) => ({
          id: (o.optionId as string) || '',
          label: (o.name as string) || '',
        }));
        const requestId = permissionRequestIdFromRpc(id);
        const autoOptionId = firstAutoApproveOptionId(options, loadConfig().opencode.acpAutoApprovePermissions);
        if (requestId && autoOptionId) {
          sendRawRpc(session, buildAcpPermissionResponse(id, autoOptionId));
          session.status = 'running';
          updateAcpStandbyStatus(session.chatId, 'running');
          broadcast('acp:event', session.chatId, { type: 'permissionResponse', requestId, rejected: false, autoApproved: true, status: 'running', queuedPrompts: session.queuedPrompts.length });
        } else {
          const token = createPendingInteraction(session, {
            kind: 'permission',
            rpcId: id,
            optionIds: new Set(options.map((option) => option.id).filter((optionId) => optionId.length > 0)),
          });
          appendAcpTimelinePermission(session, {
            interactionKind: 'permission',
            requestId: token,
            header: 'Permission Required',
            question: (params.message as string) || '',
            options,
          });
          session.status = 'permission';
          updateAcpStandbyStatus(session.chatId, 'permission');
          broadcast('acp:event', session.chatId, {
            type: 'permission',
            requestId: token,
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
      if (method === 'opencode/question') {
        const params = msg.params as Record<string, unknown>;
        const questionsRaw = (params.questions as Array<Record<string, unknown>>) || [];
        const questions = questionsRaw.map((q) => {
          const optionsRaw = (q.options as Array<Record<string, unknown>>) || [];
          return {
            header: (q.header as string) || 'Question',
            question: (q.question as string) || '',
            options: optionsRaw.map((o, idx) => {
              const label = (o.label as string) || (o.name as string) || String(idx + 1);
              return {
                id: String(idx),
                label,
                description: (o.description as string) || '',
              };
            }),
          };
        });
        const token = createPendingInteraction(session, {
          kind: 'question',
          rpcId: id,
          questionCount: Math.max(questions.length, 1),
        });
        const first = questions[0] || { header: 'Question', question: '', options: [] };
        appendAcpTimelinePermission(session, {
          interactionKind: 'question',
          requestId: token,
          header: first.header,
          question: first.question,
          options: first.options,
        });
        session.status = 'permission';
        updateAcpStandbyStatus(session.chatId, 'permission');
        broadcast('acp:event', session.chatId, {
          type: 'question',
          requestId: token,
          header: first.header,
          question: first.question,
          options: first.options,
          questions,
          sessionId: session.sessionId,
          status: 'permission',
        });
        return;
      }
      return;
    }

    if ((method === 'session/request_permission' || method === 'opencode/question') && !isJsonRpcId(id)) {
      broadcastAcpWarning(session, `ACP ${method} request did not include a valid JSON-RPC id.`);
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
          appendAcpTimelineMessageChunk(session, 'assistant', text);
          broadcast('acp:event', session.chatId, { type: 'messageChunk', text, role: 'assistant' });
          return;
        }
        case 'user_message_chunk': {
          const content = (update.content as Record<string, unknown>) || {};
          const text = (content.text as string) || '';
          appendAcpTimelineMessageChunk(session, 'user', text);
          broadcast('acp:event', session.chatId, { type: 'messageChunk', text, role: 'user' });
          return;
        }
        case 'tool_call': {
          const toolCallId = (update.toolCallId as string) || '';
          const title = (update.title as string) || '';
          const kind = (update.kind as string) || '';
          const status = (update.status as string) || 'pending';
          session.messages.push({ role: 'system', text: `${title} (${kind})`, timestamp: Date.now() });
          upsertAcpTimelineTool(session, { toolCallId, title, kind, status, raw: update });
          if (isAcpFileModifyingToolKind(kind) && isTerminalToolStatus(status)) {
            scheduleAcpChangeSummary(session);
          }
          broadcast('acp:event', session.chatId, { type: 'toolCall', toolCallId, title, kind, status, raw: update });
          return;
        }
        case 'tool_call_update': {
          const toolCallId = (update.toolCallId as string) || '';
          const status = (update.status as string) || '';
          const tool = updateAcpTimelineToolStatus(session, toolCallId, status);
          if (tool?.type === 'tool' && isAcpFileModifyingToolKind(tool.kind) && isTerminalToolStatus(status)) {
            scheduleAcpChangeSummary(session);
          }
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
          const serverCommands = (update.availableCommands as AcpAvailableCommand[]) || (update.commands as AcpAvailableCommand[]) || [];
          // Merge custom commands from .zed/skills/ with server commands
          const customCommands = loadSlashCommandsFromSkills(session.cwd || process.cwd());
          session.availableCommands = mergeAcpSlashCommands(customCommands, serverCommands, defaultAcpSlashCommands());
          broadcast('acp:event', session.chatId, { type: 'commands', commands: session.availableCommands });
          return;
        }
        default:
          appendAcpStatusFromProtocolUpdate(session, update);
          return;
      }
    }

    if (method) {
      appendAcpStatusFromProtocolUpdate(session, msg);
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

  // CLI adapters spawn one process per queued prompt.
  if (session.tool === 'claude_acp') {
    sendClaudeCodePrompt(session, next.text, next.attachments);
    return;
  }
  if (session.tool === 'codex_acp') {
    sendCodexPrompt(session, next.text, next.attachments);
    return;
  }

  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  updateAcpChatTitleFromPrompt(session, fullText);
  appendAcpTimelineMessage(session, 'user', fullText);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, prompt: [{ type: 'text', text: fullText }] });
  broadcast('acp:event', session.chatId, { type: 'promptSent', text: fullText, queuedPrompts: session.queuedPrompts.length });
}

const PLAN_SYSTEM_INSTRUCTION = `You are in PLAN MODE. Your task is to analyze the user's request and create a detailed implementation plan. Follow these rules:
1. Read and understand the relevant code files before making a plan.
2. Create a clear, step-by-step implementation plan with specific file paths and changes.
3. Do NOT make any code changes, edits, or modifications. Plan only.
4. Present the plan in a structured format with sections: Overview, Steps, Files to modify, and Risks/Considerations.
5. Be specific about what code to add, modify, or remove in each step.`;

export function claudeCodeArgsForMode(modeId?: string): string[] {
  return ['--print', '--output-format', 'stream-json', '--verbose', '--permission-mode', modeId === 'plan' ? 'plan' : 'bypassPermissions'];
}

export function claudeCodePromptTextForMode(promptText: string, _modeId?: string): string {
  return promptText;
}

function queueAdapterPromptIfRunning(session: AcpSession, promptText: string, attachments: string[], fullText: string): boolean {
  if (session.status !== 'running') return false;
  updateAcpChatTitleFromPrompt(session, fullText);
  session.queuedPrompts.push({ text: promptText, attachments, modeId: session.currentModeId || 'build', finalPromptText: fullText });
  broadcast('acp:event', session.chatId, { type: 'queued', count: session.queuedPrompts.length, queuedPrompts: session.queuedPrompts.length });
  return true;
}

function completeAdapterPlanTurn(session: AcpSession): void {
  session.status = 'permission';
  const requestId = `plan-complete-${Date.now()}`;
  const planQuestion: OpenCodeQuestion = {
    kind: 'question',
    header: 'Plan Complete',
    question: 'The plan has been generated. How would you like to proceed?',
    options: [
      { id: 'accept_implement', label: 'Accept & Implement', description: 'Approve the plan and start implementing it' },
      { id: 'accept', label: 'Accept Plan', description: 'Keep the plan, stay in plan mode for refinement' },
      { id: 'reject', label: 'Reject & Request Changes', description: 'Discard the plan and provide new instructions' },
    ],
    multiple: false,
    custom: false,
    requestId,
    sessionId: session.sessionId || '',
  };
  session.pendingInteractions.set(requestId, { kind: 'question', rpcId: requestId, questionCount: 1 });
  appendAcpTimelinePermission(session, { interactionKind: 'question', requestId, header: planQuestion.header, question: planQuestion.question, options: planQuestion.options });
  broadcast('acp:event', session.chatId, { type: 'question', ...planQuestion });
}

function completeAdapterPromptTurn(session: AcpSession): void {
  session.status = 'idle';
  clearPendingInteractions(session);
  scheduleAcpChangeSummary(session);
  broadcast('acp:event', session.chatId, { type: 'promptResponse', stopReason: 'end_turn', queuedPrompts: session.queuedPrompts.length });
  flushNextQueuedPrompt(session);
}

function sendClaudeCodePrompt(session: AcpSession, promptText: string, attachments: string[]): void {
  const isPlanMode = session.currentModeId === 'plan';
  const effectiveText = claudeCodePromptTextForMode(promptText, session.currentModeId);
  const fullText = buildAcpPromptText(effectiveText, attachments);

  // Queue if already running
  if (queueAdapterPromptIfRunning(session, promptText, attachments, fullText)) {
    return;
  }

  session.status = 'running';
  updateAcpChatTitleFromPrompt(session, fullText);
  appendAcpTimelineMessage(session, 'user', fullText);
  broadcast('acp:event', session.chatId, { type: 'promptSent', text: fullText, queuedPrompts: session.queuedPrompts.length });

  const args = claudeCodeArgsForMode(session.currentModeId);

  try {
    // On Windows, .cmd files require shell:true. Stdin piping through
    // cmd.exe doesn't work, so pass prompt via temp file + shell redirect.
    let proc: ReturnType<typeof spawn>;
    if (process.platform === 'win32') {
      const tmpFile = path.join(os.tmpdir(), `mergen-claude-${Date.now()}-${Math.random().toString(36).slice(2, 8)}.txt`);
      fs.writeFileSync(tmpFile, fullText, 'utf-8');
      // Use shell:true with stdin redirect from temp file
      proc = spawn(`claude.cmd ${args.join(' ')} < "${tmpFile}"`, [], {
        cwd: session.cwd,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
        shell: true,
      });
      const cleanup = () => { try { fs.unlinkSync(tmpFile); } catch { /* ignore */ } };
      proc.on('exit', cleanup);
      proc.on('error', cleanup);
    } else {
      proc = spawn('claude', args, {
        cwd: session.cwd,
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true,
      });
      proc.stdin?.write(fullText);
      proc.stdin?.end();
    }

    session.process = proc;

    // Parse NDJSON output
    let buffer = '';
    proc.stdout?.on('data', (data) => {
      buffer += data.toString();
      let idx;
      while ((idx = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 1);
        if (line.trim()) {
          handleClaudeCodeLine(session, line);
        }
      }
    });

    proc.stderr?.on('data', (data) => {
      const text = stripAnsi(data.toString()).trim();
      if (!text) return;
      session.partialStderr = (session.partialStderr || '') + text + '\n';
      appendAcpTimelineNotice(session, 'stderr', text);
      broadcast('acp:event', session.chatId, { type: 'stderr', text });
    });

    proc.on('exit', (_code) => {
      session.process = null;

      // Plan mode: show plan-complete question instead of normal completion
      if (isPlanMode) {
        completeAdapterPlanTurn(session);
        return;
      }

      completeAdapterPromptTurn(session);
    });

    proc.on('error', (err) => {
      session.process = null;
      session.status = 'error';
      clearPendingInteractions(session);
      const text = err.message;
      appendAcpSystemMessage(session, `Claude Code error: ${text}`);
      appendAcpTimelineNotice(session, 'error', `Claude Code error: ${text}`);
      broadcast('acp:event', session.chatId, { type: 'error', text: `Claude Code error: ${text}` });
    });
  } catch (err) {
    session.status = 'error';
    const text = err instanceof Error ? err.message : String(err);
    appendAcpSystemMessage(session, `Claude Code spawn error: ${text}`);
    appendAcpTimelineNotice(session, 'error', `Claude Code spawn error: ${text}`);
    broadcast('acp:event', session.chatId, { type: 'error', text: `Claude Code spawn error: ${text}` });
  }
}

function sendCodexPrompt(session: AcpSession, promptText: string, attachments: string[]): void {
  const isPlanMode = session.currentModeId === 'plan';
  const effectiveText = isPlanMode ? `${PLAN_SYSTEM_INSTRUCTION}\n\nUser request: ${promptText}` : promptText;
  const fullText = buildAcpPromptText(effectiveText, attachments);

  if (queueAdapterPromptIfRunning(session, promptText, attachments, fullText)) {
    return;
  }

  session.status = 'running';
  updateAcpChatTitleFromPrompt(session, fullText);
  appendAcpTimelineMessage(session, 'user', fullText);
  broadcast('acp:event', session.chatId, { type: 'promptSent', text: fullText, queuedPrompts: session.queuedPrompts.length });

  const codexBin = getCodexBinPath();
  const args = codexExecJsonArgs(session.cwd);

  try {
    let proc: ReturnType<typeof spawn>;
    if (process.platform === 'win32' && codexBin.toLowerCase().endsWith('.cmd')) {
      const tmpFile = path.join(os.tmpdir(), `mergen-codex-acp-${Date.now()}-${Math.random().toString(36).slice(2, 8)}.txt`);
      fs.writeFileSync(tmpFile, fullText, 'utf-8');
      const command = [...[codexBin], ...args].map(windowsShellQuote).join(' ');
      proc = spawn(`${command} < ${windowsShellQuote(tmpFile)}`, [], {
        cwd: session.cwd,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
        shell: true,
      });
      const cleanup = () => { try { fs.unlinkSync(tmpFile); } catch { /* ignore */ } };
      proc.on('exit', cleanup);
      proc.on('error', cleanup);
    } else {
      proc = spawn(codexBin, args, {
        cwd: session.cwd,
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true,
      });
      proc.stdin?.write(fullText);
      proc.stdin?.end();
    }

    session.process = proc;

    let buffer = '';
    proc.stdout?.on('data', (data) => {
      buffer += data.toString();
      let idx;
      while ((idx = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 1);
        if (line.trim()) {
          handleCodexLine(session, line);
        }
      }
    });

    proc.stderr?.on('data', (data) => {
      const text = stripAnsi(data.toString()).trim();
      if (!text) return;
      session.partialStderr = (session.partialStderr || '') + text + '\n';
      appendAcpTimelineNotice(session, 'stderr', text);
      broadcast('acp:event', session.chatId, { type: 'stderr', text });
    });

    proc.on('exit', (code) => {
      const cancelled = Boolean(session.cancelGraceUntil && Date.now() < session.cancelGraceUntil);
      session.process = null;
      if (cancelled) return;

      if (typeof code === 'number' && code !== 0) {
        session.status = 'error';
        clearPendingInteractions(session);
        const text = `Codex exited with code ${code}`;
        appendAcpSystemMessage(session, text);
        appendAcpTimelineNotice(session, 'error', text);
        broadcast('acp:event', session.chatId, { type: 'error', text });
        return;
      }

      if (isPlanMode) {
        completeAdapterPlanTurn(session);
        return;
      }

      completeAdapterPromptTurn(session);
    });

    proc.on('error', (err) => {
      session.process = null;
      session.status = 'error';
      clearPendingInteractions(session);
      const text = err.message;
      appendAcpSystemMessage(session, `Codex error: ${text}`);
      appendAcpTimelineNotice(session, 'error', `Codex error: ${text}`);
      broadcast('acp:event', session.chatId, { type: 'error', text: `Codex error: ${text}` });
    });
  } catch (err) {
    session.status = 'error';
    clearPendingInteractions(session);
    const text = err instanceof Error ? err.message : String(err);
    appendAcpSystemMessage(session, `Codex spawn error: ${text}`);
    appendAcpTimelineNotice(session, 'error', `Codex spawn error: ${text}`);
    broadcast('acp:event', session.chatId, { type: 'error', text: `Codex spawn error: ${text}` });
  }
}

function handleCodexLine(session: AcpSession, line: string): void {
  const event = parseCodexExecJsonLine(line);
  if (!event) return;

  if (event.kind === 'assistant_message') {
    appendAcpTimelineMessageChunk(session, 'assistant', event.text);
    broadcast('acp:event', session.chatId, { type: 'messageChunk', text: event.text, role: 'assistant' });
    return;
  }

  if (event.kind === 'tool') {
    const item = event.status === 'running'
      ? upsertAcpTimelineTool(session, { toolCallId: event.id, title: event.title, kind: event.toolKind, status: event.status, raw: event.raw })
      : updateAcpTimelineToolStatus(session, event.id, event.status)
        ?? upsertAcpTimelineTool(session, { toolCallId: event.id, title: event.title, kind: event.toolKind, status: event.status, raw: event.raw });
    if (item?.type === 'tool' && isAcpFileModifyingToolKind(item.kind) && event.status !== 'running') {
      scheduleAcpChangeSummary(session);
    }
    broadcast('acp:event', session.chatId, event.status === 'running'
      ? { type: 'toolCall', toolCallId: event.id, title: event.title, kind: event.toolKind, status: event.status, raw: event.raw }
      : { type: 'toolCallUpdate', toolCallId: event.id, status: event.status });
    return;
  }

  if (event.kind === 'status') {
    appendAcpTimelineStatus(session, 'status', event.title, event.text);
    return;
  }

  appendAcpSystemMessage(session, `Codex error: ${event.text}`);
  appendAcpTimelineNotice(session, 'error', `Codex error: ${event.text}`);
  broadcast('acp:event', session.chatId, { type: 'error', text: `Codex error: ${event.text}` });
}

function windowsShellQuote(value: string): string {
  return `"${value.replace(/"/g, '\\"')}"`;
}

function handleClaudeCodeLine(session: AcpSession, line: string): void {
  try {
    const msg = JSON.parse(line) as Record<string, unknown>;

    // System init message
    if (msg.type === 'system' && msg.subtype === 'init') {
      return;
    }

    // Assistant message with content blocks
    if (msg.type === 'assistant' && msg.message) {
      const message = msg.message as Record<string, unknown>;
      const content = message.content as Array<Record<string, unknown>> | undefined;
      if (Array.isArray(content)) {
        for (const block of content) {
          if (block.type === 'thinking' && typeof block.thinking === 'string') {
            appendAcpTimelineThinkingChunk(session, block.thinking);
            broadcast('acp:event', session.chatId, { type: 'thinkingChunk', text: block.thinking });
          } else if (block.type === 'text' && typeof block.text === 'string') {
            appendAcpTimelineMessageChunk(session, 'assistant', block.text);
            broadcast('acp:event', session.chatId, { type: 'messageChunk', text: block.text, role: 'assistant' });
          } else if (block.type === 'tool_use' && typeof block.id === 'string') {
            const toolCallId = block.id;
            const name = (typeof block.name === 'string' ? block.name : '').trim();
            const input = (block.input as Record<string, unknown>) || {};
            const title = claudeToolUseTitle(name, input);
            const kind = claudeToolUseKind(name);
            upsertAcpTimelineTool(session, { toolCallId, title, kind, status: 'running', raw: block });
            broadcast('acp:event', session.chatId, { type: 'toolCall', toolCallId, title, kind, status: 'running', raw: block });
          }
        }
      }
      return;
    }

    // User message with tool_result blocks
    if (msg.type === 'user' && msg.message) {
      const message = msg.message as Record<string, unknown>;
      const content = message.content as Array<Record<string, unknown>> | undefined;
      if (Array.isArray(content)) {
        for (const block of content) {
          if (block.type === 'tool_result' && typeof block.tool_use_id === 'string') {
            const toolCallId = block.tool_use_id;
            const isError = block.is_error === true;
            const tool = updateAcpTimelineToolStatus(session, toolCallId, isError ? 'failed' : 'completed');
            if (tool?.type === 'tool' && isAcpFileModifyingToolKind(tool.kind)) {
              scheduleAcpChangeSummary(session);
            }
            broadcast('acp:event', session.chatId, { type: 'toolCallUpdate', toolCallId, status: isError ? 'failed' : 'completed' });
          }
        }
      }
      return;
    }

    // Result message (final response)
    if (msg.type === 'result') {
      // The final result text is already streamed via messageChunk events
      return;
    }
  } catch {
    // Ignore unparseable lines
  }
}

function claudeToolUseKind(name: string): string {
  const lower = name.toLowerCase();
  if (lower === 'bash' || lower === 'shell' || lower === 'terminal') return 'bash';
  if (lower === 'edit' || lower === 'write' || lower === 'create') return 'edit';
  if (lower === 'read' || lower === 'readfile') return 'read';
  if (lower === 'grep' || lower === 'glob' || lower === 'search') return 'search';
  if (lower === 'todowrite' || lower === 'task') return 'todo';
  return lower || 'tool';
}

function claudeToolUseTitle(name: string, input: Record<string, unknown>): string {
  const lower = name.toLowerCase();
  // Bash: show command
  if ((lower === 'bash' || lower === 'shell' || lower === 'terminal') && typeof input.command === 'string') {
    const cmd = input.command.trim();
    return cmd.length > 120 ? `${cmd.slice(0, 117)}...` : cmd;
  }
  // Edit/Write/Read: show file path
  const filePath = input.file_path ?? input.path ?? input.filePath;
  if (typeof filePath === 'string' && filePath.trim()) {
    return filePath.trim();
  }
  // Fallback to tool name
  return name || 'Tool';
}

function isValidQueueIndex(session: AcpSession, index: number): boolean {
  return Number.isInteger(index) && index >= 0 && index < session.queuedPrompts.length;
}

function broadcastQueueUpdated(session: AcpSession): void {
  broadcast('acp:event', session.chatId, {
    type: 'queueUpdated',
    count: session.queuedPrompts.length,
    queuedPrompts: session.queuedPrompts.length,
  });
}

export function runAcpQueuedPromptNext(chatId: string, index: number): boolean {
  const session = sessions.get(chatId);
  if (!session || !isValidQueueIndex(session, index)) return false;

  const [prompt] = session.queuedPrompts.splice(index, 1);
  session.queuedPrompts.unshift(prompt);
  broadcastQueueUpdated(session);

  if (session.status === 'idle') {
    flushNextQueuedPrompt(session);
  }

  return true;
}

export function deleteAcpQueuedPrompt(chatId: string, index: number): boolean {
  const session = sessions.get(chatId);
  if (!session || !isValidQueueIndex(session, index)) return false;

  session.queuedPrompts.splice(index, 1);
  broadcastQueueUpdated(session);
  return true;
}

export function moveAcpQueuedPrompt(chatId: string, fromIndex: number, toIndex: number): boolean {
  const session = sessions.get(chatId);
  if (!session || !isValidQueueIndex(session, fromIndex) || !isValidQueueIndex(session, toIndex)) return false;
  if (fromIndex === toIndex) return true;

  const [moved] = session.queuedPrompts.splice(fromIndex, 1);
  session.queuedPrompts.splice(toIndex, 0, moved);
  broadcastQueueUpdated(session);
  return true;
}

export function restoreAcpQueuedPrompt(chatId: string, index: number, prompt: QueuedAcpPrompt): boolean {
  const session = sessions.get(chatId);
  if (!session || !Number.isInteger(index) || index < 0) return false;

  const restored: QueuedAcpPrompt = {
    text: prompt.text,
    attachments: [...prompt.attachments],
    modeId: prompt.modeId,
    finalPromptText: prompt.finalPromptText,
  };
  const insertIndex = Math.min(index, session.queuedPrompts.length);
  session.queuedPrompts.splice(insertIndex, 0, restored);
  broadcastQueueUpdated(session);
  return true;
}

export function sendAcpPrompt(chatId: string, promptText: string, attachments: string[], modeId?: string, returnIndex?: number): void {
  const session = sessions.get(chatId);
  if (!session) return;

  // Claude Code: spawn a new process per prompt
  if (session.tool === 'claude_acp') {
    sendClaudeCodePrompt(session, promptText, attachments);
    return;
  }
  if (session.tool === 'codex_acp') {
    sendCodexPrompt(session, promptText, attachments);
    return;
  }

  const effectiveModeId = modeId || session.currentModeId || 'build';
  const fullText = buildAcpPromptText(promptText, attachments);

  // Queue when session is not ready or a turn is active
  const shouldQueue = !session.sessionId || session.status === 'starting' || session.status === 'session_created' || session.status === 'running' || session.status === 'permission';
  if (shouldQueue) {
    updateAcpChatTitleFromPrompt(session, fullText);
    const entry = { text: promptText, attachments, modeId: effectiveModeId, finalPromptText: fullText };
    if (returnIndex !== undefined && returnIndex >= 0) {
      const insertAt = Math.min(returnIndex, session.queuedPrompts.length);
      session.queuedPrompts.splice(insertAt, 0, entry);
    } else {
      session.queuedPrompts.push(entry);
    }
    broadcast('acp:event', chatId, { type: 'queued', count: session.queuedPrompts.length, queuedPrompts: session.queuedPrompts.length });
    return;
  }

  // Apply mode if explicitly provided and different
  if (modeId && modeId !== session.currentModeId) {
    setSessionMode(session, modeId);
  }

  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  updateAcpChatTitleFromPrompt(session, fullText);
  appendAcpTimelineMessage(session, 'user', fullText);
  sendRpc(session, 'session/prompt', { sessionId: session.sessionId, prompt: [{ type: 'text', text: fullText }] });
  broadcast('acp:event', chatId, { type: 'promptSent', text: fullText, queuedPrompts: session.queuedPrompts.length });
}

export function cancelAcpPrompt(chatId: string): void {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return;

  // CLI adapters run one child process per prompt.
  if (session.tool === 'claude_acp' || session.tool === 'codex_acp') {
    if (session.process) {
      session.cancelGraceUntil = Date.now() + 2000;
      if (process.platform === 'win32' && session.process.pid) {
        try {
          spawn('taskkill', ['/pid', String(session.process.pid), '/T', '/F'], { windowsHide: true });
        } catch { /* ignore */ }
      } else {
        session.process.kill();
      }
      session.process = null;
    }
    session.status = 'idle';
    clearPendingInteractions(session);
    appendAcpTimelineNotice(session, 'cancelled', `${acpLabelForTool(session.tool)} turn cancelled.`);
    broadcast('acp:event', chatId, { type: 'cancelled', queuedPrompts: session.queuedPrompts.length });
    return;
  }

  session.cancelGraceUntil = Date.now() + 2000;
  clearPendingInteractions(session);
  if (!session.cancelUnsupported) {
    sendRawRpc(session, buildAcpCancelNotification(session.sessionId));
  }
  session.status = 'idle';
  updateAcpStandbyStatus(session.chatId, 'idle');
  appendAcpTimelineNotice(session, 'cancelled', 'ACP turn cancelled.');
  broadcast('acp:event', chatId, { type: 'cancelled', queuedPrompts: session.queuedPrompts.length });
}

export function setAcpConfigOption(chatId: string, configId: string, value: string): void {
  const session = sessions.get(chatId);
  if (!session) return;

  if (configId === 'mode') {
    session.currentModeId = value;
    broadcast('acp:event', session.chatId, { type: 'modeUpdate', modeId: value });
    // For real ACP processes, also send RPC to the process.
    if (session.tool === 'opencode' && session.sessionId) {
      sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId: 'mode', value });
    }
    return;
  }

  // CLI adapters use their own configured/default model for now.
  if (session.tool === 'claude_acp' || session.tool === 'codex_acp') return;

  if (!session.sessionId) return;

  sendRpc(session, 'session/set_config_option', { sessionId: session.sessionId, configId, value });

  if (configId === 'model') {
    session.currentModel = value;
  } else if (configId === 'effort') {
    session.currentEffort = value;
  }
}

export function sendAcpPermissionResponse(chatId: string, requestId: string, answers: string[], rejected: boolean): boolean {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return false;

  const interaction = session.pendingInteractions.get(requestId);
  if (!interaction || interaction.kind !== 'permission') {
    broadcastAcpWarning(session, 'Ignoring stale ACP permission response.');
    return false;
  }

  const optionId = answers.find((answer) => answer.length > 0) || '';
  if (!rejected && (!optionId || !interaction.optionIds.has(optionId))) {
    broadcastAcpWarning(session, 'Ignoring ACP permission response with no valid selected option.');
    return false;
  }

  session.pendingInteractions.delete(requestId);
  resolveAcpTimelinePermission(session, requestId, rejected);
  sendRawRpc(session, buildAcpPermissionResponse(interaction.rpcId, optionId, rejected));
  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  broadcast('acp:event', chatId, { type: 'permissionResponse', requestId, rejected, status: 'running', queuedPrompts: session.queuedPrompts.length });
  return true;
}

export function sendAcpQuestionResponse(chatId: string, requestId: string, answers: string[][], rejected: boolean): boolean {
  const session = sessions.get(chatId);
  if (!session || !session.sessionId) return false;

  const interaction = session.pendingInteractions.get(requestId);
  if (!interaction || interaction.kind !== 'question') {
    broadcastAcpWarning(session, 'Ignoring stale ACP question response.');
    return false;
  }

  const normalizedAnswers = answers.map((answer) => answer.filter((value) => value.length > 0));
  if (!rejected && (normalizedAnswers.length < interaction.questionCount || normalizedAnswers.some((answer) => answer.length === 0))) {
    broadcastAcpWarning(session, 'Ignoring ACP question response with incomplete answers.');
    return false;
  }

  session.pendingInteractions.delete(requestId);
  resolveAcpTimelinePermission(session, requestId, rejected);

  // Plan-complete question: handle locally (no ACP process to respond to)
  if (requestId.startsWith('plan-complete-')) {
    handlePlanCompleteAnswer(session, normalizedAnswers, rejected);
    return true;
  }

  sendRawRpc(session, buildAcpQuestionResponse(interaction.rpcId, normalizedAnswers, rejected));
  session.status = 'running';
  updateAcpStandbyStatus(session.chatId, 'running');
  broadcast('acp:event', chatId, { type: 'questionResponse', requestId, rejected, status: 'running', queuedPrompts: session.queuedPrompts.length });
  return true;
}

function handlePlanCompleteAnswer(session: AcpSession, answers: string[][], rejected: boolean): void {
  // The answer is the option label (e.g., "Accept & Implement"). Map to action.
  const answerLabel = (answers[0]?.[0] || '').toLowerCase();

  if (rejected || answerLabel.includes('reject')) {
    // Reject: clear and stay in plan mode, user can type new prompt
    session.status = 'idle';
    broadcast('acp:event', session.chatId, { type: 'questionResponse', requestId: '', rejected: true, status: 'idle', queuedPrompts: session.queuedPrompts.length });
    return;
  }

  if (answerLabel.includes('implement')) {
    // Accept and implement: switch to build mode and send implementation prompt
    session.currentModeId = 'build';
    broadcast('acp:event', session.chatId, { type: 'modeUpdate', modeId: 'build' });

    // Collect the plan from the last assistant messages
    const planText = session.messages
      .filter((m) => m.role === 'assistant')
      .map((m) => m.text)
      .join('\n\n');

    const implementPrompt = `Based on the plan we just discussed, please implement all the changes now. Here is the plan:\n\n${planText}`;
    if (session.tool === 'codex_acp') {
      sendCodexPrompt(session, implementPrompt, []);
    } else {
      sendClaudeCodePrompt(session, implementPrompt, []);
    }
    return;
  }

  if (answerLabel.includes('accept')) {
    // Accept plan only: stay in plan mode, user can refine
    session.status = 'idle';
    broadcast('acp:event', session.chatId, { type: 'promptResponse', stopReason: 'end_turn', queuedPrompts: session.queuedPrompts.length });
    return;
  }

  // Unknown choice: just clear
  session.status = 'idle';
  broadcast('acp:event', session.chatId, { type: 'promptResponse', stopReason: 'end_turn', queuedPrompts: session.queuedPrompts.length });
}

export function getAcpSession(chatId: string): AcpChatSession | undefined {
  const session = sessions.get(chatId);
  if (!session) return undefined;
  return {
    sessionId: session.sessionId,
    status: session.status,
    title: session.title,
    timeline: session.timeline,
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
    tool: session.tool,
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
