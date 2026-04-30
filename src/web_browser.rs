//! Embedded browser module for Mergen ADE.
//!
//! Provides a target-gated facade for embedded WebView functionality.
//! - Windows: Uses WebView2 for native browser rendering
//! - Non-Windows: Safe stub that reports unsupported status

use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};

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
    Error(String),
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
        let (event_sender, event_receiver) = channel();

        Self {
            status: BrowserStatus::Uninitialized,
            event_sender,
            event_receiver,
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
        let result = create_webview_sync(hwnd, self.event_sender.clone());

        match result {
            Ok((controller, webview)) => {
                self.inner = Some(WindowsWebView {
                    controller,
                    webview,
                });
                self.status = BrowserStatus::Ready;
                log::info!("WebView2 created successfully");

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

    /// Set the visibility of the browser view.
    pub fn set_visible(&mut self, visible: bool) {
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
            if let Some(_inner) = self.inner.take() {
                // Controller and webview are dropped automatically
                log::info!("WebView2 resources released");
            }
        }
        self.status = BrowserStatus::Uninitialized;
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

#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2Environment, ICoreWebView2, ICoreWebView2Controller,
};

#[cfg(target_os = "windows")]
struct WindowsWebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
}

#[cfg(target_os = "windows")]
fn create_webview_sync(
    parent_hwnd: windows::Win32::Foundation::HWND,
    _event_sender: Sender<BrowserEvent>,
) -> Result<(ICoreWebView2Controller, ICoreWebView2), Box<dyn std::error::Error>> {
    use std::sync::mpsc;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment;
    use webview2_com::{
        CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    };
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

    // Create environment (async)
    unsafe {
        CreateCoreWebView2Environment(&env_handler)?;
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

    log::info!("WebView2 environment and controller created successfully");
    Ok((controller, webview))
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
    fn embedded_browser_drain_events_empty_when_no_events() {
        let mut browser = EmbeddedBrowser::new();
        let events = browser.drain_events();
        assert!(events.is_empty());
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
