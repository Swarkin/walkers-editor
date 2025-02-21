mod places;
mod windows;
mod editor;
mod providers;
mod osm;

use editor::{changes::EditorOsmData, visual::Visualization, EditorPluginState};
use eframe::egui;
use egui::{Context, Frame, Vec2};
use providers::Provider;
use std::collections::HashMap;
use walkers::{Map, MapMemory, Position, Tiles};
use windows::Windows;

#[cfg(feature = "debug")]
use std::time::Instant;

#[cfg(feature = "debug")]
type DebugTimes = Vec<(&'static str, u32)>;

#[derive(Default)]
pub struct MyApp {
	providers: HashMap<Provider, Box<dyn Tiles + Send>>,
	selected_provider: Provider,
	selected_visualizer: Visualization,
	map_memory: MapMemory,
	editor_osm: EditorOsmData,
	editor_state: EditorPluginState,
	hidden_windows: u8,
	scale_factor: f32,
	prev_size: Vec2,
	prev_zoom: f64,
	prev_pos: Position,
	regenerate_points: bool,
	#[cfg(feature = "debug")]
	debug_times: DebugTimes,
}

impl MyApp {
	pub fn new(egui_ctx: Context) -> Self {
		Self {
			providers: providers::providers(egui_ctx),
			scale_factor: 1.0,
			..Default::default()
		}
	}
}

impl eframe::App for MyApp {
	fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
		egui::TopBottomPanel::top("bar").show(ctx, |ui| {
			use egui::menu;

			menu::bar(ui, |ui| {
				ui.menu_button("Windows", |ui| {
					for window in [Windows::Tags, Windows::Controls, Windows::History, Windows::Download, #[cfg(feature = "debug")] Windows::Debug] {
						let name = window.to_string();
						let bit = window as u8;
						let state = (self.hidden_windows & bit) == 0;
						let mut change = state;

						ui.toggle_value(&mut change, name);
						if state != change {
							self.hidden_windows ^= bit;
						}
					}
				});
				if ui.button("Upload").clicked() {
					// temporary for testing
					// osmchange seems to work now? code is very rough
					let osmchange = editor::changes::osmchange::OsmChange::from(&self.editor_osm.changes);
					println!("{:?}", quick_xml::se::to_string_with_root("osmChange", &osmchange).unwrap());
				}
			});
		});
		egui::CentralPanel::default()
			.frame(Frame::NONE)
			.show(ctx, |ui| {
				#[cfg(feature = "debug")]
				let time_total = Instant::now();
				let tiles = self
					.providers
					.get_mut(&self.selected_provider)
					.unwrap()
					.as_mut();

				self.prev_zoom = self.map_memory.zoom();
				self.prev_pos = self.map_memory.detached().unwrap_or_else(places::school);

				// todo: option to disable displaying tiles
				ui.add(Map::new(Some(tiles), &mut self.map_memory, places::school())
					.with_plugin(editor::EditorPlugin {
						state: &mut self.editor_state,
						osm: &mut self.editor_osm,
						scale_factor: self.scale_factor,
						visualization: self.selected_visualizer,
						regenerate_points: self.regenerate_points,
						#[cfg(feature = "debug")]
						debug_times: &mut self.debug_times,
					}));

				// determine whether regenerating the points cache is necessary
				self.regenerate_points = self.prev_zoom != self.map_memory.zoom() || self.prev_pos != self.map_memory.detached().unwrap_or_else(places::school) || self.prev_size != ctx.screen_rect().size();

				#[cfg(feature = "debug")]
				let time_windows = {
					self.debug_times.push(("ui.add Map", time_total.elapsed().as_micros() as u32));
					Instant::now()
				};

				windows::acknowledge(ui, tiles.attribution());

				if (self.hidden_windows & (Windows::Tags as u8)) == 0 {
					if let Some(id) = self.editor_state.selected.or(self.editor_state.hovered) {
						windows::tags(ui, &self.editor_osm.data.ways.get(&id).unwrap().tags);
					}
				}

				if (self.hidden_windows & (Windows::History as u8)) == 0 {
					windows::history(ui, &self.editor_osm.changes);
				}

				if (self.hidden_windows & (Windows::Controls as u8)) == 0 {
					windows::controls(ui, &mut self.selected_provider, &mut self.providers.keys(), &mut self.selected_visualizer, &mut self.scale_factor);
				}
				
				if (self.hidden_windows & (Windows::Download as u8)) == 0 {
					if let Some(downloaded_data) = windows::download(ui, self.editor_state.map_bbox) {
						osm::append_new_nodes_ways(&mut self.editor_osm.data, downloaded_data);
						self.regenerate_points = true;
					}
				}

				#[cfg(feature = "debug")] {
					self.debug_times.push(("windows", time_windows.elapsed().as_micros() as u32));
					self.debug_times.push(("App::update", time_total.elapsed().as_micros() as u32));
					if (self.hidden_windows & (Windows::Debug as u8)) == 0 {
						windows::debug(ui, &self.debug_times);
					}
				}

				self.prev_size = ctx.screen_rect().size();
			});

		#[cfg(feature = "debug")]
		self.debug_times.clear();
	}
}
