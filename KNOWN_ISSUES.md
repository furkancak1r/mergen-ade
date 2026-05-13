
---

#### Create worktree popup did not show existing unregistered worktrees {#create-worktree-popup-missing-existing}
- Date: 2026-05-13
- Context: User reported that git worktrees already existing on disk but not registered in Mergen were invisible in the Create Worktree popup. The popup only supported creating brand-new worktrees.
- Error signature: Existing worktrees appeared in Source Control panel but the Create Worktree modal had no way to add them to Mergen without manually clicking in Source Control.
- Symptoms/Impact:
  1. Users had to switch to Source Control panel to add existing worktrees.
  2. The Create Worktree button in Terminal Manager only created new worktrees, missing the "add existing" use case.
- Root cause: `draw_create_worktree_popup()` only rendered a "new worktree" form. There was no discovery or listing of `git worktree list` entries that were already on disk but absent from `self.projects`.
- Resolution:
  - Added `discover_existing_worktrees_for_popup()` helper that runs `crate::worktree::discover_worktrees()` on the root repo and filters out already-registered paths via canonical comparison.
  - Popup now shows a scrollable "Existing worktrees not in Mergen" section with branch labels and `Add to Mergen` buttons.
  - One-click addition calls `add_project_with_worktree()` with `is_worktree: true` and inherits saved messages from the root project.
  - The popup remains open after adding so multiple worktrees can be registered in sequence.
- Prevent recurrence:
  - Added the behavior to AGENTS.md Git Worktree Integration Guidelines.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-13

---

#### Smart Input caused OpenCode terminal black areas, scroll lock, and mid-viewport jumps {#smart-input-opencode-scroll-issues}
- Date: 2026-05-12
- Context: User reported that since Smart Input arrived, OpenCode terminal view broke: "bazen yukarısı siyah oluyor hiçbir şey göstermiyor, bazen scroll yapamıyorum, bazen de ortasına atıyor beni en aşağı scroll yapmak zorunda kalıyorum."
- Error signature: Smart Input footer presence caused PTY resize jitter, activation scroll aligned to prompt row instead of bottom, and manual-scroll detach never re-enabled bottom-stick.
- Symptoms/Impact:
  1. **Black top area**: Smart Input footer height changes triggered PTY resize. OpenCode cleared/redrew on resize, but `activation_scroll_align_pending` jumped the viewport to the prompt row (middle of long output) instead of the bottom, showing blank uncleared rows.
  2. **Scroll lock**: `opencode_manual_scroll_detached` was set on any upward wheel event but never cleared when the user scrolled back to the bottom, so `stick_to_bottom` stayed disabled and new output did not auto-scroll.
  3. **Mid-viewport jump**: Switching to an OpenCode terminal always set `activation_scroll_align_pending = true`, causing an explicit scroll offset to the prompt row on the first frame.
- Root cause:
  - `draw_terminal_pane` used raw pixel `output_size` for `TerminalDimensions`, so small footer height changes (even within one cell) jittered `pixel_height` and triggered PTY resize.
  - `set_active_terminal` unconditionally set `activation_scroll_align_pending = true` for ALL terminals, including OpenCode. OpenCode should start with `stick_to_bottom`, not prompt-alignment.
  - `opencode_manual_scroll_detached` was a one-way latch: set on Mergen wheel consume, but never reset when the viewport returned to the bottom.
- Resolution:
  - Quantized PTY resize pixel dimensions to cell boundaries: `pixel_width = cols * char_width`, `pixel_height = lines * line_height`. Small footer changes within one cell no longer jitter the PTY.
  - `set_active_terminal` now skips `activation_scroll_align_pending` for OpenCode terminals (`ai_session.tool == Some(AiCliTool::OpenCode)`).
  - After `ScrollArea` processes each frame, check if the OpenCode viewport is at the bottom (`content_size <= viewport` or `offset >= content - viewport`). If so, clear `opencode_manual_scroll_detached` so `stick_to_bottom` resumes.
