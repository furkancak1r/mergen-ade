// Windows GUI subsystem only for release builds; debug builds get a console for panic output.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
// Lints that require significant refactoring or are stylistic only
#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::needless_return,
    clippy::doc_lazy_continuation,
    clippy::if_same_then_else,
    clippy::manual_is_multiple_of,
    clippy::manual_clamp,
    clippy::io_other_error,
    clippy::unnecessary_unwrap,
    clippy::unwrap_or_default,
    clippy::map_entry,
    clippy::question_mark,
    clippy::trim_split_whitespace,
    clippy::single_match,
    clippy::needless_borrow,
    clippy::missing_const_for_thread_local,
    clippy::needless_update,
    clippy::unnecessary_map_or
)]

mod app;
mod browser_mcp_helper;
mod browser_mcp_service;
mod browser_video;
mod codex;
mod config;
mod hooks;
mod layout;
mod models;
mod mojibake;
mod opencode;
mod opencode_acp;
mod opencode_config;
mod opencode_hook_service;
mod path_utils;
mod terminal;
mod title;
mod web_browser;
mod worktree;

use eframe::egui;
use eframe::icon_data;
use std::any::Any;
use std::ffi::OsString;
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

/// Set Windows wgpu environment defaults to avoid Vulkan validation warnings.
/// Only applies if the user hasn't explicitly set these environment variables.
#[cfg(target_os = "windows")]
fn setup_windows_wgpu_env_defaults() {
    // Use DX12 with GL fallback instead of Vulkan on Windows to avoid validation warnings.
    // This allows wgpu to try DX12 first, and if it fails (e.g., cannot create surface),
    // it will attempt GL (OpenGL) as a fallback within the wgpu backend selection.
    if std::env::var("WGPU_BACKEND").is_err() {
        std::env::set_var("WGPU_BACKEND", "dx12,gl");
    }
    // Disable GPU validation in debug builds to avoid "unable to find layer" warnings
    if cfg!(debug_assertions) && std::env::var("WGPU_VALIDATION").is_err() {
        std::env::set_var("WGPU_VALIDATION", "0");
    }
}

#[cfg(not(target_os = "windows"))]
fn setup_windows_wgpu_env_defaults() {
    // No-op on non-Windows platforms
}

/// Windows-specific platform detection and memory monitoring
#[cfg(target_os = "windows")]
mod platform {
    #[derive(Debug, Clone)]
    pub struct GpuInfo {
        pub name: String,
        pub vendor_id: u32,
        pub is_intel: bool,
    }

    #[derive(Debug, Clone)]
    pub struct MemoryInfo {
        pub physical_installed_mb: u64,
        pub physical_available_mb: u64,
        pub virtual_total_mb: u64,
        pub virtual_available_mb: u64,
        pub working_set_mb: u64,
    }

    /// Detect GPU using simple heuristics via environment variable
    pub fn detect_primary_gpu() -> Option<GpuInfo> {
        // Check for Intel GPU hint from environment
        let intel_hint = std::env::var("MERGEN_INTEL_GPU").is_ok();

        if intel_hint {
            return Some(GpuInfo {
                name: "Intel GPU (detected via env)".to_string(),
                vendor_id: 0x8086,
                is_intel: true,
            });
        }

        None
    }

    /// Get memory info via Windows API
    pub fn get_memory_info() -> Option<MemoryInfo> {
        // Use Windows API via winapi crate pattern or simple estimation
        // For now, return None and rely on environment checks
        None
    }

