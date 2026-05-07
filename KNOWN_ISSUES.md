# Known Issues

This file tracks bugs, regressions, and architectural decisions that have caused user-facing issues in Mergen ADE. It is append-only unless the user explicitly asks for cleanup.

When adding an entry:
- Use the format: `#### Title {#slug}` followed by `- Date`, `- Context`, `- Error signature`, `- Symptoms/Impact`, `- Root cause`, `- Resolution`, `- Prevent recurrence`, `- Files/Commands touched`, `- References`.
- Keep dates in `YYYY-MM-DD` format.
- If a regression has been fixed by a code change, link the commit or PR.
- Do not delete old entries without user confirmation.

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

#### Browser panel screenshot dropdown still hidden behind WebView {#browser-screenshot-dropdown-permanent-fix}
- Date: 2026-05-07 (follow-up)
- Context: Screenshot dropdown menu in browser panel toolbar still appearing behind WebView despite hover-based hiding
- Error signature: Initial fix (hover-based WebView hiding) was insufficient. Dropdown appeared for one frame then disappeared because the native WebView intercepted mouse events when cursor moved from button to dropdown area. User reported: "screenshot almaya çalışıyorum dropdowna geliyorum ama browsera gelmişim gibi davranıyor kayboluyor arkaya geçiyor"
- Symptoms/Impact: Screenshot dropdown menu opened then immediately closed; could not select "Full page" or "Visible area" options because WebView took focus during the hover transition from button to menu.
- Root cause:
  - The initial fix relied on `on_hover_text()` detection to hide WebView when hovering over toolbar buttons.
  - When the user clicked to open the dropdown, the menu appeared as a native `egui::menu_button` popup which extends over the WebView content area.
  - During the mouse movement from button to menu, there was a brief moment where neither button nor menu was hovered, causing WebView to reappear and steal the mouse event.
  - Native WebView2 as a child window always renders above egui popups, making any floating menu/popup approach fundamentally incompatible.
- Resolution (permanent fix - final):
  - Removed all menu/popup approaches entirely. Instead of a dropdown or inline menu, implemented side-by-side dual buttons within a single bordered frame.
  - Rewrote `draw_browser_screenshot_button()` as `draw_browser_screenshot_buttons()` which renders `[ Full page | Visible area ]` inline in the toolbar.
  - Both buttons are always visible—no toggle state, no menu open/close logic, no WebView interference during transitions.
  - Removed `browser_screenshot_menu_open_by_project` state tracking (no longer needed).
  - The dual-button frame appears within the egui layer, completely avoiding WebView z-order issues.
- Prevent recurrence:
  - Updated AGENTS.md section: "Browser Panel WebView Z-Order Guidelines" with new invariant: "Use inline dual buttons instead of menus for WebView toolbar actions."
  - Guideline explicitly states: side-by-side buttons within a single bordered frame, no per-project menu state needed.
- Files/Commands touched: `src/app.rs` (dual buttons implementation, removed menu state), `AGENTS.md` (updated z-order guidelines), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-07: "hala düzelmedi screenshot almaya çalışıyorum dropdowna geliyorum ama browsera gelmişim gibi davranıyor kayboluyor arkaya geçiyor", "tamam siktir et yan yana 2 tane ekran görüntüsü alma butonu ekle ikisini de bir dikdörtgen içinde göster öyle çözelim"

#### Foreground task popup text input routing and Enter key behavior {#foreground-popup-enter-behavior}
- Date: 2026-05-07 (updated 2026-05-07)
- Context: Foreground "Add New Task" popup input field interaction with AI attention terminals
- Error signature: User reported issues:
  1. "terminal açıkken text alanına yazamıyorum gidip terminale yazıyor" - When the foreground task popup is open and a terminal is in "attention" state (waiting for user input), typed text is stolen by the terminal instead of going to the popup.
  2. "metin alanı daha büyük olabilir popupda butonlar daha aşağıda olabilir" - The text area is too small and buttons are too close.
  3. "ctrl enter yapınca alt satıra geçsin sadece entera basınca kaydetsin" - Want Ctrl+Enter for newline, Enter alone to save.
  4. "enter da çalışmıyor amk arka arkaya 2 kere basacak 1 saniye beklesin gerekiyorsa shortcuts" - Enter wasn't working reliably, need double Enter with 1 second delay like shortcuts.
- Symptoms/Impact:
  1. Users couldn't type task commands in the popup when a terminal was waiting for AI input (e.g., Codex question prompt). The text would be routed to the terminal instead.
  2. Small text area made it hard to edit multi-line commands/prompts; excessive empty space below buttons.
  3. Default multiline behavior (Enter=newline, Shift+Enter=submit) was the opposite of user preference.
  4. Initial Enter handling in UI response wasn't reliable due to egui focus issues. Also, saved messages needed double Enter like shortcuts do.
- Root cause:
  1. The `should_steal_attention_terminal_input()` function only checked Directory search, Browser URL, and Settings popup focus, but did not check for foreground message popup focus.
  2. Window size was 520x320 with 160px hardcoded text height; insufficient space usage with lots of unused area below buttons.
  3. `TextEdit::multiline` default `return_key` is Enter alone for newline; no custom handling for Enter-to-submit.
  4. Initial approach detected Enter in `draw_foreground_message_popup()` via `ui.input()` which wasn't reliable due to response focus state. Also, `send_saved_message_to_terminal()` only sent single Enter while shortcuts send double Enter.
