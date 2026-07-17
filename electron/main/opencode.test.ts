import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_OPENCODE_BUILD_MODEL } from '../shared/types';
import { generateOpencodeTerminalConfig, verifyOpencodePonytailPlugin } from './opencode';
import { getBrowserMcpStdioConfig } from './browserMcpCommand';

let tempDir: string | undefined;

describe('OpenCode config generation', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-electron-opencode-'));
  });

  afterEach(() => {
    if (tempDir) {
      fs.rmSync(tempDir, { recursive: true, force: true });
    }
    tempDir = undefined;
  });

  it('writes the Mimo default build model and provider into terminal configs', () => {
    const configPath = generateOpencodeTerminalConfig(tempDir!, { kimiStrictPermissions: false });
    const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));
    const helper = getBrowserMcpStdioConfig();

    expect(config.agent.build.model).toBe(DEFAULT_OPENCODE_BUILD_MODEL);
    expect(config.mode.build.model).toBe(DEFAULT_OPENCODE_BUILD_MODEL);
    expect(config.provider.mimo.models['mimo-v2.5-pro']).toEqual({
      id: 'mimo-v2.5-pro',
      name: 'Mimo v2.5 Pro',
    });
    expect(config.provider.mimo.options).toMatchObject({
      apiKey: '{env:MIMO_API_KEY}',
      baseURL: 'https://token-plan-sgp.xiaomimimo.com/v1',
    });
    expect(config.agent.build.permission['*']).toBe('allow');
    expect(config.mcp['mergen-browser']).toEqual({
      type: 'local',
      command: [helper.command, ...helper.args],
      enabled: true,
      environment: helper.env,
    });
  });

  it('preserves existing project settings while adding the Mergen browser MCP', () => {
    const configDir = path.join(tempDir!, '.opencode');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(path.join(configDir, 'opencode.json'), `{
      // OpenCode accepts JSONC-style comments and trailing commas.
      "tools": { "custom": false },
      "mcp": { "existing": { "type": "remote", "url": "https://example.test/mcp" } },
    }`, 'utf-8');

    const configPath = generateOpencodeTerminalConfig(tempDir!, { kimiStrictPermissions: false });
    const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

    expect(config.tools).toEqual({ custom: false });
    expect(config.mcp.existing).toEqual({ type: 'remote', url: 'https://example.test/mcp' });
    expect(config.mcp['mergen-browser'].enabled).toBe(true);
  });

  it('does not overwrite a user-owned Mergen browser MCP entry', () => {
    const configDir = path.join(tempDir!, '.opencode');
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(path.join(configDir, 'opencode.json'), JSON.stringify({
      mcp: { 'mergen-browser': { type: 'remote', url: 'https://example.test/mergen' } },
    }), 'utf-8');

    const configPath = generateOpencodeTerminalConfig(tempDir!, { kimiStrictPermissions: false });
    const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));

    expect(config.mcp['mergen-browser']).toEqual({
      type: 'remote',
      url: 'https://example.test/mergen',
    });
  });

  it('blocks OpenCode unless the configured Ponytail plugin injects its rules', async () => {
    const configDir = path.join(tempDir!, 'user-opencode');
    const pluginPath = path.join(configDir, 'plugins', 'ponytail.mjs');
    fs.mkdirSync(path.dirname(pluginPath), { recursive: true });
    fs.writeFileSync(pluginPath, `export default async () => ({
      'experimental.chat.system.transform': async (_input, output) => output.system.push('PONYTAIL MODE ACTIVE'),
    });`, 'utf-8');
    const configPath = path.join(configDir, 'opencode.json');
    fs.writeFileSync(configPath, JSON.stringify({ plugin: ['./plugins/ponytail.mjs'] }), 'utf-8');

    await expect(verifyOpencodePonytailPlugin(configDir)).resolves.toBe(pluginPath);
    fs.writeFileSync(configPath, JSON.stringify({ plugin: [] }), 'utf-8');
    await expect(verifyOpencodePonytailPlugin(configDir)).rejects.toThrow('configured plugin module is missing');
  });
});
