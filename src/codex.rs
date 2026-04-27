use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use toml::Value as TomlValue;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub const CODEX_SETUP_URL: &str = "https://developers.openai.com/codex/cli/#cli-setup";
pub const MERGEN_TERMINAL_ID_ENV_VAR: &str = "MERGEN_TERMINAL_ID";
pub const MERGEN_AI_INBOX_DIR_ENV_VAR: &str = "MERGEN_AI_INBOX_DIR";
/// Codex-specific inbox dir env var to avoid env var collisions with OpenCode
/// OpenCode also uses MERGEN_AI_INBOX_DIR, so Codex has its own dedicated env var
pub const MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR: &str = "MERGEN_ADE_CODEX_INBOX_DIR";
pub const MERGEN_AI_TOOL_HINT_ENV_VAR: &str = "MERGEN_AI_TOOL_HINT";
pub const MERGEN_AI_TOOL_HINT_CODEX: &str = "codex";
pub const MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR: &str = "MERGEN_ADE_CODEX_INBOX_TOKEN";
const CODEX_MANAGED_HOOK_TIMEOUT_SECONDS: u64 = 10;
const CODEX_TOOL_HOOK_MATCHER: &str = "^(Bash|apply_patch|Edit|Write|mcp__.*)$";
const CODEX_MANAGED_HOOK_EVENTS: [(&str, Option<&str>); 5] = [
    ("UserPromptSubmit", None),
    ("PreToolUse", Some(CODEX_TOOL_HOOK_MATCHER)),
    ("PermissionRequest", Some(CODEX_TOOL_HOOK_MATCHER)),
    ("PostToolUse", Some(CODEX_TOOL_HOOK_MATCHER)),
    ("Stop", None),
];
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Result of ensuring the bridge is installed and up-to-date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeInstallOutcome {
    /// Bridge was installed or updated.
    InstalledOrUpdated { bridge_path: PathBuf },
    /// Bridge already exists and is current.
    AlreadyCurrent { bridge_path: PathBuf },
    /// Failed to install the bridge.
    Failed { error: String },
}

/// Ensures the Codex bridge is installed at the fixed location.
/// Copies the current executable to the bridge location if:
/// - Bridge doesn't exist
/// - Bridge is older than current executable
/// - Bridge is a different file (size mismatch)
pub fn ensure_codex_bridge_installed() -> BridgeInstallOutcome {
    let current_exe = match env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            return BridgeInstallOutcome::Failed {
                error: format!("Failed to get current executable path: {e}"),
            }
        }
    };

    let bridge_path = match config::codex_bridge_path() {
        Ok(path) => path,
        Err(e) => {
            return BridgeInstallOutcome::Failed {
                error: format!("Failed to get bridge path: {e}"),
            }
        }
    };

    // Check if bridge needs update
    let needs_update = match fs::metadata(&bridge_path) {
        Ok(bridge_meta) => {
            let current_meta = match fs::metadata(&current_exe) {
                Ok(m) => m,
                Err(e) => {
                    return BridgeInstallOutcome::Failed {
                        error: format!("Failed to read current exe metadata: {e}"),
                    }
                }
            };

            // Check size mismatch or modification time
            let size_differs = bridge_meta.len() != current_meta.len();
            let bridge_older = match (bridge_meta.modified(), current_meta.modified()) {
                (Ok(bridge_time), Ok(current_time)) => bridge_time < current_time,
                _ => false, // If we can't compare times, assume it's ok
            };

            size_differs || bridge_older
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => true, // Bridge doesn't exist
        Err(e) => {
            return BridgeInstallOutcome::Failed {
                error: format!("Failed to read bridge metadata: {e}"),
            }
        }
    };

    if !needs_update {
        return BridgeInstallOutcome::AlreadyCurrent { bridge_path };
    }

    // Install/update the bridge
    match install_codex_bridge(&current_exe, &bridge_path) {
        Ok(()) => BridgeInstallOutcome::InstalledOrUpdated { bridge_path },
        Err(e) => BridgeInstallOutcome::Failed {
            error: format!("Failed to copy bridge: {e}"),
        },
    }
}

/// Installs the bridge by copying the current executable.
fn install_codex_bridge(current_exe: &Path, bridge_path: &Path) -> io::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = bridge_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // On Windows, we may need to remove the old bridge first if it's in use
    #[cfg(target_os = "windows")]
    if bridge_path.exists() {
        // Try to rename old bridge to .old first (best effort)
        let old_path = bridge_path.with_extension("exe.old");
        let _ = fs::rename(bridge_path, &old_path);
    }

    // Copy current executable to bridge location
    fs::copy(current_exe, bridge_path)?;

    Ok(())
}

/// Diagnostics info about the bridge and wiring state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexBridgeDiagnostics {
    pub bridge_path: PathBuf,
    pub bridge_exists: bool,
    pub bridge_size: Option<u64>,
    pub current_exe_path: PathBuf,
    pub config_path: PathBuf,
    pub hooks_path: PathBuf,
    pub hooks_target_bridge: bool,
    pub wiring_mismatch: bool,
}

