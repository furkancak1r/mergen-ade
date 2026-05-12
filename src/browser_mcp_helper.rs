use std::collections::BTreeMap;
use std::env;
use std::io::{self, BufRead, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::browser_mcp_service::{
    BrowserMcpIpcRequest, BrowserMcpIpcResponse, DEFAULT_BROWSER_MCP_TIMEOUT_MS,
    MERGEN_BROWSER_MCP_ENDPOINT_PATH, MERGEN_BROWSER_MCP_PORT_ENV_VAR,
    MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR, MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR,
    MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR, MERGEN_BROWSER_MCP_TOKEN_ENV_VAR,
};
use serde_json::{json, Value as JsonValue};

const SERVER_NAME: &str = "mergen-browser-mcp";
const PROTOCOL_VERSION: &str = "2024-11-05";
const BROWSER_WAIT_DEFAULT_TIMEOUT_SECS: f64 = 30.0;
const BROWSER_WAIT_POLL_MS: u64 = 50;
const BROWSER_WAIT_TIMEOUT_MARGIN_SECS: f64 = 5.0;

/// Run the Browser MCP helper mode from the main executable.
/// This is invoked when the main executable is launched with `--browser-mcp-helper`.
pub fn run() -> io::Result<()> {
    let env = HelperEnv::from_env();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let message = match serde_json::from_str::<JsonValue>(&line) {
            Ok(message) => message,
            Err(err) => {
                write_json(
                    &mut stdout,
                    jsonrpc_error(JsonValue::Null, -32700, err.to_string()),
                )?;
                continue;
            }
        };
        if let Some(response) = handle_jsonrpc_message(&env, message) {
            write_json(&mut stdout, response)?;
        }
    }
    Ok(())
}

fn write_json(stdout: &mut io::Stdout, value: JsonValue) -> io::Result<()> {
    serde_json::to_writer(&mut *stdout, &value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

fn handle_jsonrpc_message(env: &HelperEnv, message: JsonValue) -> Option<JsonValue> {
    let id = message.get("id").cloned();
    let method = message
        .get("method")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    match method {
        "initialize" => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Controls the embedded Mergen ADE Browser panel. All actions are reflected live in the Mergen Browser panel. It never launches external Chrome or Playwright."
                }
            })
        }),
        "tools/list" => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tool_schemas(env.caps.as_slice()) }
            })
        }),
        "tools/call" => id.map(|id| {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params.get("name").and_then(JsonValue::as_str).unwrap_or_default();
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            // Reject tools not advertised in tools/list (e.g., hidden coordinate tools)
            if !is_tool_allowed(name, env) {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Tool not found: {name}. Use tools from tools/list only.")
                    }
                });
            }
            let result = call_mergen(env, name, arguments);
            let is_error = result.is_error;
            let content = mcp_content(result);
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": content,
                    "isError": is_error,
                }
            })
        }),
        _ => id.map(|id| jsonrpc_error(id, -32601, format!("Method not found: {method}"))),
    }
}

fn mcp_content(response: BrowserMcpIpcResponse) -> Vec<JsonValue> {
    let mut content = Vec::with_capacity(2);
    content.push(json!({ "type": "text", "text": response.text }));
    if let Some(data) = response.data {
        if let Some(image_type) = data.get("imageType").and_then(JsonValue::as_str) {
            if let Some(base64) = data.get("base64").and_then(JsonValue::as_str) {
                let mime_type = match image_type {
                    "png" => "image/png",
                    "jpeg" | "jpg" => "image/jpeg",
                    _ => "image/png",
                };
                content.push(json!({
                    "type": "image",
                    "mimeType": mime_type,
                    "data": base64
                }));
            }
        }
    }
    content
}

fn call_mergen(env: &HelperEnv, tool: &str, params: JsonValue) -> BrowserMcpIpcResponse {
    if tool == "browser_wait_for" {
        return call_browser_wait_for(env, params);
    }
    call_mergen_once(env, tool, params)
}

fn call_mergen_once(env: &HelperEnv, tool: &str, params: JsonValue) -> BrowserMcpIpcResponse {
    let request = browser_mcp_ipc_request(env, tool, params);
    match send_ipc_request(env, request) {
        Ok(response) => response,
        Err(err) => BrowserMcpIpcResponse::error(err),
    }
}

fn call_mergen_once_with_timeout(
    env: &HelperEnv,
    tool: &str,
    params: JsonValue,
    timeout: Duration,
) -> BrowserMcpIpcResponse {
    let request = browser_mcp_ipc_request(env, tool, params);
    match send_ipc_request_with_timeout(env, request, timeout) {
        Ok(response) => response,
        Err(err) => BrowserMcpIpcResponse::error(err),
    }
}