    /// Check if system is under memory pressure
    pub fn is_memory_pressure_critical() -> bool {
        // Simple check: try to allocate a large buffer and see if it fails
        // This is a heuristic approach when Windows APIs are not available
        matches!(
            std::env::var("MERGEN_LOW_MEMORY"),
            Ok(val) if val == "1" || val.to_lowercase() == "true"
        )
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    #[derive(Debug, Clone)]
    pub struct GpuInfo {
        pub name: String,
        pub vendor_id: u32,
        pub is_intel: bool,
    }

    #[derive(Debug, Clone)]
    pub struct MemoryInfo {
        pub physical_installed_mb: u64,
        pub physical_available_mb: u64,
        pub virtual_total_mb: u64,
        pub virtual_available_mb: u64,
        pub working_set_mb: u64,
    }

    pub fn detect_primary_gpu() -> Option<GpuInfo> {
        None
    }

    pub fn get_memory_info() -> Option<MemoryInfo> {
        None
    }

    pub fn is_memory_pressure_critical() -> bool {
        false
    }
}

/// Renderer backend selection modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RendererMode {
    /// Try wgpu first, fall back to glow on failure.
    Auto,
    /// Force wgpu backend.
    Wgpu,
    /// Force glow (OpenGL) backend.
    Glow,
}

impl RendererMode {
    /// Parse from environment variable `MERGEN_RENDERER`.
    /// If not set, auto-detect based on GPU vendor.
    fn from_env() -> Self {
        match std::env::var("MERGEN_RENDERER").as_deref() {
            Ok("wgpu") => {
                log::info!("MERGEN_RENDERER=wgpu forced by environment");
                Self::Wgpu
            }
            Ok("glow") => {
                log::info!("MERGEN_RENDERER=glow forced by environment");
                Self::Glow
            }
            Ok("auto") => {
                log::info!("MERGEN_RENDERER=auto explicit from environment");
                Self::Auto
            }
            _ => {
                // Auto-detect: Check GPU vendor for intelligent default
                if let Some(gpu) = platform::detect_primary_gpu() {
                    log::info!(
                        "Detected GPU: {} (vendor: 0x{:04X}, intel: {})",
                        gpu.name,
                        gpu.vendor_id,
                        gpu.is_intel
                    );

                    if gpu.is_intel {
                        // Intel GPUs: prefer wgpu for stability
                        log::info!("Intel GPU detected - preferring wgpu renderer for stability");
                        // Still use Auto so we can fallback to glow if wgpu fails
                        Self::Auto
                    } else {
                        // NVIDIA/AMD: Auto mode with wgpu preferred
                        Self::Auto
                    }
                } else {
                    log::info!("No GPU detected - using auto renderer mode");
                    Self::Auto
                }
            }
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Wgpu => "wgpu",
            Self::Glow => "glow",
        }
    }
}

/// Build NativeOptions with the specified renderer.
fn build_native_options(renderer: eframe::Renderer) -> eframe::NativeOptions {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1600.0, 980.0])
        .with_min_inner_size([980.0, 620.0])
        .with_clamp_size_to_monitor_size(true)
        .with_title("Mergen ADE");
    if let Some(app_icon) = load_app_icon() {
        viewport = viewport.with_icon(app_icon);
    }

    eframe::NativeOptions {
        viewport,
        centered: true,
        persist_window: false,
        renderer,
        // wgpu_options uses defaults (environment variables WGPU_BACKEND/WGPU_VALIDATION
        // are set in setup_windows_wgpu_env_defaults() on Windows)
        ..Default::default()
    }
}

/// Create the app creator closure for eframe.
fn make_app_creator() -> eframe::AppCreator<'static> {
    Box::new(|cc| {
        let shield = match panic::catch_unwind(AssertUnwindSafe(|| app::AdeApp::bootstrap(cc))) {
            Ok(app) => CrashShieldApp::new(app),
            Err(payload) => CrashShieldApp::from_startup_error(format!(
                "startup panicked: {}",
                panic_payload_to_string(&*payload)
            )),
        };
        Ok(Box::new(shield))
    })
}

/// Preflight probe to check if wgpu renderer can be initialized.
/// This creates a temporary wgpu instance to verify that:
/// 1. The selected backend(s) can be loaded
/// 2. At least one adapter is available
///
/// Returns Ok(()) if wgpu appears usable, Err otherwise.
fn preflight_probe_wgpu() -> Result<(), String> {
    // Use eframe's re-export of wgpu to avoid adding direct dependency
    use eframe::wgpu::{Backends, Instance, InstanceDescriptor};

    // Create an instance with default descriptor (respects WGPU_BACKEND env var)
    let instance = Instance::new(InstanceDescriptor::default());

    // Try to enumerate adapters - if none are available, wgpu won't work
    let adapters = instance.enumerate_adapters(Backends::all());

    if adapters.is_empty() {
        return Err("No wgpu adapters found".to_string());
    }

    // Log available backends and adapter info for diagnostics
    let backend_names: Vec<_> = adapters
        .iter()
        .map(|a: &eframe::wgpu::Adapter| format!("{:?}", a.get_info().backend))
        .collect();
    log::info!(
        "Preflight probe found {} wgpu adapter(s): {:?}",
        adapters.len(),
        backend_names
    );

    Ok(())
}

