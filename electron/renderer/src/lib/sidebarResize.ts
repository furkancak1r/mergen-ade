export interface PanelWidthFromPointerDragOptions {
  pointerX: number;
  startPointerX: number;
  startWidth: number;
  minWidth: number;
  maxWidth: number;
  direction?: 'left' | 'right';
}

export function clampPanelWidth(width: number, minWidth: number, maxWidth: number): number {
  return Math.max(minWidth, Math.min(maxWidth, width));
}

export function panelWidthFromPointerDrag(options: PanelWidthFromPointerDragOptions): number {
  const delta = options.pointerX - options.startPointerX;
  const direction = options.direction ?? 'right';
  const nextWidth = direction === 'left'
    ? options.startWidth - delta
    : options.startWidth + delta;
  return clampPanelWidth(nextWidth, options.minWidth, options.maxWidth);
}
