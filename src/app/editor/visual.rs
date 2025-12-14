use super::attribute2d::{Attribute2D, TagValue};
use super::cache::Change;
use super::consts::osm::*;
use crate::app::editor::consts::{prepare_icon, prepare_icon_with_tint};
use crate::app::editor::{consume_key, theme};
use crate::app::icons;
use eframe::egui;
use eframe::egui::{Image, ImageSource, Key, Modifiers, Rect, Vec2, Widget};
use eframe::epaint::{PathShape, Stroke};
use egui::{Color32, Pos2, Shape, Ui, Window};
use osm_parser::types::merge_tags;
use osm_parser::{Tags, Way};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Visualization {
	#[default] Default,
	Sidewalks,
}

impl Visualization {
	pub const ITER: [Self; 2] = [Self::Default, Self::Sidewalks];
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
	Wireframe,
	#[default] Partial,
	Full,
}

impl FillMode {
	pub const ITER: [Self; 3] = [Self::Full, Self::Partial, Self::Wireframe];
}

pub const HIGHWAYS_WITH_SIDEWALK: &[&str; 15] = &[
	UNCLASSIFIED, RESIDENTIAL, LIVING_STREET, PEDESTRIAN, SERVICE,
	MOTORWAY, TRUNK, PRIMARY, SECONDARY, TERTIARY,
	MOTORWAY_LINK, TRUNK_LINK, PRIMARY_LINK, SECONDARY_LINK, TERTIARY_LINK,
];

#[expect(clippy::option_if_let_else)]
pub fn width_default(theme: &theme::Theme, w: &Way) -> f32 {
	if let Some(building) = w.tags.get("building") {
		match building.as_str() {
			"no" => theme.way_width,
			_ => theme.building_width,
		}
	} else if let Some(highway) = w.tags.get("highway") {
		match highway.as_str() {
				"path" | "footway" | "steps" => theme.path_width,
				"service" | "track" => theme.service_road_width,
				"residential" => theme.minor_road_width,
			"tertiary" | "secondary" | "primary" | "trunk" | "motorway"
			| "tertiary_link" | "secondary_link" | "primary_link" | "trunk_link" | "motorway_link" =>
				theme.major_road_width,
				_ => theme.way_width,
		}
	} else {
		theme.way_width
	}
}

#[expect(clippy::option_if_let_else)]
pub fn color_default(theme: &theme::Theme, w: &Way) -> Color32 {
	if let Some(t) = w.tags.get("building") {
		match t.as_str() {
			"no" => theme.way_color,
			_ => theme.building_color,
		}
	} else if let Some(t) = w.tags.get("highway") {
		match t.as_str() {
			"path" => theme.highway_path_color,
			"footway" => theme.highway_footway_color,
			"steps" => theme.highway_steps_color,
			"track" => theme.highway_track_color,
			_ => Color32::WHITE,
		}
	} else if let Some(t) = w.tags.get("landuse") {
		match t.as_str() {
			"farmland" => theme.landuse_farmland_color,
			"residential" => theme.landuse_residential_color,
			"forest" => theme.landuse_forest_color,
			"grass" => theme.landuse_grass_color,
			"commercial" => theme.landuse_commercial_color,
			_ => theme.way_color,
		}
	} else if let Some(t) = w.tags.get("natural") {
		match t.as_str() {
			"water" => theme.natural_water_color,
			"wood" => theme.natural_wood_color,
			"scrub" => theme.natural_scrub_color,
			"wetland" => theme.natural_wetland_color,
			"grassland" => theme.natural_grassland_color,
			_ => theme.way_color,
		}
	} else {
		theme.way_color
	}
}

pub fn sidewalks(theme: &theme::Theme, tags: &Tags, points: &[Pos2], width: f32, scale_factor: f32) -> [Shape; 2] {
	let attr = Attribute2D::new(tags, "sidewalk");
	let mut iter = points.windows(2).peekable();
	let count = iter.len() + 1;

	let mut path_left = PathShape::line(Vec::with_capacity(count), Stroke::new(theme.sidewalk_width * scale_factor, tagvalue_to_color(theme, attr.left)));
	let mut path_right = PathShape::line(Vec::with_capacity(count), Stroke::new(theme.sidewalk_width + scale_factor, tagvalue_to_color(theme, attr.right)));

	/* first point */ {
		let from = points[0];
		let to = points[1];
		let orthogonal = (to - from).normalized().rot90();
		let offset = orthogonal * width;

		path_left.points.push(from + offset);
		path_right.points.push(from - offset);
	}

	while let Some(points) = iter.next() {
		let from = points[0];
		let to = points[1];
		let mut orthogonal = (to - from).rot90();

		if let Some(points) = iter.peek() {
			let from = points[0];
			let to = points[1];
			let orthogonal_next = (to - from).rot90();

			orthogonal += orthogonal_next;
		}

		orthogonal = orthogonal.normalized();

		path_left.points.push(to + orthogonal * width);
		path_right.points.push(to - orthogonal * width);
	}

	debug_assert!(path_left.points.len() == count && path_right.points.len() == count);
	[path_left.into(), path_right.into()]
}