- Prevent recurrence:
  - Added regression tests:
    - `terminal_resize_pixel_dimensions_quantized_to_cell_boundaries` — 5px height change within one cell keeps same `lines`
    - `set_active_terminal_skips_activation_scroll_align_for_opencode` — OpenCode activation does not set `activation_scroll_align_pending`
    - `opencode_output_scroll_behavior_sticks_to_bottom_when_not_detached` — verifies `stick_to_bottom` is true when detach is false
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-12

---

#### OpenCode stale Working recovery hid spinner during long-running work {#opencode-stale-working-hid-spinner}
- Date: 2026-05-12
- Context: Code review found that `clear_opencode_stale_working_if_needed()` flipped `Running` to `Attention` after 6 seconds of no hook/notify refresh, even while the OpenCode process was still alive.
- Error signature: Long-running OpenCode turns (e.g., multi-step tool calls) lost their active-work spinner and appeared as if waiting for user input.
- Symptoms/Impact:
  1. The AI badge changed from spinner to Attention/pulse during normal uninterrupted work.
  2. Smart Input After Done could prematurely dispatch the next queued task because the state machine thought the turn was idle.
- Root cause:
  - `running_stale` check (elapsed time >= 6s) was treated as sufficient to clear Working and synthesize Attention, without verifying whether the process had actually exited.
- Resolution:
  - Removed the `running_stale` automatic cleanup path.
  - `clear_opencode_stale_working_if_needed()` now only clears Working when `opencode_process_missing_since` is present and trailing grace has expired.
  - Explicit visible/hook Idle/TurnComplete signals remain the only ways to stop the spinner while the process is alive.
- Prevent recurrence:
  - Added guard that stale Working never clears while process is live.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `cargo test`
- References: Internal review 2026-05-12

---

#### Smart Input attachment-only tasks pasted but did not submit {#smart-input-attachment-only-no-submit}
- Date: 2026-05-12
- Context: Code review found that Smart Input tasks with image attachments and empty text silently pasted the image path but never pressed Enter or marked OpenCode as Working.
- Error signature: Attachment-only Smart Input tasks appeared sent in the UI but stayed in the OpenCode input without being submitted.
- Symptoms/Impact:
  1. Queued attachment-only tasks were removed from the queue but never actually dispatched.
  2. OpenCode remained Idle, so After Done immediately tried to dispatch the next task.
  3. The pasted image path sat in the terminal input until the user manually pressed Enter.
- Root cause:
  - `process_smart_input_queues`, `execute_smart_input_draft_submit`, and `handle_smart_input_pane_action` all had `if !text.trim().is_empty()` guards that skipped the submit path when text was empty, even if attachments were present.
- Resolution:
  - Added `submit_smart_input_attachment_only()` helper that sends a bare Enter, transitions OpenCode to Working/PromptSubmit, schedules confirmation Enters, and bumps layout epoch.
  - Updated all three dispatch paths to call the helper for attachment-only payloads.
- Prevent recurrence:
  - Regression tests cover auto dispatch, Steer Now, and Now-button attachment-only submission.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `cargo test`
- References: Internal review 2026-05-12

---

#### Factory Droid completion notifications used permission filter {#factory-droid-notification-filter}
- Date: 2026-05-12
- Context: Code review found that Factory Droid stop/completion attention events with no explicit reason were mapped to `OsNotificationKind::Permission`.
- Error signature: Disabling "Permission" notifications in Settings also suppressed Factory Droid completion alerts, while "Turn Complete" had no effect on them.
- Symptoms/Impact:
  1. Users who disabled permission alerts lost Factory Droid completion notifications entirely.
  2. The UI toggle for Turn Complete did not gate Factory Droid completion events.
- Root cause:
  - `apply_factory_droid_status` used `None => OsNotificationKind::Permission` for attention events without an explicit `AskUser` or `SpecificationApproval` reason.
