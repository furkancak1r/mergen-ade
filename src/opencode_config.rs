use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde_json::{json, Value as JsonValue};

use crate::browser_mcp_service::{
    BrowserMcpEndpointEnv, DEFAULT_BROWSER_MCP_TIMEOUT_MS, MERGEN_BROWSER_MCP_HELPER_ARG,
    MERGEN_BROWSER_MCP_PORT_ENV_VAR, MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR,
    MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR, MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR,
    MERGEN_BROWSER_MCP_TOKEN_ENV_VAR,
};

/// Returns the global OpenCode config path (~/.config/opencode/opencode.json).
pub fn global_opencode_config_path() -> io::Result<PathBuf> {
    let base_dirs = BaseDirs::new().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User home directory is unavailable",
        )
    })?;
    Ok(base_dirs
        .home_dir()
        .join(".config")
        .join("opencode")
        .join("opencode.json"))
}

/// Patches the global OpenCode config to set the build agent model.
/// Preserves all other configuration including provider, tools, modes, etc.
/// Uses JSONC-safe parsing (strips comments before parsing if needed).
pub fn patch_global_opencode_config(build_model: &str) -> io::Result<OpenCodePatchOutcome> {
    let path = global_opencode_config_path()?;

    // Read existing or start with empty object
    let mut value = match fs::read_to_string(&path) {
        Ok(text) => {
            // Strip comments for JSONC compatibility before parsing
            let json_text = strip_jsonc_comments(&text);
            serde_json::from_str::<JsonValue>(&json_text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            // Create minimal config
            json!({})
        }
        Err(err) => return Err(err),
    };

    // Ensure root is an object
    if !value.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "OpenCode config root must be a JSON object",
        ));
    }

    // Navigate/create the agent.build.model path used by current OpenCode.
    let root = value.as_object_mut().unwrap();

    let agent = root
        .entry("agent")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "agent must be an object"))?;

    let build = agent
        .entry("build")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "agent.build must be an object")
        })?;

    let previous_model = build.get("model").cloned();
    build.insert("model".to_owned(), json!(build_model));

    let agent_changed = previous_model.as_ref().and_then(|v| v.as_str()) != Some(build_model);

    // Also set mode.build.model so build mode reads the same model.
    let mode = root
        .entry("mode")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode must be an object"))?;

    let mode_build = mode
        .entry("build")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "mode.build must be an object")
        })?;

    let previous_mode_model = mode_build.get("model").cloned();
    mode_build.insert("model".to_owned(), json!(build_model));

    let mode_changed = previous_mode_model.as_ref().and_then(|v| v.as_str()) != Some(build_model);

    let changed = agent_changed || mode_changed;

    // Write back with pretty formatting
    let rendered = serde_json::to_string_pretty(&value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, rendered)?;

    if changed {
        Ok(OpenCodePatchOutcome::Updated)
    } else {
        Ok(OpenCodePatchOutcome::Unchanged)
    }
}

/// Outcome of patching the OpenCode config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenCodePatchOutcome {
    /// Config was updated.
    Updated,
    /// Config was already correct.
    Unchanged,
}

/// Writes the runtime OpenCode config for a specific terminal.
/// This goes into the Mergen-managed runtime directory (OPENCODE_CONFIG_DIR).
pub fn write_terminal_runtime_config(
    runtime_dir: &Path,
    terminal_id: u64,
    build_model: &str,
) -> io::Result<PathBuf> {
    write_terminal_runtime_config_with_browser_mcp(runtime_dir, terminal_id, build_model, None)
}

pub const MERGEN_BROWSER_MCP_SERVER_NAME: &str = "mergen-browser";
const GLOBAL_PLAYWRIGHT_MCP_SERVER_NAME: &str = "mcp-server-playwright";

