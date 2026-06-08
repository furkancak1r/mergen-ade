import { describe, it, expect } from 'vitest';
import { normalizeWindowsVerbatimPath } from './pathUtils';

describe('pathUtils', () => {
  it('strips verbatim prefix on Windows', () => {
    expect(normalizeWindowsVerbatimPath('\\\\?\\C:\\Users\\test')).toBe('C:\\Users\\test');
  });

  it('strips verbatim UNC prefix', () => {
    expect(normalizeWindowsVerbatimPath('\\\\?\\UNC\\server\\share')).toBe('\\\\server\\share');
  });

  it('leaves normal paths unchanged', () => {
    expect(normalizeWindowsVerbatimPath('C:\\Users\\test')).toBe('C:\\Users\\test');
  });
});