- Resolution:
  - Changed `None` reason mapping to `OsNotificationKind::TurnComplete`.
  - `AskUser` and `SpecificationApproval` still map to `Permission`.
- Prevent recurrence:
  - Regression tests assert stop uses turn-complete filter and ask-user uses permission filter.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `cargo test`
- References: Internal review 2026-05-12

---

#### Smart Input delayed-enter test fast-forwarded wrong timestamp {#smart-input-test-wrong-timestamp}
- Date: 2026-05-12
- Context: Code review found that `smart_input_delayed_enters_cleared_between_auto_dispatches` mutated `opencode_last_prompt_submit_at` while the settle guard reads `opencode_prompt_submit_since`.
- Error signature: The test could pass without proving that the second auto-dispatch actually happened, because the settle guard remained blocked by the unmodified `opencode_prompt_submit_since`.
- Symptoms/Impact:
  1. Test gave false confidence that delayed Enters were cleared between dispatches.
  2. A real bug in the settle-guard/dispatch interaction could go undetected.
- Root cause:
  - Test used the wrong field name; `opencode_last_prompt_submit_at` is not checked by `smart_input_auto_dispatch_ready`.
- Resolution:
  - Updated the test to mutate `opencode_prompt_submit_since` instead.
  - Added assertion that the second task is actually dispatched (captured bytes and empty queue).
- Prevent recurrence:
  - Test now fails if the wrong timestamp is mutated.
- Files/Commands touched: `src/app.rs`, `cargo test`
- References: Internal review 2026-05-12

---

#### Smart Input stale Idle dispatched queued tasks after settle guard {#smart-input-stale-idle-auto-dispatch}
- Date: 2026-05-12
- Context: Code review found that stale OpenCode Idle/TurnComplete events could be retained after Smart Input dispatch.
- Error signature: Second queued task dispatches after 300ms even though the previous task may still be running.
- Symptoms/Impact:
  1. Smart Input sends first queued task.
  2. Delayed previous-turn Idle arrives immediately and changes state to Idle.
  3. After settle window expires, Smart Input sends the next queued task incorrectly.
- Root cause:
  - `smart_input_auto_dispatch_ready()` delayed dispatch but did not prevent stale Idle state from being stored.
- Resolution:
  - Added `is_stale_opencode_completion()` helper that suppresses/drops completion signals received during the post-submit settle window.
  - Applied suppression in `apply_opencode_transport_status()`, `apply_opencode_status()`, and `process_terminal_events()` so stale Idle can never overwrite Working.
  - Updated `should_accept_opencode_turn_complete_chunk()` to also reject visible chunks during settle.
- Prevent recurrence:
  - Regression tests verify stale Idle still cannot dispatch after fast-forwarding beyond settle.
  - Regression tests verify visible turn-complete is suppressed during settle even when Hook/Notify Working is not present.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `cargo test`
- References: Internal review 2026-05-12

---

#### Smart Input attachment-only submit left stale terminal input buffers {#smart-input-attachment-only-stale-buffers}
- Date: 2026-05-12
- Context: Code review found that Smart Input attachment-only tasks pasted image paths through the generic terminal paste helper.
- Error signature: Later terminal Enter can use stale image path from `pending_line_for_title` / `pending_input_for_history`.
- Symptoms/Impact:
  1. Attachment-only task sends image path and Enter.
  2. Internal pending input buffers still contain the path.
  3. Later user input/history/title can be polluted by the stale path.
- Root cause:
  - Smart Input attachment delivery used `deliver_pasted_text_to_terminal()`, which appends paste text to terminal pending buffers.
  - `submit_smart_input_attachment_only()` sent Enter but did not clear those buffers.
- Resolution:
  - Added `deliver_smart_input_attachment_to_terminal()` helper that delivers bracketed paste bytes without mutating title/history buffers.
  - Replaced all Smart Input attachment loops to use the new helper.
  - `submit_smart_input_attachment_only()` now clears `pending_line_for_title` and `pending_input_for_history` after sending Enter.
