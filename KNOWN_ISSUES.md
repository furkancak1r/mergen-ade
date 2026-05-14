  

---

#### OpenCode question prompts required manual terminal TUI interaction {#opencode-question-terminal-fallback}
- Date: 2026-05-14
- Context: When OpenCode asked a question (e.g., model selection, confirmation), Mergen only detected the `Permission` attention state. Users had to read the question text in the terminal TUI and type answers directly into the PTY, bypassing Smart Input entirely.
- Error signature: `question.asked` events from OpenCode were not captured; Mergen had no structured question metadata (text, options, custom flag). Smart Input auto-dispatch did not pause for questions, so queued tasks could race against the pending question.
- Symptoms/Impact:
  1. Users had to focus the terminal and interact with OpenCode's TUI to answer questions.
  2. Multi-line or option-based questions were hard to answer correctly through raw PTY input.
  3. Smart Input queued tasks could auto-dispatch while a question was still pending, causing unexpected behavior.
- Root cause: The OpenCode plugin (`mergen-opencode-status.js`) only forwarded status strings (`permission`), not the full event payload. Mergen had no question model, no answer bridge, and no UI for rendering questions.
- Resolution:
  - Extended the plugin to forward the complete `question.asked` event properties (header, question, options, multiple, custom, request_id, session_id) in the hook POST body.
  - Added `GET /answer` endpoint to the hook service so the plugin can poll for answers every 500ms.
  - Plugin stores pending questions and calls `client.question.reply({ requestID, answers })` or `client.question.reject(requestID)` when an answer is retrieved.
  - Added `OpenCodeQuestionInfo`, `OpenCodeQuestionOption`, and question UI state to `TerminalEntry`.
  - Smart Input footer now renders a question card with selectable labels, custom input, Submit, and Reject buttons.
  - `smart_input_auto_dispatch_ready` returns `false` while a question is pending.
  - Question state is cleared automatically when OpenCode transitions to `Working` or `Idle`, or when the terminal exits.
  - Added regression tests:
    - `hook_service_preserves_question_payload_in_raw_json`
    - `hook_service_answer_endpoint_returns_stored_answer`
    - `hook_service_answer_endpoint_returns_204_when_empty`
- Prevent recurrence:
  - Added guidelines to AGENTS.md OpenCode Smart Input section documenting the question bridge flow.
  - `cargo test` passes.
