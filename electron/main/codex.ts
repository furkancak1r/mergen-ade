import path from 'path';
import fs from 'fs';
import { spawn } from 'child_process';
import { getHookServicePort, getHookInboxDir } from './hookService';

const DEFAULT_CODEX_INBOX_DIR = () => {
  const appData = process.env.APPDATA || path.join(require('os').homedir(), 'AppData', 'Roaming');
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

export function handleCodexNotifyMode(): boolean {
  try {
    const args = process.argv.slice(2);
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

export function handleCodexHookMode(eventName: string): void {
  try {
    const eventJson = process.argv.slice(3).join(' ');
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
    const result = require('child_process').execSync('npm list -g @openai/codex --depth=0', { encoding: 'utf-8', timeout: 10000 });
    const match = result.match(/@openai\/codex@(\d+\.\d+\.\d+)/);
    return match ? match[1] : null;
  } catch {
    return null;
  }
}

export function installCodexCli(): void {
  killStaleCodexProcesses();
  try {
    const proc = spawn('npm', ['install', '-g', '@openai/codex'], {
      stdio: 'inherit',
      windowsHide: true,
      shell: process.platform === 'win32',
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

function getCodexConfigDir(): string {
  const homeDir = require('os').homedir();
  const dir = path.join(homeDir, '.codex');
  fs.mkdirSync(dir, { recursive: true });
  return dir;
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
