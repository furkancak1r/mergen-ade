import { describe, expect, it } from 'vitest';
import { selectedTextFromRange } from './fileEditor';

describe('fileEditor helpers', () => {
  it('returns null when no text is selected', () => {
    expect(selectedTextFromRange('hello', 2, 2)).toBeNull();
  });

  it('returns the selected text for forward and reversed ranges', () => {
    expect(selectedTextFromRange('hello world', 0, 5)).toBe('hello');
    expect(selectedTextFromRange('hello world', 5, 0)).toBe('hello');
  });

  it('clamps ranges to the text bounds', () => {
    expect(selectedTextFromRange('hello', -10, 20)).toBe('hello');
  });
});
