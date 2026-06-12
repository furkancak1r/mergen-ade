export interface TerminalWheelOverlayState {
  settingsOpen: boolean;
  terminalManagerOverlayOpen: boolean;
}

export const terminalWheelEnabled = (state: TerminalWheelOverlayState): boolean => {
  return !state.settingsOpen && !state.terminalManagerOverlayOpen;
};
