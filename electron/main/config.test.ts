import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BuiltinLauncherKind, BuiltinLauncherKindDefaultLaunchCommand, DEFAULT_OPENCODE_BUILD_MODEL, defaultAppConfig, type ProjectRecord } from '../shared/types';
import { configPath, legacyConfigPath, loadConfig } from './config';

const originalAppData = process.env.APPDATA;
const CODEX_ENV_KEYS = ['CODEX_WORKSPACE_ROOT', 'CODEX_PROJECT_PATH', 'CODEX_PROJECT', 'CODEX_SAVED_MESSAGES_JSON', 'CODEX_MESSAGE'] as const;
const originalCodexEnv = new Map<string, string | undefined>(CODEX_ENV_KEYS.map((key) => [key, process.env[key]]));
let tempDir: string | undefined;

function writeConfigJson(config: Record<string, unknown>) {
  const p = configPath();
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, JSON.stringify(config, null, 2), 'utf-8');
}

function writeConfigToml(content: string) {
  const p = legacyConfigPath();
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, content, 'utf-8');
}

const project = (partial: Partial<ProjectRecord>): ProjectRecord => ({
  id: partial.id ?? 1,
  name: partial.name ?? 'Project',
  path: partial.path ?? 'C:\\repo',
  savedMessages: partial.savedMessages ?? [],
  aiConfig: partial.aiConfig ?? {},
  checklist: partial.checklist ?? [],
  browserLastUrl: partial.browserLastUrl,
  repoRoot: partial.repoRoot,
  isWorktree: partial.isWorktree ?? false,
});

function clearCodexEnv() {
  for (const key of CODEX_ENV_KEYS) delete process.env[key];
}

function restoreCodexEnv() {
  for (const key of CODEX_ENV_KEYS) {
    const original = originalCodexEnv.get(key);
    if (original === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = original;
    }
  }
}

