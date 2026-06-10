import type { GitDiffSummary } from './gitDiffSummary';

export enum ShellKind {
  PowerShell = 'powershell',
  Cmd = 'cmd',
  Zsh = 'zsh',
}

export const ShellKindLabel: Record<ShellKind, string> = {
  [ShellKind.PowerShell]: 'PowerShell',
  [ShellKind.Cmd]: 'CMD',
  [ShellKind.Zsh]: 'zsh',
};

export const ShellKindCommand = (kind: ShellKind): [string, string[]] => {
  switch (kind) {
    case ShellKind.PowerShell:
      return ['powershell.exe', ['-NoLogo']];
    case ShellKind.Cmd:
      return ['cmd.exe', []];
    case ShellKind.Zsh:
      return ['zsh', ['-l']];
  }
};

export const defaultShellForPlatform = (): ShellKind => {
  if (process.platform === 'win32') return ShellKind.PowerShell;
  return ShellKind.Zsh;
};

export const supportedShellsForPlatform = (): ShellKind[] => {
  if (process.platform === 'win32') return [ShellKind.PowerShell, ShellKind.Cmd];
  return [ShellKind.Zsh];
};

export enum BuiltinLauncherKind {
  Droid = 'droid',
  Codex = 'codex',
  OpenCode = 'opencode',
  Claude = 'claude',
}

export const BuiltinLauncherKindAll: BuiltinLauncherKind[] = [
  BuiltinLauncherKind.OpenCode,
  BuiltinLauncherKind.Codex,
  BuiltinLauncherKind.Droid,
  BuiltinLauncherKind.Claude,
];

export const BuiltinLauncherKindId = (kind: BuiltinLauncherKind): string => {
  switch (kind) {
    case BuiltinLauncherKind.Codex: return 'codex';
    case BuiltinLauncherKind.Claude: return 'claude';
    case BuiltinLauncherKind.Droid: return 'droid';
    case BuiltinLauncherKind.OpenCode: return 'opencode';
  }
};

export const BuiltinLauncherKindDefaultDisplayName = (kind: BuiltinLauncherKind): string => {
  switch (kind) {
    case BuiltinLauncherKind.Codex: return 'Codex';
    case BuiltinLauncherKind.Claude: return 'Claude';
    case BuiltinLauncherKind.Droid: return 'Droid';
    case BuiltinLauncherKind.OpenCode: return 'OpenCode';
  }
};

export const BuiltinLauncherKindDefaultLaunchCommand = (kind: BuiltinLauncherKind): string => {
  switch (kind) {
    case BuiltinLauncherKind.Codex: return 'codex';
    case BuiltinLauncherKind.Claude: return 'claude';
    case BuiltinLauncherKind.Droid: return 'droid';
    case BuiltinLauncherKind.OpenCode: return 'opencode';
  }
};

export enum LauncherIconKey {
  Codex = 'codex',
  Claude = 'claude',
  Droid = 'droid',
  OpenCode = 'opencode',
  Terminal = 'terminal',
  Spark = 'spark',
  Message = 'message',
  Bot = 'bot',
  Code = 'code',
  Wrench = 'wrench',
  Rocket = 'rocket',
}

export const LauncherIconKeyLabel: Record<LauncherIconKey, string> = {
  [LauncherIconKey.Codex]: 'Codex',
  [LauncherIconKey.Claude]: 'Claude',
  [LauncherIconKey.Droid]: 'Droid',
  [LauncherIconKey.OpenCode]: 'OpenCode',
  [LauncherIconKey.Terminal]: 'Terminal',
  [LauncherIconKey.Spark]: 'Spark',
  [LauncherIconKey.Message]: 'Message',
  [LauncherIconKey.Bot]: 'Bot',
  [LauncherIconKey.Code]: 'Code',
  [LauncherIconKey.Wrench]: 'Wrench',
  [LauncherIconKey.Rocket]: 'Rocket',
};

export const LauncherIconKeyCustomPresets: LauncherIconKey[] = [
  LauncherIconKey.Terminal,
  LauncherIconKey.Spark,
  LauncherIconKey.Message,
  LauncherIconKey.Bot,
  LauncherIconKey.Code,
  LauncherIconKey.Wrench,
  LauncherIconKey.Rocket,
];

export interface LauncherEntry {
  id: string;
  builtin?: BuiltinLauncherKind;
  displayName: string;
  launchCommand: string;
  enabled: boolean;
  iconKey: LauncherIconKey;
  bypassPermissions?: boolean;
}

