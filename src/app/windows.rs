use super::editor::{cache::ElementRef, consts::{osm::*, *}, consume_key, visual::{FillMode, Visualization}, EditMode, EditOperation};
use super::icons;
use super::providers::Provider;
use crate::app::editor::cache::ElementId;
use crate::app::osm::{Bbox, OrderedTags, TargetServer};
use crate::app::states::{MapDownloadState, MapState, SelectionFlag, SettingsIOResult};
use eframe::egui;
use eframe::egui::scroll_area::ScrollBarVisibility;
use egui::text::LayoutJob;
use egui::{Align2, Area, AtomExt, Button, Color32, Context, CornerRadius, CursorIcon, Event, FontId, Frame, Hyperlink, Image, ImageSource, InnerResponse, Key, Label, Margin, Modal, Modifiers, Order, Pos2, Rect, Sense, Shadow, Stroke, TextEdit, TextFormat, TextWrapMode, Ui, Vec2, Widget, WidgetText};
use egui_extras::{Column, TableBuilder};
use osm_parser::OsmData;
use walkers::sources::Attribution;
use walkers::Position;

const TRANSPARENT_FRAME_DARK: Frame = Frame {
	inner_margin: Margin::same(6),
	fill: Color32::from_rgba_premultiplied(20, 20, 20, 240),
	stroke: Stroke { width: 1.0, color: Color32::from_gray(60) },
	corner_radius: CornerRadius::same(6),
	outer_margin: Margin::ZERO,
	shadow: Shadow::NONE,
};

const TRANSPARENT_FRAME_LIGHT: Frame = Frame {
	inner_margin: Margin::same(6),
	fill: Color32::from_rgba_premultiplied(240, 240, 240, 240),
	stroke: Stroke { width: 1.0, color: Color32::from_gray(200) },
	corner_radius: CornerRadius::same(6),
	outer_margin: Margin::ZERO,
	shadow: Shadow::NONE,
};

pub type WindowBitflag = u8;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Window {
	Tags = 1 << 0,
	Map = 1 << 1,
	Toolbar = 1 << 2,
	Location = 1 << 3,
	#[cfg(feature = "debug")]
	Debug = 1 << 7,
}

impl std::fmt::Display for Window {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Self::Tags => "Tags",
			Self::Map => "Controls",
			Self::Toolbar => "Toolbar",
			Self::Location => "Location",
			#[cfg(feature = "debug")]
			Self::Debug => "Debug",
		})
	}
}

impl Window {
	#[cfg(not(feature = "debug"))]
	pub const ITER: [Self; 4] = [Self::Tags, Self::Map, Self::Toolbar, Self::Location];
	#[cfg(feature = "debug")]
	pub const ITER: [Self; 5] = [Self::Tags, Self::Map, Self::Toolbar, Self::Location, Self::Debug];
}

fn themed_frame(theme: egui::Theme) -> Frame {
	if theme == egui::Theme::Dark {
		TRANSPARENT_FRAME_DARK
	} else {
		TRANSPARENT_FRAME_LIGHT
	}
}

pub fn attribution(ui: &Ui, attribution: Attribution, simple: bool) {
	egui::Window::new("Acknowledge")
		.title_bar(false)
		.auto_sized()
		.order(Order::Background)
		.anchor(Align2::LEFT_BOTTOM, Vec2::ZERO)
		.frame(themed_frame(ui.ctx().theme())
			.multiply_with_opacity(0.85)
			.inner_margin(Margin { left: 0, right: 6, top: 2, bottom: 2 })
			.corner_radius(CornerRadius { nw: 0, ne: 6, sw: 0, se: 0 })
			.stroke(Stroke::NONE)
		)
		.show(ui.ctx(), |ui| {
			egui::CollapsingHeader::new("Attribution").default_open(true).show(ui, |ui| {
				if simple {
					ui.add(Hyperlink::from_label_and_url("© OpenStreetMap", ATTRIBUTION_URL).open_in_new_tab(true));
				} else {
					ui.horizontal(|ui| {
						let resp = ui.label("Imagery:");
						if let Some(logo) = attribution.logo_light {
							ui.add(Image::new(logo).max_height(resp.rect.height()).max_width(80.0));
						}
						ui.add(Hyperlink::from_label_and_url(attribution.text, attribution.url).open_in_new_tab(true));
					});
					ui.horizontal(|ui| {
						ui.label("Map data:");
						ui.add(Hyperlink::from_label_and_url("© OpenStreetMap", ATTRIBUTION_URL).open_in_new_tab(true));
					});
				}
			});
		});
}

