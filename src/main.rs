mod app;

use app::MyApp;

fn main() -> Result<(), eframe::Error> {
    eframe::run_native(
        "MyApp",
        Default::default(),
        Box::new(|cc| Ok(Box::new(MyApp::new(cc.egui_ctx.clone())))),
    )
}
