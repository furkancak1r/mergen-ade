//! Embedded browser module for Mergen ADE.
//!
//! Provides a target-gated facade for embedded WebView functionality.
//! - Windows: Uses WebView2 for native browser rendering
//! - Non-Windows: Safe stub that reports unsupported status

use eframe::egui;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Bounds for the browser view in physical pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BrowserBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Events emitted by the embedded browser.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserEvent {
    UrlChanged(String),
    LoadStarted(String),
    LoadFinished(String),
    DesignInspectReady,
    DesignElementClicked(DesignElementInfo),
    McpToolResult {
        request_id: String,
        result: Result<BrowserMcpToolOutput, String>,
    },
    Error(String),
}

/// Element data captured by the browser design inspect mode.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DesignElementInfo {
    pub page_url: String,
    pub url: String,
    pub tag: String,
    pub id: String,
    pub classes: Vec<String>,
    pub text: String,
    pub selector: String,
    pub rect: DesignElementRect,
    pub styles: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct DesignElementRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserMcpToolOutput {
    pub text: String,
    pub data: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
struct DesignInspectWireMessage {
    source: String,
    #[serde(default)]
    token: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    #[serde(rename = "pageUrl")]
    page_url: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    classes: Vec<String>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    selector: String,
    rect: Option<DesignElementRect>,
    #[serde(default)]
    styles: BTreeMap<String, String>,
}

const DESIGN_INSPECT_MESSAGE_SOURCE: &str = "mergen-ade-design-inspect";
const SCREENSHOT_DEFAULT_JPEG_QUALITY: u8 = 74;
static DESIGN_INSPECT_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn screenshot_format_from_params(params: &JsonValue) -> &'static str {
    let image_type = params
        .get("type")
        .and_then(JsonValue::as_str)
        .unwrap_or("jpeg");
    if image_type.eq_ignore_ascii_case("png") {
        "png"
    } else if image_type.eq_ignore_ascii_case("jpg") || image_type.eq_ignore_ascii_case("jpeg") {
        "jpeg"
    } else {
        "jpeg"
    }
}

fn screenshot_quality_from_params(params: &JsonValue) -> u8 {
    params
        .get("quality")
        .and_then(JsonValue::as_i64)
        .map(|quality| quality.clamp(1, 100) as u8)
        .unwrap_or(SCREENSHOT_DEFAULT_JPEG_QUALITY)
}

fn screenshot_cdp_params(params: &JsonValue) -> (&'static str, JsonValue) {
    screenshot_cdp_params_with_surface(params, true)
}

fn screenshot_cdp_params_with_surface(
    params: &JsonValue,
    from_surface: bool,
) -> (&'static str, JsonValue) {
    let format = screenshot_format_from_params(params);
    let capture_beyond_viewport = params
        .get("fullPage")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let mut cdp_params = json!({
        "format": format,
        "captureBeyondViewport": capture_beyond_viewport,
        "fromSurface": from_surface,
        "optimizeForSpeed": true
    });
    if format == "jpeg" {
        cdp_params["quality"] = json!(screenshot_quality_from_params(params));
    }
    (format, cdp_params)
}

#[cfg(target_os = "windows")]
fn screenshot_preflight_runtime_params() -> JsonValue {
    json!({
        "expression": "(() => new Promise((resolve) => { let done = false; const finish = () => { if (done) return; done = true; resolve({ readyState: document.readyState, visibilityState: document.visibilityState, width: window.innerWidth, height: window.innerHeight, url: location.href }); }; requestAnimationFrame(() => requestAnimationFrame(finish)); setTimeout(finish, 90); }))()",
        "awaitPromise": true,
        "returnByValue": true
    })
}

pub(crate) fn browser_mcp_tool_uses_async_script(tool: &str) -> bool {
    matches!(
        tool,
        "browser_evaluate"
            | "browser_page_summary"
            | "browser_click"
            | "browser_hover"
            | "browser_select_option"
            | "browser_fill_form"
            | "browser_press_key"
            | "browser_type"
            | "browser_mouse_move_xy"
            | "browser_mouse_click_xy"
            | "browser_mouse_drag_xy"
            | "browser_mouse_down"
            | "browser_mouse_up"
            | "browser_mouse_wheel"
    )
}

#[cfg(target_os = "windows")]
fn browser_mcp_script_expression(tool: &str, params: &JsonValue) -> Result<String, String> {
    let payload = json!({ "tool": tool, "params": params });
    let payload_json = serde_json::to_string(&payload).map_err(|err| err.to_string())?;
    Ok(format!(
        "(() => {{ const __mergenMcpPayload = {payload_json}; {} return window.__mergenMcpRun(__mergenMcpPayload.tool, __mergenMcpPayload.params); }})()",
        MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
    ))
}

#[cfg(target_os = "windows")]
fn browser_mcp_runtime_evaluate_params(
    tool: &str,
    params: &JsonValue,
) -> Result<JsonValue, String> {
    Ok(json!({
        "expression": browser_mcp_script_expression(tool, params)?,
        "awaitPromise": true,
        "returnByValue": true,
        "userGesture": true
    }))
}

pub(crate) fn browser_mcp_screenshot_output_from_devtools_raw(
    raw: &str,
    format: &str,
) -> Result<BrowserMcpToolOutput, String> {
    browser_mcp_screenshot_output_from_devtools_raw_with_metadata(raw, format, 0, None)
}

fn browser_mcp_screenshot_output_from_devtools_raw_with_metadata(
    raw: &str,
    format: &str,
    retry_count: u8,
    elapsed: Option<std::time::Duration>,
) -> Result<BrowserMcpToolOutput, String> {
    let parsed = serde_json::from_str::<JsonValue>(&raw).unwrap_or_else(|_| json!({}));
    let data = parsed
        .get("data")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    if data.is_empty() {
        return Err("WebView2 did not return screenshot data".to_owned());
    }
    Ok(BrowserMcpToolOutput {
        text: format!(
            "Screenshot captured from the embedded Mergen browser ({} {}, {} base64 bytes, {} retries).",
            format,
            elapsed
                .map(|duration| format!("{}ms", duration.as_millis()))
                .unwrap_or_else(|| "elapsed unknown".to_owned()),
            data.len(),
            retry_count
        ),
        data: Some(json!({
            "imageType": format,
            "base64": data,
            "retryCount": retry_count,
            "elapsedMs": elapsed.map(|duration| duration.as_millis() as u64)
        })),
    })
}

fn browser_mcp_screenshot_retry_reason_from_devtools_raw(raw: &str) -> Option<&'static str> {
    let parsed = serde_json::from_str::<JsonValue>(raw).ok()?;
    let data = parsed.get("data").and_then(JsonValue::as_str)?;
    if screenshot_base64_is_probably_black(data) {
        Some("near-black frame")
    } else {
        None
    }
}

fn screenshot_base64_is_probably_black(base64_data: &str) -> bool {
    use base64::Engine as _;

    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(base64_data) else {
        return false;
    };
    screenshot_bytes_are_probably_black(bytes.as_slice())
}

fn screenshot_bytes_are_probably_black(bytes: &[u8]) -> bool {
    let Ok(image) = image::load_from_memory(bytes) else {
        return false;
    };
    let image = image.to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return false;
    }

    let columns = width.min(12);
    let rows = height.min(8);
    let mut samples = 0u64;
    let mut transparent_samples = 0u64;
    let mut luma_sum = 0u64;
    let mut luma_sq_sum = 0u64;
    let mut max_channel = 0u8;

    for row in 0..rows {
        let y = ((row as u64 * height as u64) / rows as u64).min(height as u64 - 1) as u32;
        for column in 0..columns {
            let x = ((column as u64 * width as u64) / columns as u64).min(width as u64 - 1) as u32;
            let pixel = image.get_pixel(x, y);
            let [red, green, blue, alpha] = pixel.0;
            samples += 1;
            if alpha < 8 {
                transparent_samples += 1;
            }
            max_channel = max_channel.max(red).max(green).max(blue);
            let luma = (red as u64 * 299 + green as u64 * 587 + blue as u64 * 114) / 1000;
            luma_sum += luma;
            luma_sq_sum += luma * luma;
        }
    }

    if samples == 0 {
        return false;
    }
    if transparent_samples == samples {
        return true;
    }

    let mean = luma_sum as f64 / samples as f64;
    let variance = (luma_sq_sum as f64 / samples as f64) - (mean * mean);
    max_channel <= 12 && mean <= 8.0 && variance <= 10.0
}

fn new_design_inspect_token() -> String {
    let counter = DESIGN_INSPECT_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("mdi-{nanos:x}-{counter:x}")
}

#[cfg(target_os = "windows")]
fn design_inspect_bootstrap_script(token: &str) -> String {
    let token_json = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".to_owned());
    DESIGN_INSPECT_BOOTSTRAP_SCRIPT_TEMPLATE.replace("__MERGEN_DESIGN_INSPECT_TOKEN__", &token_json)
}

#[cfg(target_os = "windows")]
const DESIGN_INSPECT_BOOTSTRAP_SCRIPT_TEMPLATE: &str = r#"
(() => {
  const SOURCE = "mergen-ade-design-inspect";
  const HOST_SOURCE = "mergen-ade-host";
  const TOKEN = __MERGEN_DESIGN_INSPECT_TOKEN__;
  const webview = window.chrome?.webview;
  const postMessage = webview && typeof webview.postMessage === "function" ? webview.postMessage.bind(webview) : null;
  const addWebMessageListener = webview && typeof webview.addEventListener === "function" ? webview.addEventListener.bind(webview) : null;
  if (window.__mergenDesignInspect && window.__mergenDesignInspect.version === 2) {
    postMessage?.(JSON.stringify({ source: SOURCE, token: TOKEN, type: "ready" }));
    return;
  }

  const state = {
    version: 2,
    enabled: false,
    current: null,
    overlay: null,
  };

  const importantStyleKeys = [
    "display", "position", "margin", "padding", "font", "color",
    "background-color", "border", "border-radius", "z-index", "transform"
  ];

  function clean(value, max) {
    const text = String(value ?? "").replace(/[\u0000-\u001f\u007f]+/g, " ").replace(/\s+/g, " ").trim();
    return text.length > max ? `${text.slice(0, max)}...` : text;
  }

  function cssEscape(value) {
    if (window.CSS && typeof window.CSS.escape === "function") {
      return window.CSS.escape(value);
    }
    return String(value).replace(/[^a-zA-Z0-9_-]/g, "\\$&");
  }

  function selectorFor(element) {
    if (!element || element.nodeType !== Node.ELEMENT_NODE) return "";
    const parts = [];
    let node = element;
    while (node && node.nodeType === Node.ELEMENT_NODE && parts.length < 5) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        part += `#${cssEscape(node.id)}`;
        parts.unshift(part);
        break;
      }
      const classNames = Array.from(node.classList || []).slice(0, 3);
      if (classNames.length) {
        part += classNames.map((name) => `.${cssEscape(name)}`).join("");
      }
      const parent = node.parentElement;
      if (parent) {
        const siblings = Array.from(parent.children).filter((child) => child.tagName === node.tagName);
        if (siblings.length > 1) {
          part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
        }
      }
      parts.unshift(part);
      node = parent;
    }
    return parts.join(" > ");
  }

  function ensureOverlay() {
    if (state.overlay) return state.overlay;
    const overlay = document.createElement("div");
    overlay.setAttribute("data-mergen-design-inspect", "overlay");
    Object.assign(overlay.style, {
      position: "fixed",
      pointerEvents: "none",
      zIndex: "2147483647",
      border: "2px solid #f59e0b",
      background: "rgba(245, 158, 11, 0.10)",
      boxShadow: "0 0 0 1px rgba(0, 0, 0, 0.35)",
      borderRadius: "3px",
      display: "none",
    });
    document.documentElement.appendChild(overlay);
    state.overlay = overlay;
    return overlay;
  }

  function updateOverlay(element) {
    const overlay = ensureOverlay();
    const rect = element.getBoundingClientRect();
    Object.assign(overlay.style, {
      display: rect.width > 0 && rect.height > 0 ? "block" : "none",
      left: `${Math.round(rect.left)}px`,
      top: `${Math.round(rect.top)}px`,
      width: `${Math.round(rect.width)}px`,
      height: `${Math.round(rect.height)}px`,
    });
  }

  function hideOverlay() {
    if (state.overlay) state.overlay.style.display = "none";
  }

  function elementPayload(element, kind) {
    const rect = element.getBoundingClientRect();
    const computed = window.getComputedStyle(element);
    const styles = {};
    for (const key of importantStyleKeys) {
      styles[key] = clean(computed.getPropertyValue(key), 180);
    }
    const frameUrl = String(window.location.href || "");
    let pageUrl = "";
    try {
      pageUrl = window.top?.location?.href ? String(window.top.location.href) : "";
    } catch (_) {
      pageUrl = String(document.referrer || "");
    }
    return {
      source: SOURCE,
      token: TOKEN,
      type: kind,
      pageUrl: pageUrl || frameUrl,
      url: frameUrl,
      tag: clean(element.tagName.toLowerCase(), 40),
      id: clean(element.id || "", 120),
      classes: Array.from(element.classList || []).slice(0, 8).map((name) => clean(name, 80)).filter(Boolean),
      text: clean(element.innerText || element.textContent || "", 220),
      selector: clean(selectorFor(element), 500),
      rect: {
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      },
      styles,
    };
  }

  function postSelection(element) {
    if (!postMessage || !element) return;
    postMessage(JSON.stringify(elementPayload(element, "click")));
  }

  function onPointerMove(event) {
    if (!state.enabled) return;
    const element = event.target;
    if (!element || element === state.overlay || element.nodeType !== Node.ELEMENT_NODE) return;
    state.current = element;
    updateOverlay(element);
  }

  function onInspectClick(event) {
    if (!state.enabled) return;
    // Only handle primary button (left click)
    if (event.button !== 0) return;
    // Prevent page actions (navigation, button handlers, form submission)
    event.preventDefault();
    event.stopPropagation();
    if (event.stopImmediatePropagation) event.stopImmediatePropagation();
    // Use event.target if available, otherwise fallback to tracked current element
    const element = event.target || state.current;
    if (!element || element === state.overlay || element.nodeType !== Node.ELEMENT_NODE) return;
    postSelection(element);
  }

  function refreshOverlay() {
    if (state.enabled && state.current && document.documentElement.contains(state.current)) {
      updateOverlay(state.current);
    } else {
      hideOverlay();
    }
  }

  function setEnabled(enabled) {
    state.enabled = Boolean(enabled);
    if (state.enabled) {
      document.addEventListener("pointermove", onPointerMove, true);
      document.addEventListener("click", onInspectClick, true);
      window.addEventListener("scroll", refreshOverlay, true);
      window.addEventListener("resize", refreshOverlay, true);
    } else {
      document.removeEventListener("pointermove", onPointerMove, true);
      document.removeEventListener("click", onInspectClick, true);
      window.removeEventListener("scroll", refreshOverlay, true);
      window.removeEventListener("resize", refreshOverlay, true);
      state.current = null;
      hideOverlay();
    }
  }

  addWebMessageListener?.("message", (event) => {
    const data = typeof event.data === "string" ? JSON.parse(event.data) : event.data;
    if (!data || data.source !== HOST_SOURCE || data.type !== "setDesignInspectEnabled") return;
    setEnabled(data.enabled);
  });

  window.__mergenDesignInspect = { version: 2, setEnabled };
  postMessage?.(JSON.stringify({ source: SOURCE, token: TOKEN, type: "ready" }));
})();
"#;

