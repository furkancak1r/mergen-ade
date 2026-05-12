
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

---

#### Smart Input plain Tab did not forward to terminal {#smart-input-tab-passthrough}
- Date: 2026-05-12
- Context: Smart Input footer draft/edit field keyboard focus.
- Error signature: User reported: "smart input alanında tab a basınca terminale de tab göndersin" — pressing Tab in Smart Input should also send Tab to the terminal.
- Symptoms/Impact:
  1. When typing in Smart Input draft or editing a queued task, pressing `Tab` did nothing useful — egui consumed it for focus traversal, moving focus away from the Smart Input field.
  2. Terminal applications (e.g., interactive prompts, editors, shell completion) that expect `Tab` input could not receive it while the user was interacting with Smart Input.
- Root cause:
  - Smart Input focus marks `ui_owns_keyboard` as true, which blocks `route_active_terminal_input()` from sending any input to the terminal.
  - Plain `Tab` events were falling through to egui's default focus traversal behavior because there was no special handling for them when Smart Input owned focus.
- Resolution:
  - Added a dedicated plain-Tab passthrough block in `raw_input_hook()` inside the `!capture_keyboard` branch.
  - When `focused_smart_input_submit_request()` returns a Draft or Edit request, plain `Tab` (no modifiers) is intercepted: removed from the UI event stream, and `\t` is sent directly to the terminal runtime that owns the focused Smart Input field.
  - `Shift+Tab` remains blocked by the existing `partition_blocked_ui_reverse_focus_traversal_events` path to prevent focus from leaving the Smart Input field.
- Prevent recurrence:
  - Added regression tests:
    - `smart_input_tab_passthrough_sends_tab_to_terminal_when_draft_focused`: verifies `\t` is sent to terminal when draft has focus.
    - `smart_input_tab_passthrough_sends_tab_to_terminal_when_edit_focused`: verifies `\t` is sent during task edit.
    - `smart_input_tab_passthrough_targets_correct_terminal_when_not_active`: verifies Tab goes to the Smart Input owner terminal even when it's not the active terminal.
    - `smart_input_shift_tab_not_passthrough_stays_blocked_for_ui`: verifies Shift+Tab is not forwarded.
- Files/Commands touched: `src/app.rs` (`raw_input_hook`), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12: "smart input alanında tab a basınca terminale de tab göndersin"

---

#### Browser password prompt repeated for every new terminal {#browser-password-repeat-terminal}
- Date: 2026-05-12
- Context: Embedded browser (WebView2) password/autofill persistence across terminals in the same project.
- Error signature: User reported: "browserda parola hatırlatma çalışmıyor doğru düzgün her yeni terminalde sekmede tekrar soruyor tekrar kaydet çıkıyor hatırlamıyor" — browser does not remember passwords; every new terminal/tab prompts to save the password again.
- Symptoms/Impact:
  1. Logging into a site in a terminal-scoped browser saved the password for that terminal only.
  2. Opening a new terminal in the same project created a new WebView2 profile folder (`webview2/projects/{pid}/terminals/{tid}/`), so the password manager database, cookies, and localStorage were empty again.
  3. WebView2 repeatedly showed the "Save password?" prompt because each profile had no prior credential record.
- Root cause:
  - `create_browser_instance_for_scope()` used `browser_user_data_dir_path_for_terminal(project_id, terminal_id)` for `BrowserScopeKey::Terminal`, creating a separate WebView2 user data folder per terminal.
  - WebView2 stores passwords, cookies, and autofill data inside the user data folder; separate folders mean separate credential stores.
- Resolution:
  - Changed `create_browser_instance_for_scope()` to use `browser_user_data_dir_path(project_id)` for both `BrowserScopeKey::Project` and `BrowserScopeKey::Terminal`, so all terminals in the same project share one WebView2 profile.
  - Terminal-scoped isolation for tabs, design inspect state, and video recordings is preserved via `BrowserScopeKey::Terminal` keyed state maps; only the underlying WebView2 user data folder is shared.
- Prevent recurrence:
  - Added regression tests:
    - `terminal_scoped_browser_uses_project_profile_folder`: verifies terminal scope uses the project profile path.
    - `same_project_terminals_share_browser_profile_folder`: verifies two terminals in the same project use the same profile path.
    - `different_projects_use_different_browser_profile_folders`: verifies cross-project profile separation remains.
- Files/Commands touched: `src/app.rs` (`create_browser_instance_for_scope`), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12: "browserda parola hatırlatma çalışmıyor doğru düzgün her yeni terminalde sekmede tekrar soruyor tekrar kaydet çıkıyor hatırlamıyor"

