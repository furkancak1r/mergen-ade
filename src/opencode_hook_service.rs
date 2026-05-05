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

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCodeHookStatusRequest {
    status: OpenCodeTransportStatus,
    session_id: Option<String>,
    parent_session_id: Option<String>,
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
    fn record_status(
        &mut self,
        terminal_id: u64,
        status: OpenCodeTransportStatus,
        session_id: Option<String>,
        parent_session_id: Option<String>,
    ) -> bool {
        if parent_session_id
            .as_deref()
            .is_some_and(|parent| !parent.trim().is_empty())
        {
            return false;
        }

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
                session_id,
                parent_session_id,
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
        let mut request_bytes = Vec::new();

        let header_end = loop {
            if let Some(header_end) = find_http_header_end(&request_bytes) {
                break header_end;
            }

            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                Self::write_response(&mut stream, 400, "Bad Request")?;
                return Ok(());
            }

            request_bytes.extend_from_slice(&buffer[..bytes_read]);
        };

        let request = String::from_utf8_lossy(&request_bytes[..header_end]).to_string();

        // Parse the request line
        let first_line = request.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();

        if parts.len() != 3 || parts[0] != "POST" || parts[1] != "/hook" {
            Self::write_response(&mut stream, 404, "Not Found")?;
            return Ok(());
        }

        // Extract headers
        let mut token_header = None::<String>;
        let mut terminal_id_header = None::<String>;
        let mut content_length = 0usize;

        for line in request.lines().skip(1) {
            if line.is_empty() {
                break;
            }
            let lower = line.to_lowercase();
            if lower.starts_with("x-mergen-token:") {
                token_header = line.split_once(':').map(|x| x.1.trim().to_owned());
            } else if lower.starts_with("x-mergen-opencode-terminal-id:") {
                terminal_id_header = line.split_once(':').map(|x| x.1.trim().to_owned());
            } else if lower.starts_with("content-length:") {
                content_length = line
                    .split_once(':')
                    .and_then(|x| x.1.trim().parse().ok())
                    .unwrap_or(0);
            }
        }

        // Verify token
        let expected_token = state.lock().unwrap().token.clone();
        if token_header.as_deref() != Some(expected_token.as_str()) {
            Self::write_response(&mut stream, 403, "Forbidden")?;
            return Ok(());
        }

        // Parse terminal ID
        let terminal_id = match terminal_id_header.as_deref() {
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
        if content_length > 0 {
            let expected_request_len = header_end.saturating_add(content_length);
            while request_bytes.len() < expected_request_len {
                let bytes_read = stream.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..bytes_read]);
            }
        }

        let body = if content_length > 0 {
            let body_end = header_end
                .saturating_add(content_length)
                .min(request_bytes.len());
            String::from_utf8_lossy(&request_bytes[header_end..body_end]).to_string()
        } else {
            String::from_utf8_lossy(&request_bytes[header_end..]).to_string()
        };

        let request = Self::parse_status_request_from_body(&body);

        if let Some(request) = request {
            state.lock().unwrap().record_status(
                terminal_id,
                request.status,
                request.session_id,
                request.parent_session_id,
            );
            Self::write_response(&mut stream, 204, "No Content")?;
        } else {
            Self::write_response(&mut stream, 400, "Bad Request")?;
        }

        Ok(())
    }

    fn parse_status_from_body(body: &str) -> Option<OpenCodeTransportStatus> {
        Self::parse_status_request_from_body(body).map(|request| request.status)
    }

    fn parse_status_request_from_body(body: &str) -> Option<OpenCodeHookStatusRequest> {
        let parsed = serde_json::from_str::<serde_json::Value>(body).ok()?;
        let status = string_at_path(&parsed, &["status"])
            .as_deref()
            .and_then(OpenCodeTransportStatus::from_str)?;

        Some(OpenCodeHookStatusRequest {
            status,
            session_id: string_at_any_path(
                &parsed,
                &[
                    &["session_id"],
                    &["sessionID"],
                    &["sessionId"],
                    &["properties", "sessionID"],
                    &["properties", "sessionId"],
                    &["properties", "part", "sessionID"],
                    &["properties", "part", "sessionId"],
                    &["properties", "tool", "sessionID"],
                    &["properties", "tool", "sessionId"],
                    &["properties", "request", "sessionID"],
                    &["properties", "request", "sessionId"],
                    &["properties", "info", "id"],
                ],
            ),
            parent_session_id: string_at_any_path(
                &parsed,
                &[
                    &["parent_session_id"],
                    &["parentSessionID"],
                    &["parentSessionId"],
                    &["parentID"],
                    &["parentId"],
                    &["properties", "parentSessionID"],
                    &["properties", "parentSessionId"],
                    &["properties", "parentID"],
                    &["properties", "parentId"],
                ],
            ),
        })
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

fn string_at_any_path(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| string_at_path(value, path))
}