export const defaultLaunchers = (): LauncherEntry[] =>
  BuiltinLauncherKindAll.map((kind) => ({
    id: BuiltinLauncherKindId(kind),
    builtin: kind,
    displayName: BuiltinLauncherKindDefaultDisplayName(kind),
    launchCommand: BuiltinLauncherKindDefaultLaunchCommand(kind),
    enabled: true,
    iconKey: (() => {
      switch (kind) {
        case BuiltinLauncherKind.Codex: return LauncherIconKey.Codex;
        case BuiltinLauncherKind.Claude: return LauncherIconKey.Claude;
        case BuiltinLauncherKind.Droid: return LauncherIconKey.Droid;
        case BuiltinLauncherKind.OpenCode: return LauncherIconKey.OpenCode;
      }
    })(),
    bypassPermissions: kind === BuiltinLauncherKind.Claude,
  }));

export enum TerminalKind {
  Foreground = 'foreground',
  Background = 'background',
}

export const TerminalKindLabel: Record<TerminalKind, string> = {
  [TerminalKind.Foreground]: 'Foreground',
  [TerminalKind.Background]: 'Background',
};

export enum TerminalManagerFilter {
  Foreground = 'foreground',
  Background = 'background',
}

export enum InputHistoryFilter {
  All = 'all',
  Foreground = 'foreground',
  Background = 'background',
}

export const InputHistoryFilterLabel: Record<InputHistoryFilter, string> = {
  [InputHistoryFilter.All]: 'All',
  [InputHistoryFilter.Foreground]: 'Foreground',
  [InputHistoryFilter.Background]: 'Background',
};

export enum MainVisibilityMode {
  Global = 'global',
  SelectedProject = 'selected_project',
}

export enum LeftSidebarTab {
  Directory = 'directory',
  SourceControl = 'source_control',
  TerminalManager = 'terminal_manager',
  InputHistory = 'input_history',
}

export enum TerminalInputHistoryFilter {
  Foreground = 'foreground',
  Background = 'background',
  All = 'all',
}

export const defaultProjectExplorerWidth = 352;
export const defaultChecklistPanelWidth = 352;
export const defaultBrowserPanelWidth = 520;

export interface UiConfig {
  showProjectExplorer: boolean;
  projectExplorerExpanded: boolean;
  projectExplorerWidth: number;
  showTerminalManager: boolean;
  terminalManagerExpanded: boolean;
  multiTerminalViewEnabled: boolean;
  terminalManagerFilter: TerminalManagerFilter;
  terminalManagerHideInactiveProjects: boolean;
  lastSelectedProjectId?: number;
  mainVisibilityMode: MainVisibilityMode;
  leftSidebarTab: LeftSidebarTab;
  checklistPanelExpanded: boolean;
  browserPanelExpanded: boolean;
  checklistPanelWidth: number;
  browserPanelWidth: number;
  inputHistoryFilter: InputHistoryFilter;
}

export const defaultUiConfig = (): UiConfig => ({
  showProjectExplorer: true,
  projectExplorerExpanded: true,
  projectExplorerWidth: defaultProjectExplorerWidth,
  showTerminalManager: true,
  terminalManagerExpanded: true,
  multiTerminalViewEnabled: false,
  terminalManagerFilter: TerminalManagerFilter.Foreground,
  terminalManagerHideInactiveProjects: false,
  lastSelectedProjectId: undefined,
  mainVisibilityMode: MainVisibilityMode.Global,
  leftSidebarTab: LeftSidebarTab.Directory,
  checklistPanelExpanded: false,
  browserPanelExpanded: false,
  checklistPanelWidth: defaultChecklistPanelWidth,
  browserPanelWidth: defaultBrowserPanelWidth,
  inputHistoryFilter: InputHistoryFilter.All,
});

export interface ProjectRecord {
  id: number;
  name: string;
  path: string;
  savedMessages: string[];
  aiConfig: ProjectAiConfig;
  checklist: string[];
  browserLastUrl?: string;
  foregroundSavedMessages: string[];
  repoRoot?: string;
  isWorktree: boolean;
}

export interface TerminalInputRecord {
  projectPath: string;
  projectName: string;
  terminalKind: TerminalKind;
  text: string;
  recordedAt: number;
}

export interface TerminalInputHistory {
  maxEntries: number;
  entries: TerminalInputRecord[];
}

export const defaultHistoryLimit = 500;

export const defaultTerminalInputHistory = (): TerminalInputHistory => ({
  maxEntries: defaultHistoryLimit,
  entries: [],
});

export interface AppHistory {
  version: number;
  projects: Record<string, TerminalInputHistory>;
}

export const defaultAppHistory = (): AppHistory => ({
  version: 1,
  projects: {},
});

export interface OpenCodeAcpModelEntry {
  value: string;
  name: string;
}

