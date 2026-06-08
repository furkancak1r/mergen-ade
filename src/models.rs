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
    /// Saved messages for background terminals (reusable snippets).
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
    /// Foreground saved messages (dynamic task queue).
    /// Sent to foreground terminals and removed from queue upon sending.
    #[serde(default)]
    pub foreground_saved_messages: Vec<String>,
    /// When this project is a git worktree, the root repository path.
    #[serde(default)]
    pub repo_root: Option<PathBuf>,
    /// True when this project was auto-discovered/added as a worktree.
    #[serde(default)]
    pub is_worktree: bool,
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

/// A cached ACP model entry from a past session so its name can be shown in Settings
/// even when no ACP session is currently active.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenCodeAcpModelEntry {
    pub value: String,
    pub name: String,
}

impl OpenCodeAcpModelEntry {
    pub fn new(value: String, name: String) -> Self {
        Self { value, name }
    }
}

pub const APP_CONFIG_VERSION: u32 = 2;
pub const DEFAULT_OPENCODE_BUILD_MODEL: &str =
    "fireworks-ai/accounts/fireworks/routers/kimi-k2p6-turbo";
pub const DEFAULT_OPENCODE_PLAN_MODEL: &str = "openai/gpt-5.5-fast";
pub const DEFAULT_OPENCODE_PLAN_EFFORT: &str = "xhigh";
pub const DEFAULT_OPENCODE_LOOP_PROTECTION_ENABLED: bool = true;
pub const DEFAULT_OPENCODE_BUILD_STEPS_LIMIT: u32 = 32;
pub const DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS: u64 = 600_000;
pub const DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS: u64 = 120_000;
pub const DEFAULT_OPENCODE_KIMI_STRICT_PERMISSIONS: bool = true;
pub const DEFAULT_OPENCODE_ACP_BIND_MODEL_TO_MODE: bool = true;
pub const DEFAULT_OPENCODE_ACP_AUTO_APPROVE_PERMISSIONS: bool = false;

/// OpenCode model configuration with two switchable slots and ACP favorites.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenCodeModelConfig {
    /// Model name for build mode slot A.
    pub build_model_slot_a: String,
    /// Model name for build mode slot B.
    pub build_model_slot_b: String,
    /// Model used by OpenCode plan mode and ACP plan sessions.
    pub plan_model: String,
    /// Effort/variant used by OpenCode plan mode and ACP plan sessions.
    pub plan_effort: String,
    /// Which slot is currently active ("a" or "b").
    pub active_build_model_slot: String,
    /// Favorite ACP model values (persisted). Only these appear in the composer dropdown.
    #[serde(default)]
    pub acp_favorite_models: Vec<String>,
    /// Cached known ACP models from recent sessions (value + display name).
    #[serde(default)]
    pub acp_known_models: Vec<OpenCodeAcpModelEntry>,
    /// Whether ACP mode changes should automatically switch to the mode's configured model.
    #[serde(default = "default_opencode_acp_bind_model_to_mode")]
    pub acp_bind_model_to_mode: bool,
    /// Whether Mergen writes OpenCode safety settings for long tool loops.
    #[serde(default = "default_opencode_loop_protection_enabled")]
    pub loop_protection_enabled: bool,
    /// Maximum OpenCode build-agent steps before OpenCode forces a text summary.
    #[serde(default = "default_opencode_build_steps_limit")]
    pub build_steps_limit: u32,
    /// Fireworks full request timeout in milliseconds.
    #[serde(default = "default_opencode_fireworks_timeout_ms")]
    pub fireworks_timeout_ms: u64,
    /// Fireworks streamed chunk timeout in milliseconds.
    #[serde(default = "default_opencode_fireworks_chunk_timeout_ms")]
    pub fireworks_chunk_timeout_ms: u64,
    /// Use stricter build permissions for Kimi-family models.
    #[serde(default = "default_opencode_kimi_strict_permissions")]
    pub kimi_strict_permissions: bool,
    /// Automatically approve ACP permission requests without user interaction.
    #[serde(default = "default_opencode_acp_auto_approve_permissions")]
    pub acp_auto_approve_permissions: bool,
}

