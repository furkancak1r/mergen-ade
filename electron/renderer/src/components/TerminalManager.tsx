import React, { useState, useCallback } from 'react';
import type { AppConfig, TerminalKind, TerminalManagerFilter, ProjectRecord, LauncherEntry } from '../../../shared/types';
import { TerminalKind as TerminalKindEnum, TerminalManagerFilter as TerminalManagerFilterEnum, BuiltinLauncherKind, activeBuildModel } from '../../../shared/types';
import type { TerminalInstance } from '../hooks/usePty';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: unknown[]) => void) => () => void } }).mergenApi;

function cappedTooltip(text: string, maxChars: number = 100): string {
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
  onKillTerminal: (id: number) => void;
  rerunBackground: (terminalId: number) => void;
  sendSavedMessageToTerminal?: (terminalId: number, message: string, recordRecentInput: boolean) => void;
  onRemoveForegroundMessage?: (projectId: number, message: string) => void;
  onAddForegroundMessage?: (projectId: number, message: string) => void;
  onUpdateForegroundMessage?: (projectId: number, index: number, message: string) => void;
  activeAcpChatByProject?: Map<number, string>;
  onActivateAcpChat?: (projectId: number) => void;
  onRemoveAcpChat?: (projectId: number) => void;
  onOpenAcpChat?: (projectId: number) => void;
}

