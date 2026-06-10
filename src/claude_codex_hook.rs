use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const MAX_REVIEW_FIX_ROUNDS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStatus {
    Planned,
    Implementing,
    Testing,
    Reviewing,
    Fixing,
    Done,
    Blocked,
}

impl PlanStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Implementing => "implementing",
            Self::Testing => "testing",
            Self::Reviewing => "reviewing",
            Self::Fixing => "fixing",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCommand {
    pub label: String,
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl TestCommand {
    fn new(label: impl Into<String>, program: impl Into<OsString>, args: Vec<OsString>) -> Self {
        Self {
            label: label.into(),
            program: program.into(),
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCommandResult {
    pub label: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PlanFileContent {
    pub session_id: String,
    pub status: Option<PlanStatus>,
    pub original_prompt: String,
    pub plan: Option<String>,
    pub plan_error: Option<String>,
    pub test_results: Vec<TestCommandResult>,
    pub test_note: Option<String>,
    pub review_round: u8,
    pub review_output: Option<String>,
    pub review_error: Option<String>,
    pub ui_changed_files: Vec<String>,
    pub ui_verification: Option<String>,
    pub final_note: Option<String>,
}

pub fn plan_path(project_path: &Path, session_id: &str) -> PathBuf {
    project_path
        .join(".claude")
        .join("plans")
        .join(format!("{session_id}.md"))
}

pub fn session_id(terminal_id: u64, counter: u64) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("mergen-{terminal_id}-{millis:x}-{counter:x}")
}

pub fn write_plan_file(path: &Path, content: &PlanFileContent) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, render_plan_file(content))
}

fn render_plan_file(content: &PlanFileContent) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("session_id: {}\n", content.session_id));
    if let Some(status) = content.status {
        out.push_str(&format!("status: {}\n", status.as_str()));
    }
    out.push_str(&format!("review_round: {}\n", content.review_round));
    out.push_str("---\n\n");
    out.push_str("# Claude Code Codex Hook Plan\n\n");
    out.push_str("## Original Prompt\n\n");
    out.push_str(content.original_prompt.trim());
    out.push_str("\n\n");

    out.push_str("## Codex Plan\n\n");
    match (&content.plan, &content.plan_error) {
        (Some(plan), _) if !plan.trim().is_empty() => out.push_str(plan.trim()),
        (_, Some(error)) => {
            out.push_str(
                "Codex planning failed; Claude Code should continue with the original prompt.\n\n",
            );
            out.push_str(error.trim());
        }
        _ => out.push_str("Codex planning is pending."),
    }
    out.push_str("\n\n");

    out.push_str("## Tests\n\n");
    if let Some(note) = &content.test_note {
        out.push_str(note.trim());
        out.push_str("\n\n");
    }
    if content.test_results.is_empty() {
        out.push_str("No test results recorded yet.\n\n");
    } else {
        for result in &content.test_results {
            out.push_str(&format!(
                "### {}\n\nstatus: {}\n\n",
                result.label,
                if result.success { "passed" } else { "failed" }
            ));
            if let Some(error) = &result.error {
                out.push_str("error:\n\n```text\n");
                out.push_str(&truncate_for_plan(error));
                out.push_str("\n```\n\n");
            }
            if !result.stdout.trim().is_empty() {
                out.push_str("stdout:\n\n```text\n");
                out.push_str(&truncate_for_plan(&result.stdout));
                out.push_str("\n```\n\n");
            }
            if !result.stderr.trim().is_empty() {
                out.push_str("stderr:\n\n```text\n");
                out.push_str(&truncate_for_plan(&result.stderr));
                out.push_str("\n```\n\n");
            }
        }
    }

    out.push_str("## Codex Review\n\n");
    if let Some(error) = &content.review_error {
        out.push_str("Codex review failed.\n\n```text\n");
        out.push_str(&truncate_for_plan(error));
        out.push_str("\n```\n\n");
    } else if let Some(review) = &content.review_output {
        out.push_str("```text\n");
        out.push_str(&truncate_for_plan(review));
        out.push_str("\n```\n\n");
    } else {
        out.push_str("No review result recorded yet.\n\n");
    }

    out.push_str("## UI Verification\n\n");
    if content.ui_changed_files.is_empty() {
        out.push_str("No UI-facing changed files detected.\n\n");
    } else {
        out.push_str("Detected UI-facing changed files:\n\n");
        for file in &content.ui_changed_files {
            out.push_str("- ");
            out.push_str(file);
            out.push('\n');
        }
        out.push('\n');
    }
    if let Some(ui_verification) = &content.ui_verification {
        out.push_str(ui_verification.trim());
        out.push_str("\n\n");
    }

    if let Some(note) = &content.final_note {
        out.push_str("## Final Note\n\n");
        out.push_str(note.trim());
        out.push('\n');
    }

    out
}

