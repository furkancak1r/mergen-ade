import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ShellKind, TerminalKind } from '../../../shared/types';

vi.mock('electron', () => ({
  BrowserWindow: { getAllWindows: () => [] },
}));

vi.mock('node-pty', () => ({
  spawn: vi.fn(() => ({
    onData: vi.fn(),
    onExit: vi.fn(),
    write: vi.fn(),
    resize: vi.fn(),
    kill: vi.fn(),
  })),
}));

import { spawn } from 'node-pty';
import { createTerminal } from '../../../main/pty';

const originalAppData = process.env.APPDATA;
let tempDir: string | undefined;

describe('PTY environment', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-electron-pty-'));
    process.env.APPDATA = tempDir;
    vi.mocked(spawn).mockClear();
  });

  afterEach(() => {
    if (originalAppData === undefined) {
      delete process.env.APPDATA;
    } else {
      process.env.APPDATA = originalAppData;
    }
    if (tempDir) fs.rmSync(tempDir, { recursive: true, force: true });
    tempDir = undefined;
  });

  it('injects Factory Droid hook context into every terminal', () => {
    createTerminal({
      shell: ShellKind.PowerShell,
      cwd: tempDir!,
      cols: 80,
      rows: 24,
      terminalId: 77,
      projectId: 1,
      kind: TerminalKind.Foreground,
    });

    const opts = vi.mocked(spawn).mock.calls[0][2] as { env: Record<string, string> };
    expect(opts.env.MERGEN_TERMINAL_ID).toBe('77');
    expect(opts.env.MERGEN_ADE_TERMINAL_ID).toBe('77');
    expect(opts.env.MERGEN_ADE_FACTORY_DROID_HOOKS_DIR).toBe(path.join(tempDir!, 'Mergen', 'MergenADE', 'hooks'));
  });
});