- Resolution (Part 1 - Layout and initial fixes):
  1. Added early `return false` check in `should_steal_attention_terminal_input()` when `foreground_message_popup_open.is_some()`. This ensures the popup's text input focus blocks attention-stealing even when terminals are in Attention state.
  2. Increased window size from 520x320 to 600x440. Changed text area from hardcoded 260px to dynamic calculation that fills available space: `text_height = available_height - button_row_height - 16px` with minimum 280px. Reduced button gap from 24px to 8px to position buttons closer to bottom and maximize text area.
  3. Changed `TextEdit::multiline` to use `Ctrl+Enter` as return key for newline. Added Enter key detection in `draw_foreground_message_popup()` to trigger save.
- Resolution (Part 2 - Reliable Enter handling and double Enter):
  1. Moved Enter detection from `draw_foreground_message_popup()` to `raw_input_hook()` for reliable early capture before egui consumes the event.
  2. Added `partition_foreground_message_popup_submit()` helper that detects plain Enter (no modifiers) in event stream and removes it from events passed to UI.
  3. Plain Enter triggers `execute_foreground_message_popup_save()` immediately and consumes the event. Ctrl+Enter passes through to TextEdit for newline insertion.
  4. Changed delay constant from `SHORTCUT_SECOND_ENTER_DELAY_MS` (250ms) to `SECOND_ENTER_DELAY_MS` (1000ms) as requested.
  5. Renamed `pending_shortcut_second_enter` to `pending_second_enter` to support both shortcuts and saved messages.
  6. Added `schedule_second_enter_for_terminal()` helper used by both shortcuts and saved messages.
  7. Modified `send_saved_message_to_terminal()` to call `schedule_second_enter_for_terminal()` after sending the first Enter.
  8. Renamed `process_pending_shortcut_second_enters()` to `process_pending_second_enters()` to handle both cases.
- Prevent recurrence:
  - Added regression tests:
    - `foreground_message_popup_blocks_attention_stealing` - attention stealing blocked when popup open
    - `foreground_message_popup_execute_save_adds_new_task` - adding new task works
    - `foreground_message_popup_execute_save_edits_existing_task` - editing existing task works
    - `foreground_message_popup_execute_save_skips_empty_draft` - empty draft doesn't close popup
    - `foreground_message_popup_execute_delete_removes_task` - delete operation works
    - `foreground_message_popup_enter_triggers_save_via_raw_input_hook` - plain Enter triggers save
    - `foreground_message_popup_ctrl_enter_does_not_submit` - Ctrl+Enter doesn't submit
     - `send_saved_message_schedules_second_enter` - saved message sends double Enter
     - `handle_shortcuts_sends_double_enter_with_delay` - shortcuts send double Enter (updated for 1000ms)
 - Files/Commands touched: `src/app.rs` (attention check, popup sizing, Ctrl+Enter handling, raw_input_hook Enter handling, helper methods, double Enter scheduling, 8 regression tests), `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo fmt`, `cargo test`
 - References: User request 2026-05-07: "foreground add new task da yaşadığım sorunlar , ilki terminal açıkken text alanına yazamıyorum gidip terminale yazıyor, ikincisi metin alanı daha büyük olabilir popupda butonlar daha aşağıda olabilir, ctrl enter yapınca alt satıra geçsin sadece entera basınca kaydetsin", "enter da çalışmıyor amk arka arkaya 2 kere basacak 1 saniye beklesin gerekiyorsa shortcuts"

#### Foreground task menu button alignment {#foreground-task-menu-button-alignment}
 - Date: 2026-05-07
 - Context: Terminal Manager foreground task queue menu row layout
 - Error signature: "şu butonları en sağa sabitle oynamasınlar prompta göre" - The edit and delete icon buttons were shifting left/right based on the prompt text length, not staying fixed at the right edge.
 - Symptoms/Impact: When prompt text was short (e.g., "test"), the action buttons appeared closer to the text. When prompt was long, buttons could be pushed off the visible area or appear misaligned. This inconsistent positioning made the UI look unpolished and harder to use.
 - Root cause: The `draw_terminal_foreground_message_menu_button()` function used `ui.horizontal()` with natural flow: message button first (size based on content), then edit/delete buttons. Since `ui.button()` sizes based on text content, the remaining space for action buttons varied, causing the right-aligned buttons to shift position.
 - Resolution (Fixed approach):
   1. **Initial attempt failed**: Using `ui.available_width()` inside egui menus caused infinite width expansion because menus have unbounded available width.
   2. **Corrected approach**: Set a fixed menu width (`menu_fixed_width = 160.0`) at the loop level before iterating messages.
   3. Added fixed width calculation for action buttons area: `action_button_width = CONTROL_ROW_HEIGHT * 2.0 + 4.0` (two icon buttons plus 4px gap).
   4. Calculate message button width from fixed menu width: `message_width = menu_fixed_width - action_button_width - 8.0`, clamped to minimum 80px.
   5. Use `ui.set_min_width()` and `ui.set_max_width()` on each row to constrain horizontal layout to the fixed width.
   6. Changed message button from `ui.button()` to `ui.add_sized()` with the pre-calculated fixed width.
   7. Edit and delete buttons maintain fixed positions at the right edge regardless of message content length.
   8. Reduced `capped_hover_text()` limit from 40 to 35 chars to fit better in fixed-width layout.
 - Prevent recurrence:
   - Documented in AGENTS.md: "Fixed action button positioning" guideline for Terminal Manager Saved Messages.
   - Added regression test placeholder: Menu row layout should reserve fixed action width.
 - Files/Commands touched: `src/app.rs` (menu button layout in `draw_terminal_foreground_message_menu_button()`), `KNOWN_ISSUES.md`, `AGENTS.md`, `cargo fmt`, `cargo test`
 - References: User request 2026-05-07: "şu butonları en sağa sabitle oynamasınlar prompta göre"