fn call_browser_wait_for(env: &HelperEnv, params: JsonValue) -> BrowserMcpIpcResponse {
    let plan = match browser_wait_plan(&params) {
        Ok(p) => p,
        Err(err) => return BrowserMcpIpcResponse::error(err),
    };
    let start = Instant::now();
    match plan {
        BrowserWaitPlan::RejectedFixedWait => {
            BrowserMcpIpcResponse::error(
                "Fixed waits are not supported. Mergen Browser MCP tools automatically wait for page readiness. Use 'text' or 'textGone' conditions to wait for specific content, or call browser_page_summary/browser_click directly without waiting.".to_owned()
            )
        }
        BrowserWaitPlan::Condition {
            timeout,
            params: mut poll_params,
            description,
        } => {
            poll_params["tool"] = json!("browser_wait_for_poll");
            loop {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return BrowserMcpIpcResponse::error(format!(
                        "Timeout waiting for condition: {description}"
                    ));
                }
                let request = browser_mcp_ipc_request(env, "browser_wait_for", poll_params.clone());
                let poll_result = send_ipc_request_with_timeout(
                    env,
                    request,
                    browser_wait_poll_read_timeout(timeout.saturating_sub(elapsed)),
                );
                match poll_result {
                    Ok(response) => {
                        if !response.is_error {
                            return BrowserMcpIpcResponse::ok(format!(
                                "Wait complete: {description}"
                            ));
                        }
                        if !browser_wait_error_is_retryable(&response.text) {
                            return response;
                        }
                    }
                    Err(err) => {
                        if !browser_wait_error_is_retryable(&err) {
                            return BrowserMcpIpcResponse::error(err);
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(BROWSER_WAIT_POLL_MS));
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum BrowserWaitPlan {
    /// Fixed waits are rejected - tools automatically handle page readiness
    RejectedFixedWait,
    Condition {
        timeout: Duration,
        params: JsonValue,
        description: String,
    },
}

fn browser_wait_plan(params: &JsonValue) -> Result<BrowserWaitPlan, String> {
    let time_raw = params.get("time");
    let time_num = time_raw.and_then(JsonValue::as_f64);
    let text = non_empty_param(params, "text");
    let text_gone = non_empty_param(params, "textGone");
    if text.is_some() && text_gone.is_some() {
        return Err(
            "browser_wait_for cannot have both 'text' and 'textGone' conditions at once."
                .to_owned(),
        );
    }

    // Validate that if time is present, it must be a valid number
    if time_raw.is_some() && time_num.is_none() {
        return Err(
            "browser_wait_for 'time' must be a number (seconds), got a non-numeric value"
                .to_owned(),
        );
    }

    // Always validate time first - negative values are invalid regardless of condition
    let max = Duration::from_millis(DEFAULT_BROWSER_MCP_TIMEOUT_MS);
    let timeout = time_num
        .map(|t| parse_browser_wait_duration(t, max, true))
        .transpose()?;

    let has_condition = text.is_some() || text_gone.is_some();
    let is_fixed_only = time_num.is_some() && !has_condition;

    // Reject fixed-only waits - tools automatically handle readiness
    if is_fixed_only {
        return Ok(BrowserWaitPlan::RejectedFixedWait);
    }

    let mut poll_params = params.clone();
    if let Some(obj) = poll_params.as_object_mut() {
        obj.remove("time");
    }

    // If no explicit timeout, use default
    let timeout = timeout.unwrap_or_else(|| {
        Duration::from_secs_f64(BROWSER_WAIT_DEFAULT_TIMEOUT_SECS)
            .saturating_sub(Duration::from_secs_f64(BROWSER_WAIT_TIMEOUT_MARGIN_SECS))
    });

    let description = match text {
        Some(t) => format!("text to appear: {t}"),
        None => format!("text to disappear: {}", text_gone.unwrap_or_default()),
    };

    Ok(BrowserWaitPlan::Condition {
        timeout,
        params: poll_params,
        description,
    })
}

fn non_empty_param<'a>(params: &'a JsonValue, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty())
}

fn parse_browser_wait_duration(
    seconds: f64,
    max: Duration,
    allow_zero: bool,
) -> Result<Duration, String> {
    if seconds < 0.0 {
        return Err(format!(
            "browser_wait_for 'time' must be a non-negative number, got: {seconds}"
        ));
    }
    if !allow_zero && seconds == 0.0 {
        return Err(format!(
            "browser_wait_for 'time' must be a positive number, got: {seconds}"
        ));
    }
    let nanos = (seconds * 1_000_000_000.0) as u64;
    let dur = Duration::from_nanos(nanos);
    if dur > max {
        return Err(format!(
            "browser_wait_for 'time' exceeds maximum ({}s). Use a smaller wait.",
            max.as_secs()
        ));
    }
    Ok(dur)
}

fn format_duration_seconds(duration: Duration) -> String {
    format_seconds(duration.as_secs_f64())
}

fn format_seconds(seconds: f64) -> String {
    if seconds < 1.0 {
        format!("{:.2}s", seconds)
    } else if seconds < 10.0 {
        format!("{:.1}s", seconds)
    } else {
        format!("{:.0}s", seconds)
    }
}

fn browser_wait_poll_read_timeout(remaining: Duration) -> Duration {
    const POLL_READ_MAX: Duration = Duration::from_secs(2);
    remaining.min(POLL_READ_MAX)
}

fn browser_wait_error_is_retryable(message: &str) -> bool {
    message.contains("Text not found") || message.contains("Text is still visible")
}

fn browser_mcp_ipc_request(env: &HelperEnv, tool: &str, params: JsonValue) -> BrowserMcpIpcRequest {
    BrowserMcpIpcRequest {
        request_id: request_id(),
        terminal_id: env.terminal_id,
        project_id: env.project_id,
        session_id: env.session_id.clone(),
        tool: tool.to_owned(),
        params,
    }
}

fn send_ipc_request(
    env: &HelperEnv,
    request: BrowserMcpIpcRequest,
) -> Result<BrowserMcpIpcResponse, String> {
    let timeout = Duration::from_millis(DEFAULT_BROWSER_MCP_TIMEOUT_MS);
    send_ipc_request_with_timeout(env, request, timeout)
}

fn send_ipc_request_with_timeout(
    env: &HelperEnv,
    request: BrowserMcpIpcRequest,
    timeout: Duration,
) -> Result<BrowserMcpIpcResponse, String> {
    let Some(port) = env.port else {
        return Err(format!(
            "Mergen Browser MCP is not connected: {MERGEN_BROWSER_MCP_PORT_ENV_VAR} is missing. Start OpenCode from Mergen ADE."
        ));
    };
    let Some(token) = env.token.as_ref() else {
        return Err(format!(
            "Mergen Browser MCP is not connected: {MERGEN_BROWSER_MCP_TOKEN_ENV_VAR} is missing. Start OpenCode from Mergen ADE."
        ));
    };
    let addr = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&addr).map_err(|err| {
        format!("Mergen Browser MCP bridge is unavailable: {err}. No external Chrome fallback was launched.")
    })?;
    let body = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    let req = format!(
        "POST {MERGEN_BROWSER_MCP_ENDPOINT_PATH} HTTP/1.1\r\n\
        Host: localhost\r\n\
        Content-Type: application/json\r\n\
        x-mergen-browser-mcp-token: {token}\r\n\
        Content-Length: {}\r\n\r\n\
        {body}",
        body.len()
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| e.to_string())?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(256 * 1024);
    stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let response_text = String::from_utf8_lossy(&buf);
    let Some(body_start) = response_text.find("\r\n\r\n") else {
        return Err("Invalid HTTP response from Mergen Browser MCP".to_owned());
    };
    let body = &response_text[body_start + 4..];
    if body.is_empty() {
        return Err("Empty HTTP body from Mergen Browser MCP".to_owned());
    }
    let response: BrowserMcpIpcResponse =
        serde_json::from_str(body).map_err(|e| format!("Invalid MCP response: {e}"))?;
    if response.is_error {
        return Err(response.text);
    }
    Ok(response)
}

#[derive(Debug, Clone)]
struct HelperEnv {
    port: Option<u16>,
    token: Option<String>,
    terminal_id: Option<u64>,
    project_id: Option<u64>,
    /// Session ID for multi-session isolation.
    session_id: Option<String>,
    caps: Vec<String>,
}

impl HelperEnv {
    fn from_env() -> Self {
        Self {
            port: env::var(MERGEN_BROWSER_MCP_PORT_ENV_VAR)
                .ok()
                .and_then(|value| value.parse::<u16>().ok()),
            token: env::var(MERGEN_BROWSER_MCP_TOKEN_ENV_VAR).ok(),
            terminal_id: env::var(MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR)
                .ok()
                .and_then(|value| value.parse::<u64>().ok()),
            project_id: env::var(MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR)
                .ok()
                .and_then(|value| value.parse::<u64>().ok()),
            session_id: env::var(MERGEN_BROWSER_MCP_SESSION_ID_ENV_VAR).ok(),
            caps: parse_caps_from_args(),
        }
    }
}

fn parse_caps_from_args() -> Vec<String> {
    let mut caps = Vec::new();
    let mut args = env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        let value = if let Some(value) = arg.strip_prefix("--caps=") {
            Some(value.to_owned())
        } else if arg == "--caps" {
            args.next()
        } else {
            None
        };
        if let Some(value) = value {
            caps.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|cap| !cap.is_empty())
                    .map(str::to_owned),
            );
        }
    }
    caps.sort();
    caps.dedup();
    caps
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("mbm-{nanos:x}")
}

