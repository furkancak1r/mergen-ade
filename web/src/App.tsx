import React, { useState, useCallback, useRef, useEffect } from 'react';
import './styles/global.css';
import { useWebSocket } from './hooks/useWebSocket';
import { useIsMobile } from './hooks/useMediaQuery';
import { TerminalPanel, TerminalPanelHandle } from './components/TerminalPanel';
import { TerminalManager } from './components/TerminalManager';
import { ProjectExplorer } from './components/ProjectExplorer';
import { ActivityRail, PanelId } from './components/ActivityRail';
import { DirectoryPanel } from './components/DirectoryPanel';
import { SourceControlPanel } from './components/SourceControlPanel';
import { BrowserPanel } from './components/BrowserPanel';
import { ChecklistPanel } from './components/ChecklistPanel';
import { SettingsPopup } from './components/SettingsPopup';
import { SmartInput } from './components/SmartInput';
import { InputHistoryPanel } from './components/InputHistoryPanel';
import { Button, Input, EmptyState } from './components/ui';
import {
  ServerMessage,
  WebProject,
  WebTerminal,
  ConfigResponse,
} from './types';

const API_URL = '';

export default function App() {
  const isMobile = useIsMobile();
  const [token, setToken] = useState(() => {
    const hash = window.location.hash.replace('#', '');
    return hash || localStorage.getItem('mergen_token') || '';
  });
  const [projects, setProjects] = useState<WebProject[]>([]);
  const [terminals, setTerminals] = useState<WebTerminal[]>([]);
  const [activeTerminalId, setActiveTerminalId] = useState<number | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [statusLine, setStatusLine] = useState('Ready');
  const [connected, setConnected] = useState(false);
  const [activePanel, setActivePanel] = useState<PanelId | null>('projects');
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [browserUrl, setBrowserUrl] = useState('');
  const [checklistOpen, setChecklistOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [config, setConfig] = useState<ConfigResponse | null>(null);
  const terminalRefs = useRef<Record<number, TerminalPanelHandle | null>>({});

  const handleTerminalOutput = useCallback((terminalId: number, data: Uint8Array) => {
    const handle = terminalRefs.current[terminalId];
    if (handle) {
      handle.writeOutput(data);
    }
  }, []);

  const handleMessage = useCallback((msg: ServerMessage) => {
    switch (msg.kind) {
      case 'state_snapshot':
        setProjects(msg.projects);
        setTerminals(msg.terminals);
        setActiveTerminalId(msg.active_terminal_id ?? null);
        setSelectedProjectId(msg.selected_project_id ?? null);
        break;
      case 'state_patch':
        for (const up of msg.updates) {
          switch (up.type) {
            case 'project_added':
              setProjects(prev => [...prev, up.project]);
              break;
            case 'project_removed':
              setProjects(prev => prev.filter(p => p.id !== up.project_id));
              break;
            case 'project_selected':
              setSelectedProjectId(up.project_id ?? null);
              break;
            case 'terminal_added':
              setTerminals(prev => [...prev, up.terminal]);
              break;
            case 'terminal_removed':
              setTerminals(prev => prev.filter(t => t.id !== up.terminal_id));
              break;
            case 'terminal_updated':
              setTerminals(prev => prev.map(t => t.id === up.terminal.id ? up.terminal : t));
              break;
            case 'active_terminal_changed':
              setActiveTerminalId(up.terminal_id ?? null);
              break;
            case 'status_line':
              setStatusLine(up.text);
              break;
          }
        }
        break;
      case 'error':
        setStatusLine(`Error: ${msg.message}`);
        break;
      default:
        break;
    }
  }, []);

  const { sendMessage, sendBinary } = useWebSocket(
    token,
    handleMessage,
    setConnected,
    handleTerminalOutput
  );

  const fetchOpts = (method: string, body?: object): RequestInit => ({
    method,
    headers: {
      'Content-Type': 'application/json',
      'X-Auth-Token': token,
    },
    body: body ? JSON.stringify(body) : undefined,
  });

  useEffect(() => {
    if (connected) {
      fetch(`${API_URL}/api/config`, fetchOpts('GET'))
        .then(r => r.json())
        .then(data => {
          if (data.success && data.data) {
            setConfig(data.data);
          }
        });
    }
  }, [connected]);

  const handleSpawnTerminal = (projectId: number, shell: string, terminalKind: string) => {
    sendMessage({ kind: 'spawn_terminal', project_id: projectId, shell, terminal_kind: terminalKind });
  };

  const handleSelectProject = (projectId: number) => {
    sendMessage({ kind: 'select_project', project_id: projectId });
    setActivePanel('projects');
    if (isMobile) setMobileSidebarOpen(false);
  };

  const handleAddProject = async (name: string, path: string) => {
    const res = await fetch(`${API_URL}/api/projects`, fetchOpts('POST', { name, path }));
    const data = await res.json();
    if (data.success) {
      setStatusLine(data.error || 'Failed to add project');
    }
  };

  const handleTogglePanel = (panel: PanelId) => {
    if (panel === 'browser') {
      setBrowserOpen(v => !v);
      return;
    }
    if (panel === 'checklist') {
      setChecklistOpen(v => !v);
      return;
    }
    if (panel === 'settings') {
      setSettingsOpen(true);
      return;
    }
    setActivePanel(prev => {
      const next = prev === panel ? null : panel;
      if (isMobile && next !== null) {
        setMobileSidebarOpen(true);
      }
      return next;
    });
  };

  const activeTerminal = terminals.find(t => t.id === activeTerminalId);
  const selectedProject = projects.find(p => p.id === selectedProjectId) ?? null;

  const renderLeftPanel = () => {
    if (!activePanel) return null;
    switch (activePanel) {
      case 'projects':
        return (
          <ProjectExplorer
            projects={projects}
            selectedProjectId={selectedProjectId}
            onSelectProject={handleSelectProject}
            onAddProject={handleAddProject}
          />
        );
      case 'directory':
        return <DirectoryPanel projectId={selectedProjectId} apiUrl={API_URL} />;
      case 'source-control':
        return <SourceControlPanel project={selectedProject} apiUrl={API_URL} />;
      case 'input-history':
        return (
          <InputHistoryPanel
            terminals={terminals}
            onSend={(id, text) => sendMessage({ kind: 'terminal_paste', terminal_id: id, text })}
          />
        );
      case 'terminal-manager':
        return (
          <TerminalManager
            terminals={terminals}
            activeTerminalId={activeTerminalId}
            onActivate={id => {
              setActiveTerminalId(id);
              if (isMobile) setMobileSidebarOpen(false);
            }}
            selectedProjectId={selectedProjectId}
            projects={projects}
            onSpawn={handleSpawnTerminal}
            onClose={id => sendMessage({ kind: 'close_terminal', terminal_id: id })}
            onSendSavedMessage={(id, msg) => sendMessage({ kind: 'terminal_paste', terminal_id: id, text: msg })}
            onSendShortcut={(id, cmd) => sendMessage({ kind: 'send_shortcut', terminal_id: id, command: cmd })}
            configLaunchers={config?.launchers ?? []}
            configShortcuts={config?.shortcuts ?? []}
          />
        );
      default:
        return null;
    }
  };

  const showSidebar = activePanel && activePanel !== 'browser' && activePanel !== 'checklist';

  return (
    <div className="main-area" style={{
      display: 'flex',
      flexDirection: 'column',
      width: '100vw',
      height: '100vh',
      overflow: 'hidden',
      background: 'var(--bg-base)',
    }}>
      {/* Top bar */}
      <div className="top-bar" style={{
        height: 40,
        background: 'var(--bg-elevated)',
        borderBottom: '1px solid var(--border-subtle)',
        display: 'flex',
        alignItems: 'center',
        padding: '0 var(--space-lg)',
        gap: 'var(--space-lg)',
        flexShrink: 0,
      }}>
        {isMobile && (
          <Button
            variant="ghost"
            onClick={() => setMobileSidebarOpen(v => !v)}
            style={{ fontSize: 20, minWidth: 44, minHeight: 44, padding: 0 }}
            title="Toggle menu"
          >
            ☰
          </Button>
        )}
        <strong style={{ color: 'var(--accent)', fontSize: 'var(--font-lg)' }}>Mergen ADE</strong>
        <span style={{ fontSize: 'var(--font-base)', color: connected ? 'var(--success)' : 'var(--danger)', display: 'flex', alignItems: 'center', gap: 'var(--space-xs)' }}>
          <span style={{ width: 8, height: 8, borderRadius: '50%', background: connected ? 'var(--success)' : 'var(--danger)', display: 'inline-block' }} />
          {connected ? 'Connected' : 'Disconnected'}
        </span>
        <div style={{ flex: 1 }} />
        {!connected && (
          <div style={{ display: 'flex', gap: 'var(--space-xs)', alignItems: 'center' }}>
            <input
              type="password"
              placeholder="Token"
              value={token}
              onChange={e => setToken(e.target.value)}
              style={{
                width: 100,
                background: 'var(--bg-input)',
                border: '1px solid var(--border-default)',
                color: 'var(--text-primary)',
                fontSize: 'var(--font-xs)',
                padding: '2px 6px',
                borderRadius: 'var(--radius-sm)',
                outline: 'none',
              }}
              onFocus={e => { e.currentTarget.style.borderColor = 'var(--border-focus)'; }}
              onBlur={e => { e.currentTarget.style.borderColor = 'var(--border-default)'; }}
            />
            <button
              onClick={() => { localStorage.setItem('mergen_token', token); window.location.reload(); }}
              style={{
                background: 'var(--accent-dim)',
                border: '1px solid var(--accent)',
                color: 'var(--accent)',
                fontSize: 'var(--font-xs)',
                cursor: 'pointer',
                padding: '2px 8px',
                borderRadius: 'var(--radius-sm)',
                fontWeight: 600,
                lineHeight: 1,
              }}
            >
              Connect
            </button>
          </div>
        )}
        <span style={{
          fontSize: 'var(--font-sm)',
          color: 'var(--text-secondary)',
          maxWidth: 400,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}>
          {statusLine}
        </span>
      </div>

      {/* Body */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden', position: 'relative' }}>
        <ActivityRail
          activePanel={activePanel}
          onTogglePanel={handleTogglePanel}
          browserOpen={browserOpen}
          checklistOpen={checklistOpen}
        />

        {/* Mobile sidebar backdrop */}
        {isMobile && (
          <div
            className={`sidebar-backdrop ${mobileSidebarOpen ? 'open' : ''}`}
            onClick={() => setMobileSidebarOpen(false)}
          />
        )}

        {/* Left sidebar */}
        {showSidebar && (
          <div
            className={`sidebar-drawer ${isMobile && mobileSidebarOpen ? 'open' : ''}`}
            style={{
              width: 280,
              flexShrink: 0,
              background: 'var(--bg-surface)',
              borderRight: '1px solid var(--border-subtle)',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
          >
            {selectedProject && (
              <div style={{
                padding: 'var(--space-md) var(--space-lg)',
                borderBottom: '1px solid var(--border-subtle)',
                background: 'var(--bg-elevated)',
              }}>
                <div style={{
                  fontSize: 'var(--font-base)',
                  fontWeight: 700,
                  color: 'var(--text-primary)',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  marginBottom: 'var(--space-sm)',
                }}>
                  {selectedProject.name}
                </div>
                <div style={{ display: 'flex', gap: 'var(--space-sm)' }}>
                  <Button
                    variant="primary"
                    onClick={() => handleSpawnTerminal(selectedProject.id, 'powershell', 'foreground')}
                    style={{ flex: 1, fontSize: 'var(--font-xs)', minHeight: 32 }}
                  >
                    + Foreground
                  </Button>
                  <Button
                    variant="secondary"
                    onClick={() => handleSpawnTerminal(selectedProject.id, 'powershell', 'background')}
                    style={{ flex: 1, fontSize: 'var(--font-xs)', minHeight: 32 }}
                  >
                    + Background
                  </Button>
                </div>
              </div>
            )}
            {renderLeftPanel()}
          </div>
        )}

        {/* Main area */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
            {/* Terminal area */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
              {activeTerminal ? (
                <TerminalPanel
                  key={activeTerminal.id}
                  ref={instance => {
                    terminalRefs.current[activeTerminal.id] = instance;
                  }}
                  terminal={activeTerminal}
                  onInput={data => {
                    const header = new Uint8Array(8);
                    const view = new DataView(header.buffer);
                    view.setBigUint64(0, BigInt(activeTerminal.id), true);
                    const payload = new Uint8Array(header.length + data.length);
                    payload.set(header, 0);
                    payload.set(data, 8);
                    sendBinary(payload);
                  }}
                  onPaste={text => sendMessage({ kind: 'terminal_paste', terminal_id: activeTerminal.id, text })}
                  onResize={(cols, lines) => sendMessage({ kind: 'terminal_resize', terminal_id: activeTerminal.id, cols, lines })}
                />
              ) : (
                <EmptyState message="No active terminal. Select a project and spawn a terminal." />
              )}
            </div>

            {/* Check-list panel (right side, hidden on mobile) */}
            {checklistOpen && !isMobile && (
              <div style={{
                width: 260,
                flexShrink: 0,
                background: 'var(--bg-surface)',
                borderLeft: '1px solid var(--border-subtle)',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden',
              }}>
                <ChecklistPanel
                  projects={projects}
                  onCopyItems={projectId => {
                    const p = projects.find(x => x.id === projectId);
                    if (p) {
                      navigator.clipboard.writeText(p.checklist.join('\n\n'));
                      setStatusLine(`Copied ${p.checklist.length} checklist items`);
                    }
                  }}
                />
              </div>
            )}

            {/* Browser panel (right side, hidden on mobile) */}
            {browserOpen && !isMobile && (
              <div style={{
                width: 360,
                flexShrink: 0,
                background: 'var(--bg-surface)',
                borderLeft: '1px solid var(--border-subtle)',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden',
              }}>
                <BrowserPanel
                  url={browserUrl}
                  onUrlChange={setBrowserUrl}
                />
              </div>
            )}
          </div>

          {/* Smart Input footer */}
          {activeTerminal && (
            <div className="smart-input-footer">
              <SmartInput
                terminal={activeTerminal}
                onSubmit={(text, mode) => {
                  sendMessage({ kind: 'smart_input_submit', terminal_id: activeTerminal.id, text, mode });
                }}
              />
            </div>
          )}
        </div>
      </div>

      {/* Settings popup */}
      {settingsOpen && config && (
        <SettingsPopup
          shortcuts={config.shortcuts}
          launchers={config.launchers}
          defaultShell={config.default_shell}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}