#[derive(Debug)]
pub enum TagsEditKind {
	Key(usize, String),
	Value(usize, String),
	NewKey(String),
	End,
}

pub fn tags(ui: &Ui, editing_tags: &OrderedTags, edit_enabled: bool) -> Option<TagsEditKind> {
	let resp = egui::Window::new("Tags")
		.collapsible(true)
		.default_size([300., 200.])
		.default_pos([WINDOW_MARGIN, WINDOW_MARGIN.mul_add(2., TOP_BAR_HEIGHT) + 42.])
		.vscroll(true)
		.scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden) // workaround for infinite window grow
		.frame(themed_frame(ui.ctx().theme()))
		.show(ui.ctx(), |ui| {
			let result = tag_editor_ui(ui, editing_tags, edit_enabled);
			ui.allocate_space(Vec2::new(0., ui.available_height()));
			result
		}).unwrap();

	resp.inner?
}

pub fn tag_editor_ui(ui: &mut Ui, editing_tags: &OrderedTags, edit_enabled: bool) -> Option<TagsEditKind> {
	let mut change = None;

	TableBuilder::new(ui)
		.striped(true)
		.resizable(true)
		.column(Column::initial(100.0).clip(true))
		.column(Column::remainder().clip(true))
		.header(16., |mut header| {
			header.col(|ui| { ui.strong("Key"); });
			header.col(|ui| { ui.strong("Value"); });
		})
		.body(|body| {
			if edit_enabled {
				body.rows(20., editing_tags.len() + 1, |mut row| {
					let i = row.index();

					if i == editing_tags.len() {
						let mut new_key = String::new();

						row.col(|ui| {
							let resp = ui.add(TextEdit::singleline(&mut new_key).hint_text("+ New Key"));
							if resp.changed() {
								change = Some(TagsEditKind::NewKey(new_key));
							}
						});
					} else {
						let pair = editing_tags.get_index(row.index()).unwrap();
						let (mut new_k, mut new_v) = (pair.0.to_owned(), pair.1.to_owned());

						row.col(|ui| {
							let resp = ui.text_edit_singleline(&mut new_k);
							if resp.changed() {
								change = Some(TagsEditKind::Key(i, new_k));
							} else if resp.lost_focus() {
								change = Some(TagsEditKind::End);
							}
						});
						row.col(|ui| {
							let resp = ui.text_edit_singleline(&mut new_v);
							if resp.changed() {
								change = Some(TagsEditKind::Value(i, new_v));
							} else if resp.lost_focus() {
								change = Some(TagsEditKind::End);
							}
						});
					}
				});
			} else {
				body.rows(20.0, editing_tags.len(), |mut row| {
					let (k, v) = editing_tags.get_index(row.index()).unwrap();
					row.col(|ui| {
						ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
						ui.add_space(2.0);
						ui.label(k);
					});
					row.col(|ui| {
						ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
						ui.add_space(2.0);
						ui.label(v);
					});
				});
			}
		});
	change
}

pub enum MapWindowResult {
	ShowLicenses,
	ShowDataViewer,
}

