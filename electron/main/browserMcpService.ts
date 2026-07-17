import { spawn, type ChildProcess } from 'child_process';
import { BrowserWindow } from 'electron';
import fs from 'fs';
import path from 'path';
import { pathToFileURL } from 'url';
import {
  getBrowserInstance,
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
  getBrowserTabsSnapshot,
} from './browserViewManager';
import { BrowserScopeKeyType, type BrowserScopeKey, type BrowserState, type BrowserTab } from '../shared/types';
import { registerBrowserMcpHandler } from './hookService';
import { normalizeBrowserMcpToolName } from './browserMcpTools';
import { getBrowserMcpStdioConfig } from './browserMcpCommand';
import { browserRecordingsDir } from './config';

interface BrowserMcpSession {
  process: ChildProcess;
  sessionId: string;
  scope: BrowserScopeKey;
  buffer: string;
}

const sessions = new Map<string, BrowserMcpSession>();
const BROWSER_VIDEO_FPS = 10;
const BROWSER_VIDEO_FRAME_INTERVAL_MS = Math.round(1000 / BROWSER_VIDEO_FPS);

interface BrowserVideoFrame {
  elapsedMs: number;
  dataUrl: string;
}

interface BrowserVideoChapter {
  elapsedMs: number;
  label: string;
}

interface BrowserVideoRecordingState {
  scope: BrowserScopeKey;
  projectId: number;
  outputBasePath: string;
  startedAt: number;
  frames: BrowserVideoFrame[];
  chapters: BrowserVideoChapter[];
  timer: NodeJS.Timeout;
  captureInFlight: boolean;
  nextFrameIndex: number;
  lastError?: string;
}

const browserVideoRecordings = new Map<string, BrowserVideoRecordingState>();

function browserMcpScopeKey(scope: BrowserScopeKey): string {
  if (scope.type === BrowserScopeKeyType.Terminal) {
    return `terminal:${scope.projectId}:${scope.terminalId ?? 0}`;
  }
  return `project:${scope.projectId}`;
}

function broadcastBrowserTabOpened(scope: BrowserScopeKey, tab: BrowserTab): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send('browser:tabOpened', scope, tab);
    }
  }
}

function broadcastBrowserTabsChanged(scope: BrowserScopeKey, state: Pick<BrowserState, 'tabs' | 'activeTabId' | 'urlDraft'> = getBrowserTabsSnapshot(scope)): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send('browser:tabsChanged', scope, state);
    }
  }
}

