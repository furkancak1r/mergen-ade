import React, { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import type { AcpChatSession, AppConfig, TerminalKind, TerminalManagerFilter, ProjectRecord, LauncherEntry } from '../../../shared/types';
import { TerminalKind as TerminalKindEnum, TerminalManagerFilter as TerminalManagerFilterEnum, BuiltinLauncherKind, activeBuildModel } from '../../../shared/types';
import type { GitDiffSummary } from '../../../shared/gitDiffSummary';
import { gitDiffSummaryLabel } from '../../../shared/gitDiffSummary';
import type { TerminalInstance } from '../hooks/usePty';
import {
  OPENCODE_ACP_OPEN_BUTTON_LABEL,
  acpLabelForTool,
  acpStatusText,
  acpTerminalManagerRowLabel,
  type AcpTerminalManagerAttentionReason,
  type AcpTerminalManagerBadgeVisual,
} from '../lib/acpUi';
import { effectiveLauncherCommand } from '../lib/launcher';
import { effectiveAiStatusForDisplay } from '../lib/smartInput';
import { shouldShowOpenCodeAcpButton, terminalManagerPathMenuLabel } from '../lib/terminalManagerState';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: unknown[]) => void) => () => void } }).mergenApi;

// Exact color palette from Rust reference (app.rs)
const APP_BG = '#101010';
const SURFACE_BG = '#181818';
const SURFACE_BG_SOFT = '#1e1e1e';
const BORDER_COLOR = '#2a2a2a';
const ACCENT = '#aaaaaa';
const TEXT_PRIMARY = '#f4f4f4';
const TEXT_MUTED = '#8a8a8a';
const BTN_BLUE = '#2563eb';
const BTN_BLUE_HOVER = '#1d4ed8';
const BTN_TEAL = '#0f4c5c';
const BTN_TEAL_HOVER = '#146478';
const BTN_RED = '#b92d2d';
const BTN_RED_HOVER = '#dc3c3c';
const BTN_ICON = '#1e1e1e';
const BTN_ICON_HOVER = '#2d2d2d';
const BTN_ICON_ACTIVE = '#2563eb';
const CONTROL_ROW_HEIGHT = 28;
const TERMINAL_MANAGER_FILTER_ROW_HEIGHT_EXTRA = 12;
const TERMINAL_MANAGER_FILTER_SIDE_PADDING = 8;
const TERMINAL_MANAGER_FILTER_CENTER_GAP = 28;
const TERMINAL_MANAGER_FILTER_TOP_PADDING = 4;
const TERMINAL_MANAGER_FILTER_UNDERLINE_GAP = 2;
const TERMINAL_MANAGER_FILTER_UNDERLINE_HEIGHT = 2;
const TERMINAL_MANAGER_WORKTREE_TERMINAL_EXTRA_INDENT = 10;
const TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH = 32;
const SIDEBAR_ROW_LEADING_INSET = 6;
const TERMINAL_HOVER_WIDTH = 320;
const TERMINAL_HISTORY_POPUP_MAX_HEIGHT = 400;
const TERMINAL_HISTORY_MESSAGE_MAX_HEIGHT = 120;
const TERMINAL_HISTORY_POPUP_MAX_VISIBLE_ENTRIES = 5;
const TERMINAL_HISTORY_POPUP_CHROME_HEIGHT_ESTIMATE = 56;
const TERMINAL_HISTORY_POPUP_ROW_GAP = 4;
const TERMINAL_MANAGER_DIFF_REFRESH_INTERVAL_MS = 30_000;
const TERMINAL_AI_BADGE_RUNNING_BORDER = 'rgba(100, 200, 100, 0.25)';
const TERMINAL_AI_BADGE_RUNNING = '#64c864';
const TERMINAL_AI_BADGE_ATTENTION = '#e8a838';
const TERMINAL_AI_BADGE_INACTIVE = '#666';

type TerminalManagerPathContextKind = 'project' | 'worktree';

interface TerminalManagerPathContextMenu {
  x: number;
  y: number;
  path: string;
  name: string;
  kind: TerminalManagerPathContextKind;
}

