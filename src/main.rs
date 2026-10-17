#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod mem_trim;
mod monitor;
mod settings;

fn main() -> eframe::Result<()> {
    let settings = settings::Settings::load();
    let size = settings.window_size(2);
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([size[0], size[1]])
        .with_min_inner_size([260.0, 120.0])
        .with_transparent(true)
        .with_decorations(true)
        .with_title("LiteMonitor")
        .with_resizable(true);
    if settings.always_on_top {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "LiteMonitor",
        options,
        Box::new(|cc| {
            fonts::setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(app::LiteMonitorApp::new()))
        }),
    )
}
