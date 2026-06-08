import { ipcMain, dialog, shell, clipboard } from 'electron';
import type { BrowserScopeKey } from '../shared/types';
import fs from 'fs';
import path from 'path';
import { loadConfig, saveConfig, loadHistory, saveHistory } from './config';
import { createTerminal, writeTerminal, resizeTerminal, killTerminal } from './pty';
import { discoverWorktrees, createWorktree, removeWorktree, getGitStatus } from './worktree';
import { spawnAcpChat, sendAcpPrompt, cancelAcpPrompt, setAcpConfigOption, getAcpSession, killAcpChat } from './acpService';
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
} from './browserViewManager';

export function registerIpcHandlers() {
  // Config
  ipcMain.handle('config:load', () => loadConfig());
  ipcMain.handle('config:save', (_event, config) => saveConfig(config));
  ipcMain.handle('history:load', () => loadHistory());
  ipcMain.handle('history:save', (_event, history) => saveHistory(history));

  // PTY
  ipcMain.handle('pty:create', (_event, opts) => createTerminal(opts));
  ipcMain.handle('pty:write', (_event, terminalId, data) => writeTerminal(terminalId, data));
  ipcMain.handle('pty:resize', (_event, terminalId, cols, rows) => resizeTerminal(terminalId, cols, rows));
  ipcMain.handle('pty:kill', (_event, terminalId, signal?) => killTerminal(terminalId, signal));

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
    return { dataUrl: img.toDataURL() };
  });

  // Shell
  ipcMain.handle('shell:openExternal', async (_event, url: string) => {
    await shell.openExternal(url);
  });

  // ACP
  ipcMain.handle('acp:spawn', async (_event, opts: { projectId: number; cwd: string; mcpServers: string[] }) => {
    return spawnAcpChat(opts);
  });
  ipcMain.handle('acp:send', async (_event, opts: { chatId: string; promptText: string; attachments: string[] }) => {
    sendAcpPrompt(opts.chatId, opts.promptText, opts.attachments);
  });
  ipcMain.handle('acp:cancel', async (_event, chatId: string) => {
    cancelAcpPrompt(chatId);
  });
  ipcMain.handle('acp:setConfigOption', async (_event, opts: { chatId: string; configId: string; value: string }) => {
    setAcpConfigOption(opts.chatId, opts.configId, opts.value);
  });
  ipcMain.handle('acp:getSession', async (_event, chatId: string) => {
    return getAcpSession(chatId);
  });
  ipcMain.handle('acp:kill', async (_event, chatId: string) => {
    killAcpChat(chatId);
  });

  // Git / Worktree
  ipcMain.handle('git:status', async (_event, repoPath: string) => {
    return getGitStatus(repoPath);
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
    try {
      const entries = await fs.promises.readdir(sourcePath);
      const envFiles = entries.filter((e) => e.startsWith('.env'));
      for (const file of envFiles) {
        await fs.promises.copyFile(path.join(sourcePath, file), path.join(targetPath, file));
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
}
