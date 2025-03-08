#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::MyApp;
use eframe::{egui::ViewportBuilder, icon_data::from_png_bytes};
use std::sync::Arc;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

fn main() -> Result<(), eframe::Error> {
	let options = eframe::NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size([980.0, 720.0])
			.with_icon(Arc::from(from_png_bytes(&include_bytes!("../assets/walkers64.png")[..])
				.expect("failed to load icon"))),
		..Default::default()
	};

	eframe::run_native(
		"walkers-editor",
		options,
		Box::new(|cc| Ok(Box::new(MyApp::new(cc.egui_ctx.clone())))),
	)
}
