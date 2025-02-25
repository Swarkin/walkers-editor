use std::fmt::{Display, Formatter};
use super::editor::{changes::Change, visual::Visualization};
use super::providers::Provider;
use eframe::egui;
use egui::{Align2, Grid, Ui, Window};
use osm_parser::OsmData;
use walkers::sources::Attribution;

pub enum Windows {
	Tags = 1 << 0,
	Controls = 1 << 1,
	History = 1 << 2,
	Download = 1 << 3,
	#[cfg(feature = "debug")]
	Debug = 1 << 4,
}

impl Display for Windows {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Windows::Tags => "Tags",
			Windows::Controls => "Controls",
			Windows::History => "History",
			Windows::Download => "Download",
			#[cfg(feature = "debug")]
			Windows::Debug => "Debug",
		})
	}
}

impl Windows {
	#[cfg(not(feature = "debug"))]
	pub const ITER: [Windows; 4] = [Windows::Tags, Windows::Controls, Windows::History, Windows::Download];
	#[cfg(feature = "debug")]
	pub const ITER: [Windows; 5] = [Windows::Tags, Windows::Controls, Windows::History, Windows::Download, Windows::Debug];
}

fn transparent_frame(style: &egui::Style) -> egui::Frame {
	let mut frame = egui::Frame::window(style);
	frame.fill = frame.fill.gamma_multiply(0.85);
	frame.shadow = egui::Shadow::NONE;
	frame
}

pub fn acknowledge(ui: &Ui, attribution: Attribution) {
	Window::new("Acknowledge")
		.collapsible(false)
		.resizable(false)
		.title_bar(false)
		.anchor(Align2::LEFT_BOTTOM, [10., -10.])
		.frame(transparent_frame(ui.style()))
		.show(ui.ctx(), |ui| {
			ui.horizontal(|ui| {
				if let Some(logo) = attribution.logo_light {
					ui.add(egui::Image::new(logo).max_height(30.0).max_width(80.0));
				}
				ui.hyperlink_to(attribution.text, attribution.url);
			});
		});
}

pub fn controls(
	ui: &Ui,
	selected_provider: &mut Provider,
	possible_providers: &mut dyn Iterator<Item = &Provider>,
	selected_visualization: &mut Visualization,
	scale_factor: &mut f32,
	zoom_with_ctrl: &mut bool,
) {
	Window::new("Controls")
		.collapsible(false)
		.resizable(false)
		.title_bar(false)
		.fixed_size([150., 150.])
		.anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
		.frame(transparent_frame(ui.style()))
		.show(ui.ctx(), |ui| {
			ui.collapsing("Map", |ui| {
				egui::ComboBox::from_label("Tile Provider")
					.selected_text(format!("{:?}", selected_provider))
					.show_ui(ui, |ui| {
						for p in possible_providers {
							ui.selectable_value(selected_provider, *p, format!("{p:?}"));
						}
					});

				egui::ComboBox::from_label("Visualization")
					.selected_text(format!("{selected_visualization:?}"))
					.show_ui(ui, |ui| {
						ui.selectable_value(selected_visualization, Visualization::Default, "Default");
						ui.selectable_value(selected_visualization, Visualization::Sidewalks, "Sidewalks");
					});

				ui.add(egui::Slider::new(scale_factor, 0.1..=2.0).text("Scale factor"));
				ui.checkbox(zoom_with_ctrl, "Zoom with Ctrl");
			});
		});
}

pub fn tags(ui: &Ui, tags: &osm_parser::Tags) {
	Window::new("Tags")
		.collapsible(true)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, [10., 42.])
		.frame(transparent_frame(ui.style()))
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

pub fn download(ui: &Ui, bbox: (f64, f64, f64, f64)) -> Option<OsmData> {
	let resp = Window::new("Download")
		.collapsible(true)
		.resizable(false)
		.title_bar(false)
		.anchor(Align2::CENTER_BOTTOM, [0., -10.])
		.frame(transparent_frame(ui.style()))
		.show(ui.ctx(), |ui| {
			if ui.button("Download Area").clicked() {
				let diff_x = (bbox.0 - bbox.2) / 2.0;
				let diff_y = (bbox.1 - bbox.3) / 2.0;
				// todo: error handling
				Some(super::osm::get_map(bbox.0 + diff_x, bbox.1 - diff_y, bbox.2 + diff_x, bbox.3 - diff_y).unwrap())
			} else { None }
		});

	if let Some(inner) = resp {
		inner.inner.unwrap()
	} else { None }
}

pub fn history(ui: &Ui, history: &Vec<Change>) {
	Window::new("History")
		.max_height(256.0)
		.anchor(Align2::RIGHT_TOP, [-10., 42.])
		.frame(transparent_frame(ui.style()))
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

#[cfg(feature = "debug")]
pub fn debug(ui: &Ui, debug_times: &super::DebugTimes) {
	Window::new("Debug")
		.collapsible(true)
		.resizable(false)
		.anchor(Align2::CENTER_TOP, [0., 42.])
		.frame(transparent_frame(ui.style()))
		.show(ui.ctx(), |ui| {
			let biggest = debug_times.iter().map(|(_, time)| time).max().unwrap();

			for (text, duration) in debug_times {
				let text = egui::WidgetText::from(format!("{: >6.2} ms: {text}", *duration as f32 / 1000.)).monospace();
				if duration == biggest {
					ui.label(text.strong());
				} else {
					ui.label(text);
				}
			}
		});
}
