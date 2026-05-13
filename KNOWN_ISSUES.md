
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
