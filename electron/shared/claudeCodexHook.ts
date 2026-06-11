export const MAX_REVIEW_FIX_ROUNDS = 3;

export type ClaudeCodexHookPhase =
  | 'planning'
  | 'awaiting_implementation'
  | 'awaiting_fix'
  | 'testing'
  | 'reviewing'
  | 'blocked'
  | 'done';

export type ClaudeCodexPlanStatus =
  | 'planned'
  | 'implementing'
  | 'testing'
  | 'reviewing'
  | 'fixing'
  | 'done'
  | 'blocked';

export interface ClaudeCodexPlanFileContent {
  sessionId: string;
  status?: ClaudeCodexPlanStatus;
  originalPrompt: string;
  plan?: string;
  planError?: string;
  testResults?: ClaudeCodexTestCommandResult[];
  testNote?: string;
  reviewRound?: number;
  reviewOutput?: string;
  reviewError?: string;
  uiChangedFiles?: string[];
  uiVerification?: string;
  finalNote?: string;
}

export interface ClaudeCodexTestCommandResult {
  label: string;
  success: boolean;
  stdout: string;
  stderr: string;
  error?: string;
}

export interface ClaudeCodexPlanResult {
  sessionId: string;
  planPath: string;
  plan?: string;
  planError?: string;
  implementationPrompt: string;
}

export interface ClaudeCodexReviewRequest {
  terminalId: number;
  projectPath: string;
  sessionId: string;
  planPath: string;
  originalPrompt: string;
  plan?: string;
  planError?: string;
  reviewRound: number;
}

export interface ClaudeCodexReviewResult {
  sessionId: string;
  planPath: string;
  reviewRound: number;
  testResults: ClaudeCodexTestCommandResult[];
  testNote?: string;
  testSummary: string;
  reviewOutput?: string;
  reviewError?: string;
  uiChangedFiles: string[];
  hasActionableFindings: boolean;
  fixPrompt?: string;
  done: boolean;
  blockedReason?: string;
}

export interface ClaudeCodexHookProgress {
  phase: ClaudeCodexHookPhase;
  sessionId: string;
  planPath?: string;
  error?: string;
  originalPrompt?: string;
  plan?: string;
  planError?: string;
  reviewRound?: number;
  reviewInFlight?: boolean;
}

export function renderClaudeCodexPlanFile(content: ClaudeCodexPlanFileContent): string {
  const reviewRound = content.reviewRound ?? 0;
  const uiChangedFiles = content.uiChangedFiles ?? [];
  let out = '';
  out += '---\n';
  out += `session_id: ${content.sessionId}\n`;
  if (content.status) out += `status: ${content.status}\n`;
  out += `review_round: ${reviewRound}\n`;
  out += '---\n\n';
  out += '# Claude Code Codex Hook Plan\n\n';
  out += '## Original Prompt\n\n';
  out += `${content.originalPrompt.trim()}\n\n`;

  out += '## Codex Plan\n\n';
  if (content.plan?.trim()) {
    out += content.plan.trim();
  } else if (content.planError?.trim()) {
    out += 'Codex planning failed; Claude Code should continue with the original prompt.\n\n';
    out += content.planError.trim();
  } else {
    out += 'Codex planning is pending.';
  }
  out += '\n\n';

  out += '## Tests\n\n';
  if (content.testNote?.trim()) {
    out += `${content.testNote.trim()}\n\n`;
  }
  const testResults = content.testResults ?? [];
  if (testResults.length === 0) {
    out += 'No test results recorded yet.\n\n';
  } else {
    for (const result of testResults) {
      out += `### ${result.label}\n\nstatus: ${result.success ? 'passed' : 'failed'}\n\n`;
      if (result.error?.trim()) {
        out += 'error:\n\n```text\n';
        out += truncateForPlan(result.error);
        out += '\n```\n\n';
      }
      if (result.stdout.trim()) {
        out += 'stdout:\n\n```text\n';
        out += truncateForPlan(result.stdout);
        out += '\n```\n\n';
      }
      if (result.stderr.trim()) {
        out += 'stderr:\n\n```text\n';
        out += truncateForPlan(result.stderr);
        out += '\n```\n\n';
      }
    }
  }

  out += '## Codex Review\n\n';
  if (content.reviewError?.trim()) {
    out += 'Codex review failed.\n\n```text\n';
    out += truncateForPlan(content.reviewError);
    out += '\n```\n\n';
  } else if (content.reviewOutput?.trim()) {
    out += '```text\n';
    out += truncateForPlan(content.reviewOutput);
    out += '\n```\n\n';
  } else {
    out += 'No review result recorded yet.\n\n';
  }

  out += '## UI Verification\n\n';
  if (uiChangedFiles.length === 0) {
    out += 'No UI-facing changed files detected.\n\n';
  } else {
    out += 'Detected UI-facing changed files:\n\n';
    for (const file of uiChangedFiles) {
      out += `- ${file}\n`;
    }
    out += '\n';
  }
  if (content.uiVerification?.trim()) {
    out += `${content.uiVerification.trim()}\n\n`;
  }

  if (content.finalNote?.trim()) {
    out += '## Final Note\n\n';
    out += `${content.finalNote.trim()}\n`;
  }

  return out;
}

