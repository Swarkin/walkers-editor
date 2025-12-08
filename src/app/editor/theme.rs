use eframe::egui;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Theme {
	pub theme: ThemeSetting,
	pub sidewalk_width: f32,
	pub sidewalk_yes_color: Color32,
	pub sidewalk_no_color: Color32,
	pub sidewalk_separate_color: Color32,
	pub sidewalk_unknown_color: Color32,
	// todo: move all constants here
}

impl Default for Theme {
	fn default() -> Self {
		Self {
			theme: Default::default(),
			sidewalk_width: 4.0,
			sidewalk_yes_color: Color32::LIGHT_GREEN,
			sidewalk_no_color: Color32::LIGHT_GRAY,
			sidewalk_separate_color: Color32::LIGHT_BLUE,
			sidewalk_unknown_color: Color32::LIGHT_RED,
		}
	}
}

#[derive(Default, Clone, Serialize, Deserialize)]
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
