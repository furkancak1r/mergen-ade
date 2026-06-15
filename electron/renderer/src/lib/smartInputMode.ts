import type { AcpRouteMode } from '../../../shared/acpRoute';
import { acpRouteLabel, normalizeAcpRouteMode } from '../../../shared/acpRoute';

export type SmartInputModeId = AcpRouteMode;
export type RuntimeSmartInputModeId = 'build' | 'plan';

export const normalizeSmartInputModeId = (modeId?: string): RuntimeSmartInputModeId => {
  return modeId === 'plan' ? 'plan' : 'build';
};

export const toggleSmartInputModeId = (modeId: SmartInputModeId): SmartInputModeId => {
  return modeId === 'plan' ? 'build' : 'plan';
};

export const smartInputModeLabel = (modeId?: string): string | undefined => {
  const route = normalizeAcpRouteMode(modeId);
  return route === 'build' ? undefined : acpRouteLabel(route);
};

export const shouldSendOpenCodeModeToggle = (currentMode: string | undefined, targetMode: string | undefined): boolean => {
  return normalizeSmartInputModeId(currentMode) !== normalizeSmartInputModeId(targetMode);
};
