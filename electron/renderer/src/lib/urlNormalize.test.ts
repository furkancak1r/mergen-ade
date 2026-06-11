import { describe, it, expect } from 'vitest';
import { normalizeBrowserUrl, isAllowedBrowserScheme } from './urlNormalize';

describe('urlNormalize', () => {
  it('returns empty for empty string', () => {
    expect(normalizeBrowserUrl('')).toBe('');
  });

  it('preserves http:// URLs', () => {
    expect(normalizeBrowserUrl('http://example.com')).toBe('http://example.com');
  });

  it('preserves https:// URLs', () => {
    expect(normalizeBrowserUrl('https://example.com')).toBe('https://example.com');
  });

  it('uses http for localhost', () => {
    expect(normalizeBrowserUrl('localhost:3000')).toBe('http://localhost:3000');
  });

  it('uses http for 127.0.0.1', () => {
    expect(normalizeBrowserUrl('127.0.0.1:8080')).toBe('http://127.0.0.1:8080');
  });

  it('uses http for 0.0.0.0', () => {
    expect(normalizeBrowserUrl('0.0.0.0')).toBe('http://0.0.0.0');
  });

  it('uses http for [::1]', () => {
    expect(normalizeBrowserUrl('[::1]:3000')).toBe('http://[::1]:3000');
  });

  it('uses https for regular domains', () => {
    expect(normalizeBrowserUrl('example.com')).toBe('https://example.com');
  });

  it('rejects file:// scheme', () => {
    expect(normalizeBrowserUrl('file:///C:/test.html')).toBe('');
  });

  it('rejects data:// scheme', () => {
    expect(normalizeBrowserUrl('data:text/html,test')).toBe('');
  });

  it('rejects javascript:// scheme', () => {
    expect(normalizeBrowserUrl('javascript:alert(1)')).toBe('');
  });

  it('trims whitespace before processing', () => {
    expect(normalizeBrowserUrl('  example.com  ')).toBe('https://example.com');
  });

  it('isAllowedBrowserScheme allows http and https', () => {
    expect(isAllowedBrowserScheme('http://example.com')).toBe(true);
    expect(isAllowedBrowserScheme('https://example.com')).toBe(true);
    expect(isAllowedBrowserScheme('ftp://example.com')).toBe(false);
    expect(isAllowedBrowserScheme('')).toBe(false);
  });
});
