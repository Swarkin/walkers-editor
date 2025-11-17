pub mod osm;
pub mod shortcuts;

use eframe::egui::{Color32, Context, Image, ImageSource, Vec2};

pub const TOP_BAR_HEIGHT: f32 = 37.0;
pub const TOP_BAR_FONT_SIZE: f32 = 14.0;
pub const TOP_BAR_BUTTON_SIZE: Vec2 = Vec2::new(80.0, ICON_SIZE + 4.0);
pub const WIDE_BUTTON_SIZE: Vec2 = Vec2::new(160.0, ICON_SIZE + 6.0);

pub const ICON_SIZE: f32 = 24.0;

pub const EDIT_MODE_COLOR: Color32 = Color32::LIGHT_RED;
pub const VIEW_MODE_COLOR: Color32 = Color32::from_rgb(60, 160, 255);
pub const MODE_INDICATOR_WIDTH: f32 = 8.0;

pub const HOVER_TOOLTIP_OFFSET: Vec2 = Vec2::splat(16.0);
pub const HOVER_TOOLTIP_COLOR: Color32 = Color32::from_black_alpha(200);
pub const HOVER_TOOLTIP_FONT_SIZE: f32 = 14.0;

pub const MAX_DOWNLOAD_AREA: f64 = 0.0005;
pub const NODE_MIN_ZOOM: f64 = 17.0;

const TINT_DARK: u8 = 222;
const TINT_LIGHT: u8 = 22;

pub const WINDOW_MARGIN: f32 = 8.0;

pub const PARTIAL_FILL_WIDTH: f32 = 12.0;
pub const PARTIAL_FILL_GAMMA_MULTIPLY: f32 = 0.5;
pub const PARTIAL_FILL_THRESHOLD: f64 = 18.0;

pub const DOWNLOAD_FEEDBACK_SECONDS: f64 = 3.0;

const fn tint(dark: bool) -> u8 {
	if dark { TINT_DARK } else { TINT_LIGHT }
}

pub fn prepare_icon<'a>(ctx: &Context, img: ImageSource<'a>, size: f32) -> Image<'a> {
	Image::new(img)
		.tint(Color32::from_gray(tint(ctx.style().visuals.dark_mode)))
		.fit_to_exact_size(Vec2::splat(size))
}

pub fn prepare_icon_with_tint(img: ImageSource, size: f32, tint: Color32) -> Image {
	Image::new(img)
		.tint(tint)
		.fit_to_exact_size(Vec2::splat(size))
}
