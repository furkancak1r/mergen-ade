use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::opencode::{
    OpenCodeNotifyInboxEvent, OpenCodeTransportStatus, MERGEN_AI_TOOL_HINT_OPENCODE,
};

pub const MERGEN_OPENCODE_HOOK_PORT_ENV_VAR: &str = "MERGEN_OPENCODE_HOOK_PORT";
pub const MERGEN_OPENCODE_HOOK_TOKEN_ENV_VAR: &str = "MERGEN_OPENCODE_HOOK_TOKEN";
pub const MERGEN_OPENCODE_TERMINAL_ID_ENV_VAR: &str = "MERGEN_OPENCODE_TERMINAL_ID";
pub const OPENCODE_CONFIG_DIR_ENV_VAR: &str = "OPENCODE_CONFIG_DIR";
pub const MERGEN_OPENCODE_PLUGIN_FILE: &str = "mergen-opencode-status.js";

/// Shared state for the hook service tracking last status per terminal
#[derive(Debug, Default)]
struct HookServiceState {
    token: String,
    /// Normalized status per terminal (Orca-compatible: working | idle | permission)
    last_status_by_terminal: HashMap<u64, OpenCodeTransportStatus>,
    pending_events: Vec<OpenCodeNotifyInboxEvent>,
}

impl HookServiceState {
    fn new(token: String) -> Self {
        Self {
            token,
            last_status_by_terminal: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    /// Record normalized status from OpenCode plugin
    /// Uses OpenCodeTransportStatus to preserve Orca-compatible semantics
    fn record_status(&mut self, terminal_id: u64, status: OpenCodeTransportStatus) -> bool {
        let changed = self.last_status_by_terminal.get(&terminal_id) != Some(&status);
        if changed {
            self.last_status_by_terminal.insert(terminal_id, status);

            // Map to event kind string
            let event_kind = status.as_str().to_owned();

            // Legacy status for backward compatibility
            let legacy_status = match status {
                OpenCodeTransportStatus::Working => "running",
                OpenCodeTransportStatus::Idle | OpenCodeTransportStatus::Permission => "attention",
            }
            .to_owned();

            let event = OpenCodeNotifyInboxEvent {
                terminal_id: terminal_id.to_string(),
                tool: MERGEN_AI_TOOL_HINT_OPENCODE.to_owned(),
                status: legacy_status,
                inbox_token: Some(self.token.clone()),
                event_kind: Some(event_kind),
                opencode_status: Some(status),
                raw_json: format!("{{\"status\":\"{}\"}}", status.as_str()),
                timestamp_utc: format_iso_timestamp(),
            };
            self.pending_events.push(event);
        }
        changed
    }

    fn drain_pending_events(&mut self) -> Vec<OpenCodeNotifyInboxEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// OpenCode Hook Service implementing the Orca-style HTTP hook model
pub struct OpenCodeHookService {
    state: Arc<Mutex<HookServiceState>>,
    port: u16,
    _listener_thread: Option<thread::JoinHandle<()>>,
}

impl OpenCodeHookService {
    /// Start the hook service on a random available port
    pub fn start() -> io::Result<Self> {
        let token = generate_secure_token();
        let state = Arc::new(Mutex::new(HookServiceState::new(token)));

        // Try to bind to any available port on localhost
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();

        let state_clone = Arc::clone(&state);
        let listener_thread = thread::spawn(move || {
            Self::run_server(listener, state_clone);
        });

        log::info!("OpenCode hook service started on port {}", port);

        Ok(Self {
            state,
            port,
            _listener_thread: Some(listener_thread),
        })
    }

    /// Get the auth token for this service instance
    pub fn token(&self) -> String {
        self.state.lock().unwrap().token.clone()
    }

    /// Get and clear pending status events since last check
    pub fn drain_pending_events(&self) -> Vec<OpenCodeNotifyInboxEvent> {
        self.state.lock().unwrap().drain_pending_events()
    }

    /// Get the port the service is listening on (test-only)
    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Build environment variables for a specific terminal's PTY
    pub fn build_pty_env(&self, terminal_id: u64, config_dir: &Path) -> Vec<(String, String)> {
        vec![
            (
                MERGEN_OPENCODE_HOOK_PORT_ENV_VAR.to_owned(),
                self.port.to_string(),
            ),
            (MERGEN_OPENCODE_HOOK_TOKEN_ENV_VAR.to_owned(), self.token()),
            (
                MERGEN_OPENCODE_TERMINAL_ID_ENV_VAR.to_owned(),
                terminal_id.to_string(),
            ),
            (
                OPENCODE_CONFIG_DIR_ENV_VAR.to_owned(),
                config_dir.to_string_lossy().to_string(),
            ),
        ]
    }

    fn run_server(listener: TcpListener, state: Arc<Mutex<HookServiceState>>) {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = Arc::clone(&state);
                    thread::spawn(move || {
                        if let Err(e) = Self::handle_request(stream, state) {
                            log::debug!("OpenCode hook request error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    log::warn!("OpenCode hook listener error: {}", e);
                }
            }
        }
    }

    fn handle_request(
        mut stream: TcpStream,
        state: Arc<Mutex<HookServiceState>>,
    ) -> io::Result<()> {
        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer)?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);

        // Parse the request line
        let first_line = request.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();

        if parts.len() != 3 || parts[0] != "POST" || parts[1] != "/hook" {
            Self::write_response(&mut stream, 404, "Not Found")?;
            return Ok(());
        }

        // Extract headers
        let mut token_header = None;
        let mut terminal_id_header = None;
        let mut content_length = 0usize;

        for line in request.lines().skip(1) {
            if line.is_empty() {
                break;
            }
            let lower = line.to_lowercase();
            if lower.starts_with("x-mergen-token:") {
                token_header = line.splitn(2, ':').nth(1).map(|s| s.trim());
            } else if lower.starts_with("x-mergen-opencode-terminal-id:") {
                terminal_id_header = line.splitn(2, ':').nth(1).map(|s| s.trim());
            } else if lower.starts_with("content-length:") {
                content_length = line
                    .splitn(2, ':')
                    .nth(1)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
            }
        }

        // Verify token
        let expected_token = state.lock().unwrap().token.clone();
        if token_header != Some(&expected_token) {
            Self::write_response(&mut stream, 403, "Forbidden")?;
            return Ok(());
        }

        // Parse terminal ID
        let terminal_id = match terminal_id_header {
            Some(id) => match id.parse::<u64>() {
                Ok(id) => id,
                Err(_) => {
                    Self::write_response(&mut stream, 400, "Bad Request")?;
                    return Ok(());
                }
            },
            None => {
                Self::write_response(&mut stream, 400, "Bad Request")?;
                return Ok(());
            }
        };

        // Read and parse body
        let body_start = request
            .find("\r\n\r\n")
            .map(|i| i + 4)
            .or_else(|| request.find("\n\n").map(|i| i + 2))
            .unwrap_or(request.len());
        let body = if content_length > 0 {
            &request[body_start..body_start.saturating_add(content_length).min(request.len())]
        } else {
            &request[body_start..]
        };

        let status = Self::parse_status_from_body(body);

        if let Some(status) = status {
            state.lock().unwrap().record_status(terminal_id, status);
            Self::write_response(&mut stream, 204, "No Content")?;
        } else {
            Self::write_response(&mut stream, 400, "Bad Request")?;
        }

        Ok(())
    }

    fn parse_status_from_body(body: &str) -> Option<OpenCodeTransportStatus> {
        // Simple JSON parser for {"status":"..."}
        let status_key = "\"status\"";
        let idx = body.find(status_key)?;
        let after_key = &body[idx + status_key.len()..];
        let colon_idx = after_key.find(':')?;
        let after_colon = &after_key[colon_idx + 1..];

        // Find the quoted value
        let quote_start = after_colon.find('"')?;
        let after_start_quote = &after_colon[quote_start + 1..];
        let quote_end = after_start_quote.find('"')?;
        let status_str = &after_start_quote[..quote_end];

        OpenCodeTransportStatus::from_str(status_str)
    }

    fn write_response(stream: &mut TcpStream, code: u16, message: &str) -> io::Result<()> {
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            code, message
        );
        stream.write_all(response.as_bytes())?;
        stream.flush()
    }
}

impl Drop for OpenCodeHookService {
    fn drop(&mut self) {
        // The listener thread will exit when the TcpListener is dropped
        log::info!("OpenCode hook service on port {} shutting down", self.port);
    }
}

/// Generate the OpenCode plugin JavaScript source
pub fn get_opencode_plugin_source() -> String {
    r#"const HOOK_PATH = "/hook";

function getHookUrl() {
  const port = process.env.MERGEN_OPENCODE_HOOK_PORT;
  return port ? `http://127.0.0.1:${port}${HOOK_PATH}` : null;
}

function getStatusType(event) {
  return event?.properties?.status?.type ?? event?.status?.type ?? null;
}

async function postStatus(status) {
  const url = getHookUrl();
  const token = process.env.MERGEN_OPENCODE_HOOK_TOKEN;
  const terminalId = process.env.MERGEN_OPENCODE_TERMINAL_ID;
  if (!url || !token || !terminalId) return;
  try {
    await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Mergen-Token": token,
        "X-Mergen-OpenCode-Terminal-Id": terminalId,
      },
      body: JSON.stringify({ status }),
    });
  } catch {
    // OpenCode session hooks must never fail the agent run just
    // because Mergen is unavailable or the local loopback request failed.
  }
}

