export interface EditorSelectionState {
  dragActive: boolean;
  startX: number;
  startY: number;
  scrollOffset: number;
}

export function createEditorSelectionState(): EditorSelectionState {
  return { dragActive: false, startX: 0, startY: 0, scrollOffset: 0 };
}

export function startDrag(state: EditorSelectionState, x: number, y: number, scrollOffset: number): EditorSelectionState {
  return { ...state, dragActive: true, startX: x, startY: y, scrollOffset: scrollOffset };
}

export function endDrag(state: EditorSelectionState): EditorSelectionState {
  return { ...state, dragActive: false };
}

export function isInsideViewport(x: number, y: number, rect: { left: number; top: number; width: number; height: number }): boolean {
  return x >= rect.left && x <= rect.left + rect.width && y >= rect.top && y <= rect.top + rect.height;
}

export function clampScrollOffset(offset: number, maxScroll: number): number {
  return Math.max(0, Math.min(offset, maxScroll));
}
