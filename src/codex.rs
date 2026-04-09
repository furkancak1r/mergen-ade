use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use toml::Value as TomlValue;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub const CODEX_SETUP_URL: &str = "https://developers.openai.com/codex/cli/#cli-setup";
pub const MERGEN_TERMINAL_ID_ENV_VAR: &str = "MERGEN_TERMINAL_ID";
pub const MERGEN_AI_INBOX_DIR_ENV_VAR: &str = "MERGEN_AI_INBOX_DIR";
pub const MERGEN_AI_TOOL_HINT_ENV_VAR: &str = "MERGEN_AI_TOOL_HINT";
pub const MERGEN_AI_TOOL_HINT_CODEX: &str = "codex";
pub const MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR: &str = "MERGEN_ADE_CODEX_INBOX_TOKEN";
pub const CODEX_NOTIFICATION_METHOD: &str = "bel";
pub const CODEX_TURN_COMPLETE_EVENT: &str = "agent-turn-complete";
pub const CODEX_APPROVAL_REQUESTED_EVENT: &str = "approval-requested";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexEnableOutcome {
    MissingInstall,
    NeedsLogin,
    CustomNotifyHookPreserved { path: PathBuf },
    ConfigUpdated { path: PathBuf, updated: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexConfigPatchOutcome {
    Updated,
    Unchanged,
    CustomNotifyHookPreserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexIntegrationStatus {
    EnabledHealthy {
        path: PathBuf,
    },
    NeedsSetup {
        path: PathBuf,
    },
    CustomNotifyHook {
        path: PathBuf,
    },
    ConfigReadError {
        path: Option<PathBuf>,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexNotifyInboxEvent {
    pub terminal_id: String,
    pub tool: String,
    pub status: String,
    #[serde(default)]
    pub inbox_token: Option<String>,
    #[serde(default)]
    pub event_kind: Option<String>,
    pub raw_json: String,
    pub timestamp_utc: String,
}

pub fn codex_setup_url() -> &'static str {
    CODEX_SETUP_URL
}

pub fn codex_notify_inbox_path_for_dir(dir: &Path, terminal_id: u64, inbox_token: &str) -> PathBuf {
    dir.join(format!("codex-{terminal_id}-{inbox_token}.jsonl"))
}

pub fn codex_env_pairs(
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
            OsString::from(MERGEN_AI_TOOL_HINT_CODEX),
        ),
        (
            MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR.to_owned(),
            OsString::from(inbox_token),
        ),
    ]
}

pub fn user_codex_config_path() -> io::Result<PathBuf> {
    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User home directory is unavailable",
        )
    })?;
    Ok(base_dirs.home_dir().join(".codex").join("config.toml"))
}

pub fn enable_codex_cli_integration(executable_path: &Path) -> io::Result<CodexEnableOutcome> {
    let install_check = run_codex_command(&["--version"]);
    let Ok(version_output) = install_check else {
        return Ok(CodexEnableOutcome::MissingInstall);
    };
    if !version_output.status.success() {
        return Ok(CodexEnableOutcome::MissingInstall);
    }

    let login_output = run_codex_command(&["login", "status"])?;
    if !login_output.status.success() {
        return Ok(CodexEnableOutcome::NeedsLogin);
    }

    let path = user_codex_config_path()?;
    match patch_codex_config_file(&path, executable_path)? {
        CodexConfigPatchOutcome::Updated => Ok(CodexEnableOutcome::ConfigUpdated {
            path,
            updated: true,
        }),
        CodexConfigPatchOutcome::Unchanged => Ok(CodexEnableOutcome::ConfigUpdated {
            path,
            updated: false,
        }),
        CodexConfigPatchOutcome::CustomNotifyHookPreserved => {
            Ok(CodexEnableOutcome::CustomNotifyHookPreserved { path })
        }
    }
}

pub fn inspect_codex_cli_integration(executable_path: &Path) -> CodexIntegrationStatus {
    match user_codex_config_path() {
        Ok(path) => inspect_codex_cli_integration_at_path(path, executable_path),
        Err(err) => CodexIntegrationStatus::ConfigReadError {
            path: None,
            error: err.to_string(),
        },
    }
}

