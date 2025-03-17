mod places;
mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod config;
mod worker;
mod visual;

use config::TargetServer;
use editor::{changes::EditorOsmData, consts::*, visual::Visualization, EditorPluginState};
use eframe::egui;
use egui::{Button, CentralPanel, Color32, ComboBox, TextEdit, Context, Frame, Grid, Image, Margin, RichText, ScrollArea, TopBottomPanel, Vec2};
use osm::OsmClient;
use osmchange::OsmChange;
use providers::Provider;
use std::collections::HashMap;
#[cfg(feature = "debug")]
use std::time::Instant;
use visual::load_icon;
use walkers::{Map, MapMemory, Position, Tiles};
use windows::Windows;
use worker::{Worker, WorkerHandle};

#[cfg(feature = "debug")]
type DebugTimes = Vec<(&'static str, u32)>;

#[derive(Default, PartialEq)]
enum View {
	#[default]
	Edit,
	Upload,
	Auth,
}

// todo: split the fields into their own structs based on usage
pub struct MyApp {
	worker_handle: WorkerHandle,
	view: View,
	target_server_ui: TargetServer, // todo: use Arc<Mutex<T>> for data shared with worker

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
	changeset_id: Option<std::num::NonZeroU32>,

	// authenticator
	token_text: String,
	auth_request_pending: bool,
}

impl MyApp {
	pub fn new(egui_ctx: Context) -> Self {
		egui_extras::install_image_loaders(&egui_ctx);

		let (request_sender, request_receiver) = crossbeam_channel::unbounded::<worker::Request>();
		let (response_sender, response_receiver) = crossbeam_channel::unbounded::<worker::Response>();

		let mut worker = Worker {
			osm_client: OsmClient::new(TargetServer::default()),
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
			selected_provider: Provider::EsriWorldImagery,
			scale_factor: 1.0,

			view: Default::default(),
			target_server_ui: Default::default(),
			#[cfg(feature = "debug")]
			debug_times: Default::default(),
			selected_visualizer: Default::default(),
			map_memory: Default::default(),
			editor_osm: Default::default(),
			editor_state: Default::default(),
			hidden_windows: Default::default(),
			zoom_with_ctrl: Default::default(),
			prev_size: Default::default(),
			prev_zoom: Default::default(),
			prev_pos: Default::default(),
			regenerate_points: Default::default(),
			map_download_pending: Default::default(),
			osmchange: Default::default(),
			osmchange_text: Default::default(),
			changeset_id: Default::default(),
			token_text: Default::default(),
			auth_request_pending: Default::default(),
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
						Err(err) => println!("{err}"), // todo: error handling
					}
					self.map_download_pending = false;
				},
				// todo: error handling using Result<String>
				worker::Response::Token(_) => {
					self.auth_request_pending = false;
				}
				worker::Response::CreatedChangeset(id) => {
					self.changeset_id = Some(std::num::NonZeroU32::new(id).unwrap());
				}
			}
		});

		TopBottomPanel::top("bar")
			.frame(Frame { fill: if ctx.style().visuals.dark_mode { Color32::from_gray(32) } else { Color32::from_gray(243) }, inner_margin: Margin::same(4), ..Default::default() })
			.exact_height(TOP_BAR_HEIGHT)
			.show(ctx, |ui| {
				ui.spacing_mut().button_padding = Vec2::splat(2.0);
				ui.spacing_mut().item_spacing = Vec2::splat(4.0);
				ui.horizontal_centered(|ui| {
					egui::Sides::new().show(ui,
						|ui| {
							let btn = title_bar_button("Editor", load_icon(ctx, egui::include_image!("../assets/ui/line.svg")));
							if ui.add_enabled(self.view != View::Edit, btn).clicked() {
								self.view = View::Edit;
							}

							let btn = title_bar_button("Upload", load_icon(ctx, egui::include_image!("../assets/ui/upload.svg")));
							if ui.add_enabled(self.view != View::Upload, btn).clicked() {
								self.view = View::Upload;
								// todo: clean up osmchange memory usage after no longer in use
								self.osmchange = OsmChange::from(&self.editor_osm.changes);
								self.osmchange.prepare_upload(0); // temporary
								// todo: handle Err case
								self.osmchange_text = self.osmchange.to_string_pretty().unwrap();
							}

							let btn = title_bar_button("Auth", load_icon(ctx, egui::include_image!("../assets/ui/user.svg")));
							if ui.add_enabled(self.view != View::Auth, btn).clicked() {
								self.view = View::Auth;
							}
						},
						|ui| {
							ui.menu_image_button(load_icon(ctx, egui::include_image!("../assets/ui/layout.svg")), |ui| {
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
						}
					);
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
					
					if ui.button("Create Changeset").clicked() {
						self.worker_handle.sender.send(worker::Request::CreateChangeset).unwrap();
					}

					if let Some(id) = self.changeset_id {
						ui.horizontal(|ui| {
							ui.label("Changeset ID: ");
							ui.hyperlink_to(id.to_string(), format!("https://{}/changeset/{}", self.target_server_ui.base_url(), id));
						});
					}
				});
			}
			View::Auth => {
				CentralPanel::default().show(ctx, |ui| {
					ui.heading("Authenticate to OpenStreetMap");

					let prev_server = self.target_server_ui;
					server_selector(ui, &mut self.target_server_ui);
					if prev_server != self.target_server_ui {
						// update target server for OsmClient of worker
						self.worker_handle.sender.send(worker::Request::SetTargetServer(self.target_server_ui)).unwrap();
					}

					ui.add_space(10.0);
					ui.label("1. Open this URL and follow the authorization process:");
					ui.hyperlink(format!("https://{}", osm::auth_url(self.target_server_ui)));

					ui.add_space(10.0);
					ui.label("2. Paste the resulting code into the field below:");
					let widget = TextEdit::singleline(&mut self.token_text);
					if ui.add_enabled(!self.auth_request_pending, widget).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
						self.worker_handle.sender.send(worker::Request::RequestToken(self.token_text.clone())).unwrap();
						self.auth_request_pending = true;
					}
				});
			}
		}

		#[cfg(feature = "debug")]
		self.debug_times.clear();
	}
}

fn title_bar_button<'a>(text: &str, img: Image<'a>) -> Button<'a> {
	Button::image_and_text(img, RichText::new(format!("{text} ")).strong().size(TOP_BAR_FONT_SIZE))
		.min_size(Vec2::new(0.0, TOP_BAR_BUTTON_SIZE))
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
						ui.hyperlink(format!("https://{}", server.base_url()));
						ui.end_row();
					}
				});
			});
	});
}
