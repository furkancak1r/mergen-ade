import { describe, expect, it } from 'vitest';
import {
  actionControlsEnabled,
  hasConfigSelectorOptions,
  nextAcpActivityState,
  shouldShowAcpWelcome,
} from './acpUi';

describe('acpUi', () => {
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
});
