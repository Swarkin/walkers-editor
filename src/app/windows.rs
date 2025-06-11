use super::editor::{
	cache::Change,
	consts::{osm::ATTRIBUTION_URL, TOP_BAR_HEIGHT, WINDOW_MARGIN},
	states::{MapDownloadState, MapState, SelectionBitflag, SelectionFlag},
	visual::{FillMode, Visualization},
};
use super::osm::Bbox;
use super::providers::Provider;
#[cfg(feature = "debug")]
use super::providers::TilesKind;
use eframe::egui;
use egui::{include_image, Align2, Button, Color32, CornerRadius, Frame, Grid, Image, ImageSource, Key, KeyboardShortcut, Margin, Modifiers, Order, Shadow, Stroke, Ui, Vec2};
use walkers::sources::Attribution;

const TOOLBAR_IMAGES: [ImageSource; 3] = [
	include_image!("../../assets/ui/primitives/node24.svg"),
	include_image!("../../assets/ui/primitives/way24.svg"),
	include_image!("../../assets/ui/primitives/area24.svg")
];

const TOOLBAR_SHORTCUTS: [KeyboardShortcut; 3] = [
	KeyboardShortcut { modifiers: Modifiers::NONE, logical_key: Key::Num1 },
	KeyboardShortcut { modifiers: Modifiers::NONE, logical_key: Key::Num2 },
	KeyboardShortcut { modifiers: Modifiers::NONE, logical_key: Key::Num3 },
];

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
	Download = 1 << 3,
	Toolbar = 1 << 4,
	#[cfg(feature = "debug")]
	Debug = 1 << 7,
}

impl std::fmt::Display for Window {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Window::Tags => "Tags",
			Window::Map => "Controls",
			Window::History => "History",
			Window::Download => "Download",
			Window::Toolbar => "Toolbar",
			#[cfg(feature = "debug")]
			Window::Debug => "Debug",
		})
	}
}

impl Window {
	#[cfg(not(feature = "debug"))]
	pub const ITER: [Window; 5] = [Window::Tags, Window::Map, Window::History, Window::Download, Window::Toolbar];
	#[cfg(feature = "debug")]
	pub const ITER: [Window; 6] = [Window::Tags, Window::Map, Window::History, Window::Download, Window::Toolbar, Window::Debug];
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

pub fn map<'a>(
	ui: &Ui,
	map_state: &mut MapState,
	providers: &mut impl Iterator<Item = &'a Provider>,
) {
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

				ui.add_enabled_ui((map_state.selection_mode & SelectionFlag::Ways as u8) == 0, |ui| {
					egui::ComboBox::from_label("Visualization")
						.selected_text(format!("{:?}", map_state.selected_visualization))
						.show_ui(ui, |ui| {
							for visualization in Visualization::ITER {
								ui.selectable_value(&mut map_state.selected_visualization, visualization, format!("{visualization:?}"));
							}
						});
				});

				ui.add(egui::Slider::new(&mut map_state.scale_factor, 0.1..=2.0).text("Scale factor"));
				ui.checkbox(&mut map_state.zoom_with_ctrl, "Zoom with Ctrl");
			});
		});
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

pub fn download(ui: &Ui, bbox: &Bbox, download_state: &MapDownloadState) -> Option<super::worker::Request> {
	egui::Window::new("Download")
		.collapsible(true)
		.resizable(false)
		.title_bar(false)
		.anchor(Align2::CENTER_BOTTOM, [0., -WINDOW_MARGIN])
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			ui.horizontal(|ui| {
				let req = if ui.add_enabled(!download_state.is_busy(), Button::new("Download Area")).clicked() {
					let diff_x = (bbox.left - bbox.right) / 2.0;
					let diff_y = (bbox.bottom - bbox.top) / 2.0;
					Some(super::worker::Request::GetMap(Box::new(Bbox{ left: bbox.left + diff_x, bottom: bbox.bottom - diff_y, right: bbox.right + diff_x, top: bbox.top - diff_y })))
				} else { None };

				match &download_state {
					MapDownloadState::Idle(prev) => {
						if let Some(prev) = prev {
							match prev {
								Ok(_) => ui.strong("✔"),
								Err(_) => ui.strong("✘"), // todo: global error modal / toast
							};
						}
					}
					MapDownloadState::Downloading => {
						ui.spinner();
					}
				}

				req
			}).inner
		})?.inner?
}

pub fn toolbar(ui: &Ui, selection_flags: &mut SelectionBitflag) {
	egui::Window::new("Toolbar")
		.title_bar(false)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, [WINDOW_MARGIN, TOP_BAR_HEIGHT + WINDOW_MARGIN])
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			ui.spacing_mut().button_padding = Vec2::splat(2.0);
			ui.horizontal(|ui| {
				for ((flag, image), shortcut) in SelectionFlag::ITER.into_iter().zip(TOOLBAR_IMAGES).zip(&TOOLBAR_SHORTCUTS) {
					let state = *selection_flags & flag as u8 != 0;
					let image = Image::new(image).fit_to_exact_size(Vec2::splat(24.0));

					if ui.add(Button::image(image).selected(state).corner_radius(CornerRadius::same(4))).clicked() || ui.input_mut(|i| i.consume_shortcut(shortcut)) {
						*selection_flags ^= flag as u8;
					}
				}
			});
		});
}

#[cfg(feature = "debug")]
pub fn debug(ui: &Ui, selected_provider: Option<&Provider>, provider: Option<&TilesKind>) {
	egui::Window::new("Debug")
		.collapsible(true)
		.resizable(false)
		.anchor(Align2::CENTER_TOP, [0., 42.])
		.frame(TRANSPARENT_FRAME)
		.show(ui.ctx(), |ui| {
			ui.heading(format!("Δt: {} ms", ui.input(|i| i.unstable_dt) * 1000.0));
			if let Some(p) = provider {
				let TilesKind::Http(http_tiles) = p;
				let stats = http_tiles.stats();
				ui.label(format!("in-progress requests for {:?}: {}", selected_provider.unwrap(), stats.in_progress));
			}
		});
}