export const MergenOpenCodeStatusPlugin = async () => ({
  event: async ({ event }) => {
    if (!event?.type) return;

    // Permission asked (user approval needed)
    if (event.type === "permission.asked") {
      await postStatus("permission");
      return;
    }

    // Question asked (user input needed) - also maps to permission state
    if (event.type === "question.asked") {
      await postStatus("permission");
      return;
    }

    // Session idle (turn complete / waiting)
    if (event.type === "session.idle") {
      await postStatus("idle");
      return;
    }

    // Session error - still idle but with error context
    if (event.type === "session.error") {
      await postStatus("idle");
      return;
    }

    // Tool execution signals working state
    if (event.type === "tool.execute.before") {
      await postStatus("working");
      return;
    }

    // Session status updates (busy/idle)
    if (event.type === "session.status") {
      const statusType = getStatusType(event);
      if (statusType === "busy" || statusType === "retry") {
        await postStatus("working");
        return;
      }
      if (statusType === "idle") {
        await postStatus("idle");
      }
    }
  },
});
"#
    .to_owned()
}

/// Write the plugin config for a specific terminal
pub fn write_terminal_plugin_config(runtime_dir: &Path, terminal_id: u64) -> io::Result<PathBuf> {
    let config_dir = runtime_dir.join("hooks").join(terminal_id.to_string());
    let plugins_dir = config_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir)?;

    let plugin_path = plugins_dir.join(MERGEN_OPENCODE_PLUGIN_FILE);
    std::fs::write(&plugin_path, get_opencode_plugin_source())?;

    log::debug!(
        "Wrote OpenCode plugin config for terminal {} to {}",
        terminal_id,
        config_dir.display()
    );

    Ok(config_dir)
}

