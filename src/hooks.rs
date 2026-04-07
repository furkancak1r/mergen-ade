use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const FACTORY_DROID_TERMINAL_ID_ENV_VAR: &str = "MERGEN_ADE_TERMINAL_ID";
pub const FACTORY_DROID_HOOKS_DIR_ENV_VAR: &str = "MERGEN_ADE_FACTORY_DROID_HOOKS_DIR";
pub const FACTORY_DROID_HOOK_INBOX_TOKEN_ENV_VAR: &str = "MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCliTool {
    FactoryDroid,
}

impl AiCliTool {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::FactoryDroid => "Factory Droid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiHookEvent {
    IdlePrompt,
    Running,
    Attention,
}

impl AiHookEvent {
    #[allow(dead_code)]
    pub fn as_status(&self) -> AiCliStatus {
        match self {
            AiHookEvent::IdlePrompt => AiCliStatus::Inactive,
            AiHookEvent::Running => AiCliStatus::Running,
            AiHookEvent::Attention => AiCliStatus::Attention,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiCliStatus {
    #[default]
    Inactive,
    Running,
    Attention,
}

impl AiCliStatus {
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn indicator(&self) -> &'static str {
        match self {
            AiCliStatus::Inactive => "○",
            AiCliStatus::Running => "●",
            AiCliStatus::Attention => "◐",
        }
    }

    pub fn tooltip(&self, tool: AiCliTool) -> String {
        match self {
            AiCliStatus::Inactive => format!("{} - Idle", tool.display_name()),
            AiCliStatus::Running => format!("{} - Working...", tool.display_name()),
            AiCliStatus::Attention => format!("{} - Waiting for you...", tool.display_name()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatusIndicators {
    pub idle: String,
    pub running: String,
}

impl Default for AiStatusIndicators {
    fn default() -> Self {
        Self {
            idle: "○".to_string(),
            running: "●".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiHookConfig {
    pub tool: AiCliTool,
    pub enabled: bool,
    pub detection_commands: Vec<String>,
    pub running_hook_events: Vec<String>,
    pub inactive_hook_events: Vec<String>,
    pub show_indicators: bool,
    pub status_indicators: AiStatusIndicators,
    /// Pattern to match in terminal title for "working" state (e.g., "[Working...]")
    pub working_title_pattern: String,
    /// Pattern to match in terminal title for "idle" state (e.g., "[Idle]")
    pub idle_title_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectAiConfig {
    pub hooks_enabled: bool,
    pub tool_overrides: BTreeMap<AiCliTool, bool>,
}

impl ProjectAiConfig {
    #[allow(dead_code)]
    pub fn is_enabled(&self, tool: AiCliTool) -> bool {
        self.tool_overrides
            .get(&tool)
            .copied()
            .unwrap_or(self.hooks_enabled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiHooksConfig {
    pub global_enabled: bool,
    pub hooks: Vec<AiHookConfig>,
    pub project_overrides: BTreeMap<u64, BTreeMap<AiCliTool, bool>>,
}

impl Default for AiHooksConfig {
    fn default() -> Self {
        Self::with_factory_droid_defaults()
    }
}

#[allow(dead_code)]
impl AiHooksConfig {
    pub fn with_factory_droid_defaults() -> Self {
        Self {
            global_enabled: true,
            hooks: vec![AiHookConfig {
                tool: AiCliTool::FactoryDroid,
                enabled: true,
                detection_commands: vec!["droid".to_string(), "factory".to_string()],
                running_hook_events: vec!["UserPromptSubmit".to_string()],
                inactive_hook_events: vec![
                    "Stop".to_string(),
                    "Notification".to_string(),
                    "idle_prompt".to_string(),
                    "permission_prompt".to_string(),
                ],
                show_indicators: true,
                status_indicators: AiStatusIndicators::default(),
                // Factory Droid sets terminal title to "[Working...]" when active
                working_title_pattern: "[Working".to_string(),
                // Factory Droid sets terminal title to "[Idle]" when waiting
                idle_title_pattern: "[Idle]".to_string(),
            }],
            project_overrides: BTreeMap::new(),
        }
    }

    pub fn config_for(&self, tool: AiCliTool) -> Option<&AiHookConfig> {
        self.hooks.iter().find(|h| h.tool == tool && h.enabled)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AiCliSession {
    pub tool: Option<AiCliTool>,
    pub status: AiCliStatus,
    pub last_event: Option<AiHookEvent>,
    #[allow(dead_code)]
    pub last_activity_at: Option<f64>,
    /// Accumulates partial PTY chunks to reconstruct complete event lines
    pub pending_line: String,
}

impl AiCliSession {
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.tool.is_some() && self.status != AiCliStatus::Inactive
    }

    pub fn detect_tool(&mut self, text: &str, config: &AiHooksConfig) -> bool {
        if self.tool.is_some() {
            return false;
        }

        if let Some((tool, _, _)) = parse_hook_event(text, config) {
            self.tool = Some(tool);
            return true;
        }

        false
    }
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            while let Some(&ch) = chars.peek() {
                chars.next();
                if ch.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }

        result.push(c);
    }

    result
}

fn normalize_hook_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Extract complete lines from a buffer, returning (complete_lines, incomplete_tail).
/// A line is complete if it ends with newline, or if it contains a full bracket pattern
/// that could be a hook event.
fn extract_complete_lines(buffer: &str) -> (Vec<&str>, &str) {
    let mut complete_lines = Vec::new();
    let mut search_start = 0;

    loop {
        // Find the next newline
        if let Some(newline_pos) = buffer[search_start..].find('\n') {
            let line_end = search_start + newline_pos;
            let line = &buffer[search_start..line_end];
            complete_lines.push(line);
            search_start = line_end + 1;
        } else {
            // No more newlines - check if the remaining text is a complete hook pattern
            let tail = &buffer[search_start..];
            if is_complete_hook_pattern(tail) {
                // Include this as a complete line
                complete_lines.push(tail);
                return (complete_lines, "");
            } else {
                // Keep as incomplete tail
                return (complete_lines, tail);
            }
        }
    }
}

/// Check if text contains a potentially complete hook event pattern.
/// This prevents discarding chunks that have partial bracket patterns.
fn is_complete_hook_pattern(text: &str) -> bool {
    let text_lower = text.to_lowercase();

    // Check for complete bracket patterns (Droid formats only)
    let bracket_patterns = ["[droid-hook:", "[factory-droid-hook:"];

    for pattern in &bracket_patterns {
        if let Some(start) = text_lower.find(pattern) {
            let after_start = &text_lower[start + pattern.len()..];
            if after_start.contains(']') {
                return true;
            }
        }
    }

    false
}

fn title_status_for_config(
    config: &AiHookConfig,
    title: &str,
) -> Option<(AiCliStatus, AiHookEvent)> {
    if title.contains(&config.working_title_pattern) {
        return Some((AiCliStatus::Running, AiHookEvent::Running));
    }

    if title.contains(&config.idle_title_pattern) {
        return Some((AiCliStatus::Attention, AiHookEvent::Attention));
    }

    None
}

fn detect_tool_from_hook_text(text: &str, config: &AiHooksConfig) -> Option<AiCliTool> {
    let text_lower = text.to_ascii_lowercase();
    for hook_config in &config.hooks {
        if !hook_config.enabled {
            continue;
        }
        if hook_config
            .detection_commands
            .iter()
            .any(|cmd| text_lower.contains(&cmd.to_ascii_lowercase()))
        {
            return Some(hook_config.tool);
        }
    }
    None
}

fn parse_hook_event(text: &str, config: &AiHooksConfig) -> Option<(AiCliTool, String, bool)> {
    let clean = strip_ansi(text);
    let text_lower = clean.to_lowercase();

    let extract_name = |value: &str| -> String { normalize_hook_name(value) };

    for (prefix, is_notification) in [
        ("[droid-hook:event=", false),
        ("[factory-droid-hook:event=", false),
        ("[droid-hook:notification=", true),
        ("[factory-droid-hook:notification=", true),
    ] {
        if let Some(pos) = text_lower.find(prefix) {
            let after = &clean[pos + prefix.len()..];
            let name: String = after
                .chars()
                .take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
                .collect();
            if !name.is_empty() {
                let tool = detect_tool_from_hook_text(prefix, config)?;
                return Some((tool, extract_name(&name), is_notification));
            }
        }
    }

    None
}

fn names_match(left: &str, right: &str) -> bool {
    normalize_hook_name(left) == normalize_hook_name(right)
}

pub struct AiHookManager {
    config: Arc<AiHooksConfig>,
    sessions: Mutex<BTreeMap<u64, AiCliSession>>,
}

#[allow(dead_code)]
impl AiHookManager {
    pub fn new(config: AiHooksConfig) -> Self {
        Self {
            config: Arc::new(config),
            sessions: Mutex::default(),
        }
    }

    pub fn ai_activity_started(&self, terminal_id: u64) -> Option<(AiCliTool, AiCliStatus)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(terminal_id).or_default();

        if session.tool.is_none() {
            let auto_detect =
                session.status != AiCliStatus::Inactive || session.last_event.is_some();
            if auto_detect {
                return None;
            }
        }

        let tool = session.tool?;
        if session.status != AiCliStatus::Running {
            session.status = AiCliStatus::Running;
            session.last_event = Some(AiHookEvent::Running);
            return Some((tool, AiCliStatus::Running));
        }

        None
    }

    pub fn ai_waiting_for_user(&self, terminal_id: u64) -> Option<(AiCliTool, AiCliStatus)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(terminal_id).or_default();

        let tool = session.tool?;
        if session.status != AiCliStatus::Attention {
            session.status = AiCliStatus::Attention;
            session.last_event = Some(AiHookEvent::Attention);
            return Some((tool, AiCliStatus::Attention));
        }

        None
    }

    pub fn user_interacted(&self, terminal_id: u64) -> Option<(AiCliTool, AiCliStatus)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(terminal_id).or_default();

        if session.status == AiCliStatus::Attention {
            session.status = AiCliStatus::Inactive;
            session.last_event = Some(AiHookEvent::IdlePrompt);
            return Some((session.tool?, AiCliStatus::Inactive));
        }

        None
    }

    pub fn set_tool(&self, terminal_id: u64, tool: AiCliTool) {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(terminal_id).or_default();
        session.tool = Some(tool);
        if session.status == AiCliStatus::default() {
            session.status = AiCliStatus::Inactive;
        }
    }

    pub fn update(
        &self,
        terminal_id: u64,
        text: &str,
    ) -> Option<(AiCliTool, AiCliStatus, Option<AiHookEvent>)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(terminal_id).or_default();

        // Step 1: Detect tool if not already detected
        if session.tool.is_none() {
            if !session.detect_tool(text, &self.config) {
                return None;
            }
            // Tool detected - fall through to check for hook events
        }

        let tool = session.tool?;
        let config = self.config.config_for(tool)?;

        // Step 2: Accumulate text into pending_line buffer
        session.pending_line.push_str(text);

        // Step 3: Process complete lines from the buffer
        // A complete line is one that ends with newline, or contains a full bracket pattern
        let buffer = session.pending_line.clone();
        let (complete_lines, incomplete_tail) = extract_complete_lines(&buffer);

        for line in complete_lines {
            if let Some((parsed_tool, event_name, is_notification)) =
                parse_hook_event(&line, &self.config)
            {
                if parsed_tool != tool {
                    continue;
                }
                let is_running_event = config
                    .running_hook_events
                    .iter()
                    .any(|candidate| names_match(candidate, &event_name));
                let is_attention_event = config
                    .inactive_hook_events
                    .iter()
                    .any(|candidate| names_match(candidate, &event_name))
                    || is_notification;

                if is_running_event {
                    session.status = AiCliStatus::Running;
                    session.last_event = Some(AiHookEvent::Running);
                    session.pending_line = incomplete_tail.to_string();
                    return Some((tool, AiCliStatus::Running, Some(AiHookEvent::Running)));
                }

                if is_attention_event {
                    session.status = AiCliStatus::Attention;
                    session.last_event = Some(AiHookEvent::Attention);
                    session.pending_line = incomplete_tail.to_string();
                    return Some((tool, AiCliStatus::Attention, Some(AiHookEvent::Attention)));
                }
            }
        }

        // Keep incomplete tail for next chunk
        session.pending_line = incomplete_tail.to_string();

        // Tool was just detected, but no hook event in this chunk - status stays Inactive
        None
    }

    /// Update AI status based on terminal title changes.
    /// This is called when the terminal title changes and checks for AI-specific patterns.
    pub fn update_from_title(
        &self,
        terminal_id: u64,
        title: &str,
    ) -> Option<(AiCliTool, AiCliStatus, Option<AiHookEvent>)> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(terminal_id).or_default();
        let matched = if let Some(tool) = session.tool {
            let config = self.config.config_for(tool)?;
            title_status_for_config(config, title).map(|(status, event)| (tool, status, event))
        } else {
            self.config.hooks.iter().find_map(|config| {
                if !config.enabled {
                    return None;
                }

                title_status_for_config(config, title)
                    .map(|(status, event)| (config.tool, status, event))
            })
        }?;

        let (tool, status, event) = matched;
        session.tool = Some(tool);

        if session.status != status || session.last_event != Some(event) {
            session.status = status;
            session.last_event = Some(event);
            return Some((tool, status, Some(event)));
        }

        None
    }

    pub fn session(&self, terminal_id: u64) -> Option<AiCliSession> {
        self.sessions.lock().unwrap().get(&terminal_id).cloned()
    }

    pub fn reset_session(&self, terminal_id: u64) {
        self.sessions.lock().unwrap().remove(&terminal_id);
    }

    pub fn is_enabled(&self) -> bool {
        self.config.global_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_hook_name_maps_camel_case_and_snake_case() {
        assert_eq!(normalize_hook_name("UserPromptSubmit"), "userpromptsubmit");
        assert_eq!(
            normalize_hook_name("user_prompt_submit"),
            "userpromptsubmit"
        );
        assert_eq!(normalize_hook_name("permission_prompt"), "permissionprompt");
        assert_eq!(normalize_hook_name("Stop"), "stop");
    }

    #[test]
    fn parse_hook_event_handles_official_names() {
        let config = AiHooksConfig::with_factory_droid_defaults();
        assert_eq!(
            parse_hook_event("[droid-hook:event=UserPromptSubmit]", &config),
            Some((
                AiCliTool::FactoryDroid,
                "userpromptsubmit".to_string(),
                false
            ))
        );
        assert_eq!(
            parse_hook_event("[factory-droid-hook:notification=idle_prompt]", &config),
            Some((AiCliTool::FactoryDroid, "idleprompt".to_string(), true))
        );
        assert_eq!(
            parse_hook_event("[droid-hook:notification=permission_prompt]", &config),
            Some((
                AiCliTool::FactoryDroid,
                "permissionprompt".to_string(),
                true
            ))
        );
    }

    #[test]
    fn parse_hook_event_rejects_loose_formats() {
        let config = AiHooksConfig::with_factory_droid_defaults();
        assert_eq!(parse_hook_event("[hook] UserPromptSubmit", &config), None);
        assert_eq!(
            parse_hook_event("[notification] matcher=idle_prompt", &config),
            None
        );
        assert_eq!(parse_hook_event("Stop", &config), None);
        assert_eq!(parse_hook_event("UserPromptSubmit", &config), None);
    }

    #[test]
    fn update_keeps_hook_marker_in_mixed_detection_chunk() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());

        // When both detection and hook event are in same chunk, hook takes precedence
        let result = manager.update(1, "droid ... [droid-hook:event=UserPromptSubmit]");
        assert_eq!(
            result,
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
    }

    #[test]
    fn task_completed_does_not_affect_status() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=UserPromptSubmit]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=TaskCompleted]"),
            None
        );
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Running
        );
    }

    #[test]
    fn droid_detection_requires_official_hook_prefix() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());

        assert_eq!(manager.update(1, "droid"), None);
        let session = manager.session(1).unwrap();
        assert_eq!(session.tool, None);
        assert_eq!(session.status, AiCliStatus::Inactive);

        assert_eq!(manager.update(1, "[droid-hook:event=SessionStart]"), None);
        let session = manager.session(1).unwrap();
        assert_eq!(session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(session.status, AiCliStatus::Inactive);
    }

    #[test]
    fn user_prompt_submit_triggers_running() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );
        assert_eq!(
            manager.update(terminal_id, "[factory-droid-hook:event=UserPromptSubmit]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Running
        );
    }

    #[test]
    fn stop_triggers_attention() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );
        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=UserPromptSubmit]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );

        assert_eq!(
            manager.update(terminal_id, "[factory-droid-hook:event=Stop]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Attention
        );
    }

    #[test]
    fn notification_triggers_attention() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );
        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=UserPromptSubmit]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );

        assert_eq!(
            manager.update(terminal_id, "[factory-droid-hook:notification=idle_prompt]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Attention
        );
    }

    #[test]
    fn permission_prompt_triggers_attention() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );
        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=UserPromptSubmit]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );

        assert_eq!(
            manager.update(
                terminal_id,
                "[factory-droid-hook:notification=permission_prompt]"
            ),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Attention
        );
    }

    #[test]
    fn user_interaction_clears_attention() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );
        assert_eq!(
            manager.update(terminal_id, "[factory-droid-hook:event=Stop]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );

        // User interaction clears attention
        assert_eq!(
            manager.user_interacted(terminal_id),
            Some((AiCliTool::FactoryDroid, AiCliStatus::Inactive))
        );
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Inactive
        );
    }

    #[test]
    fn line_buffer_accumulates_across_chunks() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]"),
            None
        );
        assert_eq!(manager.update(terminal_id, "some text "), None);
        assert_eq!(manager.update(terminal_id, "more text "), None);

        let session = manager.session(terminal_id).unwrap();
        assert_eq!(session.status, AiCliStatus::Inactive);

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=UserPromptSubmit]\n"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
    }

    #[test]
    fn chunked_official_event_detection() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]\n"),
            None
        );

        assert_eq!(manager.update(terminal_id, "some out"), None);
        assert_eq!(
            manager.update(terminal_id, "put\n[droid-hook:event=User"),
            None
        );

        assert_eq!(
            manager.update(terminal_id, "PromptSubmit]\n"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
    }

    #[test]
    fn extract_complete_lines_handles_newlines() {
        let (complete, tail) = extract_complete_lines("line1\nline2\nline3");
        assert_eq!(complete, vec!["line1", "line2"]);
        assert_eq!(tail, "line3");
    }

    #[test]
    fn extract_complete_lines_handles_incomplete_final_line() {
        let (complete, tail) = extract_complete_lines("line1\nline2 with no newline");
        assert_eq!(complete, vec!["line1"]);
        assert_eq!(tail, "line2 with no newline");
    }

    #[test]
    fn extract_complete_lines_handles_complete_hook_pattern() {
        // When there's a complete bracket pattern at the end, treat as complete
        let (complete, tail) = extract_complete_lines("some text [droid-hook:event=Stop]");
        assert_eq!(complete, vec!["some text [droid-hook:event=Stop]"]);
        assert_eq!(tail, "");
    }

    #[test]
    fn extract_complete_lines_keeps_partial_hook_pattern_buffered() {
        let (complete, tail) = extract_complete_lines("some text [droid-hook:event=User");
        assert!(complete.is_empty());
        assert_eq!(tail, "some text [droid-hook:event=User");
    }

    #[test]
    fn update_from_title_detects_working_pattern_without_prior_detection() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "C:\\Users\\test> [Working...]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
        let session = manager.session(terminal_id).expect("seeded session");
        assert_eq!(session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(session.status, AiCliStatus::Running);
    }

    #[test]
    fn update_from_title_detects_idle_pattern_without_prior_detection() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "C:\\Users\\test> [Idle]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("seeded session");
        assert_eq!(session.tool, Some(AiCliTool::FactoryDroid));
        assert_eq!(session.status, AiCliStatus::Attention);
    }

    #[test]
    fn update_from_title_ignores_normal_titles() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(manager.update_from_title(terminal_id, "git status"), None);
        assert_eq!(
            manager.session(terminal_id).unwrap().status,
            AiCliStatus::Inactive
        );
    }

    #[test]
    fn update_from_title_transitions_back_to_attention_after_running() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "[Working...]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
        assert_eq!(
            manager.update_from_title(terminal_id, "[Idle]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
    }
}
