#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::multiple_crate_versions, clippy::wildcard_imports)]

mod app;

use app::MyApp;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[cfg(not(debug_assertions))]
const LICENSES_TEXT: &str = include_str!(concat!(env!("OUT_DIR"), "/deps.txt"));

#[cfg(not(target_family = "wasm"))]
fn main() -> Result<(), eframe::Error> {
	use eframe::icon_data::from_png_bytes;
	use eframe::egui::ViewportBuilder;

	let options = eframe::NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size([980.0, 720.0])
			.with_min_inner_size([300.0, 200.0])
			.with_clamp_size_to_monitor_size(true)
			.with_icon(from_png_bytes(include_bytes!("../assets/icon/64.png"))
				.expect("failed to load icon")),
		..Default::default()
	};

	eframe::run_native(
		"walkers-editor",
		options,
		Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
	)
}

#[cfg(target_family = "wasm")]
static UPDATE_FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_update_flag(value: bool) {
	UPDATE_FLAG.store(value, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_family = "wasm")]
fn main() {
	use eframe::wasm_bindgen::JsCast as _;

	eframe::WebLogger::init(log::LevelFilter::Info).unwrap();

	wasm_bindgen_futures::spawn_local(async {
		let canvas = web_sys::window()
			.expect("No window")
			.document()
			.expect("No document")
			.get_element_by_id("canvas")
			.expect("Failed to find canvas")
			.dyn_into::<web_sys::HtmlCanvasElement>()
			.expect("Invalid canvas element");

		let start_result = eframe::WebRunner::new()
			.start(
				canvas,
				eframe::WebOptions::default(),
				Box::new(|cc| Ok(Box::new(MyApp::new(cc)))),
			)
			.await;

		match start_result {
			Ok(()) => log::info!("App started successfully"),
			Err(e) => log::error!("App failed to start: {e:?}"),
		}
	});
}
