use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const FACTORY_DROID_TERMINAL_ID_ENV_VAR: &str = "MERGEN_ADE_TERMINAL_ID";
pub const FACTORY_DROID_HOOKS_DIR_ENV_VAR: &str = "MERGEN_ADE_FACTORY_DROID_HOOKS_DIR";
pub const FACTORY_DROID_HOOK_INBOX_TOKEN_ENV_VAR: &str = "MERGEN_ADE_FACTORY_DROID_INBOX_TOKEN";
const MAX_PENDING_HOOK_LINE_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCliTool {
    FactoryDroid,
    CodexCli,
    OpenCode,
    Claude,
}

impl AiCliTool {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::FactoryDroid => "Factory Droid",
            Self::CodexCli => "Codex CLI",
            Self::OpenCode => "OpenCode",
            Self::Claude => "Claude Code",
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
            hooks: vec![
                AiHookConfig {
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
                },
                AiHookConfig {
                    tool: AiCliTool::CodexCli,
                    enabled: true,
                    // Codex is tracked through explicit launch and status paths,
                    // not broad PTY text matching.
                    detection_commands: Vec::new(),
                    running_hook_events: Vec::new(),
                    inactive_hook_events: Vec::new(),
                    show_indicators: true,
                    status_indicators: AiStatusIndicators::default(),
                    working_title_pattern: String::new(),
                    idle_title_pattern: String::new(),
                },
                AiHookConfig {
                    tool: AiCliTool::OpenCode,
                    enabled: true,
                    // OpenCode is tracked through plugin-based and hook-based events per documentation
                    // Events: session.idle (completion), permission.asked (interactive),
                    //         tool.execute.before (running), session.error (error)
                    detection_commands: Vec::new(),
                    // tool.execute.before signals work is starting
                    running_hook_events: vec!["tool.execute.before".to_string()],
                    // session.idle is the canonical completion signal
                    // permission.asked is for interactive prompts
                    // session.error indicates something went wrong
                    inactive_hook_events: vec![
                        "session.idle".to_string(),
                        "permission.asked".to_string(),
                        "session.error".to_string(),
                    ],
                    show_indicators: true,
                    status_indicators: AiStatusIndicators::default(),
                    // Title patterns are secondary/backup signals
                    working_title_pattern: "Working".to_string(),
                    idle_title_pattern: "Idle".to_string(),
                },
            ],
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

// ─── Claude Code Title-Based Detection ─────────────────────────────────────
// Orca-compatible Claude Code status detection via terminal title patterns.
// Claude Code sets OSC titles to task descriptions with status prefixes:
// - ✳ (eight-spoked asterisk) = idle
// - . (dot space) = working
// - * (asterisk space) = idle
// - Braille spinner (U+2800-U+28FF) = working
// - "action required"/"permission"/"waiting" + agent name = permission

/// Normalized Claude Code status values (Orca-compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeTransportStatus {
    Working,
    Idle,
    Permission,
}

/// Claude Code attention reasons for UI differentiation.
/// Simplified to minimal variants needed for current behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeAttentionReason {
    PermissionAsked,
}

impl ClaudeAttentionReason {
    pub fn tooltip(&self) -> &'static str {
        match self {
            Self::PermissionAsked => "Claude Code - Permission needed",
        }
    }
}

const CLAUDE_IDLE_PREFIX: char = '\u{2733}'; // ✳ eight-spoked asterisk

/// Returns true if the character is a Braille pattern (U+2800–U+28FF)
fn is_braille_spinner(c: char) -> bool {
    let code_point = c as u32;
    (0x2800..=0x28FF).contains(&code_point)
}

/// Check if title contains any Braille spinner characters
fn contains_braille_spinner(title: &str) -> bool {
    title.chars().any(is_braille_spinner)
}

/// Returns true if the title contains an agent name (case-insensitive)
fn contains_agent_name(title: &str) -> bool {
    let lower = title.to_lowercase();
    ["claude", "codex", "gemini", "opencode", "aider"]
        .iter()
        .any(|name| lower.contains(name))
}

