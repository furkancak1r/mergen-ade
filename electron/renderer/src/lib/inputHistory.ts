import type { AppHistory, InputHistoryFilter, ProjectRecord, TerminalInputRecord, TerminalKind } from '../../../shared/types';
import { defaultTerminalInputHistory, InputHistoryFilter as InputHistoryFilterEnum, TerminalKind as TerminalKindEnum } from '../../../shared/types';

export interface InputHistoryTerminalLike {
  id: number;
  projectId: number;
  kind: TerminalKind;
  recentInputs: string[];
}

export interface InputHistoryEntry {
  text: string;
  projectId: number;
  projectName: string;
  kind: TerminalKind;
  recordedAt: number;
}

export interface InputHistoryResult {
  entries: InputHistoryEntry[];
  totalMatching: number;
}

export function inputHistoryFilterMatchesKind(filter: InputHistoryFilter, kind: TerminalKind): boolean {
  if (filter === InputHistoryFilterEnum.Foreground) return kind === TerminalKindEnum.Foreground;
  if (filter === InputHistoryFilterEnum.Background) return kind === TerminalKindEnum.Background;
  return true;
}

export function collectInputHistoryEntries(
  history: AppHistory,
  projects: ProjectRecord[],
  selectedProjectId: number | null,
  filter: InputHistoryFilter,
  searchQuery: string,
  foregroundLimit = 5,
): InputHistoryResult {
  if (selectedProjectId === null) return { entries: [], totalMatching: 0 };

  const search = searchQuery.trim().toLowerCase();
  const project = projects.find((candidate) => candidate.id === selectedProjectId);
  if (!project) return { entries: [], totalMatching: 0 };

  const entries: InputHistoryEntry[] = [];
  const projectHistory = history.projects[project.path];
  const records = projectHistory?.entries ?? [];

  for (const record of records) {
    if (!inputHistoryFilterMatchesKind(filter, record.terminalKind)) continue;
    if (search && !record.text.toLowerCase().includes(search)) continue;
    entries.push({
      text: record.text,
      projectId: selectedProjectId,
      projectName: record.projectName || project.name,
      kind: record.terminalKind,
      recordedAt: record.recordedAt,
    });
  }

  const totalMatching = entries.length;
  if (filter === InputHistoryFilterEnum.Foreground && entries.length > foregroundLimit) {
    return { entries: entries.slice(0, foregroundLimit), totalMatching };
  }

  return { entries, totalMatching };
}

export function recordInputHistory(
  history: AppHistory,
  project: ProjectRecord | undefined,
  kind: TerminalKind,
  text: string,
  recordedAt: number,
): AppHistory {
  const trimmed = text.trim();
  if (!project || kind === TerminalKindEnum.Background || !trimmed || trimmed.startsWith('/')) {
    return history;
  }

  const existing = history.projects[project.path] ?? defaultTerminalInputHistory();
  const maxEntries = existing.maxEntries || defaultTerminalInputHistory().maxEntries;
  const record: TerminalInputRecord = {
    projectPath: project.path,
    projectName: project.name,
    terminalKind: kind,
    text: trimmed,
    recordedAt,
  };
  const nextEntries = [record, ...existing.entries].slice(0, maxEntries);

  return {
    ...history,
    projects: {
      ...history.projects,
      [project.path]: {
        maxEntries,
        entries: nextEntries,
      },
    },
  };
}

export function removeProjectInputHistory(history: AppHistory, projectPath: string | undefined): AppHistory {
  if (!projectPath || !(projectPath in history.projects)) {
    return history;
  }

  const { [projectPath]: _removed, ...projects } = history.projects;
  return {
    ...history,
    projects,
  };
}

export function removeProjectsInputHistory(history: AppHistory, projectPaths: string[]): AppHistory {
  let next = history;
  for (const projectPath of projectPaths) {
    next = removeProjectInputHistory(next, projectPath);
  }
  return next;
}

export function formatHistoryRelativeTime(recordedAt: number, nowSeconds = Math.floor(Date.now() / 1000)): string {
  const ageSeconds = Math.max(0, nowSeconds - recordedAt);
  if (ageSeconds < 60) return 'just now';
  if (ageSeconds < 3600) return `${Math.floor(ageSeconds / 60)}m ago`;
  if (ageSeconds < 86400) return `${Math.floor(ageSeconds / 3600)}h ago`;
  if (ageSeconds < 604800) return `${Math.floor(ageSeconds / 86400)}d ago`;
  if (ageSeconds < 2592000) return `${Math.floor(ageSeconds / 604800)}w ago`;
  return `${Math.floor(ageSeconds / 2592000)}mo ago`;
}
