export interface TooltipAnchorRect {
  top: number;
  right: number;
  bottom: number;
  left: number;
  width: number;
  height: number;
}

export type TooltipPlacement = 'top' | 'bottom';

export type TooltipHorizontalAlign = 'center' | 'left-edge' | 'right-edge';

export interface TooltipPositionOptions {
  viewportWidth: number;
  viewportHeight: number;
  preferredPlacement?: TooltipPlacement;
  horizontalAlign?: TooltipHorizontalAlign;
  maxWidth?: number;
  margin?: number;
  gap?: number;
  estimatedHeight?: number;
}

export interface TooltipPosition {
  left: number;
  top: number;
  maxWidth: number;
  placement: TooltipPlacement;
}

export function viewportTooltipMaxWidth(
  viewportWidth: number,
  margin = 8,
  maxWidth = 360,
): number {
  return Math.max(0, Math.min(maxWidth, viewportWidth - margin * 2));
}

export function viewportTooltipPosition(
  rect: TooltipAnchorRect,
  options: TooltipPositionOptions,
): TooltipPosition {
  const margin = options.margin ?? 8;
  const gap = options.gap ?? 6;
  const estimatedHeight = options.estimatedHeight ?? 28;
  const maxWidth = viewportTooltipMaxWidth(options.viewportWidth, margin, options.maxWidth ?? 360);
  const hAlign = options.horizontalAlign ?? 'center';

  let left: number;
  if (hAlign === 'left-edge') {
    left = clamp(rect.right + gap, margin, options.viewportWidth - margin);
  } else if (hAlign === 'right-edge') {
    left = clamp(rect.left - gap, margin, options.viewportWidth - margin);
  } else {
    const center = rect.left + rect.width / 2;
    const minLeft = margin + maxWidth / 2;
    const maxLeft = options.viewportWidth - margin - maxWidth / 2;
    left = maxWidth > 0
      ? clamp(center, Math.min(minLeft, maxLeft), Math.max(minLeft, maxLeft))
      : margin;
  }

  const topFits = rect.top - gap - estimatedHeight >= margin;
  const bottomFits = rect.bottom + gap + estimatedHeight <= options.viewportHeight - margin;
  const preferred = options.preferredPlacement ?? 'top';
  const placement: TooltipPlacement = preferred === 'bottom'
    ? (bottomFits || !topFits ? 'bottom' : 'top')
    : (topFits || !bottomFits ? 'top' : 'bottom');
  const top = placement === 'bottom'
    ? Math.min(rect.bottom + gap, options.viewportHeight - margin)
    : Math.max(rect.top - gap, margin);

  return { left, top, maxWidth, placement };
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
