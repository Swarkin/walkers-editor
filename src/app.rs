mod windows;
mod editor;
mod providers;
mod osm;
mod osmchange;
mod worker;
pub mod icons;

use editor::cache::{Change, ElementId, ElementRef};
use editor::visual::FillMode;
use editor::Editor;
use editor::{consts::*, states::*, EditMode, EditOperation};
use eframe::egui;
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::{Button, CentralPanel, Color32, Context, DragPanButtons, Frame, Image, Key, Margin, Modifiers, PopupCloseBehavior, RichText, ScrollArea, Theme, TopBottomPanel, Ui, Vec2};
use indexmap::IndexMap;
use osm::{OsmClient, TargetServer};
use osmchange::OsmChange;
use providers::providers;
use providers::Provider;
use rustc_hash::FxHashSet;
use walkers::{Map, MapMemory, Position};
use windows::{DataViewerModal, MapWindowResult};
use windows::{TagsEditKind, Window};
use worker::{Request, Response, Worker, WorkerHandle};

/// State related to the application itself
#[derive(Default)]
pub struct AppState {
	pub view: View,
	pub target_server_ui: TargetServer,
	pub open_modals: u8,
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
	editor_state: EditorState,
	uploader_state: UploaderState,
	authenticator_state: AuthenticatorState,
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
							let btn = title_bar_button("Editor", prepare_icon(ctx, icons::PRIMITIVE_WAY_ICON, ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Edit, btn).clicked() {
								self.state.view = View::Edit;
							}

							let btn = title_bar_button("Upload", prepare_icon(ctx, icons::UPLOAD, ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Upload, btn).clicked() {
								self.state.view = View::Upload;
								// todo: clean up osmchange memory usage after no longer in use
								self.uploader_state.osmchange = OsmChange::from(&self.editor_state.editor.osm_data.changes);
								// todo: handle Err case
								self.uploader_state.osmchange_text = self.uploader_state.osmchange.to_string_pretty().unwrap();
							}

							let btn = title_bar_button("Auth", prepare_icon(ctx, icons::USER, ICON_SIZE));
							if ui.add_enabled(self.state.view != View::Auth, btn).clicked() {
								self.state.view = View::Auth;
							}
						},
						|ui| {
							let icon = prepare_icon(ctx, icons::LAYOUT, ICON_SIZE);
							MenuButton::new(icon)
								.config(MenuConfig::default().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
								.ui(ui, |ui| {
									for window in Window::ITER {
										let mut state = self.editor_state.editor.window_flags & window as u8 == 0;
										if ui.toggle_value(&mut state, window.to_string()).changed() {
											self.editor_state.editor.window_flags ^= window as u8;
										}
									}
								});

							let (new_theme, theme_icon) = if ctx.theme() == Theme::Dark { (Theme::Light, icons::MOON) } else { (Theme::Dark, icons::SUN) };
							let btn = title_bar_button("", prepare_icon(ctx, theme_icon, ICON_SIZE));
							if ui.add(btn).clicked() {
								ctx.set_theme(new_theme);
							}
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
				let curr_size = ctx.content_rect().size();

				// todo: dont regenerate cache during zoom animation
				if curr_size != self.editor_state.editor.prev_size {
					self.editor_state.editor.osm_data.refresh_in_view_flag = true;
				}

				if ctx.input_mut(|i| i.consume_shortcut(shortcuts::WIREFRAME)) {
					// todo: avoid refreshing the mesh cache if fill mode isnt partial
					self.editor_state.editor.map_state.selected_fill_mode = match self.editor_state.editor.map_state.selected_fill_mode {
						FillMode::Wireframe => FillMode::Partial,
						FillMode::Partial | FillMode::Full => FillMode::Wireframe,
					}
				}

				let fill = if self.editor_state.editor.map_state.selected_provider == Some(Provider::OpenStreetMap) {
					Color32::from_rgb(242, 239, 233)
				} else { Color32::from_gray(32) };

				CentralPanel::default().frame(Frame::default().fill(fill)).show(ctx, |ui| {
					self.map(ui);

					// todo: textbox mode like in iD
					if self.editor_state.editor.window_flags & Window::Tags as u8 == 0	{
						if let Some(focused_element) = self.editor_state.editor.selected.as_ref().or_else(|| self.editor_state.editor.hovered.first()) {
							let element = self.editor_state.editor.osm_data.get(focused_element.id_ref()).expect("id not found");
							if let Some((editing_id, editing_tags)) = &mut self.editor_state.editor.edit_window {
								if editing_id != focused_element {
									*editing_tags = IndexMap::from_iter(element.tags().to_owned());
									focused_element.clone_into(editing_id);
								}
							} else {
								// todo: avoid allocation when window never focused
								let mut map = IndexMap::from_iter(element.tags().to_owned());
								map.sort_unstable_keys();
								self.editor_state.editor.edit_window = Some((focused_element.to_owned(), map));
							}

							let (_, editing_tags) = self.editor_state.editor.edit_window.as_mut().unwrap();
							let edit_enabled = self.editor_state.editor.mode == EditMode::Edit;

							if let Some(edit_kind) = windows::tags(ui, editing_tags, edit_enabled) {
								match edit_kind {
									TagsEditKind::Key(i, k) => {
										if let Some((_, value)) = editing_tags.get_index(i).map(|(k, v)| (k.clone(), v.clone())) {
											editing_tags.shift_remove_index(i);
											editing_tags.insert_before(i, k, value);
										}
									}
									TagsEditKind::Value(i, v) => {
										*editing_tags.get_index_mut(i).unwrap().1 = v;
									}
									TagsEditKind::NewKey(new_key) => {
										editing_tags.insert(new_key, String::new());
									}
									TagsEditKind::End => {
										let new_tags = editing_tags.clone().into_iter().collect::<osm_parser::Tags>();
										match element {
											ElementRef::Node(node) => {
												let mut new_node = node.clone();
												new_node.tags = new_tags;

												let change = Change::ModifyNode(node.id, new_node);
												self.editor_state.editor.osm_data.apply_change(change);
											}
											ElementRef::Way(way) => {
												let mut new_way = way.clone();
												new_way.tags = new_tags;

												let change = Change::ModifyWay(way.id, new_way);
												self.editor_state.editor.osm_data.apply_change(change);
											}
										}
									}
								}
							}
						} else {
							self.editor_state.editor.edit_window = None;
						}
					}

					if self.editor_state.editor.window_flags & Window::History as u8 == 0 {
						windows::history(ui, &self.editor_state.editor.osm_data.changes);
					}

					if self.editor_state.editor.window_flags & Window::Map as u8 == 0 {
						let prev_fill_mode = self.editor_state.editor.map_state.selected_fill_mode;

						if let Some(result) = windows::map(ui, &mut self.editor_state.editor.map_state, &mut self.editor_state.tile_providers.keys()) {
							match result {
								MapWindowResult::ShowLicenses => self.state.open_modals |= ModalFlag::Licenses as u8,
								MapWindowResult::ShowDataViewer => self.state.open_modals |= ModalFlag::DataViewer as u8,
							}
						}

						if self.editor_state.editor.map_state.selected_fill_mode == FillMode::Full && prev_fill_mode != FillMode::Full {
							self.editor_state.editor.osm_data.cache_flags |= CacheFlag::WayMeshAndAreaSize as u8;
						}
					}

					if self.editor_state.editor.window_flags & Window::Toolbar as u8 == 0 {
						#[allow(clippy::collapsible_if)]
						if windows::toolbar(ui, &mut self.editor_state.editor.map_state, &mut self.editor_state.editor.mode, &mut self.editor_state.editor.operation, &self.editor_state.editor.map_bbox) {
							let request = Request::GetMap(Box::new(self.editor_state.editor.map_bbox.clone()));
							self.worker_handle.send_message(request);

							self.editor_state.editor.map_state.download = MapDownloadState::Downloading;
						}
					}

					if self.editor_state.editor.window_flags & Window::Location as u8 == 0 && let Some(pos) = self.editor_state.map_memory.detached() {
						let pos = windows::location(ui, pos, self.editor_state.map_memory.zoom());
						if let Some(pos) = pos {
							self.editor_state.map_memory.center_at(pos);
						}
					}

					#[cfg(feature = "debug")] {
						if (self.editor_state.editor.window_flags & (Window::Debug as u8)) == 0 {
							let tiles = self.editor_state.editor.map_state.selected_provider.as_ref()
								.map(|a| self.editor_state.tile_providers.get(a).unwrap());

							windows::debug(ui, self.editor_state.editor.map_state.selected_provider.as_ref(), tiles, &self.editor_state.editor.osm_data);
						}
					}

					self.editor_state.editor.prev_size = curr_size;
				});

				if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
					self.editor_state.editor.mode = match self.editor_state.editor.mode {
						EditMode::View => EditMode::Edit,
						EditMode::Edit => {
							self.editor_state.editor.operation = EditOperation::Idle;
							EditMode::View
						},
					};
				}
			}
			View::Upload => {
				CentralPanel::default().show(ctx, |ui| {
					use osmchange::Tag;
					use egui_extras::syntax_highlighting;

					ui.heading("Upload to OpenStreetMap");
					ui.collapsing("View osmChange", |ui| {
						ScrollArea::vertical().show(ui, |ui| {
							syntax_highlighting::code_view_ui(ui, &syntax_highlighting::CodeTheme::from_style(ui.style()), &self.uploader_state.osmchange_text, "xml");
						});
					});

					// todo: simple function to check whether authentication exists
					if self.authenticator_state.token.get(&self.state.target_server_ui).is_some_and(Result::is_ok) {
						ui.add_space(10.0);
						if ui.button("Create Changeset").clicked() {
							// todo: figure out why tags do not show up on OSM
							let tags = vec![Tag { k: "created_by".into(), v: crate::USER_AGENT.into() }];
							self.worker_handle.send_message(Request::CreateChangeset(tags));
						}

						if let Some(result) = &self.uploader_state.changeset_creation {
							match result {
								Ok(id) => {
									ui.horizontal(|ui| {
										ui.label("Changeset: ");
										ui.hyperlink_to(id.to_string(), format!("https://{}/changeset/{}", self.state.target_server_ui.base_url(), id));
									});

									if ui.add_enabled(!self.uploader_state.request_pending, Button::new("Upload")).clicked() {
										self.uploader_state.request_pending = true;
										self.worker_handle.send_message(Request::UploadDiff(*id, self.uploader_state.osmchange_text.clone()));
									}

									if ui.add_enabled(!self.uploader_state.request_pending, Button::new("Close Changeset")).clicked() {
										self.uploader_state.request_pending = true;
										self.worker_handle.send_message(Request::CloseChangeset(*id));
									}
								}
								Err(e) => {
									ui.label(RichText::new(format!("Failed to create changeset:\n{e}")).color(ui.visuals().error_fg_color));
								}
							}
						}

						if let Some(result) = &self.uploader_state.diff_upload {
							match result {
								Ok(resp) => {
									// todo: remove debug info
									ui.collapsing("Upload API response", |ui| { ui.monospace(resp); });
								}
								Err(e) => {
									ui.label(RichText::new(format!("Failed to upload:\n{e}")).color(ui.visuals().error_fg_color));
								}
							}
						}
					} else {
						ui.horizontal(|ui| {
							ui.strong("Please authenticate to OSM using the");
							if ui.small_button("Auth").clicked() { self.state.view = View::Auth; }
							ui.strong("tab.");
						});
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
						let widget = TextEdit::singleline(&mut self.authenticator_state.authorization_code);
						if ui.add_enabled(!self.authenticator_state.request_pending, widget).lost_focus()
							&& ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
							&& !self.authenticator_state.authorization_code.is_empty()
						{
							self.worker_handle.send_message(Request::FetchToken(self.authenticator_state.authorization_code.clone()));
							self.authenticator_state.request_pending = true;
						}

						if self.authenticator_state.request_pending {
							ui.horizontal(|ui| {
								ui.spinner();
								ui.strong("Request in progress...");
							});
						}

						// todo: ui should change based on the result of the authentication
						// todo: logout button
					}
				});
			}
		}
	}

	fn map(&mut self, ui: &mut Ui) -> egui::InnerResponse<()> {
		let tiles = self.editor_state.editor.map_state.selected_provider.map(|x| {
			self.editor_state.tile_providers.get_mut(&x).unwrap().as_mut()
		});

		if let Some(tiles) = &tiles {
			windows::attribution(ui, tiles.attribution(), self.editor_state.editor.map_state.selected_provider == Some(Provider::OpenStreetMap));
		}

		Map::new(tiles, &mut self.editor_state.map_memory, Position::new(10.216_837, 50.059_561))
			.zoom_with_ctrl(self.editor_state.editor.map_state.zoom_with_ctrl)
			.drag_pan_buttons(DragPanButtons::PRIMARY | DragPanButtons::MIDDLE | DragPanButtons::SECONDARY)
			.show(ui, |ui, projector, map_memory| {
				self.editor_state.editor.run(ui, projector, map_memory);
			})
	}
}

