import { describe, it, expect } from 'vitest';
import { sanitizeTitle, truncateTitle } from './title';

describe('title', () => {
  it('removes control characters', () => {
    expect(sanitizeTitle('hello\x01world')).toBe('helloworld');
  });

  it('compacts whitespace', () => {
    expect(sanitizeTitle('hello   world')).toBe('hello world');
  });

  it('trims edges', () => {
    expect(sanitizeTitle('  hello  ')).toBe('hello');
  });

  it('truncates long titles', () => {
    const long = 'a'.repeat(100);
    expect(truncateTitle(long, 50).length).toBe(50);
    expect(truncateTitle(long, 50).endsWith('…')).toBe(true);
  });

  it('does not truncate short titles', () => {
    expect(truncateTitle('hello', 50)).toBe('hello');
  });
});
