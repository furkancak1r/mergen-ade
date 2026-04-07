use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::Deserialize;

use crate::models::{AppConfig, ProjectRecord, ShellKind, UiConfig};

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "Mergen";
const APPLICATION: &str = "MergenADE";
const FACTORY_DROID_HOOK_RUNTIME_DIR: &str = "runtime/factory-droid-hooks";

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

    let tmp_path = path.with_extension("toml.tmp");
    let data = toml::to_string_pretty(config)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;

    fs::write(&tmp_path, data)?;

    if path.exists() {
        fs::remove_file(path)?;
    }

    fs::rename(tmp_path, path)?;
    Ok(())
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
                }
            })
            .collect();

        AppConfig {
            version: value.version,
            default_shell: value.default_shell,
            ui: value.ui,
            projects,
            ai_hooks: crate::hooks::AiHooksConfig::default(),
        }
    }
}

const fn default_config_version() -> u32 {
    1
}

fn normalize_config_for_current_platform(config: &mut AppConfig) {
    config.default_shell = config.default_shell.normalize_for_current_platform();
}

#[cfg(test)]
mod tests {
    use super::{load_config, save_config};
    use crate::models::{AppConfig, ShellKind};
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
    fn save_and_load_preserves_multi_terminal_setting() {
        let path = unique_temp_path("preserve-multi-terminal-setting");
        let mut config = AppConfig::default();
        config.ui.multi_terminal_view_enabled = true;

        save_config(&path, &config).expect("should save config");

        let loaded = load_config(&path).expect("should load config");

        assert!(loaded.ui.multi_terminal_view_enabled);

        let _ = fs::remove_file(path);
    }

    fn unique_temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("mergen-ade-{label}-{unique}.toml"))
    }
}
