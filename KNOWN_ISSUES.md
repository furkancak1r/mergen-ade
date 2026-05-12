
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

---

#### Source Control auto refresh flashed loading indicator and Create Worktree path was manual {#source-control-silent-refresh-worktree-auto-path}
- Date: 2026-05-12
- Context: User-facing UX feedback on worktree integration.
- Error signature:
  1. "source control refresh oluyor sürekli olarak refresh olduğu görüntüsü ui a gelmesin" — auto refresh should not constantly show "Refreshing source control...".
  2. "worktree pathi ben yazmayayım mümkünse otomatik oluşsun isme göre anlık gözüksün" — worktree path should auto-generate from branch name.
- Symptoms/Impact:
  1. Every 5–20 seconds the Source Control panel flashed "Refreshing source control..." even though no user action occurred, causing visual distraction.
  2. The Terminal Manager diff summary showed `...` every auto refresh cycle, making the row width jump.
  3. The Create Worktree modal required the user to manually type the worktree path even though it should always be a predictable sibling directory derived from the branch name.
- Root cause:
  - `request_source_control_refresh()` unconditionally set `snapshot.loading = true` regardless of whether the request was triggered by a user (manual) or by the periodic scheduler (auto).
  - `draw_create_worktree_popup()` presented the worktree path as a user-editable `TextEdit::singleline` with no auto-generation logic.
- Resolution:
  - Changed `request_source_control_refresh()` to only set `snapshot.loading = true` when `manual || run_fetch`. Auto refreshes now silently update the snapshot in the background without any loading UI.
  - Added `sanitize_worktree_slug()` and `default_worktree_path_for_branch()` helpers to compute the worktree path live from the branch name.
  - Replaced the editable path `TextEdit` in the Create Worktree modal with a read-only `.interactive(false)` field that updates in real time as the user types the branch name.
  - Path defaults to `<repo_parent>/worktrees/<branch-slug>` with `/`, `\`, spaces, and Windows-invalid characters replaced by `-`.
- Prevent recurrence:
  - Added regression tests:
    - `auto_source_control_refresh_does_not_set_loading`
    - `manual_source_control_refresh_sets_loading`
    - `worktree_slug_sanitizes_branch_name`
    - `default_worktree_path_computed_from_repo_parent`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12

---

#### Smart Input could not paste image-only clipboard data {#smart-input-image-paste}
- Date: 2026-05-12
- Context: Smart Input draft and queued-task edit fields only handled `Event::Paste(_)`, which egui does not emit when the clipboard contains an image without text.
- Error signature: User reported inability to paste images into Smart Input.
- Symptoms/Impact:
  1. Pressing Ctrl+V / Cmd+V in Smart Input with an image-only clipboard did nothing.
  2. Explorer-copied image files and screenshots could not be inserted as paths.
- Root cause:
  - `raw_input_hook` waited for an explicit `Event::Paste` from egui, which never arrives for image-only clipboard data.
- Resolution:
  - Added `synthesize_smart_input_image_paste_events()` to intercept the primary paste shortcut (`Ctrl+V` / `Cmd+V`) when Smart Input is focused.
  - If the clipboard contains an image file (via CF_HDROP) or bitmap data, it is saved to disk and a synthetic `Event::Paste(image_path)` is injected into the event stream so egui's TextEdit inserts the path.
- Prevent recurrence:
  - Added regression tests:
    - `smart_input_synthesizes_image_paste_on_primary_paste_key`
    - `smart_input_leaves_non_paste_keys_untouched`
    - `smart_input_leaves_shift_ctrl_v_untouched`
    - `smart_input_falls_back_to_normal_key_when_no_clipboard_image`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: Internal fix 2026-05-12

---

#### Smart Input and shortcuts sent only one delayed Enter, causing commands to stall waiting for confirmation {#smart-input-shortcut-confirmation-enters}
- Date: 2026-05-12
- Context: Smart Input manual/auto dispatch and terminal shortcuts schedule one delayed confirmation Enter after the initial submit. Users reported that the second Enter either never appeared (Smart Input looked like it sent nothing) or arrived too quickly (250ms) before the shell/AI CLI had time to render a confirmation prompt, so it was swallowed.
- Error signature: User reported that shortcuts and Smart Input "only press Enter once" and "Smart Input does not press Enter at all", and requested three staggered Enter presses with increased delay.
- Symptoms/Impact:
  1. Smart Input slash-prefixed commands (e.g. `/prepare-fix-plan`) sent only one immediate Enter and one short-delayed Enter (250ms). Many AI CLIs need two confirmation Enters after bracketed-paste delivery, so the prompt stalled waiting for additional input.
  2. The 250ms delay was too short for bracketed-paste processing + prompt rendering on slower machines or busy terminals, making the second Enter invisible or lost.
  3. Terminal shortcuts experienced the same too-short delay, making confirmation unreliable.
- Root cause:
  - `TerminalPromptSubmitOptions` only tracked a boolean `schedule_confirmation_enter` and always scheduled exactly one delayed Enter with a fixed 250ms delay for slash-prefixed commands.
  - No mechanism existed to send multiple staggered confirmation Enters.
- Resolution:
  - Replaced the boolean flag with `confirmation_enter_count: usize` in `TerminalPromptSubmitOptions`.
  - Set `confirmation_enter_count: 2` for `smart_manual()` and `smart_auto()` (producing an immediate Enter plus two delayed Enters).
  - Increased `SHORTCUT_SECOND_ENTER_DELAY_MS` from 250ms to 600ms so each confirmation Enter arrives after the shell/AI CLI has had time to process the paste and display any prompt.
  - `schedule_delayed_enters_for_terminal()` already supported `count`; wired `options.confirmation_enter_count` into `submit_prompt_to_terminal()`.
- Prevent recurrence:
  - Updated regression tests:
    - `smart_input_dispatches_one_task_on_opencode_turn_complete` now asserts `pending_second_enter.len() == 2`.
    - `smart_input_steer_now_slash_prefix_schedules_two_confirmation_enters` replaced the old 250ms single-Enter test and verifies two staggered delays (≈600ms and ≈1200ms) plus three total Enter bytes.
  - Updated `AGENTS.md` to document `confirmation_enter_count: 2` for Smart Input and the new 600ms shortcut delay.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12

---

#### Bright white resize handle on left panel was too harsh {#left-panel-resize-handle-harsh}
- Date: 2026-05-12
- Context: UI visual feedback on resizable left panel.
- Error signature: The default egui resize handle uses `hovered/active.fg_stroke` which is near-white, causing a glaring bright vertical bar when hovering or dragging the Project Explorer edge in the dark theme.
- Symptoms/Impact:
  1. When the mouse hovered over the right edge of the Project Explorer, a bright white vertical line appeared.
  2. During resize drag, the line remained bright and distracting against the dark background.
- Root cause:
  - egui's `SidePanel` draws the resize indicator using the widget `fg_stroke`, which is configured to near-white for hover/active states in the Mergen dark theme.
- Resolution:
  - Added a dim foreground-layer overlay on the panel's right edge after `draw_project_explorer()` finishes. The overlay uses `Color32::from_rgb(45, 45, 45)` normally and `Color32::from_rgb(80, 80, 80)` when the pointer is near the handle, replacing the bright default line without altering the resize hitbox or cursor behavior.
- Prevent recurrence:
  - The overlay is scoped inside `draw_project_explorer()` and only affects the Project Explorer edge.
  - Resize functionality (cursor, drag, width persistence) remains unchanged.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`
