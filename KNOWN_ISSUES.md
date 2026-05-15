  

---

#### Smart Input focus was too aggressive and stole input from terminals and popups {#smart-input-focus-stealing}
- Date: 2026-05-14
- Context: User reported that Smart Input was so dominant that typing and paste went to Smart Input even when they clicked the terminal output or opened the Create Worktree popup. Keyboard input should go wherever the user last clicked.
- Error signature: `ensure_smart_input_focus()` checked `smart_input_has_focus(ctx)` first and returned early, so once Smart Input was focused it never surrendered focus even when a modal/popup opened. `set_active_terminal()` unconditionally re-focused Smart Input for both same-terminal re-clicks and terminal switches, ignoring whether the user explicitly wanted to type in the terminal PTY.
- Symptoms/Impact:
  1. Create Worktree popup input fields could not receive keyboard input because Smart Input retained focus in the background.
  2. Clicking on terminal output and typing still sent text to Smart Input instead of the terminal.
  3. Ctrl+V paste went to Smart Input even when the user intended to paste into the terminal.
- Root cause:
  - `ensure_smart_input_focus()` had the `smart_input_has_focus` early-return guard before the popup/modal checks, so opening a popup never caused Smart Input to yield.
  - `set_active_terminal()` always called `request_focus(draft_id)` for both same-terminal re-selections and new terminal activations, with no way to distinguish "user clicked terminal output to type there" from "user switched terminals via keyboard/manager".
- Resolution:
  - Moved modal/popup/context-menu checks to the TOP of `ensure_smart_input_focus()` so they take absolute precedence over the Smart Input keep-focus guard.
  - Added `terminal_output_focus_override: Option<u64>` to `AdeApp` to track when the user explicitly clicks terminal output.
  - `draw_terminal_pane()` now tracks `output_clicked` separately from `pane_clicked` and sets `terminal_output_focus_override` when the output surface is clicked.
  - `set_active_terminal()` respects the override: for same-terminal re-clicks it skips Smart Input re-focus when the override is active; for terminal switches it clears the old override and only focuses Smart Input if there's no override for the new terminal.
  - `ensure_smart_input_focus()` clears the override when Smart Input actually gains focus, and honors the override when deciding whether to auto-focus Smart Input.
  - `close_terminal()` cleans up the override if the closed terminal had it.
  - `surrender_ui_text_focus()` now also surrenders `smart_input_question_custom_input_id`.
  - Create Worktree branch input now has an explicit `CREATE_WORKTREE_BRANCH_INPUT_ID` and receives `request_focus()` immediately when the popup opens from both the Source Control panel and Terminal Manager.
