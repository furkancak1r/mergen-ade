# UI Overlay & Utility Guidelines

## Window Close Confirmation Guidelines
- **Window close confirmation must not early-return before rendering.** When intercepting a close request (`ViewportCommand::CancelClose`), do not use `return` to exit the update function early. The confirmation popup should be rendered in the same frame by setting the state flag and allowing the normal render path to continue.
- **Avoid `request_repaint()` after showing the confirmation popup.** Since the popup will be drawn later in the same update cycle by `draw_exit_confirm_popup()`, an explicit repaint request is unnecessary and can cause visual flicker.
- **Popup overlay should use appropriate layer order.** Modal confirmation dialogs should render above the main UI surface; use `egui::Order::Foreground` or appropriate z-ordering for overlay backdrops to ensure the modal appears on top without obscuring the underlying content during the same-frame transition.

## Clipboard Paste Guidelines
- **Terminal paste should preserve text fallback.** Text clipboard paste must continue to use the existing queued paste path.
- **Clipboard images paste as paths.** If the clipboard contains an image file path, paste the image path into the terminal instead of image bytes.
- **On Windows, copied image files from Explorer must be read from CF_HDROP before bitmap materialization.** This preserves the original file path and avoids creating duplicate saved images.
- **Prefer the original copied image file path over saving a duplicate bitmap.** When CF_HDROP provides an image file path, use that path directly instead of materializing a new screenshot.
- **Bitmap clipboard images must be materialized.** If the clipboard contains bitmap/image data without a file path, save it to a user-accessible screenshots folder and paste the saved file path.
- **Do not block normal paste on image failures.** If image extraction or saving fails, fall back to text clipboard paste when text exists; otherwise show a clear status-line error.
- **Generated image paths must be terminal-safe.** Normalize generated paths consistently and avoid control characters in filenames.
- **Clipboard image path normalization must reject control characters and produce terminal-safe paths.**
- **CF_HDROP handle must be used directly without GlobalLock.** The handle returned by `GetClipboardData(CF_HDROP)` is an `HDROP` handle; pass it directly to `DragQueryFileW`. Do not call `GlobalLock` on it - that returns a pointer to `DROPFILES`, not an `HDROP` handle.
- **Clipboard close must be guaranteed on all return paths.** Use a scope guard or RAII pattern to ensure `CloseClipboard()` is called after successful `OpenClipboard()`, even on early returns or errors.

## Resizable Panel Guidelines
- **Side panels should be horizontally resizable.** Project Explorer, Check-list, and Browser panels should allow mouse-driven width resizing while keeping full-height SidePanel behavior.
- **Panel widths are persisted UI config.** Store user-resized widths in `UiConfig` and clamp them to safe min/max ranges.
- **Do not make settings popups resizable.** Modal/pop-up windows such as Settings must keep their fixed sizing unless explicitly redesigned.
- **Avoid per-frame config writes.** Persist resized panel widths only when width changes meaningfully to prevent excessive disk writes.
- **Config recovery must preserve persisted panel width fields.** Ensure `recover_config_state()` preserves `project_explorer_width`, `checklist_panel_width`, and `browser_panel_width` when `pending_config_changes.ui` is true.
- **Resize handle visuals must match the dark theme on every resizable panel.** Override egui's default bright white resize handle (`fg_stroke`) with a dim overlay (`Color32::from_rgb(45, 45, 45)`) on the panel edge so the separator does not appear harsh against the dark background. The overlay must be painted in the foreground layer after the panel renders for **Project Explorer**, **Check-list**, and **Browser** panels.
- **Panel resize chrome must be disabled while any modal or popup is open.** Use a centralized helper (e.g., `panel_resize_chrome_enabled`) that checks Settings, exit confirmation, terminal history, foreground message, create worktree, checklist floating, egui popup, and context menu state. Gate both `SidePanel::resizable` and the custom overlay paint so the resize handle does not compete with overlay interactions.

## OS Notifications Guidelines

- **Windows notifications use `Shell_NotifyIconW` balloon popups** as the primary delivery method. This produces a visible system-tray notification popup without requiring AppUserModelID or Start Menu shortcuts.
- **Fallback to `FlashWindowEx`** taskbar flash occurs automatically if the tray icon cannot be initialized or the balloon fails to send. Both paths are attempted in the same attention event.
- **Notification triggers** are tied to AI CLI attention state transitions: when Factory Droid, Codex CLI, OpenCode, or Claude status changes to `AiCliStatus::Attention`, a notification is queued.
- **Attention reasons map to notification kinds** (`Permission`, `TurnComplete`, `SessionError`) so that settings filters (`on_permission`, `on_turn_complete`, `on_session_error`) actually gate delivery. Factory Droid `AskUser` and `SpecificationApproval` map to `Permission`; completion/stop attention with no explicit reason maps to `TurnComplete`.
- **Focus-aware delivery** via `ctx.input(|i| i.viewport().focused)`. When `only_when_unfocused` is enabled, notifications are suppressed if the app is currently focused.
- **Cooldown deduplication** prevents notification spam. Each terminal tracks its last notification time; new notifications are blocked until `cooldown_secs` has elapsed.
- **Pending notification pattern**: Status apply functions set `pending_os_notification: Option<PendingOsNotification>` (carrying `terminal_id`, `tool`, and `kind`) since they lack access to `ctx`. The `update` loop then processes the pending notification with full access to the egui context.
- **Tray icon lifecycle**: A hidden tray icon is added lazily on first notification via `NIM_ADD`, updated with `NIM_MODIFY` + `NIF_INFO` for each balloon, and removed with `NIM_DELETE` on app exit (`on_exit`).
- **Config structure**: `OsNotificationConfig` contains `enabled`, `only_when_unfocused`, `on_permission`, `on_turn_complete`, `on_session_error`, and `cooldown_secs` fields.
- **Settings UI**: Notifications section accessible via Settings popup with toggle for each trigger type and cooldown duration control. Description text must mention both the Windows notification popup and the taskbar flash fallback.
- **Notification click must preserve window state**: `restore_window_for_os_notification_click()` must only call `ShowWindow(hwnd, SW_RESTORE)` when the window is currently minimized (`IsIconic(hwnd) != 0`). Calling `SW_RESTORE` on a visible maximized or normal window un-maximizes/resizes it. After restoring (if needed), always call `SetForegroundWindow(hwnd)` to bring the app to the foreground without changing its size.

## UI Popup & Overlay Guidelines

- **Popup menus must have an explicit opaque backing.** Do not rely solely on `Frame::menu` fill; paint a full `SURFACE_BG` rect inside the popup closure before content rows. Compute the rect size deterministically from the expected content height (rows, gaps, padding) so the backing always covers the entire popup area and never leaves transparent gaps. This prevents background elements from bleeding through gaps or margins.
- **Foreground overlays must yield to open popups.** `Order::Foreground` painters such as resize-handle overlays and sidebar seam fixes must check `ctx.memory(|m| m.any_popup_open())` and skip painting while any popup is open. Popups use the same layer order; later foreground shapes paint over them and create visual glitches.
- **Transient toast width must be text-driven.** The bottom-right status toast (`draw_transient_toast`) should measure the message text and size the content area to `text_width + padding`, clamped between a small minimum (~140 px) and a maximum (~640 px). Do not use a large fixed minimum that causes short messages like "Sent: /review-guard" to appear in an oversized box.
