import React, { useEffect, useRef, useState, useCallback } from 'react';
import type { AppConfig, ShellKind, TerminalKind, ProjectRecord, LeftSidebarTab, BrowserScopeKey, SmartInputState, SmartInputAttachment, AiHookEvent, AiCliAttentionKind, TerminalShortcutEntry, BrowserTab } from '../../shared/types';
import { BrowserScopeKeyType } from '../../shared/types';
import { LeftSidebarTab as LeftSidebarTabEnum, defaultAppConfig } from '../../shared/types';
import { MainArea } from './components/MainArea';
import { ProjectExplorer } from './components/ProjectExplorer';
import { TerminalManager } from './components/TerminalManager';
import { SourceControl } from './components/SourceControl';
import { FileEditor } from './components/FileEditor';
import { Checklist } from './components/Checklist';
import { AcpChatPanel } from './components/AcpChatPanel';
import { BrowserPanel } from './components/BrowserPanel';
import { SettingsPopup } from './components/SettingsPopup';
import { InputHistory } from './components/InputHistory';
import { usePty } from './hooks/usePty';
import { activeBrowserScope as resolveActiveBrowserScope, scopeKeyString } from './lib/browserScope';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [activeTab, setActiveTab] = useState<LeftSidebarTab>(LeftSidebarTabEnum.Directory);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [activeTerminalId, setActiveTerminalId] = useState<number | null>(null);
  const [terminals, setTerminals] = useState<ReturnType<ReturnType<typeof usePty>['getTerminals']>>([]);
  const [mainSize, setMainSize] = useState({ width: 800, height: 600 });
  const mainRef = useRef<HTMLDivElement>(null);
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [isResizing, setIsResizing] = useState(false);
  const sidebarRef = useRef<HTMLDivElement>(null);

  const [fileEditorOpen, setFileEditorOpen] = useState(false);
  const [fileEditorPath, setFileEditorPath] = useState<string | null>(null);
  const [fileEditorName, setFileEditorName] = useState<string | null>(null);
  const [fileEditorDirty, setFileEditorDirty] = useState(false);

  const [checklistVisible, setChecklistVisible] = useState(false);

  const [activeAcpChat, setActiveAcpChat] = useState<{ chatId: string; projectId: number } | null>(null);
  const [activeAcpChatByProject, setActiveAcpChatByProject] = useState<Map<number, string>>(new Map());
  const [acpRunning, setAcpRunning] = useState(false);
  const [acpQueuedPrompts, setAcpQueuedPrompts] = useState(false);

  const [browserOpenProjects, setBrowserOpenProjects] = useState<Set<number>>(new Set());
  const [browserPanelWidth, setBrowserPanelWidth] = useState(520);
  const [isResizingBrowser, setIsResizingBrowser] = useState(false);

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [branchNameByProject, setBranchNameByProject] = useState<Map<number, string>>(new Map());

  const [activeBrowserScope, setActiveBrowserScope] = useState<BrowserScopeKey | null>(null);
  const [browserPanelVisibleScopeByProject, setBrowserPanelVisibleScopeByProject] = useState<Map<number, BrowserScopeKey>>(new Map());

  // Browser state keyed by scope (must persist across project switches)
  const [browserTabsByScope, setBrowserTabsByScope] = useState<Map<string, BrowserTab[]>>(new Map());
  const [browserActiveTabByScope, setBrowserActiveTabByScope] = useState<Map<string, string | null>>(new Map());
  const [browserUrlDraftByScope, setBrowserUrlDraftByScope] = useState<Map<string, string>>(new Map());
  const [browserDesignInspectByScope, setBrowserDesignInspectByScope] = useState<Map<string, boolean>>(new Map());

  const pty = usePty();
  const suppressAcpRestoreRef = useRef(false);

  useEffect(() => {
    api.invoke('config:load').then((cfg) => {
      const loaded = cfg as AppConfig;
      setConfig(loaded);
      setActiveTab(loaded.ui.leftSidebarTab);
      setSidebarWidth(loaded.ui.projectExplorerWidth);
      setBrowserPanelWidth(loaded.ui.browserPanelWidth);
      if (loaded.projects.length > 0) {
        const lastId = loaded.ui.lastSelectedProjectId;
        const match = lastId ? loaded.projects.find((p) => p.id === lastId) : undefined;
        setSelectedProjectId(match ? match.id : loaded.projects[0].id);
      }
    });
  }, []);

  useEffect(() => {
    if (!config) return;
    const timer = setTimeout(() => {
      api.invoke('config:save', config);
    }, 500);
    return () => clearTimeout(timer);
  }, [config]);

  // Persist UI state changes to config
  useEffect(() => {
    if (!config || config.ui.leftSidebarTab === activeTab) return;
    setConfig((prev) => prev ? { ...prev, ui: { ...prev.ui, leftSidebarTab: activeTab } } : prev);
  }, [activeTab]);

  useEffect(() => {
    if (!config || config.ui.projectExplorerWidth === sidebarWidth) return;
    setConfig((prev) => prev ? { ...prev, ui: { ...prev.ui, projectExplorerWidth: sidebarWidth } } : prev);
  }, [sidebarWidth]);

  useEffect(() => {
    if (!config || config.ui.browserPanelWidth === browserPanelWidth) return;
    setConfig((prev) => prev ? { ...prev, ui: { ...prev.ui, browserPanelWidth } } : prev);
  }, [browserPanelWidth]);

  useEffect(() => {
    if (!config || config.ui.lastSelectedProjectId === selectedProjectId) return;
    setConfig((prev) => prev ? { ...prev, ui: { ...prev.ui, lastSelectedProjectId: selectedProjectId ?? undefined } } : prev);
  }, [selectedProjectId]);

  useEffect(() => {
    const unsub = pty.subscribe(() => {
      setTerminals(pty.getTerminals());
    });
    return () => { unsub(); };
  }, [pty]);

  useEffect(() => {
    const unsub = (window as unknown as { mergenApi: { on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi.on('window:closeRequest', () => {
      const parts: string[] = [];
      if (fileEditorDirty) parts.push('You have unsaved changes in the file editor.');
      if (terminals.length > 0) parts.push(`${terminals.length} terminal session(s) are still running.`);
      if (acpRunning) parts.push('An ACP chat session is still running.');
      if (acpQueuedPrompts) parts.push('You have queued prompts in the ACP chat.');
      const message = parts.length > 0
        ? `Are you sure you want to close Mergen ADE?\n\n${parts.join('\n')}`
        : 'Are you sure you want to close Mergen ADE?';
      const ok = window.confirm(message);
      if (ok) {
        api.invoke('window:confirmClose', true);
      }
    });
    return () => { unsub(); };
  }, [fileEditorDirty, terminals.length, acpRunning, acpQueuedPrompts]);

  // Track ACP running state and queued prompts
  useEffect(() => {
    const unsub = api.on('acp:event', (eventChatId: string, event: { type: string; status?: string; queuedPrompts?: number; count?: number }) => {
      if (activeAcpChat && eventChatId === activeAcpChat.chatId) {
        setAcpRunning(event.status === 'running' || event.status === 'permission');
        const queued = event.queuedPrompts ?? event.count ?? 0;
        setAcpQueuedPrompts(queued > 0);
      }
    });
    return () => { unsub(); };
  }, [activeAcpChat]);

  // Browser hide on modal open (Settings, Checklist) with grace period after close
  useEffect(() => {
    if (settingsOpen || checklistVisible) {
      api.invoke('browser:hideAll');
      return;
    }
    const timer = setTimeout(() => {
      if (activeBrowserScope) {
        api.invoke('browser:showActive', activeBrowserScope);
      }
    }, 150);
    return () => clearTimeout(timer);
  }, [settingsOpen, checklistVisible, activeBrowserScope]);

  // OS notification handling for AI attention events
  useEffect(() => {
    const unsub = api.on('hook:status', (event: AiHookEvent) => {
      if (!config?.notifications?.enabled) return;
      if (event.status !== 'attention') return;

      const attentionKind = event.attentionKind;
      if (!attentionKind) return;

      // Check if this attention kind is enabled in settings
      const nc = config.notifications;
      if (attentionKind === 'permission' && !nc.onPermission) return;
      if (attentionKind === 'turn_complete' && !nc.onTurnComplete) return;
      if (attentionKind === 'session_error' && !nc.onSessionError) return;

      const toolName = event.tool.charAt(0).toUpperCase() + event.tool.slice(1);
      const kindLabel = attentionKind === 'permission' ? 'Permission' : attentionKind === 'turn_complete' ? 'Turn Complete' : attentionKind === 'session_error' ? 'Session Error' : 'Attention';
      const title = `${toolName} — ${kindLabel}`;
      const body = event.reason || 'Your AI assistant needs attention.';

      api.invoke('notify:show', {
        terminalId: event.terminalId,
        tool: event.tool,
        kind: attentionKind,
        title,
        body,
        onlyWhenUnfocused: nc.onlyWhenUnfocused,
        cooldownSecs: nc.cooldownSecs,
      });
    });
    return () => { unsub(); };
  }, [config]);

  // Persist browser URL for project-scoped browsers
  useEffect(() => {
    const unsub = api.on('browser:urlChanged', (scope: BrowserScopeKey, url: string) => {
      if (!config) return;
      if (scope.type !== BrowserScopeKeyType.Project) return;
      const project = config.projects.find((p) => p.id === scope.projectId);
      if (!project) return;
      if (project.browserLastUrl === url) return;
      setConfig((prev) => {
        if (!prev) return prev;
        const projects = prev.projects.map((p) =>
          p.id === scope.projectId ? { ...p, browserLastUrl: url } : p
        );
        return { ...prev, projects };
      });
    });
    return () => { unsub(); };
  }, [config]);

  const activateTerminal = useCallback((id: number) => {
    setActiveTerminalId(id);
    setFileEditorOpen(false);
    setActiveAcpChat(null);
    suppressAcpRestoreRef.current = true;
    const t = terminals.find((x) => x.id === id);
    if (t) {
      setSelectedProjectId(t.projectId);
    }
    // Clear terminal output focus override for the newly activated terminal
    pty.setTerminalOutputFocusOverride(id, false);
  }, [terminals, pty]);

  const restoreActiveAcpForProject = useCallback((projectId: number) => {
    const chatId = activeAcpChatByProject.get(projectId);
    if (chatId) {
      setActiveAcpChat({ chatId, projectId });
    }
  }, [activeAcpChatByProject]);

  // Restore ACP chat when switching to a project that has an active chat
  useEffect(() => {
    if (!selectedProjectId) return;
    if (suppressAcpRestoreRef.current) {
      suppressAcpRestoreRef.current = false;
      return;
    }
    const chatId = activeAcpChatByProject.get(selectedProjectId);
    if (chatId) {
      setActiveAcpChat({ chatId, projectId: selectedProjectId });
    }
  }, [selectedProjectId, activeAcpChatByProject]);

  const activeTerminals = terminals.filter((t) => {
    if (config?.ui.mainVisibilityMode === 'selected_project') {
      return t.projectId === selectedProjectId;
    }
    return true;
  });

  const mainAreaTerminals = config?.ui.multiTerminalViewEnabled
    ? activeTerminals
    : activeTerminals.filter((t) => t.id === activeTerminalId);

  const shortcutMatchesEvent = useCallback((shortcut: TerminalShortcutEntry, e: KeyboardEvent): boolean => {
    if (shortcut.key !== e.key) return false;
    const onMac = navigator.platform.toLowerCase().includes('mac');
    const ctrl = !!shortcut.modifiers.ctrl;
    const alt = !!shortcut.modifiers.alt;
    const shift = !!shortcut.modifiers.shift;
    const command = !!shortcut.modifiers.command;
    if (onMac) {
      return ctrl === e.ctrlKey && alt === e.altKey && shift === e.shiftKey && command === e.metaKey;
    }
    // Windows/Linux: treat ctrl=true,command=true as legacy Ctrl-only
    if (ctrl && command) {
      return e.ctrlKey && !e.altKey && shift === e.shiftKey && !e.metaKey;
    }
    // command-only shortcuts are unpressable on non-macOS
    if (!ctrl && command) {
      return false;
    }
    return ctrl === e.ctrlKey && alt === e.altKey && shift === e.shiftKey && command === e.metaKey;
  }, []);

  const formatShortcutCombo = useCallback((shortcut: TerminalShortcutEntry): string => {
    const parts: string[] = [];
    if (shortcut.modifiers.ctrl) parts.push('Ctrl');
    if (shortcut.modifiers.alt) parts.push('Alt');
    if (shortcut.modifiers.shift) parts.push('Shift');
    if (shortcut.modifiers.command) parts.push('Cmd');
    parts.push(shortcut.key);
    return parts.join('+');
  }, []);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!config) return;

    // ACP chat visible blocks terminal keyboard capture
    if (activeAcpChat) return;

    // File editor open blocks terminal keyboard capture
    if (fileEditorOpen) return;

    // Modal/popup open blocks terminal keyboard capture
    if (settingsOpen || checklistVisible) return;

    // Check if Smart Input has focus
    const smartInputFocused = document.activeElement?.closest('[data-smart-input]') !== null;

    const hasTextFocus = (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement || (e.target instanceof HTMLElement && e.target.isContentEditable));
    // Do not block shortcuts when Smart Input is focused; it should redirect shortcuts to queue
    if (hasTextFocus && !smartInputFocused) return;

    // Collect all enabled shortcuts that match this event
    const matched = config.terminalShortcuts.filter((s) => s.enabled && shortcutMatchesEvent(s, e));
    if (matched.length > 1) {
      e.preventDefault();
      const combo = formatShortcutCombo(matched[0]);
      console.warn(`Duplicate shortcut conflict: ${combo} is mapped to ${matched.length} commands. None executed.`);
      return;
    }
    if (matched.length === 1) {
      const shortcut = matched[0];
      e.preventDefault();
      if (smartInputFocused) {
        // Redirect to Smart Input queue
        const activeTerminal = terminals.find((t) => t.id === activeTerminalId);
        if (activeTerminal) {
          const task = { text: shortcut.command, attachments: [], modeId: 'build', afterDone: true };
          pty.updateSmartInputState(activeTerminal.id, {
            queue: [...activeTerminal.smartInputState.queue, task],
          });
        }
      } else {
        const activeTerminal = terminals.find((t) => t.id === activeTerminalId);
        if (activeTerminal) {
          pty.sendShortcutToTerminal(activeTerminal.id, shortcut.command);
        }
      }
      return;
    }

    // Ctrl+Arrow: grid navigation through filtered active terminals
    if (e.ctrlKey && !e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowLeft')) {
      e.preventDefault();
      const idx = activeTerminals.findIndex((t) => t.id === activeTerminalId);
      if (idx > 0) activateTerminal(activeTerminals[idx - 1].id);
    }
    if (e.ctrlKey && !e.altKey && (e.key === 'ArrowDown' || e.key === 'ArrowRight')) {
      e.preventDefault();
      const idx = activeTerminals.findIndex((t) => t.id === activeTerminalId);
      if (idx >= 0 && idx < activeTerminals.length - 1) activateTerminal(activeTerminals[idx + 1].id);
    }
    // Ctrl+Alt+Arrow: linear navigation through all terminals
    if (e.ctrlKey && e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowLeft')) {
      e.preventDefault();
      const idx = terminals.findIndex((t) => t.id === activeTerminalId);
      if (idx > 0) activateTerminal(terminals[idx - 1].id);
    }
    if (e.ctrlKey && e.altKey && (e.key === 'ArrowDown' || e.key === 'ArrowRight')) {
      e.preventDefault();
      const idx = terminals.findIndex((t) => t.id === activeTerminalId);
      if (idx >= 0 && idx < terminals.length - 1) activateTerminal(terminals[idx + 1].id);
    }
  }, [config, activeTerminals, activeTerminalId, pty, activateTerminal, activeAcpChat, terminals, shortcutMatchesEvent, formatShortcutCombo, settingsOpen, checklistVisible, fileEditorOpen]);

  useEffect(() => {
    if (!mainRef.current) return;
    const el = mainRef.current;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const cr = entry.contentRect;
        setMainSize({ width: cr.width, height: cr.height });
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  useEffect(() => {
    if (!isResizing) return;
    function handleMove(e: MouseEvent) {
      const newWidth = Math.max(200, Math.min(500, e.clientX));
      setSidebarWidth(newWidth);
    }
    function handleUp() {
      setIsResizing(false);
    }
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [isResizing]);

  useEffect(() => {
    if (!isResizingBrowser) return;
    function handleMove(e: MouseEvent) {
      if (!mainRef.current) return;
      const mainRect = mainRef.current.getBoundingClientRect();
      const newWidth = Math.max(240, Math.min(800, mainRect.right - e.clientX));
      setBrowserPanelWidth(newWidth);
    }
    function handleUp() {
      setIsResizingBrowser(false);
    }
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [isResizingBrowser]);

  const selectedProject = config?.projects.find((p) => p.id === selectedProjectId) ?? null;

  const isBrowserOpen = selectedProjectId !== null && browserOpenProjects.has(selectedProjectId);

  // Compute active browser scope: override first, then terminal-scoped, then project-scoped
  useEffect(() => {
    if (!isBrowserOpen || !selectedProjectId) {
      setActiveBrowserScope(null);
      return;
    }
    const scope = resolveActiveBrowserScope(
      selectedProjectId,
      activeTerminalId ?? undefined,
      browserPanelVisibleScopeByProject.get(selectedProjectId),
      (terminalId: number) => {
        const t = terminals.find((x) => x.id === terminalId);
        if (!t) return false;
        const key = scopeKeyString({ type: BrowserScopeKeyType.Terminal, projectId: t.projectId, terminalId: t.id });
        return browserTabsByScope.has(key) && (browserTabsByScope.get(key) ?? []).length > 0;
      },
      (projectId: number) => {
        const key = scopeKeyString({ type: BrowserScopeKeyType.Project, projectId });
        return browserTabsByScope.has(key) && (browserTabsByScope.get(key) ?? []).length > 0;
      },
    );
    setActiveBrowserScope(scope ?? null);
  }, [isBrowserOpen, selectedProjectId, activeTerminalId, terminals, browserPanelVisibleScopeByProject, browserTabsByScope]);

  // Clear visible scope override when active terminal has no browser tabs
  useEffect(() => {
    if (!selectedProjectId || !activeTerminalId) return;
    const t = terminals.find((x) => x.id === activeTerminalId);
    if (!t) return;
    const key = scopeKeyString({ type: BrowserScopeKeyType.Terminal, projectId: t.projectId, terminalId: t.id });
    const hasTabs = browserTabsByScope.has(key) && (browserTabsByScope.get(key) ?? []).length > 0;
    if (!hasTabs) {
      setBrowserPanelVisibleScopeByProject((prev) => {
        const override = prev.get(selectedProjectId);
        if (override && override.type === BrowserScopeKeyType.Terminal && override.terminalId === activeTerminalId) {
          const copy = new Map(prev);
          copy.delete(selectedProjectId);
          return copy;
        }
        return prev;
      });
    }
  }, [activeTerminalId, selectedProjectId, terminals, browserTabsByScope]);

  const spawnTerminal = useCallback(async (projectId: number, kind: TerminalKind): Promise<number> => {
    if (!config) return 0;
    const project = config.projects.find((p) => p.id === projectId);
    const cwd = project?.path ?? process.cwd();
    try {
      const id = await pty.createTerminal({
        shell: config.defaultShell,
        cwd,
        cols: 80,
        rows: 24,
        projectId,
        kind,
      });
      setActiveTerminalId(id);
      return id;
    } catch (err) {
      console.error('Terminal spawn failed:', err);
      alert('Terminal spawn failed: ' + (err instanceof Error ? err.message : String(err)));
      return 0;
    }
  }, [config, pty]);

  const killTerminal = useCallback((id: number) => {
    const t = terminals.find((x) => x.id === id);
    if (t) {
      api.invoke('browser:destroyInstance', { type: BrowserScopeKeyType.Terminal, projectId: t.projectId, terminalId: t.id });
      // Clear visible scope override if it points to this terminal
      setBrowserPanelVisibleScopeByProject((prev) => {
        const override = prev.get(t.projectId);
        if (override && override.type === BrowserScopeKeyType.Terminal && override.terminalId === id) {
          const copy = new Map(prev);
          copy.delete(t.projectId);
          return copy;
        }
        return prev;
      });
    }
    pty.killTerminal(id);
    if (activeTerminalId === id) {
      setActiveTerminalId(null);
    }
  }, [pty, activeTerminalId, terminals]);

  const openFile = useCallback((filePath: string) => {
    setFileEditorPath(filePath);
    setFileEditorName(filePath.split(/[\\/]/).pop() || filePath);
    setFileEditorOpen(true);
    setActiveAcpChat(null);
  }, []);

  const closeFileEditor = useCallback(() => {
    setFileEditorOpen(false);
  }, []);

  const handleAddProject = useCallback(async () => {
    const result = await api.invoke('dialog:showOpen', { properties: ['openDirectory'] }) as string[] | undefined;
    if (!result || result.length === 0) return;
    const selectedPath = result[0];
    const folderName = selectedPath.split(/[\\/]/).pop() || 'project';
    const newProject: ProjectRecord = {
      id: Date.now(),
      name: folderName,
      path: selectedPath,
      savedMessages: [],
      aiConfig: {},
      checklist: [],
      foregroundSavedMessages: [],
      isWorktree: false,
    };
    setConfig((prev) => {
      if (!prev) return prev;
      return { ...prev, projects: [...prev.projects, newProject] };
    });
    setSelectedProjectId(newProject.id);
  }, []);

  const openAcpChat = useCallback(async (projectId: number) => {
    if (!config) return;
    const project = config.projects.find((p) => p.id === projectId);
    if (!project) return;
    const chatId = await api.invoke('acp:spawn', { projectId, cwd: project.path, mcpServers: [] }) as string;
    setActiveAcpChatByProject((prev) => {
      const next = new Map(prev);
      next.set(projectId, chatId);
      return next;
    });
    setActiveAcpChat({ chatId, projectId });
    setFileEditorOpen(false);
  }, [config]);

  const closeAcpChat = useCallback(() => {
    setActiveAcpChat(null);
  }, []);

  const removeAcpChatForProject = useCallback((projectId: number) => {
    setActiveAcpChatByProject((prev) => {
      const next = new Map(prev);
      next.delete(projectId);
      return next;
    });
    if (activeAcpChat && activeAcpChat.projectId === projectId) {
      setActiveAcpChat(null);
    }
  }, [activeAcpChat]);

  const toggleBrowser = useCallback(() => {
    if (!selectedProjectId) return;
    setBrowserOpenProjects((prev) => {
      const next = new Set(prev);
      if (next.has(selectedProjectId)) {
        next.delete(selectedProjectId);
      } else {
        next.add(selectedProjectId);
      }
      return next;
    });
  }, [selectedProjectId]);

  const acpProject = activeAcpChat ? config?.projects.find((p) => p.id === activeAcpChat.projectId) ?? null : null;

  const handleUpdateSmartInputState = useCallback((terminalId: number, state: Partial<SmartInputState>) => {
    pty.updateSmartInputState(terminalId, state);
  }, [pty]);

  const handleSendToTerminal = useCallback((terminalId: number, text: string, attachments: SmartInputAttachment[]) => {
    pty.sendSmartInputToTerminal(terminalId, text, attachments);
  }, [pty]);

  const handleUpdateQuestionState = useCallback((terminalId: number, updates: { focusIndex?: number; selectedOptions?: string[]; customText?: string }) => {
    pty.updateQuestionState(terminalId, updates);
  }, [pty]);

  const handleTerminalOutputClick = useCallback((terminalId: number) => {
    pty.setTerminalOutputFocusOverride(terminalId, true);
  }, [pty]);

  const handleClearTerminalOutputFocusOverride = useCallback((terminalId: number) => {
    pty.setTerminalOutputFocusOverride(terminalId, false);
  }, [pty]);

  const handleScrollDetached = useCallback((terminalId: number, detached: boolean) => {
    const t = terminals.find((x) => x.id === terminalId);
    if (!t) return;
    t.opencodeManualScrollDetached = detached;
    // Notify subscribers to re-render
    pty.updateSmartInputState(terminalId, { ...t.smartInputState });
  }, [pty, terminals]);

  // Clear ACP chat state for projects that no longer exist in config
  useEffect(() => {
    if (!config) return;
    setActiveAcpChatByProject((prev) => {
      const existingIds = new Set(config.projects.map((p) => p.id));
      let changed = false;
      const next = new Map(prev);
      for (const [projectId] of prev) {
        if (!existingIds.has(projectId)) {
          next.delete(projectId);
          changed = true;
        }
      }
      if (changed) {
        // Also clear activeAcpChat if its project was removed
        if (activeAcpChat && !existingIds.has(activeAcpChat.projectId)) {
          setActiveAcpChat(null);
        }
        return next;
      }
      return prev;
    });
  }, [config, activeAcpChat]);

  // Ensure Smart Input focus when visible and no override
  useEffect(() => {
    const activeTerminal = activeTerminals.find((t) => t.id === activeTerminalId);
    if (!activeTerminal) return;
    const showSmartInput = activeTerminal.kind === 'foreground' && activeTerminal.aiTool === 'opencode' && activeTerminal.opencodeSessionActive;
    if (!showSmartInput) return;
    if (activeTerminal.terminalOutputFocusOverride) return;
    if (activeAcpChat) return;
    // Surrender Smart Input focus when any modal/popup is open
    if (settingsOpen || checklistVisible) return;
    // Auto-focus Smart Input draft
    const smartInput = document.querySelector(`[data-smart-input="${activeTerminalId}"]`) as HTMLElement | null;
    if (smartInput && document.activeElement !== smartInput) {
      smartInput.focus();
    }
  }, [activeTerminals, activeTerminalId, activeAcpChat, settingsOpen, checklistVisible]);

  return (
    <div className="app-container">
      <div className="activity-rail">
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.Directory ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.Directory)}
          title="Project Explorer"
        >
          Explorer
        </button>
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.TerminalManager ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.TerminalManager)}
          title="Terminal Manager"
        >
          Terminals
        </button>
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.SourceControl ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.SourceControl)}
          title="Source Control"
        >
          Git
        </button>
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.InputHistory ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.InputHistory)}
          title="Input History"
        >
          History
        </button>
        <div style={{ flex: 1 }} />
        <button
          className={`rail-btn ${isBrowserOpen ? 'active' : ''}`}
          onClick={toggleBrowser}
          title="Browser"
        >
          Web
        </button>
        <button
          className={`rail-btn ${checklistVisible ? 'active' : ''}`}
          onClick={() => setChecklistVisible((v) => !v)}
          title="Checklist"
        >
          Check
        </button>
        <button
          className={`rail-btn ${settingsOpen ? 'active' : ''}`}
          onClick={() => setSettingsOpen(true)}
          title="Settings"
        >
          ⚙
        </button>
      </div>

      <div
        ref={sidebarRef}
        className="sidebar"
        style={{ width: sidebarWidth, minWidth: 200, maxWidth: 500, display: 'flex', flexDirection: 'column', overflow: 'hidden', borderRight: '1px solid #222' }}
      >
        {activeTab === LeftSidebarTabEnum.Directory && selectedProject && (
          <ProjectExplorer
            project={selectedProject}
            selectedPath={selectedProject.path}
            onOpenFile={openFile}
          />
        )}
        {activeTab === LeftSidebarTabEnum.Directory && !selectedProject && (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', padding: 24, gap: 16 }}>
            <span style={{ fontSize: 14, color: '#888' }}>No projects yet.</span>
            <button
              onClick={handleAddProject}
              style={{
                padding: '8px 16px',
                background: '#0078d4',
                border: 'none',
                borderRadius: 4,
                color: '#fff',
                fontSize: 13,
                cursor: 'pointer',
              }}
            >
              Add Project
            </button>
          </div>
        )}
        {activeTab === LeftSidebarTabEnum.TerminalManager && config && (
          <TerminalManager
            config={config}
            terminals={terminals}
            activeTerminalId={activeTerminalId}
            onActivateTerminal={activateTerminal}
            onSpawnTerminal={spawnTerminal}
            onKillTerminal={killTerminal}
            rerunBackground={pty.rerunBackground}
            sendSavedMessageToTerminal={pty.sendSavedMessageToTerminal}
            onRemoveForegroundMessage={(projectId, message) => {
              if (!config) return;
              const newProjects = config.projects.map((p) => {
                if (p.id === projectId) {
                  return { ...p, foregroundSavedMessages: p.foregroundSavedMessages.filter((m) => m !== message) };
                }
                return p;
              });
              setConfig({ ...config, projects: newProjects });
            }}
            onAddForegroundMessage={(projectId, message) => {
              if (!config) return;
              const newProjects = config.projects.map((p) => {
                if (p.id === projectId) {
                  return { ...p, foregroundSavedMessages: [...p.foregroundSavedMessages, message] };
                }
                return p;
              });
              setConfig({ ...config, projects: newProjects });
            }}
            onUpdateForegroundMessage={(projectId, index, message) => {
              if (!config) return;
              const newProjects = config.projects.map((p) => {
                if (p.id === projectId) {
                  const newMessages = [...p.foregroundSavedMessages];
                  newMessages[index] = message;
                  return { ...p, foregroundSavedMessages: newMessages };
                }
                return p;
              });
              setConfig({ ...config, projects: newProjects });
            }}
            activeAcpChatByProject={activeAcpChatByProject}
            onActivateAcpChat={restoreActiveAcpForProject}
            onRemoveAcpChat={removeAcpChatForProject}
            onOpenAcpChat={openAcpChat}
          />
        )}
        {activeTab === LeftSidebarTabEnum.SourceControl && selectedProject && config && (
          <SourceControl
            project={selectedProject}
            registeredWorktreePaths={config.projects
              .filter((p) => p.isWorktree && p.repoRoot === selectedProject.path)
              .map((p) => p.path)}
            onOrphanWorktrees={(orphanPaths) => {
              if (!config) return;
              // Remove orphan worktree projects from config
              const removedProjects = config.projects.filter((p) => orphanPaths.includes(p.path));
              const newProjects = config.projects.filter((p) => !orphanPaths.includes(p.path));
              if (newProjects.length !== config.projects.length) {
                setConfig({ ...config, projects: newProjects });
              }
              // Clear browser visible scope override for removed projects
              if (removedProjects.length > 0) {
                setBrowserPanelVisibleScopeByProject((prev) => {
                  const copy = new Map(prev);
                  for (const rp of removedProjects) {
                    copy.delete(rp.id);
                  }
                  return copy;
                });
              }
              // Kill any terminals running in orphan worktrees
              for (const path of orphanPaths) {
                const normalizedPath = path.replace(/\\/g, '/');
                const orphanTerminals = terminals.filter((t) => {
                  const normalizedCwd = t.cwd.replace(/\\/g, '/');
                  return normalizedCwd === normalizedPath || normalizedCwd.startsWith(normalizedPath + '/');
                });
                for (const t of orphanTerminals) {
                  killTerminal(t.id);
                }
              }
            }}
            onAddWorktree={(worktree) => {
              if (!config) return;
              const newProject: ProjectRecord = {
                id: Date.now(),
                name: worktree.branch || 'worktree',
                path: worktree.path,
                savedMessages: config.projects.find((p) => p.path === selectedProject?.repoRoot || p.path === selectedProject?.path)?.savedMessages || [],
                aiConfig: {},
                checklist: [],
                foregroundSavedMessages: [],
                isWorktree: true,
                repoRoot: selectedProject?.repoRoot || selectedProject?.path,
              };
              const newConfig = { ...config, projects: [...config.projects, newProject] };
              setConfig(newConfig);
              setSelectedProjectId(newProject.id);
            }}
            onDeleteGitWorktree={async (worktree) => {
              if (!config) return;
              await api.invoke('git:removeWorktree', selectedProject.path, worktree.path);
              // Remove worktree project from config
              const removedProject = config.projects.find((p) => p.path === worktree.path);
              const newProjects = config.projects.filter((p) => p.path !== worktree.path);
              setConfig({ ...config, projects: newProjects });
              // Clear browser visible scope override for removed project
              if (removedProject) {
                setBrowserPanelVisibleScopeByProject((prev) => {
                  const copy = new Map(prev);
                  copy.delete(removedProject.id);
                  return copy;
                });
              }
            }}
            hasLiveTerminals={(path) => {
              const normalizedPath = path.replace(/\\/g, '/');
              return terminals.some((t) => {
                const normalizedCwd = t.cwd.replace(/\\/g, '/');
                return normalizedCwd === normalizedPath || normalizedCwd.startsWith(normalizedPath + '/');
              });
            }}
            onBranchChange={(branch) => {
              setBranchNameByProject((prev) => {
                const next = new Map(prev);
                next.set(selectedProject.id, branch);
                return next;
              });
            }}
          />
        )}
        {activeTab === LeftSidebarTabEnum.InputHistory && config && (
          <InputHistory
            config={config}
            terminals={terminals}
            activeTerminalId={activeTerminalId}
            onActivateTerminal={activateTerminal}
          />
        )}
      </div>

      <div
        className="resize-handle"
        onMouseDown={() => setIsResizing(true)}
        style={{
          width: 4,
          cursor: 'col-resize',
          background: isResizing ? '#0078d4' : 'transparent',
        }}
      />

      <div className="main-area" ref={mainRef}>
        {activeAcpChat && acpProject ? (
          <AcpChatPanel
            project={acpProject}
            chatId={activeAcpChat.chatId}
            config={config || defaultAppConfig()}
            onClose={closeAcpChat}
            disabled={settingsOpen || checklistVisible}
            branchName={branchNameByProject.get(activeAcpChat.projectId)}
          />
        ) : fileEditorOpen && fileEditorPath && fileEditorName ? (
          <FileEditor
            filePath={fileEditorPath}
            displayName={fileEditorName}
            onClose={closeFileEditor}
            onDirtyChange={setFileEditorDirty}
          />
        ) : (
          <MainArea
            terminals={mainAreaTerminals}
            activeTerminalId={activeTerminalId}
            onTerminalClick={activateTerminal}
            width={mainSize.width}
            height={mainSize.height}
            onUpdateSmartInputState={handleUpdateSmartInputState}
            onSendToTerminal={handleSendToTerminal}
            onUpdateQuestionState={handleUpdateQuestionState}
            onTerminalOutputClick={handleTerminalOutputClick}
            onClearTerminalOutputFocusOverride={handleClearTerminalOutputFocusOverride}
            onScrollDetached={handleScrollDetached}
            wheelEnabled={!settingsOpen && !checklistVisible}
            disabled={settingsOpen || checklistVisible}
          />
        )}
      </div>

      {isBrowserOpen && selectedProject && (
        <>
          <div
            className="resize-handle"
            onMouseDown={() => setIsResizingBrowser(true)}
            style={{
              width: 4,
              cursor: 'col-resize',
              background: isResizingBrowser ? '#0078d4' : 'transparent',
            }}
          />
          <div style={{ width: browserPanelWidth, minWidth: 240, maxWidth: 800, display: 'flex', flexDirection: 'column', overflow: 'hidden', borderLeft: '1px solid #222' }}>
            <BrowserPanel
              project={selectedProject}
              activeTerminalId={activeTerminalId}
              visibleScopeOverride={browserPanelVisibleScopeByProject.get(selectedProjectId) ?? undefined}
              onClose={toggleBrowser}
              hidden={settingsOpen || checklistVisible}
              tabsByScope={browserTabsByScope}
              activeTabByScope={browserActiveTabByScope}
              urlDraftByScope={browserUrlDraftByScope}
              designInspectByScope={browserDesignInspectByScope}
              onTabsChange={setBrowserTabsByScope}
              onActiveTabChange={setBrowserActiveTabByScope}
              onUrlDraftChange={setBrowserUrlDraftByScope}
              onDesignInspectChange={setBrowserDesignInspectByScope}
              onScopeEmpty={(scope) => {
                if (scope.type === BrowserScopeKeyType.Terminal) {
                  setBrowserPanelVisibleScopeByProject((prev) => {
                    const override = prev.get(selectedProjectId);
                    if (override && override.type === BrowserScopeKeyType.Terminal && override.terminalId === scope.terminalId) {
                      const copy = new Map(prev);
                      copy.delete(selectedProjectId);
                      return copy;
                    }
                    return prev;
                  });
                }
              }}
            />
          </div>
        </>
      )}

      {checklistVisible && config && (
        <Checklist
          projects={config.projects}
          onRemoveItem={(projectId, index) => {
            const newProjects = config.projects.map((p) => {
              if (p.id === projectId) {
                const newChecklist = [...p.checklist];
                newChecklist.splice(index, 1);
                return { ...p, checklist: newChecklist };
              }
              return p;
            });
            setConfig({ ...config, projects: newProjects });
          }}
          onClose={() => setChecklistVisible(false)}
        />
      )}

      {settingsOpen && config && (
        <SettingsPopup
          config={config}
          onSave={(newConfig) => setConfig(newConfig)}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}

export default App;
