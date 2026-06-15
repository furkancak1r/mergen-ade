import React, { useState, useMemo, useEffect } from 'react';
import type {
  AcpTimelineChangeSummaryItem,
  AcpTimelineItem,
  AcpTimelineStatusItem,
  AcpTimelineThinkingItem,
  AcpTimelineToolItem,
} from '../../../shared/types';
import {
  acpTimelineStatusTitle,
  acpTimelineNoticeTitle,
  acpTimelineTodoEntries,
  acpTimelineToolDisplayTitle,
  acpTimelineToolKindLabel,
} from '../../../shared/acpTimeline';
import { AcpMarkdownMessage } from './AcpMarkdownMessage';

interface AcpTimelineProps {
  items: AcpTimelineItem[];
  onCopyMessage?: (text: string) => void;
  activeThinkingId?: string;
}

/** A visual group of consecutive timeline items. */
type TimelineGroup =
  | { type: 'item'; item: AcpTimelineItem; index: number }
  | { type: 'tool-group'; items: AcpTimelineToolItem[]; kind: string; indices: number[] };

function groupTimelineItems(items: AcpTimelineItem[]): TimelineGroup[] {
  const groups: TimelineGroup[] = [];
  let toolBuffer: AcpTimelineToolItem[] = [];
  let toolIndices: number[] = [];
  let toolKind = '';

  const flushTools = () => {
    if (toolBuffer.length === 0) return;
    if (toolBuffer.length === 1) {
      groups.push({ type: 'item', item: toolBuffer[0], index: toolIndices[0] });
    } else {
      groups.push({ type: 'tool-group', items: toolBuffer, kind: toolKind, indices: toolIndices });
    }
    toolBuffer = [];
    toolIndices = [];
    toolKind = '';
  };

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
      if (item.type === 'tool' && acpTimelineToolKindLabel(item.kind) !== 'Todo') {
      const kind = acpTimelineToolKindLabel(item.kind);
      if (toolBuffer.length > 0 && kind !== toolKind) {
        flushTools();
      }
      toolBuffer.push(item);
      toolIndices.push(i);
      toolKind = kind;
    } else {
      flushTools();
      groups.push({ type: 'item', item, index: i });
    }
  }
  flushTools();
  return groups;
}

export const AcpTimeline: React.FC<AcpTimelineProps> = ({
  items,
  onCopyMessage,
  activeThinkingId,
}) => {
  const groups = useMemo(() => groupTimelineItems(items), [items]);

  return (
    <div className="acp-timeline">
      {groups.map((group) => {
        if (group.type === 'tool-group') {
          return (
            <AcpToolGroupCard
              key={`group-${group.indices[0]}`}
              items={group.items}
              kind={group.kind}
            />
          );
        }
        const { item } = group;
        return (
          <div key={item.id} className="acp-timeline-row">
            <AcpTimelineItemView
              item={item}
              onCopyMessage={onCopyMessage}
              activeThinking={item.type === 'thinking' && item.id === activeThinkingId}
            />
          </div>
        );
      })}
    </div>
  );
};

