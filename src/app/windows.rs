use super::editor::{
	cache::{Change, ElementRef},
	consts::{osm::*, *},
	states::{MapDownloadState, MapState, SelectionFlag},
	visual::{FillMode, Visualization},
};
use super::icons;
use super::osm::Bbox;
use super::providers::Provider;
use eframe::egui;
use egui::text::LayoutJob;
use egui::{Align2, Area, AtomExt, Button, Color32, CornerRadius, Event, FontId, Frame, Grid, Image, ImageSource, InnerResponse, Key, Margin, Order, Pos2, Shadow, Stroke, TextFormat, Ui, Vec2};
use walkers::sources::Attribution;

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
			#[cfg(feature = "debug")]
			Self::Debug => "Debug",
		})
	}
}

impl Window {
	#[cfg(not(feature = "debug"))]
	pub const ITER: [Self; 4] = [Self::Tags, Self::Map, Self::History, Self::Toolbar];
	#[cfg(feature = "debug")]
	pub const ITER: [Self; 5] = [Self::Tags, Self::Map, Self::History, Self::Toolbar, Self::Debug];
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

pub fn tags(ui: &Ui, tags: &osm_parser::Tags) {
	egui::Window::new("Tags")
		.collapsible(true)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, [WINDOW_MARGIN, TOP_BAR_HEIGHT + WINDOW_MARGIN + 54.]) // todo: extract magic number
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			Grid::new("tags").show(ui, |ui| {
				for (k, v) in tags {
					ui.label(k);
					ui.label(v);
					ui.end_row();
				}
			});
		});
}

// Returns whether the licenses button was pressed
pub fn map<'a>(
	ui: &Ui,
	map_state: &mut MapState,
	providers: &mut impl Iterator<Item = &'a Provider>,
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
pub fn toolbar(ui: &Ui, state: &mut MapState, bbox: &Bbox) -> bool {
	egui::Window::new("Toolbar")
		.title_bar(false)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, [WINDOW_MARGIN, TOP_BAR_HEIGHT + WINDOW_MARGIN])
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			ui.spacing_mut().button_padding = Vec2::splat(2.0);
			ui.horizontal(|ui| {
				/* selection modes */ {
					const ICONS: [ImageSource; 2] = [
						icons::PRIMITIVE_NODE_ICON,
						icons::PRIMITIVE_WAY_ICON,
					];
					const KEYS: [Key; 2] = [Key::Num1, Key::Num2];

					for ((flag, icon), key) in SelectionFlag::ITER.into_iter()
						.zip(ICONS).zip(KEYS)
					{
						let selected = state.selection_mode & flag as u8 != 0;
						let image = Image::new(icon).fit_to_exact_size(Vec2::splat(24.0));

						let resp = ui.add(Button::image(image).selected(selected).corner_radius(4));
						if !ui.ctx().wants_keyboard_input()
							&& (resp.clicked() || ui.input_mut(|i| i.key_pressed(key)))
						{
							state.selection_mode ^= flag as u8;
						}
					}
				}

				ui.separator();

				/* map download */ {
					match &state.download {
						MapDownloadState::Idle(status) => {
							let enabled = bbox.area() < MAX_DOWNLOAD_AREA;
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

#[cfg(feature = "debug")]
use crate::app::editor::{cache::EditorOsmData, states::CacheFlag};

#[cfg(feature = "debug")]
pub fn debug(ui: &Ui, selected_provider: Option<&Provider>, provider: Option<&super::providers::TilesKind>, editor_osm_data: &EditorOsmData) {
	egui::Window::new("Debug")
		.resizable(false)
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			ui.heading(format!("Δt: {} ms", ui.input(|i| i.unstable_dt) * 1000.0));
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
				egui_extras::TableBuilder::new(ui)
					.striped(true)
					.columns(egui_extras::Column::auto(), 3)
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
				let text = "Licenses are not loaded in a debug build.";

				ui.label(text);
			});
		ui.separator();
		ui.label("Packages marked with (*) have been \"de-duplicated\".\n\
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