// Returns whether the licenses button was pressed
pub fn map<'a>(
	ui: &Ui,
	map_state: &mut MapState,
	providers: &mut impl Iterator<Item=&'a Provider>,
) -> Option<MapWindowResult> {
	let mut result = None;

	egui::Window::new("Map")
		.collapsible(false)
		.resizable(false)
		.title_bar(false)
		.fixed_size([150., 150.])
		.anchor(Align2::RIGHT_BOTTOM, [-WINDOW_MARGIN, -WINDOW_MARGIN])
		.frame(themed_frame(ui.ctx().theme()))
		.show(ui.ctx(), |ui| {
			ui.collapsing("Map", |ui| {
				let text = map_state.selected_provider
					.map_or_else(|| "None".into(), |provider| format!("{provider:?}"));

				egui::ComboBox::from_label("Tile Provider")
					.selected_text(text)
					.show_ui(ui, |ui| {
						for p in providers {
							let mut selected = map_state.selected_provider == Some(*p);
							if ui.toggle_value(&mut selected, format!("{p:?}")).changed() {
								map_state.selected_provider = if selected { Some(*p) } else { None }
							}
						}
					});

				egui::ComboBox::from_label("Fill Mode")
					.selected_text(format!("{:?}", map_state.selected_fill_mode))
					.show_ui(ui, |ui| {
						for fill_mode in FillMode::ITER {
							ui.selectable_value(&mut map_state.selected_fill_mode, fill_mode, format!("{fill_mode:?}"));
						}
					});

				egui::ComboBox::from_label("Visualization")
					.selected_text(format!("{:?}", map_state.selected_visualization))
					.show_ui(ui, |ui| {
						for visualization in Visualization::ITER {
							ui.selectable_value(&mut map_state.selected_visualization, visualization, format!("{visualization:?}"));
						}
					});

				ui.add(egui::Slider::new(&mut map_state.scale_factor, 0.1..=2.0).text("Scale factor"));
				ui.checkbox(&mut map_state.zoom_with_ctrl, "Zoom with Ctrl");

				if ui.button("Data Viewer").clicked() {
					result = Some(MapWindowResult::ShowDataViewer);
				}

				if ui.button("Open-source Licenses").clicked() {
					result = Some(MapWindowResult::ShowLicenses);
				}
			});
		});

	result
}

