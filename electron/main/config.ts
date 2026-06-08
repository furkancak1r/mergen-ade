import path from 'path';
import fs from 'fs';
import os from 'os';
import { parse as parseToml } from '@iarna/toml';
import type { AppConfig, AppHistory, ProjectRecord, TerminalShortcutEntry, LauncherEntry } from '../shared/types';
import { BuiltinLauncherKind, LauncherIconKey, ShellKind, AcpStartupMode, defaultAppConfig, defaultTerminalShortcuts, defaultLaunchers, defaultOpenCodeModelConfig, APP_CONFIG_VERSION, DEFAULT_OPENCODE_BUILD_MODEL, DEFAULT_OPENCODE_PLAN_MODEL, DEFAULT_OPENCODE_PLAN_EFFORT, ensureConfiguredModelsAreFavorites } from '../shared/types';

const QUALIFIER = 'com';
const ORGANIZATION = 'Mergen';
const APPLICATION = 'MergenADE';

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
  const dir = path.join(base, sub);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function browserUserDataDir(projectId: number): string {
  const { dataDir } = getProjectDirs();
  const dir = path.join(dataDir, 'webview2', 'projects', String(projectId));
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function browserRecordingsDir(projectId: number): string {
  const { dataDir } = getProjectDirs();
  const dir = path.join(dataDir, 'browser-recordings', 'projects', String(projectId));
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

export function loadConfigWithStatus(): { config: AppConfig; repaired: boolean } {
  const legacyPath = legacyConfigPath();
  const jsonPath = configPath();
  if (fs.existsSync(legacyPath)) {
    try {
      const text = fs.readFileSync(legacyPath, 'utf-8');
      const parsed = parseToml(text) as unknown as AppConfig;
      const config = normalizeConfig(parsed);
      const repaired = true;
      return { config, repaired };
    } catch {
      // fall through to JSON
    }
  }
  if (fs.existsSync(jsonPath)) {
    try {
      const text = fs.readFileSync(jsonPath, 'utf-8');
      const parsed = JSON.parse(text) as AppConfig;
      const config = normalizeConfig(parsed);
      return { config, repaired: false };
    } catch {
      return { config: defaultAppConfig(), repaired: false };
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
    if (config.acpStartupMode === AcpStartupMode.Build) {
      config.acpStartupMode = AcpStartupMode.Plan;
      changed = true;
    }
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
  if (config.opencode.buildModelSlotA === 'fireworks-ai/accounts/fireworks/routers/kimi-k2p5-turbo') {
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
  if (ensureConfiguredModelsAreFavorites(config.opencode)) {
    changed = true;
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

  return config;
}

export function normalizeWindowsVerbatimPath(p: string): string {
  if (process.platform !== 'win32') return p;
  if (p.startsWith('\\\\?\\')) return p.slice(4);
  if (p.startsWith('\\\\?\\UNC\\')) return '\\\\' + p.slice(8);
  return p;
}

function normalizeLauncherEntries(entries: LauncherEntry[]): LauncherEntry[] {
  const normalized: LauncherEntry[] = [];
  for (const builtin of [BuiltinLauncherKind.OpenCode, BuiltinLauncherKind.Codex, BuiltinLauncherKind.Droid, BuiltinLauncherKind.Claude]) {
    const existing = entries.find((e) => e.builtin === builtin || e.id === builtin);
    if (existing) {
      normalized.push({
        id: builtin,
        builtin,
        displayName: existing.displayName?.trim() || builtin.charAt(0).toUpperCase() + builtin.slice(1),
        launchCommand: existing.launchCommand?.trim() || builtin,
        enabled: existing.enabled,
        iconKey: (() => {
          switch (builtin) {
            case BuiltinLauncherKind.Codex: return LauncherIconKey.Codex;
            case BuiltinLauncherKind.Claude: return LauncherIconKey.Claude;
            case BuiltinLauncherKind.Droid: return LauncherIconKey.Droid;
            case BuiltinLauncherKind.OpenCode: return LauncherIconKey.OpenCode;
          }
        })(),
      });
    } else {
      normalized.push({
        id: builtin,
        builtin,
        displayName: builtin.charAt(0).toUpperCase() + builtin.slice(1),
        launchCommand: builtin,
        enabled: true,
        iconKey: (() => {
          switch (builtin) {
            case BuiltinLauncherKind.Codex: return LauncherIconKey.Codex;
            case BuiltinLauncherKind.Claude: return LauncherIconKey.Claude;
            case BuiltinLauncherKind.Droid: return LauncherIconKey.Droid;
            case BuiltinLauncherKind.OpenCode: return LauncherIconKey.OpenCode;
          }
        })(),
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

