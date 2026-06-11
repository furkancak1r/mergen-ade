import fs from 'fs';
import os from 'os';
import path from 'path';
import { spawn } from 'child_process';
import {
  buildClaudeCodexFixPrompt,
  buildClaudeCodexImplementationPrompt,
  buildCodexReviewPrompt,
  buildCodexPlanPrompt,
  type ClaudeCodexReviewRequest,
  type ClaudeCodexReviewResult,
  type ClaudeCodexTestCommandResult,
  type ClaudeCodexPlanResult,
  MAX_REVIEW_FIX_ROUNDS,
  reviewHasActionableFindings,
  renderClaudeCodexPlanFile,
  testSummary,
} from '../shared/claudeCodexHook';

let sessionCounter = 0;

export interface RunClaudeCodexPlanRequest {
  terminalId: number;
  projectPath: string;
  originalPrompt: string;
}

export async function runClaudeCodexPlan(request: RunClaudeCodexPlanRequest): Promise<ClaudeCodexPlanResult> {
  const sessionId = nextSessionId(request.terminalId);
  const planPath = path.join(request.projectPath, '.claude', 'plans', `${sessionId}.md`);
  writePlanFile(planPath, {
    sessionId,
    status: 'planned',
    originalPrompt: request.originalPrompt,
  });

  const prompt = buildCodexPlanPrompt(request.originalPrompt, planPath);
  const result = await runCodexExec(request.projectPath, prompt);
  const plan = result.ok ? result.output : undefined;
  const planError = result.ok ? undefined : result.error;

  writePlanFile(planPath, {
    sessionId,
    status: 'implementing',
    originalPrompt: request.originalPrompt,
    plan,
    planError,
  });

  return {
    sessionId,
    planPath,
    plan,
    planError,
    implementationPrompt: buildClaudeCodexImplementationPrompt({
      sessionId,
      planPath,
      originalPrompt: request.originalPrompt,
      plan,
      planError,
    }),
  };
}

export async function runClaudeCodexReview(request: ClaudeCodexReviewRequest): Promise<ClaudeCodexReviewResult> {
  const reviewRound = request.reviewRound + 1;
  writePlanFile(request.planPath, {
    sessionId: request.sessionId,
    status: 'testing',
    originalPrompt: request.originalPrompt,
    plan: request.plan,
    planError: request.planError,
    reviewRound,
  });

  const commands = discoverTestCommands(request.projectPath);
  const testResults = await runTestCommands(request.projectPath, commands);
  const testNote = commands.length === 0 ? 'No test/lint/typecheck commands were detected.' : undefined;
  const summary = testSummary(testResults, testNote);
  const uiChangedFiles = detectUiChangedFiles(request.projectPath);

  writePlanFile(request.planPath, {
    sessionId: request.sessionId,
    status: 'reviewing',
    originalPrompt: request.originalPrompt,
    plan: request.plan,
    planError: request.planError,
    testResults,
    testNote,
    reviewRound,
    uiChangedFiles,
  });

  const reviewPrompt = buildCodexReviewPrompt(request.planPath, summary, reviewRound);
  const review = await runCodexExec(request.projectPath, reviewPrompt);
  const reviewOutput = review.ok ? review.output : undefined;
  const reviewError = review.ok ? undefined : review.error;
  const hasActionableFindings = review.ok ? reviewHasActionableFindings(review.output) : false;

  let status: Parameters<typeof renderClaudeCodexPlanFile>[0]['status'] = 'done';
  let finalNote: string | undefined = 'Codex review reported no actionable P0-P3 findings.';
  let blockedReason: string | undefined;
  let fixPrompt: string | undefined;
  if (!review.ok) {
    status = 'blocked';
    blockedReason = 'Codex review failed; implementation was left intact.';
    finalNote = blockedReason;
  } else if (hasActionableFindings) {
    if (reviewRound >= MAX_REVIEW_FIX_ROUNDS) {
      status = 'blocked';
      blockedReason = `Codex review still reported findings after ${MAX_REVIEW_FIX_ROUNDS} fix rounds.`;
      finalNote = blockedReason;
    } else {
      status = 'fixing';
      finalNote = undefined;
      fixPrompt = buildClaudeCodexFixPrompt({
        reviewRound,
        planPath: request.planPath,
        reviewOutput,
        testSummary: summary,
      });
    }
  }

  writePlanFile(request.planPath, {
    sessionId: request.sessionId,
    status,
    originalPrompt: request.originalPrompt,
    plan: request.plan,
    planError: request.planError,
    testResults,
    testNote,
    reviewRound,
    reviewOutput,
    reviewError,
    uiChangedFiles,
    uiVerification: uiChangedFiles.length === 0
      ? 'UI verification skipped: no UI-facing changed files detected.'
      : 'UI verification pending: UI-facing changed files were detected.',
    finalNote,
  });

  return {
    sessionId: request.sessionId,
    planPath: request.planPath,
    reviewRound,
    testResults,
    testNote,
    testSummary: summary,
    reviewOutput,
    reviewError,
    uiChangedFiles,
    hasActionableFindings,
    fixPrompt,
    done: status === 'done',
    blockedReason,
  };
}

