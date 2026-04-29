use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use serde::Deserialize;

use crate::models::{
    default_launchers, normalize_launcher_entries, AppConfig, AppHistory, ProjectRecord, ShellKind,
    UiConfig,
};

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "Mergen";
const APPLICATION: &str = "MergenADE";
const FACTORY_DROID_HOOK_RUNTIME_DIR: &str = "runtime/factory-droid-hooks";
const CODEX_CLI_RUNTIME_DIR: &str = "runtime/codex-cli";
const OPENCODE_RUNTIME_DIR: &str = "runtime/opencode";
const CODEX_BRIDGE_DIR: &str = "bin";
const CODEX_BRIDGE_EXE: &str = "mergen-codex-bridge.exe";

fn project_dirs() -> io::Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "App data directory not available"))
}

pub fn config_path() -> io::Result<PathBuf> {
    let config_dir = project_dirs()?.config_dir().to_path_buf();
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("config.toml"))
}

pub fn factory_droid_hook_runtime_dir() -> io::Result<PathBuf> {
    let runtime_dir = project_dirs()?
        .config_dir()
        .join(FACTORY_DROID_HOOK_RUNTIME_DIR);
    fs::create_dir_all(&runtime_dir)?;
    Ok(runtime_dir)
}

pub fn codex_cli_runtime_dir() -> io::Result<PathBuf> {
    let runtime_dir = project_dirs()?.config_dir().join(CODEX_CLI_RUNTIME_DIR);
    fs::create_dir_all(&runtime_dir)?;
    Ok(runtime_dir)
}

pub fn opencode_cli_runtime_dir() -> io::Result<PathBuf> {
    let runtime_dir = project_dirs()?.config_dir().join(OPENCODE_RUNTIME_DIR);
    fs::create_dir_all(&runtime_dir)?;
    Ok(runtime_dir)
}

/// Path to the terminal input history file (JSON).
pub fn history_path() -> io::Result<PathBuf> {
    let data_dir = project_dirs()?.data_dir().to_path_buf();
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join("history.json"))
}

pub fn load_history(path: &Path) -> io::Result<AppHistory> {
    if !path.exists() {
        return Ok(AppHistory::default());
    }
    let text = fs::read_to_string(path)?;
    let mut history: AppHistory = serde_json::from_str(&text)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    // Migrate legacy data: normalize max_entries == 0 to default (500)
    for project_history in history.projects.values_mut() {
        if project_history.max_entries == 0 {
            project_history.max_entries = 500;
        }
    }
    Ok(history)
}

pub fn save_history(path: &Path, history: &AppHistory) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = temp_save_path(path);
    let data = serde_json::to_string_pretty(history)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    fs::write(&tmp_path, data)?;
    atomic_replace_file(&tmp_path, path)?;
    Ok(())
}

/// Returns the fixed bridge path used for Codex CLI hooks.
/// This is `%APPDATA%\Mergen\MergenADE\bin\mergen-codex-bridge.exe`
/// The bridge is a copy of the main executable that serves as a stable
/// target for ~/.codex/config.toml and ~/.codex/hooks.json, independent
/// of where the actual Mergen binary is installed.
pub fn codex_bridge_path() -> io::Result<PathBuf> {
    let bridge_dir = project_dirs()?.data_dir().join(CODEX_BRIDGE_DIR);
    fs::create_dir_all(&bridge_dir)?;
    Ok(bridge_dir.join(CODEX_BRIDGE_EXE))
}

pub fn load_config(path: &Path) -> io::Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let text = fs::read_to_string(path)?;
    let mut config = if let Ok(parsed) = toml::from_str::<AppConfig>(&text) {
        parsed
    } else {
        let legacy = toml::from_str::<LegacyAppConfig>(&text)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        legacy.into()
    };
    normalize_config_for_current_platform(&mut config);
    Ok(config)
}

pub fn save_config(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = temp_save_path(path);
    let data = toml::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;

    fs::write(&tmp_path, data)?;

    atomic_replace_file(&tmp_path, path)?;
    Ok(())
}

