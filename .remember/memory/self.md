# Fixes

## ACP welcome layout (2026-06-02)

- Wrong: composer always bottom-aligned with thread/header chrome
- Correct: `acp_shows_welcome_center()` centers context chips + tall composer; switches to chat layout after first message

## ACP composer icons + send (2026-06-02)

- Wrong: `horizontal_centered` footer; `TEXT_MUTED` icons; send after model without spacer
- Correct: `horizontal` + `add_space` before send; `ACP_COMPOSER_ICON_COLOR` / `ACP_COMPOSER_SEND_ACTIVE_FILL`; context chips split icon vs label

## ACP composer footer layout (2026-06-02)

- Wrong: welcome 136px capsule + footer in same horizontal row; chat capsule 48px < 36px controls; `add_space(available_width)` pushed send off layout
- Correct: `ACP_COMPOSER_PADDING_*`, `FOOTER_HEIGHT`, welcome text row + fixed footer strip; plus/send `add_sized(ctrl_h)`; widths precomputed from `footer_rect.width()`

## ACP standby pool (2026-06-02)

- Wrong: startup/foreground `force_new` + cold `spawn_acp_process` -> "Waiting for session..."
- Correct: startup only `ensure_acp_standby`; `maybe_promote_standby_to_active` on SessionCreated; foreground `force_new: false`; `promote_standby_to_active_if_present`; New Chat only uses standby pool, no cold active spawn

## ACP welcome project picker (2026-06-02)

- Wrong: static project chip label only; welcome Enter always sent ACP prompt
- Correct: ComboBox + chevron on project chip; `focus_project_in_terminal_manager`; welcome Enter/send routes via `submit_acp_welcome_to_foreground_terminal` (spawn opencode FG if needed)
