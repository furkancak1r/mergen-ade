import React, { useState, useEffect, useRef, useCallback } from 'react';
import type { AcpChatSession, ProjectRecord, QueuedAcpPrompt, AppConfig, OpenCodeQuestion } from '../../../shared/types';
import { activeBuildModel, defaultAppConfig, effectivePlanModel } from '../../../shared/types';
import { fallbackTimelineFromMessages, normalizeAcpTimelineToolStatus } from '../../../shared/acpTimeline';
import { appendMentionsToInput, pathToMention, removeMentionFromInput } from '../lib/acpParser';
import {
  ACP_QUEUED_PROMPT_MAX_VISIBLE_ROWS,
  actionControlsEnabled,
  acpComposerHintText,
  acpHeaderStatusColor,
  acpKimiProtectionBadge,
  acpModeUiLabel,
  acpQueuedPromptDraftEditBlockedMessage,
  acpQueuedPromptAttachmentLabel,
  acpQueuedPromptHeaderLabel,
  acpQueuedPromptIndexLabel,
  acpQueuedPromptPlanCount,
  acpQueuedPromptVisibleRowCount,
  acpStatusText,
  hasConfigSelectorOptions,
  openCodeAcpPanelTitle,
  openCodeAcpWelcomeText,
  optionValues,
  queuedPromptPreview,
  shouldShowAcpWelcome,
  slashCommandItemsForComposer,
  type AcpSlashCommandItem,
} from '../lib/acpUi';
import { AcpTimeline } from './AcpTimeline';
import { AcpChangesPanel } from './AcpChangesPanel';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

interface AcpPanelEvent {
  type: string;
  text?: string;
  role?: string;
  options?: OpenCodeQuestion['options'];
  questions?: OpenCodeQuestion['questions'];
  multiple?: boolean;
  custom?: boolean;
  requestId?: string;
  sessionId?: string;
  message?: string;
  header?: string;
  question?: string;
  count?: number;
  modeId?: string;
  commands?: unknown;
  toolCallId?: string;
  title?: string;
  kind?: string;
  status?: string;
}

interface AcpChatPanelProps {
  project: ProjectRecord;
  chatId: string;
  config?: AppConfig;
  onClose?: () => void;
  disabled?: boolean;
  branchName?: string;
  draft?: string;
  onDraftChange?: (chatId: string, draft: string) => void;
}

interface QueuedPromptEditReturn {
  index: number;
  prompt: QueuedAcpPrompt;
}

function localTimelineId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

function withTimelineMessageChunk(session: AcpChatSession, role: 'user' | 'assistant' | 'system', text: string): AcpChatSession {
  const messages = [...session.messages];
  const lastMessage = messages[messages.length - 1];
  if (lastMessage && lastMessage.role === role) {
    messages[messages.length - 1] = { ...lastMessage, text: lastMessage.text + text };
  } else {
    messages.push({ role, text, timestamp: Date.now() });
  }

  const timeline = [...(session.timeline && session.timeline.length > 0 ? session.timeline : fallbackTimelineFromMessages(session.messages))];
  const lastTimeline = timeline[timeline.length - 1];
  if (lastTimeline && lastTimeline.type === 'message' && lastTimeline.role === role) {
    timeline[timeline.length - 1] = { ...lastTimeline, text: lastTimeline.text + text };
  } else {
    timeline.push({ id: localTimelineId(`${role}-message`), type: 'message', role, text, timestamp: Date.now() });
  }
  return { ...session, messages, timeline };
}

function withTimelineThinkingChunk(session: AcpChatSession, text: string): AcpChatSession {
  const timeline = [...(session.timeline && session.timeline.length > 0 ? session.timeline : fallbackTimelineFromMessages(session.messages))];
  const lastTimeline = timeline[timeline.length - 1];
  if (lastTimeline && lastTimeline.type === 'thinking') {
    timeline[timeline.length - 1] = { ...lastTimeline, text: lastTimeline.text + text };
  } else {
    timeline.push({ id: localTimelineId('thinking'), type: 'thinking', text, timestamp: Date.now() });
  }
  return { ...session, timeline };
}

function withTimelineTool(session: AcpChatSession, event: AcpPanelEvent): AcpChatSession {
  const toolCallId = event.toolCallId || localTimelineId('tool-call');
  const timeline = [...(session.timeline && session.timeline.length > 0 ? session.timeline : fallbackTimelineFromMessages(session.messages))];
  const existingIndex = timeline.findIndex((item) => item.type === 'tool' && item.toolCallId === toolCallId);
  const now = Date.now();
  const status = normalizeAcpTimelineToolStatus(event.status);
  if (existingIndex >= 0) {
    const current = timeline[existingIndex];
    if (current.type === 'tool') {
      timeline[existingIndex] = {
        ...current,
        title: event.title || current.title,
        kind: event.kind || current.kind,
        status,
        updatedAt: now,
      };
    }
  } else {
    timeline.push({
      id: localTimelineId('tool'),
      type: 'tool',
      toolCallId,
      title: event.title || '',
      kind: event.kind || '',
      status,
      startedAt: now,
      updatedAt: now,
    });
  }
  return { ...session, timeline };
}

