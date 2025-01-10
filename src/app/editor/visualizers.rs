use super::attribute2d::Attribute2D;
use super::consts::osm::*;
use super::consts::*;
use eframe::egui;
use eframe::epaint::PathStroke;
use egui::{Color32, Pos2, Shape, Vec2};
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
	if let Some(tags) = &w.tags {
		if let Some(building) = tags.get("building") {
			return match building.as_str() {
				"no" => DEFAULT_WIDTH,
				_ => BUILDING_WIDTH,
			}
		} else if let Some(highway) = tags.get("highway") {
			return match highway.as_str() {
				"path" | "footway" | "steps" => PATH_WIDTH,
				"service" | "track" => SERVICE_ROAD_WIDTH,
				"residential" => MINOR_ROAD_WIDTH,
				"tertiary" | "secondary" | "primary" | "trunk" | "motorway" |
				"tertiary_link" | "secondary_link" | "primary_link" | "trunk_link" | "motorway_link" => MAJOR_ROAD_WIDTH,
				_ => DEFAULT_WIDTH,
			}
		}
	}
	DEFAULT_WIDTH
}

pub fn determine_color_default(w: &Way) -> Color32 {
	if let Some(tags) = &w.tags {
		if let Some(building) = tags.get("building") {
			return match building.as_str() {
				"no" => DEFAULT_COLOR,
				_ => BUILDING_COLOR,
			}
		} else if let Some(highway) = tags.get("highway") {
			return match highway.as_str() {
				"path" => PATH_COLOR,
				"footway" => FOOTWAY_COLOR,
				"steps" => STEPS_COLOR,
				"track" => TRACK_COLOR,
				_ => Color32::WHITE,
			}
		}
	}
	DEFAULT_COLOR
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

	if let Some(tags) = &way.tags {
		if let Some(highway) = tags.get("highway") {
			if !HIGHWAYS_WITH_SIDEWALK.contains(&highway.as_str()) { return shapes; };

			let start = points[0];
			let end = points[1];
			let distance = width;

			let between = (end - start).normalized();           // difference and normalize
			let orthogonal = Vec2::new(between.y, -between.x);  // flip x and y and make one negative
			let offset = orthogonal * distance;                 // apply offset and distance

			let attr = Attribute2D::new(tags, "sidewalk");

			shapes.push(Shape::LineSegment {
				points: [start + offset, end + offset],
				stroke: PathStroke::new(width, attr.left),
			});
			shapes.push(Shape::LineSegment {
				points: [start - offset, end - offset],
				stroke: PathStroke::new(width, attr.right),
			});
		}
	};

	shapes
}