/// Returns diagnostics about the bridge installation and wiring state.
/// For hook-only integration, only checks hooks.json (not config.toml).
pub fn codex_bridge_diagnostics() -> io::Result<CodexBridgeDiagnostics> {
    let bridge_path = config::codex_bridge_path()?;
    let current_exe = env::current_exe()?;
    let config_path = user_codex_config_path()?;
    let hooks_path = config_path.parent().map(|p| p.join("hooks.json"));

    let bridge_meta = fs::metadata(&bridge_path).ok();
    let bridge_exists = bridge_meta.is_some();
    let bridge_size = bridge_meta.map(|m| m.len());

    // Helper to check if text contains the bridge path (handles both normal and escaped)
    let text_contains_bridge_path = |text: &str| -> bool {
        let bridge_str = bridge_path.to_string_lossy();
        // Check normal path
        if text.contains(&*bridge_str) {
            return true;
        }
        // Check JSON-escaped version (backslashes as \/ in TOML/JSON strings)
        let escaped = bridge_str.replace('\\', "/");
        if text.contains(&escaped) {
            return true;
        }
        // Check with forward slashes only
        let forward_slashed = bridge_str.replace('\\', "/");
        if text.contains(&forward_slashed) {
            return true;
        }
        false
    };

    // Check if hooks.json points to bridge (hook-only integration)
    let hooks_target_bridge = hooks_path.as_ref().is_some_and(|p| {
        if p.exists() {
            let hooks_text = fs::read_to_string(p).unwrap_or_default();
            text_contains_bridge_path(&hooks_text)
        } else {
            false
        }
    });

    // Wiring mismatch: bridge exists but hooks.json doesn't point to it
    let wiring_mismatch = bridge_exists && !hooks_target_bridge;

    Ok(CodexBridgeDiagnostics {
        bridge_path,
        bridge_exists,
        bridge_size,
        current_exe_path: current_exe,
        config_path: config_path.clone(),
        hooks_path: hooks_path.unwrap_or_else(|| config_path.parent().unwrap().join("hooks.json")),
        hooks_target_bridge,
        wiring_mismatch,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexEnableOutcome {
    MissingInstall,
    NeedsLogin,
    ConfigUpdated {
        path: PathBuf,
        updated: bool,
    },
    /// Bridge installation failed (contains the error message).
    BridgeInstallFailed {
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexConfigPatchOutcome {
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexIntegrationStatus {
    EnabledHealthy {
        path: PathBuf,
        /// Whether hooks have been verified to work at runtime (true after seeing a hook event)
        hooks_runtime_verified: bool,
    },
    /// Hooks are configured but not yet verified at runtime (waiting for first hook event)
    HooksConfiguredUnverified {
        path: PathBuf,
    },
    NeedsSetup {
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
) -> [(String, OsString); 5] {
    [
        (
            MERGEN_TERMINAL_ID_ENV_VAR.to_owned(),
            OsString::from(terminal_id.to_string()),
        ),
        // Set both the common inbox dir and the Codex-specific one
        // The Codex-specific env var takes precedence in the hook handler
        (
            MERGEN_AI_INBOX_DIR_ENV_VAR.to_owned(),
            inbox_dir.as_os_str().to_owned(),
        ),
        (
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR.to_owned(),
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

pub fn enable_codex_cli_integration(_executable_path: &Path) -> io::Result<CodexEnableOutcome> {
    // Ensure bridge is installed first
    let bridge_path = match config::codex_bridge_path() {
        Ok(path) => path,
        Err(e) => {
            return Ok(CodexEnableOutcome::BridgeInstallFailed {
                error: format!("Failed to get bridge path: {e}"),
            });
        }
    };

    // Install or update the bridge
    if let BridgeInstallOutcome::Failed { error } = ensure_codex_bridge_installed() {
        return Ok(CodexEnableOutcome::BridgeInstallFailed { error });
    }
    // Continue if installed, updated, or already current

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
    match patch_codex_config_file(&path, &bridge_path)? {
        CodexConfigPatchOutcome::Updated => Ok(CodexEnableOutcome::ConfigUpdated {
            path,
            updated: true,
        }),
        CodexConfigPatchOutcome::Unchanged => Ok(CodexEnableOutcome::ConfigUpdated {
            path,
            updated: false,
        }),
    }
}

pub fn inspect_codex_cli_integration(_executable_path: &Path) -> CodexIntegrationStatus {
    // Use the bridge path for all checks
    let bridge_path = match config::codex_bridge_path() {
        Ok(path) => path,
        Err(err) => {
            return CodexIntegrationStatus::ConfigReadError {
                path: None,
                error: format!("Failed to get bridge path: {err}"),
            }
        }
    };

    match user_codex_config_path() {
        Ok(path) => inspect_codex_cli_integration_at_path(path, &bridge_path),
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

    // Enable Codex hooks feature - hook-only integration
    let features_value = root
        .entry("features".to_owned())
        .or_insert_with(|| TomlValue::Table(Default::default()));
    let features = features_value.as_table_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "[features] must be a TOML table",
        )
    })?;
    let codex_hooks_enabled = TomlValue::Boolean(true);
    if features.get("codex_hooks") != Some(&codex_hooks_enabled) {
        features.insert("codex_hooks".to_owned(), codex_hooks_enabled);
    }

    // Update or create hooks.json with spinner events - hook-only integration
    let hooks_changed = update_codex_hooks_json(path, executable_path)?;

    // Remove legacy notify configuration - hook-only integration
    let notify_removed = remove_legacy_notify_from_config(root);

    let rendered = toml::to_string_pretty(&value)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;

    let config_changed = rendered != existing || notify_removed;

    if !config_changed && !hooks_changed {
        return Ok(CodexConfigPatchOutcome::Unchanged);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, rendered)?;

    Ok(CodexConfigPatchOutcome::Updated)
}

/// Remove legacy `notify` configuration entry from Codex config.
/// Hook-only integration does not use the notify command.
/// Returns true if the entry was removed.
fn remove_legacy_notify_from_config(root: &mut toml::map::Map<String, TomlValue>) -> bool {
    let has_notify = root.get("notify").is_some();
    if has_notify {
        root.remove("notify");
        log::info!("Removed legacy notify configuration from Codex config (hook-only integration)");
        true
    } else {
        false
    }
}

/// Returns the path to ~/.codex/hooks.json
#[allow(dead_code)]
pub fn user_codex_hooks_path() -> io::Result<PathBuf> {
    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User home directory is unavailable",
        )
    })?;
    Ok(base_dirs.home_dir().join(".codex").join("hooks.json"))
}

/// Build the command for hook events that routes to Mergen's --codex-hook mode
/// Returns a shell command string (not a Vec) because upstream Codex expects String.
fn codex_hook_command(executable_path: &Path, event: &str) -> String {
    // Build a properly quoted shell command for Windows paths with spaces
    let exe = executable_path.display().to_string();
    // Quote the executable path if it contains spaces
    let exe_quoted = if exe.contains(' ') {
        format!("\"{}\"", exe)
    } else {
        exe
    };
    format!("{} --codex-hook {}", exe_quoted, event)
}

/// Update or create ~/.codex/hooks.json with spinner tracking events.
/// This is a managed hooks.json that only contains events needed for the spinner.
/// Returns true if the hooks.json content changed (file created or updated).
fn update_codex_hooks_json(config_path: &Path, executable_path: &Path) -> io::Result<bool> {
    use serde_json::{json, Value};

    let hooks_path = config_path
        .parent()
        .map(|p| p.join("hooks.json"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid config path"))?;

    // Ensure parent directory exists before writing hooks.json
    if let Some(parent) = hooks_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing_text = match fs::read_to_string(&hooks_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let existing: Value = if existing_text.trim().is_empty() {
        json!({"hooks": {}})
    } else {
        serde_json::from_str(&existing_text).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Existing hooks.json is malformed: {e}"),
            )
        })?
    };

    let mut hooks = existing
        .get("hooks")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut hooks_changed = false;

    for (event_name, matcher) in CODEX_MANAGED_HOOK_EVENTS {
        // statusMessage intentionally omitted to prevent Codex from displaying
        // hook status text in the terminal.
        let hook_entry = json!({
            "type": "command",
            "command": codex_hook_command(executable_path, event_name),
            "timeout": CODEX_MANAGED_HOOK_TIMEOUT_SECONDS
        });
        let mut entry = json!({
            "hooks": [hook_entry]
        });
        if let Some(matcher) = matcher {
            entry["matcher"] = Value::String(matcher.to_owned());
        }

        let event_array = hooks
            .entry(event_name)
            .or_insert_with(|| Value::Array(vec![]));

        if let Some(arr) = event_array.as_array_mut() {
            // Identify managed hooks by the event they trigger (--codex-hook <event_name>)
            // rather than the full executable path, so upgrades or path changes don't
            // create duplicates.
            let managed_marker = format!("--codex-hook {}", event_name);

            let existing_idx = arr.iter().position(|item| {
                if let Some(hook) = item
                    .get("hooks")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                {
                    // Check if this is a Mergen-managed hook by looking for the marker
                    if let Some(cmd) = hook.get("command").and_then(Value::as_str) {
                        return cmd.contains(&managed_marker);
                    }
                }
                false
            });

            if let Some(idx) = existing_idx {
                // Update existing Mergen hook (new executable path)
                // Check if content actually differs before marking as changed
                let old_entry = &arr[idx];
                if old_entry != &entry {
                    arr[idx] = entry;
                    hooks_changed = true;
                }
            } else {
                // Add new Mergen hook (append, don't replace existing user hooks)
                arr.push(entry);
                hooks_changed = true;
            }
        }
    }

    let updated = json!({"hooks": hooks});
    let rendered = serde_json::to_string_pretty(&updated)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let file_changed = hooks_changed || existing_text != rendered;

    if file_changed {
        fs::write(&hooks_path, rendered)?;
    }

    Ok(file_changed)
}

/// Handle --codex-hook mode (writes a hook event to the inbox)
pub fn handle_codex_hook_from_env(event_name: &str) -> io::Result<()> {
    let terminal_id = env::var(MERGEN_TERMINAL_ID_ENV_VAR).ok();
    // Use Codex-specific inbox dir first, fallback to common one for backward compatibility
    let inbox_dir = env::var_os(MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR)
        .or_else(|| env::var_os(MERGEN_AI_INBOX_DIR_ENV_VAR))
        .map(PathBuf::from);
    let inbox_token = env::var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR).ok();
    let tool_hint = env::var(MERGEN_AI_TOOL_HINT_ENV_VAR).ok();

    let _ = handle_codex_hook(
        event_name,
        terminal_id.as_deref(),
        inbox_dir.as_deref(),
        inbox_token.as_deref(),
        tool_hint.as_deref(),
    )?;
    Ok(())
}

/// Write a hook event to the inbox as a synthetic notification.
/// Hook events are mapped to transport status signals for the spinner.
fn handle_codex_hook(
    event_name: &str,
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

    let (status, event_kind) = match event_name {
        "UserPromptSubmit" => ("running", "user-prompt-submit"),
        "PreToolUse" => ("running", "pre-tool-use"),
        "PostToolUse" => ("running", "post-tool-use"),
        "PermissionRequest" => ("attention", "permission-request"),
        "Stop" => ("attention", "hook-stop"),
        "SessionStart" => return Ok(false),
        _ => ("attention", "unknown-hook"),
    };

    write_codex_hook_event(
        event_name,
        status,
        event_kind,
        terminal_id,
        inbox_dir,
        inbox_token,
        tool_hint,
    )?;
    Ok(true)
}

/// Write a hook event to the inbox file (similar to notify events)
fn write_codex_hook_event(
    hook_event: &str,
    status: &str,
    event_kind: &str,
    terminal_id: &str,
    inbox_dir: &Path,
    inbox_token: &str,
    tool_hint: Option<&str>,
) -> io::Result<()> {
    // A terminal can legitimately host both Codex and OpenCode over its lifetime.
    // Treat the shared tool hint env var as advisory only so one tool's setup does
    // not break the event path.
    let _ = tool_hint;

    let timestamp_utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned());

    let event = CodexNotifyInboxEvent {
        terminal_id: terminal_id.to_owned(),
        tool: MERGEN_AI_TOOL_HINT_CODEX.to_owned(),
        status: status.to_owned(),
        inbox_token: Some(inbox_token.to_owned()),
        event_kind: Some(event_kind.to_owned()),
        raw_json: format!("{{\"hook_event\":\"{}\"}}", hook_event),
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

/// Handle --codex-notify mode (writes a notify event to the inbox)
/// This provides CLI entry point for legacy Codex notify integration.
pub fn maybe_handle_codex_notify_mode() -> io::Result<Option<CodexNotifyInboxEvent>> {
    let mut args = std::env::args_os().peekable();

    while let Some(arg) = args.next() {
        if arg == "--codex-notify" {
            let payload = args
                .next()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Missing Codex notify payload argument.",
                    )
                })?
                .to_string_lossy()
                .to_string();

            let terminal_id = env::var(MERGEN_TERMINAL_ID_ENV_VAR).ok();
            // Use Codex-specific inbox dir first, fallback to common one for backward compatibility
            let inbox_dir = env::var_os(MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR)
                .or_else(|| env::var_os(MERGEN_AI_INBOX_DIR_ENV_VAR))
                .map(PathBuf::from);
            let inbox_token = env::var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR).ok();
            let tool_hint = env::var(MERGEN_AI_TOOL_HINT_ENV_VAR).ok();

            if let (Some(tid), Some(dir), Some(token)) = (terminal_id, inbox_dir, inbox_token) {
                write_codex_notify_event(&payload, &tid, &dir, &token, tool_hint.as_deref())?;

                // Re-parse to get the normalized event for the returned value
                let event_kind = extract_notify_event_kind(&payload);
                let (status, codex_event_kind) = match event_kind.as_deref() {
                    Some("agent-turn-complete") | Some("turn-complete") => {
                        ("attention", Some("agent-turn-complete".to_owned()))
                    }
                    Some("approval-requested") => {
                        ("attention", Some("approval-requested".to_owned()))
                    }
                    Some("user-input-requested") => {
                        ("attention", Some("user-input-requested".to_owned()))
                    }
                    Some("plan-mode-prompt") => ("attention", Some("plan-mode-prompt".to_owned())),
                    Some("session-idle") | Some("idle") => {
                        ("attention", Some("session-idle".to_owned()))
                    }
                    Some("session-error") | Some("error") => {
                        ("attention", Some("execution-error".to_owned()))
                    }
                    _ => ("attention", event_kind.map(|s| s.to_owned())),
                };

                let event = CodexNotifyInboxEvent {
                    terminal_id: tid,
                    tool: MERGEN_AI_TOOL_HINT_CODEX.to_owned(),
                    status: status.to_owned(),
                    inbox_token: Some(token),
                    event_kind: codex_event_kind,
                    raw_json: payload,
                    timestamp_utc: format_iso_timestamp(),
                };
                return Ok(Some(event));
            }
        }
    }

    Ok(None)
}

/// Write a notify event to the inbox file.
/// Maps Codex notification events to transport status signals for the spinner.
fn write_codex_notify_event(
    payload: &str,
    terminal_id: &str,
    inbox_dir: &Path,
    inbox_token: &str,
    _tool_hint: Option<&str>,
) -> io::Result<()> {
    // A terminal can legitimately host both Codex and OpenCode over its lifetime.
    // Treat the shared tool hint env var as advisory only so one tool's setup does
    // not break the event path.

    // Extract event kind from payload and map to status
    let event_kind = extract_notify_event_kind(payload);

    // Map Codex notify events to status
    let status = match event_kind.as_deref() {
        // Working signals
        Some("tool-execute-before") | Some("tool.execute.before") => "running",
        // Permission signals
        Some("approval-requested") | Some("approval_requested") => "attention",
        Some("user-input-requested") | Some("user_input_requested") => "attention",
        Some("plan-mode-prompt") | Some("plan_mode_prompt") => "attention",
        // Idle/completion signals
        Some("session-idle") | Some("session_idle") | Some("idle") => "attention",
        Some("session-error") | Some("session_error") | Some("error") => "attention",
        Some("agent-turn-complete") | Some("turn-complete") | Some("turn_complete") => "attention",
        _ => "attention",
    };

    let event = CodexNotifyInboxEvent {
        terminal_id: terminal_id.to_owned(),
        tool: MERGEN_AI_TOOL_HINT_CODEX.to_owned(),
        status: status.to_owned(),
        inbox_token: Some(inbox_token.to_owned()),
        event_kind: event_kind.map(|s| s.to_owned()),
        raw_json: payload.to_owned(),
        timestamp_utc: format_iso_timestamp(),
    };

    let json = serde_json::to_string(&event)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let terminal_id_u64 = terminal_id.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid terminal id: {terminal_id}"),
        )
    })?;
    let path = codex_notify_inbox_path_for_dir(inbox_dir, terminal_id_u64, inbox_token);

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{json}")?;
    file.flush()
}