// Returns whether a download was triggered
pub fn toolbar(ui: &mut Ui, state: &mut MapState, editor_mode: &mut EditMode, editor_operation: &mut EditOperation, map_bbox: &Bbox) -> bool {
	let top_left = Pos2::from([WINDOW_MARGIN, TOP_BAR_HEIGHT + WINDOW_MARGIN]);
	let frame = themed_frame(ui.ctx().theme())
		.corner_radius(CornerRadius { ne: 6, nw: 0, se: 6, sw: 0 });
	let rect = Rect::from_two_pos(top_left, top_left + Vec2::new(MODE_INDICATOR_WIDTH, frame.total_margin().top.mul_add(2., 24. + 4.)));

	// Draw mode indicator
	if ui.allocate_rect(rect, Sense::click()).on_hover_text(format!("{editor_mode} mode\nPress Space to toggle")).clicked() {
		*editor_mode = match editor_mode {
			EditMode::View => EditMode::Edit,
			EditMode::Edit => EditMode::View,
		}
	}
	ui.painter().rect_filled(rect, CornerRadius::ZERO, editor_mode.color());

	// Draw toolbar
	egui::Window::new("Toolbar")
		.title_bar(false)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, top_left.to_vec2() + Vec2::new(MODE_INDICATOR_WIDTH, 0.0))
		.frame(frame)
		.show(ui.ctx(), |ui| {
			ui.spacing_mut().button_padding = Vec2::splat(2.0);
			ui.horizontal(|ui| {
				/* primitives buttons */ {
					const ICONS: [ImageSource; 2] = [
						icons::PRIMITIVE_NODE_ICON,
						icons::PRIMITIVE_WAY_ICON,
					];
					const KEYS: [Key; 2] = [Key::Num1, Key::Num2];

					for ((flag, icon), key) in SelectionFlag::ITER.into_iter()
						.zip(ICONS).zip(KEYS)
					{
						let selected = if *editor_mode == EditMode::View {
							state.selection_mode & flag as u8 != 0
						} else { false };

						let resp = ui.add(Button::image(prepare_icon(ui.ctx(), icon, ICON_SIZE)).selected(selected).corner_radius(4));

						if !ui.ctx().wants_keyboard_input()
							&& (resp.clicked() || consume_key(ui.ctx(), key, Modifiers::NONE))
						{
							if *editor_mode == EditMode::View {
								state.selection_mode ^= flag as u8;
							} else {
								#[allow(clippy::single_match)]
								match flag {
									SelectionFlag::Nodes => *editor_operation = EditOperation::AddNode,
									SelectionFlag::Ways => *editor_operation = EditOperation::AddWay(vec![]),
									SelectionFlag::Areas => {}
								}
							}
						}
					}
				}

				ui.separator();

				/* map download */ {
					match &state.download {
						MapDownloadState::Idle(status) => {
							let enabled = map_bbox.area() < MAX_DOWNLOAD_AREA;
							let time = ui.ctx().input(|i| i.time);

							let button_resp = if let Some((status, prev_time)) = status && time - prev_time < DOWNLOAD_FEEDBACK_SECONDS {
								let text = egui::RichText::new(if status.is_ok() { "✔" } else { "✘" }).strong();
								ui.add_enabled(enabled, Button::new(text).min_size(Vec2::splat(TOP_BAR_BUTTON_SIZE.y)).corner_radius(4))
							} else { // todo: global error modal / success toast
								let image = prepare_icon(ui.ctx(), icons::DOWNLOAD, ICON_SIZE);
								ui.add_enabled(enabled, Button::new(image).min_size(Vec2::splat(TOP_BAR_BUTTON_SIZE.y)).corner_radius(4))
							};

							// Return whether a download was triggered
							enabled && !ui.ctx().wants_keyboard_input() && (button_resp.clicked() || ui.input_mut(|i| {
								let any_echo_events = i.events.iter().any(|e| {
									if let Event::Key { repeat, .. } = e { *repeat } else { false }
								});
								!any_echo_events && i.consume_shortcut(shortcuts::DOWNLOAD)
							}))
						}
						MapDownloadState::Downloading => {
							let resp = ui.add_enabled(false, Button::new(()).min_size(Vec2::splat(TOP_BAR_BUTTON_SIZE.y)).corner_radius(4));
							ui.put(resp.rect, egui::Spinner::new());

							false
						}
					}
				}
			}).inner
		}).unwrap().inner.unwrap()
}

pub fn location(ui: &Ui, pos: Position, zoom: f64) -> Option<Position> {
	egui::Window::new("Location")
		.default_pos(Pos2::new(ui.available_width() / 2.0, TOP_BAR_HEIGHT + WINDOW_MARGIN))
		.frame(themed_frame(ui.ctx().theme()))
		.resizable(false)
		.show(ui.ctx(), |ui| {
			ui.style_mut().spacing.item_spacing = Vec2::splat(4.0);

			let base_deg_per_tile = 360.0;
			let deg_per_pixel = base_deg_per_tile / (256.0 * zoom.exp2());
			let cos_lat = pos.0.y.to_radians().cos();
			let vertical_speed = deg_per_pixel * cos_lat;

			let mut edit_pos = pos;

			ui.horizontal(|ui| {
				let dragger = egui::DragValue::new(&mut edit_pos.0.y)
					.fixed_decimals(6)
					.range(-89.9..=89.9)
					.speed(vertical_speed);

				if ui.add(dragger).hovered() {
					ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
				}

				let dragger = egui::DragValue::new(&mut edit_pos.0.x)
					.fixed_decimals(6)
					.range(-179.9..=179.9)
					.speed(deg_per_pixel);
				ui.add(dragger);

				ui.separator();

				if ui.button("Copy").clicked() {
					ui.ctx().copy_text(format!("{:.6}, {:.6}", pos.y(), pos.x()));
				}
			});

			if edit_pos == pos { None } else { Some(edit_pos) }
		})?.inner?
}