export const APP_CONFIG_VERSION = 2;
export const DEFAULT_OPENCODE_BUILD_MODEL = 'fireworks-ai/accounts/fireworks/routers/kimi-k2p6-turbo';
export const DEFAULT_OPENCODE_PLAN_MODEL = 'openai/gpt-5.5-fast';
export const DEFAULT_OPENCODE_PLAN_EFFORT = 'xhigh';
export const DEFAULT_OPENCODE_LOOP_PROTECTION_ENABLED = true;
export const DEFAULT_OPENCODE_BUILD_STEPS_LIMIT = 32;
export const DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS = 600_000;
export const DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS = 120_000;
export const DEFAULT_OPENCODE_ACP_BIND_MODEL_TO_MODE = true;
export const DEFAULT_OPENCODE_ACP_AUTO_APPROVE_PERMISSIONS = false;
export const DEFAULT_OPENCODE_KIMI_STRICT_PERMISSIONS = true;

export interface OpenCodeModelConfig {
  buildModelSlotA: string;
  buildModelSlotB: string;
  planModel: string;
  planEffort: string;
  activeBuildModelSlot: string;
  acpFavoriteModels: string[];
  acpKnownModels: OpenCodeAcpModelEntry[];
  acpBindModelToMode: boolean;
  loopProtectionEnabled: boolean;
  buildStepsLimit: number;
  fireworksTimeoutMs: number;
  fireworksChunkTimeoutMs: number;
  acpAutoApprovePermissions: boolean;
  kimiStrictPermissions: boolean;
}

export const defaultOpenCodeModelConfig = (): OpenCodeModelConfig => {
  const config: OpenCodeModelConfig = {
    buildModelSlotA: DEFAULT_OPENCODE_BUILD_MODEL,
    buildModelSlotB: DEFAULT_OPENCODE_PLAN_MODEL,
    planModel: DEFAULT_OPENCODE_PLAN_MODEL,
    planEffort: DEFAULT_OPENCODE_PLAN_EFFORT,
    activeBuildModelSlot: 'a',
    acpFavoriteModels: [],
    acpKnownModels: [],
    acpBindModelToMode: DEFAULT_OPENCODE_ACP_BIND_MODEL_TO_MODE,
    loopProtectionEnabled: DEFAULT_OPENCODE_LOOP_PROTECTION_ENABLED,
    buildStepsLimit: DEFAULT_OPENCODE_BUILD_STEPS_LIMIT,
    fireworksTimeoutMs: DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS,
    fireworksChunkTimeoutMs: DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS,
    acpAutoApprovePermissions: DEFAULT_OPENCODE_ACP_AUTO_APPROVE_PERMISSIONS,
    kimiStrictPermissions: DEFAULT_OPENCODE_KIMI_STRICT_PERMISSIONS,
  };
  ensureConfiguredModelsAreFavorites(config);
  return config;
};

export const activeBuildModel = (config: OpenCodeModelConfig): string => {
  if (config.activeBuildModelSlot === 'b') return config.buildModelSlotB;
  return config.buildModelSlotA;
};

export const effectivePlanModel = (config: OpenCodeModelConfig): string => {
  const m = config.planModel.trim();
  return m || DEFAULT_OPENCODE_PLAN_MODEL;
};

export const effectivePlanEffort = (config: OpenCodeModelConfig): string => {
  const e = config.planEffort.trim();
  return e || DEFAULT_OPENCODE_PLAN_EFFORT;
};

export const effectiveBuildStepsLimit = (config: OpenCodeModelConfig): number => {
  if (config.buildStepsLimit === 0) return DEFAULT_OPENCODE_BUILD_STEPS_LIMIT;
  return config.buildStepsLimit;
};

export const effectiveFireworksTimeoutMs = (config: OpenCodeModelConfig): number => {
  if (config.fireworksTimeoutMs === 0) return DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS;
  return config.fireworksTimeoutMs;
};

export const effectiveFireworksChunkTimeoutMs = (config: OpenCodeModelConfig): number => {
  if (config.fireworksChunkTimeoutMs === 0) return DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS;
  return config.fireworksChunkTimeoutMs;
};

export const isAcpModelFavorite = (config: OpenCodeModelConfig, value: string): boolean => {
  const v = value.trim();
  return config.acpFavoriteModels.some((x) => x === v);
};

export const addAcpModelFavorite = (config: OpenCodeModelConfig, value: string): boolean => {
  const v = value.trim();
  if (!v || isAcpModelFavorite(config, v)) return false;
  config.acpFavoriteModels.push(v);
  return true;
};

export const toggleAcpModelFavorite = (config: OpenCodeModelConfig, value: string): boolean => {
  const v = value.trim();
  if (!v) return false;
  const idx = config.acpFavoriteModels.indexOf(v);
  if (idx >= 0) {
    config.acpFavoriteModels.splice(idx, 1);
    return false;
  }
  config.acpFavoriteModels.push(v);
  return true;
};

export const normalizeAcpFavoriteModels = (config: OpenCodeModelConfig): boolean => {
  const before = config.acpFavoriteModels;
  const seen = new Set<string>();
  config.acpFavoriteModels = before.filter((value) => {
    const v = value.trim();
    if (!v || seen.has(v)) return false;
    seen.add(v);
    return true;
  });
  return config.acpFavoriteModels.length !== before.length || config.acpFavoriteModels.some((v, i) => v !== before[i]);
};