/// Returns true if the title contains any of the given keywords (case-insensitive)
fn contains_any(title: &str, keywords: &[&str]) -> bool {
    let lower = title.to_lowercase();
    keywords.iter().any(|kw| lower.contains(kw))
}

/// Detect Claude Code status from terminal title (Orca-compatible).
/// Returns None if the title does not match Claude Code conventions.
///
/// Claude Code title conventions:
/// - "✳ " prefix = idle (task description follows)
/// - ". " prefix = working
/// - "* " prefix = idle
/// - Braille spinner anywhere = working
/// - "claude" + action/permission/waiting = permission
/// - "claude" + ready/idle/done = idle
/// - "claude" + working/thinking/running = working
/// - Bare "claude" = idle (default when no status indicators)
pub fn detect_claude_status_from_title(title: &str) -> Option<ClaudeTransportStatus> {
    if title.is_empty() {
        return None;
    }

    // Claude Code uses ✳ prefix for idle - check before braille/agent-name
    // because the title text is the task description, not "Claude Code"
    if title.starts_with(CLAUDE_IDLE_PREFIX) || title == CLAUDE_IDLE_PREFIX.to_string() {
        return Some(ClaudeTransportStatus::Idle);
    }

    // Braille spinner characters indicate working state
    if contains_braille_spinner(title) {
        return Some(ClaudeTransportStatus::Working);
    }

    // Claude Code title prefixes: ". " = working, "* " = idle
    if title.starts_with(". ") {
        return Some(ClaudeTransportStatus::Working);
    }
    if title.starts_with("* ") {
        return Some(ClaudeTransportStatus::Idle);
    }

    // Check for agent names with status keywords
    if contains_agent_name(title) {
        if contains_any(title, &["action required", "permission", "waiting"]) {
            return Some(ClaudeTransportStatus::Permission);
        }
        if contains_any(title, &["ready", "idle", "done"]) {
            return Some(ClaudeTransportStatus::Idle);
        }
        if contains_any(title, &["working", "thinking", "running"]) {
            return Some(ClaudeTransportStatus::Working);
        }

        // Permission/action-required Claude titles can omit the usual prefixes
        // but start with "claude" keyword
        if title.to_lowercase().starts_with("claude") {
            return Some(ClaudeTransportStatus::Idle);
        }
    }

    None
}

/// Returns true when the terminal title matches Claude Code's conventions.
/// Used to scope session detection to Claude Code specifically.
pub fn is_claude_agent_title(title: &str) -> bool {
    if title.is_empty() {
        return false;
    }

    // Claude-specific prefixes must win over other agents
    if title.starts_with(CLAUDE_IDLE_PREFIX) || title == CLAUDE_IDLE_PREFIX.to_string() {
        return true;
    }
    if title.starts_with(". ") || title.starts_with("* ") {
        return true;
    }
    if contains_braille_spinner(title) {
        return true;
    }
    // Permission/action-required Claude titles can omit the usual prefixes
    if title.to_lowercase().starts_with("claude") {
        return true;
    }

    false
}

/// Strip working-status indicators from a title so that detection
/// will no longer return 'working'. Used to clear stale titles
/// when an agent exits without resetting its title.
#[cfg(test)]
pub fn clear_claude_working_indicators(title: &str) -> String {
    let mut cleaned = title.to_string();

    // Strip Braille spinner characters (U+2800–U+28FF)
    cleaned = cleaned
        .chars()
        .filter(|&c| !is_braille_spinner(c))
        .collect();

    // Strip Claude Code ". " working prefix
    if cleaned.starts_with(". ") {
        cleaned = cleaned[2..].to_string();
    }

    // Strip working keywords when agent name is present
    if contains_agent_name(&cleaned) {
        for keyword in ["working", "thinking", "running"] {
            cleaned = cleaned.replace(&keyword.to_string(), "");
            cleaned = cleaned.replace(&keyword.to_uppercase(), "");
        }
    }

    // Collapse whitespace after removals
    cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");

    if cleaned.is_empty() {
        title.to_string()
    } else {
        cleaned
    }
}