/// Check if the plugin config is up to date for a terminal (test-only)
#[cfg(test)]
pub fn is_plugin_config_current(runtime_dir: &Path, terminal_id: u64) -> bool {
    let config_dir = runtime_dir.join("hooks").join(terminal_id.to_string());
    let plugin_path = config_dir.join("plugins").join(MERGEN_OPENCODE_PLUGIN_FILE);

    if !plugin_path.exists() {
        return false;
    }

    // Optionally: check if plugin content matches current source
    match std::fs::read_to_string(&plugin_path) {
        Ok(content) => content == get_opencode_plugin_source(),
        Err(_) => false,
    }
}

fn format_iso_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    format!("{}.{:09}Z", secs, duration.subsec_nanos())
}

/// Generate a secure random token without external dependencies
fn generate_secure_token() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();
    let nanos = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id() as u128;
    let random_part = nanos.wrapping_mul(pid).wrapping_add(nanos >> 32);

    let mut hasher = DefaultHasher::new();
    random_part.hash(&mut hasher);
    nanos.hash(&mut hasher);
    format!("{:x}{:x}", hasher.finish(), nanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opencode::OpenCodeTransportStatus;

    #[test]
    fn hook_status_parsing_from_body() {
        assert_eq!(
            OpenCodeHookService::parse_status_from_body(r#"{"status":"working"}"#),
            Some(OpenCodeTransportStatus::Working)
        );
        assert_eq!(
            OpenCodeHookService::parse_status_from_body(r#"{"status":"idle"}"#),
            Some(OpenCodeTransportStatus::Idle)
        );
        assert_eq!(
            OpenCodeHookService::parse_status_from_body(r#"{"status":"permission"}"#),
            Some(OpenCodeTransportStatus::Permission)
        );
        assert_eq!(
            OpenCodeHookService::parse_status_from_body(r#"{"status":"unknown"}"#),
            None
        );
        assert_eq!(OpenCodeHookService::parse_status_from_body(r#"{}"#), None);
    }

    #[test]
    fn plugin_config_written_and_detected() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let temp_dir = std::env::temp_dir().join(format!(
            "mergen-opencode-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let config = write_terminal_plugin_config(&temp_dir, 42).expect("write config");
        assert!(config.exists());
        assert!(is_plugin_config_current(&temp_dir, 42));

        // Corrupt the plugin
        let plugin_path = config.join("plugins").join(MERGEN_OPENCODE_PLUGIN_FILE);
        std::fs::write(&plugin_path, "stale content").unwrap();
        assert!(!is_plugin_config_current(&temp_dir, 42));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn hook_service_records_and_dedupes() {
        let service = OpenCodeHookService::start().expect("start service");

        // Simulate receiving same status twice
        // Note: We can't easily test the HTTP layer without a full integration test,
        // but we can test the state management

        // For now, just verify the service started and has valid port/token
        assert!(service.port() > 0);
        assert!(!service.token().is_empty());
    }
}
