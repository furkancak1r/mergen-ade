import React, { useEffect, useRef, useState, useCallback } from 'react';
import type { AppConfig, ShellKind, TerminalKind, ProjectRecord, LeftSidebarTab, BrowserScopeKey } from '../../shared/types';
import { LeftSidebarTab as LeftSidebarTabEnum } from '../../shared/types';
import { MainArea } from './components/MainArea';
import { ProjectExplorer } from './components/ProjectExplorer';
import { TerminalManager } from './components/TerminalManager';
import { SourceControl } from './components/SourceControl';
import { FileEditor } from './components/FileEditor';
import { Checklist } from './components/Checklist';
import { AcpChatPanel } from './components/AcpChatPanel';
import { BrowserPanel } from './components/BrowserPanel';
import { usePty } from './hooks/usePty';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [activeTab, setActiveTab] = useState<LeftSidebarTab>(LeftSidebarTabEnum.Directory);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [activeTerminalId, setActiveTerminalId] = useState<number | null>(null);
  const [terminals, setTerminals] = useState<ReturnType<ReturnType<typeof usePty>['getTerminals']>>([]);
  const [mainSize, setMainSize] = useState({ width: 800, height: 600 });
  const mainRef = useRef<HTMLDivElement>(null);
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [isResizing, setIsResizing] = useState(false);
  const sidebarRef = useRef<HTMLDivElement>(null);

  const [fileEditorOpen, setFileEditorOpen] = useState(false);
  const [fileEditorPath, setFileEditorPath] = useState<string | null>(null);
  const [fileEditorName, setFileEditorName] = useState<string | null>(null);

  const [checklistVisible, setChecklistVisible] = useState(false);

  const [activeAcpChat, setActiveAcpChat] = useState<{ chatId: string; projectId: number } | null>(null);

  const [browserOpenProjects, setBrowserOpenProjects] = useState<Set<number>>(new Set());
  const [browserPanelWidth, setBrowserPanelWidth] = useState(520);
  const [isResizingBrowser, setIsResizingBrowser] = useState(false);

  const pty = usePty();

  useEffect(() => {
    api.invoke('config:load').then((cfg) => {
      const loaded = cfg as AppConfig;
      setConfig(loaded);
      if (loaded.projects.length > 0) {
        setSelectedProjectId(loaded.projects[0].id);
      }
    });
  }, []);

  useEffect(() => {
    if (!config) return;
    const timer = setTimeout(() => {
      api.invoke('config:save', config);
    }, 500);
    return () => clearTimeout(timer);
  }, [config]);

  useEffect(() => {
    const unsub = pty.subscribe(() => {
      setTerminals(pty.getTerminals());
    });
    return () => { unsub(); };
  }, [pty]);

  useEffect(() => {
    const unsub = (window as unknown as { mergenApi: { on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi.on('window:closeRequest', () => {
      const ok = window.confirm('Are you sure you want to close Mergen ADE?');
      if (ok) {
        api.invoke('window:confirmClose', true);
      }
    });
    return () => { unsub(); };
  }, []);

  useEffect(() => {
    if (!mainRef.current) return;
    const el = mainRef.current;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const cr = entry.contentRect;
        setMainSize({ width: cr.width, height: cr.height });
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    if (!isResizing) return;
    function handleMove(e: MouseEvent) {
      const newWidth = Math.max(200, Math.min(500, e.clientX));
      setSidebarWidth(newWidth);
    }
    function handleUp() {
      setIsResizing(false);
    }
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [isResizing]);

  useEffect(() => {
    if (!isResizingBrowser) return;
    function handleMove(e: MouseEvent) {
      if (!mainRef.current) return;
      const mainRect = mainRef.current.getBoundingClientRect();
      const newWidth = Math.max(240, Math.min(800, mainRect.right - e.clientX));
      setBrowserPanelWidth(newWidth);
    }
    function handleUp() {
      setIsResizingBrowser(false);
    }
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [isResizingBrowser]);

  const selectedProject = config?.projects.find((p) => p.id === selectedProjectId) ?? null;

  const isBrowserOpen = selectedProjectId !== null && browserOpenProjects.has(selectedProjectId);

  const spawnTerminal = useCallback(async (projectId: number, kind: TerminalKind) => {
    if (!config) return;
    const project = config.projects.find((p) => p.id === projectId);
    const cwd = project?.path ?? process.cwd();
    const id = await pty.createTerminal({
      shell: config.defaultShell,
      cwd,
      cols: 80,
      rows: 24,
      projectId,
      kind,
    });
    setActiveTerminalId(id);
  }, [config, pty]);

  const activateTerminal = useCallback((id: number) => {
    setActiveTerminalId(id);
    setFileEditorOpen(false);
    setActiveAcpChat(null);
    const t = terminals.find((x) => x.id === id);
    if (t) {
      setSelectedProjectId(t.projectId);
    }
  }, [terminals]);

  const killTerminal = useCallback((id: number) => {
    pty.killTerminal(id);
    if (activeTerminalId === id) {
      setActiveTerminalId(null);
    }
  }, [pty, activeTerminalId]);

  const openFile = useCallback((filePath: string) => {
    setFileEditorPath(filePath);
    setFileEditorName(filePath.split('/').pop() || filePath.split('\\').pop() || filePath);
    setFileEditorOpen(true);
    setActiveAcpChat(null);
  }, []);

  const closeFileEditor = useCallback(() => {
    setFileEditorOpen(false);
  }, []);

  const openAcpChat = useCallback(async (projectId: number) => {
    if (!config) return;
    const project = config.projects.find((p) => p.id === projectId);
    if (!project) return;
    const chatId = await api.invoke('acp:spawn', { projectId, cwd: project.path, mcpServers: [] }) as string;
    setActiveAcpChat({ chatId, projectId });
    setFileEditorOpen(false);
  }, [config]);

  const closeAcpChat = useCallback(() => {
    setActiveAcpChat(null);
  }, []);

  const toggleBrowser = useCallback(() => {
    if (!selectedProjectId) return;
    setBrowserOpenProjects((prev) => {
      const next = new Set(prev);
      if (next.has(selectedProjectId)) {
        next.delete(selectedProjectId);
      } else {
        next.add(selectedProjectId);
      }
      return next;
    });
  }, [selectedProjectId]);

  const activeTerminals = terminals.filter((t) => {
    if (config?.ui.mainVisibilityMode === 'selected_project') {
      return t.projectId === selectedProjectId;
    }
    return true;
  });

  const acpProject = activeAcpChat ? config?.projects.find((p) => p.id === activeAcpChat.projectId) ?? null : null;

  return (
    <div className="app-container">
      <div className="activity-rail">
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.Directory ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.Directory)}
          title="Project Explorer"
        >
          Explorer
        </button>
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.TerminalManager ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.TerminalManager)}
          title="Terminal Manager"
        >
          Terminals
        </button>
        <button
          className={`rail-btn ${activeTab === LeftSidebarTabEnum.SourceControl ? 'active' : ''}`}
          onClick={() => setActiveTab(LeftSidebarTabEnum.SourceControl)}
          title="Source Control"
        >
          Git
        </button>
        <div style={{ flex: 1 }} />
        <button
          className={`rail-btn ${isBrowserOpen ? 'active' : ''}`}
          onClick={toggleBrowser}
          title="Browser"
        >
          Web
        </button>
        <button
          className={`rail-btn ${checklistVisible ? 'active' : ''}`}
          onClick={() => setChecklistVisible((v) => !v)}
          title="Checklist"
        >
          Check
        </button>
      </div>

      <div
        ref={sidebarRef}
        className="sidebar"
        style={{ width: sidebarWidth, minWidth: 200, maxWidth: 500, display: 'flex', flexDirection: 'column', overflow: 'hidden', borderRight: '1px solid #222' }}
      >
        {activeTab === LeftSidebarTabEnum.Directory && selectedProject && (
          <ProjectExplorer
            project={selectedProject}
            selectedPath={selectedProject.path}
            onOpenFile={openFile}
          />
        )}
        {activeTab === LeftSidebarTabEnum.TerminalManager && config && (
          <TerminalManager
            config={config}
            terminals={terminals}
            activeTerminalId={activeTerminalId}
            onActivateTerminal={activateTerminal}
            onSpawnTerminal={spawnTerminal}
            onKillTerminal={killTerminal}
          />
        )}
        {activeTab === LeftSidebarTabEnum.SourceControl && selectedProject && (
          <SourceControl project={selectedProject} />
        )}
      </div>

      <div
        className="resize-handle"
        onMouseDown={() => setIsResizing(true)}
        style={{
          width: 4,
          cursor: 'col-resize',
          background: isResizing ? '#0078d4' : 'transparent',
        }}
      />

      <div className="main-area" ref={mainRef}>
        {activeAcpChat && acpProject ? (
          <AcpChatPanel
            project={acpProject}
            chatId={activeAcpChat.chatId}
            onClose={closeAcpChat}
          />
        ) : fileEditorOpen && fileEditorPath && fileEditorName ? (
          <FileEditor
            filePath={fileEditorPath}
            displayName={fileEditorName}
            onClose={closeFileEditor}
          />
        ) : (
          <MainArea
            terminals={activeTerminals}
            activeTerminalId={activeTerminalId}
            onTerminalClick={activateTerminal}
            width={mainSize.width}
            height={mainSize.height}
          />
        )}
      </div>

      {isBrowserOpen && selectedProject && (
        <>
          <div
            className="resize-handle"
            onMouseDown={() => setIsResizingBrowser(true)}
            style={{
              width: 4,
              cursor: 'col-resize',
              background: isResizingBrowser ? '#0078d4' : 'transparent',
            }}
          />
          <div style={{ width: browserPanelWidth, minWidth: 240, maxWidth: 800, display: 'flex', flexDirection: 'column', overflow: 'hidden', borderLeft: '1px solid #222' }}>
            <BrowserPanel
              project={selectedProject}
              activeTerminalId={activeTerminalId}
              onClose={toggleBrowser}
            />
          </div>
        </>
      )}

      {checklistVisible && config && (
        <Checklist
          projects={config.projects}
        />
      )}
    </div>
  );
}

export default App;