function withAlpha(color: string, alpha: number): string {
  const r = parseInt(color.slice(1, 3), 16);
  const g = parseInt(color.slice(3, 5), 16);
  const b = parseInt(color.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha / 255})`;
}

function cappedHoverText(text: string, maxChars: number = 100): string {
  const chars = Array.from(text);
  if (chars.length <= maxChars) return text;
  return chars.slice(0, maxChars).join('') + '…';
}

interface TerminalManagerProps {
  config: AppConfig;
  terminals: TerminalInstance[];
  activeTerminalId: number | null;
  onActivateTerminal: (id: number) => void;
  onSpawnTerminal: (projectId: number, kind: TerminalKind) => Promise<number>;
  onMarkClaudeLaunchPending?: (terminalId: number, title?: string) => void;
  onKillTerminal: (id: number) => void;
  rerunBackground: (terminalId: number) => void;
  sendSavedMessageToTerminal?: (terminalId: number, message: string, recordRecentInput: boolean) => void;
  activeAcpChatByProject?: Map<string, string>;
  activeAcpSessionByProject?: Map<string, AcpChatSession>;
  activeAcpAttentionByProject?: Map<string, AcpTerminalManagerAttentionReason>;
  activeAcpProjectId?: number | null;
  onActivateAcpChat?: (projectId: number) => void;
  onRemoveAcpChat?: (projectId: number) => void;
  onOpenAcpChat?: (projectId: number, tool?: string) => void;
  onOverlayOpenChange?: (open: boolean) => void;
  onUpdateFilter?: (filter: TerminalManagerFilter) => void;
  onToggleHideInactiveProjects?: () => void;
}

export const TerminalManager: React.FC<TerminalManagerProps> = ({
  config,
  terminals,
  activeTerminalId,
  onActivateTerminal,
  onSpawnTerminal,
  onMarkClaudeLaunchPending,
  onKillTerminal,
  rerunBackground,
  sendSavedMessageToTerminal,
  activeAcpChatByProject,
  activeAcpSessionByProject,
  activeAcpAttentionByProject,
  activeAcpProjectId,
  onActivateAcpChat,
  onRemoveAcpChat,
  onOpenAcpChat,
  onOverlayOpenChange,
  onUpdateFilter,
  onToggleHideInactiveProjects,
}) => {
  // Only expand projects that have active terminals or ACP chats
  const initialExpanded = useMemo(() => {
    const expanded = new Set<number>();
    for (const p of config.projects) {
      const hasTerminal = terminals.some((t) => t.projectId === p.id && !t.exited);
      const prefix = `${p.id}:`;
      const hasAcp = activeAcpChatByProject ? Array.from(activeAcpChatByProject.keys()).some((k) => k.startsWith(prefix)) : false;
      if (hasTerminal || hasAcp) expanded.add(p.id);
    }
    return expanded;
  }, [config.projects, terminals, activeAcpChatByProject]);
  const [expandedProjects, setExpandedProjects] = useState<Set<number>>(initialExpanded);
  const [showSavedMessages, setShowSavedMessages] = useState<number | null>(null);
  const [showLauncherMenu, setShowLauncherMenu] = useState<number | null>(null);
  const [diffSummaries, setDiffSummaries] = useState<Map<number, GitDiffSummary>>(new Map());
  const [diffSummaryLoading, setDiffSummaryLoading] = useState<Set<number>>(new Set());
  const [historyPopupTerminalId, setHistoryPopupTerminalId] = useState<number | null>(null);
  const [historyPopupJustOpened, setHistoryPopupJustOpened] = useState(false);
  const [pathContextMenu, setPathContextMenu] = useState<TerminalManagerPathContextMenu | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const feedbackTimerRef = useRef<number | null>(null);

  const overlayOpen = showSavedMessages !== null
    || showLauncherMenu !== null
    || historyPopupTerminalId !== null
    || pathContextMenu !== null;

  useEffect(() => {
    onOverlayOpenChange?.(overlayOpen);
    return () => onOverlayOpenChange?.(false);
  }, [overlayOpen, onOverlayOpenChange]);

  const projectDiffKey = config.projects.map((p) => `${p.id}:${p.path}`).join('\0');

  useEffect(() => {
    let cancelled = false;

    const refreshDiffSummaries = (showLoading: boolean) => {
      const projects = config.projects;
      if (showLoading) {
        setDiffSummaryLoading(new Set(projects.map((p) => p.id)));
      }
      setDiffSummaries((prev) => {
        const next = new Map<number, GitDiffSummary>();
        for (const project of projects) {
          const existing = prev.get(project.id);
          if (existing) next.set(project.id, existing);
        }
        return next;
      });

      for (const project of projects) {
        api.invoke('git:diffSummary', project.path)
          .then((value) => {
            if (cancelled) return;
            setDiffSummaries((prev) => new Map(prev).set(project.id, value as GitDiffSummary));
          })
          .catch((error) => {
            if (cancelled) return;
            setDiffSummaries((prev) => new Map(prev).set(project.id, {
              status: 'error',
              addedLines: 0,
              removedLines: 0,
              error: error instanceof Error ? error.message : String(error),
            }));
          })
          .finally(() => {
            if (cancelled || !showLoading) return;
            setDiffSummaryLoading((prev) => {
              const next = new Set(prev);
              next.delete(project.id);
              return next;
            });
          });
      }
    };

    refreshDiffSummaries(true);
    const interval = window.setInterval(
      () => refreshDiffSummaries(false),
      TERMINAL_MANAGER_DIFF_REFRESH_INTERVAL_MS,
    );

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [projectDiffKey]);

  const toggleProject = useCallback((projectId: number) => {
    setExpandedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
      }
      return next;
    });
  }, []);

  const filter = config.ui.terminalManagerFilter ?? TerminalManagerFilterEnum.Foreground;
  const hideInactive = config.ui.terminalManagerHideInactiveProjects;

  const filteredTerminals = terminals.filter((t) => {
    if (filter === TerminalManagerFilterEnum.Foreground) return t.kind === TerminalKindEnum.Foreground;
    if (filter === TerminalManagerFilterEnum.Background) return t.kind === TerminalKindEnum.Background;
    return true;
  });

  const getProjectTerminals = (projectId: number) => filteredTerminals.filter((t) => t.projectId === projectId);

  const showFeedback = useCallback((message: string) => {
    setFeedback(message);
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
    feedbackTimerRef.current = window.setTimeout(() => {
      setFeedback(null);
      feedbackTimerRef.current = null;
    }, 1600);
  }, []);

  useEffect(() => () => {
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
  }, []);

  const sendSavedMessage = useCallback(async (projectId: number, message: string, kind: TerminalKind) => {
    let target: TerminalInstance | undefined;
    if (kind === TerminalKindEnum.Foreground) {
      target = terminals.find((t) => t.projectId === projectId && t.kind === kind && t.id === activeTerminalId);
      if (!target) {
        target = terminals.find((t) => t.projectId === projectId && t.kind === kind);
      }
    } else {
      target = terminals.find((t) => t.projectId === projectId && t.kind === kind);
    }
    if (!target) return;
    sendSavedMessageToTerminal?.(target.id, message, kind === TerminalKindEnum.Background);
  }, [terminals, activeTerminalId, sendSavedMessageToTerminal]);

  // Group projects: root projects first, then their worktrees
  const rootProjects = config.projects.filter((p) => !p.isWorktree);
  const worktreesByRoot = new Map<number, ProjectRecord[]>();
  for (const p of config.projects) {
    if (p.isWorktree && p.repoRoot) {
      const root = config.projects.find((r) => r.path === p.repoRoot);
      if (root) {
        const list = worktreesByRoot.get(root.id) || [];
        list.push(p);
        worktreesByRoot.set(root.id, list);
      }
    }
  }

  // Close history popup when clicking outside
  useEffect(() => {
    if (historyPopupTerminalId === null) return;
    const handleClick = (e: MouseEvent) => {
      if (historyPopupJustOpened) {
        setHistoryPopupJustOpened(false);
        return;
      }
      const popup = document.querySelector('[data-history-popup]');
      if (popup && !popup.contains(e.target as Node)) {
        setHistoryPopupTerminalId(null);
      }
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  }, [historyPopupTerminalId, historyPopupJustOpened]);

  // Close saved messages popup when clicking outside
  useEffect(() => {
    if (showSavedMessages === null) return;
    const handleClick = (e: MouseEvent) => {
      const savedPopup = document.querySelector('[data-saved-popup]');
      if (savedPopup && !savedPopup.contains(e.target as Node)) {
        setShowSavedMessages(null);
      }
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  }, [showSavedMessages]);

  // Close launcher menu when clicking outside
  useEffect(() => {
    if (showLauncherMenu === null) return;
    const handleClick = (e: MouseEvent) => {
      const launcherPopup = document.querySelector('[data-launcher-popup]');
      if (launcherPopup && !launcherPopup.contains(e.target as Node)) {
        setShowLauncherMenu(null);
      }
    };
    document.addEventListener('click', handleClick);
    return () => document.removeEventListener('click', handleClick);
  }, [showLauncherMenu]);

  useEffect(() => {
    if (!pathContextMenu) return undefined;

    const closeMenu = () => setPathContextMenu(null);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('click', closeMenu);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('click', closeMenu);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [pathContextMenu]);

  const openPathContextMenu = useCallback((event: React.MouseEvent, project: ProjectRecord, kind: TerminalManagerPathContextKind) => {
    event.preventDefault();
    event.stopPropagation();
    setShowSavedMessages(null);
    setShowLauncherMenu(null);
    setPathContextMenu({
      x: event.clientX,
      y: event.clientY,
      path: project.path,
      name: project.name,
      kind,
    });
  }, []);

  return (
    <div ref={panelRef} className="terminal-manager" style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', background: SURFACE_BG, position: 'relative' }}>
      {/* Filter tabs */}
      <div style={{ display: 'flex', padding: `${TERMINAL_MANAGER_FILTER_TOP_PADDING}px ${TERMINAL_MANAGER_FILTER_SIDE_PADDING}px`, borderBottom: `1px solid ${BORDER_COLOR}`, height: CONTROL_ROW_HEIGHT + TERMINAL_MANAGER_FILTER_ROW_HEIGHT_EXTRA }}>
        {([TerminalManagerFilterEnum.Foreground, TerminalManagerFilterEnum.Background] as TerminalManagerFilter[]).map((f) => {
          const isSelected = filter === f;
          const label = f === TerminalManagerFilterEnum.Foreground ? 'Foreground' : 'Background';
          const color = f === TerminalManagerFilterEnum.Foreground ? ACCENT : TEXT_MUTED;
          return (
            <div
              key={f}
              onClick={() => onUpdateFilter?.(f)}
              style={{
                flex: 1,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                cursor: 'pointer',
                position: 'relative',
              }}
            >
              <span style={{
                fontSize: 13,
                fontWeight: 600,
                color: isSelected ? color : withAlpha(TEXT_MUTED, 180),
                userSelect: 'none',
                borderBottom: isSelected ? `${TERMINAL_MANAGER_FILTER_UNDERLINE_HEIGHT}px solid ${color}` : 'none',
                paddingBottom: isSelected ? TERMINAL_MANAGER_FILTER_UNDERLINE_GAP : 0,
              }}>
                {label}
              </span>
            </div>
          );
        })}
        <button
          onClick={onToggleHideInactiveProjects}
          data-tooltip={hideInactive ? 'Show projects without live terminals' : 'Hide projects without live terminals'}
          style={{
            width: 28,
            height: 24,
            alignSelf: 'center',
            marginLeft: 4,
            borderRadius: 4,
            border: `1px solid ${hideInactive ? BTN_ICON_ACTIVE : BORDER_COLOR}`,
            background: hideInactive ? withAlpha(BTN_ICON_ACTIVE, 170) : BTN_ICON,
            color: hideInactive ? TEXT_PRIMARY : TEXT_MUTED,
            cursor: 'pointer',
            fontSize: 12,
            lineHeight: 1,
          }}
        >
          {hideInactive ? '◉' : '○'}
        </button>
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {rootProjects.map((project) => {
          const childWorktrees = worktreesByRoot.get(project.id) || [];
          const visibleCount = getProjectTerminals(project.id).length;
          const hasLiveTerminal = getProjectTerminals(project.id).some((t) => !t.exited);
          const worktreeVisibleCount = childWorktrees.reduce((sum, wt) => sum + getProjectTerminals(wt.id).length, 0);
          const worktreeHasLive = childWorktrees.some((wt) => getProjectTerminals(wt.id).some((t) => !t.exited));

          if (hideInactive && !hasLiveTerminal && !worktreeHasLive && childWorktrees.length === 0) {
            return null;
          }

          const hasChildren = visibleCount > 0 || childWorktrees.length > 0 || worktreeVisibleCount > 0;
          const expanded = expandedProjects.has(project.id);

          return (
            <div key={project.id}>
              <ProjectGroup
                project={project}
                terminals={getProjectTerminals(project.id)}
                diffSummary={diffSummaries.get(project.id)}
                diffSummaryLoading={diffSummaryLoading.has(project.id)}
                filter={filter}
                expanded={expanded}
                onToggle={() => toggleProject(project.id)}
                activeTerminalId={activeTerminalId}
                onActivate={onActivateTerminal}
                onSpawn={onSpawnTerminal}
                onMarkClaudeLaunchPending={onMarkClaudeLaunchPending}
                onKill={onKillTerminal}
                showSavedMessages={showSavedMessages}
                setShowSavedMessages={setShowSavedMessages}
                showLauncherMenu={showLauncherMenu}
                setShowLauncherMenu={setShowLauncherMenu}
                sendSavedMessage={sendSavedMessage}
                rerunBackground={rerunBackground}
                config={config}
                allTerminals={terminals}
                rootProject={project}
                activeAcpChatByProject={activeAcpChatByProject}
                activeAcpSessionByProject={activeAcpSessionByProject}
                activeAcpAttentionByProject={activeAcpAttentionByProject}
                activeAcpProjectId={activeAcpProjectId}
                onActivateAcpChat={onActivateAcpChat}
                onRemoveAcpChat={onRemoveAcpChat}
                onOpenAcpChat={onOpenAcpChat}
                onPathContextMenu={openPathContextMenu}
                historyPopupTerminalId={historyPopupTerminalId}
                setHistoryPopupTerminalId={(id) => {
                  setHistoryPopupTerminalId(id);
                  if (id !== null) setHistoryPopupJustOpened(true);
                }}
                setHistoryPopupJustOpened={setHistoryPopupJustOpened}
                panelRight={panelRef.current?.getBoundingClientRect().right ?? 0}
              />
              {/* Worktrees under this root */}
              {expanded && childWorktrees.map((worktree) => (
                <div key={worktree.id} style={{ paddingLeft: 12 }}>
                  <ProjectGroup
                    project={worktree}
                    terminals={getProjectTerminals(worktree.id)}
                    diffSummary={diffSummaries.get(worktree.id)}
                    diffSummaryLoading={diffSummaryLoading.has(worktree.id)}
                    filter={filter}
                    expanded={expandedProjects.has(worktree.id)}
                    onToggle={() => toggleProject(worktree.id)}
                    activeTerminalId={activeTerminalId}
                    onActivate={onActivateTerminal}
                    onSpawn={onSpawnTerminal}
                    onMarkClaudeLaunchPending={onMarkClaudeLaunchPending}
                    onKill={onKillTerminal}
                    showSavedMessages={showSavedMessages}
                    setShowSavedMessages={setShowSavedMessages}
                    showLauncherMenu={showLauncherMenu}
                    setShowLauncherMenu={setShowLauncherMenu}
                    sendSavedMessage={sendSavedMessage}
                    rerunBackground={rerunBackground}
                    config={config}
                    allTerminals={terminals}
                    rootProject={project}
                    isWorktree
                    activeAcpChatByProject={activeAcpChatByProject}
                    activeAcpSessionByProject={activeAcpSessionByProject}
                    activeAcpAttentionByProject={activeAcpAttentionByProject}
                    activeAcpProjectId={activeAcpProjectId}
                    onActivateAcpChat={onActivateAcpChat}
                    onRemoveAcpChat={onRemoveAcpChat}
                    onOpenAcpChat={onOpenAcpChat}
                    onPathContextMenu={openPathContextMenu}
                    historyPopupTerminalId={historyPopupTerminalId}
                    setHistoryPopupTerminalId={(id) => {
                      setHistoryPopupTerminalId(id);
                      if (id !== null) setHistoryPopupJustOpened(true);
                    }}
                    setHistoryPopupJustOpened={setHistoryPopupJustOpened}
                    panelRight={panelRef.current?.getBoundingClientRect().right ?? 0}
                  />
                </div>
              ))}
            </div>
          );
        })}
        {config.projects.length === 0 && (
          <div style={{ padding: 12, color: TEXT_MUTED, fontSize: 12 }}>No projects. Add a project in Settings.</div>
        )}
      </div>

      {feedback && (
        <div className="terminal-manager-feedback-toast" role="status">
          {feedback}
        </div>
      )}

      {pathContextMenu && (
        <div
          className="terminal-manager-context-menu"
          style={{ left: pathContextMenu.x, top: pathContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            onClick={() => {
              const { path, name, kind } = pathContextMenu;
              api.invoke('clipboard:writeText', path).catch(() => {});
              showFeedback(`Copied path for ${kind} '${name}'`);
              setPathContextMenu(null);
            }}
          >
            {terminalManagerPathMenuLabel('copy_path')}
          </button>
          <button
            type="button"
            onClick={() => {
              const { path, name, kind } = pathContextMenu;
              api.invoke('shell:showItemInFolder', path)
                .then(() => showFeedback(`Opened ${kind} '${name}' in Explorer`))
                .catch((error) => showFeedback(`Open folder failed: ${error instanceof Error ? error.message : String(error)}`));
              setPathContextMenu(null);
            }}
          >
            {terminalManagerPathMenuLabel('open_folder')}
          </button>
        </div>
      )}
    </div>
  );
};

interface ProjectGroupProps {
  project: ProjectRecord;
  terminals: TerminalInstance[];
  diffSummary?: GitDiffSummary;
  diffSummaryLoading: boolean;
  filter: TerminalManagerFilter;
  expanded: boolean;
  onToggle: () => void;
  activeTerminalId: number | null;
  onActivate: (id: number) => void;
  onSpawn: (projectId: number, kind: TerminalKind) => Promise<number>;
  onMarkClaudeLaunchPending?: (terminalId: number, title?: string) => void;
  onKill: (id: number) => void;
  showSavedMessages: number | null;
  setShowSavedMessages: (id: number | null) => void;
  showLauncherMenu: number | null;
  setShowLauncherMenu: (id: number | null) => void;
  sendSavedMessage: (projectId: number, message: string, kind: TerminalKind) => void;
  rerunBackground: (terminalId: number) => void;
  config: AppConfig;
  allTerminals: TerminalInstance[];
  rootProject?: ProjectRecord;
  isWorktree?: boolean;
  activeAcpChatByProject?: Map<string, string>;
  activeAcpSessionByProject?: Map<string, AcpChatSession>;
  activeAcpAttentionByProject?: Map<string, AcpTerminalManagerAttentionReason>;
  activeAcpProjectId?: number | null;
  onActivateAcpChat?: (projectId: number) => void;
  onRemoveAcpChat?: (projectId: number) => void;
  onOpenAcpChat?: (projectId: number, tool?: string) => void;
  onPathContextMenu?: (event: React.MouseEvent, project: ProjectRecord, kind: TerminalManagerPathContextKind) => void;
  historyPopupTerminalId: number | null;
  setHistoryPopupTerminalId: (id: number | null) => void;
  setHistoryPopupJustOpened: (v: boolean) => void;
  panelRight: number;
}

const ProjectGroup: React.FC<ProjectGroupProps> = ({
  project,
  terminals,
  diffSummary,
  diffSummaryLoading,
  filter,
  expanded,
  onToggle,
  activeTerminalId,
  onActivate,
  onSpawn,
  onMarkClaudeLaunchPending,
  onKill,
  showSavedMessages,
  setShowSavedMessages,
  showLauncherMenu,
  setShowLauncherMenu,
  sendSavedMessage,
  rerunBackground,
  config,
  allTerminals,
  rootProject,
  isWorktree,
  activeAcpChatByProject,
  activeAcpSessionByProject,
  activeAcpAttentionByProject,
  activeAcpProjectId,
  onActivateAcpChat,
  onRemoveAcpChat,
  onOpenAcpChat,
  onPathContextMenu,
  historyPopupTerminalId,
  setHistoryPopupTerminalId,
  setHistoryPopupJustOpened,
  panelRight,
}) => {
  const [hoveredRow, setHoveredRow] = useState<number | null>(null);
  const [hoveredProject, setHoveredProject] = useState(false);
  const [hoveredAcpRow, setHoveredAcpRow] = useState(false);
  const [launcherMenuButtonRect, setLauncherMenuButtonRect] = useState<DOMRect | null>(null);
  const effectiveSavedMessages = isWorktree && rootProject ? rootProject.savedMessages : project.savedMessages;
  const hasSavedMessages = effectiveSavedMessages.length > 0;
  const hasLiveTerminals = terminals.length > 0;
  const isSelected = activeTerminalId !== null && terminals.some((t) => t.id === activeTerminalId);
  const hasLiveTerminal = terminals.some((t) => !t.exited);
  const acpKeyPrefix = `${project.id}:`;
  const hasActiveAcpChat = activeAcpChatByProject ? Array.from(activeAcpChatByProject.keys()).some((k) => k.startsWith(acpKeyPrefix)) : false;
  const activeAcpSession = activeAcpSessionByProject ? Array.from(activeAcpSessionByProject.entries()).find(([k]) => k.startsWith(acpKeyPrefix))?.[1] : undefined;
  const activeAcpStatus = hasActiveAcpChat ? (activeAcpSession?.status ?? 'starting') : undefined;
  const activeAcpAttention = activeAcpAttentionByProject ? Array.from(activeAcpAttentionByProject.entries()).find(([k]) => k.startsWith(acpKeyPrefix))?.[1] : undefined;
  const activeAcpBadge = acpTerminalManagerRowBadgeVisual(activeAcpStatus, activeAcpAttention);
  const activeAcpRow = activeAcpProjectId === project.id;
  const activeAcpRowChrome = terminalManagerRowChrome(activeAcpRow, hoveredAcpRow);
  const showOpenCodeAcpButton = shouldShowOpenCodeAcpButton(filter, hasActiveAcpChat);
  const diffLabel = gitDiffSummaryLabel(diffSummary, diffSummaryLoading);
  const showReadyDiff = Boolean(
    diffSummary
      && diffSummary.status === 'ready'
      && !diffSummaryLoading
      && (diffSummary.addedLines > 0 || diffSummary.removedLines > 0),
  );
  const diffTooltip = diffSummary?.status === 'error'
    ? diffSummary.error || 'Git diff summary unavailable'
    : 'Changed lines in this worktree';

  // Project header text color: bright only when has live terminal
  const headerTextColor = hasLiveTerminal ? TEXT_PRIMARY : withAlpha(TEXT_MUTED, 180);

  return (
    <div style={{ marginBottom: 2 }}>
      {/* Project / Worktree Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          height: CONTROL_ROW_HEIGHT,
          padding: '0 8px',
          cursor: 'pointer',
          borderRadius: 8,
          position: 'relative',
        }}
        onClick={onToggle}
        onMouseEnter={() => setHoveredProject(true)}
        onMouseLeave={() => setHoveredProject(false)}
        onContextMenu={(event) => {
          onPathContextMenu?.(event, project, isWorktree ? 'worktree' : 'project');
        }}
      >
        {/* Hover background */}
        {(hoveredProject || isSelected) && (
          <div style={{
            position: 'absolute',
            inset: 1,
            borderRadius: 8,
            background: isSelected ? withAlpha('#262626', 220) : withAlpha(BTN_ICON_HOVER, 110),
            border: isSelected ? `1px solid ${withAlpha('#646464', 220)}` : 'none',
            pointerEvents: 'none',
          }} />
        )}
        <ProjectFolderIcon expanded={expanded} color={headerTextColor} />
        <span style={{
          fontSize: 12,
          fontWeight: 600,
          color: headerTextColor,
          flex: 1,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          zIndex: 1,
          marginLeft: 4,
        }}>
          {project.name}
        </span>
        {filter === TerminalManagerFilterEnum.Foreground && hoveredProject && showReadyDiff ? (
          <span
            data-tooltip={diffTooltip}
            style={{
              display: 'inline-flex',
              gap: 4,
              alignItems: 'center',
              fontSize: 10,
              fontWeight: 600,
              zIndex: 1,
              marginLeft: 6,
              flexShrink: 0,
              fontVariantNumeric: 'tabular-nums',
            }}
          >
            <span style={{ color: '#64c38c' }}>+{diffSummary!.addedLines}</span>
            <span style={{ color: '#d47a7a' }}>-{diffSummary!.removedLines}</span>
          </span>
        ) : filter === TerminalManagerFilterEnum.Foreground && hoveredProject && diffLabel ? (
          <span
            data-tooltip={diffTooltip}
            style={{
              fontSize: 10,
              color: withAlpha(TEXT_MUTED, 150),
              zIndex: 1,
              marginLeft: 6,
              flexShrink: 0,
              fontVariantNumeric: 'tabular-nums',
            }}
          >
            {diffLabel}
          </span>
        ) : null}
        {isWorktree && (
          <span style={{ fontSize: 10, color: withAlpha(ACCENT, 200), zIndex: 1, marginLeft: 4 }}>🌿</span>
        )}

        {/* Action buttons on hover for project header */}
        {hoveredProject && (
          <div style={{ display: 'flex', gap: 2, zIndex: 1, marginLeft: 4 }}>
            {filter === TerminalManagerFilterEnum.Foreground && !isWorktree && (
              <>
                <IconButton
                  icon="▶"
                  tooltip="Open Foreground Launcher"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (!expanded) {
                      onToggle();
                    }
                    const isOpening = showLauncherMenu !== project.id;
                    setShowLauncherMenu(isOpening ? project.id : null);
                    if (isOpening) {
                      setLauncherMenuButtonRect(e.currentTarget.getBoundingClientRect());
                    }
                  }}
                />
              </>
            )}
            {filter === TerminalManagerFilterEnum.Background && (
              <IconButton
                icon="+"
                tooltip="New Background Terminal"
                onClick={async (e) => {
                  e.stopPropagation();
                  if (!expanded) {
                    onToggle();
                  }
                  await onSpawn(project.id, TerminalKindEnum.Background);
                }}
              />
            )}
          </div>
        )}
      </div>

      {expanded && (
        <div>
          {/* Launcher popup - positioned to the right of the panel */}
          {showLauncherMenu === project.id && (
            <div data-launcher-popup style={{
              position: 'fixed',
              zIndex: 20,
              top: launcherMenuButtonRect ? launcherMenuButtonRect.top : 100,
              left: Math.max(8, panelRight + 8),
              background: SURFACE_BG,
              border: `1px solid ${BORDER_COLOR}`,
              borderRadius: 4,
              padding: '4px 0',
              width: 168,
              boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
            }}>
              <div style={{ background: SURFACE_BG, borderRadius: 4 }}>
                {config.launchers.filter((l) => l.enabled).map((l) => (
                  <button
                    key={l.id}
                    onClick={async () => {
                      try {
                        setShowLauncherMenu(null);
                        if (l.builtin === BuiltinLauncherKind.OpenCodeAcp) {
                          onOpenAcpChat?.(project.id, 'opencode');
                          return;
                        }
                        if (l.builtin === BuiltinLauncherKind.ClaudeAcp) {
                          onOpenAcpChat?.(project.id, 'claude_acp');
                          return;
                        }
                        const cmd = effectiveLauncherCommand(l, config.defaultShell);
                        if (l.builtin === BuiltinLauncherKind.OpenCode) {
                          const model = activeBuildModel(config.opencode);
                          await api.invoke('opencode:generateTerminalConfig', {
                            cwd: project.path,
                            model,
                            effort: config.opencode.planEffort,
                            kimiStrictPermissions: config.opencode.kimiStrictPermissions,
                          });
                        }
                        const targetId = await onSpawn(project.id, TerminalKindEnum.Foreground);
                        if (targetId) {
                          if (l.builtin === BuiltinLauncherKind.Claude) {
                            onMarkClaudeLaunchPending?.(targetId, l.launchCommand || cmd);
                          }
                          const isSlash = cmd.startsWith('/');
                          await api.invoke('pty:write', targetId, '\x1b[200~' + cmd + '\x1b[201~');
                          await api.invoke('pty:write', targetId, '\r');
                          if (isSlash) {
                            setTimeout(() => api.invoke('pty:write', targetId, '\r'), 600);
                            setTimeout(() => api.invoke('pty:write', targetId, '\r'), 1200);
                          } else {
                            setTimeout(() => api.invoke('pty:write', targetId, '\r'), 1200);
                          }
                        }
                      } catch (err) {
                        console.error('Launcher click failed:', err);
                        alert('Launcher error: ' + (err instanceof Error ? err.message : String(err)));
                      }
                    }}
                    style={{
                      display: 'block',
                      width: '100%',
                      textAlign: 'left',
                      padding: '4px 8px',
                      fontSize: 11,
                      background: 'transparent',
                      border: 'none',
                      color: '#ccc',
                      cursor: 'pointer',
                    }}
                    data-tooltip={l.displayName + (l.launchCommand ? ' — ' + l.launchCommand : '')}
                  >
                    {l.displayName}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Saved messages popup - per terminal */}
          {showSavedMessages !== null && (() => {
            const t = terminals.find((x) => x.id === showSavedMessages);
            if (!t) return null;
            const msgs = isWorktree && rootProject ? rootProject.savedMessages : project.savedMessages;
            return (
              <div data-saved-popup style={{
                position: 'fixed',
                top: 100,
                left: Math.max(8, panelRight + 8),
                zIndex: 20,
                background: SURFACE_BG,
                border: `1px solid ${BORDER_COLOR}`,
                borderRadius: 4,
                padding: '4px 0',
                width: 200,
                maxHeight: 300,
                overflowY: 'auto',
                boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
              }}>
                {msgs.length === 0 && (
                  <div style={{ padding: '4px 8px', fontSize: 11, color: TEXT_MUTED }}>No saved messages</div>
                )}
                {msgs.map((msg, i) => (
                  <button
                    key={i}
                    onClick={() => {
                      sendSavedMessage(t.projectId, msg, TerminalKindEnum.Background);
                      setShowSavedMessages(null);
                    }}
                    style={{
                      display: 'block',
                      width: '100%',
                      textAlign: 'left',
                      padding: '4px 8px',
                      fontSize: 11,
                      background: 'transparent',
                      border: 'none',
                      color: '#ccc',
                      cursor: 'pointer',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                    data-tooltip={cappedHoverText(msg)}
                  >
                    {msg}
                  </button>
                ))}
              </div>
            );
          })()}

          {/* OpenCode ACP row - only in Foreground filter */}
          {hasActiveAcpChat && filter === TerminalManagerFilterEnum.Foreground && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                height: CONTROL_ROW_HEIGHT,
                padding: isWorktree ? '0 8px 0 36px' : '0 8px 0 24px',
                cursor: 'pointer',
                borderRadius: 8,
                margin: '1px 4px',
                position: 'relative',
              }}
              onClick={() => onActivateAcpChat?.(project.id)}
              onMouseEnter={() => setHoveredAcpRow(true)}
              onMouseLeave={() => setHoveredAcpRow(false)}
            >
              {(activeAcpRow || hoveredAcpRow) && (
                <div style={{
                  position: 'absolute',
                  inset: 1,
                  borderRadius: 8,
                  background: activeAcpRowChrome.fill,
                  border: activeAcpRowChrome.stroke,
                  pointerEvents: 'none',
                }} />
              )}
              <span
                className={`terminal-ai-badge terminal-ai-badge--${activeAcpBadge.kind}`}
                style={terminalManagerBadgeStyle(activeAcpBadge)}
                data-tooltip={`${acpLabelForTool(activeAcpSession?.tool)}: ${acpStatusText(activeAcpStatus)}`}
              />
              <span style={{
                fontSize: 11,
                color: activeAcpStatus === 'error' ? BTN_RED : activeAcpRowChrome.titleColor,
                flex: 1,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
                marginLeft: 6,
                zIndex: 1,
              }}>
                {acpTerminalManagerRowLabel(activeAcpSession)}
              </span>
              <div style={{ display: 'flex', gap: 2, zIndex: 1, minWidth: CONTROL_ROW_HEIGHT }}>
                <div style={{ display: 'flex', gap: 2, visibility: (hoveredAcpRow || activeAcpRow) ? 'visible' : 'hidden' }}>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemoveAcpChat?.(project.id);
                    }}
                    style={{
                      padding: '1px 4px',
                      fontSize: 10,
                      background: 'transparent',
                      border: '1px solid #444',
                      color: BTN_RED,
                      borderRadius: 3,
                      cursor: 'pointer',
                      flexShrink: 0,
                      width: CONTROL_ROW_HEIGHT,
                      height: CONTROL_ROW_HEIGHT - 4,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                    }}
                    data-tooltip={`Close ${acpLabelForTool(activeAcpSession?.tool)}`}
                  >
                    ✕
                  </button>
                </div>
              </div>
            </div>
          )}
          {/* OpenCode ACP button removed */}

          {/* Terminal rows */}
          {terminals.map((t) => {
            const active = t.id === activeTerminalId;
            const hovered = hoveredRow === t.id;
            const rowChrome = terminalManagerRowChrome(active, hovered);
            const displayAiStatus = effectiveAiStatusForDisplay(t.aiTool, t.aiStatus, t.claudeLaunchPending);
            return (
              <div
                key={t.id}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  height: CONTROL_ROW_HEIGHT,
                  padding: isWorktree ? '0 8px 0 36px' : '0 8px 0 24px',
                  cursor: 'pointer',
                  borderRadius: 8,
                  margin: '1px 4px',
                  position: 'relative',
                }}
                onClick={() => onActivate(t.id)}
                onMouseEnter={() => setHoveredRow(t.id)}
                onMouseLeave={() => setHoveredRow(null)}
              >
                {/* Row background */}
                {(active || hovered) && (
                  <div style={{
                    position: 'absolute',
                    inset: 1,
                    borderRadius: 8,
                    background: rowChrome.fill,
                    border: rowChrome.stroke,
                    pointerEvents: 'none',
                  }} />
                )}
                {/* AI status badge */}
                <span
                  className={`terminal-ai-badge terminal-ai-badge--${displayAiStatus === 'running' ? 'spinner' : displayAiStatus === 'attention' ? 'pulse' : 'solid'}`}
                  style={
                    displayAiStatus === 'running'
                      ? { borderColor: TERMINAL_AI_BADGE_RUNNING_BORDER, borderTopColor: TERMINAL_AI_BADGE_RUNNING }
                      : displayAiStatus === 'attention'
                        ? { background: TERMINAL_AI_BADGE_ATTENTION }
                        : { background: TERMINAL_AI_BADGE_INACTIVE }
                  }
                />
                <span style={{
                  fontSize: 11,
                  color: t.exited ? withAlpha(TEXT_MUTED, 160) : rowChrome.titleColor,
                  flex: 1,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  marginLeft: 6,
                  zIndex: 1,
                }}>
                  {t.title || `${t.shell} #${t.id}`}
                </span>

                {/* Action buttons — always reserve space, visible only on hover/active */}
                {(() => {
                  const actionVisible = hovered || active;
                  const actionMinWidth = (() => {
                    let w = 0;
                    let count = 0;
                    w += CONTROL_ROW_HEIGHT; count += 1; // kill
                    if (t.kind === 'background') {
                      w += CONTROL_ROW_HEIGHT; count += 1; // rerun
                      w += TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH; count += 1; // saved messages
                    } else {
                      if (t.recentInputs.length > 0) { w += CONTROL_ROW_HEIGHT; count += 1; }
                      w += TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH; count += 1; // tasks
                    }
                    return w + (count - 1) * 2;
                  })();
                  return (
                    <div style={{ display: 'flex', gap: 2, zIndex: 1, minWidth: actionMinWidth }}>
                      {/* Saved messages button — always visible for background terminals */}
                      {t.kind === 'background' && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setShowSavedMessages(showSavedMessages === t.id ? null : t.id);
                          }}
                          style={{
                            padding: '1px 4px',
                            fontSize: 10,
                            background: 'transparent',
                            border: '1px solid #444',
                            color: effectiveSavedMessages.length > 0 ? TEXT_MUTED : '#555',
                            borderRadius: 3,
                            cursor: 'pointer',
                            flexShrink: 0,
                            width: TERMINAL_MANAGER_MESSAGE_BUTTON_WIDTH,
                            height: CONTROL_ROW_HEIGHT - 4,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            opacity: effectiveSavedMessages.length > 0 ? 1 : 0.5,
                          }}
                          data-tooltip={effectiveSavedMessages.length > 0 ? 'Send saved message' : 'No saved messages'}
                        >
                          💬
                        </button>
                      )}
                      {/* Remaining action buttons — visible only on hover/active */}
                      <div style={{ display: 'flex', gap: 2, visibility: actionVisible ? 'visible' : 'hidden' }}>
                      {t.kind === 'background' && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            rerunBackground(t.id);
                          }}
                          style={{
                            padding: '1px 4px',
                            fontSize: 10,
                            background: 'transparent',
                            border: '1px solid #444',
                            color: TEXT_MUTED,
                            borderRadius: 3,
                            cursor: 'pointer',
                            flexShrink: 0,
                            width: CONTROL_ROW_HEIGHT,
                            height: CONTROL_ROW_HEIGHT - 4,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                          }}
                          data-tooltip={displayAiStatus === 'running' ? 'Interrupt' : 'Rerun'}
                        >
                          {displayAiStatus === 'running' ? '✕' : '↻'}
                        </button>
                      )}
                      {t.kind === 'foreground' && t.recentInputs.length > 0 && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setHistoryPopupTerminalId(t.id);
                            setHistoryPopupJustOpened(true);
                          }}
                          style={{
                            padding: '1px 4px',
                            fontSize: 10,
                            background: 'transparent',
                            border: '1px solid #444',
                            color: TEXT_MUTED,
                            borderRadius: 3,
                            cursor: 'pointer',
                            flexShrink: 0,
                            width: CONTROL_ROW_HEIGHT,
                            height: CONTROL_ROW_HEIGHT - 4,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                          }}
                          data-tooltip="Show input history"
                        >
                          🕒
                        </button>
                      )}
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onKill(t.id);
                        }}
                        style={{
                          padding: '1px 4px',
                          fontSize: 10,
                          background: 'transparent',
                          border: '1px solid #444',
                          color: BTN_RED,
                          borderRadius: 3,
                          cursor: 'pointer',
                          flexShrink: 0,
                          width: CONTROL_ROW_HEIGHT,
                          height: CONTROL_ROW_HEIGHT - 4,
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                        }}
                        data-tooltip="Kill"
                      >
                        ✕
                      </button>
                      </div>
                    </div>
                  );
                })()}
              </div>
            );
          })}
          {terminals.length === 0 && !hasActiveAcpChat && (
            <div style={{ padding: '2px 8px 2px 24px', fontSize: 11, color: withAlpha(TEXT_MUTED, 120) }}>No terminals</div>
          )}
        </div>
      )}

      {/* Input History Popup - positioned to the right of the panel */}
      {historyPopupTerminalId !== null && (() => {
        const t = allTerminals.find((x) => x.id === historyPopupTerminalId);
        if (!t) return null;
        const popupWidth = Math.min(TERMINAL_HOVER_WIDTH, Math.max(200, window.innerWidth - panelRight - 16));
        const recentInputs = t.recentInputs;
        const deduplicated = recentInputs.filter((item, idx, arr) => arr.indexOf(item) === idx);
        return (
          <div
            data-history-popup
            style={{
              position: 'fixed',
              top: 100,
              left: Math.max(8, panelRight + 8),
              width: popupWidth,
              maxHeight: TERMINAL_HISTORY_POPUP_MAX_HEIGHT,
              background: SURFACE_BG,
              border: `1px solid ${BORDER_COLOR}`,
              borderRadius: 8,
              zIndex: 1000,
              overflow: 'hidden',
              display: 'flex',
              flexDirection: 'column',
              boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px', borderBottom: `1px solid ${BORDER_COLOR}` }}>
              <span style={{ fontSize: 14, fontWeight: 600, color: TEXT_PRIMARY }}>Input History</span>
              <button
                onClick={() => setHistoryPopupTerminalId(null)}
                style={{ background: 'transparent', border: 'none', color: TEXT_MUTED, cursor: 'pointer', fontSize: 14 }}
              >
                ✕
              </button>
            </div>
            <div style={{ overflow: 'auto', padding: '8px 16px' }}>
              {deduplicated.length === 0 && (
                <div style={{ fontSize: 12, color: TEXT_MUTED, padding: '8px 0' }}>No recent inputs.</div>
              )}
              {deduplicated.slice(0, TERMINAL_HISTORY_POPUP_MAX_VISIBLE_ENTRIES).map((input, i) => (
                <div
                  key={i}
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 6,
                    padding: '4px 0',
                    borderBottom: i < deduplicated.length - 1 ? `1px solid ${BORDER_COLOR}` : 'none',
                  }}
                >
                  <span style={{ fontSize: 10, color: withAlpha(TEXT_MUTED, 120), flexShrink: 0, marginTop: 2 }}>{i + 1}.</span>
                  <div style={{ flex: 1, overflow: 'auto', maxHeight: TERMINAL_HISTORY_MESSAGE_MAX_HEIGHT }}>
                    <pre style={{ margin: 0, fontSize: 11, color: '#ccc', fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, "Courier New", monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                      {input}
                    </pre>
                  </div>
                </div>
              ))}
            </div>
          </div>
        );
      })()}
    </div>
  );
};

