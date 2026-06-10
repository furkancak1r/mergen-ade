import { describe, it, expect } from 'vitest';
import {
  isStaleOpencodeCompletion,
  canAutoDispatch,
  canAutoDispatchClaude,
  effectiveAiStatusForDisplay,
  smartInputFooterHeight,
  selectionEdgeAutoscrollDelta,
  shouldShowSmartInputFooter,
  SMART_INPUT_AUTO_DISPATCH_SETTLE_MS,
} from './smartInput';

describe('smartInput', () => {
  describe('isStaleOpencodeCompletion', () => {
    it('returns false when no submit timestamp', () => {
      expect(isStaleOpencodeCompletion(undefined, 1000)).toBe(false);
    });

    it('returns true when within settle window', () => {
      const now = 1000;
      expect(isStaleOpencodeCompletion(now - 100, now)).toBe(true);
    });

    it('returns false after settle window expires', () => {
      const now = 1000;
      expect(isStaleOpencodeCompletion(now - SMART_INPUT_AUTO_DISPATCH_SETTLE_MS - 1, now)).toBe(false);
    });
  });

  describe('canAutoDispatch', () => {
    it('allows dispatch when idle and no guards', () => {
      const result = canAutoDispatch(
        [{ text: 'task', attachments: [] }],
        'Idle',
        'attention',
        'TurnComplete',
        false,
        undefined,
        false,
        false,
        Date.now(),
      );
      expect(result).toBe(true);
    });

    it('blocks dispatch when queue is empty', () => {
      const result = canAutoDispatch([], 'Idle', 'attention', 'TurnComplete', false, undefined, false, false, Date.now());
      expect(result).toBe(false);
    });

    it('blocks dispatch when question is pending', () => {
      const result = canAutoDispatch(
        [{ text: 't', attachments: [] }],
        'Idle',
        'attention',
        'TurnComplete',
        true,
        undefined,
        false,
        false,
        Date.now(),
      );
      expect(result).toBe(false);
    });

    it('blocks dispatch when thought loop is blocked', () => {
      const result = canAutoDispatch(
        [{ text: 't', attachments: [] }],
        'Idle',
        'attention',
        'TurnComplete',
        false,
        undefined,
        true,
        false,
        Date.now(),
      );
      expect(result).toBe(false);
    });

    it('blocks dispatch when loop limit is emitted', () => {
      const result = canAutoDispatch(
        [{ text: 't', attachments: [] }],
        'Idle',
        'attention',
        'TurnComplete',
        false,
        undefined,
        false,
        true,
        Date.now(),
      );
      expect(result).toBe(false);
    });

    it('blocks dispatch when not idle and no turn complete', () => {
      const result = canAutoDispatch(
        [{ text: 't', attachments: [] }],
        'Working',
        'running',
        'PromptSubmit',
        false,
        undefined,
        false,
        false,
        Date.now(),
      );
      expect(result).toBe(false);
    });

    it('blocks dispatch during settle guard', () => {
      const now = Date.now();
      const result = canAutoDispatch(
        [{ text: 't', attachments: [] }],
        'Idle',
        'attention',
        'TurnComplete',
        false,
        now - 100,
        false,
        false,
        now,
      );
      expect(result).toBe(false);
    });
  });

  describe('canAutoDispatchClaude', () => {
    it('allows Claude queue dispatch on turn-complete attention', () => {
      expect(canAutoDispatchClaude(
        [{ text: 'task', attachments: [] }],
        'attention',
        'turn_complete',
        undefined,
        Date.now(),
      )).toBe(true);
    });

    it('blocks Claude queue dispatch for running, permission, empty queue, and settle guard', () => {
      const queue = [{ text: 'task', attachments: [] }];
      const now = Date.now();
      expect(canAutoDispatchClaude(queue, 'running', undefined, undefined, now)).toBe(false);
      expect(canAutoDispatchClaude(queue, 'attention', 'permission', undefined, now)).toBe(false);
      expect(canAutoDispatchClaude([], 'attention', 'turn_complete', undefined, now)).toBe(false);
      expect(canAutoDispatchClaude(queue, 'attention', 'turn_complete', now - 100, now)).toBe(false);
    });
  });

  describe('shouldShowSmartInputFooter', () => {
    it('shows Smart Input for active OpenCode and Claude foreground terminals', () => {
      expect(shouldShowSmartInputFooter('foreground', 'opencode', 'running', true)).toBe(true);
      expect(shouldShowSmartInputFooter('foreground', 'claude', 'running', false)).toBe(true);
      expect(shouldShowSmartInputFooter('foreground', 'claude', 'attention', false)).toBe(true);
      expect(shouldShowSmartInputFooter('foreground', 'claude', 'inactive', false, true)).toBe(true);
    });

    it('hides Smart Input for inactive/background terminals and inactive OpenCode sessions', () => {
      expect(shouldShowSmartInputFooter('background', 'claude', 'attention', false)).toBe(false);
      expect(shouldShowSmartInputFooter('foreground', 'claude', 'inactive', false)).toBe(false);
      expect(shouldShowSmartInputFooter('foreground', 'opencode', 'attention', false)).toBe(false);
      expect(shouldShowSmartInputFooter('foreground', 'codex', 'attention', false)).toBe(false);
    });
  });

  describe('effectiveAiStatusForDisplay', () => {
    it('shows a pending Claude launcher as running without changing stored status semantics', () => {
      expect(effectiveAiStatusForDisplay('claude', 'inactive', true)).toBe('running');
      expect(effectiveAiStatusForDisplay('claude', 'attention', true)).toBe('attention');
      expect(effectiveAiStatusForDisplay('opencode', 'inactive', true)).toBe('inactive');
      expect(effectiveAiStatusForDisplay(undefined, undefined, true)).toBe('inactive');
    });
  });

  describe('smartInputFooterHeight', () => {
    it('uses computed height when no user override', () => {
      const h = smartInputFooterHeight(2, true, true, 60, undefined, 100, 300);
      expect(h).toBeGreaterThanOrEqual(100);
      expect(h).toBeLessThanOrEqual(300);
    });

    it('clamps user height to safe range', () => {
      expect(smartInputFooterHeight(2, true, true, 60, 50, 100, 300)).toBe(100);
      expect(smartInputFooterHeight(2, true, true, 60, 400, 100, 300)).toBe(300);
    });

    it('returns at least safeMin', () => {
      expect(smartInputFooterHeight(0, false, false, 60, undefined, 80, 200)).toBe(80);
    });
  });

  describe('selectionEdgeAutoscrollDelta', () => {
    it('returns 0 when pointer is in safe zone', () => {
      const delta = selectionEdgeAutoscrollDelta(100, 0, 200, 20);
      expect(delta).toBe(0);
    });

    it('returns negative delta when near top edge', () => {
      const delta = selectionEdgeAutoscrollDelta(10, 0, 200, 20);
      expect(delta).toBeLessThan(0);
    });

    it('returns positive delta when near bottom edge', () => {
      const delta = selectionEdgeAutoscrollDelta(190, 0, 200, 20);
      expect(delta).toBeGreaterThan(0);
    });

    it('scales speed with distance from edge', () => {
      const delta1 = selectionEdgeAutoscrollDelta(0, 0, 200, 20);
      const delta2 = selectionEdgeAutoscrollDelta(30, 0, 200, 20);
      expect(Math.abs(delta1)).toBeGreaterThanOrEqual(Math.abs(delta2));
    });

    it('clamps speed to max 8 lines', () => {
      const delta = selectionEdgeAutoscrollDelta(-1000, 0, 200, 20);
      expect(Math.abs(delta)).toBe(160); // 8 * 20
    });
  });
});
