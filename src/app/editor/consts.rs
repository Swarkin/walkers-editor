pub mod osm;

use eframe::egui::Color32;

pub const TOP_BAR_HEIGHT: f32 = 37.0;
pub const TOP_BAR_FONT_SIZE: f32 = 14.0;
pub const TOP_BAR_BUTTON_SIZE: f32 = 28.0;
pub const TOP_BAR_ICON_SIZE: f32 = 24.0;

pub const HOVER_COLOR: Color32 = Color32::from_rgb(100, 200, 255);
pub const HOVER_SIZE_INCREASE: f32 = 1.0;
pub const SELECTION_COLOR: Color32 = Color32::from_rgb(40, 180, 255);
pub const SELECTION_SIZE_INCREASE: f32 = 2.0;
pub const DEFAULT_COLOR: Color32 = Color32::GRAY;

//region sidewalk overlay
pub const SIDEWALK_YES_COLOR: Color32 = Color32::LIGHT_GREEN;
pub const SIDEWALK_NO_COLOR: Color32 = Color32::LIGHT_GRAY;
pub const SIDEWALK_SEPARATE_COLOR: Color32 = Color32::LIGHT_BLUE;
pub const SIDEWALK_UNKNOWN_COLOR: Color32 = Color32::LIGHT_RED;
//endregion
