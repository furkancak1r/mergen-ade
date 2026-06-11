# File Editor Guidelines

## File Editor Guidelines
- Long editor content must be wrapped in a stable-id `ScrollArea` (e.g., `FILE_EDITOR_SCROLL_ID`).
- `TextEdit::multiline` must allocate enough rows for the full line count (`max(visible_rows, line_count)`), not only the visible viewport.
- Preserve `FILE_EDITOR_INPUT_ID` focus isolation so editor input never routes to terminal capture.
- Dirty detection must compare against `saved_text` (taking snapshots before mutable borrow to avoid borrow issues).
- Add regression tests for scroll behavior with files longer than screen height.
- Drag-selecting text in the file editor must continue tracking pointer position when the pointer leaves the editor viewport.
- While file editor text selection drag is active near viewport edges, the editor `ScrollArea` must autoscroll with `ui.scroll_with_delta` and request repaint.
- File editor selection drag state must be runtime-only (`FileEditorState.selection_drag_active`) and reset when opening, closing, or navigating between files.
- Use `input.pointer.interact_pos()` as the pointer fallback for file editor drag autoscroll; do not rely only on `TextEdit` hover state.
- Use shared `selection_edge_autoscroll_delta()` helper for consistent edge autoscroll behavior across terminal and file editor.
- Add regression tests for file editor selection drag state transitions and edge autoscroll delta behavior.
- Editor header buttons must use high-contrast `editor_header_icon_button()` helper to ensure visibility against dark surfaces.
- Editor has separate "open" (has active buffer) and "visible" (shown in main area) states.
- Use `FileEditorState::is_visible()` for main area rendering, `is_open()` for buffer existence checks.
- Call `FileEditorState::hide()` when switching to terminal; call `close()` when truly closing.
- `set_active_terminal()` must hide the editor before early-return checks to ensure terminal click always switches from editor.
- File editor selection-aware context menus must use `TextEdit::show()`/`TextEditOutput`, not plain `ui.add(text_edit)`, so cursor state and selection data remain available.
- Right-clicking `egui::TextEdit` can collapse the active selection before menu handling; capture the pre-click `TextEditState` selection and restore non-empty selections when opening editor copy menus.
- Copying selected editor text must use character cursor ranges (`CCursorRange`/`CursorRange`) and char-safe slicing; never byte-index slice editor text.
- Defer clipboard feedback that mutates `AdeApp` state until after the active editor buffer mutable borrow ends.
- Detect editor copy requests using `Event::Copy` (generated when TextEdit handles the copy shortcut) rather than just raw key detection, with `Ctrl+C`/`Command+C` as fallback. This ensures copy feedback works regardless of whether TextEdit consumed the shortcut.
- Add regression tests for editor context menu selection preservation, Unicode-safe selected-text extraction, and empty-selection behavior.
- Test both `Event::Copy` and raw `Ctrl+C`/`Command+C` paths for editor copy feedback.
