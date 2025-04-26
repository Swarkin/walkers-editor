use super::editor::{changes::Change, visual::Visualization};
use super::osm::Bbox;
use super::providers::Provider;
#[cfg(feature = "debug")]
use super::providers::TilesKind;
use crate::app::editor::states::MapDownloadState;
use eframe::egui;
use egui::{Align2, Grid, Ui, Window};
use std::fmt::{Display, Formatter};
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

// todo: make const
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

pub fn controls<'a>(
	ui: &Ui,
	selected_provider: &mut Option<Provider>,
	possible_providers: &mut impl Iterator<Item = &'a Provider>,
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
				let selected_text = if let Some(selected_provider) = selected_provider {
					format!("{selected_provider:?}")
				} else { "None".into() };

				egui::ComboBox::from_label("Tile Provider")
					.selected_text(selected_text)
					.show_ui(ui, |ui| {
						for p in possible_providers {
							let mut selected = *selected_provider == Some(*p);
							if ui.toggle_value(&mut selected, format!("{p:?}")).changed() {
								*selected_provider = if selected { Some(*p) } else { None }
							}
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

pub fn download(ui: &Ui, bbox: &Bbox, download_state: &MapDownloadState) -> Option<super::worker::Request> {
	Window::new("Download")
		.collapsible(true)
		.resizable(false)
		.title_bar(false)
		.anchor(Align2::CENTER_BOTTOM, [0., -10.])
		.frame(transparent_frame(ui.style()))
		.show(ui.ctx(), |ui| {
			ui.horizontal(|ui| {
				let req = if ui.add_enabled(!download_state.is_busy(), egui::Button::new("Download Area")).clicked() {
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
pub fn debug(ui: &Ui, debug_times: &super::DebugTimes, selected_provider: Option<&Provider>, provider: Option<&TilesKind>) {
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

			if let Some(p) = provider {
				let TilesKind::Http(http_tiles) = p;
				let stats = http_tiles.stats();
				ui.label(format!("in-progress requests for {:?}: {}", selected_provider.unwrap(), stats.in_progress));
			}
		});
}