export function updateClaudeCodexUiVerification(opts: { planPath: string; note: string }): boolean {
  if (!opts.planPath || !fs.existsSync(opts.planPath)) return false;
  const text = fs.readFileSync(opts.planPath, 'utf-8');
  const replacement = opts.note.trim() || 'UI verification unavailable.';
  let next = text.replace(
    'UI verification pending: UI-facing changed files were detected.',
    replacement,
  );
  if (next === text) {
    const finalNoteIndex = text.indexOf('\n## Final Note\n');
    if (finalNoteIndex >= 0) {
      next = `${text.slice(0, finalNoteIndex).trimEnd()}\n\n${replacement}\n${text.slice(finalNoteIndex)}`;
    } else {
      next = `${text.trimEnd()}\n\n${replacement}\n`;
    }
  }
  fs.writeFileSync(opts.planPath, next, 'utf-8');
  return true;
}

function nextSessionId(terminalId: number): string {
  sessionCounter += 1;
  return `mergen-${terminalId}-${Date.now().toString(16)}-${sessionCounter.toString(16)}`;
}

function writePlanFile(planPath: string, content: Parameters<typeof renderClaudeCodexPlanFile>[0]): void {
  fs.mkdirSync(path.dirname(planPath), { recursive: true });
  fs.writeFileSync(planPath, renderClaudeCodexPlanFile(content), 'utf-8');
}

interface TestCommand {
  label: string;
  program: string;
  args: string[];
}

function discoverTestCommands(projectPath: string): TestCommand[] {
  const commands: TestCommand[] = [];
  if (fs.existsSync(path.join(projectPath, 'Cargo.toml'))) {
    commands.push({ label: 'cargo test', program: cargoProgram(), args: ['test'] });
  }
  const packageJsonPath = path.join(projectPath, 'package.json');
  if (fs.existsSync(packageJsonPath)) {
    try {
      const parsed = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8')) as { scripts?: Record<string, unknown> };
      const scripts = parsed.scripts || {};
      for (const script of ['lint', 'typecheck', 'test']) {
        if (Object.prototype.hasOwnProperty.call(scripts, script)) {
          commands.push({
            label: script === 'test' ? 'npm test' : `npm run ${script}`,
            program: npmProgram(),
            args: script === 'test' ? ['test'] : ['run', script],
          });
        }
      }
    } catch {
      // Invalid package.json: no JS test commands discovered.
    }
  }
  return commands;
}

async function runTestCommands(projectPath: string, commands: TestCommand[]): Promise<ClaudeCodexTestCommandResult[]> {
  const results: ClaudeCodexTestCommandResult[] = [];
  for (const command of commands) {
    results.push(await runTestCommand(projectPath, command));
  }
  return results;
}

async function runTestCommand(projectPath: string, command: TestCommand): Promise<ClaudeCodexTestCommandResult> {
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(command.program, command.args, {
        cwd: projectPath,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
      });
    } catch (error) {
      resolve({
        label: command.label,
        success: false,
        stdout: '',
        stderr: '',
        error: error instanceof Error ? error.message : String(error),
      });
      return;
    }
    let stdout = '';
    let stderr = '';
    child.stdout?.setEncoding('utf-8');
    child.stderr?.setEncoding('utf-8');
    child.stdout?.on('data', (chunk) => { stdout += chunk; });
    child.stderr?.on('data', (chunk) => { stderr += chunk; });
    child.on('error', (error) => {
      resolve({ label: command.label, success: false, stdout, stderr, error: error.message });
    });
    child.on('close', (code) => {
      resolve({ label: command.label, success: code === 0, stdout, stderr });
    });
  });
}

