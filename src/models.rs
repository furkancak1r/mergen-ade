use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::hooks::{AiHooksConfig, ProjectAiConfig};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinLauncherKind {
    Droid,
    Codex,
    OpenCode,
    Claude,
}

impl BuiltinLauncherKind {
    pub const ALL: [Self; 4] = [Self::OpenCode, Self::Codex, Self::Droid, Self::Claude];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Droid => "droid",
            Self::OpenCode => "opencode",
        }
    }

    pub const fn default_display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Droid => "Droid",
            Self::OpenCode => "OpenCode",
        }
    }

    pub const fn default_launch_command(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Droid => "droid",
            Self::OpenCode => "opencode",
        }
    }

    pub const fn icon_key(self) -> LauncherIconKey {
        match self {
            Self::Codex => LauncherIconKey::Codex,
            Self::Claude => LauncherIconKey::Claude,
            Self::Droid => LauncherIconKey::Droid,
            Self::OpenCode => LauncherIconKey::OpenCode,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherIconKey {
    Codex,
    Claude,
    Droid,
    OpenCode,
    Terminal,
    Spark,
    Message,
    Bot,
    Code,
    Wrench,
    Rocket,
}

impl LauncherIconKey {
    pub const CUSTOM_PRESETS: [Self; 7] = [
        Self::Terminal,
        Self::Spark,
        Self::Message,
        Self::Bot,
        Self::Code,
        Self::Wrench,
        Self::Rocket,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Droid => "Droid",
            Self::OpenCode => "OpenCode",
            Self::Terminal => "Terminal",
            Self::Spark => "Spark",
            Self::Message => "Message",
            Self::Bot => "Bot",
            Self::Code => "Code",
            Self::Wrench => "Wrench",
            Self::Rocket => "Rocket",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherEntry {
    pub id: String,
    #[serde(default)]
    pub builtin: Option<BuiltinLauncherKind>,
    pub display_name: String,
    pub launch_command: String,
    #[serde(default = "default_launcher_enabled")]
    pub enabled: bool,
    pub icon_key: LauncherIconKey,
}

impl LauncherEntry {
    pub fn builtin(kind: BuiltinLauncherKind) -> Self {
        Self {
            id: kind.id().to_owned(),
            builtin: Some(kind),
            display_name: kind.default_display_name().to_owned(),
            launch_command: kind.default_launch_command().to_owned(),
            enabled: true,
            icon_key: kind.icon_key(),
        }
    }
}

const fn default_launcher_enabled() -> bool {
    true
}

pub fn default_launchers() -> Vec<LauncherEntry> {
    BuiltinLauncherKind::ALL
        .into_iter()
        .map(LauncherEntry::builtin)
        .collect()
}

pub fn normalize_launcher_entries(entries: &mut Vec<LauncherEntry>) {
    let mut normalized = Vec::new();

    for builtin in BuiltinLauncherKind::ALL {
        if let Some(existing) = entries
            .iter()
            .find(|entry| entry.builtin == Some(builtin) || entry.id == builtin.id())
        {
            normalized.push(LauncherEntry {
                id: builtin.id().to_owned(),
                builtin: Some(builtin),
                display_name: if existing.display_name.trim().is_empty() {
                    builtin.default_display_name().to_owned()
                } else {
                    existing.display_name.clone()
                },
                launch_command: if existing.launch_command.trim().is_empty() {
                    builtin.default_launch_command().to_owned()
                } else {
                    existing.launch_command.clone()
                },
                enabled: existing.enabled,
                icon_key: builtin.icon_key(),
            });
        } else {
            normalized.push(LauncherEntry::builtin(builtin));
        }
    }

    for (index, entry) in entries.iter().enumerate() {
        if entry.builtin.is_some() {
            continue;
        }
        if entry.display_name.trim().is_empty() || entry.launch_command.trim().is_empty() {
            continue;
        }

        let id = if entry.id.trim().is_empty() {
            format!("custom-{}", index + 1)
        } else {
            entry.id.clone()
        };

        normalized.push(LauncherEntry {
            id,
            builtin: None,
            display_name: entry.display_name.clone(),
            launch_command: entry.launch_command.clone(),
            enabled: entry.enabled,
            icon_key: entry.icon_key,
        });
    }

    *entries = normalized;
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
pub enum TerminalManagerFilter {
    #[default]
    Foreground,
    Background,
}

impl TerminalManagerFilter {
    pub const fn terminal_kind(self) -> TerminalKind {
        match self {
            Self::Foreground => TerminalKind::Foreground,
            Self::Background => TerminalKind::Background,
        }
    }
}

/// Filter for input history panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputHistoryFilter {
    #[default]
    All,
    Foreground,
    Background,
}

impl InputHistoryFilter {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Foreground => "Foreground",
            Self::Background => "Background",
        }
    }

    pub fn matches(self, kind: TerminalKind) -> bool {
        match self {
            Self::All => true,
            Self::Foreground => kind == TerminalKind::Foreground,
            Self::Background => kind == TerminalKind::Background,
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
    InputHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub show_project_explorer: bool,
    pub project_explorer_expanded: bool,
    pub show_terminal_manager: bool,
    pub terminal_manager_expanded: bool,
    pub multi_terminal_view_enabled: bool,
    pub terminal_manager_filter: TerminalManagerFilter,
    pub terminal_manager_hide_inactive_projects: bool,
    pub last_selected_project_id: Option<u64>,
    pub main_visibility_mode: MainVisibilityMode,
    pub left_sidebar_tab: LeftSidebarTab,
    pub checklist_panel_expanded: bool,
    pub input_history_filter: InputHistoryFilter,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_project_explorer: true,
            project_explorer_expanded: true,
            show_terminal_manager: true,
            terminal_manager_expanded: true,
            multi_terminal_view_enabled: false,
            terminal_manager_filter: TerminalManagerFilter::Foreground,
            terminal_manager_hide_inactive_projects: false,
            last_selected_project_id: None,
            main_visibility_mode: MainVisibilityMode::Global,
            left_sidebar_tab: LeftSidebarTab::Directory,
            checklist_panel_expanded: false,
            input_history_filter: InputHistoryFilter::default(),
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
    #[serde(default)]
    pub ai_config: ProjectAiConfig,
    /// Checklist items (prompts marked by user from history popup).
    /// Persisted across sessions; survives terminal closure.
    #[serde(default)]
    pub checklist: Vec<String>,
}

/// A single recorded terminal input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalInputRecord {
    /// Project path (stable across restarts).
    pub project_path: PathBuf,
    /// Project name at time of recording.
    pub project_name: String,
    /// Terminal kind when recorded.
    pub terminal_kind: TerminalKind,
    /// Raw input text (may include $ prefix, etc).
    pub text: String,
    /// Unix timestamp (seconds since epoch) when recorded.
    pub recorded_at: u64,
}

/// Per-project terminal input history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInputHistory {
    /// Maximum entries to keep per project.
    #[serde(default = "default_history_limit")]
    pub max_entries: usize,
    /// Recorded inputs (newest first).
    #[serde(default)]
    pub entries: Vec<TerminalInputRecord>,
}

impl Default for TerminalInputHistory {
    fn default() -> Self {
        Self {
            max_entries: default_history_limit(),
            entries: Vec::new(),
        }
    }
}

const fn default_history_limit() -> usize {
    500
}

/// Root history file container.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppHistory {
    /// Version for future migrations.
    #[serde(default)]
    pub version: u32,
    /// Per-project history keyed by project path string.
    #[serde(default)]
    pub projects: std::collections::BTreeMap<String, TerminalInputHistory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub default_shell: ShellKind,
    pub ui: UiConfig,
    #[serde(default = "default_launchers")]
    pub launchers: Vec<LauncherEntry>,
    pub projects: Vec<ProjectRecord>,
    pub ai_hooks: AiHooksConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            default_shell: ShellKind::default(),
            ui: UiConfig::default(),
            launchers: default_launchers(),
            projects: Vec::new(),
            ai_hooks: AiHooksConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_launchers, normalize_launcher_entries, BuiltinLauncherKind, LauncherEntry,
        LauncherIconKey, ShellKind,
    };

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

    #[test]
    fn default_launchers_include_expected_builtins() {
        let launchers = default_launchers();

        assert_eq!(launchers.len(), 4);
        assert_eq!(
            launchers[0],
            LauncherEntry::builtin(BuiltinLauncherKind::OpenCode)
        );
        assert_eq!(
            launchers[1],
            LauncherEntry::builtin(BuiltinLauncherKind::Codex)
        );
        assert_eq!(
            launchers[2],
            LauncherEntry::builtin(BuiltinLauncherKind::Droid)
        );
        assert_eq!(
            launchers[3],
            LauncherEntry::builtin(BuiltinLauncherKind::Claude)
        );
    }

    #[test]
    fn normalize_launcher_entries_restores_missing_builtins_and_keeps_custom_entries() {
        let mut launchers = vec![LauncherEntry {
            id: "custom-launcher".to_owned(),
            builtin: None,
            display_name: "Custom".to_owned(),
            launch_command: "my-cli".to_owned(),
            enabled: true,
            icon_key: LauncherIconKey::Rocket,
        }];

        normalize_launcher_entries(&mut launchers);

        assert_eq!(launchers.len(), 5);
        assert_eq!(launchers[0].builtin, Some(BuiltinLauncherKind::OpenCode));
        assert_eq!(launchers[4].builtin, None);
        assert_eq!(launchers[4].display_name, "Custom");
    }
}
