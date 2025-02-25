mod places;
mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod config;

use config::TargetServer;
use editor::{changes::EditorOsmData, visual::Visualization, EditorPluginState};
use eframe::egui;
use egui::{CentralPanel, Color32, ComboBox, Context, Frame, Grid, Margin, ScrollArea, SelectableLabel, TopBottomPanel, Vec2};
use osmchange::OsmChange;
use providers::Provider;
use std::collections::HashMap;
#[cfg(feature = "debug")]
use std::time::Instant;
use walkers::{Map, MapMemory, Position, Tiles};
use windows::Windows;

#[derive(Default, PartialEq)]
enum View {
	#[default]
	Edit,
	Upload,
	Auth,
}

#[cfg(feature = "debug")]
type DebugTimes = Vec<(&'static str, u32)>;

#[derive(Default)]
pub struct MyApp {
	view: View,
	target_server: TargetServer,

	#[cfg(feature = "debug")]
	debug_times: DebugTimes,

	// editor
	providers: HashMap<Provider, Box<dyn Tiles + Send>>,
	selected_provider: Provider,
	selected_visualizer: Visualization,
	map_memory: MapMemory,
	editor_osm: EditorOsmData,
	editor_state: EditorPluginState,
	hidden_windows: u8,
	scale_factor: f32,
	zoom_with_ctrl: bool,
	prev_size: Vec2,
	prev_zoom: f64,
	prev_pos: Position,
	regenerate_points: bool,

	// uploader
	osmchange: OsmChange,
	osmchange_text: String,
}

impl MyApp {
	pub fn new(egui_ctx: Context) -> Self {
		egui_extras::install_image_loaders(&egui_ctx);
		Self {
			providers: providers::providers(egui_ctx),
			scale_factor: 1.0,
			..Default::default()
		}
	}
}

impl eframe::App for MyApp {
	fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
		TopBottomPanel::top("bar")
			.frame(Frame { fill: Color32::from_gray(32), inner_margin: Margin::same(4), ..Default::default() })
			.exact_height(34.0)
			.show(ctx, |ui| {
				ui.horizontal_centered(|ui| {
					ui.menu_image_button(egui::include_image!("../assets/ui/layout.svg"), |ui| {
						for window in Windows::ITER {
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
					ui.separator();
					if ui.add(SelectableLabel::new(self.view == View::Edit, "Editor")).clicked() {
						self.view = View::Edit;
					}
					if ui.add(SelectableLabel::new(self.view == View::Upload, "Upload")).clicked() {
						self.view = View::Upload;
						// todo: clean up osmchange memory usage after no longer in use
						self.osmchange = OsmChange::from(&self.editor_osm.changes);
						self.osmchange.prepare_upload(0); // temporary
						// todo: handle Err case
						self.osmchange_text = self.osmchange.to_string_pretty().unwrap();
					}
					if ui.add(SelectableLabel::new(self.view == View::Auth, "Auth")).clicked() {
						self.view = View::Auth;
					}
				});
			});
		match self.view {
			View::Edit => {
				CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
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
					let map = Map::new(Some(tiles), &mut self.map_memory, places::school())
						.zoom_with_ctrl(self.zoom_with_ctrl)
						.with_plugin(editor::EditorPlugin {
							state: &mut self.editor_state,
							osm: &mut self.editor_osm,
							scale_factor: self.scale_factor,
							visualization: self.selected_visualizer,
							regenerate_points: self.regenerate_points,
							#[cfg(feature = "debug")]
							debug_times: &mut self.debug_times,
						});

					ui.add(map);

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
						windows::controls(ui, &mut self.selected_provider, &mut self.providers.keys(), &mut self.selected_visualizer, &mut self.scale_factor, &mut self.zoom_with_ctrl);
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
			}
			View::Upload => {
				CentralPanel::default().show(ctx, |ui| {
					ui.heading("Upload to OpenStreetMap");
					ui.collapsing("View osmChange", |ui| {
						ScrollArea::vertical().show(ui, |ui| {
							egui_extras::syntax_highlighting::code_view_ui(ui, &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()), &self.osmchange_text, "xml");
						});
					});
				});
			}
			View::Auth => {
				CentralPanel::default().show(ctx, |ui| {
					ui.heading("Authenticate to OpenStreetMap");
					server_selector(ui, &mut self.target_server);
				});
			}
		}

		#[cfg(feature = "debug")]
		self.debug_times.clear();
	}
}

fn server_selector(ui: &mut egui::Ui, value: &mut TargetServer) {
	ui.horizontal(|ui| {
		ui.label("Server");
		ComboBox::from_id_salt(ui.id())
			.selected_text(value.description())
			.show_ui(ui, |ui| {
				Grid::new(ui.id()).num_columns(TargetServer::ITER.len()).show(ui, |ui| {
					for server in TargetServer::ITER {
						ui.selectable_value(value, server, server.description());
						ui.hyperlink(format!("https://{}", server.url()));
						ui.end_row();
					}
				});
			});
	});
}
