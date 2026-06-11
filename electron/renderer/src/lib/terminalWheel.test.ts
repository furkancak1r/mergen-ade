import { describe, expect, it } from 'vitest';
import { terminalWheelEnabled } from './terminalWheel';

describe('terminalWheelEnabled', () => {
  it('allows terminal wheel handling when no overlays are open', () => {
    expect(terminalWheelEnabled({ settingsOpen: false, checklistVisible: false, terminalManagerOverlayOpen: false })).toBe(true);
  });

  it('disables terminal wheel handling while Settings is open', () => {
    expect(terminalWheelEnabled({ settingsOpen: true, checklistVisible: false, terminalManagerOverlayOpen: false })).toBe(false);
  });

  it('disables terminal wheel handling while checklist overlay is visible', () => {
    expect(terminalWheelEnabled({ settingsOpen: false, checklistVisible: true, terminalManagerOverlayOpen: false })).toBe(false);
  });

  it('disables terminal wheel handling while Terminal Manager popups are open', () => {
    expect(terminalWheelEnabled({ settingsOpen: false, checklistVisible: false, terminalManagerOverlayOpen: true })).toBe(false);
  });
});