export const ensureConfiguredModelsAreFavorites = (config: OpenCodeModelConfig): boolean => {
  const configured = [
    config.buildModelSlotA.trim(),
    config.buildModelSlotB.trim(),
    effectivePlanModel(config).trim(),
  ];
  let changed = normalizeAcpFavoriteModels(config);
  for (const m of configured) {
    if (m) changed ||= addAcpModelFavorite(config, m);
  }
  return changed;
};

export const normalizeAcpKnownModels = (config: OpenCodeModelConfig): void => {
  const seen = new Set<string>();
  config.acpKnownModels = config.acpKnownModels.filter((e) => {
    if (!e.value || seen.has(e.value)) return false;
    seen.add(e.value);
    return true;
  });
};

export const mergeAcpKnownModels = (config: OpenCodeModelConfig, options: Iterable<[string, string]>): boolean => {
  let changed = false;
  for (const [value, name] of options) {
    const existing = config.acpKnownModels.find((e) => e.value === value);
    if (existing) {
      if (existing.name !== name) {
        existing.name = name;
        changed = true;
      }
    } else {
      config.acpKnownModels.push({ value, name });
      changed = true;
    }
  }
  normalizeAcpKnownModels(config);
  return changed;
};

export interface ShortcutModifiers {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  command: boolean;
}

export const defaultShortcutModifiers = (): ShortcutModifiers => ({
  ctrl: false,
  alt: false,
  shift: false,
  command: false,
});

export interface TerminalShortcutEntry {
  id: string;
  label: string;
  key: string;
  modifiers: ShortcutModifiers;
  command: string;
  enabled: boolean;
}

export const defaultTerminalShortcuts = (): TerminalShortcutEntry[] => [
  { id: 'github-push', label: 'GitHub Push', key: 'F5', modifiers: defaultShortcutModifiers(), command: '/gt', enabled: true },
  { id: 'prepare-fix-plan', label: 'Prepare Fix Plan', key: 'F6', modifiers: defaultShortcutModifiers(), command: '/prepare-fix-plan', enabled: true },
  { id: 'implement-plan', label: 'Implement Plan', key: 'F11', modifiers: defaultShortcutModifiers(), command: '/implement-plan', enabled: true },
  { id: 'review-guard', label: 'Review Guard', key: 'F7', modifiers: defaultShortcutModifiers(), command: '/review-guard', enabled: true },
];

export interface OsNotificationConfig {
  enabled: boolean;
  onlyWhenUnfocused: boolean;
  onPermission: boolean;
  onTurnComplete: boolean;
  onSessionError: boolean;
  cooldownSecs: number;
}

export const defaultOsNotificationConfig = (): OsNotificationConfig => ({
  enabled: true,
  onlyWhenUnfocused: true,
  onPermission: true,
  onTurnComplete: true,
  onSessionError: true,
  cooldownSecs: 30,
});

export interface AcpModeToggleShortcut {
  key: string;
  modifiers: ShortcutModifiers;
  enabled: boolean;
}

export const defaultAcpModeToggleShortcut = (): AcpModeToggleShortcut => ({
  key: 'Tab',
  modifiers: defaultShortcutModifiers(),
  enabled: true,
});

export enum AcpStartupMode {
  Build = 'build',
  Plan = 'plan',
}

export const AcpStartupModeLabel: Record<AcpStartupMode, string> = {
  [AcpStartupMode.Build]: 'Default',
  [AcpStartupMode.Plan]: 'Plan',
};

export const AcpStartupModeAsModeId = (mode: AcpStartupMode): string => {
  switch (mode) {
    case AcpStartupMode.Build: return 'build';
    case AcpStartupMode.Plan: return 'plan';
  }
};

export interface AiCliConfig {
  // per-project AI CLI settings
}

export interface ProjectAiConfig {
  // placeholder for future per-project AI settings
}

export const defaultProjectAiConfig = (): ProjectAiConfig => ({});

export interface AiHooksConfig {
  // placeholder for hook-level config
}

export const defaultAiHooksConfig = (): AiHooksConfig => ({});

export interface AppConfig {
  version: number;
  defaultShell: ShellKind;
  claudeCodeCodexHookEnabled: boolean;
  ui: UiConfig;
  launchers: LauncherEntry[];
  terminalShortcuts: TerminalShortcutEntry[];
  projects: ProjectRecord[];
  aiHooks: AiHooksConfig;
  opencode: OpenCodeModelConfig;
  notifications: OsNotificationConfig;
  acpModeToggleShortcut: AcpModeToggleShortcut;
  acpStartupMode: AcpStartupMode;
}

