import { ipcMain, dialog, shell, clipboard } from 'electron';
import type { BrowserScopeKey } from '../shared/types';
import fs from 'fs';
import path from 'path';
import { loadConfig, saveConfig, loadHistory, saveHistory, repairMojibakePath } from './config';
import { createTerminal, writeTerminal, resizeTerminal, killTerminal, getTerminalState } from './pty';
import { discoverWorktrees, createWorktree, removeWorktree, getGitStatus, getGitDiffSummary } from './worktree';
import { spawnAcpChat, sendAcpPrompt, cancelAcpPrompt, setAcpConfigOption, sendAcpPermissionResponse, sendAcpQuestionResponse, getAcpSession, killAcpChat, warmAcpStandby, getAcpStandby, clearAcpStandby, promoteAcpStandby, clearAllAcpStandby } from './acpService';
import { submitAnswer } from './hookService';
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
  browserDesignInspect,
  browserAddTab,
  browserCloseTab,
  browserSwitchTab,
  destroyBrowserInstance,
  hideAllBrowserViews,
  showAllBrowserViews,
  setActiveBrowserScope,
  showActiveBrowserView,
} from './browserViewManager';
import {
  spawnBrowserMcpSession,
  executeBrowserMcpTool,
  killBrowserMcpSession,
  prepareBrowserMcpToolScope,
  getBrowserMcpCommandArray,
} from './browserMcpService';
import { generateOpencodeTerminalConfig, generateOpencodeRuntimeConfig } from './opencode';
import { getAppDiagnostics } from './diagnostics';

