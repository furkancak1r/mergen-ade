import { describe, expect, it, vi } from 'vitest';

vi.mock('electron', () => ({
  BrowserWindow: { getAllWindows: () => [] },
}));

import { AiCliAttentionKind, AiCliStatus, AiCliTool } from '../../../shared/types';
import { parseStatusRequest } from '../../../main/hookService';

describe('hook service status parsing', () => {
  it('normalizes string terminal ids from plugin HTTP events', () => {
    expect(parseStatusRequest(JSON.stringify({
      type: 'opencode-hook:permission.asked',
      terminalId: '42',
      rawJson: '{}',
    }))).toMatchObject({
      terminalId: 42,
      tool: AiCliTool.OpenCode,
      status: AiCliStatus.Attention,
      attentionKind: AiCliAttentionKind.Permission,
    });
  });

  it('maps Factory Droid notifications to attention states', () => {
    expect(parseStatusRequest(JSON.stringify({
      type: 'factory-droid-hook:Notification',
      terminalId: '7',
      reason: 'idle_prompt',
    }))).toMatchObject({
      terminalId: 7,
      tool: AiCliTool.Droid,
      status: AiCliStatus.Attention,
      attentionKind: AiCliAttentionKind.UserInputRequested,
    });
  });
});
