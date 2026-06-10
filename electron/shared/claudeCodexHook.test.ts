import { describe, expect, it } from 'vitest';
import {
  buildClaudeCodexFixPrompt,
  buildClaudeCodexImplementationPrompt,
  buildCodexReviewPrompt,
  buildCodexPlanPrompt,
  claudeCodexHookProgressText,
  renderClaudeCodexPlanFile,
  reviewHasActionableFindings,
  testSummary,
} from './claudeCodexHook';

describe('claude codex hook helpers', () => {
  it('renders the Rust-style plan file sections', () => {
    const text = renderClaudeCodexPlanFile({
      sessionId: 'mergen-1-test-1',
      status: 'planned',
      originalPrompt: '  build chess  ',
      plan: 'Use existing board logic.',
      testResults: [{
        label: 'npm test',
        success: false,
        stdout: 'ok',
        stderr: 'failed one',
      }],
      reviewRound: 0,
    });

    expect(text).toContain('session_id: mergen-1-test-1');
    expect(text).toContain('status: planned');
    expect(text).toContain('# Claude Code Codex Hook Plan');
    expect(text).toContain('## Original Prompt\n\nbuild chess');
    expect(text).toContain('## Codex Plan\n\nUse existing board logic.');
    expect(text).toContain('### npm test');
    expect(text).toContain('status: failed');
    expect(text).toContain('stdout:');
    expect(text).toContain('No review result recorded yet.');
  });

  it('builds read-only Codex planning prompts with the plan file path', () => {
    const prompt = buildCodexPlanPrompt('make chat faster', 'C:/repo/.claude/plans/mergen.md');

    expect(prompt).toContain('read-only planning hook');
    expect(prompt).toContain('Do not edit files');
    expect(prompt).toContain('C:/repo/.claude/plans/mergen.md');
    expect(prompt).toContain('make chat faster');
  });

  it('builds Claude implementation prompts from Codex plan or planning errors', () => {
    const planned = buildClaudeCodexImplementationPrompt({
      sessionId: 's1',
      planPath: 'C:/repo/.claude/plans/s1.md',
      originalPrompt: 'fix bug',
      plan: 'Patch the parser.',
    });
    expect(planned).toContain('Claude Code Codex hook implementation session: s1');
    expect(planned).toContain('Codex read-only implementation instructions');
    expect(planned).toContain('Patch the parser.');

    const failed = buildClaudeCodexImplementationPrompt({
      sessionId: 's2',
      planPath: 'C:/repo/.claude/plans/s2.md',
      originalPrompt: 'fix bug',
      planError: 'Codex unavailable',
    });
    expect(failed).toContain('continue with the original prompt');
    expect(failed).toContain('Codex unavailable');
  });

  it('builds review and fix prompts matching Rust hook semantics', () => {
    const review = buildCodexReviewPrompt('C:/repo/.claude/plans/s1.md', '- npm test: passed', 2);
    expect(review).toContain('read-only review hook');
    expect(review).toContain('NO_FINDINGS');
    expect(review).toContain('review pass 2');

    const fix = buildClaudeCodexFixPrompt({
      reviewRound: 2,
      planPath: 'C:/repo/.claude/plans/s1.md',
      reviewOutput: 'P1: fix parser',
      testSummary: '- npm test: failed',
    });
    expect(fix).toContain('fix round 2/3');
    expect(fix).toContain('P1: fix parser');
    expect(fix).toContain('- npm test: failed');
  });

  it('summarizes tests and detects actionable review findings', () => {
    expect(testSummary([{ label: 'npm test', success: true, stdout: '', stderr: '' }])).toBe('- npm test: passed\n');
    expect(testSummary([], undefined)).toBe('No test/lint/typecheck commands were detected.');
    expect(reviewHasActionableFindings('NO_FINDINGS')).toBe(false);
    expect(reviewHasActionableFindings('No actionable issues found')).toBe(false);
    expect(reviewHasActionableFindings('P2: missing validation')).toBe(true);
    expect(reviewHasActionableFindings('copy2 text')).toBe(false);
  });

  it('formats progress text like the Rust terminal band', () => {
    expect(claudeCodexHookProgressText({
      phase: 'planning',
      sessionId: 's1',
    })).toBe('Claude Code Codex hook: Codex planning');
    expect(claudeCodexHookProgressText({
      phase: 'awaiting_fix',
      sessionId: 's1',
      reviewRound: 2,
    })).toBe('Claude Code Codex hook: Claude fixing Codex review findings (2/3)');
  });
});
