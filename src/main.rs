// Windows GUI subsystem only for release builds; debug builds get a console for panic output.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod codex;
mod config;
mod hooks;
mod layout;
mod models;
mod opencode;
mod opencode_hook_service;
mod terminal;
mod title;

use eframe::egui;
use eframe::icon_data;
use std::any::Any;
use std::ffi::OsString;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::time::Duration;

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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Mergen ADE recovered from an internal error");
                        ui.add_space(8.0);
                        ui.label(message);
                        ui.add_space(8.0);
                        ui.label("Restart the app to restore normal operation.");
                    });
                },
            );
        });
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

impl eframe::App for CrashShieldApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.inner.is_none() {
            self.render_fallback(ctx);
            return;
        }

        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            if let Some(inner) = self.inner.as_mut() {
                inner.update(ctx, frame);
            }
        }));

        if let Err(payload) = result {
            self.note_crash("update", payload);
            self.render_fallback(ctx);
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
            inner.raw_input_hook(ctx, raw_input);
        })) {
            self.note_crash("raw_input_hook", payload);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
            inner.save(storage);
        })) {
            self.note_crash("save", payload);
        }
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

fn maybe_handle_codex_notify_mode_with<I, F>(mut args: I, mut handler: F) -> Result<bool, String>
where
    I: Iterator<Item = OsString>,
    F: FnMut(&str) -> io::Result<()>,
{
    let Some(mode) = args.next() else {
        return Ok(false);
    };
    if mode != "--codex-notify" {
        return Ok(false);
    }

    let payload = args
        .next()
        .ok_or_else(|| "Missing Codex notify payload argument.".to_owned())?
        .into_string()
        .map_err(|_| "Codex notify payload must be valid UTF-8.".to_owned())?;

    handler(&payload).map_err(|err| err.to_string())?;
    Ok(true)
}

fn maybe_handle_codex_notify_mode<I>(args: I) -> Result<bool, String>
where
    I: Iterator<Item = OsString>,
{
    maybe_handle_codex_notify_mode_with(args, codex::handle_codex_notify_from_env)
}

fn maybe_handle_opencode_notify_mode<I>(mut args: I) -> Result<bool, String>
where
    I: Iterator<Item = OsString>,
{
    let Some(mode) = args.next() else {
        return Ok(false);
    };
    if mode != "--opencode-notify" {
        return Ok(false);
    }

    match opencode::maybe_handle_opencode_notify_mode() {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => Err(err.to_string()),
    }
}

fn main() -> Result<(), eframe::Error> {
    let mut args = std::env::args_os();
    let _ = args.next();

    // Try OpenCode notify mode first
    match maybe_handle_opencode_notify_mode(&mut args) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(err) => {
            eprintln!("Failed to process OpenCode notify payload: {err}");
            std::process::exit(1);
        }
    }

    // Then try Codex notify mode
    match maybe_handle_codex_notify_mode(&mut args) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(err) => {
            eprintln!("Failed to process Codex notify payload: {err}");
            std::process::exit(1);
        }
    }

    setup_panic_hook();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1600.0, 980.0])
        .with_min_inner_size([980.0, 620.0])
        .with_clamp_size_to_monitor_size(true)
        .with_title("Mergen ADE");
    if let Some(app_icon) = load_app_icon() {
        viewport = viewport.with_icon(app_icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Mergen ADE",
        options,
        Box::new(|cc| {
            let shield = match panic::catch_unwind(AssertUnwindSafe(|| app::AdeApp::bootstrap(cc)))
            {
                Ok(app) => CrashShieldApp::new(app),
                Err(payload) => CrashShieldApp::from_startup_error(format!(
                    "startup panicked: {}",
                    panic_payload_to_string(&*payload)
                )),
            };
            Ok(Box::new(shield))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::maybe_handle_codex_notify_mode_with;
    use crate::codex::{
        codex_notify_inbox_path_for_dir, CodexNotifyInboxEvent, MERGEN_AI_TOOL_HINT_CODEX,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn codex_notify_mode_forwards_agent_turn_complete_payload_from_argv() {
        let temp = TestTempDir::new("codex-notify-argv-turn-complete");
        let handled = maybe_handle_codex_notify_mode_with(
            vec![
                OsString::from("--codex-notify"),
                OsString::from(r#"{"type":"agent-turn-complete"}"#),
            ]
            .into_iter(),
            |payload| {
                write_test_notify_event(payload, "41", test_codex_inbox_token(41), &temp.path)
            },
        )
        .expect("argv payload should be handled");

        assert!(handled);

        let payload = fs::read_to_string(codex_notify_inbox_path_for_dir(
            &temp.path,
            41,
            test_codex_inbox_token(41),
        ))
        .expect("should read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(payload.trim()).expect("should parse inbox event");

        assert_eq!(event.status, "attention");
        assert_eq!(event.event_kind.as_deref(), Some("agent-turn-complete"));
        assert_eq!(event.raw_json, r#"{"type":"agent-turn-complete"}"#);
    }

    #[test]
    fn codex_notify_mode_forwards_approval_requested_payload_from_argv() {
        let temp = TestTempDir::new("codex-notify-argv-approval");
        let handled = maybe_handle_codex_notify_mode_with(
            vec![
                OsString::from("--codex-notify"),
                OsString::from(r#"{"event":"approval-requested"}"#),
            ]
            .into_iter(),
            |payload| {
                write_test_notify_event(payload, "42", test_codex_inbox_token(42), &temp.path)
            },
        )
        .expect("argv payload should be handled");

        assert!(handled);

        let payload = fs::read_to_string(codex_notify_inbox_path_for_dir(
            &temp.path,
            42,
            test_codex_inbox_token(42),
        ))
        .expect("should read inbox");
        let event: CodexNotifyInboxEvent =
            serde_json::from_str(payload.trim()).expect("should parse inbox event");

        assert_eq!(event.status, "attention");
        assert_eq!(event.event_kind.as_deref(), Some("approval-requested"));
        assert_eq!(event.raw_json, r#"{"event":"approval-requested"}"#);
    }

    #[test]
    fn codex_notify_mode_requires_payload_argument() {
        let err = maybe_handle_codex_notify_mode_with(
            vec![OsString::from("--codex-notify")].into_iter(),
            |_| panic!("handler should not be called without a payload"),
        )
        .expect_err("missing payload should fail");

        assert_eq!(err, "Missing Codex notify payload argument.");
    }

    #[test]
    fn invalid_app_icon_bytes_are_non_fatal() {
        assert!(super::decode_app_icon_from_bytes(&[]).is_none());
    }

    fn write_test_notify_event(
        payload: &str,
        terminal_id: &str,
        inbox_token: &str,
        inbox_dir: &std::path::Path,
    ) -> std::io::Result<()> {
        crate::codex::write_codex_notify_event(
            payload,
            terminal_id,
            inbox_dir,
            inbox_token,
            Some(MERGEN_AI_TOOL_HINT_CODEX),
        )
    }

    fn test_codex_inbox_token(terminal_id: u64) -> &'static str {
        match terminal_id {
            41 => "test-codex-inbox-token-41",
            42 => "test-codex-inbox-token-42",
            _ => "test-codex-inbox-token",
        }
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(label: &str) -> Self {
            let unique_suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mergen-ade-{label}-{}-{unique_suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
