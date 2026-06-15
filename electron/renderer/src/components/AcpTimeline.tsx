import React, { useState } from 'react';
import type { AcpTimelineItem, AcpTimelineThinkingItem, AcpTimelineToolItem } from '../../../shared/types';
import {
  acpTimelineNoticeTitle,
  acpTimelineToolDisplayTitle,
  acpTimelineToolKindLabel,
} from '../../../shared/acpTimeline';
import { AcpMarkdownMessage } from './AcpMarkdownMessage';

interface AcpTimelineProps {
  items: AcpTimelineItem[];
  dragSourceIndex: number | null;
  dragTargetIndex: number | null;
  onDragStart: (index: number) => void;
  onDragOver: (event: React.DragEvent, index: number) => void;
  onDrop: (index: number) => void;
  onDragEnd: () => void;
}

export const AcpTimeline: React.FC<AcpTimelineProps> = ({
  items,
  dragSourceIndex,
  dragTargetIndex,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
}) => {
  return (
    <div className="acp-timeline">
      {items.map((item, index) => (
        <div
          key={item.id}
          className={`acp-timeline-row ${dragTargetIndex === index && dragSourceIndex !== index ? 'is-drag-target' : ''}`}
          onDragOver={(event) => {
            event.preventDefault();
            onDragOver(event, index);
          }}
          onDrop={() => onDrop(index)}
        >
          <div
            className="acp-timeline-drag"
            draggable
            onDragStart={() => onDragStart(index)}
            onDragEnd={onDragEnd}
            title="Drag to reorder"
          >
            ::
          </div>
          <AcpTimelineItemView item={item} />
        </div>
      ))}
    </div>
  );
};

const AcpTimelineItemView: React.FC<{ item: AcpTimelineItem }> = ({ item }) => {
  switch (item.type) {
    case 'message':
      return (
        <div className={`acp-message acp-message--${item.role}`}>
          <AcpMarkdownMessage text={item.text} />
        </div>
      );
    case 'tool':
      return <AcpToolCard item={item} />;
    case 'permission':
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
      return <AcpThinkingCard item={item} />;
  }
};

const AcpToolCard: React.FC<{ item: AcpTimelineToolItem }> = ({ item }) => {
  const [expanded, setExpanded] = useState(false);
  const title = acpTimelineToolDisplayTitle(item.title, item.kind);
  const kind = acpTimelineToolKindLabel(item.kind);
  const rawText = stringifyRawToolEvent(item.raw);
  return (
    <div className={`acp-event-card acp-tool-card acp-tool-card--${item.status}`}>
      <div className="acp-event-card-header">
        <span className="acp-tool-status-dot" />
        <span className="acp-event-card-kind">{kind}</span>
        <span className="acp-event-card-status">{item.status}</span>
        {item.updatedAt > item.startedAt && (
          <span className="acp-event-card-duration">{formatDuration(item.updatedAt - item.startedAt)}</span>
        )}
      </div>
      <div className="acp-event-card-title">{title}</div>
      {item.kind && item.kind.toLowerCase() !== kind.toLowerCase() && <div className="acp-event-card-meta">{item.kind}</div>}
      {rawText && (
        <>
          <button className="acp-card-link-button" onClick={() => setExpanded((value) => !value)}>
            {expanded ? 'Hide details' : 'Show details'}
          </button>
          {expanded && <pre className="acp-event-card-pre">{rawText}</pre>}
        </>
      )}
    </div>
  );
};

const AcpThinkingCard: React.FC<{ item: AcpTimelineThinkingItem }> = ({ item }) => {
  const [expanded, setExpanded] = useState(false);
  return (
    <div className="acp-thinking-card">
      <button className="acp-thinking-toggle" onClick={() => setExpanded((value) => !value)}>
        <span className="acp-thinking-icon">{expanded ? '▼' : '▶'}</span>
        <span className="acp-thinking-label">Thinking</span>
      </button>
      {expanded && (
        <div className="acp-thinking-content">
          <AcpMarkdownMessage text={item.text} />
        </div>
      )}
    </div>
  );
};

function stringifyRawToolEvent(raw: unknown): string {
  if (!raw) return '';
  try {
    return JSON.stringify(raw, null, 2);
  } catch {
    return String(raw);
  }
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
}
