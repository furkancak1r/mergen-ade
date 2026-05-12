
---

#### Smart Input shortcut redirect targeted wrong terminal and ignored edit mode {#smart-input-shortcut-wrong-target}
- Date: 2026-05-12
- Context: Terminal command shortcuts (F5/F6/F7/F11) when Smart Input footer is focused.
- Error signature: User reported: "shortcuts smart input açıksa ona etki etmeli kapalı ise normal terminale etki etsin" — shortcuts should affect Smart Input when open, otherwise normal terminal.
- Symptoms/Impact:
  1. When Smart Input draft was focused on terminal A but the active terminal was B, pressing a shortcut wrote the command into terminal B's draft instead of terminal A.
  2. When editing a queued task (task-edit field focused), shortcuts were still written into `draft` instead of `edit_draft`, corrupting the queued task text.
  3. When Smart Input was not focused, shortcuts correctly went to the terminal via `buffered_terminal_command_shortcuts`.
- Root cause:
  - `raw_input_hook` used `active_terminal_accepts_input()` to determine the target terminal for shortcut redirect, which returns the active terminal rather than the terminal whose Smart Input field actually has keyboard focus.
  - The redirect code only handled `terminal.smart_input.draft.push_str()` and had no branch for `SmartInputSubmitRequest::Edit`.
- Resolution:
  - Replaced `smart_input_has_focus()` with `focused_smart_input_submit_request()` which returns the exact `terminal_id` (and `task_id` for edit mode) of the focused Smart Input field.
  - Added a `match` on `SmartInputSubmitRequest::Draft { terminal_id }` and `SmartInputSubmitRequest::Edit { terminal_id, task_id }` so shortcuts write to the correct field.
- Prevent recurrence:
  - Added regression tests:
    - `raw_input_hook_redirects_shortcut_to_smart_input_draft_when_focused`: verifies draft receives command.
    - `raw_input_hook_redirects_shortcut_to_smart_input_edit_draft_when_focused`: verifies `edit_draft` receives command during task edit.
- Files/Commands touched: `src/app.rs` (`raw_input_hook`), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12: "shortcuts smart input açıksa ona etki etmeli kapalı ise normal terminale etki etsin"

---

#### Smart Input drag-drop reorder could not move task to end of queue {#smart-input-reorder-end}
- Date: 2026-05-12
- Context: Smart Input footer queue drag-and-drop reorder.
- Error signature: Dragging a task to the bottom of the queue (after the last row) did not move it to the last position.
- Symptoms/Impact:
  1. Dropping a task below the last row visually showed a drop indicator but the task order did not change.
  2. The `reorder_task_to_index` function clamped `target_index` to `len - 1`, so dropping at index `len` (after the last element) was silently clamped to `len - 1`, which was the same as the current position for the last task, causing a no-op.
- Root cause:
  - `reorder_task_to_index` used `target_index.min(self.tasks.len().saturating_sub(1))` which prevented appending at the end.
  - After removing the dragged task, the vector length decreases by 1, so inserting at the new length (old length - 1) is valid for `Vec::insert` and places the task at the end.
- Resolution:
  - Changed clamp to `target_index.min(self.tasks.len())` so the last-row drop target maps to appending at the end.
- Prevent recurrence:
  - Added regression test `smart_input_reorder_task_to_end`: verifies first task can be moved to the end via `target_index == tasks.len()`.
- Files/Commands touched: `src/app.rs` (`SmartInputState::reorder_task_to_index`), `cargo test`
- References: Internal review 2026-05-12

---

#### Smart Input footer max height slightly shortchanged terminal output {#smart-input-footer-max-height-3-lines}
- Date: 2026-05-12
- Context: Terminal-bottom Smart Input footer resize and max height clamp.
- Error signature: When the footer was sized to its maximum allowed height, the terminal output area could be a few pixels smaller than 3 terminal lines.
- Symptoms/Impact:
  1. In small terminal panes, when the footer expanded to `max_footer`, the remaining output height was `pane_height - header - header_gap - smart_footer_gap - max_footer`.
  2. The old `max_footer` formula did not subtract `SMART_INPUT_FOOTER_GAP` (6.0px), so output height ended up being `3 * line_height - 6.0px`, which could fall just below the `3 * line_height` threshold and prevent terminal resize from allocating 3 lines.
- Root cause:
  - `smart_input_footer_height()` and the resize handle drag handler both computed `max_footer = pane_height - TERMINAL_HEADER_HEIGHT - TERMINAL_HEADER_GAP - line_height * 3.0`, forgetting that the layout also reserves `SMART_INPUT_FOOTER_GAP` between terminal output and the footer.
- Resolution:
  - Updated both `smart_input_footer_height()` and the resize handle drag handler to subtract `SMART_INPUT_FOOTER_GAP` from `max_footer`:
    `max_footer = pane_height - TERMINAL_HEADER_HEIGHT - TERMINAL_HEADER_GAP - SMART_INPUT_FOOTER_GAP - line_height * 3.0`.
- Prevent recurrence:
  - Added regression test `smart_input_footer_preserves_three_terminal_lines_at_max_height`: verifies `output_height >= line_height * 3.0` when footer is at maximum.
- Files/Commands touched: `src/app.rs` (`smart_input_footer_height`, resize handle block), `cargo test`
- References: Internal review 2026-05-12

