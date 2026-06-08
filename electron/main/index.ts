import { app, BrowserWindow, ipcMain, Notification, dialog, shell, clipboard, nativeImage } from 'electron';
import path from 'path';
import fs from 'fs';
import { registerIpcHandlers } from './ipcHandlers';
import { handleBrowserMcpHelperMode } from './browserMcpHelper';
import { handleOpencodeNotifyMode } from './opencode';
import { handleCodexNotifyMode, handleCodexHookMode } from './codex';
import { startHookService, stopHookService } from './hookService';

const isDev = !app.isPackaged;
let mainWindow: BrowserWindow | null = null;
let pendingClose = false;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1600,
    height: 980,
    minWidth: 980,
    minHeight: 620,
    title: 'Mergen ADE',
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  if (isDev) {
    mainWindow.loadURL('http://localhost:5173');
  } else {
    mainWindow.loadFile(path.join(__dirname, '../renderer/dist/index.html'));
  }

  mainWindow.once('ready-to-show', () => {
    mainWindow?.show();
  });

  mainWindow.on('close', (e) => {
    if (pendingClose) {
      return;
    }
    e.preventDefault();
    pendingClose = true;
    mainWindow?.webContents.send('window:closeRequest');
  });

  mainWindow.on('closed', () => {
    mainWindow = null;
  });

  mainWindow.on('focus', () => {
    mainWindow?.webContents.send('window:focused', true);
  });

  mainWindow.on('blur', () => {
    mainWindow?.webContents.send('window:focused', false);
  });
}

function confirmClose(confirmed: boolean) {
  if (!mainWindow) return;
  if (confirmed) {
    pendingClose = false;
    mainWindow.destroy();
  } else {
    pendingClose = false;
  }
}

function showNotification(payload: { title: string; body: string }) {
  if (!mainWindow) return;
  const notification = new Notification({
    title: payload.title,
    body: payload.body,
  });
  notification.on('click', () => {
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });
  notification.show();
  if (!mainWindow.isFocused()) {
    mainWindow.flashFrame(true);
  }
}

app.whenReady().then(() => {
  registerIpcHandlers();
  startHookService();

  ipcMain.handle('window:confirmClose', (_event, confirmed: boolean) => {
    confirmClose(confirmed);
  });

  ipcMain.on('notify:show', (_event, payload) => {
    showNotification(payload);
  });

  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  stopHookService();
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// CLI mode dispatch
function dispatchCliMode() {
  const args = process.argv.slice(2);
  if (args.length === 0) return false;
  const mode = args[0];
  switch (mode) {
    case '--browser-mcp-helper': {
      handleBrowserMcpHelperMode();
      return true;
    }
    case '--opencode-notify': {
      const ok = handleOpencodeNotifyMode();
      if (!ok) process.exit(1);
      return true;
    }
    case '--codex-notify': {
      const ok = handleCodexNotifyMode();
      if (!ok) process.exit(1);
      return true;
    }
    case '--codex-hook': {
      if (args.length < 2) {
        console.error('Missing Codex hook event argument.');
        process.exit(1);
      }
      const eventName = args[1];
      handleCodexHookMode(eventName);
      return true;
    }
    default:
      return false;
  }
}

if (dispatchCliMode()) {
  app.quit();
}

export { mainWindow };
