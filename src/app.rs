mod places;
mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod config;
mod worker;
mod visual;

use crate::app::osmchange::Tag;
use crate::app::visual::TOP_BAR_ICON_SIZE;
use config::TargetServer;
use editor::{consts::*, states::*};
use eframe::egui;
use egui::{Button, CentralPanel, Color32, ComboBox, Context, Frame, Grid, Image, Margin, RichText, ScrollArea, TextEdit, TopBottomPanel, Ui, Vec2};
use osm::OsmClient;
use osmchange::OsmChange;
#[cfg(feature = "debug")]
use std::time::Instant;
use visual::load_icon;
use walkers::{Map, Tiles};
use windows::Window;
use worker::{Response, Worker, WorkerHandle};

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
	target_server_ui: TargetServer, // todo: use Arc<RwLock<T>> for data shared with worker

	#[cfg(feature = "debug")]
	debug_times: DebugTimes,

	editor: EditorState,
	uploader: UploaderState,
	authenticator: AuthenticatorState,
}

impl MyApp {
	pub fn new(egui_ctx: &Context) -> Self {
		egui_extras::install_image_loaders(egui_ctx);

		let (request_sender, request_receiver) = crossbeam_channel::unbounded::<worker::Request>();
		let (response_sender, response_receiver) = crossbeam_channel::unbounded::<Response>();

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
			editor: EditorState::new(egui_ctx),
			uploader: UploaderState::default(),
			authenticator: AuthenticatorState::default(),

			view: Default::default(),
			target_server_ui: Default::default(),
			#[cfg(feature = "debug")]
			debug_times: Default::default(),
		}
	}
}