pub(crate) fn parse_design_inspect_message(
    message: &str,
    expected_token: &str,
) -> Option<BrowserEvent> {
    let wire: DesignInspectWireMessage = serde_json::from_str(message).ok()?;
    if wire.source != DESIGN_INSPECT_MESSAGE_SOURCE || wire.token != expected_token {
        return None;
    }

    match wire.kind.as_str() {
        "ready" => Some(BrowserEvent::DesignInspectReady),
        "click" => {
            if wire.tag.trim().is_empty() || wire.selector.trim().is_empty() {
                return None;
            }
            Some(BrowserEvent::DesignElementClicked(DesignElementInfo {
                page_url: wire.page_url,
                url: wire.url,
                tag: wire.tag,
                id: wire.id,
                classes: wire.classes,
                text: wire.text,
                selector: wire.selector,
                rect: wire.rect?,
                styles: wire.styles,
            }))
        }
        _ => None,
    }
}

/// Status of the embedded browser.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserStatus {
    Unsupported(&'static str),
    Uninitialized,
    Creating,
    Ready,
    Failed(String),
}

/// Facade for embedded browser functionality.
/// Target-gated implementation: Windows uses WebView2, others use stub.
pub struct EmbeddedBrowser {
    status: BrowserStatus,
    event_sender: Sender<BrowserEvent>,
    event_receiver: Receiver<BrowserEvent>,
    /// Last requested visibility state (for testability and state tracking)
    requested_visible: bool,
    design_inspect_enabled: bool,
    design_inspect_token: String,
    user_data_folder: Option<PathBuf>,
    #[cfg(target_os = "windows")]
    inner: Option<WindowsWebView>,
    #[cfg(target_os = "windows")]
    pending_url: Option<String>,
    #[cfg(target_os = "windows")]
    pending_bounds: Option<BrowserBounds>,
    #[cfg(target_os = "windows")]
    pending_visibility: Option<bool>,
}

impl EmbeddedBrowser {
    /// Create a new embedded browser instance.
    pub fn new() -> Self {
        Self::new_with_user_data_folder(None)
    }

    /// Create a new embedded browser instance with a persistent WebView2 user data folder.
    pub fn new_with_user_data_folder(user_data_folder: Option<PathBuf>) -> Self {
        let (event_sender, event_receiver) = channel();

        Self {
            status: BrowserStatus::Uninitialized,
            event_sender,
            event_receiver,
            requested_visible: false,
            design_inspect_enabled: false,
            design_inspect_token: new_design_inspect_token(),
            user_data_folder,
            #[cfg(target_os = "windows")]
            inner: None,
            #[cfg(target_os = "windows")]
            pending_url: None,
            #[cfg(target_os = "windows")]
            pending_bounds: None,
            #[cfg(target_os = "windows")]
            pending_visibility: None,
        }
    }

    /// Ensure the browser is created with the given parent window handle.
    /// Returns the current status after creation attempt.
    #[cfg(target_os = "windows")]
    pub fn ensure_created(&mut self, parent_hwnd: Option<isize>) -> BrowserStatus {
        use windows::Win32::Foundation::HWND;

        if matches!(self.status, BrowserStatus::Ready | BrowserStatus::Creating) {
            return self.status.clone();
        }

        let Some(hwnd) = parent_hwnd else {
            let err_msg = "No parent window handle available".to_owned();
            log::error!("{}", err_msg);
            self.report_error(err_msg);
            return self.status.clone();
        };

        self.status = BrowserStatus::Creating;

        // Create WebView2 environment using synchronous helper
        let hwnd = HWND(hwnd as *mut std::ffi::c_void);
        let result = create_webview_sync(
            hwnd,
            self.event_sender.clone(),
            self.design_inspect_token.clone(),
            self.user_data_folder.clone(),
        );

        match result {
            Ok((
                controller,
                webview,
                source_changed_token,
                web_message_received_token,
                navigation_starting_token,
                content_loading_token,
                navigation_completed_token,
            )) => {
                self.inner = Some(WindowsWebView {
                    controller,
                    webview,
                    source_changed_token: Some(source_changed_token),
                    web_message_received_token: Some(web_message_received_token),
                    navigation_starting_token: Some(navigation_starting_token),
                    content_loading_token: Some(content_loading_token),
                    navigation_completed_token: Some(navigation_completed_token),
                });
                self.status = BrowserStatus::Ready;
                log::info!("WebView2 created successfully");

                self.set_design_inspect_enabled_internal(self.design_inspect_enabled);

                // Apply any pending operations
                if let Some(url) = self.pending_url.take() {
                    self.navigate_internal(&url);
                }
                if let Some(bounds) = self.pending_bounds.take() {
                    self.sync_position_internal(&bounds);
                }
                if let Some(visible) = self.pending_visibility.take() {
                    self.set_visible_internal(visible);
                }
            }
            Err(e) => {
                let err_msg = format!("WebView2 creation failed: {}", e);
                log::error!("{}", err_msg);
                self.report_error(err_msg);
            }
        }

        self.status.clone()
    }

    #[cfg(not(target_os = "windows"))]
    pub fn ensure_created(&mut self, _parent_hwnd: Option<isize>) -> BrowserStatus {
        self.status = BrowserStatus::Unsupported("Embedded browser is currently Windows-only");
        self.status.clone()
    }

    /// Synchronize the browser position with the given bounds.
    /// Called after UI render to update native WebView position.
    pub fn sync_position(&mut self, bounds: &BrowserBounds) {
        #[cfg(target_os = "windows")]
        {
            if self.inner.is_some() {
                self.sync_position_internal(bounds);
            } else {
                self.pending_bounds = Some(*bounds);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = bounds;
        }
    }

    #[cfg(target_os = "windows")]
    fn sync_position_internal(&self, bounds: &BrowserBounds) {
        use windows::Win32::Foundation::RECT;

        if let Some(inner) = &self.inner {
            let rect = RECT {
                left: bounds.x,
                top: bounds.y,
                right: bounds.x + bounds.width,
                bottom: bounds.y + bounds.height,
            };
            unsafe {
                let _ = inner.controller.SetBounds(rect);
            }
            log::debug!(
                "WebView2 bounds synced: x={}, y={}, w={}, h={}",
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height
            );
        }
    }

    /// Hide the browser view.
    /// Called when the browser panel is collapsed.
    pub fn hide(&mut self) {
        self.set_visible(false);
    }

    /// Show the browser view.
    pub fn show(&mut self) {
        self.set_visible(true);
    }

    /// Enable or disable browser design inspect mode for the current page.
    pub fn set_design_inspect_enabled(&mut self, enabled: bool) {
        self.design_inspect_enabled = enabled;
        #[cfg(target_os = "windows")]
        {
            if self.inner.is_some() {
                self.set_design_inspect_enabled_internal(enabled);
            }
        }
    }

    pub fn design_inspect_enabled(&self) -> bool {
        self.design_inspect_enabled
    }

    #[cfg(target_os = "windows")]
    fn set_design_inspect_enabled_internal(&self, enabled: bool) {
        if let Some(inner) = &self.inner {
            let json = if enabled {
                r#"{"source":"mergen-ade-host","type":"setDesignInspectEnabled","enabled":true}"#
            } else {
                r#"{"source":"mergen-ade-host","type":"setDesignInspectEnabled","enabled":false}"#
            };
            let json = windows::core::HSTRING::from(json);
            unsafe {
                let _ = inner.webview.PostWebMessageAsJson(&json);
            }
        }
    }

    /// Set the visibility of the browser view.
    pub fn set_visible(&mut self, visible: bool) {
        self.requested_visible = visible;
        #[cfg(target_os = "windows")]
        {
            if self.inner.is_some() {
                self.set_visible_internal(visible);
            } else {
                self.pending_visibility = Some(visible);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::debug!("Embedded browser visible: {}", visible);
        }
    }

    #[cfg(target_os = "windows")]
    fn set_visible_internal(&self, visible: bool) {
        use windows::Win32::Foundation::BOOL;

        if let Some(inner) = &self.inner {
            unsafe {
                let _ = inner.controller.SetIsVisible(BOOL::from(visible));
            }
            log::debug!("WebView2 visibility set to: {}", visible);
        }
    }

    fn report_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        let _ = self.event_sender.send(BrowserEvent::Error(message.clone()));
        self.status = BrowserStatus::Failed(message);
    }

    /// Navigate to the given URL.
    pub fn navigate(&mut self, url: &str) {
        #[cfg(target_os = "windows")]
        {
            if self.inner.is_some() {
                self.navigate_internal(url);
            } else {
                // Store for later when browser is ready
                self.pending_url = Some(url.to_owned());
                log::info!("WebView2 navigation queued: {}", url);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::info!("Embedded browser navigate (stub): {}", url);
        }
    }

    #[cfg(target_os = "windows")]
    fn navigate_internal(&mut self, url: &str) {
        let navigation_result = self.inner.as_ref().map(|inner| {
            let url_hstring = windows::core::HSTRING::from(url);
            unsafe { inner.webview.Navigate(&url_hstring) }
        });

        if let Some(result) = navigation_result {
            match result {
                Ok(_) => log::info!("WebView2 navigated to: {}", url),
                Err(e) => {
                    let err_msg = format!("WebView2 navigation failed: {:?}", e);
                    log::error!("{}", err_msg);
                    self.report_error(err_msg);
                }
            }
        }
    }

    /// Reload the current page.
    pub fn reload(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(inner) = &self.inner {
                unsafe {
                    let _ = inner.webview.Reload();
                }
                log::info!("WebView2 reload");
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::info!("Embedded browser reload (stub)");
        }
    }

    /// Go back in history.
    pub fn go_back(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(inner) = &self.inner {
                unsafe {
                    let mut can_go_back = windows::Win32::Foundation::BOOL::default();
                    if inner.webview.CanGoBack(&mut can_go_back).is_ok() && can_go_back.as_bool() {
                        let _ = inner.webview.GoBack();
                        log::info!("WebView2 go_back");
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::info!("Embedded browser go_back (stub)");
        }
    }

    /// Go forward in history.
    pub fn go_forward(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(inner) = &self.inner {
                unsafe {
                    let mut can_go_forward = windows::Win32::Foundation::BOOL::default();
                    if inner.webview.CanGoForward(&mut can_go_forward).is_ok()
                        && can_go_forward.as_bool()
                    {
                        let _ = inner.webview.GoForward();
                        log::info!("WebView2 go_forward");
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::info!("Embedded browser go_forward (stub)");
        }
    }

    pub fn run_mcp_tool(
        &mut self,
        tool: &str,
        params: &JsonValue,
    ) -> Result<BrowserMcpToolOutput, String> {
        match tool {
            "browser_snapshot"
            | "browser_page_summary"
            | "browser_click"
            | "browser_hover"
            | "browser_select_option"
            | "browser_evaluate"
            | "browser_fill_form"
            | "browser_press_key"
            | "browser_type"
            | "browser_wait_for"
            | "browser_console_messages"
            | "browser_network_requests"
            | "browser_network_request"
            | "browser_resize"
            | "browser_highlight"
            | "browser_hide_highlight"
            | "browser_mouse_move_xy"
            | "browser_mouse_click_xy"
            | "browser_mouse_drag_xy"
            | "browser_mouse_down"
            | "browser_mouse_up"
            | "browser_mouse_wheel"
            | "browser_localstorage_list"
            | "browser_localstorage_get"
            | "browser_localstorage_set"
            | "browser_localstorage_delete"
            | "browser_localstorage_clear"
            | "browser_sessionstorage_list"
            | "browser_sessionstorage_get"
            | "browser_sessionstorage_set"
            | "browser_sessionstorage_delete"
            | "browser_sessionstorage_clear"
            | "browser_cookie_list"
            | "browser_cookie_get"
            | "browser_cookie_set"
            | "browser_cookie_delete"
            | "browser_cookie_clear" => self.run_mcp_script_tool(tool, params),
            "browser_take_screenshot" => self.run_mcp_screenshot_tool(params),
            "browser_drag" => Err("Drag-and-drop is not implemented in the embedded Mergen browser MCP yet. Use click/type tools for now.".to_owned()),
            "browser_run_code_unsafe" => Err("browser_run_code_unsafe is intentionally not supported by Mergen Browser MCP because it would execute arbitrary code in the helper process. Use browser_evaluate for page JavaScript.".to_owned()),
            "browser_resume" | "browser_annotate" | "browser_start_tracing" | "browser_stop_tracing"
            | "browser_start_video" | "browser_stop_video" | "browser_video_chapter"
            | "browser_network_state_set" | "browser_route" | "browser_route_list"
            | "browser_unroute" | "browser_pdf_save" | "browser_storage_state"
            | "browser_set_storage_state" => Err(format!(
                "{tool} is not implemented by Mergen Browser MCP yet. The embedded browser stayed in Mergen ADE; no external Chrome fallback was launched."
            )),
            _ => Err(format!("Unsupported browser MCP tool: {tool}")),
        }
    }

    fn run_mcp_script_tool(
        &mut self,
        tool: &str,
        params: &JsonValue,
    ) -> Result<BrowserMcpToolOutput, String> {
        #[cfg(target_os = "windows")]
        {
            if browser_mcp_tool_uses_async_script(tool) {
                let cdp_params = browser_mcp_runtime_evaluate_params(tool, params)?;
                let raw = self.call_devtools_protocol_method("Runtime.evaluate", &cdp_params)?;
                return browser_mcp_output_from_devtools_runtime_raw(&raw);
            }
            let script = browser_mcp_script_expression(tool, params)?;
            let value = self.execute_script_value(&script)?;
            browser_mcp_output_from_script_value(value)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (tool, params);
            Err("Embedded browser automation is currently Windows-only".to_owned())
        }
    }

    pub fn start_mcp_script_tool(
        &mut self,
        request_id: String,
        tool: &str,
        params: &JsonValue,
    ) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let cdp_params = browser_mcp_runtime_evaluate_params(tool, params)?;
            self.call_devtools_protocol_method_for_script_async(request_id, &cdp_params)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (request_id, tool, params);
            Err("Embedded browser automation is currently Windows-only".to_owned())
        }
    }

    fn run_mcp_screenshot_tool(
        &mut self,
        params: &JsonValue,
    ) -> Result<BrowserMcpToolOutput, String> {
        #[cfg(target_os = "windows")]
        {
            let started = Instant::now();
            let (format, cdp_params) = screenshot_cdp_params(params);
            let (_, retry_cdp_params) = screenshot_cdp_params_with_surface(params, false);
            self.run_screenshot_preflight();
            let raw = self.call_devtools_protocol_method("Page.captureScreenshot", &cdp_params)?;
            if browser_mcp_screenshot_retry_reason_from_devtools_raw(&raw).is_some() {
                self.run_screenshot_preflight();
                let retry_raw = self
                    .call_devtools_protocol_method("Page.captureScreenshot", &retry_cdp_params)?;
                if let Some(reason) =
                    browser_mcp_screenshot_retry_reason_from_devtools_raw(&retry_raw)
                {
                    return Err(format!(
                        "WebView2 screenshot stayed blank after retry ({reason}, {}ms).",
                        started.elapsed().as_millis()
                    ));
                }
                browser_mcp_screenshot_output_from_devtools_raw_with_metadata(
                    &retry_raw,
                    format,
                    1,
                    Some(started.elapsed()),
                )
            } else {
                browser_mcp_screenshot_output_from_devtools_raw_with_metadata(
                    &raw,
                    format,
                    0,
                    Some(started.elapsed()),
                )
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = params;
            Err("Embedded browser screenshots are currently Windows-only".to_owned())
        }
    }

    pub fn start_mcp_screenshot_tool(
        &mut self,
        request_id: String,
        params: &JsonValue,
    ) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let (format, cdp_params) = screenshot_cdp_params(params);
            let (_, retry_cdp_params) = screenshot_cdp_params_with_surface(params, false);
            self.call_devtools_protocol_method_for_screenshot_async(
                request_id,
                format.to_owned(),
                &cdp_params,
                &retry_cdp_params,
            )
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (request_id, params);
            Err("Embedded browser screenshots are currently Windows-only".to_owned())
        }
    }

    #[cfg(target_os = "windows")]
    fn run_screenshot_preflight(&mut self) {
        if let Err(err) = self.call_devtools_protocol_method(
            "Runtime.evaluate",
            &screenshot_preflight_runtime_params(),
        ) {
            log::debug!("WebView2 screenshot preflight failed; continuing capture: {err}");
        }
    }

    #[cfg(target_os = "windows")]
    fn execute_script_value(&mut self, script: &str) -> Result<JsonValue, String> {
        let raw = self.execute_script_raw(script)?;
        serde_json::from_str::<JsonValue>(&raw)
            .map_err(|err| format!("WebView2 script returned invalid JSON ({err}): {raw}"))
    }

    #[cfg(target_os = "windows")]
    fn execute_script_raw(&mut self, script: &str) -> Result<String, String> {
        use std::sync::{Arc, Mutex};
        use webview2_com::{CoTaskMemPWSTR, ExecuteScriptCompletedHandler};

        let Some(inner) = &self.inner else {
            return Err("Embedded browser is not ready".to_owned());
        };
        let webview = inner.webview.clone();
        let script = script.to_owned();
        let result_slot = Arc::new(Mutex::new(None::<String>));
        let completed_slot = Arc::clone(&result_slot);
        ExecuteScriptCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                let script = CoTaskMemPWSTR::from(script.as_str());
                unsafe {
                    webview
                        .ExecuteScript(*script.as_ref().as_pcwstr(), &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }
            }),
            Box::new(move |error_code, result| {
                error_code?;
                if let Ok(mut slot) = completed_slot.lock() {
                    *slot = Some(result);
                }
                Ok(())
            }),
        )
        .map_err(|err| format!("WebView2 ExecuteScript failed: {err}"))?;
        result_slot
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .ok_or_else(|| "WebView2 ExecuteScript completed without a result".to_owned())
    }

    #[cfg(target_os = "windows")]
    fn call_devtools_protocol_method(
        &mut self,
        method: &str,
        params: &JsonValue,
    ) -> Result<String, String> {
        use std::sync::{Arc, Mutex};
        use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};

        let Some(inner) = &self.inner else {
            return Err("Embedded browser is not ready".to_owned());
        };
        let webview = inner.webview.clone();
        let method = method.to_owned();
        let params = serde_json::to_string(params).map_err(|err| err.to_string())?;
        let result_slot = Arc::new(Mutex::new(None::<String>));
        let completed_slot = Arc::clone(&result_slot);
        CallDevToolsProtocolMethodCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| {
                let method = CoTaskMemPWSTR::from(method.as_str());
                let params = CoTaskMemPWSTR::from(params.as_str());
                unsafe {
                    webview
                        .CallDevToolsProtocolMethod(
                            *method.as_ref().as_pcwstr(),
                            *params.as_ref().as_pcwstr(),
                            &handler,
                        )
                        .map_err(webview2_com::Error::WindowsError)
                }
            }),
            Box::new(move |error_code, result| {
                error_code?;
                if let Ok(mut slot) = completed_slot.lock() {
                    *slot = Some(result);
                }
                Ok(())
            }),
        )
        .map_err(|err| format!("WebView2 DevTools method failed: {err}"))?;
        result_slot
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
            .ok_or_else(|| "WebView2 DevTools method completed without a result".to_owned())
    }

    #[cfg(target_os = "windows")]
    fn call_devtools_protocol_method_for_screenshot_async(
        &mut self,
        request_id: String,
        format: String,
        params: &JsonValue,
        retry_params: &JsonValue,
    ) -> Result<(), String> {
        let Some(inner) = &self.inner else {
            return Err("Embedded browser is not ready".to_owned());
        };
        let webview = inner.webview.clone();
        let event_sender = self.event_sender.clone();
        let params = serde_json::to_string(params).map_err(|err| err.to_string())?;
        let retry_params = serde_json::to_string(retry_params).map_err(|err| err.to_string())?;
        start_screenshot_capture_after_preflight(
            webview,
            event_sender,
            request_id,
            format,
            params,
            retry_params,
            Instant::now(),
        )
    }

    #[cfg(target_os = "windows")]
    fn call_devtools_protocol_method_for_script_async(
        &mut self,
        request_id: String,
        params: &JsonValue,
    ) -> Result<(), String> {
        use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};

        let Some(inner) = &self.inner else {
            return Err("Embedded browser is not ready".to_owned());
        };
        let webview = inner.webview.clone();
        let event_sender = self.event_sender.clone();
        let params = serde_json::to_string(params).map_err(|err| err.to_string())?;
        let callback = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
            move |error_code, result| -> windows::core::Result<()> {
                let tool_result = match error_code {
                    Ok(()) => browser_mcp_output_from_devtools_runtime_raw(&result),
                    Err(err) => Err(format!("WebView2 Runtime.evaluate failed: {err:?}")),
                };
                let _ = event_sender.send(BrowserEvent::McpToolResult {
                    request_id: request_id.clone(),
                    result: tool_result,
                });
                Ok(())
            },
        ));
        let method = CoTaskMemPWSTR::from("Runtime.evaluate");
        let params = CoTaskMemPWSTR::from(params.as_str());
        unsafe {
            webview
                .CallDevToolsProtocolMethod(
                    *method.as_ref().as_pcwstr(),
                    *params.as_ref().as_pcwstr(),
                    &callback,
                )
                .map_err(|err| format!("WebView2 Runtime.evaluate failed: {err:?}"))?;
        }
        Ok(())
    }

    /// Drain all pending browser events.
    pub fn drain_events(&mut self) -> Vec<BrowserEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get the current browser status.
    pub fn status(&self) -> BrowserStatus {
        self.status.clone()
    }

    /// Shutdown the browser and release resources.
    pub fn shutdown(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(inner) = self.inner.take() {
                // Remove SourceChanged event handler before dropping WebView
                if let Some(token) = inner.source_changed_token {
                    unsafe {
                        let _ = inner.webview.remove_SourceChanged(token);
                    }
                }
                if let Some(token) = inner.web_message_received_token {
                    unsafe {
                        let _ = inner.webview.remove_WebMessageReceived(token);
                    }
                }
                if let Some(token) = inner.navigation_starting_token {
                    unsafe {
                        let _ = inner.webview.remove_NavigationStarting(token);
                    }
                }
                if let Some(token) = inner.content_loading_token {
                    unsafe {
                        let _ = inner.webview.remove_ContentLoading(token);
                    }
                }
                if let Some(token) = inner.navigation_completed_token {
                    unsafe {
                        let _ = inner.webview.remove_NavigationCompleted(token);
                    }
                }
                log::info!("WebView2 resources released");
            }
        }
        self.status = BrowserStatus::Uninitialized;
        self.requested_visible = false;
        self.design_inspect_enabled = false;
    }

    /// Get the last requested visibility state (for testing).
    #[cfg(test)]
    pub fn requested_visible(&self) -> bool {
        self.requested_visible
    }

    #[cfg(test)]
    pub fn user_data_folder(&self) -> Option<&PathBuf> {
        self.user_data_folder.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn send_test_event(&self, event: BrowserEvent) {
        let _ = self.event_sender.send(event);
    }

    #[cfg(test)]
    pub(crate) fn set_test_status(&mut self, status: BrowserStatus) {
        self.status = status;
    }
}

