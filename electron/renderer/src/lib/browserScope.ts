import type { BrowserScopeKey, BrowserTab, ProjectRecord } from '../../../shared/types';
import { BrowserScopeKeyType } from '../../../shared/types';

export type ProjectFamilyRecord = Pick<ProjectRecord, 'id' | 'path' | 'repoRoot' | 'browserLastUrl' | 'isWorktree'>;

export function scopeKeyString(scope: BrowserScopeKey): string {
  if (scope.type === BrowserScopeKeyType.Project) return `project:${scope.projectId}`;
  return `terminal:${scope.projectId}:${scope.terminalId}`;
}

export function activeBrowserScope(
  activeProjectId: number,
  activeTerminalId: number | undefined,
  visibleScopeOverride: BrowserScopeKey | undefined,
  terminalHasTabs: (terminalId: number) => boolean,
  projectHasTabs: (projectId: number) => boolean,
): BrowserScopeKey | undefined {
  if (visibleScopeOverride) {
    return visibleScopeOverride;
  }
  if (activeTerminalId !== undefined && terminalHasTabs(activeTerminalId)) {
    return { type: BrowserScopeKeyType.Terminal, projectId: activeProjectId, terminalId: activeTerminalId };
  }
  if (projectHasTabs(activeProjectId)) {
    return { type: BrowserScopeKeyType.Project, projectId: activeProjectId };
  }
  return undefined;
}

export function shouldPersistUrl(scope: BrowserScopeKey): boolean {
  return scope.type === BrowserScopeKeyType.Project;
}

export function isTerminalScope(scope: BrowserScopeKey): boolean {
  return scope.type === BrowserScopeKeyType.Terminal;
}

export function projectFamilyRootPath(project: ProjectFamilyRecord): string {
  return project.repoRoot || project.path;
}

export function isProjectInFamily(target: ProjectFamilyRecord, candidate: ProjectFamilyRecord): boolean {
  const rootPath = projectFamilyRootPath(target);
  const candidateRoot = candidate.repoRoot || candidate.path;
  return candidate.id === target.id || candidate.path === rootPath || candidateRoot === rootPath;
}

function orderedProjectFamily<T extends ProjectFamilyRecord>(
  project: T,
  projects: T[],
  rootFirst: boolean,
): T[] {
  const members = projects.filter((candidate) => isProjectInFamily(project, candidate));
  const rootPath = projectFamilyRootPath(project);
  const root = members.find((candidate) => !candidate.isWorktree && candidate.path === rootPath)
    ?? members.find((candidate) => candidate.path === rootPath);
  const ordered: T[] = [];
  const seen = new Set<number>();
  const push = (candidate: T | undefined) => {
    if (!candidate || seen.has(candidate.id)) return;
    seen.add(candidate.id);
    ordered.push(candidate);
  };

  if (rootFirst) {
    push(root);
    push(project);
  } else {
    push(project);
    push(root);
  }
  for (const member of members) {
    push(member);
  }
  return ordered;
}

export function browserLastUrlForProjectFamily<T extends ProjectFamilyRecord>(
  project: T,
  projects: T[],
): string | undefined {
  return orderedProjectFamily(project, projects, true).find((member) => member.browserLastUrl)?.browserLastUrl;
}

export function activeBrowserTabUrlForScope(
  scope: BrowserScopeKey,
  tabsByScope: Map<string, BrowserTab[]>,
  activeTabByScope: Map<string, string | null | undefined>,
): string | undefined {
  const key = scopeKeyString(scope);
  const tabs = tabsByScope.get(key) ?? [];
  const activeTabId = activeTabByScope.get(key);
  if (!activeTabId) {
    return tabs.find((tab) => tab.url)?.url;
  }
  return tabs.find((tab) => tab.id === activeTabId)?.url ?? tabs.find((tab) => tab.url)?.url;
}

export function browserUrlForProjectFamily<T extends ProjectFamilyRecord>(
  project: T,
  projects: T[],
  tabsByScope: Map<string, BrowserTab[]>,
  activeTabByScope: Map<string, string | null | undefined>,
  visibleScopeByProject?: Map<number, BrowserScopeKey>,
): string | undefined {
  const projectScope = { type: BrowserScopeKeyType.Project, projectId: project.id };
  const visibleScope = visibleScopeByProject?.get(project.id) ?? projectScope;
  const activeVisibleUrl = activeBrowserTabUrlForScope(visibleScope, tabsByScope, activeTabByScope);
  if (activeVisibleUrl) return activeVisibleUrl;

  if (visibleScope.type !== projectScope.type || visibleScope.projectId !== projectScope.projectId) {
    const activeProjectUrl = activeBrowserTabUrlForScope(projectScope, tabsByScope, activeTabByScope);
    if (activeProjectUrl) return activeProjectUrl;
  }

  for (const member of orderedProjectFamily(project, projects, false)) {
    if (member.id === project.id) continue;
    const url = activeBrowserTabUrlForScope(
      { type: BrowserScopeKeyType.Project, projectId: member.id },
      tabsByScope,
      activeTabByScope,
    );
    if (url) return url;
  }

  return browserLastUrlForProjectFamily(project, projects);
}

export function withBrowserLastUrlForProjectFamily<T extends ProjectFamilyRecord>(
  projects: T[],
  projectId: number,
  url: string,
): T[] {
  const target = projects.find((project) => project.id === projectId);
  if (!target) return projects;

  let changed = false;
  const next = projects.map((project) => {
    if (!isProjectInFamily(target, project) || project.browserLastUrl === url) return project;
    changed = true;
    return { ...project, browserLastUrl: url } as T;
  });
  return changed ? next : projects;
}

export function withoutBrowserLastUrlForProjectFamily<T extends ProjectFamilyRecord>(
  projects: T[],
  projectId: number,
): T[] {
  const target = projects.find((project) => project.id === projectId);
  if (!target) return projects;

  let changed = false;
  const next = projects.map((project) => {
    if (!isProjectInFamily(target, project) || !project.browserLastUrl) return project;
    changed = true;
    const { browserLastUrl: _browserLastUrl, ...rest } = project;
    return rest as T;
  });
  return changed ? next : projects;
}
