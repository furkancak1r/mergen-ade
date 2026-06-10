import { describe, expect, it } from 'vitest';
import { BrowserScopeKeyType } from '../../../shared/types';
import { browserProjectIdsAfterScopeEmpty, browserToolbarCanClearUrl, clearBrowserActiveTabUrl } from './browserToolbar';

describe('browser toolbar helpers', () => {
  it('enables clear when draft, active tab url, or page title exists', () => {
    expect(browserToolbarCanClearUrl('', undefined)).toBe(false);
    expect(browserToolbarCanClearUrl('https://example.com', undefined)).toBe(true);
    expect(browserToolbarCanClearUrl('', { url: 'https://example.com' })).toBe(true);
    expect(browserToolbarCanClearUrl('', { url: '', title: 'Example' })).toBe(true);
    expect(browserToolbarCanClearUrl('', { url: '', title: 'New Tab' })).toBe(false);
  });

  it('clears the active tab url and resets its title', () => {
    expect(clearBrowserActiveTabUrl([
      { id: 'one', url: 'https://one.test', title: 'One' },
      { id: 'two', url: 'https://two.test', title: 'Two' },
    ], 'two')).toEqual([
      { id: 'one', url: 'https://one.test', title: 'One' },
      { id: 'two', url: '', title: 'New Tab' },
    ]);
  });

  it('closes the project browser panel when the project-scoped tab set becomes empty', () => {
    expect(Array.from(browserProjectIdsAfterScopeEmpty(new Set([1, 2]), {
      type: BrowserScopeKeyType.Project,
      projectId: 1,
    })).sort()).toEqual([2]);
  });

  it('keeps browser panel open when a terminal-scoped tab set becomes empty', () => {
    expect(Array.from(browserProjectIdsAfterScopeEmpty(new Set([1, 2]), {
      type: BrowserScopeKeyType.Terminal,
      projectId: 1,
      terminalId: 10,
    })).sort()).toEqual([1, 2]);
  });
});
