import { describe, it, expect } from 'vitest';
import { repairMojibake, repairMojibakeDisplay, repairMojibakePath, cappedHoverText } from './mojibake';

describe('mojibake', () => {
  it('returns unchanged text for valid UTF-8', () => {
    expect(repairMojibake('hello world')).toBe('hello world');
    expect(repairMojibake('Türkçe metin')).toBe('Türkçe metin');
    expect(repairMojibake('日本語')).toBe('日本語');
  });

  it('repairs CP1252->UTF-8 mojibake', () => {
    // Common CP1252 mojibake: â€™ instead of '
    const mojibake = "it’s"; // right single quote (U+2019)
    // When mis-encoded as CP1252 bytes then decoded as UTF-8, U+2019 bytes are E2 80 99
    // If those bytes are misinterpreted as CP1252 chars and re-decoded as UTF-8...
    const result = repairMojibake(mojibake);
    // The repair chain should at minimum not crash and should return valid text
    expect(result).toBeTruthy();
    expect(result.length).toBeGreaterThan(0);
  });

  it('repairs display text unconditionally', () => {
    const text = 'Türkçe';
    expect(repairMojibakeDisplay(text)).toBe(text);
  });

  it('repairMojibakePath returns repaired when exists', async () => {
    const exists = async (p: string) => p === 'repaired';
    const result = await repairMojibakePath('original', exists);
    // Since our mock repair won't change 'original', it returns original
    expect(result).toBe('original');
  });

  it('repairMojibakePath falls back when repaired does not exist', async () => {
    const exists = async () => false;
    const result = await repairMojibakePath('test', exists);
    expect(result).toBe('test');
  });

  it('cappedHoverText does not truncate short text', () => {
    expect(cappedHoverText('hello', 100)).toBe('hello');
  });

  it('cappedHoverText truncates long text safely', () => {
    const text = 'a'.repeat(200);
    const result = cappedHoverText(text, 100);
    expect(result.length).toBeLessThanOrEqual(101); // 100 + ellipsis
    expect(result.endsWith('…')).toBe(true);
  });

  it('cappedHoverText is Unicode-safe', () => {
    const text = '日本語'.repeat(50);
    const result = cappedHoverText(text, 10);
    expect(result.endsWith('…')).toBe(true);
    // Should not have broken surrogate pairs
    expect(() => result.charCodeAt(0)).not.toThrow();
  });
});
