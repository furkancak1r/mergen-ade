export const MERGEN_BROWSER_MCP_PORT_ENV_VAR = 'MERGEN_BROWSER_MCP_PORT';
export const MERGEN_BROWSER_MCP_TOKEN_ENV_VAR = 'MERGEN_BROWSER_MCP_TOKEN';
export const MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR = 'MERGEN_BROWSER_MCP_TERMINAL_ID';
export const MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR = 'MERGEN_BROWSER_MCP_PROJECT_ID';
export const MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR = 'MERGEN_BROWSER_MCP_SESSION_ID';
export const MERGEN_BROWSER_MCP_ENDPOINT_PATH = '/browser-mcp';

export interface BrowserMcpToolSchema {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export function parseBrowserMcpCapsFromArgs(args: string[]): string[] {
  const caps: string[] = [];
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    let value: string | undefined;
    if (arg.startsWith('--caps=')) {
      value = arg.slice('--caps='.length);
    } else if (arg === '--caps') {
      value = args[i + 1];
      i += 1;
    }
    if (value) {
      caps.push(...value.split(',').map((cap) => cap.trim()).filter(Boolean));
    }
  }
  return normalizeBrowserMcpCaps(caps);
}

export function normalizeBrowserMcpCaps(caps: readonly string[]): string[] {
  return Array.from(new Set(caps.map((cap) => cap.trim()).filter(Boolean))).sort();
}

function hasCap(caps: readonly string[], cap: string): boolean {
  return caps.length === 0 || caps.includes(cap);
}

function tool(name: string, description: string, inputSchema: Record<string, unknown>): BrowserMcpToolSchema {
  return { name, description, inputSchema };
}

function schema(properties: Record<string, unknown>, required: string[] = []): Record<string, unknown> {
  return { type: 'object', properties, required };
}

function elementProperties(extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    type: { type: 'string' },
    selector: { type: 'string', description: 'CSS selector fallback for Electron compatibility' },
    ...extra,
  };
}

function coreTools(): BrowserMcpToolSchema[] {
  return [
    tool('browser_close', 'Close the active Mergen Browser tab.', schema({
      index: { type: 'integer' },
      tabId: { type: 'string' },
    })),
    tool('browser_cookie_clear', 'Clear browser cookies.', schema({})),
    tool('browser_cookie_delete', 'Delete a browser cookie.', schema({
      name: { type: 'string' },
      url: { type: 'string' },
    }, ['name'])),
    tool('browser_cookie_get', 'Get a browser cookie by name.', schema({
      name: { type: 'string' },
      url: { type: 'string' },
    }, ['name'])),
    tool('browser_cookie_list', 'List browser cookies.', schema({
      url: { type: 'string' },
    })),
    tool('browser_cookie_set', 'Set a browser cookie.', schema({
      name: { type: 'string' },
      value: { type: 'string' },
      url: { type: 'string' },
      domain: { type: 'string' },
      path: { type: 'string' },
      secure: { type: 'boolean' },
      httpOnly: { type: 'boolean' },
      sameSite: { type: 'string', enum: ['Strict', 'Lax', 'None'] },
      expires: { type: 'number' },
    }, ['name', 'value'])),
    tool('browser_navigate', 'Navigate to a URL in the embedded Mergen Browser panel.', schema({
      url: { type: 'string' },
    }, ['url'])),
    tool('browser_navigate_back', 'Go back in the browser history.', schema({})),
    tool('browser_navigate_forward', 'Go forward in the browser history.', schema({})),
    tool('browser_reload', 'Reload the current page.', schema({})),
    tool('browser_tabs', 'List, create, select, or close tabs in the Mergen Browser panel. At most five tabs can be open per project.', schema({
      action: { type: 'string', enum: ['list', 'new', 'select', 'close'], default: 'list' },
      index: { type: 'integer' },
      tabId: { type: 'string' },
      url: { type: 'string' },
    })),
    tool('browser_type', 'Type text into an element with user-like input/change events.', schema(elementProperties({
      ref: { type: 'string', description: 'Element ref from browser_page_summary, e.g. e1' },
      text: { type: 'string' },
      submit: { type: 'boolean' },
      commit: { type: 'boolean', default: true },
    }), ['text'])),
    tool('browser_press_key', 'Press a key in the browser.', schema({
      key: { type: 'string' },
    }, ['key'])),
  ];
}