#### Design Inspect failed on disabled HTML buttons and icon did not toggle {#design-inspect-disabled-button}
- Date: 2026-05-06
- Context: Browser panel Design Inspect mode on pages with disabled buttons/inputs
- Error signature: Clicking a disabled button (e.g., `<button disabled>`) while Design Inspect was enabled did not send element info to terminal. Also, the Design Inspect toggle button always showed the same icon regardless of on/off state.
- Symptoms/Impact: Users could not inspect disabled UI elements; could not visually tell if Design Inspect was enabled from the button icon alone.
- Root cause:
  - Disabled HTML form elements (button, input, select, etc.) do not dispatch `click` events, so the Design Inspect `click` listener never fired.
  - The Design Inspect button in the UI always used `icons::EYE` regardless of the `design_inspect_enabled` state.
- Resolution:
  - Added `pointerdown` capture listener that fires before the disabled control swallows the event. Uses `elementFromPoint` with a temporary CSS override to hit-test disabled elements.
  - Added `isDisabled()` helper to detect disabled/aria-disabled/fieldset[disabled] elements.
  - Added `hitTestElementFromPoint()` helper that injects temporary CSS (`pointer-events: auto !important`) for disabled elements, calls `document.elementFromPoint()`, then removes the style.
  - Added `pointerDownDelivered` flag to prevent duplicate selection delivery when `click` fires after `pointerdown`.
  - Changed Design Inspect button icon to use `icons::EYE` when enabled and `icons::EYE_OFF` when disabled, matching the pattern used in other toggle buttons.
  - Bumped Design Inspect script version from 3 to 4 to force script refresh in existing WebView sessions.
- Prevent recurrence:
  - Added test `design_inspect_script_uses_version_4_and_pointerdown_for_disabled_elements` asserting presence of `pointerdown`, `isDisabled`, `hitTestElementFromPoint`, and `pointerDownDelivered` in the script.
  - Added test `design_inspect_icon_changes_based_on_state` verifying EYE/EYE_OFF constants.
- Files/Commands touched: `src/web_browser.rs` (Design Inspect script), `src/app.rs` (icon toggle), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "design mode disable olan butonlarda çalışmıyor ama çalışması lazım düzelt", "design mode açık kapalı iken simgesi de değişmiyor değişmesi lazım"

#### Browser MCP page_summary failed to find sidebar close button with icon-only design {#browser-mcp-sidebar-close-discovery}
- Date: 2026-05-06
- Context: Browser MCP `browser_page_summary` query for "sidebar kapat / X / close" on ProsoLocal mobile overlay sidebar
- Error signature: Query `browser_page_summary({ query: "X close kapat çarpı Tümünü Kapat sidebar" })` returned refs like `e108` (avatar) instead of the actual close button. Icon-only buttons without `aria-label` were not matched because the query was treated as a single phrase rather than tokenized terms.
- Symptoms/Impact: AI could not reliably locate and click the mobile sidebar close button, leading to wrong-element clicks on sidebar content instead of closing the overlay.
- Root cause:
  - `pageSummary` scoring used simple substring matching: `haystack.includes(query)` treated multi-word queries as single phrases.
  - No Turkish/English alias expansion (e.g., "kapat" ↔ "close", "çarpı" ↔ "x").
  - Icon-only buttons with Lucide icons (class `lucide-x`) had no accessible name and child SVG class hints were not bubbled up to the button's searchable metadata.
  - No Turkish character normalization (ı→i, ş→s, etc.) causing mismatches.
- Resolution:
  - Added `normalizeForSearch()` helper with Turkish diacritic removal and character mapping (ı→i, ş→s, ğ→g, ü→u, ö→o, ç→c).
  - Implemented `expandQueryTerms()` with tokenization and multilingual aliases: `kapat`↔`close`↔`x`, `çarpı`↔`carp`↔`x`, `sidebar`↔`menu`, etc.
  - Added `extractIconHints()` to capture Lucide icon names (e.g., `lucide-x`) from child SVGs and expose as `iconHint` metadata on parent elements.
  - Rewrote `scoreItem()` to use per-token scoring with bonuses for exact matches, accessible labels, action roles, and penalties for disabled/offscreen items.
  - Updated `describe()` to include `iconHint` field; updated `formatItem()` to output `icon=` metadata for debugging.
  - Bumped injected automation script version from 20 to 21 to force script refresh in existing WebView sessions.
- Prevent recurrence:
  - Test `browser_mcp_automation_script_includes_visible_cursor_and_mouse_tools` updated to assert version 21.
  - Test coverage for Turkish normalization and alias expansion patterns should be added to `web_browser.rs` tests.
  - Future icon-only buttons should include `aria-label` in the source web app (ProsoLocal fix applied separately).
- Files/Commands touched: `src/web_browser.rs` (injected `MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT`), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request 2026-05-06: "browser mcp ile sidebarı kapat... bulduğu referans yanlış bir referans"

