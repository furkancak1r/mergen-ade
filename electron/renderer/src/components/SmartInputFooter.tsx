import React, { useState, useCallback, useEffect, useRef } from 'react';
import type { SmartInputState, SmartInputTask, SmartInputAttachment, OpenCodeQuestion } from '../../../shared/types';
import { removeMentionFromInput } from '../lib/acpParser';
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
  disabled = false,
}) => {
  const [deliveryMode, setDeliveryMode] = useState<'now' | 'after'>('now');
  const [draftMode, setDraftMode] = useState<SmartInputModeId>('build');
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);
  const [resizing, setResizing] = useState(false);
  const footerRef = useRef<HTMLDivElement>(null);
  const customInputRef = useRef<HTMLInputElement>(null);

  const updateDraft = useCallback((text: string) => {
    onUpdateState({ ...state, draftText: text });
  }, [state, onUpdateState]);

  const addTask = useCallback(() => {
    if (!state.draftText.trim() && state.draftAttachments.length === 0) return;
    const task: SmartInputTask = {
      text: state.draftText.trim(),
      attachments: [...state.draftAttachments],
      modeId: draftMode,
      afterDone: deliveryMode === 'after',
    };
    onUpdateState({
      ...state,
      queue: [...state.queue, task],
      draftText: '',
      draftAttachments: [],
    });
  }, [state, draftMode, deliveryMode, onUpdateState]);

  const sendNow = useCallback(() => {
    if (!state.draftText.trim() && state.draftAttachments.length === 0) return;
    onSendToTerminal(terminalId, state.draftText.trim(), state.draftAttachments, draftMode);
    onUpdateState({ ...state, draftText: '', draftAttachments: [] });
  }, [state, terminalId, draftMode, onSendToTerminal, onUpdateState]);

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
    if (deliveryMode === 'now') {
      sendNow();
    } else {
      addTask();
    }
  }, [deliveryMode, sendNow, addTask]);

  // Footer resize handling
  useEffect(() => {
    if (!resizing) return;
    function handleMove(e: MouseEvent) {
      if (!footerRef.current) return;
      const rect = footerRef.current.getBoundingClientRect();
      const newHeight = Math.max(60, Math.min(400, rect.height - e.movementY));
      onUpdateState({ ...state, userHeight: newHeight });
    }
    function handleUp() {
      setResizing(false);
    }
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [resizing, onUpdateState, state]);

  // Question keyboard handling
  useEffect(() => {
    if (disabled) return;
    if (!question) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      // Do not steal keyboard events from other text inputs (ACP composer, file editor, etc.)
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

  // Compute safe minimum height based on visible content
  const queueVisibleHeight = state.queue.length > 0 ? Math.min(state.queue.length * 20 + 8, 120) : 0;
  const safeMinHeight = 32 + queueVisibleHeight + 40 + 8; // header + queue + draft + padding
  const footerHeight = state.userHeight ? Math.max(safeMinHeight, Math.min(400, state.userHeight)) : undefined;
  const footerStyle: React.CSSProperties = footerHeight
    ? { height: footerHeight, minHeight: safeMinHeight, maxHeight: 400, display: 'flex', flexDirection: 'column', background: '#0c0c0c', borderTop: '1px solid #222' }
    : { borderTop: '1px solid #222', padding: '6px 8px', background: '#0c0c0c' };

  // Context menu selection preservation
  const draftRef = useRef<HTMLTextAreaElement>(null);
  const editRef = useRef<HTMLTextAreaElement>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; target: 'draft' | 'edit' } | null>(null);

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
    setContextMenu({ x: e.clientX, y: e.clientY, target: 'draft' });
  }, [state, onUpdateState]);

  const handleEditContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const ta = editRef.current;
    if (ta) {
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      if (start !== end) {
        onUpdateState({ ...state, editContextMenuSelectionRange: [start, end] });
      }
    }
    setContextMenu({ x: e.clientX, y: e.clientY, target: 'edit' });
  }, [state, onUpdateState]);

  const handleMenuCopy = useCallback(() => {
    if (!contextMenu) return;
    const isDraft = contextMenu.target === 'draft';
    const ta = isDraft ? draftRef.current : editRef.current;
    if (!ta) return;
    const range = isDraft ? state.draftContextMenuSelectionRange : state.editContextMenuSelectionRange;
    const text = isDraft ? state.draftText : state.editText;
    if (range) {
      const [start, end] = range;
      const selected = text.slice(start, end);
      navigator.clipboard.writeText(selected).catch(() => {});
    } else {
      const selected = text.slice(ta.selectionStart, ta.selectionEnd);
      navigator.clipboard.writeText(selected).catch(() => {});
    }
    setContextMenu(null);
  }, [contextMenu, state.draftText, state.draftContextMenuSelectionRange, state.editText, state.editContextMenuSelectionRange]);

  const handleMenuPaste = useCallback(async () => {
    if (!contextMenu) return;
    const isDraft = contextMenu.target === 'draft';
    const ta = isDraft ? draftRef.current : editRef.current;
    if (!ta) return;
    try {
      const text = await navigator.clipboard.readText();
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      const currentText = isDraft ? state.draftText : state.editText;
      const newText = currentText.slice(0, start) + text + currentText.slice(end);
      if (isDraft) {
        updateDraft(newText);
      } else {
        onUpdateState({ ...state, editText: newText });
      }
      requestAnimationFrame(() => {
        ta.selectionStart = ta.selectionEnd = start + text.length;
      });
    } catch {
      // ignore
    }
    setContextMenu(null);
  }, [contextMenu, state.draftText, state.editText, updateDraft, onUpdateState, state]);

  // Close context menu on outside click
  useEffect(() => {
    if (!contextMenu) return;
    const handleClick = () => setContextMenu(null);
    window.addEventListener('click', handleClick);
    return () => window.removeEventListener('click', handleClick);
  }, [contextMenu]);

  return (
    <div ref={footerRef} style={footerStyle}>
      {/* Resize handle */}
      {!isQuestionActive && (
        <div
          onMouseDown={() => setResizing(true)}
          style={{
            height: 4,
            cursor: 'row-resize',
            background: resizing ? '#0078d4' : 'transparent',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexShrink: 0,
          }}
        >
          <div style={{ width: 24, height: 2, background: '#444', borderRadius: 1 }} />
        </div>
      )}
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
            <div style={{ display: 'flex', gap: 4, marginLeft: 'auto' }}>
              <button
                onClick={() => setDeliveryMode('now')}
                style={{ fontSize: 10, padding: '2px 6px', borderRadius: 3, border: '1px solid #333', background: deliveryMode === 'now' ? '#1f3a4c' : 'transparent', color: '#ccc', cursor: 'pointer' }}
              >
                Steer Now
              </button>
              <button
                onClick={() => setDeliveryMode('after')}
                style={{ fontSize: 10, padding: '2px 6px', borderRadius: 3, border: '1px solid #333', background: deliveryMode === 'after' ? '#1f3a4c' : 'transparent', color: '#ccc', cursor: 'pointer' }}
              >
                After Done
              </button>
            </div>
          </div>

          {state.queue.length > 0 && (
            <div style={{ marginBottom: 6, maxHeight: 120, overflow: 'auto' }}>
              {state.queue.map((task, i) => (
                <div key={i}>
                  {dropIndex === i && dragIndex !== i && (
                    <div style={{ height: 2, background: '#64c864', borderRadius: 1, margin: '2px 0' }} />
                  )}
                  <div
                    draggable={state.editIndex !== i}
                    onDragStart={() => setDragIndex(i)}
                    onDragEnd={() => {
                      if (dragIndex !== null && dropIndex !== null && dragIndex !== dropIndex) {
                        moveTask(dragIndex, dropIndex);
                      }
                      setDragIndex(null);
                      setDropIndex(null);
                    }}
                    onDragOver={(e) => {
                      e.preventDefault();
                      if (dragIndex !== null && dragIndex !== i) {
                        setDropIndex(i);
                      }
                    }}
                    style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '3px 0', fontSize: 11, color: '#aaa', cursor: state.editIndex === i ? 'default' : 'grab' }}
                  >
                    <span style={{ color: '#666', cursor: 'grab' }}>⋮⋮</span>
                    <span style={{ color: '#666' }}>{i + 1}.</span>
                    {smartInputModeLabel(task.modeId) && (
                      <span style={{ color: '#dcb43c', fontSize: 10, border: '1px solid #5d4722', borderRadius: 3, padding: '1px 4px', background: '#281c10' }}>
                        {smartInputModeLabel(task.modeId)}
                      </span>
                    )}
                    {state.editIndex === i ? (
                      <textarea
                        ref={editRef}
                        value={state.editText}
                        onChange={(e) => onUpdateState({ ...state, editText: e.target.value })}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey) {
                            e.preventDefault();
                            // Save edit
                            const queue = state.queue.map((t, idx) =>
                              idx === i ? { ...t, text: state.editText.trim(), attachments: [...state.editAttachments] } : t
                            );
                            onUpdateState({ ...state, queue, editIndex: undefined, editText: '', editAttachments: [] });
                          }
                          if (e.key === 'Enter' && e.ctrlKey) {
                            e.preventDefault();
                            onUpdateState({ ...state, editText: state.editText + '\n' });
                          }
                          if (e.key === 'Escape') {
                            onUpdateState({ ...state, editIndex: undefined, editText: '', editAttachments: [] });
                          }
                        }}
                        autoFocus
                        onFocus={() => onClearTerminalOutputFocusOverride?.()}
                        onContextMenu={handleEditContextMenu}
                        onPaste={async (e) => {
                          e.preventDefault();
                          const paste = snapshotClipboardPaste(e.clipboardData);
                          if (shouldReadNativeClipboardFilePaths(paste)) {
                            const paths = await api.invoke('clipboard:readFilePaths') as string[] | undefined;
                            if (paths && paths.length > 0) {
                              let newText = state.editText;
                              const newAttachments = [...state.editAttachments];
                              for (const p of paths) {
                                const name = p.split(/[/\\]/).pop() || p;
                                newAttachments.push({ path: p, name });
                                newText = newText ? `${newText} @${name}` : `@${name}`;
                              }
                              onUpdateState({ ...state, editAttachments: newAttachments, editText: newText });
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
                                editAttachments: [...state.editAttachments, { path: p, name }],
                                editText: state.editText ? `${state.editText} @${name}` : `@${name}`,
                              });
                              return;
                            }
                          }
                          const text = paste.text;
                          if (text) {
                            onUpdateState({ ...state, editText: state.editText + text });
                          }
                        }}
                        style={{
                          flex: 1,
                          background: '#1a1a1a',
                          border: '1px solid #333',
                          borderRadius: 4,
                          padding: '3px 6px',
                          color: '#ccc',
                          fontSize: 11,
                          resize: 'none',
                          minHeight: 24,
                          maxHeight: 60,
                          outline: 'none',
                        }}
                        rows={1}
                      />
                    ) : (
                      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{task.text || '(attachment)'}</span>
                    )}
                    {state.editIndex === i ? (
                      <>
                        {/* Edit attachment chips */}
                        {state.editAttachments.length > 0 && (
                          <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginRight: 4 }}>
                            {state.editAttachments.map((a, ai) => (
                              <span key={ai} style={{ fontSize: 10, color: '#aaa', background: '#1a1a1a', padding: '1px 4px', borderRadius: 3, display: 'flex', alignItems: 'center', gap: 2, border: '1px solid #333' }}>
                                {a.name}
                                <button
                                  onClick={() => {
                                    const mention = `@${a.name}`;
                                    const newAttachments = state.editAttachments.filter((_, idx) => idx !== ai);
                                    const newEditText = removeMentionFromInput(state.editText, mention);
                                    onUpdateState({ ...state, editAttachments: newAttachments, editText: newEditText });
                                  }}
                                  style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 9 }}
                                >
                                  ✕
                                </button>
                              </span>
                            ))}
                          </div>
                        )}
                        <button
                          onClick={() => {
                            const queue = state.queue.map((t, idx) =>
                              idx === i ? { ...t, text: state.editText.trim(), attachments: [...state.editAttachments] } : t
                            );
                            onUpdateState({ ...state, queue, editIndex: undefined, editText: '', editAttachments: [] });
                          }}
                          style={{ background: 'transparent', border: 'none', color: '#64c864', cursor: 'pointer', fontSize: 10 }}
                        >
                          ✓
                        </button>
                        <button
                          onClick={() => onUpdateState({ ...state, editIndex: undefined, editText: '', editAttachments: [] })}
                          style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                        >
                          ✕
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          onClick={() => onUpdateState({ ...state, editIndex: i, editText: task.text, editAttachments: [...task.attachments] })}
                          style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                        >
                          ✎
                        </button>
                        <button onClick={() => removeTask(i)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}>✕</button>
                      </>
                    )}
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
            <div
              title="Tab toggles Build/Plan"
              style={{
                width: 58,
                height: 24,
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                border: '1px solid #333',
                borderRadius: 6,
                overflow: 'hidden',
                background: '#141414',
                flexShrink: 0,
                marginBottom: 4,
              }}
            >
              {(['build', 'plan'] as SmartInputModeId[]).map((candidate) => {
                const active = draftMode === candidate;
                const plan = candidate === 'plan';
                return (
                  <button
                    key={candidate}
                    onClick={() => setDraftMode(candidate)}
                    style={{
                      border: 'none',
                      borderRight: candidate === 'build' ? '1px solid #333' : 'none',
                      background: active ? (plan ? '#281c10' : '#222') : '#141414',
                      color: active ? (plan ? '#dcb43c' : '#d6d6d6') : '#777',
                      fontSize: 9,
                      padding: 0,
                      cursor: 'pointer',
                    }}
                  >
                    {plan ? 'Plan' : 'Build'}
                  </button>
                );
              })}
            </div>
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
                resize: 'none',
                minHeight: 32,
                maxHeight: 80,
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
