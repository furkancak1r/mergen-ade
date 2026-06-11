use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde_json::{json, Value as JsonValue};

pub const MIMO_CLAUDE_MODEL: &str = "mimo-v2.5-pro";
pub const MIMO_ANTHROPIC_BASE_URL: &str = "https://token-plan-sgp.xiaomimimo.com/anthropic";
pub const MIMO_KEY_HELPER_FILE_NAME: &str = "mimo-key-helper.cmd";
pub const MIMO_CLAUDE_ENV: [(&str, &str); 9] = [
    ("ANTHROPIC_BASE_URL", MIMO_ANTHROPIC_BASE_URL),
    ("ANTHROPIC_MODEL", MIMO_CLAUDE_MODEL),
    ("ANTHROPIC_SMALL_FAST_MODEL", MIMO_CLAUDE_MODEL),
    ("ANTHROPIC_DEFAULT_SONNET_MODEL", MIMO_CLAUDE_MODEL),
    ("ANTHROPIC_DEFAULT_HAIKU_MODEL", MIMO_CLAUDE_MODEL),
    ("ANTHROPIC_DEFAULT_OPUS_MODEL", MIMO_CLAUDE_MODEL),
    ("CLAUDE_CODE_SUBAGENT_MODEL", MIMO_CLAUDE_MODEL),
    ("DISABLE_AUTOUPDATER", "1"),
    ("DISABLE_UPDATES", "1"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeSettingsRepairOutcome {
    Missing {
        path: PathBuf,
    },
    Unchanged {
        path: PathBuf,
    },
    Updated {
        path: PathBuf,
        backup_path: Option<PathBuf>,
        helper_path: Option<PathBuf>,
    },
}

pub fn user_claude_settings_path() -> io::Result<PathBuf> {
    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User home directory is unavailable",
        )
    })?;
    Ok(base_dirs.home_dir().join(".claude").join("settings.json"))
}

pub fn repair_user_claude_settings() -> io::Result<ClaudeSettingsRepairOutcome> {
    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User home directory is unavailable",
        )
    })?;
    let home_dir = base_dirs.home_dir();
    let helper_path = mimo_key_helper_path_for_home(home_dir);
    let helper_updated = ensure_mimo_key_helper(&helper_path)?;
    let path = home_dir.join(".claude").join("settings.json");
    let outcome = repair_claude_settings_file_with_helper(&path, &helper_path)?;

    Ok(merge_helper_update_outcome(
        outcome,
        helper_updated,
        helper_path,
    ))
}

pub fn project_claude_settings_path(project_path: &Path) -> PathBuf {
    project_path.join(".claude").join("settings.local.json")
}

pub fn repair_project_claude_settings(
    project_path: &Path,
) -> io::Result<ClaudeSettingsRepairOutcome> {
    let path = project_claude_settings_path(project_path);
    if !path.try_exists()? {
        return Ok(ClaudeSettingsRepairOutcome::Missing { path });
    }

    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User home directory is unavailable",
        )
    })?;
    let helper_path = mimo_key_helper_path_for_home(base_dirs.home_dir());
    let helper_updated = ensure_mimo_key_helper(&helper_path)?;
    let outcome = repair_project_claude_settings_with_helper(project_path, &helper_path)?;

    Ok(merge_helper_update_outcome(
        outcome,
        helper_updated,
        helper_path,
    ))
}

pub fn repair_claude_settings_file(path: &Path) -> io::Result<ClaudeSettingsRepairOutcome> {
    let helper_path = path
        .parent()
        .map(|claude_dir| claude_dir.join("bin").join(MIMO_KEY_HELPER_FILE_NAME))
        .unwrap_or_else(|| PathBuf::from(MIMO_KEY_HELPER_FILE_NAME));
    repair_claude_settings_file_with_helper(path, &helper_path)
}

fn repair_project_claude_settings_with_helper(
    project_path: &Path,
    helper_path: &Path,
) -> io::Result<ClaudeSettingsRepairOutcome> {
    let path = project_claude_settings_path(project_path);
    if !path.try_exists()? {
        return Ok(ClaudeSettingsRepairOutcome::Missing { path });
    }
    repair_claude_settings_file_with_helper(&path, helper_path)
}

