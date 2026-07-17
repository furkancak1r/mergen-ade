import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { codexExecJsonArgs, generateCodexTerminalConfig, parseCodexExecJsonLine, parseCodexPonytailPluginList, verifyCodexPonytailPlugin } from './codex';
import { getBrowserMcpStdioConfig } from './browserMcpCommand';

let tempDir: string | undefined;

describe('codex helpers', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-electron-codex-'));
  });

  afterEach(() => {
    if (tempDir) fs.rmSync(tempDir, { recursive: true, force: true });
    tempDir = undefined;
  });

  it('builds JSON exec args with non-interactive approval policy', () => {
    expect(codexExecJsonArgs('C:/repo')).toEqual([
      '-a',
      'never',
      '-C',
      'C:/repo',
      'exec',
      '--json',
      '--sandbox',
      'workspace-write',
      '--skip-git-repo-check',
      '-',
    ]);
  });

  it('parses Codex assistant message events', () => {
    const parsed = parseCodexExecJsonLine(JSON.stringify({
      type: 'item.completed',
      item: { id: 'item_0', type: 'agent_message', text: 'READY' },
    }));

    expect(parsed).toEqual({ kind: 'assistant_message', text: 'READY' });
  });

  it('parses Codex command tool events', () => {
    const started = parseCodexExecJsonLine(JSON.stringify({
      type: 'item.started',
      item: { id: 'call_1', type: 'command_execution', command: 'npm test' },
    }));
    const completed = parseCodexExecJsonLine(JSON.stringify({
      type: 'item.completed',
      item: { id: 'call_1', type: 'command_execution', command: 'npm test' },
    }));

    expect(started).toMatchObject({
      kind: 'tool',
      id: 'call_1',
      title: 'npm test',
      toolKind: 'bash',
      status: 'running',
    });
    expect(completed).toMatchObject({
      kind: 'tool',
      id: 'call_1',
      title: 'npm test',
      toolKind: 'bash',
      status: 'completed',
    });
  });

  it('ignores non-json lines from Codex stderr-style output', () => {
    expect(parseCodexExecJsonLine('WARN not json')).toBeUndefined();
  });

  it('adds one managed Mergen browser MCP block without replacing project settings', () => {
    const configDir = path.join(tempDir!, '.codex');
    fs.mkdirSync(configDir, { recursive: true });
    const configPath = path.join(configDir, 'config.toml');
    fs.writeFileSync(configPath, '[features]\nunified_exec = true\n', 'utf-8');

    generateCodexTerminalConfig(tempDir!);
    generateCodexTerminalConfig(tempDir!);
    const config = fs.readFileSync(configPath, 'utf-8');
    const helper = getBrowserMcpStdioConfig();

    expect(config).toContain('[features]\nunified_exec = true');
    expect(config).toContain('[mcp_servers.mergen-browser]');
    expect(config).toContain(`command = ${JSON.stringify(helper.command)}`);
    expect(config).toContain(`args = ${JSON.stringify(helper.args)}`);
    expect(config).toContain('[mcp_servers.mergen-browser.env]\nELECTRON_RUN_AS_NODE = "1"');
    expect(config.match(/# BEGIN MERGEN ADE BROWSER MCP/g)).toHaveLength(1);
  });

  it('leaves a user-owned Mergen browser MCP block unchanged', () => {
    const configDir = path.join(tempDir!, '.codex');
    fs.mkdirSync(configDir, { recursive: true });
    const configPath = path.join(configDir, 'config.toml');
    const original = '["mcp_servers"."mergen-browser"]\ncommand = "custom-helper"\n';
    fs.writeFileSync(configPath, original, 'utf-8');

    generateCodexTerminalConfig(tempDir!);

    expect(fs.readFileSync(configPath, 'utf-8')).toBe(original);
  });

  it('does not modify an invalid existing Codex project config', () => {
    const configDir = path.join(tempDir!, '.codex');
    fs.mkdirSync(configDir, { recursive: true });
    const configPath = path.join(configDir, 'config.toml');
    const original = '[features\nunified_exec = true\n';
    fs.writeFileSync(configPath, original, 'utf-8');

    expect(() => generateCodexTerminalConfig(tempDir!)).toThrow('Invalid Codex project config');
    expect(fs.readFileSync(configPath, 'utf-8')).toBe(original);
  });

  it('blocks Codex unless the Ponytail plugin, hook, and runtime are active', () => {
    const codexDir = path.join(tempDir!, '.codex');
    const pluginRoot = path.join(codexDir, 'plugins', 'cache', 'ponytail', 'ponytail', '4.8.4');
    for (const dir of ['.codex-plugin', 'skills/ponytail', 'hooks']) {
      fs.mkdirSync(path.join(pluginRoot, dir), { recursive: true });
    }
    fs.writeFileSync(path.join(pluginRoot, '.codex-plugin', 'plugin.json'), '{}', 'utf-8');
    fs.writeFileSync(path.join(pluginRoot, 'skills', 'ponytail', 'SKILL.md'), '# Ponytail', 'utf-8');
    fs.writeFileSync(path.join(pluginRoot, 'hooks', 'claude-codex-hooks.json'), '{}', 'utf-8');
    fs.writeFileSync(path.join(pluginRoot, 'hooks', 'ponytail-config.js'), "module.exports={getDefaultMode:()=> 'full'};", 'utf-8');
    fs.writeFileSync(path.join(pluginRoot, 'hooks', 'ponytail-instructions.js'), "module.exports={getPonytailInstructions:()=> 'PONYTAIL MODE ACTIVE'};", 'utf-8');
    const configPath = path.join(codexDir, 'config.toml');
    fs.writeFileSync(configPath, [
      '[plugins."ponytail@ponytail"]',
      'enabled = true',
      '[hooks.state."ponytail@ponytail:hooks/claude-codex-hooks.json:session_start:0:0"]',
      'trusted_hash = "sha256:test"',
    ].join('\n'), 'utf-8');

    expect(verifyCodexPonytailPlugin(tempDir!)).toBe('full');
    fs.writeFileSync(configPath, '[plugins."ponytail@ponytail"]\nenabled = false\n', 'utf-8');
    expect(() => verifyCodexPonytailPlugin(tempDir!)).toThrow('plugin is not enabled');
  });

  it('requires the Codex CLI to report Ponytail installed and enabled', () => {
    expect(parseCodexPonytailPluginList(JSON.stringify({ installed: [{
      pluginId: 'ponytail@ponytail', installed: true, enabled: true, version: '4.8.4',
    }] }))).toBe('4.8.4');
    expect(() => parseCodexPonytailPluginList('{"installed":[]}')).toThrow('not installed and enabled');
  });
});
