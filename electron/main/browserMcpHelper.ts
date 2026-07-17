import http from 'http';
import {
  browserMcpToolSchemas,
  isBrowserMcpToolAllowed,
  MERGEN_BROWSER_MCP_ENDPOINT_PATH,
  MERGEN_BROWSER_MCP_PORT_ENV_VAR,
  MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR,
  MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR,
  MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR,
  MERGEN_BROWSER_MCP_TOKEN_ENV_VAR,
  parseBrowserMcpCapsFromArgs,
} from './browserMcpTools';

interface BrowserMcpMessage {
  jsonrpc: '2.0';
  id?: string | number | null;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: { code: number; message: string };
}

interface BrowserMcpRelayResult {
  success?: boolean;
  error?: string;
  text?: string;
  data?: Record<string, unknown>;
  dataUrl?: string;
  [key: string]: unknown;
}

type BrowserMcpRelay = (method: string, params: unknown) => Promise<BrowserMcpRelayResult>;

const pendingProcessRelays = new Map<string | number, (result: BrowserMcpRelayResult) => void>();

process.on('message', (msg: unknown) => {
  if (msg && typeof msg === 'object') {
    const m = msg as Record<string, unknown>;
    if (m.type === 'browserMcpResponse' && m.id !== undefined) {
      const resolve = pendingProcessRelays.get(m.id as string | number);
      if (resolve) {
        pendingProcessRelays.delete(m.id as string | number);
        resolve((m.result as BrowserMcpRelayResult) ?? { success: false, error: 'Empty Browser MCP IPC response' });
      }
    }
  }
});

function sendMessage(msg: BrowserMcpMessage): void {
  process.stdout.write(JSON.stringify(msg) + '\n');
}

function readCaps(): string[] {
  return parseBrowserMcpCapsFromArgs(process.argv);
}