impl Default for OpenCodeModelConfig {
    fn default() -> Self {
        let mut config = Self {
            build_model_slot_a: DEFAULT_OPENCODE_BUILD_MODEL.to_owned(),
            build_model_slot_b: DEFAULT_OPENCODE_PLAN_MODEL.to_owned(),
            plan_model: DEFAULT_OPENCODE_PLAN_MODEL.to_owned(),
            plan_effort: DEFAULT_OPENCODE_PLAN_EFFORT.to_owned(),
            active_build_model_slot: "a".to_owned(),
            acp_favorite_models: Vec::new(),
            acp_known_models: Vec::new(),
            acp_bind_model_to_mode: DEFAULT_OPENCODE_ACP_BIND_MODEL_TO_MODE,
            loop_protection_enabled: DEFAULT_OPENCODE_LOOP_PROTECTION_ENABLED,
            build_steps_limit: DEFAULT_OPENCODE_BUILD_STEPS_LIMIT,
            fireworks_timeout_ms: DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS,
            fireworks_chunk_timeout_ms: DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS,
            kimi_strict_permissions: DEFAULT_OPENCODE_KIMI_STRICT_PERMISSIONS,
            acp_auto_approve_permissions: DEFAULT_OPENCODE_ACP_AUTO_APPROVE_PERMISSIONS,
        };
        config.ensure_configured_models_are_favorites();
        config
    }
}

fn default_opencode_loop_protection_enabled() -> bool {
    DEFAULT_OPENCODE_LOOP_PROTECTION_ENABLED
}

fn default_opencode_build_steps_limit() -> u32 {
    DEFAULT_OPENCODE_BUILD_STEPS_LIMIT
}

fn default_opencode_fireworks_timeout_ms() -> u64 {
    DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS
}

fn default_opencode_fireworks_chunk_timeout_ms() -> u64 {
    DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS
}

fn default_opencode_kimi_strict_permissions() -> bool {
    DEFAULT_OPENCODE_KIMI_STRICT_PERMISSIONS
}

fn default_opencode_acp_bind_model_to_mode() -> bool {
    DEFAULT_OPENCODE_ACP_BIND_MODEL_TO_MODE
}

fn default_opencode_acp_auto_approve_permissions() -> bool {
    DEFAULT_OPENCODE_ACP_AUTO_APPROVE_PERMISSIONS
}

impl OpenCodeModelConfig {
    /// Returns the currently active build model name.
    pub fn active_build_model(&self) -> &str {
        match self.active_build_model_slot.as_str() {
            "b" => &self.build_model_slot_b,
            _ => &self.build_model_slot_a,
        }
    }

    pub fn effective_plan_model(&self) -> &str {
        let model = self.plan_model.trim();
        if model.is_empty() {
            DEFAULT_OPENCODE_PLAN_MODEL
        } else {
            model
        }
    }

    pub fn effective_plan_effort(&self) -> &str {
        let effort = self.plan_effort.trim();
        if effort.is_empty() {
            DEFAULT_OPENCODE_PLAN_EFFORT
        } else {
            effort
        }
    }

    pub fn effective_build_steps_limit(&self) -> u32 {
        if self.build_steps_limit == 0 {
            DEFAULT_OPENCODE_BUILD_STEPS_LIMIT
        } else {
            self.build_steps_limit
        }
    }

    pub fn effective_fireworks_timeout_ms(&self) -> u64 {
        if self.fireworks_timeout_ms == 0 {
            DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS
        } else {
            self.fireworks_timeout_ms
        }
    }

    pub fn effective_fireworks_chunk_timeout_ms(&self) -> u64 {
        if self.fireworks_chunk_timeout_ms == 0 {
            DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS
        } else {
            self.fireworks_chunk_timeout_ms
        }
    }