function devtoolsTools(): BrowserMcpToolSchema[] {
  return [
    tool('browser_hide_highlight', 'Hide the active browser highlight overlay.', schema({})),
    tool('browser_highlight', 'Show a highlight overlay for a visible element.', schema(elementProperties({
      ref: { type: 'string', description: 'Element ref from browser_page_summary, e.g. e1' },
      x: { type: 'number' },
      y: { type: 'number' },
      width: { type: 'number' },
      height: { type: 'number' },
      color: { type: 'string', default: '#16a34a' },
      label: { type: 'string' },
      padding: { type: 'number', default: 8 },
      radius: { type: 'number', default: 10 },
    }))),
    tool('browser_page_summary', 'Fast page map for discovering clickable elements, buttons, links, icons, and form fields before browser_click.', schema({
      query: { type: 'string' },
      roles: { type: 'array', items: { type: 'string' } },
      includeBoxes: { type: 'boolean', default: false },
      maxItems: { type: 'integer', default: 40 },
    })),
    tool('browser_snapshot', 'Take a DOM snapshot of the page.', schema(elementProperties({
      ref: { type: 'string' },
      scope: { type: 'string' },
    }))),
    tool('browser_click', 'Click an element on the page using a ref from browser_page_summary or a selector fallback.', schema(elementProperties({
      ref: { type: 'string', description: 'Element ref from browser_page_summary, e.g. e1' },
      button: { type: 'string', enum: ['left', 'middle', 'right'], default: 'left' },
      doubleClick: { type: 'boolean' },
    }))),
    tool('browser_hover', 'Hover over an element on the page using a ref from browser_page_summary or a selector fallback.', schema(elementProperties({
      ref: { type: 'string', description: 'Element ref from browser_page_summary, e.g. e1' },
    }))),
    tool('browser_select_option', 'Select an option in a dropdown.', schema(elementProperties({
      ref: { type: 'string' },
      value: { type: 'string' },
    }), ['value'])),
    tool('browser_fill_form', 'Fill multiple form fields with user-like interaction.', schema({
      fields: {
        type: 'array',
        items: schema({
          name: { type: 'string' },
          target: { type: 'string' },
          selector: { type: 'string' },
          ref: { type: 'string' },
          type: { type: 'string', enum: ['textbox', 'checkbox', 'radio', 'combobox', 'slider'] },
          value: { type: 'string' },
          commit: { type: 'boolean', default: true },
        }, ['value']),
      },
    }, ['fields'])),
    tool('browser_evaluate', 'Read/evaluate JavaScript in the browser.', schema({
      script: { type: 'string' },
      frame: { type: 'string' },
      ref: { type: 'string' },
    }, ['script'])),
    tool('browser_wait_for', 'Wait for text to appear or disappear in the embedded page.', schema({
      time: { type: 'number', description: 'Maximum timeout in seconds' },
      text: { type: 'string' },
      textGone: { type: 'string' },
    })),
  ];
}

function visionTools(): BrowserMcpToolSchema[] {
  return [
    tool('browser_take_screenshot', 'Take a screenshot of the embedded Mergen browser page.', schema(elementProperties({
      ref: { type: 'string' },
      type: { type: 'string', enum: ['png', 'jpeg'], default: 'jpeg' },
      quality: { type: 'integer', default: 74 },
      fullPage: { type: 'boolean', default: false },
    }))),
    tool('browser_start_video', 'Start recording the embedded Mergen browser panel to a video file.', schema({})),
    tool('browser_stop_video', 'Stop recording the embedded Mergen browser panel and save the video file.', schema({})),
    tool('browser_video_chapter', 'Add a timestamped chapter marker to the active embedded Mergen browser video recording.', schema({
      title: { type: 'string' },
      label: { type: 'string' },
    })),
  ];
}