- Prevent recurrence:
  - Regression tests cover attachment-only and text+attachment buffer cleanliness.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `cargo test`
- References: Internal review 2026-05-12

---

#### Terminal Manager foreground launcher menu opened at absurd width {#launcher-menu-absurd-width}
- Date: 2026-05-13
- Context: User reported that clicking the "new terminal" (foreground launcher) button in the Terminal Manager opened a menu that was extremely wide.
- Error signature: `ui.available_width()` inside an egui popup can return a very large or effectively unbounded value, causing the launcher menu rows to stretch far beyond the sidebar.
- Symptoms/Impact:
  1. The foreground launcher dropdown was visually broken, spanning hundreds of pixels.
  2. Long launcher display names or commands made the menu even wider.
- Root cause:
  - `styled_launcher_menu_button` used `ui.available_width().max(0.0)` for each launcher row inside a popup. In egui popups `available_width()` is not constrained by the parent sidebar width.
- Resolution:
  - Introduced `FOREGROUND_LAUNCHER_MENU_WIDTH` (220 px) constant.
  - The popup now calls `ui.set_min_width` / `ui.set_max_width` with this fixed width.
  - Added `FOREGROUND_LAUNCHER_MENU_PADDING_X` (6 px) and `FOREGROUND_LAUNCHER_MENU_PADDING_Y` (6 px) constants for inner padding.
  - Row backgrounds are explicitly shrunk by `FOREGROUND_LAUNCHER_MENU_PADDING_X` on both sides so they never touch the popup border.
  - Added `FOREGROUND_LAUNCHER_ROW_GAP` (4 px) between rows for visual breathing room.
  - Launcher names and commands use `.truncate()` with a hover tooltip for the full text.
- Prevent recurrence:
  - Added regression tests:
    - `foreground_launcher_menu_row_does_not_exceed_fixed_width` — width cap
    - `foreground_launcher_menu_row_is_inset_from_menu_edges` — padding/inset verification
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`
- References: User request 2026-05-13

---

#### Foreground launcher menu minimalized and aligned {#launcher-menu-minimal}
- Date: 2026-05-13
- Context: User asked to "adam gibi hizala ve küçült şunları minimal olsun" for the foreground launcher dropdown.
- Error signature: The menu was too wide (220 px), padding too large (6 px), rows too tall (32 px), and the small `launch_command` label repeated the tool name, cluttering the UI.
- Symptoms/Impact:
  1. Menu felt bulky compared to the rest of the minimal UI.
  2. Duplicate small command text ("codex", "opencode") appeared under every launcher name.
  3. 20 px icons and 32 px rows wasted vertical space.
- Root cause:
  - Original fix only addressed width/padding but kept the large dimensions and duplicate label from the initial implementation.
- Resolution:
  - Reduced `FOREGROUND_LAUNCHER_MENU_WIDTH` from 220.0 to 168.0.
  - Reduced `FOREGROUND_LAUNCHER_MENU_PADDING_X` from 6.0 to 4.0 and `FOREGROUND_LAUNCHER_MENU_PADDING_Y` from 6.0 to 4.0.
  - Reduced `FOREGROUND_LAUNCHER_ROW_GAP` from 4.0 to 2.0.
  - Reduced row height from `CONTROL_ROW_HEIGHT + 4.0` (32 px) to a fixed 24 px.
  - Reduced icon size from 20.0 to 16.0.
  - Removed the small `launch_command` label under `display_name` to eliminate duplicate text.
  - Replaced `ui.vertical_centered` with `ui.vertical` inside the row so the label is left-aligned and properly centered vertically by the outer `Align::Center` layout.
- Prevent recurrence:
  - Regression tests updated to match new dimensions and pointer positions.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`
- References: User request 2026-05-13

---

