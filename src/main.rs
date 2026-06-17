mod app;
mod git;
mod graph;
mod theme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Git Agent")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Git Agent",
        options,
        Box::new(|cc| Ok(Box::new(app::GitAgentApp::new(cc)))),
    )
}
