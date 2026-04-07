// Windows GUI subsystem only for release builds; debug builds get a console for panic output.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod config;
mod hooks;
mod layout;
mod models;
mod terminal;
mod title;

use eframe::egui;
use eframe::icon_data;

fn setup_panic_hook() {
    use std::io::Write as _;
    use std::panic;

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

fn main() -> Result<(), eframe::Error> {
    setup_panic_hook();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let app_icon =
        icon_data::from_png_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/app-icon.png")))
            .expect("generated app icon should decode");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([980.0, 620.0])
            .with_clamp_size_to_monitor_size(true)
            .with_icon(app_icon)
            .with_title("Mergen ADE"),
        centered: true,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Mergen ADE",
        options,
        Box::new(|cc| Ok(Box::new(app::AdeApp::bootstrap(cc)))),
    )
}