#### Worktree projects lacked saved messages from root repo {#worktree-saved-messages-missing}
- Date: 2026-05-13
- Context: User reported that saved messages created in the root repo were not available in worktree projects.
- Error signature: Worktree `ProjectRecord`s were created with `saved_messages: Vec::new()`, and Settings add/remove operated only on the selected project, so the Terminal Manager showed empty saved-message menus for worktrees.
- Symptoms/Impact:
  1. Background terminal saved-message button in Terminal Manager showed "No saved messages" for worktrees even when the root repo had snippets.
  2. Adding a message in Settings for a worktree did not propagate to the root or sibling worktrees.
  3. Removing a message from the root did not clean it from worktrees.
- Root cause:
  - `add_project_with_worktree` did not copy `saved_messages` from the root project.
  - Settings Saved Messages section used per-project add/remove without family awareness.
  - No startup backfill existed for existing worktree records loaded from config.
- Resolution:
  - `add_project_with_worktree` now copies the root project's `saved_messages` into new worktree records.
  - Added `worktree_family_root_path`, `add_saved_message_to_family`, `remove_saved_message_from_family`, and `backfill_worktree_saved_messages` helpers.
  - Startup bootstrap calls `backfill_worktree_saved_messages` so existing worktrees receive the union of family messages.
  - Settings add/remove now applies to every project in the same repo family.
- Prevent recurrence:
  - Regression tests cover:
    - `add_project_with_worktree_copies_root_saved_messages`
    - `backfill_worktree_saved_messages_merges_family`
    - `add_saved_message_to_family_avoids_duplicates`
    - `remove_saved_message_from_family_removes_from_all_members`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-13

---

#### Worktree row in Terminal Manager ignored Background filter {#worktree-row-ignored-background-filter}
- Date: 2026-05-13
- Context: User reported that when Terminal Manager filter was set to Background, worktree rows still showed "Open Foreground Launcher" menu instead of a background spawn button.
- Error signature: `draw_terminal_manager_worktree_row` always rendered a foreground launcher menu and hard-coded `TerminalKind::Foreground` when spawning.
- Symptoms/Impact:
  1. Background filter active → clicking worktree action opened a foreground launcher dropdown.
  2. Spawned terminals for worktrees were always Foreground even when user expected Background.
- Root cause:
  - `draw_terminal_manager_worktree_row` did not accept the active `TerminalKind` filter.
  - The call site in `draw_terminal_manager_contents` always passed `TerminalKind::Foreground` to `spawn_terminal_for_project` for worktrees.
- Resolution:
  - Added `action_kind: TerminalKind` parameter to `draw_terminal_manager_worktree_row`.
  - When `action_kind == TerminalKind::Background`, the row renders a single `LIST` icon button ("New Background Terminal") using the same spec as the root project header.
  - The call site now routes worktree spawn through `match visible_kind`, spawning `TerminalKind::Background` with no launcher when the filter is Background.
- Prevent recurrence:
  - Regression tests:
    - `draw_terminal_manager_worktree_row_background_returns_no_launcher` — verifies Background row returns `None` launcher and no auto-click
    - `draw_terminal_manager_worktree_row_foreground_returns_no_launcher_without_interaction` — verifies Foreground row shape
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-13

---

#### Terminal Manager worktree rows lacked source control diff summary {#worktree-diff-summary-missing}
- Date: 2026-05-13
- Context: User reported that worktree rows in Terminal Manager did not show the `+` / `-` source control summary like normal project headers.
- Error signature: `draw_terminal_manager_worktree_row` only painted the worktree name without any diff summary.
- Symptoms/Impact:
  1. Users could not see at a glance whether a worktree had uncommitted changes.
  2. Worktree rows looked inconsistent with root project rows which showed `+N -M`.
- Root cause:
  - `draw_terminal_manager_worktree_row` did not accept or render a `TerminalManagerDiffSummaryModel`.
  - `draw_terminal_manager_contents` did not compute the worktree's diff summary.
