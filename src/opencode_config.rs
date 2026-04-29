use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde_json::{json, Value as JsonValue};

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
    let config_dir = runtime_dir.join("hooks").join(terminal_id.to_string());
    fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("opencode.json");

    let config = json!({
        "mode": {
            "build": {
                "model": build_model
            }
        }
    });

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
}
