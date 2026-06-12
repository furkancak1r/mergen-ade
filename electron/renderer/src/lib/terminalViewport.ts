export interface TerminalViewportScrollSnapshot {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  atBottom: boolean;
}

export function isTerminalViewportAtBottom(
  scrollTop: number,
  scrollHeight: number,
  clientHeight: number,
  tolerance = 2,
): boolean {
  return scrollTop + clientHeight >= scrollHeight - tolerance;
}

export function terminalViewportMaxScrollTop(scrollHeight: number, clientHeight: number): number {
  return Math.max(0, scrollHeight - clientHeight);
}

export function nextTerminalViewportScrollTop(
  snapshot: TerminalViewportScrollSnapshot,
  nextScrollHeight: number,
  nextClientHeight: number,
): number {
  const maxScrollTop = terminalViewportMaxScrollTop(nextScrollHeight, nextClientHeight);
  if (snapshot.atBottom) return maxScrollTop;
  return Math.min(Math.max(0, snapshot.scrollTop), maxScrollTop);
}
