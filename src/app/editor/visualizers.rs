use super::attribute2d::Attribute2D;
use super::consts::osm::*;
use super::consts::*;
use eframe::egui;
use eframe::epaint::PathStroke;
use egui::{Color32, Painter, Pos2, Vec2};
use osm_parser::Way;
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visualization {
	#[default] Default,
	Sidewalks,
}

#[allow(unused_variables)]
pub trait Visualizer {
	fn paint(&self, painter: &Painter, w: &Way, width: f32, color: Color32, points: [Pos2; 2]) {
		painter.line_segment(points, PathStroke::new(width, color));
	}

	fn determine_width(&self, w: &Way) -> f32 {
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

	fn determine_color(&self, w: &Way) -> Color32 {
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
	
	fn can_select(&self, w: &Way) -> bool { true }
}


struct Default {}

impl Visualizer for Default {}


// highlighted features, inspired by StreetComplete
pub const HIGHWAYS_WITH_SIDEWALK: &[&str; 15] = &[
	UNCLASSIFIED, RESIDENTIAL, LIVING_STREET, PEDESTRIAN, SERVICE,
	MOTORWAY, TRUNK, PRIMARY, SECONDARY, TERTIARY,
	MOTORWAY_LINK, TRUNK_LINK, PRIMARY_LINK, SECONDARY_LINK, TERTIARY_LINK,
];

struct Sidewalks {}

impl Visualizer for Sidewalks {
	fn paint(&self, painter: &Painter, w: &Way, width: f32, color: Color32, points: [Pos2; 2]) {
		painter.line_segment(points, PathStroke::new(width, color));

		let start = points[0];
		let end = points[1];
		let distance = 5.0;

		let between = (end - start).normalized();           // difference and normalize
		let orthogonal = Vec2::new(between.y, -between.x);  // flip x and y and make one negative
		let offset = orthogonal * distance;                 // apply offset and distance

		if let Some(tags) = &w.tags {
			if let Some(highway) = tags.get("highway") {
				if !HIGHWAYS_WITH_SIDEWALK.contains(&highway.as_str()) { return };

				let attr = Attribute2D::new(tags, "sidewalk");
				painter.line_segment([start + offset, end + offset], PathStroke::new(width * 0.8, attr.left));   // left
				painter.line_segment([start - offset, end - offset], PathStroke::new(width * 0.8, attr.right));  // right
			}
		};
	}

	fn can_select(&self, w: &Way) -> bool {
		if let Some(tags) = &w.tags {
			if let Some(highway) = tags.get("highway") {
				return HIGHWAYS_WITH_SIDEWALK.contains(&highway.as_str())
			}
		}
		false
	}
}


pub fn visualizers() -> HashMap<Visualization, Box<dyn Visualizer>> {
	let mut visualizers: HashMap<Visualization, Box<dyn Visualizer>> = HashMap::default();
	
	visualizers.insert(
		Visualization::Default,
		Box::new(Default {}),
	);
	
	visualizers.insert(
		Visualization::Sidewalks,
		Box::new(Sidewalks {}),
	);
	
	visualizers
}