/// Extract complete lines from a buffer, returning (complete_lines, incomplete_tail).
/// A line is complete if it ends with newline, or if it contains a full bracket pattern
/// that could be a hook event.
#[cfg(test)]
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
            // No more newlines - split at the first complete official hook if present.
            let tail = &buffer[search_start..];
            if let Some(line_end) = complete_hook_pattern_end(tail) {
                let line_end = search_start + line_end;
                let line = &buffer[search_start..line_end];
                complete_lines.push(line);
                search_start = line_end;
                if search_start >= buffer.len() {
                    return (complete_lines, "");
                }
            } else {
                // Keep as incomplete tail
                return (complete_lines, tail);
            }
        }
    }
}

fn complete_hook_pattern_end(text: &str) -> Option<usize> {
    let text_lower = text.to_ascii_lowercase();
    let mut match_end = None;

    for pattern in ["[droid-hook:", "[factory-droid-hook:", "[opencode-hook:"] {
        if let Some(start) = text_lower.find(pattern) {
            let end = text_lower[start + pattern.len()..]
                .find(']')
                .map(|end| start + pattern.len() + end + 1);
            match (match_end, end) {
                (None, Some(end)) => match_end = Some((start, end)),
                (Some((best_start, _)), Some(end)) if start < best_start => {
                    match_end = Some((start, end))
                }
                _ => {}
            }
        }
    }

    match_end.map(|(_, end)| end)
}

fn title_status_for_config(
    config: &AiHookConfig,
    title: &str,
) -> Option<(AiCliStatus, AiHookEvent)> {
    if !config.working_title_pattern.is_empty() && title.contains(&config.working_title_pattern) {
        return Some((AiCliStatus::Running, AiHookEvent::Running));
    }

    if !config.idle_title_pattern.is_empty() && title.contains(&config.idle_title_pattern) {
        return Some((AiCliStatus::Attention, AiHookEvent::Attention));
    }

    None
}

fn parse_hook_event(text: &str, config: &AiHooksConfig) -> Option<(AiCliTool, String, bool)> {
    let clean = strip_ansi(text);
    let text_lower = clean.to_ascii_lowercase();

    let extract_name = |value: &str| -> String { normalize_hook_name(value) };

    for (prefix, is_notification, detected_tool) in [
        ("[droid-hook:event=", false, AiCliTool::FactoryDroid),
        ("[factory-droid-hook:event=", false, AiCliTool::FactoryDroid),
        ("[opencode-hook:event=", false, AiCliTool::OpenCode),
        ("[droid-hook:notification=", true, AiCliTool::FactoryDroid),
        (
            "[factory-droid-hook:notification=",
            true,
            AiCliTool::FactoryDroid,
        ),
        ("[opencode-hook:notification=", true, AiCliTool::OpenCode),
    ] {
        if let Some(pos) = text_lower.find(prefix) {
            let after = &clean[pos + prefix.len()..];
            let name: String = after
                .chars()
                .take_while(|&c| c.is_ascii_alphanumeric() || c == '_')
                .collect();
            if !name.is_empty() {
                // Verify the detected tool is enabled in config
                let config_for_tool = config.config_for(detected_tool);
                if config_for_tool.map_or(false, |c| c.enabled) {
                    return Some((detected_tool, extract_name(&name), is_notification));
                }
            }
        }
    }

    None
}

