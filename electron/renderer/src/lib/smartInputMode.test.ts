import { describe, expect, it } from 'vitest';
import {
  normalizeSmartInputModeId,
  shouldSendOpenCodeModeToggle,
  smartInputModeLabel,
  toggleSmartInputModeId,
} from './smartInputMode';

describe('smartInputMode', () => {
  it('normalizes unknown modes to build', () => {
    expect(normalizeSmartInputModeId(undefined)).toBe('build');
    expect(normalizeSmartInputModeId('build')).toBe('build');
    expect(normalizeSmartInputModeId('default')).toBe('build');
    expect(normalizeSmartInputModeId('plan')).toBe('plan');
  });

  it('toggles between build and plan', () => {
    expect(toggleSmartInputModeId('build')).toBe('plan');
    expect(toggleSmartInputModeId('plan')).toBe('build');
  });

  it('shows a compact label only for plan queued tasks', () => {
    expect(smartInputModeLabel('plan')).toBe('Plan');
    expect(smartInputModeLabel('build')).toBeUndefined();
  });

  it('requests an OpenCode mode toggle only when target mode differs', () => {
    expect(shouldSendOpenCodeModeToggle(undefined, 'build')).toBe(false);
    expect(shouldSendOpenCodeModeToggle('build', 'plan')).toBe(true);
    expect(shouldSendOpenCodeModeToggle('plan', 'build')).toBe(true);
    expect(shouldSendOpenCodeModeToggle('plan', 'plan')).toBe(false);
  });
});
