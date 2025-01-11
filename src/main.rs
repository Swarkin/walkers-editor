mod app;

use app::MyApp;
use eframe::egui::ViewportBuilder;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([1000.0, 800.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "MyApp",
        options,
        Box::new(|cc| Ok(Box::new(MyApp::new(cc.egui_ctx.clone())))),
    )
}
