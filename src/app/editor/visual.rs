use super::attribute2d::Attribute2D;
use super::consts::osm::*;
use super::consts::*;
use eframe::egui;
use eframe::epaint::PathStroke;
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

pub fn determine_width_default(w: &Way) -> f32 {
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

pub fn determine_color_default(w: &Way) -> Color32 {
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


pub fn default(points: [Pos2; 2], color: Color32, width: f32) -> Vec<Shape> {
	vec![Shape::LineSegment {
		points,
		stroke: PathStroke::new(width, color),
	}]
}

pub fn sidewalks(way: &Way, points: [Pos2; 2], color: Color32, width: f32) -> Vec<Shape> {
	let mut shapes = Vec::with_capacity(3);

	shapes.push(Shape::LineSegment {
		points,
		stroke: PathStroke::new(width, color),
	});


	if way.tags.keys().any(|k| k.starts_with("sidewalk")) {
		if !sidewalks_relevant(&way.tags) { return shapes; };
		let attr = Attribute2D::new(&way.tags, "sidewalk");

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

		return shapes;
	}


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
		.default_pos(pos)
		.open(&mut open)
		.show(ui.ctx(), |ui| {
			// TODO: UI
			ui.label("TODO");
		});

	open
}