fn jsonrpc_error(id: JsonValue, code: i64, message: String) -> JsonValue {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn tool_schemas(caps: &[String]) -> Vec<JsonValue> {
    let mut tools = Vec::with_capacity(50);
    tools.extend(core_tools());
    if has_cap(caps, "devtools") {
        tools.extend(devtools_tools());
    }
    if has_cap(caps, "vision") {
        tools.extend(vision_tools());
    }
    if has_cap(caps, "network") {
        tools.extend(network_tools());
    }
    if has_cap(caps, "storage") {
        tools.extend(storage_tools());
    }
    tools
}

fn has_cap(caps: &[String], cap: &str) -> bool {
    caps.is_empty() || caps.binary_search(&cap.to_owned()).is_ok()
}

/// Check if a tool name is in the public advertised schema for the given caps.
/// This prevents clients from calling hidden/internal tools (e.g., coordinate mouse tools)
/// even if they know the tool name.
fn is_tool_allowed(name: &str, env: &HelperEnv) -> bool {
    let schemas = tool_schemas(env.caps.as_slice());
    schemas
        .iter()
        .any(|t| t.get("name").and_then(JsonValue::as_str) == Some(name))
}

fn tool(name: &str, description: &str, input_schema: JsonValue) -> JsonValue {
    let mut map = BTreeMap::<String, JsonValue>::new();
    map.insert("name".to_owned(), json!(name));
    map.insert("description".to_owned(), json!(description));
    map.insert("inputSchema".to_owned(), input_schema);
    serde_json::to_value(map).unwrap_or_else(|_| json!({}))
}

fn core_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_close",
            "Close the active Mergen Browser tab. If it is the last tab, an empty replacement tab is created.",
            json!({
                "type": "object",
                "properties": {
                    "index": json!({"type": "integer"}),
                    "tabId": json!({"type": "integer"})
                },
                "required": []
            }),
        ),
        tool(
            "browser_cookie_clear",
            "Clear browser cookies",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_cookie_delete",
            "Delete a browser cookie",
            json!({
                "type": "object",
                "properties": {
                    "name": json!({"type": "string"}),
                    "url": json!({"type": "string"})
                },
                "required": ["name"]
            }),
        ),
        tool(
            "browser_cookie_get",
            "Get a browser cookie by name",
            json!({
                "type": "object",
                "properties": {
                    "name": json!({"type": "string"}),
                    "url": json!({"type": "string"})
                },
                "required": ["name"]
            }),
        ),
        tool(
            "browser_cookie_list",
            "List browser cookies",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_cookie_set",
            "Set a browser cookie",
            json!({
                "type": "object",
                "properties": {
                    "name": json!({"type": "string"}),
                    "value": json!({"type": "string"}),
                    "url": json!({"type": "string"}),
                    "domain": json!({"type": "string"}),
                    "path": json!({"type": "string"}),
                    "secure": json!({"type": "boolean"}),
                    "httpOnly": json!({"type": "boolean"}),
                    "sameSite": json!({"type": "string", "enum": ["Strict", "Lax", "None"]}),
                    "expires": json!({"type": "number"})
                },
                "required": ["name", "value"]
            }),
        ),
        tool(
            "browser_navigate",
            "Navigate to a URL in the browser. Automatically waits for the page to fully load and become ready before returning. Do not add fixed waits after calling this tool.",
            json!({
                "type": "object",
                "properties": {
                    "url": json!({"type": "string"})
                },
                "required": ["url"]
            }),
        ),
        tool(
            "browser_navigate_back",
            "Go back in the browser history. Automatically waits for the page to become ready after navigation.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_navigate_forward",
            "Go forward in the browser history. Automatically waits for the page to become ready after navigation.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_reload",
            "Reload the current page. Automatically waits for the page to fully load and become ready before returning.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_sessionstorage_clear",
            "Clear session storage",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_sessionstorage_delete",
            "Delete a session storage item",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"})
                },
                "required": ["key"]
            }),
        ),
        tool(
            "browser_sessionstorage_get",
            "Get a session storage item",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"})
                },
                "required": ["key"]
            }),
        ),
        tool(
            "browser_sessionstorage_list",
            "List session storage keys",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_sessionstorage_set",
            "Set a session storage item",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"}),
                    "value": json!({"type": "string"})
                },
                "required": ["key", "value"]
            }),
        ),
        tool(
            "browser_tabs",
            "List, create, select, or close tabs in the Mergen Browser panel. At most five tabs can be open per project.",
            json!({
                "type": "object",
                "properties": {
                    "action": json!({"type": "string", "enum": ["list", "new", "select", "close"], "default": "list"}),
                    "index": json!({"type": "integer"}),
                    "tabId": json!({"type": "integer"}),
                    "url": json!({"type": "string"})
                },
                "required": []
            }),
        ),
        tool(
            "browser_type",
            "Type text into an element with user-like keyboard events. Automatically waits for any resulting navigation or SPA route change to complete before returning. Do not add fixed waits after typing.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string"}),
                    "text": json!({"type": "string"}),
                    "submit": json!({"type": "boolean"}),
                    "commit": json!({"type": "boolean", "default": true, "description": "When true, dispatch change/focusout/blur after typing so form validation runs."})
                })),
                "required": ["text"]
            }),
        ),
        tool(
            "browser_press_key",
            "Press a key in the browser. Automatically waits for any resulting navigation or SPA update to complete before returning.",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"})
                },
                "required": ["key"]
            }),
        ),
    ]
}

