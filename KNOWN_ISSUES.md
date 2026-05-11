# Known Issues

This file tracks bugs, regressions, and architectural decisions that have caused user-facing issues in Mergen ADE. It is append-only unless the user explicitly asks for cleanup.

When adding an entry:
- Use the format: `#### Title {#slug}` followed by `- Date`, `- Context`, `- Error signature`, `- Symptoms/Impact`, `- Root cause`, `- Resolution`, `- Prevent recurrence`, `- Files/Commands touched`, `- References`.
- Keep dates in `YYYY-MM-DD` format.
- If a regression has been fixed by a code change, link the commit or PR.
- Do not delete old entries without user confirmation.

---

#### Settings popup hidden behind browser WebView {#settings-popup-webview-z-order}
- Date: 2026-05-08
- Context: Settings modal and other UI overlays appearing behind the embedded browser WebView2 window
- Error signature: User reported: "settings browser filan açıkken arkada kalıyor tıklanmıyor hiçbir şekilde rengi filan değişiyor daha koyu oluyor" — Settings popup appears behind the browser panel and cannot be clicked; the browser content appears darker.
- Symptoms/Impact:
  1. When opening Settings while the browser panel is visible, the Settings modal appears behind the native WebView2 window.
  2. The Settings modal is visible but appears darker/less prominent (WebView content shows through the overlay).
  3. Mouse clicks do not register on the Settings modal because the WebView window is intercepting them.
  4. The Settings modal cannot be interacted with (buttons don't work, text inputs don't receive focus).
- Root cause:
  - Native WebView2 renders as a child window (`HWND`) above egui's immediate-mode rendering layer.
  - The `sync_embedded_browser()` function that hides/shows the WebView is called AFTER UI rendering, not before.
  - When Settings opens, the WebView remains visible and intercepts all mouse/keyboard events that should go to the modal.
  - The modal overlay (dark backdrop) renders in egui but the WebView window stays on top, creating a visual conflict.
- Resolution:
  - Modified `open_settings_popup()` to immediately hide all embedded browsers and clear `pending_browser_rect` when the popup opens:
    ```rust
    self.hide_embedded_browsers();
    self.pending_browser_rect = None;
    ```
  - Modified `draw_settings_popup()` to call `hide_embedded_browsers()` and clear `pending_browser_rect` at the start of rendering, ensuring the WebView is hidden BEFORE the first frame of the modal is drawn.
  - Applied the same fix to `draw_foreground_message_popup()` and `draw_exit_confirm_popup()` for consistency.
  - The `sync_embedded_browser()` function already had logic to hide browsers when overlays are active, but calling it at the start of modal rendering ensures immediate hiding without waiting for the frame end.
- Prevent recurrence:
  - Added regression tests verifying modal browser hiding:
    - `open_settings_popup_hides_browsers_immediately`: Verifies browser is hidden immediately when Settings opens
    - `open_settings_popup_hides_terminal_scoped_browsers`: Verifies terminal-scoped browsers are also hidden
    - `foreground_message_popup_hides_browsers_immediately`: Verifies foreground message popup hides browsers
    - `exit_confirm_popup_hides_browsers_immediately`: Verifies exit confirm popup hides browsers
  - Updated `AGENTS.md` Embedded Browser Panel Guidelines to document the requirement that all modal popups must hide embedded browsers before rendering.
- Files/Commands touched: `src/app.rs` (`open_settings_popup()`, `draw_settings_popup()`, `draw_foreground_message_popup()`, `draw_exit_confirm_popup()`, 4 new regression tests), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User bug report 2026-05-08: "settings browser filan açıkken arkada kalıyor tıklanmıyor hiçbir şekilde rengi filan değişiyor daha koyu oluyor"

---

#### OS Notifications MVP implemented with FlashWindowEx {#os-notifications-mvp}
- Date: 2026-05-08
- Context: New feature - OS attention notifications for AI CLI status changes
- Error signature: N/A - feature implementation, not a bug fix
- Symptoms/Impact:
  1. Users needed visual/audible attention signals when AI CLIs (Factory Droid, OpenCode, Claude) require user attention.
  2. No portable EXE-friendly notification mechanism existed (Windows toast notifications require AppUserModelID/shortcut setup).
  3. Notification spam risk if multiple terminals trigger attention states rapidly.
- Root cause: N/A - new feature requirement
- Resolution:
  - Implemented OS notifications using Windows `FlashWindowEx` API for portable EXE compatibility.
  - Added `OsNotificationConfig` struct with fields: `enabled`, `only_when_unfocused`, `on_permission`, `on_turn_complete`, `on_session_error`, `cooldown_secs`.
  - Notifications trigger when AI CLI status transitions to `AiCliStatus::Attention` (Factory Droid, OpenCode, Claude).
  - Focus-aware delivery: checks `ctx.input(|i| i.viewport().focused)` to suppress notifications when app is focused (if `only_when_unfocused` is enabled).
  - Cooldown deduplication: each terminal tracks last notification time via `os_notification_last_by_terminal: BTreeMap<u64, Instant>`.
  - Pending notification pattern: status apply functions set `pending_os_notification: Option<u64>`, processed by `process_pending_os_notifications()` in update loop.
  - Settings UI added with toggles for each trigger type and cooldown duration control.
- Prevent recurrence:
  - Updated `AGENTS.md` with OS Notifications Guidelines section documenting the FlashWindowEx approach, trigger conditions, focus-aware delivery, and cooldown mechanism.
  - Config persistence ensures user preferences survive restarts.
- Files/Commands touched:
  - `src/models.rs`: Added `OsNotificationConfig` struct with `PartialEq` derive and `Default` impl
  - `src/models.rs`: Added `notifications: OsNotificationConfig` field to `AppConfig`
  - `src/config.rs`: Updated `AppConfig` legacy conversion to include `notifications` field
  - `src/app.rs`: Added `os_notification_last_by_terminal`, `pending_os_notification` to `AdeApp`
  - `src/app.rs`: Added `SettingsSection::Notifications` variant with `Eye` icon
  - `src/app.rs`: Added `draw_settings_notifications_section()` with toggle UI and cooldown control
  - `src/app.rs`: Added `process_pending_os_notifications()` with `FlashWindowEx` implementation
  - `src/app.rs`: Added pending notification triggers in `apply_factory_droid_status`, `apply_opencode_transport_status`, `apply_claude_status`
  - `src/app.rs`: Added `note_notifications_change()` to `SettingsEditOutcome`
  - `AGENTS.md`: Added OS Notifications Guidelines section
  - `KNOWN_ISSUES.md`: Added this entry
  - `cargo build --release --target x86_64-pc-windows-msvc` (successful)
- References: Feature request: "10 OS Notifications MVP — Windows-first, FlashWindowEx, portable-EXE constraints"

---

#### Foreground tasks now use paste-safe bracketed paste delivery {#foreground-tasks-bracketed-paste}
- Date: 2026-05-08
- Context: Foreground task queue send behavior in Terminal Manager - user requested tasks behave like paste instead of typing
- Error signature: User reported: "foregrounddan terminale yazarken normal kullanıcı yazıyormuş gibi yazıyor ama kopyala yapıştırdaki gibi hareket etmesini istiyorum tasks" - Foreground tasks were being sent to terminal as raw keystrokes instead of paste-safe delivery.
- Symptoms/Impact: When sending a foreground task to the terminal, it would type character-by-character like a user typing, which could cause issues with AI CLI slash menus interpreting the `/` key as a menu submission instead of part of the command.
- Root cause:
  - `send_saved_message_to_terminal()` used `message.as_bytes().to_vec()` and `terminal.runtime.send_bytes(outbound)` which sends raw bytes as keystrokes.
  - This differs from clipboard paste and terminal shortcuts which use `capture_paste_bytes()` and `send_paste_bytes()` for bracketed paste support.
  - When bracketed paste is enabled (`\x1b[?2004h`), paste operations wrap content in `ESC[200~...ESC[201~` sequences to indicate atomic paste content.
  - Without bracketed paste, slash-prefixed commands like `/prepare-fix-plan` could be misinterpreted by AI CLI slash menus.
- Resolution:
  - Changed `send_saved_message_to_terminal()` to use paste-safe delivery path:
    - Replace raw `send_bytes()` with `capture_paste_bytes()` + `send_paste_bytes()`.
    - This automatically applies bracketed paste wrapping when enabled in the terminal.
  - Preserved all existing behavior:
    - Title/history tracking via `append_pending_line()` and `pending_input_for_history.clear()`.
    - AI state detection (Factory Droid, Codex, OpenCode) via `has_non_empty_line` checks.
    - Two delayed Enter presses (0.5s and 1s) for command confirmation.
    - Recent inputs tracking via `push_recent_input()`.
    - Terminal visibility update via `in_main_view = true`.
  - Added clear error handling: if `capture_paste_bytes()` fails, show status message "Failed to prepare message for terminal".