interface IconButtonProps {
  icon: string;
  tooltip: string;
  onClick: (e: React.MouseEvent) => void;
}

const IconButton: React.FC<IconButtonProps> = ({ icon, tooltip, onClick }) => {
  const [hovered, setHovered] = useState(false);
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      data-tooltip={tooltip}
      style={{
        width: CONTROL_ROW_HEIGHT,
        height: CONTROL_ROW_HEIGHT,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'transparent',
        border: 'none',
        borderRadius: 8,
        cursor: 'pointer',
        color: hovered ? TEXT_PRIMARY : withAlpha(TEXT_PRIMARY, 178),
        fontSize: 14,
        ...(hovered ? { background: withAlpha(BTN_ICON_HOVER, 110) } : {}),
      }}
    >
      {icon}
    </button>
  );
};

const ProjectFolderIcon: React.FC<{ expanded: boolean; color: string }> = ({ expanded, color }) => (
  <svg
    aria-hidden="true"
    focusable="false"
    viewBox="0 0 16 16"
    style={{
      width: 13,
      height: 13,
      flex: '0 0 13px',
      color,
      zIndex: 1,
    }}
  >
    {expanded ? (
      <path
        d="M1.5 5.5h4.2l1.1 1.2h7.5l-1.3 6.1H2.8L1.5 5.5Zm.8-2.3h4.1l1.1 1.2h6.2v1.1H6.4L5.3 4.3h-3v.9h-1V4c0-.4.4-.8 1-.8Z"
        fill="currentColor"
      />
    ) : (
      <path
        d="M2.2 3.4h4.2l1.1 1.2h6.3c.5 0 .9.4.9.9v6.2c0 .5-.4.9-.9.9H2.2c-.5 0-.9-.4-.9-.9V4.3c0-.5.4-.9.9-.9Zm-.1 2.1v6.1h11.8V5.5H6.9L5.8 4.3H2.2c-.1 0-.1 0-.1.1v1.1Z"
        fill="currentColor"
      />
    )}
  </svg>
);

