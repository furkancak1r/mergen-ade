import type { AcpConfigOption, AcpAvailableCommand, AcpChatMessage } from '../../../shared/types';
import { repairMojibakeDisplay } from './mojibake';

export interface AcpParsedMessage {
  type: 'initialized' | 'sessionCreated' | 'promptResponse' | 'configOptions' | 'modeUpdate' | 'commands' | 'permission' | 'raw' | 'unknown';
  sessionId?: string;
  text?: string;
  options?: AcpConfigOption[];
  modeId?: string;
  commands?: AcpAvailableCommand[];
  requestId?: string | number;
  message?: string;
  raw?: Record<string, unknown>;
}

export function parseAcpLine(line: string): AcpParsedMessage | undefined {
  try {
    const msg = JSON.parse(line) as Record<string, unknown>;
    if (msg.method === 'initialized') {
      return { type: 'initialized' };
    } else if (msg.method === 'session/new') {
      const result = msg.result as Record<string, unknown>;
      return {
        type: 'sessionCreated',
        sessionId: (result.sessionId as string) || undefined,
        options: (result.configOptions as AcpConfigOption[]) || undefined,
      };
    } else if (msg.method === 'session/prompt') {
      const result = msg.result as Record<string, unknown>;
      return {
        type: 'promptResponse',
        text: (result.text as string) || '',
      };
    } else if (msg.method === 'config_option_update') {
      const params = msg.params as Record<string, unknown>;
      return {
        type: 'configOptions',
        options: (params.configOptions as AcpConfigOption[]) || undefined,
      };
    } else if (msg.method === 'current_mode_update') {
      const params = msg.params as Record<string, unknown>;
      return {
        type: 'modeUpdate',
        modeId: (params.currentModeId as string) || (params.modeId as string) || undefined,
      };
    } else if (msg.method === 'available_commands_update') {
      const params = msg.params as Record<string, unknown>;
      return {
        type: 'commands',
        commands: (params.availableCommands as AcpAvailableCommand[]) || (params.commands as AcpAvailableCommand[]) || undefined,
      };
    } else if (msg.method === 'permission_request') {
      const params = msg.params as Record<string, unknown>;
      return {
        type: 'permission',
        requestId: params.requestId as string | number,
        message: params.message as string,
      };
    } else {
      return { type: 'raw', raw: msg };
    }
  } catch {
    return undefined;
  }
}

export function buildAcpPromptText(text: string, attachments: string[]): string {
  if (attachments.length === 0) return text;
  const attachmentBlock = `Attached file paths:\n${attachments.join('\n')}`;
  if (text.length === 0) return attachmentBlock;
  return `${text}\n\n${attachmentBlock}`;
}

export function pathToMention(path: string): string {
  const parts = path.split(/[/\\]/);
  const fileName = parts[parts.length - 1] || path;
  return `@${repairMojibakeDisplay(fileName)}`;
}

export function appendMentionsToInput(input: string, paths: string[]): string {
  if (paths.length === 0) return input;
  const mentions = paths.map(pathToMention).join(' ');
  if (input.length === 0) return mentions;
  return `${input} ${mentions}`;
}

export function removeMentionFromInput(input: string, mention: string): string {
  const idx = input.lastIndexOf(mention);
  if (idx < 0) return input;
  const before = input.slice(0, idx);
  const after = input.slice(idx + mention.length);
  if (before.length > 0 && before.endsWith(' ')) {
    return before.slice(0, -1) + after;
  }
  return before + after;
}
