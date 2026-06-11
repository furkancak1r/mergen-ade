import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { renderClaudeCodexPlanFile } from '../shared/claudeCodexHook';
import { updateClaudeCodexUiVerification } from './claudeCodexHook';

let tempDir: string | undefined;

describe('claude codex hook main helpers', () => {
  beforeEach(() => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-claude-codex-'));
  });

  afterEach(() => {
    if (tempDir) fs.rmSync(tempDir, { recursive: true, force: true });
    tempDir = undefined;
  });

  it('updates the UI verification note in an existing plan file', () => {
    const planPath = path.join(tempDir!, '.claude', 'plans', 's1.md');
    fs.mkdirSync(path.dirname(planPath), { recursive: true });
    fs.writeFileSync(planPath, renderClaudeCodexPlanFile({
      sessionId: 's1',
      status: 'done',
      originalPrompt: 'build UI',
      plan: 'change component',
      uiChangedFiles: ['electron/renderer/src/App.tsx'],
      uiVerification: 'UI verification pending: UI-facing changed files were detected.',
      finalNote: 'Codex review reported no actionable P0-P3 findings.',
    }), 'utf-8');

    expect(updateClaudeCodexUiVerification({
      planPath,
      note: 'UI verification queued: Browser panel opened.',
    })).toBe(true);

    const text = fs.readFileSync(planPath, 'utf-8');
    expect(text).toContain('UI verification queued: Browser panel opened.');
    expect(text).not.toContain('UI verification pending');
  });
});
