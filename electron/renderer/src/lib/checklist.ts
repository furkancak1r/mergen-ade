import type { ProjectRecord } from '../../../shared/types';

export const CHECKLIST_EMPTY_MESSAGE = 'No items in Check-list\nOpen history popup and check items to add';

export function projectsWithChecklistItems(projects: ProjectRecord[]): ProjectRecord[] {
  return projects
    .filter((project) => project.checklist.length > 0)
    .sort((a, b) => a.id - b.id);
}

export function formatChecklistForClipboard(items: string[]): string {
  return items.join('\n\n');
}

export function checklistCopiedItemsMessage(count: number): string {
  return `Copied ${count} checklist items`;
}

export function checklistRightOffset(browserPanelOpen: boolean, browserPanelWidth: number): number {
  return browserPanelOpen ? browserPanelWidth + 24 : 18;
}
