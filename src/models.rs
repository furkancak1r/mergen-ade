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
    // Panel widths (persisted user resizing)
    #[serde(default = "default_project_explorer_width")]
    pub project_explorer_width: f32,
    pub show_terminal_manager: bool,
    pub terminal_manager_expanded: bool,
    pub multi_terminal_view_enabled: bool,
    pub terminal_manager_filter: TerminalManagerFilter,
    pub terminal_manager_hide_inactive_projects: bool,
    pub last_selected_project_id: Option<u64>,
    pub main_visibility_mode: MainVisibilityMode,
    pub left_sidebar_tab: LeftSidebarTab,
    pub checklist_panel_expanded: bool,
    pub browser_panel_expanded: bool,
    #[serde(default = "default_checklist_panel_width")]
    pub checklist_panel_width: f32,
    #[serde(default = "default_browser_panel_width")]
    pub browser_panel_width: f32,
    pub input_history_filter: InputHistoryFilter,
}

const fn default_project_explorer_width() -> f32 {
    352.0
}

const fn default_checklist_panel_width() -> f32 {
    352.0
}

const fn default_browser_panel_width() -> f32 {
    520.0
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_project_explorer: true,
            project_explorer_expanded: true,
            project_explorer_width: default_project_explorer_width(),
            show_terminal_manager: true,
            terminal_manager_expanded: true,
            multi_terminal_view_enabled: false,
            terminal_manager_filter: TerminalManagerFilter::Foreground,
            terminal_manager_hide_inactive_projects: false,
            last_selected_project_id: None,
            main_visibility_mode: MainVisibilityMode::Global,
            left_sidebar_tab: LeftSidebarTab::Directory,
            checklist_panel_expanded: false,
            browser_panel_expanded: false,
            checklist_panel_width: default_checklist_panel_width(),
            browser_panel_width: default_browser_panel_width(),
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
    /// Last browser URL for this project (project-scoped browsing).
    #[serde(default)]
    pub browser_last_url: Option<String>,
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

/// OpenCode model configuration with two switchable slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenCodeModelConfig {
    /// Model name for build mode slot A.
    pub build_model_slot_a: String,
    /// Model name for build mode slot B.
    pub build_model_slot_b: String,
    /// Which slot is currently active ("a" or "b").
    pub active_build_model_slot: String,
}

impl Default for OpenCodeModelConfig {
    fn default() -> Self {
        Self {
            build_model_slot_a: "fireworks-ai/accounts/fireworks/routers/kimi-k2p5-turbo"
                .to_owned(),
            build_model_slot_b: "openai/gpt-5.5-fast".to_owned(),
            active_build_model_slot: "a".to_owned(),
        }
    }
}

impl OpenCodeModelConfig {
    /// Returns the currently active build model name.
    pub fn active_build_model(&self) -> &str {
        match self.active_build_model_slot.as_str() {
            "b" => &self.build_model_slot_b,
            _ => &self.build_model_slot_a,
        }
    }

    /// Sets the active slot ("a" or "b").
    pub fn set_active_slot(&mut self, slot: &str) {
        if slot.eq_ignore_ascii_case("a") || slot.eq_ignore_ascii_case("b") {
            self.active_build_model_slot = slot.to_ascii_lowercase();
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(default)]
pub struct ShortcutModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub command: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TerminalShortcutEntry {
    pub id: String,
    pub label: String,
    pub key: String, // e.g., "F6", "P", "Enter"
    pub modifiers: ShortcutModifiers,
    pub command: String, // e.g., "/prepare-fix-plan"
    pub enabled: bool,
}

impl Default for TerminalShortcutEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            key: String::new(),
            modifiers: ShortcutModifiers::default(),
            command: String::new(),
            enabled: true,
        }
    }
}

/// Returns the default terminal shortcuts that ship with Mergen ADE.
pub fn default_terminal_shortcuts() -> Vec<TerminalShortcutEntry> {
    vec![
        TerminalShortcutEntry {
            id: "semgrep-check".to_owned(),
            label: "Semgrep Check".to_owned(),
            key: "F5".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "/gt".to_owned(),
            enabled: true,
        },
        TerminalShortcutEntry {
            id: "prepare-fix-plan".to_owned(),
            label: "Prepare Fix Plan".to_owned(),
            key: "F6".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "/prepare-fix-plan".to_owned(),
            enabled: true,
        },
        TerminalShortcutEntry {
            id: "implement-plan".to_owned(),
            label: "Implement Plan".to_owned(),
            key: "F7".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "/implement-plan".to_owned(),
            enabled: true,
        },
        TerminalShortcutEntry {
            id: "review-guard".to_owned(),
            label: "Review Guard".to_owned(),
            key: "F8".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "/review-guard".to_owned(),
            enabled: true,
        },
    ]
}

