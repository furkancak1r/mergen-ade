#[allow(dead_code)]
#[path = "../browser_mcp_service.rs"]
mod browser_mcp_service;

use std::collections::BTreeMap;
use std::env;
use std::io::{self, BufRead, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use browser_mcp_service::{
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

fn main() {
    if let Err(err) = run() {
        let _ = writeln!(io::stderr(), "Mergen Browser MCP failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
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
                    "isError": is_error
                }
            })
        }),
        "ping" => id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
        method if method.starts_with("notifications/") => None,
        _ => id.map(|id| jsonrpc_error(id, -32601, format!("Unknown method: {method}"))),
    }
}

fn mcp_content(response: BrowserMcpIpcResponse) -> Vec<JsonValue> {
    let mut content = vec![json!({ "type": "text", "text": response.text })];
    if let Some(data) = response.data {
        let image_type = data
            .get("imageType")
            .and_then(JsonValue::as_str)
            .unwrap_or("png");
        if let Some(base64) = data.get("base64").and_then(JsonValue::as_str) {
            let mime_type = if image_type.eq_ignore_ascii_case("jpeg") {
                "image/jpeg"
            } else {
                "image/png"
            };
            content.push(json!({ "type": "image", "data": base64, "mimeType": mime_type }));
        } else {
            let rendered = serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string());
            content.push(json!({ "type": "text", "text": rendered }));
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
    call_mergen_once_with_timeout(env, tool, params, Duration::from_secs(95))
}

fn call_mergen_once_with_timeout(
    env: &HelperEnv,
    tool: &str,
    params: JsonValue,
    read_timeout: Duration,
) -> BrowserMcpIpcResponse {
    let Some(port) = env.port else {
        return BrowserMcpIpcResponse::error(format!(
            "Mergen Browser MCP is not connected: {MERGEN_BROWSER_MCP_PORT_ENV_VAR} is missing. Start OpenCode from Mergen ADE."
        ));
    };
    let Some(token) = env.token.as_deref() else {
        return BrowserMcpIpcResponse::error(format!(
            "Mergen Browser MCP is not connected: {MERGEN_BROWSER_MCP_TOKEN_ENV_VAR} is missing. Start OpenCode from Mergen ADE."
        ));
    };

    let request = BrowserMcpIpcRequest {
        request_id: request_id(),
        terminal_id: env.terminal_id,
        project_id: env.project_id,
        tool: tool.to_owned(),
        params,
    };
    match send_ipc_request(port, token, &request, read_timeout) {
        Ok(response) => response,
        Err(err) => BrowserMcpIpcResponse::error(format!(
            "Mergen Browser MCP bridge is unavailable: {err}. No external Chrome fallback was launched."
        )),
    }
}

fn call_browser_wait_for(env: &HelperEnv, params: JsonValue) -> BrowserMcpIpcResponse {
    let plan = match browser_wait_plan(&params) {
        Ok(plan) => plan,
        Err(err) => return BrowserMcpIpcResponse::error(err),
    };

    match plan {
        BrowserWaitPlan::Fixed { duration } => {
            std::thread::sleep(duration);
            BrowserMcpIpcResponse::ok(format!(
                "Waited {} seconds",
                format_duration_seconds(duration)
            ))
        }
        BrowserWaitPlan::Condition {
            timeout,
            params,
            description,
        } => {
            let started_at = Instant::now();
            let mut attempted_once = false;
            loop {
                let elapsed = started_at.elapsed();
                if attempted_once && elapsed >= timeout {
                    return BrowserMcpIpcResponse::error(format!(
                        "Timed out waiting for {description} after {} seconds",
                        format_duration_seconds(timeout)
                    ));
                }
                let remaining = timeout.saturating_sub(elapsed);
                let response = call_mergen_once_with_timeout(
                    env,
                    "browser_wait_for",
                    params.clone(),
                    browser_wait_poll_read_timeout(remaining),
                );
                attempted_once = true;
                if !response.is_error {
                    return response;
                }
                if !browser_wait_error_is_retryable(&response.text) {
                    return response;
                }

                let elapsed = started_at.elapsed();
                if elapsed >= timeout {
                    let suffix = if response.text.trim().is_empty() {
                        String::new()
                    } else {
                        format!(": {}", response.text)
                    };
                    return BrowserMcpIpcResponse::error(format!(
                        "Timed out waiting for {description} after {} seconds{suffix}",
                        format_duration_seconds(timeout)
                    ));
                }

                std::thread::sleep(
                    Duration::from_millis(BROWSER_WAIT_POLL_MS)
                        .min(timeout.saturating_sub(elapsed)),
                );
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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
    let text = non_empty_param(params, "text");
    let text_gone = non_empty_param(params, "textGone");
    if text.is_some() && text_gone.is_some() {
        return Err("browser_wait_for accepts either text or textGone, not both".to_owned());
    }

    let has_condition = text.is_some() || text_gone.is_some();
    let duration = parse_browser_wait_duration(params.get("time"), has_condition)?;
    if !has_condition {
        if params.get("time").is_none() {
            return Err("browser_wait_for requires time, text, or textGone".to_owned());
        }
        return Ok(BrowserWaitPlan::Fixed { duration });
    }

    let mut immediate_params = params.clone();
    if let JsonValue::Object(map) = &mut immediate_params {
        map.remove("time");
    }
    let description = if let Some(text) = text {
        format!("text to appear: {text}")
    } else {
        format!("text to disappear: {}", text_gone.unwrap_or_default())
    };

    Ok(BrowserWaitPlan::Condition {
        timeout: duration,
        params: immediate_params,
        description,
    })
}

fn non_empty_param<'a>(params: &'a JsonValue, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_browser_wait_duration(
    value: Option<&JsonValue>,
    default_when_missing: bool,
) -> Result<Duration, String> {
    let seconds = match value {
        Some(value) => value
            .as_f64()
            .ok_or_else(|| "browser_wait_for time must be a number of seconds".to_owned())?,
        None if default_when_missing => BROWSER_WAIT_DEFAULT_TIMEOUT_SECS,
        None => 0.0,
    };

    if !seconds.is_finite() || seconds < 0.0 {
        return Err("browser_wait_for time must be a finite non-negative number".to_owned());
    }
    let max_seconds =
        (DEFAULT_BROWSER_MCP_TIMEOUT_MS as f64 / 1000.0) - BROWSER_WAIT_TIMEOUT_MARGIN_SECS;
    if seconds > max_seconds {
        return Err(format!(
            "browser_wait_for time must be <= {} seconds",
            format_seconds(max_seconds)
        ));
    }

    Ok(Duration::from_secs_f64(seconds))
}

fn format_duration_seconds(duration: Duration) -> String {
    format_seconds(duration.as_secs_f64())
}

fn format_seconds(seconds: f64) -> String {
    let mut text = format!("{seconds:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn browser_wait_poll_read_timeout(remaining: Duration) -> Duration {
    remaining
        .min(Duration::from_secs(2))
        .max(Duration::from_millis(1))
}

fn browser_wait_error_is_retryable(message: &str) -> bool {
    message.starts_with("Text not found:") || message.starts_with("Text is still visible:")
}

fn send_ipc_request(
    port: u16,
    token: &str,
    request: &BrowserMcpIpcRequest,
    read_timeout: Duration,
) -> io::Result<BrowserMcpIpcResponse> {
    let body = serde_json::to_vec(request)?;
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(read_timeout))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let header = format!(
        "POST {MERGEN_BROWSER_MCP_ENDPOINT_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nX-Mergen-Browser-MCP-Token: {token}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    let _ = stream.shutdown(Shutdown::Write);
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let body_start = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|idx| idx + 4)
        .or_else(|| {
            raw.windows(2)
                .position(|window| window == b"\n\n")
                .map(|idx| idx + 2)
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing HTTP response body"))?;
    serde_json::from_slice::<BrowserMcpIpcResponse>(&raw[body_start..])
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
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
        "error": { "code": code, "message": message }
    })
}

fn tool_schemas(caps: &[String]) -> Vec<JsonValue> {
    let mut tools = core_tools();
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
    caps.iter().any(|existing| existing == cap)
}

fn tool(
    name: &str,
    description: &str,
    properties: JsonValue,
    required: &[&str],
    read_only: bool,
) -> JsonValue {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required
        },
        "annotations": { "readOnlyHint": read_only }
    })
}

fn core_tools() -> Vec<JsonValue> {
    vec![
        tool("browser_close", "Close the page", json!({}), &[], false),
        tool(
            "browser_resize",
            "Resize the browser window",
            json!({"width":{"type":"number"},"height":{"type":"number"}}),
            &["width", "height"],
            false,
        ),
        tool(
            "browser_console_messages",
            "Returns all console messages",
            json!({"level":{"type":"string","enum":["error","warning","info","debug"],"default":"info"},"all":{"type":"boolean"},"filename":{"type":"string"}}),
            &[],
            true,
        ),
        tool(
            "browser_snapshot",
            "Capture accessibility snapshot of the current page",
            json!({"target":{"type":"string"},"filename":{"type":"string"},"depth":{"type":"number"},"boxes":{"type":"boolean"}}),
            &[],
            true,
        ),
        tool(
            "browser_click",
            "Perform click on a web page",
            element_props(
                json!({"doubleClick":{"type":"boolean"},"button":{"type":"string","enum":["left","right","middle"]},"modifiers":{"type":"array","items":{"type":"string"}}}),
            ),
            &["target"],
            false,
        ),
        tool(
            "browser_drag",
            "Perform drag and drop between two elements",
            json!({"startElement":{"type":"string"},"startTarget":{"type":"string"},"endElement":{"type":"string"},"endTarget":{"type":"string"}}),
            &["startTarget", "endTarget"],
            false,
        ),
        tool(
            "browser_hover",
            "Hover over element on page",
            element_props(json!({})),
            &["target"],
            false,
        ),
        tool(
            "browser_select_option",
            "Select an option in a dropdown",
            element_props(json!({"values":{"type":"array","items":{"type":"string"}}})),
            &["target", "values"],
            false,
        ),
        tool(
            "browser_evaluate",
            "Evaluate JavaScript expression on page or element",
            element_props(json!({"function":{"type":"string"},"filename":{"type":"string"}})),
            &["function"],
            false,
        ),
        tool(
            "browser_fill_form",
            "Fill multiple form fields",
            json!({"fields":{"type":"array","items":{"type":"object"}}}),
            &["fields"],
            false,
        ),
        tool(
            "browser_press_key",
            "Press a key on the keyboard",
            json!({"key":{"type":"string"}}),
            &["key"],
            false,
        ),
        tool(
            "browser_type",
            "Type text into editable element",
            element_props(
                json!({"text":{"type":"string"},"submit":{"type":"boolean"},"slowly":{"type":"boolean"}}),
            ),
            &["target", "text"],
            false,
        ),
        tool(
            "browser_navigate",
            "Navigate to a URL",
            json!({"url":{"type":"string"}}),
            &["url"],
            false,
        ),
        tool(
            "browser_navigate_back",
            "Go back to the previous page in the history",
            json!({}),
            &[],
            false,
        ),
        tool(
            "browser_navigate_forward",
            "Go forward to the next page in the history",
            json!({}),
            &[],
            false,
        ),
        tool(
            "browser_reload",
            "Reload the current page",
            json!({}),
            &[],
            false,
        ),
        tool(
            "browser_network_requests",
            "Returns a numbered list of network requests since loading the page",
            json!({"static":{"type":"boolean","default":false},"filter":{"type":"string"},"filename":{"type":"string"}}),
            &[],
            true,
        ),
        tool(
            "browser_network_request",
            "Returns full details of a single network request",
            json!({"index":{"type":"integer"},"part":{"type":"string"},"filename":{"type":"string"}}),
            &["index"],
            true,
        ),
        tool(
            "browser_take_screenshot",
            "Take a screenshot of the current page",
            element_props(
                json!({"type":{"type":"string","enum":["png","jpeg"],"default":"png"},"filename":{"type":"string"},"fullPage":{"type":"boolean"}}),
            ),
            &[],
            true,
        ),
        tool(
            "browser_tabs",
            "List, create, close, or select a browser tab",
            json!({"action":{"type":"string","enum":["list","new","close","select"]},"index":{"type":"number"},"url":{"type":"string"}}),
            &["action"],
            false,
        ),
        tool(
            "browser_wait_for",
            "Wait for text to appear or disappear or a specified time to pass",
            json!({"time":{"type":"number"},"text":{"type":"string"},"textGone":{"type":"string"}}),
            &[],
            false,
        ),
    ]
}

fn devtools_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_highlight",
            "Show a persistent highlight overlay around the element",
            element_props(json!({"style":{"type":"string"}})),
            &["target"],
            true,
        ),
        tool(
            "browser_hide_highlight",
            "Remove a highlight overlay previously added for the element",
            element_props(json!({})),
            &[],
            true,
        ),
        tool(
            "browser_resume",
            "Resume paused script execution",
            json!({"step":{"type":"boolean"},"location":{"type":"string"}}),
            &[],
            false,
        ),
        tool(
            "browser_annotate",
            "Annotate the current page",
            json!({}),
            &[],
            true,
        ),
        tool(
            "browser_start_tracing",
            "Start trace recording",
            json!({}),
            &[],
            true,
        ),
        tool(
            "browser_stop_tracing",
            "Stop trace recording",
            json!({}),
            &[],
            true,
        ),
        tool(
            "browser_start_video",
            "Start video recording",
            json!({"filename":{"type":"string"},"size":{"type":"object"}}),
            &[],
            true,
        ),
        tool(
            "browser_stop_video",
            "Stop video recording",
            json!({}),
            &[],
            true,
        ),
        tool(
            "browser_video_chapter",
            "Add a chapter marker to the video recording",
            json!({"title":{"type":"string"},"description":{"type":"string"},"duration":{"type":"number"}}),
            &["title"],
            true,
        ),
    ]
}