const AcpTimelineItemView: React.FC<{
  item: AcpTimelineItem;
  onCopyMessage?: (text: string) => void;
  activeThinking?: boolean;
}> = ({ item, onCopyMessage, activeThinking = false }) => {
  switch (item.type) {
    case 'message':
      return (
        <div className={`acp-message acp-message--${item.role}`}>
          <AcpMarkdownMessage text={item.text} />
          {item.role === 'user' && (
            <button
              className="acp-message-copy-btn"
              onClick={() => onCopyMessage?.(item.text)}
              title="Copy message"
              aria-label="Copy message"
            >
              ⧉
            </button>
          )}
        </div>
      );
    case 'tool':
      if (acpTimelineToolKindLabel(item.kind) === 'Todo') return <AcpTodoCard item={item} />;
      return <AcpToolCardCompact item={item} />;
    case 'permission':
      if (item.status !== 'pending') return <AcpPermissionMarker item={item} />;
      return (
        <div className={`acp-event-card acp-event-card--permission acp-permission-status-${item.status}`}>
          <div className="acp-event-card-header">
            <span className="acp-event-card-kind">{item.interactionKind === 'question' ? 'Question' : 'Permission'}</span>
            <span className="acp-event-card-status">{item.status}</span>
          </div>
          <div className="acp-event-card-title">{item.header}</div>
          {item.question && <div className="acp-event-card-body">{item.question}</div>}
          {item.options.length > 0 && (
            <div className="acp-option-chip-row">
              {item.options.map((option) => (
                <span className="acp-option-chip" key={option.id}>{option.label}</span>
              ))}
            </div>
          )}
        </div>
      );
    case 'notice':
      return (
        <div className={`acp-event-card acp-event-card--${item.kind}`}>
          <div className="acp-event-card-header">
            <span className="acp-event-card-kind">{acpTimelineNoticeTitle(item.kind)}</span>
          </div>
          <pre className="acp-event-card-pre">{item.text}</pre>
        </div>
      );
    case 'thinking':
      return <AcpThinkingMinimal item={item} active={activeThinking} />;
    case 'change_summary':
      return <AcpChangeSummaryCard item={item} />;
    case 'status':
      return <AcpStatusCard item={item} />;
  }
};

const AcpPermissionMarker: React.FC<{ item: Extract<AcpTimelineItem, { type: 'permission' }> }> = ({ item }) => {
  const label = item.interactionKind === 'question'
    ? 'Asked 1 question'
    : item.status === 'rejected'
      ? 'Rejected permission'
      : 'Answered permission';
  return <div className="acp-group-marker acp-permission-marker">{label}</div>;
};

const AcpTodoCard: React.FC<{ item: AcpTimelineToolItem }> = ({ item }) => {
  const [expanded, setExpanded] = useState(true);
  const entries = acpTimelineTodoEntries(item.raw);
  const isRunning = item.status === 'running' || item.status === 'pending';
  const completedCount = entries.filter((entry) => /done|complete/i.test(entry.status || '')).length;
  const inProgressCount = entries.filter((entry) => /progress|doing|active/i.test(entry.status || '')).length;
  const summary = entries.length > 0
    ? `${entries.length} items${completedCount > 0 ? `, ${completedCount} done` : ''}${inProgressCount > 0 ? `, ${inProgressCount} active` : ''}`
    : acpTimelineToolDisplayTitle(item.title, item.kind);

  return (
    <div className={`acp-todo-card ${isRunning ? 'is-running' : ''}`}>
      <button className="acp-todo-header" onClick={() => setExpanded((value) => !value)}>
        <span className={`acp-tool-dot ${isRunning ? 'acp-tool-dot--pulse' : ''}`} />
        <span className="acp-todo-title">To Do</span>
        <span className="acp-todo-summary">{summary}</span>
        <span className="acp-todo-chevron">{expanded ? '▾' : '▸'}</span>
      </button>
      <div className={`acp-todo-body ${expanded ? 'is-expanded' : ''}`}>
        {entries.length > 0 ? (
          <div className="acp-todo-list">
            {entries.map((entry, index) => (
              <div className="acp-todo-item" key={`${entry.text}-${index}`}>
                <span className="acp-todo-item-status">{entry.status || 'todo'}</span>
                <span className="acp-todo-item-text">{entry.text}</span>
                {entry.priority && <span className="acp-todo-item-priority">{entry.priority}</span>}
              </div>
            ))}
          </div>
        ) : (
          <pre className="acp-tool-details">{stringifyRawToolEvent(item.raw) || acpTimelineToolDisplayTitle(item.title, item.kind)}</pre>
        )}
      </div>
    </div>
  );
};