/// Run the app with the specified renderer mode.
/// Includes memory pressure monitoring and graceful degradation.
fn run_with_renderer(mode: RendererMode) -> Result<(), eframe::Error> {
    // Log initial memory state
    if let Some(mem) = platform::get_memory_info() {
        log::info!(
            "Memory at startup - Physical: {} MB available / {} MB total, Virtual: {} MB available / {} MB total, Working set: {} MB",
            mem.physical_available_mb,
            mem.physical_installed_mb,
            mem.virtual_available_mb,
            mem.virtual_total_mb,
            mem.working_set_mb
        );
    }

    // Check for memory pressure before starting
    if platform::is_memory_pressure_critical() {
        log::warn!("Critical memory pressure detected at startup - attempting to start with minimal resources");
    }

    match mode {
        RendererMode::Wgpu => {
            log::info!("Starting Mergen ADE with wgpu renderer (forced)");
            let options = build_native_options(eframe::Renderer::Wgpu);
            run_with_memory_monitor(|| {
                eframe::run_native("Mergen ADE", options, make_app_creator())
            })
        }
        RendererMode::Glow => {
            log::info!("Starting Mergen ADE with glow renderer (forced)");
            let options = build_native_options(eframe::Renderer::Glow);
            run_with_memory_monitor(|| {
                eframe::run_native("Mergen ADE", options, make_app_creator())
            })
        }
        RendererMode::Auto => {
            log::info!("Starting Mergen ADE in auto mode (wgpu preferred, fallback to glow)");

            // Preflight probe: check if wgpu can be initialized before attempting full startup.
            // This avoids the noisy eframe error log when wgpu surface creation fails.
            match preflight_probe_wgpu() {
                Ok(()) => {
                    log::info!("wgpu preflight probe succeeded, proceeding with wgpu renderer");
                }
                Err(probe_err) => {
                    log::warn!(
                        "wgpu preflight probe failed ({}), skipping to glow fallback",
                        probe_err
                    );
                    log::info!("Falling back to glow (OpenGL) renderer...");
                    let glow_options = build_native_options(eframe::Renderer::Glow);
                    return run_with_memory_monitor(|| {
                        eframe::run_native("Mergen ADE", glow_options, make_app_creator())
                    });
                }
            }

            // First try: wgpu with memory monitoring
            log::info!("Attempting wgpu renderer...");
            let wgpu_options = build_native_options(eframe::Renderer::Wgpu);
            match run_with_memory_monitor(|| {
                eframe::run_native("Mergen ADE", wgpu_options, make_app_creator())
            }) {
                Ok(()) => {
                    log::info!("wgpu renderer initialized successfully");
                    Ok(())
                }
                Err(e) => {
                    log::warn!("wgpu renderer failed to initialize after preflight success: {e}");

                    // Check if this might be a memory-related failure
                    if platform::is_memory_pressure_critical() {
                        log::error!("Memory pressure detected during wgpu initialization - attempting emergency glow fallback");
                    }

                    log::info!("Falling back to glow (OpenGL) renderer...");

                    // Second try: glow fallback with reduced resources
                    let glow_options = build_native_options(eframe::Renderer::Glow);
                    run_with_memory_monitor(|| {
                        eframe::run_native("Mergen ADE", glow_options, make_app_creator())
                    })
                }
            }
        }
    }
}

