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
use std::time::{SystemTime, UNIX_EPOCH};

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
    DesignElementHovered(DesignElementInfo),
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
static DESIGN_INSPECT_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

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
  if (window.__mergenDesignInspect && window.__mergenDesignInspect.version === 1) {
    postMessage?.(JSON.stringify({ source: SOURCE, token: TOKEN, type: "ready" }));
    return;
  }

  const state = {
    version: 1,
    enabled: false,
    current: null,
    overlay: null,
    lastSignature: "",
    pendingTimer: 0,
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

  function elementPayload(element) {
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
      type: "hover",
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

  function postHover(element) {
    if (!postMessage || !element) return;
    const payload = elementPayload(element);
    const signature = `${payload.url}|${payload.selector}|${payload.rect.x}|${payload.rect.y}|${payload.rect.width}|${payload.rect.height}`;
    if (signature === state.lastSignature) return;
    state.lastSignature = signature;
    postMessage(JSON.stringify(payload));
  }

  function scheduleHover(element) {
    window.clearTimeout(state.pendingTimer);
    state.pendingTimer = window.setTimeout(() => postHover(element), 120);
  }

  function onPointerMove(event) {
    if (!state.enabled) return;
    const element = event.target;
    if (!element || element === state.overlay || element.nodeType !== Node.ELEMENT_NODE) return;
    state.current = element;
    updateOverlay(element);
    scheduleHover(element);
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
    state.lastSignature = "";
    window.clearTimeout(state.pendingTimer);
    if (state.enabled) {
      document.addEventListener("pointermove", onPointerMove, true);
      window.addEventListener("scroll", refreshOverlay, true);
      window.addEventListener("resize", refreshOverlay, true);
    } else {
      document.removeEventListener("pointermove", onPointerMove, true);
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

  window.__mergenDesignInspect = { version: 1, setEnabled };
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
        "hover" => {
            if wire.tag.trim().is_empty() || wire.selector.trim().is_empty() {
                return None;
            }
            Some(BrowserEvent::DesignElementHovered(DesignElementInfo {
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
            Ok((controller, webview, source_changed_token, web_message_received_token)) => {
                self.inner = Some(WindowsWebView {
                    controller,
                    webview,
                    source_changed_token: Some(source_changed_token),
                    web_message_received_token: Some(web_message_received_token),
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
            | "browser_tabs"
            | "browser_close"
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
            let payload = json!({ "tool": tool, "params": params });
            let payload_json = serde_json::to_string(&payload).map_err(|err| err.to_string())?;
            let script = format!(
                "(() => {{ const __mergenMcpPayload = {payload_json}; {} return window.__mergenMcpRun(__mergenMcpPayload.tool, __mergenMcpPayload.params); }})()",
                MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT
            );
            let value = self.execute_script_value(&script)?;
            browser_mcp_output_from_script_value(value)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (tool, params);
            Err("Embedded browser automation is currently Windows-only".to_owned())
        }
    }

    fn run_mcp_screenshot_tool(
        &mut self,
        params: &JsonValue,
    ) -> Result<BrowserMcpToolOutput, String> {
        #[cfg(target_os = "windows")]
        {
            let image_type = params
                .get("type")
                .and_then(JsonValue::as_str)
                .unwrap_or("png");
            let format = if image_type.eq_ignore_ascii_case("jpeg") {
                "jpeg"
            } else {
                "png"
            };
            let capture_beyond_viewport = params
                .get("fullPage")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);
            let cdp_params = json!({
                "format": format,
                "captureBeyondViewport": capture_beyond_viewport
            });
            let raw = self.call_devtools_protocol_method("Page.captureScreenshot", &cdp_params)?;
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
                    "Screenshot captured from the embedded Mergen browser ({} base64 bytes).",
                    data.len()
                ),
                data: Some(json!({ "imageType": format, "base64": data })),
            })
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = params;
            Err("Embedded browser screenshots are currently Windows-only".to_owned())
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

#[cfg(target_os = "windows")]
const MERGEN_BROWSER_MCP_AUTOMATION_SCRIPT: &str = r#"
if (!window.__mergenMcpRun || window.__mergenMcpRun.version !== 2) {
  window.__mergenMcpState = window.__mergenMcpState || { refCounter: 1, consoleMessages: [], networkRequests: [], routes: [], highlighted: null };

  const state = window.__mergenMcpState;
  const clean = (value, max = 240) => String(value ?? '').replace(/[\u0000-\u001f\u007f]+/g, ' ').replace(/\s+/g, ' ').trim().slice(0, max);
  const visible = (element) => {
    if (!element || element.nodeType !== Node.ELEMENT_NODE) return false;
    const style = window.getComputedStyle(element);
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false;
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
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
  const event = (element, name) => element.dispatchEvent(new Event(name, { bubbles: true, cancelable: true }));
  const inputText = (element, text) => {
    element.scrollIntoView({ block: 'center', inline: 'center' });
    element.focus?.();
    if (element.isContentEditable) {
      element.textContent = text;
    } else if ('value' in element) {
      element.value = text;
    }
    event(element, 'input');
    event(element, 'change');
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
      if (tool === 'browser_console_messages') return fail('Console capture is not implemented by Mergen Browser MCP yet.');
      if (tool === 'browser_network_requests') return fail('Network request capture is not implemented by Mergen Browser MCP yet.');
      if (tool === 'browser_network_request') return fail('Detailed network capture is not implemented by Mergen Browser MCP yet.');
      if (tool === 'browser_tabs') return params.action === 'list' ? ok(`- 0: (current) [${document.title || 'Page'}](${location.href})`) : fail('Mergen Browser MCP supports only browser_tabs action=list.');
      if (tool === 'browser_close') return fail('Mergen Browser MCP does not destroy the embedded Mergen Browser panel.');
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
      if (tool === 'browser_press_key') {
        const active = document.activeElement || document.body;
        const key = String(params.key || '');
        active.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
        if (key === 'Enter' && active.form) active.form.requestSubmit?.();
        active.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
        return ok(`Pressed ${key}`);
      }
      if (tool === 'browser_fill_form') {
        for (const field of params.fields || []) {
          const element = resolve(field.target);
          if (!element) return fail(`Element not found: ${field.target}`);
          if (field.type === 'checkbox' || field.type === 'radio') element.checked = String(field.value) === 'true';
          else if (field.type === 'combobox') { element.value = String(field.value); }
          else inputText(element, String(field.value ?? ''));
          event(element, 'change');
        }
        return ok('Form fields filled');
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
      if (tool.startsWith('browser_mouse_')) return fail(`${tool} is not implemented by Mergen Browser MCP yet.`);
      const element = resolve(params.target);
      if (['browser_click', 'browser_hover', 'browser_select_option', 'browser_type', 'browser_evaluate', 'browser_highlight', 'browser_hide_highlight'].includes(tool) && !element && tool !== 'browser_evaluate' && tool !== 'browser_hide_highlight') return fail(`Element not found: ${params.target}`);
      if (tool === 'browser_click') { element.scrollIntoView({ block: 'center', inline: 'center' }); element.focus?.(); params.doubleClick ? element.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })) : element.click(); return ok(`Clicked ${params.target}`); }
      if (tool === 'browser_hover') { element.scrollIntoView({ block: 'center', inline: 'center' }); element.dispatchEvent(new MouseEvent('mouseover', { bubbles: true })); return ok(`Hovered ${params.target}`); }
      if (tool === 'browser_select_option') { const values = params.values || []; element.value = String(values[0] ?? ''); event(element, 'input'); event(element, 'change'); return ok(`Selected option ${element.value}`); }
      if (tool === 'browser_type') { inputText(element, String(params.text ?? '')); if (params.submit) { element.form?.requestSubmit?.(); element.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); } return ok(`Typed into ${params.target}`); }
      if (tool === 'browser_highlight') { highlight(element, params.style); return ok(`Highlighted ${params.target}`); }
      if (tool === 'browser_hide_highlight') { if (state.highlighted) state.highlighted.style.display = 'none'; return ok('Highlight hidden'); }
      if (tool === 'browser_evaluate') {
        const expr = String(params.function || '');
        const value = eval(`(${expr})`);
        const result = typeof value === 'function' ? value(element || undefined) : value;
        return ok(JSON.stringify(result, null, 2) ?? 'undefined', { result });
      }
      return fail(`Unsupported browser MCP tool in page script: ${tool}`);
    } catch (error) {
      return fail(error && error.stack ? String(error.stack) : String(error));
    }
  };
  window.__mergenMcpRun.version = 2;
}
"#;

#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Profile6, ICoreWebView2WebMessageReceivedEventArgs, ICoreWebView2_13,
};
#[cfg(target_os = "windows")]
use webview2_com::WebMessageReceivedEventHandler;
#[cfg(target_os = "windows")]
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, SourceChangedEventHandler,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::WinRT::EventRegistrationToken;

#[cfg(target_os = "windows")]
struct WindowsWebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    source_changed_token: Option<EventRegistrationToken>,
    web_message_received_token: Option<EventRegistrationToken>,
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

    log::info!("WebView2 environment and controller created successfully");
    Ok((
        controller,
        webview,
        source_changed_token,
        web_message_received_token,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_design_inspect_ready_message() {
        let event = parse_design_inspect_message(
            r#"{"source":"mergen-ade-design-inspect","token":"test-token","type":"ready"}"#,
            "test-token",
        );

        assert!(matches!(event, Some(BrowserEvent::DesignInspectReady)));
    }

    #[test]
    fn parse_design_inspect_hover_message() {
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

        let Some(BrowserEvent::DesignElementHovered(info)) = event else {
            panic!("expected hover event");
        };
        assert_eq!(info.page_url, "https://example.com/page");
        assert_eq!(info.tag, "button");
        assert_eq!(info.selector, "button#save");
        assert_eq!(info.rect.width, 3);
        assert_eq!(info.styles.get("display"), Some(&"flex".to_owned()));
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
