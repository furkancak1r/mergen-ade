import { describe, expect, it } from 'vitest';
import { viewportTooltipPosition } from './tooltipPosition';

describe('viewportTooltipPosition', () => {
  it('clamps a tooltip near the right edge into the viewport', () => {
    const pos = viewportTooltipPosition(
      { left: 780, right: 800, top: 80, bottom: 108, width: 20, height: 28 },
      { viewportWidth: 800, viewportHeight: 600, maxWidth: 320 },
    );

    expect(pos.left + pos.maxWidth / 2).toBeLessThanOrEqual(792);
  });

  it('moves to the bottom when there is not enough room above', () => {
    const pos = viewportTooltipPosition(
      { left: 20, right: 60, top: 4, bottom: 32, width: 40, height: 28 },
      { viewportWidth: 800, viewportHeight: 600, preferredPlacement: 'top' },
    );

    expect(pos.placement).toBe('bottom');
    expect(pos.top).toBeGreaterThan(32);
  });

  it('left-edge alignment places tooltip at anchor right edge plus gap', () => {
    const pos = viewportTooltipPosition(
      { left: 4, right: 44, top: 100, bottom: 140, width: 40, height: 40 },
      { viewportWidth: 1200, viewportHeight: 800, horizontalAlign: 'left-edge', gap: 6 },
    );

    expect(pos.left).toBe(50);
  });

  it('left-edge alignment clamps when anchor is near viewport right edge', () => {
    const pos = viewportTooltipPosition(
      { left: 1180, right: 1200, top: 100, bottom: 140, width: 20, height: 40 },
      { viewportWidth: 1200, viewportHeight: 800, horizontalAlign: 'left-edge', margin: 8 },
    );

    expect(pos.left).toBeLessThanOrEqual(1192);
  });
});