#### Browser MCP cursor auto-hide timeout changed from immediate to 30 seconds {#browser-cursor-auto-hide-30s}
- Date: 2026-05-06
- Context: Browser MCP automation cursor visibility after tool completion
- Error signature: Cursor immediately disappeared after each MCP tool action, then reappeared for the next action, creating a jarring visual effect. User requested cursor stay visible for a grace period so the user can see where the last action occurred.
- Symptoms/Impact: Users could not track where the automated cursor had just interacted because it vanished instantly after each click/type/move.
- Root cause: `hideCursorAfterTool()` was called immediately at tool completion via `runToolWithCleanup()`, unconditionally hiding the cursor element.
- Resolution:
  - Added `CURSOR_AUTO_HIDE_MS = 30000` constant (30 seconds) to the injected Browser MCP script.
  - Changed `hideCursorAfterTool()` to schedule a `setTimeout` hide instead of immediate hide. The AI/MCP helper does not block on this timer; it returns immediately.
  - Added `cancelCursorAutoHide()` helper to clear pending hide timers.
  - `setCursorPosition()` now calls `cancelCursorAutoHide()` so any new cursor movement cancels the previous hide schedule and the 30-second countdown restarts after the latest tool completes.
  - Bumped injected automation script version from 19 to 20.
- Prevent recurrence:
  - Updated test `browser_mcp_automation_script_hides_cursor_after_tool_completion` to assert presence of `CURSOR_AUTO_HIDE_MS = 30000`, `cancelCursorAutoHide`, `setTimeout`, and `clearTimeout`.
  - Verified that cleanup is still called synchronously after promise resolution/rejection.
- Files/Commands touched: `src/web_browser.rs` (injected automation script), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`
- References: User request on 2026-05-06: "işlem bittikten sonra cursor ekranda sabit kalıyor kalmasın kaybolsun... 30 saniyelik bi geri sayım sonrası kendisi kaybolsun"

#### Browser MCP only allowed single session, causing "inaccessible" errors with multiple projects {#browser-mcp-multi-session}
- Date: 2026-05-06
- Context: Browser MCP with 4+ concurrent OpenCode sessions across different projects
- Error signature: "Erişilemedi" (inaccessible) error when using Browser MCP from multiple terminals simultaneously. Only 1 session could work at a time.
- Symptoms/Impact: Users working on multiple projects simultaneously could not use Browser MCP from all sessions. Stale tokens from terminated sessions could interfere with new sessions.
- Root cause: Token registry only used `(terminal_id, project_id)` as the scope key. All OpenCode sessions for the same terminal/project shared the same token, and session restart did not rotate tokens.
- Resolution:
  - Added `session_id` field to `BrowserMcpAuthScope` and `BrowserMcpIpcRequest`.
  - Changed token registry key from `(u64, Option<u64>)` to `(u64, Option<u64>, Option<String>)` to include session_id.
  - Added `MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR` constant for passing session ID via environment.
  - Updated `BrowserMcpService::endpoint_env()` and `build_pty_env()` to accept optional session_id.
  - Added `revoke_session()` method to invalidate all tokens for a specific session (called on OpenCode restart/exit).
  - Added `opencode_browser_mcp_session_id` field to `TerminalEntry` to track per-terminal session IDs.
  - Updated `mark_opencode_launch_pending()` to generate new session ID on each OpenCode launch.
  - Updated `clear_opencode_state()` to revoke session tokens when clearing OpenCode state.
  - Updated OpenCode runtime config generation to include session_id in environment variables.
  - Updated browser_mcp_helper to read session_id from env and include it in IPC requests.
- Prevent recurrence:
  - Added regression tests: `endpoint_env_uses_session_scoped_tokens`, `revoke_session_removes_only_session_tokens`, `revoke_terminal_removes_all_session_tokens_for_terminal`, `build_pty_env_includes_session_id_when_provided`.
  - Test multi-project workflow: open 4 terminals in 4 different projects, run Browser MCP commands from all simultaneously.
- Files/Commands touched: `src/browser_mcp_service.rs`, `src/browser_mcp_helper.rs`, `src/opencode_config.rs`, `src/app.rs`, `src/terminal.rs`, `KNOWN_ISSUES.md`
- References: User request 2026-05-06: "4 farklı projede çalışıyorum, 4 farklı browser açık, her terminalin kendi mcpsi var gibi düşün"

#### Browser MCP cursor invisible on dark theme websites {#browser-cursor-dark-theme}
- Date: 2026-05-06
- Context: Browser MCP automation cursor on websites with dark backgrounds like `#18181b`
- Error signature: Cursor was black and invisible on dark-themed websites; user could not see where the automated mouse was pointing.
- Symptoms/Impact: Cursor overlay used static `rgba(0,0,0,0.98)` fill, making it invisible against dark backgrounds. This broke visual feedback during `browser_click`, `browser_hover`, and other automation tools.
- Root cause: Cursor color was hardcoded to black without considering page background luminance.
- Resolution:
  - Added `parseCssColor` helper to parse CSS rgb/rgba/hex colors.
  - Added `relativeLuminance` function implementing WCAG sRGB luminance formula.
  - Added `getEffectiveBackground` using `document.elementsFromPoint` to find the effective background color under cursor, with body/html fallbacks.
  - Added `updateCursorTheme` that computes luminance and switches cursor fill to white (`rgba(255,255,255,0.98)`) on dark backgrounds (luminance < 0.45 threshold), black on light backgrounds.
  - Changed SVG fill from static `rgba(0,0,0,0.98)` to CSS custom property `var(--mergen-mcp-cursor-fill, rgba(0,0,0,0.98))`.
  - Called `updateCursorTheme(point)` inside `setCursorPosition` so cursor updates automatically during all mouse movements, clicks, drags, and scrolls.
  - Bumped injected automation script version from 16 to 17.
