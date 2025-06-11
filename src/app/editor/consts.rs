pub mod osm;

use eframe::egui::{Color32, Context, Image, ImageSource, Vec2};

pub const TOP_BAR_HEIGHT: f32 = 37.0;
pub const TOP_BAR_FONT_SIZE: f32 = 14.0;
pub const TOP_BAR_BUTTON_SIZE: f32 = 28.0;
pub const TOP_BAR_ICON_SIZE: f32 = 24.0;

const TINT_DARK: u8 = 222;
const TINT_LIGHT: u8 = 22;

pub const WINDOW_MARGIN: f32 = 8.0;

pub const PARTIAL_FILL_WIDTH: f32 = 12.0;
pub const PARTIAL_FILL_GAMMA_MULTIPLY: f32 = 0.5;
pub const PARTIAL_FILL_THRESHOLD: f64 = 18.0;

//region sidewalk overlay
pub const SIDEWALK_YES_COLOR: Color32 = Color32::LIGHT_GREEN;
pub const SIDEWALK_NO_COLOR: Color32 = Color32::LIGHT_GRAY;
pub const SIDEWALK_SEPARATE_COLOR: Color32 = Color32::LIGHT_BLUE;
pub const SIDEWALK_UNKNOWN_COLOR: Color32 = Color32::LIGHT_RED;
//endregion

fn tint(dark: bool) -> u8 {
	if dark { TINT_DARK } else { TINT_LIGHT }
}

pub fn load_icon<'a>(ctx: &Context, img: ImageSource<'a>, size: f32) -> Image<'a> {
	Image::new(img)
		.tint(Color32::from_gray(tint(ctx.style().visuals.dark_mode)))
		.fit_to_exact_size(Vec2::splat(size))
}