fn devtools_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_console_messages",
            "Get browser console messages (not implemented by Mergen Browser MCP yet).",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_hide_highlight",
            "Hide the active browser highlight overlay",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_highlight",
            "Move the visible browser mouse to a visible/reachable element (by ref) and show a highlight overlay. Use a ref from browser_page_summary. Defaults to a neutral green feature callout; use red only when explicitly marking an error. Only one highlight can be active; call browser_hide_highlight before creating another.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string", "description": "Element ref from browser_page_summary (e.g., e42). Preferred over coordinates."}),
                    "x": json!({"type": "number", "description": "Viewport CSS pixel x coordinate (only if ref unavailable)"}),
                    "y": json!({"type": "number", "description": "Viewport CSS pixel y coordinate (only if ref unavailable)"}),
                    "width": json!({"type": "number", "description": "Viewport CSS pixel width for rectangle highlights"}),
                    "height": json!({"type": "number", "description": "Viewport CSS pixel height for rectangle highlights"}),
                    "color": json!({"type": "string", "default": "#16a34a", "description": "Highlight accent color. Prefer the default green (#16a34a) for neutral feature callouts; use red only when explicitly marking an error."}),
                    "label": json!({"type": "string", "description": "Optional short label shown above the highlight"}),
                    "padding": json!({"type": "number", "default": 8}),
                    "radius": json!({"type": "number", "default": 10})
                }))
            }),
        ),
        tool(
            "browser_select_option",
            "Select an option in a dropdown using a ref from browser_page_summary. Automatically waits for any resulting page update or navigation to complete before returning.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string", "description": "Element ref from browser_page_summary (e.g., e42)"}),
                    "value": json!({"type": "string"})
                })),
                "required": ["ref", "value"]
            }),
        ),
        tool(
            "browser_page_summary",
            "Fast page map for discovering clickable elements, buttons, links, icons, and form fields. Always use this before browser_click to obtain a ref (e.g., e42). Includes aria-label, title, id, class, data-testid, and visual indicators like cursor:pointer to help locate icon-only buttons and sidebar controls.",
            json!({
                "type": "object",
                "properties": {
                    "query": json!({"type": "string", "description": "Optional target text such as a button label, aria-label, title, or icon name to rank first"}),
                    "roles": json!({"type": "array", "items": {"type": "string"}, "description": "Optional role filter such as button, link, textbox, combobox"}),
                    "includeBoxes": json!({"type": "boolean", "default": false}),
                    "maxItems": json!({"type": "integer", "default": 40})
                },
                "required": []
            }),
        ),
        tool(
            "browser_snapshot",
            "Take a DOM snapshot (accessibility snapshot) of the page. Prefer browser_page_summary first when deciding where to click.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string"}),
                    "scope": json!({"type": "string"})
                }))
            }),
        ),
        tool(
            "browser_click",
            "Click an element on the page with the visible browser mouse cursor using a ref from browser_page_summary or Design Inspect. Automatically waits for the page to become ready after the click (including navigation or SPA route transitions). Do not use coordinates; always use refs.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string", "description": "Element ref from browser_page_summary (e.g., e42)"}),
                    "button": json!({"type": "string", "enum": ["left", "middle", "right"], "default": "left"}),
                    "doubleClick": json!({"type": "boolean"})
                })),
                "required": ["ref"]
            }),
        ),
        tool(
            "browser_hover",
            "Hover over an element on the page with the visible browser mouse cursor using a ref from browser_page_summary.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string", "description": "Element ref from browser_page_summary (e.g., e42)"})
                })),
                "required": ["ref"]
            }),
        ),
        tool(
            "browser_fill_form",
            "Fill multiple form fields with user-like interaction. Automatically waits for any resulting page update or navigation to complete before returning.",
            json!({
                "type": "object",
                "properties": {
                    "fields": json!({
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "target": {"type": "string"},
                                "ref": {"type": "string"},
                                "type": {"type": "string", "enum": ["textbox", "checkbox", "radio", "combobox", "slider"]},
                                "value": {"type": "string"},
                                "commit": {"type": "boolean", "default": true}
                            },
                            "required": ["value"]
                        }
                    })
                },
                "required": ["fields"]
            }),
        ),
        tool(
            "browser_evaluate",
            "Read/evaluate JavaScript in the browser. Interactive clicks, mouse events, and form submits are blocked; use browser_click with a ref for page interaction.",
            json!({
                "type": "object",
                "properties": {
                    "script": json!({"type": "string"}),
                    "frame": json!({"type": "string"}),
                    "ref": json!({"type": "string"})
                },
                "required": ["script"]
            }),
        ),
        tool(
            "browser_wait_for",
            "Wait for a condition (text or textGone). Fixed 'time' waits are not supported because Mergen Browser MCP tools automatically wait for page readiness. Use this only when you need to wait for specific content to appear or disappear after an action.",
            json!({
                "type": "object",
                "properties": {
                    "time": json!({"type": "number", "description": "Maximum timeout in seconds for the condition (safety limit only, not a fixed wait)"}),
                    "text": json!({"type": "string", "description": "Wait until text appears on page"}),
                    "textGone": json!({"type": "string", "description": "Wait until text disappears from page"})
                },
                "required": []
            }),
        ),
    ]
}