export const defaultAppConfig = (): AppConfig => ({
  version: APP_CONFIG_VERSION,
  defaultShell: defaultShellForPlatform(),
  claudeCodeCodexHookEnabled: true,
  ui: defaultUiConfig(),
  launchers: defaultLaunchers(),
  terminalShortcuts: defaultTerminalShortcuts(),
  projects: [],
  aiHooks: defaultAiHooksConfig(),
  opencode: defaultOpenCodeModelConfig(),
  notifications: defaultOsNotificationConfig(),
  acpModeToggleShortcut: defaultAcpModeToggleShortcut(),
  acpStartupMode: AcpStartupMode.Plan,
});

export enum AiCliTool {
  OpenCode = 'opencode',
  Codex = 'codex',
  Droid = 'droid',
  Claude = 'claude',
}

export enum AiCliStatus {
  Inactive = 'inactive',
  Running = 'running',
  Attention = 'attention',
}

export enum AiCliAttentionKind {
  Permission = 'permission',
  TurnComplete = 'turn_complete',
  SessionError = 'session_error',
  UserInputRequested = 'user_input_requested',
  PlanModePrompt = 'plan_mode_prompt',
}

export interface AiHookEvent {
  terminalId: number;
  tool: AiCliTool;
  status: AiCliStatus;
  reason?: string;
  attentionKind?: AiCliAttentionKind;
  rawJson?: string;
  eventKind?: string;
  question?: OpenCodeQuestion;
}

export interface AiHooksManager {
  events: AiHookEvent[];
}

export interface TileGrid {
  rows: number;
  cols: number;
}

export interface TerminalSession {
  id: number;
  projectId: number;
  kind: TerminalKind;
  shell: ShellKind;
  cwd: string;
  cols: number;
  rows: number;
  title: string;
  pendingLineForTitle: string;
  pendingInputForHistory: string;
  recentInputs: string[];
  ptyPid?: number;
  aiTool?: AiCliTool;
  aiStatus: AiCliStatus;
  aiStatusReason?: string;
  opencodeSessionActive: boolean;
  opencodeTransportStatus?: string;
  opencodePromptSubmitSince?: number;
  opencodePendingQuestion?: OpenCodeQuestion;
  opencodeQuestionFocusIndex: number;
  opencodeQuestionSelectedOptions: string[];
  opencodeQuestionCustomText: string;
  opencodeManualScrollDetached: boolean;
  opencodeLeadingBlankRows: number;
  opencodeThoughtLoopBlocked: boolean;
  opencodeLoopLimitEmitted: boolean;
  opencodeThinkingGuard?: string;
  smartInputState?: SmartInputState;
  terminalOutputFocusOverride: boolean;
}

export interface SmartInputState {
  draftText: string;
  draftAttachments: SmartInputAttachment[];
  queue: SmartInputTask[];
  expanded: boolean;
  userHeight?: number;
  editIndex?: number;
  editText: string;
  editAttachments: SmartInputAttachment[];
  draftUserHeight?: number;
  draftContextMenuSelectionRange?: [number, number];
  editContextMenuSelectionRange?: [number, number];
}

export interface SmartInputAttachment {
  path: string;
  name: string;
}

export interface SmartInputTask {
  text: string;
  attachments: SmartInputAttachment[];
  modeId: string;
  afterDone: boolean;
}

export interface OpenCodeQuestion {
  kind?: 'permission' | 'question';
  header: string;
  question: string;
  options: OpenCodeQuestionOption[];
  multiple: boolean;
  custom: boolean;
  requestId: string;
  sessionId: string;
  questions?: OpenCodeQuestionPrompt[];
}

export interface OpenCodeQuestionOption {
  id: string;
  label: string;
  description?: string;
}

export interface OpenCodeQuestionPrompt {
  header: string;
  question: string;
  options: OpenCodeQuestionOption[];
}

export interface DirectoryNode {
  name: string;
  path: string;
  isDirectory: boolean;
  isDeferred: boolean;
  isSymlink: boolean;
  children?: DirectoryNode[];
  isExpanded: boolean;
  isLoading: boolean;
  error?: string;
}

export interface BrowserTab {
  id: string;
  url: string;
  title?: string;
  kind?: 'page' | 'recording';
}

export const BROWSER_MAX_TABS_PER_SCOPE = 5;

export enum BrowserScopeKeyType {
  Project = 'project',
  Terminal = 'terminal',
}

export interface BrowserScopeKey {
  type: BrowserScopeKeyType;
  projectId: number;
  terminalId?: number;
}

export interface BrowserState {
  tabs: BrowserTab[];
  activeTabId?: string;
  urlDraft: string;
  designInspectEnabled: boolean;
}

export interface SourceControlSnapshot {
  loading: boolean;
  error?: string;
  files: SourceControlFile[];
  worktrees: GitWorktreeInfo[];
  branch?: string;
  ahead?: number;
  behind?: number;
  lastUpdated?: number;
}

