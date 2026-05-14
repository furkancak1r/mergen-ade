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

  // Shared fetch options with auth token
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
    <div className="main-area" style={{ display: 'flex', flexDirection: 'column', width: '100vw', height: '100vh', overflow: 'hidden', background: '#0c0c0c' }}>
      {/* Top bar */}
      <div className="top-bar" style={{ height: 40, background: '#1a1a1a', borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', padding: '0 12px', gap: 12, flexShrink: 0 }}>
        {isMobile && (
          <button
            onClick={() => setMobileSidebarOpen(v => !v)}
            style={{ background: 'transparent', border: 'none', color: '#e0e0e0', fontSize: 20, cursor: 'pointer', minWidth: 44, minHeight: 44, display: 'flex', alignItems: 'center', justifyContent: 'center' }}
            aria-label="Toggle menu"
          >
            ☰
          </button>
        )}
        <strong style={{ color: '#4fc3f7', fontSize: 14 }}>Mergen ADE</strong>
        <span style={{ fontSize: 12, color: connected ? '#4caf50' : '#f44336' }}>
          {connected ? '● Connected' : '● Disconnected'}
        </span>
        <div style={{ flex: 1 }} />
        {!connected && (
          <div style={{ display: 'flex', gap: 4 }}>
            <input
              type="text"
              placeholder="Auth token"
              value={token}
              onChange={e => setToken(e.target.value)}
              style={{ background: '#222', border: '1px solid #444', color: '#e0e0e0', padding: '2px 6px', fontSize: 12 }}
            />
            <button
              onClick={() => { localStorage.setItem('mergen_token', token); window.location.reload(); }}
              style={{ background: '#333', border: '1px solid #555', color: '#e0e0e0', fontSize: 12, cursor: 'pointer', padding: '6px 12px' }}
            >
              Connect
            </button>
          </div>
        )}
        <span style={{ fontSize: 11, color: '#888', maxWidth: 400, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
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
              background: '#141414',
              borderRight: '1px solid #333',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
          >
            {selectedProject && (
              <div style={{ padding: '8px 10px', borderBottom: '1px solid #333', background: '#1a1a1a' }}>
                <div style={{ fontSize: 12, fontWeight: 'bold', color: '#e0e0e0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', marginBottom: 6 }}>
                  {selectedProject.name}
                </div>
                <div style={{ display: 'flex', gap: 4 }}>
                  <button
                    onClick={() => handleSpawnTerminal(selectedProject.id, 'powershell', 'foreground')}
                    style={{ flex: 1, fontSize: 10, background: '#1e3a5f', border: '1px solid #4fc3f7', color: '#4fc3f7', cursor: 'pointer', padding: '6px 6px', borderRadius: 3, minHeight: 32 }}
                  >
                    + Foreground
                  </button>
                  <button
                    onClick={() => handleSpawnTerminal(selectedProject.id, 'powershell', 'background')}
                    style={{ flex: 1, fontSize: 10, background: '#2a2a2a', border: '1px solid #888', color: '#888', cursor: 'pointer', padding: '6px 6px', borderRadius: 3, minHeight: 32 }}
                  >
                    + Background
                  </button>
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
                <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#666', fontSize: 14, textAlign: 'center', padding: 16 }}>
                  No active terminal. Select a project and spawn a terminal.
                </div>
              )}
            </div>

            {/* Check-list panel (right side, hidden on mobile) */}
            {checklistOpen && !isMobile && (
              <div style={{ width: 260, flexShrink: 0, background: '#141414', borderLeft: '1px solid #333', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
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
              <div style={{ width: 360, flexShrink: 0, background: '#141414', borderLeft: '1px solid #333', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
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
