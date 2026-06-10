import type { AcpConfigOption, AcpAvailableCommand, AcpChatMessage } from '../../../shared/types';

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
  const lines = attachments.map((a) => `- ${a}`);
  const attachmentBlock = `Attached file paths:\n${lines.join('\n')}`;
  if (!text.trim()) return attachmentBlock;
  return `${text}\n\n${attachmentBlock}`;
}

export function removeMentionFromInput(input: string, mention: string): string {
  const idx = input.lastIndexOf(mention);
  if (idx < 0) return input;
  return input.slice(0, idx) + input.slice(idx + mention.length);
}