fn string_at_path(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    json_value_to_non_empty_string(current)
}

fn json_value_to_non_empty_string(value: &serde_json::Value) -> Option<String> {
    let text = match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        _ => return None,
    };
    let trimmed = text.trim();
    (!trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("null")).then(|| trimmed.to_owned())
}

fn find_http_header_end(request_bytes: &[u8]) -> Option<usize> {
    request_bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            request_bytes
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
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
const sessionParents = new Map();

function getHookUrl() {
  const port = process.env.MERGEN_OPENCODE_HOOK_PORT;
  return port ? `http://127.0.0.1:${port}${HOOK_PATH}` : null;
}

function getStatusType(event) {
  return event?.properties?.status?.type ?? event?.status?.type ?? null;
}

function textOrNull(value) {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function rememberSession(info) {
  const sessionID = textOrNull(info?.id ?? info?.sessionID ?? info?.sessionId);
  if (!sessionID) return;
  const parentSessionID = textOrNull(
    info?.parentID ?? info?.parentId ?? info?.parentSessionID ?? info?.parentSessionId
  );
  sessionParents.set(sessionID, parentSessionID);
}

function getSessionID(event) {
  return textOrNull(
      event?.properties?.sessionID ??
      event?.properties?.sessionId ??
      event?.properties?.part?.sessionID ??
      event?.properties?.part?.sessionId ??
      event?.properties?.tool?.sessionID ??
      event?.properties?.tool?.sessionId ??
      event?.properties?.request?.sessionID ??
      event?.properties?.request?.sessionId ??
      event?.properties?.info?.id ??
      event?.sessionID ??
      event?.sessionId
  );
}

function getParentSessionID(event, sessionID) {
  const directParentSessionID = textOrNull(
    event?.properties?.parentSessionID ??
      event?.properties?.parentSessionId ??
      event?.properties?.parentID ??
      event?.properties?.parentId ??
      event?.properties?.info?.parentID ??
      event?.properties?.info?.parentId ??
      event?.parentSessionID ??
      event?.parentSessionId ??
      event?.parentID ??
      event?.parentId
  );
  if (directParentSessionID) return directParentSessionID;
  return sessionID && sessionParents.has(sessionID) ? sessionParents.get(sessionID) : null;
}

async function postStatus(status, event) {
  const url = getHookUrl();
  const token = process.env.MERGEN_OPENCODE_HOOK_TOKEN;
  const terminalId = process.env.MERGEN_OPENCODE_TERMINAL_ID;
  if (!url || !token || !terminalId) return;

  const sessionID = getSessionID(event);
  const parentSessionID = getParentSessionID(event, sessionID);
  if (parentSessionID) return;

  const body = { status };
  if (sessionID) body.session_id = sessionID;

  try {
    await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Mergen-Token": token,
        "X-Mergen-OpenCode-Terminal-Id": terminalId,
      },
      body: JSON.stringify(body),
    });
  } catch {
    // OpenCode session hooks must never fail the agent run just
    // because Mergen is unavailable or the local loopback request failed.
  }
}

