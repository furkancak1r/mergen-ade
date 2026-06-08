import React, { useMemo } from 'react';
import { TerminalPane } from './TerminalPane';
import { computeTileGrid } from '../lib/layout';
import type { TerminalInstance } from '../hooks/usePty';

interface MainAreaProps {
  terminals: TerminalInstance[];
  activeTerminalId: number | null;
  onTerminalClick: (id: number) => void;
  width: number;
  height: number;
}

export const MainArea: React.FC<MainAreaProps> = ({ terminals, activeTerminalId, onTerminalClick, width, height }) => {
  const grid = useMemo(() => computeTileGrid(terminals.length, width, height), [terminals.length, width, height]);

  if (terminals.length === 0) {
    return (
      <div className="main-area-empty">
        <p>No active terminals. Use the Terminal Manager to spawn one.</p>
      </div>
    );
  }

  return (
    <div
      className="tile-grid"
      style={{
        display: 'grid',
        gridTemplateRows: `repeat(${grid.rows}, 1fr)`,
        gridTemplateColumns: `repeat(${grid.cols}, 1fr)`,
        width: '100%',
        height: '100%',
        gap: '2px',
      }}
    >
      {terminals.map((t) => (
        <div key={t.id} className="tile-cell" style={{ minHeight: 0, minWidth: 0 }}>
          <TerminalPane terminalId={t.id} active={t.id === activeTerminalId} onClick={() => onTerminalClick(t.id)} />
        </div>
      ))}
    </div>
  );
};