impl MyApp {
	pub fn new(cc: &eframe::CreationContext) -> Self {
		#[cfg(not(target_family = "wasm"))]
		use crossbeam_channel as channel;
		#[cfg(target_family = "wasm")]
		use futures::channel::mpsc as channel;

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

		#[cfg(not(target_family = "wasm"))]
		let cache_dir = dirs::cache_dir()
			.map(|x| x.join(env!("CARGO_PKG_NAME")));

		#[cfg(target_family = "wasm")]
		let cache_dir = None;

		let tile_providers = providers(&cc.egui_ctx, cache_dir);

		#[cfg(target_family = "wasm")]
		let app_state = AppState {
			open_modals: if cc.integration_info.web_info.user_agent.to_lowercase().contains("firefox") { ModalFlag::FirefoxNotice as u8 } else { Default::default() },
			..Default::default()
		};

		#[cfg(not(target_family = "wasm"))]
		let app_state = AppState::default();

		Self {
			worker_handle,
			state: app_state,
			editor_state: EditorState {
				editor: Editor::default(),
				map_memory: MapMemory::default(),
				tile_providers,
			},
			uploader_state: UploaderState::default(),
			authenticator_state: AuthenticatorState::default(),
		}
	}

	fn handle_message(&mut self, msg: Response, ctx: &Context) {
		match msg {
			Response::Map(result) => {
				let result = result.map(|mut data| {
					let mut local_changes = FxHashSet::default();
					for change in &self.editor_state.editor.osm_data.changes {
						local_changes.insert(change.element_id());
					}

					data.nodes.retain(|id, _| {
						// todo: handle conflicts
						// if let Some(node) = self.editor.editor.osm_data.data.nodes.get(id) && node.version != n.version {}
						!local_changes.contains(&ElementId::Node(*id))
					});
					data.ways.retain(|id, _| !local_changes.contains(&ElementId::Way(*id)));

					self.editor_state.editor.osm_data.append_new_nodes_ways(data);
					self.editor_state.editor.osm_data.refresh_in_view_flag = true;
				});

				let time = ctx.input(|i| i.time);
				self.editor_state.editor.map_state.download = MapDownloadState::Idle(Some((result, time)));
			},
			Response::Token(token, target_server) => {
				self.authenticator_state.token.insert(target_server, token);
				self.authenticator_state.request_pending = false;
			}
			Response::CreatedChangeset(result) => {
				self.uploader_state.changeset_creation = Some(result);
				self.uploader_state.request_pending = false;
			}
			Response::DiffUploaded(result) => {
				self.uploader_state.diff_upload = Some(result);
				self.uploader_state.request_pending = false;
			}
			Response::ClosedChangeset(result) => {
				self.uploader_state.changeset_closure = Some(result);
				self.uploader_state.request_pending = false;
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
		if self.state.open_modals & ModalFlag::FirefoxNotice as u8 != 0
			&& windows::firefox_modal(ctx)
		{
			self.state.open_modals &= ModalFlag::FirefoxNotice as u8;
		}

		if self.state.open_modals & ModalFlag::Licenses as u8 != 0
			&& windows::licenses_modal(ctx)
		{
			self.state.open_modals &= !(ModalFlag::Licenses as u8);
		}

		if self.state.open_modals & ModalFlag::DataViewer as u8 != 0 {
			if self.editor_state.editor.data_viewer.is_none() {
				self.editor_state.editor.data_viewer = Some(DataViewerModal::new(&self.editor_state.editor.osm_data.data));
			} else {
				let data_viewer = self.editor_state.editor.data_viewer.as_mut().unwrap();
				if data_viewer.show(ctx, &self.editor_state.editor.osm_data.data) {
					self.state.open_modals &= !(ModalFlag::DataViewer as u8);
					self.editor_state.editor.data_viewer = None;
				}
			}
		}

		#[cfg(not(feature = "kiosk"))]
		if ctx.input_mut(|i| i.consume_shortcut(shortcuts::FULLSCREEN)) {
			let state = ctx.input(|i| i.viewport().fullscreen.unwrap_or(true));
			ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!state));
		}
	}
}

fn title_bar_button<'a>(text: &str, img: Image<'a>) -> Button<'a> {
	if text.is_empty() {
		Button::image(img)
			.min_size(Vec2::new(0.0, TOP_BAR_BUTTON_SIZE))
	} else {
		Button::image_and_text(img, RichText::new(format!("{text} ")).strong().size(TOP_BAR_FONT_SIZE))
			.min_size(Vec2::new(0.0, TOP_BAR_BUTTON_SIZE))
	}
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