- Files/Commands touched: `src/app.rs`, `src/opencode_hook_service.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-14

---

#### Terminal Manager project list lacked scroll and overflowed the sidebar {#terminal-manager-no-scroll}
- Date: 2026-05-14
- Context: User reported that the Terminal Manager list had no scroll and overflowed the left sidebar, making it impossible to view projects/terminals that fell off-screen.
- Error signature: `draw_terminal_manager_contents` rendered all project headers, worktree rows, and terminal rows directly into the sidebar panel without a `ScrollArea`, so tall lists simply extended past the bottom of the panel.
- Symptoms/Impact:
  1. With many projects or open terminals, the bottom of the Terminal Manager was unreachable.
  2. Mouse wheel events had no effect because there was no scroll container.
- Root cause: The content list was not wrapped in a `ScrollArea`, unlike Directory, Source Control, and Input History tabs.
- Resolution:
  - Wrapped the project/worktree/terminal loop in `draw_terminal_manager_contents` inside `egui::ScrollArea::vertical()` with `.id_salt("terminal-manager-scroll")`, `.max_height(ui.available_height())`, and `.auto_shrink([false, false])`.
  - Filter tabs and the hide-inactive toggle remain visible above the scroll area.
  - Existing popup positioning (history popup, launcher menu, etc.) is unaffected because `row_rect` coordinates inside the `ScrollArea` remain screen-space.
- Prevent recurrence:
  - Added guideline to AGENTS.md Terminal Manager section.
  - `cargo test` passes.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-14

---

#### Check-list panel made permanently visible and stacked left of Browser panel {#checklist-always-visible}
- Date: 2026-05-14
- Context: User requested that the Check-list panel be always visible and that the right-side panel order be Terminal → Check-list → Browser.
- Error signature: Previously the Check-list and Browser panels were mutually exclusive; opening one closed the other. The Check-list also auto-collapsed when empty or when the last item was removed.
- Symptoms/Impact:
  1. Users could not keep the Check-list open while browsing.
  2. The Check-list disappeared when the last checklist item was unchecked.
  3. Startup logic collapsed the Check-list if no items existed.
- Root cause:
  - Historical design treated Check-list as a transient right panel sharing space with Browser.
- Resolution:
  - Removed all mutual-exclusivity logic: Browser opening no longer closes Check-list, and terminal activation no longer closes Check-list.
  - Removed auto-collapse on empty checklist in `draw_checklist_panel`, Terminal Manager history popup, and `recover_config_state`.
  - `draw_checklist_panel` now always renders (no early return on `checklist_panel_expanded == false`).
  - UI render order changed: `draw_browser_panel` is called before `draw_checklist_panel`, producing the visual order Terminal | Check-list | Browser.
  - `main_area_size_from_chrome` updated to subtract both `checklist_rect` and `browser_rect` widths.
  - Activity rail Check-list icon is now a pinned indicator (always active, no toggle behavior).
  - `UiConfig::default()` sets `checklist_panel_expanded: true` so new installs start with the panel visible.
- Prevent recurrence:
  - Regression tests updated:
    - `browser_panel_and_checklist_can_coexist`
    - `recover_config_keeps_checklist_panel_even_when_empty`
    - `checklist_panel_remains_open_when_last_item_removed`
    - `main_area_size_from_chrome_accounts_for_both_right_panels`
- Files/Commands touched: `src/app.rs`, `src/models.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-14

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

#### Smart Input scroll exposed blank leading rows above OpenCode output {#smart-input-opencode-leading-blank-scroll}
- Date: 2026-05-14
- Context: User reported that scrolling up in an OpenCode terminal with Smart Input visible reveals a large black blank area at the top where nothing is rendered.
- Error signature: OpenCode clears the display (`ED2`) during TUI redraws, leaving blank rows in scrollback history. When Smart Input reduces the terminal viewport height and the user scrolls up past the real content, these blank rows become visible.
- Symptoms/Impact:
  1. Scrolling up shows a black empty strip above the actual OpenCode output.
  2. The blank area is fully selectable but contains no text, making it look like a rendering bug.
- Root cause:
  - `snapshot_from_terminal` preserves all scrollback rows, including blank rows created by OpenCode display clears.
  - `opencode_manual_scroll_detached` disables `stick_to_bottom`, allowing the user to scroll into the blank prefix.
- Resolution:
  - Added `terminal_leading_blank_rows` helper that counts empty rows at the start of the snapshot.
  - After `ScrollArea` processes each frame for an OpenCode terminal with detached scroll, clamp the scroll offset to `leading_blank_rows * line_height` so the user cannot scroll above the first real content row.
