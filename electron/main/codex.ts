import path from 'path';
import fs from 'fs';
import os from 'os';
import { execFile, spawn, spawnSync } from 'child_process';
import { parse as parseToml } from '@iarna/toml';
import { getHookServicePort, getHookInboxDir } from './hookService';
import { getBrowserMcpStdioConfig } from './browserMcpCommand';
import { resolveChildPath } from './safePath';

export type CodexExecParsedEvent =
  | { kind: 'assistant_message'; text: string }
  | { kind: 'tool'; id: string; title: string; toolKind: string; status: 'running' | 'completed' | 'failed'; raw: Record<string, unknown> }
  | { kind: 'error'; text: string }
  | { kind: 'status'; title: string; text: string };

const DEFAULT_CODEX_INBOX_DIR = () => {
  const appData = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
  return path.join(appData, 'Mergen', 'MergenADE', 'runtime', 'codex-cli');
};

function ensureCodexDir(): string {
  const dir = process.env.MERGEN_ADE_CODEX_INBOX_DIR || DEFAULT_CODEX_INBOX_DIR();
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function getCodexInboxDir(): string {
  return ensureCodexDir();
}

export function getCodexBinPath(): string {
  const found = findCodexOnPath();
  if (found) return found;

  const homeDir = os.homedir();
  const candidates = [
    path.join(homeDir, 'AppData', 'Roaming', 'npm', 'codex.cmd'),
    path.join(homeDir, 'AppData', 'Roaming', 'npm', 'codex'),
    path.join(homeDir, '.npm', 'global', 'bin', 'codex'),
    path.join(homeDir, '.nvm', 'versions', 'node', 'current', 'bin', 'codex'),
  ];
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return process.platform === 'win32' ? 'codex.cmd' : 'codex';
}

function findCodexOnPath(): string | undefined {
  const command = process.platform === 'win32' ? 'where' : 'which';
  const result = spawnSync(command, ['codex'], {
    encoding: 'utf-8',
    timeout: 5000,
    windowsHide: true,
    shell: false,
  });
  if (result.error || result.status !== 0) return undefined;
  const lines = String(result.stdout || '').split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
  if (process.platform === 'win32') {
    return lines.find((s) => s.toLowerCase().endsWith('.exe'))
      || lines.find((s) => s.toLowerCase().endsWith('.cmd'))
      || lines[0];
  }
  return lines[0];
}

export function codexExecJsonArgs(cwd: string): string[] {
  const args = ['-a', 'never'];
  if (cwd.trim()) {
    args.push('-C', cwd);
  }
  args.push('exec', '--json', '--sandbox', 'workspace-write', '--skip-git-repo-check', '-');
  return args;
}

export function parseCodexExecJsonLine(line: string): CodexExecParsedEvent | undefined {
  let msg: Record<string, unknown>;
  try {
    msg = JSON.parse(line) as Record<string, unknown>;
  } catch {
    return undefined;
  }

  if (typeof msg.error === 'string' && msg.error.trim()) {
    return { kind: 'error', text: msg.error.trim() };
  }
  if (msg.type === 'turn.started') {
    return { kind: 'status', title: 'Codex turn started', text: 'Codex is working...' };
  }

  const item = asRecord(msg.item);
  if (!item) return undefined;

  const itemType = stringValue(item.type).toLowerCase();
  if (msg.type === 'item.completed' && itemType === 'agent_message') {
    const text = stringValue(item.text);
    return text ? { kind: 'assistant_message', text } : undefined;
  }

  if (msg.type !== 'item.started' && msg.type !== 'item.completed') return undefined;
  if (!isCodexToolItem(item)) return undefined;

  const id = stringValue(item.id) || stringValue(item.call_id) || stringValue(item.callId) || `${itemType || 'tool'}-${Date.now()}`;
  const title = codexToolItemTitle(item);
  const status = msg.type === 'item.started'
    ? 'running'
    : item.error || item.is_error === true
      ? 'failed'
      : 'completed';

  return {
    kind: 'tool',
    id,
    title,
    toolKind: codexToolItemKind(item),
    status,
    raw: item,
  };
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function isCodexToolItem(item: Record<string, unknown>): boolean {
  const type = stringValue(item.type).toLowerCase();
  return Boolean(
    type.includes('tool')
    || type.includes('command')
    || type.includes('exec')
    || stringValue(item.command)
    || stringValue(item.name),
  );
}

function codexToolItemTitle(item: Record<string, unknown>): string {
  const command = stringValue(item.command);
  if (command) return truncateCodexTitle(command);

  const argumentsRecord = asRecord(item.arguments) ?? asRecord(item.args) ?? asRecord(item.input);
  if (argumentsRecord) {
    const nestedCommand = stringValue(argumentsRecord.command) || stringValue(argumentsRecord.cmd);
    if (nestedCommand) return truncateCodexTitle(nestedCommand);
    const pathValue = stringValue(argumentsRecord.path) || stringValue(argumentsRecord.file_path) || stringValue(argumentsRecord.filePath);
    if (pathValue) return truncateCodexTitle(pathValue);
  }

  return stringValue(item.name) || stringValue(item.type) || 'Codex tool';
}

function codexToolItemKind(item: Record<string, unknown>): string {
  const type = stringValue(item.type).toLowerCase();
  const name = stringValue(item.name).toLowerCase();
  const title = codexToolItemTitle(item).toLowerCase();
  const value = `${type} ${name} ${title}`;
  if (value.includes('shell') || value.includes('command') || value.includes('exec')) return 'bash';
  if (value.includes('edit') || value.includes('write') || value.includes('patch')) return 'edit';
  if (value.includes('read')) return 'read';
  if (value.includes('search') || value.includes('grep') || value.includes('rg')) return 'search';
  return name || type || 'tool';
}

function truncateCodexTitle(value: string): string {
  return value.length > 120 ? `${value.slice(0, 117)}...` : value;
}

export function handleCodexNotifyMode(args = process.argv.slice(2)): boolean {
  try {
    const eventName = args[0];
    const eventJson = args[1];
    if (!eventName) {
      console.error('Missing event name for --codex-notify');
      return false;
    }

    const port = getHookServicePort();
    const dir = getCodexInboxDir();

    // Write to Codex-specific inbox
    const inboxFile = path.join(dir, `codex-${Date.now()}.json`);
    fs.writeFileSync(inboxFile, JSON.stringify({ type: eventName, rawJson: eventJson, timestamp: Date.now() }), 'utf-8');

    // Send to TCP hook service
    if (port) {
      const net = require('net');
      const client = net.createConnection({ port, host: '127.0.0.1' }, () => {
        const event = {
          type: `codex-hook:${eventName}`,
          terminalId: parseInt(process.env.MERGEN_TERMINAL_ID || '0', 10),
          rawJson: eventJson,
          eventKind: eventName,
        };
        client.write(JSON.stringify(event) + '\n');
        client.end();
      });
      client.on('error', () => {});
    }

    return true;
  } catch (err) {
    console.error('Codex notify mode failed:', err);
    return false;
  }
}

export function handleCodexHookMode(eventName: string, eventJson = process.argv.slice(3).join(' ')): void {
  try {
    const port = getHookServicePort();
    const dir = getCodexInboxDir();

    // Write to Codex-specific inbox
    const inboxFile = path.join(dir, `codex-hook-${Date.now()}.json`);
    fs.writeFileSync(inboxFile, JSON.stringify({ type: eventName, rawJson: eventJson, timestamp: Date.now() }), 'utf-8');

    // Send to TCP hook service
    if (port) {
      const net = require('net');
      const client = net.createConnection({ port, host: '127.0.0.1' }, () => {
        const event = {
          type: `codex-hook:${eventName}`,
          terminalId: parseInt(process.env.MERGEN_TERMINAL_ID || '0', 10),
          rawJson: eventJson,
          eventKind: eventName,
        };
        client.write(JSON.stringify(event) + '\n');
        client.end();
      });
      client.on('error', () => {});
    }
  } catch (err) {
    console.error('Codex hook mode failed:', err);
  }
}

export function killStaleCodexProcesses(): void {
  try {
    const ps = spawn('powershell.exe', [
      '-NoProfile',
      '-Command',
      `Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'node.exe' -and ($_.CommandLine -like '*codex*' -or $_.CommandLine -like '*@openai*') } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }`,
    ], { windowsHide: true });
    ps.on('error', () => {});
  } catch {
    // ignore
  }
}

export function verifyCodexVersion(): string | null {
  try {
    const command = npmCommand();
    const result = spawnSync(command.file, [...command.args, 'list', '-g', '@openai/codex', '--depth=0'], {
      encoding: 'utf-8',
      timeout: 10000,
      windowsHide: true,
      shell: false,
    });
    if (result.error || result.status !== 0) return null;
    const match = String(result.stdout || '').match(/@openai\/codex@(\d+\.\d+\.\d+)/);
    return match ? match[1] : null;
  } catch {
    return null;
  }
}

export function installCodexCli(): void {
  killStaleCodexProcesses();
  try {
    const command = npmCommand();
    const proc = spawn(command.file, [...command.args, 'install', '-g', '@openai/codex'], {
      stdio: 'inherit',
      windowsHide: true,
      shell: false,
    });
    proc.on('exit', (code) => {
      if (code === 0) {
        const version = verifyCodexVersion();
        console.log(`Codex CLI updated to version ${version || 'unknown'}`);
      } else {
        console.error('Codex CLI update failed');
      }
    });
  } catch (err) {
    console.error('Failed to install Codex CLI:', err);
  }
}

function npmCommand(): { file: string; args: string[] } {
  if (process.platform !== 'win32') return { file: 'npm', args: [] };
  const npmCli = path.join(path.dirname(process.execPath), 'node_modules', 'npm', 'bin', 'npm-cli.js');
  if (fs.existsSync(npmCli)) return { file: process.execPath, args: [npmCli] };
  return { file: 'cmd.exe', args: ['/d', '/s', '/c', 'npm'] };
}

function getCodexConfigDir(): string {
  const homeDir = os.homedir();
  const dir = path.join(homeDir, '.codex');
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

export function verifyCodexPonytailPlugin(homeDir = os.homedir()): string {
  const codexDir = resolveChildPath(homeDir, '.codex');
  const configPath = resolveChildPath(codexDir, 'config.toml');
  if (!fs.existsSync(configPath)) throw new Error('Codex Ponytail preflight failed: config.toml is missing');

  let config: Record<string, unknown>;
  try {
    config = asRecord(parseToml(fs.readFileSync(configPath, 'utf-8'))) ?? {};
  } catch (err) {
    throw new Error(`Codex Ponytail preflight failed: invalid config.toml (${err instanceof Error ? err.message : String(err)})`);
  }

  const plugin = asRecord(asRecord(config.plugins)?.['ponytail@ponytail']);
  if (plugin?.enabled !== true) throw new Error('Codex Ponytail preflight failed: plugin is not enabled');

  const hookState = asRecord(asRecord(config.hooks)?.state) ?? {};
  const trustedHook = Object.entries(hookState).some(([key, value]) => {
    const state = asRecord(value);
    return key.startsWith('ponytail@ponytail:')
      && key.includes('session_start')
      && state?.enabled !== false
      && typeof state?.trusted_hash === 'string';
  });
  if (!trustedHook) throw new Error('Codex Ponytail preflight failed: no trusted session hook');

  const cacheRoot = resolveChildPath(codexDir, 'plugins', 'cache', 'ponytail', 'ponytail');
  if (!fs.existsSync(cacheRoot)) throw new Error('Codex Ponytail preflight failed: plugin cache is missing');
  const version = fs.readdirSync(cacheRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((a, b) => b.localeCompare(a, undefined, { numeric: true }))[0];
  if (!version) throw new Error('Codex Ponytail preflight failed: plugin cache is missing');

  const pluginRoot = resolveChildPath(cacheRoot, version);
  for (const required of [
    '.codex-plugin/plugin.json',
    'skills/ponytail/SKILL.md',
    'hooks/claude-codex-hooks.json',
    'hooks/ponytail-config.js',
    'hooks/ponytail-instructions.js',
  ]) {
    if (!fs.existsSync(resolveChildPath(pluginRoot, required))) {
      throw new Error(`Codex Ponytail preflight failed: ${required} is missing`);
    }
  }

  try {
    const { getDefaultMode } = require(resolveChildPath(pluginRoot, 'hooks', 'ponytail-config.js')) as {
      getDefaultMode: () => string;
    };
    const { getPonytailInstructions } = require(resolveChildPath(pluginRoot, 'hooks', 'ponytail-instructions.js')) as {
      getPonytailInstructions: (mode: string) => string;
    };
    const mode = getDefaultMode();
    const instructions = mode === 'off' ? '' : getPonytailInstructions(mode);
    if (!['lite', 'full', 'ultra'].includes(mode) || !/ponytail/i.test(instructions)) {
      throw new Error(`inactive mode: ${mode}`);
    }
    return mode;
  } catch (err) {
    throw new Error(`Codex Ponytail preflight failed: plugin runtime check failed (${err instanceof Error ? err.message : String(err)})`);
  }
}

export function parseCodexPonytailPluginList(output: string): string {
  const installed = asRecord(JSON.parse(output))?.installed;
  const plugin = Array.isArray(installed)
    ? installed.map(asRecord).find((entry) => entry?.pluginId === 'ponytail@ponytail')
    : undefined;
  if (plugin?.installed !== true || plugin.enabled !== true || typeof plugin.version !== 'string') {
    throw new Error('Ponytail is not installed and enabled');
  }
  return plugin.version;
}

export function verifyCodexPonytailCli(): Promise<string> {
  const bin = getCodexBinPath();
  return new Promise((resolve, reject) => execFile(
    bin,
    ['plugin', 'list', '--marketplace', 'ponytail', '--json'],
    { encoding: 'utf-8', timeout: 15_000, windowsHide: true, shell: process.platform === 'win32' && /\.(cmd|bat)$/i.test(bin) },
    (error, stdout, stderr) => {
      if (error) return reject(new Error(`Codex Ponytail preflight failed: CLI plugin check failed (${error.message || stderr})`));
      try {
        resolve(parseCodexPonytailPluginList(stdout));
      } catch (err) {
        reject(new Error(`Codex Ponytail preflight failed: CLI plugin check failed (${err instanceof Error ? err.message : String(err)})`));
      }
    },
  ));
}

const MERGEN_MCP_BLOCK_START = '# BEGIN MERGEN ADE BROWSER MCP';
const MERGEN_MCP_BLOCK_END = '# END MERGEN ADE BROWSER MCP';

export function generateCodexTerminalConfig(cwd: string): string {
  if (!cwd || typeof cwd !== 'string') {
    throw new Error('Invalid cwd for Codex terminal config');
  }

  const configPath = resolveChildPath(cwd, '.codex', 'config.toml');
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  const current = fs.existsSync(configPath) ? fs.readFileSync(configPath, 'utf-8') : '';
  const eol = current.includes('\r\n') ? '\r\n' : '\n';
  const helper = getBrowserMcpStdioConfig();
  const block = [
    MERGEN_MCP_BLOCK_START,
    '[mcp_servers.mergen-browser]',
    `command = ${JSON.stringify(helper.command)}`,
    `args = ${JSON.stringify(helper.args)}`,
    'enabled = true',
    '[mcp_servers.mergen-browser.env]',
    'ELECTRON_RUN_AS_NODE = "1"',
    MERGEN_MCP_BLOCK_END,
  ].join(eol);
  const managedBlock = new RegExp(`${MERGEN_MCP_BLOCK_START}[\\s\\S]*?${MERGEN_MCP_BLOCK_END}`);
  let existingMergen = false;
  if (current) {
    try {
      const parsed = asRecord(parseToml(current));
      const servers = asRecord(parsed?.mcp_servers);
      existingMergen = Boolean(servers && Object.prototype.hasOwnProperty.call(servers, 'mergen-browser'));
    } catch (err) {
      throw new Error(`Invalid Codex project config: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  let next: string;
  if (managedBlock.test(current)) {
    next = current.replace(managedBlock, block);
  } else if (existingMergen) {
    return configPath;
  } else {
    const separator = current.endsWith(`${eol}${eol}`) ? '' : current.endsWith(eol) ? eol : `${eol}${eol}`;
    next = current ? `${current}${separator}${block}${eol}` : `${block}${eol}`;
  }

  if (next !== current) {
    fs.writeFileSync(configPath, next, 'utf-8');
  }
  return configPath;
}

export function writeCodexHooksConfig(): string {
  const configPath = path.join(getCodexConfigDir(), 'hooks.json');
  const exe = process.execPath;
  const config = {
    hooks: [
      {
        event: 'UserPromptSubmit',
        command: `"${exe}" --codex-notify UserPromptSubmit`,
      },
      {
        event: 'PreToolUse',
        command: `"${exe}" --codex-notify PreToolUse`,
      },
      {
        event: 'PostToolUse',
        command: `"${exe}" --codex-notify PostToolUse`,
      },
      {
        event: 'PermissionRequest',
        command: `"${exe}" --codex-notify PermissionRequest`,
      },
      {
        event: 'Stop',
        command: `"${exe}" --codex-notify Stop`,
      },
    ],
  };
  const text = JSON.stringify(config, null, 2);
  fs.writeFileSync(configPath, text, 'utf-8');
  return configPath;
}

export function generateCodexHooksConfig(): string {
  const exe = process.execPath;
  const config = {
    hooks: [
      {
        event: 'UserPromptSubmit',
        command: `"${exe}" --codex-notify UserPromptSubmit`,
      },
      {
        event: 'PreToolUse',
        command: `"${exe}" --codex-notify PreToolUse`,
      },
      {
        event: 'PostToolUse',
        command: `"${exe}" --codex-notify PostToolUse`,
      },
      {
        event: 'PermissionRequest',
        command: `"${exe}" --codex-notify PermissionRequest`,
      },
      {
        event: 'Stop',
        command: `"${exe}" --codex-notify Stop`,
      },
    ],
  };
  return JSON.stringify(config, null, 2);
}
