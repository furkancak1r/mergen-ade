use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

pub const MERGEN_BROWSER_MCP_PORT_ENV_VAR: &str = "MERGEN_BROWSER_MCP_PORT";
pub const MERGEN_BROWSER_MCP_TOKEN_ENV_VAR: &str = "MERGEN_BROWSER_MCP_TOKEN";
pub const MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR: &str = "MERGEN_BROWSER_MCP_TERMINAL_ID";
pub const MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR: &str = "MERGEN_BROWSER_MCP_PROJECT_ID";
/// Session ID to distinguish concurrent OpenCode/browser sessions for the same terminal/project.
pub const MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR: &str = "MERGEN_BROWSER_MCP_SESSION_ID";
pub const MERGEN_BROWSER_MCP_ENDPOINT_PATH: &str = "/browser-mcp";
/// CLI argument to run Browser MCP helper mode from the main executable.
pub const MERGEN_BROWSER_MCP_HELPER_ARG: &str = "--browser-mcp-helper";
pub const DEFAULT_BROWSER_MCP_TIMEOUT_MS: u64 = 90_000;
const MAX_BROWSER_MCP_HEADER_BYTES: usize = 64 * 1024;
const MAX_BROWSER_MCP_REQUEST_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserMcpIpcRequest {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub terminal_id: Option<u64>,
    #[serde(default)]
    pub project_id: Option<u64>,
    /// Session ID for multi-session isolation (concurrent OpenCode sessions).
    #[serde(default)]
    pub session_id: Option<String>,
    pub tool: String,
    #[serde(default)]
    pub params: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserMcpIpcResponse {
    pub text: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub data: Option<JsonValue>,
}

impl BrowserMcpIpcResponse {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            data: None,
        }
    }

    pub fn ok_with_data(text: impl Into<String>, data: JsonValue) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            data: Some(data),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            data: None,
        }
    }

    pub fn error_with_data(text: impl Into<String>, data: JsonValue) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            data: Some(data),
        }
    }
}

pub struct BrowserMcpCommand {
    pub request: BrowserMcpIpcRequest,
    pub auth_scope: BrowserMcpAuthScope,
    pub respond_to: mpsc::Sender<BrowserMcpIpcResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserMcpAuthScope {
    pub terminal_id: u64,
    pub project_id: Option<u64>,
    /// Session ID for multi-session isolation.
    pub session_id: Option<String>,
}

pub struct BrowserMcpService {
    command_rx: crossbeam_channel::Receiver<BrowserMcpCommand>,
    port: u16,
    token_registry: Arc<Mutex<BrowserMcpTokenRegistry>>,
    _listener_thread: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, Default)]
struct BrowserMcpTokenRegistry {
    scopes_by_token: BTreeMap<String, BrowserMcpAuthScope>,
    /// Key: (terminal_id, project_id, session_id) for multi-session isolation.
    token_by_scope: BTreeMap<(u64, Option<u64>, Option<String>), String>,
}

impl BrowserMcpTokenRegistry {
    fn token_for_scope(&mut self, scope: BrowserMcpAuthScope) -> String {
        let key = (
            scope.terminal_id,
            scope.project_id,
            scope.session_id.clone(),
        );
        if let Some(token) = self.token_by_scope.get(&key) {
            return token.clone();
        }

        let token = generate_token();
        self.scopes_by_token.insert(token.clone(), scope);
        self.token_by_scope.insert(key, token.clone());
        token
    }

    fn scope_for_token(&self, token: &str) -> Option<BrowserMcpAuthScope> {
        self.scopes_by_token.get(token).cloned()
    }

    fn revoke_terminal(&mut self, terminal_id: u64) {
        let revoked = self
            .token_by_scope
            .iter()
            .filter(|((tid, _, _), _)| *tid == terminal_id)
            .map(|(_, token)| token.clone())
            .collect::<Vec<_>>();
        self.token_by_scope
            .retain(|(tid, _, _), _| *tid != terminal_id);
        for token in revoked {
            self.scopes_by_token.remove(&token);
        }
    }

    fn revoke_project(&mut self, project_id: u64) {
        let revoked = self
            .token_by_scope
            .iter()
            .filter(|((_, pid, _), _)| *pid == Some(project_id))
            .map(|(_, token)| token.clone())
            .collect::<Vec<_>>();
        self.token_by_scope
            .retain(|(_, pid, _), _| *pid != Some(project_id));
        for token in revoked {
            self.scopes_by_token.remove(&token);
        }
    }

