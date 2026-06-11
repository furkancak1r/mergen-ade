import { app, BrowserWindow, ipcMain, Notification, dialog, shell, clipboard, nativeImage, Tray } from 'electron';
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
    const devPort = process.env.VITE_DEV_PORT || '5174';
    mainWindow.loadURL(`http://localhost:${devPort}`);
  } else {
    mainWindow.loadFile(path.join(__dirname, '../dist/index.html'));
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

// OS Notifications with tray icon and cooldown
let tray: Tray | null = null;
const lastNotificationByTerminal = new Map<string, number>();

function ensureTray() {
  if (tray) return;
  // Create an empty transparent image for the tray icon
  const icon = nativeImage.createEmpty();
  tray = new Tray(icon);
  tray.setToolTip('Mergen ADE');
  tray.on('click', () => {
    if (mainWindow) {
      // Only restore if minimized; do not un-maximize a visible window
      if (mainWindow.isMinimized()) {
        mainWindow.restore();
      }
      mainWindow.show();
      mainWindow.focus();
    }
  });
}

function showNotification(payload: {
  terminalId: number;
  tool: string;
  kind: string;
  title: string;
  body: string;
  onlyWhenUnfocused?: boolean;
  cooldownSecs?: number;
}): void {
  if (!mainWindow) return;

  const focused = mainWindow.isFocused();
  if (payload.onlyWhenUnfocused && focused) {
    return;
  }

  // Cooldown deduplication
  const key = `${payload.terminalId}-${payload.tool}-${payload.kind}`;
  const cooldownMs = (payload.cooldownSecs ?? 30) * 1000;
  const last = lastNotificationByTerminal.get(key);
  const now = Date.now();
  if (last && now - last < cooldownMs) {
    return;
  }
  lastNotificationByTerminal.set(key, now);

  // Try tray balloon first on Windows
  if (process.platform === 'win32') {
    try {
      ensureTray();
      if (tray) {
        tray.displayBalloon({
          iconType: 'none',
          title: payload.title,
          content: payload.body,
        });
      }
    } catch {
      // Fallback to Notification API
      const n = new Notification({ title: payload.title, body: payload.body });
      n.show();
    }
  } else {
    const n = new Notification({
      title: payload.title,
      body: payload.body,
    });
    n.on('click', () => {
      if (mainWindow) {
        if (mainWindow.isMinimized()) mainWindow.restore();
        mainWindow.focus();
      }
    });
    n.show();
  }

  // Flash frame fallback
  if (!focused) {
    mainWindow.flashFrame(true);
  }
}

function clearTray() {
  if (tray) {
    tray.destroy();
    tray = null;
  }
}

app.whenReady().then(() => {
  console.log('Electron app ready');
  registerIpcHandlers();
  startHookService();

  ipcMain.handle('window:confirmClose', (_event, confirmed: boolean) => {
    confirmClose(confirmed);
  });

  ipcMain.handle('notify:show', (_event, payload) => {
    showNotification(payload);
  });

  createWindow();
  console.log('Window created');

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

// Crash shield: catch unhandled errors and log without crashing
process.on('uncaughtException', (err) => {
  console.error('Uncaught exception:', err);
});

process.on('unhandledRejection', (reason) => {
  console.error('Unhandled rejection:', reason);
});

app.on('window-all-closed', () => {
  stopHookService();
  clearTray();
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// CLI mode dispatch
function dispatchCliMode() {
  const args = process.argv.slice(2);
  console.log('CLI args:', args);
  if (args.length === 0) return false;
  const mode = args[0];
  console.log('CLI mode:', mode);
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
  // Browser MCP helper runs as a long-lived child process; do not exit
  if (process.argv[2] !== '--browser-mcp-helper') {
    process.exit(0);
  }
}

export { mainWindow };