impl eframe::App for MyApp {
	fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
		self.worker_handle.receiver.try_iter().for_each(|req| {
			match req {
				Response::Map(result) => {
					let r = match result {
						Ok(data) => {
							osm::append_new_nodes_ways(&mut self.editor.editor_osm.data, data);
							self.editor.regenerate_points = true;
							self.editor.regenerate_orphan = true;
							Ok(())
						}
						Err(e) => Err(e),
					};

					self.editor.map_download = MapDownloadState::Idle(Some(r));
				},
				Response::Token(token, target_server) => {
					self.authenticator.token.insert(target_server, token);
					self.authenticator.request_pending = false;
				}
				Response::CreatedChangeset(result) => {
					self.uploader.changeset_creation = Some(result);
				}
				Response::ClosedChangeset(_result) => {
					todo!();
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
							let btn = title_bar_button("Editor", load_icon(ctx, egui::include_image!("../assets/ui/line.svg"), TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.view != View::Edit, btn).clicked() {
								self.view = View::Edit;
							}

							let btn = title_bar_button("Upload", load_icon(ctx, egui::include_image!("../assets/ui/upload.svg"), TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.view != View::Upload, btn).clicked() {
								self.view = View::Upload;
								// todo: clean up osmchange memory usage after no longer in use
								self.uploader.osmchange = OsmChange::from(&self.editor.editor_osm.changes);
								self.uploader.osmchange.prepare_upload(0); // temporary
								// todo: handle Err case
								self.uploader.osmchange_text = self.uploader.osmchange.to_string_pretty().unwrap();
							}

							let btn = title_bar_button("Auth", load_icon(ctx, egui::include_image!("../assets/ui/user.svg"), TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.view != View::Auth, btn).clicked() {
								self.view = View::Auth;
							}
						},
						|ui| {
							ui.menu_image_button(load_icon(ctx, egui::include_image!("../assets/ui/layout.svg"), TOP_BAR_ICON_SIZE), |ui| {
								for window in Window::ITER {
									let mut state = self.editor.hidden_windows & window as u8 == 0;
									if ui.toggle_value(&mut state, window.to_string()).changed() {
										self.editor.hidden_windows ^= window as u8;
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

					self.editor.prev_zoom = self.editor.map_memory.zoom();
					self.editor.prev_pos = self.editor.map_memory.detached().unwrap_or_else(places::school);

					let editor_plugin = editor::EditorPlugin {
						state: &mut self.editor.editor_state,
						osm: &mut self.editor.editor_osm,
						scale_factor: self.editor.scale_factor,
						current_zoom: self.editor.map_memory.zoom(),
						visualization: self.editor.selected_visualizer,
						selection_mode: self.editor.selection_mode,
						fill_mode: self.editor.selected_fill_mode,
						regenerate_points: self.editor.regenerate_points,
						regenerate_orphan: self.editor.regenerate_orphan,
						#[cfg(feature = "debug")]
						debug_times: &mut self.debug_times,
					};

					if let Some(selected_provider) = &self.editor.selected_provider {
						let tiles = self.editor.providers.get_mut(selected_provider).unwrap().as_mut();
						map(ui, Some(tiles), &mut self.editor.map_memory, self.editor.zoom_with_ctrl, editor_plugin);
						windows::acknowledge(ui, tiles.attribution());
					} else {
						map(ui, None, &mut self.editor.map_memory, self.editor.zoom_with_ctrl, editor_plugin);
					};

					// determine whether regenerating the points cache is necessary
					// todo(optimization): store and use a simple pan offset to avoid recalculating points on move
					self.editor.regenerate_points = self.editor.prev_zoom != self.editor.map_memory.zoom()
						|| self.editor.prev_pos != self.editor.map_memory.detached().unwrap_or_else(places::school)
						|| self.editor.prev_size != ctx.screen_rect().size();

					if self.editor.regenerate_orphan {
						self.editor.regenerate_orphan = false;
					}

					#[cfg(feature = "debug")]
					let time_windows = {
						self.debug_times.push(("ui.add Map", time_total.elapsed().as_micros() as u32));
						Instant::now()
					};

					if (self.editor.hidden_windows & (Window::Tags as u8)) == 0 {
						if let Some(id) = self.editor.editor_state.selected.or(self.editor.editor_state.hovered) {
							let element = self.editor.editor_osm.get_by_id(&id).expect("id not found");
							windows::tags(ui, element.tags());
						}
					}

					if (self.editor.hidden_windows & (Window::History as u8)) == 0 {
						windows::history(ui, &self.editor.editor_osm.changes);
					}

					if (self.editor.hidden_windows & (Window::Map as u8)) == 0 {
						windows::map(ui, &mut self.editor.selected_provider, &mut self.editor.providers.keys(), &mut self.editor.selected_fill_mode, &mut self.editor.selected_visualizer, &mut self.editor.selection_mode, &mut self.editor.scale_factor, &mut self.editor.zoom_with_ctrl);
					}

					if (self.editor.hidden_windows & (Window::Download as u8)) == 0 {
						if let Some(request) = windows::download(ui, &self.editor.editor_state.map_bbox, &self.editor.map_download) {
							self.worker_handle.sender.send(request).unwrap();
							self.editor.map_download = MapDownloadState::Downloading;
						}
					}

					if (self.editor.hidden_windows & (Window::Toolbar as u8)) == 0 {
						windows::toolbar(ui, &mut self.editor.selection_mode);
					}

					#[cfg(feature = "debug")] {
						self.debug_times.push(("windows", time_windows.elapsed().as_micros() as u32));
						self.debug_times.push(("App::update", time_total.elapsed().as_micros() as u32));
						if (self.editor.hidden_windows & (Window::Debug as u8)) == 0 {
							let tiles = self.editor.selected_provider.as_ref()
								.map(|a| self.editor.providers.get(a).unwrap());

							windows::debug(ui, &self.debug_times, self.editor.selected_provider.as_ref(), tiles);
						}
					}

					self.editor.prev_size = ctx.screen_rect().size();
				});
			}
			View::Upload => {
				CentralPanel::default().show(ctx, |ui| {
					ui.heading("Upload to OpenStreetMap");
					ui.collapsing("View osmChange", |ui| {
						ScrollArea::vertical().show(ui, |ui| {
							egui_extras::syntax_highlighting::code_view_ui(ui, &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()), &self.uploader.osmchange_text, "xml");
						});
					});

					// todo: simple function to check whether authentication exists
					if !self.authenticator.token.get(&self.target_server_ui).is_some_and(|x| x.is_ok()) {
						ui.strong("Please authenticate to OSM using the Auth tab.");
					} else {
						ui.add_space(10.0);
						if ui.button("Create Changeset").clicked() {
							// todo: figure out why tags do not show up on OSM
							let tags = vec![Tag { k: "created_by".into(), v: crate::USER_AGENT.into() }]; // todo
							self.worker_handle.sender.send(worker::Request::CreateChangeset(tags)).unwrap();
						}

						if let Some(result) = &self.uploader.changeset_creation {
							match result {
								Ok(id) => {
									ui.horizontal(|ui| {
										ui.label("Changeset ID: ");
										ui.hyperlink_to(id.to_string(), format!("https://{}/changeset/{}", self.target_server_ui.base_url(), id));
									});
								}
								Err(err) => {
									ui.label(RichText::new(format!("Failed to create changeset:\n{err}")).color(ui.visuals().error_fg_color));
								}
							}
						}
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

					if self.target_server_ui == TargetServer::OpenStreetMap {
						ui.strong("The main OpenStreetMap instance is not available for editing in walkers as of now.");
					} else {
						ui.label("1. Open this URL and follow the authorization process:");
						ui.hyperlink(osm::client_auth_url(self.target_server_ui));

						ui.add_space(10.0);
						ui.label("2. Paste the resulting code into the field below:");
						let widget = TextEdit::singleline(&mut self.authenticator.authorization_code);
						if ui.add_enabled(!self.authenticator.request_pending, widget).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
							self.worker_handle.sender.send(worker::Request::FetchToken(self.authenticator.authorization_code.clone())).unwrap();
							self.authenticator.request_pending = true;
						}

						// todo: ui should change based on the result of the authentication
						// todo: logout button
					}
				});
			}
		}

		#[cfg(feature = "debug")]
		self.debug_times.clear();
	}
}

fn map(
	ui: &mut Ui,
	tiles: Option<&mut dyn Tiles>,
	map_memory: &mut walkers::MapMemory,
	zoom_with_ctrl: bool,
	editor_plugin: editor::EditorPlugin,
) -> egui::Response {
	ui.add(Map::new(tiles, map_memory, places::school())
		.zoom_with_ctrl(zoom_with_ctrl)
		.with_plugin(editor_plugin)
	)
}

fn title_bar_button<'a>(text: &str, img: Image<'a>) -> Button<'a> {
	Button::image_and_text(img, RichText::new(format!("{text} ")).strong().size(TOP_BAR_FONT_SIZE))
		.min_size(Vec2::new(0.0, TOP_BAR_BUTTON_SIZE))
}

fn server_selector(ui: &mut Ui, value: &mut TargetServer) {
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
