export interface TileGrid {
  rows: number;
  cols: number;
}

export function computeTileGrid(count: number, viewportWidth: number, viewportHeight: number): TileGrid {
  if (count === 0) {
    return { rows: 0, cols: 0 };
  }

  const safeWidth = Math.max(1, viewportWidth);
  const safeHeight = Math.max(1, viewportHeight);

  let best: TileGrid = { rows: count, cols: 1 };
  let bestScore = Infinity;

  for (let cols = 1; cols <= count; cols++) {
    const rows = Math.ceil(count / cols);
    const cellW = safeWidth / cols;
    const cellH = safeHeight / rows;

    const cellAspect = cellW / cellH;
    const targetAspect = 1.65;
    const aspectPenalty = Math.abs(cellAspect - targetAspect);
    const emptyCells = rows * cols - count;

    const score = aspectPenalty * 4.0 + emptyCells * 0.25 + rows * 0.01;
    if (score < bestScore) {
      bestScore = score;
      best = { rows, cols };
    }
  }

  return best;
}