function detectUiChangedFiles(projectPath: string): string[] {
  const files = [
    ...gitOutputLines(projectPath, ['diff', '--name-only', 'HEAD', '--']),
    ...gitOutputLines(projectPath, ['ls-files', '--others', '--exclude-standard']),
  ];
  return Array.from(new Set(files.sort())).filter(isUiFacingPath);
}

function gitOutputLines(projectPath: string, args: string[]): string[] {
  const result = spawnSyncText('git', ['-C', projectPath, ...args]);
  if (!result.ok) return [];
  return result.stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
}

function spawnSyncText(program: string, args: string[]): { ok: boolean; stdout: string } {
  const child = require('child_process').spawnSync(program, args, {
    encoding: 'utf-8',
    windowsHide: true,
  }) as { status: number | null; stdout?: string };
  return { ok: child.status === 0, stdout: child.stdout || '' };
}

function isUiFacingPath(filePath: string): boolean {
  const normalized = filePath.replace(/\\/g, '/').toLowerCase();
  if (
    normalized.endsWith('src/app.rs')
    || normalized.includes('/ui/')
    || normalized.includes('/components/')
    || normalized.includes('/pages/')
    || normalized.includes('/views/')
  ) {
    return true;
  }
  return ['css', 'scss', 'sass', 'html', 'htm', 'jsx', 'tsx', 'vue', 'svelte', 'png', 'jpg', 'jpeg', 'webp', 'gif', 'svg']
    .includes(path.extname(normalized).replace('.', ''));
}

async function runCodexExec(projectPath: string, prompt: string): Promise<{ ok: true; output: string } | { ok: false; error: string }> {
  const program = codexProgram();
  const args = [
    '--ask-for-approval',
    'never',
    'exec',
    '--skip-git-repo-check',
    '--sandbox',
    'read-only',
    '--cd',
    projectPath,
    '-',
  ];

  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(program, args, {
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true,
      });
    } catch (error) {
      resolve({ ok: false, error: `Failed to start Codex CLI at ${program}: ${error instanceof Error ? error.message : String(error)}` });
      return;
    }

    let stdout = '';
    let stderr = '';
    child.stdout?.setEncoding('utf-8');
    child.stderr?.setEncoding('utf-8');
    child.stdout?.on('data', (chunk) => { stdout += chunk; });
    child.stderr?.on('data', (chunk) => { stderr += chunk; });
    child.on('error', (error) => {
      resolve({ ok: false, error: `Failed to start Codex CLI at ${program}: ${error.message}` });
    });
    child.on('close', (code, signal) => {
      const cleanStdout = stdout.trim();
      const cleanStderr = stderr.trim();
      if (code === 0) {
        resolve({ ok: true, output: cleanStdout || cleanStderr });
        return;
      }
      resolve({
        ok: false,
        error: `Codex CLI exited with ${code ?? signal ?? 'unknown'}.\nstdout:\n${cleanStdout}\nstderr:\n${cleanStderr}`,
      });
    });
    child.stdin?.end(prompt);
  });
}

function codexProgram(): string {
  if (process.platform === 'win32') {
    const appData = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
    const appDataCodex = path.join(appData, 'npm', 'codex.cmd');
    if (fs.existsSync(appDataCodex)) return appDataCodex;
    return 'codex.cmd';
  }
  return 'codex';
}

function cargoProgram(): string {
  if (process.platform === 'win32') {
    const userProfile = process.env.USERPROFILE || os.homedir();
    const cargo = path.join(userProfile, '.cargo', 'bin', 'cargo.exe');
    if (fs.existsSync(cargo)) return cargo;
    return 'cargo.exe';
  }
  return 'cargo';
}

function npmProgram(): string {
  return process.platform === 'win32' ? 'npm.cmd' : 'npm';
}
