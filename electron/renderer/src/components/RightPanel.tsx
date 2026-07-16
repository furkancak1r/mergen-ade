import React from 'react';
import type { ProjectRecord, GitWorktreeInfo, BrowserScopeKey, BrowserTab, SourceControlSnapshot } from '../../../shared/types';
import { BrowserScopeKeyType } from '../../../shared/types';
import { SourceControl } from './SourceControl';
import { BrowserPanel } from './BrowserPanel';

export type RightPanelTab = 'sourceControl' | 'browser';

interface RightPanelProps {
  activeTab: RightPanelTab;
  onTabChange: (tab: RightPanelTab) => void;
  onClose: () => void;

  // Source Control props
  project: ProjectRecord;
  projects?: ProjectRecord[];
  selectedProjectId?: number | null;
  onSelectProject?: (projectId: number) => void;
  onAddWorktree?: (worktree: GitWorktreeInfo) => void;
  onRemoveWorktree?: (worktree: GitWorktreeInfo) => void;
  onDeleteGitWorktree?: (worktree: GitWorktreeInfo) => void;
  hasLiveTerminals?: (path: string) => boolean;
  registeredWorktreePaths?: string[];
  onOrphanWorktrees?: (paths: string[]) => void;
  onBranchChange?: (branch: string) => void;

  // Browser props
  browserVisible: boolean;
  activeTerminalId?: number | null;
  visibleScopeOverride?: BrowserScopeKey;
  tabsByScope: Map<string, BrowserTab[]>;
  activeTabByScope: Map<string, string | null>;
  urlDraftByScope: Map<string, string>;
  designInspectByScope: Map<string, boolean>;
  onTabsChange: React.Dispatch<React.SetStateAction<Map<string, BrowserTab[]>>>;
  onActiveTabChange: React.Dispatch<React.SetStateAction<Map<string, string | null>>>;
  onUrlDraftChange: React.Dispatch<React.SetStateAction<Map<string, string>>>;
  onDesignInspectChange: React.Dispatch<React.SetStateAction<Map<string, boolean>>>;
  onScopeEmpty?: (scope: BrowserScopeKey) => void;
  onClearProjectBrowserLastUrl?: (projectId: number) => void;
  settingsOpen?: boolean;
}

export const RightPanel: React.FC<RightPanelProps> = ({
  activeTab,
  onTabChange,
  onClose,
  project,
  projects,
  selectedProjectId,
  onSelectProject,
  onAddWorktree,
  onRemoveWorktree,
  onDeleteGitWorktree,
  hasLiveTerminals,
  registeredWorktreePaths,
  onOrphanWorktrees,
  onBranchChange,
  browserVisible,
  activeTerminalId,
  visibleScopeOverride,
  tabsByScope,
  activeTabByScope,
  urlDraftByScope,
  designInspectByScope,
  onTabsChange,
  onActiveTabChange,
  onUrlDraftChange,
  onDesignInspectChange,
  onScopeEmpty,
  onClearProjectBrowserLastUrl,
  settingsOpen,
}) => {
  return (
    <div className="right-panel">
      <div className="right-panel-tab-bar">
        <button
          className={`right-panel-tab ${activeTab === 'sourceControl' ? 'active' : ''}`}
          onClick={() => onTabChange('sourceControl')}
          type="button"
        >
          <span className="right-panel-tab-icon">&#x2402;</span>
          Source Control
        </button>
        <button
          className={`right-panel-tab ${activeTab === 'browser' ? 'active' : ''}`}
          onClick={() => onTabChange('browser')}
          type="button"
        >
          <span className="right-panel-tab-icon">&#x25CE;</span>
          Browser
        </button>
        <div style={{ flex: 1 }} />
        <button
          className="right-panel-close-btn"
          onClick={onClose}
          type="button"
          data-tooltip="Close panel"
          aria-label="Close panel"
        >
          &#x2715;
        </button>
      </div>

      <div className="right-panel-content">
        {activeTab === 'sourceControl' && (
          <SourceControl
            project={project}
            projects={projects}
            selectedProjectId={selectedProjectId}
            onSelectProject={onSelectProject}
            onAddWorktree={onAddWorktree}
            onRemoveWorktree={onRemoveWorktree}
            onDeleteGitWorktree={onDeleteGitWorktree}
            hasLiveTerminals={hasLiveTerminals}
            registeredWorktreePaths={registeredWorktreePaths}
            onOrphanWorktrees={onOrphanWorktrees}
            onBranchChange={onBranchChange}
          />
        )}
        {activeTab === 'browser' && browserVisible && (
          <BrowserPanel
            project={project}
            activeTerminalId={activeTerminalId}
            visibleScopeOverride={visibleScopeOverride}
            onClose={() => onTabChange('sourceControl')}
            hidden={settingsOpen}
            tabsByScope={tabsByScope}
            activeTabByScope={activeTabByScope}
            urlDraftByScope={urlDraftByScope}
            designInspectByScope={designInspectByScope}
            onTabsChange={onTabsChange}
            onActiveTabChange={onActiveTabChange}
            onUrlDraftChange={onUrlDraftChange}
            onDesignInspectChange={onDesignInspectChange}
            onScopeEmpty={onScopeEmpty}
            onClearProjectBrowserLastUrl={onClearProjectBrowserLastUrl}
          />
        )}
        {activeTab === 'browser' && !browserVisible && (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', padding: 24, gap: 16 }}>
            <span style={{ fontSize: 28, opacity: 0.3 }}>&#x25CE;</span>
            <span style={{ fontSize: 13, color: '#888', textAlign: 'center' }}>
              Browser requires a foreground terminal.
            </span>
          </div>
        )}
      </div>
    </div>
  );
};
