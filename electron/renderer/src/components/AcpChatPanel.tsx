import React, { useState, useEffect, useRef, useCallback } from 'react';
import type { AcpChatSession, AcpChatMessage, ProjectRecord, AcpConfigOption, QueuedAcpPrompt, AppConfig, OpenCodeQuestion } from '../../../shared/types';
import { AcpStartupMode as AcpStartupModeEnum, defaultAppConfig } from '../../../shared/types';
import { buildAcpPromptText, removeMentionFromInput } from '../lib/acpParser';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

interface AcpChatPanelProps {
  project: ProjectRecord;
  chatId: string;
  config?: AppConfig;
  onClose?: () => void;
  disabled?: boolean;
  branchName?: string;
}

export const AcpChatPanel: React.FC<AcpChatPanelProps> = ({ project, chatId, config, onClose, disabled = false, branchName }) => {
  const [session, setSession] = useState<AcpChatSession | null>(null);
  const [input, setInput] = useState('');
  const [attachments, setAttachments] = useState<string[]>([]);
  const [modeDropdownOpen, setModeDropdownOpen] = useState(false);
  const [pendingQuestion, setPendingQuestion] = useState<OpenCodeQuestion | null>(null);
  const [selectedOptions, setSelectedOptions] = useState<string[]>([]);
  const [customAnswer, setCustomAnswer] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const modeDropdownRef = useRef<HTMLDivElement>(null);
  const customInputRef = useRef<HTMLInputElement>(null);

  const refreshSession = useCallback(async () => {
    const s = await api.invoke('acp:getSession', chatId) as AcpChatSession | null;
    setSession(s);
  }, [chatId]);

  useEffect(() => {
    refreshSession();
    const unsub = api.on('acp:event', (eventChatId: string, event: { type: string; text?: string; options?: OpenCodeQuestion['options']; multiple?: boolean; custom?: boolean; requestId?: string; sessionId?: string; message?: string; header?: string; question?: string; count?: number; modeId?: string }) => {
      if (eventChatId !== chatId) return;
      refreshSession();
      if (event.type === 'permission') {
        setPendingQuestion({
          header: 'Permission Required',
          question: event.message || '',
          options: event.options || [],
          multiple: event.multiple ?? false,
          custom: event.custom ?? false,
          requestId: event.requestId || '',
          sessionId: event.sessionId || '',
        });
      }
      if (event.type === 'promptResponse' || event.type === 'cancelled') {
        setPendingQuestion(null);
        setSelectedOptions([]);
        setCustomAnswer('');
      }
    });
    return () => { unsub(); };
  }, [chatId, refreshSession]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [session?.messages]);

  useEffect(() => {
    setPendingQuestion(null);
    setSelectedOptions([]);
    setCustomAnswer('');
  }, [chatId]);

  useEffect(() => {
    if (disabled) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        if (modeDropdownOpen) {
          setModeDropdownOpen(false);
          return;
        }
        if (session?.status === 'running' || session?.status === 'permission') {
          e.preventDefault();
          cancelAcp();
        }
      }
      if (e.key === 'Tab' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        if (document.activeElement === inputRef.current) {
          e.preventDefault();
          toggleMode();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [session?.status, disabled]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (modeDropdownRef.current && !modeDropdownRef.current.contains(e.target as Node)) {
        setModeDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const send = useCallback(async () => {
    if (!input.trim() && attachments.length === 0) return;
    const text = input.trim();
    const promptText = buildAcpPromptText(text, attachments);
    await api.invoke('acp:send', { chatId, promptText, attachments: [] });
    setInput('');
    setAttachments([]);
  }, [chatId, input, attachments]);

  const cancelAcp = useCallback(async () => {
    await api.invoke('acp:cancel', chatId);
  }, [chatId]);

  const removeAttachment = useCallback((index: number) => {
    const path = attachments[index];
    const name = path.split(/[/\\]/).pop() || path;
    const mention = `@${name}`;
    setAttachments((prev) => prev.filter((_, idx) => idx !== index));
    setInput((prev) => removeMentionFromInput(prev, mention));
  }, [attachments]);

  const toggleMode = useCallback(() => {
    const currentMode = session?.currentModeId || 'build';
    const nextMode = currentMode === 'plan' ? 'build' : 'plan';
    api.invoke('acp:setConfigOption', { chatId, configId: 'mode', value: nextMode });
  }, [chatId, session?.currentModeId]);

  const isRunning = session?.status === 'running' || session?.status === 'permission';
  const hasDraft = input.trim().length > 0 || attachments.length > 0;
  const showStop = isRunning && !hasDraft;
  const canSend = hasDraft;
  const currentMode = session?.currentModeId || 'build';
  const currentModel = session?.currentModel || 'Unknown';
  const currentEffort = session?.currentEffort || '';

  const modelLabel = currentModel ? `${currentModel} ${currentEffort}`.trim() : 'Model';

  const modelOptions = session?.configOptions?.find((o) => o.id === 'model');
  const effortOptions = session?.configOptions?.find((o) => o.id === 'effort');

  const resolvedConfig = config ?? defaultAppConfig();
  const favoriteModels = resolvedConfig.opencode.acpFavoriteModels;
  const filteredModelOptions = modelOptions && favoriteModels.length > 0
    ? { ...modelOptions, options: modelOptions.options.filter((o) => favoriteModels.includes(o.value)) }
    : modelOptions;

  const setModel = (value: string) => {
    api.invoke('acp:setConfigOption', { chatId, configId: 'model', value });
  };

  const setEffort = (value: string) => {
    api.invoke('acp:setConfigOption', { chatId, configId: 'effort', value });
  };

  const branchNameDisplay = branchName || 'main';

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c0c' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>
          ACP Chat — {project.name}
        </span>
        <span style={{ fontSize: 11, color: '#888' }}>
          {session?.status || 'Loading...'}
        </span>
        {onClose && (
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
            ✕
          </button>
        )}
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: '8px 12px' }}>
        {session?.messages.length === 0 && (
          <div style={{ color: '#666', fontSize: 12, textAlign: 'center', marginTop: 40 }}>
            Welcome to ACP Chat. Type a message to start.
          </div>
        )}
        {session?.messages.map((msg, i) => (
          <MessageBubble key={i} message={msg} />
        ))}
        {session?.queuedPrompts && session.queuedPrompts.length > 0 && (
          <div style={{ marginTop: 8 }}>
            {session.queuedPrompts.map((qp, i) => (
              <QueuedPromptRow key={i} prompt={qp} />
            ))}
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Status row */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 12px', fontSize: 11, color: '#666' }}>
        <span>{branchNameDisplay}</span>
        <span>Local</span>
        <span>{session?.status || 'Idle'}</span>
      </div>

      {/* Permission card */}
      {pendingQuestion && (
        <div style={{ padding: '8px 12px', borderTop: '1px solid #222', background: '#1a1a1a' }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>{pendingQuestion.header}</div>
          <div style={{ fontSize: 12, color: '#aaa', marginBottom: 8 }}>{pendingQuestion.question}</div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {pendingQuestion.options.map((opt) => (
              <label key={opt.id} style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: '#ccc', cursor: 'pointer' }}>
                <input
                  type={pendingQuestion.multiple ? 'checkbox' : 'radio'}
                  name="acp-permission"
                  checked={selectedOptions.includes(opt.id)}
                  onChange={() => {
                    if (pendingQuestion.multiple) {
                      setSelectedOptions((prev) => prev.includes(opt.id) ? prev.filter((x) => x !== opt.id) : [...prev, opt.id]);
                    } else {
                      setSelectedOptions([opt.id]);
                    }
                  }}
                />
                {opt.label}
              </label>
            ))}
          </div>
          {pendingQuestion.custom && (
            <input
              ref={customInputRef}
              value={customAnswer}
              onChange={(e) => setCustomAnswer(e.target.value)}
              placeholder="Your answer..."
              style={{ marginTop: 8, width: '100%', background: '#0c0c0c', border: '1px solid #333', color: '#ccc', fontSize: 12, padding: '4px 8px', borderRadius: 4 }}
            />
          )}
          <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
            <button
              onClick={() => {
                const answers = pendingQuestion.custom ? [...selectedOptions, customAnswer] : selectedOptions;
                api.invoke('acp:permissionResponse', { chatId, requestId: pendingQuestion.requestId, answers: answers.filter((a) => a.length > 0), rejected: false });
                setPendingQuestion(null);
                setSelectedOptions([]);
                setCustomAnswer('');
              }}
              style={{ fontSize: 12, padding: '4px 12px', borderRadius: 4, border: '1px solid #333', background: '#1f3a4c', color: '#ccc', cursor: 'pointer' }}
            >
              Submit
            </button>
            <button
              onClick={() => {
                api.invoke('acp:permissionResponse', { chatId, requestId: pendingQuestion.requestId, answers: [], rejected: true });
                setPendingQuestion(null);
                setSelectedOptions([]);
                setCustomAnswer('');
              }}
              style={{ fontSize: 12, padding: '4px 12px', borderRadius: 4, border: '1px solid #333', background: 'transparent', color: '#888', cursor: 'pointer' }}
            >
              Reject
            </button>
          </div>
        </div>
      )}

      {/* Composer capsule */}
      <div style={{ padding: '8px 12px', borderTop: '1px solid #222' }}>
        {/* Attachment chips */}
        {attachments.length > 0 && (
          <div style={{ display: 'flex', gap: 4, marginBottom: 6, flexWrap: 'wrap' }}>
            {attachments.map((a, i) => (
              <span key={i} style={{ fontSize: 11, color: '#aaa', background: '#1a1a1a', padding: '2px 6px', borderRadius: 3, display: 'flex', alignItems: 'center', gap: 4 }}>
                {a}
                <button onClick={() => removeAttachment(i)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}>✕</button>
              </span>
            ))}
          </div>
        )}

        <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', background: '#1b1b1b', borderRadius: 12, padding: '8px 12px' }}>
          <button
            onClick={async () => {
              const paths = await api.invoke('dialog:showOpen', { properties: ['openFile', 'multiSelections'] }) as string[] | undefined;
              if (paths) {
                setAttachments((prev) => [...prev, ...paths]);
                const mentions = paths.map((p) => `@${p.split(/[/\\]/).pop() || p}`).join(' ');
                setInput((prev) => prev ? `${prev} ${mentions}` : mentions);
              }
            }}
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
            +
          </button>

          {/* Mode pill */}
          {currentMode === 'plan' && (
            <button
              onClick={toggleMode}
              style={{ fontSize: 11, padding: '2px 8px', borderRadius: 4, border: '1px solid #333', background: '#1f3a4c', color: '#ccc', cursor: 'pointer', flexShrink: 0 }}
            >
              Plan
            </button>
          )}

          {/* Model + effort selector */}
          <div style={{ position: 'relative', flexShrink: 0 }} ref={modeDropdownRef}>
            <button
              onClick={() => setModeDropdownOpen((v) => !v)}
              style={{ fontSize: 11, padding: '2px 8px', borderRadius: 4, border: '1px solid #333', background: 'transparent', color: '#ccc', cursor: 'pointer', maxWidth: 160, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            >
              {modelLabel}
            </button>
            {modeDropdownOpen && (
              <div style={{ position: 'absolute', bottom: '100%', left: 0, marginBottom: 4, background: '#1a1a1a', border: '1px solid #333', borderRadius: 6, padding: 8, minWidth: 200, zIndex: 10 }}>
                {filteredModelOptions && (
                  <div style={{ marginBottom: 8 }}>
                    <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Model</div>
                    {filteredModelOptions.options.map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => { setModel(opt.value); setModeDropdownOpen(false); }}
                        style={{ display: 'block', width: '100%', textAlign: 'left', fontSize: 11, padding: '4px 8px', background: filteredModelOptions.currentValue === opt.value ? '#1f3a4c' : 'transparent', border: 'none', color: '#ccc', cursor: 'pointer', borderRadius: 3 }}
                      >
                        {opt.label}
                      </button>
                    ))}
                    {filteredModelOptions.options.length === 0 && (
                      <div style={{ fontSize: 11, color: '#888' }}>No favorite models. Add favorites in Settings &gt; OpenCode.</div>
                    )}
                  </div>
                )}
                {effortOptions && (
                  <div>
                    <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Effort</div>
                    {effortOptions.options.map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => { setEffort(opt.value); setModeDropdownOpen(false); }}
                        style={{ display: 'block', width: '100%', textAlign: 'left', fontSize: 11, padding: '4px 8px', background: effortOptions.currentValue === opt.value ? '#1f3a4c' : 'transparent', border: 'none', color: '#ccc', cursor: 'pointer', borderRadius: 3 }}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                )}
                {!modelOptions && !effortOptions && (
                  <div style={{ fontSize: 11, color: '#888' }}>No model options available.</div>
                )}
              </div>
            )}
          </div>

          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
                e.preventDefault();
                if (hasDraft) send();
              }
              if (e.key === 'Enter' && e.ctrlKey && !e.altKey && !e.metaKey) {
                e.preventDefault();
                setInput((prev) => prev + '\n');
              }
            }}
            placeholder="Type a message..."
            style={{
              flex: 1,
              background: 'transparent',
              border: 'none',
              color: '#ccc',
              fontSize: 13,
              resize: 'none',
              minHeight: 36,
              maxHeight: 120,
              outline: 'none',
            }}
            rows={1}
          />
          <button
            onClick={showStop ? cancelAcp : send}
            disabled={!hasDraft && !isRunning}
            style={{
              width: 28,
              height: 28,
              borderRadius: '50%',
              background: showStop ? '#c44' : '#1a1a1a',
              border: '1px solid #333',
              color: '#ccc',
              cursor: (!hasDraft && !isRunning) ? 'not-allowed' : 'pointer',
              fontSize: 14,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
              opacity: (!hasDraft && !isRunning) ? 0.5 : 1,
            }}
          >
            {showStop ? '✕' : '➤'}
          </button>
        </div>
      </div>
    </div>
  );
};

const MessageBubble: React.FC<{ message: AcpChatMessage }> = ({ message }) => {
  const isUser = message.role === 'user';
  return (
    <div style={{
      display: 'flex',
      justifyContent: isUser ? 'flex-end' : 'flex-start',
      marginBottom: 8,
    }}>
      <div style={{
        maxWidth: '80%',
        padding: '8px 12px',
        borderRadius: 12,
        background: isUser ? '#1f3a4c' : '#1a1a1a',
        color: '#ccc',
        fontSize: 13,
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-word',
      }}>
        {message.text}
      </div>
    </div>
  );
};

const QueuedPromptRow: React.FC<{ prompt: QueuedAcpPrompt }> = ({ prompt }) => {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 12px', background: '#1a1a1a', borderRadius: 8, marginBottom: 4, fontSize: 12, color: '#888' }}>
      <span style={{ color: '#666', fontWeight: 600 }}>Queued</span>
      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{prompt.text || prompt.finalPromptText}</span>
      {prompt.modeId && prompt.modeId !== 'build' && (
        <span style={{ fontSize: 10, color: '#666', background: '#222', padding: '2px 6px', borderRadius: 3 }}>{prompt.modeId}</span>
      )}
    </div>
  );
};

