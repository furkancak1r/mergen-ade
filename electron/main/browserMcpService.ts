import { spawn, type ChildProcess } from 'child_process';
import { app } from 'electron';
import path from 'path';
import {
  createBrowserView,
  getBrowserInstance,
  syncBrowserBounds,
  showBrowserView,
  hideBrowserView,
  navigateBrowser,
  browserGoBack,
  browserGoForward,
  browserReload,
  browserExecuteJs,
  browserScreenshot,
  browserAddTab,
  browserCloseTab,
  browserSwitchTab,
  destroyBrowserInstance,
} from './browserViewManager';
import { BrowserScopeKeyType, type BrowserScopeKey } from '../shared/types';

interface BrowserMcpSession {
  process: ChildProcess;
  sessionId: string;
  scope: BrowserScopeKey;
  buffer: string;
}

const sessions = new Map<string, BrowserMcpSession>();

function getExePath(): string {
  return process.execPath;
}

function getMcpCommand(): string[] {
  return [getExePath(), '--browser-mcp-helper', '--caps=devtools,vision,network,storage'];
}

export function spawnBrowserMcpSession(sessionId: string, scope: BrowserScopeKey): string {
  const cmd = getMcpCommand();
  const proc = spawn(cmd[0], cmd.slice(1), {
    stdio: ['pipe', 'pipe', 'pipe', 'ipc'],
    windowsHide: true,
  });

  const session: BrowserMcpSession = {
    process: proc,
    sessionId,
    scope,
    buffer: '',
  };

  sessions.set(sessionId, session);

  proc.stdout?.on('data', (data) => {
    session.buffer += data.toString();
    let idx;
    while ((idx = session.buffer.indexOf('\n')) >= 0) {
      const line = session.buffer.slice(0, idx);
      session.buffer = session.buffer.slice(idx + 1);
      if (line.trim()) {
        handleMcpResponse(session, line.trim());
      }
    }
  });

  proc.stderr?.on('data', (data) => {
    console.error(`Browser MCP stderr [${sessionId}]:`, data.toString());
  });

  proc.on('exit', (code) => {
    console.log(`Browser MCP session ${sessionId} exited with code ${code}`);
    sessions.delete(sessionId);
  });

  proc.on('message', (msg: unknown) => {
    if (msg && typeof msg === 'object') {
      const m = msg as Record<string, unknown>;
      if (m.type === 'browserMcpRequest') {
        handleBrowserMcpRequestFromHelper(session, m).catch((err) => {
          console.error('Browser MCP relay error:', err);
        });
      }
    }
  });

  // Send initialize
  sendMcpRequest(session, 'initialize', {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'mergen-browser-mcp', version: '1.0.0' },
  });

  return sessionId;
}

async function handleBrowserMcpRequestFromHelper(
  session: BrowserMcpSession,
  msg: Record<string, unknown>,
): Promise<void> {
  const id = msg.id as string | number;
  const method = msg.method as string;
  const params = msg.params as Record<string, unknown>;
  const result = await executeBrowserMcpToolDirect(session.scope, method, params);
  session.process.send?.({ type: 'browserMcpResponse', id, result });
}

function sendMcpRequest(session: BrowserMcpSession, method: string, params: unknown): void {
  const req = { jsonrpc: '2.0', id: Date.now(), method, params };
  session.process.stdin?.write(JSON.stringify(req) + '\n');
}

function handleMcpResponse(session: BrowserMcpSession, line: string): void {
  try {
    const msg = JSON.parse(line) as Record<string, unknown>;
    if (msg.result) {
      // Response to a request
      console.log(`Browser MCP response [${session.sessionId}]:`, msg.result);
    } else if (msg.error) {
      console.error(`Browser MCP error [${session.sessionId}]:`, msg.error);
    }
  } catch {
    // ignore non-JSON
  }
}

export async function executeBrowserMcpTool(
  sessionId: string,
  method: string,
  params: unknown,
): Promise<unknown> {
  const session = sessions.get(sessionId);
  if (!session) return { success: false, error: 'Session not found' };
  return executeBrowserMcpToolDirect(session.scope, method, params);
}

