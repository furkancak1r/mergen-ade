import { createServer, type Server, type Socket } from 'net';
import crypto from 'crypto';
import { BrowserWindow } from 'electron';
import type { AiHookEvent, AiCliTool, AiCliStatus, AiCliAttentionKind } from '../shared/types';
import { AiCliTool as AiCliToolEnum, AiCliStatus as AiCliStatusEnum, AiCliAttentionKind as AiCliAttentionKindEnum } from '../shared/types';
import { MERGEN_BROWSER_MCP_ENDPOINT_PATH } from './browserMcpTools';

let server: Server | null = null;
let serverPort = 0;
const browserMcpToken = crypto.randomBytes(24).toString('hex');

type BrowserMcpHttpHandler = (body: Record<string, unknown>) => Promise<unknown> | unknown;
let browserMcpHandler: BrowserMcpHttpHandler | null = null;

const hooksDir = () => {
  const appData = process.env.APPDATA || require('path').join(require('os').homedir(), 'AppData', 'Roaming');
  return require('path').join(appData, 'Mergen', 'MergenADE', 'hooks');
};

export function getHookServicePort(): number {
  return serverPort;
}

export function getBrowserMcpToken(): string {
  return browserMcpToken;
}

export function registerBrowserMcpHandler(handler: BrowserMcpHttpHandler): void {
  browserMcpHandler = handler;
}

export function getHookInboxDir(): string {
  const dir = hooksDir();
  require('fs').mkdirSync(dir, { recursive: true });
  return dir;
}

interface OpenCodeQuestionAnswer {
  requestId: string;
  answers: string[];
  rejected: boolean;
}

const pendingAnswers: OpenCodeQuestionAnswer[] = [];

export function submitAnswer(answer: OpenCodeQuestionAnswer): void {
  pendingAnswers.push(answer);
}

export function peekAnswer(): OpenCodeQuestionAnswer | undefined {
  return pendingAnswers[0];
}

export function ackAnswer(): void {
  pendingAnswers.shift();
}

export function startHookService(): void {
  if (server) return;

  // Use TCP server only (named pipe on Windows conflicts with legacy Rust app)
  server = createServer((socket) => {
    let buffer = '';
    socket.on('data', (data) => {
      buffer += data.toString();
      // Check if this looks like an HTTP request
      if (buffer.startsWith('GET ') || buffer.startsWith('POST ')) {
        // Wait for the full HTTP request to arrive before parsing
        if (isHttpRequestComplete(buffer)) {
          void handleHttpRequest(socket, buffer);
          buffer = '';
        }
        return;
      }
      // Otherwise process as newline-delimited JSON events
      let idx;
      while ((idx = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 1);
        if (line.trim()) {
          try {
            const event = JSON.parse(line) as AiHookEvent;
            processHookEvent(event);
          } catch {
            // ignore malformed JSON
          }
        }
      }
    });
  });

  server.listen(0, '127.0.0.1', () => {
    const addr = server!.address();
    if (addr && typeof addr === 'object') {
      serverPort = addr.port;
      console.log(`Hook TCP service listening on port ${serverPort}`);
    }
  });
}

function isHttpRequestComplete(data: string): boolean {
  const headerEnd = data.indexOf('\r\n\r\n');
  if (headerEnd < 0) return false;
  const headers = data.slice(0, headerEnd).split('\r\n');
  const contentLengthHeader = headers.find((line) => line.toLowerCase().startsWith('content-length:'));
  if (!contentLengthHeader) return true;
  const contentLength = Number(contentLengthHeader.slice(contentLengthHeader.indexOf(':') + 1).trim());
  if (!Number.isFinite(contentLength) || contentLength <= 0) return true;
  return Buffer.byteLength(data.slice(headerEnd + 4)) >= contentLength;
}

function httpRequestBody(data: string): string {
  const headerEnd = data.indexOf('\r\n\r\n');
  if (headerEnd < 0) return '';
  return data.slice(headerEnd + 4);
}

function writeJsonResponse(socket: Socket, status: number, bodyValue: unknown): void {
  const body = JSON.stringify(bodyValue);
  const reason = status === 200 ? 'OK' : status === 403 ? 'Forbidden' : status === 404 ? 'Not Found' : 'Internal Server Error';
  socket.write(`HTTP/1.1 ${status} ${reason}\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`);
}