describe('config normalization', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-electron-config-'));
    process.env.APPDATA = tempDir;
    clearCodexEnv();
  });

  afterEach(() => {
    restoreCodexEnv();
    if (originalAppData === undefined) {
      delete process.env.APPDATA;
    } else {
      process.env.APPDATA = originalAppData;
    }
    if (tempDir) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
    tempDir = undefined;
  });

  it('enables the Claude Code Codex hook by default like Rust', () => {
    expect(defaultAppConfig().claudeCodeCodexHookEnabled).toBe(true);
  });

  it('restores the Claude Code Codex hook setting when old JSON config is missing it', () => {
    const config = defaultAppConfig() as unknown as Record<string, unknown>;
    delete config.claudeCodeCodexHookEnabled;
    writeConfigJson(config);

    expect(loadConfig().claudeCodeCodexHookEnabled).toBe(true);
  });

  it('preserves the legacy snake_case Claude Code Codex hook value', () => {
    const config = defaultAppConfig() as unknown as Record<string, unknown>;
    delete config.claudeCodeCodexHookEnabled;
    config.claude_code_codex_hook_enabled = false;
    writeConfigJson(config);

    expect(loadConfig().claudeCodeCodexHookEnabled).toBe(false);
  });

  it('migrates the legacy Claude cc launcher to the real Claude CLI command', () => {
    const config = defaultAppConfig();
    const claude = config.launchers.find((entry) => entry.builtin === BuiltinLauncherKind.Claude);
    expect(claude).toBeDefined();
    claude!.launchCommand = 'cc';
    writeConfigJson(config as unknown as Record<string, unknown>);

    const loadedClaude = loadConfig().launchers.find((entry) => entry.builtin === BuiltinLauncherKind.Claude);
    expect(loadedClaude?.launchCommand).toBe(BuiltinLauncherKindDefaultLaunchCommand(BuiltinLauncherKind.Claude));
  });

  it('removes ACP launchers from older launcher configs', () => {
    const config = defaultAppConfig();
    for (const id of ['opencode_acp', 'codex_acp', 'claude_acp']) {
      config.launchers.push({
        id,
        builtin: id as BuiltinLauncherKind,
        displayName: id,
        launchCommand: '',
        enabled: true,
        iconKey: config.launchers[0].iconKey,
      });
    }
    const legacy = config as unknown as Record<string, unknown>;
    legacy.acpModeToggleShortcut = { key: 'Tab' };
    legacy.acpStartupMode = 'plan';
    Object.assign(config.opencode as unknown as Record<string, unknown>, {
      acpFavoriteModels: ['legacy'],
      acpKnownModels: [],
      acpBindModelToMode: true,
      acpAutoApprovePermissions: true,
    });
    writeConfigJson(config as unknown as Record<string, unknown>);

    const loaded = loadConfig();
    expect(loaded.launchers.some((entry) => entry.id.endsWith('_acp'))).toBe(false);
    expect(loaded).not.toHaveProperty('acpModeToggleShortcut');
    expect(loaded).not.toHaveProperty('acpStartupMode');
    expect(loaded.opencode).not.toHaveProperty('acpFavoriteModels');
    expect(loaded.opencode).not.toHaveProperty('acpKnownModels');
    expect(loaded.opencode).not.toHaveProperty('acpBindModelToMode');
    expect(loaded.opencode).not.toHaveProperty('acpAutoApprovePermissions');
  });

  it('migrates the old Kimi K2.5 OpenCode build default to Mimo', () => {
    const config = defaultAppConfig();
    config.opencode.buildModelSlotA = 'fireworks-ai/accounts/fireworks/routers/kimi-k2p5-turbo';
    writeConfigJson(config as unknown as Record<string, unknown>);

    expect(loadConfig().opencode.buildModelSlotA).toBe(DEFAULT_OPENCODE_BUILD_MODEL);
  });

  it('migrates the old Kimi K2.6 OpenCode build default to Mimo', () => {
    const config = defaultAppConfig();
    config.opencode.buildModelSlotA = 'fireworks-ai/accounts/fireworks/routers/kimi-k2p6-turbo';
    writeConfigJson(config as unknown as Record<string, unknown>);

    expect(loadConfig().opencode.buildModelSlotA).toBe(DEFAULT_OPENCODE_BUILD_MODEL);
  });

  it('merges saved_messages from legacy TOML (snake_case) into JSON config', () => {
    const config = defaultAppConfig();
    config.projects = [
      { id: 1, name: 'TestProject', path: 'C:\\repo', savedMessages: [], aiConfig: {}, checklist: [], isWorktree: false },
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);

    const toml = `
[[projects]]
id = 1
name = "TestProject"
path = 'C:\\repo'
saved_messages = ["npm run dev", "npm run build"]
checklist = []
foreground_saved_messages = []
is_worktree = false

[projects.ai_config]
hooks_enabled = false
`;
    writeConfigToml(toml);

    const loaded = loadConfig();
    expect(loaded.projects[0].savedMessages).toEqual(['npm run dev', 'npm run build']);
  });

  it('merges browser_last_url from legacy TOML into JSON config', () => {
    const config = defaultAppConfig();
    config.projects = [
      { id: 1, name: 'TestProject', path: 'C:\\repo', savedMessages: [], aiConfig: {}, checklist: [], isWorktree: false },
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);

    const toml = `
[[projects]]
id = 1
name = "TestProject"
path = 'C:\\repo'
saved_messages = []
checklist = []
browser_last_url = "http://localhost:5174"
foreground_saved_messages = []
is_worktree = false

[projects.ai_config]
hooks_enabled = false
`;
    writeConfigToml(toml);

    const loaded = loadConfig();
    expect(loaded.projects[0].browserLastUrl).toBe('http://localhost:5174');
  });

  it('adds missing legacy TOML projects to JSON config with saved messages', () => {
    const config = defaultAppConfig();
    config.projects = [];
    writeConfigJson(config as unknown as Record<string, unknown>);

    const toml = `
[[projects]]
id = 5
name = "LegacyProject"
path = 'C:\\legacy'
saved_messages = ["cargo run"]
checklist = []
foreground_saved_messages = []
is_worktree = false

[projects.ai_config]
hooks_enabled = false
`;
    writeConfigToml(toml);

    const loaded = loadConfig();
    expect(loaded.projects).toHaveLength(1);
    expect(loaded.projects[0].name).toBe('LegacyProject');
    expect(loaded.projects[0].savedMessages).toEqual(['cargo run']);
  });

  it('imports Codex env JSON saved messages by workspace root', () => {
    const config = defaultAppConfig();
    config.projects = [
      project({ id: 1, name: 'Repo', path: 'C:\\repo', savedMessages: ['npm test'] }),
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);
    process.env.CODEX_WORKSPACE_ROOT = 'C:/repo/';
    process.env.CODEX_SAVED_MESSAGES_JSON = JSON.stringify([' npm run dev ', 'npm test', '']);

    const loaded = loadConfig();

    expect(loaded.projects[0].savedMessages).toEqual(['npm test', 'npm run dev']);
    const saved = JSON.parse(fs.readFileSync(configPath(), 'utf-8')) as { projects: ProjectRecord[] };
    expect(saved.projects[0].savedMessages).toEqual(['npm test', 'npm run dev']);
  });

  it('falls back to a matching Codex project selector when an earlier selector is unmatched', () => {
    const config = defaultAppConfig();
    config.projects = [project({ id: 1, name: 'Repo', path: 'C:\\repo' })];
    writeConfigJson(config as unknown as Record<string, unknown>);
    process.env.CODEX_WORKSPACE_ROOT = 'C:\\missing';
    process.env.CODEX_PROJECT_PATH = 'C:\\repo';
    process.env.CODEX_MESSAGE = 'npm start';

    expect(loadConfig().projects[0].savedMessages).toEqual(['npm start']);
  });

  it('imports a single Codex message by unique CODEX_PROJECT name', () => {
    const config = defaultAppConfig();
    config.projects = [
      project({ id: 1, name: 'Named Project', path: 'C:\\named' }),
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);
    process.env.CODEX_PROJECT = 'Named Project';
    process.env.CODEX_MESSAGE = ' npm start ';

    const loaded = loadConfig();

    expect(loaded.projects[0].savedMessages).toEqual(['npm start']);
  });

  it('syncs Codex env messages across the root/worktree saved-message family', () => {
    const config = defaultAppConfig();
    config.projects = [
      project({ id: 1, name: 'Root', path: 'C:\\repo', savedMessages: ['root command'] }),
      project({ id: 2, name: 'Feature', path: 'C:\\worktrees\\feature', repoRoot: 'C:\\repo', isWorktree: true, savedMessages: ['worktree command'] }),
      project({ id: 3, name: 'Other', path: 'C:\\other', savedMessages: ['keep'] }),
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);
    process.env.CODEX_WORKSPACE_ROOT = 'C:\\worktrees\\feature';
    process.env.CODEX_SAVED_MESSAGES_JSON = JSON.stringify(['codex command']);

    const loaded = loadConfig();

    expect(loaded.projects[0].savedMessages).toEqual(['root command', 'worktree command', 'codex command']);
    expect(loaded.projects[1].savedMessages).toEqual(['root command', 'worktree command', 'codex command']);
    expect(loaded.projects[2].savedMessages).toEqual(['keep']);
  });

  it('ignores invalid Codex message JSON without changing matching projects', () => {
    const config = defaultAppConfig();
    config.projects = [
      project({ id: 1, name: 'Repo', path: 'C:\\repo', savedMessages: ['keep'] }),
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);
    process.env.CODEX_WORKSPACE_ROOT = 'C:\\repo';
    process.env.CODEX_SAVED_MESSAGES_JSON = '{not json';

    const loaded = loadConfig();

    expect(loaded.projects[0].savedMessages).toEqual(['keep']);
  });

  it('ignores unmatched and ambiguous Codex project selectors', () => {
    const config = defaultAppConfig();
    config.projects = [
      project({ id: 1, name: 'Duplicate', path: 'C:\\one', savedMessages: ['one'] }),
      project({ id: 2, name: 'Duplicate', path: 'C:\\two', savedMessages: ['two'] }),
    ];
    writeConfigJson(config as unknown as Record<string, unknown>);
    process.env.CODEX_PROJECT = 'Duplicate';
    process.env.CODEX_MESSAGE = 'codex command';

    let loaded = loadConfig();
    expect(loaded.projects[0].savedMessages).toEqual(['one']);
    expect(loaded.projects[1].savedMessages).toEqual(['two']);

    process.env.CODEX_PROJECT = 'Missing';
    loaded = loadConfig();
    expect(loaded.projects[0].savedMessages).toEqual(['one']);
    expect(loaded.projects[1].savedMessages).toEqual(['two']);
  });
});
