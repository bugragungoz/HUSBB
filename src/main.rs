#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! HUSBB - Hantek DSO2D15 USB Bridge.
//!
//! Options (for automation and testing):
//!   --resource <VISA name>     use this scope instead of the first one found
//!   --screenshot <file.png>    save the window after --after seconds (default 8), then quit
//!   --cursors track|voltage    start with cursors shown
//!   --tab measure              start on the Measure tab

mod ui;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let opts = ui::options_from_args();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title(format!("{} - Hantek {} USB Bridge", ui::APP_NAME, ui::MODEL))
        .with_app_id(ui::APP_NAME)
        .with_inner_size([1440.0, 880.0])
        .with_min_inner_size([1180.0, 760.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let native = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(ui::APP_NAME, native, Box::new(move |cc| Ok(Box::new(ui::HusbbApp::new(cc, opts)))))
}