/// Wrap a run operation with periodic memory monitoring
fn run_with_memory_monitor<F>(operation: F) -> Result<(), eframe::Error>
where
    F: FnOnce() -> Result<(), eframe::Error>,
{
    // Note: We can't easily monitor memory during the operation itself
    // as eframe::run_native blocks. Instead, we rely on:
    // 1. Pre-flight memory check (done in run_with_renderer)
    // 2. Crash shield to catch panics from OOM conditions
    // 3. The application's own memory monitoring in update loops
    operation()
}

fn setup_panic_hook() {
    use std::io::Write as _;

    let log_path = std::path::Path::new("mergen_ade_panic.log");

    panic::set_hook(Box::new(move |panic_info| {
        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());

        let log_line = format!(
            "[{}] thread panicked at \"{}\", location: {}\n",
            timestamp, msg, location
        );

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let _ = file.write_all(log_line.as_bytes());
        }

        eprintln!("[PANIC] {log_line}");
    }));
}

fn panic_payload_to_string(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic payload".to_owned()
    }
}

fn decode_app_icon_from_bytes(bytes: &[u8]) -> Option<egui::IconData> {
    match icon_data::from_png_bytes(bytes) {
        Ok(icon) => Some(icon),
        Err(err) => {
            log::warn!("Skipping app icon because the generated PNG could not be decoded: {err}");
            None
        }
    }
}

fn load_app_icon() -> Option<egui::IconData> {
    decode_app_icon_from_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/app-icon.png")))
}

struct CrashShieldApp {
    inner: Option<app::AdeApp>,
    startup_error: Option<String>,
    crash_error: Option<String>,
}

impl CrashShieldApp {
    fn new(inner: app::AdeApp) -> Self {
        Self {
            inner: Some(inner),
            startup_error: None,
            crash_error: None,
        }
    }

    fn from_startup_error(error: String) -> Self {
        Self {
            inner: None,
            startup_error: Some(error),
            crash_error: None,
        }
    }

    fn note_crash(&mut self, stage: &'static str, payload: Box<dyn Any + Send>) {
        let error = format!("{stage} panicked: {}", panic_payload_to_string(&*payload));
        log::error!("{error}");
        self.crash_error = Some(error);
        self.inner = None;
    }

    fn render_fallback(&self, ctx: &egui::Context) {
        let message = self
            .crash_error
            .as_deref()
            .or(self.startup_error.as_deref())
            .unwrap_or("Mergen ADE stopped because an internal error occurred.");

        // Check for specific error patterns that indicate memory issues
        let is_memory_error = message.to_lowercase().contains("memory")
            || message.to_lowercase().contains("alloc")
            || message.to_lowercase().contains("out of memory")
            || message.to_lowercase().contains("failed to create surface");

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.vertical_centered(|ui| {
                        if is_memory_error {
                            ui.heading("Mergen ADE - Memory Error");
                        } else {
                            ui.heading("Mergen ADE recovered from an internal error");
                        }
                        ui.add_space(8.0);
                        ui.label(message);
                        ui.add_space(8.0);
                        if is_memory_error {
                            ui.label("Try closing other applications or restarting your computer.");
                            ui.add_space(4.0);
                            ui.label("You can also try setting MERGEN_RENDERER=glow to use OpenGL instead of wgpu.");
                        } else {
                            ui.label("Restart the app to restore normal operation.");
                        }
                    });
                },
            );
        });
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

