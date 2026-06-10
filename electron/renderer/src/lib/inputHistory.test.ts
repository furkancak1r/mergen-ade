import { describe, expect, it } from 'vitest';
import type { AppHistory, ProjectRecord, TerminalInputRecord } from '../../../shared/types';
import { defaultAppHistory, InputHistoryFilter, TerminalKind } from '../../../shared/types';
import {
  collectInputHistoryEntries,
  formatHistoryRelativeTime,
  inputHistoryFilterMatchesKind,
  recordInputHistory,
} from './inputHistory';

function project(partial: Partial<ProjectRecord>): ProjectRecord {
  return {
    id: partial.id ?? 1,
    name: partial.name ?? 'Project',
    path: partial.path ?? 'C:\\repo',
    savedMessages: [],
    foregroundSavedMessages: [],
    aiConfig: {},
    checklist: [],
    isWorktree: false,
    ...partial,
  };
}

function record(partial: Partial<TerminalInputRecord>): TerminalInputRecord {
  return {
    projectPath: partial.projectPath ?? 'C:\\repo',
    projectName: partial.projectName ?? 'Project',
    terminalKind: partial.terminalKind ?? TerminalKind.Foreground,
    text: partial.text ?? 'cmd',
    recordedAt: partial.recordedAt ?? 1000,
  };
}

function history(entries: TerminalInputRecord[], path = 'C:\\repo', maxEntries = 500): AppHistory {
  return {
    version: 1,
    projects: {
      [path]: {
        maxEntries,
        entries,
      },
    },
  };
}

describe('inputHistory', () => {
  it('matches All, Foreground, and Background filters by terminal kind', () => {
    expect(inputHistoryFilterMatchesKind(InputHistoryFilter.All, TerminalKind.Foreground)).toBe(true);
    expect(inputHistoryFilterMatchesKind(InputHistoryFilter.All, TerminalKind.Background)).toBe(true);
    expect(inputHistoryFilterMatchesKind(InputHistoryFilter.Foreground, TerminalKind.Foreground)).toBe(true);
    expect(inputHistoryFilterMatchesKind(InputHistoryFilter.Foreground, TerminalKind.Background)).toBe(false);
    expect(inputHistoryFilterMatchesKind(InputHistoryFilter.Background, TerminalKind.Background)).toBe(true);
  });

  it('collects only selected project entries and includes both kinds for All', () => {
    const result = collectInputHistoryEntries(
      history([
        record({ text: 'fg', terminalKind: TerminalKind.Foreground }),
        record({ text: 'bg', terminalKind: TerminalKind.Background }),
      ]),
      [project({ id: 1, name: 'Root', path: 'C:\\repo' }), project({ id: 2, name: 'Other', path: 'C:\\other' })],
      1,
      InputHistoryFilter.All,
      '',
    );

    expect(result.entries.map((entry) => entry.text)).toEqual(['fg', 'bg']);
    expect(result.totalMatching).toBe(2);
  });

  it('limits foreground history but preserves total matching count', () => {
    const result = collectInputHistoryEntries(
      history(['one', 'two', 'three', 'four', 'five', 'six'].map((text, index) => record({ text, recordedAt: index }))),
      [project({ id: 1, path: 'C:\\repo' })],
      1,
      InputHistoryFilter.Foreground,
      '',
      5,
    );

    expect(result.entries.map((entry) => entry.text)).toEqual(['one', 'two', 'three', 'four', 'five']);
    expect(result.totalMatching).toBe(6);
  });

  it('filters entries by search text', () => {
    const result = collectInputHistoryEntries(
      history(['npm test', 'cargo test', 'git status'].map((text) => record({ text }))),
      [project({ id: 1 })],
      1,
      InputHistoryFilter.All,
      'test',
    );

    expect(result.entries.map((entry) => entry.text)).toEqual(['npm test', 'cargo test']);
  });

  it('records foreground input into persistent project history', () => {
    const appHistory = defaultAppHistory();
    const next = recordInputHistory(
      appHistory,
      project({ id: 1, name: 'Root', path: 'C:\\repo' }),
      TerminalKind.Foreground,
      ' npm test ',
      1234,
    );

    expect(next).not.toBe(appHistory);
    expect(next.projects['C:\\repo'].entries[0]).toMatchObject({
      projectPath: 'C:\\repo',
      projectName: 'Root',
      terminalKind: TerminalKind.Foreground,
      text: 'npm test',
      recordedAt: 1234,
    });
  });

  it('skips background, slash, and empty inputs for persistent recording', () => {
    const appHistory = defaultAppHistory();
    const root = project({ id: 1, path: 'C:\\repo' });

    expect(recordInputHistory(appHistory, root, TerminalKind.Background, 'bg task', 1)).toBe(appHistory);
    expect(recordInputHistory(appHistory, root, TerminalKind.Foreground, '/slash', 1)).toBe(appHistory);
    expect(recordInputHistory(appHistory, root, TerminalKind.Foreground, '   ', 1)).toBe(appHistory);
  });

  it('migrates zero max entries and enforces history limit', () => {
    const appHistory = history([record({ text: 'old' })], 'C:\\repo', 0);
    const next = recordInputHistory(
      appHistory,
      project({ id: 1, path: 'C:\\repo' }),
      TerminalKind.Foreground,
      'new',
      2,
    );

    expect(next.projects['C:\\repo'].maxEntries).toBe(500);
    expect(next.projects['C:\\repo'].entries.map((entry) => entry.text)).toEqual(['new', 'old']);
  });

  it('formats relative history times', () => {
    expect(formatHistoryRelativeTime(100, 120)).toBe('now');
    expect(formatHistoryRelativeTime(60, 180)).toBe('2m');
    expect(formatHistoryRelativeTime(0, 7200)).toBe('2h');
    expect(formatHistoryRelativeTime(0, 172800)).toBe('2d');
  });
});