fn temp_save_path(path: &Path) -> PathBuf {
    let stem = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config.toml");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!("{stem}.tmp-{}-{unique}", std::process::id());
    path.with_file_name(tmp_name)
}

fn atomic_replace_file(src: &Path, dst: &Path) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt as _;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };

        let mut src_wide: Vec<u16> = src.as_os_str().encode_wide().collect();
        src_wide.push(0);
        let mut dst_wide: Vec<u16> = dst.as_os_str().encode_wide().collect();
        dst_wide.push(0);

        unsafe {
            if MoveFileExW(
                src_wide.as_ptr(),
                dst_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            ) != 0
            {
                return Ok(());
            }
        }

        let err = io::Error::last_os_error();
        let _ = fs::remove_file(src);
        Err(err)
    }

    #[cfg(not(target_os = "windows"))]
    {
        match fs::rename(src, dst) {
            Ok(()) => Ok(()),
            Err(err) => {
                let _ = fs::remove_file(src);
                Err(err)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyProjectRecord {
    id: u64,
    name: String,
    path: PathBuf,
    shell_override: Option<ShellKind>,
    #[serde(default)]
    saved_messages: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyAppConfig {
    #[serde(default = "default_config_version")]
    version: u32,
    #[serde(default)]
    default_shell: ShellKind,
    #[serde(default)]
    ui: UiConfig,
    #[serde(default)]
    projects: Vec<LegacyProjectRecord>,
}

impl From<LegacyAppConfig> for AppConfig {
    fn from(value: LegacyAppConfig) -> Self {
        let projects = value
            .projects
            .into_iter()
            .map(|project| {
                let _ = project.shell_override;
                ProjectRecord {
                    id: project.id,
                    name: project.name,
                    path: project.path,
                    saved_messages: project.saved_messages,
                    ai_config: crate::hooks::ProjectAiConfig::default(),
                    checklist: Vec::new(),
                }
            })
            .collect();

        AppConfig {
            version: value.version,
            default_shell: value.default_shell,
            ui: value.ui,
            launchers: default_launchers(),
            projects,
            ai_hooks: crate::hooks::AiHooksConfig::default(),
            opencode: crate::models::OpenCodeModelConfig::default(),
        }
    }
}

const fn default_config_version() -> u32 {
    1
}

fn normalize_config_for_current_platform(config: &mut AppConfig) {
    config.default_shell = config.default_shell.normalize_for_current_platform();
    normalize_launcher_entries(&mut config.launchers);
}

#[cfg(test)]
mod tests {
    use super::{load_config, load_history, save_config, save_history};
    use crate::models::{AppConfig, BuiltinLauncherKind, ShellKind, TerminalManagerFilter};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_project_without_saved_messages_field() {
        let path = unique_temp_path("missing-saved-messages");
        fs::write(
            &path,
            r#"
version = 1
default_shell = "powershell"

[[projects]]
id = 7
name = "Demo"
path = "C:/work/demo"
"#,
        )
        .expect("should write config");

        let config = load_config(&path).expect("should load config");

        assert_eq!(
            config.default_shell,
            ShellKind::PowerShell.normalize_for_current_platform()
        );
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].name, "Demo");
        assert!(config.projects[0].saved_messages.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn normalizes_default_shell_for_current_platform() {
        let mut config = AppConfig {
            default_shell: ShellKind::PowerShell,
            ..AppConfig::default()
        };

        super::normalize_config_for_current_platform(&mut config);

        #[cfg(target_os = "windows")]
        assert_eq!(config.default_shell, ShellKind::PowerShell);

        #[cfg(not(target_os = "windows"))]
        assert_eq!(config.default_shell, ShellKind::Zsh);
    }

    #[test]
    fn missing_default_shell_uses_platform_default() {
        let path = unique_temp_path("missing-default-shell");
        fs::write(
            &path,
            r#"
version = 1

[[projects]]
id = 7
name = "Demo"
path = "C:/work/demo"
"#,
        )
        .expect("should write config");

        let config = load_config(&path).expect("should load config");

        assert_eq!(config.default_shell, ShellKind::default());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_multi_terminal_setting_defaults_to_single_terminal_view() {
        let path = unique_temp_path("missing-multi-terminal-setting");
        fs::write(
            &path,
            r#"
version = 1
default_shell = "powershell"

[ui]
show_project_explorer = true
project_explorer_expanded = true
show_terminal_manager = true
terminal_manager_expanded = true
"#,
        )
        .expect("should write config");

        let config = load_config(&path).expect("should load config");

        assert!(!config.ui.multi_terminal_view_enabled);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_terminal_manager_filter_defaults_to_foreground() {
        let path = unique_temp_path("missing-terminal-manager-filter");
        fs::write(
            &path,
            r#"
version = 1
default_shell = "powershell"

[ui]
show_project_explorer = true
project_explorer_expanded = true
show_terminal_manager = true
terminal_manager_expanded = true
multi_terminal_view_enabled = true
"#,
        )
        .expect("should write config");

        let config = load_config(&path).expect("should load config");

        assert_eq!(
            config.ui.terminal_manager_filter,
            TerminalManagerFilter::Foreground
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_and_load_preserves_multi_terminal_setting() {
        let path = unique_temp_path("preserve-multi-terminal-setting");
        let mut config = AppConfig::default();
        config.ui.multi_terminal_view_enabled = true;

        save_config(&path, &config).expect("should save config");

        let loaded = load_config(&path).expect("should load config");

        assert!(loaded.ui.multi_terminal_view_enabled);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_and_load_preserves_terminal_manager_filter() {
        let path = unique_temp_path("preserve-terminal-manager-filter");
        let mut config = AppConfig::default();
        config.ui.terminal_manager_filter = TerminalManagerFilter::Background;

        save_config(&path, &config).expect("should save config");

        let loaded = load_config(&path).expect("should load config");

        assert_eq!(
            loaded.ui.terminal_manager_filter,
            TerminalManagerFilter::Background
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_launchers_field_restores_default_builtins() {
        let path = unique_temp_path("missing-launchers-field");
        fs::write(
            &path,
            r#"
version = 1
default_shell = "powershell"
"#,
        )
        .expect("should write config");

        let config = load_config(&path).expect("should load config");

        assert_eq!(config.launchers.len(), 4);
        assert_eq!(
            config.launchers[0].builtin,
            Some(BuiltinLauncherKind::OpenCode)
        );
        assert_eq!(
            config.launchers[1].builtin,
            Some(BuiltinLauncherKind::Codex)
        );
        assert_eq!(
            config.launchers[2].builtin,
            Some(BuiltinLauncherKind::Droid)
        );
        assert_eq!(
            config.launchers[3].builtin,
            Some(BuiltinLauncherKind::Claude)
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_and_load_preserves_launcher_command_edits() {
        let path = unique_temp_path("preserve-launcher-command");
        let mut config = AppConfig::default();
        config
            .launchers
            .iter_mut()
            .find(|launcher| launcher.builtin == Some(BuiltinLauncherKind::Claude))
            .expect("claude launcher")
            .launch_command = "cc".to_owned();

        save_config(&path, &config).expect("should save config");

        let loaded = load_config(&path).expect("should load config");
        let claude = loaded
            .launchers
            .iter()
            .find(|launcher| launcher.builtin == Some(BuiltinLauncherKind::Claude))
            .expect("claude launcher");

        assert_eq!(claude.launch_command, "cc");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn save_and_load_preserves_project_checklist() {
        use crate::models::ProjectRecord;
        use std::path::PathBuf;

        let path = unique_temp_path("preserve-project-checklist");
        let mut config = AppConfig::default();
        config.projects.push(ProjectRecord {
            id: 1,
            name: "Test Project".to_owned(),
            path: PathBuf::from("C:/test"),
            saved_messages: vec!["msg1".to_owned()],
            ai_config: crate::hooks::ProjectAiConfig::default(),
            checklist: vec!["item1".to_owned(), "item2".to_owned()],
        });

        save_config(&path, &config).expect("should save config");

        let loaded = load_config(&path).expect("should load config");
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].checklist.len(), 2);
        assert!(loaded.projects[0].checklist.contains(&"item1".to_owned()));
        assert!(loaded.projects[0].checklist.contains(&"item2".to_owned()));

        let _ = fs::remove_file(path);
    }

    fn unique_temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("mergen-ade-{label}-{unique}.toml"))
    }

    #[test]
    fn save_and_load_history_roundtrip() {
        use crate::models::{AppHistory, TerminalInputHistory, TerminalInputRecord, TerminalKind};
        use std::path::PathBuf;

        let path = unique_temp_path("history-roundtrip").with_extension("json");
        let mut history = AppHistory::default();

        // Add some entries
        let project_history = TerminalInputHistory {
            max_entries: 500,
            entries: vec![
                TerminalInputRecord {
                    project_path: PathBuf::from("C:/test"),
                    project_name: "Test Project".to_owned(),
                    terminal_kind: TerminalKind::Foreground,
                    text: "cargo build".to_owned(),
                    recorded_at: 1234567890,
                },
                TerminalInputRecord {
                    project_path: PathBuf::from("C:/test"),
                    project_name: "Test Project".to_owned(),
                    terminal_kind: TerminalKind::Background,
                    text: "git status".to_owned(),
                    recorded_at: 1234567891,
                },
            ],
        };
        history
            .projects
            .insert("C:/test".to_owned(), project_history);

        // Save and reload
        save_history(&path, &history).expect("should save history");
        let loaded = load_history(&path).expect("should load history");

        // Verify
        assert_eq!(loaded.projects.len(), 1);
        let loaded_project = loaded
            .projects
            .get("C:/test")
            .expect("project should exist");
        assert_eq!(loaded_project.entries.len(), 2);
        assert_eq!(loaded_project.entries[0].text, "cargo build");
        assert_eq!(loaded_project.entries[1].text, "git status");
        assert_eq!(
            loaded_project.entries[0].terminal_kind,
            TerminalKind::Foreground
        );
        assert_eq!(
            loaded_project.entries[1].terminal_kind,
            TerminalKind::Background
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_history_migrates_zero_max_entries_to_default() {
        use crate::models::{AppHistory, TerminalInputHistory, TerminalInputRecord, TerminalKind};
        use std::path::PathBuf;

        let path = unique_temp_path("history-migration").with_extension("json");

        // Create history with max_entries = 0 (legacy bug state)
        let mut history = AppHistory::default();
        let project_history = TerminalInputHistory {
            max_entries: 0,
            entries: vec![
                TerminalInputRecord {
                    project_path: PathBuf::from("C:/test"),
                    project_name: "Test".to_owned(),
                    terminal_kind: TerminalKind::Foreground,
                    text: "cmd1".to_owned(),
                    recorded_at: 1000,
                },
                TerminalInputRecord {
                    project_path: PathBuf::from("C:/test"),
                    project_name: "Test".to_owned(),
                    terminal_kind: TerminalKind::Foreground,
                    text: "cmd2".to_owned(),
                    recorded_at: 1001,
                },
            ],
        };
        history
            .projects
            .insert("C:/test".to_owned(), project_history);

        // Save legacy state
        save_history(&path, &history).expect("should save history");

        // Load should migrate max_entries to 500
        let loaded = load_history(&path).expect("should load history");
        let loaded_project = loaded
            .projects
            .get("C:/test")
            .expect("project should exist");
        assert_eq!(
            loaded_project.max_entries, 500,
            "max_entries should be migrated from 0 to 500"
        );
        assert_eq!(
            loaded_project.entries.len(),
            2,
            "both entries should be preserved after migration"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn terminal_input_history_default_has_500_limit() {
        use crate::models::TerminalInputHistory;

        let history = TerminalInputHistory::default();
        assert_eq!(
            history.max_entries, 500,
            "default max_entries should be 500"
        );
        assert!(history.entries.is_empty());
    }
}
