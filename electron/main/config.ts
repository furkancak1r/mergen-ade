import path from 'path';
import fs from 'fs';
import os from 'os';
import { parse as parseToml } from '@iarna/toml';
import type { AppConfig, AppHistory, ProjectRecord, TerminalShortcutEntry, LauncherEntry } from '../shared/types';
import { BuiltinLauncherKind, BuiltinLauncherKindAll, BuiltinLauncherKindDefaultDisplayName, LauncherIconKey, ShellKind, defaultAppConfig, defaultTerminalShortcuts, defaultLaunchers, defaultOpenCodeModelConfig, defaultOsNotificationConfig, APP_CONFIG_VERSION, DEFAULT_OPENCODE_BUILD_MODEL, DEFAULT_OPENCODE_PLAN_MODEL, DEFAULT_OPENCODE_PLAN_EFFORT, normalizeBuiltinLaunchCommand } from '../shared/types';

const QUALIFIER = 'com';
const ORGANIZATION = 'Mergen';
const APPLICATION = 'MergenADE';
const CODEX_PROJECT_ENV_KEYS = ['CODEX_WORKSPACE_ROOT', 'CODEX_PROJECT_PATH', 'CODEX_PROJECT'] as const;
const CODEX_MESSAGES_JSON_ENV = 'CODEX_SAVED_MESSAGES_JSON';
const CODEX_MESSAGE_ENV = 'CODEX_MESSAGE';

function getProjectDirs() {
  const appData = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
  const base = path.join(appData, ORGANIZATION, APPLICATION);
  const configDir = path.join(base, 'config');
  const dataDir = path.join(base, 'data');
  return { configDir, dataDir, base };
}

export function configPath(): string {
  const { configDir } = getProjectDirs();
  fs.mkdirSync(configDir, { recursive: true });
  return path.join(configDir, 'config.json');
}

export function legacyConfigPath(): string {
  const { configDir } = getProjectDirs();
  return path.join(configDir, 'config.toml');
}

export function historyPath(): string {
  const { dataDir } = getProjectDirs();
  fs.mkdirSync(dataDir, { recursive: true });
  return path.join(dataDir, 'history.json');
}

