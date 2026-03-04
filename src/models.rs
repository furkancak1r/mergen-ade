use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    #[default]
    PowerShell,
    Cmd,
}

impl ShellKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Cmd => "CMD",
        }
    }

    pub fn command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::PowerShell => ("powershell.exe", &["-NoLogo"]),
            Self::Cmd => ("cmd.exe", &[]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    #[default]
    Foreground,
    Background,
}

impl TerminalKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Foreground => "Foreground",
            Self::Background => "Background",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutoTileScope {
    #[default]
    AllVisible,
    SelectedProjectOnly,
}

impl AutoTileScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AllVisible => "All visible terminals",
            Self::SelectedProjectOnly => "Selected project only",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub show_project_explorer: bool,
    pub last_selected_project_id: Option<u64>,
    pub project_filter_mode: bool,
    pub auto_tile_scope: AutoTileScope,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_project_explorer: true,
            last_selected_project_id: None,
            project_filter_mode: true,
            auto_tile_scope: AutoTileScope::AllVisible,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub shell_override: Option<ShellKind>,
    pub saved_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub default_shell: ShellKind,
    pub ui: UiConfig,
    pub projects: Vec<ProjectRecord>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            default_shell: ShellKind::PowerShell,
            ui: UiConfig::default(),
            projects: Vec::new(),
        }
    }
}