- Prevent recurrence:
  - Added regression test `foreground_saved_message_uses_bracketed_paste_when_enabled` that:
    - Enables bracketed paste mode (`\x1b[?2004h`) in terminal.
    - Sends a foreground task via `send_saved_message_to_terminal()`.
    - Verifies output contains bracketed paste markers (`ESC[200~...ESC[201~`).
    - Verifies no immediate Enter is sent (Enters are scheduled via `pending_second_enter`).
    - Verifies two delayed Enters are scheduled.
  - Updated `AGENTS.md` Terminal Manager & Input History Guidelines to clarify that foreground tasks use paste-safe delivery.
- Files/Commands touched: `src/app.rs` (`send_saved_message_to_terminal()` paste-safe implementation, new regression test), `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-08: "foregrounddan terminale yazarken normal kullanıcı yazıyormuş gibi yazıyor ama kopyala yapıştırdaki gibi hareket etmesini istiyorum tasks"

---

#### Browser toolbar tooltips now centered above buttons {#browser-tooltips-centered-above}
- Date: 2026-05-08
- Context: Browser panel toolbar button hover tooltips (Design Inspect, Go, Clear, Screenshot, tab controls)
- Error signature: User reported: "yanında çıkıyor hover üstte sağ çaprazda çıksın butonun üstünde" — Tooltips on browser toolbar buttons were appearing offset to the side (right/corner) instead of being centered directly above the button.
- Symptoms/Impact: Hover tooltips on browser toolbar buttons (especially Design Inspect toggle) appeared horizontally offset from the button center, making them appear to the side rather than directly above the button as expected.
- Root cause:
  - `show_tooltip_at()` function uses `find_tooltip_position()` which determines tooltip placement based on available screen space.
  - When the suggested position (`rect.center_top()`) was passed, egui treated this as a `LEFT_TOP` anchor point rather than a center anchor.
  - The tooltip positioning logic would shift the tooltip horizontally to fit on screen, causing the offset appearance.
  - The pivot/anchor system in `show_tooltip_at()` doesn't support center-based anchoring directly.
- Resolution:
  - Replaced `egui::containers::show_tooltip_at()` calls with custom `egui::Area` based implementation in all tooltip helpers:
    - `browser_toolbar_icon_button()`: Go, Clear URL buttons
    - `browser_toolbar_toggle_button()`: Design Inspect toggle
    - `show_tooltip_above()`: Tab close, tab title, add tab buttons
  - New implementation uses `egui::Area::new()` with explicit configuration:
    - `pivot(egui::Align2::CENTER_BOTTOM)`: Centers tooltip horizontally above the anchor point
    - `fixed_pos(tooltip_anchor)`: Positions at button's center_top minus gap
    - `kind(egui::UiKind::Tooltip)` and `order(egui::Order::Tooltip)`: Proper tooltip layer ordering
    - `Frame::popup()` for consistent tooltip styling
  - Tooltip anchor calculation: `rect.center_top() + vec2(0.0, -BROWSER_TOOLBAR_TOOLTIP_GAP)`
  - This ensures tooltip is horizontally centered on the button regardless of tooltip content width.
- Prevent recurrence:
  - Updated `AGENTS.md` Browser Panel WebView Z-Order Guidelines to specify "centered above" instead of just "above":
    - "Toolbar and tab strip tooltips must appear centered above buttons"
    - "These helpers use `egui::Area` with `CENTER_BOTTOM` pivot so the tooltip is horizontally centered on the button, not offset to the side"
  - All three tooltip helpers now use the same centering implementation for consistency.
- Files/Commands touched: `src/app.rs` (updated `browser_toolbar_icon_button()`, `browser_toolbar_toggle_button()`, `show_tooltip_above()` to use Area-based centered tooltips), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User feedback 2026-05-08: "yanında çıkıyor hover üstte sağ çaprazda çıksın butonun üstünde"

---

#### Foreground tasks menu and popup max height with scroll {#foreground-tasks-max-height-scroll}
- Date: 2026-05-08
- Context: Foreground tasks dropdown menu and Add/Edit Task popup - user reported long text causing layout issues
- Error signature: User reported: "foreground tasks penceresine default max height ekle içine scroll ekle uzun metin olunca bozuluyor bozulmasın" — Foreground tasks menu and popup were breaking layout with long task lists or long text content.
- Symptoms/Impact:
  1. Foreground tasks dropdown menu with many tasks would grow indefinitely, potentially exceeding screen bounds.
  2. The Add/Edit Task popup's multiline text input would expand with long content, pushing buttons off-screen or expanding the window beyond viewport.
  3. Task tooltips with very long commands could extend beyond screen bounds.
- Root cause:
  - Task list in dropdown was not wrapped in a ScrollArea, allowing unlimited vertical growth.
  - TextEdit::multiline in the popup was not constrained with max height or ScrollArea, allowing content to expand the window.
  - Tooltips used raw message text without truncation, allowing unlimited width.
- Resolution:
  - Added `FOREGROUND_TASKS_MENU_MAX_HEIGHT = 300.0` constant and wrapped task list in `ScrollArea::vertical().max_height(FOREGROUND_TASKS_MENU_MAX_HEIGHT)`.
  - Added `FOREGROUND_MESSAGE_TEXT_MAX_HEIGHT = 320.0` constant and wrapped popup TextEdit in ScrollArea with clamped height between 200px min and 320px max.
  - Added `FOREGROUND_TASK_TOOLTIP_MAX_CHARS = 100` constant and applied `capped_hover_text()` to task tooltips.
  - Popup text input now scrolls internally instead of expanding the window.
- Prevent recurrence:
  - Updated `AGENTS.md` Terminal Manager Saved Messages Guidelines with new rules:
    - "Task list max height with scroll" documenting the ScrollArea requirement
    - "Popup layout" documenting min/max height and ScrollArea wrapping
    - "Popup text input scroll behavior" documenting internal scrolling
    - "Tooltip length limits" documenting tooltip character cap
- Files/Commands touched: `src/app.rs` (constants, `draw_foreground_message_popup()` ScrollArea wrapping, `draw_terminal_foreground_message_menu_button()` ScrollArea and tooltip changes), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-08: "foreground tasks penceresine default max height ekle içine scroll ekle uzun metin olunca bozuluyor bozulmasın"

---

#### Input History panel Foreground filter limited to 5 entries {#input-history-foreground-limit}
- Date: 2026-05-08
- Context: Input History left sidebar panel showing too many foreground entries
- Error signature: User reported: "input historyde terminal foregroundda max 5 tane gözüksün şuan hepsi geliyor" — The Foreground filter in Input History panel was showing all matching entries instead of limiting to 5.
- Symptoms/Impact: When viewing Input History with Foreground filter selected, users would see all historical foreground entries (potentially hundreds), making the list overwhelming and difficult to navigate.
- Root cause:
  - `draw_input_history_entries()` in `src/app.rs` collected all matching entries without applying any display limit.
  - The `RECENT_INPUTS_MAX = 5` constant existed but was only used for `recent_inputs` (terminal popup/runtime history), not for the Input History panel display.
  - The Foreground filter displayed every matching entry from persistent history (up to 500 entries per project).
- Resolution:
  - Modified `draw_input_history_entries()` to apply `truncate(Self::RECENT_INPUTS_MAX)` when `InputHistoryFilter::Foreground` is selected.
  - The total matching count is preserved for the UI label (shows "X entries" where X is total matching, not limited count).
  - `All` and `Background` filters remain unlimited — they show all matching entries.
  - Persistent storage limit (500 entries per project) unchanged — only the display is limited.
- Prevent recurrence:
  - Added regression test `input_history_panel_foreground_shows_max_five_entries` verifying:
    - 7 foreground entries in history, Foreground filter shows only 5
    - Total count (7) preserved for UI label
  - Added regression test `input_history_panel_all_and_background_unlimited` verifying:
    - All filter shows all entries (14 total: 7 foreground + 7 background)
    - Background filter shows all 7 background entries (no limit)
  - Updated `AGENTS.md` Terminal Manager & Input History Guidelines with the new rule.
- Files/Commands touched: `src/app.rs` (draw_input_history_entries() truncation logic, 2 new regression tests), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-08: "input historyde terminal foregroundda max 5 tane gözüksün şuan hepsi geliyor"

---

#### Browser tab strip tooltips now appear above tabs {#browser-tab-strip-tooltips-above}
- Date: 2026-05-08
- Context: Browser panel tab strip hover tooltips (Close tab, tab URL/title, New tab button)
- Error signature: User reported: "browserda toolbarda üstüne gelince hover geliyor ya o gelen hoverlar altta değil üstte çıksın" — Tooltips on tab strip elements (close button, tab title, add tab button) were appearing below the tabs, potentially overlapping the WebView content area.
- Symptoms/Impact: Tab strip tooltips appeared below the tab strip by default (egui standard behavior), causing them to extend into the WebView area where they could be obscured by the native WebView2 window.
- Root cause:
  - Tab close button, tab title hover, and add tab (+) button used standard `on_hover_text()` which positions tooltips below the widget.
  - Native WebView2 renders as a child window above egui's immediate-mode rendering, so any tooltip overlapping the WebView area gets obscured.
  - Previous fixes addressed toolbar buttons (Go, Clear, Design Inspect, Screenshot) but tab strip tooltips were missed.
- Resolution:
  - Changed tab close button tooltip from `.on_hover_text("Close tab")` to `show_tooltip_above(ui, &close_response, "Close tab")`.
  - Changed tab title/URL tooltip from `.on_hover_text(url...)` to `show_tooltip_above(ui, &tab_response, tab_url_tooltip)`.
  - Changed add tab (+) button tooltip from `.on_hover_text(if can_add_tab {...})` to `show_tooltip_above(ui, &add_response, add_tooltip)`.
  - All tab strip tooltips now render entirely within the egui layer above the WebView using the shared `show_tooltip_above()` helper.
- Prevent recurrence:
  - Updated `AGENTS.md` Browser Panel WebView Z-Order Guidelines with new rule: "Tab strip tooltips must also appear above."
  - The pattern is consistent with toolbar buttons which already use `browser_toolbar_icon_button()` and `show_tooltip_above()` helpers.
- Files/Commands touched: `src/app.rs` (tab strip tooltip rendering in `draw_browser_panel()`), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-08: "browserda toolbarda üstüne gelince hover geliyor ya o gelen hoverlar altta değil üstte çıksın"

---

#### Browser tab lifecycle improvements {#browser-tab-lifecycle-improvements}
- Date: 2026-05-08
- Context: Browser panel tab closing behavior and auto-loading saved URLs
- Error signature: User reported: "tek sekme varken kapattığımda kapanmıyor sekme kapansın ve artı ile açılabilsin ilk sekme ve browser açıldığında url varsa yüklesin ben ok tuşuna basmayayım" — Single tab doesn't close properly, should be able to create first tab with + button, and browser should auto-load saved URL without pressing Enter.
- Symptoms/Impact:
  1. Closing the last tab would immediately recreate a new empty tab instead of leaving the browser in an empty state.
  2. The + button couldn't create the first tab when no tabs existed (because ensure_browser_tab_state was called implicitly).
  3. Browser panel wouldn't automatically load the saved URL when opened - user had to press Enter or click Go.
  4. URL input was auto-filled with browser_last_url even when no tabs existed, causing confusion.
- Root cause:
  - `close_browser_tab()` called `ensure_browser_tab_state()` when the last tab was closed, immediately creating a replacement tab.
  - `add_browser_tab()` called `ensure_browser_tab_state()` before adding a new tab, which could create an implicit empty tab first.
  - `draw_browser_panel()` called `ensure_browser_tab_state()` every frame, making it impossible to have an empty browser state.
  - `draw_browser_panel()` initialized URL draft with `browser_last_url` unconditionally, regardless of whether tabs existed.
  - No logic existed to auto-create a tab with the saved URL when the browser panel opened.
- Resolution:
  - Removed `ensure_browser_tab_state()` call from `close_browser_tab()` when the last tab is closed.
  - Changed last-tab-close cleanup to properly remove all browser state maps (`active_browser_tab_by_scope`, `browser_tabs_by_scope`, `browser_url_draft_by_scope`) and shut down WebViews.
  - Removed `ensure_browser_tab_state()` call from `add_browser_tab()` - it now handles creating the first tab directly via `entry().or_default()`.
  - Removed `ensure_browser_tab_state()` call from `draw_browser_panel()`.
  - Added auto-creation logic in `draw_browser_panel()`: when browser opens with `browser_last_url` but no tabs exist, automatically creates first tab with that URL and triggers navigation.
  - Modified URL draft initialization to only use `browser_last_url` when tabs exist; when no tabs exist, URL input starts empty.
  - Removed `ensure_browser_tab_state()` calls from MCP tab handlers (`handle_browser_mcp_tabs_request`, `handle_browser_mcp_close_request`) as they're no longer needed.
- Prevent recurrence:
  - Updated `AGENTS.md` with new "Browser Tab Lifecycle Guidelines" section documenting:
    - Closing the last tab leaves the browser empty (no auto-recreate)
    - The (+) Add Tab button creates the first tab when none exist
    - Opening browser panel with saved URL auto-creates first tab
    - URL input is empty when no tabs exist
    - Explicit tab creation only (no implicit ensure_browser_tab_state calls)
    - Cleanup requirements on last tab close
  - Updated regression test: renamed `closing_last_browser_tab_recreates_empty_tab` to `closing_last_browser_tab_leaves_no_tabs` with inverted assertions.
  - Added new regression test `add_tab_creates_first_tab_when_no_tabs_exist` verifying + button works from empty state.
- Files/Commands touched: `src/app.rs` (close_browser_tab, add_browser_tab, draw_browser_panel, handle_browser_mcp_tabs_request, handle_browser_mcp_close_request, updated/added tests), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-08: "tek sekme varken kapattığımda kapanmıyor sekme kapansın ve artı ile açılabilsin ilk sekme ve browser açıldığında url varsa yüklesin ben ok tuşuna basmayayım"

---

#### Browser toolbar/tab hover no longer hides WebView content {#browser-toolbar-hover-fix}
- Date: 2026-05-07
- Context: Browser panel toolbar and tab strip hover interactions causing WebView flicker/black screen
- Error signature: User reported: "browserda toolbara gelince browser açık olan pencere siyah veya beyaz oluyor" — Hovering over the browser toolbar caused the WebView content area to turn black or white.
- Symptoms/Impact: When users hovered over browser toolbar buttons (Go, Clear, Design Inspect, Screenshot) or tab strip elements, the WebView content would disappear, showing a black or white empty area instead of the web page content.
- Root cause: The code was incorrectly treating toolbar/tab hover states as "overlay active" conditions that should hide the native WebView. The `browser_panel_overlay_active` flag was being set to `true` whenever any toolbar button or tab was hovered, causing `sync_embedded_browser()` to hide the entire WebView. This was unnecessary because egui tooltips render above the WebView layer without requiring the WebView to be hidden.
- Resolution:
  - Removed `any_tab_strip_hovered` tracking from tab strip rendering; hover no longer triggers overlay state.
  - Removed `go_hovered`, `clear_hovered`, `inspect_hovered` variable tracking from toolbar button rendering.
  - Removed the code that aggregated these hover states into `browser_panel_overlay_active`.
  - Updated the regression test `sync_embedded_browser_hides_while_hover_tooltip_active` → renamed to `sync_embedded_browser_does_not_hide_while_hover_tooltip_active` and inverted assertions to verify WebView remains visible during toolbar hover.
  - `browser_panel_overlay_active` is now only set by actual modal overlays (dropdown menus, context menus, popups), not by simple hover tooltips.
- Prevent recurrence:
  - Updated `AGENTS.md` Browser Panel WebView Z-Order Guidelines to clarify that toolbar hover should not hide WebView.
  - Added explicit comment in code: "Toolbar hover does NOT trigger WebView hide; toolbar buttons/tooltips render above WebView in egui layer without needing WebView to be hidden."
  - Regression test now verifies correct behavior (WebView stays visible during hover).
- Files/Commands touched: `src/app.rs` (removed hover tracking from `draw_browser_panel()`, updated test), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User bug report 2026-05-07: "browserda toolbara gelince browser açık olan pencere siyah veya beyaz oluyor araştır düzelt"

---

#### Foreground task menu button icon turns green when queue has items {#foreground-task-icon-green}
- Date: 2026-05-07
- Context: Terminal Manager foreground task queue menu button visual feedback
- Error signature: User requested: "foreground task içinde öge varsa beyaz yerine yeşil olsun, arka plan rengi değil kendi rengi" - The foreground task menu button should show a green icon when there are tasks in the queue, not just white.
- Symptoms/Impact: Users could not quickly distinguish whether a foreground terminal had pending tasks in its queue without opening the menu. The white icon looked the same regardless of queue state.
- Root cause: The `draw_terminal_foreground_message_menu_button()` function used a static white/normal color for the `icons::CHAT_TEXT` icon without checking if `foreground_messages` was empty.
- Resolution:
  - Added conditional icon color selection based on queue state:
    - If `foreground_messages.is_empty()`: use `with_alpha(TEXT_PRIMARY, 190)` (white/gray)
    - If queue has items: use `Color32::from_rgb(100, 200, 100)` (green)
  - Changed `ui.menu_button()` call to use `RichText::new(format!("{}", icons::CHAT_TEXT)).color(icon_color)` instead of plain `format!()` string.
  - Only the icon text color changes; button background remains unchanged as requested.
- Prevent recurrence:
  - Documented in AGENTS.md Terminal Manager section with explicit guideline: "Foreground task menu icon color indicates queue state."
  - Color constant `Color32::from_rgb(100, 200, 100)` is the same green used for other "active/completed" states in the codebase (e.g., `TurnComplete` pulse, checklist done state).
- Files/Commands touched: `src/app.rs` (`draw_terminal_foreground_message_menu_button()` icon color logic), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-07: "foreground task içinde öge varsa beyaz yerine yeşil olsun, arka plan rengi değil kendi rengi"

---

#### Design Inspect auto-disables after successful element delivery {#design-inspect-auto-disable}
- Date: 2026-05-07
- Context: Browser panel Design Inspect mode UX improvement
- Error signature: User requested that Design Inspect mode automatically turns off after an element is successfully sent to the terminal, to prevent accidental duplicate clicks.
- Symptoms/Impact: Previously, Design Inspect remained enabled after sending element info to the terminal, causing potential duplicate clicks if the user clicked the same element again.
- Root cause: Design Inspect mode was intentionally persistent (manual toggle on/off), but this led to accidental re-clicks before the user had a chance to disable it.
- Resolution:
  - Modified `forward_design_inspect_click_to_terminal()` in `src/app.rs` to automatically disable Design Inspect mode after `queue_pasted_text_to_terminal()` returns true (successful paste queue).
  - Updated status message from "Design inspect info sent to terminal" to "Design inspect info sent to terminal; design inspect disabled" for clearer user feedback.
  - The auto-disable behavior prevents duplicate deliveries because the second click will find `is_browser_design_inspect_enabled_for_scope()` returning false.
- Prevent recurrence:
  - Added regression test `design_inspect_auto_disables_after_successful_delivery` that verifies:
    - Design Inspect starts enabled
    - First click queues one paste and disables the mode
    - Second click (duplicate) does not produce a second paste because mode is now disabled
  - Updated `AGENTS.md` Browser Design Inspect Guidelines with the new rule: "Design Inspect auto-disables after successful delivery."
- Files/Commands touched: `src/app.rs` (auto-disable logic in `forward_design_inspect_click_to_terminal()`, new regression test), `AGENTS.md` (updated Design Inspect guidelines), `KNOWN_ISSUES.md`
- References: User request 2026-05-07: "design mode da bir tane öge terminale yazıldıktan sonra design mode kapansın"

---

#### Browser MCP multi-terminal isolation for same-project sessions {#browser-mcp-terminal-isolation}
- Date: 2026-05-07
- Context: Browser MCP with multiple terminals in the same project using the browser simultaneously
- Error signature: Multiple AI agents in the same project sharing the same browser instance, causing conflicts when navigating, clicking, or inspecting elements. Terminal-scoped sessions were not properly isolated.
- Symptoms/Impact: When two or more terminals in the same project used Browser MCP simultaneously, they would interfere with each other's browser state (cookies, localStorage, session), causing navigation commands to affect the wrong browser session and producing inconsistent results.
- Root cause:
  - Browser state maps (`embedded_browsers_by_project`, `browser_tabs_by_project`, `browser_url_draft_by_project`, etc.) used only `project_id: u64` as the key.
  - All terminals in the same project shared a single browser instance, WebView2 profile, and session state.
  - The `BrowserMcpAuthScope` had `terminal_id` and `session_id` fields, but `resolve_browser_mcp_project_id()` only returned `project_id`, collapsing all terminal scopes to the project level.
  - Terminal link routing, browser panel rendering, and MCP handlers all used project-scoped browser lookups.
- Resolution:
  - Introduced `BrowserScopeKey` enum with `Project(u64)` and `Terminal { project_id, terminal_id }` variants to distinguish project-wide vs terminal-specific browser instances.
  - Updated all browser state maps to use `BrowserScopeKey` instead of raw `u64`:
    - `browser_url_draft_by_scope`
    - `embedded_browsers_by_scope`
    - `browser_tabs_by_scope`
    - `active_browser_tab_by_scope`
    - `browser_design_inspect_enabled_scopes`
    - `browser_design_inspect_terminal_by_scope`
    - `browser_video_recordings_by_scope`
  - Updated `inactive_browser_tab_browsers` key from `(u64, u64)` to `(BrowserScopeKey, u64)`.
  - Added `browser_user_data_dir_path_for_terminal(project_id, terminal_id)` in `config.rs` for isolated WebView2 profiles per terminal (`webview2/projects/{project_id}/terminals/{terminal_id}/`).
  - Changed `resolve_browser_mcp_project_id()` to `resolve_browser_mcp_scope()` returning `BrowserScopeKey::Terminal` with session ID validation.
  - Updated `project_browser()` to `browser_for_scope()` accepting `BrowserScopeKey` and creating terminal-scoped browser instances with isolated profiles.
  - Updated `sync_embedded_browser()` to show/hide based on active terminal scope, falling back to project scope for UI-initiated browser usage.
  - Updated `draw_browser_panel()` to render terminal-specific browser when active terminal has a terminal-scoped browser open.
  - Added terminal browser cleanup on terminal close to prevent resource leaks.
  - Updated all MCP handlers to use terminal-scoped browser lookups via `resolve_browser_mcp_scope()`.
  - Project-scoped browsers (`BrowserScopeKey::Project`) retained for UI-initiated navigation (terminal links, manual browser panel open).
  - Terminal-scoped browser URLs are runtime-only (not persisted to `ProjectRecord::browser_last_url`).
- Prevent recurrence:
  - Added `BrowserMcpMultiTerminalIsolation` architecture documentation in `AGENTS.md`.
  - All browser state maps now use `BrowserScopeKey` making scope explicit at the type level.
  - Session ID validation in `resolve_browser_mcp_scope()` prevents cross-session contamination.
  - Terminal browser cleanup ensures resources are freed when terminals exit.
  - Regression tests verify terminal-scoped browser creation, isolated profile directories, and proper cleanup.
- Files/Commands touched: `src/app.rs` (BrowserScopeKey enum, state map updates, scope resolution, browser lifecycle, cleanup, UI rendering, MCP handlers), `src/config.rs` (terminal-scoped profile paths), `AGENTS.md` (new guidelines section), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: Browser MCP multi-terminal isolation implementation 2026-05-07

---

#### Settings panel scroll wheel not working {#settings-scroll-wheel}
- Date: 2026-05-06
- Context: Settings popup and Shortcuts section scroll behavior with mouse wheel
- Error signature: User reported that mouse scroll wheel did not work in Settings panel (including Shortcuts section), but dragging the scrollbar manually worked.
- Symptoms/Impact: Users could not scroll through Settings content using mouse wheel; only scrollbar drag worked. This affected all Settings sections with overflow content.
- Root cause:
  - Terminal output mouse wheel handling code was consuming `smooth_scroll_delta` from the global input state before Settings popup's ScrollArea could process it.
  - The terminal wheel handling code runs inside `draw_terminal_pane()` which is called for all visible terminals even when UI overlays (Settings, exit confirm, etc.) are open.
  - When the terminal has mouse reporting enabled (especially with OpenCode active), it would consume wheel events that should have been handled by the overlay's ScrollArea.
- Resolution:
  - Added `terminal_output_mouse_wheel_enabled()` helper function that checks if any UI overlay is open (Settings popup, exit confirm popup, terminal history popup, foreground message popup).
  - Modified `draw_terminal_pane()` to skip terminal wheel handling when any overlay is active, allowing wheel events to reach the overlay's ScrollArea instead.
  - The guard is applied both inside the terminal ScrollArea closure (for immediate wheel capture) and after the ScrollArea (for OpenCode fallback handling).
  - Added 6 regression tests covering all overlay scenarios:
    - `terminal_output_mouse_wheel_enabled_returns_true_when_no_overlays`
    - `terminal_output_mouse_wheel_enabled_returns_false_when_settings_open`
    - `terminal_output_mouse_wheel_enabled_returns_false_when_exit_confirm_open`
    - `terminal_output_mouse_wheel_enabled_returns_false_when_terminal_history_open`
    - `terminal_output_mouse_wheel_enabled_returns_false_when_foreground_message_open`
    - `terminal_output_mouse_wheel_enabled_returns_false_when_multiple_overlays`
- Prevent recurrence:
  - The regression tests ensure the helper function correctly returns false for each overlay type and true when no overlays are open.
  - Future UI overlays should be added to `terminal_output_mouse_wheel_enabled()` check.
  - The pattern is consistent with `embedded_browser_should_yield_to_ui_layer()` which already handles similar overlay detection.
- Files/Commands touched: `src/app.rs` (new helper function, wheel handling guard in `draw_terminal_pane()`, 6 regression tests), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "settingsde scroll çalışmıyor shortcuts da denedim Mouse scrollu tekerlek çalışmadı kendim yandaki scrollu tutup çektiğimde geliyor"

---

#### Terminal shortcuts send double Enter for confirmation {#terminal-shortcut-double-enter}
- Date: 2026-05-06
- Context: Terminal command shortcuts (F5, F6, F7, F11) for AI CLI tools
- Error signature: User requested that all terminal shortcuts send the command followed by two Enter presses, with a short delay between them, to ensure commands are confirmed and processed.
- Symptoms/Impact: AI CLI tools like Codex, OpenCode, and Factory Droid sometimes require explicit confirmation after receiving a command. A single Enter might not always trigger immediate execution.
- Root cause:
  - Terminal shortcuts previously sent only one Enter after the command.
  - Some AI CLI tools or shell prompts require an additional Enter press to confirm the action.
- Resolution:
  - Added `SHORTCUT_SECOND_ENTER_DELAY_MS` constant (50ms) to define the delay between Enter presses.
  - Added `pending_shortcut_second_enter: Vec<(u64, Instant)>` field to `AdeApp` to track scheduled second Enter presses per terminal.
  - Modified `execute_terminal_shortcut()` to schedule a second Enter after successfully sending the first command+Enter.
  - Added `process_pending_shortcut_second_enters()` function to handle the delayed second Enter sends in the update loop.
  - Called the new processor in `update()` after `process_pending_reruns()` (Phase 3c).
  - Initialized the new field in both production and test app constructors.
  - Added regression test `handle_shortcuts_sends_double_enter_with_delay` to verify the behavior.
- Prevent recurrence:
  - Added regression test that verifies initial output contains `command + \r` and total output after delay contains `command + \r\r`.
  - Updated `AGENTS.md` Terminal Shortcut Guidelines with the new rule about double Enter.
- Files/Commands touched: `src/app.rs` (constants, struct, methods, test), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "shortcuts da hepsinde yazdıktan sonra 2 kere entera tıklasın enter tıklamaları arasında çok az beklesin"

---

#### Terminal shortcut keys reorganized for better workflow {#terminal-shortcut-reorganization}
- Date: 2026-05-06
- Context: Default terminal command shortcuts for AI CLI tools
- Error signature: User requested changes to default shortcut keys: F7 -> /implement-plan moved to F11, F8 -> /review-guard moved to F7, and /gt labeled as "GitHub Push" instead of "Semgrep Check".
- Symptoms/Impact: Existing users with old configs would have outdated shortcut key bindings that don't match the new defaults.
- Root cause:
  - Old default shortcuts had `implement-plan` on F7 and `review-guard` on F8.
  - The `/gt` command was labeled as "Semgrep Check" but user wanted "GitHub Push".
- Resolution:
  - Changed default shortcuts:
    - F5 -> /gt (GitHub Push) - label changed from "Semgrep Check"
    - F6 -> /prepare-fix-plan (unchanged)
    - F11 -> /implement-plan (moved from F7)
    - F7 -> /review-guard (moved from F8)
  - Added `migrate_legacy_shortcut()` function to automatically migrate old configs:
    - ID "semgrep-check" migrated to "github-push" with label update
    - implement-plan key F7 migrated to F11
    - review-guard key F8 migrated to F7
  - User customizations are preserved: if user changed key from old default, migration is skipped for that shortcut.
  - Updated shortcut ID from "semgrep-check" to "github-push" for consistency.
- Prevent recurrence:
  - Added tests `normalize_terminal_shortcuts_migrates_legacy_defaults` and `normalize_terminal_shortcuts_preserves_user_customizations_during_migration`.
  - Updated `AGENTS.md` shortcut guidelines with new defaults.
- Files/Commands touched: `src/models.rs` (defaults and migration), `src/config.rs` (tests), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "shortcutda f7 yi f11 e taşı, f8 i de f7 ye taşı ve /gt için github push adını ver"

---

#### Browser panel dropdown menus hidden behind native WebView {#browser-dropdown-webview-z-order}
- Date: 2026-05-06
- Context: Browser panel screenshot dropdown button and other toolbar menus
- Error signature: Clicking the screenshot camera icon opened a dropdown menu, but it appeared behind the embedded browser content and could not be clicked.
- Symptoms/Impact: Users could not access dropdown menus in the browser panel toolbar because the native WebView was rendering on top of the egui menu.
- Root cause:
  - Native WebView2 renders as a child window above egui's immediate-mode rendering.
  - The `menu_button` dropdown is tracked via `BarState` (not `Memory.popup`), so `ctx.memory().any_popup_open()` did not detect it.
  - The existing `should_hide_embedded_browser_for_ui_layer()` function relied on `any_popup_open()`, missing menu dropdowns.
- Resolution:
  - Added `browser_panel_dropdown_open: bool` runtime flag to `AdeApp`.
  - Modified `draw_browser_screenshot_button()` to return `bool` indicating if menu is open (via `InnerResponse.inner.is_some()`).
  - Reset `browser_panel_dropdown_open` at the start of `draw_browser_panel()` each frame.
  - Set `browser_panel_dropdown_open = true` when any dropdown is open.
  - Updated `embedded_browser_should_yield_to_ui_layer()` to accept and check the new `browser_dropdown_open` parameter.
  - Updated `should_hide_embedded_browser_for_ui_layer()` to pass the flag.
  - When `browser_panel_dropdown_open` is true, `sync_embedded_browser()` hides the WebView via `browser.hide()`, allowing the menu to appear on top.
- Prevent recurrence:
  - Added test `sync_embedded_browser_hides_while_dropdown_open` verifying the browser is hidden when `browser_panel_dropdown_open` is true.
  - Updated existing test `embedded_browser_yields_to_ui_overlay_layers` to include the new parameter.
- Files/Commands touched: `src/app.rs` (AdeApp struct, `draw_browser_panel()`, `draw_browser_screenshot_button()`, `embedded_browser_should_yield_to_ui_layer()`, `should_hide_embedded_browser_for_ui_layer()`, `sync_embedded_browser()`, tests), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "ekran görüntüsü almak için tıklıyorum dropdown browserın arkasında kalıyor ve tıklanmıyor düzelt"

#### Browser panel hover tooltips hidden behind native WebView {#browser-tooltip-webview-z-order}
- Date: 2026-05-07
- Context: Browser panel toolbar buttons (Go, Clear, Design Inspect, Screenshot, tabs, Add tab) showing hover tooltips
- Error signature: Hovering over browser toolbar buttons showed tooltips (e.g., "Take screenshot", "Go to URL", "Close tab") behind the embedded browser content, making them unreadable.
- Symptoms/Impact: Users could not see hover tooltips on browser panel toolbar buttons because the native WebView was rendering on top of egui tooltips. This affected all toolbar buttons including screenshot dropdown, URL actions, Design Inspect toggle, and tab controls.
- Root cause:
  - Native WebView2 renders as a child window above egui's immediate-mode rendering.
  - Hover tooltips from `on_hover_text()` are rendered via egui's tooltip system, appearing below the WebView z-order.
  - The existing `browser_panel_dropdown_open` flag only tracked dropdown menu state, not hover tooltip state.
  - Toolbar buttons like Go, Clear URL, and Screenshot use `on_hover_text()` which displays tooltips that get obscured.
- Resolution:
  - Renamed `browser_panel_dropdown_open` to `browser_panel_overlay_active` to reflect broader purpose covering dropdowns, tooltips, and other overlays.
  - Created `styled_icon_button_response()` helper that returns full `Response` (instead of just `bool`) to enable hover detection.
  - Modified toolbar buttons (Go, Clear URL) to use the new helper and capture hover state.
  - Design Inspect button already returned `Response` from `activity_rail_icon_button()`, so hover tracking was added directly.
  - Modified `draw_browser_screenshot_button()` to return `(menu_open, hovered)` tuple tracking both dropdown and hover states.
  - Updated tab strip rendering to track hover on tabs, close buttons, and Add tab button via `any_tab_strip_hovered` flag.
  - Aggregated all hover states in `browser_panel_overlay_active` flag within `draw_browser_panel()`.
  - Updated `embedded_browser_should_yield_to_ui_layer()` to use the renamed parameter `browser_overlay_active`.
  - Updated `should_hide_embedded_browser_for_ui_layer()` to pass the renamed field.
  - When `browser_panel_overlay_active` is true (dropdown OR hover), `sync_embedded_browser()` hides the WebView, allowing tooltips and menus to appear on top.
- Prevent recurrence:
  - Added test `sync_embedded_browser_hides_while_hover_tooltip_active` verifying browser is hidden when overlay flag is true (simulating hover state).
  - Updated existing test `sync_embedded_browser_hides_while_dropdown_open` to use renamed field.
  - Updated existing test `embedded_browser_yields_to_ui_overlay_layers` with renamed parameter and updated comments.
- Files/Commands touched: `src/app.rs` (AdeApp struct field rename, new `styled_icon_button_response()` helper, modified `draw_browser_panel()`, `draw_browser_screenshot_button()` signature change, tab strip hover tracking, `embedded_browser_should_yield_to_ui_layer()` parameter rename, `should_hide_embedded_browser_for_ui_layer()` update, new and updated tests), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-07: "browserın altında kalıyor açılan dropdownlar hoverlar"

---

#### Browser toolbar hover tooltips now appear above buttons {#browser-toolbar-tooltip-above}
- Date: 2026-05-08
- Context: Browser panel toolbar buttons (Go, Clear, Design Inspect, Screenshot) hover tooltip positioning
- Error signature: User reported: "browserda toolbarda üstüne gelince hover geliyor ya o gelen hoverlar altta değil üstte çıksın" — Hover tooltips on browser toolbar buttons were appearing below the buttons, potentially overlapping the WebView content area.
- Symptoms/Impact: Hover tooltips appeared below toolbar buttons by default (egui standard behavior), causing them to extend into the WebView area where they could be obscured or cause visual issues.
- Root cause:
  - Standard egui `on_hover_text()` positions tooltips below widgets by default.
  - The WebView renders as a native child window above egui, so any tooltip overlapping the WebView area gets obscured.
  - Previous fix (#browser-tooltip-webview-z-order) addressed this by hiding the WebView during hover, but this caused flicker and unnecessary WebView toggling.
- Resolution (permanent fix):
  - Changed approach: instead of hiding WebView on hover, position tooltips above the buttons so they never overlap the WebView content area.
  - Added `BROWSER_TOOLBAR_TOOLTIP_GAP` constant (4px) to control spacing between button and tooltip.
  - Created `browser_toolbar_icon_button()` helper that shows tooltips above using `egui::containers::show_tooltip_at()` with `rect.center_top()` as anchor.
  - Created `browser_toolbar_toggle_button()` for toggle-style buttons (Design Inspect) with above-tooltip behavior.
  - Created `show_tooltip_above()` utility for general use with custom buttons.
  - Updated toolbar buttons:
    - Go button: changed from `styled_icon_button_response()` to `browser_toolbar_icon_button()`
    - Clear URL button: same change
    - Design Inspect button: changed from `activity_rail_icon_button()` to `browser_toolbar_toggle_button()`
    - Screenshot buttons: use `show_tooltip_above()` with custom button rendering
  - Tooltips now render entirely within the egui layer above the WebView, eliminating z-order conflicts.
- Prevent recurrence:
  - Updated `AGENTS.md` Browser Panel WebView Z-Order Guidelines with new rule: "Toolbar tooltips must appear above buttons."
  - Added helpers to codebase: `browser_toolbar_icon_button()`, `browser_toolbar_toggle_button()`, `show_tooltip_above()`.
  - Removed reliance on hover-based WebView hiding for tooltips (kept only for actual modal overlays like dropdowns, menus).
- Files/Commands touched: `src/app.rs` (new constants, new helper functions, updated button calls in `draw_browser_panel()` and `draw_browser_screenshot_buttons()`), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo build`
- References: User request 2026-05-08: "browserda toolbarda üstüne gelince hover geliyor ya o gelen hoverlar altta değil üstte çıksın"