export function runtimeDir(sub: string): string {
  const { base } = getProjectDirs();
  const safeSub = String(sub).replace(/[^a-zA-Z0-9_\-/\\]/g, '');
  const dir = path.join(base, safeSub);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function browserUserDataDir(projectId: number): string {
  const { dataDir } = getProjectDirs();
  const safeId = Math.abs(Math.floor(Number(projectId)));
  if (!Number.isFinite(safeId)) {
    throw new Error('Invalid projectId for browser user data dir');
  }
  const dir = [dataDir, 'webview2', 'projects', String(safeId)].join(path.sep);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function browserRecordingsDir(projectId: number): string {
  const { dataDir } = getProjectDirs();
  const safeId = Math.abs(Math.floor(Number(projectId)));
  if (!Number.isFinite(safeId)) {
    throw new Error('Invalid projectId for browser recordings dir');
  }
  const dir = [dataDir, 'browser-recordings', 'projects', String(safeId)].join(path.sep);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function codexBridgePath(): string {
  const { dataDir } = getProjectDirs();
  const dir = path.join(dataDir, 'bin');
  fs.mkdirSync(dir, { recursive: true });
  return path.join(dir, 'mergen-codex-bridge.exe');
}

export function loadConfig(): AppConfig {
  const { config, repaired } = loadConfigWithStatus();
  return config;
}

function normalizeLegacyProject(raw: Record<string, unknown>): ProjectRecord {
  return {
    id: typeof raw.id === 'number' ? raw.id : 0,
    name: typeof raw.name === 'string' ? raw.name : '',
    path: typeof raw.path === 'string' ? raw.path : '',
    savedMessages: Array.isArray(raw.savedMessages) ? raw.savedMessages
      : Array.isArray(raw.saved_messages) ? raw.saved_messages as unknown as string[]
      : [],
    aiConfig: (raw.aiConfig ?? raw.ai_config ?? { hooksEnabled: false, toolOverrides: {} }) as ProjectRecord['aiConfig'],
    checklist: Array.isArray(raw.checklist) ? raw.checklist : [],
    browserLastUrl: (raw.browserLastUrl ?? raw.browser_last_url) as string | undefined,
    isWorktree: Boolean(raw.isWorktree ?? raw.is_worktree),
  };
}

function mergeLegacyTomlProjects(jsonConfig: AppConfig): { config: AppConfig; merged: boolean } {
  const legacyPath = legacyConfigPath();
  if (!fs.existsSync(legacyPath)) return { config: jsonConfig, merged: false };
  try {
    const text = fs.readFileSync(legacyPath, 'utf-8');
    const legacy = parseToml(text) as unknown as { projects?: Record<string, unknown>[] };
    const rawLegacyProjects: Record<string, unknown>[] = legacy.projects ?? [];
    if (rawLegacyProjects.length === 0) return { config: jsonConfig, merged: false };
    const legacyProjects: ProjectRecord[] = rawLegacyProjects.map(normalizeLegacyProject);

    const normalize = (p: string) => normalizeWindowsVerbatimPath(p).replace(/[\\/]+$/, '').toLowerCase();
    const existingPaths = new Set(jsonConfig.projects.map((p) => normalize(p.path)));
    let maxId = jsonConfig.projects.reduce((max, p) => Math.max(max, p.id), 0);
    let merged = false;

    for (const legacyProject of legacyProjects) {
      const legacyPathNorm = normalize(legacyProject.path);
      const existing = jsonConfig.projects.find((p) => normalize(p.path) === legacyPathNorm);
      if (existing) {
        // Recover saved messages from legacy if current project has none
        if ((!existing.savedMessages || existing.savedMessages.length === 0)
          && legacyProject.savedMessages && legacyProject.savedMessages.length > 0) {
          existing.savedMessages = legacyProject.savedMessages;
          merged = true;
        }
        if (!existing.checklist || existing.checklist.length === 0) {
          if (legacyProject.checklist && legacyProject.checklist.length > 0) {
            existing.checklist = legacyProject.checklist;
            merged = true;
          }
        }
        if (!existing.browserLastUrl && legacyProject.browserLastUrl) {
          existing.browserLastUrl = legacyProject.browserLastUrl;
          merged = true;
        }
      } else if (existingPaths.has(legacyPathNorm)) {
        // Duplicate path, skip
      } else {
        // Add missing legacy project
        maxId++;
        jsonConfig.projects.push({
          id: maxId,
          name: legacyProject.name,
          path: legacyProject.path,
          savedMessages: legacyProject.savedMessages ?? [],
          aiConfig: legacyProject.aiConfig ?? { hooksEnabled: false, toolOverrides: {} },
          checklist: legacyProject.checklist ?? [],
          browserLastUrl: legacyProject.browserLastUrl,
          isWorktree: legacyProject.isWorktree ?? false,
        });
        existingPaths.add(legacyPathNorm);
        merged = true;
      }
    }
    return { config: jsonConfig, merged };
  } catch {
    return { config: jsonConfig, merged: false };
  }
}

function applyCodexEnvSavedMessages(config: AppConfig, env: NodeJS.ProcessEnv = process.env): boolean {
  const messages = codexEnvSavedMessages(env);
  if (messages.length === 0) return false;

  const projects = config.projects ?? [];
  const targetProject = findCodexEnvProjectFromEnv(projects, env);
  if (!targetProject) return false;

  const ownerId = savedMessageOwnerProjectId(projects, targetProject);
  const family = projects.filter((project) => savedMessageOwnerProjectId(projects, project) === ownerId);
  if (family.length === 0) return false;

  const mergedMessages = mergeSavedMessageFamily(family, messages);
  let changed = false;
  for (const project of family) {
    if (!stringArraysEqual(project.savedMessages ?? [], mergedMessages)) {
      project.savedMessages = mergedMessages;
      changed = true;
    }
  }
  return changed;
}

function findCodexEnvProjectFromEnv(projects: ProjectRecord[], env: NodeJS.ProcessEnv): ProjectRecord | undefined {
  for (const key of CODEX_PROJECT_ENV_KEYS) {
    const value = env[key]?.trim();
    if (!value) continue;
    const project = findCodexEnvProject(projects, value, key === 'CODEX_PROJECT');
    if (project) return project;
  }
  return undefined;
}

function codexEnvSavedMessages(env: NodeJS.ProcessEnv): string[] {
  const values: string[] = [];
  const json = env[CODEX_MESSAGES_JSON_ENV]?.trim();
  if (json) {
    try {
      const parsed = JSON.parse(json) as unknown;
      if (Array.isArray(parsed)) {
        for (const value of parsed) {
          if (typeof value === 'string') values.push(value);
        }
      }
    } catch {
      // Invalid Codex env should never block app startup.
    }
  }
  const single = env[CODEX_MESSAGE_ENV];
  if (single) values.push(single);
  return normalizedUniqueMessages(values);
}

function normalizedUniqueMessages(messages: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const message of messages) {
    const trimmed = message.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    seen.add(trimmed);
    result.push(trimmed);
  }
  return result;
}

function findCodexEnvProject(projects: ProjectRecord[], selector: string, allowNameMatch: boolean): ProjectRecord | undefined {
  const selectorPath = normalizeProjectPathForMatch(selector);
  const pathMatch = projects.find((project) => normalizeProjectPathForMatch(project.path) === selectorPath);
  if (pathMatch) return pathMatch;

  if (!allowNameMatch) return undefined;
  const nameMatches = projects.filter((project) => project.name === selector);
  return nameMatches.length === 1 ? nameMatches[0] : undefined;
}

function savedMessageOwnerProjectId(projects: ProjectRecord[], project: ProjectRecord): number {
  if (!project.isWorktree || !project.repoRoot) return project.id;
  const repoRoot = normalizeProjectPathForMatch(project.repoRoot);
  const root = projects.find((candidate) => !candidate.isWorktree && normalizeProjectPathForMatch(candidate.path) === repoRoot);
  return root?.id ?? project.id;
}

function mergeSavedMessageFamily(projects: ProjectRecord[], envMessages: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  const add = (message: string) => {
    const trimmed = message.trim();
    if (!trimmed || seen.has(trimmed)) return;
    seen.add(trimmed);
    result.push(trimmed);
  };
  for (const project of projects) {
    for (const message of project.savedMessages ?? []) add(message);
  }
  for (const message of envMessages) add(message);
  return result;
}

function normalizeProjectPathForMatch(p: string): string {
  return normalizeWindowsVerbatimPath(p.trim()).replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
}

function stringArraysEqual(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((value, index) => value === b[index]);
}

export function loadConfigWithStatus(): { config: AppConfig; repaired: boolean } {
  const legacyPath = legacyConfigPath();
  const jsonPath = configPath();
  if (fs.existsSync(jsonPath)) {
    try {
      const text = fs.readFileSync(jsonPath, 'utf-8');
      const parsed = JSON.parse(text) as AppConfig;
      let config = normalizeConfig(parsed);
      // Merge legacy TOML projects and saved messages into JSON config
      const { config: merged, merged: didMerge } = mergeLegacyTomlProjects(config);
      config = merged;
      const didImportCodexEnv = applyCodexEnvSavedMessages(config);
      if (didMerge || didImportCodexEnv) {
        saveConfig(config);
      }
      return { config, repaired: didMerge || didImportCodexEnv };
    } catch {
      return { config: defaultAppConfig(), repaired: false };
    }
  }
  // No JSON yet — try legacy TOML as the sole source
  if (fs.existsSync(legacyPath)) {
    try {
      const text = fs.readFileSync(legacyPath, 'utf-8');
      const parsed = parseToml(text) as unknown as AppConfig;
      const config = normalizeConfig(parsed);
      applyCodexEnvSavedMessages(config);
      saveConfig(config);
      return { config, repaired: true };
    } catch {
      // fall through
    }
  }
  return { config: defaultAppConfig(), repaired: false };
}

export function saveConfig(config: AppConfig): void {
  const p = configPath();
  const dir = path.dirname(p);
  fs.mkdirSync(dir, { recursive: true });
  const tmp = path.join(dir, `config.json.tmp-${process.pid}-${Date.now()}`);
  fs.writeFileSync(tmp, JSON.stringify(config, null, 2), 'utf-8');
  try {
    fs.renameSync(tmp, p);
  } catch {
    fs.copyFileSync(tmp, p);
    try { fs.unlinkSync(tmp); } catch {}
  }
}

export function loadHistory(): AppHistory {
  const p = historyPath();
  if (!fs.existsSync(p)) {
    return { version: 1, projects: {} };
  }
  try {
    const text = fs.readFileSync(p, 'utf-8');
    const history = JSON.parse(text) as AppHistory;
    for (const key of Object.keys(history.projects)) {
      const h = history.projects[key];
      if (h.maxEntries === 0) {
        h.maxEntries = 500;
      }
    }
    return history;
  } catch {
    return { version: 1, projects: {} };
  }
}

export function saveHistory(history: AppHistory): void {
  const p = historyPath();
  const dir = path.dirname(p);
  fs.mkdirSync(dir, { recursive: true });
  const tmp = path.join(dir, `history.json.tmp-${process.pid}-${Date.now()}`);
  fs.writeFileSync(tmp, JSON.stringify(history, null, 2), 'utf-8');
  try {
    fs.renameSync(tmp, p);
  } catch {
    fs.copyFileSync(tmp, p);
    try { fs.unlinkSync(tmp); } catch {}
  }
}

function normalizeConfig(config: AppConfig): AppConfig {
  let changed = false;
  const loadedVersion = config.version ?? 0;

  if (loadedVersion < 2) {
    if (!config.opencode?.planModel?.trim()) {
      const slotB = config.opencode?.buildModelSlotB?.trim() ?? '';
      if (!config.opencode) config.opencode = defaultOpenCodeModelConfig();
      config.opencode.planModel = slotB || DEFAULT_OPENCODE_PLAN_MODEL;
      changed = true;
    }
  }

  if (config.version !== APP_CONFIG_VERSION) {
    config.version = APP_CONFIG_VERSION;
    changed = true;
  }

  const legacyClaudeCodexHook = (config as unknown as { claude_code_codex_hook_enabled?: unknown }).claude_code_codex_hook_enabled;
  if (typeof config.claudeCodeCodexHookEnabled !== 'boolean') {
    config.claudeCodeCodexHookEnabled = typeof legacyClaudeCodexHook === 'boolean'
      ? legacyClaudeCodexHook
      : true;
    changed = true;
  }
  if ('claude_code_codex_hook_enabled' in (config as unknown as Record<string, unknown>)) {
    delete (config as unknown as Record<string, unknown>).claude_code_codex_hook_enabled;
    changed = true;
  }

  // Normalize shell for current platform
  const supported = process.platform === 'win32' ? ['powershell', 'cmd'] : ['zsh'];
  const shellVal = config.defaultShell;
  if (!supported.includes(shellVal)) {
    config.defaultShell = process.platform === 'win32' ? ShellKind.PowerShell : ShellKind.Zsh;
    changed = true;
  }

  // Normalize launchers
  config.launchers = normalizeLauncherEntries(config.launchers ?? []);

  // Normalize shortcuts
  config.terminalShortcuts = normalizeTerminalShortcutEntries(config.terminalShortcuts ?? []);

  // OpenCode model migration
  if (!config.opencode) {
    config.opencode = defaultOpenCodeModelConfig();
    changed = true;
  }
  if (
    config.opencode.buildModelSlotA === 'fireworks-ai/accounts/fireworks/routers/kimi-k2p5-turbo'
    || config.opencode.buildModelSlotA === 'fireworks-ai/accounts/fireworks/routers/kimi-k2p6-turbo'
  ) {
    config.opencode.buildModelSlotA = DEFAULT_OPENCODE_BUILD_MODEL;
    changed = true;
  }
  if (!config.opencode.planModel?.trim()) {
    config.opencode.planModel = DEFAULT_OPENCODE_PLAN_MODEL;
    changed = true;
  }
  if (!config.opencode.planEffort?.trim()) {
    config.opencode.planEffort = DEFAULT_OPENCODE_PLAN_EFFORT;
    changed = true;
  }
  const legacyConfig = config as unknown as Record<string, unknown>;
  for (const key of ['acpModeToggleShortcut', 'acpStartupMode']) {
    if (key in legacyConfig) {
      delete legacyConfig[key];
      changed = true;
    }
  }
  const legacyOpenCode = config.opencode as unknown as Record<string, unknown>;
  for (const key of ['acpFavoriteModels', 'acpKnownModels', 'acpBindModelToMode', 'acpAutoApprovePermissions']) {
    if (key in legacyOpenCode) {
      delete legacyOpenCode[key];
      changed = true;
    }
  }

  // Strip Windows verbatim path prefixes
  for (const project of config.projects ?? []) {
    if (project.path) {
      project.path = normalizeWindowsVerbatimPath(project.path);
    }
    if (project.repoRoot) {
      project.repoRoot = normalizeWindowsVerbatimPath(project.repoRoot);
    }
  }

  // Mojibake repair for project names and paths
  for (const project of config.projects ?? []) {
    const repairedName = repairMojibake(project.name);
    if (repairedName !== project.name) {
      project.name = repairedName;
      changed = true;
    }
    const repairedPath = repairMojibake(project.path);
    if (repairedPath !== project.path) {
      project.path = repairedPath;
      changed = true;
    }
  }

  // Sanitize legacy browser_panel_expanded (always false)
  if (config.ui && config.ui.browserPanelExpanded !== false) {
    config.ui.browserPanelExpanded = false;
    changed = true;
  }

  // Ensure notifications config exists
  if (!config.notifications) {
    config.notifications = defaultOsNotificationConfig();
    changed = true;
  }

  // Remove legacy foregroundSavedMessages from all projects
  for (const project of config.projects ?? []) {
    const rec = project as unknown as Record<string, unknown>;
    if ('foregroundSavedMessages' in rec) {
      delete rec.foregroundSavedMessages;
      changed = true;
    }
  }

  return config;
}

function repairMojibake(text: string): string {
  // If text contains any code points outside CP1252 range (0-255), it's already UTF-8
  let hasNonCp1252 = false;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) > 255) {
      hasNonCp1252 = true;
      break;
    }
  }
  if (hasNonCp1252) return text;

  let current = text;
  for (let round = 0; round < 5; round++) {
    const bytes = new Uint8Array(current.length);
    for (let i = 0; i < current.length; i++) {
      bytes[i] = current.charCodeAt(i) < 256 ? current.charCodeAt(i) : 0x3F;
    }
    try {
      const repaired = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
      if (repaired === current) break;
      current = repaired;
    } catch {
      break;
    }
  }
  return current;
}