fn truncate_for_plan(text: &str) -> String {
    const MAX_CHARS: usize = 12_000;
    let mut truncated = text.chars().take(MAX_CHARS).collect::<String>();
    if text.chars().count() > MAX_CHARS {
        truncated.push_str("\n[truncated]");
    }
    truncated
}

pub fn build_codex_plan_prompt(original_prompt: &str, plan_path: &Path) -> String {
    format!(
        "You are the read-only planning hook for a Claude Code implementation turn.\n\
         Do not edit files. Do not run commands that write to disk. Inspect only what is needed.\n\
         Produce a concise implementation plan with risks and likely validation commands.\n\
         Mergen ADE will save the plan at: {}\n\n\
         User prompt:\n{}",
        plan_path.display(),
        original_prompt.trim()
    )
}

pub fn build_codex_review_prompt(plan_path: &Path, test_summary: &str, review_round: u8) -> String {
    format!(
        "You are the read-only review hook after Claude Code implementation.\n\
         Review the current uncommitted workspace changes against the plan at {}.\n\
         You may inspect files and diffs only. Do not edit files.\n\
         Report only real, actionable findings with severity P0, P1, P2, or P3.\n\
         If there are no actionable P0-P3 findings, respond exactly with NO_FINDINGS.\n\
         This is review pass {}. At most {} review-fix remediation rounds may be requested.\n\n\
         Validation summary:\n{}",
        plan_path.display(),
        review_round,
        MAX_REVIEW_FIX_ROUNDS,
        test_summary.trim()
    )
}

pub fn codex_exec_args(project_path: &Path) -> Vec<OsString> {
    vec![
        OsString::from("--ask-for-approval"),
        OsString::from("never"),
        OsString::from("exec"),
        OsString::from("--skip-git-repo-check"),
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--cd"),
        project_path.as_os_str().to_owned(),
        OsString::from("-"),
    ]
}

