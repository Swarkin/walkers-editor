use super::attribute2d::Attribute2D;
use super::consts::osm::*;
use super::consts::*;
use eframe::egui;
use eframe::epaint::{PathShape, PathStroke};
use egui::{Color32, Pos2, Shape, Window};
use osm_parser::Way;

#[derive(Debug, Default, Clone, Copy)]
#[derive(PartialEq)]
pub enum Visualization {
	#[default] Default,
	Sidewalks,
}

pub const HIGHWAYS_WITH_SIDEWALK: &[&str; 15] = &[
	UNCLASSIFIED, RESIDENTIAL, LIVING_STREET, PEDESTRIAN, SERVICE,
	MOTORWAY, TRUNK, PRIMARY, SECONDARY, TERTIARY,
	MOTORWAY_LINK, TRUNK_LINK, PRIMARY_LINK, SECONDARY_LINK, TERTIARY_LINK,
];

pub fn width_default(w: &Way) -> f32 {
	if let Some(building) = w.tags.get("building") {
		return match building.as_str() {
			"no" => DEFAULT_WIDTH,
			_ => BUILDING_WIDTH,
		}
	} else if let Some(highway) = w.tags.get("highway") {
		return match highway.as_str() {
			"path" | "footway" | "steps" => PATH_WIDTH,
			"service" | "track" => SERVICE_ROAD_WIDTH,
			"residential" => MINOR_ROAD_WIDTH,
			"tertiary" | "secondary" | "primary" | "trunk" | "motorway" |
			"tertiary_link" | "secondary_link" | "primary_link" | "trunk_link" | "motorway_link" => MAJOR_ROAD_WIDTH,
			_ => DEFAULT_WIDTH,
		}
	} else { DEFAULT_WIDTH }
}

pub fn width_sidewalk(w: &Way) -> f32 {
	width_default(w)
}

pub fn color_default(w: &Way) -> Color32 {
	if let Some(building) = w.tags.get("building") {
		return match building.as_str() {
			"no" => DEFAULT_COLOR,
			_ => BUILDING_COLOR,
		}
	} else if let Some(highway) = w.tags.get("highway") {
		return match highway.as_str() {
			"path" => PATH_COLOR,
			"footway" => FOOTWAY_COLOR,
			"steps" => STEPS_COLOR,
			"track" => TRACK_COLOR,
			_ => Color32::WHITE,
		}
	} else { DEFAULT_COLOR }
}

pub fn color_sidewalk(w: &Way) -> Color32 {
	color_default(w)
}

pub fn default(points: Vec<Pos2>, color: Color32, width: f32) -> Vec<Shape> {
	vec![Shape::Path(PathShape::line(
		points,
		PathStroke::new(width, color),
	))]
}

pub fn sidewalks(way: &Way, points: Vec<Pos2>, color: Color32, width: f32) -> Vec<Shape> {
	let mut shapes = vec![];

	if way.tags.keys().any(|k| k.starts_with("sidewalk")) {
		if !sidewalks_relevant(&way.tags) { return shapes; };
		let attr = Attribute2D::new(&way.tags, "sidewalk");

		for points in points.windows(2) {
			let from = points[0];
			let to = points[1];

			let orthogonal = (to - from).normalized().rot90();
			let offset = orthogonal * width;

			shapes.push(Shape::LineSegment {
				points: [from + offset, to + offset],
				stroke: PathStroke::new(width, attr.left),
			});
			shapes.push(Shape::LineSegment {
				points: [from - offset, to - offset],
				stroke: PathStroke::new(width, attr.right),
			});
		}
	}

	shapes.push(Shape::Path(PathShape::line(
		points,
		PathStroke::new(width, color),
	)));

	shapes
}

pub fn sidewalks_relevant(tags: &osm_parser::Tags) -> bool {
	if let Some(highway) = tags.get("highway") {
		HIGHWAYS_WITH_SIDEWALK.contains(&highway.as_str())
	} else { false }
}


pub fn sidewalks_ui(ui: &mut egui::Ui, pos: Pos2) -> bool {
	let mut open = true;
	Window::new("Sidewalks")
		.current_pos(pos)
		.title_bar(false)
		.resizable(false)
		.movable(false)
		.open(&mut open)
		.show(ui.ctx(), |ui| {
			// TODO: UI
			egui::Sides::new().spacing(0.0).height(32.0).show(ui,
				|ui| {
					ui.label("Left");
				},
				|ui| {
					ui.label("Right");
				},
			)
		});

	open
}
