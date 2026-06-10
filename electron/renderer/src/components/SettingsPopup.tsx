import React, { useState, useCallback } from 'react';
import type {
  AppConfig,
  TerminalShortcutEntry,
  ShortcutModifiers,
  OpenCodeModelConfig,
  OsNotificationConfig,
  AcpModeToggleShortcut,
  AcpStartupMode,
  MainVisibilityMode,
} from '../../../shared/types';
import { AcpStartupModeLabel } from '../../../shared/types';

interface SettingsPopupProps {
  config: AppConfig;
  onSave: (config: AppConfig) => void;
  onClose: () => void;
}

type SettingsTab = 'general' | 'opencode' | 'shortcuts' | 'notifications';

const tabs: { id: SettingsTab; label: string }[] = [
  { id: 'general', label: 'General' },
  { id: 'opencode', label: 'OpenCode' },
  { id: 'shortcuts', label: 'Shortcuts' },
  { id: 'notifications', label: 'Notifications' },
];

export const SettingsPopup: React.FC<SettingsPopupProps> = ({ config, onSave, onClose }) => {
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [draft, setDraft] = useState<AppConfig>({ ...config });
  const [recordingShortcutIndex, setRecordingShortcutIndex] = useState<number | null>(null);
  const [recordingAcpShortcut, setRecordingAcpShortcut] = useState(false);

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
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>ACP Mode Toggle Shortcut</div>
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
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 8 }}>ACP Favorite Models</div>
                {draft.opencode.acpKnownModels.length === 0 ? (
                  <div style={{ fontSize: 11, color: '#666' }}>No known models yet. Open an ACP chat to populate this list.</div>
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