async function handleHttpRequest(socket: Socket, data: string): Promise<void> {
  const lines = data.split('\r\n');
  const firstLine = lines[0];
  if (!firstLine) return;

  const [method, path] = firstLine.split(' ');

  if (method === 'GET' && path === '/answer') {
    const answer = peekAnswer();
    if (answer) {
      writeJsonResponse(socket, 200, { requestId: answer.requestId, answers: answer.answers, rejected: answer.rejected });
    } else {
      writeJsonResponse(socket, 200, {});
    }
  } else if (method === 'POST' && path === '/answer/ack') {
    ackAnswer();
    writeJsonResponse(socket, 200, { ok: true });
  } else if (method === 'POST' && path === MERGEN_BROWSER_MCP_ENDPOINT_PATH) {
    try {
      const body = JSON.parse(httpRequestBody(data)) as Record<string, unknown>;
      if (body.token !== browserMcpToken) {
        writeJsonResponse(socket, 403, { ok: false, error: 'Invalid Browser MCP token' });
      } else if (!browserMcpHandler) {
        writeJsonResponse(socket, 500, { ok: false, error: 'Browser MCP handler is not registered' });
      } else {
        const result = await browserMcpHandler(body);
        writeJsonResponse(socket, 200, { ok: true, result });
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      writeJsonResponse(socket, 500, { ok: false, error: message });
    }
  } else {
    socket.write('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n');
  }
  socket.end();
}

function processHookEvent(event: AiHookEvent): void {
  // Broadcast to all renderer windows
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send('hook:status', event);
    }
  }
}

export function parseStatusRequest(body: string): AiHookEvent | null {
  try {
    const parsed = JSON.parse(body) as Record<string, unknown>;
    const eventType = (parsed.type as string) || '';
    const tool = parseTool(eventType);
    if (!tool) return null;

    const status = parseStatus(eventType, parsed);
    const attentionKind = parseAttentionKind(eventType, parsed);

    // Parse question payload from question.asked events
    let question: AiHookEvent['question'] | undefined;
    if (eventType.includes('question.asked') && parsed.question) {
      const q = parsed.question as Record<string, unknown>;
      question = {
        header: (q.header as string) || '',
        question: (q.question as string) || '',
        options: (q.options as { id: string; label: string }[]) || [],
        multiple: (q.multiple as boolean) || false,
        custom: (q.custom as boolean) || false,
        requestId: (q.requestId as string) || '',
        sessionId: (q.sessionId as string) || '',
      };
    }

    return {
      terminalId: (parsed.terminalId as number) || 0,
      tool,
      status,
      reason: (parsed.reason as string) || undefined,
      attentionKind,
      rawJson: body,
      eventKind: eventType,
      question,
    };
  } catch {
    return null;
  }
}

function parseTool(eventType: string): AiCliTool | null {
  if (eventType.startsWith('droid-hook:') || eventType.startsWith('factory-droid-hook:')) return AiCliToolEnum.Droid;
  if (eventType.startsWith('codex-hook:')) return AiCliToolEnum.Codex;
  if (eventType.startsWith('opencode-hook:') || eventType.startsWith('opencode-notify:')) return AiCliToolEnum.OpenCode;
  // Claude uses title-based detection only; no hook support
  return null;
}

function parseStatus(eventType: string, parsed: Record<string, unknown>): AiCliStatus {
  if (eventType.includes('UserPromptSubmit') || eventType.includes('PreToolUse') || eventType.includes('PostToolUse')) {
    return AiCliStatusEnum.Running;
  }
  if (eventType.includes('PermissionRequest') || eventType.includes('QuestionAsked') || eventType.includes('UserInputRequested') || eventType.includes('permission.asked') || eventType.includes('plan_mode_prompt')) {
    return AiCliStatusEnum.Attention;
  }
  // Codex/OpenCode Stop uses debounce-to-turn-complete; map to Attention with TurnComplete kind
  if (eventType.includes('Stop')) {
    return AiCliStatusEnum.Attention;
  }
  if (eventType.includes('TurnComplete')) {
    return AiCliStatusEnum.Attention;
  }
  if (eventType.includes('Idle')) {
    return AiCliStatusEnum.Inactive;
  }
  return AiCliStatusEnum.Inactive;
}

function parseAttentionKind(eventType: string, parsed: Record<string, unknown>): AiCliAttentionKind | undefined {
  if (eventType.includes('PermissionRequest') || eventType.includes('permission.asked')) return AiCliAttentionKindEnum.Permission;
  if (eventType.includes('TurnComplete')) return AiCliAttentionKindEnum.TurnComplete;
  if (eventType.includes('SessionError')) return AiCliAttentionKindEnum.SessionError;
  if (eventType.includes('QuestionAsked') || eventType.includes('UserInputRequested') || eventType.includes('question.asked')) return AiCliAttentionKindEnum.UserInputRequested;
  if (eventType.includes('plan_mode_prompt')) return AiCliAttentionKindEnum.PlanModePrompt;
  // Codex/OpenCode Stop → Attention with TurnComplete kind (debounced downstream)
  if (eventType.includes('Stop')) return AiCliAttentionKindEnum.TurnComplete;
  return undefined;
}

export function stopHookService(): void {
  if (server) {
    server.close();
    server = null;
  }
}
