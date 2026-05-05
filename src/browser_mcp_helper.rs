use std::collections::BTreeMap;
use std::env;
use std::io::{self, BufRead, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::browser_mcp_service::{
    BrowserMcpIpcRequest, BrowserMcpIpcResponse, DEFAULT_BROWSER_MCP_TIMEOUT_MS,
    MERGEN_BROWSER_MCP_ENDPOINT_PATH, MERGEN_BROWSER_MCP_PORT_ENV_VAR,
    MERGEN_BROWSER_MCP_PROJECT_ID_ENV_VAR, MERGEN_BROWSER_MCP_TERMINAL_ID_ENV_VAR,
    MERGEN_BROWSER_MCP_TOKEN_ENV_VAR,
};
use serde_json::{json, Value as JsonValue};

const SERVER_NAME: &str = "mergen-browser-mcp";
const PROTOCOL_VERSION: &str = "2024-11-05";
const BROWSER_WAIT_DEFAULT_TIMEOUT_SECS: f64 = 30.0;
const BROWSER_WAIT_POLL_MS: u64 = 100;
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
                    "instructions": "Controls the embedded Mergen ADE Browser panel. It never launches external Chrome."
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
        BrowserWaitPlan::Fixed { duration } => {
            std::thread::sleep(duration);
            BrowserMcpIpcResponse::ok(format!("Waited for {}.", format_duration_seconds(duration)))
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
    Fixed {
        duration: Duration,
    },
    Condition {
        timeout: Duration,
        params: JsonValue,
        description: String,
    },
}

fn browser_wait_plan(params: &JsonValue) -> Result<BrowserWaitPlan, String> {
    let time_num = params.get("time").and_then(JsonValue::as_f64);
    let text = non_empty_param(params, "text");
    let text_gone = non_empty_param(params, "textGone");
    if text.is_some() && text_gone.is_some() {
        return Err(
            "browser_wait_for cannot have both 'text' and 'textGone' conditions at once."
                .to_owned(),
        );
    }
    let max = Duration::from_millis(DEFAULT_BROWSER_MCP_TIMEOUT_MS);
    let is_fixed_wait = text.is_none() && text_gone.is_none();
    let timeout = time_num
        .map(|t| parse_browser_wait_duration(t, max, !is_fixed_wait))
        .transpose()?;
    if is_fixed_wait {
        match timeout {
            Some(d) if d > Duration::ZERO => {
                return Ok(BrowserWaitPlan::Fixed { duration: d });
            }
            _ => {
                return Err(
                    "browser_wait_for requires 'time' (seconds) for a fixed wait, or 'text'/'textGone' for conditional waits."
                        .to_owned(),
                );
            }
        }
    }
    let mut poll_params = params.clone();
    if let Some(obj) = poll_params.as_object_mut() {
        obj.remove("time");
    }
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
            "Close the browser. Not supported in Mergen (Mergen never launches external Chrome).",
            json!({
                "type": "object",
                "properties": {},
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
            "Navigate to a URL in the browser",
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
            "Go back in the browser history",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_navigate_forward",
            "Go forward in the browser history",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        tool(
            "browser_reload",
            "Reload the current page",
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
            "List browser tabs (Mergen Browser panel only).",
            json!({
                "type": "object",
                "properties": {
                    "action": json!({"type": "string", "enum": ["list"], "default": "list"})
                },
                "required": []
            }),
        ),
        tool(
            "browser_type",
            "Type text into a focused element",
            json!({
                "type": "object",
                "properties": {
                    "text": json!({"type": "string"}),
                    "submit": json!({"type": "boolean"})
                },
                "required": ["text"]
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
            "browser_evaluate",
            "Execute JavaScript in the browser",
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
            "browser_hide_highlight",
            "Hide element highlight",
            json!({
                "type": "object",
                "properties": {
                    "ref": json!({"type": "string"})
                },
                "required": []
            }),
        ),
        tool(
            "browser_highlight",
            "Highlight an element on the page",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string"}),
                    "color": json!({"type": "string"}),
                    "label": json!({"type": "string"})
                }))
            }),
        ),
        tool(
            "browser_select_option",
            "Select an option in a dropdown",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string"}),
                    "value": json!({"type": "string"})
                })),
                "required": ["value"]
            }),
        ),
        tool(
            "browser_snapshot",
            "Take a DOM snapshot (accessibility snapshot) of the page",
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
            "Click an element on the page",
            json!({
                "type": "object",
                "properties": element_props(json!({
                    "ref": json!({"type": "string"})
                }))
            }),
        ),
        tool(
            "browser_wait_for",
            "Wait for a condition (time, text, or textGone). Fixed waits run in helper process; conditions are polled against the browser.",
            json!({
                "type": "object",
                "properties": {
                    "time": json!({"type": "number", "description": "Wait duration in seconds (required for fixed waits)"}),
                    "text": json!({"type": "string", "description": "Wait until text appears on page"}),
                    "textGone": json!({"type": "string", "description": "Wait until text disappears from page"})
                },
                "required": []
            }),
        ),
    ]
}

fn vision_tools() -> Vec<JsonValue> {
    vec![tool(
        "browser_take_screenshot",
        "Take a screenshot of the browser page",
        json!({
            "type": "object",
            "properties": element_props(json!({
                "ref": json!({"type": "string"}),
                "width": json!({"type": "integer"}),
                "height": json!({"type": "integer"})
            }))
        }),
    )]
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
        assert!(names.contains(&"browser_click".to_owned()));
        assert!(names.contains(&"browser_evaluate".to_owned()));
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
    fn browser_wait_plan_fixed_wait_uses_time_duration() {
        let plan = browser_wait_plan(&json!({ "time": 0.25 })).expect("wait plan");

        assert_eq!(
            plan,
            BrowserWaitPlan::Fixed {
                duration: Duration::from_millis(250)
            }
        );
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
        assert!(browser_wait_plan(&json!({ "time": -1 })).is_err());
        assert!(browser_wait_plan(&json!({ "time": "1" })).is_err());
        assert!(browser_wait_plan(&json!({ "time": DEFAULT_BROWSER_MCP_TIMEOUT_MS })).is_err());
    }

    #[test]
    fn browser_wait_plan_rejects_ambiguous_or_empty_requests() {
        assert!(browser_wait_plan(&json!({})).is_err());
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