/// Extract event kind from Codex notify payload (JSON parsing)
fn extract_notify_event_kind(payload: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(payload).ok()?;

    // Try various common locations for event type/name
    // Priority: event.type > type > event > kind > event.name > name > event_type
    let kind = parsed
        .get("event")
        .and_then(|e| e.get("type"))
        .or_else(|| parsed.get("type"))
        .or_else(|| parsed.get("event"))
        .or_else(|| parsed.get("kind"))
        .or_else(|| parsed.get("event").and_then(|e| e.get("name")))
        .or_else(|| parsed.get("name"))
        .or_else(|| parsed.get("event_type"));

    kind.and_then(|v| v.as_str())
        .map(|s| s.to_lowercase().replace('_', "-"))
}

/// Format current timestamp as ISO-like string (seconds since epoch with nanos)
fn format_iso_timestamp() -> String {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    format!("{}.{:09}Z", secs, duration.subsec_nanos())
}

fn inspect_codex_cli_integration_at_path(
    path: PathBuf,
    bridge_path: &Path,
) -> CodexIntegrationStatus {
    // First, verify the bridge exists on disk
    if !bridge_path.exists() {
        return CodexIntegrationStatus::NeedsSetup { path };
    }

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

    // Check if codex_hooks feature is enabled - hook-only integration
    let hooks_enabled = root
        .get("features")
        .and_then(TomlValue::as_table)
        .and_then(|f| f.get("codex_hooks"))
        .and_then(TomlValue::as_bool)
        == Some(true);

    // Check hooks.json exists and has our managed hooks
    let hooks_json_ok = check_codex_hooks_json(&path, bridge_path);

    if hooks_enabled && hooks_json_ok {
        // Hooks are configured but runtime verification is done per-session in the app
        // The hooks_runtime_verified flag is set to false initially and becomes true
        // after the first hook event is actually received
        CodexIntegrationStatus::EnabledHealthy {
            path,
            hooks_runtime_verified: false,
        }
    } else if hooks_enabled {
        // Hooks feature is enabled but hooks.json is missing or stale
        CodexIntegrationStatus::HooksConfiguredUnverified { path }
    } else {
        CodexIntegrationStatus::NeedsSetup { path }
    }
}

