import { describe, it, expect } from 'vitest';
import { computeTileGrid } from './layout';

describe('layout', () => {
  it('single terminal uses single cell', () => {
    const grid = computeTileGrid(1, 1920, 1080);
    expect(grid.rows).toBe(1);
    expect(grid.cols).toBe(1);
  });

  it('grid always has enough cells', () => {
    for (let count = 1; count <= 20; count++) {
      const grid = computeTileGrid(count, 1920, 1080);
      expect(grid.rows * grid.cols).toBeGreaterThanOrEqual(count);
    }
  });

  it('tall viewport prefers more rows', () => {
    const grid = computeTileGrid(6, 900, 1600);
    expect(grid.rows).toBeGreaterThanOrEqual(2);
  });

  it('wide viewport prefers more columns', () => {
    const grid = computeTileGrid(6, 2200, 900);
    expect(grid.cols).toBeGreaterThanOrEqual(2);
  });

  it('empty input has zero grid', () => {
    const grid = computeTileGrid(0, 1000, 700);
    expect(grid.rows).toBe(0);
    expect(grid.cols).toBe(0);
  });
});
