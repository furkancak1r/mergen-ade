import { useEffect, useRef, useCallback } from 'react';
import type { TerminalKind, ShellKind, AiHookEvent, AiCliTool } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

export interface TerminalInstance {
  id: number;
  projectId: number;
  kind: TerminalKind;
  shell: ShellKind;
  cwd: string;
  cols: number;
  rows: number;
  title: string;
  aiTool?: AiCliTool;
  aiStatus: string;
  aiStatusReason?: string;
  recentInputs: string[];
}

export function usePty() {
  const terminalsRef = useRef<Map<number, TerminalInstance>>(new Map());
  const listenersRef = useRef<Set<() => void>>(new Set());

  const notify = useCallback(() => {
    listenersRef.current.forEach((cb) => cb());
  }, []);

  const createTerminal = useCallback(async (opts: {
    shell: ShellKind;
    cwd: string;
    cols: number;
    rows: number;
    projectId: number;
    kind: TerminalKind;
    env?: Record<string, string>;
  }) => {
    const id = await api.invoke('pty:create', opts) as number;
    terminalsRef.current.set(id, { id, projectId: opts.projectId, kind: opts.kind, shell: opts.shell, cwd: opts.cwd, cols: opts.cols, rows: opts.rows, title: '', aiStatus: 'inactive', recentInputs: [] });
    notify();
    return id;
  }, [notify]);

  const writeTerminal = useCallback((terminalId: number, data: string) => {
    api.invoke('pty:write', terminalId, data);
  }, []);

  const resizeTerminal = useCallback((terminalId: number, cols: number, rows: number) => {
    const t = terminalsRef.current.get(terminalId);
    if (t) {
      t.cols = cols;
      t.rows = rows;
    }
    api.invoke('pty:resize', terminalId, cols, rows);
  }, []);

  const killTerminal = useCallback((terminalId: number, signal?: string) => {
    api.invoke('pty:kill', terminalId, signal);
    terminalsRef.current.delete(terminalId);
    notify();
  }, [notify]);

  useEffect(() => {
    const unsubData = api.on('pty:data', (terminalId: number, data: string) => {
      // data events are consumed by TerminalPane directly via its own listener
    });
    const unsubExit = api.on('pty:exit', (terminalId: number, _exitCode: number) => {
      terminalsRef.current.delete(terminalId);
      notify();
    });
    const unsubHook = api.on('hook:status', (event: AiHookEvent) => {
      const t = terminalsRef.current.get(event.terminalId);
      if (t) {
        t.aiTool = event.tool;
        t.aiStatus = event.status;
        t.aiStatusReason = event.reason;
        notify();
      }
    });
    return () => {
      unsubData();
      unsubExit();
      unsubHook();
    };
  }, [notify]);

  const getTerminals = useCallback(() => {
    return Array.from(terminalsRef.current.values());
  }, []);

  const subscribe = useCallback((cb: () => void) => {
    listenersRef.current.add(cb);
    return () => { listenersRef.current.delete(cb); };
  }, []);

  return {
    createTerminal,
    writeTerminal,
    resizeTerminal,
    killTerminal,
    getTerminals,
    subscribe,
  };
}