- Resolution:
  - Added `diff_summary: &TerminalManagerDiffSummaryModel` parameter to `draw_terminal_manager_worktree_row`.
  - The row now uses `draw_terminal_manager_title_and_diff_summary()` inside the body rect, producing the same `+N -M` label as root projects.
  - The call site computes `wt_diff_summary` from `self.source_control_state.get(&wt_project_id)` before drawing the row.
- Prevent recurrence:
  - Regression tests pass `TerminalManagerDiffSummaryModel::default()` to worktree row calls.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-13

---

#### Smart Input shortcuts typed into draft instead of queuing {#smart-input-shortcut-draft-instead-of-queue}
- Date: 2026-05-13
- Context: User reported that pressing a terminal shortcut (e.g., F6) while Smart Input was focused only typed the command into the draft field and left it there, instead of adding it to the queue.
- Error signature: `raw_input_hook` appended shortcut command text to `smart_input.draft` (or `edit_draft` during edit mode).
- Symptoms/Impact:
  1. The draft field contained unexpected command text.
  2. The command was not queued for After Done dispatch.
  3. Users had to manually submit or delete the typed command.
- Root cause:
  - The shortcut redirect branch in `raw_input_hook` simply called `draft.push_str(&command)`.
- Resolution:
  - Added `SmartInputState::queue_command_task()` helper that pushes a new `SmartInputTask` without mutating the draft.
  - Updated `raw_input_hook` redirect for both Draft and Edit focus to call `queue_command_task()`.
- Prevent recurrence:
  - Regression tests assert that draft/edit_draft remain empty and a queued task is created.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-13

---

#### Smart Input focus blocked Ctrl+Arrow terminal navigation {#smart-input-focus-blocked-ctrl-arrow}
- Date: 2026-05-13
- Context: User reported that Ctrl+Arrow keys for terminal grid/filter navigation did not work while Smart Input draft was focused.
- Error signature: `raw_input_hook` consumed all key events when Smart Input was focused, but only processed command shortcuts and plain Up/Down history navigation. Ctrl+Arrow events were dropped on the floor.
- Symptoms/Impact:
  1. Users could not switch terminals with Ctrl+Left/Right while typing in Smart Input.
  2. Ctrl+Up/Down single-view navigation was also blocked.
- Root cause:
  - The Smart Input focus branch in `raw_input_hook` did not call `partition_terminal_navigation_shortcuts` for the remaining events.
- Resolution:
  - After processing Smart Input history navigation (plain Up/Down), added a check: if Smart Input still has focus, partition navigation shortcuts and buffer them into `buffered_terminal_navigation`.
- Prevent recurrence:
  - Added regression test `raw_input_hook_buffers_ctrl_arrow_for_terminal_navigation_while_smart_input_focused`.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-13

---

#### Smart Input UI overflow and missing interactions {#smart-input-ui-polish}
- Date: 2026-05-13
- Context: User reported multiple Smart Input usability issues: task row action buttons overflowed off-screen, no right-click copy/paste, input height was fixed and small, and buttons used text labels instead of icons.
- Error signature: Task row used `ui.add_space(remaining)` to push buttons right, which overflowed narrow panels. Input height was hard-coded `42.0`. No context menu on TextEdit. Buttons were text-based.
- Symptoms/Impact:
  1. "Now", "Edit", "Del" buttons could render outside the visible row.
  2. Users could not copy selected text or paste clipboard into Smart Input inputs.
  3. Resizing the footer did not enlarge the text input area.
  4. Text buttons consumed extra horizontal space and looked cluttered.
- Root cause:
  - Row layout manually pushed buttons right without reserving fixed space.
  - No `.context_menu()` was attached to the TextEdit responses.
  - Input size used a constant.
  - Buttons used `ui.button(RichText::new("..."))`.
