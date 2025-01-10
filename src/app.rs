mod places;
mod windows;
mod editor;
mod providers;

use editor::visualizers::Visualization;
use editor::EditorPluginState;
use eframe::egui;
use egui::{Context, Frame};
use osm_parser::OsmData;
use providers::Provider;
use std::collections::HashMap;
use walkers::{Map, MapMemory, Tiles};

pub struct MyApp {
	providers: HashMap<Provider, Box<dyn Tiles + Send>>,
	selected_provider: Provider,
	selected_visualizer: Visualization,
	map_memory: MapMemory,
	osm_data: OsmData,
	scale_factor: f32,
	editor_state: EditorPluginState,
}

impl MyApp {
	pub fn new(egui_ctx: Context) -> Self {
		Self {
			providers: providers::providers(egui_ctx),
			selected_provider: Default::default(),
			selected_visualizer: Default::default(),
			map_memory: Default::default(),
			osm_data: osm_parser::parse("school.osm").unwrap(),
			scale_factor: 1.0,
			editor_state: Default::default(),
		}
	}
}

impl eframe::App for MyApp {
	fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
		egui::CentralPanel::default()
			.frame(Frame::none())
			.show(ctx, |ui| {
				let tiles = self
					.providers
					.get_mut(&self.selected_provider)
					.unwrap()
					.as_mut();
				let attribution = tiles.attribution();
				
				ui.add(Map::new(Some(tiles), &mut self.map_memory, places::school())
					.with_plugin(editor::EditorPlugin { 
						state: &mut self.editor_state, 
						osm_data: &self.osm_data, 
						scale_factor: self.scale_factor, 
						visualization: self.selected_visualizer,
					})
				);
				
				windows::zoom(ui, &mut self.map_memory);
				windows::controls(ui, &mut self.selected_provider, &mut self.providers.keys(), &mut self.selected_visualizer, &mut self.scale_factor);
				windows::acknowledge(ui, attribution);

				if let Some(id) = self.editor_state.selected.or(self.editor_state.hovered) {
					if let Some(tags) = &self.osm_data.ways[&id].tags {
						windows::tags(ui, tags);
					}
				}
			});
	}
}
