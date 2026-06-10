export function selectedTextFromRange(text: string, selectionStart: number, selectionEnd: number): string | null {
  const start = Math.max(0, Math.min(selectionStart, selectionEnd, text.length));
  const end = Math.max(0, Math.min(Math.max(selectionStart, selectionEnd), text.length));
  if (start === end) return null;
  return text.slice(start, end);
}

export const FILE_EDITOR_MAX_HISTORY = 20;

export interface FileEditorLocation {
  path: string;
  displayName: string;
}

export interface FileEditorNavigationState {
  active: FileEditorLocation | null;
  open: boolean;
  backStack: FileEditorLocation[];
  forwardStack: FileEditorLocation[];
  maxHistory: number;
}

export function fileEditorDisplayName(filePath: string): string {
  const parts = filePath.split(/[\\/]/);
  return parts[parts.length - 1] || filePath;
}

export function fileEditorLocationFromPath(filePath: string): FileEditorLocation {
  return {
    path: filePath,
    displayName: fileEditorDisplayName(filePath),
  };
}

export function initialFileEditorNavigationState(maxHistory = FILE_EDITOR_MAX_HISTORY): FileEditorNavigationState {
  return {
    active: null,
    open: false,
    backStack: [],
    forwardStack: [],
    maxHistory,
  };
}

function boundedPush(stack: FileEditorLocation[], location: FileEditorLocation, maxHistory: number): FileEditorLocation[] {
  const next = [...stack, location];
  const overflow = next.length - maxHistory;
  return overflow > 0 ? next.slice(overflow) : next;
}

export function withFileEditorOpened(
  state: FileEditorNavigationState,
  location: FileEditorLocation,
): FileEditorNavigationState {
  if (state.active?.path === location.path) {
    return {
      ...state,
      active: location,
      open: true,
    };
  }

  return {
    ...state,
    active: location,
    open: true,
    backStack: state.active
      ? boundedPush(state.backStack, state.active, state.maxHistory)
      : state.backStack,
    forwardStack: [],
  };
}

export function withFileEditorHidden(state: FileEditorNavigationState): FileEditorNavigationState {
  return {
    ...state,
    open: false,
  };
}

export function withFileEditorClosed(state: FileEditorNavigationState): FileEditorNavigationState {
  return {
    ...state,
    active: null,
    open: false,
    backStack: [],
    forwardStack: [],
  };
}

export function withFileEditorNavigateBack(state: FileEditorNavigationState): FileEditorNavigationState {
  if (!state.active || state.backStack.length === 0) return state;

  const previous = state.backStack[state.backStack.length - 1];
  return {
    ...state,
    active: previous,
    open: true,
    backStack: state.backStack.slice(0, -1),
    forwardStack: boundedPush(state.forwardStack, state.active, state.maxHistory),
  };
}

export function withFileEditorNavigateForward(state: FileEditorNavigationState): FileEditorNavigationState {
  if (!state.active || state.forwardStack.length === 0) return state;

  const next = state.forwardStack[state.forwardStack.length - 1];
  return {
    ...state,
    active: next,
    open: true,
    backStack: boundedPush(state.backStack, state.active, state.maxHistory),
    forwardStack: state.forwardStack.slice(0, -1),
  };
}