pub fn patch_codex_config_file(
    path: &Path,
    executable_path: &Path,
) -> io::Result<CodexConfigPatchOutcome> {
    let existing = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let mut value = if existing.trim().is_empty() {
        TomlValue::Table(Default::default())
    } else {
        toml::from_str::<TomlValue>(&existing)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?
    };

    let root = value.as_table_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Codex config root must be a TOML table",
        )
    })?;

    let notify_command = codex_notify_command(executable_path);
    let mut preserved_custom_notify_hook = false;
    match root.get("notify") {
        Some(existing_notify)
            if !toml_value_string_array_matches(existing_notify, notify_command.as_slice()) =>
        {
            preserved_custom_notify_hook = true;
        }
        Some(_) => {}
        None => {
            root.insert(
                "notify".to_owned(),
                TomlValue::Array(
                    notify_command
                        .iter()
                        .cloned()
                        .map(TomlValue::String)
                        .collect(),
                ),
            );
        }
    }

    let tui_value = root
        .entry("tui".to_owned())
        .or_insert_with(|| TomlValue::Table(Default::default()));
    let tui = tui_value
        .as_table_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "[tui] must be a TOML table"))?;
    let required_notifications = TomlValue::Array(codex_notification_events());
    if tui.get("notifications") != Some(&required_notifications) {
        tui.insert("notifications".to_owned(), required_notifications);
    }
    let required_notification_method = TomlValue::String(CODEX_NOTIFICATION_METHOD.to_owned());
    if tui.get("notification_method") != Some(&required_notification_method) {
        tui.insert(
            "notification_method".to_owned(),
            required_notification_method,
        );
    }

    let rendered = toml::to_string_pretty(&value)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    if rendered == existing {
        return if preserved_custom_notify_hook {
            Ok(CodexConfigPatchOutcome::CustomNotifyHookPreserved)
        } else {
            Ok(CodexConfigPatchOutcome::Unchanged)
        };
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, rendered)?;
    if preserved_custom_notify_hook {
        Ok(CodexConfigPatchOutcome::CustomNotifyHookPreserved)
    } else {
        Ok(CodexConfigPatchOutcome::Updated)
    }
}

pub fn handle_codex_notify_from_env(payload: &str) -> io::Result<()> {
    let terminal_id = env::var(MERGEN_TERMINAL_ID_ENV_VAR).ok();
    let inbox_dir = env::var_os(MERGEN_AI_INBOX_DIR_ENV_VAR).map(PathBuf::from);
    let inbox_token = env::var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR).ok();
    let tool_hint = env::var(MERGEN_AI_TOOL_HINT_ENV_VAR).ok();
    let _ = handle_codex_notify(
        payload,
        terminal_id.as_deref(),
        inbox_dir.as_deref(),
        inbox_token.as_deref(),
        tool_hint.as_deref(),
    )?;
    Ok(())
}

fn handle_codex_notify(
    payload: &str,
    terminal_id: Option<&str>,
    inbox_dir: Option<&Path>,
    inbox_token: Option<&str>,
    tool_hint: Option<&str>,
) -> io::Result<bool> {
    let (Some(terminal_id), Some(inbox_dir), Some(inbox_token)) =
        (terminal_id, inbox_dir, inbox_token)
    else {
        return Ok(false);
    };

    write_codex_notify_event(payload, terminal_id, inbox_dir, inbox_token, tool_hint)?;
    Ok(true)
}

