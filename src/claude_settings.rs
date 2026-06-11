use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeSettingsRepairOutcome {
    Missing { path: PathBuf },
    Unchanged { path: PathBuf },
    Updated { path: PathBuf, backup_path: PathBuf },
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
    let path = user_claude_settings_path()?;
    repair_claude_settings_file(&path)
}

pub fn repair_claude_settings_file(path: &Path) -> io::Result<ClaudeSettingsRepairOutcome> {
    let raw_settings = match fs::read_to_string(path) {
        Ok(raw_settings) => raw_settings,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(ClaudeSettingsRepairOutcome::Missing {
                path: path.to_path_buf(),
            });
        }
        Err(err) => return Err(err),
    };

    let mut settings = serde_json::from_str::<JsonValue>(&raw_settings).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Claude settings JSON is invalid: {err}"),
        )
    })?;

    if !settings.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Claude settings root must be a JSON object",
        ));
    }

    if !repair_claude_settings_value(&mut settings) {
        return Ok(ClaudeSettingsRepairOutcome::Unchanged {
            path: path.to_path_buf(),
        });
    }

    let backup_path = next_backup_path(path);
    fs::copy(path, &backup_path)?;

    let rendered = serde_json::to_string_pretty(&settings)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    fs::write(path, rendered)?;

    Ok(ClaudeSettingsRepairOutcome::Updated {
        path: path.to_path_buf(),
        backup_path,
    })
}

pub(crate) fn repair_claude_settings_value(settings: &mut JsonValue) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_hooks(hooks: JsonValue) -> JsonValue {
        serde_json::json!({
            "apiKeyHelper": "C:/Users/example/.claude/mimo-key-helper.cmd",
            "env": {
                "ANTHROPIC_MODEL": "mimo-v2.5-pro"
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

        assert!(repair_claude_settings_value(&mut settings));
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

        assert!(repair_claude_settings_value(&mut settings));
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

        assert!(repair_claude_settings_value(&mut settings));
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

        assert!(repair_claude_settings_value(&mut settings));
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

        assert!(!repair_claude_settings_value(&mut settings));
        assert_eq!(settings, original);
    }
}
