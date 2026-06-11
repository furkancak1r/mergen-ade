import { app } from 'electron';
import fs from 'fs';
import os from 'os';
import path from 'path';
import type { AppDiagnostics } from '../shared/types';
import { codexBridgePath, configPath, historyPath, legacyConfigPath } from './config';
import { getBrowserMcpCommandArray, getBrowserMcpSessionCount } from './browserMcpService';
import { getCodexInboxDir } from './codex';
import { getHookInboxDir, getHookServicePort } from './hookService';

export function getAppDiagnostics(): AppDiagnostics {
  const codexHooksPath = path.join(os.homedir(), '.codex', 'hooks.json');
  const bridgePath = codexBridgePath();

  return {
    appVersion: app.getVersion(),
    platform: process.platform,
    arch: process.arch,
    electronVersion: process.versions.electron || '',
    chromeVersion: process.versions.chrome || '',
    nodeVersion: process.versions.node || '',
    execPath: process.execPath,
    cwd: process.cwd(),
    configPath: configPath(),
    legacyConfigPath: legacyConfigPath(),
    historyPath: historyPath(),
    hookInboxDir: getHookInboxDir(),
    hookServicePort: getHookServicePort(),
    codexInboxDir: getCodexInboxDir(),
    codexHooksPath,
    codexHooksInstalled: fs.existsSync(codexHooksPath),
    codexBridgePath: bridgePath,
    codexBridgeInstalled: fs.existsSync(bridgePath),
    browserMcpCommand: getBrowserMcpCommandArray(),
    browserMcpSessionCount: getBrowserMcpSessionCount(),
  };
}