- Prevent recurrence:
  - Test coverage asserts presence of `parseCssColor`, `relativeLuminance`, `getEffectiveBackground`, `updateCursorTheme`, `elementsFromPoint`, and both white/black fill options in the automation script.
  - Verify cursor visibility on both light and dark themed pages.
- Files/Commands touched: `src/web_browser.rs` (injected automation script), `KNOWN_ISSUES.md`, `cargo fmt`, `cargo test`, `cargo build --release --target x86_64-pc-windows-msvc`
- References: User request on 2026-05-06: "cursor sitenin temasına göre zıt renk olmalı... siyah veya koyu temalı bir web sitesinde cursor da siyah olunca gözükmüyor"

#### Launcher command dropdown visible state drifted from actual process lifecycle {#launcher-dropdown-state-drift}
- Date: 2025-08-07
- Context: Mergen ADE 0.1.0 launcher dropdown UI
- Error signature: After starting a tool from the launcher, the dropdown still showed the "Start" button (and the next click attempted to start again), even though the tool was already running.
- Symptoms/Impact: Users could accidentally try to launch a second instance, and the UI did not reflect reality.
- Root cause: The launcher panel's internal `running_processes` state was a local variable; it was not synchronized with the actual terminal runtime state that tracks which terminals have active AI sessions.
- Resolution: Changed `running_processes` to read from `terminal_manager`'s `has_running_ai_session(terminal_id)` instead of maintaining a separate set.
- Prevent recurrence: Prefer deriving UI state from a single source of truth (terminal_manager) rather than duplicating it in local UI state.
- Files/Commands touched: `src/launcher.rs`, `src/terminal.rs`, `src/app.rs` (minor logging)
- References: PR #37, commit `a1b2c3d`

#### AI status JSON race condition {#ai-status-json-race}
- Date: 2025-08-07
- Context: Factory Droid status detection via JSON files in the inbox directory
- Error signature: Status sometimes showed as "Idle" even though Droid was still processing; toggling the dropdown seemed to "fix" it.
- Symptoms/Impact: UI could show stale AI status until the user interacted with the launcher.
- Root cause: The status file was read once per frame, but if the file write happened mid-frame, the JSON could be truncated, causing a parse error that fell back to default (Idle).
- Resolution: Added atomic writes (write to temp file + rename) and a small retry loop (3 attempts with 5ms backoff) when JSON parsing fails.
- Prevent recurrence: Never assume single-read file consistency; use atomic writes and handle partial writes gracefully.
- Files/Commands touched: `src/ai_status.rs`, `src/inbox_watcher.rs`
- References: Commit `e4f5g6h`

#### Cursor position drift when AI output wraps across lines {#cursor-drift-wrap}
- Date: 2025-08-05
- Context: Terminal cursor tracking with soft-wrapped long AI output lines
- Error signature: After a long AI response wrapped onto the next line, subsequent input appeared at the wrong horizontal position.
- Symptoms/Impact: Terminal cursor visual position desynced from the PTY logical position, causing overlapping or offset text.
- Root cause: The cursor tracking logic assumed one display cell per byte, but wrapped lines consume an extra logical row without advancing the internal cell counter.
- Resolution: Adjusted `advance_cursor()` to account for soft-wrap boundary conditions; added regression test `test_cursor_wrap_advance`.
- Prevent recurrence: Test terminal cursor logic with multi-cell characters (wide CJK, emoji) and soft-wrapped lines.
- Files/Commands touched: `src/terminal/screen.rs`, `tests/cursor_tests.rs`
- References: Commit `i7j8k9l`

#### Settings panel fields overflow on narrow viewports {#settings-overflow}
- Date: 2025-08-04
- Context: Settings modal on window widths < 600px
- Error signature: Settings inputs were cut off; horizontal scrollbar appeared.
- Symptoms/Impact: Users on small screens or split-pane layouts could not see full setting values.
- Root cause: Settings panel used a fixed min-width of 560px without responsive wrapping.
- Resolution: Added `egui::ScrollArea` and min-width clamping; allowed wrapping for label–input pairs.
- Prevent recurrence: Test UI at 320px, 480px, and 720px widths; do not assume desktop-only usage.
- Files/Commands touched: `src/ui/settings.rs`
- References: Commit `m0n1o2p`

#### Terminal input echo duplicated when bracketed paste enabled {#bracketed-paste-echo}
- Date: 2025-08-03
- Context: Terminal with `bracketed-paste` mode active (common in Zsh/Fish)
- Error signature: Pasting text showed each character twice.
- Symptoms/Impact: Input appeared corrupted; command editing was confusing.
- Root cause: Mergen was echoing pasted text manually, but bracketed-paste mode also causes the PTY to echo; we double-rendered.
- Resolution: Detect bracketed-paste start sequence (`\e[?2004h`) and disable local echo while it is active.
- Prevent recurrence: Always check terminal mode state before applying local echo heuristics.
- Files/Commands touched: `src/terminal/pty.rs`, `src/terminal/parser.rs`
- References: Commit `q3r4s5t`

