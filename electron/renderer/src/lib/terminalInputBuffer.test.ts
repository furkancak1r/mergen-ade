import { describe, expect, it } from 'vitest';
import { shouldFlushTerminalInput, TERMINAL_INPUT_BATCH_MAX_CHARS } from './terminalInputBuffer';

describe('terminalInputBuffer', () => {
  it('batches ordinary text below the max character threshold', () => {
    expect(shouldFlushTerminalInput('a', 1)).toBe(false);
    expect(shouldFlushTerminalInput('hello', 12)).toBe(false);
  });

  it('flushes immediately for submit and terminal control input', () => {
    expect(shouldFlushTerminalInput('\r', 4)).toBe(true);
    expect(shouldFlushTerminalInput('\x1b[A', 3)).toBe(true);
    expect(shouldFlushTerminalInput('\x03', 1)).toBe(true);
  });

  it('flushes when the queued input reaches the batch size', () => {
    expect(shouldFlushTerminalInput('x', TERMINAL_INPUT_BATCH_MAX_CHARS)).toBe(true);
  });
});