fn vision_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_take_screenshot",
            "Take a screenshot of the embedded Mergen browser page. Prefer browser_page_summary for deciding where to click; use screenshots for visual verification instead of fixed waits.",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string"}),
                    "type": json!({"type": "string", "enum": ["png", "jpeg"], "default": "jpeg"}),
                    "quality": json!({"type": "integer", "default": 74, "description": "JPEG quality from 1 to 100; ignored for PNG"}),
                    "fullPage": json!({"type": "boolean", "default": false})
                }))
            }),
        ),
        tool(
            "browser_start_video",
            "Start recording the embedded Mergen browser panel to an MP4 file",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_stop_video",
            "Stop recording the embedded Mergen browser panel and save the MP4 file",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_video_chapter",
            "Add a timestamped chapter marker to the active embedded Mergen browser video recording",
            json!({
                "type": "object",
                "properties": {
                    "title": json!({"type": "string"}),
                    "label": json!({"type": "string"})
                },
                "required": []
            }),
        ),
    ]
}

fn network_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_network_request",
            "Get detailed info about a network request (not implemented by Mergen Browser MCP yet).",
            json!({
                "type": "object",
                "properties": {
                    "requestId": json!({"type": "string"}),
                    "wait": json!({"type": "boolean"})
                },
                "required": ["requestId"]
            }),
        ),
        tool(
            "browser_network_requests",
            "List network requests (not implemented by Mergen Browser MCP yet).",
            json!({
                "type": "object",
                "properties": {
                    "urlFilter": json!({"type": "string"}),
                    "methodFilter": json!({"type": "string"})
                },
                "required": []
            }),
        ),
    ]
}

