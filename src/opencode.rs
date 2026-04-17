use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MERGEN_TERMINAL_ID_ENV_VAR: &str = "MERGEN_TERMINAL_ID";
pub const MERGEN_AI_INBOX_DIR_ENV_VAR: &str = "MERGEN_AI_INBOX_DIR";
pub const MERGEN_AI_TOOL_HINT_ENV_VAR: &str = "MERGEN_AI_TOOL_HINT";
pub const MERGEN_AI_TOOL_HINT_OPENCODE: &str = "opencode";
pub const MERGEN_ADE_OPENCODE_INBOX_TOKEN_ENV_VAR: &str = "MERGEN_ADE_OPENCODE_INBOX_TOKEN";

pub const OPENCODE_SESSION_IDLE_EVENT: &str = "session.idle";
pub const OPENCODE_SESSION_ERROR_EVENT: &str = "session.error";
pub const OPENCODE_PERMISSION_ASKED_EVENT: &str = "permission.asked";
pub const OPENCODE_TOOL_EXECUTE_BEFORE_EVENT: &str = "tool.execute.before";
pub const OPENCODE_TOOL_EXECUTE_AFTER_EVENT: &str = "tool.execute.after";

// Legacy/internal names for backward compatibility
pub const OPENCODE_TURN_COMPLETE_EVENT: &str = "turn-complete";
pub const OPENCODE_QUESTION_PROMPT_EVENT: &str = "question-prompt";
pub const OPENCODE_APPROVAL_PROMPT_EVENT: &str = "approval-prompt";

/// Normalized Codex CLI status values (Orca-compatible)
/// These are the canonical states for Codex CLI semantic status tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexTransportStatus {
    Working,
    Idle,
    Permission,
}

impl CodexTransportStatus {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Permission => "permission",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "working" => Some(Self::Working),
            "idle" => Some(Self::Idle),
            "permission" => Some(Self::Permission),
            _ => None,
        }
    }

    /// Map transport status to the generic status string for legacy consumers
    #[allow(dead_code)]
    pub fn to_generic_status(&self) -> String {
        match self {
            Self::Working => "running".to_owned(),
            Self::Idle | Self::Permission => "attention".to_owned(),
        }
    }
}

/// Normalized OpenCode status values (Orca-compatible)
/// These are the canonical states transported from OpenCode plugin/notify mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenCodeTransportStatus {
    Working,
    Idle,
    Permission,
}

