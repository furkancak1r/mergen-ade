import React, { useState, useCallback, useEffect, useRef } from 'react';
import type { SmartInputState, SmartInputTask, SmartInputAttachment, OpenCodeQuestion } from '../../../shared/types';
import { removeMentionFromInput } from '../lib/smartInput';
import { shouldReadNativeClipboardFilePaths, shouldReadNativeClipboardImage, snapshotClipboardPaste } from '../lib/clipboardPaste';
import type { SmartInputModeId } from '../lib/smartInputMode';
import { smartInputModeLabel, toggleSmartInputModeId } from '../lib/smartInputMode';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface SmartInputFooterProps {
  terminalId: number;
  state: SmartInputState;
  question?: OpenCodeQuestion;
  questionFocusIndex: number;
  questionSelectedOptions: string[];
  questionCustomText: string;
  onUpdateState: (state: SmartInputState) => void;
  onSendToTerminal: (terminalId: number, text: string, attachments: SmartInputAttachment[], modeId: SmartInputModeId) => void;
  onUpdateQuestionState?: (updates: { focusIndex?: number; selectedOptions?: string[]; customText?: string }) => void;
  onClearTerminalOutputFocusOverride?: () => void;
  modeControlsVisible?: boolean;
  disabled?: boolean;
}

export const SmartInputFooter: React.FC<SmartInputFooterProps> = ({
  terminalId,
  state,
  question,
  questionFocusIndex,
  questionSelectedOptions,
  questionCustomText,
  onUpdateState,
  onSendToTerminal,
  onUpdateQuestionState,
  onClearTerminalOutputFocusOverride,
  modeControlsVisible = true,
  disabled = false,
}) => {
  const [draftMode, setDraftMode] = useState<SmartInputModeId>('auto');
  const [editRestoreIndex, setEditRestoreIndex] = useState<number | null>(null);
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const customInputRef = useRef<HTMLInputElement>(null);

  const updateDraft = useCallback((text: string) => {
    onUpdateState({ ...state, draftText: text });
  }, [state, onUpdateState]);

  const addTask = useCallback(() => {
    if (!state.draftText.trim() && state.draftAttachments.length === 0) return;
    const task: SmartInputTask = {
      text: state.draftText.trim(),
      attachments: [...state.draftAttachments],
      modeId: modeControlsVisible ? draftMode : 'build',
      afterDone: true,
    };
    const queue = [...state.queue];
    if (editRestoreIndex !== null && editRestoreIndex >= 0 && editRestoreIndex <= queue.length) {
      queue.splice(editRestoreIndex, 0, task);
    } else {
      queue.push(task);
    }
    onUpdateState({
      ...state,
      queue,
      draftText: '',
      draftAttachments: [],
    });
    setDraftMode('auto');
    setEditRestoreIndex(null);
  }, [state, draftMode, modeControlsVisible, editRestoreIndex, onUpdateState]);

  const steerTask = useCallback((index: number) => {
    const task = state.queue[index];
    if (!task) return;
    onSendToTerminal(terminalId, task.text, task.attachments, task.modeId as SmartInputModeId);
    const queue = state.queue.filter((_, i) => i !== index);
    onUpdateState({ ...state, queue });
  }, [state, terminalId, onSendToTerminal, onUpdateState]);

  const removeTask = useCallback((index: number) => {
    const queue = state.queue.filter((_, i) => i !== index);
    onUpdateState({ ...state, queue });
  }, [state, onUpdateState]);

  const moveTask = useCallback((fromIndex: number, toIndex: number) => {
    const queue = [...state.queue];
    const [moved] = queue.splice(fromIndex, 1);
    queue.splice(toIndex, 0, moved);
    onUpdateState({ ...state, queue });
  }, [state, onUpdateState]);

  const submit = useCallback(() => {
    addTask();
  }, [addTask]);

  // Footer resize handling (removed - height is now fixed based on queue)

  // Question keyboard handling
  useEffect(() => {
    if (disabled) return;
    if (!question) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      // Do not steal keyboard events from other text inputs such as the file editor.
      const active = document.activeElement;
      if (active) {
        const tag = active.tagName.toLowerCase();
        const isTextInput = tag === 'textarea' || tag === 'input' || (active as HTMLElement).isContentEditable;
        if (isTextInput && active !== customInputRef.current) {
          return;
        }
      }

      const options = question.options;
      const hasCustom = question.custom;
      const totalRows = options.length + (hasCustom ? 1 : 0);

      if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') {
        e.preventDefault();
        const next = Math.max(0, questionFocusIndex - 1);
        onUpdateQuestionState?.({ focusIndex: next });
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowRight') {
        e.preventDefault();
        const next = Math.min(totalRows - 1, questionFocusIndex + 1);
        onUpdateQuestionState?.({ focusIndex: next });
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (hasCustom && questionFocusIndex === options.length) {
          // Submit custom answer
          const answers = questionCustomText.trim() ? [questionCustomText.trim()] : [];
          api.invoke('hook:answer', { requestId: question.requestId, answers, rejected: false });
        } else {
          // Submit focused option
          const opt = options[questionFocusIndex];
          if (opt) {
            if (question.multiple) {
              const selected = questionSelectedOptions.includes(opt.id)
                ? questionSelectedOptions.filter((id) => id !== opt.id)
                : [...questionSelectedOptions, opt.id];
              onUpdateQuestionState?.({ selectedOptions: selected });
            } else {
              api.invoke('hook:answer', { requestId: question.requestId, answers: [opt.id], rejected: false });
            }
          }
        }
      } else if (e.key === ' ') {
        e.preventDefault();
        const opt = options[questionFocusIndex];
        if (opt && question.multiple) {
          const selected = questionSelectedOptions.includes(opt.id)
            ? questionSelectedOptions.filter((id) => id !== opt.id)
            : [...questionSelectedOptions, opt.id];
          onUpdateQuestionState?.({ selectedOptions: selected });
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        api.invoke('hook:answer', { requestId: question.requestId, answers: [], rejected: true });
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [question, questionFocusIndex, questionSelectedOptions, questionCustomText, onUpdateQuestionState, disabled]);

  const submitQuestion = useCallback(() => {
    if (!question) return;
    if (question.multiple) {
      api.invoke('hook:answer', { requestId: question.requestId, answers: questionSelectedOptions, rejected: false });
    } else if (questionSelectedOptions.length > 0) {
      api.invoke('hook:answer', { requestId: question.requestId, answers: [questionSelectedOptions[0]], rejected: false });
    } else if (question.custom && questionCustomText.trim()) {
      api.invoke('hook:answer', { requestId: question.requestId, answers: [questionCustomText.trim()], rejected: false });
    }
  }, [question, questionSelectedOptions, questionCustomText]);

  const rejectQuestion = useCallback(() => {
    if (!question) return;
    api.invoke('hook:answer', { requestId: question.requestId, answers: [], rejected: true });
  }, [question]);

  const isQuestionActive = !!question;

  // Compute fixed height based on queue items (max 5 rows)
  const maxVisibleRows = 5;
  const rowHeight = 24;
  const queueRowCount = Math.min(state.queue.length, maxVisibleRows);
  const queueHeight = queueRowCount > 0 ? queueRowCount * rowHeight + 8 : 0;
  const headerHeight = 32;
  const draftHeight = 40;
  const padding = 8;
  const footerHeight = headerHeight + queueHeight + draftHeight + padding;
  const footerStyle: React.CSSProperties = {
    height: footerHeight,
    minHeight: footerHeight,
    maxHeight: footerHeight,
    display: 'flex',
    flexDirection: 'column',
    background: '#0c0c0c',
    borderTop: '1px solid #222',
  };

  // Context menu selection preservation
  const draftRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize draft textarea to fit content
  const resizeDraft = useCallback(() => {
    const ta = draftRef.current;
    if (!ta) return;
    ta.style.height = 'auto';
    ta.style.height = Math.min(ta.scrollHeight, 160) + 'px';
  }, []);

  useEffect(() => { resizeDraft(); }, [state.draftText, resizeDraft]);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const handleDraftContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const ta = draftRef.current;
    if (ta) {
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      if (start !== end) {
        onUpdateState({ ...state, draftContextMenuSelectionRange: [start, end] });
      }
    }
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, [state, onUpdateState]);

  const handleMenuCopy = useCallback(() => {
    if (!contextMenu) return;
    const ta = draftRef.current;
    if (!ta) return;
    if (state.draftContextMenuSelectionRange) {
      const [start, end] = state.draftContextMenuSelectionRange;
      const selected = state.draftText.slice(start, end);
      navigator.clipboard.writeText(selected).catch(() => {});
    } else {
      const selected = state.draftText.slice(ta.selectionStart, ta.selectionEnd);
      navigator.clipboard.writeText(selected).catch(() => {});
    }
    setContextMenu(null);
  }, [contextMenu, state.draftText, state.draftContextMenuSelectionRange]);

  const handleMenuPaste = useCallback(async () => {
    if (!contextMenu) return;
    const ta = draftRef.current;
    if (!ta) return;
    try {
      const text = await navigator.clipboard.readText();
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      const newText = state.draftText.slice(0, start) + text + state.draftText.slice(end);
      updateDraft(newText);
      requestAnimationFrame(() => {
        ta.selectionStart = ta.selectionEnd = start + text.length;
      });
    } catch {
      // ignore
    }
    setContextMenu(null);
  }, [contextMenu, state.draftText, updateDraft]);

  // Close context menu on outside click
  useEffect(() => {
    if (!contextMenu) return;
    const handleClick = () => setContextMenu(null);
    window.addEventListener('click', handleClick);
    return () => window.removeEventListener('click', handleClick);
  }, [contextMenu]);

  return (
    <div style={footerStyle}>
      {/* Resize handle */}
      {/* Resize handle removed - height is fixed based on queue */}
      {/* Question Card */}
      {isQuestionActive && (
        <div style={{ marginBottom: 8, padding: '8px 10px', background: '#141414', border: '1px solid #333', borderRadius: 8 }}>
          {question.header && (
            <div style={{ fontSize: 11, fontWeight: 600, color: '#eee', marginBottom: 4 }}>{question.header}</div>
          )}
          <div style={{ fontSize: 12, color: '#ccc', marginBottom: 8 }}>{question.question}</div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {question.options.map((opt, i) => {
              const focused = i === questionFocusIndex;
              const selected = questionSelectedOptions.includes(opt.id);
              return (
                <div
                  key={opt.id}
                  onClick={() => {
                    onUpdateQuestionState?.({ focusIndex: i });
                    if (question.multiple) {
                      const selected = questionSelectedOptions.includes(opt.id)
                        ? questionSelectedOptions.filter((id) => id !== opt.id)
                        : [...questionSelectedOptions, opt.id];
                      onUpdateQuestionState?.({ selectedOptions: selected });
                    } else {
                      onUpdateQuestionState?.({ selectedOptions: [opt.id] });
                    }
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                    padding: '4px 8px',
                    borderRadius: 4,
                    cursor: 'pointer',
                    background: focused ? 'rgba(0,120,212,0.2)' : selected ? 'rgba(100,200,100,0.15)' : 'transparent',
                    border: '1px solid ' + (focused ? '#0078d4' : selected ? '#64c864' : 'transparent'),
                  }}
                >
                  <span style={{ fontSize: 11, color: '#888' }}>
                    {question.multiple ? (selected ? '☑' : '☐') : (selected ? '◉' : '○')}
                  </span>
                  <span style={{ fontSize: 12, color: '#ccc' }}>{opt.label}</span>
                </div>
              );
            })}
            {question.custom && (
              <div
                style={{
                  padding: '4px 8px',
                  borderRadius: 4,
                  background: questionFocusIndex === question.options.length ? 'rgba(0,120,212,0.2)' : 'transparent',
                  border: '1px solid ' + (questionFocusIndex === question.options.length ? '#0078d4' : 'transparent'),
                }}
                onClick={() => onUpdateQuestionState?.({ focusIndex: question.options.length })}
              >
                <input
                  ref={customInputRef}
                  type="text"
                  value={questionCustomText}
                  onChange={(e) => onUpdateQuestionState?.({ customText: e.target.value })}
                  placeholder="Custom answer..."
                  style={{
                    width: '100%',
                    background: 'transparent',
                    border: 'none',
                    color: '#ccc',
                    fontSize: 12,
                    outline: 'none',
                  }}
                />
              </div>
            )}
          </div>
          <div style={{ display: 'flex', gap: 8, marginTop: 8, justifyContent: 'flex-end' }}>
            <button
              onClick={rejectQuestion}
              style={{ padding: '4px 12px', fontSize: 11, background: '#1a1a1a', border: '1px solid #333', color: '#888', borderRadius: 4, cursor: 'pointer' }}
            >
              Reject
            </button>
            <button
              onClick={submitQuestion}
              style={{ padding: '4px 12px', fontSize: 11, background: '#1f3a4c', border: '1px solid #1f3a4c', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}
            >
              Submit
            </button>
          </div>
        </div>
      )}

      {/* Smart Input Controls */}
      {!isQuestionActive && (
        <>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
            <span style={{ fontSize: 11, color: '#888', fontWeight: 600 }}>Smart Input</span>
          </div>

          {state.queue.length > 0 && (
            <div style={{ marginBottom: 6, maxHeight: 120, overflow: 'auto' }}>
              {state.queue.map((task, i) => (
                <div key={i}>
                  {dropIndex === i && dragIndex !== i && (
                    <div style={{ height: 2, background: '#64c864', borderRadius: 1, margin: '2px 0' }} />
                  )}
                  <div
                    onDragOver={(e) => {
                      e.preventDefault();
                      if (dragIndex !== null && dragIndex !== i) {
                        setDropIndex(i);
                      }
                    }}
                    style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '3px 0', fontSize: 11, color: '#aaa' }}
                  >
                    <span
                      draggable
                      onDragStart={() => setDragIndex(i)}
                      onDragEnd={() => {
                        if (dragIndex !== null && dropIndex !== null && dragIndex !== dropIndex) {
                          moveTask(dragIndex, dropIndex);
                        }
                        setDragIndex(null);
                        setDropIndex(null);
                      }}
                      style={{ color: '#666' }}
                    >⋮⋮</span>
                    <span style={{ color: '#666' }}>{i + 1}.</span>
                    {modeControlsVisible && smartInputModeLabel(task.modeId) && (
                      <span style={{ color: '#dcb43c', fontSize: 10, border: '1px solid #5d4722', borderRadius: 3, padding: '1px 4px', background: '#281c10' }}>
                        {smartInputModeLabel(task.modeId)}
                      </span>
                    )}
                    <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{task.text || '(attachment)'}</span>
                    <button
                      onClick={() => steerTask(i)}
                      title="Steer now"
                      style={{ background: 'transparent', border: 'none', color: '#dcb43c', cursor: 'pointer', fontSize: 10 }}
                    >
                      ⚡
                    </button>
                    <button
                      onClick={() => {
                        onUpdateState({
                          ...state,
                          queue: state.queue.filter((_, idx) => idx !== i),
                          draftText: task.text,
                          draftAttachments: [...task.attachments],
                        });
                        setDraftMode(task.modeId as SmartInputModeId);
                        setEditRestoreIndex(i);
                      }}
                      title="Düzenle"
                      style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                    >
                      ✎
                    </button>
                    <button onClick={() => removeTask(i)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}>✕</button>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Attachment chips */}
          {state.draftAttachments.length > 0 && (
            <div style={{ display: 'flex', gap: 4, marginBottom: 6, flexWrap: 'wrap' }}>
              {state.draftAttachments.map((a, i) => (
                <span key={i} style={{ fontSize: 11, color: '#aaa', background: '#1a1a1a', padding: '2px 6px', borderRadius: 3, display: 'flex', alignItems: 'center', gap: 4, border: '1px solid #333' }}>
                  {a.name}
                  <button
                    onClick={() => {
                      const mention = `@${a.name}`;
                      const newAttachments = state.draftAttachments.filter((_, idx) => idx !== i);
                      const newDraft = removeMentionFromInput(state.draftText, mention);
                      onUpdateState({ ...state, draftAttachments: newAttachments, draftText: newDraft });
                    }}
                    style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                  >
                    ✕
                  </button>
                </span>
              ))}
            </div>
          )}

          <div style={{ display: 'flex', gap: 6, alignItems: 'flex-end' }}>
            {modeControlsVisible && (
              <div
                title="Auto decides per message; manual choices apply once"
                style={{
                  width: 152,
                  height: 24,
                  display: 'grid',
                  gridTemplateColumns: 'repeat(4, 1fr)',
                  border: '1px solid #333',
                  borderRadius: 6,
                  overflow: 'hidden',
                  background: '#141414',
                  flexShrink: 0,
                  marginBottom: 4,
                }}
              >
                {(['auto', 'build', 'plan', 'codex_plan'] as SmartInputModeId[]).map((candidate) => {
                  const active = draftMode === candidate;
                  const plan = candidate === 'plan';
                  const codex = candidate === 'codex_plan';
                  return (
                    <button
                      key={candidate}
                      onClick={() => setDraftMode(candidate)}
                      style={{
                        border: 'none',
                        borderRight: candidate === 'codex_plan' ? 'none' : '1px solid #333',
                        background: active ? (plan || codex ? '#281c10' : '#222') : '#141414',
                        color: active ? (plan || codex ? '#dcb43c' : '#d6d6d6') : '#777',
                        fontSize: 9,
                        padding: 0,
                        cursor: 'pointer',
                      }}
                    >
                      {candidate === 'codex_plan' ? 'Codex' : candidate === 'auto' ? 'Auto' : plan ? 'Plan' : 'Build'}
                    </button>
                  );
                })}
              </div>
            )}
            <textarea
              ref={draftRef}
              data-smart-input={terminalId}
              value={state.draftText}
              onChange={(e) => updateDraft(e.target.value)}
              onContextMenu={handleDraftContextMenu}
              onFocus={() => {
                onClearTerminalOutputFocusOverride?.();
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey) {
                  e.preventDefault();
                  submit();
                }
                if (e.key === 'Enter' && e.ctrlKey) {
                  e.preventDefault();
                  updateDraft(state.draftText + '\n');
                }
                if (e.key === 'Tab' && !e.ctrlKey && !e.altKey && !e.metaKey) {
                  e.preventDefault();
                  if (!modeControlsVisible) return;
                  if (e.shiftKey) {
                    // Shift+Tab blocked to prevent reverse focus traversal
                    return;
                  }
                  setDraftMode((current) => toggleSmartInputModeId(current));
                }
              }}
              onPaste={async (e) => {
                e.preventDefault();
                const paste = snapshotClipboardPaste(e.clipboardData);
                if (shouldReadNativeClipboardFilePaths(paste)) {
                  const paths = await api.invoke('clipboard:readFilePaths') as string[] | undefined;
                  if (paths && paths.length > 0) {
                    let newDraft = state.draftText;
                    const newAttachments = [...state.draftAttachments];
                    for (const p of paths) {
                      const name = p.split(/[/\\]/).pop() || p;
                      newAttachments.push({ path: p, name });
                      newDraft = newDraft ? `${newDraft} @${name}` : `@${name}`;
                    }
                    onUpdateState({
                      ...state,
                      draftAttachments: newAttachments,
                      draftText: newDraft,
                    });
                    return;
                  }
                }
                if (shouldReadNativeClipboardImage(paste)) {
                  const imgResult = await api.invoke('clipboard:readImage') as { path?: string; dataUrl?: string } | undefined;
                  if (imgResult?.path) {
                    const p = imgResult.path;
                    const name = p.split(/[/\\]/).pop() || p;
                    onUpdateState({
                      ...state,
                      draftAttachments: [...state.draftAttachments, { path: p, name }],
                      draftText: state.draftText ? `${state.draftText} @${name}` : `@${name}`,
                    });
                    return;
                  }
                }
                const text = paste.text;
                if (text) {
                  updateDraft(state.draftText + text);
                }
              }}
              placeholder="Type a prompt..."
              style={{
                flex: 1,
                background: '#1a1a1a',
                border: '1px solid #333',
                borderRadius: 6,
                padding: '6px 8px',
                color: '#ccc',
                fontSize: 12,
                fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif",
                resize: 'none',
                minHeight: 32,
                maxHeight: 160,
                overflow: 'auto',
                outline: 'none',
              }}
              rows={1}
            />
            <button
              onClick={submit}
              style={{
                width: 28,
                height: 28,
                borderRadius: '50%',
                background: '#1a1a1a',
                border: '1px solid #333',
                color: '#ccc',
                cursor: 'pointer',
                fontSize: 14,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexShrink: 0,
              }}
            >
              ➤
            </button>
          </div>

          {/* Context menu */}
          {contextMenu && (
            <div
              style={{
                position: 'fixed',
                left: contextMenu.x,
                top: contextMenu.y,
                zIndex: 1000,
                background: '#1a1a1a',
                border: '1px solid #333',
                borderRadius: 4,
                padding: '4px 0',
                fontSize: 12,
                color: '#ccc',
              }}
            >
              <button
                onClick={handleMenuCopy}
                style={{
                  display: 'block',
                  width: '100%',
                  textAlign: 'left',
                  padding: '4px 12px',
                  background: 'transparent',
                  border: 'none',
                  color: '#ccc',
                  cursor: 'pointer',
                  fontSize: 12,
                }}
                onMouseEnter={(e) => { (e.target as HTMLElement).style.background = '#333'; }}
                onMouseLeave={(e) => { (e.target as HTMLElement).style.background = 'transparent'; }}
              >
                Copy
              </button>
              <button
                onClick={handleMenuPaste}
                style={{
                  display: 'block',
                  width: '100%',
                  textAlign: 'left',
                  padding: '4px 12px',
                  background: 'transparent',
                  border: 'none',
                  color: '#ccc',
                  cursor: 'pointer',
                  fontSize: 12,
                }}
                onMouseEnter={(e) => { (e.target as HTMLElement).style.background = '#333'; }}
                onMouseLeave={(e) => { (e.target as HTMLElement).style.background = 'transparent'; }}
              >
                Paste
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
};