#[cfg(target_os = "windows")]
fn start_screenshot_capture_after_preflight(
    webview: ICoreWebView2,
    event_sender: Sender<BrowserEvent>,
    request_id: String,
    format: String,
    params: String,
    retry_params: String,
    started: Instant,
) -> Result<(), String> {
    use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};

    let webview_for_capture = webview.clone();
    let event_sender_for_capture = event_sender.clone();
    let request_id_for_capture = request_id.clone();
    let format_for_capture = format.clone();
    let params_for_capture = params.clone();
    let retry_params_for_capture = retry_params.clone();
    let callback = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
        move |_error_code, _result| -> windows::core::Result<()> {
            if let Err(err) = start_screenshot_cdp_capture(
                webview_for_capture.clone(),
                event_sender_for_capture.clone(),
                request_id_for_capture.clone(),
                format_for_capture.clone(),
                params_for_capture.clone(),
                Some(retry_params_for_capture.clone()),
                started,
                0,
            ) {
                let _ = event_sender_for_capture.send(BrowserEvent::McpToolResult {
                    request_id: request_id_for_capture.clone(),
                    result: Err(err),
                });
            }
            Ok(())
        },
    ));
    let method = CoTaskMemPWSTR::from("Runtime.evaluate");
    let preflight_params = serde_json::to_string(&screenshot_preflight_runtime_params())
        .map_err(|err| err.to_string())?;
    let preflight_params = CoTaskMemPWSTR::from(preflight_params.as_str());
    let preflight_result = unsafe {
        webview.CallDevToolsProtocolMethod(
            *method.as_ref().as_pcwstr(),
            *preflight_params.as_ref().as_pcwstr(),
            &callback,
        )
    };
    if let Err(err) = preflight_result {
        log::debug!("WebView2 screenshot preflight start failed; capturing anyway: {err:?}");
        start_screenshot_cdp_capture(
            webview,
            event_sender,
            request_id,
            format,
            params,
            Some(retry_params),
            started,
            0,
        )?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn start_screenshot_cdp_capture(
    webview: ICoreWebView2,
    event_sender: Sender<BrowserEvent>,
    request_id: String,
    format: String,
    params: String,
    retry_params: Option<String>,
    started: Instant,
    retry_count: u8,
) -> Result<(), String> {
    use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};

    let webview_for_retry = webview.clone();
    let callback = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
        move |error_code, result| -> windows::core::Result<()> {
            let tool_result = match error_code {
                Ok(()) => {
                    if let Some(reason) =
                        browser_mcp_screenshot_retry_reason_from_devtools_raw(&result)
                    {
                        if retry_count == 0 {
                            if let Some(retry_params) = retry_params.clone() {
                                let retry_result = start_screenshot_cdp_capture(
                                    webview_for_retry.clone(),
                                    event_sender.clone(),
                                    request_id.clone(),
                                    format.clone(),
                                    retry_params,
                                    None,
                                    started,
                                    1,
                                );
                                if let Err(err) = retry_result {
                                    let _ = event_sender.send(BrowserEvent::McpToolResult {
                                        request_id: request_id.clone(),
                                        result: Err(err),
                                    });
                                }
                                return Ok(());
                            }
                        }
                        Err(format!(
                            "WebView2 screenshot stayed blank after retry ({reason}, {}ms).",
                            started.elapsed().as_millis()
                        ))
                    } else {
                        browser_mcp_screenshot_output_from_devtools_raw_with_metadata(
                            &result,
                            &format,
                            retry_count,
                            Some(started.elapsed()),
                        )
                    }
                }
                Err(err) => Err(format!("WebView2 DevTools method failed: {err:?}")),
            };
            let _ = event_sender.send(BrowserEvent::McpToolResult {
                request_id: request_id.clone(),
                result: tool_result,
            });
            Ok(())
        },
    ));
    let method = CoTaskMemPWSTR::from("Page.captureScreenshot");
    let params = CoTaskMemPWSTR::from(params.as_str());
    unsafe {
        webview
            .CallDevToolsProtocolMethod(
                *method.as_ref().as_pcwstr(),
                *params.as_ref().as_pcwstr(),
                &callback,
            )
            .map_err(|err| format!("WebView2 DevTools method failed: {err:?}"))?;
    }
    Ok(())
}

impl Default for EmbeddedBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert an egui Rect to browser bounds in physical pixels.
/// Uses the pixels_per_point scale factor from egui context.
pub fn browser_bounds_from_egui_rect(rect: egui::Rect, pixels_per_point: f32) -> BrowserBounds {
    let x = (rect.min.x * pixels_per_point).round() as i32;
    let y = (rect.min.y * pixels_per_point).round() as i32;
    let width = (rect.width() * pixels_per_point).round() as i32;
    let height = (rect.height() * pixels_per_point).round() as i32;

    // Clamp to minimum positive size
    let width = width.max(1);
    let height = height.max(1);

    BrowserBounds {
        x,
        y,
        width,
        height,
    }
}

fn browser_mcp_output_from_script_value(value: JsonValue) -> Result<BrowserMcpToolOutput, String> {
    if value
        .get("ok")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false)
    {
        let text = value
            .get("text")
            .and_then(JsonValue::as_str)
            .unwrap_or("Browser command completed")
            .to_owned();
        Ok(BrowserMcpToolOutput {
            text,
            data: value.get("data").cloned(),
        })
    } else {
        Err(value
            .get("error")
            .and_then(JsonValue::as_str)
            .unwrap_or("Browser command failed")
            .to_owned())
    }
}

pub(crate) fn browser_mcp_output_from_devtools_runtime_raw(
    raw: &str,
) -> Result<BrowserMcpToolOutput, String> {
    let parsed = serde_json::from_str::<JsonValue>(raw)
        .map_err(|err| format!("WebView2 Runtime.evaluate returned invalid JSON ({err}): {raw}"))?;
    if let Some(exception) = parsed.get("exceptionDetails") {
        let message = exception
            .get("exception")
            .and_then(|value| value.get("description").or_else(|| value.get("value")))
            .and_then(JsonValue::as_str)
            .or_else(|| exception.get("text").and_then(JsonValue::as_str))
            .unwrap_or("Browser command failed during page evaluation");
        return Err(message.to_owned());
    }
    let result = parsed
        .get("result")
        .ok_or_else(|| format!("WebView2 Runtime.evaluate response missing result: {raw}"))?;
    let value = result
        .get("value")
        .cloned()
        .ok_or_else(|| format!("WebView2 Runtime.evaluate response missing value: {raw}"))?;
    browser_mcp_output_from_script_value(value)
}

