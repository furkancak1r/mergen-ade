import { describe, it, expect } from 'vitest';
import { BrowserScopeKeyType } from '../../../shared/types';
import {
  scopeKeyString,
  activeBrowserScope,
  shouldPersistUrl,
  isTerminalScope,
} from './browserScope';

describe('browserScope', () => {
  it('scopeKeyString for project', () => {
    expect(scopeKeyString({ type: BrowserScopeKeyType.Project, projectId: 5 })).toBe('project:5');
  });

  it('scopeKeyString for terminal', () => {
    expect(scopeKeyString({ type: BrowserScopeKeyType.Terminal, projectId: 5, terminalId: 12 })).toBe('terminal:5:12');
  });

  it('activeBrowserScope uses visible override first', () => {
    const override = { type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 99 };
    const result = activeBrowserScope(1, 2, override, () => false, () => false);
    expect(result).toEqual(override);
  });

  it('activeBrowserScope falls back to terminal scope when tabs exist', () => {
    const result = activeBrowserScope(1, 2, undefined, () => true, () => false);
    expect(result).toEqual({ type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 2 });
  });

  it('activeBrowserScope falls back to project scope when no terminal tabs', () => {
    const result = activeBrowserScope(1, 2, undefined, () => false, () => true);
    expect(result).toEqual({ type: BrowserScopeKeyType.Project, projectId: 1 });
  });

  it('activeBrowserScope returns undefined when no tabs anywhere', () => {
    const result = activeBrowserScope(1, 2, undefined, () => false, () => false);
    expect(result).toBeUndefined();
  });

  it('shouldPersistUrl true only for project scope', () => {
    expect(shouldPersistUrl({ type: BrowserScopeKeyType.Project, projectId: 1 })).toBe(true);
    expect(shouldPersistUrl({ type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 2 })).toBe(false);
  });

  it('isTerminalScope true only for terminal scope', () => {
    expect(isTerminalScope({ type: BrowserScopeKeyType.Project, projectId: 1 })).toBe(false);
    expect(isTerminalScope({ type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 2 })).toBe(true);
  });
});
