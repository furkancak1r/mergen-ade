# Fixes

## ACP welcome layout (2026-06-02)

- Wrong: composer always bottom-aligned with thread/header chrome
- Correct: `acp_shows_welcome_center()` centers context chips + tall composer; switches to chat layout after first message

## ACP composer icons + send (2026-06-02)

- Wrong: `horizontal_centered` footer; `TEXT_MUTED` icons; send after model without spacer
- Correct: `horizontal` + `add_space` before send; `ACP_COMPOSER_ICON_COLOR` / `ACP_COMPOSER_SEND_ACTIVE_FILL`; context chips split icon vs label