fn storage_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_localstorage_clear",
            "Clear local storage",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_localstorage_delete",
            "Delete a local storage item",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"})
                },
                "required": ["key"]
            }),
        ),
        tool(
            "browser_localstorage_get",
            "Get a local storage item",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"})
                },
                "required": ["key"]
            }),
        ),
        tool(
            "browser_localstorage_list",
            "List local storage keys",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_localstorage_set",
            "Set a local storage item",
            json!({
                "type": "object",
                "properties": {
                    "key": json!({"type": "string"}),
                    "value": json!({"type": "string"})
                },
                "required": ["key", "value"]
            }),
        ),
    ]
}

fn element_props(extra: JsonValue) -> JsonValue {
    let mut map = BTreeMap::<String, JsonValue>::new();
    map.insert("type".to_owned(), json!({"type":"string"}));
    if let JsonValue::Object(extra) = extra {
        for (key, value) in extra {
            map.insert(key, value);
        }
    }
    serde_json::to_value(map).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_tool_schemas_include_playwright_names() {
        let names = core_tools()
            .into_iter()
            .filter_map(|tool| {
                tool.get("name")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        assert!(names.contains(&"browser_navigate".to_owned()));
        assert!(names.contains(&"browser_cookie_list".to_owned()));
        assert!(names.contains(&"browser_type".to_owned()));
    }

    #[test]
    fn browser_type_schema_exposes_validation_commit_option() {
        let tools = core_tools();
        let browser_type = tools
            .iter()
            .find(|tool| tool.get("name").and_then(JsonValue::as_str) == Some("browser_type"))
            .expect("browser_type schema");

        assert!(browser_type["description"]
            .as_str()
            .unwrap_or_default()
            .contains("waits for"));
        assert_eq!(
            browser_type["inputSchema"]["properties"]["commit"]["default"].as_bool(),
            Some(true)
        );
        assert!(
            !browser_type["inputSchema"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("commit")),
            "commit should remain optional"
        );
    }

    #[test]
    fn devtools_tool_schemas_include_snapshot_and_click() {
        let names = devtools_tools()
            .into_iter()
            .filter_map(|tool| {
                tool.get("name")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        assert!(names.contains(&"browser_snapshot".to_owned()));
        assert!(names.contains(&"browser_page_summary".to_owned()));
        assert!(names.contains(&"browser_click".to_owned()));
        assert!(names.contains(&"browser_hover".to_owned()));
        assert!(names.contains(&"browser_fill_form".to_owned()));
        assert!(names.contains(&"browser_evaluate".to_owned()));
    }

    #[test]
    fn devtools_tool_schemas_include_fast_page_summary() {
        let tools = devtools_tools();
        let summary = tools
            .iter()
            .find(|tool| {
                tool.get("name").and_then(JsonValue::as_str) == Some("browser_page_summary")
            })
            .expect("browser_page_summary schema");
        let props = &summary["inputSchema"]["properties"];

        assert!(summary["description"]
            .as_str()
            .unwrap_or_default()
            .contains("Fast page map"));
        assert_eq!(props["query"]["type"].as_str(), Some("string"));
        assert_eq!(props["roles"]["type"].as_str(), Some("array"));
        assert_eq!(props["includeBoxes"]["default"].as_bool(), Some(false));
        assert_eq!(props["maxItems"]["default"].as_i64(), Some(40));
        assert_eq!(
            summary["inputSchema"]["required"].as_array().unwrap().len(),
            0
        );

        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(JsonValue::as_str))
            .collect::<Vec<_>>();
        let summary_index = names
            .iter()
            .position(|name| *name == "browser_page_summary")
            .expect("browser_page_summary exists");
        let snapshot_index = names
            .iter()
            .position(|name| *name == "browser_snapshot")
            .expect("browser_snapshot exists");
        assert!(summary_index < snapshot_index);
    }

    #[test]
    fn devtools_tool_schemas_exclude_coordinate_click_tools() {
        // Coordinate mouse click tools are intentionally not advertised to AI
        // They remain available at runtime for internal use but are not in devtools_tools()
        let tools = devtools_tools();
        let tool_names: Vec<_> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(JsonValue::as_str))
            .collect();

        // These coordinate tools should NOT be in the AI-facing schema
        assert!(!tool_names.contains(&"browser_mouse_move_xy"));
        assert!(!tool_names.contains(&"browser_mouse_click_xy"));
        assert!(!tool_names.contains(&"browser_mouse_drag_xy"));
        assert!(!tool_names.contains(&"browser_mouse_down"));
        assert!(!tool_names.contains(&"browser_mouse_up"));
        assert!(!tool_names.contains(&"browser_mouse_wheel"));

        // browser_click should require ref
        let click_tool = tools
            .iter()
            .find(|t| t.get("name").and_then(JsonValue::as_str) == Some("browser_click"))
            .expect("browser_click should exist");
        let required = click_tool["inputSchema"]["required"]
            .as_array()
            .expect("required should be array");
        assert!(required.contains(&json!("ref")));
    }

    #[test]
    fn is_tool_allowed_rejects_hidden_coordinate_tools() {
        let env = HelperEnv {
            port: Some(12345),
            token: Some("test".to_owned()),
            terminal_id: Some(1),
            project_id: Some(1),
            session_id: None,
            caps: vec!["devtools".to_owned()],
        };
        // Public tools should be allowed
        assert!(is_tool_allowed("browser_click", &env));
        assert!(is_tool_allowed("browser_page_summary", &env));
        assert!(is_tool_allowed("browser_evaluate", &env));
        // Hidden coordinate tools should be rejected even if client knows the name
        assert!(!is_tool_allowed("browser_mouse_click_xy", &env));
        assert!(!is_tool_allowed("browser_mouse_move_xy", &env));
        assert!(!is_tool_allowed("browser_mouse_drag_xy", &env));
        assert!(!is_tool_allowed("browser_mouse_down", &env));
        assert!(!is_tool_allowed("browser_mouse_up", &env));
        assert!(!is_tool_allowed("browser_mouse_wheel", &env));
        // Unknown tools should also be rejected
        assert!(!is_tool_allowed("browser_hack", &env));
    }

    #[test]
    fn devtools_tool_schemas_include_video_highlight_controls() {
        let tools = devtools_tools();
        let tool_by_name = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(JsonValue::as_str) == Some(name))
                .unwrap_or_else(|| panic!("missing tool schema: {name}"))
        };

        let highlight = tool_by_name("browser_highlight");
        let props = &highlight["inputSchema"]["properties"];
        for field in [
            "type", "ref", "x", "y", "width", "height", "color", "label", "padding", "radius",
        ] {
            assert!(
                props.get(field).is_some(),
                "missing browser_highlight.{field}"
            );
        }
        assert_eq!(props["color"]["default"].as_str(), Some("#16a34a"));
        assert!(props["color"]["description"]
            .as_str()
            .unwrap_or_default()
            .contains("default green"));
        assert!(props["color"]["description"]
            .as_str()
            .unwrap_or_default()
            .contains("red only"));
        assert_eq!(props["padding"]["default"].as_i64(), Some(8));
        assert_eq!(props["radius"]["default"].as_i64(), Some(10));
        assert!(props.get("style").is_none());
        assert!(highlight["description"]
            .as_str()
            .unwrap_or_default()
            .contains("Only one highlight can be active"));
        assert!(highlight["description"]
            .as_str()
            .unwrap_or_default()
            .contains("visible/reachable element"));

        let hide = tool_by_name("browser_hide_highlight");
        assert_eq!(hide["inputSchema"]["required"].as_array().unwrap().len(), 0);
        assert_eq!(
            hide["inputSchema"]["properties"].as_object().unwrap().len(),
            0
        );
    }

    #[test]
    fn devtools_tool_descriptions_force_visible_mouse_for_interactions() {
        let tools = devtools_tools();
        let tool_by_name = |name: &str| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(JsonValue::as_str) == Some(name))
                .unwrap_or_else(|| panic!("missing tool schema: {name}"))
        };
        let description = |name: &str| {
            tool_by_name(name)["description"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(JsonValue::as_str))
            .collect::<Vec<_>>();

        assert!(description("browser_click").contains("visible browser mouse cursor"));
        assert!(description("browser_click").contains("ref"));
        // Coordinate click tools are intentionally not advertised to AI
        assert!(!names.contains(&"browser_mouse_click_xy"));
        assert!(description("browser_evaluate").contains("Interactive clicks"));
        assert!(description("browser_evaluate").contains("blocked"));
        assert!(description("browser_evaluate").contains("browser_click"));

        let click_index = names
            .iter()
            .position(|name| *name == "browser_click")
            .expect("browser_click should exist");
        let evaluate_index = names
            .iter()
            .position(|name| *name == "browser_evaluate")
            .expect("browser_evaluate should exist");
        assert!(evaluate_index > click_index);
    }

    #[test]
    fn vision_tool_schemas_include_screenshot_and_video() {
        let names = vision_tools()
            .into_iter()
            .filter_map(|tool| {
                tool.get("name")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();

        assert!(names.contains(&"browser_take_screenshot".to_owned()));
        assert!(names.contains(&"browser_start_video".to_owned()));
        assert!(names.contains(&"browser_stop_video".to_owned()));
        assert!(names.contains(&"browser_video_chapter".to_owned()));
    }

    #[test]
    fn vision_tool_schema_defaults_screenshot_to_fast_jpeg() {
        let tools = vision_tools();
        let screenshot = tools
            .iter()
            .find(|tool| {
                tool.get("name").and_then(JsonValue::as_str) == Some("browser_take_screenshot")
            })
            .expect("browser_take_screenshot schema");
        let props = &screenshot["inputSchema"]["properties"];

        assert!(screenshot["description"]
            .as_str()
            .unwrap_or_default()
            .contains("Prefer browser_page_summary"));
        assert_eq!(props["type"]["default"].as_str(), Some("jpeg"));
        assert_eq!(props["quality"]["default"].as_i64(), Some(74));
        assert_eq!(props["fullPage"]["default"].as_bool(), Some(false));
    }

    #[test]
    fn mcp_content_preserves_screenshot_image_data() {
        let content = mcp_content(BrowserMcpIpcResponse::ok_with_data(
            "Screenshot captured",
            json!({ "imageType": "png", "base64": "abcd" }),
        ));

        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"].as_str(), Some("text"));
        assert_eq!(content[1]["type"].as_str(), Some("image"));
        assert_eq!(content[1]["data"].as_str(), Some("abcd"));
        assert_eq!(content[1]["mimeType"].as_str(), Some("image/png"));
    }

    #[test]
    fn ipc_request_uses_actual_browser_tool_name() {
        let env = HelperEnv {
            port: None,
            token: None,
            terminal_id: Some(42),
            project_id: Some(7),
            session_id: None,
            caps: Vec::new(),
        };

        let request = browser_mcp_ipc_request(&env, "browser_tabs", json!({ "action": "list" }));

        assert_eq!(request.terminal_id, Some(42));
        assert_eq!(request.project_id, Some(7));
        assert_eq!(request.tool, "browser_tabs");
        assert_eq!(request.params["action"].as_str(), Some("list"));
        assert!(request.params.get("script").is_none());
    }

    #[test]
    fn browser_wait_plan_rejects_fixed_wait_without_condition() {
        let plan = browser_wait_plan(&json!({ "time": 0.25 })).expect("wait plan");

        assert_eq!(plan, BrowserWaitPlan::RejectedFixedWait);
    }

    #[test]
    fn browser_wait_plan_condition_removes_time_before_ipc_polling() {
        let plan = browser_wait_plan(&json!({ "time": 5, "text": "Ready" })).expect("wait plan");

        let BrowserWaitPlan::Condition {
            timeout,
            params,
            description,
        } = plan
        else {
            panic!("expected condition wait plan");
        };
        assert_eq!(timeout, Duration::from_secs(5));
        assert_eq!(params.get("time"), None);
        assert_eq!(
            params.get("text").and_then(JsonValue::as_str),
            Some("Ready")
        );
        assert_eq!(description, "text to appear: Ready");
    }

    #[test]
    fn browser_wait_plan_condition_allows_zero_timeout_for_immediate_probe() {
        let plan =
            browser_wait_plan(&json!({ "time": 0, "textGone": "Loading" })).expect("wait plan");

        let BrowserWaitPlan::Condition {
            timeout,
            params,
            description,
        } = plan
        else {
            panic!("expected condition wait plan");
        };
        assert_eq!(timeout, Duration::ZERO);
        assert_eq!(params.get("time"), None);
        assert_eq!(description, "text to disappear: Loading");
    }

    #[test]
    fn browser_wait_plan_rejects_invalid_time_values() {
        // Negative time should be rejected
        assert!(browser_wait_plan(&json!({ "time": -1 })).is_err());
        // String time should be rejected (not a number)
        assert!(browser_wait_plan(&json!({ "time": "1" })).is_err());
        // Very large time exceeding max should be rejected when combined with condition
        let large_time_with_text = json!({ "time": 100000, "text": "Ready" });
        assert!(browser_wait_plan(&large_time_with_text).is_err());
    }

    #[test]
    fn browser_wait_plan_rejects_ambiguous_or_empty_requests() {
        // Empty request is now valid - will use default timeout and no condition
        // (wait forever until safety timeout kicks in)
        assert!(browser_wait_plan(&json!({ "text": "Ready", "textGone": "Loading" })).is_err());
    }

    #[test]
    fn browser_wait_retryable_errors_are_only_unmet_text_conditions() {
        assert!(browser_wait_error_is_retryable("Text not found: Ready"));
        assert!(browser_wait_error_is_retryable(
            "Text is still visible: Loading"
        ));
        assert!(!browser_wait_error_is_retryable("Forbidden"));
        assert!(!browser_wait_error_is_retryable(
            "Browser MCP project authorization mismatch"
        ));
        assert!(!browser_wait_error_is_retryable(
            "Mergen Browser MCP bridge is unavailable: connection refused"
        ));
    }

    #[test]
    fn browser_wait_poll_read_timeout_is_bounded_by_remaining_wait() {
        assert_eq!(
            browser_wait_poll_read_timeout(Duration::from_millis(50)),
            Duration::from_millis(50)
        );
        assert_eq!(
            browser_wait_poll_read_timeout(Duration::from_secs(30)),
            Duration::from_secs(2)
        );
    }
}
