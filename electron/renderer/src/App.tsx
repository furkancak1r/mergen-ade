import React, { useEffect, useRef, useState, useCallback } from 'react';
import type { AcpChatSession, AppConfig, ShellKind, TerminalKind, ProjectRecord, LeftSidebarTab, BrowserScopeKey, SmartInputState, SmartInputAttachment, AiHookEvent, AiCliAttentionKind, TerminalShortcutEntry, BrowserTab, InputHistoryFilter, TerminalManagerFilter, AppHistory } from '../../shared/types';
import type { ClaudeCodexPlanResult, ClaudeCodexReviewResult } from '../../shared/claudeCodexHook';
import { resolveAcpRoute } from '../../shared/acpRoute';
import { BrowserScopeKeyType, AiCliTool as AiCliToolEnum } from '../../shared/types';
import { LeftSidebarTab as LeftSidebarTabEnum, defaultAppConfig, defaultAppHistory } from '../../shared/types';
import { MainArea } from './components/MainArea';
import { ProjectExplorer } from './components/ProjectExplorer';
import { TerminalManager } from './components/TerminalManager';
import { FileEditor } from './components/FileEditor';
import { AcpChatPanel } from './components/AcpChatPanel';
import { AcpErrorBoundary } from './components/AcpErrorBoundary';
import { RightPanel } from './components/RightPanel';
import type { RightPanelTab } from './components/RightPanel';
import { SettingsPopup } from './components/SettingsPopup';
import { InputHistory } from './components/InputHistory';
import { GlobalTooltip } from './components/Tooltip';
import { usePty } from './hooks/usePty';
import {
  activeBrowserScope as resolveActiveBrowserScope,
  browserUrlForProjectFamily,
  scopeKeyString,
  withBrowserLastUrlForProjectFamily,
  withoutBrowserLastUrlForProjectFamily,
} from './lib/browserScope';
import {
  OPENCODE_ACP_LABEL,
  nextAcpActivityState,
  nextAcpTerminalManagerAttention,
  type AcpEventLike,
  type AcpTerminalManagerAttentionReason,
} from './lib/acpUi';
import type { SmartInputModeId } from './lib/smartInputMode';
import { shouldShowSmartInputFooter } from './lib/smartInput';
import { terminalWheelEnabled } from './lib/terminalWheel';
import {
  normalizeTerminalManagerStartupState,
  withTerminalManagerFilter,
  withTerminalManagerOpened,
  withToggledTerminalManagerHideInactive,
} from './lib/terminalManagerState';
import { recordInputHistory, removeProjectsInputHistory } from './lib/inputHistory';
import { activityRailItem, isLeftSidebarTabActive, withLeftSidebarRailToggle, withLeftSidebarTabOpen } from './lib/activityRail';
import { browserProjectIdsAfterScopeEmpty } from './lib/browserToolbar';
import { panelWidthFromPointerDrag } from './lib/sidebarResize';
import {
  fileEditorLocationFromPath,
  initialFileEditorNavigationState,
  withFileEditorClosed,
  withFileEditorHidden,
  withFileEditorNavigateBack,
  withFileEditorNavigateForward,
  withFileEditorOpened,
} from './lib/fileEditor';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [history, setHistory] = useState<AppHistory>(defaultAppHistory());
  const [activeTab, setActiveTab] = useState<LeftSidebarTab>(LeftSidebarTabEnum.Directory);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [activeTerminalId, setActiveTerminalId] = useState<number | null>(null);
  const [terminals, setTerminals] = useState<ReturnType<ReturnType<typeof usePty>['getTerminals']>>([]);
  const [mainSize, setMainSize] = useState({ width: 800, height: 600 });
  const mainRef = useRef<HTMLDivElement>(null);
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [isResizing, setIsResizing] = useState(false);
  const sidebarRef = useRef<HTMLDivElement>(null);
  const sidebarResizeStartRef = useRef<{ pointerX: number; width: number } | null>(null);

  const [fileEditorState, setFileEditorState] = useState(initialFileEditorNavigationState);
  const [fileEditorDirty, setFileEditorDirty] = useState(false);
  const fileEditorOpen = fileEditorState.open;
  const fileEditorPath = fileEditorState.active?.path ?? null;
  const fileEditorName = fileEditorState.active?.displayName ?? null;

  const [activeAcpChat, setActiveAcpChat] = useState<{ chatId: string; projectId: number; tool: string } | null>(null);
  const [activeAcpChatByProject, setActiveAcpChatByProject] = useState<Map<string, string>>(new Map());
  const [activeAcpSessionByProject, setActiveAcpSessionByProject] = useState<Map<string, AcpChatSession>>(new Map());
  const [activeAcpAttentionByProject, setActiveAcpAttentionByProject] = useState<Map<string, AcpTerminalManagerAttentionReason>>(new Map());
  const [acpRunning, setAcpRunning] = useState(false);
  const [acpQueuedPrompts, setAcpQueuedPrompts] = useState(false);
  const [acpDraftByChatId, setAcpDraftByChatId] = useState<Map<string, string>>(new Map());

  const [rightPanelOpen, setRightPanelOpen] = useState(false);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>('sourceControl');
  const [browserOpenProjects, setBrowserOpenProjects] = useState<Set<number>>(new Set());
  const [rightPanelWidth, setRightPanelWidth] = useState(520);
  const [isResizingRightPanel, setIsResizingRightPanel] = useState(false);
  const rightPanelResizeStartRef = useRef<{ pointerX: number; width: number } | null>(null);

  const [settingsOpen, setSettingsOpen] = useState(false);
  const [terminalManagerOverlayOpen, setTerminalManagerOverlayOpen] = useState(false);
  const [branchNameByProject, setBranchNameByProject] = useState<Map<number, string>>(new Map());


  const [activeBrowserScope, setActiveBrowserScope] = useState<BrowserScopeKey | null>(null);
  const [browserPanelVisibleScopeByProject, setBrowserPanelVisibleScopeByProject] = useState<Map<number, BrowserScopeKey>>(new Map());

  // Browser state keyed by scope (must persist across project switches)
  const [browserTabsByScope, setBrowserTabsByScope] = useState<Map<string, BrowserTab[]>>(new Map());
  const [browserActiveTabByScope, setBrowserActiveTabByScope] = useState<Map<string, string | null>>(new Map());
  const [browserUrlDraftByScope, setBrowserUrlDraftByScope] = useState<Map<string, string>>(new Map());
  const [browserDesignInspectByScope, setBrowserDesignInspectByScope] = useState<Map<string, boolean>>(new Map());

  const pty = usePty({ allowClaudeCodexPlan: Boolean(config?.claudeCodeCodexHookEnabled) });
  const suppressAcpRestoreRef = useRef(false);
  const historyLoadedRef = useRef(false);
  const terminalHistorySignatureRef = useRef<Map<number, string>>(new Map());

  useEffect(() => {
    api.invoke('config:load').then((cfg) => {
      const loaded = normalizeTerminalManagerStartupState(cfg as AppConfig);
      setConfig(loaded);
      setActiveTab(loaded.ui.leftSidebarTab);
      setSidebarWidth(loaded.ui.projectExplorerWidth);
      setRightPanelWidth(loaded.ui.browserPanelWidth);
      if (loaded.projects.length > 0) {
        const lastId = loaded.ui.lastSelectedProjectId;
        const match = lastId ? loaded.projects.find((p) => p.id === lastId) : undefined;
        setSelectedProjectId(match ? match.id : loaded.projects[0].id);
      }
    });
  }, []);

  useEffect(() => {
    api.invoke('history:load').then((loadedHistory) => {
      setHistory(loadedHistory as AppHistory);
      historyLoadedRef.current = true;
    });
  }, []);

  useEffect(() => {
    if (!historyLoadedRef.current) return;
    const timer = setTimeout(() => {
      api.invoke('history:save', history);
    }, 500);
    return () => clearTimeout(timer);
  }, [history]);

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
    if (!config || config.ui.browserPanelWidth === rightPanelWidth) return;
    setConfig((prev) => prev ? { ...prev, ui: { ...prev.ui, browserPanelWidth: rightPanelWidth } } : prev);
  }, [rightPanelWidth]);

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
    if (!config || !historyLoadedRef.current) return;
    const liveTerminalIds = new Set(terminals.map((terminal) => terminal.id));
    for (const terminalId of terminalHistorySignatureRef.current.keys()) {
      if (!liveTerminalIds.has(terminalId)) {
        terminalHistorySignatureRef.current.delete(terminalId);
      }
    }

    setHistory((prev) => {
      let next = prev;
      for (const terminal of terminals) {
        const signature = terminal.recentInputs.join('\0');
        if (terminalHistorySignatureRef.current.get(terminal.id) === signature) continue;
        terminalHistorySignatureRef.current.set(terminal.id, signature);

        const latestInput = terminal.recentInputs[0];
        if (!latestInput) continue;
        const project = config.projects.find((candidate) => candidate.id === terminal.projectId);
        next = recordInputHistory(
          next,
          project,
          terminal.kind,
          latestInput,
          Math.floor(Date.now() / 1000),
        );
      }
      return next;
    });
  }, [config, terminals]);

  useEffect(() => {
    const unsub = (window as unknown as { mergenApi: { on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi.on('window:closeRequest', () => {
      const parts: string[] = [];
      if (fileEditorDirty) parts.push('You have unsaved changes in the file editor.');
      if (terminals.length > 0) parts.push(`${terminals.length} terminal session(s) are still running.`);
      if (acpRunning) parts.push(`An ${OPENCODE_ACP_LABEL} session is still running.`);
      if (acpQueuedPrompts) parts.push(`You have queued prompts in ${OPENCODE_ACP_LABEL}.`);
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
    const unsub = api.on('acp:event', (eventChatId: string, event: AcpEventLike) => {
      const projectEntry = Array.from(activeAcpChatByProject.entries()).find(([, chatId]) => chatId === eventChatId);
      if (projectEntry) {
        const [projectKey] = projectEntry;
        setActiveAcpAttentionByProject((prev) => {
          const currentReason = prev.get(projectKey);
          const nextReason = nextAcpTerminalManagerAttention(currentReason, event);
          if (nextReason === currentReason) return prev;
          const next = new Map(prev);
          if (nextReason) {
            next.set(projectKey, nextReason);
          } else {
            next.delete(projectKey);
          }
          return next;
        });
        api.invoke('acp:getSession', eventChatId).then((session) => {
          setActiveAcpSessionByProject((prev) => {
            const next = new Map(prev);
            if (session) {
              next.set(projectKey, session as AcpChatSession);
            } else {
              next.delete(projectKey);
            }
            return next;
          });
        });
      }
      if (activeAcpChat && eventChatId === activeAcpChat.chatId) {
        const next = nextAcpActivityState({ running: acpRunning, hasQueuedPrompts: acpQueuedPrompts }, event);
        setAcpRunning(next.running);
        setAcpQueuedPrompts(next.hasQueuedPrompts);
      }
    });
    return () => { unsub(); };
  }, [activeAcpChat, activeAcpChatByProject, acpRunning, acpQueuedPrompts]);

  // Browser hide on modal open (Settings) with grace period after close.
  useEffect(() => {
    if (settingsOpen) {
      api.invoke('browser:hideAll');
      return;
    }
    const timer = setTimeout(() => {
      if (activeBrowserScope) {
        api.invoke('browser:showActive', activeBrowserScope);
      }
    }, 150);
    return () => clearTimeout(timer);
  }, [settingsOpen, activeBrowserScope]);

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
      setConfig((prev) => {
        if (!prev) return prev;
        const projects = withBrowserLastUrlForProjectFamily(prev.projects, scope.projectId, url);
        return projects === prev.projects ? prev : { ...prev, projects };
      });
    });
    return () => { unsub(); };
  }, [config]);

  // Browser MCP can open tabs from the main process; mirror those into renderer state.
  useEffect(() => {
    const unsub = api.on('browser:tabOpened', (scope: BrowserScopeKey, tab: BrowserTab) => {
      if (config && !config.projects.some((project) => project.id === scope.projectId)) return;
      const key = scopeKeyString(scope);
      setBrowserTabsByScope((prev) => {
        const current = prev.get(key) ?? [];
        const nextTabs = current.some((existing) => existing.id === tab.id)
          ? current.map((existing) => existing.id === tab.id ? { ...existing, ...tab } : existing)
          : [...current, tab];
        const copy = new Map(prev);
        copy.set(key, nextTabs);
        return copy;
      });
      setBrowserActiveTabByScope((prev) => {
        const copy = new Map(prev);
        copy.set(key, tab.id);
        return copy;
      });
      setBrowserUrlDraftByScope((prev) => {
        const copy = new Map(prev);
        copy.set(key, tab.url);
        return copy;
      });
      setBrowserOpenProjects((prev) => {
        const next = new Set(prev);
        next.add(scope.projectId);
        return next;
      });
      setBrowserPanelVisibleScopeByProject((prev) => {
        const copy = new Map(prev);
        copy.set(scope.projectId, scope);
        return copy;
      });
    });
    return () => { unsub(); };
  }, [config]);

  useEffect(() => {
    const unsub = api.on('browser:tabsChanged', (
      scope: BrowserScopeKey,
      state: { tabs: BrowserTab[]; activeTabId?: string; urlDraft?: string },
    ) => {
      if (config && !config.projects.some((project) => project.id === scope.projectId)) return;
      const key = scopeKeyString(scope);
      const tabs = state.tabs ?? [];
      const activeTabId = state.activeTabId ?? tabs[0]?.id ?? null;
      const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];

      setBrowserTabsByScope((prev) => {
        const copy = new Map(prev);
        if (tabs.length > 0) {
          copy.set(key, tabs);
        } else {
          copy.delete(key);
        }
        const projectHasTabs = Array.from(copy.entries()).some(([scopeKey, scopeTabs]) => (
          scopeTabs.length > 0
          && (scopeKey === `project:${scope.projectId}` || scopeKey.startsWith(`terminal:${scope.projectId}:`))
        ));
        setBrowserOpenProjects((openPrev) => {
          const next = new Set(openPrev);
          if (projectHasTabs) {
            next.add(scope.projectId);
          } else {
            next.delete(scope.projectId);
          }
          return next;
        });
        return copy;
      });
      setBrowserActiveTabByScope((prev) => {
        const copy = new Map(prev);
        if (activeTabId) {
          copy.set(key, activeTabId);
        } else {
          copy.delete(key);
        }
        return copy;
      });
      setBrowserUrlDraftByScope((prev) => {
        const copy = new Map(prev);
        const urlDraft = state.urlDraft || activeTab?.url || '';
        if (urlDraft) {
          copy.set(key, urlDraft);
        } else {
          copy.delete(key);
        }
        return copy;
      });
      if (tabs.length > 0) {
        setBrowserPanelVisibleScopeByProject((prev) => {
          const copy = new Map(prev);
          copy.set(scope.projectId, scope);
          return copy;
        });
      } else if (scope.type === BrowserScopeKeyType.Terminal) {
        setBrowserPanelVisibleScopeByProject((prev) => {
          const override = prev.get(scope.projectId);
          if (override && override.type === BrowserScopeKeyType.Terminal && override.terminalId === scope.terminalId) {
            const copy = new Map(prev);
            copy.delete(scope.projectId);
            return copy;
          }
          return prev;
        });
      }
    });
    return () => { unsub(); };
  }, [config]);

  const clearProjectBrowserLastUrl = useCallback((projectId: number) => {
    setConfig((prev) => {
      if (!prev) return prev;
      const projects = withoutBrowserLastUrlForProjectFamily(prev.projects, projectId);
      return projects === prev.projects ? prev : { ...prev, projects };
    });
  }, []);

  const activateTerminal = useCallback((id: number) => {
    setActiveTerminalId(id);
    setFileEditorState((prev) => withFileEditorHidden(prev));
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
    const prefix = `${projectId}:`;
    const entry = Array.from(activeAcpChatByProject.entries()).find(([key]) => key.startsWith(prefix));
    if (entry) {
      const [projectKey, chatId] = entry;
      setActiveAcpAttentionByProject((prev) => {
        if (!prev.has(projectKey)) return prev;
        const next = new Map(prev);
        next.delete(projectKey);
        return next;
      });
      const tool = projectKey.slice(prefix.length);
      setActiveAcpChat({ chatId, projectId, tool });
      setFileEditorState((prev) => withFileEditorHidden(prev));
      setActiveTab(LeftSidebarTabEnum.TerminalManager);
      setConfig((prev) => prev ? withTerminalManagerOpened(prev) : prev);
    }
  }, [activeAcpChatByProject]);

  // Warm ACP standby for selected project
  useEffect(() => {
    if (!selectedProjectId || !config) return;
    const project = config.projects.find((p) => p.id === selectedProjectId);
    if (!project) return;
    // Warm standby after a short delay so it doesn't fire on every rapid selection change
    const timer = setTimeout(() => {
      api.invoke('acp:standby:warm', selectedProjectId, project.path);
    }, 500);
    return () => clearTimeout(timer);
  }, [selectedProjectId, config]);

  // Restore ACP chat when switching to a project that has an active chat
  useEffect(() => {
    if (!selectedProjectId) return;
    if (suppressAcpRestoreRef.current) {
      suppressAcpRestoreRef.current = false;
      return;
    }
    const prefix = `${selectedProjectId}:`;
    const entry = Array.from(activeAcpChatByProject.entries()).find(([key]) => key.startsWith(prefix));
    if (entry) {
      const [projectKey, chatId] = entry;
      const tool = projectKey.slice(prefix.length);
      setActiveAcpChat({ chatId, projectId: selectedProjectId, tool });
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
    if (settingsOpen) return;

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
  }, [config, activeTerminals, activeTerminalId, pty, activateTerminal, activeAcpChat, terminals, shortcutMatchesEvent, formatShortcutCombo, settingsOpen, fileEditorOpen]);

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
    document.body.classList.add('is-column-resizing');
    function handleMove(e: PointerEvent) {
      e.preventDefault();
      const start = sidebarResizeStartRef.current;
      if (!start) return;
      setSidebarWidth(panelWidthFromPointerDrag({
        pointerX: e.clientX,
        startPointerX: start.pointerX,
        startWidth: start.width,
        minWidth: 200,
        maxWidth: 500,
      }));
    }
    function handleUp(e: PointerEvent) {
      e.preventDefault();
      sidebarResizeStartRef.current = null;
      setIsResizing(false);
    }
    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleUp);
    window.addEventListener('pointercancel', handleUp);
    return () => {
      document.body.classList.remove('is-column-resizing');
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('pointerup', handleUp);
      window.removeEventListener('pointercancel', handleUp);
    };
  }, [isResizing]);

  useEffect(() => {
    if (!isResizingRightPanel) return;
    document.body.classList.add('is-column-resizing');
    function handleMove(e: PointerEvent) {
      e.preventDefault();
      const start = rightPanelResizeStartRef.current;
      if (!start) return;
      setRightPanelWidth(panelWidthFromPointerDrag({
        pointerX: e.clientX,
        startPointerX: start.pointerX,
        startWidth: start.width,
        minWidth: 240,
        maxWidth: 800,
        direction: 'left',
      }));
    }
    function handleUp(e: PointerEvent) {
      e.preventDefault();
      rightPanelResizeStartRef.current = null;
      setIsResizingRightPanel(false);
    }
    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleUp);
    window.addEventListener('pointercancel', handleUp);
    return () => {
      document.body.classList.remove('is-column-resizing');
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('pointerup', handleUp);
      window.removeEventListener('pointercancel', handleUp);
    };
  }, [isResizingRightPanel]);

  const selectedProject = config?.projects.find((p) => p.id === selectedProjectId) ?? null;
  const activeTerminalForSettings = terminals.find((t) => t.id === activeTerminalId);

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
      activateTerminal(id);
      return id;
    } catch (err) {
      console.error('Terminal spawn failed:', err);
      alert('Terminal spawn failed: ' + (err instanceof Error ? err.message : String(err)));
      return 0;
    }
  }, [config, pty, activateTerminal]);

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
    setFileEditorState((prev) => withFileEditorOpened(prev, fileEditorLocationFromPath(filePath)));
    setActiveAcpChat(null);
  }, []);

  const closeFileEditor = useCallback(() => {
    setFileEditorState((prev) => withFileEditorClosed(prev));
    setFileEditorDirty(false);
  }, []);

  const navigateFileEditorBack = useCallback(() => {
    if (fileEditorDirty) return;
    setFileEditorState((prev) => withFileEditorNavigateBack(prev));
    setFileEditorDirty(false);
  }, [fileEditorDirty]);

  const navigateFileEditorForward = useCallback(() => {
    if (fileEditorDirty) return;
    setFileEditorState((prev) => withFileEditorNavigateForward(prev));
    setFileEditorDirty(false);
  }, [fileEditorDirty]);

  const handleAddProject = useCallback(async () => {
    const result = await api.invoke('dialog:showOpen', { properties: ['openDirectory'] }) as string[] | undefined;
    if (!result || result.length === 0) return;
    const selectedPath = result[0];
    const folderName = selectedPath.split(/[\\/]/).pop() || 'project';
    const newProject: ProjectRecord = {
      id: Date.now() + Math.floor(Math.random() * 1000),
      name: folderName,
      path: selectedPath,
      savedMessages: [],
      aiConfig: {},
      checklist: [],
      isWorktree: false,
    };
    setConfig((prev) => {
      if (!prev) return prev;
      return { ...prev, projects: [...prev.projects, newProject] };
    });
    setSelectedProjectId(newProject.id);
  }, []);

  const removeProjectsByPath = useCallback((projectPaths: string[]): ProjectRecord[] => {
    if (!config || projectPaths.length === 0) return [];

    const removePathSet = new Set(projectPaths);
    const removedProjects = config.projects.filter((project) => removePathSet.has(project.path));
    if (removedProjects.length === 0) return [];

    const removedIds = new Set(removedProjects.map((project) => project.id));
    const newProjects = config.projects.filter((project) => !removePathSet.has(project.path));
    setConfig({ ...config, projects: newProjects });
    setHistory((prev) => removeProjectsInputHistory(prev, removedProjects.map((project) => project.path)));

    setBrowserOpenProjects((prev) => {
      const next = new Set(prev);
      for (const projectId of removedIds) next.delete(projectId);
      return next;
    });
    setBrowserPanelVisibleScopeByProject((prev) => {
      const next = new Map(prev);
      for (const projectId of removedIds) next.delete(projectId);
      return next;
    });
    setActiveBrowserScope((prev) => (prev && removedIds.has(prev.projectId) ? null : prev));

    if (selectedProjectId !== null && removedIds.has(selectedProjectId)) {
      setSelectedProjectId(newProjects[0]?.id ?? null);
    }

    return removedProjects;
  }, [config, selectedProjectId]);

  const openAcpChat = useCallback(async (projectId: number, tool?: string) => {
    if (!config) return;
    const project = config.projects.find((p) => p.id === projectId);
    if (!project) return;

    const effectiveTool = tool || 'opencode';
    const projectKey = `${projectId}:${effectiveTool}`;
    let chatId: string;

    // CLI-backed adapters bypass OpenCode's standby pool.
    if (tool === 'claude_acp' || tool === 'codex_acp') {
      chatId = await api.invoke('acp:spawn', { projectId, cwd: project.path, mcpServers: [], tool }) as string;
    } else {
      // OpenCode: try to promote standby first
      const standby = await api.invoke('acp:standby:get', projectId) as { chatId: string; sessionId?: string; status: string } | undefined;
      if (standby && standby.sessionId && (standby.status === 'idle' || standby.status === 'running' || standby.status === 'permission')) {
        const promoted = await api.invoke('acp:standby:promote', projectId, '') as { chatId: string } | undefined;
        if (promoted) {
          chatId = promoted.chatId;
        } else {
          chatId = await api.invoke('acp:spawn', { projectId, cwd: project.path, mcpServers: [] }) as string;
        }
      } else {
        chatId = await api.invoke('acp:spawn', { projectId, cwd: project.path, mcpServers: [] }) as string;
      }
    }

    setActiveAcpChatByProject((prev) => {
      const next = new Map(prev);
      next.set(projectKey, chatId);
      return next;
    });
    setActiveAcpAttentionByProject((prev) => {
      if (!prev.has(projectKey)) return prev;
      const next = new Map(prev);
      next.delete(projectKey);
      return next;
    });
    const session = await api.invoke('acp:getSession', chatId) as AcpChatSession | undefined;
    if (session) {
      setActiveAcpSessionByProject((prev) => {
        const next = new Map(prev);
        next.set(projectKey, session);
        return next;
      });
    }
    setActiveAcpChat({ chatId, projectId, tool: effectiveTool });
    setFileEditorState((prev) => withFileEditorHidden(prev));
    // Reveal Terminal Manager with Foreground filter and expand root project
    setActiveTab(LeftSidebarTabEnum.TerminalManager);
    setConfig((prev) => prev ? withTerminalManagerOpened(prev) : prev);
  }, [config]);

  const closeAcpChat = useCallback(() => {
    setActiveAcpChat(null);
  }, []);

  const removeAcpChatForProject = useCallback((projectId: number) => {
    const prefix = `${projectId}:`;
    // Kill all ACP chats for this project
    for (const [key, chatId] of activeAcpChatByProject.entries()) {
      if (key.startsWith(prefix)) {
        api.invoke('acp:kill', chatId);
      }
    }
    setActiveAcpChatByProject((prev) => {
      const next = new Map(prev);
      for (const key of prev.keys()) {
        if (key.startsWith(prefix)) next.delete(key);
      }
      return next;
    });
    setActiveAcpSessionByProject((prev) => {
      const next = new Map(prev);
      for (const key of prev.keys()) {
        if (key.startsWith(prefix)) next.delete(key);
      }
      return next;
    });
    setActiveAcpAttentionByProject((prev) => {
      const next = new Map(prev);
      for (const key of prev.keys()) {
        if (key.startsWith(prefix)) next.delete(key);
      }
      return next;
    });
    setAcpDraftByChatId((prev) => {
      const next = new Map(prev);
      for (const [key, chatId] of activeAcpChatByProject.entries()) {
        if (key.startsWith(prefix)) next.delete(chatId);
      }
      return next;
    });
    if (activeAcpChat && activeAcpChat.projectId === projectId) {
      setActiveAcpChat(null);
    }
    api.invoke('acp:standby:clear', projectId);
  }, [activeAcpChat, activeAcpChatByProject]);

  const toggleRightPanel = useCallback((tab?: RightPanelTab) => {
    if (!selectedProjectId) return;
    if (rightPanelOpen && !tab) {
      // Close the panel
      setRightPanelOpen(false);
      return;
    }
    // Open the panel with specified tab or default to sourceControl
    const targetTab = tab ?? 'sourceControl';
    setRightPanelTab(targetTab);
    setRightPanelOpen(true);
    // If switching to browser, also register the project in browserOpenProjects
    if (targetTab === 'browser') {
      const hasForegroundTerminal = terminals.some((t) => t.projectId === selectedProjectId && t.kind === 'foreground' && !t.exited);
      const prefix = `${selectedProjectId}:`;
      const hasAcpChat = Array.from(activeAcpChatByProject.keys()).some((k) => k.startsWith(prefix));
      if (hasForegroundTerminal || hasAcpChat) {
        setBrowserOpenProjects((prev) => {
          const next = new Set(prev);
          next.add(selectedProjectId);
          return next;
        });
      }
    }
  }, [selectedProjectId, rightPanelOpen, terminals, activeAcpChatByProject]);

  const toggleBrowser = useCallback(() => {
    if (!selectedProjectId) return;
    const isCurrentlyOpen = browserOpenProjects.has(selectedProjectId);
    if (!isCurrentlyOpen) {
      const hasForegroundTerminal = terminals.some((t) => t.projectId === selectedProjectId && t.kind === 'foreground' && !t.exited);
      const prefix = `${selectedProjectId}:`;
      const hasAcpChat = Array.from(activeAcpChatByProject.keys()).some((k) => k.startsWith(prefix));
      if (!hasForegroundTerminal && !hasAcpChat) return;
      setBrowserOpenProjects((prev) => {
        const next = new Set(prev);
        next.add(selectedProjectId);
        return next;
      });
      setRightPanelTab('browser');
      setRightPanelOpen(true);
    } else {
      // Switch to browser tab if panel is open, otherwise open it
      if (rightPanelOpen && rightPanelTab === 'browser') {
        setRightPanelOpen(false);
      } else {
        setRightPanelTab('browser');
        setRightPanelOpen(true);
      }
    }
  }, [selectedProjectId, browserOpenProjects, terminals, activeAcpChatByProject, rightPanelOpen, rightPanelTab]);

  // Auto-close browser when project loses all foreground terminals and ACP chats
  useEffect(() => {
    setBrowserOpenProjects((prev) => {
      if (prev.size === 0) return prev;
      const next = new Set<number>();
      for (const projectId of prev) {
        const hasForegroundTerminal = terminals.some((t) => t.projectId === projectId && t.kind === 'foreground' && !t.exited);
        const prefix = `${projectId}:`;
        const hasAcpChat = Array.from(activeAcpChatByProject.keys()).some((k) => k.startsWith(prefix));
        if (hasForegroundTerminal || hasAcpChat) {
          next.add(projectId);
        }
      }
      return next.size === prev.size ? prev : next;
    });
  }, [terminals, activeAcpChatByProject]);

  const acpProject = activeAcpChat ? config?.projects.find((p) => p.id === activeAcpChat.projectId) ?? null : null;

  const handleUpdateSmartInputState = useCallback((terminalId: number, state: Partial<SmartInputState>) => {
    pty.updateSmartInputState(terminalId, state);
  }, [pty]);

  const handleSendToTerminal = useCallback((terminalId: number, text: string, attachments: SmartInputAttachment[], modeId: SmartInputModeId) => {
    const terminal = terminals.find((candidate) => candidate.id === terminalId);
    const trimmed = text.trim();
    const project = terminal ? config?.projects.find((candidate) => candidate.id === terminal.projectId) : undefined;
    const route = resolveAcpRoute(trimmed, {
      selectedRoute: modeId,
      allowCodexPlan: config?.claudeCodeCodexHookEnabled,
      attachmentCount: attachments.length,
    });
    const runtimeMode: SmartInputModeId = route.route === 'plan' ? 'plan' : 'build';
    if (route.question && terminal?.aiTool === AiCliToolEnum.Claude) {
      pty.updateClaudeCodexHookProgress(terminalId, {
        phase: 'blocked',
        sessionId: 'auto-route',
        error: route.question,
      });
      return;
    }
    const shouldRunClaudeCodexHook = Boolean(
      config?.claudeCodeCodexHookEnabled
      && terminal?.aiTool === AiCliToolEnum.Claude
      && terminal.kind === 'foreground'
      && route.route === 'codex_plan'
      && trimmed
      && attachments.length === 0
      && project?.path
      && terminal.claudeCodexHookProgress?.phase !== 'planning'
    );

    if (shouldRunClaudeCodexHook && terminal && project) {
      pty.updateClaudeCodexHookProgress(terminalId, {
        phase: 'planning',
        sessionId: 'pending',
      });
      api.invoke('claudeCodex:runPlan', {
        terminalId,
        projectPath: project.path,
        originalPrompt: trimmed,
      }).then((value) => {
        const result = value as ClaudeCodexPlanResult;
        pty.updateClaudeCodexHookProgress(terminalId, {
          phase: 'awaiting_implementation',
          sessionId: result.sessionId,
          planPath: result.planPath,
          error: result.planError,
          originalPrompt: trimmed,
          plan: result.plan,
          planError: result.planError,
          reviewRound: 0,
        });
        pty.sendSmartInputToTerminal(terminalId, result.implementationPrompt, [], 'build');
      }).catch((error) => {
        pty.updateClaudeCodexHookProgress(terminalId, {
          phase: 'blocked',
          sessionId: 'failed',
          error: error instanceof Error ? error.message : String(error),
        });
        pty.sendSmartInputToTerminal(terminalId, text, attachments, runtimeMode);
      });
      return;
    }

    pty.sendSmartInputToTerminal(terminalId, text, attachments, runtimeMode);
  }, [config, pty, terminals]);

  const browserUrlForProject = useCallback((project: ProjectRecord): string | undefined => {
    if (!config) return undefined;
    return browserUrlForProjectFamily(
      project,
      config.projects,
      browserTabsByScope,
      browserActiveTabByScope,
      browserPanelVisibleScopeByProject,
    );
  }, [browserActiveTabByScope, browserPanelVisibleScopeByProject, browserTabsByScope, config]);

  const queueClaudeCodexUiVerification = useCallback((project: ProjectRecord, planPath: string, uiChangedFiles: string[]) => {
    if (uiChangedFiles.length === 0) {
      return;
    }
    const url = browserUrlForProject(project);
    if (!url) {
      api.invoke('claudeCodex:updateUiVerification', {
        planPath,
        note: 'UI verification tool available, but no browser URL is known for this project.',
      });
      return;
    }

    const scope = { type: BrowserScopeKeyType.Project, projectId: project.id } as BrowserScopeKey;
    const scopeKey = scopeKeyString(scope);
    const tabId = `hook-ui-${Date.now()}`;
    setSelectedProjectId(project.id);
    setBrowserOpenProjects((prev) => new Set(prev).add(project.id));
    setBrowserPanelVisibleScopeByProject((prev) => {
      const copy = new Map(prev);
      copy.set(project.id, scope);
      return copy;
    });
    setBrowserTabsByScope((prev) => {
      const copy = new Map(prev);
      const current = copy.get(scopeKey) ?? [];
      copy.set(scopeKey, current.length === 0 ? [{ id: tabId, url }] : current);
      return copy;
    });
    setBrowserActiveTabByScope((prev) => {
      const copy = new Map(prev);
      const current = browserTabsByScope.get(scopeKey) ?? [];
      copy.set(scopeKey, current[0]?.id ?? tabId);
      return copy;
    });
    setBrowserUrlDraftByScope((prev) => {
      const copy = new Map(prev);
      copy.set(scopeKey, url);
      return copy;
    });
    api.invoke('browser:navigate', { scope, url });

    window.setTimeout(async () => {
      try {
        const dataUrl = await api.invoke('browser:screenshot', { scope, fullPage: false }) as string;
        if (dataUrl) {
          const link = document.createElement('a');
          link.href = dataUrl;
          link.download = `claude-codex-ui-${Date.now()}.png`;
          link.click();
        }
        await api.invoke('claudeCodex:updateUiVerification', {
          planPath,
          note: 'UI verification queued: Browser panel opened and a desktop visible-area screenshot was requested. Mobile viewport screenshot is not supported by the current embedded Browser integration.',
        });
      } catch (error) {
        await api.invoke('claudeCodex:updateUiVerification', {
          planPath,
          note: `UI verification attempted, but browser screenshot failed: ${error instanceof Error ? error.message : String(error)}`,
        });
      }
    }, 900);
  }, [browserTabsByScope, browserUrlForProject]);

  useEffect(() => {
    if (!config?.claudeCodeCodexHookEnabled) return;
    for (const terminal of terminals) {
      const progress = terminal.claudeCodexHookProgress;
      if (!progress) continue;
      if (progress.reviewInFlight) continue;
      if (progress.phase !== 'awaiting_implementation' && progress.phase !== 'awaiting_fix') continue;
      if (terminal.aiTool !== AiCliToolEnum.Claude || terminal.aiStatus !== 'attention' || terminal.aiAttentionKind !== 'turn_complete') continue;
      const submitSince = terminal.opencodePromptSubmitSince ?? 0;
      if (Date.now() - submitSince < 300) continue;
      const project = config.projects.find((candidate) => candidate.id === terminal.projectId);
      if (!project || !progress.planPath || !progress.originalPrompt) continue;

      pty.updateClaudeCodexHookProgress(terminal.id, {
        ...progress,
        phase: 'testing',
        reviewInFlight: true,
      });
      api.invoke('claudeCodex:runReview', {
        terminalId: terminal.id,
        projectPath: project.path,
        sessionId: progress.sessionId,
        planPath: progress.planPath,
        originalPrompt: progress.originalPrompt,
        plan: progress.plan,
        planError: progress.planError,
        reviewRound: progress.reviewRound ?? 0,
      }).then((value) => {
        const result = value as ClaudeCodexReviewResult;
        if (result.fixPrompt) {
          pty.updateClaudeCodexHookProgress(terminal.id, {
            ...progress,
            phase: 'awaiting_fix',
            reviewRound: result.reviewRound,
            reviewInFlight: false,
          });
          pty.sendSmartInputToTerminal(terminal.id, result.fixPrompt, [], 'build');
          return;
        }
        if (result.done) {
          queueClaudeCodexUiVerification(project, result.planPath, result.uiChangedFiles);
        }
        pty.updateClaudeCodexHookProgress(terminal.id, {
          ...progress,
          phase: result.blockedReason ? 'blocked' : 'done',
          reviewRound: result.reviewRound,
          reviewInFlight: false,
          error: result.blockedReason || result.reviewError,
        });
      }).catch((error) => {
        pty.updateClaudeCodexHookProgress(terminal.id, {
          ...progress,
          phase: 'blocked',
          reviewInFlight: false,
          error: error instanceof Error ? error.message : String(error),
        });
      });
    }
  }, [config, pty, queueClaudeCodexUiVerification, terminals]);

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
    pty.updateOpencodeManualScrollDetached(terminalId, detached);
  }, [pty]);

  // Clear ACP chat state for projects that no longer exist in config
  useEffect(() => {
    if (!config) return;
    const existingIds = new Set(config.projects.map((p) => p.id));
    // Find composite keys whose projectId no longer exists
    const removedKeys = Array.from(activeAcpChatByProject.keys()).filter((key) => {
      const pid = parseInt(key.split(':')[0], 10);
      return !existingIds.has(pid);
    });
    if (removedKeys.length === 0) return;
    const removedKeySet = new Set(removedKeys);
    // Collect removed projectIds for standby cleanup
    const removedProjectIds = new Set(removedKeys.map((key) => parseInt(key.split(':')[0], 10)));

    setActiveAcpChatByProject((prev) => {
      const next = new Map(prev);
      for (const key of removedKeySet) next.delete(key);
      return next;
    });
    if (activeAcpChat && removedProjectIds.has(activeAcpChat.projectId)) {
      setActiveAcpChat(null);
    }
    setActiveAcpSessionByProject((prev) => {
      const next = new Map(prev);
      for (const key of removedKeySet) next.delete(key);
      return next;
    });
    setActiveAcpAttentionByProject((prev) => {
      const next = new Map(prev);
      for (const key of removedKeySet) next.delete(key);
      return next;
    });
    // Kill ACP chats and clear standby for removed projects
    for (const key of removedKeys) {
      const chatId = activeAcpChatByProject.get(key);
      if (chatId) {
        api.invoke('acp:kill', chatId);
      }
    }
    for (const projectId of removedProjectIds) {
      api.invoke('acp:standby:clear', projectId);
    }
  }, [config, activeAcpChat, activeAcpChatByProject]);

  // Ensure Smart Input focus when visible and no override
  useEffect(() => {
    const activeTerminal = activeTerminals.find((t) => t.id === activeTerminalId);
    if (!activeTerminal) return;
    const showSmartInput = shouldShowSmartInputFooter(activeTerminal.kind, activeTerminal.aiTool, activeTerminal.aiStatus, activeTerminal.opencodeSessionActive, activeTerminal.claudeLaunchPending);
    if (!showSmartInput) return;
    if (activeTerminal.terminalOutputFocusOverride) return;
    if (activeAcpChat) return;
    // Surrender Smart Input focus when any modal/popup is open
    if (settingsOpen) return;
    // Do not steal focus from browser URL input or other text inputs
    const active = document.activeElement;
    if (active && (active.hasAttribute('data-browser-url') || active.closest('[data-browser-panel]'))) return;
    if (active && (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement || active instanceof HTMLSelectElement || (active instanceof HTMLElement && active.isContentEditable))) return;
    // Auto-focus Smart Input draft
    const smartInput = document.querySelector(`[data-smart-input="${activeTerminalId}"]`) as HTMLElement | null;
    if (smartInput && document.activeElement !== smartInput) {
      smartInput.focus();
    }
  }, [activeTerminals, activeTerminalId, activeAcpChat, settingsOpen]);

  const leftSidebarVisible = Boolean(config?.ui.showProjectExplorer && config.ui.projectExplorerExpanded);
  const openLeftSidebarTab = useCallback((tab: LeftSidebarTab) => {
    setActiveTab(tab);
    setConfig((prev) => prev ? withLeftSidebarRailToggle(prev, tab) : prev);
  }, []);

  return (
    <div className="app-container">
      <GlobalTooltip />
      <div className="activity-rail">
        <button
          className={`rail-btn ${isLeftSidebarTabActive(config, LeftSidebarTabEnum.Directory) ? 'active' : ''}`}
          onClick={() => openLeftSidebarTab(LeftSidebarTabEnum.Directory)}
          data-tooltip={activityRailItem('directory').title}
          data-tooltip-right=""
          aria-label={activityRailItem('directory').title}
        >
          <span className="rail-icon">{activityRailItem('directory').icon}</span>
        </button>
        <button
          className={`rail-btn ${isLeftSidebarTabActive(config, LeftSidebarTabEnum.TerminalManager) ? 'active' : ''}`}
          onClick={() => openLeftSidebarTab(LeftSidebarTabEnum.TerminalManager)}
          data-tooltip={activityRailItem('terminalManager').title}
          data-tooltip-right=""
          aria-label={activityRailItem('terminalManager').title}
        >
          <span className="rail-icon terminal">{activityRailItem('terminalManager').icon}</span>
        </button>
        <button
          className={`rail-btn ${isLeftSidebarTabActive(config, LeftSidebarTabEnum.InputHistory) ? 'active' : ''}`}
          onClick={() => openLeftSidebarTab(LeftSidebarTabEnum.InputHistory)}
          data-tooltip={activityRailItem('inputHistory').title}
          data-tooltip-right=""
          aria-label={activityRailItem('inputHistory').title}
        >
          <span className="rail-icon">{activityRailItem('inputHistory').icon}</span>
        </button>
        <div style={{ flex: 1 }} />
        <button
          className={`rail-btn ${rightPanelOpen ? 'active' : ''}`}
          onClick={() => toggleRightPanel()}
          data-tooltip={activityRailItem('tools').title}
          data-tooltip-right=""
          aria-label={activityRailItem('tools').title}
        >
          <span className="rail-icon">{activityRailItem('tools').icon}</span>
        </button>
        <button
          className={`rail-btn ${settingsOpen ? 'active' : ''}`}
          onClick={() => setSettingsOpen(true)}
          data-tooltip={activityRailItem('settings').title}
          data-tooltip-right=""
          aria-label={activityRailItem('settings').title}
        >
          <span className="rail-icon">{activityRailItem('settings').icon}</span>
        </button>
      </div>

      {leftSidebarVisible && (
        <div
          ref={sidebarRef}
          className="sidebar"
          style={{ width: sidebarWidth, minWidth: 200, maxWidth: 500, display: 'flex', flexDirection: 'column', overflow: 'hidden', borderRight: '1px solid #222' }}
        >
        {activeTab === LeftSidebarTabEnum.Directory && selectedProject && (
          <ProjectExplorer
            project={selectedProject}
            projects={config?.projects}
            selectedProjectId={selectedProjectId}
            selectedPath={selectedProject.path}
            onSelectProject={setSelectedProjectId}
            onAddProject={handleAddProject}
            onRemoveProject={(project) => {
              removeProjectsByPath([project.path]);
            }}
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
            onMarkClaudeLaunchPending={pty.markClaudeLaunchPending}
            onMarkLauncherAiTool={pty.markLauncherAiTool}
            onKillTerminal={killTerminal}
            rerunBackground={pty.rerunBackground}
            sendSavedMessageToTerminal={pty.sendSavedMessageToTerminal}
            activeAcpChatByProject={activeAcpChatByProject}
            activeAcpSessionByProject={activeAcpSessionByProject}
            activeAcpAttentionByProject={activeAcpAttentionByProject}
            activeAcpProjectId={activeAcpChat?.projectId ?? null}
            onActivateAcpChat={restoreActiveAcpForProject}
            onRemoveAcpChat={removeAcpChatForProject}
            onOpenAcpChat={openAcpChat}
            onOverlayOpenChange={setTerminalManagerOverlayOpen}
            onUpdateFilter={(terminalManagerFilter: TerminalManagerFilter) => {
              setConfig((prev) => prev ? withTerminalManagerFilter(prev, terminalManagerFilter) : prev);
            }}
            onToggleHideInactiveProjects={() => {
              setConfig((prev) => prev ? withToggledTerminalManagerHideInactive(prev) : prev);
            }}
          />
        )}
        {activeTab === LeftSidebarTabEnum.InputHistory && config && (
          <InputHistory
            config={config}
            history={history}
            selectedProjectId={selectedProjectId}
            onUpdateFilter={(inputHistoryFilter: InputHistoryFilter) => {
              setConfig((prev) => prev ? { ...prev, ui: { ...prev.ui, inputHistoryFilter } } : prev);
            }}
          />
        )}
        </div>
      )}

      {leftSidebarVisible && (
        <div
          className="resize-handle"
          onPointerDown={(event) => {
            event.preventDefault();
            event.stopPropagation();
            sidebarResizeStartRef.current = { pointerX: event.clientX, width: sidebarWidth };
            event.currentTarget.setPointerCapture?.(event.pointerId);
            setIsResizing(true);
          }}
          style={{
            width: 4,
            cursor: 'col-resize',
            background: isResizing ? '#0078d4' : undefined,
          }}
        />
      )}

      <div className="main-area" ref={mainRef}>
        {activeAcpChat && acpProject ? (
          <AcpErrorBoundary key={activeAcpChat.chatId} onClose={closeAcpChat}>
            <AcpChatPanel
              project={acpProject}
              chatId={activeAcpChat.chatId}
              config={config || defaultAppConfig()}
              onClose={closeAcpChat}
              disabled={settingsOpen}
              branchName={branchNameByProject.get(activeAcpChat.projectId)}
              draft={acpDraftByChatId.get(activeAcpChat.chatId)}
              onDraftChange={(id, text) => setAcpDraftByChatId((prev) => { const next = new Map(prev); next.set(id, text); return next; })}
            />
          </AcpErrorBoundary>
        ) : fileEditorOpen && fileEditorPath && fileEditorName ? (
          <FileEditor
            filePath={fileEditorPath}
            displayName={fileEditorName}
            canNavigateBack={fileEditorState.backStack.length > 0}
            canNavigateForward={fileEditorState.forwardStack.length > 0}
            onNavigateBack={navigateFileEditorBack}
            onNavigateForward={navigateFileEditorForward}
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
            wheelEnabled={terminalWheelEnabled({ settingsOpen, terminalManagerOverlayOpen })}
            disabled={settingsOpen}
          />
        )}
      </div>

      {rightPanelOpen && selectedProject && config && (
        <>
          <div
            className="resize-handle"
            onPointerDown={(event) => {
              event.preventDefault();
              event.stopPropagation();
              rightPanelResizeStartRef.current = { pointerX: event.clientX, width: rightPanelWidth };
              event.currentTarget.setPointerCapture?.(event.pointerId);
              setIsResizingRightPanel(true);
            }}
            style={{
              width: 4,
              cursor: 'col-resize',
              background: isResizingRightPanel ? '#0078d4' : undefined,
            }}
          />
          <div style={{ width: rightPanelWidth, minWidth: 240, maxWidth: 800, display: 'flex', flexDirection: 'column', overflow: 'hidden', borderLeft: '1px solid #222' }}>
            <RightPanel
              activeTab={rightPanelTab}
              onTabChange={(tab) => {
                setRightPanelTab(tab);
                if (tab === 'browser' && selectedProjectId) {
                  const hasForegroundTerminal = terminals.some((t) => t.projectId === selectedProjectId && t.kind === 'foreground' && !t.exited);
                  const prefix = `${selectedProjectId}:`;
                  const hasAcpChat = Array.from(activeAcpChatByProject.keys()).some((k) => k.startsWith(prefix));
                  if (hasForegroundTerminal || hasAcpChat) {
                    setBrowserOpenProjects((prev) => {
                      const next = new Set(prev);
                      next.add(selectedProjectId);
                      return next;
                    });
                  }
                }
              }}
              onClose={() => setRightPanelOpen(false)}
              project={selectedProject}
              projects={config.projects}
              selectedProjectId={selectedProjectId}
              onSelectProject={setSelectedProjectId}
              registeredWorktreePaths={config.projects
                .filter((p) => p.isWorktree && p.repoRoot === selectedProject.path)
                .map((p) => p.path)}
              onOrphanWorktrees={(orphanPaths) => {
                const removedProjects = removeProjectsByPath(orphanPaths);
                if (removedProjects.length === 0) return;
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
              onRemoveWorktree={(worktree) => {
                removeProjectsByPath([worktree.path]);
              }}
              onAddWorktree={(worktree) => {
                if (!config) return;
                const newProject: ProjectRecord = {
                  id: Date.now() + Math.floor(Math.random() * 1000),
                  name: worktree.branch || 'worktree',
                  path: worktree.path,
                  savedMessages: config.projects.find((p) => p.path === selectedProject?.repoRoot || p.path === selectedProject?.path)?.savedMessages || [],
                  aiConfig: {},
                  checklist: [],
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
                removeProjectsByPath([worktree.path]);
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
              browserVisible={isBrowserOpen}
              activeTerminalId={activeTerminalId}
              visibleScopeOverride={selectedProjectId != null ? browserPanelVisibleScopeByProject.get(selectedProjectId) ?? undefined : undefined}
              tabsByScope={browserTabsByScope}
              activeTabByScope={browserActiveTabByScope}
              urlDraftByScope={browserUrlDraftByScope}
              designInspectByScope={browserDesignInspectByScope}
              onTabsChange={setBrowserTabsByScope}
              onActiveTabChange={setBrowserActiveTabByScope}
              onUrlDraftChange={setBrowserUrlDraftByScope}
              onDesignInspectChange={setBrowserDesignInspectByScope}
              onScopeEmpty={(scope) => {
                setBrowserOpenProjects((prev) => browserProjectIdsAfterScopeEmpty(prev, scope));
                if (scope.type === BrowserScopeKeyType.Terminal && selectedProjectId != null) {
                  setBrowserPanelVisibleScopeByProject((prev) => {
                    const override = prev.get(selectedProjectId!);
                    if (override && override.type === BrowserScopeKeyType.Terminal && override.terminalId === scope.terminalId) {
                      const copy = new Map(prev);
                      copy.delete(selectedProjectId!);
                      return copy;
                    }
                    return prev;
                  });
                }
              }}
              onClearProjectBrowserLastUrl={clearProjectBrowserLastUrl}
              settingsOpen={settingsOpen}
            />
          </div>
        </>
      )}

      {settingsOpen && config && (
        <SettingsPopup
          config={config}
          activeTerminal={activeTerminalForSettings ? {
            id: activeTerminalForSettings.id,
            title: activeTerminalForSettings.title,
            cwd: activeTerminalForSettings.cwd,
            kind: activeTerminalForSettings.kind,
            aiTool: activeTerminalForSettings.aiTool,
            aiStatus: activeTerminalForSettings.aiStatus,
            aiStatusReason: activeTerminalForSettings.aiStatusReason,
            opencodeSessionActive: activeTerminalForSettings.opencodeSessionActive,
            opencodeTransportStatus: activeTerminalForSettings.opencodeTransportStatus,
            opencodeAttentionReason: activeTerminalForSettings.opencodeAttentionReason,
            exited: activeTerminalForSettings.exited,
          } : undefined}
          onSave={(newConfig) => setConfig(newConfig)}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}

export default App;
