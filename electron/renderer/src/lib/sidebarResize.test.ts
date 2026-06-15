import { describe, expect, it } from 'vitest';
import { clampPanelWidth, panelWidthFromPointerDrag } from './sidebarResize';

describe('sidebarResize', () => {
  it('keeps width unchanged when the pointer does not move', () => {
    expect(panelWidthFromPointerDrag({
      pointerX: 360,
      startPointerX: 360,
      startWidth: 312,
      minWidth: 200,
      maxWidth: 500,
    })).toBe(312);
  });

  it('resizes a left sidebar from pointer delta instead of absolute screen x', () => {
    expect(panelWidthFromPointerDrag({
      pointerX: 380,
      startPointerX: 360,
      startWidth: 312,
      minWidth: 200,
      maxWidth: 500,
    })).toBe(332);
  });

  it('resizes a right-side panel in the opposite direction', () => {
    expect(panelWidthFromPointerDrag({
      pointerX: 330,
      startPointerX: 360,
      startWidth: 520,
      minWidth: 240,
      maxWidth: 800,
      direction: 'left',
    })).toBe(550);
  });

  it('clamps panel widths to configured bounds', () => {
    expect(clampPanelWidth(100, 200, 500)).toBe(200);
    expect(clampPanelWidth(700, 200, 500)).toBe(500);
  });
});