function storageTools(): BrowserMcpToolSchema[] {
  return [
    tool('browser_localstorage_clear', 'Clear local storage.', schema({})),
    tool('browser_localstorage_delete', 'Delete a local storage item.', schema({ key: { type: 'string' } }, ['key'])),
    tool('browser_localstorage_get', 'Get a local storage item.', schema({ key: { type: 'string' } }, ['key'])),
    tool('browser_localstorage_list', 'List local storage keys.', schema({})),
    tool('browser_localstorage_set', 'Set a local storage item.', schema({
      key: { type: 'string' },
      value: { type: 'string' },
    }, ['key', 'value'])),
    tool('browser_sessionstorage_clear', 'Clear session storage.', schema({})),
    tool('browser_sessionstorage_delete', 'Delete a session storage item.', schema({ key: { type: 'string' } }, ['key'])),
    tool('browser_sessionstorage_get', 'Get a session storage item.', schema({ key: { type: 'string' } }, ['key'])),
    tool('browser_sessionstorage_list', 'List session storage keys.', schema({})),
    tool('browser_sessionstorage_set', 'Set a session storage item.', schema({
      key: { type: 'string' },
      value: { type: 'string' },
    }, ['key', 'value'])),
  ];
}

function networkTools(): BrowserMcpToolSchema[] {
  return [
    tool('browser_network_request', 'Get detailed info about a network request (not implemented by Electron Browser MCP yet).', schema({
      requestId: { type: 'string' },
      wait: { type: 'boolean' },
    }, ['requestId'])),
    tool('browser_network_requests', 'List network requests (not implemented by Electron Browser MCP yet).', schema({
      urlFilter: { type: 'string' },
      methodFilter: { type: 'string' },
    })),
  ];
}

export function browserMcpToolSchemas(caps: readonly string[]): BrowserMcpToolSchema[] {
  const normalizedCaps = normalizeBrowserMcpCaps(caps);
  const tools = [...coreTools()];
  if (hasCap(normalizedCaps, 'devtools')) tools.push(...devtoolsTools());
  if (hasCap(normalizedCaps, 'vision')) tools.push(...visionTools());
  if (hasCap(normalizedCaps, 'network')) tools.push(...networkTools());
  if (hasCap(normalizedCaps, 'storage')) tools.push(...storageTools());
  return tools;
}

export function isBrowserMcpToolAllowed(name: string, caps: readonly string[]): boolean {
  return browserMcpToolSchemas(caps).some((toolSchema) => toolSchema.name === name);
}

export function normalizeBrowserMcpToolName(method: string): string {
  switch (method) {
    case 'browser/navigate':
      return 'browser_navigate';
    case 'browser/click':
      return 'browser_click';
    case 'browser/type':
      return 'browser_type';
    case 'browser/screenshot':
      return 'browser_take_screenshot';
    case 'browser/close':
      return 'browser_close';
    case 'browser/waitFor':
      return 'browser_wait_for';
    case 'browser/goBack':
      return 'browser_navigate_back';
    case 'browser/goForward':
      return 'browser_navigate_forward';
    case 'browser/reload':
      return 'browser_reload';
    case 'browser/newTab':
    case 'browser/switchTab':
    case 'browser/closeTab':
      return 'browser_tabs';
    case 'browser/highlight':
      return 'browser_highlight';
    case 'browser/getHtml':
      return 'browser_snapshot';
    case 'browser/getText':
      return 'browser_snapshot';
    default:
      return method;
  }
}
