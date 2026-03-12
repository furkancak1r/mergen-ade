use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    #[serde(alias = "powershell")]
    #[serde(alias = "PowerShell")]
    #[serde(alias = "powerShell")]
    PowerShell,
    Cmd,
    Zsh,
}

impl ShellKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Cmd => "CMD",
            Self::Zsh => "zsh",
        }
    }

    pub fn command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::PowerShell => ("powershell.exe", &["-NoLogo"]),
            Self::Cmd => ("cmd.exe", &[]),
            Self::Zsh => ("zsh", &["-l"]),
        }
    }

    pub const fn default_for_current_platform() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::PowerShell
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self::Zsh
        }
    }

    pub const fn supported_on_current_platform(self) -> bool {
        #[cfg(target_os = "windows")]
        {
            matches!(self, Self::PowerShell | Self::Cmd)
        }

        #[cfg(not(target_os = "windows"))]
        {
            matches!(self, Self::Zsh)
        }
    }

    pub const fn normalize_for_current_platform(self) -> Self {
        if self.supported_on_current_platform() {
            self
        } else {
            Self::default_for_current_platform()
        }
    }

    pub fn available_for_current_platform() -> &'static [Self] {
        #[cfg(target_os = "windows")]
        {
            &[Self::PowerShell, Self::Cmd]
        }

        #[cfg(not(target_os = "windows"))]
        {
            &[Self::Zsh]
        }
    }
}

impl Default for ShellKind {
    fn default() -> Self {
        Self::default_for_current_platform()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MainVisibilityMode {
    #[default]
    Global,
    SelectedProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LeftSidebarTab {
    #[default]
    Directory,
    SourceControl,
    TerminalManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub show_project_explorer: bool,
    pub project_explorer_expanded: bool,
    pub show_terminal_manager: bool,
    pub terminal_manager_expanded: bool,
    pub last_selected_project_id: Option<u64>,
    pub project_filter_mode: bool,
    pub auto_tile_scope: AutoTileScope,
    pub main_visibility_mode: MainVisibilityMode,
    pub left_sidebar_tab: LeftSidebarTab,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_project_explorer: true,
            project_explorer_expanded: true,
            show_terminal_manager: true,
            terminal_manager_expanded: true,
            last_selected_project_id: None,
            project_filter_mode: false,
            auto_tile_scope: AutoTileScope::AllVisible,
            main_visibility_mode: MainVisibilityMode::Global,
            left_sidebar_tab: LeftSidebarTab::Directory,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub saved_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
            default_shell: ShellKind::default(),
            ui: UiConfig::default(),
            projects: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ShellKind;

    #[test]
    fn shell_kind_default_matches_platform() {
        #[cfg(target_os = "windows")]
        assert_eq!(ShellKind::default(), ShellKind::PowerShell);

        #[cfg(not(target_os = "windows"))]
        assert_eq!(ShellKind::default(), ShellKind::Zsh);
    }

    #[test]
    fn shell_kind_available_list_matches_platform() {
        #[cfg(target_os = "windows")]
        assert_eq!(
            ShellKind::available_for_current_platform(),
            &[ShellKind::PowerShell, ShellKind::Cmd]
        );

        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            ShellKind::available_for_current_platform(),
            &[ShellKind::Zsh]
        );
    }
}