    /// Sets the active slot ("a" or "b").
    pub fn set_active_slot(&mut self, slot: &str) {
        if slot.eq_ignore_ascii_case("a") || slot.eq_ignore_ascii_case("b") {
            self.active_build_model_slot = slot.to_ascii_lowercase();
        }
    }

    /// Check whether a given model value is marked as a favorite.
    pub fn is_acp_model_favorite(&self, value: &str) -> bool {
        let value = value.trim();
        self.acp_favorite_models.iter().any(|v| v == value)
    }

    /// Add a model value to the favorite list. Empty values are ignored.
    pub fn add_acp_model_favorite(&mut self, value: &str) -> bool {
        let value = value.trim();
        if value.is_empty() || self.is_acp_model_favorite(value) {
            return false;
        }
        self.acp_favorite_models.push(value.to_owned());
        true
    }

    /// Toggle a model value in the favorite list. Returns true if the model is now a favorite.
    pub fn toggle_acp_model_favorite(&mut self, value: &str) -> bool {
        let value = value.trim();
        if value.is_empty() {
            return false;
        }
        if let Some(pos) = self.acp_favorite_models.iter().position(|v| v == value) {
            self.acp_favorite_models.remove(pos);
            false
        } else {
            self.acp_favorite_models.push(value.to_owned());
            true
        }
    }

    /// Remove empty or duplicate entries from the favorite-models list.
    pub fn normalize_acp_favorite_models(&mut self) -> bool {
        let before = self.acp_favorite_models.clone();
        let mut seen = std::collections::HashSet::new();
        self.acp_favorite_models = before
            .iter()
            .filter_map(|value| {
                let value = value.trim();
                if value.is_empty() || !seen.insert(value.to_owned()) {
                    None
                } else {
                    Some(value.to_owned())
                }
            })
            .collect();
        self.acp_favorite_models != before
    }

    /// Ensure the currently configured Plan and Build models are available in ACP favorites.
    pub fn ensure_configured_models_are_favorites(&mut self) -> bool {
        let configured_models = [
            self.build_model_slot_a.trim().to_owned(),
            self.build_model_slot_b.trim().to_owned(),
            self.effective_plan_model().trim().to_owned(),
        ];
        let mut changed = self.normalize_acp_favorite_models();
        for model in configured_models {
            changed |= self.add_acp_model_favorite(&model);
        }
        changed
    }

    /// Remove empty or duplicate entries from the known-models cache.
    pub fn normalize_acp_known_models(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.acp_known_models.retain(|e| {
            let keep = !e.value.is_empty() && seen.insert(e.value.clone());
            keep
        });
    }

    /// Merge newly discovered ACP model options into the known-models cache.
    /// Updates existing entries with the latest name, adds new ones.
    /// Returns true when the cache changed.
    pub fn merge_acp_known_models(
        &mut self,
        options: impl Iterator<Item = (String, String)>,
    ) -> bool {
        let mut changed = false;
        for (value, name) in options {
            if let Some(existing) = self.acp_known_models.iter_mut().find(|e| e.value == value) {
                if existing.name != name {
                    existing.name = name;
                    changed = true;
                }
            } else {
                self.acp_known_models
                    .push(OpenCodeAcpModelEntry::new(value, name));
                changed = true;
            }
        }
        self.normalize_acp_known_models();
        changed
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
            id: "github-push".to_owned(),
            label: "GitHub Push".to_owned(),
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
            key: "F11".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "/implement-plan".to_owned(),
            enabled: true,
        },
        TerminalShortcutEntry {
            id: "review-guard".to_owned(),
            label: "Review Guard".to_owned(),
            key: "F7".to_owned(),
            modifiers: ShortcutModifiers::default(),
            command: "/review-guard".to_owned(),
            enabled: true,
        },
    ]
}