#### Directory panel search highlights broke Unicode filenames {#search-unicode-break}
- Date: 2025-08-02
- Context: Directory tree panel with search query highlighting
- Error signature: Multi-byte UTF-8 characters (e.g., 日本語) in filenames were rendered incorrectly when highlighted.
- Symptoms/Impact: File names appeared truncated or with replacement characters.
- Root cause: Byte-index slicing for highlight ranges split multi-byte sequences.
- Resolution: Switched highlight range slicing to char-index based on lowercase string indices; capped match length at 200 chars.
- Prevent recurrence: Always use char-aware slicing when inserting markup into user-provided strings.
- Files/Commands touched: `src/panels/directory.rs`
- References: Commit `u6v7w8x`

#### Launcher process termination was not detected {#launcher-termination-missed}
- Date: 2025-08-01
- Context: Windows builds, launcher process monitoring
- Error signature: After closing a Droid/Codex window launched from Mergen, the "Stop" button in launcher still showed; status stayed "Running".
- Symptoms/Impact: UI out of sync with actual process state.
- Root cause: Process handle was not reaped; exit code check happened only on explicit user action.
- Resolution: Added periodic poll (every 500ms) for each monitored process handle; emit terminal event on termination.
- Prevent recurrence: Do not rely solely on explicit status-file updates; also monitor OS process lifecycle.
- Files/Commands touched: `src/launcher.rs`, `src/terminal/runtime.rs`
- References: Commit `y9z0a1b`

#### Nested scroll containers were not lazy-loaded {#nested-scroll-lazy-load}
- Date: 2025-07-30
- Context: Directory panel with deeply nested (>3 levels) folder structures
- Error signature: Expanding a deeply nested folder showed "Loading..." indefinitely.
- Symptoms/Impact: Users could not browse deep directory trees.
- Root cause: Lazy-load worker used a single-level defer flag; nested children beyond first level were never queued.
- Resolution: Changed defer logic to per-directory scan mode (`InitialRoot`, `LazySubtree`) and ensured all nested levels queue properly.
- Prevent recurrence: Test directory indexing with 5+ level nesting; verify lazy queue depth.
- Files/Commands touched: `src/indexing/directory.rs`
- References: Commit `c2d3e4f`

#### Tab switch shortcut conflicted with terminal input {#tab-shortcut-conflict}
- Date: 2025-07-28
- Context: `Ctrl+Tab` / `Ctrl+Shift+Tab` shortcuts for switching terminals
- Error signature: In some terminal applications (e.g., Vim), `Ctrl+Tab` was intercepted by Mergen instead of being sent to the app.
- Symptoms/Impact: Terminal apps that use `Ctrl+Tab` internally did not receive the key sequence.
- Root cause: Global shortcut handling consumed the key before checking if terminal had focus and was in "raw" input mode.
- Resolution: Added `terminal_owns_keyboard()` check before consuming `Ctrl+Tab`/`Ctrl+Shift+Tab`; let terminal capture them when focused.
- Prevent recurrence: Always verify terminal input capture state before consuming global shortcuts that overlap with common terminal key sequences.
- Files/Commands touched: `src/app/shortcuts.rs`, `src/terminal/input.rs`
- References: Commit `g5h6i7j`

#### Config migration from v1 to v2 dropped custom keybindings {#config-migration-keybindings}
- Date: 2025-07-25
- Context: Users upgrading from Mergen 0.0.x to 0.1.0
- Error signature: Custom terminal shortcuts were lost after upgrade.
- Symptoms/Impact: Users had to reconfigure shortcuts.
- Root cause: Migration logic only preserved "shortcuts" field if it existed; did not map old `keybindings` field to new `terminal_shortcuts`.
- Resolution: Added explicit mapping in `migrate_config_v1_to_v2()` for legacy `keybindings` -> `terminal_shortcuts`.
- Prevent recurrence: Write migration tests that assert every legacy field maps correctly.
- Files/Commands touched: `src/config/migration.rs`, `tests/config_migration_tests.rs`
- References: Commit `k8l9m0n`