export function spawnBrowserMcpSession(sessionId: string, scope: BrowserScopeKey): string {
  const helper = getBrowserMcpStdioConfig();
  const proc = spawn(helper.command, helper.args, {
    stdio: ['pipe', 'pipe', 'pipe', 'ipc'],
    windowsHide: true,
    shell: false,
    env: { ...process.env, ...helper.env },
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
    console.error('Browser MCP stderr [%s]: %s', sessionId, data.toString());
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
      console.log('Browser MCP response [%s]:', session.sessionId, msg.result);
    } else if (msg.error) {
      console.error('Browser MCP error [%s]:', session.sessionId, msg.error);
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
  const tool = normalizeBrowserMcpToolName(method);
  switch (tool) {
    case 'browser_navigate': {
      const url = String(p.url || '');
      const tab = navigateBrowser(scope, url);
      if (tab) {
        broadcastBrowserTabOpened(scope, tab);
        broadcastBrowserTabsChanged(scope);
      }
      return { success: true, text: `Navigating to ${url}`, url };
    }
    case 'browser_click': {
      const result = await browserExecuteJs(scope, browserElementActionScript('click', p));
      return normalizeJsToolResult(result, 'Clicked browser element');
    }
    case 'browser_hover': {
      const result = await browserExecuteJs(scope, browserElementActionScript('hover', p));
      return normalizeJsToolResult(result, 'Hovered browser element');
    }
    case 'browser_type': {
      const result = await browserExecuteJs(scope, browserElementActionScript('type', p));
      return normalizeJsToolResult(result, 'Typed into browser element');
    }
    case 'browser_select_option': {
      const result = await browserExecuteJs(scope, browserElementActionScript('select', p));
      return normalizeJsToolResult(result, 'Selected browser option');
    }
    case 'browser_press_key': {
      const result = await browserExecuteJs(scope, browserPressKeyScript(String(p.key || '')));
      return normalizeJsToolResult(result, `Pressed key ${String(p.key || '')}`);
    }
    case 'browser_fill_form': {
      const result = await browserExecuteJs(scope, browserFillFormScript(Array.isArray(p.fields) ? p.fields : []));
      return normalizeJsToolResult(result, 'Filled browser form');
    }
    case 'browser_take_screenshot': {
      const dataUrl = await browserScreenshot(scope, !!p.fullPage);
      return { success: !!dataUrl, text: dataUrl ? 'Captured browser screenshot' : 'Browser screenshot failed', dataUrl };
    }
    case 'browser_page_summary': {
      const result = await browserExecuteJs(scope, browserPageSummaryScript(p));
      return normalizePageSummaryResult(result);
    }
    case 'browser_snapshot': {
      const result = await browserExecuteJs(scope, browserSnapshotScript(p));
      return normalizeJsToolResult(result, 'Captured browser snapshot');
    }
    case 'browser_evaluate': {
      const script = String(p.script || '');
      if (!script.trim()) return { success: false, error: 'browser_evaluate requires script' };
      const result = await browserExecuteJs(scope, script);
      return {
        success: true,
        text: typeof result === 'string' ? result : JSON.stringify(result),
        result,
      };
    }
    case 'browser_close': {
      const closed = closeBrowserTabForParams(scope, p);
      broadcastBrowserTabsChanged(scope);
      return { success: true, text: closed ? 'Closed browser tab' : 'Closed browser panel' };
    }
    case 'browser_wait_for': {
      return waitForBrowserCondition(scope, p);
    }
    case 'browser_tabs': {
      return handleBrowserTabsTool(scope, method, p);
    }
    case 'browser_navigate_back': {
      browserGoBack(scope);
      return { success: true, text: 'Navigated back in browser history' };
    }
    case 'browser_navigate_forward': {
      browserGoForward(scope);
      return { success: true, text: 'Navigated forward in browser history' };
    }
    case 'browser_reload': {
      browserReload(scope);
      return { success: true, text: 'Reloaded browser page' };
    }
    case 'browser/scroll': {
      const x = Number(p.x || 0);
      const y = Number(p.y || 0);
      const script = `window.scrollBy(${x}, ${y})`;
      await browserExecuteJs(scope, script);
      return { success: true, text: 'Scrolled browser page' };
    }
    case 'browser_highlight': {
      const result = await browserExecuteJs(scope, browserHighlightScript(p));
      return normalizeJsToolResult(result, 'Highlighted browser element');
    }
    case 'browser_hide_highlight': {
      await browserExecuteJs(scope, `(() => { document.getElementById('__mergen-mcp-highlight')?.remove(); return true; })()`);
      return { success: true, text: 'Hidden browser highlight' };
    }
    case 'browser_cookie_clear':
    case 'browser_cookie_delete':
    case 'browser_cookie_get':
    case 'browser_cookie_list':
    case 'browser_cookie_set': {
      return handleBrowserCookieTool(scope, tool, p);
    }
    case 'browser_localstorage_clear':
    case 'browser_localstorage_delete':
    case 'browser_localstorage_get':
    case 'browser_localstorage_list':
    case 'browser_localstorage_set':
    case 'browser_sessionstorage_clear':
    case 'browser_sessionstorage_delete':
    case 'browser_sessionstorage_get':
    case 'browser_sessionstorage_list':
    case 'browser_sessionstorage_set': {
      const result = await browserExecuteJs(scope, browserStorageScript(tool, p));
      return normalizeJsToolResult(result, 'Updated browser storage');
    }
    case 'browser_network_request':
    case 'browser_network_requests':
    case 'browser_console_messages':
      return { success: false, error: `${tool} is not implemented by Electron Browser MCP yet` };
    case 'browser_start_video':
      return startBrowserVideoRecording(scope);
    case 'browser_stop_video':
      return stopBrowserVideoRecording(scope);
    case 'browser_video_chapter':
      return addBrowserVideoChapter(scope, p);
    default:
      return { success: false, error: `Unknown method: ${method}` };
  }
}

function startBrowserVideoRecording(scope: BrowserScopeKey): unknown {
  const projectId = scope.projectId;
  if (!getBrowserInstance(scope)) {
    return {
      success: false,
      error: 'Browser video recording requires an active embedded Browser tab for this terminal or project',
    };
  }
  if (Array.from(browserVideoRecordings.values()).some((state) => state.projectId === projectId)) {
    return { success: false, error: 'Browser video recording is already running for this project' };
  }

  const stamp = new Date().toISOString().replace(/[:.]/g, '-');
  const outputBasePath = [browserRecordingsDir(projectId), `browser-recording-${stamp}`].join(path.sep);
  const state: BrowserVideoRecordingState = {
    scope,
    projectId,
    outputBasePath,
    startedAt: Date.now(),
    frames: [],
    chapters: [],
    captureInFlight: false,
    nextFrameIndex: 0,
    timer: setInterval(() => {
      void captureBrowserVideoFrame(state);
    }, BROWSER_VIDEO_FRAME_INTERVAL_MS),
  };
  browserVideoRecordings.set(browserMcpScopeKey(scope), state);
  void captureBrowserVideoFrame(state);

  return {
    success: true,
    text: `Started embedded Mergen browser video recording: ${outputBasePath}.webm`,
    data: {
      videoPath: `${outputBasePath}.webm`,
      fps: BROWSER_VIDEO_FPS,
      mimeType: 'video/webm',
    },
  };
}

async function stopBrowserVideoRecording(scope: BrowserScopeKey): Promise<unknown> {
  const key = browserMcpScopeKey(scope);
  const state = browserVideoRecordings.get(key);
  if (!state) {
    return { success: false, error: 'Browser video recording is not running for this project' };
  }
  browserVideoRecordings.delete(key);
  clearInterval(state.timer);
  if (state.captureInFlight) {
    await new Promise((resolve) => setTimeout(resolve, BROWSER_VIDEO_FRAME_INTERVAL_MS + 25));
  }

  if (state.frames.length === 0) {
    const detail = state.lastError ? ` Last capture error: ${state.lastError}` : '';
    return {
      success: false,
      error: `Browser video recording stopped before any frames were captured.${detail}`,
    };
  }

  const durationMs = Math.max(Date.now() - state.startedAt, Math.round((state.frames.length * 1000) / BROWSER_VIDEO_FPS));
  let encoded: { dataUrl: string; mimeType: string };
  try {
    encoded = await encodeBrowserVideoWebm(state.frames, BROWSER_VIDEO_FPS);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { success: false, error: `Browser video encoding failed: ${message}` };
  }

  const outputPath = `${state.outputBasePath}.webm`;
  try {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, dataUrlToBuffer(encoded.dataUrl));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { success: false, error: `Could not save browser video recording: ${message}` };
  }

  const videoUrl = pathToFileURL(outputPath).toString();
  const data = {
    videoPath: outputPath,
    videoUrl,
    mimeType: encoded.mimeType || 'video/webm',
    frameCount: state.frames.length,
    durationMs,
    chapters: state.chapters,
    tabId: undefined as string | undefined,
  };

  try {
    const tabId = browserAddTab(scope, videoUrl, `Recording ${path.basename(outputPath)}`, 'recording');
    data.tabId = tabId;
    broadcastBrowserTabOpened(scope, {
      id: tabId,
      url: videoUrl,
      title: `Recording ${path.basename(outputPath)}`,
      kind: 'recording',
    });
    broadcastBrowserTabsChanged(scope);
    return {
      success: true,
      text: `Saved embedded Mergen browser video and opened it in tab ${tabId}: ${outputPath} (${state.frames.length} frames).`,
      data,
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return {
      success: false,
      error: `Saved embedded Mergen browser video, but could not open it in a new tab: ${message}. Video path: ${outputPath}`,
      data,
    };
  }
}

function addBrowserVideoChapter(scope: BrowserScopeKey, params: Record<string, unknown>): unknown {
  const state = browserVideoRecordings.get(browserMcpScopeKey(scope));
  if (!state) {
    return { success: false, error: 'Browser video recording is not running for this project' };
  }
  const rawLabel = String(params.title || params.label || 'Chapter').trim();
  const label = rawLabel || 'Chapter';
  const elapsedMs = Date.now() - state.startedAt;
  state.chapters.push({ elapsedMs, label });
  return {
    success: true,
    text: `Added browser video chapter at ${elapsedMs}ms: ${label}`,
    data: { elapsedMs, label },
  };
}

async function captureBrowserVideoFrame(state: BrowserVideoRecordingState): Promise<void> {
  if (state.captureInFlight) return;
  state.captureInFlight = true;
  try {
    const dataUrl = await browserScreenshot(state.scope, false);
    if (dataUrl) {
      state.frames.push({
        elapsedMs: Date.now() - state.startedAt,
        dataUrl,
      });
      state.nextFrameIndex += 1;
      state.lastError = undefined;
    } else {
      state.lastError = 'Browser screenshot returned no image data';
    }
  } catch (err) {
    state.lastError = err instanceof Error ? err.message : String(err);
  } finally {
    state.captureInFlight = false;
  }
}

function dataUrlToBuffer(dataUrl: string): Buffer {
  const match = dataUrl.match(/^data:[^;]+;base64,(.+)$/);
  if (!match) {
    throw new Error('Encoded browser video did not return a base64 data URL');
  }
  return Buffer.from(match[1], 'base64');
}

async function encodeBrowserVideoWebm(frames: BrowserVideoFrame[], fps: number): Promise<{ dataUrl: string; mimeType: string }> {
  const win = new BrowserWindow({
    show: false,
    width: 640,
    height: 480,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  try {
    await win.loadURL('data:text/html;charset=utf-8,<html><body></body></html>');
    const script = `(${browserVideoEncoderScript()})(${JSON.stringify(frames)}, ${fps})`;
    return await win.webContents.executeJavaScript(script, true) as { dataUrl: string; mimeType: string };
  } finally {
    if (!win.isDestroyed()) {
      win.destroy();
    }
  }
}

function browserVideoEncoderScript(): string {
  return `async function(frames, fps) {
    if (!frames.length) throw new Error('No browser video frames were captured');
    if (typeof MediaRecorder === 'undefined') throw new Error('MediaRecorder is not available in this Electron runtime');

    const loadImage = (src) => new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = () => reject(new Error('Could not decode browser video frame'));
      image.src = src;
    });
    const first = await loadImage(frames[0].dataUrl);
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(2, first.naturalWidth || first.width || 2);
    canvas.height = Math.max(2, first.naturalHeight || first.height || 2);
    if (canvas.width % 2 !== 0) canvas.width += 1;
    if (canvas.height % 2 !== 0) canvas.height += 1;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Could not create browser video canvas context');

    const mimeCandidates = [
      'video/webm;codecs=vp9',
      'video/webm;codecs=vp8',
      'video/webm'
    ];
    const mimeType = mimeCandidates.find((candidate) => MediaRecorder.isTypeSupported(candidate)) || '';
    const stream = canvas.captureStream(Math.max(1, Math.min(60, fps || 10)));
    const chunks = [];
    const recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
    recorder.ondataavailable = (event) => {
      if (event.data && event.data.size) chunks.push(event.data);
    };
    const stopped = new Promise((resolve, reject) => {
      recorder.onstop = resolve;
      recorder.onerror = () => reject(recorder.error || new Error('Browser video recorder failed'));
    });
    recorder.start();

    const frameDelay = Math.max(1, Math.round(1000 / Math.max(1, Math.min(60, fps || 10))));
    for (const frame of frames) {
      const image = frame === frames[0] ? first : await loadImage(frame.dataUrl);
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
      await new Promise((resolve) => setTimeout(resolve, frameDelay));
    }
    recorder.stop();
    await stopped;
    stream.getTracks().forEach((track) => track.stop());

    const blob = new Blob(chunks, { type: recorder.mimeType || mimeType || 'video/webm' });
    const dataUrl = await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result);
      reader.onerror = () => reject(reader.error || new Error('Could not read browser video blob'));
      reader.readAsDataURL(blob);
    });
    return { dataUrl, mimeType: blob.type || 'video/webm' };
  }`;
}

function handleBrowserMcpHttpRequest(body: Record<string, unknown>): Promise<unknown> {
  const terminalId = Number(body.terminalId);
  const projectId = Number(body.projectId);
  if (!Number.isFinite(terminalId) || terminalId <= 0) {
    return Promise.resolve({ success: false, error: 'Browser MCP request is missing terminalId' });
  }
  if (!Number.isFinite(projectId) || projectId <= 0) {
    return Promise.resolve({ success: false, error: 'Browser MCP request is missing projectId' });
  }
  const scope = prepareBrowserMcpToolScope(terminalId, projectId);
  return executeBrowserMcpToolDirect(scope, String(body.method || ''), body.params);
}

function closeBrowserTabForParams(scope: BrowserScopeKey, params: Record<string, unknown>): boolean {
  const instance = getBrowserInstance(scope) as { tabs?: { id: string }[]; activeTabId?: string } | undefined;
  const tabId = String(params.tabId || instance?.activeTabId || '');
  if (tabId) {
    browserCloseTab(scope, tabId);
    return true;
  }
  destroyBrowserInstance(scope);
  return false;
}

function handleBrowserTabsTool(scope: BrowserScopeKey, originalMethod: string, params: Record<string, unknown>): unknown {
  const instance = getBrowserInstance(scope) as { tabs?: { id: string; url: string; title?: string }[]; activeTabId?: string } | undefined;
  const action = originalMethod === 'browser/newTab'
    ? 'new'
    : originalMethod === 'browser/switchTab'
      ? 'select'
      : originalMethod === 'browser/closeTab'
        ? 'close'
        : String(params.action || 'list');
  if (action === 'new') {
    const url = String(params.url || '');
    const tabId = browserAddTab(scope, url);
    broadcastBrowserTabOpened(scope, { id: tabId, url, kind: 'page' });
    broadcastBrowserTabsChanged(scope);
    return { success: true, text: `Opened browser tab ${tabId}`, tabId };
  }
  if (action === 'select') {
    const tabId = tabIdFromParams(instance, params);
    if (!tabId) return { success: false, error: 'browser_tabs select requires tabId or index' };
    browserSwitchTab(scope, tabId);
    broadcastBrowserTabsChanged(scope);
    return { success: true, text: `Selected browser tab ${tabId}`, tabId };
  }
  if (action === 'close') {
    const tabId = tabIdFromParams(instance, params);
    if (!tabId) return { success: false, error: 'browser_tabs close requires tabId or index' };
    browserCloseTab(scope, tabId);
    broadcastBrowserTabsChanged(scope);
    return { success: true, text: `Closed browser tab ${tabId}`, tabId };
  }
  const tabs = instance?.tabs ?? [];
  return {
    success: true,
    text: tabs.length
      ? tabs.map((tab, index) => `${index + 1}. ${tab.id}${tab.id === instance?.activeTabId ? ' *' : ''} ${tab.title || tab.url || 'New Tab'}`).join('\n')
      : 'No browser tabs are open',
    tabs,
    activeTabId: instance?.activeTabId,
  };
}

function tabIdFromParams(instance: { tabs?: { id: string }[] } | undefined, params: Record<string, unknown>): string {
  const explicit = String(params.tabId || '');
  if (explicit) return explicit;
  const index = Number(params.index);
  if (Number.isFinite(index) && index > 0) {
    return instance?.tabs?.[index - 1]?.id ?? '';
  }
  return '';
}

async function waitForBrowserCondition(scope: BrowserScopeKey, params: Record<string, unknown>): Promise<unknown> {
  const timeoutSecs = Number(params.time ?? params.duration ?? 5);
  const timeoutMs = Math.max(0, Math.min(60_000, timeoutSecs * 1000));
  const selector = String(params.selector || '');
  const text = String(params.text || '');
  const textGone = String(params.textGone || '');
  const start = Date.now();
  const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
  while (Date.now() - start <= timeoutMs) {
    const result = await browserExecuteJs(scope, browserWaitProbeScript({ selector, text, textGone }));
    if (result && typeof result === 'object' && (result as { success?: boolean }).success) {
      return { success: true, text: `Browser wait condition met after ${Date.now() - start}ms`, duration: Date.now() - start };
    }
    await wait(100);
  }
  return { success: false, error: `Browser wait condition was not met within ${timeoutMs}ms` };
}

function normalizeJsToolResult(result: unknown, successText: string): unknown {
  if (result && typeof result === 'object') {
    const obj = result as Record<string, unknown>;
    if (obj.success === false) {
      return { success: false, error: String(obj.error || 'Browser MCP tool failed'), ...obj };
    }
    if (typeof obj.text === 'string') {
      return { success: true, ...obj };
    }
    return { success: true, text: successText, ...obj };
  }
  return { success: Boolean(result), text: result ? successText : 'Browser MCP tool failed', result };
}

function normalizePageSummaryResult(result: unknown): unknown {
  if (!result || typeof result !== 'object') {
    return { success: false, error: 'Browser page summary failed' };
  }
  const obj = result as Record<string, unknown>;
  return {
    success: true,
    text: typeof obj.summary === 'string' ? obj.summary : JSON.stringify(obj),
    ...obj,
  };
}

async function handleBrowserCookieTool(scope: BrowserScopeKey, tool: string, params: Record<string, unknown>): Promise<unknown> {
  const instance = getBrowserInstance(scope) as { view?: Electron.BrowserView; tabs?: { id: string; url: string; title?: string }[]; activeTabId?: string } | undefined;
  if (!instance?.view) {
    return { success: false, error: 'Browser cookie tools require an active embedded Browser tab' };
  }
  const cookieUrl = browserCookieUrl(instance, params);
  const filter = cookieUrl ? { url: cookieUrl } : {};
  const cookies = instance.view.webContents.session.cookies;

  if (tool === 'browser_cookie_list') {
    const listed = await cookies.get(filter);
    const text = listed.length
      ? listed.map((cookie) => `${cookie.name}=${cookie.value}; domain=${cookie.domain}; path=${cookie.path}`).join('\n')
      : 'No browser cookies';
    return { success: true, text, cookies: listed };
  }

  const name = String(params.name || '').trim();
  if (!name) {
    return { success: false, error: `${tool} requires name` };
  }

  if (tool === 'browser_cookie_get') {
    const listed = await cookies.get({ ...filter, name });
    const cookie = listed[0];
    return cookie
      ? { success: true, text: cookie.value, cookie }
      : { success: false, error: `Cookie not found: ${name}` };
  }

  if (tool === 'browser_cookie_set') {
    const value = String(params.value ?? '');
    const targetUrl = cookieUrl || browserCookieUrl(instance, { ...params, url: activeBrowserUrl(instance) });
    if (!targetUrl) {
      return { success: false, error: 'browser_cookie_set requires url or an active http/https browser page' };
    }
    const details: Electron.CookiesSetDetails = {
      url: targetUrl,
      name,
      value,
      path: String(params.path || '/'),
    };
    const domain = String(params.domain || '').trim();
    if (domain) details.domain = domain;
    if (typeof params.secure === 'boolean') details.secure = params.secure;
    if (typeof params.httpOnly === 'boolean') details.httpOnly = params.httpOnly;
    if (typeof params.expires === 'number' && Number.isFinite(params.expires)) {
      details.expirationDate = params.expires;
    }
    const sameSite = String(params.sameSite || '').toLowerCase();
    if (sameSite === 'strict' || sameSite === 'lax' || sameSite === 'no_restriction') {
      details.sameSite = sameSite as Electron.CookiesSetDetails['sameSite'];
    } else if (sameSite === 'none') {
      details.sameSite = 'no_restriction';
    }
    await cookies.set(details);
    return { success: true, text: `Cookie set: ${name}`, cookie: { name, value, url: targetUrl } };
  }

  if (tool === 'browser_cookie_delete') {
    const listed = await cookies.get({ ...filter, name });
    if (listed.length === 0) {
      return { success: false, error: `Cookie not found: ${name}` };
    }
    for (const cookie of listed) {
      await cookies.remove(browserCookieRemovalUrl(cookie, cookieUrl), cookie.name);
    }
    return { success: true, text: `Cookie deleted: ${name}`, count: listed.length };
  }

  if (tool === 'browser_cookie_clear') {
    const listed = await cookies.get(filter);
    for (const cookie of listed) {
      await cookies.remove(browserCookieRemovalUrl(cookie, cookieUrl), cookie.name);
    }
    return { success: true, text: `Browser cookies cleared (${listed.length})`, count: listed.length };
  }

  return { success: false, error: `Unknown cookie tool: ${tool}` };
}

function activeBrowserUrl(instance: { view?: Electron.BrowserView; tabs?: { id: string; url: string }[]; activeTabId?: string }): string {
  const webUrl = instance.view?.webContents.getURL() ?? '';
  if (webUrl) return webUrl;
  return instance.tabs?.find((tab) => tab.id === instance.activeTabId)?.url ?? '';
}

function browserCookieUrl(instance: { view?: Electron.BrowserView; tabs?: { id: string; url: string }[]; activeTabId?: string }, params: Record<string, unknown>): string {
  const explicit = String(params.url || '').trim();
  const candidate = explicit || activeBrowserUrl(instance);
  const lower = candidate.toLowerCase();
  if (lower.startsWith('http://') || lower.startsWith('https://')) {
    return candidate;
  }
  return '';
}

function browserCookieRemovalUrl(cookie: Electron.Cookie, fallbackUrl?: string): string {
  if (fallbackUrl) return fallbackUrl;
  const domain = (cookie.domain || '').replace(/^\./, '');
  if (!domain) return 'http://localhost/';
  const scheme = cookie.secure ? 'https' : 'http';
  const cookiePath = cookie.path || '/';
  return `${scheme}://${domain}${cookiePath.startsWith('/') ? cookiePath : `/${cookiePath}`}`;
}

function js(value: unknown): string {
  return JSON.stringify(value);
}

function browserFindElementPrelude(params: Record<string, unknown>): string {
  const ref = String(params.ref || '');
  const selector = String(params.selector || params.target || '');
  const name = String(params.name || '');
  return `
    const ref = ${js(ref)};
    const selector = ${js(selector)};
    const name = ${js(name)};
    function cssEscape(value) {
      return String(value).replace(/["\\\\]/g, '\\\\$&');
    }
    function findElement() {
      if (ref && window.__mergenMcpRefs && window.__mergenMcpRefs[ref]) return window.__mergenMcpRefs[ref];
      if (ref) {
        const byRef = document.querySelector('[data-mergen-mcp-ref="' + cssEscape(ref) + '"]');
        if (byRef) return byRef;
      }
      if (selector) return document.querySelector(selector);
      if (name) return document.querySelector('[name="' + cssEscape(name) + '"]');
      return document.activeElement;
    }
  `;
}

function browserElementActionScript(action: 'click' | 'hover' | 'type' | 'select', params: Record<string, unknown>): string {
  const text = String(params.text || '');
  const value = String(params.value ?? params.text ?? '');
  const submit = Boolean(params.submit);
  const doubleClick = Boolean(params.doubleClick);
  return `(() => {
    ${browserFindElementPrelude(params)}
    const el = findElement();
    if (!el) return { success: false, error: 'Element not found' };
    el.scrollIntoView({ block: 'center', inline: 'center' });
    if (${js(action)} === 'click') {
      el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }));
      el.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true }));
      el.click();
      if (${doubleClick}) el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      return { success: true, text: 'Clicked element' };
    }
    if (${js(action)} === 'hover') {
      el.dispatchEvent(new MouseEvent('mouseover', { bubbles: true, cancelable: true }));
      el.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, cancelable: true }));
      return { success: true, text: 'Hovered element' };
    }
    if (${js(action)} === 'select') {
      el.focus();
      el.value = ${js(value)};
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      return { success: true, text: 'Selected option' };
    }
    el.focus();
    el.value = ${js(text)};
    el.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: ${js(text)} }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    if (${submit}) {
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      el.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', bubbles: true }));
      const form = el.closest('form');
      if (form) form.requestSubmit ? form.requestSubmit() : form.submit();
    }
    return { success: true, text: 'Typed into element' };
  })()`;
}

function browserPressKeyScript(key: string): string {
  return `(() => {
    const key = ${js(key)};
    const el = document.activeElement || document.body;
    el.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    el.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true, cancelable: true }));
    return { success: true, text: 'Pressed key ' + key };
  })()`;
}

function browserFillFormScript(fields: unknown[]): string {
  return `(() => {
    const fields = ${js(fields)};
    window.__mergenMcpRefs = window.__mergenMcpRefs || {};
    function cssEscape(value) { return String(value).replace(/["\\\\]/g, '\\\\$&'); }
    function findField(field) {
      if (field.ref && window.__mergenMcpRefs[field.ref]) return window.__mergenMcpRefs[field.ref];
      if (field.ref) {
        const byRef = document.querySelector('[data-mergen-mcp-ref="' + cssEscape(field.ref) + '"]');
        if (byRef) return byRef;
      }
      if (field.selector) return document.querySelector(field.selector);
      if (field.target) return document.querySelector(field.target);
      if (field.name) return document.querySelector('[name="' + cssEscape(field.name) + '"]');
      return null;
    }
    const updated = [];
    for (const field of fields) {
      const el = findField(field);
      if (!el) continue;
      el.focus();
      if (field.type === 'checkbox' || field.type === 'radio') {
        el.checked = field.value === true || field.value === 'true' || field.value === '1';
      } else {
        el.value = String(field.value ?? '');
      }
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      updated.push(field.name || field.ref || field.selector || field.target || el.tagName);
    }
    return { success: updated.length === fields.length, text: 'Filled fields: ' + updated.join(', '), updated };
  })()`;
}

function browserPageSummaryScript(params: Record<string, unknown>): string {
  const maxItems = Math.max(1, Math.min(200, Number(params.maxItems || 40)));
  const query = String(params.query || '').toLowerCase();
  const includeBoxes = Boolean(params.includeBoxes);
  return `(() => {
    const maxItems = ${maxItems};
    const query = ${js(query)};
    const includeBoxes = ${includeBoxes};
    window.__mergenMcpRefs = {};
    const selector = 'a,button,input,textarea,select,[role],[onclick],[tabindex],label,summary';
    const candidates = Array.from(document.querySelectorAll(selector));
    function visible(el) {
      const rect = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0';
    }
    function role(el) {
      return el.getAttribute('role') || (el.tagName || '').toLowerCase();
    }
    function label(el) {
      return (el.innerText || el.value || el.getAttribute('aria-label') || el.getAttribute('title') || el.id || el.name || '').trim().replace(/\\s+/g, ' ').slice(0, 160);
    }
    const ranked = candidates
      .filter(visible)
      .map((el) => ({ el, label: label(el), haystack: (label(el) + ' ' + (el.id || '') + ' ' + (el.className || '')).toLowerCase() }))
      .sort((a, b) => {
        if (!query) return 0;
        const am = a.haystack.includes(query) ? 0 : 1;
        const bm = b.haystack.includes(query) ? 0 : 1;
        return am - bm;
      })
      .slice(0, maxItems);
    const items = ranked.map(({ el, label }, index) => {
      const ref = 'e' + (index + 1);
      window.__mergenMcpRefs[ref] = el;
      try { el.setAttribute('data-mergen-mcp-ref', ref); } catch {}
      const rect = el.getBoundingClientRect();
      const item = {
        ref,
        role: role(el),
        tag: (el.tagName || '').toLowerCase(),
        text: label,
        ariaLabel: el.getAttribute('aria-label') || '',
        title: el.getAttribute('title') || '',
        id: el.id || '',
        className: typeof el.className === 'string' ? el.className : '',
        href: el.href || '',
      };
      if (includeBoxes) item.box = { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
      return item;
    });
    const summary = items.length
      ? items.map((item) => item.ref + ' [' + item.role + '] ' + (item.text || item.ariaLabel || item.title || item.id || item.href || '(no label)')).join('\\n')
      : 'No visible browser elements found';
    return { success: true, url: location.href, title: document.title, itemCount: items.length, items, summary };
  })()`;
}

function browserSnapshotScript(params: Record<string, unknown>): string {
  return `(() => {
    ${browserFindElementPrelude(params)}
    const el = ${params.ref || params.selector ? 'findElement()' : 'document.documentElement'};
    if (!el) return { success: false, error: 'Element not found' };
    const text = (el.innerText || el.textContent || '').trim().replace(/\\s+/g, ' ').slice(0, 4000);
    return { success: true, text: text || el.outerHTML.slice(0, 4000), html: el.outerHTML.slice(0, 4000), url: location.href, title: document.title };
  })()`;
}

function browserWaitProbeScript(params: { selector: string; text: string; textGone: string }): string {
  return `(() => {
    const selector = ${js(params.selector)};
    const text = ${js(params.text)};
    const textGone = ${js(params.textGone)};
    const bodyText = document.body ? document.body.innerText || '' : '';
    if (selector && document.querySelector(selector)) return { success: true };
    if (text && bodyText.includes(text)) return { success: true };
    if (textGone && !bodyText.includes(textGone)) return { success: true };
    return { success: false };
  })()`;
}

function browserHighlightScript(params: Record<string, unknown>): string {
  const color = String(params.color || '#16a34a');
  const label = String(params.label || '');
  const padding = Math.max(0, Math.min(100, Number(params.padding ?? 8) || 0));
  const radius = Math.max(0, Math.min(100, Number(params.radius ?? 10) || 0));
  return `(() => {
    const color = ${js(color)};
    const padding = ${padding};
    const radius = ${radius};
    ${browserFindElementPrelude(params)}
    const el = findElement();
    if (!el) return { success: false, error: 'Element not found' };
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return { success: false, error: 'Element is not visible' };
    document.getElementById('__mergen-mcp-highlight')?.remove();
    const overlay = document.createElement('div');
    overlay.id = '__mergen-mcp-highlight';
    overlay.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;border:3px solid ' + color + ';border-radius:' + radius + 'px;box-shadow:0 0 0 9999px rgba(0,0,0,0.08);left:' + Math.max(0, rect.left - padding) + 'px;top:' + Math.max(0, rect.top - padding) + 'px;width:' + (rect.width + padding * 2) + 'px;height:' + (rect.height + padding * 2) + 'px;';
    if (${js(label)}) {
      const badge = document.createElement('div');
      badge.textContent = ${js(label)};
      badge.style.cssText = 'position:absolute;left:-3px;top:-24px;background:' + color + ';color:white;font:12px sans-serif;padding:2px 6px;border-radius:4px;';
      overlay.appendChild(badge);
    }
    document.body.appendChild(overlay);
    return { success: true, text: 'Highlighted element', box: { x: rect.x, y: rect.y, width: rect.width, height: rect.height } };
  })()`;
}

function browserStorageScript(tool: string, params: Record<string, unknown>): string {
  const storageName = tool.includes('sessionstorage') ? 'sessionStorage' : 'localStorage';
  const op = tool.split('_').pop() || 'list';
  const key = String(params.key || '');
  const value = String(params.value ?? '');
  return `(() => {
    const store = window[${js(storageName)}];
    const op = ${js(op)};
    const key = ${js(key)};
    if (op === 'clear') {
      store.clear();
      return { success: true, text: 'Cleared ${storageName}' };
    }
    if (op === 'delete') {
      store.removeItem(key);
      return { success: true, text: 'Deleted ${storageName} key ' + key };
    }
    if (op === 'get') {
      const value = store.getItem(key);
      return { success: value !== null, text: value ?? '', key, value };
    }
    if (op === 'set') {
      store.setItem(key, ${js(value)});
      return { success: true, text: 'Set ${storageName} key ' + key, key };
    }
    const keys = [];
    for (let i = 0; i < store.length; i += 1) keys.push(store.key(i));
    return { success: true, text: keys.join('\\n'), keys };
  })()`;
}

registerBrowserMcpHandler(handleBrowserMcpHttpRequest);

export function killBrowserMcpSession(sessionId: string): void {
  const session = sessions.get(sessionId);
  if (!session) return;
  session.process.kill();
  sessions.delete(sessionId);
}

export function getBrowserMcpCommandArray(): string[] {
  const helper = getBrowserMcpStdioConfig();
  return [helper.command, ...helper.args];
}

export function prepareBrowserMcpToolScope(terminalId: number, projectId: number): BrowserScopeKey {
  return { type: BrowserScopeKeyType.Terminal, projectId, terminalId };
}

export function getBrowserMcpSessionCount(): number {
  return sessions.size;
}
