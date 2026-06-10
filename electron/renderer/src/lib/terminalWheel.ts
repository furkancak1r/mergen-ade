export interface TerminalWheelOverlayState {
  settingsOpen: boolean;
  checklistVisible: boolean;
  terminalManagerOverlayOpen: boolean;
}

export const terminalWheelEnabled = (state: TerminalWheelOverlayState): boolean => {
  return !state.settingsOpen && !state.checklistVisible && !state.terminalManagerOverlayOpen;
};