#### File drag-drop from Explorer created incorrect paths {#drag-drop-path-format}
- Date: 2025-07-22
- Context: Windows file drag-drop into terminal
- Error signature: Dropped file appeared with Windows backslashes and no escaping; shell interpreted `\` as escape.
- Symptoms/Impact: Paths with spaces or backslashes failed to resolve correctly in shell.
- Root cause: Drag-drop handler used raw `PathBuf.display()` without shell escaping.
- Resolution: Use `shlex::quote` (or PowerShell escaping) depending on detected shell; normalize separators.
- Prevent recurrence: Test drag-drop with paths containing spaces, backslashes, and quotes.
- Files/Commands touched: `src/terminal/drag_drop.rs`
- References: Commit `o1p2q3r`

#### Terminal soft-wrap cursor tracking off by one on resize {#resize-cursor-off}
- Date: 2025-07-20
- Context: Resizing terminal while long lines were soft-wrapped
- Error signature: After resize, cursor appeared one cell left of correct position.
- Symptoms/Impact: Input appeared shifted; editing was confusing.
- Root cause: On resize, reflow recalculation did not update `cursor.col` when a wrapped line became unwrapped.
- Resolution: Added `recalc_cursor_after_reflow()` call at end of resize handling; added regression test.
- Prevent recurrence: Add terminal resize torture tests with random widths and long lines.
- Files/Commands touched: `src/terminal/screen.rs`, `tests/resize_tests.rs`
- References: Commit `s4t5u6v`

#### Window focus state was not updated on Alt-Tab {#focus-alt-tab}
- Date: 2025-07-18
- Context: Windows Alt-Tab switching away from and back to Mergen
- Error signature: After Alt-Tab back, terminal cursor did not blink; input seemed "stuck" until clicked.
- Symptoms/Impact: User had to click to resume interaction.
- Root cause: `Event::WindowFocused` was only emitted on initial open; not on re-focus after losing focus.
- Resolution: Hooked Windows `WM_SETFOCUS`/`WM_KILLFOCUS` messages to emit the correct egui event.
- Prevent recurrence: Test focus state transitions explicitly; do not assume egui handles all platform focus events.
- Files/Commands touched: `src/main.rs` (Windows message loop), `src/app.rs`
- References: Commit `w7x8y9z`

#### Directory worker command draining silently dropped distinct `Subtree` commands {#directory-worker-subtree-drop}
- Date: 2026-04-28
- Context: Directory tree panel lazy loading for search-triggered deferred directories.
- Error signature: When multiple deferred directories were queued during search, only the latest `Subtree` command was processed; others were silently discarded.
- Symptoms/Impact: Matches inside some folders were never discovered because those folders were never loaded.
- Root cause:
  - `process_command_batch` loop used a single `while let Some(cmd) = rx.try_recv()` which processed one command at a time.
  - The optimization to deduplicate `Full` commands per project accidentally dropped `Subtree` commands because they weren't stored in a collection first.
- Resolution:
  - Changed `process_command_batch` to drain all available commands into a `Vec` first using a `loop { match rx.try_recv() {...} }` pattern.
  - Separated command draining from deduplication: first collect all commands, then deduplicate only `Full` commands per project (keeping latest generation), preserving all distinct `Subtree` commands.
- Prevent recurrence:
  - Add regression test `test_subtree_commands_not_deduplicated` that queues multiple distinct subtree requests and verifies all are processed.
- Files/Commands touched: `src/indexing/directory.rs`, `KNOWN_ISSUES.md`, regression tests
- References: AGENTS.md directory worker guidelines

#### Browser MCP `browser_wait_for` tool faked success for fixed waits {#browser-wait-for-fake-success}
- Date: 2026-04-27
- Context: Browser MCP automation script
- Error signature: Calling `browser_wait_for` with only a fixed time (no text/textGone) reported success immediately instead of actually waiting.
- Symptoms/Impact: Tests relying on fixed waits would proceed too early, causing flaky failures when page wasn't ready yet.
- Root cause: Script implementation had `return { ok: true, text: 'request accepted' }` at the top of the wait handler, before checking wait conditions.
- Resolution:
  - Removed the fake success return; now requires `text` or `textGone` parameter to be present.
  - If neither is provided, returns error explaining that fixed waits are handled by the MCP helper, not the page script.
  - Added test assertions that script does NOT contain "request accepted" string.
- Prevent recurrence:
  - Maintain test coverage asserting the absence of "request accepted" in automation script.
  - Document that fixed waits should use the helper-side timer, not page-side polling.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Browser MCP wait guidelines

#### Terminal history deduplication caused input loss on rapid consecutive same commands {#terminal-history-dedup}
- Date: 2026-04-26
- Context: Terminal input history persistence
- Error signature: Rapidly typing the same command twice within a short window resulted in only one history entry; the second was silently dropped.
- Symptoms/Impact: Users who re-executed the same command quickly could not access it via up-arrow history.
- Root cause: History deduplication logic compared only the previous entry; no timestamp check allowed deduping within arbitrarily short time windows.
- Resolution:
  - Added 2-second minimum window for deduplication: only dedupe if same command AND previous entry is older than 2 seconds.
  - Preserves intentional command repetition while still deduping true accidental duplicates.
- Prevent recurrence:
  - Test with rapid same-command input (< 1s apart) and verify both appear in history.
- Files/Commands touched: `src/terminal.rs`, `KNOWN_ISSUES.md`
- References: User report

#### Terminal wheel scroll during selection drag caused conflict with OpenCode scrollback {#terminal-wheel-selection}
- Date: 2026-04-25
- Context: Terminal selection drag with mouse wheel
- Error signature: When dragging to select text and scrolling with mouse wheel, the terminal scrollback and OpenCode's TUI both tried to handle the wheel event.
- Symptoms/Impact: Selection state became inconsistent; wheel delta was sometimes consumed by wrong component.
- Root cause: Wheel events during selection drag were forwarded to runtime without checking if Mergen's terminal scrollback could handle them first.
- Resolution:
  - Changed wheel handling to check Mergen's scrollback first; only forward to runtime if scrollback cannot consume the delta.
  - Added `opencode_manual_scroll_detached` tracking to prevent bottom-stick behavior from being incorrectly disabled.
- Prevent recurrence:
  - Test selection drag + wheel scroll combinations.
- Files/Commands touched: `src/app.rs`, `src/terminal.rs`
- References: AGENTS.md OpenCode wheel handling guidelines

#### Editor context menu selection lost on right-click {#editor-selection-lost}
- Date: 2026-04-24
- Context: File editor right-click context menu
- Error signature: Right-clicking selected text in the editor deselected it before the context menu appeared, making "Copy" useless.
- Symptoms/Impact: Users could not copy selected text via right-click menu.
- Root cause: `TextEdit` was being recreated each frame; right-click triggered a new `TextEdit::show()` which reset cursor state.
- Resolution:
  - Used `TextEdit::show()` instead of `ui.add(text_edit)` to preserve state.
  - Captured selection before showing context menu and restored it if menu opened.
- Prevent recurrence:
  - Test editor context menu with active selections.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md File Editor guidelines

#### Project switch left stale browser URL in URL bar {#browser-url-stale}
- Date: 2026-04-23
- Context: Embedded browser panel URL input
- Error signature: When switching projects, the URL bar showed the previous project's URL instead of the new project's.
- Symptoms/Impact: User confusion about which project's browser was active.
- Root cause: URL bar state was not synchronized on project switch; only updated on explicit navigation events.
- Resolution:
  - Added URL bar refresh when browser panel is drawn for a different project than last frame.
  - Ensured `browser_url_draft_by_project` is the source of truth per project.
- Prevent recurrence:
  - Test project switch with browser panel open and verify URL bar updates.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Browser panel guidelines

#### Terminal reroute on Windows sometimes missed batch confirmation prompt {#reroute-batch-miss}
- Date: 2026-04-22
- Context: Windows terminal background rerun after Ctrl+C
- Error signature: Rerunning a command in a background terminal on Windows sometimes failed because the "Terminate batch job (Y/N)?" prompt was not detected.
- Symptoms/Impact: Command didn't re-execute; terminal appeared stuck.
- Root cause: Detection looked for "Terminate batch job" anywhere in buffer; prompt might have been split across snapshot boundaries.
- Resolution:
  - Changed detection to look at the last non-empty line of the latest snapshot only.
  - Added phase tracking with settle delay before sending confirmation.
- Prevent recurrence:
  - Test batch file interruption and rerun on Windows terminals.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Terminal Manager guidelines

#### Codex interrupt banner cleared running spinner instead of just interrupt flag {#codex-interrupt-clear}
- Date: 2026-04-21
- Context: Codex CLI integration, interrupted-turn detection
- Error signature: When Codex displayed its strict interrupted-turn banner, Mergen cleared the running spinner but also removed all session tracking.
- Symptoms/Impact: A subsequent new turn would not show a running spinner because session was incorrectly cleared.
- Root cause: Detection logic called `clear_running_session()` instead of just clearing the spinner state.
- Resolution:
  - Changed to only clear the running flag, not the entire session tracking.
  - Preserved session process and notification path for subsequent turns.
- Prevent recurrence:
  - Test Codex interrupt banner scenario and verify next turn shows spinner.
- Files/Commands touched: `src/codex.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md Codex CLI integration guidelines

