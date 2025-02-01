use super::editor::{changes::Change, visual::Visualization};
use super::providers::Provider;
use eframe::egui;
use egui::{Align2, Grid, Ui, Vec2b, Window};
use osm_parser::OsmData;
use walkers::sources::Attribution;

pub fn acknowledge(ui: &Ui, attribution: Attribution) {
	Window::new("Acknowledge")
		.collapsible(false)
		.resizable(false)
		.title_bar(false)
		.anchor(Align2::LEFT_BOTTOM, [10., -10.])
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
) {
	Window::new("Controls")
		.collapsible(false)
		.resizable(false)
		.title_bar(false)
		.anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
		.fixed_size([150., 150.])
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
			});
		});
}

pub fn tags(ui: &Ui, tags: &osm_parser::Tags) {
	Window::new("Tags")
		.collapsible(true)
		.resizable(false)
		.anchor(Align2::LEFT_TOP, [10., 10.])
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
		.show(ui.ctx(), |ui| {
			if ui.button("Download Area").clicked() {
				let diff_x = (bbox.0 - bbox.2) / 2.0;
				let diff_y = (bbox.1 - bbox.3) / 2.0;
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
		.anchor(Align2::RIGHT_TOP, [-10., 10.])
		.show(ui.ctx(), |ui| {
			if history.is_empty() {
				ui.weak("Empty");
			} else {
				egui::ScrollArea::vertical().auto_shrink(Vec2b::new(false, false)).show(ui, |ui| {
					for change in history {
						ui.label(format!("{change}"));
					}
				});
			}
		});
}
