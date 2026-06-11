# Mergen ADE

- Electron + React desktop IDE
- OpenCode ACP: açılışta seçili proje için otomatik ACP; proje başına 1 standby (ısıtılmış, thread listesinde yok); New Chat hazır standby’ı kullanır, sonra yeni standby arka planda
- ACP welcome: centered composer when empty; bottom chat after first message
- ACP welcome context row: proje adı ComboBox (aşağı ok) → Terminal Manager FG’ye geçiş; welcome Enter/send → foreground terminal CLI’ye gönderim (ACP mesajı değil)
- No mic / Plan New Idea / Run in Cloud in ACP UI
- Smart Input: OpenCode + Claude Code (`cc`) FG terminallerinde; kuyruk/draft/attachment/auto-dispatch aynı; Claude'da Build/Plan pill ve question card yok; Tab PTY'ye passthrough