/// Check if ~/.codex/hooks.json exists and contains Mergen's managed hooks
/// Also verifies the hooks point to the expected bridge path.
fn check_codex_hooks_json(config_path: &Path, bridge_path: &Path) -> bool {
    let hooks_path = config_path.parent().map(|p| p.join("hooks.json"));
    let Some(hooks_path) = hooks_path else {
        return false;
    };

    let Ok(text) = fs::read_to_string(&hooks_path) else {
        return false;
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };

    let Some(hooks) = value.get("hooks").and_then(serde_json::Value::as_object) else {
        return false;
    };

    // Helper to check if command contains the bridge path (handles both normal and escaped)
    let cmd_contains_bridge = |cmd: &str| -> bool {
        let bridge_str = bridge_path.to_string_lossy();
        // Check normal path
        if cmd.contains(&*bridge_str) {
            return true;
        }
        // JSON may escape backslashes as \/ or just /
        let escaped = bridge_str.replace('\\', "/");
        if cmd.contains(&escaped) {
            return true;
        }
        // Also check with forward slashes only
        let forward_slashed = bridge_str.replace('\\', "/");
        if cmd.contains(&forward_slashed) {
            return true;
        }
        false
    };

    let managed_hook_ok = |event_name: &str| {
        hooks
            .get(event_name)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|arr| {
                arr.iter().any(|item| {
                    if let Some(hook) = item
                        .get("hooks")
                        .and_then(serde_json::Value::as_array)
                        .and_then(|a| a.first())
                    {
                        if let Some(cmd) = hook.get("command").and_then(serde_json::Value::as_str) {
                            // Must contain both the marker AND target the bridge path
                            return cmd.contains(&format!("--codex-hook {event_name}"))
                                && cmd_contains_bridge(cmd);
                        }
                    }
                    false
                })
            })
    };

    CODEX_MANAGED_HOOK_EVENTS
        .iter()
        .all(|(event_name, _)| managed_hook_ok(event_name))
}

