export const TERMINAL_INPUT_BATCH_FLUSH_MS = 4;
export const TERMINAL_INPUT_BATCH_MAX_CHARS = 64;

export function shouldFlushTerminalInput(
  latestChunk: string,
  queuedLength: number,
  maxChars = TERMINAL_INPUT_BATCH_MAX_CHARS,
): boolean {
  if (latestChunk.length === 0) return false;
  if (/[\r\n\x03\x04\x1b]/.test(latestChunk)) return true;
  return queuedLength >= maxChars;
}
