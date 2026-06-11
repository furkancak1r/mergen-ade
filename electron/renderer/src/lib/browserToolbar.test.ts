import { describe, expect, it } from 'vitest';
import { BrowserScopeKeyType } from '../../../shared/types';
import { browserAddTabTooltip, browserCanAddTab, browserProjectIdsAfterScopeEmpty, browserScreenshotButtonMeta, browserTabTitle, browserToolbarButtonMeta, browserToolbarCanClearUrl, clearBrowserActiveTabUrl } from './browserToolbar';

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

  it('uses icon-only screenshot button metadata matching the Rust toolbar split buttons', () => {
    expect(browserScreenshotButtonMeta('fullPage')).toEqual({
      mode: 'fullPage',
      fullPage: true,
      icon: '▣',
      tooltip: 'Full page screenshot',
      ariaLabel: 'Full page screenshot',
    });
    expect(browserScreenshotButtonMeta('visibleArea')).toEqual({
      mode: 'visibleArea',
      fullPage: false,
      icon: '▤',
      tooltip: 'Visible area screenshot',
      ariaLabel: 'Visible area screenshot',
    });
  });

  it('marks screenshot tooltips as pending without changing the icon labels', () => {
    const pending = browserScreenshotButtonMeta('visibleArea', true);
    expect(pending.icon).toBe('▤');
    expect(pending.tooltip).toBe('Visible area screenshot (pending...)');
    expect(pending.tooltip).not.toContain('Full');
  });

  it('uses Rust browser tab title fallback order', () => {
    expect(browserTabTitle({ title: 'Example', url: 'https://example.test' })).toBe('Example');
    expect(browserTabTitle({ title: '   ', url: 'https://example.test' })).toBe('https://example.test');
    expect(browserTabTitle({ title: '', url: '' })).toBe('New Tab');
  });

  it('enforces the Rust browser tab limit in toolbar state', () => {
    expect(browserCanAddTab(4)).toBe(true);
    expect(browserCanAddTab(5)).toBe(false);
    expect(browserAddTabTooltip(4)).toBe('New tab');
    expect(browserAddTabTooltip(5)).toBe('Browser tab limit reached (5)');
  });

  it('uses icon-only toolbar metadata for refresh, clear, and Design Inspect', () => {
    expect(browserToolbarButtonMeta('refresh')).toMatchObject({
      icon: '↻',
      tooltip: 'Refresh',
      ariaLabel: 'Refresh',
      selected: false,
    });
    expect(browserToolbarButtonMeta('clearUrl')).toMatchObject({
      icon: '⌫',
      tooltip: 'Clear URL',
      ariaLabel: 'Clear URL',
      selected: false,
    });
    expect(browserToolbarButtonMeta('designInspect', false)).toMatchObject({
      icon: '✎',
      tooltip: 'Design Inspect: OFF (click to enable)',
      ariaLabel: 'Enable Design Inspect',
      selected: false,
    });
    expect(browserToolbarButtonMeta('designInspect', true)).toMatchObject({
      icon: '✎',
      tooltip: 'Design Inspect: ON (click to disable)',
      ariaLabel: 'Disable Design Inspect',
      selected: true,
    });
  });
});
