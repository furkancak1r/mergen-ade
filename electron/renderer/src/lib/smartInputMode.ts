export type SmartInputModeId = 'auto' | 'build' | 'plan' | 'codex_plan';
export type RuntimeSmartInputModeId = 'build' | 'plan';

export const normalizeSmartInputModeId = (modeId?: string): RuntimeSmartInputModeId => {
  return modeId === 'plan' ? 'plan' : 'build';
};

export const toggleSmartInputModeId = (modeId: SmartInputModeId): SmartInputModeId => {
  return modeId === 'plan' ? 'build' : 'plan';
};

export const smartInputModeLabel = (modeId?: string): string | undefined => {
  if (modeId === 'plan') return 'Plan';
  if (modeId === 'codex_plan') return 'Codex Plan';
  if (modeId === 'build') return undefined;
  return 'Auto';
};

export const shouldSendOpenCodeModeToggle = (currentMode: string | undefined, targetMode: string | undefined): boolean => {
  return normalizeSmartInputModeId(currentMode) !== normalizeSmartInputModeId(targetMode);
};