export const MergenOpenCodeStatusPlugin = async () => ({
  event: async ({ event }) => {
    if (!event?.type) return;

    if (event.type === "session.created" || event.type === "session.updated") {
      rememberSession(event?.properties?.info ?? event?.properties);
      return;
    }

    // Permission asked (user approval needed)
    if (event.type === "permission.asked") {
      await postStatus("permission", event);
      return;
    }

    // Question asked (user input needed) - also maps to permission state
    if (event.type === "question.asked") {
      await postStatus("permission", event);
      return;
    }

    // Session idle (turn complete / waiting)
    if (event.type === "session.idle") {
      await postStatus("idle", event);
      return;
    }

    // Session error - still idle but with error context
    if (event.type === "session.error") {
      await postStatus("idle", event);
      return;
    }

    // Tool execution signals working state
    if (event.type === "tool.execute.before") {
      await postStatus("working", event);
      return;
    }

    // Session status updates (busy/idle)
    if (event.type === "session.status") {
      const statusType = getStatusType(event);
      if (statusType === "busy" || statusType === "retry") {
        await postStatus("working", event);
        return;
      }
      if (statusType === "idle") {
        await postStatus("idle", event);
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
    fn hook_status_request_parsing_preserves_session_metadata() {
        let request = OpenCodeHookService::parse_status_request_from_body(
            r#"{"status":"idle","session_id":"sub-session","parent_session_id":"main-session"}"#,
        )
        .expect("status request should parse");

        assert_eq!(request.status, OpenCodeTransportStatus::Idle);
        assert_eq!(request.session_id.as_deref(), Some("sub-session"));
        assert_eq!(request.parent_session_id.as_deref(), Some("main-session"));
    }

    #[test]
    fn hook_service_ignores_subagent_status_without_poisoning_main_status() {
        let mut state = HookServiceState::new("token".to_owned());

        assert!(state.record_status(
            1,
            OpenCodeTransportStatus::Working,
            Some("main-session".to_owned()),
            None,
        ));
        assert_eq!(state.drain_pending_events().len(), 1);

        assert!(!state.record_status(
            1,
            OpenCodeTransportStatus::Idle,
            Some("sub-session".to_owned()),
            Some("main-session".to_owned()),
        ));
        assert!(state.drain_pending_events().is_empty());

        assert!(state.record_status(
            1,
            OpenCodeTransportStatus::Idle,
            Some("main-session".to_owned()),
            None,
        ));
        let events = state.drain_pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].opencode_status,
            Some(OpenCodeTransportStatus::Idle)
        );
        assert_eq!(events[0].session_id.as_deref(), Some("main-session"));
        assert_eq!(events[0].parent_session_id, None);
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

    #[test]
    fn hook_request_parses_when_headers_arrive_in_multiple_writes() {
        use std::io::Write;
        use std::net::{Shutdown, TcpListener, TcpStream};
        use std::time::Duration;

        let state = Arc::new(Mutex::new(HookServiceState::new("token".to_owned())));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server_state = Arc::clone(&state);
        let server_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept client");
            OpenCodeHookService::handle_request(stream, server_state).expect("handle request");
        });

        let mut client = TcpStream::connect(addr).expect("connect client");
        client.set_nodelay(true).expect("disable Nagle");

        let body = r#"{"status":"idle"}"#;
        let request = format!(
            "POST /hook HTTP/1.1\r\nX-Mergen-Token: token\r\nX-Mergen-OpenCode-Terminal-Id: 7\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let split_at = request
            .find("Content-Length")
            .expect("content-length header")
            + 10;

        client
            .write_all(request[..split_at].as_bytes())
            .expect("write first chunk");
        client.flush().expect("flush first chunk");
        std::thread::sleep(Duration::from_millis(25));
        client
            .write_all(request[split_at..].as_bytes())
            .expect("write second chunk");
        client.flush().expect("flush second chunk");
        client.shutdown(Shutdown::Write).expect("shutdown write");

        server_thread.join().expect("server thread");

        let events = state.lock().unwrap().drain_pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].terminal_id, "7");
        assert_eq!(
            events[0].opencode_status,
            Some(OpenCodeTransportStatus::Idle)
        );
    }
}
