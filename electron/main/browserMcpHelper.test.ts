import { describe, expect, it } from 'vitest';
import { browserMcpContent, handleBrowserMcpJsonRpcMessage } from './browserMcpHelper';

describe('browser MCP helper protocol', () => {
  it('returns MCP initialize metadata with tools capability', async () => {
    const response = await handleBrowserMcpJsonRpcMessage({
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
    }, ['devtools']);

    expect(response?.result).toMatchObject({
      protocolVersion: '2024-11-05',
      capabilities: { tools: {} },
      serverInfo: { name: 'mergen-browser-mcp' },
    });
  });

  it('returns tools/list using Rust-compatible tool names', async () => {
    const response = await handleBrowserMcpJsonRpcMessage({
      jsonrpc: '2.0',
      id: 'tools',
      method: 'tools/list',
    }, ['devtools', 'vision']);

    const tools = ((response?.result as { tools: { name: string }[] }).tools).map((tool) => tool.name);
    expect(tools).toContain('browser_navigate');
    expect(tools).toContain('browser_page_summary');
    expect(tools).toContain('browser_take_screenshot');
  });

  it('wraps tools/call relay responses as MCP content', async () => {
    const response = await handleBrowserMcpJsonRpcMessage({
      jsonrpc: '2.0',
      id: 'call',
      method: 'tools/call',
      params: {
        name: 'browser_navigate',
        arguments: { url: 'https://example.com' },
      },
    }, [], async (method, params) => ({
      success: true,
      text: `called ${method}`,
      params,
    }));

    expect(response?.result).toMatchObject({
      isError: false,
      content: [{ type: 'text', text: 'called browser_navigate' }],
    });
  });

  it('rejects tools that are not advertised for the active caps', async () => {
    const response = await handleBrowserMcpJsonRpcMessage({
      jsonrpc: '2.0',
      id: 'bad-tool',
      method: 'tools/call',
      params: {
        name: 'browser_localstorage_get',
        arguments: { key: 'x' },
      },
    }, ['devtools']);

    expect(response?.error).toMatchObject({
      code: -32601,
    });
  });

  it('converts screenshot data urls into MCP image content', () => {
    const content = browserMcpContent({
      success: true,
      text: 'Captured browser screenshot',
      dataUrl: 'data:image/png;base64,abcd',
    });

    expect(content).toEqual([
      { type: 'text', text: 'Captured browser screenshot' },
      { type: 'image', mimeType: 'image/png', data: 'abcd' },
    ]);
  });
});
