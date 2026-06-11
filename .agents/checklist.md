# Check-list Panel Guidelines

## Check-list Panel Guidelines
- **Check-list is a floating popup, not a fixed side panel.** It opens as an `egui::Window` anchored to `Align2::RIGHT_BOTTOM` with a close button on the title bar. It is triggered by the activity rail icon and does not reduce the main terminal area width.
- **Project headers are collapsible accordion rows.** Each project name in the Check-list popup is a clickable header that toggles the visibility of its checklist items. Clicking the header (excluding the copy button) expands or collapses the items.
- **Collapse state is per-project and runtime-only.** `checklist_collapsed_by_project` tracks open/closed state independently for each project. Default is expanded. State is not persisted across app restarts.
- **Copy-all button sits on the right side of the header.** Clicking the copy button copies all items for that project without toggling the accordion.
- **All other Check-list item behaviors remain unchanged** (checkbox removal, individual item copy, scroll clamp, tooltips).
