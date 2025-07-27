mod places;
mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod worker;
pub mod icons;

use editor::{consts::*, states::*, visual::FillMode};
use eframe::egui;
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::{AtomExt, Button, CentralPanel, Color32, Context, Frame, Image, Margin, PopupCloseBehavior, RichText, ThemePreference, TopBottomPanel, Ui, Vec2};
use osm::{OsmClient, TargetServer};
use osmchange::OsmChange;
use providers::{providers, Provider};
use walkers::{Map, Tiles};
use windows::Window;
use worker::{Request, Response, Worker, WorkerHandle};

#[derive(Default)]
pub struct AppState {
	pub view: View,
	pub target_server_ui: TargetServer,
	pub show_licenses_modal: bool,
	#[cfg(target_family = "wasm")]
	pub show_firefox_modal: bool,
}

#[derive(Default, PartialEq, Eq)]
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

// ui components
impl MyApp {
	fn top_bar(&mut self, ctx: &Context) {
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
							let btn = title_bar_button("Editor", prepare_icon(ctx, icons::PRIMITIVE_WAY_ICON, TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Edit, btn).clicked() {
								self.state.view = View::Edit;
							}

							let btn = title_bar_button("Upload", prepare_icon(ctx, icons::UPLOAD, TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Upload, btn).clicked() {
								self.state.view = View::Upload;
								// todo: clean up osmchange memory usage after no longer in use
								self.uploader.osmchange = OsmChange::from(&self.editor.osm_data.changes);
								self.uploader.osmchange.prepare_upload(0); // temporary
								// todo: handle Err case
								self.uploader.osmchange_text = self.uploader.osmchange.to_string_pretty().unwrap();
							}

							let btn = title_bar_button("Auth", prepare_icon(ctx, icons::USER, TOP_BAR_ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Auth, btn).clicked() {
								self.state.view = View::Auth;
							}
						},
						|ui| {
							MenuButton::new(icons::LAYOUT.atom_size(Vec2::splat(TOP_BAR_ICON_SIZE)))
								.config(MenuConfig::default().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
								.ui(ui, |ui| {
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
	}

	#[allow(clippy::too_many_lines)]
	fn content(&mut self, ctx: &Context) {
		match self.state.view {
			View::Edit => {
				// regenerate cache on zoom or resize
				let curr_size = ctx.screen_rect().size();

				// todo: dont regenerate cache during zoom animation
				if curr_size != self.editor.prev_size {
					self.editor.osm_data.refresh_in_view_flag = true;
				}

				if ctx.input_mut(|i| i.consume_shortcut(shortcuts::WIREFRAME)) {
					// todo: avoid refreshing the mesh cache if fill mode isnt partial
					self.editor.map_state.selected_fill_mode = match self.editor.map_state.selected_fill_mode {
						FillMode::Wireframe => FillMode::Partial,
						FillMode::Partial | FillMode::Full => FillMode::Wireframe,
					}
				}

				CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
					let tiles = self.editor.map_state.selected_provider.map(|x| {
						self.editor.tile_providers.get_mut(&x).unwrap().as_mut()
					});

					// construct plugin
					let editor_plugin = editor::EditorPlugin {
						editor_state: &mut self.editor.plugin_state,
						map_state: &mut self.editor.map_state,
						osm: &mut self.editor.osm_data,
						prev_zoom: self.editor.map_memory.zoom(),
					};

					if let Some(tiles) = tiles {
						map(ui, Some(tiles), &mut self.editor.map_memory, editor_plugin);
						windows::acknowledge(ui, tiles.attribution(), self.editor.map_state.selected_provider == Some(Provider::OpenStreetMap));
					} else {
						map(ui, None, &mut self.editor.map_memory, editor_plugin);
					}

					if self.editor.window_flags & Window::Tags as u8 == 0 {
						if let Some(element) = self.editor.plugin_state.selected.as_ref().or_else(|| self.editor.plugin_state.hovered.first()) {
							let element = self.editor.osm_data.get(element.id_ref()).expect("id not found");
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
							self.editor.osm_data.cache_flags |= CacheFlag::WayMeshAndAreaSize as u8;
						}
					}

					if self.editor.window_flags & Window::Toolbar as u8 == 0 && windows::toolbar(ui, &mut self.editor.map_state, &self.editor.plugin_state.map_bbox) {
						let request = Request::GetMap(Box::new(self.editor.plugin_state.map_bbox.clone()));
						self.worker_handle.send_message(request);

						self.editor.map_state.download = MapDownloadState::Downloading;
					}

					#[cfg(feature = "debug")] {
						if (self.editor.window_flags & (Window::Debug as u8)) == 0 {
							let tiles = self.editor.map_state.selected_provider.as_ref()
								.map(|a| self.editor.tile_providers.get(a).unwrap());

							windows::debug(ui, self.editor.map_state.selected_provider.as_ref(), tiles, &self.editor.osm_data);
						}
					}

					self.editor.prev_size = curr_size;
				});
			}
			View::Upload => {
				CentralPanel::default().show(ctx, |ui| {
					use egui::ScrollArea;
					use osmchange::Tag;

					ui.heading("Upload to OpenStreetMap");
					ui.collapsing("View osmChange", |ui| {
						ScrollArea::vertical().show(ui, |ui| {
							egui_extras::syntax_highlighting::code_view_ui(ui, &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()), &self.uploader.osmchange_text, "xml");
						});
					});

					// todo: simple function to check whether authentication exists
					if self.authenticator.token.get(&self.state.target_server_ui).is_some_and(Result::is_ok) {
						ui.add_space(10.0);
						if ui.button("Create Changeset").clicked() {
							// todo: figure out why tags do not show up on OSM
							let tags = vec![Tag { k: "created_by".into(), v: crate::USER_AGENT.into() }]; // todo
							self.worker_handle.send_message(Request::CreateChangeset(tags));
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
					} else {
						ui.strong("Please authenticate to OSM using the Auth tab.");
					}
				});
			}
			View::Auth => {
				CentralPanel::default().show(ctx, |ui| {
					use egui::TextEdit;

					ui.heading("Authenticate to OpenStreetMap");

					let prev_server = self.state.target_server_ui;
					server_selector(ui, &mut self.state.target_server_ui);
					if prev_server != self.state.target_server_ui {
						// update target server for OsmClient of worker
						self.worker_handle.send_message(Request::SetTargetServer(self.state.target_server_ui));
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
							self.worker_handle.send_message(Request::FetchToken(self.authenticator.authorization_code.clone()));
							self.authenticator.request_pending = true;
						}

						// todo: ui should change based on the result of the authentication
						// todo: logout button
					}
				});
			}
		}
	}
}

impl MyApp {
	pub fn new(cc: &eframe::CreationContext) -> Self {
		#[cfg(not(target_family = "wasm"))]
		use crossbeam_channel as channel;
		#[cfg(target_family = "wasm")]
		use futures::channel::mpsc as channel;

		cc.egui_ctx.options_mut(|x| x.theme_preference = ThemePreference::Dark);
		egui_extras::install_image_loaders(&cc.egui_ctx);

		let (request_sender, request_receiver) = channel::unbounded::<Request>();
		let (response_sender, response_receiver) = channel::unbounded::<Response>();

		let mut worker = Worker {
			osm_client: OsmClient::new(TargetServer::default()),
			sender: response_sender,
		};

		#[cfg(not(target_family = "wasm"))]
		let worker_handle = WorkerHandle {
			thread: std::thread::spawn(move || worker.run(request_receiver)),
			sender: request_sender,
			receiver: response_receiver,
		};

		#[cfg(target_family = "wasm")]
		wasm_bindgen_futures::spawn_local(async move {
			worker.run(request_receiver).await;
		});

		#[cfg(target_family = "wasm")]
		let worker_handle = WorkerHandle {
			sender: request_sender,
			receiver: response_receiver,
		};

		#[cfg(target_family = "wasm")]
		let state = AppState {
			show_firefox_modal: cc.integration_info.web_info.user_agent.to_lowercase().contains("firefox"),
			..Default::default()
		};

		#[cfg(not(target_family = "wasm"))]
		let state = AppState::default();

		Self {
			worker_handle, state,
			editor: EditorState::new(providers(&cc.egui_ctx)),
			uploader: UploaderState::default(),
			authenticator: AuthenticatorState::default(),
		}
	}

	fn handle_message(&mut self, msg: Response, ctx: &Context) {
		match msg {
			Response::Map(result) => {
				let result = result.map(|data| {
					self.editor.osm_data.append_new_nodes_ways(data);
					self.editor.osm_data.refresh_in_view_flag = true;
				});

				let time = ctx.input(|i| i.time);
				self.editor.map_state.download = MapDownloadState::Idle(Some((result, time)));
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
	}
}

impl eframe::App for MyApp {
	fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
		for msg in self.worker_handle.recv_messages() {
		    self.handle_message(msg, ctx);
		}

		self.top_bar(ctx);
		self.content(ctx);

		#[cfg(target_family = "wasm")]
		if crate::UPDATE_FLAG.load(std::sync::atomic::Ordering::Relaxed)
			&& windows::update_modal(ctx)
		{
			crate::set_update_flag(false);
		}

		#[cfg(target_family = "wasm")]
		if self.state.show_firefox_modal
			&& windows::firefox_modal(ctx)
		{
			self.state.show_firefox_modal = false;
		}

		if self.state.show_licenses_modal
			&& windows::licenses_modal(ctx)
		{
			self.state.show_licenses_modal = false;
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
	use egui::{ComboBox, Grid};

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