    /// Revoke all tokens for a specific session (e.g., when OpenCode restarts).
    fn revoke_session(&mut self, session_id: &str) {
        let revoked = self
            .token_by_scope
            .iter()
            .filter(|((_, _, sid), _)| sid.as_deref() == Some(session_id))
            .map(|(_, token)| token.clone())
            .collect::<Vec<_>>();
        self.token_by_scope
            .retain(|(_, _, sid), _| sid.as_deref() != Some(session_id));
        for token in revoked {
            self.scopes_by_token.remove(&token);
        }
    }
}

impl BrowserMcpService {
    pub fn start(repaint: Option<Arc<dyn Fn() + Send + Sync + 'static>>) -> io::Result<Self> {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let token_registry = Arc::new(Mutex::new(BrowserMcpTokenRegistry::default()));
        let listener_registry = Arc::clone(&token_registry);
        let listener_thread = thread::spawn(move || {
            run_listener(listener, command_tx, listener_registry, repaint);
        });

        Ok(Self {
            command_rx,
            port,
            token_registry,
            _listener_thread: Some(listener_thread),
        })
    }

    pub fn drain_commands(&self) -> Vec<BrowserMcpCommand> {
        let mut commands = Vec::new();
        while let Ok(command) = self.command_rx.try_recv() {
            commands.push(command);
        }
        commands
    }

    pub fn build_pty_env(
        &self,
        terminal_id: u64,
        project_id: Option<u64>,
        session_id: Option<&str>,
    ) -> Vec<(String, String)> {
        let token = self.token_for_scope(terminal_id, project_id, session_id);
        let mut env = vec![
            (
                MERGEN_BROWSER_MCP_PORT_ENV_VAR.to_owned(),
                self.port.to_string(),
            ),
            (MERGEN_BROWSER_MCP_TOKEN_ENV_VAR.to_owned(), token),
            (
                MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR.to_owned(),
                terminal_id.to_string(),
            ),
        ];
        if let Some(project_id) = project_id {
            env.push((
                MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR.to_owned(),
                project_id.to_string(),
            ));
        }
        if let Some(session_id) = session_id {
            env.push((
                MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR.to_owned(),
                session_id.to_owned(),
            ));
        }
        env
    }