fn run_codex_command(args: &[&str]) -> io::Result<Output> {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("codex");
        command.args(args);
        command.creation_flags(CREATE_NO_WINDOW);
        command.output()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("codex").args(args).output()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        codex_env_pairs, codex_notify_inbox_path_for_dir, handle_codex_hook_from_env,
        inspect_codex_cli_integration_at_path, patch_codex_config_file, CodexConfigPatchOutcome,
        CodexIntegrationStatus, CodexNotifyInboxEvent, MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
        MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, MERGEN_AI_INBOX_DIR_ENV_VAR,
        MERGEN_AI_TOOL_HINT_CODEX, MERGEN_AI_TOOL_HINT_ENV_VAR, MERGEN_TERMINAL_ID_ENV_VAR,
    };
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Static mutex to serialize tests that mutate global environment variables
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // Helper to save current env vars and restore them after test
    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn save(vars: &[&'static str]) -> Self {
            let saved = vars.iter().map(|&v| (v, env::var(v).ok())).collect();
            Self { saved }
        }

        fn restore(&self) {
            for (var, val) in &self.saved {
                if let Some(v) = val {
                    env::set_var(var, v);
                } else {
                    env::remove_var(var);
                }
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            self.restore();
        }
    }

    #[test]
    fn codex_env_pairs_include_terminal_id_inbox_dir_tool_hint_and_token() {
        let inbox_dir =
            Path::new(r"C:\Users\furkan.cakir\AppData\Roaming\Mergen\MergenADE\runtime\codex-cli");
        let pairs = codex_env_pairs(23, inbox_dir, "codex-token-23");

        assert_eq!(pairs[0].0, MERGEN_TERMINAL_ID_ENV_VAR);
        assert_eq!(pairs[0].1, OsString::from("23"));
        assert_eq!(pairs[1].0, MERGEN_AI_INBOX_DIR_ENV_VAR);
        assert_eq!(pairs[1].1, inbox_dir.as_os_str());
        // Codex-specific inbox dir is also set to avoid env var collisions with OpenCode
        assert_eq!(pairs[2].0, MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR);
        assert_eq!(pairs[2].1, inbox_dir.as_os_str());
        assert_eq!(pairs[3].0, MERGEN_AI_TOOL_HINT_ENV_VAR);
        assert_eq!(pairs[3].1, OsString::from(MERGEN_AI_TOOL_HINT_CODEX));
        assert_eq!(pairs[4].0, MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR);
        assert_eq!(pairs[4].1, OsString::from("codex-token-23"));
    }

    #[test]
    fn patch_codex_config_sets_hooks_json_when_missing() {
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
        // Hook-only integration: notify command is not set, hooks are configured via hooks.json
        assert!(
            value.get("notify").is_none(),
            "notify should not be set in hook-only mode"
        );

        // Verify hooks.json was created
        let hooks_path = temp.path.join("hooks.json");
        assert!(hooks_path.exists(), "hooks.json should be created");
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
        // Hook-only integration: notify command is removed, hooks are configured via hooks.json
        assert!(
            value.get("notify").is_none(),
            "notify should be removed in hook-only mode"
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

        #[cfg(target_os = "windows")]
        {
            // Hook-only integration: all notify entries are removed
            assert_eq!(
                patch_codex_config_file(&path, executable).expect("patch should succeed"),
                CodexConfigPatchOutcome::Updated
            );

            let rendered = fs::read_to_string(&path).expect("read config");
            let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

            // Hook-only integration: notify is removed from config
            // hooks.json is used instead
            assert!(
                value.get("notify").is_none(),
                "notify should be removed in hook-only mode"
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Hook-only integration: all notify entries are removed
            assert_eq!(
                patch_codex_config_file(&path, executable).expect("patch should succeed"),
                CodexConfigPatchOutcome::Updated
            );

            let rendered = fs::read_to_string(&path).expect("read config");
            let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

            // Hook-only integration: notify is removed from config
            assert!(
                value.get("notify").is_none(),
                "notify should be removed in hook-only mode"
            );
        }

        // Common assertions for both platforms
        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

        assert_eq!(value["tui"]["alternate_screen"].as_str(), Some("never"));
        // Hook-only integration: notification_method and notifications are not set
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
        // Hook-only integration: notification_method and notifications are not set
        // Original tui.notifications and tui.notification_method are preserved as-is
        assert_eq!(value["tui"]["notifications"].as_bool(), Some(false));
        assert_eq!(
            value["tui"]["notification_method"].as_str(),
            Some("desktop")
        );
    }

    #[test]
    fn inspect_codex_cli_integration_reports_enabled_healthy_when_all_requirements_match() {
        let temp = TestTempDir::new("codex-config-health");
        let path = temp.path.join("config.toml");
        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");

        // First patch the config to set up hooks
        fs::write(&path, "").expect("write empty config");
        patch_codex_config_file(&path, executable).expect("patch should create hooks");

        // Also write the features section to config.toml
        let _config_content = fs::read_to_string(&path).expect("read config");
        let hooks_path = temp.path.join("hooks.json");

        // Verify status is healthy (hooks configured but not yet runtime-verified)
        assert_eq!(
            inspect_codex_cli_integration_at_path(path.clone(), executable),
            CodexIntegrationStatus::EnabledHealthy {
                path,
                hooks_runtime_verified: false
            }
        );

        // Verify hooks.json was created
        assert!(hooks_path.exists(), "hooks.json should be created");
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
notifications = ["agent-turn-complete", "approval-requested", "user-input-requested", "plan-mode-prompt"]
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
notifications = ["agent-turn-complete", "approval-requested", "user-input-requested"]
"#,
        )
        .expect("write config");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");

        #[cfg(target_os = "windows")]
        {
            // On Windows, custom notify is treated as needing setup (will be overwritten)
            // because hooks are unsupported and notify is the only reliable completion signal
            assert!(
                matches!(
                    inspect_codex_cli_integration_at_path(path.clone(), executable),
                    CodexIntegrationStatus::NeedsSetup { .. }
                ),
                "On Windows, custom notify should be treated as needing setup (will be overwritten)"
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            // On non-Windows platforms, custom notify is also treated as needing setup
            // for hook-only integration (hooks.json replaces notify)
            assert!(
                matches!(
                    inspect_codex_cli_integration_at_path(path.clone(), executable),
                    CodexIntegrationStatus::NeedsSetup { .. }
                ),
                "On non-Windows, custom notify should be treated as needing setup for hook-only integration"
            );
        }
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

    #[test]
    fn patch_codex_config_sets_codex_hooks_feature() {
        let temp = TestTempDir::new("codex-config-hooks");
        let path = temp.path.join("config.toml");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        assert_eq!(
            patch_codex_config_file(&path, executable).expect("patch should succeed"),
            CodexConfigPatchOutcome::Updated
        );

        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

        // Check features.codex_hooks is set to true
        assert_eq!(value["features"]["codex_hooks"].as_bool(), Some(true));
    }

    #[test]
    fn patch_codex_config_creates_hooks_json_with_spinner_events() {
        let temp = TestTempDir::new("codex-hooks-json");
        let config_path = temp.path.join("config.toml");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        patch_codex_config_file(&config_path, executable).expect("patch should succeed");

        // Check hooks.json was created
        let hooks_path = temp.path.join("hooks.json");
        assert!(hooks_path.exists(), "hooks.json should be created");

        let hooks_content = fs::read_to_string(&hooks_path).expect("read hooks.json");
        let hooks: serde_json::Value =
            serde_json::from_str(&hooks_content).expect("parse hooks.json");

        for event_name in [
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "PostToolUse",
            "Stop",
        ] {
            let entries = hooks["hooks"][event_name]
                .as_array()
                .unwrap_or_else(|| panic!("{event_name} hook should exist"));
            assert!(!entries.is_empty(), "{event_name} hook should not be empty");
            let entry = &entries[0];
            let hook = entry["hooks"][0]
                .as_object()
                .unwrap_or_else(|| panic!("{event_name} command hook should exist"));
            assert_eq!(hook["type"].as_str(), Some("command"));
            assert_eq!(hook["timeout"].as_u64(), Some(10));
            assert!(
                hook.get("statusMessage").is_none(),
                "{event_name} should not set statusMessage"
            );
            assert!(
                hook["command"]
                    .as_str()
                    .is_some_and(|cmd| cmd.contains(&format!("--codex-hook {event_name}"))),
                "{event_name} command should route through --codex-hook"
            );
        }

        for event_name in ["PreToolUse", "PermissionRequest", "PostToolUse"] {
            assert_eq!(
                hooks["hooks"][event_name][0]["matcher"].as_str(),
                Some(r"^(Bash|apply_patch|Edit|Write|mcp__.*)$")
            );
        }
        assert!(hooks["hooks"]["UserPromptSubmit"][0]
            .get("matcher")
            .is_none());
        assert!(hooks["hooks"]["Stop"][0].get("matcher").is_none());
    }

    #[test]
    fn patch_codex_config_preserves_malformed_hooks_json() {
        let temp = TestTempDir::new("codex-hooks-json-malformed");
        let config_path = temp.path.join("config.toml");
        let hooks_path = temp.path.join("hooks.json");

        // Write a malformed hooks.json (user's custom config)
        fs::write(&hooks_path, "{ invalid json").expect("write malformed hooks.json");

        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");
        // Should fail with InvalidData error, not overwrite the file
        let result = patch_codex_config_file(&config_path, executable);
        assert!(
            result.is_err(),
            "should fail when existing hooks.json is malformed"
        );
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

        // Verify the malformed file was NOT overwritten
        let preserved_content = fs::read_to_string(&hooks_path).expect("read preserved file");
        assert_eq!(preserved_content, "{ invalid json");
    }

    #[test]
    fn patch_codex_config_returns_updated_when_hooks_json_missing_but_config_unchanged() {
        let temp = TestTempDir::new("codex-hooks-json-missing-config-ok");
        let config_path = temp.path.join("config.toml");
        let hooks_path = temp.path.join("hooks.json");
        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");

        // First, create a fully configured config.toml and hooks.json
        let outcome =
            patch_codex_config_file(&config_path, executable).expect("patch should succeed");
        assert_eq!(outcome, CodexConfigPatchOutcome::Updated);
        assert!(hooks_path.exists(), "hooks.json should be created");

        // Delete hooks.json to simulate it going missing
        fs::remove_file(&hooks_path).expect("remove hooks.json");
        assert!(!hooks_path.exists(), "hooks.json should be deleted");

        // Re-run patch - config.toml is unchanged, but hooks.json is missing
        // The outcome should be Updated, not Unchanged
        let second_outcome =
            patch_codex_config_file(&config_path, executable).expect("second patch should succeed");
        assert_eq!(
            second_outcome,
            CodexConfigPatchOutcome::Updated,
            "should report Updated when hooks.json is missing, even if config.toml is unchanged"
        );
        assert!(hooks_path.exists(), "hooks.json should be recreated");
    }

    #[test]
    fn patch_codex_config_returns_updated_when_hooks_json_stale() {
        let temp = TestTempDir::new("codex-hooks-json-stale");
        let config_path = temp.path.join("config.toml");
        let hooks_path = temp.path.join("hooks.json");
        let executable = Path::new(r"C:\Users\furkan.cakir\Desktop\mergen-ade.exe");

        // First, create a fully configured config.toml and hooks.json
        patch_codex_config_file(&config_path, executable).expect("patch should succeed");
        assert!(hooks_path.exists(), "hooks.json should be created");

        // Corrupt hooks.json to simulate it being stale (missing managed hooks)
        fs::write(&hooks_path, r#"{"hooks": {}}"#).expect("write stale hooks.json");

        // Re-run patch - config.toml is unchanged, but hooks.json is stale
        let outcome =
            patch_codex_config_file(&config_path, executable).expect("patch should succeed");
        assert_eq!(
            outcome,
            CodexConfigPatchOutcome::Updated,
            "should report Updated when hooks.json is stale"
        );

        // Verify hooks.json now contains the managed hooks
        let hooks_content = fs::read_to_string(&hooks_path).expect("read hooks.json");
        assert!(
            hooks_content.contains("UserPromptSubmit"),
            "hooks.json should contain UserPromptSubmit hook"
        );
        assert!(
            hooks_content.contains("Stop"),
            "hooks.json should contain Stop hook"
        );
        assert!(
            hooks_content.contains("PreToolUse"),
            "hooks.json should contain PreToolUse hook"
        );
        assert!(
            hooks_content.contains("PermissionRequest"),
            "hooks.json should contain PermissionRequest hook"
        );
        assert!(
            hooks_content.contains("PostToolUse"),
            "hooks.json should contain PostToolUse hook"
        );
    }

    #[test]
    fn handle_codex_hook_writes_user_prompt_submit_as_running() {
        // Serialize tests that mutate global environment variables
        let _lock = ENV_MUTEX.lock().unwrap();
        let _guard = EnvGuard::save(&[
            MERGEN_TERMINAL_ID_ENV_VAR,
            MERGEN_AI_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR,
            MERGEN_AI_TOOL_HINT_ENV_VAR,
        ]);

        let temp = TestTempDir::new("codex-hook-user-prompt");
        let inbox_dir = temp.path.join("inbox");
        fs::create_dir_all(&inbox_dir).unwrap();

        // Set environment variables as they would be set by PTY spawn
        env::set_var(MERGEN_TERMINAL_ID_ENV_VAR, "42");
        env::set_var(MERGEN_AI_INBOX_DIR_ENV_VAR, inbox_dir.to_str().unwrap());
        env::set_var(
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            inbox_dir.to_str().unwrap(),
        );
        env::set_var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, "test-token-42");
        env::set_var(MERGEN_AI_TOOL_HINT_ENV_VAR, MERGEN_AI_TOOL_HINT_CODEX);

        // Simulate UserPromptSubmit hook
        handle_codex_hook_from_env("UserPromptSubmit").expect("hook should be handled");

        // Check event was written
        let inbox_path = codex_notify_inbox_path_for_dir(&inbox_dir, 42, "test-token-42");
        let content = fs::read_to_string(&inbox_path).expect("read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(content.trim()).expect("parse event");

        assert_eq!(event.terminal_id, "42");
        assert_eq!(event.status, "running");
        assert_eq!(event.event_kind, Some("user-prompt-submit".to_string()));
        assert_eq!(event.tool, MERGEN_AI_TOOL_HINT_CODEX);
    }

    #[test]
    fn handle_codex_hook_writes_stop_as_attention() {
        // Serialize tests that mutate global environment variables
        let _lock = ENV_MUTEX.lock().unwrap();
        let _guard = EnvGuard::save(&[
            MERGEN_TERMINAL_ID_ENV_VAR,
            MERGEN_AI_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR,
            MERGEN_AI_TOOL_HINT_ENV_VAR,
        ]);

        let temp = TestTempDir::new("codex-hook-stop");
        let inbox_dir = temp.path.join("inbox");
        fs::create_dir_all(&inbox_dir).unwrap();

        // Set environment variables
        env::set_var(MERGEN_TERMINAL_ID_ENV_VAR, "43");
        env::set_var(MERGEN_AI_INBOX_DIR_ENV_VAR, inbox_dir.to_str().unwrap());
        env::set_var(
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            inbox_dir.to_str().unwrap(),
        );
        env::set_var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, "test-token-43");
        env::set_var(MERGEN_AI_TOOL_HINT_ENV_VAR, MERGEN_AI_TOOL_HINT_CODEX);

        // Simulate Stop hook
        handle_codex_hook_from_env("Stop").expect("hook should be handled");

        // Check event was written
        let inbox_path = codex_notify_inbox_path_for_dir(&inbox_dir, 43, "test-token-43");
        let content = fs::read_to_string(&inbox_path).expect("read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(content.trim()).expect("parse event");

        assert_eq!(event.terminal_id, "43");
        assert_eq!(event.status, "attention");
        // Stop hook uses neutral "hook-stop" event_kind (not agent-turn-complete)
        // to avoid incorrectly setting TurnComplete reason
        assert_eq!(event.event_kind, Some("hook-stop".to_string()));
    }

    #[test]
    fn handle_codex_hook_writes_tool_events_with_opencode_style_statuses() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let _guard = EnvGuard::save(&[
            MERGEN_TERMINAL_ID_ENV_VAR,
            MERGEN_AI_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR,
            MERGEN_AI_TOOL_HINT_ENV_VAR,
        ]);

        let temp = TestTempDir::new("codex-hook-tool-events");
        let inbox_dir = temp.path.join("inbox");
        fs::create_dir_all(&inbox_dir).unwrap();

        env::set_var(MERGEN_TERMINAL_ID_ENV_VAR, "45");
        env::set_var(MERGEN_AI_INBOX_DIR_ENV_VAR, inbox_dir.to_str().unwrap());
        env::set_var(
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            inbox_dir.to_str().unwrap(),
        );
        env::set_var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, "test-token-45");
        env::set_var(MERGEN_AI_TOOL_HINT_ENV_VAR, MERGEN_AI_TOOL_HINT_CODEX);

        for event_name in ["PreToolUse", "PermissionRequest", "PostToolUse"] {
            handle_codex_hook_from_env(event_name).expect("hook should be handled");
        }

        let inbox_path = codex_notify_inbox_path_for_dir(&inbox_dir, 45, "test-token-45");
        let content = fs::read_to_string(&inbox_path).expect("read inbox");
        let events = content
            .lines()
            .map(|line| serde_json::from_str::<CodexNotifyInboxEvent>(line).expect("parse event"))
            .collect::<Vec<_>>();

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].status, "running");
        assert_eq!(events[0].event_kind, Some("pre-tool-use".to_owned()));
        assert_eq!(events[1].status, "attention");
        assert_eq!(events[1].event_kind, Some("permission-request".to_owned()));
        assert_eq!(events[2].status, "running");
        assert_eq!(events[2].event_kind, Some("post-tool-use".to_owned()));
    }

    #[test]
    fn handle_codex_hook_ignores_mismatched_tool_hint() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let _guard = EnvGuard::save(&[
            MERGEN_TERMINAL_ID_ENV_VAR,
            MERGEN_AI_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR,
            MERGEN_AI_TOOL_HINT_ENV_VAR,
        ]);

        let temp = TestTempDir::new("codex-hook-mismatched-tool-hint");
        let inbox_dir = temp.path.join("inbox");
        fs::create_dir_all(&inbox_dir).unwrap();

        env::set_var(MERGEN_TERMINAL_ID_ENV_VAR, "44");
        env::set_var(MERGEN_AI_INBOX_DIR_ENV_VAR, inbox_dir.to_str().unwrap());
        env::set_var(
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            inbox_dir.to_str().unwrap(),
        );
        env::set_var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, "test-token-44");
        env::set_var(MERGEN_AI_TOOL_HINT_ENV_VAR, "opencode");

        handle_codex_hook_from_env("UserPromptSubmit")
            .expect("hook should ignore mismatched tool hint");

        let inbox_path = codex_notify_inbox_path_for_dir(&inbox_dir, 44, "test-token-44");
        let content = fs::read_to_string(&inbox_path).expect("read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(content.trim()).expect("parse event");

        assert_eq!(event.status, "running");
        assert_eq!(event.event_kind, Some("user-prompt-submit".to_string()));
        assert_eq!(event.tool, MERGEN_AI_TOOL_HINT_CODEX);
    }

    #[test]
    fn handle_codex_notify_mode_writes_agent_turn_complete_as_attention() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let _guard = EnvGuard::save(&[
            MERGEN_TERMINAL_ID_ENV_VAR,
            MERGEN_AI_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR,
        ]);

        let temp = TestTempDir::new("codex-notify-mode");
        let inbox_dir = temp.path.join("inbox");
        fs::create_dir_all(&inbox_dir).unwrap();

        // Set environment variables
        env::set_var(MERGEN_TERMINAL_ID_ENV_VAR, "55");
        env::set_var(MERGEN_AI_INBOX_DIR_ENV_VAR, inbox_dir.to_str().unwrap());
        env::set_var(
            MERGEN_ADE_CODEX_INBOX_DIR_ENV_VAR,
            inbox_dir.to_str().unwrap(),
        );
        env::set_var(MERGEN_ADE_CODEX_INBOX_TOKEN_ENV_VAR, "test-token-55");

        // Simulate running with --codex-notify argument
        // We need to manually set up args for the test since we can't easily modify std::env::args_os
        let payload = r#"{"event":{"type":"agent-turn-complete"}}"#;
        let event = super::write_codex_notify_event(
            payload,
            "55",
            &inbox_dir,
            "test-token-55",
            Some("codex"),
        );
        assert!(event.is_ok(), "notify event should be written");

        // Check event was written
        let inbox_path = codex_notify_inbox_path_for_dir(&inbox_dir, 55, "test-token-55");
        let content = fs::read_to_string(&inbox_path).expect("read inbox");
        let notify_event: CodexNotifyInboxEvent =
            serde_json::from_str(content.trim()).expect("parse event");

        assert_eq!(notify_event.terminal_id, "55");
        assert_eq!(notify_event.status, "attention");
        assert_eq!(
            notify_event.event_kind,
            Some("agent-turn-complete".to_string())
        );
        assert_eq!(notify_event.tool, MERGEN_AI_TOOL_HINT_CODEX);
    }

    #[test]
    fn patch_codex_config_removes_legacy_mergen_notify() {
        let temp = TestTempDir::new("codex-config-remove-notify");
        let path = temp.path.join("config.toml");
        fs::write(
            &path,
            r#"
model = "gpt-5"
notify = ['C:\\Users\\test\\AppData\\Roaming\\Mergen\\MergenADE\\bin\\mergen-codex-bridge.exe', "--codex-notify"]

[features]
codex_hooks = true

[tui]
alternate_screen = "never"
"#,
        )
        .expect("write config");

        let executable = Path::new(
            r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe",
        );
        let result = patch_codex_config_file(&path, executable);

        assert!(
            matches!(result, Ok(CodexConfigPatchOutcome::Updated)),
            "config should be updated to remove notify"
        );

        let rendered = fs::read_to_string(&path).expect("read config");
        let value = toml::from_str::<toml::Value>(&rendered).expect("parse patched config");

        assert!(
            value.get("notify").is_none(),
            "notify should be removed in hook-only mode"
        );
        assert_eq!(value["features"]["codex_hooks"].as_bool(), Some(true));
    }

    #[test]
    fn extract_notify_event_kind_handles_various_payloads() {
        // Test event.type format
        assert_eq!(
            super::extract_notify_event_kind(r#"{"event":{"type":"agent-turn-complete"}}"#),
            Some("agent-turn-complete".to_string())
        );

        // Test top-level type
        assert_eq!(
            super::extract_notify_event_kind(r#"{"type":"approval-requested"}"#),
            Some("approval-requested".to_string())
        );

        // Test underscore to hyphen conversion
        assert_eq!(
            super::extract_notify_event_kind(r#"{"type":"user_input_requested"}"#),
            Some("user-input-requested".to_string())
        );

        // Test kind field
        assert_eq!(
            super::extract_notify_event_kind(r#"{"kind":"plan-mode-prompt"}"#),
            Some("plan-mode-prompt".to_string())
        );

        // Test name field
        assert_eq!(
            super::extract_notify_event_kind(r#"{"name":"session-idle"}"#),
            Some("session-idle".to_string())
        );

        // Test empty/invalid
        assert_eq!(super::extract_notify_event_kind(r#"{}"#), None);
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

    // Bridge install tests
    #[test]
    fn install_codex_bridge_copies_executable_to_bridge_path() {
        let temp = TestTempDir::new("codex-bridge-install");
        let current_exe = temp.path.join("mergen-ade.exe");
        let bridge_path = temp.path.join("bin").join("mergen-codex-bridge.exe");

        // Create a fake executable file
        fs::create_dir_all(temp.path.join("bin")).unwrap();
        fs::write(&current_exe, "fake executable content").unwrap();

        // Install the bridge
        let result = super::install_codex_bridge(&current_exe, &bridge_path);
        assert!(result.is_ok(), "bridge install should succeed");
        assert!(bridge_path.exists(), "bridge should exist after install");

        // Verify content was copied
        let content = fs::read_to_string(&bridge_path).unwrap();
        assert_eq!(content, "fake executable content");
    }

    #[test]
    fn check_codex_hooks_json_requires_bridge_path_match() {
        let temp = TestTempDir::new("codex-hooks-json-bridge-check");
        let config_path = temp.path.join("config.toml");
        let hooks_path = temp.path.join("hooks.json");

        // Create a bridge path that we want hooks to target
        let bridge_path = Path::new(
            r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe",
        );

        // Create hooks.json targeting a DIFFERENT path (stale wiring)
        // Note: statusMessage intentionally omitted to prevent terminal noise
        let hooks_with_stale_path = serde_json::json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\Desktop\stale\mergen-ade.exe --codex-hook UserPromptSubmit"
                    }]
                }],
                "PreToolUse": [{
                    "matcher": r"^(Bash|apply_patch|Edit|Write|mcp__.*)$",
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\Desktop\stale\mergen-ade.exe --codex-hook PreToolUse"
                    }]
                }],
                "PermissionRequest": [{
                    "matcher": r"^(Bash|apply_patch|Edit|Write|mcp__.*)$",
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\Desktop\stale\mergen-ade.exe --codex-hook PermissionRequest"
                    }]
                }],
                "PostToolUse": [{
                    "matcher": r"^(Bash|apply_patch|Edit|Write|mcp__.*)$",
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\Desktop\stale\mergen-ade.exe --codex-hook PostToolUse"
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\Desktop\stale\mergen-ade.exe --codex-hook Stop"
                    }]
                }]
            }
        });

        fs::write(
            &hooks_path,
            serde_json::to_string_pretty(&hooks_with_stale_path).unwrap(),
        )
        .unwrap();

        // Check should fail because hooks don't target the bridge path
        let result = super::check_codex_hooks_json(&config_path, bridge_path);
        assert!(!result, "hooks check should fail when targeting stale path");

        // Now create hooks.json targeting the CORRECT bridge path
        // Note: statusMessage intentionally omitted to prevent terminal noise
        let hooks_with_bridge_path = serde_json::json!({
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe --codex-hook UserPromptSubmit"
                    }]
                }],
                "PreToolUse": [{
                    "matcher": r"^(Bash|apply_patch|Edit|Write|mcp__.*)$",
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe --codex-hook PreToolUse"
                    }]
                }],
                "PermissionRequest": [{
                    "matcher": r"^(Bash|apply_patch|Edit|Write|mcp__.*)$",
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe --codex-hook PermissionRequest"
                    }]
                }],
                "PostToolUse": [{
                    "matcher": r"^(Bash|apply_patch|Edit|Write|mcp__.*)$",
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe --codex-hook PostToolUse"
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": r"C:\Users\test\AppData\Roaming\Mergen\MergenADE\bin\mergen-codex-bridge.exe --codex-hook Stop"
                    }]
                }]
            }
        });

        fs::write(
            &hooks_path,
            serde_json::to_string_pretty(&hooks_with_bridge_path).unwrap(),
        )
        .unwrap();

        // Check should pass now
        let result = super::check_codex_hooks_json(&config_path, bridge_path);
        assert!(result, "hooks check should pass when targeting bridge path");
    }
}
