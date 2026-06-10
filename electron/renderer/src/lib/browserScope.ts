import type { BrowserScopeKey } from '../../../shared/types';
import { BrowserScopeKeyType } from '../../../shared/types';

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