#[cfg(feature = "debug")]
pub fn debug(ui: &Ui, selected_provider: Option<&Provider>, provider: Option<&super::providers::TilesKind>, editor_osm_data: &crate::app::editor::cache::EditorOsmData) {
	use crate::app::states::CacheFlag;

	egui::Window::new("Debug")
		.resizable(false)
		.frame(themed_frame(ui.ctx().theme()))
		.show(ui.ctx(), |ui| {
			let (frame_i, frame_times) = &editor_osm_data.frame_timing;

			ui.heading(format!("Δt: {:.4} ms", frame_times[*frame_i] * 1000.0));

			#[allow(clippy::cast_precision_loss)]
			let avg_timing = frame_times.iter().sum::<f32>() / frame_times.len() as f32;
			ui.label(format!("Avg Δt: {:.4} ms", avg_timing * 1000.0));

			if let Some(p) = provider {
				let super::providers::TilesKind::Http(http_tiles) = p;
				let stats = http_tiles.stats();
				ui.label(format!("in-progress requests for {:?}: {}", selected_provider.unwrap(), stats.in_progress));
			}

			ui.collapsing("Elements", |ui| {
				ui.strong("In memory:");
				ui.monospace(format!("Nodes: {:>5}", editor_osm_data.data.nodes.len()));
				ui.monospace(format!("Ways:  {:>5}", editor_osm_data.data.ways.len()));
				ui.strong("In view:");
				ui.monospace(format!("Nodes: {:>5}", editor_osm_data.nodes_in_view.len()));
				ui.monospace(format!("Ways:  {:>5}", editor_osm_data.ways_in_view.len()));
			});

			ui.collapsing("Cache Timings", |ui| {
				ui.label(format!("Refresh View: {} ns", editor_osm_data.view_timing));
				TableBuilder::new(ui)
					.striped(true)
					.columns(Column::auto(), 3)
					.header(18.0, |mut header| {
						header.col(|ui| { ui.label("Cache"); });
						header.col(|ui| { ui.label("Time (ms)"); });
						header.col(|ui| { ui.label("Refresh"); });
					})
					.body(|body| {
						#[allow(clippy::cast_precision_loss)]
						body.rows(18.0, CacheFlag::SIZE, |mut row| {
							let i = row.index();
							let (time, refresh) = editor_osm_data.cache_debug.0[i];
							row.col(|ui| { ui.label(format!("{:?}", CacheFlag::ITER[i])); });
							row.col(|ui| { ui.label(format!("{}", time as f32 / 1000.0)); });
							row.col(|ui| { ui.label(format!("{refresh}")); });
						});
					});
			});
		});
}

pub fn licenses_modal(ctx: &Context) -> bool {
	let screen = ctx.content_rect();
	let area = Area::new("licenses_area".into())
		.anchor(Align2::CENTER_CENTER, Vec2::new(0.0, TOP_BAR_HEIGHT / 2.0));

	Modal::new("licenses".into()).area(area).show(ctx, |ui| {
		ui.heading("Open-Source Licenses");
		ui.separator();
		egui::ScrollArea::vertical().max_height(screen.height() * 0.8).show(ui, |ui| {
			ui.heading("Packages");
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing = Vec2::ZERO;
				ui.add(Hyperlink::from_label_and_url(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_REPOSITORY")).open_in_new_tab(true));
				ui.label(" has been made possible by the following awesome open-source libraries:");
			});
			ui.collapsing("View package tree", |ui| {
				#[cfg(not(debug_assertions))]
				let text = egui::RichText::new(crate::LICENSES_TEXT)
					.text_style(egui::TextStyle::Monospace);

				#[cfg(debug_assertions)]
				let text = "\nLicenses are not loaded in a debug build.\n";

				egui::ScrollArea::vertical().show(ui, |ui| {
					ui.label(text);
				});
				ui.small("Packages marked with (*) have been \"de-duplicated\".\n\
				    The dependencies for the package have already been shown elsewhere in the graph, \
				    and so are not repeated.");
			});
			ui.separator();
			ui.heading("Icons");
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing = Vec2::ZERO;
				ui.label("All icons under ");
				ui.add(Hyperlink::from_label_and_url("/assets/ui", format!("{}{}", env!("CARGO_PKG_REPOSITORY"), "/tree/main/assets/ui")).open_in_new_tab(true));
				ui.label(" are from ");
				ui.add(Hyperlink::from_label_and_url("tabler-icons", "https://github.com/tabler/tabler-icons").open_in_new_tab(true));
				ui.label(", licensed under the MIT license.");
			});
			ui.collapsing("View tabler-icons License", |ui| {
				egui::ScrollArea::vertical().show(ui, |ui| {
					#[cfg(not(debug_assertions))]
					let text = include_str!("../../assets/ui/LICENSE");

					#[cfg(debug_assertions)]
					let text = "\nLicense not loaded in a debug build.\n";

					ui.monospace(text);
				});
			});
		});
		ui.add_space(4.0);
		ui.vertical_centered_justified(|ui| ui.button("Close").clicked()).inner
	}).inner
}