---

#### Browser URL input context menu and double-click select all {#browser-url-context-menu}
- Date: 2026-05-06
- Context: Browser panel URL input field - user requested right-click Copy/Paste and double-click to select all text
- Error signature: Browser URL input had no context menu (right-click did nothing) and double-click did not select the entire URL.
- Symptoms/Impact: Users could not copy or paste URLs via context menu, and selecting the entire URL required manual text selection.
- Root cause:
  - The URL TextEdit in draw_browser_panel() used ui.add() directly without context menu handling.
  - No double-click detection was implemented for the URL input.
  - Context menus for text inputs require careful handling of selection state and borrow issues.
- Resolution:
  - Added context menu with Copy/Paste buttons using with_minimal_button_chrome() styling.
  - Copy button copies selected text (or entire URL if no selection) to clipboard.
  - Paste button inserts clipboard text at cursor position, or replaces selected text.
  - Double-click on URL input selects all text using response.double_clicked() and CCursorRange.
  - Used TextEditState to set character selection range (Unicode-safe using char indices, not byte indices).
  - Deferred copy/paste actions outside the context menu closure to avoid Rust borrow issues.
  - Added extract_text_from_char_range() helper for Unicode-safe text extraction.
- Prevent recurrence:
  - Added 7 regression tests for the new functionality.