- Resolution:
  - Replaced task row layout: action icons (`ARROW_RIGHT`, `CODE`, `TRASH`) are drawn first on the left, followed by a truncating label and attachment chips.
  - Added `.context_menu()` to both the draft multiline TextEdit and the edit singleline TextEdit, supporting Copy (when selection exists) and Paste.
  - Replaced fixed `42.0` input height with `ui.available_height().max(42.0)` so the multiline area grows when the footer is resized taller.
  - Converted task row buttons, Clear button, Save/Cancel buttons, and the Queue/Send submit button to icon-only (`styled_icon_button`) with tooltips.
- Prevent recurrence:
  - Visual verification via manual test; regression tests cover Smart Input state helpers.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-13

---

#### Smart Input context menu lost selected text after right-click {#smart-input-context-menu-selection}
- Date: 2026-05-13
- Context: Review found that Smart Input Copy/Paste context menus read `TextEditState` after right-click, but `egui::TextEdit` can collapse the selection on secondary click.
- Error signature: `TextEditState::load(...).cursor.char_range()` inside `.context_menu()` after `TextEdit` can collapse selection.
- Symptoms/Impact:
  1. Copy disabled after right-clicking selected Smart Input text.
  2. Paste appends or fails to replace the intended selected text.
  3. Affects both draft and queued-task edit fields.
- Root cause:
  - Smart Input did not preserve pre-click selection like the file editor does.
- Resolution:
  - Added `draft_context_menu_selection_range` / `edit_context_menu_selection_range` to `SmartInputState`.
  - Before rendering a Smart Input `TextEdit`, detect `secondary_pressed` and store the pre-click non-empty selection.
  - Use the stored effective selection for Copy and Paste in the context menu.
  - Restore the stored selection after the menu opens so the highlight remains visible.
  - Clear stored ranges when the menu closes or when draft/edit state resets (enqueue, submit, save, cancel, start-edit).
- Prevent recurrence:
  - Regression tests:
    - `smart_input_draft_context_menu_selection_clears_on_enqueue`
    - `smart_input_edit_context_menu_selection_clears_on_cancel`
    - `smart_input_edit_context_menu_selection_clears_on_start_edit`
    - `smart_input_context_menu_effective_range_prefers_stored`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-13

---


#### Worktree terminal rows lacked extra indentation in Terminal Manager {#worktree-terminal-indent-missing}
- Date: 2026-05-13
- Context: User reported that when a terminal is opened inside a worktree in the Terminal Manager, the terminal row should open slightly more to the right to preserve the BOM-style tree view structure.
- Error signature: draw_terminal_manager_contents drew worktree terminals inside a standard ui.indent() block, making them start at the same horizontal offset as the worktree row itself.
- Symptoms/Impact:
  1. Worktree terminal rows visually aligned with the worktree row, breaking the BOM tree hierarchy.
  2. Root project terminals and worktree terminals looked equally indented, reducing visual clarity.
- Root cause:
  - draw_terminal_manager_contents applied only the default ui.indent() for worktree terminals, with no additional horizontal offset.
- Resolution:
  - Added TERMINAL_MANAGER_WORKTREE_TERMINAL_EXTRA_INDENT constant (10 px).
  - Wrapped the worktree terminal ui.indent() block in an extra ui.scope() that increments ui.spacing_mut().indent by the constant before allocating terminal rows.
  - Root project terminals remain unchanged.
- Prevent recurrence:
  - Regression test worktree_terminal_indent_exceeds_root_terminal asserts that a row allocated under the extra indent starts farther to the right than a row without it.
- Files/Commands touched: src/app.rs, AGENTS.md, KNOWN_ISSUES.md, cargo test, cargo fmt
- References: User request 2026-05-13

---


#### Orphan worktree projects were not auto-removed when deleted externally {#orphan-worktree-not-auto-removed}
- Date: 2026-05-13
- Context: User reported that if a worktree is deleted externally (no longer in source control / git worktree list) but still registered as a project in Mergen, it should be automatically removed along with its terminals.
- Error signature: Mergen kept stale worktree ProjectRecords and their terminals even after the worktree directory was deleted and git stopped tracking it.
- Symptoms/Impact:
  1. Terminal Manager showed ghost worktree rows with no actual directory behind them.
  2. Terminals inside deleted worktrees remained open, wasting resources.
  3. Users had to manually remove orphaned worktrees from Mergen.
