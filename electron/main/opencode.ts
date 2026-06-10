import { spawn } from 'child_process';
import path from 'path';
import fs from 'fs';
import { getHookServicePort, getHookInboxDir } from './hookService';

// OpenCode process detection and plugin lifecycle
const OPENCODE_PLUGIN_DIR = () => {
  const appData = process.env.APPDATA || path.join(require('os').homedir(), 'AppData', 'Roaming');
  return path.join(appData, 'Mergen', 'MergenADE', 'runtime', 'opencode');
};

function ensurePluginDir(): string {
  const dir = OPENCODE_PLUGIN_DIR();
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function getPluginPath(): string {
  return path.join(ensurePluginDir(), 'mergen-opencode-status.js');
}

const PLUGIN_SCRIPT = `
// Mergen OpenCode Status Plugin
const http = require('http');

function getHookPort() {
  // Read from environment or a known file
  const port = process.env.MERGEN_HOOK_PORT;
  return port ? parseInt(port, 10) : 0;
}

function sendEvent(event) {
  const port = getHookPort();
  if (!port) return;
  const data = JSON.stringify(event);
  const req = http.request({
    hostname: '127.0.0.1',
    port,
    path: '/',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': Buffer.byteLength(data),
    },
  }, (res) => {
    res.on('data', () => {});
    res.on('end', () => {});
  });
  req.on('error', () => {});
  req.write(data);
  req.end();
}

// Poll for questions
function pollAnswers() {
  const port = getHookPort();
  if (!port) return;
  const req = http.request({
    hostname: '127.0.0.1',
    port,
    path: '/answer',
    method: 'GET',
  }, (res) => {
    let data = '';
    res.on('data', (chunk) => { data += chunk; });
    res.on('end', () => {
      try {
        const answer = JSON.parse(data);
        if (answer.requestId && typeof answer.answers !== 'undefined') {
          const client = globalThis.__opencodeClient || (typeof client !== 'undefined' ? client : null);
          if (client?.question?.reply) {
            client.question.reply({ requestID: answer.requestId, answers: answer.answers });
            // Acknowledge
            const ack = http.request({
              hostname: '127.0.0.1',
              port,
              path: '/answer/ack',
              method: 'POST',
              headers: { 'Content-Length': 0 },
            }, () => {});
            ack.on('error', () => {});
            ack.end();
          } else if (client?.question?.reject && answer.rejected) {
            client.question.reject({ requestID: answer.requestId });
            const ack = http.request({
              hostname: '127.0.0.1',
              port,
              path: '/answer/ack',
              method: 'POST',
              headers: { 'Content-Length': 0 },
            }, () => {});
            ack.on('error', () => {});
            ack.end();
          }
        }
      } catch { /* ignore */ }
    });
  });
  req.on('error', () => {});
  req.end();
}

setInterval(pollAnswers, 500);

module.exports = {
  onEvent(event) {
    if (event.type === 'UserPromptSubmit') {
      sendEvent({ type: 'opencode-hook:UserPromptSubmit', terminalId: process.env.MERGEN_TERMINAL_ID || 0, reason: 'PromptSubmit' });
    } else if (event.type === 'Stop') {
      sendEvent({ type: 'opencode-hook:Stop', terminalId: process.env.MERGEN_TERMINAL_ID || 0, reason: 'TurnComplete' });
    } else if (event.type === 'question.asked') {
      sendEvent({ type: 'opencode-hook:question.asked', terminalId: process.env.MERGEN_TERMINAL_ID || 0, rawJson: JSON.stringify({ question: event.question }), eventKind: 'question.asked' });
    } else if (event.type === 'permission.asked') {
      sendEvent({ type: 'opencode-hook:permission.asked', terminalId: process.env.MERGEN_TERMINAL_ID || 0, rawJson: JSON.stringify({ permission: event.permission }), eventKind: 'permission.asked' });
    } else if (event.type === 'plan_mode_prompt') {
      sendEvent({ type: 'opencode-hook:plan_mode_prompt', terminalId: process.env.MERGEN_TERMINAL_ID || 0, rawJson: JSON.stringify({ plan_mode_prompt: event.plan_mode_prompt }), eventKind: 'plan_mode_prompt' });
    }
  },
};
`.trim();

export function ensureOpencodePlugin(): string {
  const pluginPath = getPluginPath();
  if (!fs.existsSync(pluginPath)) {
    fs.writeFileSync(pluginPath, PLUGIN_SCRIPT, 'utf-8');
  }
  return pluginPath;
}

export function getOpencodePluginDir(): string {
  return ensurePluginDir();
}

export function getOpencodeBinPath(): string {
  // Try to resolve opencode via PATH using native commands
  try {
    if (process.platform === 'win32') {
      const result = require('child_process').execSync('where opencode', { encoding: 'utf-8', timeout: 5000 });
      const lines = result.split('\n').map((s: string) => s.trim()).filter((s: string) => s);
      // Prefer .exe directly if available; .cmd can cause spawn EINVAL on Node.js 22+
      const exeLine = lines.find((s: string) => s.toLowerCase().endsWith('.exe'));
      if (exeLine) return exeLine;
      const cmdLine = lines.find((s: string) => s.toLowerCase().endsWith('.cmd'));
      if (cmdLine) {
        // If .cmd found, try to find the real .exe inside node_modules next to it
        const cmdDir = path.dirname(cmdLine);
        const exeCandidate = path.join(cmdDir, 'node_modules', 'opencode-ai', 'bin', 'opencode.exe');
        if (fs.existsSync(exeCandidate)) return exeCandidate;
        return cmdLine;
      }
      const first = lines[0];
      if (first) return first;
    } else {
      const result = require('child_process').execSync('which opencode', { encoding: 'utf-8', timeout: 5000 });
      const first = result.trim();
      if (first) return first;
    }
  } catch {
    // fallback
  }
  // Check known npm global paths
  const homeDir = require('os').homedir();
  const candidates = [
    path.join(homeDir, 'AppData', 'Roaming', 'npm', 'node_modules', 'opencode-ai', 'bin', 'opencode.exe'),
    path.join(homeDir, 'AppData', 'Roaming', 'npm', 'opencode.cmd'),
    path.join(homeDir, '.npm', 'global', 'bin', 'opencode'),
    path.join(homeDir, '.nvm', 'versions', 'node', 'current', 'bin', 'opencode'),
  ];
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return 'opencode';
}

export function handleOpencodeNotifyMode(): boolean {
  try {
    const args = process.argv.slice(2);
    const eventName = args[0];
    const eventJson = args[1];
    if (!eventName) {
      console.error('Missing event name for --opencode-notify');
      return false;
    }

    const port = getHookServicePort();
    const dir = getHookInboxDir();

    // Write to inbox
    const inboxFile = path.join(dir, `opencode-${Date.now()}.json`);
    fs.writeFileSync(inboxFile, JSON.stringify({ type: eventName, rawJson: eventJson, timestamp: Date.now() }), 'utf-8');

    // Also send to TCP hook service
    if (port) {
      const net = require('net');
      const client = net.createConnection({ port, host: '127.0.0.1' }, () => {
        const event = {
          type: `opencode-notify:${eventName}`,
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
    console.error('OpenCode notify mode failed:', err);
    return false;
  }
}

function getOpencodeGlobalConfigPath(): string {
  const homeDir = require('os').homedir();
  const dir = path.join(homeDir, '.opencode');
  fs.mkdirSync(dir, { recursive: true });
  return path.join(dir, 'opencode.json');
}

export function generateOpencodeRuntimeConfig(cwd: string, opts: {
  model?: string;
  effort?: string;
  mcpServers?: string[];
  kimiStrictPermissions?: boolean;
}): string {
  // Write to global OpenCode config so it does not overwrite per-project terminal config
  const configPath = getOpencodeGlobalConfigPath();

  const mcpServers: Record<string, { command: string[]; enabled: boolean }> = {};
  // Disable external browser MCP servers
  for (const name of ['playwright', 'browser', 'puppeteer']) {
    mcpServers[name] = { command: ['echo', 'disabled'], enabled: false };
  }

  // Enable mergen-browser MCP
  const exe = process.execPath;
  mcpServers['mergen-browser'] = {
    command: [exe, '--browser-mcp-helper', '--caps=devtools,vision,network,storage'],
    enabled: true,
  };

  if (opts.mcpServers) {
    for (const s of opts.mcpServers) {
      mcpServers[s] = { command: ['echo', s], enabled: true };
    }
  }

  const permission: Record<string, string> | undefined = opts.kimiStrictPermissions
    ? { '*': 'ask', 'edit': 'ask', 'task/external_directory': 'deny', 'bash': 'ask' }
    : opts.kimiStrictPermissions === false
      ? { '*': 'allow', 'edit': 'allow', 'task/external_directory': 'allow', 'bash': 'allow' }
      : undefined;

  const config: Record<string, unknown> = {
    agent: {
      build: {
        model: opts.model || 'sonnet',
      },
    },
    mode: {
      build: {
        model: opts.model || 'sonnet',
      },
    },
    mcpServers,
  };

  if (permission) {
    (config.agent as Record<string, unknown>).build = {
      ...((config.agent as Record<string, unknown>).build as Record<string, unknown>),
      permission,
    };
    (config.mode as Record<string, unknown>).build = {
      ...((config.mode as Record<string, unknown>).build as Record<string, unknown>),
      permission,
    };
  }

  fs.writeFileSync(configPath, JSON.stringify(config, null, 2), 'utf-8');
  return configPath;
}

export function generateOpencodeTerminalConfig(cwd: string, opts: {
  model?: string;
  effort?: string;
  kimiStrictPermissions?: boolean;
}): string {
  if (!cwd || typeof cwd !== 'string') {
    throw new Error('Invalid cwd for OpenCode terminal config');
  }
  const configPath = [cwd, '.opencode', 'opencode.json'].join(path.sep);
  fs.mkdirSync(path.dirname(configPath), { recursive: true });

  const permission: Record<string, string> = opts.kimiStrictPermissions
    ? { '*': 'ask', 'edit': 'ask', 'task/external_directory': 'deny', 'bash': 'ask' }
    : { '*': 'allow', 'edit': 'allow', 'task/external_directory': 'allow', 'bash': 'allow' };

  const config = {
    agent: {
      build: {
        model: opts.model || 'sonnet',
        permission,
      },
    },
    mode: {
      build: {
        model: opts.model || 'sonnet',
        permission,
      },
    },
  };

  fs.writeFileSync(configPath, JSON.stringify(config, null, 2), 'utf-8');
  return configPath;
}