- Files/Commands touched: src/app.rs (URL input section, new helper function), KNOWN_ISSUES.md, cargo fmt, cargo test
- References: User request 2026-05-06: browser linkinde sag tik kopyala gelmiyor kopyala ve yapistir gelsin sag tikta ve cift tiklayinca tum linki secsin

(End of file - total 3872 lines)

---

#### Browser panel compact UI - vertical space optimization {#browser-panel-compact-ui}
- Date: 2026-05-06
- Context: Browser panel UX - user requested more vertical space for the web view
- Error signature: The browser panel header, project name, wrapping tabs, URL input, and action buttons were consuming ~170-190px of vertical space, leaving less room for the actual embedded browser content.
- Symptoms/Impact: On smaller screens, the web view was cramped; tabs could wrap to multiple lines taking even more space; browser panel felt cluttered with excessive chrome.
- Root cause:
  - Separate header row with "Browser" title and separator.
  - Separate project name display row.
  - Tabs used `horizontal_wrapped` which could consume multiple rows when the panel was narrow.
  - URL input was full-width on its own row.
  - Action buttons (Go, Clear, Design Inspect, Screenshot) were on a separate row.
  - Inner margins were 10px on all sides, and spacing between elements was 8-16px.
- Resolution:
  - Removed separate "Browser" header and project name rows - the active project is implicit via the activity rail and terminal context.
  - Changed tabs from `horizontal_wrapped` to single-row `horizontal` with `ScrollArea::horizontal()` for overflow handling - tabs never wrap, always stay on one line.
  - Combined URL input and all action buttons into a single compact toolbar row:
    - URL input takes available width minus button area.
    - Go (arrow), Clear (trash), Design Inspect (eye), and Screenshot (camera) buttons are inline after the URL field.
  - Reduced inner margins from 10px to 6px symmetric.
  - Reduced spacing between UI elements from 8-16px to 4-6px.
  - Reduced tab height from 26px to 22px, tab close size from 18px to 16px, and adjusted padding/margins proportionally.
  - Reduced font size in tabs from 12px to 11px.
  - Vertical space saved: approximately 110-130px (from ~170-190px to ~60-80px of chrome).