    pub fn endpoint_env(
        &self,
        terminal_id: u64,
        project_id: Option<u64>,
        session_id: Option<&str>,
    ) -> BrowserMcpEndpointEnv {
        BrowserMcpEndpointEnv {
            port: self.port,
            token: self.token_for_scope(terminal_id, project_id, session_id),
            terminal_id,
            project_id,
            session_id: session_id.map(|s| s.to_owned()),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn revoke_terminal(&self, terminal_id: u64) {
        if let Ok(mut registry) = self.token_registry.lock() {
            registry.revoke_terminal(terminal_id);
        }
    }

    pub fn revoke_project(&self, project_id: u64) {
        if let Ok(mut registry) = self.token_registry.lock() {
            registry.revoke_project(project_id);
        }
    }

    fn token_for_scope(
        &self,
        terminal_id: u64,
        project_id: Option<u64>,
        session_id: Option<&str>,
    ) -> String {
        self.token_registry
            .lock()
            .map(|mut registry| {
                registry.token_for_scope(BrowserMcpAuthScope {
                    terminal_id,
                    project_id,
                    session_id: session_id.map(|s| s.to_owned()),
                })
            })
            .unwrap_or_else(|_| generate_token())
    }

    /// Revoke all tokens for a specific session (e.g., when OpenCode restarts).
    pub fn revoke_session(&self, session_id: &str) {
        if let Ok(mut registry) = self.token_registry.lock() {
            registry.revoke_session(session_id);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserMcpEndpointEnv {
    pub port: u16,
    pub token: String,
    pub terminal_id: u64,
    pub project_id: Option<u64>,
    /// Session ID for multi-session isolation.
    pub session_id: Option<String>,
}

fn run_listener(
    listener: TcpListener,
    command_tx: crossbeam_channel::Sender<BrowserMcpCommand>,
    token_registry: Arc<Mutex<BrowserMcpTokenRegistry>>,
    repaint: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let command_tx = command_tx.clone();
                let token_registry = Arc::clone(&token_registry);
                let repaint = repaint.clone();
                thread::spawn(move || {
                    if let Err(err) =
                        handle_stream(stream, command_tx, token_registry, repaint.as_deref())
                    {
                        log::debug!("Browser MCP request failed: {err}");
                    }
                });
            }
            Err(err) => log::warn!("Browser MCP listener failed: {err}"),
        }
    }
}

fn handle_stream(
    mut stream: TcpStream,
    command_tx: crossbeam_channel::Sender<BrowserMcpCommand>,
    token_registry: Arc<Mutex<BrowserMcpTokenRegistry>>,
    repaint: Option<&(dyn Fn() + Send + Sync + 'static)>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut raw = read_http_headers(&mut stream)?;
    let (auth_scope, content_length) = {
        let (request_line, headers, _) = parse_http_request(&raw)?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default();
        let path = request_parts.next().unwrap_or_default();
        if method != "POST" || path != MERGEN_BROWSER_MCP_ENDPOINT_PATH {
            write_http_response(&mut stream, 404, BrowserMcpIpcResponse::error("Not Found"))?;
            return Ok(());
        }

        let token = header_value(&headers, "x-mergen-browser-mcp-token")
            .or_else(|| header_value(&headers, "x-mergen-token"));
        let auth_scope = token.and_then(|token| {
            token_registry
                .lock()
                .ok()
                .and_then(|registry| registry.scope_for_token(token))
        });
        let Some(auth_scope) = auth_scope else {
            write_http_response(&mut stream, 403, BrowserMcpIpcResponse::error("Forbidden"))?;
            return Ok(());
        };
        let content_length = header_value(&headers, "content-length")
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        (auth_scope, content_length)
    };
    read_remaining_http_body(&mut stream, &mut raw, content_length)?;
    let body_start = find_header_end(&raw)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing HTTP header end"))?
        .body_start;
    let body_end = body_start.saturating_add(content_length).min(raw.len());
    let body = &raw[body_start..body_end];

    let request = serde_json::from_slice::<BrowserMcpIpcRequest>(body)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let (response_tx, response_rx) = mpsc::channel();
    command_tx
        .send(BrowserMcpCommand {
            request,
            auth_scope,
            respond_to: response_tx,
        })
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Browser MCP command queue closed",
            )
        })?;
    if let Some(repaint) = repaint {
        repaint();
    }
    let response = response_rx
        .recv_timeout(Duration::from_secs(90))
        .unwrap_or_else(|_| {
            BrowserMcpIpcResponse::error("Timed out waiting for Mergen ADE browser response")
        });
    write_http_response(&mut stream, 200, response)?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn read_http_headers(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
        if find_header_end(&buffer).is_some() {
            break;
        }
        if buffer.len() > MAX_BROWSER_MCP_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Browser MCP request headers are too large",
            ));
        }
    }
    Ok(buffer)
}

fn read_remaining_http_body(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    content_length: usize,
) -> io::Result<()> {
    if content_length > MAX_BROWSER_MCP_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Browser MCP request is too large",
        ));
    }
    let body_start = find_header_end(buffer)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing HTTP header end"))?
        .body_start;
    let expected_len = body_start.saturating_add(content_length);
    let mut chunk = [0_u8; 4096];
    while buffer.len() < expected_len {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len() > expected_len {
            buffer.truncate(expected_len);
            break;
        }
    }
    Ok(())
}

fn parse_http_request(raw: &[u8]) -> io::Result<(&str, Vec<(&str, &str)>, &[u8])> {
    let body_start = find_header_end(raw)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing HTTP header end"))?;
    let header_text = std::str::from_utf8(&raw[..body_start.header_len])
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let mut lines = header_text.lines();
    let request_line = lines.next().unwrap_or_default();
    let headers = lines
        .filter_map(|line| line.split_once(':').map(|(k, v)| (k.trim(), v.trim())))
        .collect::<Vec<_>>();
    Ok((request_line, headers, &raw[body_start.body_start..]))
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    response: BrowserMcpIpcResponse,
) -> io::Result<()> {
    let body = serde_json::to_vec(&response)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()
}

fn header_value<'a>(headers: &'a [(&str, &str)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| *value)
}

struct HeaderEnd {
    header_len: usize,
    body_start: usize,
}

fn find_header_end(buffer: &[u8]) -> Option<HeaderEnd> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|idx| HeaderEnd {
            header_len: idx,
            body_start: idx + 4,
        })
        .or_else(|| {
            buffer
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|idx| HeaderEnd {
                    header_len: idx,
                    body_start: idx + 2,
                })
        })
}

