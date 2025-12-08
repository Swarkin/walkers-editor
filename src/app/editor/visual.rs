use super::attribute2d::{Attribute2D, TagValue};
use super::cache::Change;
use super::consts::osm::*;
use crate::app::editor::consts::{prepare_icon, prepare_icon_with_tint};
use crate::app::editor::consume_key;
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

pub fn width_default(w: &Way) -> f32 {
	w.tags.get("building").map_or_else(
		|| w.tags.get("highway")
			.map_or(WAY_WIDTH, |highway| match highway.as_str() {
				"path" | "footway" | "steps" => PATH_WIDTH,
				"service" | "track" => SERVICE_ROAD_WIDTH,
				"residential" => MINOR_ROAD_WIDTH,
				"tertiary" | "secondary" | "primary" | "trunk" | "motorway" | "tertiary_link"
				| "secondary_link" | "primary_link" | "trunk_link" | "motorway_link" => MAJOR_ROAD_WIDTH,
				_ => WAY_WIDTH,
			}),
		|building| match building.as_str() {
			"no" => WAY_WIDTH,
			_ => BUILDING_WIDTH,
		}
	)
}

pub fn color_default(w: &Way) -> Color32 {
	w.tags.get("building").map_or_else(
		|| w.tags.get("highway")
			.map_or(WAY_COLOR, |highway| match highway.as_str() {
				"path" => PATH_COLOR,
				"footway" => FOOTWAY_COLOR,
				"steps" => STEPS_COLOR,
				"track" => TRACK_COLOR,
				_ => Color32::WHITE,
			}),
		|building| match building.as_str() {
			"no" => WAY_COLOR,
			_ => BUILDING_COLOR,
		}
	)
}

pub fn sidewalks(tags: &Tags, points: &[Pos2], width: f32, scale_factor: f32) -> [Shape; 2] {
	let attr = Attribute2D::new(tags, "sidewalk");
	let mut iter = points.windows(2).peekable();
	let count = iter.len() + 1;

	let mut path_left = PathShape::line(Vec::with_capacity(count), Stroke::new(SIDEWALK_WIDTH * scale_factor, attr.left));
	let mut path_right = PathShape::line(Vec::with_capacity(count), Stroke::new(SIDEWALK_WIDTH + scale_factor, attr.right));

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

pub fn sidewalks_relevant(tags: &Tags) -> bool {
	tags.get("highway")
		.is_some_and(|highway| HIGHWAYS_WITH_SIDEWALK.contains(&highway.as_str()))
}

pub fn sidewalks_ui(ui: &Ui, way: &Way, pos: Pos2) -> Option<Change> {
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
					attribute2d_selectable_value(ui, &mut attr.left, true, 2)
				).inner { edited = true; }
				ui.separator();
				if ui.vertical(|ui|
					attribute2d_selectable_value(ui, &mut attr.right, false, 2)
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

fn attribute2d_selectable_value(ui: &mut Ui, current: &mut TagValue, flip: bool, buttons_per_row: u8) -> bool {
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
			let color = (*tag_value).into();
			let mut image = prepare_icon(ui.ctx(), icon_bg.clone(), 48.);

			if flip {
				image = image.rotate(std::f32::consts::TAU / 2., Vec2::splat(0.5));
			}

			let overlay = prepare_icon_with_tint(icon_fg.clone(), 16., color);

			sidewalk_overlay_button(ui, current, *tag_value, image, &overlay, flip);
			if consume_key(ui.ctx(), if flip { key.0 } else { key.1 }, Modifiers::NONE) { *current = *tag_value; }

			button_i += 1;
			if button_i.is_multiple_of(buttons_per_row) {
				ui.end_row();
			}
		}
	});

	original != *current
}

fn sidewalk_overlay_button(ui: &mut Ui, current: &mut TagValue, new: TagValue, image: Image, overlay: &Image, flip: bool) {
	ui.visuals_mut().selection.bg_fill = new.into();

	let resp = egui::Button::image(image)
		.min_size(Vec2::splat(56.))
		.stroke(Stroke::new(2., new))
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