pub fn write_codex_notify_event(
    payload: &str,
    terminal_id: &str,
    inbox_dir: &Path,
    inbox_token: &str,
    tool_hint: Option<&str>,
) -> io::Result<()> {
    if let Some(tool_hint) = tool_hint {
        if !tool_hint.eq_ignore_ascii_case(MERGEN_AI_TOOL_HINT_CODEX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unexpected tool hint: {tool_hint}"),
            ));
        }
    }

    let event_kind = classify_codex_notify_payload(payload);
    let status = if event_kind.is_some() {
        "attention"
    } else {
        "unknown"
    };
    let timestamp_utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned());
    let event = CodexNotifyInboxEvent {
        terminal_id: terminal_id.to_owned(),
        tool: MERGEN_AI_TOOL_HINT_CODEX.to_owned(),
        status: status.to_owned(),
        inbox_token: Some(inbox_token.to_owned()),
        event_kind: event_kind.map(str::to_owned),
        raw_json: payload.trim().to_owned(),
        timestamp_utc,
    };

    fs::create_dir_all(inbox_dir)?;
    let terminal_id_u64 = terminal_id.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid terminal id: {terminal_id}"),
        )
    })?;
    let path = codex_notify_inbox_path_for_dir(inbox_dir, terminal_id_u64, inbox_token);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(&event)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn classify_codex_notify_payload(payload: &str) -> Option<&'static str> {
    let lower = payload.to_ascii_lowercase();
    if lower.contains(CODEX_APPROVAL_REQUESTED_EVENT)
        || lower.contains("approval_requested")
        || lower.contains("approval requested")
    {
        return Some(CODEX_APPROVAL_REQUESTED_EVENT);
    }

    if lower.contains(CODEX_TURN_COMPLETE_EVENT)
        || lower.contains("agent_turn_complete")
        || lower.contains("agent turn complete")
        || lower.contains("turn/completed")
    {
        return Some(CODEX_TURN_COMPLETE_EVENT);
    }

    None
}

fn codex_notify_command(executable_path: &Path) -> Vec<String> {
    vec![
        executable_path.display().to_string(),
        "--codex-notify".to_owned(),
    ]
}

fn codex_notification_events() -> Vec<TomlValue> {
    vec![
        TomlValue::String(CODEX_TURN_COMPLETE_EVENT.to_owned()),
        TomlValue::String(CODEX_APPROVAL_REQUESTED_EVENT.to_owned()),
    ]
}

fn toml_value_string_array_matches(value: &TomlValue, expected: &[String]) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };

    items.len() == expected.len()
        && items
            .iter()
            .zip(expected)
            .all(|(item, expected_item)| item.as_str() == Some(expected_item.as_str()))
}

fn toml_value_string_array_contains_all(value: &TomlValue, required: &[&str]) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };

    required.iter().all(|required_item| {
        items
            .iter()
            .any(|item| item.as_str() == Some(*required_item))
    })
}

fn inspect_codex_cli_integration_at_path(
    path: PathBuf,
    executable_path: &Path,
) -> CodexIntegrationStatus {
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return CodexIntegrationStatus::NeedsSetup { path };
        }
        Err(err) => {
            return CodexIntegrationStatus::ConfigReadError {
                path: Some(path),
                error: err.to_string(),
            };
        }
    };

    let value = if existing.trim().is_empty() {
        TomlValue::Table(Default::default())
    } else {
        match toml::from_str::<TomlValue>(&existing) {
            Ok(value) => value,
            Err(err) => {
                return CodexIntegrationStatus::ConfigReadError {
                    path: Some(path),
                    error: format!("Invalid config.toml: {err}"),
                };
            }
        }
    };

    let Some(root) = value.as_table() else {
        return CodexIntegrationStatus::ConfigReadError {
            path: Some(path),
            error: "Codex config root must be a TOML table".to_owned(),
        };
    };

    let notify_command = codex_notify_command(executable_path);
    let notify_matches = match root.get("notify") {
        Some(existing_notify)
            if toml_value_string_array_matches(existing_notify, notify_command.as_slice()) =>
        {
            true
        }
        Some(_) => {
            return CodexIntegrationStatus::CustomNotifyHook { path };
        }
        None => false,
    };

    let Some(tui) = root.get("tui").and_then(TomlValue::as_table) else {
        return CodexIntegrationStatus::NeedsSetup { path };
    };
    let notification_method_matches = tui.get("notification_method").and_then(TomlValue::as_str)
        == Some(CODEX_NOTIFICATION_METHOD);
    let notifications_match = tui.get("notifications").is_some_and(|notifications| {
        toml_value_string_array_contains_all(
            notifications,
            &[CODEX_TURN_COMPLETE_EVENT, CODEX_APPROVAL_REQUESTED_EVENT],
        )
    });

    if notify_matches && notification_method_matches && notifications_match {
        CodexIntegrationStatus::EnabledHealthy { path }
    } else {
        CodexIntegrationStatus::NeedsSetup { path }
    }
}