interface RowChrome {
  fill: string;
  stroke: string;
  titleColor: string;
}

function acpTerminalManagerRowBadgeVisual(
  status: AcpChatSession['status'] | undefined,
  attentionReason?: AcpTerminalManagerAttentionReason,
): AcpTerminalManagerBadgeVisual {
  switch (status) {
    case 'starting':
    case 'connected':
    case 'session_created':
    case 'running':
      return { kind: 'spinner', color: TERMINAL_AI_BADGE_RUNNING };
    case 'permission':
      return { kind: 'pulse', color: TERMINAL_AI_BADGE_ATTENTION };
    case 'idle':
      return attentionReason === 'turn_complete'
        ? { kind: 'pulse', color: TERMINAL_AI_BADGE_ATTENTION }
        : { kind: 'solid', color: TERMINAL_AI_BADGE_INACTIVE };
    case 'error':
      return { kind: 'solid', color: BTN_RED };
    default:
      return { kind: 'solid', color: TERMINAL_AI_BADGE_INACTIVE };
  }
}

function terminalManagerBadgeStyle(badge: AcpTerminalManagerBadgeVisual): React.CSSProperties {
  return badge.kind === 'spinner'
    ? { borderColor: TERMINAL_AI_BADGE_RUNNING_BORDER, borderTopColor: badge.color }
    : { background: badge.color };
}

function terminalManagerRowChrome(isActive: boolean, isHovered: boolean): RowChrome {
  if (isActive) {
    return {
      fill: '#262626',
      stroke: `1px solid ${withAlpha('#646464', 220)}`,
      titleColor: TEXT_PRIMARY,
    };
  } else if (isHovered) {
    return {
      fill: withAlpha(SURFACE_BG_SOFT, 180),
      stroke: `1px solid ${withAlpha(BORDER_COLOR, 210)}`,
      titleColor: withAlpha(TEXT_PRIMARY, 230),
    };
  } else {
    return {
      fill: 'transparent',
      stroke: 'none',
      titleColor: withAlpha(TEXT_PRIMARY, 210),
    };
  }
}