export interface SourceControlFile {
  path: string;
  status: string;
  staged: boolean;
}

export interface GitWorktreeInfo {
  path: string;
  branch: string;
  head?: string;
  detached: boolean;
  locked: boolean;
  prunable: boolean;
}

export interface SourceControlStatus {
  files: SourceControlFile[];
  branch: string;
  ahead: number;
  behind: number;
  error?: string;
}

export interface AcpChatSession {
  sessionId?: string;
  status: 'starting' | 'connected' | 'session_created' | 'idle' | 'running' | 'permission' | 'error';
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
}

export interface AcpChatMessage {
  role: 'user' | 'assistant' | 'system';
  text: string;
  timestamp: number;
}

export interface AcpConfigOption {
  id: string;
  name: string;
  category: string;
  currentValue: string;
  options: { label: string; value: string }[];
}

export interface AcpAvailableCommand {
  id: string;
  name: string;
  description?: string;
}

export interface QueuedAcpPrompt {
  text: string;
  attachments: string[];
  modeId: string;
  finalPromptText: string;
}

export interface AcpStandbyEntry {
  chatId: string;
  sessionId?: string;
  status: AcpChatSession['status'];
  projectId: number;
  retryCooldownUntil?: number;
}

export interface FileEditorState {
  open: boolean;
  visible: boolean;
  filePath?: string;
  displayName?: string;
  text: string;
  savedText: string;
  selectionDragActive: boolean;
  scrollOffset: number;
}

export interface OsNotificationPayload {
  terminalId: number;
  tool: AiCliTool;
  kind: AiCliAttentionKind;
  title: string;
  body: string;
}

export interface AppDiagnostics {
  appVersion: string;
  platform: string;
  arch: string;
  electronVersion: string;
  chromeVersion: string;
  nodeVersion: string;
  execPath: string;
  cwd: string;
  configPath: string;
  legacyConfigPath: string;
  historyPath: string;
  hookInboxDir: string;
  hookServicePort: number;
  codexInboxDir: string;
  codexHooksPath: string;
  codexHooksInstalled: boolean;
  codexBridgePath: string;
  codexBridgeInstalled: boolean;
  browserMcpCommand: string[];
  browserMcpSessionCount: number;
}

export interface ClaudeCodexRunPlanRequest {
  terminalId: number;
  projectPath: string;
  originalPrompt: string;
}

export interface IpcChannels {
  // Config
  'config:load': () => Promise<AppConfig>;
  'config:save': (config: AppConfig) => Promise<void>;
  'history:load': () => Promise<AppHistory>;
  'history:save': (history: AppHistory) => Promise<void>;
  'diagnostics:get': () => Promise<AppDiagnostics>;

  // PTY
  'pty:create': (opts: { shell: ShellKind; cwd: string; cols: number; rows: number; env?: Record<string, string>; terminalId?: number; projectId: number; kind: TerminalKind }) => Promise<number>;
  'pty:write': (terminalId: number, data: string) => Promise<void>;
  'pty:resize': (terminalId: number, cols: number, rows: number) => Promise<void>;
  'pty:kill': (terminalId: number, signal?: string) => Promise<void>;
  'pty:getState': (terminalId: number) => Promise<{ pendingLineForTitle: string; pendingInputForHistory: string; recentInputs: string[]; title: string; aiStatus: string; aiStatusReason?: string } | undefined>;
  'pty:data': (terminalId: number, data: string) => void;
  'pty:exit': (terminalId: number, exitCode: number) => void;

  // FS
  'fs:readDir': (path: string) => Promise<{ name: string; isDirectory: boolean; isSymlink: boolean }[]>;
  'fs:readFile': (path: string) => Promise<string>;
  'fs:writeFile': (path: string, text: string) => Promise<void>;
  'fs:exists': (path: string) => Promise<boolean>;
  'fs:stat': (path: string) => Promise<{ isDirectory: boolean; isFile: boolean; size: number; mtimeMs: number }>;

  // Git / Worktree
  'git:diffSummary': (repoPath: string) => Promise<GitDiffSummary>;
  'git:status': (repoPath: string, runFetch?: boolean) => Promise<SourceControlStatus>;
  'git:discoverWorktrees': (repoPath: string) => Promise<GitWorktreeInfo[]>;
  'git:createWorktree': (repoPath: string, branch: string, worktreePath: string, baseBranch?: string) => Promise<boolean>;
  'git:removeWorktree': (repoPath: string, worktreePath: string) => Promise<boolean>;
  'git:copyEnvFiles': (sourcePath: string, targetPath: string) => Promise<boolean>;