export const TerminalManager: React.FC<TerminalManagerProps> = ({
  config,
  terminals,
  activeTerminalId,
  onActivateTerminal,
  onSpawnTerminal,
  onKillTerminal,
  rerunBackground,
  sendSavedMessageToTerminal,
  onRemoveForegroundMessage,
  onAddForegroundMessage,
  onUpdateForegroundMessage,
  activeAcpChatByProject,
  onActivateAcpChat,
  onRemoveAcpChat,
  onOpenAcpChat,
}) => {
  const [filter, setFilter] = useState<TerminalManagerFilter>(config.ui.terminalManagerFilter ?? TerminalManagerFilterEnum.Foreground);
  const [expandedProjects, setExpandedProjects] = useState<Set<number>>(new Set(config.projects.map((p) => p.id)));
  const [showSavedMessages, setShowSavedMessages] = useState<number | null>(null);
  const [showFgMessages, setShowFgMessages] = useState<number | null>(null);
  const [showLauncherMenu, setShowLauncherMenu] = useState<number | null>(null);
  const [fgMessagePopupProject, setFgMessagePopupProject] = useState<number | null>(null);
  const [fgMessagePopupText, setFgMessagePopupText] = useState('');
  const [fgMessageEditIndex, setFgMessageEditIndex] = useState<number | null>(null);
  const [historyPopupTerminalId, setHistoryPopupTerminalId] = useState<number | null>(null);

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

  const filteredTerminals = terminals.filter((t) => {
    if (filter === TerminalManagerFilterEnum.Foreground) return t.kind === TerminalKindEnum.Foreground;
    if (filter === TerminalManagerFilterEnum.Background) return t.kind === TerminalKindEnum.Background;
    return true;
  });

  const getProjectTerminals = (projectId: number) => filteredTerminals.filter((t) => t.projectId === projectId);

  const sendSavedMessage = useCallback(async (projectId: number, message: string, kind: TerminalKind) => {
    let target: TerminalInstance | undefined;
    if (kind === TerminalKindEnum.Foreground) {
      // Target the active foreground terminal for this project
      target = terminals.find((t) => t.projectId === projectId && t.kind === kind && t.id === activeTerminalId);
      if (!target) {
        // Fallback to first foreground terminal
        target = terminals.find((t) => t.projectId === projectId && t.kind === kind);
      }
    } else {
      target = terminals.find((t) => t.projectId === projectId && t.kind === kind);
    }
    if (!target) return;
    // Use the centralized saved message delivery which handles bracketed paste,
    // recent input tracking, and correct confirmation Enter scheduling.
    sendSavedMessageToTerminal?.(target.id, message, kind === TerminalKindEnum.Background);
    // Foreground saved messages are send-and-remove
    if (kind === TerminalKindEnum.Foreground) {
      onRemoveForegroundMessage?.(projectId, message);
    }
  }, [terminals, activeTerminalId, sendSavedMessageToTerminal, onRemoveForegroundMessage]);

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

  return (
    <div className="terminal-manager" style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ display: 'flex', gap: 2, padding: '6px 8px', borderBottom: '1px solid #222' }}>
        {([TerminalManagerFilterEnum.Foreground, TerminalManagerFilterEnum.Background] as TerminalManagerFilter[]).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            style={{
              flex: 1,
              padding: '4px 8px',
              fontSize: 11,
              background: filter === f ? '#1f3a4c' : 'transparent',
              color: filter === f ? '#7ec0ee' : '#888',
              border: '1px solid ' + (filter === f ? '#1f3a4c' : '#333'),
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            {f === TerminalManagerFilterEnum.Foreground ? 'FG' : 'BG'}
          </button>
        ))}
        <button
          onClick={() => setFilter(TerminalManagerFilterEnum.Foreground)}
          style={{
            flex: 1,
            padding: '4px 8px',
            fontSize: 11,
            background: filter !== TerminalManagerFilterEnum.Foreground && filter !== TerminalManagerFilterEnum.Background ? '#1f3a4c' : 'transparent',
            color: filter !== TerminalManagerFilterEnum.Foreground && filter !== TerminalManagerFilterEnum.Background ? '#7ec0ee' : '#888',
            border: '1px solid ' + (filter !== TerminalManagerFilterEnum.Foreground && filter !== TerminalManagerFilterEnum.Background ? '#1f3a4c' : '#333'),
            borderRadius: 4,
            cursor: 'pointer',
          }}
        >
          All
        </button>
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {rootProjects.map((project) => (
          <div key={project.id}>
            <ProjectGroup
              project={project}
              terminals={getProjectTerminals(project.id)}
              filter={filter}
              expanded={expandedProjects.has(project.id)}
              onToggle={() => toggleProject(project.id)}
              activeTerminalId={activeTerminalId}
              onActivate={onActivateTerminal}
              onSpawn={onSpawnTerminal}
              onKill={onKillTerminal}
              showSavedMessages={showSavedMessages}
              setShowSavedMessages={setShowSavedMessages}
              showFgMessages={showFgMessages}
              setShowFgMessages={setShowFgMessages}
              showLauncherMenu={showLauncherMenu}
              setShowLauncherMenu={setShowLauncherMenu}
              sendSavedMessage={sendSavedMessage}
              rerunBackground={rerunBackground}
              config={config}
              allTerminals={terminals}
              rootProject={project}
              activeAcpChatByProject={activeAcpChatByProject}
              onActivateAcpChat={onActivateAcpChat}
              onRemoveAcpChat={onRemoveAcpChat}
              onOpenAcpChat={onOpenAcpChat}
              onRemoveForegroundMessage={onRemoveForegroundMessage}
              onOpenFgMessagePopup={(projectId, message, index) => {
                setFgMessagePopupProject(projectId);
                setFgMessagePopupText(message);
                setFgMessageEditIndex(index);
              }}
              onOpenAddFgMessagePopup={(projectId) => {
                setFgMessagePopupProject(projectId);
                setFgMessagePopupText('');
                setFgMessageEditIndex(null);
              }}
            />
            {/* Worktrees under this root */}
            {(worktreesByRoot.get(project.id) || []).map((worktree) => (
              <div key={worktree.id} style={{ paddingLeft: 12 }}>
                <ProjectGroup
                  project={worktree}
                  terminals={getProjectTerminals(worktree.id)}
                  filter={filter}
                  expanded={expandedProjects.has(worktree.id)}
                  onToggle={() => toggleProject(worktree.id)}
                  activeTerminalId={activeTerminalId}
                  onActivate={onActivateTerminal}
                  onSpawn={onSpawnTerminal}
                  onKill={onKillTerminal}
                  showSavedMessages={showSavedMessages}
                  setShowSavedMessages={setShowSavedMessages}
                  showFgMessages={showFgMessages}
                  setShowFgMessages={setShowFgMessages}
                  showLauncherMenu={showLauncherMenu}
                  setShowLauncherMenu={setShowLauncherMenu}
                  sendSavedMessage={sendSavedMessage}
                  rerunBackground={rerunBackground}
                  config={config}
                  allTerminals={terminals}
                  rootProject={project}
                  isWorktree
                  activeAcpChatByProject={activeAcpChatByProject}
                  onActivateAcpChat={onActivateAcpChat}
                  onRemoveAcpChat={onRemoveAcpChat}
                  onOpenAcpChat={onOpenAcpChat}
                  onRemoveForegroundMessage={onRemoveForegroundMessage}
                  onOpenFgMessagePopup={(projectId, message, index) => {
                    setFgMessagePopupProject(projectId);
                    setFgMessagePopupText(message);
                    setFgMessageEditIndex(index);
                  }}
                  onOpenAddFgMessagePopup={(projectId) => {
                    setFgMessagePopupProject(projectId);
                    setFgMessagePopupText('');
                    setFgMessageEditIndex(null);
                  }}
                />
              </div>
            ))}
          </div>
        ))}
        {config.projects.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No projects. Add a project in Settings.</div>
        )}
      </div>

      {/* Foreground Message Add/Edit Popup */}
      {fgMessagePopupProject !== null && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={(e) => {
            if (e.target === e.currentTarget) {
              setFgMessagePopupProject(null);
              setFgMessagePopupText('');
              setFgMessageEditIndex(null);
            }
          }}
        >
          <div
            style={{
              background: '#141414',
              border: '1px solid #333',
              borderRadius: 8,
              width: 480,
              maxWidth: '90vw',
              padding: '16px',
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span style={{ fontSize: 14, fontWeight: 600, color: '#eee' }}>
                {fgMessageEditIndex !== null ? 'Edit Task' : 'Add Task'}
              </span>
              <button
                onClick={() => {
                  setFgMessagePopupProject(null);
                  setFgMessagePopupText('');
                  setFgMessageEditIndex(null);
                }}
                style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}
              >
                ✕
              </button>
            </div>
            <textarea
              value={fgMessagePopupText}
              onChange={(e) => setFgMessagePopupText(e.target.value)}
              placeholder="Enter task text..."
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter' && e.ctrlKey) {
                  e.preventDefault();
                  setFgMessagePopupText((prev) => prev + '\n');
                }
                if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey) {
                  e.preventDefault();
                  const text = fgMessagePopupText.trim();
                  if (!text) return;
                  if (fgMessageEditIndex !== null) {
                    onUpdateForegroundMessage?.(fgMessagePopupProject, fgMessageEditIndex, text);
                  } else {
                    onAddForegroundMessage?.(fgMessagePopupProject, text);
                  }
                  setFgMessagePopupProject(null);
                  setFgMessagePopupText('');
                  setFgMessageEditIndex(null);
                }
                if (e.key === 'Escape') {
                  setFgMessagePopupProject(null);
                  setFgMessagePopupText('');
                  setFgMessageEditIndex(null);
                }
              }}
              style={{
                width: '100%',
                background: '#1a1a1a',
                border: '1px solid #333',
                color: '#ccc',
                padding: '8px',
                fontSize: 12,
                borderRadius: 4,
                outline: 'none',
                resize: 'none',
                minHeight: 80,
                maxHeight: 160,
              }}
              rows={3}
            />
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
              <button
                onClick={() => {
                  setFgMessagePopupProject(null);
                  setFgMessagePopupText('');
                  setFgMessageEditIndex(null);
                }}
                style={{ padding: '6px 16px', fontSize: 12, background: 'transparent', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  const text = fgMessagePopupText.trim();
                  if (!text) return;
                  if (fgMessageEditIndex !== null) {
                    onUpdateForegroundMessage?.(fgMessagePopupProject, fgMessageEditIndex, text);
                  } else {
                    onAddForegroundMessage?.(fgMessagePopupProject, text);
                  }
                  setFgMessagePopupProject(null);
                  setFgMessagePopupText('');
                  setFgMessageEditIndex(null);
                }}
                disabled={!fgMessagePopupText.trim()}
                style={{
                  padding: '6px 16px',
                  fontSize: 12,
                  background: '#1f3a4c',
                  border: '1px solid #1f3a4c',
                  color: '#ccc',
                  borderRadius: 4,
                  cursor: fgMessagePopupText.trim() ? 'pointer' : 'not-allowed',
                  opacity: fgMessagePopupText.trim() ? 1 : 0.5,
                }}
              >
                {fgMessageEditIndex !== null ? 'Save' : 'Add'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

interface ProjectGroupProps {
  project: ProjectRecord;
  terminals: TerminalInstance[];
  filter: TerminalManagerFilter;
  expanded: boolean;
  onToggle: () => void;
  activeTerminalId: number | null;
  onActivate: (id: number) => void;
  onSpawn: (projectId: number, kind: TerminalKind) => Promise<number>;
  onKill: (id: number) => void;
  showSavedMessages: number | null;
  setShowSavedMessages: (id: number | null) => void;
  showFgMessages: number | null;
  setShowFgMessages: (id: number | null) => void;
  showLauncherMenu: number | null;
  setShowLauncherMenu: (id: number | null) => void;
  sendSavedMessage: (projectId: number, message: string, kind: TerminalKind) => void;
  rerunBackground: (terminalId: number) => void;
  config: AppConfig;
  allTerminals: TerminalInstance[];
  rootProject?: ProjectRecord;
  isWorktree?: boolean;
  activeAcpChatByProject?: Map<number, string>;
  onActivateAcpChat?: (projectId: number) => void;
  onRemoveAcpChat?: (projectId: number) => void;
  onOpenAcpChat?: (projectId: number) => void;
  onRemoveForegroundMessage?: (projectId: number, message: string) => void;
  onOpenFgMessagePopup?: (projectId: number, message: string, index: number) => void;
  onOpenAddFgMessagePopup?: (projectId: number) => void;
}

const ProjectGroup: React.FC<ProjectGroupProps> = ({
  project,
  terminals,
  filter,
  expanded,
  onToggle,
  activeTerminalId,
  onActivate,
  onSpawn,
  onKill,
  showSavedMessages,
  setShowSavedMessages,
  showFgMessages,
  setShowFgMessages,
  showLauncherMenu,
  setShowLauncherMenu,
  sendSavedMessage,
  rerunBackground,
  config,
  allTerminals,
  rootProject,
  isWorktree,
  activeAcpChatByProject,
  onActivateAcpChat,
  onRemoveAcpChat,
  onOpenAcpChat,
  onRemoveForegroundMessage,
  onOpenFgMessagePopup,
  onOpenAddFgMessagePopup,
}) => {
  const [historyPopupTerminalId, setHistoryPopupTerminalId] = useState<number | null>(null);
  // Worktrees inherit saved messages from root project
  const effectiveSavedMessages = isWorktree && rootProject ? rootProject.savedMessages : project.savedMessages;
  const hasSavedMessages = effectiveSavedMessages.length > 0;
  const hasFgMessages = project.foregroundSavedMessages.length > 0;
  const hasLiveTerminals = terminals.length > 0;
  const isSelected = activeTerminalId !== null && terminals.some((t) => t.id === activeTerminalId);

  return (
    <div style={{ marginBottom: 2 }}>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 4,
          padding: '4px 8px',
          cursor: 'pointer',
          borderRadius: 3,
        }}
        onClick={onToggle}
      >
        <span style={{ fontSize: 10, color: '#888', width: 12 }}>{expanded ? '▼' : '▶'}</span>
        <span style={{
          fontSize: 12,
          fontWeight: 600,
          color: isSelected && hasLiveTerminals ? '#eee' : '#ccc',
          flex: 1,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
        }}>
          {project.name}
        </span>
        {isWorktree && (
          <span style={{ fontSize: 10, color: '#888' }}>🌿</span>
        )}
        <span style={{ fontSize: 10, color: '#666' }}>{terminals.length}</span>
      </div>
      {expanded && (
        <div>
          <div style={{ display: 'flex', gap: 4, padding: '2px 8px 6px', position: 'relative' }}>
            <button
              onClick={() => onSpawn(project.id, TerminalKindEnum.Foreground)}
              style={{
                padding: '2px 8px',
                fontSize: 11,
                background: '#1a1a1a',
                border: '1px solid #333',
                color: '#ccc',
                borderRadius: 3,
                cursor: 'pointer',
              }}
            >
              + FG
            </button>
            <button
              onClick={() => onSpawn(project.id, TerminalKindEnum.Background)}
              style={{
                padding: '2px 8px',
                fontSize: 11,
                background: '#1a1a1a',
                border: '1px solid #333',
                color: '#ccc',
                borderRadius: 3,
                cursor: 'pointer',
              }}
            >
              + BG
            </button>
            {filter === TerminalManagerFilterEnum.Foreground && (
              <div style={{ position: 'relative' }}>
                <button
                  onClick={() => setShowLauncherMenu(showLauncherMenu === project.id ? null : project.id)}
                  style={{
                    padding: '2px 8px',
                    fontSize: 11,
                    background: showLauncherMenu === project.id ? '#1f3a4c' : '#1a1a1a',
                    border: '1px solid #333',
                    color: '#ccc',
                    borderRadius: 3,
                    cursor: 'pointer',
                  }}
                >
                  Launch
                </button>
                {showLauncherMenu === project.id && (
                  <div style={{ position: 'absolute', top: '100%', left: 0, zIndex: 10, background: '#1a1a1a', border: '1px solid #333', borderRadius: 4, padding: '4px 0', width: 168, marginTop: 2 }}>
                    {config.launchers.filter((l) => l.enabled).map((l) => (
                      <button
                        key={l.id}
                        onClick={async () => {
                          setShowLauncherMenu(null);
                          const cmd = l.launchCommand;
                          // For OpenCode, generate terminal runtime config before launching
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
                        }}
                        style={{ display: 'block', width: '100%', textAlign: 'left', padding: '4px 8px', fontSize: 11, background: 'transparent', border: 'none', color: '#ccc', cursor: 'pointer' }}
                        title={l.displayName + (l.launchCommand ? ' — ' + l.launchCommand : '')}
                      >
                        {l.displayName}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
            {hasSavedMessages && filter !== TerminalManagerFilterEnum.Foreground && (
              <button
                onClick={() => setShowSavedMessages(showSavedMessages === project.id ? null : project.id)}
                style={{
                  padding: '2px 8px',
                  fontSize: 11,
                  background: showSavedMessages === project.id ? '#1f3a4c' : '#1a1a1a',
                  border: '1px solid #333',
                  color: '#ccc',
                  borderRadius: 3,
                  cursor: 'pointer',
                }}
              >
                Msg
              </button>
            )}
            {hasFgMessages && filter === TerminalManagerFilterEnum.Foreground && (
              <button
                onClick={() => setShowFgMessages(showFgMessages === project.id ? null : project.id)}
                style={{
                  padding: '2px 8px',
                  fontSize: 11,
                  background: showFgMessages === project.id ? '#1f3a4c' : '#1a1a1a',
                  border: '1px solid #333',
                  color: hasFgMessages ? '#64c864' : '#ccc',
                  borderRadius: 3,
                  cursor: 'pointer',
                }}
              >
                Tasks
              </button>
            )}
          </div>

          {showSavedMessages === project.id && (
            <div style={{ padding: '0 8px 4px' }}>
              {effectiveSavedMessages.map((msg, i) => (
                <div key={i} style={{ display: 'flex', gap: 4, marginBottom: 2 }}>
                  <button
                    onClick={() => sendSavedMessage(project.id, msg, TerminalKindEnum.Background)}
                    style={{
                      flex: 1,
                      textAlign: 'left',
                      padding: '3px 6px',
                      fontSize: 11,
                      background: 'transparent',
                      border: '1px solid #333',
                      color: '#aaa',
                      borderRadius: 3,
                      cursor: 'pointer',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                    title={cappedTooltip(msg)}
                  >
                    {msg}
                  </button>
                </div>
              ))}
            </div>
          )}

          {showFgMessages === project.id && (
            <div style={{ padding: '0 8px 4px', maxHeight: 300, overflow: 'auto' }}>
              {project.foregroundSavedMessages.length === 0 && (
                <div style={{ fontSize: 11, color: '#888', padding: '2px 0' }}>No tasks in queue</div>
              )}
              {project.foregroundSavedMessages.map((msg, i) => (
                <div key={i} style={{ display: 'flex', gap: 4, marginBottom: 2, alignItems: 'center' }}>
                  <button
                    onClick={() => {
                      sendSavedMessage(project.id, msg, TerminalKindEnum.Foreground);
                    }}
                    style={{
                      flex: 1,
                      textAlign: 'left',
                      padding: '3px 6px',
                      fontSize: 11,
                      background: 'transparent',
                      border: '1px solid #333',
                      color: '#aaa',
                      borderRadius: 3,
                      cursor: 'pointer',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                    title={cappedTooltip(msg)}
                  >
                    {msg}
                  </button>
                  <button
                    onClick={() => {
                      onOpenFgMessagePopup?.(project.id, msg, i);
                    }}
                    style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                    title="Edit"
                  >
                    ✎
                  </button>
                  <button
                    onClick={() => {
                      onRemoveForegroundMessage?.(project.id, msg);
                    }}
                    style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                    title="Delete"
                  >
                    🗑
                  </button>
                </div>
              ))}
              <button
                onClick={() => {
                  onOpenAddFgMessagePopup?.(project.id);
                }}
                style={{
                  marginTop: 4,
                  padding: '3px 6px',
                  fontSize: 11,
                  background: 'transparent',
                  border: '1px solid #333',
                  color: '#888',
                  borderRadius: 3,
                  cursor: 'pointer',
                  width: '100%',
                  textAlign: 'left',
                }}
              >
                + Add New
              </button>
            </div>
          )}

          {/* ACP Chat row for this project */}
          {activeAcpChatByProject && activeAcpChatByProject.has(project.id) && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 4,
                padding: isWorktree ? '3px 8px 3px 36px' : '3px 8px 3px 24px',
                cursor: 'pointer',
                borderRadius: 3,
                background: 'rgba(0,120,212,0.15)',
              }}
              onClick={() => onActivateAcpChat?.(project.id)}
            >
              <span style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                background: '#7ec0ee',
                flexShrink: 0,
              }} />
              <span style={{ fontSize: 11, color: '#7ec0ee', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                OpenCode Chat
              </span>
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
                  color: '#888',
                  borderRadius: 3,
                  cursor: 'pointer',
                  flexShrink: 0,
                }}
                title="Close Chat"
              >
                ✕
              </button>
            </div>
          )}
          {/* OpenCode Chat button when no active chat but project has foreground terminals */}
          {activeAcpChatByProject && !activeAcpChatByProject.has(project.id) && filter === TerminalManagerFilterEnum.Foreground && terminals.some((t) => t.kind === 'foreground' && t.aiTool === 'opencode') && (
            <div style={{ padding: isWorktree ? '2px 8px 2px 36px' : '2px 8px 2px 24px' }}>
              <button
                onClick={() => onOpenAcpChat?.(project.id)}
                style={{
                  padding: '2px 8px',
                  fontSize: 11,
                  background: '#1a1a1a',
                  border: '1px solid #333',
                  color: '#7ec0ee',
                  borderRadius: 3,
                  cursor: 'pointer',
                }}
              >
                + Chat
              </button>
            </div>
          )}

          {terminals.map((t) => (
            <div
              key={t.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 4,
                padding: isWorktree ? '3px 8px 3px 36px' : '3px 8px 3px 24px',
                cursor: 'pointer',
                borderRadius: 3,
                background: t.id === activeTerminalId ? 'rgba(0,120,212,0.2)' : 'transparent',
              }}
              onClick={() => onActivate(t.id)}
            >
              <span style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                background: t.aiStatus === 'running' ? '#64c864' : t.aiStatus === 'attention' ? '#e8a838' : '#666',
                flexShrink: 0,
              }} />
              <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {t.title || `${t.shell} #${t.id}`}
              </span>
              {t.recentInputs.length > 0 && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setHistoryPopupTerminalId(t.id);
                  }}
                  style={{
                    padding: '1px 4px',
                    fontSize: 10,
                    background: 'transparent',
                    border: '1px solid #444',
                    color: '#888',
                    borderRadius: 3,
                    cursor: 'pointer',
                    flexShrink: 0,
                  }}
                  title="Show input history"
                >
                  🕒
                </button>
              )}
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
                    color: '#888',
                    borderRadius: 3,
                    cursor: 'pointer',
                    flexShrink: 0,
                  }}
                  title={t.aiStatus === 'running' ? 'Interrupt' : 'Rerun'}
                >
                  {t.aiStatus === 'running' ? '✕' : '↻'}
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
                  color: '#888',
                  borderRadius: 3,
                  cursor: 'pointer',
                  flexShrink: 0,
                }}
                title="Kill"
              >
                ✕
              </button>
            </div>
          ))}
          {terminals.length === 0 && (
            <div style={{ padding: '2px 8px 2px 24px', fontSize: 11, color: '#666' }}>No terminals</div>
          )}

          {/* Input History Popup */}
          {historyPopupTerminalId !== null && (() => {
            const t = allTerminals.find((x) => x.id === historyPopupTerminalId);
            if (!t) return null;
            return (
              <div
                style={{
                  position: 'fixed',
                  inset: 0,
                  background: 'rgba(0,0,0,0.6)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  zIndex: 1000,
                }}
                onClick={(e) => {
                  if (e.target === e.currentTarget) setHistoryPopupTerminalId(null);
                }}
              >
                <div
                  style={{
                    background: '#141414',
                    border: '1px solid #333',
                    borderRadius: 8,
                    width: 480,
                    maxWidth: '90vw',
                    maxHeight: 400,
                    display: 'flex',
                    flexDirection: 'column',
                    overflow: 'hidden',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px', borderBottom: '1px solid #222' }}>
                    <span style={{ fontSize: 14, fontWeight: 600, color: '#eee' }}>Input History</span>
                    <button
                      onClick={() => setHistoryPopupTerminalId(null)}
                      style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}
                    >
                      ✕
                    </button>
                  </div>
                  <div style={{ overflow: 'auto', padding: '8px 16px' }}>
                    {t.recentInputs.length === 0 && (
                      <div style={{ fontSize: 12, color: '#888', padding: '8px 0' }}>No recent inputs.</div>
                    )}
                    {t.recentInputs.map((input, i) => (
                      <div
                        key={i}
                        style={{
                          display: 'flex',
                          alignItems: 'flex-start',
                          gap: 6,
                          padding: '4px 0',
                          borderBottom: '1px solid #222',
                        }}
                      >
                        <span style={{ fontSize: 10, color: '#666', flexShrink: 0, marginTop: 2 }}>{i + 1}.</span>
                        <div style={{ flex: 1, overflow: 'auto', maxHeight: 120 }}>
                          <pre style={{ margin: 0, fontSize: 11, color: '#ccc', fontFamily: 'Consolas, "Courier New", monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
                            {input}
                          </pre>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            );
          })()}
        </div>
      )}
    </div>
  );
};