fn run_codex_command(args: &[&str]) -> io::Result<Output> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("codex");
        command.args(args);
        command.creation_flags(CREATE_NO_WINDOW);
        return command.output();
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("codex").args(args).output()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        codex_env_pairs, codex_notify_inbox_path_for_dir, handle_codex_notify,
        inspect_codex_cli_integration_at_path, patch_codex_config_file, write_codex_notify_event,
        CodexConfigPatchOutcome, CodexIntegrationStatus, CodexNotifyInboxEvent,
        CODEX_APPROVAL_REQUESTED_EVENT, CODEX_NOTIFICATION_METHOD, CODEX_TURN_COMPLETE_EVENT,
        MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, MERGEN_AI_INBOX_DIR_ENV_VAR,
        MERGEN_AI_TOOL_HINT_CODEX, MERGEN_AI_TOOL_HINT_ENV_VAR, MERGEN_TERMINAL_ID_ENV_VAR,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn codex_env_pairs_include_terminal_id_inbox_dir_tool_hint_and_token() {
        let inbox_dir =
            Path::new(r"C:\Users\furkan.cakir\AppData\Roaming\Mergen\MergenADE\runtime\codex-cli");
        let pairs = codex_env_pairs(23, inbox_dir, "codex-token-23");

        assert_eq!(pairs[0].0, MERGEN_TERMINAL_ID_ENV_VAR);
        assert_eq!(pairs[0].1, OsString::from("23"));
        assert_eq!(pairs[1].0, MERGEN_AI_INBOX_DIR_ENV_VAR);
        assert_eq!(pairs[1].1, inbox_dir.as_os_str());
        assert_eq!(pairs[2].0, MERGEN_AI_TOOL_HINT_ENV_VAR);
        assert_eq!(pairs[2].1, OsString::from(MERGEN_AI_TOOL_HINT_CODEX));
        assert_eq!(pairs[3].0, MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR);
        assert_eq!(pairs[3].1, OsString::from("codex-token-23"));
    }

    #[test]
    fn codex_notify_writer_marks_approval_requests_as_attention() {
        let temp = TestTempDir::new("codex-notify-approval");
        write_codex_notify_event(
            r#"{"event":"approval-requested"}"#,
            "7",
            &temp.path,
            "codex-token-7",
            Some(MERGEN_AI_TOOL_HINT_CODEX),
        )
        .expect("should write codex notify event");

        let path = codex_notify_inbox_path_for_dir(&temp.path, 7, "codex-token-7");
        let payload = fs::read_to_string(path).expect("should read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(payload.trim()).expect("should parse inbox event");

        assert_eq!(event.status, "attention");
        assert_eq!(event.inbox_token.as_deref(), Some("codex-token-7"));
        assert_eq!(
            event.event_kind.as_deref(),
            Some(CODEX_APPROVAL_REQUESTED_EVENT)
        );
    }

    #[test]
    fn codex_notify_writer_keeps_unknown_payloads_for_bell_fallback() {
        let temp = TestTempDir::new("codex-notify-unknown");
        write_codex_notify_event(
            r#"{"message":"something else"}"#,
            "8",
            &temp.path,
            "codex-token-8",
            Some(MERGEN_AI_TOOL_HINT_CODEX),
        )
        .expect("should write codex notify event");

        let path = codex_notify_inbox_path_for_dir(&temp.path, 8, "codex-token-8");
        let payload = fs::read_to_string(path).expect("should read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(payload.trim()).expect("should parse inbox event");

        assert_eq!(event.status, "unknown");
        assert_eq!(event.inbox_token.as_deref(), Some("codex-token-8"));
        assert_eq!(event.event_kind, None);
    }

    #[test]
    fn codex_notify_handler_without_routing_is_noop() {
        let handled = handle_codex_notify(
            r#"{"event":"approval-requested"}"#,
            None,
            None,
            None,
            Some(MERGEN_AI_TOOL_HINT_CODEX),
        )
        .expect("missing routing should not fail");

        assert!(!handled);
    }

    #[test]
    fn codex_notify_handler_with_partial_routing_is_noop() {
        let temp = TestTempDir::new("codex-notify-partial");
        let handled = handle_codex_notify(
            r#"{"event":"approval-requested"}"#,
            None,
            Some(temp.path.as_path()),
            Some("codex-token-partial"),
            Some(MERGEN_AI_TOOL_HINT_CODEX),
        )
        .expect("partial routing should not fail");

        assert!(!handled);
        assert!(fs::read_dir(&temp.path)
            .expect("temp dir should exist")
            .next()
            .is_none());
    }

    #[test]
    fn patch_codex_config_sets_notify_when_missing() {
        let temp = TestTempDir::new("codex-config");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
model = "gpt-5"

[features]
web_search = true

[tui]
alternate_screen = "never"
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            patch_codex_config_file(&path, executable).expect("patch should succeed"),
            CodexConfigPatchOutcome::Updated
        );

        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

        assert_eq!(value["model"].as_str(), Some("gpt-5"));
        assert_eq!(value["features"]["web_search"].as_bool(), Some(true));
        assert_eq!(value["tui"]["alternate_screen"].as_str(), Some("never"));
        assert_eq!(
            value["tui"]["notification_method"].as_str(),
            Some(CODEX_NOTIFICATION_METHOD)
        );
        assert_eq!(
            value["tui"]["notifications"]
                .as_array()
                .expect("notifications array")
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![CODEX_TURN_COMPLETE_EVENT, CODEX_APPROVAL_REQUESTED_EVENT]
        );
        assert_eq!(
            value["notify"]
                .as_array()
                .expect("notify command")
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![
                r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe",
                "--codex-notify"
            ]
        );
    }

    #[test]
    fn patch_codex_config_is_idempotent_for_existing_mergen_notify() {
        let temp = TestTempDir::new("codex-config-idempotent");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
model = "gpt-5"

[features]
web_search = true

notify = ['C:\Users\furkan.cakir\Desktop\mergen-ade.exe', "--codex-notify"]

[tui]
alternate_screen = "never"
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            patch_codex_config_file(&path, executable).expect("first patch should succeed"),
            CodexConfigPatchOutcome::Updated
        );
        assert!(matches!(
            patch_codex_config_file(&path, executable).expect("second patch should be idempotent"),
            CodexConfigPatchOutcome::Unchanged
        ));

        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

        assert_eq!(value["model"].as_str(), Some("gpt-5"));
        assert_eq!(value["features"]["web_search"].as_bool(), Some(true));
        assert_eq!(value["tui"]["alternate_screen"].as_str(), Some("never"));
        assert_eq!(
            value["tui"]["notification_method"].as_str(),
            Some(CODEX_NOTIFICATION_METHOD)
        );
        assert_eq!(
            value["tui"]["notifications"]
                .as_array()
                .expect("notifications array")
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![CODEX_TURN_COMPLETE_EVENT, CODEX_APPROVAL_REQUESTED_EVENT]
        );
        assert_eq!(
            value["notify"]
                .as_array()
                .expect("notify command")
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![
                r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe",
                "--codex-notify"
            ]
        );
    }

    #[test]
    fn patch_codex_config_preserves_existing_custom_notify_hook() {
        let temp = TestTempDir::new("codex-config-custom-notify");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
model = "gpt-5"
notify = ["powershell.exe", "-File", "custom-notify.ps1"]

[tui]
alternate_screen = "never"
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            patch_codex_config_file(&path, executable)
                .expect("patch should preserve custom notify"),
            CodexConfigPatchOutcome::CustomNotifyHookPreserved
        );

        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

        assert_eq!(value["notify"][0].as_str(), Some("powershell.exe"));
        assert_eq!(value["notify"][1].as_str(), Some("-File"));
        assert_eq!(value["notify"][2].as_str(), Some("custom-notify.ps1"));
        assert_eq!(value["tui"]["alternate_screen"].as_str(), Some("never"));
        assert_eq!(
            value["tui"]["notifications"]
                .as_array()
                .expect("notifications array")
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![CODEX_TURN_COMPLETE_EVENT, CODEX_APPROVAL_REQUESTED_EVENT]
        );
        assert_eq!(
            value["tui"]["notification_method"].as_str(),
            Some(CODEX_NOTIFICATION_METHOD)
        );
    }

    #[test]
    fn patch_codex_config_overwrites_existing_tui_notification_preferences() {
        let temp = TestTempDir::new("codex-config-preserve-tui");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
notify = ['C:\Users\furkan.cakir\Desktop\mergen-ade.exe', "--codex-notify"]

[tui]
notifications = false
notification_method = "desktop"
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            patch_codex_config_file(&path, executable).expect("patch should succeed"),
            CodexConfigPatchOutcome::Updated
        );

        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");
        assert_eq!(
            value["tui"]["notifications"]
                .as_array()
                .expect("notifications array")
                .iter()
                .map(|item| item.as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![CODEX_TURN_COMPLETE_EVENT, CODEX_APPROVAL_REQUESTED_EVENT]
        );
        assert_eq!(
            value["tui"]["notification_method"].as_str(),
            Some(CODEX_NOTIFICATION_METHOD)
        );
    }

    #[test]
    fn inspect_codex_cli_integration_reports_enabled_healthy_when_notify_and_tui_match() {
        let temp = TestTempDir::new("codex-config-health");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
notify = ['C:\Users\furkan.cakir\Desktop\mergen-ade.exe', "--codex-notify"]

[tui]
notification_method = "bel"
notifications = ["agent-turn-complete", "approval-requested", "extra-event"]
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            inspect_codex_cli_integration_at_path(path.clone(), executable),
            CodexIntegrationStatus::EnabledHealthy { path }
        );
    }

    #[test]
    fn inspect_codex_cli_integration_reports_needs_setup_when_notify_is_missing() {
        let temp = TestTempDir::new("codex-config-missing-notify");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
[tui]
notification_method = "bel"
notifications = ["agent-turn-complete", "approval-requested"]
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            inspect_codex_cli_integration_at_path(path.clone(), executable),
            CodexIntegrationStatus::NeedsSetup { path }
        );
    }

    #[test]
    fn inspect_codex_cli_integration_reports_custom_notify_hook_when_notify_differs() {
        let temp = TestTempDir::new("codex-config-custom-inspect");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
notify = ["powershell.exe", "-File", "custom-notify.ps1"]

[tui]
notification_method = "bel"
notifications = ["agent-turn-complete", "approval-requested"]
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            inspect_codex_cli_integration_at_path(path.clone(), executable),
            CodexIntegrationStatus::CustomNotifyHook { path }
        );
    }

    #[test]
    fn inspect_codex_cli_integration_reports_needs_setup_when_required_notifications_are_missing() {
        let temp = TestTempDir::new("codex-config-missing-events");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
notify = ['C:\Users\furkan.cakir\Desktop\mergen-ade.exe', "--codex-notify"]

[tui]
notification_method = "bel"
notifications = ["agent-turn-complete"]
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            inspect_codex_cli_integration_at_path(path.clone(), executable),
            CodexIntegrationStatus::NeedsSetup { path }
        );
    }

    #[test]
    fn inspect_codex_cli_integration_reports_config_read_error_for_invalid_toml() {
        let temp = TestTempDir::new("codex-config-invalid");
        let path = temp.path.join("config.toml");
        fs::write(&path, "notify = [").expect("write invalid config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        let status = inspect_codex_cli_integration_at_path(path.clone(), executable);

        assert!(matches!(
            status,
            CodexIntegrationStatus::ConfigReadError {
                path: Some(observed_path),
                ..
            } if observed_path == path
        ));
    }

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
}
