mod places;
mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod config;
mod worker;

use config::TargetServer;
use editor::{consts::*, states::*, visual::FillMode};
use eframe::egui;
use egui::{Button, CentralPanel, Color32, ComboBox, Context, Frame, Grid, Image, Margin, RichText, ScrollArea, TextEdit, TopBottomPanel, Ui, Vec2};
use osm::OsmClient;
use osmchange::{OsmChange, Tag};
use providers::{providers, Provider};
use walkers::{Map, Tiles};
use windows::Window;
use worker::{Request, Response, Worker, WorkerHandle};

#[derive(Default)]
pub struct AppState {
	pub view: View,
	pub target_server_ui: TargetServer,
	pub show_licenses_modal: bool,
}

#[derive(Default, PartialEq)]
pub enum View {
	#[default]
	Edit,
	Upload,
	Auth,
}

pub struct MyApp {
	worker_handle: WorkerHandle,
	state: AppState,
	editor: EditorState,
	uploader: UploaderState,
	authenticator: AuthenticatorState,
}

impl MyApp {
	pub fn new(egui_ctx: &Context) -> Self {
		egui_extras::install_image_loaders(egui_ctx);

		let (request_sender, request_receiver) = crossbeam_channel::unbounded::<Request>();
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
			state: AppState::default(),
			editor: EditorState::new(providers(egui_ctx)),
			uploader: UploaderState::default(),
			authenticator: AuthenticatorState::default(),
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
							self.editor.osm_data.append_new_nodes_ways(data);
							Ok(())
						}
						Err(e) => Err(e),
					};

					self.editor.map_state.download = MapDownloadState::Idle(Some(r));
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
			.frame(Frame {
				fill: Color32::from_gray(if ctx.style().visuals.dark_mode { 32 } else { 243 }),
				inner_margin: Margin::same(4),
				..Default::default()
			})
			.exact_height(TOP_BAR_HEIGHT)
			.show(ctx, |ui| {
				ui.spacing_mut().button_padding = Vec2::splat(2.0);
				ui.spacing_mut().item_spacing = Vec2::splat(4.0);
				ui.horizontal_centered(|ui| {
					egui::Sides::new().show(ui,
						|ui| {
							let btn = title_bar_button("Editor", load_icon(ctx, egui::include_image!("../assets/ui/line.svg"), TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Edit, btn).clicked() {
								self.state.view = View::Edit;
							}

							let btn = title_bar_button("Upload", load_icon(ctx, egui::include_image!("../assets/ui/upload.svg"), TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Upload, btn).clicked() {
								self.state.view = View::Upload;
								// todo: clean up osmchange memory usage after no longer in use
								self.uploader.osmchange = OsmChange::from(&self.editor.osm_data.changes);
								self.uploader.osmchange.prepare_upload(0); // temporary
								// todo: handle Err case
								self.uploader.osmchange_text = self.uploader.osmchange.to_string_pretty().unwrap();
							}

							let btn = title_bar_button("Auth", load_icon(ctx, egui::include_image!("../assets/ui/user.svg"), TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Auth, btn).clicked() {
								self.state.view = View::Auth;
							}
						},
						|ui| {
							ui.menu_image_button(load_icon(ctx, egui::include_image!("../assets/ui/layout.svg"), TOP_BAR_ICON_SIZE), |ui| {
								for window in Window::ITER {
									let mut state = self.editor.window_flags & window as u8 == 0;
									if ui.toggle_value(&mut state, window.to_string()).changed() {
										self.editor.window_flags ^= window as u8;
									}
								}
							});
						}
					);
				});
			});

		match self.state.view {
			View::Edit => {
				CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
					// determine whether regenerating a cache is necessary
					let curr_zoom = self.editor.map_memory.zoom();
					let curr_size = ctx.screen_rect().size();

					if self.editor.prev_zoom != curr_zoom && self.editor.prev_zoom != 0.0 { // avoid running on first frame
						self.editor.osm_data.cache_flags |= CacheFlag::NodeProjection as u8 | CacheFlag::WayMesh as u8;
					}

					let size_diff = (curr_size - self.editor.prev_size) / 2.0;
					if size_diff != Vec2::ZERO {
						self.editor.osm_data.node_offset_resize += size_diff;
						self.editor.osm_data.mesh_offset_resize += size_diff;
					}

					let tiles = self.editor.map_state.selected_provider.map(|x| {
						self.editor.tile_providers.get_mut(&x).unwrap().as_mut()
					});

					// construct plugin
					let editor_plugin = editor::EditorPlugin {
						editor_state: &mut self.editor.plugin_state,
						map_state: &mut self.editor.map_state,
						osm: &mut self.editor.osm_data,
						map_memory: self.editor.map_memory.clone(),
					};

					if let Some(tiles) = tiles {
						map(ui, Some(tiles), &mut self.editor.map_memory, editor_plugin);
						windows::acknowledge(ui, tiles.attribution(), self.editor.map_state.selected_provider == Some(Provider::OpenStreetMap));
					} else {
						map(ui, None, &mut self.editor.map_memory, editor_plugin);
					};

					if self.editor.window_flags & Window::Tags as u8 == 0 {
						if let Some(id) = self.editor.plugin_state.selected.or(self.editor.plugin_state.hovered) {
							let element = self.editor.osm_data.get(&id).expect("id not found");
							windows::tags(ui, element.tags());
						}
					}

					if self.editor.window_flags & Window::History as u8 == 0 {
						windows::history(ui, &self.editor.osm_data.changes);
					}

					if self.editor.window_flags & Window::Map as u8 == 0 {
						let prev_fill_mode = self.editor.map_state.selected_fill_mode;

						let show_licenses = windows::map(ui, &mut self.editor.map_state, &mut self.editor.tile_providers.keys());
						if show_licenses {
							self.state.show_licenses_modal = true;
						}

						if self.editor.map_state.selected_fill_mode == FillMode::Full && prev_fill_mode != FillMode::Full {
							self.editor.osm_data.cache_flags |= CacheFlag::WayMesh as u8;
						}
					}

					if self.editor.window_flags & Window::Toolbar as u8 == 0 && windows::toolbar(ui, &mut self.editor.map_state) {
						self.worker_handle.sender.send(Request::GetMap(Box::new(self.editor.plugin_state.map_bbox.clone()))).unwrap();
						self.editor.map_state.download = MapDownloadState::Downloading;
					}

					#[cfg(feature = "debug")] {
						if (self.editor.window_flags & (Window::Debug as u8)) == 0 {
							let tiles = self.editor.map_state.selected_provider.as_ref()
								.map(|a| self.editor.tile_providers.get(a).unwrap());

							windows::debug(ui, self.editor.map_state.selected_provider.as_ref(), tiles);
						}
					}

					self.editor.prev_zoom = curr_zoom;
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
					if !self.authenticator.token.get(&self.state.target_server_ui).is_some_and(|x| x.is_ok()) {
						ui.strong("Please authenticate to OSM using the Auth tab.");
					} else {
						ui.add_space(10.0);
						if ui.button("Create Changeset").clicked() {
							// todo: figure out why tags do not show up on OSM
							let tags = vec![Tag { k: "created_by".into(), v: crate::USER_AGENT.into() }]; // todo
							self.worker_handle.sender.send(Request::CreateChangeset(tags)).unwrap();
						}

						if let Some(result) = &self.uploader.changeset_creation {
							match result {
								Ok(id) => {
									ui.horizontal(|ui| {
										ui.label("Changeset ID: ");
										ui.hyperlink_to(id.to_string(), format!("https://{}/changeset/{}", self.state.target_server_ui.base_url(), id));
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

					let prev_server = self.state.target_server_ui;
					server_selector(ui, &mut self.state.target_server_ui);
					if prev_server != self.state.target_server_ui {
						// update target server for OsmClient of worker
						self.worker_handle.sender.send(Request::SetTargetServer(self.state.target_server_ui)).unwrap();
					}

					ui.add_space(10.0);

					if self.state.target_server_ui == TargetServer::OpenStreetMap {
						ui.strong(format!("The main OpenStreetMap instance is not available for editing in {} as of now.", env!("CARGO_PKG_NAME")));
					} else {
						ui.label("1. Open this URL and follow the authorization process:");
						ui.hyperlink(osm::client_auth_url(self.state.target_server_ui));

						ui.add_space(10.0);
						ui.label("2. Paste the resulting code into the field below:");
						let widget = TextEdit::singleline(&mut self.authenticator.authorization_code);
						if ui.add_enabled(!self.authenticator.request_pending, widget).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
							self.worker_handle.sender.send(Request::FetchToken(self.authenticator.authorization_code.clone())).unwrap();
							self.authenticator.request_pending = true;
						}

						// todo: ui should change based on the result of the authentication
						// todo: logout button
					}
				});
			}
		}

		if self.state.show_licenses_modal {
			let close_modal = windows::licenses_modal(ctx);
			if close_modal {
				self.state.show_licenses_modal = false;
			}
		}
	}
}

fn map(
	ui: &mut Ui,
	tiles: Option<&mut dyn Tiles>,
	map_memory: &mut walkers::MapMemory,
	editor_plugin: editor::EditorPlugin,
) -> egui::Response {
	ui.add(Map::new(tiles, map_memory, places::school())
		.zoom_with_ctrl(editor_plugin.map_state.zoom_with_ctrl)
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