fn http_body_start_and_length(buffer: &[u8]) -> Option<(usize, usize)> {
    let end = find_header_end(buffer)?;
    let header_text = std::str::from_utf8(&buffer[..end.header_len]).ok()?;
    let content_length = header_text
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    Some((end.body_start, content_length))
}

fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    fill_secure_random(&mut bytes).expect("Browser MCP token generation requires OS randomness");
    format!("mbm-{}", bytes_to_hex(&bytes))
}

#[cfg(target_os = "windows")]
fn fill_secure_random(bytes: &mut [u8]) -> io::Result<()> {
    use windows_sys::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };

    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status >= 0 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("BCryptGenRandom failed with NTSTATUS {status:#x}"),
        ))
    }
}

#[cfg(not(target_os = "windows"))]
fn fill_secure_random(bytes: &mut [u8]) -> io::Result<()> {
    let mut file = std::fs::File::open("/dev/urandom")?;
    file.read_exact(bytes)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> BrowserMcpService {
        BrowserMcpService {
            command_rx: crossbeam_channel::unbounded().1,
            port: 1234,
            token_registry: Arc::new(Mutex::new(BrowserMcpTokenRegistry::default())),
            _listener_thread: None,
        }
    }

    #[test]
    fn generated_tokens_use_random_hex_payloads() {
        let first = generate_token();
        let second = generate_token();

        assert_ne!(first, second);
        assert_eq!(first.len(), "mbm-".len() + 64);
        assert!(first
            .strip_prefix("mbm-")
            .is_some_and(|payload| { payload.bytes().all(|byte| byte.is_ascii_hexdigit()) }));
    }

    #[test]
    fn build_pty_env_includes_endpoint_and_project() {
        let service = test_service();

        let env = service.build_pty_env(42, Some(7), None);
        assert_eq!(
            env[0],
            (
                MERGEN_BROWSER_MCP_PORT_ENV_VAR.to_owned(),
                "1234".to_owned()
            )
        );
        assert_eq!(env[1].0, MERGEN_BROWSER_MCP_TOKEN_ENV_VAR.to_owned());
        assert!(!env[1].1.is_empty());
        let scope = service
            .token_registry
            .lock()
            .expect("token registry")
            .scope_for_token(&env[1].1)
            .expect("token should be registered");
        assert_eq!(scope.terminal_id, 42);
        assert_eq!(scope.project_id, Some(7));
        assert_eq!(
            env[2],
            (
                MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR.to_owned(),
                "42".to_owned()
            )
        );
        assert_eq!(
            env[3],
            (
                MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR.to_owned(),
                "7".to_owned()
            )
        );
    }

    #[test]
    fn build_pty_env_includes_session_id_when_provided() {
        let service = test_service();

        let env = service.build_pty_env(42, Some(7), Some("session-abc-123"));
        assert_eq!(
            env[0],
            (
                MERGEN_BROWSER_MCP_PORT_ENV_VAR.to_owned(),
                "1234".to_owned()
            )
        );
        assert_eq!(env[1].0, MERGEN_BROWSER_MCP_TOKEN_ENV_VAR.to_owned());
        assert!(!env[1].1.is_empty());

        // Find the session_id entry
        let session_entry = env
            .iter()
            .find(|(k, _)| k == MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR);
        assert!(
            session_entry.is_some(),
            "SESSION_ID env var should be present"
        );
        assert_eq!(session_entry.unwrap().1, "session-abc-123");

        let scope = service
            .token_registry
            .lock()
            .expect("token registry")
            .scope_for_token(&env[1].1)
            .expect("token should be registered");
        assert_eq!(scope.terminal_id, 42);
        assert_eq!(scope.project_id, Some(7));
        assert_eq!(scope.session_id, Some("session-abc-123".to_owned()));
    }

    #[test]
    fn endpoint_env_uses_terminal_scoped_tokens() {
        let service = test_service();

        let first = service.endpoint_env(1, Some(7), None);
        let second = service.endpoint_env(2, Some(8), None);
        let first_again = service.endpoint_env(1, Some(7), None);

        assert_ne!(first.token, second.token);
        assert_eq!(first.token, first_again.token);
        let registry = service.token_registry.lock().expect("token registry");
        assert_eq!(
            registry.scope_for_token(&first.token),
            Some(BrowserMcpAuthScope {
                terminal_id: 1,
                project_id: Some(7),
                session_id: None,
            })
        );
        assert_eq!(
            registry.scope_for_token(&second.token),
            Some(BrowserMcpAuthScope {
                terminal_id: 2,
                project_id: Some(8),
                session_id: None,
            })
        );
    }

    #[test]
    fn endpoint_env_uses_session_scoped_tokens() {
        let service = test_service();

        // Same terminal and project, but different sessions should get different tokens
        let session_a = service.endpoint_env(1, Some(7), Some("session-a"));
        let session_b = service.endpoint_env(1, Some(7), Some("session-b"));
        let session_a_again = service.endpoint_env(1, Some(7), Some("session-a"));

        assert_ne!(
            session_a.token, session_b.token,
            "Different sessions should have different tokens"
        );
        assert_eq!(
            session_a.token, session_a_again.token,
            "Same session should have same token"
        );

        let registry = service.token_registry.lock().expect("token registry");
        assert_eq!(
            registry.scope_for_token(&session_a.token),
            Some(BrowserMcpAuthScope {
                terminal_id: 1,
                project_id: Some(7),
                session_id: Some("session-a".to_owned()),
            })
        );
        assert_eq!(
            registry.scope_for_token(&session_b.token),
            Some(BrowserMcpAuthScope {
                terminal_id: 1,
                project_id: Some(7),
                session_id: Some("session-b".to_owned()),
            })
        );
    }

    #[test]
    fn revoke_session_removes_all_tokens_with_session_id_globally() {
        let service = test_service();
        
        let session_a_terminal_1 = service.endpoint_env(1, Some(7), Some("session-a"));
        let session_b = service.endpoint_env(1, Some(7), Some("session-b"));
        let session_a_terminal_2 = service.endpoint_env(2, Some(8), Some("session-a"));

        service.revoke_session("session-a");

        let registry = service.token_registry.lock().expect("token registry");
        assert_eq!(
            registry.scope_for_token(&session_a_terminal_1.token),
            None,
            "session-a token for terminal 1 should be revoked"
        );
        assert_eq!(
            registry.scope_for_token(&session_b.token),
            Some(BrowserMcpAuthScope {
                terminal_id: 1,
                project_id: Some(7),
                session_id: Some("session-b".to_owned()),
            }),
            "session-b token should remain"
        );
        // Session IDs are global - revoking "session-a" should revoke it everywhere
        assert_eq!(
            registry.scope_for_token(&session_a_terminal_2.token),
            None,
            "session-a token for terminal 2 should also be revoked (session IDs are global)"
        );
    }

    #[test]
    fn revoke_terminal_removes_terminal_tokens() {
        let service = test_service();
        let terminal_token = service.endpoint_env(1, Some(7), None).token;
        let other_token = service.endpoint_env(2, Some(8), None).token;

        service.revoke_terminal(1);

        let registry = service.token_registry.lock().expect("token registry");
        assert_eq!(registry.scope_for_token(&terminal_token), None);
        assert_eq!(
            registry.scope_for_token(&other_token),
            Some(BrowserMcpAuthScope {
                terminal_id: 2,
                project_id: Some(8),
                session_id: None,
            })
        );
    }

    #[test]
    fn revoke_terminal_removes_all_session_tokens_for_terminal() {
        let service = test_service();
        let session_a = service.endpoint_env(1, Some(7), Some("session-a"));
        let session_b = service.endpoint_env(1, Some(7), Some("session-b"));
        let other_terminal = service.endpoint_env(2, Some(8), Some("session-a"));

        service.revoke_terminal(1);

        let registry = service.token_registry.lock().expect("token registry");
        assert_eq!(
            registry.scope_for_token(&session_a.token),
            None,
            "session-a token should be revoked"
        );
        assert_eq!(
            registry.scope_for_token(&session_b.token),
            None,
            "session-b token should be revoked"
        );
        assert_eq!(
            registry.scope_for_token(&other_terminal.token),
            Some(BrowserMcpAuthScope {
                terminal_id: 2,
                project_id: Some(8),
                session_id: Some("session-a".to_owned()),
            }),
            "Other terminal should remain"
        );
    }

    #[test]
    fn http_body_start_handles_crlf_headers() {
        let raw = b"POST /browser-mcp HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(http_body_start_and_length(raw), Some((raw.len() - 2, 2)));
    }
}
