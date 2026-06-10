import { createServer, type Server, type Socket } from 'net';
import { BrowserWindow } from 'electron';
import type { AiHookEvent, AiCliTool, AiCliStatus, AiCliAttentionKind } from '../shared/types';
import { AiCliTool as AiCliToolEnum, AiCliStatus as AiCliStatusEnum, AiCliAttentionKind as AiCliAttentionKindEnum } from '../shared/types';

let server: Server | null = null;
let serverPort = 0;

const hooksDir = () => {
  const appData = process.env.APPDATA || require('path').join(require('os').homedir(), 'AppData', 'Roaming');
  return require('path').join(appData, 'Mergen', 'MergenADE', 'hooks');
};

export function getHookServicePort(): number {
  return serverPort;
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
        if (buffer.includes('\r\n\r\n')) {
          handleHttpRequest(socket, buffer);
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

function handleHttpRequest(socket: Socket, data: string): void {
  const lines = data.split('\r\n');
  const firstLine = lines[0];
  if (!firstLine) return;

  const [method, path] = firstLine.split(' ');

  if (method === 'GET' && path === '/answer') {
    const answer = peekAnswer();
    if (answer) {
      const body = JSON.stringify({ requestId: answer.requestId, answers: answer.answers, rejected: answer.rejected });
      socket.write(`HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`);
    } else {
      const body = JSON.stringify({});
      socket.write(`HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`);
    }
  } else if (method === 'POST' && path === '/answer/ack') {
    ackAnswer();
    const body = JSON.stringify({ ok: true });
    socket.write(`HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`);
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
