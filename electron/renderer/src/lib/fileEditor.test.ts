import { describe, expect, it } from 'vitest';
import {
  fileEditorDisplayName,
  fileEditorLocationFromPath,
  initialFileEditorNavigationState,
  selectedTextFromRange,
  withFileEditorClosed,
  withFileEditorHidden,
  withFileEditorNavigateBack,
  withFileEditorNavigateForward,
  withFileEditorOpened,
} from './fileEditor';

describe('fileEditor helpers', () => {
  it('returns null when no text is selected', () => {
    expect(selectedTextFromRange('hello', 2, 2)).toBeNull();
  });

  it('returns the selected text for forward and reversed ranges', () => {
    expect(selectedTextFromRange('hello world', 0, 5)).toBe('hello');
    expect(selectedTextFromRange('hello world', 5, 0)).toBe('hello');
  });

  it('clamps ranges to the text bounds', () => {
    expect(selectedTextFromRange('hello', -10, 20)).toBe('hello');
  });

  it('derives display names from Windows and POSIX paths', () => {
    expect(fileEditorDisplayName('C:\\repo\\src\\app.tsx')).toBe('app.tsx');
    expect(fileEditorDisplayName('/repo/src/app.tsx')).toBe('app.tsx');
  });

  it('opens files and pushes the previous active file to back history', () => {
    let state = initialFileEditorNavigationState();
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\two.ts'));

    expect(state.open).toBe(true);
    expect(state.active?.displayName).toBe('two.ts');
    expect(state.backStack.map((entry) => entry.displayName)).toEqual(['one.ts']);
    expect(state.forwardStack).toEqual([]);
  });

  it('does not duplicate history when the active file is reopened', () => {
    let state = initialFileEditorNavigationState();
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));

    expect(state.backStack).toEqual([]);
    expect(state.active?.displayName).toBe('one.ts');
  });

  it('navigates back and forward while preserving opposite history stacks', () => {
    let state = initialFileEditorNavigationState();
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\two.ts'));

    state = withFileEditorNavigateBack(state);
    expect(state.active?.displayName).toBe('one.ts');
    expect(state.backStack).toEqual([]);
    expect(state.forwardStack.map((entry) => entry.displayName)).toEqual(['two.ts']);

    state = withFileEditorNavigateForward(state);
    expect(state.active?.displayName).toBe('two.ts');
    expect(state.backStack.map((entry) => entry.displayName)).toEqual(['one.ts']);
    expect(state.forwardStack).toEqual([]);
  });

  it('clears forward history when opening a different file after navigating back', () => {
    let state = initialFileEditorNavigationState();
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\two.ts'));
    state = withFileEditorNavigateBack(state);
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\three.ts'));

    expect(state.active?.displayName).toBe('three.ts');
    expect(state.backStack.map((entry) => entry.displayName)).toEqual(['one.ts']);
    expect(state.forwardStack).toEqual([]);
  });

  it('caps file editor history to the configured maximum', () => {
    let state = initialFileEditorNavigationState(2);
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\two.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\three.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\four.ts'));

    expect(state.backStack.map((entry) => entry.displayName)).toEqual(['two.ts', 'three.ts']);
  });

  it('hides the editor without clearing active file or history', () => {
    let state = initialFileEditorNavigationState();
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\two.ts'));
    state = withFileEditorHidden(state);

    expect(state.open).toBe(false);
    expect(state.active?.displayName).toBe('two.ts');
    expect(state.backStack.map((entry) => entry.displayName)).toEqual(['one.ts']);
  });

  it('closes the editor by clearing active file and history', () => {
    let state = initialFileEditorNavigationState();
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\one.ts'));
    state = withFileEditorOpened(state, fileEditorLocationFromPath('C:\\repo\\two.ts'));
    state = withFileEditorClosed(state);

    expect(state.open).toBe(false);
    expect(state.active).toBeNull();
    expect(state.backStack).toEqual([]);
    expect(state.forwardStack).toEqual([]);
  });
});
