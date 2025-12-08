use eframe::egui;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
	#[allow(clippy::struct_field_names)]
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
	pub path_color: Color32,
	pub footway_color: Color32,
	pub steps_color: Color32,
	pub track_color: Color32,
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
			path_color: Color32::from_rgb(221, 204, 170),
			footway_color: Color32::WHITE,
			steps_color: Color32::from_rgb(129, 210, 92),
			track_color: Color32::from_rgb(197, 181, 159),
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