- Prevent recurrence:
  - The compact layout should be tested with 5 tabs (max) to ensure horizontal scrolling works properly.
  - Test with very narrow panel widths to verify the URL field remains usable (minimum 100px).
  - Updated test `browser_tab_close_rect_stays_inside_tab_top_right` to use dynamic top calculation based on centered close button.
- Files/Commands touched: `src/app.rs` (constants BROWSER_TAB_HEIGHT, BROWSER_TAB_CLOSE_SIZE, BROWSER_TAB_CLOSE_MARGIN, BROWSER_TAB_LABEL_LEFT_PADDING, BROWSER_TAB_LABEL_RIGHT_GAP; `draw_browser_panel()` function), `KNOWN_ISSUES.md`
- References: User request 2026-05-06: browser kismi dikeyde browser disindaki ogeler linkler butonlar filan cok yer kapliyor orayi modifiye etmeni istiyorum amacim browserin dikeyde daha cok yere yayilmasi ux odakli ilerle

(End of file - total 3872 lines)

---

#### Terminal Manager foreground/background saved messages separation {#terminal-manager-saved-messages-split}
- Date: 2026-05-06
- Context: Terminal Manager saved messages feature - user requested different behavior for foreground vs background terminals
- Error signature: User wanted saved messages to work differently for foreground (dynamic task queue) vs background (reusable snippets) terminals. Foreground messages should be removed from queue when sent, and have add/edit/delete functionality.
- Symptoms/Impact: Previously both foreground and background terminals used the same saved messages from `ProjectRecord::saved_messages`, which didn'"'"'t support the dynamic queue workflow the user wanted for foreground terminals.
- Root cause: Single `saved_messages` field was used for both terminal kinds without distinguishing their different use cases.
- Resolution:
  - Added new `ProjectRecord::foreground_saved_messages: Vec<String>` field for foreground task queue.
  - Existing `ProjectRecord::saved_messages` now used exclusively for background terminals (reusable snippets).
  - Terminal Manager shows different message menus based on terminal kind:
    - Background: traditional saved messages menu (unchanged behavior)
    - Foreground: task queue with send (removes from queue), edit, delete actions per message
  - Added "+ Add New" button at bottom of foreground menu to open popup for adding tasks.
  - Created `draw_foreground_message_popup()` with multiline `TextEdit::multiline` input, supporting both add and edit modes.
  - Popup uses `Order::Foreground` layer to render above other UI elements (same as Settings popup).
  - Focus automatically set to text input when popup opens; terminal keyboard capture disabled while popup is open.
  - Edit mode pre-fills text input with existing message; Save button updates in place.
  - Delete button (red) available in edit mode for removing tasks.
  - Cancel button closes popup without saving.
  - Empty or whitespace-only messages are rejected (not saved).
  - Multi-line content preserved for complex commands/prompts.
  - When foreground message is sent to terminal, it'"'"'s automatically removed from queue (depleted task list behavior).
  - Added popup state fields to `AdeApp`: `foreground_message_popup_open: Option<u64>`, `foreground_message_popup_editing_index: Option<usize>`, `foreground_message_popup_draft: String`.
  - Updated `embedded_browser_should_yield_to_ui_layer()` to accept new `foreground_message_popup_open` parameter to hide native WebView while popup is open.
  - Updated `text_input_has_focus_extended()` to return true when foreground popup is open.
  - Updated `surrender_ui_text_focus()` to clear foreground message input focus.
  - Added constant `FOREGROUND_MESSAGE_INPUT_ID` for input focus management.
  - Config persistence: foreground messages saved to TOML config in `ProjectRecord::foreground_saved_messages`.
  - Config migration: legacy configs without the field default to empty queue; recovery merge logic updated to preserve foreground messages.
  - Updated `test_project()` helper and all test fixtures to include new field.
  - Updated `embedded_browser_yields_to_ui_overlay_layers` test to include 8th parameter.
