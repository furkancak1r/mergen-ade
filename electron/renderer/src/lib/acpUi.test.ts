import { describe, expect, it } from 'vitest';
import {
  OPENCODE_ACP_CLOSE_TOOLTIP,
  OPENCODE_ACP_LABEL,
  OPENCODE_ACP_OPEN_BUTTON_LABEL,
  actionControlsEnabled,
  acpModeUiLabel,
  hasConfigSelectorOptions,
  nextAcpActivityState,
  openCodeAcpPanelTitle,
  openCodeAcpWelcomeText,
  shouldShowAcpWelcome,
  slashCommandHint,
  slashCommandHints,
} from './acpUi';

describe('acpUi', () => {
  it('uses OpenCode ACP as the canonical user-facing label', () => {
    expect(OPENCODE_ACP_LABEL).toBe('OpenCode ACP');
    expect(OPENCODE_ACP_OPEN_BUTTON_LABEL).toBe('+ ACP');
    expect(OPENCODE_ACP_CLOSE_TOOLTIP).toBe('Close OpenCode ACP');
    expect(openCodeAcpPanelTitle('Mergen')).toBe('OpenCode ACP - Mergen');
    expect(openCodeAcpWelcomeText()).toBe('Welcome to OpenCode ACP');
  });

  it('keeps ACP running true after promptSent events without a status field', () => {
    const next = nextAcpActivityState({ running: false, hasQueuedPrompts: false }, { type: 'promptSent' });
    expect(next.running).toBe(true);
  });

  it('keeps the previous running state for queued events without status', () => {
    const next = nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'queued', count: 2 });
    expect(next.running).toBe(true);
    expect(next.hasQueuedPrompts).toBe(true);
  });

  it('clears ACP running on terminal response and cancel events', () => {
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'promptResponse' }).running).toBe(false);
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'cancelled' }).running).toBe(false);
  });

  it('keeps ACP running for non-fatal stderr and warning events', () => {
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'stderr' }).running).toBe(true);
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'warning' }).running).toBe(true);
  });

  it('does not show welcome while queued prompts are visible', () => {
    expect(shouldShowAcpWelcome([], [{ text: 'queued' }])).toBe(false);
    expect(shouldShowAcpWelcome([], [])).toBe(true);
  });

  it('opens config selector only when model or effort options exist', () => {
    expect(hasConfigSelectorOptions(undefined, undefined)).toBe(false);
    expect(hasConfigSelectorOptions({ id: 'model', name: 'Model', category: 'model', currentValue: '', options: [] }, undefined)).toBe(false);
    expect(hasConfigSelectorOptions(undefined, { id: 'effort', name: 'Effort', category: 'effort', currentValue: '', options: [{ label: 'High', value: 'high' }] })).toBe(true);
  });

  it('enables action controls only after ACP session id exists', () => {
    expect(actionControlsEnabled(null)).toBe(false);
    expect(actionControlsEnabled({ sessionId: undefined })).toBe(false);
    expect(actionControlsEnabled({ sessionId: 'sess-1' })).toBe(true);
  });

  it('matches Rust ACP mode labels for composer and queued rows', () => {
    expect(acpModeUiLabel('plan')).toBe('Plan');
    expect(acpModeUiLabel('build')).toBeUndefined();
    expect(acpModeUiLabel(undefined)).toBeUndefined();
    expect(acpModeUiLabel('custom')).toBe('custom');
  });

  it('normalizes slash command hints from id or name', () => {
    expect(slashCommandHint({ id: 'init', name: 'Initialize' })).toBe('/init');
    expect(slashCommandHint({ name: 'plan' })).toBe('/plan');
    expect(slashCommandHint({ id: '/build' })).toBe('/build');
  });

  it('drops malformed slash commands instead of throwing', () => {
    expect(slashCommandHint({ id: undefined, name: undefined })).toBeUndefined();
    expect(slashCommandHint({ name: 'two words' })).toBeUndefined();
    expect(slashCommandHints(undefined, '')).toEqual([]);
    expect(slashCommandHints([null, {}, { id: 7 }, { name: 'run' }], '')).toEqual(['/run']);
  });

  it('filters slash command hints by id or name without requiring id', () => {
    const commands = [
      { name: 'init' },
      { id: 'apply', name: 'Build Apply' },
      { id: 'review', name: 'Review' },
    ];
    expect(slashCommandHints(commands, '')).toEqual(['/init', '/apply', '/review']);
    expect(slashCommandHints(commands, 'bu')).toEqual(['/apply']);
    expect(slashCommandHints(commands, 're')).toEqual(['/review']);
  });
});
