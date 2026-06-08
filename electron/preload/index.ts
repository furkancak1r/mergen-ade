import { contextBridge, ipcRenderer } from 'electron';
import type { IpcInvokeChannel, IpcSendChannel } from '../shared/types';

const api = {
  invoke: <K extends IpcInvokeChannel>(channel: K, ...args: unknown[]) => ipcRenderer.invoke(channel, ...args),
  on: <K extends IpcSendChannel>(channel: K, callback: (...args: any[]) => void) => {
    const wrapped = (_event: Electron.IpcRendererEvent, ...args: any[]) => callback(...args);
    ipcRenderer.on(channel, wrapped);
    return () => ipcRenderer.removeListener(channel, wrapped);
  },
  once: <K extends IpcSendChannel>(channel: K, callback: (...args: any[]) => void) => {
    const wrapped = (_event: Electron.IpcRendererEvent, ...args: any[]) => callback(...args);
    ipcRenderer.once(channel, wrapped);
    return () => ipcRenderer.removeListener(channel, wrapped);
  },
  removeAllListeners: <K extends IpcSendChannel>(channel: K) => ipcRenderer.removeAllListeners(channel),
};

contextBridge.exposeInMainWorld('mergenApi', api);

export type MergenApi = typeof api;
