import { spawn } from 'child_process';
import { app } from 'electron';

interface BrowserMcpMessage {
  jsonrpc: '2.0';
  id?: string | number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code: number; message: string };
}

function sendMessage(msg: BrowserMcpMessage): void {
  process.stdout.write(JSON.stringify(msg) + '\n');
}

function readArgs(): string[] {
  const args = process.argv.slice(2);
  const capsIdx = args.indexOf('--caps');
  if (capsIdx >= 0 && capsIdx + 1 < args.length) {
    return args[capsIdx + 1].split(',');
  }
  return ['devtools', 'vision', 'network', 'storage'];
}

function getExePath(): string {
  return process.execPath;
}

function getMcpCommand(): string[] {
  const caps = readArgs();
  return [getExePath(), '--browser-mcp-helper', `--caps=${caps.join(',')}`];
}

export function getBrowserMcpCommandArray(): string[] {
  return getMcpCommand();
}

export function handleBrowserMcpHelperMode(): void {
  const caps = readArgs();
  let buffer = '';

  process.stdin.on('data', (data: Buffer) => {
    buffer += data.toString();
    let idx;
    while ((idx = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 1);
      if (line.trim()) {
        handleLine(line.trim(), caps);
      }
    }
  });

  process.stdin.on('end', () => {
    process.exit(0);
  });
}

async function handleLine(line: string, caps: string[]): Promise<void> {
  try {
    const msg = JSON.parse(line) as BrowserMcpMessage;
    if (msg.method && msg.id !== undefined) {
      // Request
      const result = await handleMethod(msg.method, msg.params, caps);
      sendMessage({ jsonrpc: '2.0', id: msg.id, result });
    } else if (msg.method) {
      // Notification
      await handleMethod(msg.method, msg.params, caps);
    }
  } catch (err) {
    // ignore malformed JSON or errors
  }
}

const pending = new Map<string | number, (result: unknown) => void>();

process.on('message', (msg: unknown) => {
  if (msg && typeof msg === 'object') {
    const m = msg as Record<string, unknown>;
    if (m.type === 'browserMcpResponse' && m.id !== undefined) {
      const resolve = pending.get(m.id as string | number);
      if (resolve) {
        pending.delete(m.id as string | number);
        resolve(m.result);
      }
    }
  }
});

async function handleMethod(method: string, params: unknown, caps: string[]): Promise<unknown> {
  if (method === 'initialize') {
    return {
      protocolVersion: '2024-11-05',
      capabilities: {},
      serverInfo: { name: 'mergen-browser-mcp', version: '1.0.0' },
    };
  }

  const p = (params as Record<string, unknown>) || {};
  const id = (p.__relayId as string | number) ?? Date.now() + Math.random();

  return new Promise((resolve) => {
    pending.set(id, resolve);
    // Relay to parent process
    if (process.send) {
      process.send({ type: 'browserMcpRequest', id, method, params, caps });
    } else {
      resolve({ success: false, error: 'No IPC channel to main process' });
    }
  });
}