fn names_match(left: &str, right: &str) -> bool {
    normalize_hook_name(left) == normalize_hook_name(right)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiHookTransition {
    pub tool: AiCliTool,
    pub status: AiCliStatus,
    pub event: Option<AiHookEvent>,
    /// Original hook event name for semantic mapping (e.g., "session.idle", "permission.asked")
    pub event_name: Option<String>,
    pub text_offset: usize,
}

fn extract_complete_lines_with_end_offsets(buffer: &str) -> (Vec<(&str, usize)>, &str) {
    let mut complete_lines = Vec::new();
    let mut search_start = 0;

    loop {
        if let Some(newline_pos) = buffer[search_start..].find('\n') {
            let line_end = search_start + newline_pos;
            let line = &buffer[search_start..line_end];
            complete_lines.push((line, line_end));
            search_start = line_end + 1;
        } else {
            let tail = &buffer[search_start..];
            if let Some(line_end) = complete_hook_pattern_end(tail) {
                let line_end = search_start + line_end;
                let line = &buffer[search_start..line_end];
                complete_lines.push((line, line_end));
                search_start = line_end;
                if search_start >= buffer.len() {
                    return (complete_lines, "");
                }
            } else {
                return (complete_lines, tail);
            }
        }
    }
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

    fn lock_sessions(&self) -> Option<std::sync::MutexGuard<'_, BTreeMap<u64, AiCliSession>>> {
        self.sessions.lock().ok()
    }

    pub fn ai_activity_started(&self, terminal_id: u64) -> Option<(AiCliTool, AiCliStatus)> {
        let mut sessions = self.lock_sessions()?;
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
        let mut sessions = self.lock_sessions()?;
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
        let mut sessions = self.lock_sessions()?;
        let session = sessions.entry(terminal_id).or_default();

        if session.status == AiCliStatus::Attention {
            session.status = AiCliStatus::Inactive;
            session.last_event = Some(AiHookEvent::IdlePrompt);
            return Some((session.tool?, AiCliStatus::Inactive));
        }

        None
    }

    pub fn set_tool(&self, terminal_id: u64, tool: AiCliTool) {
        let Some(mut sessions) = self.lock_sessions() else {
            return;
        };
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
        self.update_with_text_offsets(terminal_id, text)
            .pop()
            .map(|transition| (transition.tool, transition.status, transition.event))
    }

    pub(crate) fn update_with_text_offsets(
        &self,
        terminal_id: u64,
        text: &str,
    ) -> Vec<AiHookTransition> {
        let Some(mut sessions) = self.lock_sessions() else {
            return Vec::new();
        };
        let session = sessions.entry(terminal_id).or_default();

        // Step 1: Detect tool if not already detected
        if session.tool.is_none() {
            if !session.detect_tool(text, &self.config) {
                return Vec::new();
            }
            // Tool detected - fall through to check for hook events
        }

        let Some(tool) = session.tool else {
            return Vec::new();
        };
        let Some(config) = self.config.config_for(tool) else {
            return Vec::new();
        };

        let prior_pending_len = session.pending_line.len();
        session.pending_line.push_str(text);
        let buffer = session.pending_line.clone();
        let (complete_lines, incomplete_tail) = extract_complete_lines_with_end_offsets(&buffer);
        let mut transitions = Vec::new();

        for (line, end_offset) in complete_lines {
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
                    transitions.push(AiHookTransition {
                        tool,
                        status: AiCliStatus::Running,
                        event: Some(AiHookEvent::Running),
                        event_name: Some(event_name.clone()),
                        text_offset: end_offset.saturating_sub(prior_pending_len).min(text.len()),
                    });
                    continue;
                }

                if is_attention_event {
                    session.status = AiCliStatus::Attention;
                    session.last_event = Some(AiHookEvent::Attention);
                    transitions.push(AiHookTransition {
                        tool,
                        status: AiCliStatus::Attention,
                        event: Some(AiHookEvent::Attention),
                        event_name: Some(event_name.clone()),
                        text_offset: end_offset.saturating_sub(prior_pending_len).min(text.len()),
                    });
                }
            }
        }

        session.pending_line = incomplete_tail.to_string();
        Self::trim_pending_line_to_limit(&mut session.pending_line);
        transitions
    }

    /// Update AI status based on terminal title changes.
    /// This is called when the terminal title changes and checks for AI-specific patterns.
    ///
    /// Claude Code detection: Uses Orca-compatible title-based detection when the title
    /// matches Claude conventions (✳ prefix, braille spinner, ./* prefixes, or "claude" keyword).
    /// This allows Claude to be detected even when the session was previously attributed
    /// to another tool (e.g., if the user switched from Codex to Claude in the same terminal).
    pub fn update_from_title(
        &self,
        terminal_id: u64,
        title: &str,
    ) -> Option<(AiCliTool, AiCliStatus, Option<AiHookEvent>)> {
        let mut sessions = self.lock_sessions()?;
        let session = sessions.entry(terminal_id).or_default();

        // Check for Claude Code title patterns first (Orca-compatible)
        // Claude titles take precedence and can override other tools
        if is_claude_agent_title(title) {
            if let Some(claude_status) = detect_claude_status_from_title(title) {
                let (status, event) = match claude_status {
                    ClaudeTransportStatus::Working => (AiCliStatus::Running, AiHookEvent::Running),
                    ClaudeTransportStatus::Idle => (AiCliStatus::Attention, AiHookEvent::Attention),
                    ClaudeTransportStatus::Permission => {
                        (AiCliStatus::Attention, AiHookEvent::Attention)
                    }
                };

                // Allow tool override: if title is clearly Claude, switch to Claude
                // even if session was previously Codex/OpenCode/FactoryDroid
                let tool_changed = session.tool != Some(AiCliTool::Claude);
                session.tool = Some(AiCliTool::Claude);

                if session.status != status || session.last_event != Some(event) || tool_changed {
                    session.status = status;
                    session.last_event = Some(event);
                    return Some((AiCliTool::Claude, status, Some(event)));
                }
                return None;
            }
        }

        // Fall back to config-based title patterns for other tools
        let matched = if let Some(tool) = session.tool {
            // Don't use config patterns for Claude - we already checked Claude patterns above
            if tool == AiCliTool::Claude {
                return None;
            }
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
        self.lock_sessions()?.get(&terminal_id).cloned()
    }

    pub fn reset_session(&self, terminal_id: u64) {
        if let Some(mut sessions) = self.lock_sessions() {
            sessions.remove(&terminal_id);
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.global_enabled
    }

    fn trim_pending_line_to_limit(pending_line: &mut String) {
        if pending_line.len() <= MAX_PENDING_HOOK_LINE_BYTES {
            return;
        }

        let trim_target = pending_line.len() - MAX_PENDING_HOOK_LINE_BYTES;
        let cut = if pending_line.is_char_boundary(trim_target) {
            trim_target
        } else {
            pending_line
                .char_indices()
                .find(|(index, _)| *index > trim_target)
                .map(|(index, _)| index)
                .unwrap_or(pending_line.len())
        };

        if cut > 0 {
            pending_line.drain(..cut);
        }
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
    fn poisoned_session_lock_returns_graceful_degradation() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        manager.set_tool(1, AiCliTool::FactoryDroid);

        let poison_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = manager.sessions.lock().unwrap();
            panic!("poison test");
        }));
        assert!(poison_result.is_err());

        assert!(manager.session(1).is_none());
        assert_eq!(manager.ai_activity_started(1), None);
        assert_eq!(manager.ai_waiting_for_user(1), None);
        assert_eq!(manager.user_interacted(1), None);
        assert_eq!(
            manager.update(1, "[droid-hook:event=UserPromptSubmit]"),
            None
        );
        manager.reset_session(1);
    }

    #[test]
    fn pending_line_is_trimmed_to_tail_limit_after_long_unmatched_input() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let long_prefix = "x".repeat(MAX_PENDING_HOOK_LINE_BYTES + 4096);

        assert_eq!(manager.update(1, &long_prefix), None);

        let session = manager.session(1).expect("session should exist");
        assert!(session.pending_line.len() <= MAX_PENDING_HOOK_LINE_BYTES);
        assert!(session.pending_line.chars().all(|ch| ch == 'x'));
    }

    #[test]
    fn pending_line_trimming_preserves_recent_hook_detection() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let prefix = "x".repeat(MAX_PENDING_HOOK_LINE_BYTES + 128);

        assert_eq!(manager.update(1, &prefix), None);
        assert_eq!(
            manager.update(1, "[droid-hook:event=UserPromptSubmit]"),
            Some((
                AiCliTool::FactoryDroid,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
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
    fn codex_default_config_does_not_detect_generic_codex_text() {
        let config = AiHooksConfig::with_factory_droid_defaults();
        let mut session = AiCliSession::default();

        assert!(!session.detect_tool("codex", &config));
        assert!(!session.detect_tool("launching codex session", &config));
        assert_eq!(session.tool, None);
    }

    #[test]
    fn codex_default_config_has_no_broad_detection_commands() {
        let config = AiHooksConfig::with_factory_droid_defaults();
        let codex = config
            .config_for(AiCliTool::CodexCli)
            .expect("codex config should exist");

        assert!(codex.detection_commands.is_empty());
    }

    #[test]
    fn factory_droid_detection_still_works_with_codex_hardened() {
        let config = AiHooksConfig::with_factory_droid_defaults();
        let mut session = AiCliSession::default();

        assert!(session.detect_tool("[factory-droid-hook:event=UserPromptSubmit]", &config));
        assert_eq!(session.tool, Some(AiCliTool::FactoryDroid));
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
    fn update_uses_last_actionable_event_in_chunk() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]\n"),
            None
        );

        assert_eq!(
            manager.update(
                terminal_id,
                "[droid-hook:event=UserPromptSubmit]\n[droid-hook:event=Stop]\n"
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
    fn update_uses_last_actionable_notification_in_chunk() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update(terminal_id, "[droid-hook:event=SessionStart]\n"),
            None
        );

        assert_eq!(
            manager.update(
                terminal_id,
                "[droid-hook:event=UserPromptSubmit]\n[droid-hook:notification=permission_prompt]\n"
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
    fn extract_complete_lines_stops_at_hook_closing_bracket() {
        let text = "some text [droid-hook:event=Stop] trailing";
        let (complete, tail) = extract_complete_lines(text);

        assert_eq!(complete, vec!["some text [droid-hook:event=Stop]"]);
        assert_eq!(tail, " trailing");
    }

    #[test]
    fn extract_complete_lines_with_end_offsets_keeps_trailing_bytes_available() {
        let line = "noise [droid-hook:event=Stop]";
        let text = format!("{line} trailing");
        let (complete, tail) = extract_complete_lines_with_end_offsets(text.as_str());

        assert_eq!(complete, vec![(line, line.len())]);
        assert_eq!(tail, " trailing");
    }

    #[test]
    fn extract_complete_lines_keeps_partial_hook_pattern_buffered() {
        let (complete, tail) = extract_complete_lines("some text [droid-hook:event=User");
        assert!(complete.is_empty());
        assert_eq!(tail, "some text [droid-hook:event=User");
    }

    #[test]
    fn update_with_text_offsets_processes_back_to_back_hook_markers_without_newlines() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let running_prefix = "[droid-hook:event=SessionStart][droid-hook:event=UserPromptSubmit]";
        let full_text = format!("{running_prefix}[droid-hook:event=Stop]");
        let transitions = manager.update_with_text_offsets(1, full_text.as_str());

        assert_eq!(
            transitions,
            vec![
                AiHookTransition {
                    tool: AiCliTool::FactoryDroid,
                    status: AiCliStatus::Running,
                    event: Some(AiHookEvent::Running),
                    event_name: Some("userpromptsubmit".to_string()),
                    text_offset: running_prefix.len(),
                },
                AiHookTransition {
                    tool: AiCliTool::FactoryDroid,
                    status: AiCliStatus::Attention,
                    event: Some(AiHookEvent::Attention),
                    event_name: Some("stop".to_string()),
                    text_offset: full_text.len(),
                },
            ]
        );
        assert_eq!(
            manager.session(1).expect("session").status,
            AiCliStatus::Attention
        );
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

    #[test]
    fn update_from_title_opencode_detects_running_pattern() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "OpenCode - Working on changes"),
            Some((
                AiCliTool::OpenCode,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::OpenCode));
        assert_eq!(session.status, AiCliStatus::Running);
    }

    #[test]
    fn update_from_title_opencode_detects_idle_pattern() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "OpenCode - Idle; waiting for input"),
            Some((
                AiCliTool::OpenCode,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::OpenCode));
        assert_eq!(session.status, AiCliStatus::Attention);
    }

    #[test]
    fn update_from_title_opencode_transitions_running_to_idle() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        let _ = manager.update_from_title(terminal_id, "OpenCode - Working...");
        let session = manager.session(terminal_id).expect("session after running");
        assert_eq!(session.status, AiCliStatus::Running);

        assert_eq!(
            manager.update_from_title(terminal_id, "OpenCode - Idle"),
            Some((
                AiCliTool::OpenCode,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("session after idle");
        assert_eq!(session.status, AiCliStatus::Attention);
    }

    // ─── Claude Code Title Detection Tests ───────────────────────────────────
    // Orca-compatible detection via terminal title patterns

    #[test]
    fn detect_claude_status_from_title_returns_none_for_empty_string() {
        assert_eq!(detect_claude_status_from_title(""), None);
    }

    #[test]
    fn detect_claude_status_from_title_returns_none_for_non_agent_titles() {
        assert_eq!(detect_claude_status_from_title("bash"), None);
        assert_eq!(detect_claude_status_from_title("vim myfile.ts"), None);
        assert_eq!(detect_claude_status_from_title("cargo build"), None);
    }

    #[test]
    fn detect_claude_status_idle_prefix_detects_idle() {
        // ✳ (eight-spoked asterisk) is Claude idle prefix
        assert_eq!(
            detect_claude_status_from_title("✳ User acknowledgment and confirmation"),
            Some(ClaudeTransportStatus::Idle)
        );
        assert_eq!(
            detect_claude_status_from_title("✳ Claude Code"),
            Some(ClaudeTransportStatus::Idle)
        );
        assert_eq!(
            detect_claude_status_from_title("✳"),
            Some(ClaudeTransportStatus::Idle)
        );
    }

    #[test]
    fn detect_claude_status_braille_spinner_detects_working() {
        // Braille patterns (U+2800-U+28FF) indicate working state
        assert_eq!(
            detect_claude_status_from_title("⠋ Fixing the bug"),
            Some(ClaudeTransportStatus::Working)
        );
        assert_eq!(
            detect_claude_status_from_title("⠂ Claude Code"),
            Some(ClaudeTransportStatus::Working)
        );
        assert_eq!(
            detect_claude_status_from_title("⠐ User acknowledgment and confirmation"),
            Some(ClaudeTransportStatus::Working)
        );
    }

    #[test]
    fn detect_claude_status_dot_prefix_detects_working() {
        // ". " prefix = working (Claude Code convention)
        assert_eq!(
            detect_claude_status_from_title(". claude"),
            Some(ClaudeTransportStatus::Working)
        );
        assert_eq!(
            detect_claude_status_from_title(". Implementing feature"),
            Some(ClaudeTransportStatus::Working)
        );
    }

    #[test]
    fn detect_claude_status_asterisk_prefix_detects_idle() {
        // "* " prefix = idle (Claude Code convention)
        assert_eq!(
            detect_claude_status_from_title("* claude"),
            Some(ClaudeTransportStatus::Idle)
        );
        assert_eq!(
            detect_claude_status_from_title("* Waiting for input"),
            Some(ClaudeTransportStatus::Idle)
        );
    }

    #[test]
    fn detect_claude_status_permission_keywords_detect_permission() {
        // Permission keywords with agent name
        assert_eq!(
            detect_claude_status_from_title("Claude Code - action required"),
            Some(ClaudeTransportStatus::Permission)
        );
        assert_eq!(
            detect_claude_status_from_title("claude - permission needed"),
            Some(ClaudeTransportStatus::Permission)
        );
        assert_eq!(
            detect_claude_status_from_title("claude waiting for input"),
            Some(ClaudeTransportStatus::Permission)
        );
    }

    #[test]
    fn detect_claude_status_idle_keywords_detect_idle() {
        // Idle keywords with agent name
        assert_eq!(
            detect_claude_status_from_title("claude ready"),
            Some(ClaudeTransportStatus::Idle)
        );
        assert_eq!(
            detect_claude_status_from_title("claude idle"),
            Some(ClaudeTransportStatus::Idle)
        );
        assert_eq!(
            detect_claude_status_from_title("claude done"),
            Some(ClaudeTransportStatus::Idle)
        );
    }

    #[test]
    fn detect_claude_status_working_keywords_detect_working() {
        // Working keywords with agent name
        assert_eq!(
            detect_claude_status_from_title("claude working on task"),
            Some(ClaudeTransportStatus::Working)
        );
        assert_eq!(
            detect_claude_status_from_title("claude thinking"),
            Some(ClaudeTransportStatus::Working)
        );
        assert_eq!(
            detect_claude_status_from_title("claude running tests"),
            Some(ClaudeTransportStatus::Working)
        );
    }

    #[test]
    fn detect_claude_status_bare_agent_name_defaults_to_idle() {
        // Bare agent name without status indicators = idle
        assert_eq!(
            detect_claude_status_from_title("claude"),
            Some(ClaudeTransportStatus::Idle)
        );
        assert_eq!(
            detect_claude_status_from_title("CLAUDE"),
            Some(ClaudeTransportStatus::Idle)
        );
    }

    #[test]
    fn is_claude_agent_title_detects_claude_patterns() {
        assert!(is_claude_agent_title("✳ User acknowledgment"));
        assert!(is_claude_agent_title("⠂ Claude Code"));
        assert!(is_claude_agent_title(". claude"));
        assert!(is_claude_agent_title("* claude"));
        assert!(is_claude_agent_title("claude ready"));
        assert!(is_claude_agent_title("Claude Code - action required"));
    }

    #[test]
    fn is_claude_agent_title_rejects_non_claude() {
        assert!(!is_claude_agent_title("bash"));
        assert!(!is_claude_agent_title("vim file.ts"));
        assert!(!is_claude_agent_title("codex")); // Not Claude specifically
        assert!(!is_claude_agent_title("OpenCode working"));
    }

    #[test]
    fn update_from_title_detects_claude_idle() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "✳ User acknowledgment and confirmation"),
            Some((
                AiCliTool::Claude,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::Claude));
        assert_eq!(session.status, AiCliStatus::Attention);
    }

    #[test]
    fn update_from_title_detects_claude_working() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "⠂ User acknowledgment and confirmation"),
            Some((
                AiCliTool::Claude,
                AiCliStatus::Running,
                Some(AiHookEvent::Running)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::Claude));
        assert_eq!(session.status, AiCliStatus::Running);
    }

    #[test]
    fn update_from_title_detects_claude_permission() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        assert_eq!(
            manager.update_from_title(terminal_id, "Claude Code - action required"),
            Some((
                AiCliTool::Claude,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::Claude));
        assert_eq!(session.status, AiCliStatus::Attention);
    }

    #[test]
    fn update_from_title_claude_overrides_previous_tool() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        // First detect as OpenCode
        manager.update_with_text_offsets(terminal_id, "[opencode-hook:event=tool.execute.before]");
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::OpenCode));

        // Then switch to Claude via title
        assert_eq!(
            manager.update_from_title(terminal_id, "✳ Switching to Claude"),
            Some((
                AiCliTool::Claude,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.tool, Some(AiCliTool::Claude));
    }

    #[test]
    fn clear_claude_working_indicators_strips_braille_and_prefixes() {
        let cleared = clear_claude_working_indicators("⠂ Claude Code");
        assert!(!cleared.contains('⠂'));
        assert!(!contains_braille_spinner(&cleared));

        let cleared = clear_claude_working_indicators(". claude");
        assert!(!cleared.starts_with(". "));

        let cleared = clear_claude_working_indicators("⠋ Working on feature");
        assert!(!cleared.contains("working"));
    }

    #[test]
    fn update_from_title_claude_working_to_idle_transition() {
        let manager = AiHookManager::new(AiHooksConfig::with_factory_droid_defaults());
        let terminal_id = 1;

        // Start working
        manager.update_from_title(terminal_id, "⠂ Implementing feature");
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.status, AiCliStatus::Running);
        assert_eq!(session.tool, Some(AiCliTool::Claude));

        // Transition to idle
        assert_eq!(
            manager.update_from_title(terminal_id, "✳ Implementing feature"),
            Some((
                AiCliTool::Claude,
                AiCliStatus::Attention,
                Some(AiHookEvent::Attention)
            ))
        );
        let session = manager.session(terminal_id).expect("session");
        assert_eq!(session.status, AiCliStatus::Attention);
        assert_eq!(session.tool, Some(AiCliTool::Claude));
    }
}
