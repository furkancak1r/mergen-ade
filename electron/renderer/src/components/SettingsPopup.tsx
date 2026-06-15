import React, { useState, useCallback, useEffect } from 'react';
import type {
  AppConfig,
  TerminalShortcutEntry,
  ShortcutModifiers,
  OpenCodeModelConfig,
  OsNotificationConfig,
  AcpModeToggleShortcut,
  AcpStartupMode,
  MainVisibilityMode,
  LauncherEntry,
  AppDiagnostics,
} from '../../../shared/types';
import {
  AcpStartupModeLabel,
  BuiltinLauncherKind,
  BuiltinLauncherKindDefaultDisplayName,
  BuiltinLauncherKindDefaultLaunchCommand,
  LauncherIconKey,
  LauncherIconKeyCustomPresets,
  LauncherIconKeyLabel,
} from '../../../shared/types';
import {
  addSavedMessage,
  removeSavedMessage,
  replaceProjectSavedMessages,
  savedMessageOwnerProjectId,
  updateSavedMessage,
} from '../lib/savedMessages';
import {
  diagnosticsColor,
  runtimeOverview,
  type ActiveTerminalDiagnostics,
} from '../lib/diagnostics';
import { OPENCODE_ACP_LABEL } from '../lib/acpUi';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface SettingsPopupProps {
  config: AppConfig;
  activeTerminal?: ActiveTerminalDiagnostics;
  onSave: (config: AppConfig) => void;
  onClose: () => void;
}

type SettingsTab = 'general' | 'launchers' | 'opencode' | 'saved' | 'shortcuts' | 'notifications' | 'diagnostics';

const tabs: { id: SettingsTab; label: string }[] = [
  { id: 'general', label: 'General' },
  { id: 'launchers', label: 'Launchers' },
  { id: 'opencode', label: 'OpenCode' },
  { id: 'saved', label: 'Saved Messages' },
  { id: 'shortcuts', label: 'Shortcuts' },
  { id: 'notifications', label: 'Notifications' },
  { id: 'diagnostics', label: 'Diagnostics' },
];