#[cfg(target_family = "wasm")]
pub fn update_modal(ctx: &Context) -> bool {
	Modal::new("update".into()).show(ctx, |ui| {
		ui.heading("Update Available");
		ui.label("Your browser has detected a new version of walkers-editor.");
		ui.separator();
		ui.strong("How to update:");
		ui.label("1. Close all instances of the editor.");
		ui.label("2. Open the latest version in a fresh browser tab.");
		ui.label("The new version should be loaded automatically.");
		ui.separator();
		ui.vertical_centered_justified(|ui| ui.button("Close").clicked()).inner
	}).inner
}

#[cfg(target_family = "wasm")]
pub fn firefox_modal(ctx: &Context) -> bool {
	Modal::new("firefox".into()).show(ctx, |ui| {
		ui.heading("Firefox Warning");
		ui.label("You are using Firefox.");
		ui.separator();
		ui.label("Please use a Chromium-based browser for a faster and less janky experience, or consider downloading the native application directly.");
		ui.separator();
		ui.vertical_centered_justified(|ui| ui.button("Close").clicked()).inner
	}).inner
}

pub struct DataViewerModal {
	selected_element: Option<ElementId>,
	cached_id_list: Vec<ElementId>,
}

impl DataViewerModal {
	pub fn new(osm: &OsmData) -> Self {
		Self {
			selected_element: None,
			cached_id_list: {
				let mut ids = osm.nodes.keys().map(|x| ElementId::Node(*x))
					.chain(osm.ways.keys().map(|x| ElementId::Way(*x)))
					.collect::<Vec<_>>();
				ids.sort_unstable();
				ids
			},
		}
	}