export function registerIpcHandlers() {
  // Config
  ipcMain.handle('config:load', () => loadConfig());
  ipcMain.handle('config:save', (_event, config) => saveConfig(config));
  ipcMain.handle('history:load', () => loadHistory());
  ipcMain.handle('history:save', (_event, history) => saveHistory(history));
  ipcMain.handle('diagnostics:get', () => getAppDiagnostics());

  // PTY
  ipcMain.handle('pty:create', (_event, opts) => createTerminal(opts));
  ipcMain.handle('pty:write', (_event, terminalId, data) => writeTerminal(terminalId, data));
  ipcMain.handle('pty:resize', (_event, terminalId, cols, rows) => resizeTerminal(terminalId, cols, rows));
  ipcMain.handle('pty:kill', (_event, terminalId, signal?) => killTerminal(terminalId, signal));
  ipcMain.handle('pty:getState', (_event, terminalId: number) => getTerminalState(terminalId));

  // FS
  ipcMain.handle('fs:readDir', async (_event, dirPath: string) => {
    const entries = await fs.promises.readdir(dirPath, { withFileTypes: true });
    return entries.map((e) => ({
      name: e.name,
      isDirectory: e.isDirectory(),
      isSymlink: e.isSymbolicLink(),
    }));
  });
  ipcMain.handle('fs:readFile', async (_event, filePath: string) => {
    return fs.promises.readFile(filePath, 'utf-8');
  });
  ipcMain.handle('fs:writeFile', async (_event, filePath: string, text: string) => {
    await fs.promises.writeFile(filePath, text, 'utf-8');
  });
  ipcMain.handle('fs:exists', async (_event, filePath: string) => {
    try {
      await fs.promises.access(filePath);
      return true;
    } catch {
      return false;
    }
  });
  ipcMain.handle('fs:stat', async (_event, filePath: string) => {
    const s = await fs.promises.stat(filePath);
    return { isDirectory: s.isDirectory(), isFile: s.isFile(), size: s.size, mtimeMs: s.mtimeMs };
  });

  // Dialog
  ipcMain.handle('dialog:showOpen', async (_event, opts) => {
    const result = await dialog.showOpenDialog(opts);
    return result.canceled ? undefined : result.filePaths;
  });
  ipcMain.handle('dialog:showSave', async (_event, opts) => {
    const result = await dialog.showSaveDialog(opts);
    return result.canceled ? undefined : result.filePath;
  });

  // Clipboard
  ipcMain.handle('clipboard:readText', () => clipboard.readText());
  ipcMain.handle('clipboard:writeText', (_event, text: string) => clipboard.writeText(text));
  ipcMain.handle('clipboard:readImage', async () => {
    const img = clipboard.readImage();
    if (img.isEmpty()) return undefined;
    // Save bitmap to screenshots folder so it can be pasted as a path
    const screenshotsDir = path.join(process.env.APPDATA || process.env.HOME || '.', 'Mergen', 'MergenADE', 'screenshots');
    if (!fs.existsSync(screenshotsDir)) {
      fs.mkdirSync(screenshotsDir, { recursive: true });
    }
    const filename = `screenshot-${Date.now()}.png`;
    const filePath = path.join(screenshotsDir, filename);
    const pngBuffer = img.toPNG();
    await fs.promises.writeFile(filePath, pngBuffer);
    return { path: filePath, dataUrl: img.toDataURL() };
  });
  ipcMain.handle('clipboard:readFilePaths', async () => {
    // On Windows, try to read CF_HDROP via buffer if available
    try {
      const buffer = clipboard.readBuffer('CF_HDROP');
      if (buffer && buffer.length > 0) {
        // Parse DROPFILES structure from buffer
        // DROPFILES: pFiles(4) + pt(8) + fNC(4) + fWide(4) = 20 bytes header
        const offset = buffer.readUInt32LE(0);
        const fWide = buffer.readUInt32LE(16);
        const fileList = fWide
          ? buffer.slice(offset).toString('ucs2')
          : buffer.slice(offset).toString('latin1');
        const paths = fileList.split('\0').filter((p) => p.trim());
        const repairedPaths = await Promise.all(paths.map(async (p) => repairMojibakePath(p, async (rp) => { try { await fs.promises.access(rp); return true; } catch { return false; } })));
        return repairedPaths;
      }
    } catch {
      // Fallback: try reading text as paths
    }
    const text = clipboard.readText();
    if (text) {
      const lines = text.split('\n').map((l) => l.trim()).filter((l) => l);
      const validPaths = await Promise.all(lines.map(async (l) => {
        const repaired = await repairMojibakePath(l, async (rp) => { try { await fs.promises.access(rp); return true; } catch { return false; } });
        return fs.existsSync(repaired) ? repaired : undefined;
      }));
      const filtered = validPaths.filter((p): p is string => p !== undefined);
      if (filtered.length > 0) return filtered;
    }
    return undefined;
  });

  // Shell
  ipcMain.handle('shell:openExternal', async (_event, url: string) => {
    await shell.openExternal(url);
  });
  ipcMain.handle('shell:openPath', async (_event, filePath: string) => {
    return shell.openPath(filePath);
  });
  ipcMain.handle('shell:showItemInFolder', async (_event, filePath: string) => {
    shell.showItemInFolder(filePath);
  });

  // ACP
  ipcMain.handle('acp:spawn', async (_event, opts: { projectId: number; cwd: string; mcpServers: string[] }) => {
    return spawnAcpChat(opts);
  });
  ipcMain.handle('acp:send', async (_event, opts: { chatId: string; promptText: string; attachments: string[]; modeId?: string }) => {
    sendAcpPrompt(opts.chatId, opts.promptText, opts.attachments, opts.modeId);
  });
  ipcMain.handle('acp:cancel', async (_event, chatId: string) => {
    cancelAcpPrompt(chatId);
  });
  ipcMain.handle('acp:setConfigOption', async (_event, opts: { chatId: string; configId: string; value: string }) => {
    setAcpConfigOption(opts.chatId, opts.configId, opts.value);
  });
  ipcMain.handle('acp:permissionResponse', async (_event, opts: { chatId: string; requestId: string; answers: string[]; rejected: boolean }) => {
    return sendAcpPermissionResponse(opts.chatId, opts.requestId, opts.answers, opts.rejected);
  });
  ipcMain.handle('acp:questionResponse', async (_event, opts: { chatId: string; requestId: string; answers: string[][]; rejected: boolean }) => {
    return sendAcpQuestionResponse(opts.chatId, opts.requestId, opts.answers, opts.rejected);
  });
  ipcMain.handle('acp:getSession', async (_event, chatId: string) => {
    return getAcpSession(chatId);
  });
  ipcMain.handle('acp:kill', async (_event, chatId: string) => {
    killAcpChat(chatId);
  });
  ipcMain.handle('acp:standby:warm', async (_event, projectId: number, cwd: string) => {
    return warmAcpStandby(projectId, cwd);
  });
  ipcMain.handle('acp:standby:get', async (_event, projectId: number) => {
    return getAcpStandby(projectId);
  });
  ipcMain.handle('acp:standby:clear', async (_event, projectId: number) => {
    return clearAcpStandby(projectId);
  });
  ipcMain.handle('acp:standby:promote', async (_event, projectId: number, visibleChatId: string) => {
    return promoteAcpStandby(projectId, visibleChatId);
  });
  ipcMain.handle('acp:standby:clearAll', async () => {
    return clearAllAcpStandby();
  });

  // Hook answer bridge
  ipcMain.handle('hook:answer', async (_event, answer: { requestId: string; answers: string[]; rejected: boolean }) => {
    submitAnswer(answer);
  });

  // Git / Worktree
  ipcMain.handle('git:diffSummary', async (_event, repoPath: string) => {
    return getGitDiffSummary(repoPath);
  });
  ipcMain.handle('git:status', async (_event, repoPath: string, runFetch?: boolean) => {
    return getGitStatus(repoPath, Boolean(runFetch));
  });
  ipcMain.handle('git:discoverWorktrees', async (_event, repoPath: string) => {
    return discoverWorktrees(repoPath);
  });
  ipcMain.handle('git:createWorktree', async (_event, repoPath: string, branch: string, worktreePath: string, baseBranch?: string) => {
    return createWorktree(repoPath, branch, worktreePath, baseBranch);
  });
  ipcMain.handle('git:removeWorktree', async (_event, repoPath: string, worktreePath: string) => {
    return removeWorktree(repoPath, worktreePath);
  });
  ipcMain.handle('git:copyEnvFiles', async (_event, sourcePath: string, targetPath: string) => {
    if (!sourcePath || typeof sourcePath !== 'string' || !targetPath || typeof targetPath !== 'string') {
      return false;
    }
    try {
      const entries = await fs.promises.readdir(sourcePath);
      const envFiles = entries.filter((e) => e.startsWith('.env'));
      for (const file of envFiles) {
        await fs.promises.copyFile(sourcePath + path.sep + file, targetPath + path.sep + file);
      }
      return true;
    } catch {
      return false;
    }
  });

  // Browser
  ipcMain.handle('browser:navigate', async (_event, opts: { scope: BrowserScopeKey; url: string }) => {
    navigateBrowser(opts.scope, opts.url);
  });
  ipcMain.handle('browser:syncBounds', async (_event, opts: { scope: BrowserScopeKey; x: number; y: number; width: number; height: number }) => {
    syncBrowserBounds(opts.scope, { x: opts.x, y: opts.y, width: opts.width, height: opts.height });
  });
  ipcMain.handle('browser:hide', async (_event, scope: BrowserScopeKey) => {
    hideBrowserView(scope);
  });
  ipcMain.handle('browser:show', async (_event, scope: BrowserScopeKey) => {
    showBrowserView(scope);
  });
  ipcMain.handle('browser:goBack', async (_event, scope: BrowserScopeKey) => {
    browserGoBack(scope);
  });
  ipcMain.handle('browser:goForward', async (_event, scope: BrowserScopeKey) => {
    browserGoForward(scope);
  });
  ipcMain.handle('browser:reload', async (_event, scope: BrowserScopeKey) => {
    browserReload(scope);
  });
  ipcMain.handle('browser:executeJs', async (_event, opts: { scope: BrowserScopeKey; script: string }) => {
    return browserExecuteJs(opts.scope, opts.script);
  });
  ipcMain.handle('browser:screenshot', async (_event, opts: { scope: BrowserScopeKey; fullPage: boolean }) => {
    return browserScreenshot(opts.scope, opts.fullPage);
  });
  ipcMain.handle('browser:designInspect', async (_event, opts: { scope: BrowserScopeKey; enabled: boolean }) => {
    browserDesignInspect(opts.scope, opts.enabled);
  });
  ipcMain.handle('browser:addTab', async (_event, scope: BrowserScopeKey, url?: string) => {
    return browserAddTab(scope, url);
  });
  ipcMain.handle('browser:closeTab', async (_event, opts: { scope: BrowserScopeKey; tabId: string }) => {
    browserCloseTab(opts.scope, opts.tabId);
  });
  ipcMain.handle('browser:switchTab', async (_event, opts: { scope: BrowserScopeKey; tabId: string }) => {
    browserSwitchTab(opts.scope, opts.tabId);
  });
  ipcMain.handle('browser:hideAll', async () => {
    hideAllBrowserViews();
  });
  ipcMain.handle('browser:showAll', async () => {
    showAllBrowserViews();
  });
  ipcMain.handle('browser:showActive', async (_event, scope: BrowserScopeKey) => {
    setActiveBrowserScope(scope);
    showActiveBrowserView();
  });
  ipcMain.handle('browser:destroyInstance', async (_event, scope: BrowserScopeKey) => {
    destroyBrowserInstance(scope);
  });

  // OpenCode config generation
  ipcMain.handle('opencode:generateTerminalConfig', async (_event, opts: { cwd: string; model?: string; effort?: string; kimiStrictPermissions?: boolean }) => {
    return generateOpencodeTerminalConfig(opts.cwd, { model: opts.model, effort: opts.effort, kimiStrictPermissions: opts.kimiStrictPermissions });
  });
  ipcMain.handle('opencode:generateRuntimeConfig', async (_event, opts: { cwd: string; model?: string; effort?: string; mcpServers?: string[]; kimiStrictPermissions?: boolean }) => {
    return generateOpencodeRuntimeConfig(opts.cwd, { model: opts.model, effort: opts.effort, mcpServers: opts.mcpServers, kimiStrictPermissions: opts.kimiStrictPermissions });
  });

  // Browser MCP
  ipcMain.handle('browserMcp:spawn', async (_event, opts: { sessionId: string; scope: BrowserScopeKey }) => {
    return spawnBrowserMcpSession(opts.sessionId, opts.scope);
  });
  ipcMain.handle('browserMcp:execute', async (_event, opts: { sessionId: string; method: string; params: unknown }) => {
    return executeBrowserMcpTool(opts.sessionId, opts.method, opts.params);
  });
  ipcMain.handle('browserMcp:kill', async (_event, sessionId: string) => {
    killBrowserMcpSession(sessionId);
  });
  ipcMain.handle('browserMcp:getCommand', async () => {
    return getBrowserMcpCommandArray();
  });
  ipcMain.handle('browserMcp:prepareScope', async (_event, terminalId: number, projectId: number) => {
    return prepareBrowserMcpToolScope(terminalId, projectId);
  });
}
