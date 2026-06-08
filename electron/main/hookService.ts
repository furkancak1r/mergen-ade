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

export function startHookService(): void {
  if (server) return;

  const dir = getHookInboxDir();
  const socketPath = require('path').join(dir, 'mergen-ade.sock');

  // Try to remove stale socket
  try {
    require('fs').unlinkSync(socketPath);
  } catch {}

  server = createServer((socket) => {
    let buffer = '';
    socket.on('data', (data) => {
      buffer += data.toString();
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

  server.listen(socketPath, () => {
    console.log(`Hook service listening on ${socketPath}`);
  });

  // Also start a TCP server for compatibility
  const tcpServer = createServer((socket) => {
    let buffer = '';
    socket.on('data', (data) => {
      buffer += data.toString();
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

  tcpServer.listen(0, '127.0.0.1', () => {
    const addr = tcpServer.address();
    if (addr && typeof addr === 'object') {
      serverPort = addr.port;
      console.log(`Hook TCP service listening on port ${serverPort}`);
    }
  });
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

    return {
      terminalId: (parsed.terminalId as number) || 0,
      tool,
      status,
      reason: (parsed.reason as string) || undefined,
      attentionKind,
      rawJson: body,
      eventKind: eventType,
    };
  } catch {
    return null;
  }
}

function parseTool(eventType: string): AiCliTool | null {
  if (eventType.startsWith('droid-hook:') || eventType.startsWith('factory-droid-hook:')) return AiCliToolEnum.Droid;
  if (eventType.startsWith('codex-hook:')) return AiCliToolEnum.Codex;
  if (eventType.startsWith('opencode-hook:') || eventType.startsWith('opencode-notify:')) return AiCliToolEnum.OpenCode;
  if (eventType.startsWith('claude-hook:')) return AiCliToolEnum.Claude;
  return null;
}

function parseStatus(eventType: string, parsed: Record<string, unknown>): AiCliStatus {
  if (eventType.includes('UserPromptSubmit') || eventType.includes('PreToolUse') || eventType.includes('PostToolUse')) {
    return AiCliStatusEnum.Running;
  }
  if (eventType.includes('Stop') || eventType.includes('PermissionRequest') || eventType.includes('QuestionAsked')) {
    return AiCliStatusEnum.Attention;
  }
  if (eventType.includes('Idle') || eventType.includes('TurnComplete')) {
    return AiCliStatusEnum.Inactive;
  }
  return AiCliStatusEnum.Inactive;
}

function parseAttentionKind(eventType: string, parsed: Record<string, unknown>): AiCliAttentionKind | undefined {
  if (eventType.includes('PermissionRequest')) return AiCliAttentionKindEnum.Permission;
  if (eventType.includes('TurnComplete')) return AiCliAttentionKindEnum.TurnComplete;
  if (eventType.includes('SessionError')) return AiCliAttentionKindEnum.SessionError;
  if (eventType.includes('QuestionAsked')) return AiCliAttentionKindEnum.UserInputRequested;
  return undefined;
}

export function stopHookService(): void {
  if (server) {
    server.close();
    server = null;
  }
}