	#[allow(clippy::too_many_lines)]
	pub fn show(&mut self, ctx: &Context, osm: &OsmData) -> bool {
		let screen = ctx.content_rect();
		let width = screen.width() * 0.8;
		let height = screen.height() * 0.6;
		let area = Area::new("data_view_area".into())
			.anchor(Align2::CENTER_CENTER, Vec2::new(0.0, TOP_BAR_HEIGHT / 2.0))
			.default_width(width);

		Modal::new(egui::Id::new("data_view_modal")).area(area).show(ctx, |ui| {
			ui.set_width_range(width..=width);
			ui.set_height_range(height..=height);

			ui.heading("Data View");
			ui.label(format!("Showing {} elements", self.cached_id_list.len()));
			ui.separator();

			let available_height = height - 100.0;

			ui.columns(2, |columns| {
				columns[0].set_width((width * 0.4).max(250.0));
				columns[0].vertical(|ui| {
					TableBuilder::new(ui)
						.striped(true)
						.resizable(false)
						.sense(Sense::click())
						.max_scroll_height(available_height)
						.column(Column::exact(40.0).clip(true))
						.column(Column::exact(120.0).clip(true))
						.header(18.0, |mut header| {
							header.col(|ui| { ui.strong("Type"); });
							header.col(|ui| { ui.strong("ID"); });
						})
						.body(|body| {
							body.rows(18.0, self.cached_id_list.len(), |mut row| {
								let i = row.index();
								let element_id = &self.cached_id_list[i];

								row.col(|ui| {
									Label::new(element_id.type_str())
										.sense(Sense::empty())
										.ui(ui);
								});
								row.col(|ui| {
									Label::new(WidgetText::Text(element_id.id_ref().to_string()).monospace())
										.sense(Sense::empty())
										.ui(ui);
								});

								if row.response().clicked() {
									self.selected_element = Some(element_id.to_owned());
								}
							});
						});
				});

				columns[1].set_width(columns[1].available_width());
				columns[1].vertical(|ui| {
					egui::ScrollArea::vertical()
						.max_height(available_height)
						.show(ui, |ui| {
							if let Some(selected_id) = &self.selected_element {
								let element = match selected_id {
									ElementId::Node(n) => osm.nodes.get(n).map(ElementRef::Node),
									ElementId::Way(w) => osm.ways.get(w).map(ElementRef::Way),
								};

								if let Some(element) = element {
									let id = element.id_ref();

									ui.horizontal(|ui| {
										ui.image(element.element_icon());
										if let Some(name) = element.name() {
											ui.heading(format!("{} {}: {name}", element.type_str(), id));
										} else {
											ui.heading(format!("{} {}", element.type_str(), id));
										}
									});

									ui.add_space(8.0);

									if *id > 0 {
										ui.horizontal(|ui| {
											ui.add(prepare_icon(ctx, icons::COMMIT, ICON_SIZE)).on_hover_text_at_pointer("Version");
											ui.monospace(element.version().to_string());
										});
										ui.horizontal(|ui| {
											ui.add(prepare_icon(ctx, icons::HASHTAG, ICON_SIZE)).on_hover_text_at_pointer("Changeset ID");
											ui.add(Hyperlink::from_label_and_url(
												WidgetText::Text(element.changeset().to_string()).monospace(),
												format!("https://{}/{}", TargetServer::OpenStreetMap.base_changeset_url(), element.changeset())
											).open_in_new_tab(true));
										});
										ui.horizontal(|ui| {
											ui.add(prepare_icon(ctx, icons::USER, ICON_SIZE)).on_hover_text_at_pointer("Username");
											ui.add(Hyperlink::from_label_and_url(
												WidgetText::Text(element.user().to_string()).monospace(),
												format!("https://{}/{}", TargetServer::OpenStreetMap.base_user_url(), element.user())
											).open_in_new_tab(true));
										});
										ui.horizontal(|ui| {
											ui.add(prepare_icon(ctx, icons::CLOCK, ICON_SIZE)).on_hover_text_at_pointer("Timestamp");
											ui.monospace(element.timestamp());
										});
									}

									match element {
										ElementRef::Node(n) => {
											let location_str = format!("Position: {:.6}, {:.6}", n.pos.lat, n.pos.lon);
											ui.label(&location_str);
											if ui.button("Copy Position").clicked() {
												ui.ctx().copy_text(location_str);
											}
										}
										ElementRef::Way(w) => {
											ui.collapsing(format!("{} Nodes", w.nodes.len()), |ui| {
												for node_id in &w.nodes {
													ui.monospace(node_id.to_string());
												}
											});
										}
									}

									ui.add_space(8.0);

									egui::Grid::new("data_view_tags")
										.min_col_width(100.0)
										.striped(true)
										.spacing([8.0, 4.0])
										.show(ui, |ui| {
											for (k, v) in element.tags() {
												ui.label(k);
												ui.label(v);
												ui.end_row();
											}
										});
								}
							} else {
								ui.weak("No element selected.");
							}
						});
				});
			});

			ui.separator();
			ui.button("Close").clicked()
		}).inner
	}
}

pub enum SettingsIOErrorModalResult {
	Quit,
	Retry,
	Continue,
}

