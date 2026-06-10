import React, { useCallback, useEffect, useRef, useState } from 'react';
import type { AppConfig, AppHistory, InputHistoryFilter } from '../../../shared/types';
import { InputHistoryFilter as InputHistoryFilterEnum, InputHistoryFilterLabel, TerminalKindLabel } from '../../../shared/types';
import { collectInputHistoryEntries, formatHistoryRelativeTime } from '../lib/inputHistory';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface InputHistoryProps {
  config: AppConfig;
  history: AppHistory;
  selectedProjectId: number | null;
  onUpdateFilter: (filter: InputHistoryFilter) => void;
}

export const InputHistory: React.FC<InputHistoryProps> = ({
  config,
  history,
  selectedProjectId,
  onUpdateFilter,
}) => {
  const [historyProjectId, setHistoryProjectId] = useState<number | null>(
    selectedProjectId ?? config.ui.lastSelectedProjectId ?? config.projects[0]?.id ?? null,
  );
  const [searchQuery, setSearchQuery] = useState('');
  const [copyFeedback, setCopyFeedback] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; text: string } | null>(null);
  const copyFeedbackTimerRef = useRef<number | null>(null);
  const filter = config.ui.inputHistoryFilter;

  useEffect(() => {
    if (historyProjectId !== null && config.projects.some((project) => project.id === historyProjectId)) return;
    setHistoryProjectId(selectedProjectId ?? config.projects[0]?.id ?? null);
  }, [config.projects, historyProjectId, selectedProjectId]);

  useEffect(() => () => {
    if (copyFeedbackTimerRef.current !== null) {
      window.clearTimeout(copyFeedbackTimerRef.current);
    }
  }, []);

  useEffect(() => {
    if (!contextMenu) return undefined;

    const closeMenu = () => setContextMenu(null);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('click', closeMenu);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('click', closeMenu);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [contextMenu]);

  const copyHistoryText = useCallback((text: string) => {
    api.invoke('clipboard:writeText', text).catch(() => {});
    setCopyFeedback('Copied to clipboard');
    if (copyFeedbackTimerRef.current !== null) {
      window.clearTimeout(copyFeedbackTimerRef.current);
    }
    copyFeedbackTimerRef.current = window.setTimeout(() => {
      setCopyFeedback(null);
      copyFeedbackTimerRef.current = null;
    }, 1600);
  }, []);

  const selectedProject = historyProjectId === null
    ? undefined
    : config.projects.find((project) => project.id === historyProjectId);
  const result = collectInputHistoryEntries(
    history,
    config.projects,
    historyProjectId,
    filter,
    searchQuery,
  );

  return (
    <div className="input-history" style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', position: 'relative' }}>
      <div style={{ padding: '8px 8px 6px', borderBottom: '1px solid #222', display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div>
          <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Project</div>
          <select
            value={historyProjectId ?? ''}
            onChange={(e) => setHistoryProjectId(e.target.value ? Number(e.target.value) : null)}
            style={{
              width: '100%',
              height: 28,
              background: '#181818',
              border: '1px solid #333',
              color: '#ccc',
              borderRadius: 4,
              fontSize: 12,
              padding: '0 8px',
            }}
          >
            {config.projects.length === 0 && <option value="">No project selected</option>}
            {config.projects.map((project) => (
              <option key={project.id} value={project.id}>{project.name}</option>
            ))}
          </select>
        </div>

        <div className="input-history-filter-tabs">
          {([InputHistoryFilterEnum.All, InputHistoryFilterEnum.Foreground, InputHistoryFilterEnum.Background] as InputHistoryFilter[]).map((candidate) => {
            const selected = filter === candidate;
            return (
              <button
                key={candidate}
                onClick={() => onUpdateFilter(candidate)}
                className={`input-history-filter-tab ${selected ? 'selected' : ''}`}
              >
                {InputHistoryFilterLabel[candidate]}
              </button>
            );
          })}
        </div>

        <input
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search history..."
          style={{
            height: 28,
            background: '#0c0c0c',
            border: '1px solid #333',
            color: '#ccc',
            borderRadius: 4,
            fontSize: 12,
            padding: '0 8px',
            outline: 'none',
          }}
        />
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: '6px 0' }}>
        {historyProjectId === null || !selectedProject ? (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Select a project to view history.</div>
        ) : result.entries.length === 0 ? (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>
            {searchQuery.trim() ? 'No matching entries.' : 'No history entries for this project yet.'}
          </div>
        ) : (
          <>
            <div style={{ padding: '0 8px 4px', color: '#888', fontSize: 11 }}>
              {result.totalMatching} {result.totalMatching === 1 ? 'entry' : 'entries'}
            </div>
            {result.entries.map((entry, i) => (
              <div
                key={`${entry.recordedAt}-${i}-${entry.text}`}
                className="input-history-row"
                onClick={() => {
                  copyHistoryText(entry.text);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  setContextMenu({ x: event.clientX, y: event.clientY, text: entry.text });
                }}
                title={`${entry.projectName} - ${TerminalKindLabel[entry.kind]}`}
              >
                <span style={{ fontSize: 11, color: entry.kind === 'foreground' ? '#aaaaaa' : '#8a8a8a', flexShrink: 0, width: 22 }}>
                  {entry.kind === 'foreground' ? 'FG' : 'BG'}
                </span>
                <span style={{ fontSize: 13, color: '#f4f4f4', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {entry.text}
                </span>
                <span style={{ fontSize: 11, color: '#8a8a8a', flexShrink: 0 }}>
                  {formatHistoryRelativeTime(entry.recordedAt)}
                </span>
              </div>
            ))}
          </>
        )}
      </div>

      {copyFeedback && (
        <div className="input-history-copy-toast" role="status">
          {copyFeedback}
        </div>
      )}

      {contextMenu && (
        <div
          className="input-history-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            onClick={() => {
              copyHistoryText(contextMenu.text);
              setContextMenu(null);
            }}
          >
            Copy
          </button>
        </div>
      )}
    </div>
  );
};