- Root cause:
  - process_source_control_events merged source control snapshots but never compared discovered worktrees against registered worktree projects.
  - No periodic or event-driven cleanup existed for orphaned worktrees.
- Resolution:
  - Added cleanup_orphan_worktrees_for_project() helper that runs after each successful source control refresh for root repos.
  - It builds a set of paths from snapshot.worktrees, then scans registered worktrees under that repo.
  - Any worktree missing from the git list AND whose path no longer exists on disk is removed via remove_project(), which closes its terminals and cleans up all associated state.
  - Cleanup is skipped when the source control refresh fails (last_error present) to avoid removing worktrees due to transient git errors.
- Prevent recurrence:
  - Regression test orphan_worktree_removed_when_source_control_refresh_shows_missing verifies that a worktree with a non-existent path and empty worktrees list is automatically removed while the root project and unrelated projects remain.
- Files/Commands touched: src/app.rs, AGENTS.md, KNOWN_ISSUES.md, cargo test, cargo fmt
- References: User request 2026-05-13

---


#### Worktree creation did not copy .env files {#worktree-env-copy-missing}
- Date: 2026-05-13
- Context: User reported that newly created worktrees did not inherit root repo .env files, causing runtime commands like npm run dev to fail because environment variables were missing.
- Error signature: git worktree add only checks out tracked files; .gitignored/untracked .env files are left behind in the root repo.
- Symptoms/Impact:
  1. Users had to manually copy .env files into the new worktree after creation.
  2. Background/foreground terminals spawned in the worktree could not find required environment configuration.
- Root cause:
  - Mergen called git worktree add but performed no post-creation file copying.
- Resolution:
  - Added copy_worktree_env_files() helper that scans the repo root for .env* files and copies them into the newly created worktree path.
  - The helper is called immediately after a successful git worktree add inside run_git_worktree_add().
  - Only root-level .env* files are copied; deeper nested files are intentionally skipped to avoid surprising overwrites in monorepos.
- Prevent recurrence:
  - Regression test copy_worktree_env_files_copies_root_env_files creates a fake repo with .env, .env.local and README.md, runs the helper into a worktree directory, and asserts that .env files arrive while README.md does not.
- Files/Commands touched: src/app.rs, AGENTS.md, KNOWN_ISSUES.md, cargo test, cargo fmt
- References: User request 2026-05-13

---


#### Smart Input queued task action buttons were pushed out of the row {#smart-input-task-buttons-clipped}
- Date: 2026-05-13
- Context: Queued Smart Input rows were changed to place prompt text/attachments on the left and actions on the right.
- Error signature: `ui.add_space(ui.available_width().max(0.0))` was used before rendering Send/Edit/Delete action buttons.
- Symptoms/Impact:
  1. Send/Edit/Delete buttons could be clipped or unreachable in normal finite-width Smart Input footers.
  2. Users could not send, edit, or delete queued tasks directly from the row.
- Root cause:
  - The spacer consumed all remaining horizontal space before the action buttons were allocated, causing them to extend past the visible row boundary.
- Resolution:
  - Reserve fixed action button width (`3.0 * CONTROL_ROW_HEIGHT + 2.0 * item_spacing.x`) before adding the flexible spacer.
  - Buttons remain right-aligned and fully visible within the row.
- Prevent recurrence:
  - Regression test `smart_input_task_row_spacer_reserves_action_buttons` creates a 300 px row with a long label, applies the reservation logic, allocates the three action buttons, and asserts that every button fits inside the row width.
- Files/Commands touched: src/app.rs, AGENTS.md, KNOWN_ISSUES.md, cargo test, cargo fmt
- References: Code review 2026-05-13

---