export const AcpChatPanel: React.FC<AcpChatPanelProps> = ({ project, chatId, config, onClose, disabled = false, branchName, draft, onDraftChange }) => {
  const [session, setSession] = useState<AcpChatSession | null>(null);
  const [localInput, setLocalInput] = useState(draft ?? '');
  const input = localInput;
  const setInput = useCallback((value: string | ((prev: string) => string)) => {
    setLocalInput((prev) => {
      const next = typeof value === 'function' ? value(prev) : value;
      onDraftChange?.(chatId, next);
      return next;
    });
  }, [chatId, onDraftChange]);
  const [attachments, setAttachments] = useState<string[]>([]);
  const [modeDropdownOpen, setModeDropdownOpen] = useState(false);
  const [pendingQuestion, setPendingQuestion] = useState<OpenCodeQuestion | null>(null);
  const [selectedOptions, setSelectedOptions] = useState<string[]>([]);
  const [customAnswer, setCustomAnswer] = useState('');
  const [questionAnswers, setQuestionAnswers] = useState<Record<number, string>>({});
  const [slashCommandItemsState, setSlashCommandItemsState] = useState<AcpSlashCommandItem[]>([]);
  const [slashCommandSelectedIndex, setSlashCommandSelectedIndex] = useState(0);
  const [queuedPromptEditReturn, setQueuedPromptEditReturn] = useState<QueuedPromptEditReturn | null>(null);
  const [queueStatusMessage, setQueueStatusMessage] = useState<string | null>(null);
  const [queueExpanded, setQueueExpanded] = useState(true);
  const [dragSourceIndex, setDragSourceIndex] = useState<number | null>(null);
  const [dragTargetIndex, setDragTargetIndex] = useState<number | null>(null);
  const [queueDragSource, setQueueDragSource] = useState<number | null>(null);
  const [queueDragTarget, setQueueDragTarget] = useState<number | null>(null);
  const [changesRefreshKey, setChangesRefreshKey] = useState(0);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const modeDropdownRef = useRef<HTMLDivElement>(null);
  const customInputRef = useRef<HTMLInputElement>(null);

  const clearPendingInteraction = useCallback(() => {
    setPendingQuestion(null);
    setSelectedOptions([]);
    setCustomAnswer('');
    setQuestionAnswers({});
  }, []);

  const refreshSession = useCallback(async () => {
    const s = await api.invoke('acp:getSession', chatId) as AcpChatSession | null;
    setSession(s);
  }, [chatId]);

  const handleDragStart = useCallback((index: number) => {
    setDragSourceIndex(index);
  }, []);

  const handleDragOver = useCallback((_e: React.DragEvent, index: number) => {
    setDragTargetIndex(index);
  }, []);

  const handleDrop = useCallback((targetIndex: number) => {
    setSession(prev => {
      if (!prev || dragSourceIndex === null || dragSourceIndex === targetIndex) return prev;
      const timeline = [...(prev.timeline && prev.timeline.length > 0 ? prev.timeline : fallbackTimelineFromMessages(prev.messages))];
      const [moved] = timeline.splice(dragSourceIndex, 1);
      timeline.splice(targetIndex, 0, moved);
      return { ...prev, timeline };
    });
    setDragSourceIndex(null);
    setDragTargetIndex(null);
  }, [dragSourceIndex]);

  const handleDragEnd = useCallback(() => {
    setDragSourceIndex(null);
    setDragTargetIndex(null);
  }, []);

  const handleQueueDragStart = useCallback((index: number) => {
    setQueueDragSource(index);
  }, []);

  const handleQueueDragOver = useCallback((_e: React.DragEvent, index: number) => {
    setQueueDragTarget(index);
  }, []);

  const handleQueueDrop = useCallback(async (targetIndex: number) => {
    if (queueDragSource !== null && queueDragSource !== targetIndex) {
      await api.invoke('acp:queueMove', { chatId, fromIndex: queueDragSource, toIndex: targetIndex });
      await refreshSession();
    }
    setQueueDragSource(null);
    setQueueDragTarget(null);
  }, [chatId, queueDragSource, refreshSession]);

  const handleQueueDragEnd = useCallback(() => {
    setQueueDragSource(null);
    setQueueDragTarget(null);
  }, []);

  useEffect(() => {
    refreshSession();
    const controlsReady = actionControlsEnabled(session);
    const unsub = api.on('acp:event', (eventChatId: string, event: AcpPanelEvent) => {
      if (eventChatId !== chatId) return;

      // Handle high-frequency message chunks locally without re-fetching session
      if (event.type === 'messageChunk') {
        setSession((prev) => {
          if (!prev) return prev;
          const role = (event.role || 'assistant') as 'user' | 'assistant' | 'system';
          return withTimelineMessageChunk(prev, role, event.text || '');
        });
        return;
      }

      if (event.type === 'thinkingChunk') {
        setSession((prev) => {
          if (!prev) return prev;
          return withTimelineThinkingChunk(prev, event.text || '');
        });
        return;
      }

      if (event.type === 'toolCall') {
        setSession((prev) => {
          if (!prev) return prev;
          return withTimelineTool(prev, event);
        });
        setChangesRefreshKey((value) => value + 1);
        return;
      }

      if (event.type === 'toolCallUpdate') {
        setChangesRefreshKey((value) => value + 1);
      }

      if (event.type === 'promptResponse' || event.type === 'cancelled' || event.type === 'permissionResponse' || event.type === 'questionResponse') {
        setChangesRefreshKey((value) => value + 1);
      }

      refreshSession();
      if (event.type === 'permission') {
        setPendingQuestion({
          kind: 'permission',
          header: 'Permission Required',
          question: event.message || '',
          options: event.options || [],
          multiple: event.multiple ?? false,
          custom: event.custom ?? false,
          requestId: event.requestId || '',
          sessionId: event.sessionId || '',
        });
        setSelectedOptions([]);
        setCustomAnswer('');
        setQuestionAnswers({});
      }
      if (event.type === 'question') {
        const questions = event.questions && event.questions.length > 0
          ? event.questions
          : [{
              header: event.header || 'Question',
              question: event.question || '',
              options: event.options || [],
            }];
        setPendingQuestion({
          kind: 'question',
          header: event.header || questions[0]?.header || 'Question',
          question: event.question || questions[0]?.question || '',
          options: event.options || questions[0]?.options || [],
          questions,
          multiple: false,
          custom: questions.some((q) => q.options.length === 0),
          requestId: event.requestId || '',
          sessionId: event.sessionId || '',
        });
        setSelectedOptions([]);
        setCustomAnswer('');
        setQuestionAnswers({});
      }
      if (event.type === 'promptResponse' || event.type === 'cancelled' || event.type === 'permissionResponse' || event.type === 'questionResponse') {
        clearPendingInteraction();
      }
      if (event.type === 'commands' && event.commands) {
        setSlashCommandItemsState((prev) => {
          const next = slashCommandItemsForComposer(event.commands, inputRef.current?.value ?? '', controlsReady);
          return next.length === prev.length && next.every((item, index) => item.hint === prev[index]?.hint && item.description === prev[index]?.description) ? prev : next;
        });
        return;
      }
    });
    return () => { unsub(); };
  }, [chatId, refreshSession, clearPendingInteraction, session?.sessionId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [session?.messages, session?.timeline]);

  useEffect(() => {
    clearPendingInteraction();
    setQueuedPromptEditReturn(null);
    setQueueStatusMessage(null);
    setQueueExpanded(true);
  }, [chatId, clearPendingInteraction]);

  // Update slash hints when input changes
  useEffect(() => {
    if (!input.startsWith('/')) {
      setSlashCommandItemsState([]);
      setSlashCommandSelectedIndex(0);
      return;
    }
    const availableCommands = session?.availableCommands ?? [];
    const items = slashCommandItemsForComposer(availableCommands, input, actionControlsEnabled(session));
    setSlashCommandItemsState(items);
    setSlashCommandSelectedIndex((prev) => (prev >= items.length ? Math.max(0, items.length - 1) : prev));
  }, [input, session?.availableCommands, session?.sessionId]);

  // Clear blocked-edit message when the composer becomes empty
  useEffect(() => {
    if (queueStatusMessage && input.trim().length === 0 && attachments.length === 0 && !queuedPromptEditReturn) {
      setQueueStatusMessage(null);
    }
  }, [input, attachments, queueStatusMessage, queuedPromptEditReturn]);

  useEffect(() => {
    if (disabled) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        if (modeDropdownOpen) {
          setModeDropdownOpen(false);
          return;
        }
        if (slashCommandItemsState.length > 0) {
          setSlashCommandItemsState([]);
          setSlashCommandSelectedIndex(0);
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
          if (slashCommandItemsState.length > 0) {
            const selected = slashCommandItemsState[slashCommandSelectedIndex];
            if (selected) {
              setInput(selected.hint + ' ');
              setSlashCommandItemsState([]);
              setSlashCommandSelectedIndex(0);
            }
          } else {
            toggleMode();
          }
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [session?.status, disabled, modeDropdownOpen, slashCommandItemsState, slashCommandSelectedIndex]);

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
    const returnIndex = queuedPromptEditReturn?.index;
    await api.invoke('acp:send', { chatId, promptText: text, attachments, modeId: session?.currentModeId, returnIndex });
    setInput('');
    setAttachments([]);
    setQueuedPromptEditReturn(null);
    setQueueStatusMessage(null);
  }, [chatId, input, attachments, session?.currentModeId, queuedPromptEditReturn]);

  const cancelAcp = useCallback(async () => {
    await api.invoke('acp:cancel', chatId);
  }, [chatId]);

  const removeAttachment = useCallback((index: number) => {
    const path = attachments[index];
    const mention = pathToMention(path);
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
  const controlsEnabled = actionControlsEnabled(session);
  const currentMode = session?.currentModeId || 'build';
  const currentModel = session?.currentModel || 'Unknown';
  const currentEffort = session?.currentEffort || '';

  const modelLabel = currentModel ? `${currentModel} ${currentEffort}`.trim() : 'Model';

  const modelOptions = session?.configOptions?.find((o) => o.id === 'model');
  const effortOptions = session?.configOptions?.find((o) => o.id === 'effort');

  const resolvedConfig = config ?? defaultAppConfig();
  const favoriteModels = resolvedConfig.opencode.acpFavoriteModels;
  const knownModels = resolvedConfig.opencode.acpKnownModels;

  // Fallback to known models from config when ACP server hasn't sent options yet
  const effectiveModelOptions = modelOptions || {
    id: 'model',
    name: 'Model',
    category: 'model',
    currentValue: session?.currentModel || '',
    options: knownModels.map((m) => ({ value: m.value, label: m.name || m.value })),
  };

  const filteredModelOptions = effectiveModelOptions && favoriteModels.length > 0
    ? { ...effectiveModelOptions, options: effectiveModelOptions.options.filter((o) => favoriteModels.includes(o.value)) }
    : effectiveModelOptions;
  const filteredModelOptionValues = optionValues(filteredModelOptions);
  const effortOptionValues = optionValues(effortOptions);
  const configSelectorHasOptions = hasConfigSelectorOptions(filteredModelOptions, effortOptions);
  const filteredModelCurrentValue = filteredModelOptions?.currentValue ?? '';
  const effortCurrentValue = effortOptions?.currentValue ?? '';

  const setModel = (value: string) => {
    api.invoke('acp:setConfigOption', { chatId, configId: 'model', value });
  };

  const setEffort = (value: string) => {
    api.invoke('acp:setConfigOption', { chatId, configId: 'effort', value });
  };

  const branchNameDisplay = branchName || 'main';

  const isWelcome = shouldShowAcpWelcome(session?.messages, session?.queuedPrompts);
  const composerHint = acpComposerHintText({
    welcomeCenter: isWelcome,
    sessionReady: controlsEnabled,
    activeMode: currentMode,
  });
  const statusText = session ? acpStatusText(session.status) : 'Loading...';
  const headerStatusColor = session ? acpHeaderStatusColor(session.status) : 'rgb(138, 138, 138)';
  const statusModel = session?.currentModel?.trim()
    || (currentMode === 'plan' ? effectivePlanModel(resolvedConfig.opencode) : activeBuildModel(resolvedConfig.opencode));
  const kimiProtectionBadge = acpKimiProtectionBadge(statusModel, resolvedConfig.opencode.loopProtectionEnabled);
  const queuedPrompts = session?.queuedPrompts ?? [];
  const queuePanelVisible = queuedPrompts.length > 0 || queuedPromptEditReturn !== null;
  const queuePlanCount = acpQueuedPromptPlanCount(queuedPrompts);
  const queueHeaderLabel = acpQueuedPromptHeaderLabel(queuedPrompts.length, queuedPromptEditReturn?.index);
  const queueVisibleRows = acpQueuedPromptVisibleRowCount(queuedPrompts.length, queueExpanded);
  const queueRowsMaxHeight = queueVisibleRows * 35;
  const timelineItems = session?.timeline && session.timeline.length > 0
    ? session.timeline
    : fallbackTimelineFromMessages(session?.messages);

  const submitPendingInteraction = async () => {
    if (!pendingQuestion) return;
    if (pendingQuestion.kind === 'question') {
      const questions = pendingQuestion.questions && pendingQuestion.questions.length > 0
        ? pendingQuestion.questions
        : [{ header: pendingQuestion.header, question: pendingQuestion.question, options: pendingQuestion.options }];
      const answers = questions.map((q, idx) => {
        const answer = (questionAnswers[idx] || '').trim();
        if (q.options.length === 0) return answer ? [answer] : [];
        const selected = q.options.find((opt) => opt.id === answer);
        return selected ? [selected.label] : [];
      });
      const accepted = await api.invoke('acp:questionResponse', {
        chatId,
        requestId: pendingQuestion.requestId,
        answers,
        rejected: false,
      }) as boolean;
      if (accepted) clearPendingInteraction();
      return;
    }

    const answers = pendingQuestion.custom ? [...selectedOptions, customAnswer.trim()] : selectedOptions;
    const accepted = await api.invoke('acp:permissionResponse', {
      chatId,
      requestId: pendingQuestion.requestId,
      answers: answers.filter((a) => a.length > 0),
      rejected: false,
    }) as boolean;
    if (accepted) clearPendingInteraction();
  };

  const rejectPendingInteraction = async () => {
    if (!pendingQuestion) return;
    const channel = pendingQuestion.kind === 'question' ? 'acp:questionResponse' : 'acp:permissionResponse';
    const payload = pendingQuestion.kind === 'question'
      ? { chatId, requestId: pendingQuestion.requestId, answers: [], rejected: true }
      : { chatId, requestId: pendingQuestion.requestId, answers: [], rejected: true };
    const accepted = await api.invoke(channel, payload) as boolean;
    if (accepted) clearPendingInteraction();
  };

  const runQueuedPromptNext = useCallback(async (index: number) => {
    const accepted = await api.invoke('acp:queueRunNext', { chatId, index }) as boolean;
    if (accepted) await refreshSession();
  }, [chatId, refreshSession]);

  const deleteQueuedPrompt = useCallback(async (index: number) => {
    const accepted = await api.invoke('acp:queueDelete', { chatId, index }) as boolean;
    if (accepted) await refreshSession();
  }, [chatId, refreshSession]);

  const copyQueuedPrompt = useCallback(async (prompt: QueuedAcpPrompt) => {
    const text = prompt.finalPromptText.trim() || prompt.text.trim() || queuedPromptPreview(prompt);
    await api.invoke('clipboard:writeText', text);
  }, []);

  const editQueuedPrompt = useCallback(async (index: number, prompt: QueuedAcpPrompt) => {
    const blockedMessage = acpQueuedPromptDraftEditBlockedMessage({
      input,
      attachments,
      editingQueuedPrompt: queuedPromptEditReturn !== null,
    });
    if (blockedMessage) {
      setQueueStatusMessage(blockedMessage);
      inputRef.current?.focus();
      return;
    }

    const accepted = await api.invoke('acp:queueDelete', { chatId, index }) as boolean;
    if (!accepted) return;

    setInput(prompt.text);
    setAttachments([...prompt.attachments]);
    setQueuedPromptEditReturn({ index, prompt: { ...prompt, attachments: [...prompt.attachments] } });
    setQueueStatusMessage(null);
    if (prompt.modeId && prompt.modeId !== session?.currentModeId) {
      await api.invoke('acp:setConfigOption', { chatId, configId: 'mode', value: prompt.modeId });
    }
    await refreshSession();
    inputRef.current?.focus();
  }, [attachments, chatId, input, queuedPromptEditReturn, refreshSession, session?.currentModeId]);

  const cancelQueuedPromptEdit = useCallback(async () => {
    if (!queuedPromptEditReturn) return;
    const accepted = await api.invoke('acp:queueRestore', {
      chatId,
      index: queuedPromptEditReturn.index,
      prompt: queuedPromptEditReturn.prompt,
    }) as boolean;
    if (!accepted) return;

    setInput('');
    setAttachments([]);
    setQueuedPromptEditReturn(null);
    setQueueStatusMessage(null);
    await refreshSession();
    inputRef.current?.focus();
  }, [chatId, queuedPromptEditReturn, refreshSession]);

  const pendingQuestionSubmitEnabled = (() => {
    if (!pendingQuestion) return false;
    if (pendingQuestion.kind !== 'question') {
      return pendingQuestion.custom ? selectedOptions.length > 0 || customAnswer.trim().length > 0 : selectedOptions.length > 0;
    }
    const questions = pendingQuestion.questions && pendingQuestion.questions.length > 0
      ? pendingQuestion.questions
      : [{ header: pendingQuestion.header, question: pendingQuestion.question, options: pendingQuestion.options }];
    return questions.every((_, idx) => (questionAnswers[idx] || '').trim().length > 0);
  })();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100%', height: '100%', background: '#0c0c0c' }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222', flexShrink: 0 }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
          <span style={{ fontSize: 16, fontWeight: 700, color: '#f4f4f4', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {openCodeAcpPanelTitle(project.name, session?.tool)}
          </span>
          <span style={{ fontSize: 12, color: headerStatusColor, flexShrink: 0 }}>
            {statusText}
          </span>
        </span>
        {onClose && (
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
            ✕
          </button>
        )}
      </div>

      <div className="acp-workspace">
        <div className="acp-chat-column">
      {/* Main content area */}
      <div style={{ flex: 1, overflow: 'auto', padding: '8px 12px', display: 'flex', flexDirection: 'column' }}>
        {isWelcome ? (
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 16 }}>
            <div style={{ color: '#666', fontSize: 13, textAlign: 'center' }}>
              {openCodeAcpWelcomeText(session?.tool)}
            </div>
          </div>
        ) : (
          <div onDragEnd={handleDragEnd}>
            <AcpTimeline
              items={timelineItems}
              dragSourceIndex={dragSourceIndex}
              dragTargetIndex={dragTargetIndex}
              onDragStart={handleDragStart}
              onDragOver={handleDragOver}
              onDrop={handleDrop}
              onDragEnd={handleDragEnd}
            />
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Queued prompts — pinned above input */}
      {queuePanelVisible && (
        <div style={{ padding: '4px 12px', flexShrink: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, padding: '0 2px 5px', color: '#ffc864', fontSize: 11 }}>
            <span style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
              <span style={{ fontWeight: 700 }}>{queueHeaderLabel}</span>
              {queuePlanCount > 0 && <span style={{ color: '#777' }}>{queuePlanCount} Plan</span>}
            </span>
            {queuedPromptEditReturn ? (
              <button
                onClick={cancelQueuedPromptEdit}
                title="Cancel queued message edit"
                style={queuedPromptHeaderButtonStyle}
              >
                Cancel
              </button>
            ) : queuedPrompts.length > 0 ? (
              <button
                onClick={() => setQueueExpanded((value) => !value)}
                title="Collapse or expand queued ACP messages"
                style={queuedPromptHeaderButtonStyle}
              >
                {queueExpanded ? '▼' : '▲'}
              </button>
            ) : null}
          </div>
          {queueExpanded && queuedPrompts.length > 0 && (
            <div style={{ maxHeight: queueRowsMaxHeight, overflowY: queuedPrompts.length > ACP_QUEUED_PROMPT_MAX_VISIBLE_ROWS ? 'auto' : 'hidden' }}>
              {queuedPrompts.map((qp, i) => (
                <QueuedPromptRow
                  key={i}
                  index={i}
                  prompt={qp}
                  onRunNext={runQueuedPromptNext}
                  onCopy={copyQueuedPrompt}
                  onEdit={editQueuedPrompt}
                  onDelete={deleteQueuedPrompt}
                  isDragTarget={queueDragTarget === i && queueDragSource !== i}
                  onDragStart={handleQueueDragStart}
                  onDragOver={handleQueueDragOver}
                  onDrop={handleQueueDrop}
                  onDragEnd={handleQueueDragEnd}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {/* Composer area */}
      <div style={{ padding: '8px 12px', borderTop: '1px solid #222', flexShrink: 0 }}>
        {/* Slash hints above input */}
        {slashCommandItemsState.length > 0 && (
          <div style={slashCommandPopupStyle}>
            <div style={{ fontSize: 11, color: '#777', marginBottom: 4 }}>Commands:</div>
            <div style={{ maxHeight: 320, overflowY: 'auto' }}>
              {slashCommandItemsState.map((item, i) => (
              <button
                key={i}
                onMouseEnter={() => setSlashCommandSelectedIndex(i)}
                onClick={() => {
                  setInput(item.hint + ' ');
                  setSlashCommandItemsState([]);
                  setSlashCommandSelectedIndex(0);
                  inputRef.current?.focus();
                }}
                style={{
                  ...slashCommandRowStyle,
                  background: i === slashCommandSelectedIndex ? '#2a2a2a' : 'transparent',
                  borderRadius: 4,
                }}
              >
                <span style={{ color: '#6fb4ff', fontWeight: 600, flexShrink: 0 }}>{item.hint}</span>
                {item.description && (
                  <span style={{ color: '#888', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{item.description}</span>
                )}
              </button>
              ))}
            </div>
          </div>
        )}

        {/* Composer capsule */}
        <div style={{ display: 'flex', gap: 8, alignItems: 'center', background: '#1b1b1b', borderRadius: 12, padding: '8px 12px' }}>
          <button
            onClick={async () => {
              if (!controlsEnabled) return;
              const paths = await api.invoke('dialog:showOpen', { properties: ['openFile', 'multiSelections'] }) as string[] | undefined;
              if (paths) {
                setAttachments((prev) => [...prev, ...paths]);
                setInput((prev) => appendMentionsToInput(prev, paths));
              }
            }}
            disabled={!controlsEnabled}
            style={{
              width: 28,
              height: 28,
              borderRadius: '50%',
              background: '#1a1a1a',
              border: '1px solid #333',
              color: '#ccc',
              cursor: controlsEnabled ? 'pointer' : 'not-allowed',
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
              style={{ fontSize: 11, padding: '2px 8px', borderRadius: 4, border: '1px solid #333', background: '#1f3a4c', color: '#ccc', cursor: 'pointer', flexShrink: 0, height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center' }}
            >
              Plan
            </button>
          )}

          {/* Model + effort selector */}
          <div style={{ position: 'relative', flexShrink: 0 }} ref={modeDropdownRef}>
            <button
              onClick={() => {
                if (!controlsEnabled || !configSelectorHasOptions) return;
                setModeDropdownOpen((v) => !v);
              }}
              disabled={!controlsEnabled || !configSelectorHasOptions}
              style={{ fontSize: 11, padding: '2px 8px', borderRadius: 4, border: '1px solid #333', background: 'transparent', color: '#ccc', cursor: controlsEnabled && configSelectorHasOptions ? 'pointer' : 'not-allowed', maxWidth: 160, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', height: 28, display: 'flex', alignItems: 'center', justifyContent: 'center', opacity: controlsEnabled && configSelectorHasOptions ? 1 : 0.55 }}
            >
              {modelLabel}
            </button>
            {modeDropdownOpen && (
              <div style={{ position: 'absolute', bottom: '100%', left: 0, marginBottom: 4, background: '#1a1a1a', border: '1px solid #333', borderRadius: 6, padding: 8, minWidth: 200, zIndex: 10 }}>
                {filteredModelOptionValues.length > 0 && (
                  <div style={{ marginBottom: 8 }}>
                    <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Model</div>
                    {filteredModelOptionValues.map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => { setModel(opt.value); setModeDropdownOpen(false); }}
                        style={{ display: 'block', width: '100%', textAlign: 'left', fontSize: 11, padding: '4px 8px', background: filteredModelCurrentValue === opt.value ? '#1f3a4c' : 'transparent', border: 'none', color: '#ccc', cursor: 'pointer', borderRadius: 3 }}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                )}
                {effortOptionValues.length > 0 && (
                  <div>
                    <div style={{ fontSize: 10, color: '#888', marginBottom: 4 }}>Effort</div>
                    {effortOptionValues.map((opt) => (
                      <button
                        key={opt.value}
                        onClick={() => { setEffort(opt.value); setModeDropdownOpen(false); }}
                        style={{ display: 'block', width: '100%', textAlign: 'left', fontSize: 11, padding: '4px 8px', background: effortCurrentValue === opt.value ? '#1f3a4c' : 'transparent', border: 'none', color: '#ccc', cursor: 'pointer', borderRadius: 3 }}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                )}
                {filteredModelOptionValues.length === 0 && effortOptionValues.length === 0 && (
                  <div style={{ fontSize: 11, color: '#888' }}>No model options available.</div>
                )}
              </div>
            )}
          </div>

          <textarea
            ref={(el) => {
              // @ts-ignore
              inputRef.current = el;
              if (el) {
                el.style.height = 'auto';
                el.style.height = Math.min(el.scrollHeight, 100) + 'px';
              }
            }}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onInput={(e) => {
              const el = e.currentTarget;
              el.style.height = 'auto';
              el.style.height = Math.min(el.scrollHeight, 100) + 'px';
            }}
            onKeyDown={(e) => {
              if (slashCommandItemsState.length > 0) {
                if (e.key === 'ArrowDown') {
                  e.preventDefault();
                  setSlashCommandSelectedIndex((prev) => Math.min(prev + 1, slashCommandItemsState.length - 1));
                  return;
                }
                if (e.key === 'ArrowUp') {
                  e.preventDefault();
                  setSlashCommandSelectedIndex((prev) => Math.max(prev - 1, 0));
                  return;
                }
                if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
                  e.preventDefault();
                  const selected = slashCommandItemsState[slashCommandSelectedIndex];
                  if (selected) {
                    setInput(selected.hint + ' ');
                    setSlashCommandItemsState([]);
                    setSlashCommandSelectedIndex(0);
                  }
                  return;
                }
              }
              if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
                e.preventDefault();
                if (hasDraft) send();
              }
              if (e.key === 'Enter' && e.ctrlKey && !e.altKey && !e.metaKey) {
                e.preventDefault();
                setInput((prev) => prev + '\n');
              }
            }}
            placeholder={composerHint}
            style={{
              flex: 1,
              background: 'transparent',
              border: 'none',
              color: '#ccc',
              fontSize: 12,
              fontWeight: 600,
              resize: 'none',
              lineHeight: '20px',
              minHeight: 20,
              maxHeight: 100,
              outline: 'none',
              padding: '4px',
              transition: 'height 0.15s ease',
              overflowY: 'auto',
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

        {/* Attachment chips below capsule */}
        {attachments.length > 0 && (
          <div style={{ display: 'flex', gap: 4, marginTop: 6, flexWrap: 'wrap' }}>
            {attachments.map((a, i) => (
              <span key={i} style={{ fontSize: 11, color: '#b4b4b4', background: '#282828', padding: '2px 6px', borderRadius: 3, display: 'flex', alignItems: 'center', gap: 4, border: '1px solid #5a5a5a' }}>
                {a.split(/[/\\]/).pop() || a}
                <button onClick={() => removeAttachment(i)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}>✕</button>
              </span>
            ))}
          </div>
        )}

        {queueStatusMessage && (
          <div style={{ marginTop: 6, fontSize: 11, color: '#d0a35f' }}>
            {queueStatusMessage}
          </div>
        )}

        {/* Permission card below capsule */}
        {pendingQuestion && (
          <div style={{ marginTop: 6, padding: '8px 10px', borderRadius: 6, border: '1px solid #2a2a2a', background: '#171717', flexShrink: 0 }}>
            <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>{pendingQuestion.header}</div>
            {pendingQuestion.kind === 'question' ? (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                {(pendingQuestion.questions && pendingQuestion.questions.length > 0 ? pendingQuestion.questions : [{ header: pendingQuestion.header, question: pendingQuestion.question, options: pendingQuestion.options }]).map((q, idx) => (
                  <div key={idx} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {idx > 0 && <div style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>{q.header}</div>}
                    <div style={{ fontSize: 12, color: '#aaa' }}>{q.question}</div>
                    {q.options.length > 0 ? (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                        {q.options.map((opt) => (
                          <label key={`${idx}-${opt.label}`} style={{ display: 'flex', alignItems: 'flex-start', gap: 6, fontSize: 12, color: '#ccc', cursor: 'pointer' }}>
                            <input
                              type="radio"
                              name={`acp-question-${idx}`}
                              checked={questionAnswers[idx] === opt.id}
                              onChange={() => setQuestionAnswers((prev) => ({ ...prev, [idx]: opt.id }))}
                              style={{ marginTop: 2 }}
                            />
                            <span>
                              <span>{opt.label}</span>
                              {opt.description && <span style={{ display: 'block', color: '#777', fontSize: 11 }}>{opt.description}</span>}
                            </span>
                          </label>
                        ))}
                      </div>
                    ) : (
                      <input
                        value={questionAnswers[idx] || ''}
                        onChange={(e) => setQuestionAnswers((prev) => ({ ...prev, [idx]: e.target.value }))}
                        placeholder="Your answer..."
                        style={{ width: '100%', background: '#0c0c0c', border: '1px solid #333', color: '#ccc', fontSize: 12, padding: '4px 8px', borderRadius: 4 }}
                      />
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <>
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
              </>
            )}
            <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
              <button
                onClick={submitPendingInteraction}
                disabled={!pendingQuestionSubmitEnabled}
                style={{ fontSize: 12, padding: '4px 12px', borderRadius: 4, border: '1px solid #333', background: '#1f3a4c', color: '#ccc', cursor: pendingQuestionSubmitEnabled ? 'pointer' : 'not-allowed', opacity: pendingQuestionSubmitEnabled ? 1 : 0.55 }}
              >
                Submit
              </button>
              <button
                onClick={rejectPendingInteraction}
                style={{ fontSize: 12, padding: '4px 12px', borderRadius: 4, border: '1px solid #333', background: 'transparent', color: '#888', cursor: 'pointer' }}
              >
                Reject
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Status row */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, padding: '0 12px 6px', fontSize: 11, color: '#666', flexShrink: 0 }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{branchNameDisplay}</span>
          <span>Local</span>
          {kimiProtectionBadge && <span style={{ color: kimiProtectionBadge.color }}>{kimiProtectionBadge.label}</span>}
        </span>
        <span>{statusText}</span>
      </div>
        </div>
        <AcpChangesPanel repoPath={project.path} refreshKey={changesRefreshKey} />
      </div>
    </div>
  );
};

const QueuedPromptRow: React.FC<{
  index: number;
  prompt: QueuedAcpPrompt;
  onRunNext: (index: number) => void;
  onCopy: (prompt: QueuedAcpPrompt) => void;
  onEdit: (index: number, prompt: QueuedAcpPrompt) => void;
  onDelete: (index: number) => void;
  isDragTarget?: boolean;
  onDragStart?: (index: number) => void;
  onDragOver?: (e: React.DragEvent, index: number) => void;
  onDrop?: (index: number) => void;
  onDragEnd?: () => void;
}> = ({ index, prompt, onRunNext, onCopy, onEdit, onDelete, isDragTarget, onDragStart, onDragOver, onDrop, onDragEnd }) => {
  const modeLabel = acpModeUiLabel(prompt.modeId);
  const indexLabel = acpQueuedPromptIndexLabel(index);
  const attachmentLabel = acpQueuedPromptAttachmentLabel(prompt.attachments.length);
  const preview = queuedPromptPreview(prompt);
  return (
    <div
      style={{ display: 'flex', alignItems: 'center', gap: 6, minHeight: 32, padding: '4px 8px', background: '#1a1a1a', borderRadius: 6, marginBottom: 3, fontSize: 12, color: '#888', borderTop: isDragTarget ? '2px solid #4a9eff' : '2px solid transparent', transition: 'border-color 0.15s' }}
      onDragOver={(e) => { e.preventDefault(); onDragOver?.(e, index); }}
      onDrop={() => onDrop?.(index)}
    >
      {indexLabel && <span style={{ color: '#666', fontWeight: 600, flexShrink: 0 }}>{indexLabel}</span>}
      <span
        draggable
        onDragStart={() => onDragStart?.(index)}
        onDragEnd={() => onDragEnd?.()}
        style={{ cursor: 'grab', color: '#555', fontSize: 12, userSelect: 'none', lineHeight: 1, opacity: 0.6, flexShrink: 0 }}
        onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.opacity = '1'; (e.currentTarget as HTMLElement).style.color = '#999'; }}
        onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.opacity = '0.6'; (e.currentTarget as HTMLElement).style.color = '#555'; }}
        title="Drag to reorder"
      >
        ⋮⋮
      </span>
      {modeLabel && (
        <span style={{ fontSize: 10, color: '#dca046', flexShrink: 0 }}>{modeLabel}</span>
      )}
      <button
        onClick={() => onCopy(prompt)}
        title="Copy queued prompt"
        style={{ flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', textAlign: 'left', background: 'transparent', border: 'none', color: '#b4b4b4', font: 'inherit', padding: 0, cursor: 'copy' }}
      >
        {preview}
      </button>
      {attachmentLabel && (
        <span style={{ fontSize: 10, color: '#666', flexShrink: 0 }}>{attachmentLabel}</span>
      )}
      <button
        onClick={() => onEdit(index, prompt)}
        title="Edit"
        style={queuedPromptActionStyle}
      >
        ✎
      </button>
      <button
        onClick={() => onDelete(index)}
        title="Delete"
        style={queuedPromptActionStyle}
      >
        ✕
      </button>
      <button
        onClick={() => onRunNext(index)}
        title="Send now"
        style={{ ...queuedPromptActionStyle, color: '#ccc' }}
      >
        ↑
      </button>
    </div>
  );
};

const queuedPromptActionStyle: React.CSSProperties = {
  width: 24,
  height: 24,
  borderRadius: '50%',
  border: '1px solid #303030',
  background: 'transparent',
  color: '#888',
  cursor: 'pointer',
  fontSize: 12,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  flexShrink: 0,
};

const queuedPromptHeaderButtonStyle: React.CSSProperties = {
  border: 'none',
  background: 'transparent',
  color: '#aaa',
  cursor: 'pointer',
  fontSize: 11,
  padding: '2px 4px',
  flexShrink: 0,
};

const slashCommandPopupStyle: React.CSSProperties = {
  marginBottom: 6,
  borderRadius: 8,
  border: '1px solid #303030',
  background: '#121212',
  padding: '6px 10px',
  boxSizing: 'border-box',
};

const slashCommandRowStyle: React.CSSProperties = {
  width: '100%',
  minHeight: 28,
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  border: 'none',
  background: 'transparent',
  color: '#ccc',
  fontSize: 12,
  padding: '3px 0',
  cursor: 'pointer',
  textAlign: 'left',
};