- Prevent recurrence:
  - Added regression tests:
    - `ensure_smart_input_focus_surrenders_smart_input_when_create_worktree_popup_open`
    - `set_active_terminal_respects_output_focus_override`
    - `set_active_terminal_clears_output_override_when_switching_terminals`
    - `close_terminal_clears_output_focus_override`
  - Updated AGENTS.md Smart Input guidelines to document the terminal output click override and popup surrender behavior.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Design Inspect icon changed to pencil and terminal-scope URL bug fixed {#design-inspect-pencil-and-url}
- Date: 2026-05-14
- Context: User requested that the Design Inspect toolbar icon be a pencil instead of an eye. Additionally, Design Inspect element clicks were not being sent to the terminal when using a terminal-scoped browser.
- Error signature: `forward_design_inspect_click_to_terminal` compared the element’s page URL against `project.browser_last_url` for all scopes. For terminal-scoped browsers, the project-level persisted URL could be stale or different, causing valid element selections to be silently dropped.
- Symptoms/Impact:
  1. Browser toolbar showed an eye icon for Design Inspect instead of a pencil.
  2. Selecting a page element in a terminal-scoped browser did not paste the design inspect context into the terminal.
  3. The bug only manifested when the terminal-scoped browser was on a different URL than the project’s last persisted URL.
- Root cause:
  - Stale-URL validation was project-centric (`project.browser_last_url`) and ignored the active tab URL of the terminal-scoped browser instance.
- Resolution:
  - Added `AppIcon::Pencil` with Lucide glyph `"pencil"` and switched the Design Inspect toolbar icon to `icons::PENCIL`.
  - Introduced `current_browser_url_for_scope()` helper that returns the active tab URL for any scope, falling back to `project.browser_last_url` only for project scopes.
  - Replaced the stale-URL check in `forward_design_inspect_click_to_terminal` with the scope-aware helper.
- Prevent recurrence:
  - Added regression tests:
    - `design_inspect_terminal_scope_uses_scope_tab_url_not_project_last_url`
    - `design_inspect_terminal_scope_rejects_stale_scope_tab_url`
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Smart Input draft resize grip drifted to panel edge {#smart-input-grip-drift}
- Date: 2026-05-14
- Context: User reported that the Smart Input metin alanı (draft text area) resize grip at the bottom-right had drifted to the far-right edge of the footer row and no longer sat next to the TextEdit.
- Error signature: `draw_smart_input_footer` placed the grip using `ui.with_layout(Layout::bottom_up(Align::Max), |ui| { ui.allocate_response(...) })` inside the draft row's horizontal layout. In egui, this caused the grip widget to consume the remaining horizontal space and align to its far-right edge—sometimes hundreds of pixels away from the TextEdit.
- Symptoms/Impact:
  1. The diagonal grip icon visually detached from the text area and appeared near the submit button or beyond.
  2. Users expected to grab the grip at the TextEdit corner, but it was nowhere near it.
  3. The interaction area was still small (12 px) but positioned so far right that it could be missed entirely.
- Root cause:
  - `with_layout(Layout::bottom_up(Align::Max))` inside a horizontal row does not anchor a widget to the previous widget; it creates a new right-aligned sub-layout that takes whatever space is left.
- Resolution:
  - Replaced the layout-based grip with explicit rect positioning: compute `grip_rect` directly from `draft_response.rect.right() - grip_size` and `draft_response.rect.bottom() - grip_size`.
  - Use `ui.interact(grip_rect, ui.id().with("draft_resize"), Sense::drag())` so egui routes drag events to the grip without relying on layout allocation semantics.
  - Keep the existing resize behavior (update `user_height`, reset `draft_user_height`) intact.
- Prevent recurrence:
  - Added regression test:
    - `smart_input_footer_grip_is_positioned_at_draft_bottom_right` — renders the footer and asserts the diagonal line segments (grip visual) have X coordinates within the TextEdit area, not at the panel edge.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

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

---

#### Terminal Manager foreground launcher popup background bleed through gaps {#launcher-menu-background-bleed}
- Date: 2026-05-14
- Context: User reported that the Terminal Manager foreground launcher dropdown shows the background UI through gaps/margins inside the popup.
- Error signature: The popup relied on `ui.available_rect_before_wrap()` to paint an opaque backing, but this did not deterministically cover all padding and row-gap areas, causing the Terminal Manager list behind the popup to remain visible.
- Symptoms/Impact:
  1. Background UI (Terminal Manager rows, sidebar) was visible through the popup.
  2. The popup felt incomplete/non-opaque.
- Root cause:
  - `styled_launcher_menu_button` used `ui.available_rect_before_wrap()` at popup open time to paint a `SURFACE_BG` rect. This rect did not guarantee full coverage of the final content area after adding top/bottom padding and row gaps.
- Resolution:
  - Moved the opaque backing into `draw_launcher_menu_contents` where the total popup height is computed deterministically from row count, row height, row gap, and vertical padding.
  - The backing rect is painted at the very beginning with `Rect::from_min_size(ui.cursor().min, vec2(menu_width, total_height))` using `SURFACE_BG`, ensuring every padding pixel and gap is covered.
  - Removed the ad-hoc `available_rect_before_wrap()` backing from `styled_launcher_menu_button`.
- Prevent recurrence:
  - Regression tests updated:
    - `foreground_launcher_menu_popup_has_opaque_backing` now asserts backing width and height cover the full deterministic popup area.
    - Added `foreground_launcher_menu_empty_has_opaque_backing` for the empty-launcher state.
    - Test helper refactored to call the production `draw_launcher_menu_contents` directly instead of duplicating its logic.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Check-list panel hover tooltip misaligned after scroll {#checklist-hover-tooltip-misaligned}
- Date: 2026-05-14
- Context: User reported that when checklist items are long and scrolled, the hover tooltip "Click to copy" appears in the wrong place (shifted down, not next to the visible item).
- Error signature: `draw_clamped_scrollable_label()` returned the inner `Label`'s response, whose `rect` reflected the full unclamped content height rather than the visible ScrollArea viewport. `on_hover_text()` positions the tooltip relative to `response.rect`, so it anchored far below the visible row.
- Symptoms/Impact:
  1. Hovering a scrolled checklist item showed the tooltip at the bottom of the full text instead of next to the visible line.
  2. In extreme cases the tooltip could appear completely off-screen or over unrelated UI.
- Root cause:
  - `draw_clamped_scrollable_label()` returned `ScrollAreaOutput::inner` (the Label response) directly.
  - The Label's `rect` spanned the entire wrapped text, not the capped 120px viewport.
- Resolution:
  - After `ScrollArea::show()`, compute a `visible_rect` by clamping the initial inner rect height to the actual content size (`min(inner_rect.height(), content_size.y)`).
  - Patch the returned `Response` so that `rect` and `interact_rect` are set to the visible rect.
  - Tooltip and click detection now anchor to the on-screen visible area.
- Prevent recurrence:
  - Regression tests added:
    - `draw_clamped_scrollable_label_response_rect_capped_for_long_text`
    - `draw_clamped_scrollable_label_response_rect_shrinks_for_short_text`
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Check-list panel project headers gained accordion collapse/expand {#checklist-accordion-headers}
- Date: 2026-05-14
- Context: User requested that Check-list panel project names behave like an accordion so items can be hidden/shown per project, similar to foreground/background filter tabs.
- Error signature: `draw_checklist_panel` rendered all checklist items for every project unconditionally. Project headers were static labels with no interaction, making long checklists hard to scan.
- Symptoms/Impact:
  1. Users could not collapse a project to hide its checklist items.
  2. With many projects or long items, the panel became cluttered and hard to navigate.
- Root cause:
  - `draw_checklist_panel` used a plain `ui.horizontal` label block for the project header with no state tracking or toggle behavior.
  - `checklist_collapsed_by_project` existed in `AdeApp` but was never read or written by the panel draw code.
- Resolution:
  - Replaced the static project header with an interactive accordion row: a clickable body area (left portion) and a separate copy button area (right portion).
  - Added a visual arrow indicator (`▸` / `▾`) and folder icon (`icons::FOLDER` / `icons::FOLDER_OPEN`) to communicate collapse state.
  - Clicking the body toggles `checklist_collapsed_by_project` for that project; the item list is only rendered when the project is expanded.
  - The copy button uses its own interaction rect so it never triggers the accordion toggle.
  - Default state remains expanded (not collapsed).
- Prevent recurrence:
  - Regression tests added:
    - `checklist_panel_project_collapsed_hides_items` — verifies panel draw does not panic when a project is collapsed
    - `checklist_panel_project_collapse_state_isolated_per_project` — verifies toggling one project does not affect another
  - Added `Check-list Panel Guidelines` section to AGENTS.md documenting accordion behavior, per-project state, and copy button separation.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Check-list and Browser panel resize handle flickered because missing dim overlay {#checklist-browser-resize-flicker}
- Date: 2026-05-14
- Context: User reported that the resize bar between the terminal and Check-list panel flickers rapidly when the mouse hovers over it.
- Error signature: The default egui `SidePanel` resize separator uses `fg_stroke`, which in this dark theme is almost white (`TEXT_PRIMARY`). When the Check-list and Browser panels were resized, the bright separator could flash against the dark background because no custom dim overlay was applied to those panels.
- Symptoms/Impact:
  1. Mouse hovering the terminal–Check-list boundary caused rapid visual flicker.
  2. The resize cursor felt unstable because the bright separator kept appearing and disappearing.
  3. Browser panel (when open next to Check-list) had the same issue on its left edge.
- Root cause:
  - Project Explorer already had a custom dim resize overlay (`project-explorer-resize-overlay`) painted in `Order::Foreground` after the panel rendered.
  - Check-list and Browser panels used `show_separator_line(false)` but did not replace the missing separator with the same dim overlay, leaving the default egui bright `fg_stroke` visible during hover.
- Resolution:
  - Extracted the overlay logic into a reusable free function `paint_panel_resize_overlay(ctx, panel_rect, side, id_suffix)` and a `PanelResizeSide` enum.
  - Replaced the inline Project Explorer overlay with a call to the new helper.
  - Added overlay calls at the end of `draw_checklist_panel` and `draw_browser_panel` so all three resizable side panels use the same visual treatment.
  - Overlay colors remain: dim `Color32::from_rgb(45, 45, 45)` normally, slightly brighter `Color32::from_rgb(80, 80, 80)` on hover, and suppressed while any popup is open.
- Prevent recurrence:
  - Regression tests added:
    - `panel_resize_overlay_right_uses_left_edge`
    - `panel_resize_overlay_left_uses_right_edge`
    - `paint_panel_resize_overlay_skips_when_popup_open`
    - `paint_panel_resize_overlay_uses_hover_color_when_near`
    - `paint_panel_resize_overlay_uses_dim_color_when_far`
  - Updated AGENTS.md Resizable Panel Guidelines to state the overlay applies to **Project Explorer**, **Check-list**, and **Browser** panels.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Terminal Manager empty worktree row turned bright white when selected {#worktree-empty-selected-bright}
- Date: 2026-05-14
- Context: User reported that clicking an empty worktree row in Terminal Manager made the label turn bright white even though the worktree had no live terminals, which felt misleading.
- Error signature: `draw_terminal_manager_worktree_row` used `if is_selected { TEXT_PRIMARY } else { with_alpha(ACCENT, 200) }` for text color, ignoring whether the worktree actually had any live terminals.
- Symptoms/Impact:
  1. Empty worktree rows looked identical to active project headers after being clicked.
  2. Users could not visually distinguish a selected but inactive worktree from one that had running terminals.
- Root cause:
  - The worktree row color logic only considered `is_selected` and did not gate brightness on `has_live_terminal`.
- Resolution:
  - Added `has_live_terminal: bool` to `draw_terminal_manager_worktree_row` signature.
  - Introduced `worktree_row_text_color(is_selected, has_live_terminal)` helper that returns `TEXT_PRIMARY` only when `is_selected && has_live_terminal`, otherwise `with_alpha(ACCENT, 200)`.
  - Call site in `draw_terminal_manager_contents` now computes `wt_has_live_terminal` from `terminal_count_live_for_project_kind` and passes it into the row draw function.
- Prevent recurrence:
  - Regression tests added:
    - `worktree_row_text_color_selected_without_live_terminal_is_muted`
    - `worktree_row_text_color_selected_with_live_terminal_is_bright`
    - `worktree_row_text_color_unselected_is_always_muted`
  - Added AGENTS.md guideline: "Worktree row text color must reflect live terminal presence".
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Check-list panel project header arrow removed {#checklist-header-arrow-removed}
- Date: 2026-05-14
- Context: User requested that the collapse/expand arrow next to the folder icon in the Check-list panel project header be removed, keeping only the folder icon.
- Error signature: `draw_checklist_panel` rendered an arrow indicator (`▸` / `▾`) next to the folder icon in the project accordion header.
- Symptoms/Impact:
  1. The arrow was visually redundant because the folder icon already changes between `icons::FOLDER` (collapsed) and `icons::FOLDER_OPEN` (expanded).
  2. User found the arrow cluttered the header.
- Root cause:
  - The header label concatenated the arrow string and folder icon string.
- Resolution:
  - Removed the `arrow` variable and changed the header label to render only the `folder_icon`.
  - Collapse/expand behavior remains intact; clicking the header still toggles `checklist_collapsed_by_project`.
- Prevent recurrence:
  - No new regression tests needed; existing accordion tests (`checklist_panel_project_collapsed_hides_items`, `checklist_panel_project_collapse_state_isolated_per_project`) continue to verify behavior.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Check-list panel converted to floating bottom-right popup {#checklist-floating-popup}
- Date: 2026-05-14
- Context: User requested that the Check-list panel be changed from a fixed right-side panel to a floating bottom-right popup that opens when clicking the activity rail icon.
- Error signature: `draw_checklist_panel` used `egui::SidePanel::right` which permanently reduced the main terminal area width and was always visible, cluttering the layout.
- Symptoms/Impact:
  1. The Check-list consumed ~352px of horizontal screen space at all times.
  2. Users could not hide it without closing the entire right panel stack.
  3. Terminal Manager tooltip right-edge calculations had to reserve space for the panel even when not needed.
- Root cause:
  - The Check-list was implemented as a persistent `SidePanel::right` with no toggle behavior.
  - `main_area_size_from_chrome` subtracted `checklist_rect.width()` from the terminal area.
  - Activity rail icon was pinned (always active) with no state tracking.
- Resolution:
  - Replaced `SidePanel::right` with `egui::Window::new("Check-list")` anchored to `Align2::RIGHT_BOTTOM` with `.open()` close button.
  - Added runtime-only `checklist_floating_open: bool` state to `AdeApp`; activity rail icon now toggles this state.
  - `draw_checklist_panel` returns `None` so it no longer affects main layout width.
  - Removed checklist width subtraction from `main_area_size_from_chrome` call sites.
  - Updated Terminal Manager tooltip `right_offset` to no longer reserve checklist width.
  - Added `checklist_floating_open` to `embedded_browser_should_yield_to_ui_layer` and `terminal_output_mouse_wheel_enabled` predicates so the floating popup participates in overlay yield correctly.
  - Sanitized legacy `UiConfig::checklist_panel_expanded` to `false` in defaults and recovery.
- Prevent recurrence:
  - Regression tests updated:
    - `checklist_panel_project_collapsed_hides_items` and `checklist_panel_renders_long_items_without_crash` now set `checklist_floating_open = true` before rendering.
    - `browser_panel_and_checklist_can_coexist` renamed to `browser_panel_and_floating_checklist_can_coexist` and updated to test floating state.
    - `checklist_open_closes_only_active_browser_for_scope_state` updated to use floating state.
    - `checklist_floating_remains_open_when_last_item_removed` verifies the popup stays open when emptied.
    - `recover_config_keeps_checklist_panel_*` tests now assert legacy field is sanitized to `false`.
    - `terminal_output_mouse_wheel_enabled_returns_false_when_checklist_floating_open` verifies wheel blocking.
    - `embedded_browser_yields_to_ui_overlay_layers` updated with new parameter and test case for checklist floating open.
- Files/Commands touched: `src/app.rs`, `src/models.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo check`, `cargo fmt`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-14

---

#### Smart Input queued items invisible because queue row expanded past footer width {#smart-input-queue-row-overflow}
- Date: 2026-05-14
- Context: User reported that queued tasks added to Smart Input were not visible; the footer height changed (items were being added) but the content seemed to stretch to the right and disappear.
- Error signature: `draw_smart_input_footer` drew queue rows inside a `ScrollArea::vertical` without capping the inner content width. A single long prompt caused the ScrollArea content width to expand, pushing action buttons (Send/Copy/Edit/Delete) far outside the visible viewport.
- Symptoms/Impact:
  1. Long queued tasks made the row extend horizontally beyond the footer boundary.
  2. Action buttons became invisible because they were positioned off-screen to the right.
  3. Users could not see or interact with queued items.
- Root cause:
  - `ScrollArea::vertical` inner `ui` did not have a finite `max_width`, so wide content widened the scrollable area.
  - `Label::truncate()` inside `ui.horizontal` relied on `available_width`, which was unbounded in this context.
  - Action buttons were placed after the label and were carried along with the expanding row.
- Resolution:
  - Inside `ScrollArea::show`, capture `queue_width = ui.available_width()` and call `ui.set_max_width(queue_width)` and `ui.set_min_width(queue_width)` to lock the scrollable content width to the viewport.
  - Inside each row `ui.horizontal`, also call `ui.set_max_width(queue_width)` to enforce the bound.
  - For the prompt label, switch from `ui.add(Label::truncate())` to `ui.add_sized(..., Label::truncate())` with an explicit width that reserves space for action buttons (`available_width - action_reserved`).
  - When a queued task has empty text but contains image attachments, render a visible placeholder label "(Image attachment)" so the row is never blank.
  - **v2 fix**: Replaced the dynamic `ScrollArea` queue with a fixed-height slot (`allocate_exact_size`) that always reserves space for up to 3 task rows. Added alternating row backgrounds so the queue area is visible even with empty text. Made the Smart Input footer height fixed (`SMART_INPUT_BASE_FOOTER_HEIGHT`) and ignored `user_height`/`draft_user_height`. Disabled both resize grips (footer handle and draft grip) by removing their drag logic so they are visual-only.
  - **v3 fix**: Restored footer and draft resize handles with active drag logic, but clamped to a `safe_min` that guarantees queue slot + draft input fit. Footer height is now dynamic again but bounded: `user_height` is clamped upward to `safe_min` (base + task_rows * row_height + margin) and downward to `max_footer`. Queue row layout changed from `Align::Center` to `Align::Min` so task text is left-aligned. Header control buttons no longer have a 100px spacer pushing them right.
- Prevent recurrence:
  - Regression tests added:
    - `smart_input_queue_row_does_not_expand_past_footer_width` — renders the footer with a 500-char queued task inside a 400px-wide viewport and asserts that no text shape exceeds the footer width.
    - `smart_input_queue_slot_is_visible_when_tasks_exist` — renders the footer with a queued task and asserts the task text appears as a text shape.
    - `smart_input_footer_user_height_clamped_to_safe_min_when_tasks_exist` — asserts that `user_height` below the safe minimum is clamped upward so the queue slot is not crushed.
  - Added AGENTS.md guidelines: "Queue rows must cap content width...", "Smart Input queue area must use a fixed slot...", "Smart Input footer height must clamp to a safe minimum so queue and draft always fit...", "Footer height is user-resizable via a drag handle...", "Draft text area resize grip resizes the overall footer..."
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-14

---

#### Smart Input draft input invisible when queue is empty and expanded {#smart-input-empty-queue-draft-clipped}
- Date: 2026-05-15
- Context: User reported that Smart Input was visible but the draft text input box was completely missing when the queue was empty and expanded.
- Error signature: `draw_smart_input_footer` always allocated a full 3-row queue slot (`SMART_INPUT_MAX_VISIBLE_TASK_ROWS * SMART_INPUT_TASK_ROW_HEIGHT + 4.0 = 88px`) whenever `state.expanded` was true, even with zero queued tasks. Meanwhile, `smart_input_footer_height()` computed `desired = SMART_INPUT_BASE_FOOTER_HEIGHT` (156px) for the empty-queue case. The rendered content actually needed ~174px (header + mode + empty-queue hint + draft + margins), so the draft input overflowed the 156px footer and was clipped off-screen.
- Symptoms/Impact:
  1. The "No queued tasks. Add a prompt below." hint appeared, but the multiline draft TextEdit below it was invisible and unreachable.
  2. Users could not type or paste into Smart Input when the queue was empty.
  3. The footer looked like it had plenty of space, but the bottom ~80px of content (draft input + submit button) was hidden.
- Root cause:
  - Height calculation (`smart_input_footer_height`) and rendering (`draw_smart_input_footer`) were out of sync: the renderer reserved 88px for an empty 3-row slot while the height calculator assumed 0px for tasks.
  - The empty-queue hint height was never accounted for in the footer height formula, causing a 18px deficit.
- Resolution:
  - Added `smart_input_visible_task_rows()` and `smart_input_queue_slot_height()` helpers so both the height calculator and the renderer use the exact same task-row count and slot size.
  - Added `SMART_INPUT_EMPTY_QUEUE_HINT_HEIGHT` (18px) to the desired and safe-min calculations when `expanded` is true and no tasks exist.
  - Updated both resize handles (main footer handle and draft grip) to use `smart_input_safe_min_footer_height()` so dragging never crushes the hint or draft.
  - In `draw_smart_input_footer`, the queue slot is now sized to `smart_input_queue_slot_height(task_rows)`, which is `0px` when empty, allowing the hint and draft to fit within the calculated footer height.
- Prevent recurrence:
  - Added regression tests:
    - `smart_input_footer_height_is_tall_enough_for_expanded_empty_queue` — asserts footer height >= base + empty hint (174px).
    - `smart_input_empty_queue_does_not_allocate_three_row_slot` — renders footer with empty queue, asserts the "No queued tasks" hint is visible and no task row backgrounds are painted.
    - `smart_input_footer_user_height_clamped_to_safe_min_when_tasks_exist` — updated to match actual slot height (base + rows*28+4 + 60).
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo test`, `cargo fmt`
- References: User request 2026-05-15

---

#### OpenCode Smart Input questions were mouse-only {#opencode-smart-input-question-keyboard-navigation}
- Date: 2026-05-15
- Context: User reported that OpenCode questions already appeared inside Smart Input, but wanted an OpenCode-like selection flow that works with arrow keys and Enter instead of requiring mouse clicks.
- Error signature: `draw_smart_input_footer` rendered question options as horizontal `selectable_label` widgets, while `raw_input_hook` only handled Smart Input draft/edit submit and history navigation. No terminal-scoped question focus state existed, so Arrow keys could fall through to Smart Input history or terminal routing instead of moving through question answers.
- Symptoms/Impact:
  1. Users had to click question options and the Submit/Reject buttons with the mouse.
  2. Arrow keys could affect Smart Input draft history rather than the active question prompt.
  3. The Smart Input question card did not visually match OpenCode's keyboard-first question prompt flow.
- Root cause:
  - Question state tracked selected labels and custom text, but not the currently highlighted answer row.
  - `ensure_smart_input_focus()` could auto-focus the draft while a question was pending, stealing keyboard intent from the question card.
  - The footer height calculation did not reserve additional space for the expanded question card.
- Resolution:
  - Added terminal-scoped `opencode_question_focus_index` and lifecycle helpers to initialize, clamp, and clear question focus alongside selected/custom answer state.
  - Added question-specific keyboard routing in `raw_input_hook`: Up/Down and Left/Right move focus, Enter submits or focuses custom input, Space toggles multi-select answers, and Escape rejects.
  - Reworked the Smart Input question card into a vertical OpenCode-style list with a highlighted row, selected markers, keyboard help text, and custom-answer focus behavior.
  - Included question-card height in Smart Input footer desired/safe-min height so the prompt is less likely to clip the draft/queue controls.
- Prevent recurrence:
  - Added regression tests:
    - `opencode_question_arrow_keys_move_focus_without_draft_history` — asserts ArrowDown is consumed by the question card and does not alter Smart Input draft history.
    - `opencode_question_enter_submits_focused_option_answer` — asserts Enter submits the highlighted option through the hook answer bridge and clears question state.
    - `opencode_question_escape_rejects_answer` — asserts Escape queues a rejected answer and clears the prompt.
    - `opencode_question_keyboard_ignores_hidden_active_terminal` — asserts hidden/off-main terminals do not consume question keyboard input.
    - `smart_input_footer_height_handles_question_safe_min_above_max_footer` — asserts question footer sizing does not panic when pane height is smaller than the safe minimum.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test opencode_question`, `cargo test smart_input_footer_height_handles_question_safe_min_above_max_footer`, `cargo test`
- References: User request 2026-05-15

---

#### Smart Input footer content could paint past the allocated width {#smart-input-footer-inner-width-overflow}
- Date: 2026-05-15
- Context: Full `cargo test` after the Smart Input question work exposed `smart_input_queue_row_does_not_expand_past_footer_width` failing with text painted at x=412 inside a 400px footer.
- Error signature: `draw_smart_input_footer()` set the framed content UI min/max width to the outer `footer_size.x` even though the frame also adds 8px horizontal inner margins. This allowed header/button/draft text shapes to render beyond the footer's allocated outer width.
- Symptoms/Impact:
  1. Long queued task scenarios could push visible text or action controls outside the footer bounds.
  2. The queue row regression test caught text shapes extending past the 400px footer width.
- Root cause:
  - The footer frame's child UI used the outer size instead of subtracting the frame's horizontal and vertical margins.
- Resolution:
  - `draw_smart_input_footer()` now computes an inner size by subtracting 16px horizontal and 12px vertical frame margins before calling `ui.set_min_size()` and `ui.set_max_width()`.
- Prevent recurrence:
  - `smart_input_queue_row_does_not_expand_past_footer_width` is covered by the full `cargo test` suite and now passes.
- Files/Commands touched: `src/app.rs`, `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test smart_input_queue_row_does_not_expand_past_footer_width`, `cargo test`
- References: Full-suite validation 2026-05-15

---

#### Codex CLI showed deprecated `features.codex_hooks` warning {#codex-hooks-feature-flag-rename}
- Date: 2026-05-15
- Context: Codex CLI v0.130.0 warned that `[features].codex_hooks` is deprecated and should be replaced with `[features].hooks`.
- Error signature: Mergen's Codex integration patcher wrote `features.codex_hooks = true` into `~/.codex/config.toml`, while current Codex docs list `features.hooks` as the stable lifecycle hook flag.
- Symptoms/Impact:
  1. Every `codex` launch could show a deprecation warning before starting MCP servers.
  2. The warning made Mergen-managed Codex hook setup look stale even though `hooks.json` was otherwise valid.
- Root cause:
  - `patch_codex_config_file()` and integration health checks still used the old `codex_hooks` feature key.
- Resolution:
  - Updated Codex config patching to write `features.hooks = true`.
  - Removed the deprecated `features.codex_hooks` key during repair.
  - Updated inspection so configs that only contain the deprecated alias are repaired instead of reported healthy.
- Prevent recurrence:
  - Updated regression assertions to require `features.hooks = true` and verify that `features.codex_hooks` is not written or preserved.
- Files/Commands touched: `src/codex.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test codex`, `cargo test`
- References: User request 2026-05-15; OpenAI Codex config docs `https://developers.openai.com/codex/config-basic#supported-features`

---

#### Foreground launcher popup could show Terminal Manager dividers through its margin {#foreground-launcher-popup-margin-opaque-backing}
- Date: 2026-05-15
- Context: User reported that the Terminal Manager foreground launcher menu (OpenCode, Codex, Droid, Claude) let the right-side Terminal Manager background/divider remain visible behind the dropdown.
- Error signature: `draw_launcher_menu_contents()` painted an opaque backing only over the deterministic content rectangle, while egui's `Frame::menu` adds a menu margin around that content.
- Symptoms/Impact:
  1. Divider lines from the Terminal Manager could remain visible inside the popup margin.
  2. The launcher dropdown looked partially transparent even though row backgrounds were opaque.
- Root cause:
  - The custom launcher-menu backing rectangle did not include `ui.spacing().menu_margin`, so the frame margin depended on the surrounding popup/frame paint and could visually blend with underlying UI lines.
- Resolution:
  - Added launcher-menu sizing helpers and expanded the custom backing rectangle by the active egui menu margin.
  - Drew the expanded backing with the normal popup border before painting launcher rows.
- Prevent recurrence:
  - Added initial content-level regression coverage for opaque backing; later follow-up replaced this with real popup-frame coverage after screenshots showed the issue persisted through the `menu_button` path.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test foreground_launcher_menu -- --nocapture`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-15

---

#### Foreground launcher popup frame was only visually solid while hovering rows {#foreground-launcher-custom-opaque-popup}
- Date: 2026-05-15
- Context: Follow-up screenshots showed the foreground launcher dropdown still revealed Terminal Manager dividers in its default open state, while row hover made the popup look solid.
- Error signature: The prior fix painted content-level backing, but the real `ui.menu_button` popup path still depended on egui menu-frame behavior and transparent minimal button chrome.
- Symptoms/Impact:
  1. Default open launcher popup could show vertical Terminal Manager lines through row gaps and popup edges.
  2. Hovering a row hid the issue because the hovered row painted a stronger fill.
- Root cause:
  - The launcher dropdown mixed custom row painting with egui's standard `menu_button` popup frame, so the outer popup surface was not controlled by the launcher widget itself.
- Resolution:
  - Replaced the launcher-only `ui.menu_button` usage with a custom `egui::Area` popup.
  - The custom popup uses an always-opaque `Frame` with `SURFACE_BG`, border stroke, menu rounding, and popup shadow before drawing launcher rows.
- Prevent recurrence:
  - Added regression coverage that opens the real launcher popup from its button and asserts an opaque fixed-width popup frame exists without hovering any row.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test foreground_launcher_menu -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User screenshots 2026-05-15

---

#### Foreground launcher popup had larger bottom padding than top padding {#foreground-launcher-equal-vertical-padding}
- Date: 2026-05-15
- Context: After the popup became fully opaque, user reported the top and bottom empty space around launcher rows should be equal.
- Error signature: `draw_launcher_menu_contents()` explicitly added equal top/bottom padding, but each row was drawn with `allocate_new_ui()`, which advanced the parent cursor by egui's default `item_spacing.y` after the row.
- Symptoms/Impact:
  1. Bottom padding looked larger than top padding.
  2. The popup frame height was a few pixels taller than the deterministic `padding + rows + row gaps` formula.
- Root cause:
  - Launcher row spacing mixed explicit `FOREGROUND_LAUNCHER_ROW_GAP` / `FOREGROUND_LAUNCHER_MENU_PADDING_Y` with implicit egui item spacing.
- Resolution:
  - Temporarily set `item_spacing.y = 0.0` while drawing launcher menu contents, then restore the previous spacing.
- Prevent recurrence:
  - Added real-popup regression tests that compare the first row's top gap and the last row's bottom gap against the popup frame.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test foreground_launcher -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User screenshot 2026-05-15

---

#### Smart Input queued task text was centered in wide rows {#smart-input-queued-task-left-align}
- Date: 2026-05-15
- Context: User reported that the queued Smart Input task text in the row above the draft editor appeared centered instead of starting near the row number.
- Error signature: The queued task preview used a fixed-width `Label`, letting egui position short text inside the whole reserved preview area.
- Symptoms/Impact:
  1. Short queued prompts such as `test` appeared in the middle of the Smart Input queue row.
  2. The row number stayed left-aligned, making the prompt look detached from its queue index.
- Root cause:
  - The UI reserved a large preview rectangle to keep action buttons stable, but delegated text placement to `Label` instead of explicitly painting from the preview rectangle's left edge.
- Resolution:
  - Kept the same preview hit area and action-button reservation, but rendered the truncated task preview galley manually at the left edge of that area.
- Prevent recurrence:
  - Added a regression test that renders a wide Smart Input footer and asserts the queued task text starts near the `1.` index label.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test smart_input_queue -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User screenshot 2026-05-15

---

#### Smart Input queued task index and text used different vertical alignment paths {#smart-input-queued-task-index-baseline}
- Date: 2026-05-15
- Context: Follow-up screenshot showed the queued task text was no longer centered in the row, but it still did not line up with the visible `1.` queue index.
- Error signature: The queue index was rendered with `row_ui.label(...)`, while the task preview was rendered manually as a galley using row-center positioning.
- Symptoms/Impact:
  1. The `1.` index and queued prompt appeared on slightly different vertical centers.
  2. The queued task row looked visually uneven even though the text started near the left edge.
- Root cause:
  - The index label and task preview used different egui layout/painting paths, so their vertical placement was not computed from the same row geometry.
- Resolution:
  - Rendered the queue index and task preview with the same manual galley path, both centered against `SMART_INPUT_TASK_ROW_HEIGHT`.
- Prevent recurrence:
  - Extended the Smart Input queue regression test to assert that the index and task text share the same vertical center.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test smart_input_queue -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User screenshot 2026-05-15

---

#### Smart Input queued task edit opened a separate inline editor {#smart-input-edit-uses-draft}
- Date: 2026-05-15
- Context: User wanted the queued task `Edit` button to put the prompt back into the main Smart Input draft area instead of editing it in the queue row.
- Error signature: Queue-row edit used `editing_task_id` and `edit_draft`, creating a second edit surface with separate focus and save/cancel controls.
- Symptoms/Impact:
  1. Editing a queued prompt required using a small inline field rather than the larger prompt input.
  2. The normal Enter-to-queue draft workflow was bypassed for edits.
- Root cause:
  - The queue edit action was modeled as an inline row state instead of returning the task to the draft workflow.
- Resolution:
  - Changed the queue `Edit` action to move the task text and attachments into the main draft input.
  - Stored the original queue index and task id so Enter re-queues the edited prompt at the same position.
  - Blocked edit when the draft already has content or attachments, preserving both draft and queue state and showing a status/toast message.
- Prevent recurrence:
  - Added Smart Input state regression tests for draft edit transfer, original-index requeue, attachment preservation, and draft-occupied blocking.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test smart_input -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-15

---

#### Smart Input queue rendered only the first visible rows {#smart-input-queue-scroll}
- Date: 2026-05-15
- Context: User reported that queued Smart Input prompts beyond the first 3-4 rows could not be seen because the queue area had no scroll.
- Error signature: `draw_smart_input_footer()` computed a capped visible row count and iterated only `0..task_rows`, so later tasks were never rendered.
- Symptoms/Impact:
  1. Queued prompts after the capped visible row count were inaccessible in the UI.
  2. The existing `queue_scroll_to_end` state had no effect because there was no queue `ScrollArea`.
- Root cause:
  - The queue slot height was correctly capped, but the render loop was also capped instead of drawing all tasks inside a scrollable viewport.
- Resolution:
  - Wrapped the queue rows in a fixed-height vertical `ScrollArea`.
  - Kept the visible queue slot height capped while rendering the full task list inside the scroll area.
  - Used `queue_scroll_to_end` to reveal newly appended tasks without forcing bottom-scroll for edits reinserted at their original index.
- Prevent recurrence:
  - Added Smart Input queue tests for capped footer height with 5 tasks and visibility of later tasks after queue scroll-to-end.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test smart_input_queue -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-15

---

#### Smart Input queue rows leaked outside the scroll viewport {#smart-input-queue-scroll-clip}
- Date: 2026-05-15
- Context: After adding queue scrolling, user screenshot showed queued rows painting over the terminal area and Smart Input header.
- Error signature: Each queue row UI called `set_clip_rect(row_rect)`, replacing the active `ScrollArea` viewport clip.
- Symptoms/Impact:
  1. Offscreen queue rows remained visibly painted above the Smart Input queue area.
  2. The Smart Input header and terminal content were visually overlapped by queued rows.
- Root cause:
  - Row-level clipping used only the row rectangle and did not intersect it with the ScrollArea viewport clip.
- Resolution:
  - Clip each queue row to `row_rect.intersect(ui.clip_rect())` so row rendering is constrained by both the row and the queue viewport.
- Prevent recurrence:
  - Added regression coverage for row/viewport clip intersection and for visible queue task rows staying below the Smart Input header.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test smart_input_queue -- --nocapture`, `cargo test --no-run`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User screenshot 2026-05-15
