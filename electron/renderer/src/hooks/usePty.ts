import { useEffect, useRef, useCallback } from 'react';
import { AiCliTool as AiCliToolEnum } from '../../../shared/types';
import type { TerminalKind, ShellKind, AiHookEvent, AiCliTool, AiCliAttentionKind, SmartInputState, SmartInputAttachment, OpenCodeQuestion } from '../../../shared/types';
import type { ClaudeCodexHookProgress } from '../../../shared/claudeCodexHook';
import type { SmartInputModeId } from '../lib/smartInputMode';
import { normalizeSmartInputModeId, shouldSendOpenCodeModeToggle } from '../lib/smartInputMode';
import { canAutoDispatchClaude } from '../lib/smartInput';

const SMART_INPUT_AUTO_DISPATCH_SETTLE_MS = 300;
const OPENCODE_RUNNING_GRACE_MS = 2000;

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

export interface TerminalInstance {
  id: number;
  projectId: number;
  kind: TerminalKind;
  shell: ShellKind;
  cwd: string;
  cols: number;
  rows: number;
  title: string;
  pendingLineForTitle: string;
  pendingInputForHistory: string;
  recentInputs: string[];
  aiTool?: AiCliTool;
  aiStatus: string;
  aiStatusReason?: string;
  aiAttentionKind?: AiCliAttentionKind;
  claudeLaunchPending: boolean;
  claudeCodexHookProgress?: ClaudeCodexHookProgress;
  opencodeSessionActive: boolean;
  opencodeTransportStatus?: string;
  opencodeAttentionReason?: string;
  opencodePromptSubmitSince?: number;
  opencodeLastHookEventSince?: number;
  opencodePendingQuestion?: OpenCodeQuestion;
  opencodeQuestionFocusIndex: number;
  opencodeQuestionSelectedOptions: string[];
  opencodeQuestionCustomText: string;
  opencodeManualScrollDetached: boolean;
  opencodeLeadingBlankRows: number;
  opencodeThoughtLoopBlocked: boolean;
  opencodeLoopLimitEmitted: boolean;
  opencodeThinkingGuard?: string;
  opencodeLastKnownMode?: SmartInputModeId;
  smartInputState: SmartInputState;
  terminalOutputFocusOverride: boolean;
  pendingDelayedEnters: number[];
  exited: boolean;
  // Background rerun state machine
  pendingRerunPhase?: 'interrupt_sent' | 'batch_confirm_sent' | 'rerun_ready';
  pendingRerunSince?: number;
  pendingRerunCommand?: string;
  // Recent output buffer for Windows batch confirmation detection
  recentOutputBuffer: string;
}

function clearOpenCodeSessionState(t: TerminalInstance) {
  t.opencodeSessionActive = false;
  t.opencodeTransportStatus = undefined;
  t.opencodeAttentionReason = undefined;
  t.opencodePendingQuestion = undefined;
  t.opencodeQuestionFocusIndex = 0;
  t.opencodeQuestionSelectedOptions = [];
  t.opencodeQuestionCustomText = '';
  t.opencodeManualScrollDetached = false;
  t.opencodeLeadingBlankRows = 0;
  t.opencodeThoughtLoopBlocked = false;
  t.opencodeLoopLimitEmitted = false;
  t.opencodeThinkingGuard = undefined;
  t.opencodeLastHookEventSince = undefined;
}