- Prevent recurrence:
  - Added tests for foreground message popup in overlay yield logic.
  - Test config roundtrip with foreground messages to ensure persistence works.
  - Test that sending foreground message removes it from queue.
  - Test add/edit/delete operations via popup.
- Files/Commands touched: `src/models.rs` (ProjectRecord), `src/config.rs` (migration), `src/app.rs` (state fields, popup drawing, Terminal Manager UI, focus handling, tests), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "terminal managerda her terminal icin send saved messages var ya foreground da farkli background da farkli calismasini istiyorum... foreground daha dinamik olacak... popup acilinca focus inputta olmali... yine buradaki mesajlar da proje bazli kayit olacak"

---

#### Shortcut second Enter delay increased from 50ms to 250ms {#shortcut-second-enter-delay-increase}
- Date: 2026-05-07
- Context: Terminal command shortcuts (F5, F6, F7, F11) sending double Enter for AI CLI confirmation
- Error signature: User reported that the second Enter press was not working properly with the 50ms delay, requiring a longer wait time for reliable command confirmation.
- Symptoms/Impact: AI CLI tools like Codex, OpenCode, and Factory Droid were not receiving the confirmation Enter consistently, causing commands to hang or require manual Enter press by user.
- Root cause:
  - The original 50ms delay between first and second Enter was too short for slower terminals or AI CLI tools that need more processing time before accepting the confirmation.
  - Some shell environments and AI CLI interactive prompts require additional settle time before processing subsequent Enter keys.
