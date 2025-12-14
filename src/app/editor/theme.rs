use eframe::egui;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
	#[expect(clippy::struct_field_names)]
	pub theme: ThemeSetting,
	pub scale_factor: f32,
	pub node_size: f32,
	pub node_size_orphan: f32,
	pub node_color: Color32,
	pub node_connected_color: Color32,
	pub node_stroke_color: Color32,
	pub node_stroke_width: f32,
	pub way_width: f32,
	pub way_color: Color32,
	pub hover_color: Color32,
	pub selection_color: Color32,
	pub path_width: f32,
	pub service_road_width: f32,
	pub minor_road_width: f32,
	pub major_road_width: f32,
	pub highway_path_color: Color32,
	pub highway_footway_color: Color32,
	pub highway_steps_color: Color32,
	pub highway_track_color: Color32,

	pub landuse_farmland_color: Color32,
	pub landuse_residential_color: Color32,
	pub landuse_forest_color: Color32,
	pub landuse_grass_color: Color32,
	pub landuse_commercial_color: Color32,

	pub natural_water_color: Color32,
	pub natural_wood_color: Color32,
	pub natural_scrub_color: Color32,
	pub natural_wetland_color: Color32,
	pub natural_grassland_color: Color32,

	pub building_width: f32,
	pub building_color: Color32,
	pub sidewalk_width: f32,
	pub sidewalk_yes_color: Color32,
	pub sidewalk_no_color: Color32,
	pub sidewalk_separate_color: Color32,
	pub sidewalk_unknown_color: Color32,
}

impl Default for Theme {
	fn default() -> Self {
		Self {
			theme: ThemeSetting::System,
			scale_factor: 1.0,
			node_size: 3.0,
			node_size_orphan: 4.0,
			node_color: Color32::WHITE,
			node_connected_color: Color32::LIGHT_GRAY,
			node_stroke_color: Color32::GRAY,
			node_stroke_width: 1.0,
			way_width: 2.0,
			way_color: Color32::GRAY,
			hover_color: Color32::from_rgb(100, 200, 255),
			selection_color: Color32::from_rgb(40, 180, 255),
			path_width: 2.5,
			service_road_width: 4.0,
			minor_road_width: 5.0,
			major_road_width: 6.0,
			highway_path_color: Color32::from_rgb(221, 204, 170),
			highway_footway_color: Color32::WHITE,
			highway_steps_color: Color32::from_rgb(129, 210, 92),
			highway_track_color: Color32::from_rgb(197, 181, 159),

			landuse_farmland_color: Color32::from_rgb(190, 232, 63),
			landuse_residential_color: Color32::from_rgb(196, 190, 25),
			landuse_forest_color: Color32::from_rgb(140, 208, 95),
			landuse_grass_color: Color32::from_rgb(140, 208, 95),
			landuse_commercial_color: Color32::from_rgb(214, 136, 26),

			natural_water_color: Color32::from_rgb(119, 212, 222),
			natural_wood_color: Color32::from_rgb(140, 208, 95),
			natural_scrub_color: Color32::from_rgb(255, 255, 148),
			natural_wetland_color: Color32::from_rgb(153, 225, 170),
			natural_grassland_color: Color32::from_rgb(140, 208, 95),

			building_width: 1.5,
			building_color: Color32::from_rgb(224, 110, 95),
			sidewalk_width: 4.0,
			sidewalk_yes_color: Color32::LIGHT_GREEN,
			sidewalk_no_color: Color32::LIGHT_GRAY,
			sidewalk_separate_color: Color32::LIGHT_BLUE,
			sidewalk_unknown_color: Color32::LIGHT_RED,
		}
	}
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSetting {
	Dark,
	Light,
	#[default] System,
}

impl From<ThemeSetting> for egui::ThemePreference {
	fn from(value: ThemeSetting) -> Self {
		match value {
			ThemeSetting::Dark => Self::Dark,
			ThemeSetting::Light => Self::Light,
			ThemeSetting::System => Self::System,
		}
	}
}

impl From<egui::ThemePreference> for ThemeSetting {
	fn from(value: egui::ThemePreference) -> Self {
		match value {
			egui::ThemePreference::Dark => Self::Dark,
			egui::ThemePreference::Light => Self::Light,
			egui::ThemePreference::System => Self::System,
		}
	}
}