export async function repairMojibakePath(path: string, existsFn: (p: string) => Promise<boolean>): Promise<string> {
  const repaired = repairMojibake(path);
  if (repaired === path) return path;
  const ok = await existsFn(repaired);
  return ok ? repaired : path;
}

export function normalizeWindowsVerbatimPath(p: string): string {
  if (process.platform !== 'win32') return p;
  if (p.startsWith('\\\\?\\UNC\\')) return '\\\\' + p.slice(8);
  if (p.startsWith('\\\\?\\')) return p.slice(4);
  return p;
}

function normalizeLauncherEntries(entries: LauncherEntry[]): LauncherEntry[] {
  const normalized: LauncherEntry[] = [];
  for (const builtin of BuiltinLauncherKindAll) {
    const existing = entries.find((e) => e.builtin === builtin || e.id === builtin);
    if (existing) {
      normalized.push({
        id: builtin,
        builtin,
        displayName: existing.displayName?.trim() || BuiltinLauncherKindDefaultDisplayName(builtin),
        launchCommand: normalizeBuiltinLaunchCommand(builtin, existing.launchCommand ?? ''),
        enabled: existing.enabled,
        iconKey: (() => {
          switch (builtin) {
            case BuiltinLauncherKind.Codex: return LauncherIconKey.Codex;
            case BuiltinLauncherKind.Claude: return LauncherIconKey.Claude;
            case BuiltinLauncherKind.Droid: return LauncherIconKey.Droid;
            case BuiltinLauncherKind.OpenCode: return LauncherIconKey.OpenCode;
          }
        })(),
        bypassPermissions: existing.bypassPermissions,
      });
    } else {
      normalized.push({
        id: builtin,
        builtin,
        displayName: BuiltinLauncherKindDefaultDisplayName(builtin),
        launchCommand: normalizeBuiltinLaunchCommand(builtin, ''),
        enabled: true,
        iconKey: (() => {
          switch (builtin) {
            case BuiltinLauncherKind.Codex: return LauncherIconKey.Codex;
            case BuiltinLauncherKind.Claude: return LauncherIconKey.Claude;
            case BuiltinLauncherKind.Droid: return LauncherIconKey.Droid;
            case BuiltinLauncherKind.OpenCode: return LauncherIconKey.OpenCode;
          }
        })(),
        bypassPermissions: builtin === BuiltinLauncherKind.Claude,
      });
    }
  }
  for (const [index, entry] of entries.entries()) {
    if (entry.builtin) continue;
    if (!entry.displayName?.trim() || !entry.launchCommand?.trim()) continue;
    const id = entry.id?.trim() || `custom-${index + 1}`;
    normalized.push({
      id,
      builtin: undefined,
      displayName: entry.displayName.trim(),
      launchCommand: entry.launchCommand.trim(),
      enabled: entry.enabled,
      iconKey: entry.iconKey || LauncherIconKey.Rocket,
      bypassPermissions: entry.bypassPermissions,
    });
  }
  return normalized;
}

