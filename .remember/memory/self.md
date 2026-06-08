# Fixes

## Smart Input soru kartı kaldırıldı (2026-06-08)

- Wrong: OpenCode soruları Smart Input footer kartı + hook answer bridge ile yanıtlanıyordu
- Correct: Soru kartı, pending question state ve `/answer` bridge kaldırıldı; `QuestionAsked` attention + terminal TUI klavye yönlendirmesi korundu

## Smart Input Kimi thought-loop guard (2026-06-08)

- Wrong: After Done auto-dispatch could send the next queued task while Kimi K2.6 was stuck in a repetitive thinking loop that briefly reported Idle/TurnComplete
- Correct: `opencode_thinking_guard` samples terminal snapshot thought windows; `smart_input_auto_dispatch_ready_for_terminal` blocks auto-dispatch when repetitive pattern or `opencode_loop_limit_emitted`; manual Send Now unchanged

## Smart Input queued task mode switch (2026-06-08)

- Wrong: `process_smart_input_queues` ignored `task.mode`; prompts sent in current OpenCode TUI mode (Plan)
- Correct: `smart_input_prepare_opencode_mode` sends Tab to TUI when task mode differs, waits 250ms settle, then dispatches

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

## ACP composer send sağ sabit (2026-06-02)

- Wrong: tek `horizontal` satır; sabit gap; model `.clamp(60,140)` → dar pencerede send kapsül dışına taşıyordu
- Correct: `acp_composer_footer_layout` + sol grup (`allocate_ui_with_layout`) + send sağda; `footer_ui.set_clip_rect`

## ACP composer model genişliği (2026-06-02)

- Wrong: welcome modda `model_w = flex_budget`; MODEL 60–140px
- Correct: MODEL 18–42px; welcome `flex_budget.min(MODEL_MAX)`; dar ekran oranı 0.135