#[cfg(target_os = "windows")]
const MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT: &str = r#"
if (!window.__mergenMcpRun || window.__mergenMcpRun.version !== 6) {
  window.__mergenMcpState = window.__mergenMcpState || { refCounter: 1, consoleMessages: [], networkRequests: [], routes: [], highlighted: null };

  const state = window.__mergenMcpState;
  state.cursor = state.cursor || { x: Math.round(window.innerWidth / 2), y: Math.round(window.innerHeight / 2), visible: false, mouseDownButton: null };
  state.cursor.anchorElement = state.cursor.anchorElement || null;
  const clean = (value, max = 240) => String(value ?? '').replace(/[\u0000-\u001f\u007f]+/g, ' ').replace(/\s+/g, ' ').trim().slice(0, max);
  const visible = (element) => {
    if (!element || element.nodeType !== Node.ELEMENT_NODE) return false;
    const style = window.getComputedStyle(element);
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false;
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const inViewport = (rect) => rect.bottom >= 0 && rect.right >= 0 && rect.top <= window.innerHeight && rect.left <= window.innerWidth;
  const isDisabled = (element) => Boolean(
    element?.disabled ||
    element?.ariaDisabled === 'true' ||
    element?.getAttribute?.('aria-disabled') === 'true' ||
    element?.inert ||
    element?.closest?.('[inert]') ||
    (element?.closest?.('fieldset[disabled]') && element.tagName !== 'FIELDSET')
  );
  const roleOf = (element) => {
    const explicit = element.getAttribute('role');
    if (explicit) return explicit;
    const tag = element.tagName.toLowerCase();
    if (tag === 'a' && element.hasAttribute('href')) return 'link';
    if (tag === 'button') return 'button';
    if (tag === 'select') return 'combobox';
    if (tag === 'textarea') return 'textbox';
    if (tag === 'input') {
      const type = (element.getAttribute('type') || 'text').toLowerCase();
      if (type === 'checkbox') return 'checkbox';
      if (type === 'radio') return 'radio';
      if (type === 'button' || type === 'submit' || type === 'reset') return 'button';
      return 'textbox';
    }
    if (/^h[1-6]$/.test(tag)) return 'heading';
    if (tag === 'img') return 'img';
    if (tag === 'li') return 'listitem';
    return 'generic';
  };
  const nameOf = (element) => {
    const aria = element.getAttribute('aria-label') || element.getAttribute('title') || element.getAttribute('alt');
    if (aria) return clean(aria);
    if ('value' in element && element.value && ['INPUT', 'TEXTAREA', 'SELECT'].includes(element.tagName)) return clean(element.value);
    return clean(element.innerText || element.textContent || '');
  };
  const ensureRef = (element) => {
    if (!element.dataset.mergenMcpRef) element.dataset.mergenMcpRef = `e${state.refCounter++}`;
    return element.dataset.mergenMcpRef;
  };
  const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve()));
  const clamp = (value, min, max) => Math.min(Math.max(value, min), max);
  const clampPoint = (x, y) => ({
    x: clamp(Number(x), 0, Math.max(0, window.innerWidth - 1)),
    y: clamp(Number(y), 0, Math.max(0, window.innerHeight - 1)),
  });
  const hasOwn = (object, key) => Object.prototype.hasOwnProperty.call(object || {}, key);
  const numberParam = (object, key, required = true) => {
    if (!hasOwn(object, key) || object[key] === null || object[key] === undefined || object[key] === '') {
      if (required) throw new Error(`${key} is required and must be a finite number`);
      return null;
    }
    const value = Number(object[key]);
    if (!Number.isFinite(value)) throw new Error(`${key} must be a finite number`);
    return value;
  };
  const requiredPoint = (params, xName = 'x', yName = 'y') => clampPoint(numberParam(params, xName), numberParam(params, yName));
  const optionalPoint = (params) => {
    const hasX = hasOwn(params, 'x') && params.x !== null && params.x !== undefined && params.x !== '';
    const hasY = hasOwn(params, 'y') && params.y !== null && params.y !== undefined && params.y !== '';
    if (hasX !== hasY) throw new Error('x and y must be supplied together');
    if (hasX && hasY) return requiredPoint(params);
    return clampPoint(state.cursor.x, state.cursor.y);
  };
  const buttonName = (value) => {
    const button = String(value || 'left').toLowerCase();
    if (button === 'left' || button === 'middle' || button === 'right') return button;
    throw new Error(`Unsupported mouse button: ${value}`);
  };
  const buttonCode = (button) => ({ left: 0, middle: 1, right: 2 }[buttonName(button)]);
  const buttonMask = (button) => ({ left: 1, middle: 4, right: 2 }[buttonName(button)]);
  const ensureCursorStyle = () => {
    const existing = document.querySelector('style[data-mergen-mcp-cursor-style]');
    if (existing?.getAttribute('data-mergen-mcp-cursor-style') === '6') return;
    existing?.remove();
    const style = document.createElement('style');
    style.setAttribute('data-mergen-mcp-cursor-style', '6');
    style.textContent = `
@keyframes mergenMcpCursorPulse {
  0% { opacity: 0; transform: translate(-50%, -50%) scale(0.55); box-shadow: 0 0 0 0 rgba(56,189,248,0.34), 0 8px 20px rgba(2,6,23,0.26); }
  22% { opacity: 0.95; }
  100% { opacity: 0; transform: translate(-50%, -50%) scale(1.85); box-shadow: 0 0 0 12px rgba(56,189,248,0), 0 12px 28px rgba(2,6,23,0); }
}
@keyframes mergenMcpCursorIdleAura {
  0%, 100% { opacity: 0.52; transform: translate(-50%, -50%) scale(0.96); }
  50% { opacity: 0.68; transform: translate(-50%, -50%) scale(1.04); }
}
[data-mergen-mcp-cursor] [data-mergen-mcp-cursor-aura],
[data-mergen-mcp-cursor] [data-mergen-mcp-cursor-focus],
[data-mergen-mcp-cursor] [data-mergen-mcp-cursor-pointer] {
  transition: opacity 130ms ease, transform 150ms cubic-bezier(0.16, 1, 0.3, 1), filter 150ms ease;
}
[data-mergen-mcp-cursor] [data-mergen-mcp-cursor-aura] {
  animation: mergenMcpCursorIdleAura 1500ms ease-in-out infinite;
}
[data-mergen-mcp-cursor][data-mergen-mcp-cursor-phase="moving"] [data-mergen-mcp-cursor-aura] {
  opacity: 0.76;
  transform: translate(-50%, -50%) scale(1.12);
}
[data-mergen-mcp-cursor][data-mergen-mcp-cursor-phase="targeting"] [data-mergen-mcp-cursor-aura],
[data-mergen-mcp-cursor][data-mergen-mcp-cursor-phase="click"] [data-mergen-mcp-cursor-aura] {
  opacity: 0.9;
  transform: translate(-50%, -50%) scale(0.82);
  animation: none;
}
[data-mergen-mcp-cursor][data-mergen-mcp-cursor-phase="targeting"] [data-mergen-mcp-cursor-focus],
[data-mergen-mcp-cursor][data-mergen-mcp-cursor-phase="click"] [data-mergen-mcp-cursor-focus] {
  opacity: 0.9;
  transform: translate(-50%, -50%) scale(0.86);
}
[data-mergen-mcp-cursor][data-mergen-mcp-cursor-phase="moving"] [data-mergen-mcp-cursor-pointer] {
  filter: drop-shadow(0 2px 2px rgba(15,23,42,0.82)) drop-shadow(0 13px 22px rgba(2,6,23,0.28));
}
`;
    document.documentElement.appendChild(style);
  };
  const ensureCursor = () => {
    ensureCursorStyle();
    if (
      state.cursorElement &&
      document.documentElement.contains(state.cursorElement) &&
      state.cursorElement.querySelector?.('[data-mergen-mcp-cursor-pointer]')
    ) {
      return state.cursorElement;
    }
    state.cursorElement?.remove?.();
    const cursor = document.createElement('div');
    cursor.setAttribute('data-mergen-mcp-cursor', 'true');
    cursor.setAttribute('data-mergen-mcp-cursor-phase', state.cursor.phase || 'idle');
    Object.assign(cursor.style, {
      position: 'fixed',
      left: '0px',
      top: '0px',
      width: '48px',
      height: '48px',
      pointerEvents: 'none',
      zIndex: '2147483647',
      display: 'none',
      transform: 'translate(0px, 0px)',
      transformOrigin: '0px 0px',
      willChange: 'transform',
      contain: 'layout style paint',
    });
    cursor.style.setProperty('--mergen-mcp-cursor-tilt', '0deg');
    const aura = document.createElement('div');
    aura.setAttribute('data-mergen-mcp-cursor-aura', 'true');
    Object.assign(aura.style, {
      position: 'absolute',
      left: '12px',
      top: '14px',
      width: '42px',
      height: '42px',
      borderRadius: '999px',
      background: 'radial-gradient(circle, rgba(15,23,42,0.24) 0%, rgba(15,23,42,0.15) 36%, rgba(56,189,248,0.12) 58%, rgba(56,189,248,0) 72%)',
      boxShadow: '0 10px 24px rgba(2,6,23,0.22)',
      transform: 'translate(-50%, -50%)',
      opacity: '0.62',
      mixBlendMode: 'normal',
    });
    const focus = document.createElement('div');
    focus.setAttribute('data-mergen-mcp-cursor-focus', 'true');
    Object.assign(focus.style, {
      position: 'absolute',
      left: '12px',
      top: '14px',
      width: '25px',
      height: '25px',
      borderRadius: '999px',
      border: '1px solid rgba(226,232,240,0.62)',
      boxShadow: '0 0 0 1px rgba(15,23,42,0.22), 0 8px 20px rgba(2,6,23,0.18)',
      transform: 'translate(-50%, -50%) scale(0.98)',
      opacity: '0.42',
    });
    const pointer = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    pointer.setAttribute('data-mergen-mcp-cursor-pointer', 'true');
    pointer.setAttribute('viewBox', '0 0 30 36');
    pointer.setAttribute('aria-hidden', 'true');
    Object.assign(pointer.style, {
      position: 'absolute',
      left: '-3px',
      top: '-3px',
      width: '30px',
      height: '36px',
      overflow: 'visible',
      transform: 'rotate(var(--mergen-mcp-cursor-tilt))',
      transformOrigin: '4px 4px',
      filter: 'drop-shadow(0 1px 1px rgba(15,23,42,0.9)) drop-shadow(0 9px 16px rgba(2,6,23,0.32))',
    });
    const pointerPath = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    pointerPath.setAttribute('d', 'M3.8 3.1 3.2 28.4 10.7 21.7 15.9 33.2 21.8 30.4 16.4 19.2 26.7 19.6 3.8 3.1Z');
    pointerPath.setAttribute('fill', 'rgba(248,250,252,0.98)');
    pointerPath.setAttribute('stroke', 'rgba(15,23,42,0.9)');
    pointerPath.setAttribute('stroke-width', '1.45');
    pointerPath.setAttribute('stroke-linejoin', 'round');
    const pointerSheen = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    pointerSheen.setAttribute('d', 'M6.7 8.6 6.4 22.1 10.1 18.6 13.5 25.9');
    pointerSheen.setAttribute('fill', 'none');
    pointerSheen.setAttribute('stroke', 'rgba(255,255,255,0.72)');
    pointerSheen.setAttribute('stroke-width', '1.15');
    pointerSheen.setAttribute('stroke-linecap', 'round');
    pointerSheen.setAttribute('stroke-linejoin', 'round');
    pointer.appendChild(pointerPath);
    pointer.appendChild(pointerSheen);
    cursor.appendChild(aura);
    cursor.appendChild(focus);
    cursor.appendChild(pointer);
    document.documentElement.appendChild(cursor);
    state.cursorElement = cursor;
    return cursor;
  };
  const clearCursorAnchor = () => {
    state.cursor.anchorElement = null;
  };
  const anchorCursorTo = (element) => {
    state.cursor.anchorElement = element || null;
  };
  const syncCursorToAnchor = () => {
    const element = state.cursor.anchorElement;
    if (!state.cursor.visible || !element || !document.documentElement.contains(element) || !visible(element)) return;
    const rect = element.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    setCursorPosition(rect.left + rect.width / 2, rect.top + rect.height / 2, { tilt: 0, phase: 'idle' });
  };
  const scheduleCursorAnchorSync = () => {
    if (state.cursorAnchorSyncQueued) return;
    state.cursorAnchorSyncQueued = true;
    requestAnimationFrame(() => {
      state.cursorAnchorSyncQueued = false;
      syncCursorToAnchor();
    });
  };
  const setCursorPhase = (phase) => {
    const cursor = ensureCursor();
    cursor.setAttribute('data-mergen-mcp-cursor-phase', phase);
    state.cursor.phase = phase;
    return cursor;
  };
  const scheduleCursorIdle = (delay = 190) => {
    const token = (state.cursor.phaseToken || 0) + 1;
    state.cursor.phaseToken = token;
    setTimeout(() => {
      if (state.cursor.phaseToken === token && state.cursor.visible) setCursorPhase('idle');
    }, delay);
  };
  const setCursorPosition = (x, y, options = {}) => {
    const point = clampPoint(x, y);
    const cursor = ensureCursor();
    cursor.style.display = 'block';
    cursor.style.transform = `translate(${point.x.toFixed(1)}px, ${point.y.toFixed(1)}px)`;
    if (hasOwn(options, 'tilt')) {
      const tilt = clamp(Number(options.tilt) || 0, -12, 12);
      cursor.style.setProperty('--mergen-mcp-cursor-tilt', `${tilt.toFixed(2)}deg`);
      state.cursor.tilt = tilt;
    }
    if (options.phase) {
      cursor.setAttribute('data-mergen-mcp-cursor-phase', options.phase);
      state.cursor.phase = options.phase;
    }
    state.cursor.x = point.x;
    state.cursor.y = point.y;
    state.cursor.visible = true;
    return point;
  };
  const easeOutCubic = (t) => 1 - Math.pow(1 - t, 3);
  const organicCursorPoint = (start, end, t, distance, options = {}) => {
    const eased = easeOutCubic(t);
    const baseX = start.x + (end.x - start.x) * eased;
    const baseY = start.y + (end.y - start.y) * eased;
    if (options.straight || distance < 18 || t >= 0.995) return { x: baseX, y: baseY, tilt: 0 };
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const length = Math.max(1, distance);
    const normalX = -dy / length;
    const normalY = dx / length;
    const launch = Math.sin(Math.PI * clamp(t / 0.18, 0, 1));
    const straighten = 1 - clamp((t - 0.72) / 0.28, 0, 1);
    const amplitude = clamp(distance * 0.045, 4, 18) * launch * straighten;
    const phase = ((Math.round(start.x + start.y + end.x + end.y) % 31) / 31) * Math.PI;
    const wave = Math.sin(t * Math.PI * 2.15 + phase) * amplitude;
    const hover = Math.sin(Math.PI * t) * amplitude * 0.18;
    const tilt = clamp((wave / Math.max(1, amplitude)) * 7 + Math.sign(dx || 1) * hover * 0.18, -10, 10) * straighten;
    return { x: baseX + normalX * wave, y: baseY + normalY * wave - hover, tilt };
  };
  const moveCursorTo = (x, y, options = {}) => new Promise((resolve) => {
    const end = clampPoint(x, y);
    const start = state.cursor.visible ? clampPoint(state.cursor.x, state.cursor.y) : clampPoint(end.x - 96, end.y - 64);
    const distance = Math.hypot(end.x - start.x, end.y - start.y);
    const duration = options.duration ?? clamp(Math.round(distance * 1.15), 650, 900);
    setCursorPosition(start.x, start.y, { tilt: 0, phase: 'moving' });
    if (duration <= 0 || distance < 1) {
      const point = setCursorPosition(end.x, end.y, { tilt: 0, phase: 'idle' });
      options.onStep?.(point);
      resolve(point);
      return;
    }
    state.cursor.phaseToken = (state.cursor.phaseToken || 0) + 1;
    const started = performance.now();
    const step = (now) => {
      const t = clamp((now - started) / duration, 0, 1);
      const visual = organicCursorPoint(start, end, t, distance, options);
      const phase = t > 0.72 ? 'targeting' : 'moving';
      const point = setCursorPosition(visual.x, visual.y, { tilt: visual.tilt, phase });
      options.onStep?.(point);
      if (t < 1) requestAnimationFrame(step);
      else {
        const finalPoint = setCursorPosition(end.x, end.y, { tilt: 0, phase: 'idle' });
        resolve(finalPoint);
      }
    };
    requestAnimationFrame(step);
  });
  const steadyCursorForAction = async (point, delay = 70) => {
    setCursorPosition(point.x, point.y, { tilt: 0, phase: 'targeting' });
    await new Promise((resolve) => setTimeout(resolve, delay));
    return setCursorPosition(point.x, point.y, { tilt: 0, phase: 'targeting' });
  };
  const pulseCursor = (point) => {
    ensureCursorStyle();
    const pulse = document.createElement('div');
    pulse.setAttribute('data-mergen-mcp-cursor-pulse', 'true');
    Object.assign(pulse.style, {
      position: 'fixed',
      left: `${point.x}px`,
      top: `${point.y}px`,
      width: '18px',
      height: '18px',
      border: '1px solid rgba(226,232,240,0.92)',
      borderRadius: '999px',
      background: 'radial-gradient(circle, rgba(56,189,248,0.22), rgba(56,189,248,0.02) 58%, rgba(56,189,248,0) 70%)',
      boxShadow: '0 0 0 1px rgba(15,23,42,0.22), 0 8px 20px rgba(2,6,23,0.26)',
      pointerEvents: 'none',
      zIndex: '2147483646',
      transform: 'translate(-50%, -50%)',
      animation: 'mergenMcpCursorPulse 360ms cubic-bezier(0.16, 1, 0.3, 1) forwards',
    });
    document.documentElement.appendChild(pulse);
    setTimeout(() => pulse.remove(), 440);
  };
  const stableElementCenterAfterScroll = async (element) => {
    element.scrollIntoView({ block: 'center', inline: 'center' });
    let previous = null;
    let latest = null;
    for (let frame = 0; frame < 5; frame++) {
      await nextFrame();
      const rect = element.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) throw new Error('Element is not visible after scrolling');
      latest = clampPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      if (previous && Math.abs(previous.x - latest.x) < 0.5 && Math.abs(previous.y - latest.y) < 0.5) return latest;
      previous = latest;
    }
    return latest;
  };
  const elementCenter = async (element) => stableElementCenterAfterScroll(element);
  const targetAt = (point, fallback = null) => document.elementFromPoint(point.x, point.y) || fallback || document.body || document.documentElement;
  const scrollableAncestor = (element, deltaX, deltaY) => {
    let node = element && element.nodeType === Node.ELEMENT_NODE ? element : element?.parentElement;
    while (node && node !== document.documentElement) {
      const style = window.getComputedStyle(node);
      const canScrollY = deltaY && node.scrollHeight > node.clientHeight && /(auto|scroll|overlay)/.test(style.overflowY);
      const canScrollX = deltaX && node.scrollWidth > node.clientWidth && /(auto|scroll|overlay)/.test(style.overflowX);
      if (canScrollY || canScrollX) return node;
      node = node.parentElement;
    }
    return document.scrollingElement || document.documentElement;
  };
  const applyWheelScrollFallback = async (target, point, deltaX, deltaY) => {
    const wheelEvent = new WheelEvent('wheel', { bubbles: true, cancelable: true, clientX: point.x, clientY: point.y, deltaX, deltaY, deltaMode: 0 });
    const wasNotCanceled = target.dispatchEvent(wheelEvent);
    if (wasNotCanceled) {
      const scrollTarget = scrollableAncestor(target, deltaX, deltaY);
      if (scrollTarget && scrollTarget !== document.documentElement && scrollTarget !== document.body) {
        scrollTarget.scrollBy({ left: deltaX, top: deltaY, behavior: 'auto' });
      } else {
        window.scrollBy({ left: deltaX, top: deltaY, behavior: 'auto' });
      }
      await nextFrame();
    }
    setCursorPosition(point.x, point.y, { tilt: 0, phase: 'idle' });
    dispatchMoveAt(point, targetAt(point, target));
  };
  const dispatchMouse = (target, type, point, button = 'left', detail = 0, buttonsOverride = null) => {
    const activeButton = buttonName(button);
    const buttons = buttonsOverride ?? (state.cursor.mouseDownButton ? buttonMask(state.cursor.mouseDownButton) : 0);
    return target.dispatchEvent(new MouseEvent(type, {
      bubbles: true,
      cancelable: true,
      view: window,
      clientX: point.x,
      clientY: point.y,
      screenX: window.screenX + point.x,
      screenY: window.screenY + point.y,
      button: buttonCode(activeButton),
      buttons,
      detail,
    }));
  };
  const dispatchMoveAt = (point, fallback = null) => {
    const target = targetAt(point, fallback);
    dispatchMouse(target, 'mouseover', point, 'left', 0, state.cursor.mouseDownButton ? buttonMask(state.cursor.mouseDownButton) : 0);
    dispatchMouse(target, 'mousemove', point, 'left', 0, state.cursor.mouseDownButton ? buttonMask(state.cursor.mouseDownButton) : 0);
    return target;
  };
  const settleAfterInteraction = () => new Promise((resolve) => {
    let done = false;
    let quietTimer = null;
    let observer = null;
    const finish = () => {
      if (done) return;
      done = true;
      if (quietTimer) clearTimeout(quietTimer);
      observer?.disconnect();
      resolve();
    };
    const armQuietTimer = () => {
      if (quietTimer) clearTimeout(quietTimer);
      quietTimer = setTimeout(finish, 45);
    };
    try {
      observer = new MutationObserver(armQuietTimer);
      observer.observe(document.documentElement, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
    } catch (_) {}
    requestAnimationFrame(() => requestAnimationFrame(armQuietTimer));
    setTimeout(finish, 180);
  });
  const clickAt = async (point, options = {}) => {
    const button = buttonName(options.button);
    const doubleClick = Boolean(options.doubleClick);
    const target = targetAt(point, options.target || null);
    await steadyCursorForAction(point);
    dispatchMoveAt(point, target);
    target.focus?.({ preventScroll: true });
    dispatchMouse(target, 'mousedown', point, button, 1, buttonMask(button));
    state.cursor.mouseDownButton = button;
    dispatchMouse(target, 'mouseup', point, button, 1, 0);
    state.cursor.mouseDownButton = null;
    dispatchMouse(target, 'click', point, button, 1, 0);
    if (doubleClick) {
      dispatchMouse(target, 'mousedown', point, button, 2, buttonMask(button));
      state.cursor.mouseDownButton = button;
      dispatchMouse(target, 'mouseup', point, button, 2, 0);
      state.cursor.mouseDownButton = null;
      dispatchMouse(target, 'click', point, button, 2, 0);
      dispatchMouse(target, 'dblclick', point, button, 2, 0);
    }
    if (button === 'right') dispatchMouse(target, 'contextmenu', point, button, 1, 0);
    setCursorPosition(point.x, point.y, { tilt: 0, phase: 'click' });
    pulseCursor(point);
    scheduleCursorIdle();
    await settleAfterInteraction();
    return target;
  };
  const cssEscape = (value) => window.CSS?.escape ? window.CSS.escape(value) : String(value).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
  const resolve = (target) => {
    if (!target) return null;
    const raw = String(target).trim();
    const ref = raw.replace(/^ref=/, '');
    if (/^e\d+$/.test(ref)) {
      const byRef = document.querySelector(`[data-mergen-mcp-ref="${cssEscape(ref)}"]`);
      if (byRef) return byRef;
    }
    try {
      const bySelector = document.querySelector(raw);
      if (bySelector) return bySelector;
    } catch (_) {}
    const needle = raw.toLowerCase();
    return Array.from(document.querySelectorAll('button,a,input,textarea,select,[role],[contenteditable],label,h1,h2,h3,h4,h5,h6'))
      .find((element) => visible(element) && nameOf(element).toLowerCase().includes(needle)) || null;
  };
  const targetValue = (params = {}) => params.target ?? params.ref ?? params.selector ?? params.element ?? '';
  const requiredElement = (params, tool) => {
    const target = targetValue(params);
    const element = resolve(target);
    if (!element) throw new Error(`Element not found: ${target || tool}`);
    return { element, target };
  };
  const event = (element, name) => element.dispatchEvent(new Event(name, { bubbles: true, cancelable: true }));
  const nativePropertySetter = (element, property) => {
    const prototypes = [];
    if (window.HTMLTextAreaElement && element instanceof window.HTMLTextAreaElement) prototypes.push(window.HTMLTextAreaElement.prototype);
    if (window.HTMLSelectElement && element instanceof window.HTMLSelectElement) prototypes.push(window.HTMLSelectElement.prototype);
    if (window.HTMLInputElement && element instanceof window.HTMLInputElement) prototypes.push(window.HTMLInputElement.prototype);
    if (element?.constructor?.prototype) prototypes.push(element.constructor.prototype);
    const objectPrototype = Object.getPrototypeOf(element);
    if (objectPrototype) prototypes.push(objectPrototype);
    for (const prototype of prototypes) {
      const descriptor = Object.getOwnPropertyDescriptor(prototype, property);
      if (descriptor?.set) return descriptor.set;
    }
    return null;
  };
  const setNativeProperty = (element, property, value) => {
    const previous = property in element ? element[property] : undefined;
    const setter = nativePropertySetter(element, property);
    if (setter) setter.call(element, value);
    else element[property] = value;
    const tracker = element._valueTracker;
    if (tracker && typeof tracker.setValue === 'function') tracker.setValue(String(previous ?? ''));
  };
  const dispatchKeyEvent = (element, type, key) => {
    const textKey = String(key || '');
    const code = textKey.length === 1 && /[a-z]/i.test(textKey) ? `Key${textKey.toUpperCase()}` : textKey;
    return element.dispatchEvent(new KeyboardEvent(type, {
      bubbles: true,
      cancelable: true,
      key: textKey,
      code,
      charCode: textKey.length === 1 ? textKey.charCodeAt(0) : 0,
      keyCode: textKey === 'Enter' ? 13 : textKey.length === 1 ? textKey.toUpperCase().charCodeAt(0) : 0,
      which: textKey === 'Enter' ? 13 : textKey.length === 1 ? textKey.toUpperCase().charCodeAt(0) : 0,
    }));
  };
  const dispatchInputLifecycleEvent = (element, type, inputType = 'insertText', data = null) => {
    if (window.InputEvent) {
      return element.dispatchEvent(new InputEvent(type, {
        bubbles: true,
        cancelable: type === 'beforeinput',
        inputType,
        data,
      }));
    }
    return element.dispatchEvent(new Event(type, { bubbles: true, cancelable: type === 'beforeinput' }));
  };
  const dispatchFocusLifecycleEvent = (element, type, bubbles) => {
    if (window.FocusEvent) {
      return element.dispatchEvent(new FocusEvent(type, {
        bubbles,
        cancelable: false,
        relatedTarget: null,
      }));
    }
    return element.dispatchEvent(new Event(type, { bubbles, cancelable: false }));
  };
  const isTextEditableElement = (element) => Boolean(element?.isContentEditable || ('value' in element));
  const editableTextValue = (element) => element.isContentEditable ? String(element.textContent ?? '') : String(element.value ?? '');
  const setEditableTextValue = (element, value) => {
    if (element.isContentEditable) element.textContent = value;
    else setNativeProperty(element, 'value', value);
  };
  const commitField = async (element, commit = true) => {
    await nextFrame();
    event(element, 'change');
    if (!commit) return;
    dispatchFocusLifecycleEvent(element, 'focusout', true);
    dispatchFocusLifecycleEvent(element, 'blur', false);
    element.blur?.();
    await nextFrame();
  };
  const typeTextLikeUser = async (element, text, options = {}) => {
    const commit = options.commit !== false;
    const nextText = String(text ?? '');
    if (!isTextEditableElement(element)) throw new Error('Element is not text-editable');
    element.focus?.({ preventScroll: true });
    if (editableTextValue(element) !== '') {
      dispatchKeyEvent(element, 'keydown', 'Backspace');
      const clearAllowed = dispatchInputLifecycleEvent(element, 'beforeinput', 'deleteContentBackward', null);
      if (clearAllowed !== false) {
        setEditableTextValue(element, '');
        dispatchInputLifecycleEvent(element, 'input', 'deleteContentBackward', null);
      }
      dispatchKeyEvent(element, 'keyup', 'Backspace');
    }
    for (const char of Array.from(nextText)) {
      const key = char === '\n' ? 'Enter' : char;
      const inputType = char === '\n' ? 'insertLineBreak' : 'insertText';
      dispatchKeyEvent(element, 'keydown', key);
      if (char !== '\n') dispatchKeyEvent(element, 'keypress', key);
      const allowed = dispatchInputLifecycleEvent(element, 'beforeinput', inputType, char);
      if (allowed !== false) {
        setEditableTextValue(element, editableTextValue(element) + char);
        dispatchInputLifecycleEvent(element, 'input', inputType, char);
      }
      dispatchKeyEvent(element, 'keyup', key);
    }
    if (nextText === '') dispatchInputLifecycleEvent(element, 'input', 'insertReplacementText', null);
    await commitField(element, commit);
  };
  const EVALUATE_INTERACTION_BLOCKED_MESSAGE = 'JavaScript clicks are blocked in Mergen Browser MCP. Use browser_click or browser_mouse_click_xy so the visible mouse moves and clicks.';
  const INTERACTIVE_EVALUATE_PATTERN = /(?:\.click\s*\(|\[['"]click['"]\]\s*\(|\.dispatchEvent\s*\(\s*new\s+(?:MouseEvent|PointerEvent)\s*\(|\[['"]dispatchEvent['"]\]\s*\(\s*new\s+(?:MouseEvent|PointerEvent)\s*\(|\.dispatchEvent\s*\(\s*new\s+Event\s*\(\s*['"`](?:click|dblclick|mousedown|mouseup|mousemove|mouseover|mouseout|mouseenter|mouseleave|contextmenu|pointerdown|pointerup|pointermove|pointerover|pointerout)['"`]|\[['"]dispatchEvent['"]\]\s*\(\s*new\s+Event\s*\(\s*['"`](?:click|dblclick|mousedown|mouseup|mousemove|mouseover|mouseout|mouseenter|mouseleave|contextmenu|pointerdown|pointerup|pointermove|pointerover|pointerout)['"`]|\.requestSubmit\s*\(|\[['"]requestSubmit['"]\]\s*\(|\.submit\s*\(|\[['"]submit['"]\]\s*\()/i;
  const assertReadOnlyEvaluateScript = (expr) => {
    if (INTERACTIVE_EVALUATE_PATTERN.test(expr)) throw new Error(EVALUATE_INTERACTION_BLOCKED_MESSAGE);
  };
  const runEvaluateTool = async (params = {}, element = null) => {
    const expr = String(params.function || params.script || '');
    assertReadOnlyEvaluateScript(expr);
    const blockedEventTypes = new Set(['click', 'dblclick', 'mousedown', 'mouseup', 'mousemove', 'mouseover', 'mouseout', 'mouseenter', 'mouseleave', 'contextmenu', 'pointerdown', 'pointerup', 'pointermove', 'pointerover', 'pointerout']);
    const restore = [];
    let blockedInteractionAttempt = false;
    const blockInteraction = () => {
      blockedInteractionAttempt = true;
      throw new Error(EVALUATE_INTERACTION_BLOCKED_MESSAGE);
    };
    const patchMethod = (prototype, name, replacement) => {
      if (!prototype || typeof prototype[name] !== 'function') return;
      const original = prototype[name];
      prototype[name] = replacement(original);
      restore.push(() => { prototype[name] = original; });
    };
    patchMethod(window.HTMLElement?.prototype, 'click', () => blockInteraction);
    patchMethod(window.HTMLFormElement?.prototype, 'submit', () => blockInteraction);
    patchMethod(window.HTMLFormElement?.prototype, 'requestSubmit', () => blockInteraction);
    patchMethod(window.EventTarget?.prototype, 'dispatchEvent', (original) => function patchedDispatchEvent(event) {
      if (event && blockedEventTypes.has(String(event.type || '').toLowerCase())) blockInteraction();
      return original.call(this, event);
    });
    try {
      let value;
      try { value = eval(`(${expr})`); } catch (_) { value = eval(expr); }
      const result = await (typeof value === 'function' ? value(element || undefined) : value);
      if (blockedInteractionAttempt) throw new Error(EVALUATE_INTERACTION_BLOCKED_MESSAGE);
      return ok(JSON.stringify(result, null, 2) ?? 'undefined', { result });
    } finally {
      for (const restoreMethod of restore.reverse()) restoreMethod();
    }
  };
  const fillOneField = async (field) => {
    const target = field.target ?? field.ref ?? field.selector ?? field.name ?? '';
    const element = resolve(target);
    if (!element) throw new Error(`Element not found: ${target}`);
    const point = await elementCenter(element);
    anchorCursorTo(element);
    await moveCursorTo(point.x, point.y);
    dispatchMoveAt(point, element);
    element.focus?.({ preventScroll: true });
    const commit = field.commit !== false;
    if (field.type === 'checkbox' || field.type === 'radio') {
      setNativeProperty(element, 'checked', field.value === true || String(field.value).toLowerCase() === 'true');
      event(element, 'input');
      await commitField(element, commit);
    } else if (field.type === 'combobox') {
      setNativeProperty(element, 'value', String(field.value ?? ''));
      event(element, 'input');
      await commitField(element, commit);
    } else {
      await typeTextLikeUser(element, String(field.value ?? ''), { commit });
    }
  };
  const runVisualTool = async (tool, params = {}) => {
    if (tool === 'browser_click') {
      const { element, target } = requiredElement(params, tool);
      const point = await elementCenter(element);
      anchorCursorTo(element);
      await moveCursorTo(point.x, point.y);
      await clickAt(point, { target: element, button: params.button, doubleClick: params.doubleClick });
      return ok(`Clicked ${target}`);
    }
    if (tool === 'browser_hover') {
      const { element, target } = requiredElement(params, tool);
      const point = await elementCenter(element);
      anchorCursorTo(element);
      await moveCursorTo(point.x, point.y);
      dispatchMoveAt(point, element);
      return ok(`Hovered ${target}`);
    }
    if (tool === 'browser_select_option') {
      const { element, target } = requiredElement(params, tool);
      const point = await elementCenter(element);
      anchorCursorTo(element);
      await moveCursorTo(point.x, point.y);
      dispatchMoveAt(point, element);
      element.focus?.({ preventScroll: true });
      const values = Array.isArray(params.values) ? params.values : [params.value ?? ''];
      setNativeProperty(element, 'value', String(values[0] ?? ''));
      event(element, 'input');
      await commitField(element, params.commit !== false);
      await settleAfterInteraction();
      return ok(`Selected option ${element.value || values[0] || ''} in ${target}`);
    }
    if (tool === 'browser_type') {
      const target = targetValue(params);
      const element = target ? resolve(target) : document.activeElement;
      if (!element) throw new Error(`Element not found: ${target || 'active element'}`);
      const point = await elementCenter(element);
      anchorCursorTo(element);
      await moveCursorTo(point.x, point.y);
      dispatchMoveAt(point, element);
      await typeTextLikeUser(element, String(params.text ?? ''), { commit: params.commit !== false });
      if (params.submit) {
        element.form?.requestSubmit?.();
        dispatchKeyEvent(element, 'keydown', 'Enter');
        dispatchKeyEvent(element, 'keyup', 'Enter');
      }
      await settleAfterInteraction();
      return ok(`Typed into ${target || 'active element'}`);
    }
    if (tool === 'browser_fill_form') {
      const fields = Array.isArray(params.fields) ? params.fields : [];
      for (const field of fields) await fillOneField(field);
      await settleAfterInteraction();
      return ok('Form fields filled');
    }
    if (tool === 'browser_press_key') {
      const active = document.activeElement || document.body || document.documentElement;
      if (active && active.getBoundingClientRect) {
        const rect = active.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) {
          const point = clampPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
          anchorCursorTo(active);
          await moveCursorTo(point.x, point.y);
        } else {
          setCursorPosition(state.cursor.x, state.cursor.y, { tilt: 0, phase: 'idle' });
        }
      } else {
        setCursorPosition(state.cursor.x, state.cursor.y, { tilt: 0, phase: 'idle' });
      }
      const key = String(params.key || '');
      active.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
      if (key === 'Enter' && active.form) active.form.requestSubmit?.();
      active.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
      await settleAfterInteraction();
      return ok(`Pressed ${key}`);
    }
    if (tool === 'browser_mouse_move_xy') {
      clearCursorAnchor();
      const point = requiredPoint(params);
      await moveCursorTo(point.x, point.y);
      dispatchMoveAt(point);
      return ok(`Moved mouse to ${Math.round(point.x)}, ${Math.round(point.y)}`, { x: point.x, y: point.y });
    }
    if (tool === 'browser_mouse_click_xy') {
      clearCursorAnchor();
      const point = requiredPoint(params);
      await moveCursorTo(point.x, point.y);
      await clickAt(point, { button: params.button, doubleClick: params.doubleClick });
      return ok(`Clicked mouse at ${Math.round(point.x)}, ${Math.round(point.y)}`, { x: point.x, y: point.y });
    }
    if (tool === 'browser_mouse_drag_xy') {
      clearCursorAnchor();
      const start = requiredPoint(params, 'startX', 'startY');
      const end = requiredPoint(params, 'endX', 'endY');
      const button = buttonName(params.button);
      await moveCursorTo(start.x, start.y);
      let target = targetAt(start);
      dispatchMoveAt(start, target);
      target.focus?.({ preventScroll: true });
      dispatchMouse(target, 'mousedown', start, button, 1, buttonMask(button));
      state.cursor.mouseDownButton = button;
      await moveCursorTo(end.x, end.y, {
        duration: 900,
        onStep: (point) => {
          target = targetAt(point, target);
          dispatchMouse(target, 'mousemove', point, button, 0, buttonMask(button));
        },
      });
      dispatchMouse(targetAt(end, target), 'mouseup', end, button, 1, 0);
      state.cursor.mouseDownButton = null;
      await settleAfterInteraction();
      return ok(`Dragged mouse from ${Math.round(start.x)}, ${Math.round(start.y)} to ${Math.round(end.x)}, ${Math.round(end.y)}`, { startX: start.x, startY: start.y, endX: end.x, endY: end.y });
    }
    if (tool === 'browser_mouse_down') {
      clearCursorAnchor();
      const point = optionalPoint(params);
      const button = buttonName(params.button);
      await moveCursorTo(point.x, point.y);
      const target = targetAt(point);
      dispatchMoveAt(point, target);
      dispatchMouse(target, 'mousedown', point, button, 1, buttonMask(button));
      state.cursor.mouseDownButton = button;
      return ok(`Mouse ${button} button down at ${Math.round(point.x)}, ${Math.round(point.y)}`, { x: point.x, y: point.y, button });
    }
    if (tool === 'browser_mouse_up') {
      clearCursorAnchor();
      const point = optionalPoint(params);
      const button = buttonName(params.button || state.cursor.mouseDownButton || 'left');
      await moveCursorTo(point.x, point.y);
      dispatchMouse(targetAt(point), 'mouseup', point, button, 1, 0);
      state.cursor.mouseDownButton = null;
      await settleAfterInteraction();
      return ok(`Mouse ${button} button up at ${Math.round(point.x)}, ${Math.round(point.y)}`, { x: point.x, y: point.y, button });
    }
    if (tool === 'browser_mouse_wheel') {
      clearCursorAnchor();
      const point = optionalPoint(params);
      const deltaX = numberParam(params, 'deltaX', false) ?? 0;
      const deltaY = numberParam(params, 'deltaY', false) ?? 0;
      await moveCursorTo(point.x, point.y);
      const target = targetAt(point);
      await applyWheelScrollFallback(target, point, deltaX, deltaY);
      return ok(`Scrolled mouse wheel at ${Math.round(point.x)}, ${Math.round(point.y)}`, { x: point.x, y: point.y, deltaX, deltaY });
    }
    throw new Error(`Unsupported visual browser MCP tool: ${tool}`);
  };
  const snapshot = (params = {}) => {
    const maxDepth = Number(params.depth ?? 8);
    const includeBoxes = Boolean(params.boxes);
    const lines = [`- Page URL: ${location.href}`, `- Page Title: ${document.title || ''}`, '', 'Snapshot:'];
    const walk = (element, depth) => {
      if (!element || depth > maxDepth || !visible(element)) return;
      const role = roleOf(element);
      const name = nameOf(element);
      const interesting = role !== 'generic' || name;
      if (interesting) {
        const ref = ensureRef(element);
        const rect = element.getBoundingClientRect();
        const indent = '  '.repeat(Math.max(0, depth));
        const box = includeBoxes ? ` [box=${Math.round(rect.left)},${Math.round(rect.top)},${Math.round(rect.width)},${Math.round(rect.height)}]` : '';
        const quoted = name ? ` "${name.replace(/"/g, '\\"')}"` : '';
        lines.push(`${indent}- ${role}${quoted} [ref=${ref}]${box}`);
      }
      for (const child of Array.from(element.children || [])) walk(child, depth + 1);
    };
    walk(document.body || document.documentElement, 0);
    return { ok: true, text: lines.join('\n'), data: { url: location.href, title: document.title || '' } };
  };
  const pageSummary = (params = {}) => {
    const query = clean(params.query || '', 120).toLowerCase();
    const includeBoxes = Boolean(params.includeBoxes);
    const rawMaxItems = Number(params.maxItems ?? 40);
    const maxItems = Number.isFinite(rawMaxItems) ? clamp(rawMaxItems, 5, 120) : 40;
    const roleFilter = new Set((Array.isArray(params.roles) ? params.roles : []).map((role) => String(role).toLowerCase()));
    const selector = [
      'button',
      'a[href]',
      'input',
      'textarea',
      'select',
      '[role]',
      '[tabindex]',
      '[contenteditable]',
      'summary',
      'label',
      'h1',
      'h2',
      'h3',
      'h4',
      'h5',
      'h6',
      '[aria-live]',
      '.alert',
      '[data-testid]',
    ].join(',');
    const uniqueElements = [];
    const seen = new Set();
    for (const element of Array.from(document.querySelectorAll(selector)).slice(0, 1600)) {
      if (!element || seen.has(element) || !visible(element)) continue;
      seen.add(element);
      uniqueElements.push(element);
      if (uniqueElements.length >= 500) break;
    }
    const scoreItem = (item) => {
      if (!query) return item.inViewport ? 10 : 0;
      const haystack = `${item.role} ${item.name} ${item.value} ${item.placeholder}`.toLowerCase();
      if (haystack === query) return 120;
      if (haystack.startsWith(query)) return 100;
      if (haystack.includes(query)) return 80;
      return item.inViewport ? 5 : 0;
    };
    const describe = (element) => {
      const role = roleOf(element);
      const rect = element.getBoundingClientRect();
      const name = nameOf(element);
      const value = 'value' in element ? clean(element.value, 80) : '';
      const placeholder = clean(element.getAttribute?.('placeholder') || '', 80);
      const disabled = isDisabled(element);
      const item = {
        ref: ensureRef(element),
        role,
        name,
        enabled: !disabled,
        disabled,
        visible: true,
        inViewport: inViewport(rect),
        required: Boolean(element.required || element.getAttribute?.('aria-required') === 'true'),
        invalid: Boolean(element.matches?.(':invalid') || element.getAttribute?.('aria-invalid') === 'true'),
        value,
        placeholder,
        checked: 'checked' in element ? Boolean(element.checked) : undefined,
        box: { x: Math.round(rect.left), y: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height) },
      };
      item.score = scoreItem(item);
      return item;
    };
    const allItems = uniqueElements
      .map(describe)
      .filter((item) => !roleFilter.size || roleFilter.has(item.role));
    const actionRoles = new Set(['button', 'link', 'checkbox', 'radio', 'combobox', 'textbox']);
    const isAction = (item) => actionRoles.has(item.role) || item.name || item.placeholder;
    const ranked = (items) => items
      .slice()
      .sort((a, b) => b.score - a.score || Number(b.inViewport) - Number(a.inViewport) || Number(b.enabled) - Number(a.enabled) || a.ref.localeCompare(b.ref))
      .slice(0, maxItems);
    const actionTargets = ranked(allItems.filter((item) => isAction(item) && item.enabled));
    const formFields = ranked(allItems.filter((item) => ['textbox', 'checkbox', 'radio', 'combobox'].includes(item.role)));
    const disabledControls = ranked(allItems.filter((item) => item.disabled));
    const semantic = ranked(allItems.filter((item) => ['heading', 'tab', 'alert', 'status'].includes(item.role) || item.role.startsWith('h')));
    const topMatches = query ? ranked(allItems.filter((item) => item.score >= 80)) : [];
    const formatItem = (item) => {
      const name = item.name || item.placeholder || item.value || '';
      const quoted = name ? ` "${name.replace(/"/g, '\\"')}"` : '';
      const flags = [
        item.enabled ? 'enabled' : 'disabled',
        item.inViewport ? 'inViewport' : 'offscreen',
        item.required ? 'required' : '',
        item.invalid ? 'invalid' : '',
        item.value ? `value="${item.value.replace(/"/g, '\\"')}"` : '',
      ].filter(Boolean).join(' ');
      const box = includeBoxes ? ` [box=${item.box.x},${item.box.y},${item.box.width},${item.box.height}]` : '';
      return `- ${item.role}${quoted} [ref=${item.ref}] ${flags}${box}`;
    };
    const section = (title, items) => [`${title}:`, ...(items.length ? items.map(formatItem) : ['- none'])];
    const lines = [
      `- Page URL: ${location.href}`,
      `- Page Title: ${document.title || ''}`,
      `- Active: ${document.activeElement ? formatItem(describe(document.activeElement)) : 'none'}`,
      query ? `- Query: "${query}"` : '- Query: none',
      '',
      ...section('Top matches', topMatches),
      '',
      ...section('Action targets', actionTargets),
      '',
      ...section('Form fields', formFields),
      '',
      ...section('Disabled controls', disabledControls),
      '',
      ...section('Headings/Tabs/Alerts', semantic),
    ];
    return ok(lines.join('\n'), {
      url: location.href,
      title: document.title || '',
      query,
      active: document.activeElement ? describe(document.activeElement) : null,
      topMatches,
      actionTargets,
      formFields,
      disabledControls,
      semantic,
    });
  };
  const highlight = (element, style) => {
    const rect = element.getBoundingClientRect();
    let overlay = state.highlighted;
    if (!overlay) {
      overlay = document.createElement('div');
      overlay.setAttribute('data-mergen-mcp-highlight', 'true');
      Object.assign(overlay.style, { position: 'fixed', pointerEvents: 'none', zIndex: 2147483647, border: '2px solid #f59e0b', background: 'rgba(245,158,11,0.12)' });
      document.documentElement.appendChild(overlay);
      state.highlighted = overlay;
    }
    Object.assign(overlay.style, { left: `${Math.round(rect.left)}px`, top: `${Math.round(rect.top)}px`, width: `${Math.round(rect.width)}px`, height: `${Math.round(rect.height)}px`, display: 'block' });
    if (style) overlay.style.cssText += `;${style}`;
  };
  const ok = (text, data) => ({ ok: true, text, data: data || { url: location.href, title: document.title || '' } });
  const fail = (message) => ({ ok: false, error: message });
  window.__mergenMcpRun = (tool, params = {}) => {
    try {
      if (tool === 'browser_snapshot') return snapshot(params);
      if (tool === 'browser_page_summary') return pageSummary(params);
      if (tool === 'browser_console_messages') return fail('Console capture is not implemented by Mergen Browser MCP yet.');
      if (tool === 'browser_network_requests') return fail('Network request capture is not implemented by Mergen Browser MCP yet.');
      if (tool === 'browser_network_request') return fail('Detailed network capture is not implemented by Mergen Browser MCP yet.');
      if (tool === 'browser_resize') return fail('Resize is controlled by the Mergen Browser panel size.');
      if (tool === 'browser_wait_for') {
        const text = params.text ? String(params.text) : '';
        const gone = params.textGone ? String(params.textGone) : '';
        if (!text && !gone) return fail('browser_wait_for requires text or textGone in the embedded page; fixed waits are handled by the MCP helper.');
        const body = document.body?.innerText || '';
        if (text && body.includes(text)) return ok(`Text found: ${text}`);
        if (gone && !body.includes(gone)) return ok(`Text is gone: ${gone}`);
        return fail(text ? `Text not found: ${text}` : `Text is still visible: ${gone}`);
      }
      if (tool.startsWith('browser_localstorage_') || tool.startsWith('browser_sessionstorage_')) {
        const store = tool.includes('sessionstorage') ? sessionStorage : localStorage;
        const action = tool.split('_').pop();
        if (action === 'list') return ok(Array.from({ length: store.length }, (_, i) => `${store.key(i)}=${store.getItem(store.key(i))}`).join('\n') || 'Storage is empty');
        if (action === 'get') return ok(String(store.getItem(params.key) ?? ''));
        if (action === 'set') { store.setItem(params.key, String(params.value ?? '')); return ok(`Storage item set: ${params.key}`); }
        if (action === 'delete') { store.removeItem(params.key); return ok(`Storage item deleted: ${params.key}`); }
        if (action === 'clear') { store.clear(); return ok('Storage cleared'); }
      }
      if (tool === 'browser_cookie_list') return ok(document.cookie || 'No document.cookie entries visible to JavaScript');
      if (tool === 'browser_cookie_get') return ok((document.cookie.split('; ').find((c) => c.startsWith(`${params.name}=`)) || '').split('=').slice(1).join('='));
      if (tool === 'browser_cookie_set') { document.cookie = `${params.name}=${params.value}; path=${params.path || '/'}`; return ok(`Cookie set: ${params.name}`); }
      if (tool === 'browser_cookie_delete') { document.cookie = `${params.name}=; Max-Age=0; path=/`; return ok(`Cookie deleted: ${params.name}`); }
      if (tool === 'browser_cookie_clear') { for (const c of document.cookie.split('; ')) document.cookie = `${c.split('=')[0]}=; Max-Age=0; path=/`; return ok('Visible document cookies cleared'); }
      if (['browser_click', 'browser_hover', 'browser_select_option', 'browser_type', 'browser_fill_form', 'browser_press_key', 'browser_mouse_move_xy', 'browser_mouse_click_xy', 'browser_mouse_drag_xy', 'browser_mouse_down', 'browser_mouse_up', 'browser_mouse_wheel'].includes(tool)) return runVisualTool(tool, params);
      const element = resolve(targetValue(params));
      if (['browser_highlight'].includes(tool) && !element) return fail(`Element not found: ${targetValue(params)}`);
      if (tool === 'browser_highlight') { highlight(element, params.style); return ok(`Highlighted ${params.target}`); }
      if (tool === 'browser_hide_highlight') { if (state.highlighted) state.highlighted.style.display = 'none'; return ok('Highlight hidden'); }
      if (tool === 'browser_evaluate') {
        return runEvaluateTool(params, element);
      }
      return fail(`Unsupported browser MCP tool in page script: ${tool}`);
    } catch (error) {
      return fail(error && error.stack ? String(error.stack) : String(error));
    }
  };
  window.addEventListener('scroll', scheduleCursorAnchorSync, true);
  window.addEventListener('resize', scheduleCursorAnchorSync);
  window.__mergenMcpRun.version = 6;
}
"#;

#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Controller2, ICoreWebView2Profile6, ICoreWebView2WebMessageReceivedEventArgs,
    ICoreWebView2_13, COREWEBVIEW2_COLOR,
};
#[cfg(target_os = "windows")]
use webview2_com::WebMessageReceivedEventHandler;
#[cfg(target_os = "windows")]
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, ContentLoadingEventHandler,
    NavigationCompletedEventHandler, NavigationStartingEventHandler, SourceChangedEventHandler,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::WinRT::EventRegistrationToken;

#[cfg(target_os = "windows")]
struct WindowsWebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    source_changed_token: Option<EventRegistrationToken>,
    web_message_received_token: Option<EventRegistrationToken>,
    navigation_starting_token: Option<EventRegistrationToken>,
    content_loading_token: Option<EventRegistrationToken>,
    navigation_completed_token: Option<EventRegistrationToken>,
}

#[cfg(target_os = "windows")]
fn current_webview_source(webview: &ICoreWebView2) -> windows::core::Result<String> {
    use webview2_com::take_pwstr;
    use windows::core::PWSTR;

    let mut source_ptr = PWSTR::null();
    unsafe {
        webview.Source(&mut source_ptr)?;
    }
    // take_pwstr converts the COM-allocated PWSTR to String and frees it
    Ok(take_pwstr(source_ptr))
}

#[cfg(target_os = "windows")]
fn web_message_as_string(
    args: &ICoreWebView2WebMessageReceivedEventArgs,
) -> windows::core::Result<String> {
    use webview2_com::take_pwstr;
    use windows::core::PWSTR;

    let mut message_ptr = PWSTR::null();
    unsafe {
        args.TryGetWebMessageAsString(&mut message_ptr)?;
    }
    Ok(take_pwstr(message_ptr))
}

#[cfg(target_os = "windows")]
fn configure_webview_profile(webview: &ICoreWebView2) {
    if let Err(err) = enable_profile_autosave(webview) {
        log::warn!(
            "WebView2 password/autofill profile settings were not applied: {:?}",
            err
        );
    }
}

#[cfg(target_os = "windows")]
fn configure_webview_controller(controller: &ICoreWebView2Controller) {
    if let Err(err) = set_default_browser_background(controller) {
        log::warn!(
            "WebView2 default background color was not applied: {:?}",
            err
        );
    }
}

#[cfg(target_os = "windows")]
fn set_default_browser_background(
    controller: &ICoreWebView2Controller,
) -> windows::core::Result<()> {
    use windows::core::Interface as _;

    let controller2: ICoreWebView2Controller2 = controller.cast()?;
    unsafe {
        controller2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
            A: 255,
            R: 255,
            G: 255,
            B: 255,
        })?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn enable_profile_autosave(webview: &ICoreWebView2) -> windows::core::Result<()> {
    use windows::core::Interface as _;
    use windows::Win32::Foundation::BOOL;

    let webview_with_profile: ICoreWebView2_13 = webview.cast()?;
    let profile = unsafe { webview_with_profile.Profile()? };
    let profile_autofill: ICoreWebView2Profile6 = profile.cast()?;

    unsafe {
        profile_autofill.SetIsPasswordAutosaveEnabled(BOOL::from(true))?;
        profile_autofill.SetIsGeneralAutofillEnabled(BOOL::from(true))?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn path_to_null_terminated_wide(path: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn install_design_inspect_bootstrap(
    webview: &ICoreWebView2,
    token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::mpsc;

    let (script_tx, script_rx) = mpsc::channel::<std::result::Result<(), windows::core::Error>>();
    let script_handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |result, _script_id| -> windows::core::Result<()> {
            let _ = script_tx.send(result);
            Ok(())
        },
    ));

    let script = windows::core::HSTRING::from(design_inspect_bootstrap_script(token));
    unsafe {
        webview.AddScriptToExecuteOnDocumentCreated(&script, &script_handler)?;
    }
    webview2_com::wait_with_pump(script_rx).map_err(|e| std::io::Error::other(e.to_string()))??;
    Ok(())
}

#[cfg(target_os = "windows")]
fn create_webview_sync(
    parent_hwnd: windows::Win32::Foundation::HWND,
    event_sender: Sender<BrowserEvent>,
    design_inspect_token: String,
    user_data_folder: Option<PathBuf>,
) -> Result<
    (
        ICoreWebView2Controller,
        ICoreWebView2,
        EventRegistrationToken,
        EventRegistrationToken,
        EventRegistrationToken,
        EventRegistrationToken,
        EventRegistrationToken,
    ),
    Box<dyn std::error::Error>,
> {
    use std::sync::mpsc;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Environment, ICoreWebView2EnvironmentOptions,
    };
    use webview2_com::{
        CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
        CreateCoreWebView2EnvironmentCompletedHandler,
    };
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    // Channel for environment creation result
    let (env_tx, env_rx) =
        mpsc::channel::<std::result::Result<ICoreWebView2Environment, windows::core::Error>>();

    // Create environment handler using webview2-com helper
    let env_handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |result: std::result::Result<(), windows::core::Error>,
              env: Option<ICoreWebView2Environment>|
              -> windows::core::Result<()> {
            let send_result = if result.is_ok() {
                env.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
            } else {
                Err(windows::core::Error::from(
                    windows::Win32::Foundation::E_FAIL,
                ))
            };
            let _ = env_tx.send(send_result);
            Ok(())
        },
    ));

    let environment_options: ICoreWebView2EnvironmentOptions =
        CoreWebView2EnvironmentOptions::default().into();
    let user_data_folder_wide = user_data_folder
        .as_deref()
        .map(path_to_null_terminated_wide);
    let user_data_folder_pcwstr = user_data_folder_wide
        .as_ref()
        .map(|wide| PCWSTR(wide.as_ptr()))
        .unwrap_or_else(PCWSTR::null);

    if let Some(path) = user_data_folder.as_ref() {
        std::fs::create_dir_all(path)?;
        log::info!("WebView2 user data folder: {}", path.display());
    }

    // Create environment (async) with an explicit persistent profile folder.
    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            user_data_folder_pcwstr,
            &environment_options,
            &env_handler,
        )?;
    }

    // Wait for environment
    let environment = webview2_com::wait_with_pump(env_rx)
        .map_err(|e| std::io::Error::other(e.to_string()))??;

    // Channel for controller creation
    let (ctrl_tx, ctrl_rx) = mpsc::channel::<
        std::result::Result<(ICoreWebView2Controller, ICoreWebView2), windows::core::Error>,
    >();

    // Create controller handler using webview2-com helper
    let ctrl_handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
        move |result: std::result::Result<(), windows::core::Error>,
              ctrl: Option<ICoreWebView2Controller>|
              -> windows::core::Result<()> {
            let send_result = if result.is_ok() {
                if let Some(controller) = ctrl {
                    // Get webview from controller
                    unsafe {
                        match controller.CoreWebView2() {
                            Ok(webview) => Ok((controller, webview)),
                            Err(e) => Err(e),
                        }
                    }
                } else {
                    Err(windows::core::Error::from(
                        windows::Win32::Foundation::E_FAIL,
                    ))
                }
            } else {
                Err(windows::core::Error::from(
                    windows::Win32::Foundation::E_FAIL,
                ))
            };
            let _ = ctrl_tx.send(send_result);
            Ok(())
        },
    ));

    // Create controller (async)
    unsafe {
        environment.CreateCoreWebView2Controller(parent_hwnd, &ctrl_handler)?;
    }

    // Wait for controller and webview
    let (controller, webview) = webview2_com::wait_with_pump(ctrl_rx)
        .map_err(|e| std::io::Error::other(e.to_string()))??;

    configure_webview_controller(&controller);
    configure_webview_profile(&webview);
    install_design_inspect_bootstrap(&webview, &design_inspect_token)?;

    let web_message_sender = event_sender.clone();
    let web_message_token = design_inspect_token.clone();
    let web_message_handler = WebMessageReceivedEventHandler::create(Box::new(
        move |_webview, args| -> windows::core::Result<()> {
            let Some(args) = args else {
                return Ok(());
            };
            if let Ok(message) = web_message_as_string(&args) {
                if let Some(event) = parse_design_inspect_message(&message, &web_message_token) {
                    let _ = web_message_sender.send(event);
                }
            }
            Ok(())
        },
    ));

    let mut web_message_received_token = EventRegistrationToken::default();
    unsafe {
        webview
            .add_WebMessageReceived(&web_message_handler, &mut web_message_received_token)
            .map_err(|e| {
                format!(
                    "Failed to register WebView2 WebMessageReceived handler: {:?}",
                    e
                )
            })?;
    }

    // Register SourceChanged event handler to emit BrowserEvent::UrlChanged
    let source_sender = event_sender.clone();
    let source_changed_handler =
        SourceChangedEventHandler::create(Box::new(move |webview, _args| {
            if let Some(webview) = webview {
                match current_webview_source(&webview) {
                    Ok(url) if !url.is_empty() => {
                        let _ = source_sender.send(BrowserEvent::UrlChanged(url));
                    }
                    Ok(_) => {}
                    Err(err) => {
                        let _ = source_sender.send(BrowserEvent::Error(format!(
                            "WebView2 source read failed: {:?}",
                            err
                        )));
                    }
                }
            }
            Ok(())
        }));

    let mut source_changed_token = EventRegistrationToken::default();
    unsafe {
        webview
            .add_SourceChanged(&source_changed_handler, &mut source_changed_token)
            .map_err(|e| format!("Failed to register WebView2 SourceChanged handler: {:?}", e))?;
    }

    let navigation_start_sender = event_sender.clone();
    let navigation_starting_handler =
        NavigationStartingEventHandler::create(Box::new(move |webview, _args| {
            let url = webview
                .and_then(|webview| current_webview_source(&webview).ok())
                .unwrap_or_default();
            let _ = navigation_start_sender.send(BrowserEvent::LoadStarted(url));
            Ok(())
        }));
    let mut navigation_starting_token = EventRegistrationToken::default();
    unsafe {
        webview
            .add_NavigationStarting(&navigation_starting_handler, &mut navigation_starting_token)
            .map_err(|e| {
                format!(
                    "Failed to register WebView2 NavigationStarting handler: {:?}",
                    e
                )
            })?;
    }

    let content_loading_sender = event_sender.clone();
    let content_loading_handler =
        ContentLoadingEventHandler::create(Box::new(move |webview, _args| {
            let url = webview
                .and_then(|webview| current_webview_source(&webview).ok())
                .unwrap_or_default();
            let _ = content_loading_sender.send(BrowserEvent::LoadStarted(url));
            Ok(())
        }));
    let mut content_loading_token = EventRegistrationToken::default();
    unsafe {
        webview
            .add_ContentLoading(&content_loading_handler, &mut content_loading_token)
            .map_err(|e| {
                format!(
                    "Failed to register WebView2 ContentLoading handler: {:?}",
                    e
                )
            })?;
    }

    let navigation_completed_sender = event_sender.clone();
    let navigation_completed_handler =
        NavigationCompletedEventHandler::create(Box::new(move |webview, _args| {
            let url = webview
                .and_then(|webview| current_webview_source(&webview).ok())
                .unwrap_or_default();
            let _ = navigation_completed_sender.send(BrowserEvent::LoadFinished(url));
            Ok(())
        }));
    let mut navigation_completed_token = EventRegistrationToken::default();
    unsafe {
        webview
            .add_NavigationCompleted(
                &navigation_completed_handler,
                &mut navigation_completed_token,
            )
            .map_err(|e| {
                format!(
                    "Failed to register WebView2 NavigationCompleted handler: {:?}",
                    e
                )
            })?;
    }

    log::info!("WebView2 environment and controller created successfully");
    Ok((
        controller,
        webview,
        source_changed_token,
        web_message_received_token,
        navigation_starting_token,
        content_loading_token,
        navigation_completed_token,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_png_bytes(fill: [u8; 4], override_pixel: Option<([u8; 4], u32, u32)>) -> Vec<u8> {
        let mut image = image::RgbaImage::from_pixel(4, 4, image::Rgba(fill));
        if let Some((pixel, x, y)) = override_pixel {
            image.put_pixel(x, y, image::Rgba(pixel));
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("test png should encode");
        bytes.into_inner()
    }

    #[test]
    fn browser_bounds_from_egui_rect_uses_pixels_per_point() {
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(300.0, 400.0));

        let bounds = browser_bounds_from_egui_rect(rect, 1.0);
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.y, 200);
        assert_eq!(bounds.width, 300);
        assert_eq!(bounds.height, 400);

        // With 2x scale
        let bounds_2x = browser_bounds_from_egui_rect(rect, 2.0);
        assert_eq!(bounds_2x.x, 200);
        assert_eq!(bounds_2x.y, 400);
        assert_eq!(bounds_2x.width, 600);
        assert_eq!(bounds_2x.height, 800);
    }

    #[test]
    fn browser_bounds_clamps_non_positive_size() {
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(0.0, -10.0));

        let bounds = browser_bounds_from_egui_rect(rect, 1.0);
        assert_eq!(bounds.width, 1); // Clamped to minimum
        assert_eq!(bounds.height, 1); // Clamped to minimum
    }

    #[test]
    fn embedded_browser_new_has_uninitialized_status() {
        let browser = EmbeddedBrowser::new();
        assert!(matches!(browser.status(), BrowserStatus::Uninitialized));
    }

    #[test]
    fn embedded_browser_stores_persistent_user_data_folder() {
        let path =
            PathBuf::from(r"C:\Users\demo\AppData\Roaming\Mergen\MergenADE\webview2\projects\7");

        let browser = EmbeddedBrowser::new_with_user_data_folder(Some(path.clone()));

        assert_eq!(browser.user_data_folder(), Some(&path));
    }

    #[test]
    fn embedded_browser_drain_events_empty_when_no_events() {
        let mut browser = EmbeddedBrowser::new();
        let events = browser.drain_events();
        assert!(events.is_empty());
    }

    #[test]
    fn browser_mcp_screenshot_output_extracts_base64_data() {
        let output = browser_mcp_screenshot_output_from_devtools_raw(r#"{"data":"abcd"}"#, "png")
            .expect("screenshot output should parse");

        assert!(output.text.contains("4 base64 bytes"));
        assert_eq!(output.data.unwrap()["base64"].as_str(), Some("abcd"));
    }

    #[test]
    fn browser_mcp_screenshot_output_rejects_empty_data() {
        let err =
            browser_mcp_screenshot_output_from_devtools_raw(r#"{"data":""}"#, "png").unwrap_err();

        assert!(err.contains("did not return screenshot data"));
    }

    #[test]
    fn browser_mcp_screenshot_params_default_to_fast_jpeg() {
        let (format, params) = screenshot_cdp_params(&json!({}));

        assert_eq!(format, "jpeg");
        assert_eq!(params["format"].as_str(), Some("jpeg"));
        assert_eq!(params["quality"].as_i64(), Some(74));
        assert_eq!(params["optimizeForSpeed"].as_bool(), Some(true));
        assert_eq!(params["captureBeyondViewport"].as_bool(), Some(false));
        assert_eq!(params["fromSurface"].as_bool(), Some(true));
    }

    #[test]
    fn browser_mcp_screenshot_params_preserve_explicit_png_and_full_page() {
        let (format, params) =
            screenshot_cdp_params(&json!({ "type": "png", "fullPage": true, "quality": 10 }));

        assert_eq!(format, "png");
        assert_eq!(params["format"].as_str(), Some("png"));
        assert_eq!(params.get("quality"), None);
        assert_eq!(params["captureBeyondViewport"].as_bool(), Some(true));
        assert_eq!(params["fromSurface"].as_bool(), Some(true));
    }

    #[test]
    fn browser_mcp_screenshot_retry_params_disable_surface_capture() {
        let (_, params) = screenshot_cdp_params_with_surface(&json!({}), false);

        assert_eq!(params["fromSurface"].as_bool(), Some(false));
        assert_eq!(params["optimizeForSpeed"].as_bool(), Some(true));
    }

    #[test]
    fn browser_mcp_screenshot_black_detector_retries_only_near_black_images() {
        let black_png = test_png_bytes([0, 0, 0, 255], None);
        let varied_png = test_png_bytes([8, 10, 12, 255], Some(([240, 240, 240, 255], 1, 1)));

        assert!(screenshot_bytes_are_probably_black(&black_png));
        assert!(!screenshot_bytes_are_probably_black(&varied_png));
    }

    #[test]
    fn browser_mcp_runtime_evaluate_output_extracts_tool_result() {
        let output = browser_mcp_output_from_devtools_runtime_raw(
            r#"{"result":{"type":"object","value":{"ok":true,"text":"Moved","data":{"x":12}}}}"#,
        )
        .expect("runtime output should parse");

        assert_eq!(output.text, "Moved");
        assert_eq!(output.data.unwrap()["x"].as_i64(), Some(12));
    }

    #[test]
    fn browser_mcp_runtime_evaluate_output_reports_exceptions() {
        let err = browser_mcp_output_from_devtools_runtime_raw(
            r#"{"exceptionDetails":{"text":"Uncaught","exception":{"description":"Error: bad coordinates"}}}"#,
        )
        .unwrap_err();

        assert!(err.contains("bad coordinates"));
    }

    #[test]
    fn browser_mcp_visual_tools_use_async_script_path() {
        for tool in [
            "browser_evaluate",
            "browser_page_summary",
            "browser_click",
            "browser_hover",
            "browser_select_option",
            "browser_fill_form",
            "browser_press_key",
            "browser_type",
            "browser_mouse_move_xy",
            "browser_mouse_click_xy",
            "browser_mouse_drag_xy",
            "browser_mouse_down",
            "browser_mouse_up",
            "browser_mouse_wheel",
        ] {
            assert!(
                browser_mcp_tool_uses_async_script(tool),
                "{tool} should await page-side cursor motion"
            );
        }

        assert!(!browser_mcp_tool_uses_async_script("browser_snapshot"));
        assert!(!browser_mcp_tool_uses_async_script(
            "browser_take_screenshot"
        ));
    }

    #[test]
    fn parse_design_inspect_ready_message() {
        let event = parse_design_inspect_message(
            r#"{"source":"mergen-ade-design-inspect","token":"test-token","type":"ready"}"#,
            "test-token",
        );

        assert!(matches!(event, Some(BrowserEvent::DesignInspectReady)));
    }

    #[test]
    fn parse_design_inspect_click_message() {
        let event = parse_design_inspect_message(
            r#"{
                "source":"mergen-ade-design-inspect",
                "token":"test-token",
                "type":"click",
                "pageUrl":"https://example.com/page",
                "url":"https://example.com",
                "tag":"button",
                "id":"save",
                "classes":["primary"],
                "text":"Save",
                "selector":"button#save",
                "rect":{"x":1,"y":2,"width":3,"height":4},
                "styles":{"display":"flex"}
            }"#,
            "test-token",
        );

        let Some(BrowserEvent::DesignElementClicked(info)) = event else {
            panic!("expected click event");
        };
        assert_eq!(info.page_url, "https://example.com/page");
        assert_eq!(info.tag, "button");
        assert_eq!(info.selector, "button#save");
        assert_eq!(info.rect.width, 3);
        assert_eq!(info.styles.get("display"), Some(&"flex".to_owned()));
    }

    #[test]
    fn parse_design_inspect_ignores_hover_message() {
        // Stale hover messages from old injected scripts should be ignored
        let event = parse_design_inspect_message(
            r#"{
                "source":"mergen-ade-design-inspect",
                "token":"test-token",
                "type":"hover",
                "pageUrl":"https://example.com/page",
                "url":"https://example.com",
                "tag":"button",
                "id":"save",
                "classes":["primary"],
                "text":"Save",
                "selector":"button#save",
                "rect":{"x":1,"y":2,"width":3,"height":4},
                "styles":{"display":"flex"}
            }"#,
            "test-token",
        );

        assert!(event.is_none(), "hover messages should be ignored");
    }

    #[test]
    fn parse_design_inspect_ignores_unknown_source() {
        assert!(parse_design_inspect_message(
            r#"{"source":"other","token":"test-token","type":"ready"}"#,
            "test-token"
        )
        .is_none());
    }

    #[test]
    fn parse_design_inspect_ignores_wrong_token() {
        assert!(parse_design_inspect_message(
            r#"{"source":"mergen-ade-design-inspect","token":"wrong","type":"ready"}"#,
            "test-token"
        )
        .is_none());
    }

    #[test]
    fn embedded_browser_tracks_design_inspect_enabled_state() {
        let mut browser = EmbeddedBrowser::new();

        browser.set_design_inspect_enabled(true);
        assert!(browser.design_inspect_enabled());

        browser.set_design_inspect_enabled(false);
        assert!(!browser.design_inspect_enabled());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn browser_wait_for_script_does_not_fake_fixed_wait_success() {
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("request accepted"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("fixed waits are handled by the MCP helper"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn browser_mcp_automation_script_includes_visible_cursor_and_mouse_tools() {
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("window.__mergenMcpRun.version = 6"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("data-mergen-mcp-cursor"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("data-mergen-mcp-cursor-aura"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("data-mergen-mcp-cursor-focus"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("data-mergen-mcp-cursor-pointer"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("createElementNS('http://www.w3.org/2000/svg', 'svg')"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("moveCursorTo"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("organicCursorPoint"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("steadyCursorForAction"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("data-mergen-mcp-cursor-phase"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("0.72"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains(
            "const finalPoint = setCursorPosition(end.x, end.y, { tilt: 0, phase: 'idle' })"
        ));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("state.cursor.visible ? clampPoint(state.cursor.x, state.cursor.y) : clampPoint(end.x - 96, end.y - 64)"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("transform: 'translate(-50%, -50%)'"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("left: '-13px'"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("data-mergen-mcp-cursor-halo"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("rgba(245,158,11,0.20)"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("border: '2px solid rgba(245,158,11,0.95)'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("stableElementCenterAfterScroll"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("anchorCursorTo(element)"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("window.addEventListener('scroll', scheduleCursorAnchorSync, true)"));
        assert!(
            MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("scrollBy({ left: deltaX, top: deltaY")
        );
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("clamp(Math.round(distance * 1.15), 650, 900)"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("duration: 900"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_move_xy"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_click_xy"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_drag_xy"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_down"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_up"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_wheel"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("180, 340"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_mouse_')) return fail"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("is not implemented by Mergen Browser MCP yet.`);"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn browser_mcp_automation_script_blocks_javascript_clicks_in_evaluate() {
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("JavaScript clicks are blocked in Mergen Browser MCP"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("INTERACTIVE_EVALUATE_PATTERN"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("\\.click\\s*\\("));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("\\[['\"]click['\"]\\]\\s*\\("));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("patchMethod(window.HTMLElement?.prototype, 'click'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("patchMethod(window.EventTarget?.prototype, 'dispatchEvent'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("for (const restoreMethod of restore.reverse()) restoreMethod();"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains(
            "const result = typeof value === 'function' ? value(element || undefined) : value;"
        ));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn browser_mcp_automation_script_types_like_user_and_commits_fields() {
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("typeTextLikeUser"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("nativePropertySetter"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("setNativeProperty(element, 'value'"));
        assert!(
            MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("setNativeProperty(element, 'checked'")
        );
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("new KeyboardEvent"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("new InputEvent"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("'beforeinput'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("'input'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("'change'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("'focusout'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("'blur'"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("commitField"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("params.commit !== false"));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("const inputText ="));
        assert!(!MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("element.value = text"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn browser_mcp_automation_script_includes_fast_page_summary() {
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("const pageSummary ="));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("browser_page_summary"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("Top matches"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("Action targets"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("Form fields"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("Disabled controls"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("Headings/Tabs/Alerts"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("scoreItem"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("roleFilter"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("includeBoxes"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("uniqueElements.length >= 500"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains(".slice(0, 1600)"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("isDisabled"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("inViewport"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn browser_mcp_automation_script_settles_after_interactions() {
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("settleAfterInteraction"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("new MutationObserver"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT.contains("setTimeout(finish, 180)"));
        assert!(MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            .contains("requestAnimationFrame(() => requestAnimationFrame(armQuietTimer))"));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn non_windows_browser_reports_unsupported() {
        let mut browser = EmbeddedBrowser::new();
        let status = browser.ensure_created(None);
        assert!(matches!(status, BrowserStatus::Unsupported(_)));
    }

    #[test]
    fn browser_shutdown_resets_status() {
        let mut browser = EmbeddedBrowser::new();
        browser.shutdown();
        assert!(matches!(browser.status(), BrowserStatus::Uninitialized));
    }
}