- Prevent recurrence:
  - Added regression tests:
    - `opencode_scroll_offset_clamps_past_leading_blank_rows` — counts blank prefix correctly
    - `non_opencode_scroll_does_not_clamp_leading_blanks` — ensures clamping does not affect other terminals
    - `scroll_clamp_noops_when_content_not_scrollable` — short / all-blank snapshots are handled safely
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`
- References: User request 2026-05-14

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
- References: User request 2026-05-14

---

#### Smart Input image paste path was silently consumed and never reached TextEdit {#smart-input-image-paste-consumed}
- Date: 2026-05-14
- Context: User reported that after `Win+Shift+S` screenshot, pressing `Ctrl+V` in the Smart Input draft did not insert the image path as text, even though the attachment chip appeared.
- Error signature: `raw_input_hook` contained two separate image-paste handling blocks. The first block added the attachment chip and synthesized an `Event::Paste(path)`, but a later block intercepted `Event::Paste(_)` and consumed it whenever `clipboard_image_path()` returned `Some`, preventing egui's `TextEdit` from receiving the path text.
- Symptoms/Impact:
  1. Smart Input draft field stayed empty after pasting a screenshot, while the attachment chip showed.
  2. The queued task only sent the image attachment, not the path text, so OpenCode could not see the path inline.
  3. Explorer-copied image files (CF_HDROP) were also affected when the second block re-read the clipboard and found the same path.
- Root cause:
  - A second `Event::Paste` interception block (labeled "Smart Input image paste: ...") ran after the primary synthesize block. It used `filter` to drop the paste event when any image was found, assuming raw image bytes were being pasted. This was wrong because the synthesized event already carried the saved path string, which should go to the TextEdit.
- Resolution:
  - Removed the second interception block entirely. Attachment creation and `Event::Paste(path)` synthesis now live in a single place in `raw_input_hook`, so the paste event survives to the TextEdit.
  - Verified that `clipboard_image_path()` already handles Win+Shift+S bitmaps via `arboard.get_image()` → `save_clipboard_image()` → `Pictures/Screenshots/Mergen_clipboard_*.png`.
- Prevent recurrence:
  - Existing tests cover synthesized paste events (`smart_input_synthesizes_image_paste_on_primary_paste_key`, `smart_input_falls_back_to_normal_key_when_no_clipboard_image`).
  - Full `raw_input_hook` integration is covered by focus/navigation tests that exercise the same event pipeline.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14


---

#### Transient toast notification was oversized for short messages {#transient-toast-oversized}
- Date: 2026-05-14
- Context: User reported that the bottom-right transient toast notification (e.g., "Sent: /review-guard" after a shortcut) was too wide, making short status messages look empty inside a large box.
- Error signature: `TRANSIENT_TOAST_MIN_WIDTH` was 420 px, so even a 19-character message like "Sent: /review-guard" rendered in a 420+ px wide toast.
- Symptoms/Impact:
  1. Short status feedback messages (shortcuts, copy feedback, saved-message sends) appeared in a wide empty box.
  2. The toast visually dominated the bottom-right corner for no content reason.
- Root cause:
  - `transient_toast_content_width(screen_width)` ignored the actual message text length and only used screen width minus a fixed margin, clamped between 420 and 640.
- Resolution:
  - Lowered `TRANSIENT_TOAST_MIN_WIDTH` from 420.0 to 140.0.
  - Introduced `TRANSIENT_TOAST_TEXT_EXTRA_PADDING` (32 px) to account for inner margins and breathing room.
  - Changed `transient_toast_content_width` to accept the egui `Context` and the message string, measure the text width via `fonts.layout()`, and return `text_width + padding` clamped between min/max and capped to available screen space.
  - Updated `draw_transient_toast` to pass the message into the width helper.
- Prevent recurrence:
  - Regression tests:
    - `transient_toast_content_width_is_small_for_short_text_on_wide_screen` — asserts width stays below max and above min for short messages.
    - `transient_toast_content_width_uses_max_for_long_text_on_wide_screen` — asserts long text still hits the 640 px cap.
    - `transient_toast_content_width_caps_at_screen_on_narrow_screen` — asserts viewport overflow protection.
    - `transient_toast_content_width_never_exceeds_screen` — asserts small-screen clamp.
    - `transient_toast_content_width_scales_between_min_and_max_for_medium_text` — asserts medium text scales dynamically.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Terminal shortcuts for OpenCode slash commands had unreliable timing {#terminal-shortcut-slash-timing}
- Date: 2026-05-14
- Context: User reported that terminal shortcuts sending OpenCode slash commands (e.g., F7 `/review-guard`, F11 `/implement-plan`) were unreliable: sometimes the command was not selected from the slash-menu or not submitted, while other times it worked.
- Error signature: Terminal shortcuts used a single delayed confirmation Enter (1200ms) after an immediate Enter. OpenCode's TUI slash-menu sometimes needs the first Enter to populate the command list, a second Enter to select the command, and a third Enter to submit it. With only two total Enters, the race between paste arrival and TUI readiness caused intermittent failures.
- Symptoms/Impact:
  1. F7 `/review-guard` frequently failed to execute in OpenCode.
  2. F11 `/implement-plan` sometimes worked and sometimes didn't.
  3. Rapid successive shortcut presses could leak stale delayed Enters from a previous shortcut into the new command state.
- Root cause:
  - `execute_terminal_shortcut` treated all shortcuts the same: immediate Enter + one delayed Enter after 1200ms.
  - Slash-prefixed commands need more robust confirmation because OpenCode's autocomplete is asynchronous and bracketed-paste insertion is not instantly reflected in the TUI input state.
  - Smart Input already used a stronger strategy (immediate Enter + two delayed Enters at 600ms intervals) but terminal shortcuts did not.
- Resolution:
  - Changed `execute_terminal_shortcut` to detect slash-prefixed commands (`command.trim_start().starts_with('/')`).
  - For slash commands: clear stale `pending_second_enter` entries for the target terminal, then schedule **two** confirmation Enters at `SHORTCUT_SECOND_ENTER_DELAY_MS` (600ms) intervals.
  - For non-slash commands: keep the existing single delayed Enter at `TERMINAL_SHORTCUT_SECOND_ENTER_DELAY_MS` (1200ms) to avoid changing behavior for plain shell commands.
  - This aligns terminal shortcut confirmation with the already-proven Smart Input strategy.
- Prevent recurrence:
  - Regression tests updated:
    - `handle_shortcuts_executes_terminal_shortcut_from_config` — asserts 2 pending delayed Enters for slash commands.
    - `handle_shortcuts_uses_bracketed_paste_for_slash_shortcut` — asserts 2 pending delayed Enters.
    - `handle_shortcuts_sends_immediate_enter_plus_delayed_confirmation` — processes both delayed Enters and asserts total output `\r\r\r`.
    - `handle_shortcuts_clears_stale_delayed_enters_for_slash_shortcut` — simulates a stale pending Enter and verifies it is replaced by 2 fresh ones.
    - `handle_shortcuts_non_slash_keeps_single_delayed_confirmation` — verifies non-slash shortcuts still get only 1 delayed Enter.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Input history popup and Check-list panel overflowed with long inputs {#long-input-overflow}
- Date: 2026-05-14
- Context: User reported that when terminal input is long, the "Show input history" popup and Check-list panel display everything at once, causing the UI to overflow the screen.
- Error signature: The popup had no scroll and no per-message height limit, so a single 2000-character input stretched the popup beyond the viewport. The Check-list panel already had an outer scroll, but one long item could consume the entire visible panel height.
- Symptoms/Impact:
  1. The "Show input history" popup could extend past the bottom of the screen.
  2. A single long checklist item could push all other items out of view.
  3. Multi-line inputs made rows extremely tall, wasting space.
- Root cause:
  - Popup message labels used `.wrap()` without any height limit.
  - Checklist items used `.wrap()` without any height limit.
  - Neither the popup nor individual items had scroll containment.
- Resolution:
  - Added `draw_clamped_scrollable_label()` helper that wraps a label in `ScrollArea::vertical().max_height(...)` so long text scrolls internally instead of expanding the container.
  - Added constants: `TERMINAL_HISTORY_POPUP_MAX_HEIGHT` (400 px), `TERMINAL_HISTORY_MESSAGE_MAX_HEIGHT` (120 px), `CHECKLIST_MESSAGE_MAX_HEIGHT` (120 px).
  - Wrapped the popup message list in an outer `ScrollArea` with `TERMINAL_HISTORY_POPUP_MAX_HEIGHT`.
  - Clamped the popup's vertical position to the viewport so it never opens below the screen edge.
  - Changed popup and checklist item layouts from `ui.horizontal()` to `ui.horizontal_top()` so checkboxes stay at the top of tall rows.
  - Applied `draw_clamped_scrollable_label()` to both the popup per-message labels and the Check-list per-item labels.
- Prevent recurrence:
  - Regression tests:
    - `draw_clamped_scrollable_label_renders_without_crash` — short and long text both render safely
    - `terminal_history_popup_renders_long_inputs_without_crash` — 2000-char input + 10-line input do not panic
    - `checklist_panel_renders_long_items_without_crash` — 2000-char checklist item renders safely
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Worktree and background terminal paths used Windows verbatim `\\?\` prefix causing UNC errors {#worktree-verbatim-path-cwd}
- Date: 2026-05-14
- Context: User reported that after creating a git worktree, background terminals opened with a `\\?\C:\...` path. `npm run` (which delegates to `cmd.exe`) treated this as a UNC path, defaulted to `C:\Windows`, and could not find `server/server.ts`.
- Error signature: Terminal prompt showed `PS Microsoft.PowerShell.Core\FileSystem::\\?\C:\...`. Running `npm run dev:server` produced `UNC paths are not supported. Defaulting to Windows directory.` and searched for scripts under `C:\Windows` instead of the worktree.
- Symptoms/Impact:
  1. Background terminals in worktrees had broken working directories.
  2. Copy Path for worktrees pasted the `\\?\` prefix into other tools.
  3. Foreground terminals were also affected because the same `project.path` was passed to PTY `spawn_command`.
- Root cause:
  - `run_git_worktree_add()` in `src/app.rs` called `resolved.canonicalize()` after `git worktree add`. On Windows, `std::fs::canonicalize()` returns verbatim/extended-length paths (`\\?\C:\...`).
  - This verbatim path was stored directly into `ProjectRecord.path` and later used as `command.cwd(working_directory)` in `TerminalRuntime::spawn()`.
  - `cmd.exe` and some other Windows tools do not handle `\\?\` paths as working directories, treating them as UNC and falling back to `C:\Windows`.
- Resolution:
  - Added `src/path_utils.rs` with `normalize_windows_verbatim_path_for_shell()` which strips `\\?\` from disk paths and converts `\\?\UNC\server\share` to standard UNC.
  - Applied normalization in `run_git_worktree_add()` so newly created worktrees never store verbatim paths.
  - Applied normalization in `TerminalRuntime::spawn()` before `command.cwd()` so any stored verbatim path is sanitized at spawn time.
  - Applied normalization in `normalize_config_for_current_platform()` and `recover_project_records()` so existing persisted config records are cleaned up on startup.
  - Applied normalization in Terminal Manager worktree Copy Path so clipboard receives a clean path.
- Prevent recurrence:
  - Regression tests in `src/path_utils.rs`:
    - `verbatim_disk_path_stripped`
    - `verbatim_unc_path_converted`
    - `normal_disk_path_unchanged`
    - `normal_unc_path_unchanged`
    - `non_verbatim_prefix_unchanged`
    - `non_windows_platform_no_op`
- Files/Commands touched: `src/path_utils.rs`, `src/main.rs`, `src/app.rs`, `src/terminal.rs`, `src/config.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Smart Input draft textarea resize grip did not work due to TextEdit overlap {#smart-input-draft-grip-overlap}
- Date: 2026-05-14
- Context: User reported that the Smart Input metin alanı (draft text area) did not grow or shrink when dragging the bottom-right resize grip.
- Error signature: The bottom-right diagonal grip was drawn as a floating overlay using `ui.interact` inside the same `ui.horizontal` that contained the `TextEdit`. Because `TextEdit` was added first in the layout and occupied the same rect, it consumed all pointer events, making the grip un-draggable.
- Symptoms/Impact:
  1. Dragging the grip had no visible effect on the text area size.
  2. The grip only resized an internal `draft_user_height` which was clamped to `available_h` (remaining footer height), so even if interaction worked, the text area could not grow beyond the existing footer.
  3. The `draft_user_height` state could get stuck at a small value, causing the text area to remain tiny even after other UI changes.
- Root cause:
  - The grip was not allocated as a real egui layout widget; it was an invisible interaction rect overlaid on the `TextEdit`.
  - The grip updated `draft_user_height` (local text area height) instead of the overall footer height (`user_height`), so the footer did not expand to make room for a larger draft.
- Resolution:
  - Replaced the floating overlay grip with a properly allocated widget inside `ui.with_layout(Layout::bottom_up(Align::Max), |ui| { ... })` placed after the submit button in the draft row. This ensures egui routes pointer events to the grip instead of the `TextEdit`.
  - Changed the grip drag logic to update `SmartInputState::user_height` (overall footer height) and reset `draft_user_height = None` so the draft area auto-fills the expanded footer.
  - Also updated the main footer resize handle (between terminal output and footer) to reset `draft_user_height` on drag.
  - Updated `draw_smart_input_footer` signature to accept `pane_height` and `line_height` so the grip can compute the same `max_footer` clamp used by the main handle.
- Prevent recurrence:
  - Regression tests:
    - `smart_input_footer_user_height_takes_precedence_over_draft_height` — verifies that `user_height` controls the footer regardless of `draft_user_height`
    - `smart_input_footer_grip_drag_respects_max_and_min` — verifies drag math clamps to min/max footer bounds
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Smart Input focus did not auto-claim when visible, causing typed text to leak to terminal PTY {#smart-input-auto-focus}
- Date: 2026-05-14
- Context: User reported that when the Smart Input area is open, focus should always be there so that typing goes directly into Smart Input instead of the terminal.
- Error signature: `raw_input_hook` calculated `capture_keyboard` before considering Smart Input visibility, so when Smart Input was visible but the user had not explicitly clicked the draft field, typed text was routed to the terminal PTY instead of the Smart Input `TextEdit`. Similarly, clicking a terminal pane called `surrender_ui_text_focus()`, which cleared Smart Input focus and left the terminal capturing keyboard.
- Symptoms/Impact:
  1. Users typed into the terminal output instead of the Smart Input draft when the Smart Input footer was visible.
  2. Clicking the terminal output or switching terminals removed Smart Input focus, requiring an explicit click on the draft to resume typing there.
  3. Starting or finishing a queued-task edit left focus in an ambiguous state.
- Root cause:
  - There was no centralized helper to restore Smart Input focus. Focus management was ad-hoc: `surrender_ui_text_focus()` cleared Smart Input IDs, but nothing re-requested them when the target terminal still showed Smart Input.
  - `draw_terminal_pane` only surrendered focus on click without restoring it.
  - `set_active_terminal` surrendered all UI focus unconditionally.
- Resolution:
  - Added `AdeApp::ensure_smart_input_focus()` helper with strict guards: only runs when no Smart Input field is already focused, no modal/popup is open, and no other UI text input owns focus.
  - Called `ensure_smart_input_focus()` at three critical points:
    1. **Start of `raw_input_hook()`** — before event routing, so the first keystroke is directed to the Smart Input `TextEdit` rather than the terminal.
    2. **Inside `set_active_terminal()`** — after `surrender_ui_text_focus()`, so switching to a terminal with visible Smart Input immediately restores draft focus.
    3. **After `draw_main_area()`** in `update()` — safety net every frame.
  - In `draw_smart_input_footer()`, after queue edit actions (`edit` / `save` / `cancel` / `delete`), restored focus to the edit input or draft input respectively.
  - In `handle_smart_input_pane_action()`, after `send_draft_now` or `send_task_now`, restored focus to the draft input so the user can keep typing.
- Prevent recurrence:
  - Regression tests:
    - `smart_input_ensure_focus_requests_draft_when_visible` — unfocused visible Smart Input gets draft focus
    - `smart_input_ensure_focus_does_not_steal_from_directory_search` — respects existing non-Smart UI focus
    - `smart_input_ensure_focus_does_not_run_when_settings_popup_open` — blocked by modal
    - `set_active_terminal_restores_smart_input_focus_when_visible` — activation restores focus
    - `smart_input_ensure_focus_preserves_existing_edit_focus` — does not steal from edit field
    - `raw_input_hook_auto_focuses_smart_input_draft` — first keystroke claims focus and text event survives for TextEdit
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Check-list panel lacked a way to copy all items for a project {#checklist-copy-all-missing}
- Date: 2026-05-14
- Context: User requested a copy button next to the project name in the Check-list panel that copies all checklist items for that project at once.
- Error signature: The Check-list panel only supported copying individual items by clicking them one-by-one. There was no bulk copy action.
- Symptoms/Impact:
  1. Users with many checklist items had to click each one separately to copy them.
  2. No way to capture the entire project's checklist as a single clipboard snippet.
- Root cause:
  - The Check-list panel project header only rendered the project name and item count; no action buttons were provided.
- Resolution:
  - Added `format_checklist_for_clipboard()` helper that joins checklist items with `\n\n` so multi-line entries stay distinct.
  - Added a `styled_icon_button` with `icons::COPY` to the project header row, aligned to the right via `ui.with_layout(Layout::right_to_left(...))`.
  - Clicking the button copies the formatted text and shows a transient toast (`"Copied N checklist items"`).
  - Existing per-item copy and checkbox-to-remove behaviors are unchanged.
- Prevent recurrence:
  - Regression tests:
    - `format_checklist_for_clipboard_joins_items_with_blank_line` — verifies ordering and blank-line separation
    - `format_checklist_for_clipboard_preserves_unicode_and_multiline` — verifies Unicode and multi-line content safety
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Smart Input auto-focus targeted wrong terminal when active had no Smart Input {#smart-input-focus-wrong-terminal}
- Date: 2026-05-14
- Context: Code review found that `ensure_smart_input_focus()` could auto-focus a visible OpenCode terminal's Smart Input even when the active terminal was a different pane without Smart Input.
- Error signature: The helper contained a fallback loop over all visible terminals. In multi-terminal view, clicking a normal terminal still sent subsequent typing to another visible OpenCode Smart Input.
- Symptoms/Impact:
  1. Keyboard input routed to the wrong terminal's Smart Input after activating a plain terminal.
  2. `set_active_terminal()` called the same helper before updating `self.active_terminal`, so switching from OpenCode terminal A to B could briefly restore focus to A's Smart Input.
- Root cause:
  - `ensure_smart_input_focus()` had a fallback loop: "any visible terminal with Smart Input in main area."
  - `set_active_terminal()` called `ensure_smart_input_focus()` while `self.active_terminal` still pointed to the previous terminal.
- Resolution:
  - Removed the visible-terminal fallback from `ensure_smart_input_focus()`; it now only considers the active terminal (or the single visible terminal in single-view mode).
  - Removed the early `ensure_smart_input_focus()` call from `set_active_terminal()`.
  - Added targeted Smart Input focus restoration inline after `self.active_terminal = terminal_id` and in the "same terminal" early-return branch, ensuring focus always lands on the correct terminal.
- Prevent recurrence:
  - Regression tests added:
    - `smart_input_ensure_focus_does_not_focus_other_visible_smart_input_when_active_has_none`
    - `set_active_terminal_focuses_new_terminal_smart_input_not_previous`
    - `set_active_terminal_does_not_focus_other_smart_input_when_target_has_none`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: Code review 2026-05-14

---

#### OpenCode hook service dropped question.asked event kind and failed to parse questions in direct polling {#hook-question-event-kind-loss}
- Date: 2026-05-14
- Context: Code review found three regressions in OpenCode Smart Input question handling.
- Error signatures:
  1. `record_status` in `opencode_hook_service.rs` derived `event_kind` from normalized `status.as_str()` ("permission"), losing the actual event kind ("question.asked") from the hook body.
  2. `poll_opencode_hook_service` in `app.rs` mapped `"permission"` to generic `PermissionAsked` without inspecting `event.raw_json` for question payloads, so `opencode_pending_question` was never populated for hook events.
  3. The `/answer` endpoint used destructive `take_answer` with LIFO (`pop()`) ordering; the plugin called `client.question.reject(answer.request_id)` with a string argument instead of `client.question.reject({ requestID })`, and had no retry mechanism if delivery failed.
- Symptoms/Impact:
  1. Smart Input question card never rendered for `question.asked` events arriving via the hook path.
  2. `smart_input_auto_dispatch_ready()` did not pause dispatch because `opencode_pending_question` was missing.
  3. Answers sent by the user could be lost if plugin delivery failed, and reject calls used an unsupported SDK signature.
- Root cause:
  - `parse_status_request_from_body` did not extract `event.type` from the body.
  - `poll_opencode_hook_service` only matched on `event_kind` string without parsing `raw_json`.
  - The answer queue was a `Vec` with destructive `pop()` and no ack protocol.
- Resolution:
  - Added `event_kind` field to `OpenCodeHookStatusRequest` and parsed it from `event.type` / `type` in the body.
  - Updated `record_status` to accept `event_kind` and enqueue metadata-bearing events even when status is unchanged.
  - Extended `poll_opencode_hook_service` to detect `question.asked` and parse `raw_json` into `opencode_pending_question`.
  - Changed `pending_answers` to `VecDeque` with `peek_answer` + `ack_answer` for non-destructive GET and explicit POST ack.
  - Fixed plugin to use `client.question.reject({ requestID: answer.request_id })`, feature-detect APIs, and call `POST /answer/ack` only after successful delivery.
- Prevent recurrence:
  - Regression tests added:
    - `hook_service_question_event_kind_enqueues_even_when_status_unchanged`
    - `hook_service_answer_peek_returns_stored_answer`
    - `hook_service_answer_fifo_ordering`
    - `parse_opencode_question_from_hook_event_shape`
    - `parse_opencode_question_from_notify_event_shape`
  - Updated AGENTS.md OpenCode Smart Input section with hook event kind and peek+ack invariants.
- Files/Commands touched: `src/opencode_hook_service.rs`, `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: Code review 2026-05-14

---

#### Clamped scrollable labels reserved full cap height for short text {#clamped-label-short-text-height}
- Date: 2026-05-14
- Context: Code review found that `draw_clamped_scrollable_label()` disabled vertical auto-shrink, causing short checklist/history items to consume the full 120px max-height cap.
- Error signature: `auto_shrink([false, false])` meant the inner `ScrollArea` always allocated the full `max_height` even when the wrapped label was only one or two lines tall.
- Symptoms/Impact:
  1. Short checklist items wasted vertical space in the Check-list panel.
  2. Short history messages in the Terminal Manager popup left large blank gaps between rows.
- Root cause:
  - `draw_clamped_scrollable_label()` used `.auto_shrink([false, false])` on its inner `ScrollArea`.
- Resolution:
  - Changed `.auto_shrink([false, false])` to `.auto_shrink([false, true])` so the vertical dimension shrinks to content when the wrapped text is shorter than the cap, while horizontal width continues to fill the available space.
- Prevent recurrence:
  - Regression tests added:
    - `draw_clamped_scrollable_label_short_text_does_not_reserve_max_height`
    - `draw_clamped_scrollable_label_long_text_is_capped`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: Code review 2026-05-14