  // Hooks
  'hook:status': (event: AiHookEvent) => void;
  'hook:answer': (answer: { requestId: string; answers: string[]; rejected: boolean }) => Promise<void>;
  'claudeCodex:runPlan': (opts: ClaudeCodexRunPlanRequest) => Promise<import('./claudeCodexHook').ClaudeCodexPlanResult>;
  'claudeCodex:runReview': (opts: import('./claudeCodexHook').ClaudeCodexReviewRequest) => Promise<import('./claudeCodexHook').ClaudeCodexReviewResult>;
  'claudeCodex:updateUiVerification': (opts: { planPath: string; note: string }) => Promise<boolean>;

  // ACP
  'acp:spawn': (opts: { projectId: number; cwd: string; mcpServers: string[] }) => Promise<string>;
  'acp:send': (opts: { chatId: string; promptText: string; attachments: string[]; modeId?: string }) => Promise<void>;
  'acp:cancel': (chatId: string) => Promise<void>;
  'acp:setConfigOption': (opts: { chatId: string; configId: string; value: string }) => Promise<void>;
  'acp:permissionResponse': (opts: { chatId: string; requestId: string; answers: string[]; rejected: boolean }) => Promise<boolean>;
  'acp:questionResponse': (opts: { chatId: string; requestId: string; answers: string[][]; rejected: boolean }) => Promise<boolean>;
  'acp:getSession': (chatId: string) => Promise<AcpChatSession | undefined>;
  'acp:queueRunNext': (opts: { chatId: string; index: number }) => Promise<boolean>;
  'acp:queueDelete': (opts: { chatId: string; index: number }) => Promise<boolean>;
  'acp:event': (chatId: string, event: unknown) => void;
  'acp:standby:warm': (projectId: number, cwd: string) => Promise<void>;
  'acp:standby:get': (projectId: number) => Promise<AcpStandbyEntry | undefined>;
  'acp:standby:clear': (projectId: number) => Promise<void>;
  'acp:standby:promote': (projectId: number, visibleChatId: string) => Promise<AcpStandbyEntry | undefined>;
  'acp:standby:clearAll': () => Promise<void>;
  'acp:kill': (chatId: string) => Promise<void>;

  // Browser
  'browser:navigate': (opts: { scope: BrowserScopeKey; url: string }) => Promise<void>;
  'browser:syncBounds': (opts: { scope: BrowserScopeKey; x: number; y: number; width: number; height: number }) => Promise<void>;
  'browser:hide': (scope: BrowserScopeKey) => Promise<void>;
  'browser:show': (scope: BrowserScopeKey) => Promise<void>;
  'browser:urlChanged': (scope: BrowserScopeKey, url: string) => void;
  'browser:tabOpened': (scope: BrowserScopeKey, tab: BrowserTab) => void;
  'browser:tabsChanged': (scope: BrowserScopeKey, state: Pick<BrowserState, 'tabs' | 'activeTabId' | 'urlDraft'>) => void;
  'browser:goBack': (scope: BrowserScopeKey) => Promise<void>;
  'browser:goForward': (scope: BrowserScopeKey) => Promise<void>;
  'browser:reload': (scope: BrowserScopeKey) => Promise<void>;
  'browser:executeJs': (opts: { scope: BrowserScopeKey; script: string }) => Promise<unknown>;
  'browser:screenshot': (opts: { scope: BrowserScopeKey; fullPage: boolean }) => Promise<string>;
  'browser:designInspect': (opts: { scope: BrowserScopeKey; enabled: boolean }) => Promise<void>;
  'browser:designElementClicked': (scope: BrowserScopeKey, elementInfo: string) => void;
  'browser:addTab': (scope: BrowserScopeKey, url?: string) => Promise<string>;
  'browser:closeTab': (opts: { scope: BrowserScopeKey; tabId: string }) => Promise<void>;
  'browser:switchTab': (opts: { scope: BrowserScopeKey; tabId: string }) => Promise<void>;
  'browser:hideAll': () => Promise<void>;
  'browser:showAll': () => Promise<void>;
  'browser:showActive': (scope: BrowserScopeKey) => Promise<void>;
  'browser:destroyInstance': (scope: BrowserScopeKey) => Promise<void>;

  // OpenCode config
  'opencode:generateTerminalConfig': (opts: { cwd: string; model?: string; effort?: string; kimiStrictPermissions?: boolean }) => Promise<string>;
  'opencode:generateRuntimeConfig': (opts: { cwd: string; model?: string; effort?: string; mcpServers?: string[]; kimiStrictPermissions?: boolean }) => Promise<string>;

  // Browser MCP
  'browserMcp:spawn': (opts: { sessionId: string; scope: BrowserScopeKey }) => Promise<string>;
  'browserMcp:execute': (opts: { sessionId: string; method: string; params: unknown }) => Promise<unknown>;
  'browserMcp:kill': (sessionId: string) => Promise<void>;
  'browserMcp:getCommand': () => Promise<string[]>;
  'browserMcp:prepareScope': (terminalId: number, projectId: number) => Promise<BrowserScopeKey>;