export function usePty() {
  const terminalsRef = useRef<Map<number, TerminalInstance>>(new Map());
  const listenersRef = useRef<Set<() => void>>(new Set());

  const notify = useCallback(() => {
    listenersRef.current.forEach((cb) => cb());
  }, []);

  const defaultSmartInputState = (): SmartInputState => ({
    draftText: '',
    draftAttachments: [],
    queue: [],
    expanded: true,
    editText: '',
    editAttachments: [],
    editIndex: undefined,
    userHeight: undefined,
    draftUserHeight: undefined,
    draftContextMenuSelectionRange: undefined,
    editContextMenuSelectionRange: undefined,
  });

  const createTerminal = useCallback(async (opts: {
    shell: ShellKind;
    cwd: string;
    cols: number;
    rows: number;
    projectId: number;
    kind: TerminalKind;
    env?: Record<string, string>;
  }) => {
    console.log('[usePty] createTerminal called', opts);
    const id = await api.invoke('pty:create', opts) as number;
    console.log('[usePty] createTerminal received id', id);
    const t: TerminalInstance = {
      id,
      projectId: opts.projectId,
      kind: opts.kind,
      shell: opts.shell,
      cwd: opts.cwd,
      cols: opts.cols,
      rows: opts.rows,
      title: '',
      pendingLineForTitle: '',
      pendingInputForHistory: '',
      recentInputs: [],
      aiStatus: 'inactive',
      claudeLaunchPending: false,
      claudeCodexHookProgress: undefined,
      opencodeSessionActive: false,
      opencodeQuestionFocusIndex: 0,
      opencodeQuestionSelectedOptions: [],
      opencodeQuestionCustomText: '',
      opencodeManualScrollDetached: false,
      opencodeLeadingBlankRows: 0,
      opencodeThoughtLoopBlocked: false,
      opencodeLoopLimitEmitted: false,
      opencodeLastKnownMode: 'build',
      smartInputState: defaultSmartInputState(),
      terminalOutputFocusOverride: false,
      pendingDelayedEnters: [],
      opencodeLastHookEventSince: undefined,
      recentOutputBuffer: '',
      exited: false,
    };
    terminalsRef.current.set(id, t);
    notify();
    return id;
  }, [notify]);

  const writeTerminal = useCallback((terminalId: number, data: string) => {
    api.invoke('pty:write', terminalId, data);
  }, []);

  const markClaudeLaunchPending = useCallback((terminalId: number, title?: string) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.aiTool = AiCliToolEnum.Claude;
    t.aiStatus = 'inactive';
    t.aiStatusReason = undefined;
    t.aiAttentionKind = undefined;
    t.claudeLaunchPending = true;
    t.opencodePromptSubmitSince = undefined;
    if (title?.trim()) {
      t.title = title.trim();
    }
    notify();
  }, [notify]);

  const markLauncherAiTool = useCallback((terminalId: number, tool: AiCliTool, title?: string) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.aiTool = tool;
    t.aiStatus = 'inactive';
    t.aiStatusReason = undefined;
    t.aiAttentionKind = undefined;
    t.claudeLaunchPending = false;
    t.opencodePromptSubmitSince = undefined;
    clearOpenCodeSessionState(t);
    if (tool === AiCliToolEnum.Codex || tool === AiCliToolEnum.Droid) {
      t.smartInputState = defaultSmartInputState();
      t.pendingDelayedEnters = [];
    }
    if (title?.trim()) {
      t.title = title.trim();
    }
    notify();
  }, [notify]);

  const resizeTerminal = useCallback((terminalId: number, cols: number, rows: number) => {
    const t = terminalsRef.current.get(terminalId);
    if (t) {
      t.cols = cols;
      t.rows = rows;
    }
    api.invoke('pty:resize', terminalId, cols, rows);
  }, []);

  const killTerminal = useCallback((terminalId: number, signal?: string) => {
    api.invoke('pty:kill', terminalId, signal);
    terminalsRef.current.delete(terminalId);
    notify();
  }, [notify]);

  // Kimi thought-loop guard: sample terminal data while Working to detect repetitive thought text
  useEffect(() => {
    const thoughtBuffers = new Map<number, { buffer: string; lastSample: number }>();
    const unsubData = api.on('pty:data', (terminalId: number, data: string) => {
      const t = terminalsRef.current.get(terminalId);
      if (!t) return;

      // Accumulate recent output buffer for batch confirmation detection
      t.recentOutputBuffer += data;
      if (t.recentOutputBuffer.length > 4096) {
        t.recentOutputBuffer = t.recentOutputBuffer.slice(-4096);
      }

      // Background rerun: Windows batch confirmation detection
      if (t.pendingRerunPhase === 'interrupt_sent') {
        const lines = t.recentOutputBuffer.split(/\r?\n/);
        const lastNonEmpty = [...lines].reverse().find((l) => l.trim().length > 0);
        if (lastNonEmpty && lastNonEmpty.includes('Terminate batch job (Y/N)?')) {
          api.invoke('pty:write', terminalId, 'y\r');
          t.pendingRerunPhase = 'batch_confirm_sent';
          t.pendingRerunSince = Date.now();
          notify();
        }
      }

      // Track ED2 (erase display) sequences to count blank leading rows for OpenCode scroll clamping
      if (t.aiTool === 'opencode' && t.opencodeSessionActive) {
        const ed2Matches = data.match(/\x1b\[2J/g);
        if (ed2Matches) {
          // Each ED2 creates blank rows equal to terminal height
          t.opencodeLeadingBlankRows += ed2Matches.length * t.rows;
          // Cap to prevent unbounded growth
          if (t.opencodeLeadingBlankRows > 1000) {
            t.opencodeLeadingBlankRows = 1000;
          }
          notify();
        }
      }

      // Only sample while OpenCode is Working and session is active
      if (t.aiTool !== 'opencode' || !t.opencodeSessionActive) return;
      if (t.opencodeTransportStatus !== 'Working' && t.opencodeTransportStatus !== 'Attention') return;

      const now = Date.now();
      let entry = thoughtBuffers.get(terminalId);
      if (!entry) {
        entry = { buffer: '', lastSample: now };
        thoughtBuffers.set(terminalId, entry);
      }
      // Accumulate data
      entry.buffer += data;
      // Sample every 1 second
      if (now - entry.lastSample < 1000) return;
      entry.lastSample = now;
      const sample = entry.buffer;
      entry.buffer = '';

      // Detect Kimi thought-loop patterns
      const isKimi = t.opencodeThinkingGuard?.toLowerCase().includes('kimi') ?? false;
      // Check for repetitive thought text
      const thoughtMatches = sample.match(/<thinking>[\s\S]*?<\/thinking>/g);
      if (thoughtMatches && thoughtMatches.length > 2) {
        // Multiple thinking blocks in one sample = possible loop
        t.opencodeThoughtLoopBlocked = true;
        notify();
      }
      // Check for loop limit messages from CLI output
      if (sample.includes('loop limit reached') || sample.includes('thinking loop detected') || sample.includes('LoopLimitExceeded')) {
        t.opencodeLoopLimitEmitted = true;
        t.opencodeThoughtLoopBlocked = true;
        notify();
      }
      if (thoughtMatches && thoughtMatches.length > 0) {
        t.opencodeThinkingGuard = 'kimi'; // Mark as Kimi-family model
      }
    });
    const unsubExit = api.on('pty:exit', (terminalId: number, _exitCode: number) => {
      const t = terminalsRef.current.get(terminalId);
      if (t) {
        t.exited = true;
        notify();
        // Remove after a short delay so UI can render exited state
        setTimeout(() => {
          terminalsRef.current.delete(terminalId);
          notify();
        }, 5000);
      } else {
        terminalsRef.current.delete(terminalId);
        notify();
      }
    });
    const unsubState = api.on('pty:state', (terminalId: number, state: { recentInputs?: string[]; title?: string }) => {
      const t = terminalsRef.current.get(terminalId);
      if (!t) return;
      if (state.recentInputs) {
        // Merge main process recentInputs with local recentInputs to preserve Smart Input commands
        const remote = state.recentInputs;
        const local = t.recentInputs.filter((item) => !remote.includes(item));
        t.recentInputs = [...remote, ...local].slice(0, 20);
      }
      if (state.title !== undefined) t.title = state.title;
      notify();
    });
    const unsubHook = api.on('hook:status', (event: AiHookEvent) => {
      const t = terminalsRef.current.get(event.terminalId);
      if (!t) return;
      t.aiTool = event.tool;
      t.aiStatus = event.status;
      t.aiStatusReason = event.reason;
      t.aiAttentionKind = event.attentionKind;
      if (event.tool === 'opencode') {
        const isWorking = event.status === 'running' || event.status === 'attention';
        if (isWorking) {
          t.opencodeSessionActive = true;
          t.opencodeTransportStatus = event.status === 'running' ? 'Working' : 'Attention';
          t.opencodeAttentionReason = event.attentionKind || undefined;
        } else if (event.status === 'inactive') {
          t.opencodeTransportStatus = 'Idle';
          t.opencodeAttentionReason = undefined;
          t.opencodeSessionActive = false;
          t.opencodeLeadingBlankRows = 0;
          t.opencodeManualScrollDetached = false;
        }
        if (event.eventKind && event.eventKind.includes('question.asked') && event.rawJson) {
          try {
            const q = JSON.parse(event.rawJson) as { question?: OpenCodeQuestion };
            if (q.question) t.opencodePendingQuestion = q.question;
          } catch { /* ignore */ }
        }
        if (event.eventKind && event.eventKind.includes('permission.asked') && event.rawJson) {
          try {
            const p = JSON.parse(event.rawJson) as { permission?: { requestId: string; message: string; options?: string[] } };
            if (p.permission) {
              t.opencodePendingQuestion = {
                header: 'Permission Request',
                question: p.permission.message,
                options: (p.permission.options || []).map((o, i) => ({ id: String(i), label: o })),
                multiple: false,
                custom: false,
                requestId: String(p.permission.requestId),
                sessionId: '',
              };
            }
          } catch { /* ignore */ }
        }
        if (event.eventKind && event.eventKind.includes('plan_mode_prompt') && event.rawJson) {
          try {
            const pm = JSON.parse(event.rawJson) as { planModePrompt?: { message: string } };
            if (pm.planModePrompt) {
              t.opencodePendingQuestion = {
                header: 'Plan Mode',
                question: pm.planModePrompt.message,
                options: [],
                multiple: false,
                custom: true,
                requestId: '',
                sessionId: '',
              };
            }
          } catch { /* ignore */ }
        }
        if (event.status === 'running' || event.status === 'attention') {
          t.opencodeLastHookEventSince = Date.now();
        }
        if (event.status === 'running') {
          // Clear pending question on Working transition
          t.aiAttentionKind = undefined;
          t.opencodePendingQuestion = undefined;
          t.opencodeQuestionSelectedOptions = [];
          t.opencodeQuestionCustomText = '';
          t.opencodeQuestionFocusIndex = 0;
        }
        if (event.status === 'running' && event.reason === 'PromptSubmit') {
          t.opencodePromptSubmitSince = Date.now();
          // Reset thought-loop guard on new prompt submit
          t.opencodeThoughtLoopBlocked = false;
          t.opencodeLoopLimitEmitted = false;
          t.opencodeThinkingGuard = undefined;
        }
        if (event.status === 'attention' && event.attentionKind === 'turn_complete') {
          t.opencodeTransportStatus = 'Idle';
          t.opencodeAttentionReason = 'turn_complete';
          // Clear thought-loop guard on turn complete
          t.opencodeThoughtLoopBlocked = false;
          t.opencodeLoopLimitEmitted = false;
        }
        if (event.status === 'attention' && event.attentionKind === 'session_error') {
          t.opencodeTransportStatus = 'Idle';
          t.opencodeAttentionReason = 'session_error';
          t.opencodeSessionActive = false;
          t.opencodeThoughtLoopBlocked = false;
          t.opencodeLoopLimitEmitted = false;
          t.opencodeLeadingBlankRows = 0;
          t.opencodeManualScrollDetached = false;
        }
        // Detect loop limit from hook reason
        if (event.reason && (event.reason.includes('loop_limit') || event.reason.includes('LoopLimitExceeded'))) {
          t.opencodeLoopLimitEmitted = true;
          t.opencodeThoughtLoopBlocked = true;
        }
      }
      if (event.tool === 'codex' || event.tool === 'droid') {
        clearOpenCodeSessionState(t);
      }
      if (event.tool === 'claude') {
        t.claudeLaunchPending = false;
        if (event.status === 'running') {
          t.aiAttentionKind = undefined;
        }
      }
      notify();
    });
    return () => {
      unsubData();
      unsubExit();
      unsubState();
      unsubHook();
    };
  }, [notify]);

  const writeSmartInputPayload = useCallback((terminal: TerminalInstance, text: string, attachments: SmartInputAttachment[], modeId?: string): number => {
    const targetMode = normalizeSmartInputModeId(modeId);
    const writePayload = () => {
      for (const a of attachments) {
        api.invoke('pty:write', terminal.id, `\x1b[200~${a.path}\x1b[201~`);
      }
      if (text) {
        api.invoke('pty:write', terminal.id, `\x1b[200~${text}\x1b[201~`);
      }
      api.invoke('pty:write', terminal.id, '\r');
    };

    if (terminal.aiTool === 'opencode' && terminal.opencodeSessionActive && shouldSendOpenCodeModeToggle(terminal.opencodeLastKnownMode, targetMode)) {
      api.invoke('pty:write', terminal.id, '\t');
      terminal.opencodeLastKnownMode = targetMode;
      window.setTimeout(writePayload, 150);
      return 150;
    }

    terminal.opencodeLastKnownMode = targetMode;
    writePayload();
    return 0;
  }, []);

  const pushRecentInput = useCallback((terminalId: number, message: string) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    const trimmed = message.trim();
    if (!trimmed || trimmed.startsWith('/')) return;
    t.recentInputs.unshift(trimmed);
    if (t.recentInputs.length > 20) {
      t.recentInputs.pop();
    }
    notify();
  }, [notify]);

  // Auto-dispatch timer for Smart Input After Done tasks + delayed enters
  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now();
      for (const t of terminalsRef.current.values()) {
        // Process delayed enters first
        if (t.pendingDelayedEnters.length > 0) {
          const due = t.pendingDelayedEnters.filter((at) => now >= at);
          if (due.length > 0) {
            t.pendingDelayedEnters = t.pendingDelayedEnters.filter((at) => now < at);
            for (const _ of due) {
              api.invoke('pty:write', t.id, '\r');
            }
          }
        }

        // Background rerun state machine
        if (t.kind === 'background' && t.pendingRerunPhase) {
          const phase = t.pendingRerunPhase;
          const since = t.pendingRerunSince ?? 0;
          const cmd = t.pendingRerunCommand;
          if (phase === 'batch_confirm_sent') {
            // Wait for batch confirmation to settle before replaying command
            if (now - since >= 200) {
              t.pendingRerunPhase = 'rerun_ready';
              t.pendingRerunSince = now;
            }
          } else if (phase === 'rerun_ready' && cmd) {
            // Replay command with bracketed paste + Enter
            api.invoke('pty:write', t.id, `\x1b[200~${cmd}\x1b[201~`);
            api.invoke('pty:write', t.id, '\r');
            // Schedule single confirmation Enter after 1000ms
            t.pendingDelayedEnters.push(now + 1000);
            // Clear rerun state
            t.pendingRerunPhase = undefined;
            t.pendingRerunSince = undefined;
            t.pendingRerunCommand = undefined;
            notify();
          }
        }

        if (t.kind !== 'foreground') continue;
        const isOpenCodeSmartInput = t.aiTool === 'opencode' && t.opencodeSessionActive;
        const isClaudeSmartInput = t.aiTool === 'claude' && (t.aiStatus === 'running' || t.aiStatus === 'attention');
        if (!isOpenCodeSmartInput && !isClaudeSmartInput) continue;
        if (isOpenCodeSmartInput && t.opencodePendingQuestion) continue;
        if (t.smartInputState.queue.length === 0) continue;

        // Kimi thought-loop guard blocks auto-dispatch only
        if (isOpenCodeSmartInput && (t.opencodeThoughtLoopBlocked || t.opencodeLoopLimitEmitted)) continue;

        // Stale Working recovery: clear stale Working if no hook event for 6s
        if (isOpenCodeSmartInput && t.opencodeTransportStatus === 'Working') {
          const lastHook = t.opencodeLastHookEventSince ?? 0;
          if (now - lastHook > OPENCODE_RUNNING_GRACE_MS * 3) {
            t.opencodeTransportStatus = 'Idle';
            notify();
          }
        }

        if (isOpenCodeSmartInput) {
          // Auto-dispatch only when Idle AND the last attention reason was TurnComplete
          const isIdle = t.opencodeTransportStatus === 'Idle';
          const hasTurnComplete = t.opencodeAttentionReason === 'turn_complete';
          if (!isIdle || !hasTurnComplete) continue;

          // Settle guard: suppress stale Idle within 300ms of prompt submit
          const submitSince = t.opencodePromptSubmitSince ?? 0;
          if (now - submitSince < SMART_INPUT_AUTO_DISPATCH_SETTLE_MS) continue;
        } else if (!canAutoDispatchClaude(t.smartInputState.queue, t.aiStatus, t.aiAttentionKind, t.opencodePromptSubmitSince, now)) {
          continue;
        }

        // Auto-dispatch the next task
        const task = t.smartInputState.queue[0];
        const nextQueue = t.smartInputState.queue.slice(1);
        t.smartInputState = {
          ...t.smartInputState,
          queue: nextQueue,
        };
        t.opencodePromptSubmitSince = now;
        if (isOpenCodeSmartInput) {
          t.opencodeTransportStatus = 'Working';
        } else {
          t.aiStatus = 'running';
          t.aiAttentionKind = undefined;
        }

        // Clear previous delayed enters
        t.pendingDelayedEnters = [];

        const payloadDelay = writeSmartInputPayload(t, task.text, task.attachments, task.modeId);

        // Record recent input directly (bracketed paste is filtered from PTY history tracking)
        if (task.text.trim()) {
          pushRecentInput(t.id, task.text);
        }

        // Schedule two confirmation Enters after 600ms and 1200ms
        t.pendingDelayedEnters.push(now + payloadDelay + 600);
        t.pendingDelayedEnters.push(now + payloadDelay + 1200);
        notify();
      }
    }, 100);
    return () => clearInterval(interval);
  }, [notify, pushRecentInput, writeSmartInputPayload]);

  const getTerminals = useCallback(() => {
    return Array.from(terminalsRef.current.values());
  }, []);

  const subscribe = useCallback((cb: () => void) => {
    listenersRef.current.add(cb);
    return () => { listenersRef.current.delete(cb); };
  }, []);

  const updateSmartInputState = useCallback((terminalId: number, state: Partial<SmartInputState>) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.smartInputState = { ...t.smartInputState, ...state };
    notify();
  }, [notify]);

  const setTerminalOutputFocusOverride = useCallback((terminalId: number, value: boolean) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.terminalOutputFocusOverride = value;
    notify();
  }, [notify]);

  const updateOpencodeManualScrollDetached = useCallback((terminalId: number, detached: boolean) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.opencodeManualScrollDetached = detached;
    notify();
  }, [notify]);

  const setOpencodeSessionActive = useCallback((terminalId: number, value: boolean) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.opencodeSessionActive = value;
    notify();
  }, [notify]);

  const updateQuestionState = useCallback((terminalId: number, updates: { focusIndex?: number; selectedOptions?: string[]; customText?: string }) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    if (updates.focusIndex !== undefined) t.opencodeQuestionFocusIndex = updates.focusIndex;
    if (updates.selectedOptions !== undefined) t.opencodeQuestionSelectedOptions = updates.selectedOptions;
    if (updates.customText !== undefined) t.opencodeQuestionCustomText = updates.customText;
    notify();
  }, [notify]);

  const updateClaudeCodexHookProgress = useCallback((terminalId: number, progress?: ClaudeCodexHookProgress) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    t.claudeCodexHookProgress = progress;
    notify();
  }, [notify]);

  const sendSmartInputToTerminal = useCallback((terminalId: number, text: string, attachments: SmartInputAttachment[], modeId: SmartInputModeId = 'build') => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    const now = Date.now();

    // Track prompt submit for settle guard
    t.opencodePromptSubmitSince = now;
    if (t.aiTool === 'opencode') {
      t.opencodeTransportStatus = 'Working';
    }
    if (t.aiTool === 'claude') {
      t.aiStatus = 'running';
      t.aiAttentionKind = undefined;
      t.claudeLaunchPending = false;
    }

    // Clear previous delayed enters
    t.pendingDelayedEnters = [];

    const payloadDelay = writeSmartInputPayload(t, text, attachments, modeId);

    // Record recent input directly (bracketed paste is filtered from PTY history tracking)
    if (text.trim()) {
      pushRecentInput(terminalId, text);
    }

    // Schedule two confirmation Enters (600ms and 1200ms) for all Smart Input dispatches
    t.pendingDelayedEnters.push(now + payloadDelay + 600);
    t.pendingDelayedEnters.push(now + payloadDelay + 1200);
    notify();
  }, [notify, pushRecentInput, writeSmartInputPayload]);

  const sendShortcutToTerminal = useCallback((terminalId: number, command: string) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    const now = Date.now();

    // Track prompt submit for settle guard
    t.opencodePromptSubmitSince = now;
    if (t.aiTool === 'opencode') {
      t.opencodeTransportStatus = 'Working';
    }
    if (t.aiTool === 'claude') {
      t.aiStatus = 'running';
      t.aiAttentionKind = undefined;
      t.claudeLaunchPending = false;
    }

    // Clear previous delayed enters
    t.pendingDelayedEnters = [];

    // Send command with bracketed paste
    api.invoke('pty:write', terminalId, `\x1b[200~${command}\x1b[201~`);
    api.invoke('pty:write', terminalId, '\r');

    // Record recent input directly (bracketed paste is filtered from PTY history tracking)
    pushRecentInput(terminalId, command);

    // Schedule delayed confirmation Enters
    // Slash-prefixed commands get two staggered confirmation Enters (600ms + 1200ms)
    // Non-slash commands get a single delayed Enter after 1200ms
    const isSlashCommand = command.trim().startsWith('/');
    if (isSlashCommand) {
      t.pendingDelayedEnters.push(now + 600);
      t.pendingDelayedEnters.push(now + 1200);
    } else {
      t.pendingDelayedEnters.push(now + 1200);
    }
    notify();
  }, [notify, pushRecentInput]);

  const sendSavedMessageToTerminal = useCallback((terminalId: number, message: string, recordRecentInput: boolean) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    const now = Date.now();

    // Track prompt submit for settle guard
    t.opencodePromptSubmitSince = now;
    if (t.aiTool === 'opencode') {
      t.opencodeTransportStatus = 'Working';
    }
    if (t.aiTool === 'claude') {
      t.aiStatus = 'running';
      t.aiAttentionKind = undefined;
      t.claudeLaunchPending = false;
    }

    // Clear previous delayed enters
    t.pendingDelayedEnters = [];

    // Send with bracketed paste
    api.invoke('pty:write', terminalId, `\x1b[200~${message}\x1b[201~`);
    api.invoke('pty:write', terminalId, '\r');

    if (recordRecentInput && message.trim()) {
      pushRecentInput(terminalId, message);
    }

    // Schedule delayed confirmation Enters
    // Slash-prefixed commands get two staggered confirmation Enters (600ms + 1200ms)
    // Non-slash commands get a single delayed Enter after 1200ms
    const isSlashCommand = message.trim().startsWith('/');
    if (isSlashCommand) {
      t.pendingDelayedEnters.push(now + 600);
      t.pendingDelayedEnters.push(now + 1200);
    } else {
      t.pendingDelayedEnters.push(now + 1200);
    }
    notify();
  }, [notify, pushRecentInput]);

  const rerunBackground = useCallback((terminalId: number) => {
    const t = terminalsRef.current.get(terminalId);
    if (!t) return;
    const now = Date.now();
    const cmd = t.recentInputs[0];
    if (!cmd) return;

    if (t.aiStatus === 'running') {
      // Send Ctrl+C, then enter batch detection state machine
      api.invoke('pty:write', terminalId, '\x03');
      t.pendingRerunPhase = 'interrupt_sent';
      t.pendingRerunSince = now;
      t.pendingRerunCommand = cmd;
    } else {
      // Idle: send command immediately
      t.pendingRerunCommand = cmd;
      t.pendingRerunPhase = 'rerun_ready';
      t.pendingRerunSince = now;
    }
    notify();
  }, [notify]);

  return {
    createTerminal,
    writeTerminal,
    markClaudeLaunchPending,
    markLauncherAiTool,
    resizeTerminal,
    killTerminal,
    getTerminals,
    subscribe,
    updateSmartInputState,
    setTerminalOutputFocusOverride,
    updateOpencodeManualScrollDetached,
    setOpencodeSessionActive,
    updateQuestionState,
    updateClaudeCodexHookProgress,
    sendSmartInputToTerminal,
    sendShortcutToTerminal,
    sendSavedMessageToTerminal,
    rerunBackground,
  };
}