pub fn settings_io_error_modal(ctx: &Context, result: &SettingsIOResult, verb: &str, buttons: &[&str]) -> Option<SettingsIOErrorModalResult> {
	Modal::new("settings_io_error".into()).show(ctx, |ui| {
		let max_width = ctx.content_rect().width() * 0.8;
		ui.set_max_width(max_width);

		ui.horizontal(|ui| {
			prepare_icon_with_tint(icons::WARNING, ICON_SIZE, Color32::LIGHT_RED).ui(ui);
			ui.heading(format!("{verb}ing settings failed"));
		});
		ui.label(format!("There was an error {}ing your settings:", verb.to_ascii_lowercase()));

		ui.add_space(4.);
		ui.group(|ui| {
			ui.heading("Config");
			if let Some(e) = &result.0 {
				let text = e.to_string();
				ui.monospace(&text);
				if ui.button("Copy Error").clicked() { ctx.copy_text(text); }
			} else {
				ui.horizontal(|ui| {
					prepare_icon_with_tint(icons::CHECK, ICON_SIZE, Color32::LIGHT_GREEN).ui(ui);
					ui.label(format!("{verb}ed successfully."));
				});
			}
		});

		ui.add_space(4.);
		ui.group(|ui| {
			ui.heading("Theme");
			if let Some(e) = &result.1 {
				let text = e.to_string();
				ui.monospace(&text);
				if ui.button("Copy Error").clicked() { ctx.copy_text(text); }
			} else {
				ui.horizontal(|ui| {
					prepare_icon_with_tint(icons::CHECK, ICON_SIZE, Color32::LIGHT_GREEN).ui(ui);
					ui.label(format!("{verb}ed successfully."));
				});
			}
		});

		ui.add_space(4.);
		ui.horizontal(|ui| {
			if let Some(text) = buttons.first() && Button::new((prepare_icon(ui.ctx(), icons::CROSS, ICON_SIZE), *text)).min_size(WIDE_BUTTON_SIZE).ui(ui).clicked() {
				return Some(SettingsIOErrorModalResult::Quit);
			}
			if let Some(text) = buttons.get(1) && Button::new((prepare_icon(ui.ctx(), icons::RELOAD, ICON_SIZE), *text)).min_size(WIDE_BUTTON_SIZE).ui(ui).clicked() {
				return Some(SettingsIOErrorModalResult::Retry);
			}
			if let Some(text) = buttons.get(2) && Button::new((prepare_icon(ui.ctx(), icons::CHECK, ICON_SIZE), *text)).min_size(WIDE_BUTTON_SIZE).ui(ui).clicked() {
				return Some(SettingsIOErrorModalResult::Continue);
			}
			None
		}).inner
	}).inner
}

pub enum OverlapSelectorResult<'a> {
	None,
	Hovered(ElementRef<'a>),
	Selected(ElementRef<'a>),
}

pub fn overlap_selector<'a>(ui: &Ui, pos: Pos2, hovered: Vec<ElementRef<'a>>) -> InnerResponse<Option<OverlapSelectorResult<'a>>> {
	egui::Window::new("On Top Selector")
		.title_bar(false)
		.auto_sized()
		.frame(themed_frame(ui.ctx().theme()))
		.fixed_pos(pos)
		.show(ui.ctx(), |ui| {
			let mut resp = OverlapSelectorResult::None;

			for element in hovered {
				let icon = element.element_icon()
					.atom_max_height(24.0);

				let name = element.name()
					.map_or_else(|| format!("Unnamed {}\n", element.type_str()), |x| format!("{x}\n"));

				let mut text = LayoutJob::default();
				text.append(&name, 0.0, TextFormat::simple(FontId::proportional(14.0), Color32::LIGHT_GRAY));
				text.append(&element.id_ref().to_string(), 0.0, TextFormat::simple(FontId::proportional(12.0), Color32::GRAY));

				let button_resp = ui.button((icon, text));
				if button_resp.clicked() {
					resp = OverlapSelectorResult::Selected(element);
				} else if button_resp.hovered() {
					resp = OverlapSelectorResult::Hovered(element);
				}
			}

			resp
		}).unwrap()
}