pub fn run_codex_exec(project_path: &Path, prompt: &str) -> Result<String, String> {
    let codex_program = codex_program();
    let mut command = Command::new(&codex_program);
    command.args(codex_exec_args(project_path));
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    hide_command_window(&mut command);

    let mut child = command.spawn().map_err(|err| {
        format!(
            "Failed to start Codex CLI at {}: {err}",
            PathBuf::from(&codex_program).display()
        )
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|err| format!("Failed to write Codex prompt: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("Failed to wait for Codex CLI: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if output.status.success() {
        if stdout.is_empty() && !stderr.is_empty() {
            Ok(stderr)
        } else {
            Ok(stdout)
        }
    } else {
        Err(format!(
            "Codex CLI exited with {}.\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        ))
    }
}

#[cfg(target_os = "windows")]
fn codex_program() -> OsString {
    codex_program_from_appdata(std::env::var_os("APPDATA"))
}

#[cfg(target_os = "windows")]
fn codex_program_from_appdata(appdata: Option<OsString>) -> OsString {
    appdata
        .map(PathBuf::from)
        .map(|path| path.join("npm").join("codex.cmd"))
        .filter(|path| path.is_file())
        .map(PathBuf::into_os_string)
        .unwrap_or_else(|| OsString::from("codex.cmd"))
}

#[cfg(not(target_os = "windows"))]
fn codex_program() -> OsString {
    OsString::from("codex")
}

pub fn discover_test_commands(project_path: &Path) -> Vec<TestCommand> {
    let mut commands = Vec::new();
    if project_path.join("Cargo.toml").is_file() {
        commands.push(TestCommand::new(
            "cargo test",
            cargo_program(),
            vec![OsString::from("test")],
        ));
    }

    let package_json = project_path.join("package.json");
    if let Ok(text) = fs::read_to_string(package_json) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            if let Some(scripts) = value.get("scripts").and_then(Value::as_object) {
                for script in ["lint", "typecheck", "test"] {
                    if scripts.contains_key(script) {
                        let label = if script == "test" {
                            "npm test".to_owned()
                        } else {
                            format!("npm run {script}")
                        };
                        let args = if script == "test" {
                            vec![OsString::from("test")]
                        } else {
                            vec![OsString::from("run"), OsString::from(script)]
                        };
                        commands.push(TestCommand::new(label, npm_program(), args));
                    }
                }
            }
        }
    }

    commands
}

pub fn run_test_commands(project_path: &Path, commands: &[TestCommand]) -> Vec<TestCommandResult> {
    commands
        .iter()
        .map(|test_command| run_test_command(project_path, test_command))
        .collect()
}

fn run_test_command(project_path: &Path, test_command: &TestCommand) -> TestCommandResult {
    let mut command = Command::new(&test_command.program);
    command.args(&test_command.args);
    command.current_dir(project_path);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    hide_command_window(&mut command);

    match command.output() {
        Ok(output) => TestCommandResult {
            label: test_command.label.clone(),
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            error: None,
        },
        Err(err) => TestCommandResult {
            label: test_command.label.clone(),
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(err.to_string()),
        },
    }
}

pub fn test_summary(results: &[TestCommandResult], no_tests_note: Option<&str>) -> String {
    if results.is_empty() {
        return no_tests_note
            .unwrap_or("No test/lint/typecheck commands were detected.")
            .to_owned();
    }

    let mut summary = String::new();
    for result in results {
        summary.push_str(&format!(
            "- {}: {}\n",
            result.label,
            if result.success { "passed" } else { "failed" }
        ));
        if let Some(error) = &result.error {
            summary.push_str(&format!("  error: {}\n", error));
        }
    }
    summary
}

pub fn any_test_failed(results: &[TestCommandResult]) -> bool {
    results.iter().any(|result| !result.success)
}

pub fn review_has_actionable_findings(output: &str) -> bool {
    let normalized = output.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized == "no_findings"
        || normalized == "no findings"
        || normalized.contains("no actionable")
        || normalized.contains("no p0")
    {
        return false;
    }

    ["p0", "p1", "p2", "p3"]
        .iter()
        .any(|severity| contains_severity_token(&normalized, severity))
}

fn contains_severity_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(idx, _)| {
        let before = idx
            .checked_sub(1)
            .and_then(|i| text.as_bytes().get(i))
            .copied();
        let after = text.as_bytes().get(idx + token.len()).copied();
        !before.is_some_and(|ch| ch.is_ascii_alphanumeric())
            && !after.is_some_and(|ch| ch.is_ascii_alphanumeric())
    })
}

pub fn detect_ui_changed_files(project_path: &Path) -> Vec<String> {
    let mut files = git_changed_files(project_path);
    files.sort();
    files.dedup();
    files
        .into_iter()
        .filter(|path| is_ui_facing_path(path))
        .collect()
}

fn git_changed_files(project_path: &Path) -> Vec<String> {
    let mut files = Vec::new();
    files.extend(git_output_lines(
        project_path,
        &["diff", "--name-only", "HEAD", "--"],
    ));
    files.extend(git_output_lines(
        project_path,
        &["ls-files", "--others", "--exclude-standard"],
    ));
    files
}

fn git_output_lines(project_path: &Path, args: &[&str]) -> Vec<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(project_path).args(args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::null());
    hide_command_window(&mut command);
    let Ok(output) = command.output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn is_ui_facing_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if normalized.ends_with("src/app.rs")
        || normalized.contains("/ui/")
        || normalized.contains("/components/")
        || normalized.contains("/pages/")
        || normalized.contains("/views/")
    {
        return true;
    }

    Path::new(&normalized)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext,
                "css"
                    | "scss"
                    | "sass"
                    | "html"
                    | "htm"
                    | "jsx"
                    | "tsx"
                    | "vue"
                    | "svelte"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "webp"
                    | "gif"
                    | "svg"
            )
        })
}

#[cfg(target_os = "windows")]
fn cargo_program() -> OsString {
    std::env::var_os("USERPROFILE")
        .map(|home| {
            PathBuf::from(home)
                .join(".cargo")
                .join("bin")
                .join("cargo.exe")
        })
        .filter(|path| path.is_file())
        .map(PathBuf::into_os_string)
        .unwrap_or_else(|| OsString::from("cargo.exe"))
}