pub fn write_terminal_runtime_config_with_browser_mcp(
    runtime_dir: &Path,
    terminal_id: u64,
    build_model: &str,
    browser_mcp: Option<(&Path, BrowserMcpEndpointEnv)>,
) -> io::Result<PathBuf> {
    let config_dir = runtime_dir.join("hooks").join(terminal_id.to_string());
    fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("opencode.json");

    let mut config = json!({
        "$schema": "https://opencode.ai/config.json",
        "agent": {
            "build": {
                "model": build_model
            }
        },
        "mode": {
            "build": {
                "model": build_model
            }
        }
    });

    if let Some((helper_path, endpoint)) = browser_mcp {
        let mut environment = json!({
            MERGEN_BROWSER_MCP_PORT_ENV_VAR: endpoint.port.to_string(),
            MERGEN_BROWSER_MCP_TOKEN_ENV_VAR: endpoint.token,
            MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR: endpoint.terminal_id.to_string()
        });
        if let Some(project_id) = endpoint.project_id {
            environment[MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR] = json!(project_id.to_string());
        }
        if let Some(session_id) = &endpoint.session_id {
            environment[MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR] = json!(session_id);
        }
        let mut mcp_servers = serde_json::Map::new();
        mcp_servers.insert(
            MERGEN_BROWSER_MCP_SERVER_NAME.to_owned(),
            json!({
                "type": "local",
                "enabled": true,
                "timeout": DEFAULT_BROWSER_MCP_TIMEOUT_MS,
                "command": [
                    helper_path.to_string_lossy().to_string(),
                    MERGEN_BROWSER_MCP_HELPER_ARG,
                    "--caps=devtools,vision,network,storage"
                ],
                "environment": environment
            }),
        );
        mcp_servers.insert(
            GLOBAL_PLAYWRIGHT_MCP_SERVER_NAME.to_owned(),
            json!({
                "type": "local",
                "enabled": false,
                "timeout": DEFAULT_BROWSER_MCP_TIMEOUT_MS,
                "command": [
                    "npx",
                    "-y",
                    "@playwright/mcp@latest"
                ]
            }),
        );
        // Also disable other common external browser MCP server names so OpenCode
        // cannot silently fall back to launching its own Chrome/Playwright.
        for disabled_name in ["playwright", "browser", "puppeteer"] {
            mcp_servers.insert(
                disabled_name.to_owned(),
                json!({
                    "type": "local",
                    "enabled": false,
                }),
            );
        }
        config["mcp"] = JsonValue::Object(mcp_servers);

        let mut permissions = serde_json::Map::new();
        permissions.insert(MERGEN_BROWSER_MCP_SERVER_NAME.to_owned(), json!("allow"));
        for tool_name in BROWSER_MCP_PERMISSION_TOOL_NAMES {
            permissions.insert(
                format!("{MERGEN_BROWSER_MCP_SERVER_NAME}_{tool_name}"),
                json!("allow"),
            );
        }
        config["permission"] = JsonValue::Object(permissions.clone());
        let tools = permissions
            .into_iter()
            .map(|(key, _)| (key, json!(true)))
            .collect::<serde_json::Map<_, _>>();
        config["tools"] = JsonValue::Object(tools);
    }

    let rendered = serde_json::to_string_pretty(&config)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    fs::write(&config_path, rendered)?;

    log::debug!(
        "Wrote OpenCode runtime config for terminal {} with model {} to {}",
        terminal_id,
        build_model,
        config_path.display()
    );

    Ok(config_dir)
}

const BROWSER_MCP_PERMISSION_TOOL_NAMES: &[&str] = &[
    "browser_close",
    "browser_resize",
    "browser_console_messages",
    "browser_snapshot",
    "browser_page_summary",
    "browser_click",
    "browser_drag",
    "browser_hover",
    "browser_select_option",
    "browser_evaluate",
    "browser_fill_form",
    "browser_press_key",
    "browser_type",
    "browser_navigate",
    "browser_navigate_back",
    "browser_navigate_forward",
    "browser_reload",
    "browser_network_requests",
    "browser_network_request",
    "browser_take_screenshot",
    "browser_start_video",
    "browser_stop_video",
    "browser_video_chapter",
    "browser_tabs",
    "browser_wait_for",
    "browser_highlight",
    "browser_hide_highlight",
    "browser_localstorage_list",
    "browser_localstorage_get",
    "browser_localstorage_set",
    "browser_localstorage_delete",
    "browser_localstorage_clear",
    "browser_sessionstorage_list",
    "browser_sessionstorage_get",
    "browser_sessionstorage_set",
    "browser_sessionstorage_delete",
    "browser_sessionstorage_clear",
    "browser_cookie_list",
    "browser_cookie_get",
    "browser_cookie_set",
    "browser_cookie_delete",
    "browser_cookie_clear",
];

