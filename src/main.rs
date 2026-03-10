#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod config;
mod layout;
mod models;
mod terminal;
mod title;

use eframe::egui;
use eframe::icon_data;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
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