export function handleBrowserMcpHelperMode(): void {
  const caps = readCaps();
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

function handleLine(line: string, caps: string[]): void {
  let msg: BrowserMcpMessage;
  try {
    msg = JSON.parse(line) as BrowserMcpMessage;
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    sendMessage(jsonRpcError(null, -32700, message));
    return;
  }

  handleBrowserMcpJsonRpcMessage(msg, caps)
    .then((response) => {
      if (response) sendMessage(response);
    })
    .catch((err) => {
      const message = err instanceof Error ? err.message : String(err);
      sendMessage(jsonRpcError(msg.id ?? null, -32603, message));
    });
}

export async function handleBrowserMcpJsonRpcMessage(
  msg: BrowserMcpMessage,
  caps: readonly string[],
  relay: BrowserMcpRelay = relayBrowserMcpRequest,
): Promise<BrowserMcpMessage | undefined> {
  const id = msg.id;
  if (id === undefined) return undefined;

  switch (msg.method) {
    case 'initialize':
      return {
        jsonrpc: '2.0',
        id,
        result: {
          protocolVersion: '2024-11-05',
          capabilities: { tools: {} },
          serverInfo: { name: 'mergen-browser-mcp', version: '1.0.0' },
          instructions: 'Controls the embedded Mergen ADE Browser panel. All actions are reflected live in the Mergen Browser panel.',
        },
      };
    case 'tools/list':
      return {
        jsonrpc: '2.0',
        id,
        result: { tools: browserMcpToolSchemas(caps) },
      };
    case 'tools/call': {
      const params = (msg.params && typeof msg.params === 'object' ? msg.params : {}) as Record<string, unknown>;
      const name = typeof params.name === 'string' ? params.name : '';
      const args = params.arguments ?? {};
      if (!isBrowserMcpToolAllowed(name, caps)) {
        return jsonRpcError(id, -32601, `Tool not found: ${name}. Use tools from tools/list only.`);
      }
      const result = await relay(name, args);
      return {
        jsonrpc: '2.0',
        id,
        result: {
          content: browserMcpContent(result),
          isError: result.success === false || Boolean(result.error),
        },
      };
    }
    default:
      return jsonRpcError(id, -32601, `Method not found: ${msg.method ?? ''}`);
  }
}

function jsonRpcError(id: string | number | null, code: number, message: string): BrowserMcpMessage {
  return { jsonrpc: '2.0', id, error: { code, message } };
}

export function browserMcpContent(result: BrowserMcpRelayResult): Record<string, unknown>[] {
  const text = result.error
    ?? result.text
    ?? (result.success === false ? 'Browser MCP tool failed' : 'Browser MCP tool completed');
  const content: Record<string, unknown>[] = [{ type: 'text', text }];
  const dataUrl = typeof result.dataUrl === 'string'
    ? result.dataUrl
    : typeof result.data?.dataUrl === 'string'
      ? result.data.dataUrl
      : undefined;
  if (dataUrl) {
    const match = dataUrl.match(/^data:(image\/[^;]+);base64,(.+)$/);
    if (match) {
      content.push({ type: 'image', mimeType: match[1], data: match[2] });
    }
  }
  if (typeof result.data?.base64 === 'string') {
    const imageType = typeof result.data.imageType === 'string' ? result.data.imageType : 'png';
    const mimeType = imageType === 'jpeg' || imageType === 'jpg' ? 'image/jpeg' : 'image/png';
    content.push({ type: 'image', mimeType, data: result.data.base64 });
  }
  return content;
}

async function relayBrowserMcpRequest(method: string, params: unknown): Promise<BrowserMcpRelayResult> {
  const port = Number(process.env[MERGEN_BROWSER_MCP_PORT_ENV_VAR]);
  const token = process.env[MERGEN_BROWSER_MCP_TOKEN_ENV_VAR];
  if (!Number.isFinite(port) || port <= 0) {
    if (process.send) {
      return relayBrowserMcpRequestOverProcessIpc(method, params);
    }
    return {
      success: false,
      error: `Mergen Browser MCP is not connected: ${MERGEN_BROWSER_MCP_PORT_ENV_VAR} is missing. Start OpenCode from Mergen ADE.`,
    };
  }
  if (!token) {
    if (process.send) {
      return relayBrowserMcpRequestOverProcessIpc(method, params);
    }
    return {
      success: false,
      error: `Mergen Browser MCP is not connected: ${MERGEN_BROWSER_MCP_TOKEN_ENV_VAR} is missing. Start OpenCode from Mergen ADE.`,
    };
  }

  const payload = JSON.stringify({
    token,
    method,
    params,
    terminalId: process.env[MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR],
    projectId: process.env[MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR],
    sessionId: process.env[MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR],
  });

  return new Promise((resolve) => {
    const req = http.request({
      hostname: '127.0.0.1',
      port,
      path: MERGEN_BROWSER_MCP_ENDPOINT_PATH,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(payload),
      },
      timeout: 600_000,
    }, (res) => {
      let body = '';
      res.on('data', (chunk) => { body += chunk.toString(); });
      res.on('end', () => {
        try {
          const parsed = JSON.parse(body) as { ok?: boolean; result?: BrowserMcpRelayResult; error?: string };
          if (parsed.ok && parsed.result) {
            resolve(parsed.result);
          } else {
            resolve({ success: false, error: parsed.error || `Browser MCP relay failed with HTTP ${res.statusCode}` });
          }
        } catch {
          resolve({ success: false, error: `Browser MCP relay returned invalid JSON with HTTP ${res.statusCode}` });
        }
      });
    });
    req.on('timeout', () => {
      req.destroy();
      resolve({ success: false, error: 'Browser MCP relay timed out' });
    });
    req.on('error', (err) => {
      resolve({ success: false, error: `Browser MCP relay failed: ${err.message}` });
    });
    req.write(payload);
    req.end();
  });
}

function relayBrowserMcpRequestOverProcessIpc(method: string, params: unknown): Promise<BrowserMcpRelayResult> {
  return new Promise((resolve) => {
    if (!process.send) {
      resolve({ success: false, error: 'No Browser MCP IPC channel to main process' });
      return;
    }
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    pendingProcessRelays.set(id, resolve);
    process.send({ type: 'browserMcpRequest', id, method, params });
    setTimeout(() => {
      if (pendingProcessRelays.delete(id)) {
        resolve({ success: false, error: 'Browser MCP IPC relay timed out' });
      }
    }, 600_000);
  });
}