export function buildCodexPlanPrompt(originalPrompt: string, planPath: string): string {
  return `You are the read-only planning hook for a Claude Code implementation turn.
Do not edit files. Do not run commands that write to disk. Inspect only what is needed.
Produce a concise implementation plan with risks and likely validation commands.
Mergen ADE will save the plan at: ${planPath}

User prompt:
${originalPrompt.trim()}`;
}

export function buildClaudeCodexImplementationPrompt(input: {
  sessionId: string;
  planPath: string;
  originalPrompt: string;
  plan?: string;
  planError?: string;
}): string {
  let prompt = `Claude Code Codex hook implementation session: ${input.sessionId}
Instruction file: ${input.planPath}

Original user prompt:
${input.originalPrompt}

Automation requirements:
- Stay in implementation/default mode; do not switch to Claude Code plan permission mode.
- Do not ask the user to approve a proposal before editing.
- Do not run slash commands for planning.
- Carry out the requested implementation directly.

`;
  if (input.plan?.trim()) {
    prompt += 'Codex read-only implementation instructions:\n';
    prompt += input.plan.trim();
    prompt += "\n\nApply these instructions now. Keep the implementation scoped to the user's request.";
  } else if (input.planError?.trim()) {
    prompt += 'Codex pre-implementation step failed, so continue with the original prompt using your normal implementation flow.\n\nPre-step error:\n';
    prompt += input.planError.trim();
  } else {
    prompt += 'Codex did not produce instructions. Continue with the original prompt.';
  }
  return prompt;
}

export function buildCodexReviewPrompt(planPath: string, testSummary: string, reviewRound: number): string {
  return `You are the read-only review hook after Claude Code implementation.
Review the current uncommitted workspace changes against the plan at ${planPath}.
You may inspect files and diffs only. Do not edit files.
Report only real, actionable findings with severity P0, P1, P2, or P3.
If there are no actionable P0-P3 findings, respond exactly with NO_FINDINGS.
This is review pass ${reviewRound}. At most ${MAX_REVIEW_FIX_ROUNDS} review-fix remediation rounds may be requested.

Validation summary:
${testSummary.trim()}`;
}

export function buildClaudeCodexFixPrompt(input: {
  reviewRound: number;
  planPath: string;
  reviewOutput?: string;
  testSummary: string;
}): string {
  return `Claude Code Codex hook fix round ${input.reviewRound}/${MAX_REVIEW_FIX_ROUNDS}.
Instruction file: ${input.planPath}

Automation requirements:
- Stay in implementation/default mode; do not switch to Claude Code plan permission mode.
- Do not ask the user to approve a proposal before editing.
- Do not run slash commands for planning.

Codex review reported actionable findings. Fix only these findings, keep the original scope, and do not start unrelated refactors.

Review output:
${input.reviewOutput || 'No review output captured.'}

Test summary:
${input.testSummary}`;
}

export function testSummary(results: ClaudeCodexTestCommandResult[], noTestsNote?: string): string {
  if (results.length === 0) {
    return noTestsNote || 'No test/lint/typecheck commands were detected.';
  }
  let summary = '';
  for (const result of results) {
    summary += `- ${result.label}: ${result.success ? 'passed' : 'failed'}\n`;
    if (result.error) {
      summary += `  error: ${result.error}\n`;
    }
  }
  return summary;
}

export function reviewHasActionableFindings(output: string): boolean {
  const normalized = output.trim().toLowerCase();
  if (
    !normalized
    || normalized === 'no_findings'
    || normalized === 'no findings'
    || normalized.includes('no actionable')
    || normalized.includes('no p0')
  ) {
    return false;
  }
  return ['p0', 'p1', 'p2', 'p3'].some((severity) => containsSeverityToken(normalized, severity));
}

export function claudeCodexHookPhaseLabel(phase: ClaudeCodexHookPhase): string {
  switch (phase) {
    case 'planning': return 'Codex planning';
    case 'awaiting_implementation': return 'Claude implementing Codex plan';
    case 'awaiting_fix': return 'Claude fixing Codex review findings';
    case 'testing': return 'Running tests';
    case 'reviewing': return 'Codex reviewing';
    case 'blocked': return 'Codex hook blocked';
    case 'done': return 'Codex hook done';
  }
}

export function claudeCodexHookProgressText(progress: ClaudeCodexHookProgress): string {
  const label = claudeCodexHookPhaseLabel(progress.phase);
  if ((progress.phase === 'reviewing' || progress.phase === 'awaiting_fix') && progress.reviewRound && progress.reviewRound > 0) {
    return `Claude Code Codex hook: ${label} (${Math.min(progress.reviewRound, MAX_REVIEW_FIX_ROUNDS)}/${MAX_REVIEW_FIX_ROUNDS})`;
  }
  return `Claude Code Codex hook: ${label}`;
}

function truncateForPlan(text: string): string {
  const chars = Array.from(text);
  if (chars.length <= 12_000) return text;
  return `${chars.slice(0, 12_000).join('')}\n[truncated]`;
}

function containsSeverityToken(text: string, token: string): boolean {
  let start = 0;
  while (start < text.length) {
    const index = text.indexOf(token, start);
    if (index === -1) return false;
    const before = index > 0 ? text[index - 1] : '';
    const after = index + token.length < text.length ? text[index + token.length] : '';
    if (!/[a-z0-9]/.test(before) && !/[a-z0-9]/.test(after)) {
      return true;
    }
    start = index + token.length;
  }
  return false;
}
