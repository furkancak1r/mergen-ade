import { describe, it, expect } from 'vitest';
import {
  createEditorSelectionState,
  startDrag,
  endDrag,
  isInsideViewport,
  clampScrollOffset,
} from './editorSelection';

describe('editorSelection', () => {
  it('creates initial state', () => {
    const state = createEditorSelectionState();
    expect(state.dragActive).toBe(false);
    expect(state.startX).toBe(0);
    expect(state.startY).toBe(0);
    expect(state.scrollOffset).toBe(0);
  });

  it('startDrag activates drag and stores coordinates', () => {
    const state = createEditorSelectionState();
    const next = startDrag(state, 100, 200, 50);
    expect(next.dragActive).toBe(true);
    expect(next.startX).toBe(100);
    expect(next.startY).toBe(200);
    expect(next.scrollOffset).toBe(50);
  });

  it('endDrag deactivates drag', () => {
    const state = startDrag(createEditorSelectionState(), 10, 20, 0);
    const next = endDrag(state);
    expect(next.dragActive).toBe(false);
  });

  it('isInsideViewport returns true for point inside', () => {
    const rect = { left: 0, top: 0, width: 100, height: 100 };
    expect(isInsideViewport(50, 50, rect)).toBe(true);
    expect(isInsideViewport(0, 0, rect)).toBe(true);
    expect(isInsideViewport(100, 100, rect)).toBe(true);
  });

  it('isInsideViewport returns false for point outside', () => {
    const rect = { left: 10, top: 10, width: 100, height: 100 };
    expect(isInsideViewport(5, 50, rect)).toBe(false);
    expect(isInsideViewport(50, 5, rect)).toBe(false);
    expect(isInsideViewport(200, 200, rect)).toBe(false);
  });

  it('clampScrollOffset clamps to 0 minimum', () => {
    expect(clampScrollOffset(-10, 100)).toBe(0);
  });

  it('clampScrollOffset clamps to max', () => {
    expect(clampScrollOffset(150, 100)).toBe(100);
  });

  it('clampScrollOffset preserves value in range', () => {
    expect(clampScrollOffset(50, 100)).toBe(50);
  });
});
