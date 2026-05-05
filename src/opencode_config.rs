use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde_json::{json, Value as JsonValue};

use crate::browser_mcp_service::{
    BrowserMcpEndpointEnv, DEFAULT_BROWSER_MCP_TIMEOUT_MS, MERGEN_BROWSER_MCP_HELPER_ARG,
    MERGEN_BROWSER_MCP_PORT_ENV_VAR, MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR,
    MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR, MERGEN_BROWSER_MCP_TOKEN_ENV_VAR,
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

/// Patches the global OpenCode config to set the build mode model.
/// Preserves all other configuration including provider, tools, modes, etc.
/// Uses JSONC-safe parsing (strips comments before parsing if needed).
pub fn patch_global_opencode_config(build_model: &str) -> io::Result<OpenCodePatchOutcome> {
    let path = global_opencode_config_path()?;

    // Read existing or start with empty object
    let (existing_text, mut value) = match fs::read_to_string(&path) {
        Ok(text) => {
            // Strip comments for JSONC compatibility before parsing
            let json_text = strip_jsonc_comments(&text);
            let value = serde_json::from_str::<JsonValue>(&json_text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            (text, value)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            // Create minimal config
            (String::new(), json!({}))
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

    // Navigate/create the mode.build.model path
    let root = value.as_object_mut().unwrap();

    // Get or create mode
    let mode = root
        .entry("mode")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode must be an object"))?;

    // Get or create build
    let build = mode
        .entry("build")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "mode.build must be an object")
        })?;

    // Set model
    let previous_model = build.get("model").cloned();
    build.insert("model".to_owned(), json!(build_model));

    // Check if changed
    let changed = previous_model.as_ref().and_then(|v| v.as_str()) != Some(build_model)
        || !existing_text.trim().is_empty();

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
        config["mcp"] = json!({
            "mcp-server-playwright": {
                "type": "local",
                "enabled": true,
                "timeout": DEFAULT_BROWSER_MCP_TIMEOUT_MS,
                "command": [
                    helper_path.to_string_lossy().to_string(),
                    MERGEN_BROWSER_MCP_HELPER_ARG,
                    "--caps=devtools,vision,network,storage"
                ],
                "environment": environment
            }
        });
        let mut permissions = serde_json::Map::new();
        permissions.insert("mcp-server-playwright".to_owned(), json!("allow"));
        for tool_name in BROWSER_MCP_PERMISSION_TOOL_NAMES {
            permissions.insert(format!("mcp-server-playwright_{tool_name}"), json!("allow"));
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
    "browser_tabs",
    "browser_wait_for",
    "browser_highlight",
    "browser_hide_highlight",
    "browser_mouse_move_xy",
    "browser_mouse_click_xy",
    "browser_mouse_drag_xy",
    "browser_mouse_down",
    "browser_mouse_up",
    "browser_mouse_wheel",
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
            parsed["mode"]["build"]["model"].as_str(),
            Some("fireworks-ai/k2-turbo")
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_terminal_runtime_config_with_browser_mcp_overrides_playwright_mcp() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mergen-opencode-browser-mcp-runtime-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let helper_path = temp_dir.join("mergen-browser-mcp.exe");
        let endpoint = BrowserMcpEndpointEnv {
            port: 43210,
            token: "test-token".to_owned(),
            terminal_id: 42,
            project_id: Some(7),
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
        let mcp = &parsed["mcp"]["mcp-server-playwright"];

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
        assert_eq!(
            parsed["permission"]["mcp-server-playwright_browser_navigate"].as_str(),
            Some("allow")
        );
        assert_eq!(
            parsed["tools"]["mcp-server-playwright_browser_navigate"].as_bool(),
            Some(true)
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