pub fn normalize_terminal_shortcut_entries(entries: &mut Vec<TerminalShortcutEntry>) {
    let existing_entries = std::mem::take(entries);
    let defaults = default_terminal_shortcuts();
    let mut normalized = Vec::new();

    for default in &defaults {
        if let Some(existing) = existing_entries.iter().find(|entry| entry.id == default.id) {
            let mut entry = existing.clone();
            entry.id = default.id.clone();
            if entry.label.trim().is_empty() {
                entry.label = default.label.clone();
            }
            if entry.key.trim().is_empty() {
                entry.key = default.key.clone();
            }
            if entry.command.trim().is_empty() {
                entry.command = default.command.clone();
            }
            normalize_shortcut_modifiers_for_current_platform(&mut entry.modifiers);
            normalized.push(entry);
        } else {
            let mut entry = default.clone();
            normalize_shortcut_modifiers_for_current_platform(&mut entry.modifiers);
            normalized.push(entry);
        }
    }

    for (index, mut entry) in existing_entries.into_iter().enumerate() {
        if defaults.iter().any(|default| default.id == entry.id) {
            continue;
        }
        if entry.id.trim().is_empty() {
            entry.id = format!("custom-{}", index + 1);
        }
        normalize_shortcut_modifiers_for_current_platform(&mut entry.modifiers);
        normalized.push(entry);
    }

    *entries = normalized;
}

