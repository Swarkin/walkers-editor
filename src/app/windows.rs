use crate::app::providers::Provider;
use eframe::egui;
use eframe::egui::Grid;
use egui::{Align2, RichText, Ui, Window};
use walkers::{sources::Attribution, MapMemory};
use crate::app::editor::visualizers::Visualization;

pub fn acknowledge(ui: &Ui, attribution: Attribution) {
    Window::new("Acknowledge")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [10., 10.])
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
                        ui.selectable_value(selected_visualization, Visualization::Default, "iD");
                        ui.selectable_value(selected_visualization, Visualization::Sidewalks, "Sidewalks");
                    });

                ui.add(egui::Slider::new(scale_factor, 0.1..=2.0).text("OSM data scale"));
            });
        });
}

pub fn zoom(ui: &Ui, map_memory: &mut MapMemory) {
    Window::new("Zoom")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_BOTTOM, [10., -10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("➕").heading()).clicked() {
                    let _ = map_memory.zoom_in();
                }

                if ui.button(RichText::new("➖").heading()).clicked() {
                    let _ = map_memory.zoom_out();
                }
            });
        });
}

pub fn tags(ui: &Ui, tags: &osm_parser::Tags) {
    Window::new("Tags")
        .collapsible(true)
        .resizable(false)
        .title_bar(true)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
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