impl OpenCodeTransportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Permission => "permission",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "working" => Some(Self::Working),
            "idle" => Some(Self::Idle),
            "permission" => Some(Self::Permission),
            _ => None,
        }
    }

    /// Map transport status to the generic status string for legacy consumers
    pub fn to_generic_status(&self) -> String {
        match self {
            Self::Working => "running".to_owned(),
            Self::Idle | Self::Permission => "attention".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenCodeNotifyInboxEvent {
    pub terminal_id: String,
    pub tool: String,
    /// Generic status for legacy compatibility ("running" | "attention")
    pub status: String,
    #[serde(default)]
    pub inbox_token: Option<String>,
    #[serde(default)]
    pub event_kind: Option<String>,
    /// Normalized OpenCode status (working | idle | permission) - Orca-compatible
    #[serde(default)]
    pub opencode_status: Option<OpenCodeTransportStatus>,
    pub raw_json: String,
    pub timestamp_utc: String,
}

pub fn opencode_notify_inbox_path_for_dir(
    dir: &Path,
    terminal_id: u64,
    inbox_token: &str,
) -> PathBuf {
    dir.join(format!("opencode-{terminal_id}-{inbox_token}.jsonl"))
}

pub fn opencode_env_pairs(
    terminal_id: u64,
    inbox_dir: &Path,
    inbox_token: &str,
) -> [(String, OsString); 4] {
    [
        (
            MERGEN_TERMINAL_ID_ENV_VAR.to_owned(),
            OsString::from(terminal_id.to_string()),
        ),
        (
            MERGEN_AI_INBOX_DIR_ENV_VAR.to_owned(),
            inbox_dir.as_os_str().to_owned(),
        ),
        (
            MERGEN_AI_TOOL_HINT_ENV_VAR.to_owned(),
            OsString::from(MERGEN_AI_TOOL_HINT_OPENCODE),
        ),
        (
            MERGEN_ADE_OPENCODE_INBOX_TOKEN_ENV_VAR.to_owned(),
            OsString::from(inbox_token),
        ),
    ]
}

pub fn write_opencode_notify_event(
    payload: &str,
    terminal_id: &str,
    inbox_dir: &Path,
    inbox_token: &str,
    tool_hint: Option<&str>,
) -> io::Result<()> {
    // A terminal can legitimately host both Codex and OpenCode over its lifetime.
    // Treat the shared tool hint env var as advisory only so one tool's setup does
    // not break the event path.
    let _ = tool_hint;

    // Extract event kind and map to normalized status
    let event_kind = extract_event_kind(payload);
    let opencode_status = event_kind.as_ref().and_then(|kind| {
        match kind.as_str() {
            // Working signals
            k if k == OPENCODE_TOOL_EXECUTE_BEFORE_EVENT => Some(OpenCodeTransportStatus::Working),
            // Permission signals
            k if k == OPENCODE_PERMISSION_ASKED_EVENT
                || k == "permission_asked"
                || k == "permission-asked"
                || k == OPENCODE_APPROVAL_PROMPT_EVENT
                || k == "approval_prompt"
                || k == "approval-prompt" =>
            {
                Some(OpenCodeTransportStatus::Permission)
            }
            // Question signals also map to permission (user interaction needed)
            k if k == "question.asked"
                || k == "question_asked"
                || k == "question-asked"
                || k == OPENCODE_QUESTION_PROMPT_EVENT
                || k == "question_prompt"
                || k == "question-prompt" =>
            {
                Some(OpenCodeTransportStatus::Permission)
            }
            // Plan mode signals also map to permission (user approval needed)
            k if k == "plan_mode_prompt" || k == "plan-mode-prompt" || k == "plan_mode" => {
                Some(OpenCodeTransportStatus::Permission)
            }
            // Idle/completion signals
            k if k == OPENCODE_SESSION_IDLE_EVENT
                || k == "session_idle"
                || k == "session-idle"
                || k == OPENCODE_TURN_COMPLETE_EVENT
                || k == "turn_complete"
                || k == "turn-complete" =>
            {
                Some(OpenCodeTransportStatus::Idle)
            }
            // Error signals - still idle but with error context
            k if k == OPENCODE_SESSION_ERROR_EVENT
                || k == "session_error"
                || k == "session-error"
                || k == "error" =>
            {
                Some(OpenCodeTransportStatus::Idle)
            }
            _ => None,
        }
    });

    // Legacy status for backward compatibility
    let legacy_status = opencode_status
        .as_ref()
        .map(|s| s.to_generic_status())
        .unwrap_or_else(|| "attention".to_owned());

    let event = OpenCodeNotifyInboxEvent {
        terminal_id: terminal_id.to_string(),
        tool: MERGEN_AI_TOOL_HINT_OPENCODE.to_owned(),
        status: legacy_status,
        inbox_token: Some(inbox_token.to_owned()),
        event_kind,
        opencode_status,
        raw_json: payload.to_owned(),
        timestamp_utc: format_iso_timestamp(),
    };

    let json = serde_json::to_string(&event)?;
    let path = opencode_notify_inbox_path_for_dir(
        inbox_dir,
        terminal_id.parse().unwrap_or(0),
        inbox_token,
    );

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{json}")?;
    file.flush()
}

fn format_iso_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    // Simple ISO-like format: seconds since epoch (not full RFC 3339, but sufficient for logging)
    format!("{}.{:09}Z", secs, duration.subsec_nanos())
}

fn extract_event_kind(payload: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(payload).ok()?;

    // Try various common locations for event type/name
    // Priority: nested event.type > top-level type > top-level event > top-level kind > nested event.name
    let kind = parsed
        .get("event")
        .and_then(|e| e.get("type"))
        .or_else(|| parsed.get("type"))
        .or_else(|| parsed.get("event"))
        .or_else(|| parsed.get("kind"))
        .or_else(|| parsed.get("event").and_then(|e| e.get("name")))
        .or_else(|| parsed.get("name"))
        .or_else(|| parsed.get("event_type"));

    kind.and_then(|v| v.as_str()).map(|s| s.to_lowercase())
}

pub fn maybe_handle_opencode_notify_mode() -> io::Result<Option<OpenCodeNotifyInboxEvent>> {
    let mut args = std::env::args_os().peekable();

    while let Some(arg) = args.next() {
        if arg == "--opencode-notify" {
            let payload = args
                .next()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Missing OpenCode notify payload argument.",
                    )
                })?
                .to_string_lossy()
                .to_string();

            let terminal_id = std::env::var(MERGEN_TERMINAL_ID_ENV_VAR).ok();
            let inbox_dir = std::env::var(MERGEN_AI_INBOX_DIR_ENV_VAR).ok();
            let inbox_token = std::env::var(MERGEN_ADE_OPENCODE_INBOX_TOKEN_ENV_VAR).ok();
            let tool_hint = std::env::var(MERGEN_AI_TOOL_HINT_ENV_VAR).ok();

            if let (Some(tid), Some(dir), Some(token)) = (terminal_id, inbox_dir, inbox_token) {
                let path = PathBuf::from(dir);
                write_opencode_notify_event(&payload, &tid, &path, &token, tool_hint.as_deref())?;

                // Re-parse to get the normalized status for the returned event
                let event_kind = extract_event_kind(&payload);
                let opencode_status = event_kind.as_ref().and_then(|kind| match kind.as_str() {
                    k if k == OPENCODE_TOOL_EXECUTE_BEFORE_EVENT => {
                        Some(OpenCodeTransportStatus::Working)
                    }
                    k if k == OPENCODE_PERMISSION_ASKED_EVENT
                        || k == "permission_asked"
                        || k == "permission-asked"
                        || k == OPENCODE_APPROVAL_PROMPT_EVENT =>
                    {
                        Some(OpenCodeTransportStatus::Permission)
                    }
                    k if k == "question.asked"
                        || k == "question_asked"
                        || k == "question-asked"
                        || k == OPENCODE_QUESTION_PROMPT_EVENT =>
                    {
                        Some(OpenCodeTransportStatus::Permission)
                    }
                    k if k == "plan_mode_prompt" || k == "plan-mode-prompt" || k == "plan_mode" => {
                        Some(OpenCodeTransportStatus::Permission)
                    }
                    k if k == OPENCODE_SESSION_IDLE_EVENT
                        || k == "session_idle"
                        || k == "session-idle"
                        || k == OPENCODE_TURN_COMPLETE_EVENT =>
                    {
                        Some(OpenCodeTransportStatus::Idle)
                    }
                    k if k == OPENCODE_SESSION_ERROR_EVENT
                        || k == "session_error"
                        || k == "session-error"
                        || k == "error" =>
                    {
                        Some(OpenCodeTransportStatus::Idle)
                    }
                    _ => None,
                });

                let legacy_status = opencode_status
                    .as_ref()
                    .map(|s| s.to_generic_status())
                    .unwrap_or_else(|| "attention".to_owned());

                let event = OpenCodeNotifyInboxEvent {
                    terminal_id: tid,
                    tool: MERGEN_AI_TOOL_HINT_OPENCODE.to_owned(),
                    status: legacy_status,
                    inbox_token: Some(token),
                    event_kind,
                    opencode_status,
                    raw_json: payload,
                    timestamp_utc: format_iso_timestamp(),
                };
                return Ok(Some(event));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
pub fn read_opencode_notify_inbox(
    dir: &Path,
    terminal_id: u64,
    inbox_token: &str,
    already_processed: &mut std::collections::HashSet<u64>,
) -> io::Result<Vec<OpenCodeNotifyInboxEvent>> {
    let path = opencode_notify_inbox_path_for_dir(dir, terminal_id, inbox_token);

    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&path)?;
    let mut events = Vec::new();

    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let event: OpenCodeNotifyInboxEvent = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Simple deduplication based on hash of raw_json
        let hash = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            event.raw_json.hash(&mut hasher);
            hasher.finish()
        };

        if already_processed.insert(hash) {
            events.push(event);
        }
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(label: &str) -> Self {
            let unique_suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mergen-ade-{label}-{}-{unique_suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn opencode_env_pairs_include_terminal_id_inbox_dir_tool_hint_and_token() {
        let inbox_dir = PathBuf::from("/tmp/opencode-inbox");
        let pairs = opencode_env_pairs(42, &inbox_dir, "opencode-token-42");

        assert_eq!(pairs[0].0, MERGEN_TERMINAL_ID_ENV_VAR);
        assert_eq!(pairs[0].1, OsString::from("42"));
        assert_eq!(pairs[1].0, MERGEN_AI_INBOX_DIR_ENV_VAR);
        assert_eq!(pairs[1].1, OsString::from("/tmp/opencode-inbox"));
        assert_eq!(pairs[2].0, MERGEN_AI_TOOL_HINT_ENV_VAR);
        assert_eq!(pairs[2].1, OsString::from(MERGEN_AI_TOOL_HINT_OPENCODE));
        assert_eq!(pairs[3].0, MERGEN_ADE_OPENCODE_INBOX_TOKEN_ENV_VAR);
        assert_eq!(pairs[3].1, OsString::from("opencode-token-42"));
    }

    #[test]
    fn extract_event_kind_finds_type_field() {
        assert_eq!(
            extract_event_kind(r#"{"type":"turn-complete"}"#),
            Some("turn-complete".to_string())
        );
    }

    #[test]
    fn extract_event_kind_finds_event_field() {
        assert_eq!(
            extract_event_kind(r#"{"event":"question-prompt"}"#),
            Some("question-prompt".to_string())
        );
    }

    #[test]
    fn extract_event_kind_returns_none_for_invalid_json() {
        assert_eq!(extract_event_kind("not valid json"), None);
    }

    #[test]
    fn write_and_read_notify_inbox_roundtrips() {
        let temp = TestTempDir::new("opencode-notify");
        let payload = r#"{"type":"turn-complete"}"#;

        write_opencode_notify_event(
            payload,
            "17",
            &temp.path,
            "test-token-17",
            Some(MERGEN_AI_TOOL_HINT_OPENCODE),
        )
        .expect("write should succeed");

        let mut processed = std::collections::HashSet::new();
        let events = read_opencode_notify_inbox(&temp.path, 17, "test-token-17", &mut processed)
            .expect("read should succeed");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].terminal_id, "17");
        assert_eq!(events[0].tool, "opencode");
        assert_eq!(events[0].status, "attention");
        assert_eq!(events[0].event_kind.as_deref(), Some("turn-complete"));
        assert_eq!(events[0].raw_json, payload);
    }

    #[test]
    fn write_accepts_mismatched_tool_hint() {
        let temp = TestTempDir::new("opencode-notify-mismatched-hint");
        write_opencode_notify_event(
            r#"{"type":"turn-complete"}"#,
            "1",
            &temp.path,
            "token",
            Some("codex"),
        )
        .expect("write should ignore mismatched tool hint");

        let mut processed = std::collections::HashSet::new();
        let events = read_opencode_notify_inbox(&temp.path, 1, "token", &mut processed)
            .expect("read should succeed");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].terminal_id, "1");
        assert_eq!(events[0].tool, "opencode");
        assert_eq!(events[0].status, "attention");
        assert_eq!(events[0].event_kind.as_deref(), Some("turn-complete"));
    }

    #[test]
    fn read_opencode_notify_inbox_deduplicates_events() {
        let temp = TestTempDir::new("opencode-notify-dedup");
        let payload = r#"{"type":"turn-complete"}"#;

        // Write same payload twice
        write_opencode_notify_event(
            payload,
            "5",
            &temp.path,
            "token-5",
            Some(MERGEN_AI_TOOL_HINT_OPENCODE),
        )
        .expect("first write");
        write_opencode_notify_event(
            payload,
            "5",
            &temp.path,
            "token-5",
            Some(MERGEN_AI_TOOL_HINT_OPENCODE),
        )
        .expect("second write");

        let mut processed = std::collections::HashSet::new();
        let events = read_opencode_notify_inbox(&temp.path, 5, "token-5", &mut processed)
            .expect("read should succeed");

        // Should deduplicate identical events
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn read_opencode_notify_inbox_returns_empty_for_missing_file() {
        let temp = TestTempDir::new("opencode-notify-missing");
        let mut processed = std::collections::HashSet::new();
        let events =
            read_opencode_notify_inbox(&temp.path, 99, "nonexistent-token", &mut processed)
                .expect("read should not error");
        assert!(events.is_empty());
    }
}
