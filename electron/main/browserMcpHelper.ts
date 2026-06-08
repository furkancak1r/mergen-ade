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

async function handleMethod(method: string, params: unknown, caps: string[]): Promise<unknown> {
  // Minimal MCP tool stubs for browser automation
  const p = (params as Record<string, unknown>) || {};
  switch (method) {
    case 'browser/navigate': {
      const url = (p.url as string) || '';
      return { success: true, url };
    }
    case 'browser/click': {
      const selector = (p.selector as string) || '';
      return { success: true, selector };
    }
    case 'browser/type': {
      const selector = (p.selector as string) || '';
      const text = (p.text as string) || '';
      return { success: true, selector, text };
    }
    case 'browser/screenshot': {
      return { success: true, dataUrl: '', cap: caps.includes('vision') };
    }
    case 'browser/getText': {
      const selector = (p.selector as string) || '';
      return { success: true, selector, text: '' };
    }
    case 'browser/close': {
      return { success: true };
    }
    case 'browser/waitFor': {
      const duration = (p.duration as number) || 1000;
      await new Promise((resolve) => setTimeout(resolve, duration));
      return { success: true, duration };
    }
    case 'initialize': {
      return {
        protocolVersion: '2024-11-05',
        capabilities: {},
        serverInfo: { name: 'mergen-browser-mcp', version: '1.0.0' },
      };
    }
    default:
      return { success: false, error: `Unknown method: ${method}` };
  }
}
