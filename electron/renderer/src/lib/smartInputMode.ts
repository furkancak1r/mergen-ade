export type SmartInputModeId = 'build' | 'plan';

export const normalizeSmartInputModeId = (modeId?: string): SmartInputModeId => {
  return modeId === 'plan' ? 'plan' : 'build';
};

export const toggleSmartInputModeId = (modeId: SmartInputModeId): SmartInputModeId => {
  return modeId === 'plan' ? 'build' : 'plan';
};

export const smartInputModeLabel = (modeId?: string): string | undefined => {
  return normalizeSmartInputModeId(modeId) === 'plan' ? 'Plan' : undefined;
};

export const shouldSendOpenCodeModeToggle = (currentMode: string | undefined, targetMode: string | undefined): boolean => {
  return normalizeSmartInputModeId(currentMode) !== normalizeSmartInputModeId(targetMode);
};
