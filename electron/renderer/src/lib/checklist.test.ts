import { describe, expect, it } from 'vitest';
import type { ProjectRecord } from '../../../shared/types';
import {
  CHECKLIST_EMPTY_MESSAGE,
  checklistCopiedItemsMessage,
  checklistRightOffset,
  formatChecklistForClipboard,
  projectsWithChecklistItems,
} from './checklist';

function project(partial: Partial<ProjectRecord>): ProjectRecord {
  return {
    id: partial.id ?? 1,
    name: partial.name ?? 'Project',
    path: partial.path ?? 'C:\\repo',
    savedMessages: [],
    aiConfig: {},
    checklist: partial.checklist ?? [],
    isWorktree: false,
    ...partial,
  };
}

describe('checklist helpers', () => {
  it('keeps only projects with checklist items sorted by project id', () => {
    expect(projectsWithChecklistItems([
      project({ id: 3, checklist: ['third'] }),
      project({ id: 1, checklist: [] }),
      project({ id: 2, checklist: ['second'] }),
    ]).map((item) => item.id)).toEqual([2, 3]);
  });

  it('formats checklist clipboard and status text like the Rust panel', () => {
    expect(CHECKLIST_EMPTY_MESSAGE).toBe('No items in Check-list\nOpen history popup and check items to add');
    expect(formatChecklistForClipboard(['one', 'two\nline'])).toBe('one\n\ntwo\nline');
    expect(checklistCopiedItemsMessage(1)).toBe('Copied 1 checklist items');
    expect(checklistCopiedItemsMessage(2)).toBe('Copied 2 checklist items');
  });

  it('offsets the floating checklist to the left of the browser panel', () => {
    expect(checklistRightOffset(false, 520)).toBe(18);
    expect(checklistRightOffset(true, 520)).toBe(544);
  });
});