- References: User request 2026-05-12

---

#### Transient toast notification was too narrow causing text wrapping {#transient-toast-too-narrow}
- Date: 2026-05-12
- Context: Shortcut command feedback toast displayed in bottom-right corner.
- Error signature: The `draw_transient_toast()` function rendered the toast message with no explicit width constraint, causing egui to wrap text at a very narrow width. Short messages like "Sent: /prepare-fix-plan" were broken across multiple short lines.
- Symptoms/Impact:
  1. After pressing a terminal shortcut (e.g., F6, F7), the bottom-right toast wrapped the command text into multiple short lines.
  2. The toast looked cramped and unreadable for even moderately long commands.
- Root cause:
  - `draw_transient_toast()` used a plain `ui.label()` inside an `egui::Area` without setting any width bounds, so the label wrapped at whatever small width the area's default layout provided.
- Resolution:
  - Added `TRANSIENT_TOAST_MIN_WIDTH` (420px), `TRANSIENT_TOAST_MAX_WIDTH` (640px), and `TRANSIENT_TOAST_SCREEN_MARGIN` (48px) constants.
  - Added `AdeApp::transient_toast_content_width(screen_width)` helper that clamps the toast width between min/max while respecting screen size.
  - Updated `draw_transient_toast()` to call `ui.set_width(toast_width)` and use `Label::new(...).wrap()` so the label fills the available width and wraps only when the message genuinely exceeds it.
- Prevent recurrence:
  - Added regression tests for `transient_toast_content_width`:
    - `transient_toast_content_width_uses_max_on_wide_screen`
    - `transient_toast_content_width_caps_at_screen_on_narrow_screen`
    - `transient_toast_content_width_never_exceeds_screen`
    - `transient_toast_content_width_scales_between_min_and_max`
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12

---

#### Worktree rows in Source Control panel lacked an icon {#worktree-row-missing-icon}
- Date: 2026-05-12
- Context: Source Control panel “Worktrees” section rendered worktree rows as plain text without a leading icon, making them hard to distinguish from branch labels and other metadata.
- Error signature: User reported “worktree deki simge görülmüyor” — the worktree icon was not visible.
- Symptoms/Impact:
  1. Worktree rows appeared as plain text with no visual affordance (e.g., main ● looked like a tiny square or invisible dot depending on font fallback).
  2. The row looked inconsistent with the rest of the Source Control panel, where file rows use an icon (CHECK_CIRCLE / CLOCK) and the branch line uses GIT_BRANCH.
- Root cause:
  - draw_clickable_sidebar_text_row() was used for worktree rows, which paints only text without any icon slot.
- Resolution:
  - Added draw_source_control_worktree_row() helper that reserves a left-side icon area, paints icons::GIT_BRANCH in TEXT_MUTED, and then draws the branch label + optional current-worktree marker in TEXT_PRIMARY.
  - Replaced the worktree loop’s draw_clickable_sidebar_text_row() call with the new helper, preserving click behavior (Add to Mergen for unregistered worktrees) and hover cursor.
- Prevent recurrence:
  - Added regression test source_control_worktree_row_uses_full_available_width to ensure the new helper respects sidebar width and icon offset.
  - AGENTS.md guideline: actionable sidebar rows that represent named resources (branches, worktrees, files) should include a contextual icon for visual consistency.
- Files/Commands touched: src/app.rs, AGENTS.md, KNOWN_ISSUES.md, cargo test
- References: User request 2026-05-12
