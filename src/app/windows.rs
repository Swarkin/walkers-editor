use super::editor::{cache::{Change, ElementRef}, consts::{osm::*, *}, states::{MapDownloadState, MapState, SelectionFlag}, visual::{FillMode, Visualization}, EditMode, EditOperation, EditorPluginState};
use super::icons;
use super::providers::Provider;
use eframe::egui;
use eframe::egui::{TextEdit, TextWrapMode};
use egui::text::LayoutJob;
use egui::{Align2, Area, AtomExt, Button, Color32, CornerRadius, CursorIcon, Event, FontId, Frame, Image, ImageSource, InnerResponse, Key, Margin, Modifiers, Order, Pos2, Rect, Sense, Shadow, Stroke, TextFormat, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use walkers::sources::Attribution;
use walkers::Position;

const TRANSPARENT_FRAME: Frame = Frame {
	inner_margin: Margin::same(6),
	fill: Color32::from_rgba_premultiplied(20, 20, 20, 240),
	stroke: Stroke { width: 1.0, color: Color32::from_gray(60) },
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
	History = 1 << 2,
	Toolbar = 1 << 3,
	Location = 1 << 4,
	#[cfg(feature = "debug")]
	Debug = 1 << 7,
}

impl std::fmt::Display for Window {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Self::Tags => "Tags",
			Self::Map => "Controls",
			Self::History => "History",
			Self::Toolbar => "Toolbar",
			Self::Location => "Location",
			#[cfg(feature = "debug")]
			Self::Debug => "Debug",
		})
	}
}

impl Window {
	#[cfg(not(feature = "debug"))]
	pub const ITER: [Self; 5] = [Self::Tags, Self::Map, Self::History, Self::Toolbar, Self::Location];
	#[cfg(feature = "debug")]
	pub const ITER: [Self; 6] = [Self::Tags, Self::Map, Self::History, Self::Toolbar, Self::Location, Self::Debug];
}

