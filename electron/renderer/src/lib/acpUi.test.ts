import { describe, expect, it } from 'vitest';
import {
  ACP_CHAT_TITLE_MAX_CHARS,
  ACP_QUEUED_PROMPT_MAX_VISIBLE_ROWS,
  ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS,
  OPENCODE_ACP_CLOSE_TOOLTIP,
  OPENCODE_ACP_LABEL,
  OPENCODE_ACP_OPEN_BUTTON_LABEL,
  actionControlsEnabled,
  acpComposerHintText,
  acpChatDisplayTitle,
  acpChatHasStartedState,
  acpChatTitleFromPrompt,
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
  acpTerminalManagerBadgeVisual,
  acpTerminalManagerRowLabel,
  hasConfigSelectorOptions,
  moveQueuedPromptToFront,
  nextAcpTerminalManagerAttention,
  nextAcpActivityState,
  openCodeAcpPanelTitle,
  openCodeAcpWelcomeText,
  queuedPromptPreview,
  removeQueuedPromptAt,
  shouldShowAcpWelcome,
  slashCommandHint,
  slashCommandHints,
  slashCommandItems,
  slashCommandItemsForComposer,
  slashCommandItemsForInput,
} from './acpUi';

describe('acpUi', () => {
  it('uses OpenCode ACP as the canonical user-facing label', () => {
    expect(OPENCODE_ACP_LABEL).toBe('OpenCode ACP');
    expect(OPENCODE_ACP_OPEN_BUTTON_LABEL).toBe('+ ACP');
    expect(OPENCODE_ACP_CLOSE_TOOLTIP).toBe('Close OpenCode ACP');
    expect(openCodeAcpPanelTitle('Mergen')).toBe('OpenCode ACP');
    expect(openCodeAcpWelcomeText()).toBe('Welcome to OpenCode ACP');
  });

  it('derives ACP chat titles from prompts like Rust', () => {
    expect(acpChatTitleFromPrompt('')).toBe(OPENCODE_ACP_LABEL);
    expect(acpChatTitleFromPrompt('  hello\n\nworld  ')).toBe('hello world');
    const maxTitle = 'x'.repeat(ACP_CHAT_TITLE_MAX_CHARS);
    expect(acpChatTitleFromPrompt(maxTitle)).toBe(maxTitle);
    expect(acpChatTitleFromPrompt(`${maxTitle}y`)).toBe(`${maxTitle}...`);
    expect(acpChatTitleFromPrompt(`${'ş'.repeat(ACP_CHAT_TITLE_MAX_CHARS)}z`)).toBe(`${'ş'.repeat(ACP_CHAT_TITLE_MAX_CHARS)}...`);
  });

  it('formats ACP chat display and Terminal Manager labels like Rust', () => {
    const emptySession = { messages: [], queuedPrompts: [], title: 'first prompt' };
    const startedSession = { messages: [{ role: 'user' as const, text: 'hello', timestamp: 1 }], queuedPrompts: [], title: 'first prompt' };
    const queuedSession = { messages: [], queuedPrompts: [{ modeId: 'build' }], title: 'queued prompt' };

    expect(acpChatHasStartedState(emptySession)).toBe(false);
    expect(acpChatDisplayTitle(emptySession)).toBe(OPENCODE_ACP_LABEL);
    expect(acpTerminalManagerRowLabel(emptySession)).toBe(OPENCODE_ACP_LABEL);
    expect(acpChatHasStartedState(startedSession)).toBe(true);
    expect(acpChatDisplayTitle(startedSession)).toBe('first prompt');
    expect(acpTerminalManagerRowLabel(startedSession)).toBe('OpenCode ACP - first prompt');
    expect(acpTerminalManagerRowLabel(queuedSession)).toBe('OpenCode ACP - queued prompt');
  });

  it('keeps ACP running true after promptSent events without a status field', () => {
    const next = nextAcpActivityState({ running: false, hasQueuedPrompts: false }, { type: 'promptSent' });
    expect(next.running).toBe(true);
  });

  it('keeps the previous running state for queued events without status', () => {
    const next = nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'queued', count: 2 });
    expect(next.running).toBe(true);
    expect(next.hasQueuedPrompts).toBe(true);
  });

  it('clears ACP running on terminal response and cancel events', () => {
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'promptResponse' }).running).toBe(false);
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'cancelled' }).running).toBe(false);
  });

  it('keeps ACP running for non-fatal stderr and warning events', () => {
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'stderr' }).running).toBe(true);
    expect(nextAcpActivityState({ running: true, hasQueuedPrompts: false }, { type: 'warning' }).running).toBe(true);
  });

  it('maps ACP status values to Rust-style display text', () => {
    expect(acpStatusText('starting')).toBe('Starting...');
    expect(acpStatusText('connected')).toBe('Starting...');
    expect(acpStatusText('session_created')).toBe('Starting...');
    expect(acpStatusText('idle')).toBe('Idle');
    expect(acpStatusText('running')).toBe('Running...');
    expect(acpStatusText('permission')).toBe('Permission');
    expect(acpStatusText('error')).toBe('Error');
    expect(acpStatusText(undefined)).toBe('Idle');
  });

  it('maps ACP header status colors to Rust-style state colors', () => {
    expect(acpHeaderStatusColor('running')).toBe('rgb(100, 200, 100)');
    expect(acpHeaderStatusColor('permission')).toBe('rgb(255, 200, 100)');
    expect(acpHeaderStatusColor('error')).toBe('rgb(185, 45, 45)');
    expect(acpHeaderStatusColor('idle')).toBe('rgb(138, 138, 138)');
    expect(acpHeaderStatusColor('starting')).toBe('rgb(138, 138, 138)');
    expect(acpHeaderStatusColor(undefined)).toBe('rgb(138, 138, 138)');
  });

  it('maps ACP Terminal Manager badge visuals to Rust status and attention states', () => {
    expect(acpTerminalManagerBadgeVisual('starting')).toEqual({ kind: 'spinner', color: 'rgb(170, 170, 170)' });
    expect(acpTerminalManagerBadgeVisual('connected')).toEqual({ kind: 'spinner', color: 'rgb(170, 170, 170)' });
    expect(acpTerminalManagerBadgeVisual('session_created')).toEqual({ kind: 'spinner', color: 'rgb(170, 170, 170)' });
    expect(acpTerminalManagerBadgeVisual('running')).toEqual({ kind: 'spinner', color: 'rgb(170, 170, 170)' });
    expect(acpTerminalManagerBadgeVisual('permission', 'permission')).toEqual({ kind: 'pulse', color: 'rgb(210, 170, 80)' });
    expect(acpTerminalManagerBadgeVisual('permission')).toEqual({ kind: 'solid', color: 'rgb(210, 170, 80)' });
    expect(acpTerminalManagerBadgeVisual('idle', 'turn_complete')).toEqual({ kind: 'pulse', color: 'rgb(90, 185, 90)' });
    expect(acpTerminalManagerBadgeVisual('idle')).toBeUndefined();
    expect(acpTerminalManagerBadgeVisual('error')).toEqual({ kind: 'solid', color: 'rgb(170, 50, 50)' });
    expect(acpTerminalManagerBadgeVisual(undefined)).toBeUndefined();
  });

  it('updates ACP Terminal Manager attention state like Rust ACP events', () => {
    expect(nextAcpTerminalManagerAttention(undefined, { type: 'permission' })).toBe('permission');
    expect(nextAcpTerminalManagerAttention('permission', { type: 'permissionResponse' })).toBeUndefined();
    expect(nextAcpTerminalManagerAttention(undefined, { type: 'promptResponse', queuedPrompts: 0 })).toBe('turn_complete');
    expect(nextAcpTerminalManagerAttention(undefined, { type: 'promptResponse', queuedPrompts: 1 })).toBeUndefined();
    expect(nextAcpTerminalManagerAttention('turn_complete', { type: 'promptSent' })).toBeUndefined();
    expect(nextAcpTerminalManagerAttention('permission', { type: 'stderr' })).toBe('permission');
  });

  it('does not show welcome while queued prompts are visible', () => {
    expect(shouldShowAcpWelcome([], [{ text: 'queued' }])).toBe(false);
    expect(shouldShowAcpWelcome([], [])).toBe(true);
  });

  it('opens config selector only when model or effort options exist', () => {
    expect(hasConfigSelectorOptions(undefined, undefined)).toBe(false);
    expect(hasConfigSelectorOptions({ id: 'model', name: 'Model', category: 'model', currentValue: '', options: [] }, undefined)).toBe(false);
    expect(hasConfigSelectorOptions(undefined, { id: 'effort', name: 'Effort', category: 'effort', currentValue: '', options: [{ label: 'High', value: 'high' }] })).toBe(true);
  });

  it('enables action controls only after ACP session id exists', () => {
    expect(actionControlsEnabled(null)).toBe(false);
    expect(actionControlsEnabled({ sessionId: undefined })).toBe(false);
    expect(actionControlsEnabled({ sessionId: 'sess-1' })).toBe(true);
  });

  it('matches Rust ACP mode labels for composer and queued rows', () => {
    expect(acpModeUiLabel('plan')).toBe('Plan');
    expect(acpModeUiLabel('build')).toBeUndefined();
    expect(acpModeUiLabel(undefined)).toBeUndefined();
    expect(acpModeUiLabel('custom')).toBe('custom');
  });

  it('matches Rust ACP composer hint text priority', () => {
    expect(acpComposerHintText({ welcomeCenter: true, sessionReady: false, activeMode: 'plan' })).toBe('Waiting for session...');
    expect(acpComposerHintText({ welcomeCenter: true, sessionReady: true, activeMode: 'build' })).toBe('Plan, Build, / for skills, @ for context');
    expect(acpComposerHintText({ welcomeCenter: false, sessionReady: true, activeMode: 'plan' })).toBe('Plan and design before coding...');
    expect(acpComposerHintText({ welcomeCenter: false, sessionReady: true, activeMode: 'build' })).toBe('Type a message...');
  });

  it('shows Kimi loop-protection badge only for risky OpenCode models', () => {
    expect(acpKimiProtectionBadge('fireworks-ai/accounts/fireworks/routers/kimi-k2p6-turbo', true)).toEqual({
      label: 'Kimi protected',
      color: 'rgb(100, 195, 140)',
    });
    expect(acpKimiProtectionBadge('provider/k2p6-fast', false)).toEqual({
      label: 'Kimi unprotected',
      color: 'rgb(220, 170, 60)',
    });
    expect(acpKimiProtectionBadge('openai/gpt-5.5-fast', true)).toBeUndefined();
    expect(acpKimiProtectionBadge(undefined, true)).toBeUndefined();
  });

  it('uses Rust-style ACP queued prompt previews', () => {
    expect(queuedPromptPreview({ text: 'draft value', finalPromptText: '  Build\n\nthis   now  ' })).toBe('Build this now');
    expect(queuedPromptPreview({ text: '  Build this  ' })).toBe('Build this');
    expect(queuedPromptPreview({ text: '', finalPromptText: 'Attached file paths:\na.png' })).toBe('Attached file paths: a.png');
    expect(queuedPromptPreview({ attachments: ['a.png'] })).toBe('(Attachment)');
    expect(queuedPromptPreview({})).toBe('(Empty prompt)');
  });

  it('truncates ACP queued prompt previews by Rust character count', () => {
    const maxPreview = 'x'.repeat(ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS);
    expect(queuedPromptPreview({ text: maxPreview })).toBe(maxPreview);
    expect(queuedPromptPreview({ text: `${maxPreview}y` })).toBe(`${maxPreview}...`);

    const unicodePreview = `${'ş'.repeat(ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS)}z`;
    expect(queuedPromptPreview({ text: unicodePreview })).toBe(`${'ş'.repeat(ACP_QUEUED_PROMPT_PREVIEW_MAX_CHARS)}...`);
  });

  it('moves queued ACP prompts to the front by index without mutating the queue', () => {
    const queue = ['first', 'second', 'third'];
    expect(moveQueuedPromptToFront(queue, 2)).toEqual(['third', 'first', 'second']);
    expect(queue).toEqual(['first', 'second', 'third']);
    expect(moveQueuedPromptToFront(queue, -1)).toEqual(queue);
    expect(moveQueuedPromptToFront(queue, 3)).toEqual(queue);
  });

  it('removes queued ACP prompts by index without mutating the queue', () => {
    const queue = ['first', 'second', 'third'];
    expect(removeQueuedPromptAt(queue, 1)).toEqual(['first', 'third']);
    expect(queue).toEqual(['first', 'second', 'third']);
    expect(removeQueuedPromptAt(queue, 99)).toEqual(queue);
  });

  it('blocks queued ACP prompt edit while the composer draft is occupied', () => {
    const message = 'Input is not empty; send or clear it before editing queued message';
    expect(acpQueuedPromptDraftEditBlockedMessage({ input: 'draft', attachments: [], editingQueuedPrompt: false })).toBe(message);
    expect(acpQueuedPromptDraftEditBlockedMessage({ input: '', attachments: ['a.png'], editingQueuedPrompt: false })).toBe(message);
    expect(acpQueuedPromptDraftEditBlockedMessage({ input: '', attachments: [], editingQueuedPrompt: true })).toBe(message);
    expect(acpQueuedPromptDraftEditBlockedMessage({ input: '  ', attachments: [], editingQueuedPrompt: false })).toBeUndefined();
  });

  it('matches Rust queued prompt panel header and visible-row rules', () => {
    const prompts = [
      { modeId: 'build' },
      { modeId: 'plan' },
      { modeId: 'plan' },
    ];

    expect(ACP_QUEUED_PROMPT_MAX_VISIBLE_ROWS).toBe(2);
    expect(acpQueuedPromptHeaderLabel(prompts.length)).toBe('Queued 3');
    expect(acpQueuedPromptHeaderLabel(prompts.length, 1)).toBe('Editing queued #2');
    expect(acpQueuedPromptPlanCount(prompts)).toBe(2);
    expect(acpQueuedPromptVisibleRowCount(prompts.length, true)).toBe(2);
    expect(acpQueuedPromptVisibleRowCount(prompts.length, false)).toBe(0);
    expect(acpQueuedPromptIndexLabel(0)).toBe('1.');
    expect(acpQueuedPromptIndexLabel(2)).toBe('3.');
    expect(acpQueuedPromptIndexLabel(-1)).toBe('');
    expect(acpQueuedPromptAttachmentLabel(0)).toBeUndefined();
    expect(acpQueuedPromptAttachmentLabel(1)).toBe('1 file');
    expect(acpQueuedPromptAttachmentLabel(2)).toBe('2 file');
  });

  it('normalizes slash command hints from id or name', () => {
    expect(slashCommandHint({ id: 'init', name: 'Initialize' })).toBe('/init');
    expect(slashCommandHint({ name: 'plan' })).toBe('/plan');
    expect(slashCommandHint({ id: '/build' })).toBe('/build');
  });

  it('drops malformed slash commands instead of throwing', () => {
    expect(slashCommandHint({ id: undefined, name: undefined })).toBeUndefined();
    expect(slashCommandHint({ name: 'two words' })).toBeUndefined();
    expect(slashCommandHints(undefined, '')).toEqual([]);
    expect(slashCommandHints([null, {}, { id: 7 }, { name: 'run' }], '')).toEqual(['/run']);
  });

  it('filters slash command hints by id or name without requiring id', () => {
    const commands = [
      { name: 'init' },
      { id: 'apply', name: 'Build Apply' },
      { id: 'review', name: 'Review' },
    ];
    expect(slashCommandHints(commands, '')).toEqual(['/init', '/apply', '/review']);
    expect(slashCommandHints(commands, 'bu')).toEqual(['/apply']);
    expect(slashCommandHints(commands, 're')).toEqual(['/review']);
  });

  it('builds slash command popup items with descriptions', () => {
    const commands = [
      { id: 'init', name: 'Initialize', description: 'Create project memory' },
      { id: 'apply', name: 'Build Apply', description: 'Apply a plan' },
    ];

    expect(slashCommandItems(commands, '')).toEqual([
      { hint: '/init', description: 'Create project memory' },
      { hint: '/apply', description: 'Apply a plan' },
    ]);
  });

  it('deduplicates and limits slash command popup items', () => {
    const commands = [
      { id: 'init', name: 'Initialize' },
      { id: '/init', name: 'Duplicate init' },
      { id: 'review', name: 'Review' },
      { id: 'apply', name: 'Apply' },
    ];

    expect(slashCommandItems(commands, '', 2)).toEqual([
      { hint: '/init', description: '' },
      { hint: '/review', description: '' },
    ]);
  });

  it('shows slash command items only for slash-prefixed composer input', () => {
    const commands = [{ id: 'init', name: 'Initialize', description: 'Create project memory' }];
    expect(slashCommandItemsForInput(commands, '')).toEqual([]);
    expect(slashCommandItemsForInput(commands, 'init')).toEqual([]);
    expect(slashCommandItemsForInput(commands, '/')).toEqual([
      { hint: '/init', description: 'Create project memory' },
    ]);
  });

  it('keeps slash command popup hidden until composer controls are enabled', () => {
    const commands = [{ id: 'init', name: 'Initialize', description: 'Create project memory' }];

    expect(slashCommandItemsForComposer(commands, '/', false)).toEqual([]);
    expect(slashCommandItemsForComposer(commands, '/', true)).toEqual([
      { hint: '/init', description: 'Create project memory' },
    ]);
  });
});
