mod export;
mod pipeline;
mod state;
mod ui;
mod worker;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([640.0, 420.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "pixelinator",
        options,
        Box::new(|cc| Ok(Box::new(ui::PixelinatorApp::new(cc)))),
    )
}
