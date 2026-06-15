import React, { useMemo } from 'react';
import { TerminalPane } from './TerminalPane';
import { SmartInputFooter } from './SmartInputFooter';
import { computeTileGrid } from '../lib/layout';
import type { TerminalInstance } from '../hooks/usePty';
import type { SmartInputState, SmartInputAttachment, OpenCodeQuestion } from '../../../shared/types';
import { AiCliTool as AiCliToolEnum } from '../../../shared/types';
import { claudeCodexHookProgressText, type ClaudeCodexHookProgress } from '../../../shared/claudeCodexHook';
import type { SmartInputModeId } from '../lib/smartInputMode';
import { shouldShowSmartInputFooter } from '../lib/smartInput';

interface MainAreaProps {
  terminals: TerminalInstance[];
  activeTerminalId: number | null;
  onTerminalClick: (id: number) => void;
  width: number;
  height: number;
  onUpdateSmartInputState: (terminalId: number, state: Partial<SmartInputState>) => void;
  onSendToTerminal: (terminalId: number, text: string, attachments: SmartInputAttachment[], modeId: SmartInputModeId) => void;
  onUpdateQuestionState?: (terminalId: number, updates: { focusIndex?: number; selectedOptions?: string[]; customText?: string }) => void;
  onTerminalOutputClick?: (terminalId: number) => void;
  onClearTerminalOutputFocusOverride?: (terminalId: number) => void;
  onScrollDetached?: (terminalId: number, detached: boolean) => void;
  wheelEnabled?: boolean;
  disabled?: boolean;
}

export const MainArea: React.FC<MainAreaProps> = ({ terminals, activeTerminalId, onTerminalClick, width, height, onUpdateSmartInputState, onSendToTerminal, onUpdateQuestionState, onTerminalOutputClick, onClearTerminalOutputFocusOverride, onScrollDetached, wheelEnabled = true, disabled = false }) => {
  const grid = useMemo(() => computeTileGrid(terminals.length, width, height), [terminals.length, width, height]);

  // Keyboard grid navigation: Ctrl+Arrow moves to adjacent terminal, Ctrl+Alt+Arrow for linear navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!e.ctrlKey) return;
    const activeIndex = terminals.findIndex((t) => t.id === activeTerminalId);
    if (activeIndex < 0) return;

    const cols = grid.cols;
    const rows = grid.rows;
    let nextIndex = activeIndex;

    if (e.altKey) {
      // Linear navigation
      if (e.key === 'ArrowLeft') {
        nextIndex = activeIndex - 1;
      } else if (e.key === 'ArrowRight') {
        nextIndex = activeIndex + 1;
      } else if (e.key === 'ArrowUp') {
        nextIndex = activeIndex - cols;
      } else if (e.key === 'ArrowDown') {
        nextIndex = activeIndex + cols;
      }
    } else {
      // Grid navigation
      const row = Math.floor(activeIndex / cols);
      const col = activeIndex % cols;
      if (e.key === 'ArrowLeft') {
        if (col > 0) nextIndex = activeIndex - 1;
      } else if (e.key === 'ArrowRight') {
        if (col < cols - 1 && activeIndex + 1 < terminals.length) nextIndex = activeIndex + 1;
      } else if (e.key === 'ArrowUp') {
        if (row > 0) nextIndex = activeIndex - cols;
      } else if (e.key === 'ArrowDown') {
        if (row < rows - 1 && activeIndex + cols < terminals.length) nextIndex = activeIndex + cols;
      }
    }

    if (nextIndex !== activeIndex && nextIndex >= 0 && nextIndex < terminals.length) {
      e.preventDefault();
      onTerminalClick(terminals[nextIndex].id);
    }
  };

  if (terminals.length === 0) {
    return (
      <div className="main-area-empty" style={{ width: '100%', height: '100%', background: '#0c0c0c' }} />
    );
  }

  return (
    <div
      className="tile-grid"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      style={{
        display: 'grid',
        gridTemplateRows: `repeat(${grid.rows}, 1fr)`,
        gridTemplateColumns: `repeat(${grid.cols}, 1fr)`,
        width: '100%',
        height: '100%',
        gap: '2px',
        outline: 'none',
      }}
    >
      {terminals.map((t) => {
        const isActive = t.id === activeTerminalId;
        const showSmartInput = shouldShowSmartInputFooter(t.kind, t.aiTool, t.aiStatus, t.opencodeSessionActive, t.claudeLaunchPending);
        const modeControlsVisible = t.aiTool === AiCliToolEnum.OpenCode || t.aiTool === AiCliToolEnum.Claude;
        return (
          <div key={t.id} className="tile-cell" style={{ minHeight: 0, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
            <div style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
              {t.claudeCodexHookProgress && (
                <ClaudeCodexHookProgressBand progress={t.claudeCodexHookProgress} />
              )}
              <div style={{ flex: 1, minHeight: 0 }}>
                <TerminalPane terminalId={t.id} projectId={t.projectId} active={isActive} onClick={() => onTerminalClick(t.id)} onTerminalOutputClick={() => onTerminalOutputClick?.(t.id)} wheelEnabled={wheelEnabled} isOpenCodeActive={t.aiTool === 'opencode' && t.opencodeSessionActive} opencodeManualScrollDetached={t.opencodeManualScrollDetached} opencodeLeadingBlankRows={t.opencodeLeadingBlankRows} onScrollDetached={(detached) => onScrollDetached?.(t.id, detached)} />
              </div>
            </div>
            {showSmartInput && (
              <SmartInputFooter
                terminalId={t.id}
                state={t.smartInputState}
                question={t.opencodePendingQuestion}
                questionFocusIndex={t.opencodeQuestionFocusIndex}
                questionSelectedOptions={t.opencodeQuestionSelectedOptions}
                questionCustomText={t.opencodeQuestionCustomText}
                onUpdateState={(state) => onUpdateSmartInputState(t.id, state)}
                onSendToTerminal={(terminalId, text, attachments, modeId) => onSendToTerminal(terminalId, text, attachments, modeId)}
                onUpdateQuestionState={(updates) => onUpdateQuestionState?.(t.id, updates)}
                onClearTerminalOutputFocusOverride={() => onClearTerminalOutputFocusOverride?.(t.id)}
                modeControlsVisible={modeControlsVisible}
                disabled={disabled}
              />
            )}
          </div>
        );
      })}
    </div>
  );
};

function ClaudeCodexHookProgressBand({ progress }: { progress: ClaudeCodexHookProgress }) {
  const accent = progress.phase === 'blocked'
    ? '#dc5050'
    : progress.phase === 'awaiting_implementation'
      ? '#78be91'
      : '#60a5fa';
  return (
    <div
      title={progress.planPath ? `Plan file: ${progress.planPath}` : progress.error}
      style={{
        height: 24,
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '0 10px',
        background: '#12181f',
        borderBottom: '1px solid #263241',
        borderLeft: `3px solid ${accent}`,
        color: '#d8dee9',
        fontSize: 11,
        whiteSpace: 'nowrap',
        overflow: 'hidden',
        textOverflow: 'ellipsis',
      }}
    >
      <span style={{ width: 6, height: 6, borderRadius: '50%', background: accent, flexShrink: 0 }} />
      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{claudeCodexHookProgressText(progress)}</span>
    </div>
  );
}