fn vision_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_mouse_move_xy",
            "Move mouse to a given position",
            json!({"x":{"type":"number"},"y":{"type":"number"}}),
            &["x", "y"],
            false,
        ),
        tool(
            "browser_mouse_click_xy",
            "Click mouse button at a given position",
            json!({"x":{"type":"number"},"y":{"type":"number"},"button":{"type":"string"},"clickCount":{"type":"number"},"delay":{"type":"number"}}),
            &["x", "y"],
            false,
        ),
        tool(
            "browser_mouse_drag_xy",
            "Drag left mouse button to a given position",
            json!({"startX":{"type":"number"},"startY":{"type":"number"},"endX":{"type":"number"},"endY":{"type":"number"}}),
            &["startX", "startY", "endX", "endY"],
            false,
        ),
        tool(
            "browser_mouse_down",
            "Press mouse down",
            json!({"button":{"type":"string"}}),
            &[],
            false,
        ),
        tool(
            "browser_mouse_up",
            "Press mouse up",
            json!({"button":{"type":"string"}}),
            &[],
            false,
        ),
        tool(
            "browser_mouse_wheel",
            "Scroll mouse wheel",
            json!({"deltaX":{"type":"number","default":0},"deltaY":{"type":"number","default":0}}),
            &[],
            false,
        ),
    ]
}