const fn tagvalue_to_color(theme: &theme::Theme, value: TagValue) -> Color32 {
	match value {
		TagValue::Yes => theme.sidewalk_yes_color,
		TagValue::No => theme.sidewalk_no_color,
		TagValue::Separate => theme.sidewalk_separate_color,
		TagValue::Unknown => theme.sidewalk_unknown_color,
	}
}

pub fn sidewalks_relevant(tags: &Tags) -> bool {
	tags.get("highway")
		.is_some_and(|highway| HIGHWAYS_WITH_SIDEWALK.contains(&highway.as_str()))
}

pub fn sidewalks_ui(ui: &Ui, theme: &theme::Theme, way: &Way, pos: Pos2) -> Option<Change> {
	const TAG: &str = "sidewalk";
	const TAG_LEFT: &str = "sidewalk:left";
	const TAG_RIGHT: &str = "sidewalk:right";
	const TAG_BOTH: &str = "sidewalk:both";
	let mut edited = false;

	Window::new("Sidewalks")
		.current_pos(pos)
		.title_bar(false)
		.resizable(false)
		.movable(false)
		.show(ui.ctx(), |ui| {
			let mut attr = Attribute2D::new(&way.tags, TAG);

			ui.horizontal(|ui| {
				if ui.vertical(|ui|
					attribute2d_selectable_value(ui, theme, &mut attr.left, true, 2)
				).inner { edited = true; }
				ui.separator();
				if ui.vertical(|ui|
					attribute2d_selectable_value(ui, theme, &mut attr.right, false, 2)
				).inner { edited = true; }
			});

			if edited {
				let mut new_way = way.clone();
				let sidewalk_tags = attr.into_tags(TAG);

				new_way.tags.remove(TAG);
				new_way.tags.remove(TAG_LEFT);
				new_way.tags.remove(TAG_RIGHT);
				new_way.tags.remove(TAG_BOTH);

				merge_tags(&mut new_way.tags, sidewalk_tags);
				Some(Change::ModifyWay(new_way.id, new_way))
			} else { None }
		})?.inner?
}

fn attribute2d_selectable_value(ui: &mut Ui, theme: &theme::Theme, current: &mut TagValue, flip: bool, buttons_per_row: u8) -> bool {
	const DATA: &[(TagValue, ImageSource, ImageSource, (Key, Key))] = &[
		(TagValue::Yes, icons::SIDEWALK_YES, icons::MISC_CHECK, (Key::Num1, Key::Q)),
		(TagValue::No, icons::SIDEWALK_NO, icons::MISC_CROSS, (Key::Num2, Key::W)),
		(TagValue::Separate, icons::SIDEWALK_SEPARATE, icons::MISC_ARROW, (Key::Num3, Key::E)),
		(TagValue::Unknown, icons::SIDEWALK_UNKNOWN, icons::MISC_QUESTION_MARK, (Key::Num4, Key::R)),
	];

	let original = *current;
	let mut button_i = 0u8;

	let uid = *current as u8 + u8::from(flip) * 69;
	egui::Grid::new(uid).show(ui, |ui| {
		for (tag_value, icon_bg, icon_fg, key) in DATA {
			let color = tagvalue_to_color(theme, *tag_value);
			let mut image = prepare_icon(ui.ctx(), icon_bg.clone(), 48.);

			if flip {
				image = image.rotate(std::f32::consts::TAU / 2., Vec2::splat(0.5));
			}

			let overlay = prepare_icon_with_tint(icon_fg.clone(), 16., color);

			sidewalk_overlay_button(ui, theme, current, *tag_value, image, &overlay, flip);
			if consume_key(ui.ctx(), if flip { key.0 } else { key.1 }, Modifiers::NONE) { *current = *tag_value; }

			button_i += 1;
			if button_i.is_multiple_of(buttons_per_row) {
				ui.end_row();
			}
		}
	});

	original != *current
}

fn sidewalk_overlay_button(ui: &mut Ui, theme: &theme::Theme, current: &mut TagValue, new: TagValue, image: Image, overlay: &Image, flip: bool) {
	let color = tagvalue_to_color(theme, new);
	ui.visuals_mut().selection.bg_fill = color;

	let resp = egui::Button::image(image)
		.min_size(Vec2::splat(56.))
		.stroke(Stroke::new(2., color))
		.selected(*current == new)
		.ui(ui);

	let center = if flip {
		resp.rect.left_center() + Vec2::new(14., 0.)
	} else {
		resp.rect.right_center() - Vec2::new(13., 0.)
	};

	overlay.paint_at(ui, Rect::from_center_size(center, Vec2::splat(16.)));

	if resp.clicked() {
		*current = new;
	}
}
