use super::attribute2d::{Attribute2D, TagValue};
use super::cache::Change;
use super::consts::osm::*;
use eframe::egui;
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

pub const SIDEWALK_WIDTH: f32 = 4.0;
pub const SIDEWALK_YES_COLOR: Color32 = Color32::LIGHT_GREEN;
pub const SIDEWALK_NO_COLOR: Color32 = Color32::LIGHT_GRAY;
pub const SIDEWALK_SEPARATE_COLOR: Color32 = Color32::LIGHT_BLUE;
pub const SIDEWALK_UNKNOWN_COLOR: Color32 = Color32::LIGHT_RED;

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
				ui.vertical(|ui| {
					ui.strong(format!("Left: {:?}", attr.left));
					if attribute2d_selectable_value(ui, &mut attr.left) { edited = true; }
				});
				ui.vertical(|ui| {
					ui.strong(format!("Right: {:?}", attr.right));
					if attribute2d_selectable_value(ui, &mut attr.right) { edited = true; }
				});
			});

			if edited {
				let mut new_way = way.clone();
				let sidewalk_tags = attr.into_tags(TAG);

				new_way.tags.remove(TAG);
				new_way.tags.remove(TAG_LEFT);
				new_way.tags.remove(TAG_RIGHT);
				new_way.tags.remove(TAG_BOTH);

				merge_tags(&mut new_way.tags, sidewalk_tags);
				Some(Change::UpdateWay(new_way.id, new_way))
			} else { None }
		})?.inner?
}

fn attribute2d_selectable_value(ui: &mut Ui, selected: &mut TagValue) -> bool {
	let original = *selected;
	ui.selectable_value(selected, TagValue::Yes, format!("{:?}", TagValue::Yes));
	ui.selectable_value(selected, TagValue::No, format!("{:?}", TagValue::No));
	ui.selectable_value(selected, TagValue::Separate, format!("{:?}", TagValue::Separate));
	ui.selectable_value(selected, TagValue::Unknown, format!("{:?}", TagValue::Unknown));
	original != *selected
}