fn merge_helper_update_outcome(
    outcome: ClaudeSettingsRepairOutcome,
    helper_updated: bool,
    helper_path: PathBuf,
) -> ClaudeSettingsRepairOutcome {
    match (outcome, helper_updated) {
        (
            ClaudeSettingsRepairOutcome::Unchanged { path }
            | ClaudeSettingsRepairOutcome::Missing { path },
            true,
        ) => ClaudeSettingsRepairOutcome::Updated {
            path,
            backup_path: None,
            helper_path: Some(helper_path),
        },
        (
            ClaudeSettingsRepairOutcome::Updated {
                path, backup_path, ..
            },
            true,
        ) => ClaudeSettingsRepairOutcome::Updated {
            path,
            backup_path,
            helper_path: Some(helper_path),
        },
        (other, false) => other,
    }
}

fn repair_claude_settings_file_with_helper(
    path: &Path,
    helper_path: &Path,
) -> io::Result<ClaudeSettingsRepairOutcome> {
    let raw_settings = match fs::read_to_string(path) {
        Ok(raw_settings) => raw_settings,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let mut settings = if raw_settings.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str::<JsonValue>(&raw_settings).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Claude settings JSON is invalid: {err}"),
            )
        })?
    };

    if !settings.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Claude settings root must be a JSON object",
        ));
    }

    if !repair_claude_settings_value(&mut settings, helper_path) {
        return Ok(ClaudeSettingsRepairOutcome::Unchanged {
            path: path.to_path_buf(),
        });
    }

    let backup_path = if raw_settings.trim().is_empty() {
        None
    } else {
        let backup_path = next_backup_path(path);
        fs::copy(path, &backup_path)?;
        Some(backup_path)
    };

    let rendered = serde_json::to_string_pretty(&settings)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, rendered)?;

    Ok(ClaudeSettingsRepairOutcome::Updated {
        path: path.to_path_buf(),
        backup_path,
        helper_path: None,
    })
}

pub(crate) fn repair_claude_settings_value(settings: &mut JsonValue, helper_path: &Path) -> bool {
    if !settings.is_object() {
        return false;
    }
    let mut changed = apply_mimo_claude_config(settings, helper_path);
    changed |= repair_claude_hooks(settings);
    changed
}