function normalizeTerminalShortcutEntries(entries: TerminalShortcutEntry[]): TerminalShortcutEntry[] {
  const defaults = defaultTerminalShortcuts();
  for (const entry of entries) {
    if (entry.id === 'semgrep-check') {
      entry.id = 'github-push';
      if (entry.label === 'Semgrep Check') entry.label = 'GitHub Push';
    }
    if (entry.id === 'implement-plan' && entry.key === 'F7') {
      entry.key = 'F11';
    }
    if (entry.id === 'review-guard' && entry.key === 'F8') {
      entry.key = 'F7';
    }
  }
  const normalized: TerminalShortcutEntry[] = [];
  for (const def of defaults) {
    const existing = entries.find((e) => e.id === def.id);
    if (existing) {
      const mod = existing.modifiers || { ctrl: false, alt: false, shift: false, command: false };
      if (process.platform !== 'darwin' && mod.ctrl && mod.command) {
        mod.command = false;
      }
      normalized.push({
        id: def.id,
        label: existing.label?.trim() || def.label,
        key: existing.key?.trim() || def.key,
        modifiers: mod,
        command: existing.command?.trim() || def.command,
        enabled: existing.enabled,
      });
    } else {
      normalized.push({ ...def });
    }
  }
  for (const [index, entry] of entries.entries()) {
    if (defaults.some((d) => d.id === entry.id)) continue;
    const id = entry.id?.trim() || `custom-${index + 1}`;
    const mod = entry.modifiers || { ctrl: false, alt: false, shift: false, command: false };
    if (process.platform !== 'darwin' && mod.ctrl && mod.command) {
      mod.command = false;
    }
    normalized.push({
      id,
      label: entry.label || '',
      key: entry.key || '',
      modifiers: mod,
      command: entry.command || '',
      enabled: entry.enabled,
    });
  }
  return normalized;
}