/// Strips JSONC-style comments from JSON text to make it valid JSON.
/// Handles both // line comments and /* block comments */.
fn strip_jsonc_comments(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' {
            match chars.peek() {
                // Line comment //
                Some(&'/') => {
                    chars.next(); // consume second /
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            result.push('\n'); // keep newlines for line numbers
                            break;
                        }
                    }
                }
                // Block comment /*
                Some(&'*') => {
                    chars.next(); // consume *
                    let mut prev = ' ';
                    while let Some(c) = chars.next() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        prev = c;
                    }
                }
                _ => {
                    result.push(ch);
                }
            }
        } else if ch == '"' {
            // String literal - copy verbatim including escaped quotes
            result.push(ch);
            while let Some(c) = chars.next() {
                result.push(c);
                if c == '\\' {
                    // Escaped character - include next char too
                    if let Some(next) = chars.next() {
                        result.push(next);
                    }
                } else if c == '"' {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn strip_jsonc_comments_removes_line_comments() {
        let input = r#"{
            // this is a comment
            "model": "gpt-4"
        }"#;
        let result = strip_jsonc_comments(input);
        assert!(!result.contains("//"));
        assert!(result.contains("\"model\""));
        assert!(result.contains("\"gpt-4\""));
    }

    #[test]
    fn strip_jsonc_comments_removes_block_comments() {
        let input = r#"{
            /* multi
               line
               comment */
            "model": "gpt-4"
        }"#;
        let result = strip_jsonc_comments(input);
        assert!(!result.contains("/*"));
        assert!(!result.contains("*/"));
        assert!(result.contains("\"model\""));
        assert!(result.contains("\"gpt-4\""));
    }

    #[test]
    fn strip_jsonc_comments_preserves_strings_with_slashes() {
        let input = r#"{"path": "C:/users/me//file"}"#;
        let result = strip_jsonc_comments(input);
        assert!(result.contains("C:/users/me//file"));
    }

    #[test]
    fn write_terminal_runtime_config_creates_correct_structure() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mergen-opencode-runtime-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let config_dir =
            write_terminal_runtime_config(&temp_dir, 42, "fireworks-ai/k2-turbo").unwrap();

        let config_path = config_dir.join("opencode.json");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();

        assert_eq!(
            parsed["agent"]["build"]["model"].as_str(),
            Some("fireworks-ai/k2-turbo")
        );
        assert_eq!(
            parsed["mode"]["build"]["model"].as_str(),
            Some("fireworks-ai/k2-turbo")
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_terminal_runtime_config_with_browser_mcp_uses_mergen_browser_name_and_disables_global_playwright(
    ) {
        let temp_dir = std::env::temp_dir().join(format!(
            "mergen-opencode-browser-mcp-runtime-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let helper_path = temp_dir.join("mergen-ade.exe");
        let endpoint = BrowserMcpEndpointEnv {
            port: 43210,
            token: "test-token".to_owned(),
            terminal_id: 42,
            project_id: Some(7),
            session_id: None,
            acp_chat_id: None,
        };

        let config_dir = write_terminal_runtime_config_with_browser_mcp(
            &temp_dir,
            42,
            "fireworks-ai/k2-turbo",
            Some((helper_path.as_path(), endpoint)),
        )
        .unwrap();

        let content = fs::read_to_string(config_dir.join("opencode.json")).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        let mcp = &parsed["mcp"][MERGEN_BROWSER_MCP_SERVER_NAME];
        let global_playwright = &parsed["mcp"][GLOBAL_PLAYWRIGHT_MCP_SERVER_NAME];

        assert_eq!(
            parsed["agent"]["build"]["model"].as_str(),
            Some("fireworks-ai/k2-turbo")
        );
        assert_eq!(
            parsed["mode"]["build"]["model"].as_str(),
            Some("fireworks-ai/k2-turbo")
        );
        assert_eq!(mcp["type"].as_str(), Some("local"));
        assert_eq!(mcp["enabled"].as_bool(), Some(true));
        assert_eq!(
            mcp["command"][0].as_str(),
            Some(helper_path.to_str().unwrap())
        );
        assert_eq!(
            mcp["command"][1].as_str(),
            Some(MERGEN_BROWSER_MCP_HELPER_ARG)
        );
        assert_eq!(
            mcp["command"][2].as_str(),
            Some("--caps=devtools,vision,network,storage")
        );
        assert_eq!(
            mcp["environment"][MERGEN_BROWSER_MCP_PORT_ENV_VAR].as_str(),
            Some("43210")
        );
        assert_eq!(
            mcp["environment"][MERGEN_BROWSER_MCP_TOKEN_ENV_VAR].as_str(),
            Some("test-token")
        );
        assert_eq!(
            mcp["environment"][MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR].as_str(),
            Some("42")
        );
        assert_eq!(
            mcp["environment"][MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR].as_str(),
            Some("7")
        );
        assert_eq!(global_playwright["enabled"].as_bool(), Some(false));
        assert_eq!(
            parsed["permission"]["mergen-browser_browser_navigate"].as_str(),
            Some("allow")
        );
        assert_eq!(
            parsed["permission"]["mergen-browser_browser_page_summary"].as_str(),
            Some("allow")
        );
        assert_eq!(
            parsed["permission"]["mergen-browser"].as_str(),
            Some("allow")
        );
        assert_eq!(
            parsed["tools"]["mergen-browser_browser_navigate"].as_bool(),
            Some(true)
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_terminal_runtime_config_uses_main_exe_with_helper_arg_not_sidecar() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mergen-single-binary-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Simulate main executable path (not a sidecar)
        let main_exe_path = temp_dir.join("mergen-ade.exe");
        let endpoint = BrowserMcpEndpointEnv {
            port: 54321,
            token: "single-binary-token".to_owned(),
            terminal_id: 99,
            project_id: Some(1),
            session_id: None,
            acp_chat_id: None,
        };

        let config_dir = write_terminal_runtime_config_with_browser_mcp(
            &temp_dir,
            99,
            "test-model",
            Some((main_exe_path.as_path(), endpoint)),
        )
        .unwrap();

        let content = fs::read_to_string(config_dir.join("opencode.json")).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed["mode"]["build"]["model"].as_str(),
            Some("test-model")
        );
        let mcp = &parsed["mcp"][MERGEN_BROWSER_MCP_SERVER_NAME];
        let command = mcp["command"].as_array().unwrap();

        // CRITICAL: Command must use main exe + --browser-mcp-helper, not sidecar
        assert_eq!(
            command[0].as_str(),
            Some(main_exe_path.to_str().unwrap()),
            "MCP command should use main executable, not sidecar"
        );
        assert_eq!(
            command[1].as_str(),
            Some(MERGEN_BROWSER_MCP_HELPER_ARG),
            "MCP command should include --browser-mcp-helper argument"
        );

        // Ensure no stale sidecar executable name is used
        // Old sidecar was named "mergen-browser-mcp.exe" - must not be referenced
        let command_exe = std::path::Path::new(command[0].as_str().unwrap())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        assert_ne!(
            command_exe, "mergen-browser-mcp.exe",
            "Command must NOT use old sidecar executable 'mergen-browser-mcp.exe'"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_terminal_runtime_config_disables_external_browser_mcp_servers() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mergen-opencode-browser-mcp-disabled-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let helper_path = temp_dir.join("mergen-ade.exe");
        let endpoint = BrowserMcpEndpointEnv {
            port: 54321,
            token: "test-token".to_owned(),
            terminal_id: 99,
            project_id: Some(1),
            session_id: None,
            acp_chat_id: None,
        };

        let config_dir = write_terminal_runtime_config_with_browser_mcp(
            &temp_dir,
            99,
            "test-model",
            Some((helper_path.as_path(), endpoint)),
        )
        .unwrap();

        let content = fs::read_to_string(config_dir.join("opencode.json")).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        let mcp = parsed["mcp"].as_object().unwrap();

        // Our own server must be enabled
        assert_eq!(
            mcp[MERGEN_BROWSER_MCP_SERVER_NAME]["enabled"].as_bool(),
            Some(true)
        );

        // External browser MCP servers must be disabled
        for server_name in ["playwright", "browser", "puppeteer"] {
            assert_eq!(
                mcp[server_name]["enabled"].as_bool(),
                Some(false),
                "External browser MCP server '{server_name}' must be disabled in runtime config"
            );
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
