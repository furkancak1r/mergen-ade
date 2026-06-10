import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';
import type { GitWorktreeInfo } from '../shared/types';

export function parseGitWorktreeList(output: string): GitWorktreeInfo[] {
  const worktrees: GitWorktreeInfo[] = [];
  const lines = output.split('\n');
  let current: Partial<GitWorktreeInfo> | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) {
      if (current && current.path) {
        worktrees.push({
          path: current.path,
          branch: current.branch ?? '',
          head: current.head,
          detached: current.detached ?? false,
          locked: current.locked ?? false,
          prunable: current.prunable ?? false,
        });
      }
      current = null;
      continue;
    }

    if (trimmed.startsWith('worktree ')) {
      current = { path: trimmed.slice(9).trim() };
    } else if (current) {
      if (trimmed.startsWith('HEAD ')) {
        current.head = trimmed.slice(5).trim();
      } else if (trimmed.startsWith('branch ')) {
        current.branch = trimmed.slice(7).trim().replace(/^refs\/heads\//, '');
      } else if (trimmed === 'detached') {
        current.detached = true;
      } else if (trimmed.startsWith('locked ')) {
        current.locked = true;
      } else if (trimmed === 'locked') {
        current.locked = true;
      } else if (trimmed === 'prunable') {
        current.prunable = true;
      }
    }
  }

  if (current && current.path) {
    worktrees.push({
      path: current.path,
      branch: current.branch ?? '',
      head: current.head,
      detached: current.detached ?? false,
      locked: current.locked ?? false,
      prunable: current.prunable ?? false,
    });
  }

  return worktrees;
}

export function discoverWorktrees(repoPath: string): Promise<GitWorktreeInfo[]> {
  return new Promise((resolve) => {
    const proc = spawn('git', ['worktree', 'list', '--porcelain'], { cwd: repoPath });
    let stdout = '';
    let stderr = '';
    proc.stdout.on('data', (data) => { stdout += data; });
    proc.stderr.on('data', (data) => { stderr += data; });
    proc.on('close', (code) => {
      if (code !== 0) {
        resolve([]);
        return;
      }
      resolve(parseGitWorktreeList(stdout));
    });
    proc.on('error', () => {
      resolve([]);
    });
  });
}

export function createWorktree(repoPath: string, branch: string, wtPath: string, baseBranch?: string): Promise<boolean> {
  return new Promise((resolve) => {
    const args = ['worktree', 'add', '-b', branch, wtPath];
    if (baseBranch) {
      args.push(baseBranch);
    }
    const proc = spawn('git', args, { cwd: repoPath });
    proc.on('close', (code) => {
      if (code === 0) {
        // Copy root .env* files to new worktree so runtime commands work immediately
        copyEnvFiles(repoPath, wtPath);
      }
      resolve(code === 0);
    });
    proc.on('error', () => {
      resolve(false);
    });
  });
}

function copyEnvFiles(fromDir: string, toDir: string): void {
  try {
    const entries = fs.readdirSync(fromDir);
    for (const entry of entries) {
      if (entry.startsWith('.env')) {
        const src = path.join(fromDir, entry);
        const dest = path.join(toDir, entry);
        const stat = fs.statSync(src);
        if (stat.isFile()) {
          fs.copyFileSync(src, dest);
        }
      }
    }
  } catch {
    // ignore copy failures
  }
}

export function cleanupOrphanWorktrees(
  registeredPaths: string[],
  repoPath: string,
): Promise<string[]> {
  return new Promise((resolve) => {
    discoverWorktrees(repoPath).then((discovered) => {
      const orphanPaths: string[] = [];
      for (const registered of registeredPaths) {
        const stillExists = discovered.some((d) => d.path === registered);
        const pathOnDisk = fs.existsSync(registered);
        if (!stillExists && !pathOnDisk) {
          orphanPaths.push(registered);
        }
      }
      resolve(orphanPaths);
    });
  });
}

export function removeWorktree(repoPath: string, path: string): Promise<boolean> {
  return new Promise((resolve) => {
    const proc = spawn('git', ['worktree', 'remove', path], { cwd: repoPath });
    proc.on('close', (code) => {
      resolve(code === 0);
    });
    proc.on('error', () => {
      resolve(false);
    });
  });
}

export function getGitStatus(repoPath: string): Promise<{ files: { path: string; status: string; staged: boolean }[]; branch: string }> {
  return new Promise((resolve) => {
    const proc = spawn('git', ['status', '--porcelain', '-b'], { cwd: repoPath });
    let stdout = '';
    let stderr = '';
    proc.stdout.on('data', (data) => { stdout += data; });
    proc.stderr.on('data', (data) => { stderr += data; });
    proc.on('close', (code) => {
      if (code !== 0) {
        resolve({ files: [], branch: '' });
        return;
      }
      const lines = stdout.split('\n');
      let branch = '';
      const files: { path: string; status: string; staged: boolean }[] = [];
      for (const line of lines) {
        if (line.startsWith('## ')) {
          branch = line.slice(3).split('...')[0].split(' ')[0];
        } else if (line.length >= 2) {
          const statusCode = line.slice(0, 2);
          const filePath = line.slice(3).trim();
          if (filePath) {
            const staged = statusCode[0] !== ' ' && statusCode[0] !== '?';
            const status = statusCode.trim();
            files.push({ path: filePath, status, staged });
          }
        }
      }
      resolve({ files, branch });
    });
    proc.on('error', () => {
      resolve({ files: [], branch: '' });
    });
  });
}
