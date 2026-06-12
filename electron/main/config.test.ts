import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BuiltinLauncherKind, BuiltinLauncherKindDefaultLaunchCommand, DEFAULT_OPENCODE_BUILD_MODEL, defaultAppConfig } from '../shared/types';
import { configPath, legacyConfigPath, loadConfig } from './config';

const originalAppData = process.env.APPDATA;
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

describe('config normalization', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-electron-config-'));
    process.env.APPDATA = tempDir;
  });

  afterEach(() => {
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
});
