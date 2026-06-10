import { describe, expect, it } from 'vitest';
import {
  browserMcpToolSchemas,
  isBrowserMcpToolAllowed,
  normalizeBrowserMcpCaps,
  normalizeBrowserMcpToolName,
  parseBrowserMcpCapsFromArgs,
} from './browserMcpTools';

describe('browser MCP tool metadata', () => {
  it('parses comma and equals caps like the Rust helper', () => {
    expect(parseBrowserMcpCapsFromArgs(['--browser-mcp-helper', '--caps=vision,devtools,vision'])).toEqual(['devtools', 'vision']);
    expect(parseBrowserMcpCapsFromArgs(['--caps', 'storage,network'])).toEqual(['network', 'storage']);
    expect(normalizeBrowserMcpCaps(['vision', ' ', 'devtools', 'vision'])).toEqual(['devtools', 'vision']);
  });

  it('lists Rust-compatible Browser MCP tools by capability', () => {
    const names = browserMcpToolSchemas(['devtools', 'vision']).map((tool) => tool.name);
    expect(names).toContain('browser_cookie_list');
    expect(names).toContain('browser_cookie_get');
    expect(names).toContain('browser_cookie_set');
    expect(names).toContain('browser_cookie_delete');
    expect(names).toContain('browser_cookie_clear');
    expect(names).toContain('browser_navigate');
    expect(names).toContain('browser_page_summary');
    expect(names).toContain('browser_click');
    expect(names).toContain('browser_take_screenshot');
    expect(names).toContain('browser_start_video');
    expect(names).toContain('browser_stop_video');
    expect(names).toContain('browser_video_chapter');
    expect(names).not.toContain('browser_localstorage_get');
  });

  it('uses advertised tools for allow checks', () => {
    expect(isBrowserMcpToolAllowed('browser_click', ['devtools'])).toBe(true);
    expect(isBrowserMcpToolAllowed('browser_cookie_list', ['devtools'])).toBe(true);
    expect(isBrowserMcpToolAllowed('browser_mouse_click_xy', ['devtools'])).toBe(false);
    expect(isBrowserMcpToolAllowed('browser_localstorage_get', ['devtools'])).toBe(false);
    expect(isBrowserMcpToolAllowed('browser_localstorage_get', ['storage'])).toBe(true);
  });

  it('normalizes legacy slash tool names to Rust-compatible names', () => {
    expect(normalizeBrowserMcpToolName('browser/navigate')).toBe('browser_navigate');
    expect(normalizeBrowserMcpToolName('browser/screenshot')).toBe('browser_take_screenshot');
    expect(normalizeBrowserMcpToolName('browser/goBack')).toBe('browser_navigate_back');
    expect(normalizeBrowserMcpToolName('browser_page_summary')).toBe('browser_page_summary');
  });
});