export const SettingsPopup: React.FC<SettingsPopupProps> = ({ config, activeTerminal, onSave, onClose }) => {
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [draft, setDraft] = useState<AppConfig>({ ...config });
  const [recordingShortcutIndex, setRecordingShortcutIndex] = useState<number | null>(null);
  const [recordingAcpShortcut, setRecordingAcpShortcut] = useState(false);
  const [diagnostics, setDiagnostics] = useState<AppDiagnostics | undefined>();
  const [diagnosticsError, setDiagnosticsError] = useState<string | undefined>();
  const [diagnosticsExpanded, setDiagnosticsExpanded] = useState(false);
  const [launcherDraft, setLauncherDraft] = useState({
    displayName: '',
    launchCommand: '',
    iconKey: LauncherIconKey.Terminal,
  });
  const [savedMessageDrafts, setSavedMessageDrafts] = useState<Record<number, string>>({});

  useEffect(() => {
    if (activeTab !== 'diagnostics') return;
    let cancelled = false;
    setDiagnosticsError(undefined);
    api.invoke('diagnostics:get')
      .then((value) => {
        if (!cancelled) setDiagnostics(value as AppDiagnostics);
      })
      .catch((error) => {
        if (!cancelled) setDiagnosticsError(error instanceof Error ? error.message : String(error));
      });
    return () => {
      cancelled = true;
    };
  }, [activeTab]);

  const updateUi = useCallback((partial: Partial<AppConfig['ui']>) => {
    setDraft((prev) => ({ ...prev, ui: { ...prev.ui, ...partial } }));
  }, []);

  const updateOpenCode = useCallback((partial: Partial<OpenCodeModelConfig>) => {
    setDraft((prev) => ({ ...prev, opencode: { ...prev.opencode, ...partial } }));
  }, []);

  const updateNotifications = useCallback((partial: Partial<OsNotificationConfig>) => {
    setDraft((prev) => ({ ...prev, notifications: { ...prev.notifications, ...partial } }));
  }, []);

  const updateAcpModeToggle = useCallback((partial: Partial<AcpModeToggleShortcut>) => {
    setDraft((prev) => ({ ...prev, acpModeToggleShortcut: { ...prev.acpModeToggleShortcut, ...partial } }));
  }, []);

  const updateShortcut = useCallback((index: number, partial: Partial<TerminalShortcutEntry>) => {
    setDraft((prev) => {
      const next = [...prev.terminalShortcuts];
      next[index] = { ...next[index], ...partial };
      return { ...prev, terminalShortcuts: next };
    });
  }, []);

  const addShortcut = useCallback(() => {
    setDraft((prev) => ({
      ...prev,
      terminalShortcuts: [
        ...prev.terminalShortcuts,
        { id: `custom-${Date.now()}`, label: 'New Shortcut', key: 'F1', modifiers: { ctrl: false, alt: false, shift: false, command: false }, command: '', enabled: true },
      ],
    }));
  }, []);

  const removeShortcut = useCallback((index: number) => {
    setDraft((prev) => ({
      ...prev,
      terminalShortcuts: prev.terminalShortcuts.filter((_, i) => i !== index),
    }));
  }, []);

  const updateLauncher = useCallback((index: number, partial: Partial<LauncherEntry>) => {
    setDraft((prev) => {
      const next = [...prev.launchers];
      const current = next[index];
      if (!current) return prev;
      next[index] = { ...current, ...partial };
      return { ...prev, launchers: next };
    });
  }, []);

  const removeLauncher = useCallback((index: number) => {
    setDraft((prev) => {
      const launcher = prev.launchers[index];
      if (!launcher || launcher.builtin) return prev;
      return { ...prev, launchers: prev.launchers.filter((_, i) => i !== index) };
    });
  }, []);

  const addLauncher = useCallback(() => {
    const displayName = launcherDraft.displayName.trim();
    const launchCommand = launcherDraft.launchCommand.trim();
    if (!displayName || !launchCommand) return;
    setDraft((prev) => ({
      ...prev,
      launchers: [
        ...prev.launchers,
        {
          id: nextCustomLauncherId(prev.launchers),
          builtin: undefined,
          displayName,
          launchCommand,
          enabled: true,
          iconKey: launcherDraft.iconKey,
          bypassPermissions: false,
        },
      ],
    }));
    setLauncherDraft({ displayName: '', launchCommand: '', iconKey: LauncherIconKey.Terminal });
  }, [launcherDraft]);

  const setProjectSavedMessages = useCallback((ownerProjectId: number, messages: string[]) => {
    setDraft((prev) => ({
      ...prev,
      projects: replaceProjectSavedMessages(prev.projects, ownerProjectId, messages),
    }));
  }, []);

  const addProjectSavedMessage = useCallback((ownerProjectId: number) => {
    const draftText = savedMessageDrafts[ownerProjectId] ?? '';
    setDraft((prev) => {
      const owner = prev.projects.find((project) => project.id === ownerProjectId);
      if (!owner) return prev;
      const nextMessages = addSavedMessage(owner.savedMessages, draftText);
      if (nextMessages === owner.savedMessages) return prev;
      return {
        ...prev,
        projects: replaceProjectSavedMessages(prev.projects, ownerProjectId, nextMessages),
      };
    });
    setSavedMessageDrafts((prev) => ({ ...prev, [ownerProjectId]: '' }));
  }, [savedMessageDrafts]);

  const duplicateBuiltinStems = (() => {
    const counts = new Map<string, number>();
    for (const launcher of draft.launchers) {
      if (!launcher.builtin) continue;
      const stem = launcherCommandStem(launcher.launchCommand);
      if (!stem) continue;
      counts.set(stem, (counts.get(stem) ?? 0) + 1);
    }
    return new Set([...counts.entries()].filter(([, count]) => count > 1).map(([stem]) => stem));
  })();

  const handleSave = useCallback(() => {
    onSave(draft);
    onClose();
  }, [draft, onSave, onClose]);

  const handleResetShortcuts = useCallback(() => {
    setDraft((prev) => ({ ...prev, terminalShortcuts: getDefaultShortcuts() }));
  }, []);

  const handleKeyCapture = useCallback(
    (e: React.KeyboardEvent, index: number) => {
      e.preventDefault();
      const key = e.key;
      if (key === 'Escape') {
        setRecordingShortcutIndex(null);
        return;
      }
      if (key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
        // Single character key without modifiers - ignore for shortcuts
        return;
      }
      const onMac = navigator.platform.toLowerCase().includes('mac');
      const modifiers: ShortcutModifiers = {
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        command: onMac ? e.metaKey : false,
      };
      updateShortcut(index, { key, modifiers });
      setRecordingShortcutIndex(null);
    },
    [updateShortcut]
  );

  const handleAcpKeyCapture = useCallback(
    (e: React.KeyboardEvent) => {
      e.preventDefault();
      const key = e.key;
      if (key === 'Escape') {
        setRecordingAcpShortcut(false);
        return;
      }
      const onMac = navigator.platform.toLowerCase().includes('mac');
      const modifiers: ShortcutModifiers = {
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        command: onMac ? e.metaKey : false,
      };
      updateAcpModeToggle({ key, modifiers });
      setRecordingAcpShortcut(false);
    },
    [updateAcpModeToggle]
  );

  const duplicateShortcuts = (() => {
    const combos = new Map<string, number[]>();
    draft.terminalShortcuts.forEach((s, i) => {
      if (!s.enabled) return;
      const combo = `${s.key}|${s.modifiers.ctrl}|${s.modifiers.alt}|${s.modifiers.shift}|${s.modifiers.command}`;
      const existing = combos.get(combo) || [];
      existing.push(i);
      combos.set(combo, existing);
    });
    const dups: string[] = [];
    for (const [combo, indices] of combos) {
      if (indices.length > 1) {
        const keys = combo.split('|')[0];
        dups.push(`${indices.length}x ${keys}`);
      }
    }
    return dups;
  })();

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.6)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        style={{
          background: '#141414',
          border: '1px solid #333',
          borderRadius: 8,
          width: 720,
          maxWidth: '90vw',
          height: 600,
          maxHeight: '90vh',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
      >
        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px', borderBottom: '1px solid #222' }}>
          <span style={{ fontSize: 14, fontWeight: 600, color: '#eee' }}>Settings</span>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
            ✕
          </button>
        </div>

        {/* Tabs */}
        <div style={{ display: 'flex', gap: 2, padding: '8px 16px 0', borderBottom: '1px solid #222' }}>
          {tabs.map((t) => (
            <button
              key={t.id}
              onClick={() => setActiveTab(t.id)}
              style={{
                padding: '6px 12px',
                fontSize: 12,
                background: activeTab === t.id ? '#1a1a1a' : 'transparent',
                border: '1px solid',
                borderColor: activeTab === t.id ? '#333' : 'transparent',
                borderBottom: activeTab === t.id ? '1px solid #1a1a1a' : '1px solid transparent',
                color: activeTab === t.id ? '#eee' : '#888',
                cursor: 'pointer',
                borderRadius: '4px 4px 0 0',
                marginBottom: -1,
              }}
            >
              {t.label}
            </button>
          ))}
        </div>

        {/* Content */}
        <div style={{ flex: 1, overflow: 'auto', padding: '16px' }}>
          {activeTab === 'general' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>Main Area Visibility</div>
                <select
                  value={draft.ui.mainVisibilityMode}
                  onChange={(e) => updateUi({ mainVisibilityMode: e.target.value as MainVisibilityMode })}
                  style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                >
                  <option value="global">Global (all terminals)</option>
                  <option value="selected_project">Selected project only</option>
                </select>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.ui.multiTerminalViewEnabled}
                  onChange={(e) => updateUi({ multiTerminalViewEnabled: e.target.checked })}
                  id="multiTerminalView"
                />
                <label htmlFor="multiTerminalView" style={{ fontSize: 12, color: '#ccc' }}>Show multiple terminals in grid (tile view)</label>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>Default Shell</div>
                <select
                  value={draft.defaultShell}
                  onChange={(e) => setDraft((prev) => ({ ...prev, defaultShell: e.target.value as any }))}
                  style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                >
                  <option value="powershell">PowerShell</option>
                  <option value="cmd">CMD</option>
                  <option value="zsh">zsh</option>
                </select>
              </div>
              <div style={{ padding: 10, background: '#1a1a1a', border: '1px solid #262626', borderRadius: 6 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <input
                    type="checkbox"
                    checked={draft.claudeCodeCodexHookEnabled}
                    onChange={(e) => setDraft((prev) => ({ ...prev, claudeCodeCodexHookEnabled: e.target.checked }))}
                    id="claudeCodeCodexHookEnabled"
                  />
                  <label htmlFor="claudeCodeCodexHookEnabled" style={{ fontSize: 12, color: '#ccc', fontWeight: 600 }}>Allow Claude Code Codex Plan route</label>
                </div>
                <div style={{ fontSize: 11, color: '#888', marginTop: 6, lineHeight: 1.4 }}>
                  Lets Auto or the Codex mode run Codex planning before Mergen-submitted Claude Code prompts.
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>ACP Startup Mode</div>
                <select
                  value={draft.acpStartupMode}
                  onChange={(e) => setDraft((prev) => ({ ...prev, acpStartupMode: e.target.value as AcpStartupMode }))}
                  style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                >
                  <option value="build">{AcpStartupModeLabel.build}</option>
                  <option value="plan">{AcpStartupModeLabel.plan}</option>
                </select>
              </div>
            </div>
          )}

          {activeTab === 'launchers' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>Foreground Launchers</div>
              {draft.launchers.map((launcher, i) => {
                const isBuiltin = Boolean(launcher.builtin);
                const isClaude = launcher.builtin === BuiltinLauncherKind.Claude || launcher.builtin === BuiltinLauncherKind.ClaudeAcp;
                const stem = launcherCommandStem(launcher.launchCommand);
                const displayNameMissing = launcher.displayName.trim().length === 0;
                const commandMissing = launcher.launchCommand.trim().length === 0;
                const duplicateBuiltinCommand = Boolean(launcher.builtin && stem && duplicateBuiltinStems.has(stem));
                const warning = duplicateBuiltinCommand
                  ? 'Built-in commands must stay unique.'
                  : commandMissing
                    ? 'Command cannot be empty.'
                    : displayNameMissing
                      ? 'Menu label cannot be empty.'
                      : '';
                return (
                  <div key={launcher.id} style={{ display: 'flex', gap: 10, padding: 10, background: '#1a1a1a', border: '1px solid #262626', borderRadius: 6 }}>
                    <div
                      title={LauncherIconKeyLabel[launcher.iconKey]}
                      style={{
                        width: 30,
                        height: 30,
                        borderRadius: 6,
                        background: launcherIconColor(launcher.iconKey),
                        color: '#f2f2f2',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        fontSize: 11,
                        fontWeight: 700,
                        flex: '0 0 auto',
                      }}
                    >
                      {LauncherIconKeyLabel[launcher.iconKey].slice(0, 2)}
                    </div>
                    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 8, minWidth: 0 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>{launcher.displayName || 'Unnamed'}</span>
                        <span style={{ fontSize: 10, color: '#888' }}>{isBuiltin ? 'Built-in' : 'Custom'}</span>
                        {isClaude && <span style={{ fontSize: 10, color: '#a8c7ff' }}>Bypass permissions</span>}
                      </div>
                      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(120px, 0.36fr) minmax(180px, 1fr)', gap: 8 }}>
                        <div>
                          <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Menu label</div>
                          <input
                            value={launcher.displayName}
                            onChange={(e) => updateLauncher(i, { displayName: e.target.value })}
                            placeholder="Launcher name"
                            style={{ width: '100%', background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                          />
                        </div>
                        <div>
                          <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Command to type and submit</div>
                          <input
                            value={launcher.launchCommand}
                            onChange={(e) => updateLauncher(i, { launchCommand: e.target.value })}
                            placeholder="Example: codex, claude.cmd, droid, opencode"
                            style={{ width: '100%', background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                          />
                        </div>
                      </div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
                        <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#ccc' }}>
                          <input
                            type="checkbox"
                            checked={launcher.enabled}
                            onChange={(e) => updateLauncher(i, { enabled: e.target.checked })}
                          />
                          Show in foreground launcher menu
                        </label>
                        {isClaude && (
                          <label title="Claude launches always use bypass permissions" style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#777' }}>
                            <input type="checkbox" checked readOnly disabled />
                            Bypass permissions
                          </label>
                        )}
                        {!isBuiltin && (
                          <select
                            value={launcher.iconKey}
                            onChange={(e) => updateLauncher(i, { iconKey: e.target.value as LauncherIconKey })}
                            style={{ background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                          >
                            {LauncherIconKeyCustomPresets.map((iconKey) => (
                              <option key={iconKey} value={iconKey}>{LauncherIconKeyLabel[iconKey]}</option>
                            ))}
                          </select>
                        )}
                        {isBuiltin && (
                          <button
                            onClick={() => {
                              const builtin = launcher.builtin!;
                              updateLauncher(i, {
                                displayName: BuiltinLauncherKindDefaultDisplayName(builtin),
                                launchCommand: BuiltinLauncherKindDefaultLaunchCommand(builtin),
                              });
                            }}
                            style={{ padding: '4px 10px', fontSize: 11, background: '#0c0c0c', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}
                          >
                            Reset
                          </button>
                        )}
                        {!isBuiltin && (
                          <button
                            onClick={() => removeLauncher(i)}
                            style={{ padding: '4px 10px', fontSize: 11, background: 'transparent', border: '1px solid #4a2a2a', color: '#d47a7a', borderRadius: 4, cursor: 'pointer', marginLeft: 'auto' }}
                          >
                            Remove
                          </button>
                        )}
                      </div>
                      {warning && <div style={{ fontSize: 11, color: '#dcb43c' }}>{warning}</div>}
                    </div>
                  </div>
                );
              })}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 10, background: '#161616', border: '1px solid #262626', borderRadius: 6 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>Add Custom Launcher</div>
                <div style={{ display: 'grid', gridTemplateColumns: 'minmax(120px, 0.32fr) minmax(180px, 1fr) 120px auto', gap: 8, alignItems: 'end' }}>
                  <div>
                    <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Menu label</div>
                    <input
                      value={launcherDraft.displayName}
                      onChange={(e) => setLauncherDraft((prev) => ({ ...prev, displayName: e.target.value }))}
                      placeholder="Example: Gemini"
                      style={{ width: '100%', background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                    />
                  </div>
                  <div>
                    <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Command to type and submit</div>
                    <input
                      value={launcherDraft.launchCommand}
                      onChange={(e) => setLauncherDraft((prev) => ({ ...prev, launchCommand: e.target.value }))}
                      placeholder="Example: gemini"
                      style={{ width: '100%', background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                    />
                  </div>
                  <select
                    value={launcherDraft.iconKey}
                    onChange={(e) => setLauncherDraft((prev) => ({ ...prev, iconKey: e.target.value as LauncherIconKey }))}
                    style={{ background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                  >
                    {LauncherIconKeyCustomPresets.map((iconKey) => (
                      <option key={iconKey} value={iconKey}>{LauncherIconKeyLabel[iconKey]}</option>
                    ))}
                  </select>
                  <button
                    onClick={addLauncher}
                    disabled={!launcherDraft.displayName.trim() || !launcherDraft.launchCommand.trim()}
                    style={{
                      padding: '5px 12px',
                      fontSize: 11,
                      background: launcherDraft.displayName.trim() && launcherDraft.launchCommand.trim() ? '#1f3a4c' : '#181818',
                      border: '1px solid #333',
                      color: '#ccc',
                      borderRadius: 4,
                      cursor: launcherDraft.displayName.trim() && launcherDraft.launchCommand.trim() ? 'pointer' : 'not-allowed',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    + Add
                  </button>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'opencode' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>Build Model Slot A</div>
                <input
                  value={draft.opencode.buildModelSlotA}
                  onChange={(e) => updateOpenCode({ buildModelSlotA: e.target.value })}
                  style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                />
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>Build Model Slot B</div>
                <input
                  value={draft.opencode.buildModelSlotB}
                  onChange={(e) => updateOpenCode({ buildModelSlotB: e.target.value })}
                  style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                />
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontSize: 12, color: '#eee' }}>Active Slot:</span>
                <button
                  onClick={() => updateOpenCode({ activeBuildModelSlot: 'a' })}
                  style={{ padding: '4px 12px', fontSize: 11, background: draft.opencode.activeBuildModelSlot === 'a' ? '#1f3a4c' : '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}
                >
                  A
                </button>
                <button
                  onClick={() => updateOpenCode({ activeBuildModelSlot: 'b' })}
                  style={{ padding: '4px 12px', fontSize: 11, background: draft.opencode.activeBuildModelSlot === 'b' ? '#1f3a4c' : '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}
                >
                  B
                </button>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>Plan Model</div>
                <input
                  value={draft.opencode.planModel}
                  onChange={(e) => updateOpenCode({ planModel: e.target.value })}
                  style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                />
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>Plan Effort</div>
                <input
                  value={draft.opencode.planEffort}
                  onChange={(e) => updateOpenCode({ planEffort: e.target.value })}
                  style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                />
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.opencode.acpBindModelToMode}
                  onChange={(e) => updateOpenCode({ acpBindModelToMode: e.target.checked })}
                  id="acpBind"
                />
                <label htmlFor="acpBind" style={{ fontSize: 12, color: '#ccc' }}>Bind ACP model to mode</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.opencode.loopProtectionEnabled}
                  onChange={(e) => updateOpenCode({ loopProtectionEnabled: e.target.checked })}
                  id="loopProtection"
                />
                <label htmlFor="loopProtection" style={{ fontSize: 12, color: '#ccc' }}>Kimi thought-loop protection</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.opencode.kimiStrictPermissions}
                  onChange={(e) => updateOpenCode({ kimiStrictPermissions: e.target.checked })}
                  id="kimiStrict"
                />
                <label htmlFor="kimiStrict" style={{ fontSize: 12, color: '#ccc' }}>Kimi strict permissions (ask mode)</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.opencode.acpAutoApprovePermissions}
                  onChange={(e) => updateOpenCode({ acpAutoApprovePermissions: e.target.checked })}
                  id="autoApprove"
                />
                <label htmlFor="autoApprove" style={{ fontSize: 12, color: '#ccc' }}>Auto-approve ACP permissions</label>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>{OPENCODE_ACP_LABEL} Mode Toggle Shortcut</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <input
                    type="checkbox"
                    checked={draft.acpModeToggleShortcut.enabled}
                    onChange={(e) => updateAcpModeToggle({ enabled: e.target.checked })}
                    id="acpToggleEnabled"
                  />
                  <label htmlFor="acpToggleEnabled" style={{ fontSize: 12, color: '#ccc' }}>Enabled</label>
                  <button
                    onClick={() => setRecordingAcpShortcut(true)}
                    onKeyDown={recordingAcpShortcut ? handleAcpKeyCapture : undefined}
                    style={{ padding: '4px 12px', fontSize: 11, background: recordingAcpShortcut ? '#1f3a4c' : '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer', outline: recordingAcpShortcut ? '1px solid #0078d4' : 'none' }}
                  >
                    {recordingAcpShortcut ? 'Press key...' : formatShortcut(draft.acpModeToggleShortcut.key, draft.acpModeToggleShortcut.modifiers)}
                  </button>
                  {recordingAcpShortcut && (
                    <span style={{ fontSize: 11, color: '#888' }}>Press Esc to cancel</span>
                  )}
                </div>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>{OPENCODE_ACP_LABEL} Favorite Models</div>
                {draft.opencode.acpKnownModels.length === 0 ? (
                  <div style={{ fontSize: 11, color: '#666' }}>No known models yet. Open {OPENCODE_ACP_LABEL} to populate this list.</div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {draft.opencode.acpKnownModels.map((model) => {
                      const isFavorite = draft.opencode.acpFavoriteModels.includes(model.value);
                      return (
                        <div key={model.value} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 8px', background: '#1a1a1a', borderRadius: 4 }}>
                          <button
                            onClick={() => {
                              const next = isFavorite
                                ? draft.opencode.acpFavoriteModels.filter((v) => v !== model.value)
                                : [...draft.opencode.acpFavoriteModels, model.value];
                              updateOpenCode({ acpFavoriteModels: next });
                            }}
                            style={{ background: 'transparent', border: 'none', color: isFavorite ? '#dcB43C' : '#666', cursor: 'pointer', fontSize: 12 }}
                            title={isFavorite ? 'Unfavorite' : 'Favorite'}
                          >
                            {isFavorite ? '★' : '☆'}
                          </button>
                          <span style={{ fontSize: 12, color: '#ccc', flex: 1 }}>{model.name || model.value}</span>
                          <span style={{ fontSize: 10, color: '#666' }}>{model.value}</span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </div>
          )}

          {activeTab === 'saved' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {draft.projects.length === 0 ? (
                <div style={{ padding: 10, background: '#1a1a1a', border: '1px solid #262626', borderRadius: 6, fontSize: 12, color: '#888' }}>
                  Add a project from the Directory panel to manage saved messages here.
                </div>
              ) : (
                draft.projects.map((project) => {
                  const ownerProjectId = savedMessageOwnerProjectId(draft.projects, project);
                  const ownerProject = draft.projects.find((candidate) => candidate.id === ownerProjectId) ?? project;
                  const messages = ownerProject.savedMessages;
                  const draftText = savedMessageDrafts[ownerProjectId] ?? '';
                  const inherited = ownerProjectId !== project.id;
                  return (
                    <div key={project.id} style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 10, background: '#1a1a1a', border: '1px solid #262626', borderRadius: 6 }}>
                      <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
                        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>{project.name}</span>
                        <span style={{ fontSize: 10, color: '#888' }}>{messages.length} saved</span>
                        {inherited && <span style={{ fontSize: 10, color: '#888' }}>Uses {ownerProject.name}</span>}
                      </div>
                      <div style={{ fontSize: 10, color: '#666', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{project.path}</div>
                      {messages.length === 0 ? (
                        <div style={{ fontSize: 11, color: '#777' }}>No saved messages for this project.</div>
                      ) : (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                          {messages.map((message, index) => (
                            <div key={`${ownerProjectId}-${index}`} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                              <input
                                value={message}
                                onChange={(e) => setProjectSavedMessages(ownerProjectId, updateSavedMessage(messages, index, e.target.value))}
                                style={{ flex: 1, background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                              />
                              <button
                                onClick={() => setProjectSavedMessages(ownerProjectId, removeSavedMessage(messages, index))}
                                style={{ padding: '4px 8px', fontSize: 11, background: 'transparent', border: '1px solid #4a2a2a', color: '#d47a7a', borderRadius: 4, cursor: 'pointer' }}
                              >
                                Remove
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                      <div style={{ display: 'flex', gap: 8 }}>
                        <input
                          value={draftText}
                          onChange={(e) => setSavedMessageDrafts((prev) => ({ ...prev, [ownerProjectId]: e.target.value }))}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault();
                              addProjectSavedMessage(ownerProjectId);
                            }
                          }}
                          placeholder="Add a saved message for this project"
                          style={{ flex: 1, background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                        />
                        <button
                          onClick={() => addProjectSavedMessage(ownerProjectId)}
                          disabled={!draftText.trim()}
                          style={{
                            padding: '4px 12px',
                            fontSize: 11,
                            background: draftText.trim() ? '#1f3a4c' : '#181818',
                            border: '1px solid #333',
                            color: '#ccc',
                            borderRadius: 4,
                            cursor: draftText.trim() ? 'pointer' : 'not-allowed',
                          }}
                        >
                          + Add
                        </button>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          )}

          {activeTab === 'shortcuts' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {duplicateShortcuts.length > 0 && (
                <div style={{ padding: '8px 12px', background: 'rgba(200,60,60,0.15)', border: '1px solid #c44', borderRadius: 4, fontSize: 11, color: '#c44' }}>
                  Duplicate shortcuts detected: {duplicateShortcuts.join(', ')}
                </div>
              )}
              <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
                <button onClick={addShortcut} style={{ padding: '4px 12px', fontSize: 11, background: '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}>
                  + Add Shortcut
                </button>
                <button onClick={handleResetShortcuts} style={{ padding: '4px 12px', fontSize: 11, background: '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}>
                  Reset Defaults
                </button>
              </div>
              {draft.terminalShortcuts.map((shortcut, i) => (
                <div key={shortcut.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px', background: '#1a1a1a', borderRadius: 4 }}>
                  <input
                    type="checkbox"
                    checked={shortcut.enabled}
                    onChange={(e) => updateShortcut(i, { enabled: e.target.checked })}
                  />
                  <input
                    value={shortcut.label}
                    onChange={(e) => updateShortcut(i, { label: e.target.value })}
                    placeholder="Label"
                    style={{ width: 120, background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                  />
                  <button
                    onClick={() => setRecordingShortcutIndex(i)}
                    onKeyDown={recordingShortcutIndex === i ? (e) => handleKeyCapture(e, i) : undefined}
                    style={{ padding: '4px 12px', fontSize: 11, background: recordingShortcutIndex === i ? '#1f3a4c' : '#0c0c0c', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer', outline: recordingShortcutIndex === i ? '1px solid #0078d4' : 'none' }}
                  >
                    {recordingShortcutIndex === i ? 'Press key...' : formatShortcut(shortcut.key, shortcut.modifiers)}
                  </button>
                  {recordingShortcutIndex === i && (
                    <span style={{ fontSize: 11, color: '#888' }}>Press Esc to cancel</span>
                  )}
                  <input
                    value={shortcut.command}
                    onChange={(e) => updateShortcut(i, { command: e.target.value })}
                    placeholder="Command"
                    style={{ flex: 1, background: '#0c0c0c', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 11, borderRadius: 4 }}
                  />
                  <button onClick={() => removeShortcut(i)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 12 }}>
                    🗑
                  </button>
                </div>
              ))}
            </div>
          )}

          {activeTab === 'notifications' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.notifications.enabled}
                  onChange={(e) => updateNotifications({ enabled: e.target.checked })}
                  id="notifEnabled"
                />
                <label htmlFor="notifEnabled" style={{ fontSize: 12, color: '#ccc' }}>Enable OS notifications</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.notifications.onlyWhenUnfocused}
                  onChange={(e) => updateNotifications({ onlyWhenUnfocused: e.target.checked })}
                  id="notifUnfocused"
                />
                <label htmlFor="notifUnfocused" style={{ fontSize: 12, color: '#ccc' }}>Only when unfocused</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.notifications.onPermission}
                  onChange={(e) => updateNotifications({ onPermission: e.target.checked })}
                  id="notifPermission"
                />
                <label htmlFor="notifPermission" style={{ fontSize: 12, color: '#ccc' }}>On permission requests</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.notifications.onTurnComplete}
                  onChange={(e) => updateNotifications({ onTurnComplete: e.target.checked })}
                  id="notifTurn"
                />
                <label htmlFor="notifTurn" style={{ fontSize: 12, color: '#ccc' }}>On turn complete</label>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <input
                  type="checkbox"
                  checked={draft.notifications.onSessionError}
                  onChange={(e) => updateNotifications({ onSessionError: e.target.checked })}
                  id="notifError"
                />
                <label htmlFor="notifError" style={{ fontSize: 12, color: '#ccc' }}>On session error</label>
              </div>
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Cooldown (seconds)</div>
                <input
                  type="number"
                  value={draft.notifications.cooldownSecs}
                  onChange={(e) => updateNotifications({ cooldownSecs: Math.max(0, parseInt(e.target.value, 10) || 0) })}
                  style={{ width: 80, background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '4px 8px', fontSize: 12, borderRadius: 4 }}
                />
              </div>
            </div>
          )}

          {activeTab === 'diagnostics' && (() => {
            const overview = runtimeOverview(diagnostics, activeTerminal);
            const overviewColor = diagnosticsColor(overview.severity);
            const hookStatus = diagnostics
              ? diagnostics.hookServicePort > 0
                ? `Listening on 127.0.0.1:${diagnostics.hookServicePort}`
                : 'Not listening'
              : 'Loading';
            const codexHooksStatus = diagnostics
              ? diagnostics.codexHooksInstalled ? 'Installed' : 'Not installed'
              : 'Loading';
            const browserMcpStatus = diagnostics
              ? `${diagnostics.browserMcpSessionCount} active session${diagnostics.browserMcpSessionCount === 1 ? '' : 's'}`
              : 'Loading';
            return (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                <DiagnosticsCard
                  title="Runtime Overview"
                  subtitle="Get a quick read on integration health before opening the verbose diagnostics."
                >
                  <DiagnosticValue value={overview.title} color={overviewColor} strong />
                  <DiagnosticValue value={diagnosticsError || overview.message} color={diagnosticsError ? '#dcaa3c' : '#888'} />
                  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(230px, 1fr))', gap: 12, marginTop: 10 }}>
                    <div>
                      <div style={{ fontSize: 11, fontWeight: 600, color: '#eee', marginBottom: 6 }}>Hook Bridge</div>
                      <DiagnosticRow label="Inbox" value={diagnostics?.hookInboxDir ?? 'Loading'} color={diagnostics?.hookInboxDir ? '#64c38c' : '#888'} />
                      <DiagnosticRow label="Service" value={hookStatus} color={diagnostics && diagnostics.hookServicePort > 0 ? '#64c38c' : '#dcaa3c'} />
                    </div>
                    <div>
                      <div style={{ fontSize: 11, fontWeight: 600, color: '#eee', marginBottom: 6 }}>Codex CLI</div>
                      <DiagnosticRow label="Inbox" value={diagnostics?.codexInboxDir ?? 'Loading'} color={diagnostics?.codexInboxDir ? '#64c38c' : '#888'} />
                      <DiagnosticRow label="Hooks" value={codexHooksStatus} color={diagnostics?.codexHooksInstalled ? '#64c38c' : '#dcaa3c'} />
                    </div>
                  </div>
                </DiagnosticsCard>

                <DiagnosticsCard
                  title="Session State"
                  subtitle="Inspect the currently selected terminal and browser bridge status."
                >
                  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(230px, 1fr))', gap: 12 }}>
                    <div>
                      <div style={{ fontSize: 11, fontWeight: 600, color: '#eee', marginBottom: 6 }}>Active Terminal</div>
                      <DiagnosticRow label="Terminal" value={activeTerminal ? `#${activeTerminal.id} ${activeTerminal.title || ''}`.trim() : 'No active terminal'} />
                      <DiagnosticRow label="Kind" value={activeTerminal?.kind ?? 'None'} />
                      <DiagnosticRow label="AI Tool" value={activeTerminal?.aiTool ?? 'None'} />
                      <DiagnosticRow label="AI Status" value={activeTerminal?.aiStatus ?? 'Inactive'} />
                      <DiagnosticRow label="OpenCode" value={activeTerminal?.opencodeTransportStatus ?? (activeTerminal?.opencodeSessionActive ? 'Active' : 'Inactive')} />
                    </div>
                    <div>
                      <div style={{ fontSize: 11, fontWeight: 600, color: '#eee', marginBottom: 6 }}>Browser MCP</div>
                      <DiagnosticRow label="Status" value={browserMcpStatus} color={diagnostics && diagnostics.browserMcpSessionCount > 0 ? '#64c38c' : '#888'} />
                      <DiagnosticRow label="Command" value={diagnostics?.browserMcpCommand.join(' ') ?? 'Loading'} />
                      <DiagnosticRow label="Bridge" value={diagnostics?.codexBridgeInstalled ? 'Installed' : diagnostics ? 'Not installed' : 'Loading'} color={diagnostics?.codexBridgeInstalled ? '#64c38c' : '#888'} />
                    </div>
                  </div>
                </DiagnosticsCard>

                <div style={{ background: '#1a1a1a', border: '1px solid #262626', borderRadius: 6, overflow: 'hidden' }}>
                  <button
                    onClick={() => setDiagnosticsExpanded((value) => !value)}
                    style={{
                      width: '100%',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '10px 12px',
                      background: 'transparent',
                      border: 'none',
                      color: '#eee',
                      cursor: 'pointer',
                      fontSize: 12,
                      fontWeight: 600,
                    }}
                  >
                    <span>Technical Details</span>
                    <span style={{ color: '#888', fontSize: 11 }}>{diagnosticsExpanded ? 'Hide' : 'Show'}</span>
                  </button>
                  {diagnosticsExpanded && (
                    <div style={{ borderTop: '1px solid #262626', padding: 12, display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <DiagnosticRow label="App Version" value={diagnostics?.appVersion ?? 'Loading'} />
                      <DiagnosticRow label="Platform" value={diagnostics ? `${diagnostics.platform} ${diagnostics.arch}` : 'Loading'} />
                      <DiagnosticRow label="Electron" value={diagnostics?.electronVersion ?? 'Loading'} />
                      <DiagnosticRow label="Chrome" value={diagnostics?.chromeVersion ?? 'Loading'} />
                      <DiagnosticRow label="Node" value={diagnostics?.nodeVersion ?? 'Loading'} />
                      <DiagnosticRow label="Executable Path" value={diagnostics?.execPath ?? 'Loading'} />
                      <DiagnosticRow label="Working Directory" value={diagnostics?.cwd ?? 'Loading'} />
                      <DiagnosticRow label="Config Path" value={diagnostics?.configPath ?? 'Loading'} />
                      <DiagnosticRow label="Legacy Config Path" value={diagnostics?.legacyConfigPath ?? 'Loading'} />
                      <DiagnosticRow label="History Path" value={diagnostics?.historyPath ?? 'Loading'} />
                      <DiagnosticRow label="Hook Inbox" value={diagnostics?.hookInboxDir ?? 'Loading'} />
                      <DiagnosticRow label="Codex Inbox" value={diagnostics?.codexInboxDir ?? 'Loading'} />
                      <DiagnosticRow label="Codex Hooks Path" value={diagnostics?.codexHooksPath ?? 'Loading'} />
                      <DiagnosticRow label="Codex Bridge Path" value={diagnostics?.codexBridgePath ?? 'Loading'} />
                      <DiagnosticRow label="Active Terminal CWD" value={activeTerminal?.cwd ?? 'No active terminal'} />
                      <DiagnosticRow label="Active Terminal Reason" value={activeTerminal?.opencodeAttentionReason || activeTerminal?.aiStatusReason || 'None'} />
                    </div>
                  )}
                </div>
              </div>
            );
          })()}
        </div>

        {/* Footer */}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, padding: '12px 16px', borderTop: '1px solid #222' }}>
          <button onClick={onClose} style={{ padding: '6px 16px', fontSize: 12, background: 'transparent', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}>
            Cancel
          </button>
          <button onClick={handleSave} style={{ padding: '6px 16px', fontSize: 12, background: '#1f3a4c', border: '1px solid #1f3a4c', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}>
            Save
          </button>
        </div>
      </div>
    </div>
  );
};

function DiagnosticsCard({ title, subtitle, children }: { title: string; subtitle: string; children: React.ReactNode }) {
  return (
    <div style={{ padding: 12, background: '#1a1a1a', border: '1px solid #262626', borderRadius: 6 }}>
      <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>{title}</div>
      <div style={{ fontSize: 11, color: '#888', marginBottom: 10 }}>{subtitle}</div>
      {children}
    </div>
  );
}

function DiagnosticValue({ value, color = '#ccc', strong = false }: { value: string; color?: string; strong?: boolean }) {
  return (
    <div
      title={value}
      style={{
        fontSize: strong ? 12 : 11,
        fontWeight: strong ? 600 : 400,
        color,
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
      }}
    >
      {value}
    </div>
  );
}

function DiagnosticRow({ label, value, color = '#ccc' }: { label: string; value: string; color?: string }) {
  const copy = useCallback(() => {
    api.invoke('clipboard:writeText', value).catch(() => {});
  }, [value]);

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '132px minmax(0, 1fr) auto', gap: 8, alignItems: 'center', minHeight: 24 }}>
      <span style={{ fontSize: 11, color: '#888' }}>{label}</span>
      <span
        title={value}
        style={{
          fontSize: 11,
          color,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}
      >
        {value}
      </span>
      <button
        onClick={copy}
        disabled={!value || value === 'Loading'}
        style={{
          padding: '2px 7px',
          fontSize: 10,
          background: '#0c0c0c',
          border: '1px solid #333',
          color: '#888',
          borderRadius: 4,
          cursor: value && value !== 'Loading' ? 'pointer' : 'not-allowed',
        }}
      >
        Copy
      </button>
    </div>
  );
}

function nextCustomLauncherId(launchers: LauncherEntry[]): string {
  let suffix = launchers.length + 1;
  let candidate = `custom-${suffix}`;
  while (launchers.some((launcher) => launcher.id === candidate)) {
    suffix += 1;
    candidate = `custom-${suffix}`;
  }
  return candidate;
}

function launcherCommandStem(command: string): string | null {
  const trimmed = command.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith('"') || trimmed.startsWith("'")) {
    const quote = trimmed[0];
    const end = trimmed.indexOf(quote, 1);
    if (end > 1) return commandBasename(trimmed.slice(1, end));
  }
  const firstToken = trimmed.split(/\s+/)[0];
  return firstToken ? commandBasename(firstToken) : null;
}

function commandBasename(command: string): string {
  const withoutExtension = command.replace(/\.(cmd|exe|ps1|bat)$/i, '');
  const parts = withoutExtension.split(/[\\/]/);
  return (parts[parts.length - 1] || withoutExtension).toLowerCase();
}

function launcherIconColor(iconKey: LauncherIconKey): string {
  switch (iconKey) {
    case LauncherIconKey.Codex:
      return '#3b4f6f';
    case LauncherIconKey.Claude:
      return '#6d4a35';
    case LauncherIconKey.Droid:
      return '#385f4a';
    case LauncherIconKey.OpenCode:
      return '#315d66';
    case LauncherIconKey.Terminal:
      return '#404040';
    case LauncherIconKey.Spark:
      return '#735c2e';
    case LauncherIconKey.Message:
      return '#4d5b76';
    case LauncherIconKey.Bot:
      return '#445f46';
    case LauncherIconKey.Code:
      return '#4b5574';
    case LauncherIconKey.Wrench:
      return '#5c5963';
    case LauncherIconKey.Rocket:
      return '#693f52';
  }
}

function formatShortcut(key: string, modifiers: ShortcutModifiers): string {
  const parts: string[] = [];
  if (modifiers.ctrl) parts.push('Ctrl');
  if (modifiers.alt) parts.push('Alt');
  if (modifiers.shift) parts.push('Shift');
  if (modifiers.command) parts.push('Cmd');
  parts.push(key);
  return parts.join('+');
}

function getDefaultShortcuts(): TerminalShortcutEntry[] {
  return [
    { id: 'github-push', label: 'GitHub Push', key: 'F5', modifiers: { ctrl: false, alt: false, shift: false, command: false }, command: '/gt', enabled: true },
    { id: 'prepare-fix-plan', label: 'Prepare Fix Plan', key: 'F6', modifiers: { ctrl: false, alt: false, shift: false, command: false }, command: '/prepare-fix-plan', enabled: true },
    { id: 'implement-plan', label: 'Implement Plan', key: 'F11', modifiers: { ctrl: false, alt: false, shift: false, command: false }, command: '/implement-plan', enabled: true },
    { id: 'review-guard', label: 'Review Guard', key: 'F7', modifiers: { ctrl: false, alt: false, shift: false, command: false }, command: '/review-guard', enabled: true },
  ];
}