/// Migrates old shortcut IDs and keys to new defaults when the entry matches legacy defaults.
/// This ensures users with old configs get updated key bindings without losing customizations.
fn migrate_legacy_shortcut(entry: &mut TerminalShortcutEntry) {
    // Migrate old "semgrep-check" ID to "github-push"
    if entry.id == "semgrep-check" {
        entry.id = "github-push".to_owned();
        // Update label if it was the old default "Semgrep Check"
        if entry.label == "Semgrep Check" {
            entry.label = "GitHub Push".to_owned();
        }
    }

    // Migrate implement-plan key from F7 to F11 if it matches old default
    if entry.id == "implement-plan" && entry.key == "F7" {
        entry.key = "F11".to_owned();
    }

    // Migrate review-guard key from F8 to F7 if it matches old default
    if entry.id == "review-guard" && entry.key == "F8" {
        entry.key = "F7".to_owned();
    }
}

pub fn normalize_terminal_shortcut_entries(entries: &mut Vec<TerminalShortcutEntry>) {
    let mut existing_entries = std::mem::take(entries);
    let defaults = default_terminal_shortcuts();

    // First, migrate legacy shortcut IDs and keys
    for entry in &mut existing_entries {
        migrate_legacy_shortcut(entry);
    }

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OsNotificationConfig {
    pub enabled: bool,
    pub only_when_unfocused: bool,
    pub on_permission: bool,
    pub on_turn_complete: bool,
    pub on_session_error: bool,
    pub cooldown_secs: u64,
}

impl Default for OsNotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            only_when_unfocused: true,
            on_permission: true,
            on_turn_complete: true,
            on_session_error: true,
            cooldown_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct AcpModeToggleShortcut {
    pub key: String,
    pub modifiers: ShortcutModifiers,
    pub enabled: bool,
}

impl AcpModeToggleShortcut {
    pub fn default_tab() -> Self {
        Self {
            key: "Tab".to_string(),
            modifiers: ShortcutModifiers::default(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AcpStartupMode {
    Build,
    Plan,
}

impl Default for AcpStartupMode {
    fn default() -> Self {
        Self::Plan
    }
}

impl AcpStartupMode {
    pub fn as_mode_id(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Plan => "plan",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Build => "Default",
            Self::Plan => "Plan",
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
    #[serde(default)]
    pub notifications: OsNotificationConfig,
    #[serde(default = "AcpModeToggleShortcut::default_tab")]
    pub acp_mode_toggle_shortcut: AcpModeToggleShortcut,
    #[serde(default)]
    pub acp_startup_mode: AcpStartupMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: APP_CONFIG_VERSION,
            default_shell: ShellKind::default(),
            ui: UiConfig::default(),
            launchers: default_launchers(),
            terminal_shortcuts: default_terminal_shortcuts(),
            projects: Vec::new(),
            ai_hooks: AiHooksConfig::default(),
            opencode: OpenCodeModelConfig::default(),
            notifications: OsNotificationConfig::default(),
            acp_mode_toggle_shortcut: AcpModeToggleShortcut::default_tab(),
            acp_startup_mode: AcpStartupMode::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_launchers, default_terminal_shortcuts, normalize_launcher_entries,
        normalize_terminal_shortcut_entries, AcpStartupMode, AppConfig, BuiltinLauncherKind,
        LauncherEntry, LauncherIconKey, OpenCodeAcpModelEntry, OpenCodeModelConfig, ShellKind,
        ShortcutModifiers, TerminalShortcutEntry, UiConfig, DEFAULT_OPENCODE_BUILD_STEPS_LIMIT,
        DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS, DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS,
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
        assert_eq!(shortcuts[0].id, "github-push");
        assert_eq!(shortcuts[0].label, "GitHub Push");
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
        assert_eq!(shortcuts[2].label, "Implement Plan");
        assert_eq!(shortcuts[2].key, "F11");
        assert_eq!(shortcuts[2].command, "/implement-plan");
        assert_eq!(shortcuts[2].modifiers, ShortcutModifiers::default());
        assert_eq!(shortcuts[3].id, "review-guard");
        assert_eq!(shortcuts[3].label, "Review Guard");
        assert_eq!(shortcuts[3].key, "F7");
        assert_eq!(shortcuts[3].command, "/review-guard");
        assert_eq!(shortcuts[3].modifiers, ShortcutModifiers::default());
    }

    #[test]
    fn app_config_default_includes_terminal_shortcuts() {
        let config = AppConfig::default();

        assert_eq!(config.terminal_shortcuts, default_terminal_shortcuts());
    }

    #[test]
    fn app_config_default_acp_mode_toggle_is_tab() {
        let config = AppConfig::default();
        assert_eq!(config.acp_mode_toggle_shortcut.key, "Tab");
        assert_eq!(
            config.acp_mode_toggle_shortcut.modifiers,
            ShortcutModifiers::default()
        );
        assert!(config.acp_mode_toggle_shortcut.enabled);
    }

    #[test]
    fn app_config_default_acp_startup_mode_is_plan() {
        let config = AppConfig::default();
        assert_eq!(config.acp_startup_mode, AcpStartupMode::Plan);
    }

    #[test]
    fn acp_startup_mode_serde_roundtrip() {
        let plan = AcpStartupMode::Plan;
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(json, "\"Plan\"");
        let back: AcpStartupMode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, AcpStartupMode::Plan);
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
            .filter(|shortcut| shortcut.id != "github-push")
            .collect::<Vec<_>>();

        normalize_terminal_shortcut_entries(&mut shortcuts);

        assert_eq!(shortcuts.len(), 4);
        assert_eq!(shortcuts[0].id, "github-push");
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

    #[test]
    fn normalize_terminal_shortcuts_migrates_legacy_defaults() {
        // Simulate old config with legacy IDs and keys
        let mut shortcuts = vec![
            TerminalShortcutEntry {
                id: "semgrep-check".to_owned(),
                label: "Semgrep Check".to_owned(),
                key: "F5".to_owned(),
                modifiers: ShortcutModifiers::default(),
                command: "/gt".to_owned(),
                enabled: true,
            },
            TerminalShortcutEntry {
                id: "implement-plan".to_owned(),
                label: "Implement Plan".to_owned(),
                key: "F7".to_owned(), // Old F7 key
                modifiers: ShortcutModifiers::default(),
                command: "/implement-plan".to_owned(),
                enabled: true,
            },
            TerminalShortcutEntry {
                id: "review-guard".to_owned(),
                label: "Review Guard".to_owned(),
                key: "F8".to_owned(), // Old F8 key
                modifiers: ShortcutModifiers::default(),
                command: "/review-guard".to_owned(),
                enabled: true,
            },
        ];

        normalize_terminal_shortcut_entries(&mut shortcuts);

        // Check that legacy IDs were migrated
        let github_push = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "github-push")
            .expect("github-push shortcut should exist");
        assert_eq!(github_push.label, "GitHub Push");
        assert_eq!(github_push.key, "F5");

        // Check that implement-plan key was migrated from F7 to F11
        let implement = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "implement-plan")
            .expect("implement-plan shortcut should exist");
        assert_eq!(implement.key, "F11");

        // Check that review-guard key was migrated from F8 to F7
        let review = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "review-guard")
            .expect("review-guard shortcut should exist");
        assert_eq!(review.key, "F7");
    }

    #[test]
    fn normalize_terminal_shortcuts_preserves_user_customizations_during_migration() {
        // User customized the old shortcut - should NOT be migrated
        let mut shortcuts = vec![
            TerminalShortcutEntry {
                id: "semgrep-check".to_owned(),
                label: "My Custom Label".to_owned(), // Custom label
                key: "F5".to_owned(),
                modifiers: ShortcutModifiers::default(),
                command: "/gt".to_owned(),
                enabled: true,
            },
            TerminalShortcutEntry {
                id: "implement-plan".to_owned(),
                label: "Implement Plan".to_owned(),
                key: "F9".to_owned(), // User customized to F9, not old default F7
                modifiers: ShortcutModifiers::default(),
                command: "/implement-plan".to_owned(),
                enabled: true,
            },
        ];

        normalize_terminal_shortcut_entries(&mut shortcuts);

        // ID should still migrate
        let github_push = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "github-push")
            .expect("github-push shortcut should exist");
        // But custom label should be preserved
        assert_eq!(github_push.label, "My Custom Label");

        // User customized key to F9, not old F7, so should NOT migrate to F11
        let implement = shortcuts
            .iter()
            .find(|shortcut| shortcut.id == "implement-plan")
            .expect("implement-plan shortcut should exist");
        assert_eq!(implement.key, "F9"); // Preserved user customization
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
    fn opencode_model_config_defaults_include_build_and_plan_models() {
        let config = OpenCodeModelConfig::default();
        assert!(config.build_model_slot_a.contains("kimi-k2p6-turbo"));
        assert!(config.build_model_slot_b.contains("gpt-5.5-fast"));
        assert_eq!(config.plan_model, "openai/gpt-5.5-fast");
        assert_eq!(config.plan_effort, "xhigh");
        assert_eq!(config.active_build_model_slot, "a");
        assert!(config.loop_protection_enabled);
        assert_eq!(config.build_steps_limit, DEFAULT_OPENCODE_BUILD_STEPS_LIMIT);
        assert_eq!(
            config.fireworks_timeout_ms,
            DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS
        );
        assert_eq!(
            config.fireworks_chunk_timeout_ms,
            DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS
        );
        assert!(config.kimi_strict_permissions);
        assert!(config.is_acp_model_favorite(&config.build_model_slot_a));
        assert!(config.is_acp_model_favorite(&config.build_model_slot_b));
        assert!(config.is_acp_model_favorite(config.effective_plan_model()));
        assert_eq!(config.acp_favorite_models.len(), 2);
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
            plan_model: "custom/plan-model".to_owned(),
            plan_effort: "xhigh".to_owned(),
            active_build_model_slot: "b".to_owned(),
            acp_favorite_models: vec!["custom/model-a".to_owned()],
            acp_known_models: vec![OpenCodeAcpModelEntry::new(
                "custom/model-a".to_owned(),
                "Custom Model A".to_owned(),
            )],
            acp_bind_model_to_mode: false,
            loop_protection_enabled: false,
            build_steps_limit: 12,
            fireworks_timeout_ms: 700_000,
            fireworks_chunk_timeout_ms: 180_000,
            kimi_strict_permissions: false,
            acp_auto_approve_permissions: false,
        };

        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: OpenCodeModelConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.build_model_slot_a, "custom/model-a");
        assert_eq!(deserialized.build_model_slot_b, "custom/model-b");
        assert_eq!(deserialized.plan_model, "custom/plan-model");
        assert_eq!(deserialized.plan_effort, "xhigh");
        assert_eq!(deserialized.active_build_model_slot, "b");
        assert_eq!(deserialized.acp_favorite_models, vec!["custom/model-a"]);
        assert_eq!(deserialized.acp_known_models.len(), 1);
        assert_eq!(deserialized.acp_known_models[0].value, "custom/model-a");
        assert_eq!(deserialized.acp_known_models[0].name, "Custom Model A");
        assert!(!deserialized.acp_bind_model_to_mode);
        assert!(!deserialized.loop_protection_enabled);
        assert_eq!(deserialized.build_steps_limit, 12);
        assert_eq!(deserialized.fireworks_timeout_ms, 700_000);
        assert_eq!(deserialized.fireworks_chunk_timeout_ms, 180_000);
        assert!(!deserialized.kimi_strict_permissions);
    }

    #[test]
    fn opencode_model_config_missing_safety_fields_use_defaults() {
        let raw = r#"{
            "build_model_slot_a": "custom/model-a",
            "build_model_slot_b": "custom/model-b",
            "plan_model": "custom/plan",
            "plan_effort": "high",
            "active_build_model_slot": "a"
        }"#;

        let config: OpenCodeModelConfig = serde_json::from_str(raw).unwrap();

        assert!(config.loop_protection_enabled);
        assert!(config.acp_bind_model_to_mode);
        assert_eq!(
            config.effective_build_steps_limit(),
            DEFAULT_OPENCODE_BUILD_STEPS_LIMIT
        );
        assert_eq!(
            config.effective_fireworks_timeout_ms(),
            DEFAULT_OPENCODE_FIREWORKS_TIMEOUT_MS
        );
        assert_eq!(
            config.effective_fireworks_chunk_timeout_ms(),
            DEFAULT_OPENCODE_FIREWORKS_CHUNK_TIMEOUT_MS
        );
        assert!(config.kimi_strict_permissions);
    }

    #[test]
    fn toggle_acp_model_favorite_adds_and_removes() {
        let mut config = OpenCodeModelConfig::default();
        config.acp_favorite_models.clear();
        assert!(!config.is_acp_model_favorite("gpt-4"));

        assert!(config.toggle_acp_model_favorite("gpt-4"));
        assert!(config.is_acp_model_favorite("gpt-4"));
        assert_eq!(config.acp_favorite_models.len(), 1);

        assert!(!config.toggle_acp_model_favorite("gpt-4"));
        assert!(!config.is_acp_model_favorite("gpt-4"));
        assert!(config.acp_favorite_models.is_empty());
    }

    #[test]
    fn ensure_configured_models_are_favorites_trims_deduplicates_and_adds_current_models() {
        let mut config = OpenCodeModelConfig::default();
        config.build_model_slot_a = " custom/build-a ".to_owned();
        config.build_model_slot_b = "custom/build-b".to_owned();
        config.plan_model = "custom/plan".to_owned();
        config.acp_favorite_models = vec![
            " custom/build-a ".to_owned(),
            "".to_owned(),
            "custom/build-a".to_owned(),
        ];

        assert!(config.ensure_configured_models_are_favorites());
        assert_eq!(
            config.acp_favorite_models,
            vec!["custom/build-a", "custom/build-b", "custom/plan"]
        );
    }

    #[test]
    fn normalize_acp_known_models_deduplicates_and_removes_empty() {
        let mut config = OpenCodeModelConfig::default();
        config.acp_known_models = vec![
            OpenCodeAcpModelEntry::new("gpt-4".to_owned(), "GPT-4".to_owned()),
            OpenCodeAcpModelEntry::new("gpt-4".to_owned(), "GPT-4 Duplicate".to_owned()),
            OpenCodeAcpModelEntry::new("".to_owned(), "Empty Value".to_owned()),
        ];
        config.normalize_acp_known_models();
        assert_eq!(config.acp_known_models.len(), 1);
        assert_eq!(config.acp_known_models[0].value, "gpt-4");
        assert_eq!(config.acp_known_models[0].name, "GPT-4");
    }

    #[test]
    fn merge_acp_known_models_updates_names_and_adds_new() {
        let mut config = OpenCodeModelConfig::default();
        config.acp_known_models = vec![OpenCodeAcpModelEntry::new(
            "gpt-4".to_owned(),
            "Old GPT-4".to_owned(),
        )];
        assert!(config.merge_acp_known_models(
            vec![
                ("gpt-4".to_owned(), "New GPT-4".to_owned()),
                ("gpt-5".to_owned(), "GPT-5".to_owned()),
            ]
            .into_iter(),
        ));
        assert_eq!(config.acp_known_models.len(), 2);
        let gpt4 = config
            .acp_known_models
            .iter()
            .find(|e| e.value == "gpt-4")
            .unwrap();
        assert_eq!(gpt4.name, "New GPT-4");
        let gpt5 = config
            .acp_known_models
            .iter()
            .find(|e| e.value == "gpt-5")
            .unwrap();
        assert_eq!(gpt5.name, "GPT-5");
    }
}