- Resolution:
  - Increased `SHORTCUT_SECOND_ENTER_DELAY_MS` constant from 50ms to 250ms in `src/app.rs`.
  - Updated test comment in `handle_shortcuts_sends_double_enter_with_delay` to reflect new 250ms duration.
  - Updated `AGENTS.md` Terminal Shortcut Guidelines to document 250ms delay instead of 50ms.
- Prevent recurrence:
  - The longer 250ms delay provides sufficient buffer for various terminal speeds and AI CLI response times.
  - Regression test `handle_shortcuts_sends_double_enter_with_delay` continues to verify double Enter behavior.
- Files/Commands touched: `src/app.rs` (constant, test comment), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-07: "shortcutda 2 kere enter çalışmıyor bekleme süresini arttır"

---

#### Browser panel add tab button not visible {#browser-add-tab-button}
- Date: 2026-05-07
- Context: Browser panel tab strip UI with 5-tab support and add button
- Error signature: User reported: "browser 5 sekme destekliyor ama yeni sekme aç butonu yok ekle" - The add tab (+) button was not visible in the browser panel despite supporting up to 5 tabs.
- Symptoms/Impact: Users could not add new browser tabs via the UI; the button was being pushed off-screen or hidden by the horizontal ScrollArea for tabs.
- Root cause:
  - The `ScrollArea::horizontal()` for tabs was consuming all available width in the horizontal layout, leaving no space for the add button.
  - The add button was rendered after the ScrollArea but was clipped out of view due to the layout flow.
- Resolution:
  - Changed layout to reserve fixed width for the add button before allocating space for the ScrollArea.
  - Added `BROWSER_ADD_TAB_BUTTON_WIDTH` constant (28px) for the button area.
  - Calculated `scroll_width = available_width - BROWSER_ADD_TAB_BUTTON_WIDTH - 4px` to ensure scrollable tabs only take remaining space.
  - Used `ui.allocate_ui_with_layout()` to explicitly size the ScrollArea container.
  - Changed button from `ui.add_enabled()` to `ui.add_sized()` with fixed dimensions for consistent sizing.
  - Increased plus icon size from 12px to 14px for better visibility.
  - Added `&& can_add_tab` check on click handler to prevent tab creation when limit is reached.
- Prevent recurrence:
  - Updated AGENTS.md Browser Panel Compact UI Guidelines with new rule: "Reserve fixed width for add tab button in tab strip layout."
- Files/Commands touched: `src/app.rs` (`draw_browser_panel()` tab strip layout, constants), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-07: "browser 5 sekme destekliyor ama yeni sekme aç butonu yok ekle"


---

#### Browser panel white screen flicker during scroll {#browser-scroll-white-flicker}
- Date: 2026-05-07
- Context: Embedded WebView2 browser panel showing white/blank areas during mouse wheel scroll
- Error signature: User reported: "scroll yapıyorum bazen böyle beyaz ekran filan geliyor" - White screen/blank areas appear randomly during scrolling in the browser panel.
- Symptoms/Impact: During scroll operations, the browser content would flicker with white/blank areas, making the page content temporarily invisible and creating a jarring user experience.
- Root cause:
  - sync_embedded_browser() was calling rowser.show() and rowser.sync_position() every UI frame (60+ times per second).
  - WebView2 native SetIsVisible(true) and SetBounds() were being invoked repeatedly even when visibility and bounds hadn't changed.
  - This caused the WebView2 child window to invalidate and repaint unnecessarily during scroll, producing white flicker artifacts.
  - Additionally, bounds were synced AFTER show() was called, meaning the browser could become visible at wrong/old dimensions for a brief moment.
- Resolution:
  - Added cached_visible: Option<bool> and cached_bounds: Option<BrowserBounds> fields to EmbeddedBrowser struct to track last applied native state.
  - Modified set_visible_internal() to skip SetIsVisible() calls when visibility hasn't changed.
  - Modified sync_position_internal() to skip SetBounds() calls when bounds haven't changed.
  - Changed sync_embedded_browser() to call sync_position() BEFORE show(), ensuring correct bounds are set before the browser becomes visible.
  - Reset cached state in shutdown() to ensure clean state on browser restart.
- Prevent recurrence:
  - Added regression test sync_embedded_browser_syncs_bounds_before_show verifying bounds are set before visibility.
  - The idempotent native sync prevents redundant COM calls to WebView2, reducing both flicker and CPU overhead.