fn network_tools() -> Vec<JsonValue> {
    vec![
        tool(
            "browser_network_state_set",
            "Sets the browser network state",
            json!({"state":{"type":"string","enum":["online","offline"]}}),
            &["state"],
            false,
        ),
        tool(
            "browser_route",
            "Set up a route to mock network requests",
            json!({"pattern":{"type":"string"},"status":{"type":"number"},"body":{"type":"string"},"contentType":{"type":"string"},"headers":{"type":"array","items":{"type":"string"}},"removeHeaders":{"type":"string"}}),
            &["pattern"],
            false,
        ),
        tool(
            "browser_route_list",
            "List all active network routes",
            json!({}),
            &[],
            true,
        ),
        tool(
            "browser_unroute",
            "Remove network routes",
            json!({"pattern":{"type":"string"}}),
            &[],
            false,
        ),
    ]
}

fn storage_tools() -> Vec<JsonValue> {
    let mut tools = Vec::new();
    for name in ["localstorage", "sessionstorage"] {
        tools.push(tool(
            &format!("browser_{name}_list"),
            "List storage key-value pairs",
            json!({}),
            &[],
            true,
        ));
        tools.push(tool(
            &format!("browser_{name}_get"),
            "Get a storage item by key",
            json!({"key":{"type":"string"}}),
            &["key"],
            true,
        ));
        tools.push(tool(
            &format!("browser_{name}_set"),
            "Set a storage item",
            json!({"key":{"type":"string"},"value":{"type":"string"}}),
            &["key", "value"],
            false,
        ));
        tools.push(tool(
            &format!("browser_{name}_delete"),
            "Delete a storage item",
            json!({"key":{"type":"string"}}),
            &["key"],
            false,
        ));
        tools.push(tool(
            &format!("browser_{name}_clear"),
            "Clear storage",
            json!({}),
            &[],
            false,
        ));
    }
    tools.push(tool(
        "browser_cookie_list",
        "List cookies",
        json!({"domain":{"type":"string"},"path":{"type":"string"}}),
        &[],
        true,
    ));
    tools.push(tool(
        "browser_cookie_get",
        "Get a specific cookie by name",
        json!({"name":{"type":"string"}}),
        &["name"],
        true,
    ));
    tools.push(tool("browser_cookie_set", "Set a cookie", json!({"name":{"type":"string"},"value":{"type":"string"},"domain":{"type":"string"},"path":{"type":"string"},"expires":{"type":"number"},"httpOnly":{"type":"boolean"},"secure":{"type":"boolean"},"sameSite":{"type":"string"}}), &["name", "value"], false));
    tools.push(tool(
        "browser_cookie_delete",
        "Delete a specific cookie",
        json!({"name":{"type":"string"}}),
        &["name"],
        false,
    ));
    tools.push(tool(
        "browser_cookie_clear",
        "Clear all cookies",
        json!({}),
        &[],
        false,
    ));
    tools.push(tool(
        "browser_storage_state",
        "Save storage state",
        json!({"filename":{"type":"string"}}),
        &[],
        true,
    ));
    tools.push(tool(
        "browser_set_storage_state",
        "Restore storage state",
        json!({"filename":{"type":"string"}}),
        &["filename"],
        false,
    ));
    tools
}

fn element_props(extra: JsonValue) -> JsonValue {
    let mut map = BTreeMap::new();
    map.insert("element".to_owned(), json!({"type":"string"}));
    map.insert("target".to_owned(), json!({"type":"string"}));
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
        assert!(names.contains(&"browser_snapshot".to_owned()));
        assert!(names.contains(&"browser_click".to_owned()));
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