  // OS
  'notify:show': (payload: OsNotificationPayload) => void;
  'window:closeRequest': () => void;
  'window:focused': (focused: boolean) => void;
  'window:confirmClose': (confirmed: boolean) => Promise<void>;

  // Dialog
  'dialog:showOpen': (opts: { title?: string; defaultPath?: string; buttonLabel?: string; filters?: { name: string; extensions: string[] }[]; properties?: ('openFile' | 'openDirectory' | 'multiSelections')[] }) => Promise<string[] | undefined>;
  'dialog:showSave': (opts: { title?: string; defaultPath?: string; buttonLabel?: string; filters?: { name: string; extensions: string[] }[] }) => Promise<string | undefined>;

  // Clipboard
  'clipboard:readText': () => Promise<string>;
  'clipboard:readImage': () => Promise<{ path?: string; dataUrl?: string } | undefined>;
  'clipboard:readFilePaths': () => Promise<string[] | undefined>;
  'clipboard:writeText': (text: string) => Promise<void>;

  // Shell / External
  'shell:openExternal': (url: string) => Promise<void>;
  'shell:openPath': (filePath: string) => Promise<string>;
  'shell:showItemInFolder': (filePath: string) => Promise<void>;
}

export interface BrowserMcpRequest {
  id: string;
  method: string;
  params?: unknown;
  authScope?: BrowserMcpAuthScope;
}

export interface BrowserMcpResponse {
  id: string;
  result?: unknown;
  error?: { code: number; message: string };
}

export interface BrowserMcpAuthScope {
  terminalId: number;
  projectId: number;
  sessionId: string;
}

export type IpcInvokeChannel = keyof Pick<IpcChannels, 'config:load' | 'config:save' | 'history:load' | 'history:save' | 'diagnostics:get' | 'pty:create' | 'pty:write' | 'pty:resize' | 'pty:kill' | 'pty:getState' | 'fs:readDir' | 'fs:readFile' | 'fs:writeFile' | 'fs:exists' | 'fs:stat' | 'git:diffSummary' | 'git:status' | 'git:discoverWorktrees' | 'git:createWorktree' | 'git:removeWorktree' | 'git:copyEnvFiles' | 'acp:spawn' | 'acp:send' | 'acp:cancel' | 'acp:setConfigOption' | 'acp:permissionResponse' | 'acp:questionResponse' | 'acp:getSession' | 'acp:queueRunNext' | 'acp:queueDelete' | 'acp:kill' | 'acp:standby:warm' | 'acp:standby:get' | 'acp:standby:clear' | 'acp:standby:promote' | 'acp:standby:clearAll' | 'hook:answer' | 'claudeCodex:runPlan' | 'claudeCodex:runReview' | 'claudeCodex:updateUiVerification' | 'browser:navigate' | 'browser:syncBounds' | 'browser:hide' | 'browser:show' | 'browser:hideAll' | 'browser:showAll' | 'browser:showActive' | 'browser:destroyInstance' | 'browser:goBack' | 'browser:goForward' | 'browser:reload' | 'browser:executeJs' | 'browser:screenshot' | 'browser:designInspect' | 'browser:addTab' | 'browser:closeTab' | 'browser:switchTab' | 'browserMcp:spawn' | 'browserMcp:execute' | 'browserMcp:kill' | 'browserMcp:getCommand' | 'browserMcp:prepareScope' | 'opencode:generateTerminalConfig' | 'opencode:generateRuntimeConfig' | 'dialog:showOpen' | 'dialog:showSave' | 'clipboard:readText' | 'clipboard:readImage' | 'clipboard:readFilePaths' | 'clipboard:writeText' | 'shell:openExternal' | 'shell:openPath' | 'shell:showItemInFolder' | 'notify:show' | 'window:confirmClose'>;

export type IpcSendChannel = keyof Pick<IpcChannels, 'pty:data' | 'pty:exit' | 'hook:status' | 'acp:event' | 'browser:urlChanged' | 'browser:tabOpened' | 'browser:tabsChanged' | 'browser:designElementClicked' | 'window:closeRequest' | 'window:focused'>;

export interface WindowState {
  width: number;
  height: number;
  x?: number;
  y?: number;
  maximized: boolean;
}

export const ANTHROPIC_ENV_VARS_TO_REMOVE = [
  'ANTHROPIC_AUTH_TOKEN',
  'ANTHROPIC_API_KEY',
  'ANTHROPIC_BASE_URL',
  'ANTHROPIC_MODEL',
  'ANTHROPIC_SMALL_FAST_MODEL',
  'ANTHROPIC_DEFAULT_SONNET_MODEL',
  'ANTHROPIC_DEFAULT_HAIKU_MODEL',
  'ANTHROPIC_DEFAULT_OPUS_MODEL',
  'CLAUDE_CODE_SUBAGENT_MODEL',
];