- Files/Commands touched: src/web_browser.rs (struct fields, set_visible_internal(), sync_position_internal(), shutdown()), src/app.rs (sync_embedded_browser() order), KNOWN_ISSUES.md, cargo fmt, cargo test, cargo build --release --target x86_64-pc-windows-msvc`n- References: User request 2026-05-07: "scroll yapıyorum bazen böyle beyaz ekran filan geliyor"


---

#### Browser URL input custom context menu removed {#browser-url-context-menu-removed}
- Date: 2026-05-07
- Context: Browser panel URL input field right-click context menu
- Error signature: User requested removal of the custom Copy/Paste context menu that appeared when right-clicking the browser URL input field.
- Symptoms/Impact: The custom context menu with Copy and Paste buttons was considered unnecessary since standard keyboard shortcuts (Ctrl+C, Ctrl+V) and system context menus are available.
- Resolution:
  - Removed the custom `url_response.context_menu()` block that rendered Copy/Paste buttons.
  - Removed associated state tracking: `pre_click_range`, `copy_requested`, `paste_requested`, `context_menu_range`, `post_show_range`, `effective_range`, `url_for_copy`, `can_paste`.
  - Removed the Copy/Paste action handlers after the UI borrow ends.
  - Standard TextEdit behavior preserved: typing, Enter to submit, double-click to select all.
- Prevent recurrence:
  - If custom context menus are needed in the future, prefer native egui menus over custom-styled ones for consistency.
  - Document intentional UX decisions in AGENTS.md.
- Files/Commands touched: `src/app.rs` (`draw_browser_panel()` URL input block), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-07: "browserda link sag tik menusunu kaldir" (interpreted as removing the URL input context menu, not WebView page menus)


---

#### Browser add tab button positioned next to last tab {#browser-add-tab-button-position}
- Date: 2026-05-07
- Context: Browser panel tab strip add tab (+) button placement
- Error signature: User requested: "yeni sekme butonu en sonuncu sekmenin hemen yaninda olsun" - The add tab button should appear immediately after the last tab, not in a separate fixed area.
- Symptoms/Impact: The add tab button was positioned in a fixed area to the right of the scrollable tabs, visually disconnected from the tab strip when tabs overflowed.
- Resolution:
  - Moved the add tab button from outside the `ScrollArea` to inside the `ScrollArea::horizontal()` block.
  - Button is now rendered immediately after the tabs loop, within the same `ui.horizontal()` container.
  - Removed the fixed width reservation calculation (`available_width - add_button_width - spacing`).
  - Button now scrolls with the tabs when the tab strip overflows.
  - Button remains visible at the end of the tab list, maintaining visual proximity to the last tab.
- Prevent recurrence:
  - Updated AGENTS.md guideline: "Place add tab button inside ScrollArea next to last tab" instead of reserving fixed width outside.
  - This pattern keeps related UI controls together and follows egui's scrollable container conventions.
- Files/Commands touched: `src/app.rs` (`draw_browser_panel()` tab strip layout), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-07: "yeni sekme butonu en sonuncu sekmenin hemen yaninda olsun"


---

#### OpenCode baseline binary segfault on Windows {#opencode-baseline-binary-segfault}
- Date: 2026-05-08
- Context: OpenCode launched from a Mergen-managed Windows terminal
- Error signature: `Bun v1.3.13 ... opencode-windows-x64-baseline ... panic(main thread): Segmentation fault`
- Symptoms/Impact: Running `opencode` exited immediately before the OpenCode UI started, even after a PC restart.
- Root cause:
  - Mergen always injected `OPENCODE_BIN_PATH` pointing at `opencode-windows-x64-baseline` as an older AVX2 workaround.
  - On the affected AVX2-capable machine, `opencode-windows-x64` works while `opencode-windows-x64-baseline` crashes.
  - Because the env var was forced, OpenCode's npm shim never got a chance to choose the working binary.
- Resolution:
  - Replaced the unconditional baseline path with dynamic OpenCode binary resolution.
  - On Windows + AVX2, Mergen now prefers the standard `opencode-windows-x64` binary when it exists.
  - If a previous Mergen terminal inherited a baseline `OPENCODE_BIN_PATH`, Mergen replaces it with the sibling standard binary on AVX2 machines.
  - Custom non-baseline `OPENCODE_BIN_PATH` values are preserved, and non-AVX2 machines still prefer baseline.
  - Added regression tests for inherited baseline replacement and fallback behavior.
- Files/Commands touched: `src/opencode.rs`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request 2026-05-08: OpenCode crash output showing `opencode-windows-x64-baseline` Bun segmentation fault.

---

#### Browser MCP terminal-scoped browser now visible in panel {#browser-mcp-terminal-scope-visible}
- Date: 2026-05-08
- Context: AI (OpenCode/Codex) used Browser MCP to navigate/interact with pages, but user saw no changes in Mergen's browser panel
- Error signature: User reported: "mergen browser mcp şuan çalışamıyor mesela çalışıyor bağlı bir sayfayı güncellemesi gerekiyor ai devam ediyor ama ben mergende hiçbir değişiklik görmüyorum browserda" — Browser MCP commands were running successfully for the AI, but the Mergen UI browser panel did not reflect any navigation or page updates.
- Symptoms/Impact:
  1. AI sends `browser_navigate` or `browser_click` via MCP; command succeeds and WebView2 navigates.
  2. User sees the browser panel still showing the old project-scoped page, or an empty state.
  3. Terminal-scoped browser state (tabs, URL) was isolated but invisible in the UI.
  4. User cannot see what the AI is doing in the browser.
- Root cause:
  - MCP commands correctly created `BrowserScopeKey::Terminal` scoped browser instances for isolation.
  - However, `draw_browser_panel()` and `sync_embedded_browser()` were hardcoded to use `BrowserScopeKey::Project(project_id)` as the visible scope.
  - The terminal-scoped browser existed and processed commands, but the UI always rendered the project-scoped browser (or nothing if no project browser existed).
  - This created a hidden browser that the AI controlled while the user saw an unrelated or empty browser panel.
- Resolution:
  - Introduced `active_browser_scope()` helper that returns the terminal scope when the active terminal has browser tabs, otherwise falls back to project scope.
  - Updated `draw_browser_panel()` to use `active_browser_scope()` for tab rendering, URL input, toolbar actions, and auto-create logic.
  - Updated `sync_embedded_browser()` to hide all non-active scopes and show/sync only the active scope's WebView2.
  - Guarded project URL persistence so terminal-scoped URL changes do not write to `ProjectRecord::browser_last_url`.
  - Prevented auto-creation of a project URL tab inside a terminal-scoped browser (terminal URLs are runtime-only).
  - Updated `submit_browser_url()` and screenshot queueing to target the active scope instead of hardcoded project scope.
- Prevent recurrence:
  - Added regression test `terminal_scoped_browser_is_visible_in_panel_when_active`: verifies that when a terminal-scoped browser has tabs, `active_browser_scope()` returns the terminal scope and `sync_embedded_browser()` hides the project browser.
  - Added regression test `terminal_scoped_url_does_not_persist_to_project_config`: verifies that `set_browser_url_for_scope()` writes `browser_last_url` only for project scope, not terminal scope.
  - Updated `AGENTS.md` Browser MCP Multi-Terminal Isolation Guidelines to codify `active_browser_scope()` as the single source of truth for panel display and URL persistence rules.
- Files/Commands touched: `src/app.rs` (`active_browser_scope()`, `draw_browser_panel()`, `sync_embedded_browser()`, `submit_browser_url()`, `set_browser_url_for_scope()`, `apply_browser_tab_observed_url()`, `queue_browser_screenshot()`, `process_pending_screenshot_requests()`, 2 new regression tests, 1 updated regression test), `AGENTS.md`, `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User bug report 2026-05-08: "mergen browser mcp şuan çalışamıyor mesela çalışıyor bağlı bir sayfayı güncellemesi gerekiyor ai devam ediyor ama ben mergende hiçbir değişiklik görmüyorum browserda"

---

#### OpenCode settings model not applied when editing active slot {#opencode-settings-model-not-applied}
- Date: 2026-05-11
- Context: OpenCode Build Model settings in Mergen ADE Settings
- Error signature: User reported: "ayarlardaki opencode settings hatalı set etmiyor şuan eski kimi k2.5 da değişmeli 2.6 olmalı" — Editing the active slot model text did not update the global/runtime OpenCode config, and the default model was still k2.5.
- Symptoms/Impact:
  1. Changing the model identifier in the active slot's text field and pressing Enter did not write the new model to OpenCode's global config or active terminal runtime configs.
  2. The "Use Slot A/B" button was disabled when the slot was already active, so there was no way to re-apply the current slot.
  3. The default Slot A model remained `kimi-k2p5-turbo` instead of the newer `kimi-k2p6-turbo`.
- Root cause:
  - `switch_opencode_build_model_slot` returned early when the active slot did not change (`if previous_slot == active_build_model_slot { return; }`), so editing the active slot's model string never triggered a global/runtime config patch.
  - The UI button for the active slot was also guarded with `&& !is_slot_a_active`, making it non-interactive when already active.
  - The default model string in `OpenCodeModelConfig::default()` had not been updated from k2.5 to k2.6.
- Resolution:
  - Updated default `build_model_slot_a` to `fireworks-ai/accounts/fireworks/routers/kimi-k2p6-turbo`.
  - Removed the early return in `switch_opencode_build_model_slot` so that clicking an already-active slot re-applies the current model to global and runtime configs.
  - Removed the `&& !is_slot_a_active` guard on the slot button click handlers so the active slot button is always clickable.
  - Added one-time config migration in `normalize_config_for_current_platform` that replaces the exact old default `kimi-k2p5-turbo` string with `kimi-k2p6-turbo` for users with existing persisted configs.
- Prevent recurrence:
  - Updated regression tests in `src/models.rs` and `src/config.rs` to expect the new default model.
  - Documented the active-slot button behavior in AGENTS.md: the active slot button must always be clickable to re-apply the model.
- Files/Commands touched: `src/models.rs`, `src/app.rs`, `src/config.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo fmt`, `cargo test`
- References: User bug report 2026-05-11: "ayarlardaki opencode settings hatalı set etmiyor şuan eski kimi k2.5 da değişmeli 2.6 olmalı ayarla"

---

#### OpenCode build mode uses stale `mode.build.model` while `agent.build.model` is updated {#opencode-mode-build-model-stale}
- Date: 2026-05-11
- Context: OpenCode Build Model Slot switching in Mergen ADE Settings
- Error signature: User reported: "slot b seçiyorum openai/gpt-5.5-fast ama build modda kimi geliyor" — After switching to Slot B (GPT), OpenCode's build mode still used the Kimi model from Slot A.
- Symptoms/Impact:
  1. Global OpenCode config (`~/.config/opencode/opencode.json`) had `agent.build.model` correctly set to the selected slot model.
  2. However, `mode.build.model` remained at the old Kimi value.
  3. OpenCode's build mode read `mode.build.model` and therefore launched with the wrong model regardless of the selected slot.
  4. Runtime terminal configs also only wrote `agent.build.model`, leaving `mode.build.model` absent or stale.
- Root cause:
  - Mergen only updated `agent.build.model` when patching the global config and when writing per-terminal runtime configs.
  - OpenCode's newer config schema uses `mode.build.model` as the authoritative build-mode model, which Mergen was not setting.
  - When `mode.build.model` existed from an earlier OpenCode version or manual edit, it took precedence and caused the model mismatch.
- Resolution:
  - Updated `patch_global_opencode_config` to also write `mode.build.model` with the same value as `agent.build.model`.
  - Updated `write_terminal_runtime_config_with_browser_mcp` (and `write_terminal_runtime_config`) to include `mode.build.model` in the emitted JSON alongside `agent.build.model`.
  - Both paths now keep the two model fields synchronized so build mode cannot drift from the selected slot.
- Prevent recurrence:
  - Updated `opencode_config.rs` regression tests to assert that runtime configs contain `mode.build.model` and that it matches the selected model.
  - Documented in `AGENTS.md` that both `agent.build.model` and `mode.build.model` must be kept in sync when writing OpenCode config.
- Files/Commands touched: `src/opencode_config.rs`, `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo fmt`, `cargo test`
- References: User bug report 2026-05-11: "slot b seçiyorum openai/gpt-5.5-fast ama build modda kimi geliyor"