#[cfg(not(target_os = "windows"))]
fn cargo_program() -> OsString {
    OsString::from("cargo")
}

#[cfg(target_os = "windows")]
fn npm_program() -> OsString {
    OsString::from("npm.cmd")
}

#[cfg(not(target_os = "windows"))]
fn npm_program() -> OsString {
    OsString::from("npm")
}

#[cfg(target_os = "windows")]
fn hide_command_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_command_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_status_serializes_expected_status_names() {
        assert_eq!(PlanStatus::Planned.as_str(), "planned");
        assert_eq!(PlanStatus::Implementing.as_str(), "implementing");
        assert_eq!(PlanStatus::Testing.as_str(), "testing");
        assert_eq!(PlanStatus::Reviewing.as_str(), "reviewing");
        assert_eq!(PlanStatus::Fixing.as_str(), "fixing");
        assert_eq!(PlanStatus::Done.as_str(), "done");
        assert_eq!(PlanStatus::Blocked.as_str(), "blocked");
    }

    #[test]
    fn session_ids_include_terminal_and_counter() {
        let first = session_id(7, 1);
        let second = session_id(7, 2);
        assert_ne!(first, second);
        assert!(first.starts_with("mergen-7-"));
    }

    #[test]
    fn codex_exec_args_are_read_only_and_non_interactive() {
        let args = codex_exec_args(Path::new("C:/repo"))
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--sandbox" && pair[1] == "read-only"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--ask-for-approval" && pair[1] == "never"));
        let approval_index = args
            .iter()
            .position(|arg| arg == "--ask-for-approval")
            .expect("approval flag");
        let exec_index = args.iter().position(|arg| arg == "exec").expect("exec arg");
        assert!(
            approval_index < exec_index,
            "approval flag is a global Codex option and must precede the exec subcommand"
        );
        assert!(
            args.contains(&"--skip-git-repo-check".to_owned()),
            "planning/review hooks must also work for non-git project folders"
        );
        assert!(args.contains(&"-".to_owned()));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn codex_program_prefers_appdata_npm_codex_cmd_on_windows() {
        let root = std::env::temp_dir().join(format!(
            "mergen-codex-program-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let npm_dir = root.join("npm");
        fs::create_dir_all(&npm_dir).unwrap();
        let expected = npm_dir.join("codex.cmd");
        fs::write(&expected, "@echo off\r\n").unwrap();

        let actual = codex_program_from_appdata(Some(root.as_os_str().to_owned()));
        let _ = fs::remove_dir_all(root);

        assert_eq!(PathBuf::from(actual), expected);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn codex_program_falls_back_to_codex_cmd_on_windows() {
        assert_eq!(
            codex_program_from_appdata(None),
            OsString::from("codex.cmd")
        );
    }

    #[test]
    fn review_parser_accepts_only_actionable_severity_tokens() {
        assert!(!review_has_actionable_findings("NO_FINDINGS"));
        assert!(!review_has_actionable_findings("No actionable findings."));
        assert!(review_has_actionable_findings(
            "P2 src/app.rs:123 - concrete regression"
        ));
        assert!(!review_has_actionable_findings(
            "The string p25 is not a valid severity"
        ));
    }

    #[test]
    fn ui_detection_matches_frontend_paths_and_assets() {
        assert!(is_ui_facing_path("src/app.rs"));
        assert!(is_ui_facing_path("web/components/Button.tsx"));
        assert!(is_ui_facing_path("style.css"));
        assert!(is_ui_facing_path("assets/logo.svg"));
        assert!(!is_ui_facing_path("src/config.rs"));
    }

    #[test]
    fn discover_test_commands_reads_cargo_and_package_scripts() {
        let root = std::env::temp_dir().join(format!(
            "mergen-hook-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='x'\nversion='0.1.0'\n",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"lint":"eslint .","typecheck":"tsc","test":"vitest"}}"#,
        )
        .unwrap();

        let labels = discover_test_commands(&root)
            .into_iter()
            .map(|command| command.label)
            .collect::<Vec<_>>();
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            labels,
            vec![
                "cargo test",
                "npm run lint",
                "npm run typecheck",
                "npm test"
            ]
        );
    }
}
