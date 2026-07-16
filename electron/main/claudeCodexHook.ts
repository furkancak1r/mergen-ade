import fs from 'fs';
import os from 'os';
import path from 'path';
import { spawn, spawnSync } from 'child_process';
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
import { getCodexBinPath } from './codex';

let sessionCounter = 0;

export interface RunClaudeCodexPlanRequest {
  terminalId: number;
  projectPath: string;
  originalPrompt: string;
}

export async function runClaudeCodexPlan(request: RunClaudeCodexPlanRequest): Promise<ClaudeCodexPlanResult> {
  const sessionId = nextSessionId(request.terminalId);
  const projectPath = safeProjectDir(request.projectPath);
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- safePlanPath enforces the generated file stays under this project's .claude/plans directory.
  const planPath = safePlanPath(projectPath, path.join(projectPath, '.claude', 'plans', `${sessionId}.md`));
  writePlanFile(planPath, {
    sessionId,
    status: 'planned',
    originalPrompt: request.originalPrompt,
  });

  const prompt = buildCodexPlanPrompt(request.originalPrompt, planPath);
  const result = await runCodexExec(projectPath, prompt);
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
  const projectPath = safeProjectDir(request.projectPath);
  const planPath = safePlanPath(projectPath, request.planPath);
  const reviewRound = request.reviewRound + 1;
  writePlanFile(planPath, {
    sessionId: request.sessionId,
    status: 'testing',
    originalPrompt: request.originalPrompt,
    plan: request.plan,
    planError: request.planError,
    reviewRound,
  });

  const commands = discoverTestCommands(projectPath);
  const testResults = await runTestCommands(projectPath, commands);
  const testNote = commands.length === 0 ? 'No test/lint/typecheck commands were detected.' : undefined;
  const summary = testSummary(testResults, testNote);
  const uiChangedFiles = detectUiChangedFiles(projectPath);

  writePlanFile(planPath, {
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

  const reviewPrompt = buildCodexReviewPrompt(planPath, summary, reviewRound);
  const review = await runCodexExec(projectPath, reviewPrompt);
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
        planPath,
        reviewOutput,
        testSummary: summary,
      });
    }
  }

  writePlanFile(planPath, {
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
    planPath,
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
  const planPath = safeExistingPlanPath(opts.planPath);
  if (!planPath) return false;
  const text = fs.readFileSync(planPath, 'utf-8');
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
  fs.writeFileSync(planPath, next, 'utf-8');
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

function safeProjectDir(projectPath: string): string {
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- The user-selected project directory is intentionally the root; statSync rejects missing and non-directory paths.
  const resolved = path.resolve(projectPath);
  const stat = fs.statSync(resolved);
  if (!stat.isDirectory()) throw new Error('Project path is not a directory.');
  return resolved;
}

function safePlanPath(projectPath: string, planPath: string): string {
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- projectPath was resolved and verified by safeProjectDir.
  const plansDir = path.join(projectPath, '.claude', 'plans');
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- The path.relative containment check below rejects traversal outside plansDir.
  const resolved = path.resolve(planPath);
  const relative = path.relative(plansDir, resolved);
  if (relative.startsWith('..') || path.isAbsolute(relative) || path.basename(resolved) !== path.basename(planPath) || path.extname(resolved) !== '.md') {
    throw new Error('Plan path must be a Markdown file under .claude/plans.');
  }
  return resolved;
}

function safeExistingPlanPath(planPath: string): string | undefined {
  if (!planPath) return undefined;
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- The following segment and extension checks restrict updates to existing .claude/plans/*.md files.
  const resolved = path.resolve(planPath);
  const parts = resolved.split(path.sep);
  if (path.extname(resolved) !== '.md' || parts.at(-3) !== '.claude' || parts.at(-2) !== 'plans') return undefined;
  return fs.existsSync(resolved) ? resolved : undefined;
}

interface TestCommand {
  label: string;
  program: string;
  args: string[];
}

function discoverTestCommands(projectPath: string): TestCommand[] {
  const commands: TestCommand[] = [];
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- projectPath was resolved and verified by safeProjectDir.
  if (fs.existsSync(path.join(projectPath, 'Cargo.toml'))) {
    commands.push({ label: 'cargo test', program: cargoProgram(), args: ['test'] });
  }
  // nosemgrep: javascript.lang.security.audit.path-traversal.path-join-resolve-traversal.path-join-resolve-traversal -- projectPath was resolved and verified by safeProjectDir.
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
      const isWindowsCommandShim = process.platform === 'win32' && command.program.toLowerCase().endsWith('.cmd');
      const program = isWindowsCommandShim ? (process.env.ComSpec || 'cmd.exe') : command.program;
      const args = isWindowsCommandShim ? ['/d', '/s', '/c', command.program, ...command.args] : command.args;
      // nosemgrep: javascript.lang.security.detect-child-process.detect-child-process -- program and args come only from the fixed commands built by discoverTestCommands; shell execution remains disabled.
      child = spawn(program, args, {
        cwd: projectPath,
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true,
        shell: false,
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
  const child = spawnSync('git', ['-C', projectPath, ...args], {
    encoding: 'utf-8',
    windowsHide: true,
  }) as { status: number | null; stdout?: string };
  if (child.status !== 0) return [];
  return (child.stdout || '').split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
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
  const command = codexCommand();
  const program = command.program;
  const args = [
    ...command.args,
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
        shell: false,
      });
      child.stdin?.end(prompt);
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
  });
}

function codexCommand(): { program: string; args: string[] } {
  const program = getCodexBinPath();
  if (process.platform !== 'win32' || !program.toLowerCase().endsWith('.cmd')) {
    return { program, args: [] };
  }
  const binDir = path.dirname(program);
  const node = path.join(binDir, 'node.exe');
  const cli = path.join(binDir, 'node_modules', '@openai', 'codex', 'bin', 'codex.js');
  return { program: fs.existsSync(node) ? node : 'node.exe', args: [cli] };
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
