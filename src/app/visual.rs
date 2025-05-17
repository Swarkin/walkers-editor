use eframe::egui;
use egui::{Color32, Context, Image, ImageSource, Vec2};

pub const TOP_BAR_ICON_SIZE: f32 = 24.0;
const TINT_DARK: u8 = 222;
const TINT_LIGHT: u8 = 22;

pub fn load_icon<'a>(ctx: &Context, img: ImageSource<'a>, size: f32) -> Image<'a> {
	Image::new(img)
		.tint(Color32::from_gray(tint(ctx.style().visuals.dark_mode)))
		.fit_to_exact_size(Vec2::splat(size))
}

fn tint(dark: bool) -> u8 {
	if dark { TINT_DARK } else { TINT_LIGHT }
}
