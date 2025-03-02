mod places;
mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod config;
mod worker;

use crate::app::worker::Request;
use config::TargetServer;
use editor::{changes::EditorOsmData, visual::Visualization, EditorPluginState};
use eframe::egui;
use egui::{CentralPanel, Color32, ComboBox, Context, Frame, Grid, Margin, ScrollArea, SelectableLabel, TopBottomPanel, Vec2};
use osm::OsmClient;
use osmchange::OsmChange;
use providers::Provider;
use std::collections::HashMap;
#[cfg(feature = "debug")]
use std::time::Instant;
use walkers::{Map, MapMemory, Position, Tiles};
use windows::Windows;
use worker::{Worker, WorkerHandle};

#[derive(Default, PartialEq)]
enum View {
	#[default]
	Edit,
	Upload,
	Auth,
}

#[cfg(feature = "debug")]
type DebugTimes = Vec<(&'static str, u32)>;

// todo: split the fields into their own structs based on usage
pub struct MyApp {
	worker_handle: WorkerHandle,
	view: View,
	target_server: TargetServer, // todo: get rid of second target_server in worker

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
	map_download_pending: bool,

	// uploader
	osmchange: OsmChange,
	osmchange_text: String,
}

impl MyApp {
	pub fn new(egui_ctx: Context) -> Self {
		egui_extras::install_image_loaders(&egui_ctx);

		// let http_client = reqwest::Client::builder()
		// 	.user_agent(crate::USER_AGENT)
		// 	.https_only(true)
		// 	.redirect(reqwest::redirect::Policy::none())
		// 	.build().expect("reqwest client should build");

		// todo: timeout
		let http_client = ureq::Agent::config_builder()
			.user_agent(crate::USER_AGENT)
			.https_only(true)
			.max_redirects(0)
			.build().into();

		let osm_client = OsmClient {
			http_client,
			target_server: TargetServer::default(),
		};

		let (request_sender, request_receiver) = crossbeam_channel::unbounded::<worker::Request>();
		let (response_sender, response_receiver) = crossbeam_channel::unbounded::<worker::Response>();

		let mut worker = Worker {
			osm_client,
			sender: response_sender,
			receiver: request_receiver,
		};

		let worker_handle = WorkerHandle {
			thread: std::thread::spawn(move || worker.run()),
			sender: request_sender,
			receiver: response_receiver,
		};

		Self {
			worker_handle,
			providers: providers::providers(egui_ctx),

			view: Default::default(),
			target_server: Default::default(),
			#[cfg(feature = "debug")]
			debug_times: Default::default(),
			selected_provider: Default::default(),
			selected_visualizer: Default::default(),
			map_memory: Default::default(),
			editor_osm: Default::default(),
			editor_state: Default::default(),
			hidden_windows: Default::default(),
			scale_factor: Default::default(),
			zoom_with_ctrl: Default::default(),
			prev_size: Default::default(),
			prev_zoom: Default::default(),
			prev_pos: Default::default(),
			regenerate_points: Default::default(),
			map_download_pending: Default::default(),
			osmchange: Default::default(),
			osmchange_text: Default::default(),
		}
	}
}

impl eframe::App for MyApp {
	fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
		self.worker_handle.receiver.try_iter().for_each(|req| {
			match req {
				worker::Response::Map(result) => {
					match result {
						Ok(downloaded_data) => {
							osm::append_new_nodes_ways(&mut self.editor_osm.data, *downloaded_data);
							self.regenerate_points = true;
						}
						Err(err) => {
							println!("{err}");
						}
					}
					self.map_download_pending = false;
				}
			}
		});

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
					// todo(optimization): store and use a simple pan offset to avoid recalculating points on move
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
						if let Some(request) = windows::download(ui, &self.editor_state.map_bbox, self.map_download_pending) {
							self.worker_handle.sender.send(request).unwrap();
							self.map_download_pending = true;
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
					let prev_server = self.target_server;
					server_selector(ui, &mut self.target_server);
					if prev_server != self.target_server {
						self.worker_handle.sender.send(Request::SetTargetServer(self.target_server)).unwrap();
					}
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