async function executeBrowserMcpToolDirect(
  scope: BrowserScopeKey,
  method: string,
  params: unknown,
): Promise<unknown> {
  const p = (params as Record<string, unknown>) || {};
  switch (method) {
    case 'browser/navigate': {
      const url = String(p.url || '');
      navigateBrowser(scope, url);
      return { success: true, url };
    }
    case 'browser/click': {
      const selector = String(p.selector || '');
      const script = `(() => {
        const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
        if (el) { el.click(); return true; }
        return false;
      })()`;
      const clicked = await browserExecuteJs(scope, script);
      return { success: clicked === true, selector };
    }
    case 'browser/type': {
      const selector = String(p.selector || '');
      const text = String(p.text || '');
      const script = `(() => {
        const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
        if (el) {
          el.focus();
          el.value = '${text.replace(/'/g, "\\'").replace(/\n/g, '\\n')}';
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return true;
        }
        return false;
      })()`;
      const typed = await browserExecuteJs(scope, script);
      return { success: typed === true, selector, text };
    }
    case 'browser/screenshot': {
      const dataUrl = await browserScreenshot(scope, !!p.fullPage);
      return { success: !!dataUrl, dataUrl };
    }
    case 'browser/getText': {
      const selector = String(p.selector || '');
      const script = `(() => {
        const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
        return el ? el.innerText : null;
      })()`;
      const text = await browserExecuteJs(scope, script);
      return { success: text !== null, selector, text: text ?? '' };
    }
    case 'browser/getHtml': {
      const script = 'document.documentElement.outerHTML';
      const html = await browserExecuteJs(scope, script);
      return { success: !!html, html };
    }
    case 'browser/close': {
      destroyBrowserInstance(scope);
      return { success: true };
    }
    case 'browser/waitFor': {
      const duration = Number(p.duration) || 1000;
      const selector = String(p.selector || '');
      const textQuery = String(p.text || '');
      const start = Date.now();

      const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
      let done = false;
      while (!done) {
        const elapsed = Date.now() - start;
        if (elapsed >= duration) {
          done = true;
          break;
        }
        if (selector) {
          const exists = await browserExecuteJs(scope, `!!document.querySelector('${selector.replace(/'/g, "\\'")}')`);
          if (exists) { done = true; break; }
        }
        if (textQuery) {
          const hasText = await browserExecuteJs(scope, `document.body.innerText.includes('${textQuery.replace(/'/g, "\\'")}')`);
          if (hasText) { done = true; break; }
        }
        const remaining = duration - elapsed;
        await wait(Math.min(200, remaining));
      }
      return { success: true, duration: Date.now() - start };
    }
    case 'browser/newTab': {
      const tabId = browserAddTab(scope, String(p.url || ''));
      return { success: true, tabId };
    }
    case 'browser/switchTab': {
      browserSwitchTab(scope, String(p.tabId || ''));
      return { success: true };
    }
    case 'browser/closeTab': {
      browserCloseTab(scope, String(p.tabId || ''));
      return { success: true };
    }
    case 'browser/goBack': {
      browserGoBack(scope);
      return { success: true };
    }
    case 'browser/goForward': {
      browserGoForward(scope);
      return { success: true };
    }
    case 'browser/reload': {
      browserReload(scope);
      return { success: true };
    }
    case 'browser/scroll': {
      const x = Number(p.x || 0);
      const y = Number(p.y || 0);
      const script = `window.scrollBy(${x}, ${y})`;
      await browserExecuteJs(scope, script);
      return { success: true };
    }
    case 'browser/highlight': {
      const selector = String(p.selector || '');
      const script = `(() => {
        const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
        if (!el) return false;
        const rect = el.getBoundingClientRect();
        const vpW = window.innerWidth;
        const vpH = window.innerHeight;
        if (rect.right < 0 || rect.bottom < 0 || rect.left > vpW || rect.top > vpH) return false;
        if (rect.width === 0 || rect.height === 0) return false;
        const cs = window.getComputedStyle(el);
        if (cs.display === 'none' || cs.visibility === 'hidden' || cs.opacity === '0') return false;
        // Check ancestor overflow clipping
        let parent = el.parentElement;
        while (parent) {
          const pcs = window.getComputedStyle(parent);
          if (pcs.overflow === 'hidden' || pcs.overflow === 'clip' || pcs.overflow === 'scroll' || pcs.overflow === 'auto') {
            const pr = parent.getBoundingClientRect();
            if (rect.left < pr.left || rect.top < pr.top || rect.right > pr.right || rect.bottom > pr.bottom) {
              return false;
            }
          }
          parent = parent.parentElement;
        }
        const style = document.createElement('style');
        style.id = '__mergen-highlight-style';
        document.head.appendChild(style);
        style.textContent = '*{outline:none!important;} ${selector.replace(/'/g, "\\'")}{outline:3px solid #0078d4!important;}';
        return { x: Math.max(0, Math.min(rect.x, vpW)), y: Math.max(0, Math.min(rect.y, vpH)), width: Math.min(rect.width, vpW - Math.max(0, rect.x)), height: Math.min(rect.height, vpH - Math.max(0, rect.y)) };
      })()`;
      const result = await browserExecuteJs(scope, script);
      return { success: !!result, result };
    }
    default:
      return { success: false, error: `Unknown method: ${method}` };
  }
}

export function killBrowserMcpSession(sessionId: string): void {
  const session = sessions.get(sessionId);
  if (!session) return;
  session.process.kill();
  sessions.delete(sessionId);
}

export function getBrowserMcpCommandArray(): string[] {
  return getMcpCommand();
}

export function prepareBrowserMcpToolScope(terminalId: number, projectId: number): BrowserScopeKey {
  return { type: BrowserScopeKeyType.Terminal, projectId, terminalId };
}

export function getBrowserMcpSessionCount(): number {
  return sessions.size;
}