const AcpChangeSummaryCard: React.FC<{ item: AcpTimelineChangeSummaryItem }> = ({ item }) => {
  const [expanded, setExpanded] = useState(false);
  const hiddenCount = Math.max(0, item.totalFiles - item.files.length);
  const totals = `+${item.addedLines} -${item.removedLines}`;

  return (
    <div className="acp-change-summary-card">
      <button className="acp-change-summary-header" onClick={() => setExpanded((value) => !value)}>
        <span className="acp-change-summary-icon">⑂</span>
        <span className="acp-change-summary-title">Changed files</span>
        <span className="acp-change-summary-count">{item.totalFiles}</span>
        <span className="acp-change-summary-totals">{totals}</span>
        <span className="acp-change-summary-chevron">{expanded ? '▾' : '▸'}</span>
      </button>
      <div className={`acp-change-summary-body ${expanded ? 'is-expanded' : ''}`}>
        <div className="acp-change-summary-list">
          {item.files.map((file) => (
            <div className="acp-change-summary-file" key={file.path}>
              <span className="acp-change-summary-status">{file.status}</span>
              <span className="acp-change-summary-path" title={file.path}>{file.path}</span>
              {file.error ? (
                <span className="acp-change-summary-error">{file.error}</span>
              ) : file.binary ? (
                <span className="acp-change-summary-meta">binary</span>
              ) : (
                <span className="acp-change-summary-lines">+{file.addedLines} -{file.removedLines}</span>
              )}
            </div>
          ))}
          {hiddenCount > 0 && <div className="acp-change-summary-more">+{hiddenCount} more files</div>}
        </div>
      </div>
    </div>
  );
};

const AcpStatusCard: React.FC<{ item: AcpTimelineStatusItem }> = ({ item }) => {
  const [expanded, setExpanded] = useState(false);
  const title = item.title || acpTimelineStatusTitle(item.kind);
  return (
    <div className={`acp-status-card acp-status-card--${item.kind}`}>
      <button className="acp-status-header" onClick={() => setExpanded((value) => !value)}>
        <span className="acp-status-title">{title}</span>
        <span className="acp-status-preview">{singleLine(item.text)}</span>
        <span className="acp-status-chevron">{expanded ? '▾' : '▸'}</span>
      </button>
      <div className={`acp-status-body ${expanded ? 'is-expanded' : ''}`}>
        <pre>{item.text}</pre>
      </div>
    </div>
  );
};

/** Compact single-line tool card. */
const AcpToolCardCompact: React.FC<{ item: AcpTimelineToolItem }> = ({ item }) => {
  const [expanded, setExpanded] = useState(false);
  const title = acpTimelineToolDisplayTitle(item.title, item.kind);
  const kind = acpTimelineToolKindLabel(item.kind);
  const rawText = stringifyRawToolEvent(item.raw);
  const shortTitle = truncatePath(title);
  const isRunning = item.status === 'running' || item.status === 'pending';
  const duration = item.updatedAt > item.startedAt ? formatDuration(item.updatedAt - item.startedAt) : null;

  return (
    <div className={`acp-tool-row acp-tool-row--${item.status}`}>
      <span className={`acp-tool-dot ${isRunning ? 'acp-tool-dot--pulse' : ''}`} />
      <span className="acp-tool-kind">{kind}</span>
      <span className="acp-tool-title" title={title}>{shortTitle}</span>
      {duration && <span className="acp-tool-duration">{duration}</span>}
      {rawText && (
        <button className="acp-tool-expand-btn" onClick={() => setExpanded((v) => !v)}>
          {expanded ? '▾' : '▸'}
        </button>
      )}
      {expanded && <pre className="acp-tool-details">{rawText}</pre>}
    </div>
  );
};

