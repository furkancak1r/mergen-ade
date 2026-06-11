import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BuiltinLauncherKind, BuiltinLauncherKindDefaultLaunchCommand, DEFAULT_OPENCODE_BUILD_MODEL, defaultAppConfig } from '../shared/types';
import { configPath, loadConfig } from './config';

const originalAppData = process.env.APPDATA;
let tempDir: string | undefined;

function writeConfigJson(config: Record<string, unknown>) {
  const p = configPath();
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, JSON.stringify(config, null, 2), 'utf-8');
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
});