fn normalize_shortcut_modifiers_for_current_platform(modifiers: &mut ShortcutModifiers) {
    #[cfg(not(target_os = "macos"))]
    {
        if modifiers.ctrl {
            modifiers.command = false;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub default_shell: ShellKind,
    pub ui: UiConfig,
    #[serde(default = "default_launchers")]
    pub launchers: Vec<LauncherEntry>,
    #[serde(default = "default_terminal_shortcuts")]
    pub terminal_shortcuts: Vec<TerminalShortcutEntry>,
    pub projects: Vec<ProjectRecord>,
    pub ai_hooks: AiHooksConfig,
    #[serde(default)]
    pub opencode: OpenCodeModelConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            default_shell: ShellKind::default(),
            ui: UiConfig::default(),
            launchers: default_launchers(),
            terminal_shortcuts: default_terminal_shortcuts(),
            projects: Vec::new(),
            ai_hooks: AiHooksConfig::default(),
            opencode: OpenCodeModelConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_launchers, default_terminal_shortcuts, normalize_launcher_entries,
        normalize_terminal_shortcut_entries, AppConfig, BuiltinLauncherKind, LauncherEntry,
        LauncherIconKey, OpenCodeModelConfig, ShellKind, ShortcutModifiers, TerminalShortcutEntry,
        UiConfig,
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
    fn default_terminal_shortcuts_include_expected_entries() {
        let shortcuts = default_terminal_shortcuts();

        assert_eq!(shortcuts.len(), 4);
        assert_eq!(shortcuts[0].id, "semgrep-check");
        assert_eq!(shortcuts[0].label, "Semgrep Check");
        assert_eq!(shortcuts[0].key, "F5");
        assert_eq!(shortcuts[0].command, "/gt");
        assert_eq!(shortcuts[0].modifiers, ShortcutModifiers::default());
        assert!(shortcuts[0].enabled);
        assert_eq!(shortcuts[1].id, "prepare-fix-plan");
        assert_eq!(shortcuts[1].label, "Prepare Fix Plan");
        assert_eq!(shortcuts[1].key, "F6");
        assert_eq!(shortcuts[1].command, "/prepare-fix-plan");
        assert_eq!(shortcuts[1].modifiers, ShortcutModifiers::default());
        assert_eq!(shortcuts[2].id, "implement-plan");
        assert_eq!(shortcuts[2].key, "F7");
        assert_eq!(shortcuts[2].command, "/implement-plan");
        assert_eq!(shortcuts[2].modifiers, ShortcutModifiers::default());
        assert_eq!(shortcuts[3].id, "review-guard");
        assert_eq!(shortcuts[3].key, "F8");
        assert_eq!(shortcuts[3].command, "/review-guard");
        assert_eq!(shortcuts[3].modifiers, ShortcutModifiers::default());
    }

    #[test]
    fn app_config_default_includes_terminal_shortcuts() {
        let config = AppConfig::default();

        assert_eq!(config.terminal_shortcuts, default_terminal_shortcuts());
    }

    #[test]
    fn terminal_shortcut_entry_default_is_enabled_and_empty() {
        let shortcut = TerminalShortcutEntry::default();

        assert!(shortcut.enabled);
        assert_eq!(shortcut.modifiers, ShortcutModifiers::default());
        assert!(shortcut.id.is_empty());
        assert!(shortcut.label.is_empty());
        assert!(shortcut.key.is_empty());
        assert!(shortcut.command.is_empty());
    }

    #[test]
    fn normalize_terminal_shortcuts_restores_missing_defaults() {
        let mut shortcuts = default_terminal_shortcuts()
            .into_iter()
            .filter(|shortcut| shortcut.id != "semgrep-check")
            .collect::<Vec<_>>();

        normalize_terminal_shortcut_entries(&mut shortcuts);

        assert_eq!(shortcuts.len(), 4);
        assert_eq!(shortcuts[0].id, "semgrep-check");
        assert_eq!(shortcuts[0].key, "F5");
        assert_eq!(shortcuts[0].command, "/gt");
    }

    #[test]
    fn normalize_terminal_shortcuts_preserves_user_edits() {
        let mut shortcuts = default_terminal_shortcuts();
        let prepare = shortcuts
            .iter_mut()
            .find(|shortcut| shortcut.id == "prepare-fix-plan")
            .expect("prepare shortcut");
        prepare.label = "Planla".to_owned();
        prepare.key = "P".to_owned();
        prepare.command = "/custom-plan".to_owned();
        prepare.modifiers.ctrl = true;
        prepare.enabled = false;
        shortcuts.push(TerminalShortcutEntry {
            id: "custom-extra".to_owned(),
            label: "Custom Extra".to_owned(),
            key: "F9".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "cargo test".to_owned(),
            enabled: true,
        });

        normalize_terminal_shortcut_entries(&mut shortcuts);

        let prepare = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "prepare-fix-plan")
            .expect("prepare shortcut");
        assert_eq!(prepare.label, "Planla");
        assert_eq!(prepare.key, "P");
        assert_eq!(prepare.command, "/custom-plan");
        assert!(prepare.modifiers.ctrl);
        assert!(!prepare.enabled);
        assert!(shortcuts
            .iter()
            .any(|shortcut| shortcut.id == "custom-extra" && shortcut.command == "cargo test"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn normalize_terminal_shortcuts_clears_legacy_command_alias_on_non_macos() {
        let mut shortcuts = vec![TerminalShortcutEntry {
            id: "legacy-ctrl-p".to_owned(),
            label: "Legacy Ctrl P".to_owned(),
            key: "P".to_owned(),
            modifiers: ShortcutModifiers {
                ctrl: true,
                command: true,
                ..ShortcutModifiers::default()
            },
            command: "/legacy".to_owned(),
            enabled: true,
        }];

        normalize_terminal_shortcut_entries(&mut shortcuts);

        let legacy = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "legacy-ctrl-p")
            .expect("legacy shortcut");
        assert!(legacy.modifiers.ctrl);
        assert!(!legacy.modifiers.command);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn normalize_terminal_shortcuts_preserves_command_only_shortcut_on_non_macos() {
        let mut shortcuts = vec![TerminalShortcutEntry {
            id: "command-only".to_owned(),
            label: "Command Only".to_owned(),
            key: "P".to_owned(),
            modifiers: ShortcutModifiers {
                command: true,
                ..ShortcutModifiers::default()
            },
            command: "/command-only".to_owned(),
            enabled: true,
        }];

        normalize_terminal_shortcut_entries(&mut shortcuts);

        let command_only = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "command-only")
            .expect("command-only shortcut");
        assert!(!command_only.modifiers.ctrl);
        assert!(command_only.modifiers.command);
    }

    #[test]
    fn ui_config_defaults_include_panel_widths() {
        let config = UiConfig::default();

        assert_eq!(config.project_explorer_width, 352.0);
        assert_eq!(config.checklist_panel_width, 352.0);
        assert_eq!(config.browser_panel_width, 520.0);
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

    #[test]
    fn opencode_model_config_default_slot_a_is_kimi_k2p5_turbo() {
        let config = OpenCodeModelConfig::default();
        assert!(config.build_model_slot_a.contains("kimi-k2p5-turbo"));
        assert!(config.build_model_slot_b.contains("gpt-5.5-fast"));
        assert_eq!(config.active_build_model_slot, "a");
    }

    #[test]
    fn opencode_model_config_active_build_model_returns_correct_slot() {
        let mut config = OpenCodeModelConfig::default();
        config.build_model_slot_a = "model-a".to_owned();
        config.build_model_slot_b = "model-b".to_owned();

        config.active_build_model_slot = "a".to_owned();
        assert_eq!(config.active_build_model(), "model-a");

        config.active_build_model_slot = "b".to_owned();
        assert_eq!(config.active_build_model(), "model-b");

        // Invalid slot defaults to a
        config.active_build_model_slot = "invalid".to_owned();
        assert_eq!(config.active_build_model(), "model-a");
    }

    #[test]
    fn opencode_model_config_set_active_slot_validates_input() {
        let mut config = OpenCodeModelConfig::default();

        config.set_active_slot("a");
        assert_eq!(config.active_build_model_slot, "a");

        config.set_active_slot("B"); // uppercase should work
        assert_eq!(config.active_build_model_slot, "b");

        config.set_active_slot("invalid"); // should not change
        assert_eq!(config.active_build_model_slot, "b");
    }

    #[test]
    fn opencode_model_config_roundtrips_through_serde() {
        let original = OpenCodeModelConfig {
            build_model_slot_a: "custom/model-a".to_owned(),
            build_model_slot_b: "custom/model-b".to_owned(),
            active_build_model_slot: "b".to_owned(),
        };

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: OpenCodeModelConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.build_model_slot_a, "custom/model-a");
        assert_eq!(deserialized.build_model_slot_b, "custom/model-b");
        assert_eq!(deserialized.active_build_model_slot, "b");
    }
}