pub fn acknowledge(ui: &Ui, attribution: Attribution, simple: bool) {
	egui::Window::new("Acknowledge")
		.title_bar(false)
		.auto_sized()
		.order(Order::Background)
		.anchor(Align2::LEFT_BOTTOM, Vec2::ZERO)
		.frame(TRANSPARENT_FRAME
			.multiply_with_opacity(0.85)
			.inner_margin(Margin { left: 0, right: 6, top: 2, bottom: 2 })
			.corner_radius(CornerRadius { nw: 0, ne: 6, sw: 0, se: 0 })
			.stroke(Stroke::NONE)
		)
		.show(ui.ctx(), |ui| {
			egui::CollapsingHeader::new("Attribution").default_open(true).show(ui, |ui| {
				if simple {
					ui.hyperlink_to("© OpenStreetMap", ATTRIBUTION_URL);
				} else {
					ui.horizontal(|ui| {
						let resp = ui.label("Imagery:");
						if let Some(logo) = attribution.logo_light {
							ui.add(Image::new(logo).max_height(resp.rect.height()).max_width(80.0));
						}
						ui.hyperlink_to(attribution.text, attribution.url);
					});
					ui.horizontal(|ui| {
						ui.label("Map data:");
						ui.hyperlink_to("© OpenStreetMap", ATTRIBUTION_URL);
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

pub fn tags(ui: &Ui, editing_tags: &indexmap::IndexMap<String, String>, edit_enabled: bool) -> Option<TagsEditKind> {
	let resp = egui::Window::new("Tags")
		.collapsible(true)
		.vscroll(true)
		.default_size([300., 200.])
		.default_pos([WINDOW_MARGIN, WINDOW_MARGIN.mul_add(2., TOP_BAR_HEIGHT) + 42.])
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			let mut change = None;

			TableBuilder::new(ui)
				.striped(true)
				.resizable(true)
				.column(Column::initial(100.0).clip(true))
				.column(Column::remainder().clip(true))
				.header(16.0, |mut header| {
					header.col(|ui| { ui.strong("Key"); });
					header.col(|ui| { ui.strong("Value"); });
				})
				.body(|body| {
					if edit_enabled {
						// todo: add ability to add new tag
						body.rows(20.0, editing_tags.len() + 1, |mut row| {
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
			ui.allocate_space(Vec2::new(0., ui.available_height()));
			change
		}).unwrap();

	resp.inner?
}

// Returns whether the licenses button was pressed
pub fn map<'a>(
	ui: &Ui,
	map_state: &mut MapState,
	providers: &mut impl Iterator<Item=&'a Provider>,
) -> bool {
	egui::Window::new("Map")
		.collapsible(false)
		.resizable(false)
		.title_bar(false)
		.fixed_size([150., 150.])
		.anchor(Align2::RIGHT_BOTTOM, [-WINDOW_MARGIN, -WINDOW_MARGIN])
		.frame(TRANSPARENT_FRAME)
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

				ui.button("Show Open-Source Licenses").clicked()
			}).body_returned.unwrap_or(false)
		}).unwrap().inner.unwrap_or(false)
}

pub fn history(ui: &Ui, history: &Vec<Change>) {
	egui::Window::new("History")
		.max_height(256.0)
		.anchor(Align2::RIGHT_TOP, [-10., 42.])
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			if history.is_empty() {
				ui.weak("Empty");
			} else {
				egui::ScrollArea::vertical().auto_shrink([true, false]).show(ui, |ui| {
					for change in history {
						ui.label(format!("{change}"));
					}
				});
			}
		});
}

// Returns whether a download was triggered
pub fn toolbar(ui: &mut Ui, state: &mut MapState, editor_state: &mut EditorPluginState) -> bool {
	let top_left = Pos2::from([WINDOW_MARGIN, TOP_BAR_HEIGHT + WINDOW_MARGIN]);
	let rect = Rect::from_two_pos(top_left, top_left + Vec2::new(MODE_INDICATOR_WIDTH, TRANSPARENT_FRAME.total_margin().top.mul_add(2., 24. + 4.)));

	// Draw mode indicator
	if ui.allocate_rect(rect, Sense::hover()).on_hover_text(format!("{} mode\nPress Space to toggle", editor_state.mode)).clicked() {
		editor_state.mode = match editor_state.mode {
			EditMode::View => EditMode::Edit,
			EditMode::Edit => EditMode::View,
		}
	}
	ui.painter().rect_filled(rect, CornerRadius::ZERO, editor_state.mode.color());

	// Draw toolbar
	egui::Window::new("Toolbar")
		.title_bar(false)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, top_left.to_vec2() + Vec2::new(MODE_INDICATOR_WIDTH, 0.0))
		.frame(TRANSPARENT_FRAME.corner_radius(CornerRadius { ne: 6, nw: 0, se: 6, sw: 0 }))
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
						let selected = if editor_state.mode == EditMode::View {
							state.selection_mode & flag as u8 != 0
						} else { false };

						let image = Image::new(icon).fit_to_exact_size(Vec2::splat(TOOLBAR_ICON_SIZE));
						let resp = ui.add(Button::image(image).selected(selected).corner_radius(4));

						if !ui.ctx().wants_keyboard_input()
							&& (resp.clicked() || ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)))
						{
							if editor_state.mode == EditMode::View {
								state.selection_mode ^= flag as u8;
							} else {
								editor_state.operation = EditOperation::AddNode;
							}
						}
					}
				}

				ui.separator();

				/* map download */ {
					match &state.download {
						MapDownloadState::Idle(status) => {
							let enabled = editor_state.map_bbox.area() < MAX_DOWNLOAD_AREA;
							let time = ui.ctx().input(|i| i.time);

							let button_resp = if let Some((status, prev_time)) = status && time - prev_time < DOWNLOAD_FEEDBACK_SECONDS {
								let text = egui::RichText::new(if status.is_ok() { "✔" } else { "✘" }).strong();
								ui.add_enabled(enabled, Button::new(text).min_size(Vec2::splat(TOP_BAR_BUTTON_SIZE)).corner_radius(4))
							} else { // todo: global error modal / success toast
								let image = Image::new(icons::DOWNLOAD).fit_to_exact_size(Vec2::splat(TOP_BAR_BUTTON_SIZE - 4.0));
								ui.add_enabled(enabled, Button::image(image).corner_radius(4))
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
							let resp = ui.add_enabled(false, Button::new(()).min_size(Vec2::splat(TOP_BAR_BUTTON_SIZE)));
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
		.frame(TRANSPARENT_FRAME)
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
use crate::app::editor::{cache::EditorOsmData, states::CacheFlag};

#[cfg(feature = "debug")]
pub fn debug(ui: &Ui, selected_provider: Option<&Provider>, provider: Option<&super::providers::TilesKind>, editor_osm_data: &EditorOsmData) {
	egui::Window::new("Debug")
		.resizable(false)
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			ui.heading(format!("Δt: {:.4} ms", ui.input(|i| i.unstable_dt) * 1000.0));
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

pub fn licenses_modal(ctx: &egui::Context) -> bool {
	let screen = ctx.screen_rect();
	let width = screen.width() * 0.8;
	let height = screen.height() * 0.6;

	let area = Area::new("licenses_area".into())
		.anchor(Align2::CENTER_CENTER, Vec2::new(0.0, TOP_BAR_HEIGHT / 2.0))
		.default_width(width);

	egui::Modal::new("licenses".into()).area(area).show(ctx, |ui| {
		ui.heading("Open-Source Licenses");
		ui.add_space(4.0);
		ui.horizontal(|ui| {
			ui.spacing_mut().item_spacing = Vec2::ZERO;
			ui.hyperlink_to(env!("CARGO_CRATE_NAME"), env!("CARGO_PKG_REPOSITORY"));
			ui.label(" has been made possible by the following awesome open-source libraries:");
		});
		ui.separator();
		egui::ScrollArea::vertical()
			.max_height(height)
			.show(ui, |ui| {
				#[cfg(not(debug_assertions))]
				let text = egui::RichText::new(crate::LICENSES_TEXT)
					.text_style(egui::TextStyle::Monospace);

				#[cfg(debug_assertions)]
				let text = "\nLicenses are not loaded in a debug build.\n";

				ui.label(text);
			});
		ui.separator();
		ui.small("Packages marked with (*) have been \"de-duplicated\".\n\
		          The dependencies for the package have already been shown elsewhere in the graph, \
		          and so are not repeated.");
		ui.add_space(4.0);
		ui.vertical_centered_justified(|ui| ui.button("Close").clicked()).inner
	}).inner
}

#[cfg(target_family = "wasm")]
pub fn update_modal(ctx: &egui::Context) -> bool {
	egui::Modal::new("update".into()).show(ctx, |ui| {
		ui.heading("Update Available");
		ui.label("Your browser has detected a new version of walkers-editor.");
		ui.separator();
		ui.strong("How to update:");
		ui.label("1. Close all instances of the editor.");
		ui.label("2. Open the latest version in a new tab.");
		ui.label("The new version should be loaded automatically.");
		ui.separator();
		ui.vertical_centered_justified(|ui| ui.button("Close").clicked()).inner
	}).inner
}

#[cfg(target_family = "wasm")]
pub fn firefox_modal(ctx: &egui::Context) -> bool {
	egui::Modal::new("firefox".into()).show(ctx, |ui| {
		ui.heading("Firefox Warning");
		ui.label("You are using Firefox.");
		ui.separator();
		ui.label("Please use a Chromium-based browser for a faster and less janky experience, or consider downloading the native application directly.");
		ui.separator();
		ui.vertical_centered_justified(|ui| ui.button("Close").clicked()).inner
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
		.frame(TRANSPARENT_FRAME)
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