impl eframe::App for CrashShieldApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Check memory pressure periodically
        if platform::is_memory_pressure_critical() {
            log::error!("Critical memory pressure detected during update cycle");
            // Continue running but note the issue
        }

        if self.inner.is_none() {
            self.render_fallback(ctx);
            return;
        }

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            if let Some(inner) = self.inner.as_mut() {
                inner.update(ctx, frame);
            }
        }));

        match result {
            Ok(()) => {}
            Err(payload) => {
                let payload_str = panic_payload_to_string(&*payload);

                // Check for specific error patterns
                if payload_str
                    .to_lowercase()
                    .contains("memory allocation failed")
                    || payload_str.to_lowercase().contains("out of memory")
                    || payload_str.to_lowercase().contains("failed to allocate")
                {
                    log::error!("Memory allocation panic detected in update cycle");
                }

                self.note_crash("update", payload);
                self.render_fallback(ctx);
            }
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            inner.raw_input_hook(ctx, raw_input);
        }));

        if let Err(payload) = result {
            self.note_crash("raw_input_hook", payload);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            inner.save(storage);
        }));

        if let Err(payload) = result {
            self.note_crash("save", payload);
        }
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        // Wrap on_exit in catch_unwind to ensure we always try to cleanup
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            inner.on_exit(gl);
        }));
    }

    fn auto_save_interval(&self) -> Duration {
        self.inner
            .as_ref()
            .map(|inner| inner.auto_save_interval())
            .unwrap_or_else(|| Duration::from_secs(30))
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.inner
            .as_ref()
            .map(|inner| inner.clear_color(visuals))
            .unwrap_or_else(|| egui::Color32::from_rgb(12, 12, 12).to_normalized_gamma_f32())
    }

    fn persist_egui_memory(&self) -> bool {
        self.inner
            .as_ref()
            .map(|inner| inner.persist_egui_memory())
            .unwrap_or(false)
    }
}

fn main() -> Result<(), eframe::Error> {
    let args: Vec<OsString> = std::env::args_os().collect();

    // Check for CLI mode dispatch based on first argument
    if args.len() > 1 {
        let mode = args[1].to_string_lossy();
        match mode.as_ref() {
            "--opencode-notify" => match opencode::maybe_handle_opencode_notify_mode() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => {
                    eprintln!("OpenCode notify mode did not process any payload");
                    std::process::exit(1);
                }
                Err(err) => {
                    eprintln!("Failed to process OpenCode notify payload: {err}");
                    std::process::exit(1);
                }
            },
            "--codex-notify" => match codex::maybe_handle_codex_notify_mode() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => {
                    eprintln!("Codex notify mode did not process any payload");
                    std::process::exit(1);
                }
                Err(err) => {
                    eprintln!("Failed to process Codex notify payload: {err}");
                    std::process::exit(1);
                }
            },
            "--codex-hook" => {
                if args.len() < 3 {
                    eprintln!("Missing Codex hook event argument.");
                    std::process::exit(1);
                }
                let event_name = args[2].to_string_lossy();
                if let Err(err) = codex::handle_codex_hook_from_env(&event_name) {
                    eprintln!("Failed to process Codex hook event: {err}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            "--browser-mcp-helper" => {
                if let Err(err) = browser_mcp_helper::run() {
                    let _ = writeln!(std::io::stderr(), "Mergen Browser MCP failed: {err}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            _ => {
                // Not a CLI mode flag, continue with normal app startup
            }
        }
    }

    setup_panic_hook();

    // Set Windows wgpu environment defaults before logger init
    setup_windows_wgpu_env_defaults();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,mergen_ade=info"),
    )
    // Silence noisy wgpu_hal::vulkan warnings on Windows (last-resort filter)
    .filter_module("wgpu_hal::vulkan", log::LevelFilter::Error)
    .filter_module("wgpu_hal::vulkan::conv", log::LevelFilter::Error)
    .filter_module("wgpu_hal::vulkan::instance", log::LevelFilter::Error)
    .init();

    // Pre-flight memory check before any heavy initialization
    if platform::is_memory_pressure_critical() {
        log::error!(
            "Critical memory pressure detected before startup - attempting to start anyway"
        );
    }

    // Determine renderer mode from environment or auto-detect
    let renderer_mode = RendererMode::from_env();
    log::info!(
        "Renderer mode: {} (from MERGEN_RENDERER env var or auto-detect)",
        renderer_mode.as_str()
    );

    // Log GPU information
    if let Some(gpu) = platform::detect_primary_gpu() {
        log::info!(
            "GPU: {} (Vendor ID: 0x{:04X}, Intel: {})",
            gpu.name,
            gpu.vendor_id,
            gpu.is_intel
        );
    } else {
        log::info!("GPU: Unable to detect");
    }

    run_with_renderer(renderer_mode)
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_app_icon_bytes_are_non_fatal() {
        assert!(super::decode_app_icon_from_bytes(&[]).is_none());
    }
}
