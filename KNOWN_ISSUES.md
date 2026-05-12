
---

#### Create Worktree popup text routed to active terminal {#create-worktree-popup-keyboard-leak}
- Date: 2026-05-12
- Context: Adding the Create Worktree modal popup without including it in keyboard-ownership guards.
- Error signature: Internal review found that typing into the Create Worktree branch/path fields while a terminal was active sent keystrokes to the terminal instead.
- Symptoms/Impact:
  1. With an active terminal accepting input, opening the Create Worktree popup and typing branch/base/path text echoed into the terminal PTY.
  2. Terminal shortcuts could fire while the popup was open because `ui_owns_keyboard` was false.
  3. AI attention routing could steal text from the popup fields.
- Root cause:
  - `show_create_worktree_popup` was not included in `text_input_has_focus_extended()`, `should_steal_attention_terminal_input()`, `embedded_browser_should_yield_to_ui_layer()`, or `terminal_output_mouse_wheel_enabled()`.
- Resolution:
  - Added `show_create_worktree_popup` to `text_input_has_focus_extended()` and `should_steal_attention_terminal_input()`.
  - Extended `embedded_browser_should_yield_to_ui_layer()` and `terminal_output_mouse_wheel_enabled()` with a `create_worktree_popup_open` parameter and wired it through all call sites and tests.
- Prevent recurrence:
  - Added regression tests:
    - `text_input_has_focus_extended_detects_create_worktree_popup`
    - `create_worktree_popup_blocks_attention_stealing`
    - `terminal_output_mouse_wheel_enabled_returns_false_when_create_worktree_open`
    - `embedded_browser_yields_to_ui_overlay_layers` now asserts yield for create worktree open.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: Internal review 2026-05-12

---

#### Discovered worktree rows were hover-only and unclickable {#worktree-row-hover-only}
- Date: 2026-05-12
- Context: Source Control panel “Worktrees” section rendered actionable rows using `draw_sidebar_text_row()`, which allocates with `Sense::hover()`.
- Error signature: Internal review found that clicking an unregistered worktree row did nothing.
- Symptoms/Impact:
  1. Unregistered worktree rows showed a pointing-hand cursor on hover but `row_response.clicked()` was always false.
  2. Users could not add discovered worktrees by clicking the row.
- Root cause:
  - `draw_sidebar_text_row()` uses `Sense::hover()`, so `Response::clicked()` can never be true.
- Resolution:
  - Added `draw_clickable_sidebar_text_row()` helper that uses `Sense::click()` and preserves the same tooltip/truncation behavior.
  - Used the new helper only for worktree rows in the Source Control panel; all other sidebar text rows remain hover-only.
- Prevent recurrence:
  - Manual temp-repo validation for the click path.
  - AGENTS.md guideline: actionable sidebar rows must use click-capable `Sense`; hover-only rows must not be used for Add-to-Mergen actions.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`
- References: Internal review 2026-05-12

---

#### Foreground task popup Enter submit was no longer drained in update loop {#foreground-popup-submit-drain-missing}
- Date: 2026-05-12
- Context: Adding the `draw_create_worktree_popup()` call in the update loop replaced/removed the deferred `process_pending_foreground_message_popup_submit()` drain.
- Error signature: Internal review found that pressing plain Enter in the foreground task popup set `submit_pending` but never saved the task.
- Symptoms/Impact:
  1. Opening the foreground message popup, typing a task, and pressing plain Enter did nothing.
  2. The Save button still worked, but keyboard submission was broken.
  3. Same-frame text before Enter was not preserved because the deferred save never executed.
- Root cause:
  - The update loop call `self.process_pending_foreground_message_popup_submit()` was removed while inserting `self.draw_create_worktree_popup(ctx)`.
- Resolution:
  - Restored `self.process_pending_foreground_message_popup_submit()` immediately after `self.draw_foreground_message_popup(ctx)` and before `self.draw_create_worktree_popup(ctx)`.
- Prevent recurrence:
  - Existing regression tests still pass:
    - `foreground_message_popup_enter_defers_save_until_after_text_edit`
    - `foreground_message_popup_submit_preserves_same_frame_text`
    - `foreground_message_popup_ctrl_enter_does_not_submit`
  - AGENTS.md guideline: the update loop must drain the deferred submit immediately after `draw_foreground_message_popup()`; new modal popups must not remove or move this drain.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: Internal review 2026-05-12
