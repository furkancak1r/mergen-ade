import React, { useState, useCallback } from 'react';
import type { AppConfig, TerminalKind, TerminalManagerFilter, ProjectRecord } from '../../../shared/types';
import { TerminalKind as TerminalKindEnum, TerminalManagerFilter as TerminalManagerFilterEnum } from '../../../shared/types';
import type { TerminalInstance } from '../hooks/usePty';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: unknown[]) => void) => () => void } }).mergenApi;

interface TerminalManagerProps {
  config: AppConfig;
  terminals: TerminalInstance[];
  activeTerminalId: number | null;
  onActivateTerminal: (id: number) => void;
  onSpawnTerminal: (projectId: number, kind: TerminalKind) => void;
  onKillTerminal: (id: number) => void;
}

export const TerminalManager: React.FC<TerminalManagerProps> = ({
  config,
  terminals,
  activeTerminalId,
  onActivateTerminal,
  onSpawnTerminal,
  onKillTerminal,
}) => {
  const [filter, setFilter] = useState<TerminalManagerFilter>(config.ui.terminalManagerFilter ?? TerminalManagerFilterEnum.Foreground);
  const [expandedProjects, setExpandedProjects] = useState<Set<number>>(new Set(config.projects.map((p) => p.id)));
  const [showSavedMessages, setShowSavedMessages] = useState<number | null>(null);
  const [showFgMessages, setShowFgMessages] = useState<number | null>(null);

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
    const projectTerminals = terminals.filter((t) => t.projectId === projectId && t.kind === kind);
    const target = projectTerminals[0];
    if (!target) return;
    await api.invoke('pty:write', target.id, message + '\r');
  }, [terminals]);

  const rerunBackground = useCallback(async (terminalId: number) => {
    const t = terminals.find((x) => x.id === terminalId);
    if (!t) return;
    if (t.aiStatus === 'running') {
      await api.invoke('pty:write', terminalId, '\x03');
    } else {
      const cmd = t.recentInputs[0];
      if (cmd) {
        await api.invoke('pty:write', terminalId, cmd + '\r');
      }
    }
  }, [terminals]);

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
        {config.projects.map((project) => (
          <ProjectGroup
            key={project.id}
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
            sendSavedMessage={sendSavedMessage}
            rerunBackground={rerunBackground}
          />
        ))}
        {config.projects.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No projects. Add a project in Settings.</div>
        )}
      </div>
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
  onSpawn: (projectId: number, kind: TerminalKind) => void;
  onKill: (id: number) => void;
  showSavedMessages: number | null;
  setShowSavedMessages: (id: number | null) => void;
  showFgMessages: number | null;
  setShowFgMessages: (id: number | null) => void;
  sendSavedMessage: (projectId: number, message: string, kind: TerminalKind) => void;
  rerunBackground: (terminalId: number) => void;
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
  sendSavedMessage,
  rerunBackground,
}) => {
  const launchers = project.aiConfig;
  const hasSavedMessages = project.savedMessages.length > 0;
  const hasFgMessages = project.foregroundSavedMessages.length > 0;

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
        <span style={{ fontSize: 12, fontWeight: 600, color: '#ccc', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {project.name}
        </span>
        <span style={{ fontSize: 10, color: '#666' }}>{terminals.length}</span>
      </div>
      {expanded && (
        <div>
          <div style={{ display: 'flex', gap: 4, padding: '2px 8px 6px' }}>
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
            {hasSavedMessages && filter !== 'foreground' && (
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
              {project.savedMessages.map((msg, i) => (
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
                    title={msg}
                  >
                    {msg}
                  </button>
                </div>
              ))}
            </div>
          )}

          {showFgMessages === project.id && (
            <div style={{ padding: '0 8px 4px' }}>
              {project.foregroundSavedMessages.length === 0 && (
                <div style={{ fontSize: 11, color: '#888', padding: '2px 0' }}>No tasks in queue</div>
              )}
              {project.foregroundSavedMessages.map((msg, i) => (
                <div key={i} style={{ display: 'flex', gap: 4, marginBottom: 2 }}>
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
                    title={msg}
                  >
                    {msg}
                  </button>
                </div>
              ))}
            </div>
          )}

          {terminals.map((t) => (
            <div
              key={t.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 4,
                padding: '3px 8px 3px 24px',
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
        </div>
      )}
    </div>
  );
};
