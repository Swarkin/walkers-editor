#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::MyApp;
use eframe::{egui::ViewportBuilder, icon_data::from_png_bytes};

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[cfg(not(debug_assertions))]
const LICENSES_TEXT: &str = include_str!(concat!(env!("OUT_DIR"), "/deps.txt"));

fn main() -> Result<(), eframe::Error> {
	let options = eframe::NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size([980.0, 720.0])
			.with_min_inner_size([300.0, 200.0])
			.with_clamp_size_to_monitor_size(true)
			.with_icon(from_png_bytes(include_bytes!("../assets/walkers64.png"))
				.expect("failed to load icon")),
		..Default::default()
	};

	eframe::run_native(
		"walkers-editor",
		options,
		Box::new(|cc| Ok(Box::new(MyApp::new(&cc.egui_ctx)))),
	)
}
