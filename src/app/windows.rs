use super::editor::{
	cache::Change,
	consts::{osm::ATTRIBUTION_URL, MAX_DOWNLOAD_AREA, TOP_BAR_HEIGHT, WINDOW_MARGIN},
	states::{MapDownloadState, MapState, SelectionFlag},
	visual::{FillMode, Visualization},
};
use super::osm::Bbox;
use super::providers::Provider;
use eframe::egui;
use eframe::egui::text::LayoutJob;
use eframe::egui::{FontId, TextFormat};
use egui::{
	include_image, Align2, Area, AtomExt, Button, Color32, CornerRadius, CursorIcon, Event, Frame,
	Grid, Image, ImageSource, InnerResponse, Key, KeyboardShortcut, Margin, Modifiers, Order, Pos2,
	RichText, Shadow, Stroke, TextStyle, Ui, Vec2
};
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
			Window::Tags => "Tags",
			Window::Map => "Controls",
			Window::History => "History",
			Window::Toolbar => "Toolbar",
			#[cfg(feature = "debug")]
			Window::Debug => "Debug",
		})
	}
}

impl Window {
	#[cfg(not(feature = "debug"))]
	pub const ITER: [Window; 4] = [Window::Tags, Window::Map, Window::History, Window::Toolbar];
	#[cfg(feature = "debug")]
	pub const ITER: [Window; 5] = [Window::Tags, Window::Map, Window::History, Window::Toolbar, Window::Debug];
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
				let text = if let Some(selected_provider) = map_state.selected_provider {
					format!("{selected_provider:?}")
				} else {
					"None".into()
				};

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
						include_image!("../../assets/ui/primitives/node24.svg"),
						include_image!("../../assets/ui/primitives/way24.svg"),
						//include_image!("../../assets/ui/primitives/area24.svg"),
					];
					static SHORTCUTS: [&KeyboardShortcut; 2] = [
						&KeyboardShortcut::new(Modifiers::NONE, Key::Num1),
						&KeyboardShortcut::new(Modifiers::NONE, Key::Num2),
						//KeyboardShortcut::new(Modifiers::NONE, Key::Num3),
					];

					for ((flag, icon), shortcut) in SelectionFlag::ITER.into_iter()
						.zip(ICONS).zip(SHORTCUTS)
					{
						let selected = state.selection_mode & flag as u8 != 0;
						let image = Image::new(icon).fit_to_exact_size(Vec2::splat(24.0));

						let resp = ui.add(Button::image(image).selected(selected).corner_radius(4));
						if !ui.ctx().wants_keyboard_input()
							&& (resp.clicked() || ui.input_mut(|i| i.consume_shortcut(shortcut)))
						{
							state.selection_mode ^= flag as u8;
						}
					}
				}

				ui.separator();

				/* map download */ {
					match &mut state.download {
						MapDownloadState::Idle(status) => {
							const ICON: ImageSource = include_image!("../../assets/ui/download.svg");
							static SHORTCUT: &KeyboardShortcut =
								&KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::ArrowDown);

							let enabled = bbox.area() < MAX_DOWNLOAD_AREA;
							let image = Image::new(ICON).fit_to_exact_size(Vec2::splat(24.0));
							let button_resp = ui.add_enabled(enabled,
								Button::image(image).corner_radius(4)
							);

							let status_resp = status.as_mut().map(|status| match status {
								Ok(_) => ui.strong("✔"),
								Err(_) => ui.strong("✘"), // todo: global error modal / success toast
							});

							if let Some(resp) = status_resp {
								if resp.clicked() {
									*status = None;
								} else if resp.hovered() {
									ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
								}
							}

							// Return whether a download was triggered
							enabled && !ui.ctx().wants_keyboard_input() && (button_resp.clicked() || ui.input_mut(|i| {
								let any_echo_events = i.events.iter().any(|e| {
									if let Event::Key { repeat, .. } = e { *repeat } else { false }
								});
								!any_echo_events && i.consume_shortcut(SHORTCUT)
							}))
						}
						MapDownloadState::Downloading => {
							let resp = ui.add_enabled(false, Button::new("").min_size(Vec2::splat(28.0)));
							ui.put(resp.rect, egui::Spinner::new());

							false
						}
					}
				}
			}).inner
		}).unwrap().inner.unwrap()
}

use crate::app::editor::cache::ElementRef;
#[cfg(feature = "debug")]
use crate::app::editor::states::CacheFlag;

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
				let text = RichText::new(crate::LICENSES_TEXT)
					.text_style(TextStyle::Monospace);

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

pub enum OverlapSelectorResult<'a> {
	None,
	Hovered(ElementRef<'a>),
	Selected(ElementRef<'a>),
}

pub fn overlap_selector<'a>(ui: &mut Ui, pos: Pos2, hovered: Vec<ElementRef<'a>>) -> InnerResponse<Option<OverlapSelectorResult<'a>>> {
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
					.map(|x| format!("{x}\n"))
					.unwrap_or_else(|| format!("Unnamed {}\n", element.type_str()));

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