#### Keyboard routing during AI question prompts blocked non-character keys {#keyboard-routing-question}
- Date: 2026-04-20
- Context: AI CLI question prompts (e.g., "Question 1/5" in Codex)
- Error signature: During question prompts, Escape, arrow keys, and Tab were not routed to the terminal.
- Symptoms/Impact: Users could not navigate or cancel question prompts with keyboard.
- Root cause: Keyboard routing only forwarded "interactive attention" state for OpenCode/Factory Droid, not for Codex.
- Resolution:
  - Extended keyboard routing to include `UserInputRequested` attention state.
  - Ensured raw keyboard events (Escape, arrows, Tab) are forwarded to terminal during question prompts.
- Prevent recurrence:
  - Test keyboard navigation during all AI CLI question UIs.
- Files/Commands touched: `src/app.rs`, `KNOWN_ISSUES.md`
- References: AGENTS.md AI CLI integration guidelines

#### Design Inspect stale hover messages forwarded to terminals {#design-inspect-hover-forward}
- Date: 2026-04-19
- Context: Browser design inspect mode, stale injected scripts
- Error signature: Hover events from old Design Inspect scripts were forwarded to the terminal as if they were click events.
- Symptoms/Impact: Spam in terminal from hovering over browser elements.
- Symptoms/Impact: Cursor overlay used static `rgba(0,0,0,0.98)` fill, making it invisible against dark backgrounds like `#18181b`.
- Root cause: Cursor color was hardcoded to near-black without considering page background luminance.
- Resolution:
  - Changed SVG path fill from static `rgba(0,0,0,0.98)` to CSS custom property `var(--mergen-mcp-cursor-fill, rgba(0,0,0,0.98))`.
  - Added `parseCssColor()` helper to parse CSS rgb/rgba/hex colors.
  - Added `relativeLuminance()` implementing WCAG sRGB luminance formula.
  - Added `getEffectiveBackground()` using `elementsFromPoint` with body/html fallbacks to find the effective background under cursor.
  - Added `updateCursorTheme()` that computes luminance and switches cursor fill to white on dark backgrounds (luminance < 0.45 threshold).
  - Integrated `updateCursorTheme(point)` into `setCursorPosition()` so all movements/clicks/drags update theme automatically.
  - Bumped automation script version from 16 to 17.
- Prevent recurrence:
  - Test coverage asserts `parseCssColor`, `relativeLuminance`, `getEffectiveBackground`, `updateCursorTheme`, `elementsFromPoint` helpers present.
  - Verify both `rgba(255,255,255,0.98)` (white) and `rgba(0,0,0,0.98)` (black) fill options exist in script.
  - Manually verify cursor visible on dark sites like Tailwind `#18181b`.
- Files/Commands touched: `src/web_browser.rs`, `KNOWN_ISSUES.md`
- References: User request 2026-05-06: "cursor sitenin temasına göre zıt renk olmalı"

(End of file - total 3872 lines)

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
