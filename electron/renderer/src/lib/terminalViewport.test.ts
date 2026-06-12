import { describe, expect, it } from 'vitest';
import {
  isTerminalViewportAtBottom,
  nextTerminalViewportScrollTop,
  terminalViewportMaxScrollTop,
  type TerminalViewportScrollSnapshot,
} from './terminalViewport';

describe('terminalViewport', () => {
  it('detects bottom with a small tolerance', () => {
    expect(isTerminalViewportAtBottom(798, 1000, 200)).toBe(true);
    expect(isTerminalViewportAtBottom(790, 1000, 200)).toBe(false);
  });

  it('keeps bottom-pinned terminals at the bottom after output changes height', () => {
    const snapshot: TerminalViewportScrollSnapshot = {
      scrollTop: 800,
      scrollHeight: 1000,
      clientHeight: 200,
      atBottom: true,
    };

    expect(nextTerminalViewportScrollTop(snapshot, 1200, 200)).toBe(1000);
  });

  it('preserves a detached user scroll position after output', () => {
    const snapshot: TerminalViewportScrollSnapshot = {
      scrollTop: 300,
      scrollHeight: 1000,
      clientHeight: 200,
      atBottom: false,
    };

    expect(nextTerminalViewportScrollTop(snapshot, 1200, 200)).toBe(300);
  });

  it('clamps restored detached scroll when the scrollback shrinks', () => {
    const snapshot: TerminalViewportScrollSnapshot = {
      scrollTop: 900,
      scrollHeight: 1200,
      clientHeight: 200,
      atBottom: false,
    };

    expect(nextTerminalViewportScrollTop(snapshot, 600, 200)).toBe(400);
    expect(terminalViewportMaxScrollTop(150, 200)).toBe(0);
  });
});