fn repair_claude_hooks(settings: &mut JsonValue) -> bool {
    let Some(root) = settings.as_object_mut() else {
        return false;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(JsonValue::as_object_mut) else {
        return false;
    };
    let event_names = hooks.keys().cloned().collect::<Vec<_>>();
    let mut changed = false;

    for event_name in event_names {
        let Some(event_value) = hooks.remove(&event_name) else {
            continue;
        };
        let (event_was_array, groups) = match event_value {
            JsonValue::Array(groups) => (true, groups),
            object @ JsonValue::Object(_) => {
                changed = true;
                (false, vec![object])
            }
            other => {
                hooks.insert(event_name, other);
                continue;
            }
        };

        let original_group_count = groups.len();
        let mut repaired_groups = Vec::new();

        for group in groups {
            let Some(mut group_object) = group.as_object().cloned() else {
                repaired_groups.push(group);
                continue;
            };

            let Some(hooks_value) = group_object.remove("hooks") else {
                repaired_groups.push(JsonValue::Object(group_object));
                continue;
            };

            let (hook_handlers, normalized_hooks_shape) = match hooks_value {
                JsonValue::Array(handlers) => (handlers, false),
                object @ JsonValue::Object(_) => (vec![object], true),
                other => {
                    group_object.insert("hooks".to_owned(), other);
                    repaired_groups.push(JsonValue::Object(group_object));
                    continue;
                }
            };

            changed |= normalized_hooks_shape;
            let original_hook_count = hook_handlers.len();
            let retained_hooks = hook_handlers
                .into_iter()
                .filter(|hook| !is_stale_claude_hook(hook))
                .collect::<Vec<_>>();

            if retained_hooks.len() != original_hook_count {
                changed = true;
            }

            if retained_hooks.is_empty() {
                changed = true;
                continue;
            }

            group_object.insert("hooks".to_owned(), JsonValue::Array(retained_hooks));
            repaired_groups.push(JsonValue::Object(group_object));
        }

        if repaired_groups.is_empty() {
            changed = true;
            continue;
        }

        if !event_was_array || repaired_groups.len() != original_group_count {
            changed = true;
        }
        hooks.insert(event_name, JsonValue::Array(repaired_groups));
    }

    changed
}

fn apply_mimo_claude_config(settings: &mut JsonValue, helper_path: &Path) -> bool {
    let root = settings
        .as_object_mut()
        .expect("caller verified Claude settings root is an object");
    let mut changed = false;

    changed |= set_json_string(root, "model", MIMO_CLAUDE_MODEL);
    changed |= set_json_string(
        root,
        "apiKeyHelper",
        &helper_path.to_string_lossy().replace('\\', "/"),
    );

    if !root.get("env").is_some_and(JsonValue::is_object) {
        root.insert("env".to_owned(), json!({}));
        changed = true;
    }
    let env = root
        .get_mut("env")
        .and_then(JsonValue::as_object_mut)
        .expect("env value must be an object");

    for (key, value) in MIMO_CLAUDE_ENV {
        changed |= set_json_string(env, key, value);
    }

    changed
}

fn set_json_string(
    object: &mut serde_json::Map<String, JsonValue>,
    key: &str,
    value: &str,
) -> bool {
    let changed = object.get(key).and_then(JsonValue::as_str) != Some(value);
    if changed {
        object.insert(key.to_owned(), JsonValue::String(value.to_owned()));
    }
    changed
}

fn is_stale_claude_hook(hook: &JsonValue) -> bool {
    let Some(command) = hook.get("command").and_then(JsonValue::as_str) else {
        return false;
    };
    is_stale_claude_hook_command(command)
}

fn is_stale_claude_hook_command(command: &str) -> bool {
    let normalized = command.replace('\\', "/").to_ascii_lowercase();
    normalized.contains(".claude/hooks/on-working.ps1")
        || normalized.contains(".claude/hooks/on-stop.ps1")
        || normalized.contains("orca")
        || normalized.contains("emdash")
}

fn next_backup_path(path: &Path) -> PathBuf {
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base = path.with_file_name(format!(
        "{}.mergen-bak-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("settings.json"),
        timestamp
    ));

    if !base.exists() {
        return base;
    }

    for suffix in 2.. {
        let candidate = path.with_file_name(format!(
            "{}.mergen-bak-{}-{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("settings.json"),
            timestamp,
            suffix
        ));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("unbounded suffix search must return an available backup path")
}

pub fn mimo_key_helper_path_for_home(home_dir: &Path) -> PathBuf {
    home_dir
        .join(".claude")
        .join("bin")
        .join(MIMO_KEY_HELPER_FILE_NAME)
}

pub fn ensure_mimo_key_helper(helper_path: &Path) -> io::Result<bool> {
    let contents = mimo_key_helper_contents();
    let current = fs::read_to_string(helper_path).ok();
    if current.as_deref() == Some(contents) {
        return Ok(false);
    }
    if let Some(parent) = helper_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(helper_path, contents)?;
    Ok(true)
}

fn mimo_key_helper_contents() -> &'static str {
    r#"@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$key = $env:MIMO_API_KEY; if ([string]::IsNullOrWhiteSpace($key)) { $key = [Environment]::GetEnvironmentVariable('MIMO_API_KEY','User') }; if ([string]::IsNullOrWhiteSpace($key)) { $desktop = [Environment]::GetFolderPath('Desktop'); $paths = @((Join-Path $desktop 'key.txt'), (Join-Path $env:USERPROFILE 'Desktop\key.txt')) | Select-Object -Unique; foreach ($path in $paths) { if (Test-Path -LiteralPath $path) { foreach ($line in Get-Content -LiteralPath $path) { $candidate = $line.Trim(); if ([string]::IsNullOrWhiteSpace($candidate)) { continue }; if ($candidate -match '^https?://') { continue }; if ($candidate -match '(?i)protocol\s*:?\s*$') { continue }; if ($candidate -ieq 'mimo-v2.5-pro') { continue }; $key = $candidate; break }; if (-not [string]::IsNullOrWhiteSpace($key)) { break } } } }; if ([string]::IsNullOrWhiteSpace($key)) { [Console]::Error.WriteLine('MIMO_API_KEY not found'); exit 1 }; [Console]::Out.WriteLine($key)"
exit /b %ERRORLEVEL%
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helper_path() -> PathBuf {
        PathBuf::from("C:/Users/example/.claude/bin/mimo-key-helper.cmd")
    }

    fn settings_with_hooks(hooks: JsonValue) -> JsonValue {
        serde_json::json!({
            "apiKeyHelper": "C:/Users/example/.claude/bin/mimo-key-helper.cmd",
            "model": "mimo-v2.5-pro",
            "env": {
                "ANTHROPIC_BASE_URL": "https://token-plan-sgp.xiaomimimo.com/anthropic",
                "ANTHROPIC_MODEL": "mimo-v2.5-pro",
                "ANTHROPIC_SMALL_FAST_MODEL": "mimo-v2.5-pro",
                "ANTHROPIC_DEFAULT_SONNET_MODEL": "mimo-v2.5-pro",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL": "mimo-v2.5-pro",
                "ANTHROPIC_DEFAULT_OPUS_MODEL": "mimo-v2.5-pro",
                "CLAUDE_CODE_SUBAGENT_MODEL": "mimo-v2.5-pro",
                "DISABLE_AUTOUPDATER": "1",
                "DISABLE_UPDATES": "1"
            },
            "hooks": hooks
        })
    }

    #[test]
    fn repair_wraps_object_shaped_hook_event() {
        let mut settings = settings_with_hooks(serde_json::json!({
            "Stop": {
                "hooks": [{
                    "type": "command",
                    "command": "powershell -NoProfile -File C:/valid/notify.ps1"
                }]
            }
        }));

        assert!(repair_claude_settings_value(&mut settings, &helper_path()));
        assert!(settings["hooks"]["Stop"].is_array());
        assert_eq!(
            settings["hooks"]["Stop"][0]["hooks"][0]["command"],
            "powershell -NoProfile -File C:/valid/notify.ps1"
        );
        assert_eq!(settings["env"]["ANTHROPIC_MODEL"], "mimo-v2.5-pro");
    }

    #[test]
    fn repair_removes_stale_on_stop_and_on_working_hooks() {
        let mut settings = settings_with_hooks(serde_json::json!({
            "Stop": {
                "hooks": [{
                    "type": "command",
                    "command": "powershell -NoProfile -File C:/Users/me/.claude/hooks/on-stop.ps1"
                }]
            },
            "SubagentStart": {
                "hooks": [{
                    "type": "command",
                    "command": "powershell -NoProfile -File C:\\Users\\me\\.claude\\hooks\\on-working.ps1"
                }]
            }
        }));

        assert!(repair_claude_settings_value(&mut settings, &helper_path()));
        assert!(settings["hooks"].get("Stop").is_none());
        assert!(settings["hooks"].get("SubagentStart").is_none());
    }

    #[test]
    fn repair_preserves_valid_hooks_when_mixed_with_stale_hooks() {
        let mut settings = settings_with_hooks(serde_json::json!({
            "TaskCompleted": [{
                "hooks": [
                    {
                        "type": "command",
                        "command": "powershell -NoProfile -File C:/Users/me/.claude/hooks/on-stop.ps1"
                    },
                    {
                        "type": "command",
                        "command": "powershell -NoProfile -File C:/valid/task-complete.ps1"
                    }
                ]
            }]
        }));

        assert!(repair_claude_settings_value(&mut settings, &helper_path()));
        let hooks = settings["hooks"]["TaskCompleted"][0]["hooks"]
            .as_array()
            .expect("hook handlers");
        assert_eq!(hooks.len(), 1);
        assert_eq!(
            hooks[0]["command"],
            "powershell -NoProfile -File C:/valid/task-complete.ps1"
        );
    }

    #[test]
    fn repair_removes_empty_hook_events_after_cleanup() {
        let mut settings = settings_with_hooks(serde_json::json!({
            "Stop": [{
                "hooks": []
            }]
        }));

        assert!(repair_claude_settings_value(&mut settings, &helper_path()));
        assert!(settings["hooks"].get("Stop").is_none());
    }

    #[test]
    fn repair_leaves_already_valid_settings_unchanged() {
        let mut settings = settings_with_hooks(serde_json::json!({
            "PostToolUse": [{
                "matcher": "Edit|Write",
                "hooks": [{
                    "type": "command",
                    "command": "npm test"
                }]
            }]
        }));
        let original = settings.clone();

        assert!(!repair_claude_settings_value(&mut settings, &helper_path()));
        assert_eq!(settings, original);
    }

    #[test]
    fn repair_updates_kimi_settings_to_mimo() {
        let mut settings = serde_json::json!({
            "apiKeyHelper": "C:/Users/example/.claude/bin/kimi-key-helper.cmd",
            "model": "kimi-k2",
            "env": {
                "ANTHROPIC_BASE_URL": "https://api.kimi.example/anthropic",
                "ANTHROPIC_MODEL": "kimi-k2",
                "CUSTOM_ENV": "preserved"
            }
        });

        assert!(repair_claude_settings_value(&mut settings, &helper_path()));
        assert_eq!(
            settings["apiKeyHelper"],
            "C:/Users/example/.claude/bin/mimo-key-helper.cmd"
        );
        assert_eq!(settings["model"], "mimo-v2.5-pro");
        assert_eq!(
            settings["env"]["ANTHROPIC_BASE_URL"],
            "https://token-plan-sgp.xiaomimimo.com/anthropic"
        );
        assert_eq!(settings["env"]["ANTHROPIC_MODEL"], "mimo-v2.5-pro");
        assert_eq!(settings["env"]["CUSTOM_ENV"], "preserved");
    }

    fn unique_temp_project(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mergen-ade-claude-settings-{name}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn repair_project_settings_missing_does_not_create_file() {
        let project_path = unique_temp_project("missing");
        let settings_path = project_claude_settings_path(&project_path);

        let outcome =
            repair_project_claude_settings_with_helper(&project_path, &helper_path()).unwrap();

        assert_eq!(
            outcome,
            ClaudeSettingsRepairOutcome::Missing {
                path: settings_path.clone()
            }
        );
        assert!(!settings_path.exists());
        assert!(!project_path.join(".claude").exists());
    }

    #[test]
    fn repair_project_settings_updates_local_kimi_override_to_mimo() {
        let project_path = unique_temp_project("local-kimi");
        let settings_path = project_claude_settings_path(&project_path);
        fs::create_dir_all(settings_path.parent().expect("settings parent")).unwrap();
        fs::write(
            &settings_path,
            r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "apiKeyHelper": "C:/Users/example/.claude/bin/fireworks-firepass-key-helper.cmd",
  "env": {
    "ANTHROPIC_BASE_URL": "https://api.fireworks.ai/inference",
    "ANTHROPIC_MODEL": "accounts/fireworks/routers/kimi-k2p6-turbo",
    "ANTHROPIC_SMALL_FAST_MODEL": "accounts/fireworks/routers/kimi-k2p6-turbo",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "accounts/fireworks/routers/kimi-k2p6-turbo",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "accounts/fireworks/routers/kimi-k2p6-turbo",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "accounts/fireworks/routers/kimi-k2p6-turbo",
    "CLAUDE_CODE_SUBAGENT_MODEL": "accounts/fireworks/routers/kimi-k2p6-turbo",
    "DISABLE_AUTOUPDATER": "1"
  },
  "permissions": {
    "allow": [
      "Bash(Get-ChildItem -Path \"src\" -Filter \"*.rs\")"
    ]
  },
  "model": "accounts/fireworks/routers/kimi-k2p6-turbo",
  "hooks": {
    "Notification": [{
      "hooks": [{
        "type": "command",
        "command": "cmd.exe /d /c \"echo EMDASH_HOOK_PORT >NUL\""
      }]
    }]
  }
}"#,
        )
        .unwrap();

        let outcome =
            repair_project_claude_settings_with_helper(&project_path, &helper_path()).unwrap();

        let ClaudeSettingsRepairOutcome::Updated {
            backup_path: Some(backup_path),
            ..
        } = outcome
        else {
            panic!("expected project-local settings to be updated");
        };
        assert!(backup_path.exists());

        let repaired =
            serde_json::from_str::<JsonValue>(&fs::read_to_string(&settings_path).unwrap())
                .unwrap();
        assert_eq!(
            repaired["apiKeyHelper"],
            "C:/Users/example/.claude/bin/mimo-key-helper.cmd"
        );
        assert_eq!(repaired["model"], "mimo-v2.5-pro");
        assert_eq!(
            repaired["env"]["ANTHROPIC_BASE_URL"],
            "https://token-plan-sgp.xiaomimimo.com/anthropic"
        );
        assert_eq!(repaired["env"]["ANTHROPIC_MODEL"], "mimo-v2.5-pro");
        assert_eq!(repaired["env"]["DISABLE_UPDATES"], "1");
        assert_eq!(
            repaired["permissions"]["allow"][0],
            "Bash(Get-ChildItem -Path \"src\" -Filter \"*.rs\")"
        );
        assert!(repaired["hooks"].get("Notification").is_none());

        let _ = fs::remove_dir_all(&project_path);
    }
}