/** Grouped consecutive tool calls of the same kind. */
const AcpToolGroupCard: React.FC<{ items: AcpTimelineToolItem[]; kind: string }> = ({ items, kind }) => {
  const [expanded, setExpanded] = useState(false);
  const runningCount = items.filter((i) => i.status === 'running' || i.status === 'pending').length;
  const hasRunning = runningCount > 0;
  const label = acpToolGroupLabel(kind, items.length, runningCount);

  return (
    <div className={`acp-tool-group ${hasRunning ? 'acp-tool-group--running' : ''}`}>
      <button className="acp-tool-group-header" onClick={() => setExpanded((v) => !v)}>
        <span className="acp-group-marker-icon">›</span>
        <span className="acp-group-marker-label">{label}</span>
      </button>
      {expanded && (
        <div className="acp-tool-group-items">
          {items.map((item) => {
            const title = acpTimelineToolDisplayTitle(item.title, item.kind);
            const shortTitle = truncatePath(title);
            const isRunning = item.status === 'running' || item.status === 'pending';
            const duration = item.updatedAt > item.startedAt ? formatDuration(item.updatedAt - item.startedAt) : null;
            return (
              <div key={item.id} className={`acp-tool-group-item acp-tool-row--${item.status}`}>
                <span className={`acp-tool-dot acp-tool-dot--sm ${isRunning ? 'acp-tool-dot--pulse' : ''}`} />
                <span className="acp-tool-title" title={title}>{shortTitle}</span>
                {duration && <span className="acp-tool-duration">{duration}</span>}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

const AcpThinkingMinimal: React.FC<{ item: AcpTimelineThinkingItem; active: boolean }> = ({ item, active }) => {
  const [manualExpanded, setManualExpanded] = useState(false);
  const hasText = item.text.trim().length > 0;
  const expanded = hasText && (active || manualExpanded);

  useEffect(() => {
    if (!active) setManualExpanded(false);
  }, [active, item.id]);

  return (
    <div className={`acp-thinking-minimal ${active ? 'is-active' : ''}`}>
      <button className="acp-thinking-minimal-btn" onClick={() => setManualExpanded((value) => !value)}>
        <span className={`acp-thinking-minimal-dot ${active ? 'is-active' : ''}`} />
        <span className="acp-thinking-minimal-label">Thinking</span>
        {hasText && <span className="acp-thinking-minimal-chevron">{expanded ? '▾' : '▸'}</span>}
      </button>
      {hasText && (
        <div className={`acp-thinking-minimal-content ${expanded ? 'is-expanded' : ''}`}>
          <div className="acp-thinking-minimal-content-inner">
            <AcpMarkdownMessage text={item.text} />
          </div>
        </div>
      )}
    </div>
  );
};

function acpToolGroupLabel(kind: string, count: number, runningCount: number): string {
  const safeCount = Math.max(0, count);
  if (runningCount > 0) return runningCount === 1 ? 'Running 1 command' : `Running ${runningCount} commands`;
  switch (kind) {
    case 'Run':
      return safeCount === 1 ? 'Ran 1 command' : `Ran ${safeCount} commands`;
    case 'Search':
      return safeCount === 1 ? 'Searched once' : `Searched ${safeCount} times`;
    case 'Read':
      return safeCount === 1 ? 'Read 1 file' : `Read ${safeCount} files`;
    case 'Edit':
      return safeCount === 1 ? 'Changed 1 file' : `Changed ${safeCount} files`;
    case 'Diagnostics':
      return safeCount === 1 ? 'Checked diagnostics' : `Checked diagnostics ${safeCount} times`;
    default:
      return safeCount === 1 ? `Used ${kind}` : `Used ${kind} ${safeCount} times`;
  }
}

/** Truncate a file path to fit on one line. Keeps the filename, trims the middle. */
function truncatePath(title: string, maxLen = 60): string {
  if (title.length <= maxLen) return title;
  // If it looks like a path, keep filename
  const lastSlash = Math.max(title.lastIndexOf('/'), title.lastIndexOf('\\'));
  if (lastSlash >= 0 && lastSlash < title.length - 1) {
    const filename = title.slice(lastSlash + 1);
    const prefix = title.slice(0, Math.min(lastSlash, 12));
    const budget = maxLen - prefix.length - filename.length - 3;
    if (budget >= 0) return `${prefix}.../${filename}`;
    // filename alone is too long
    if (filename.length > maxLen - 6) return `...${filename.slice(-(maxLen - 6))}`;
    return `.../${filename}`;
  }
  // Plain text truncation
  return title.slice(0, maxLen - 3) + '...';
}

function stringifyRawToolEvent(raw: unknown): string {
  if (!raw) return '';
  try {
    return JSON.stringify(raw, null, 2);
  } catch {
    return String(raw);
  }
}

function singleLine(text: string, maxLen = 96): string {
  const collapsed = text.split(/\s+/).filter(Boolean).join(' ');
  if (collapsed.length <= maxLen) return collapsed;
  return `${collapsed.slice(0, maxLen - 3)}...`;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
}
