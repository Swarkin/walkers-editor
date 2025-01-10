pub mod visualizers;
mod consts;
mod attribute2d;

use consts::*;
use eframe::egui::{Pos2, Response, Ui};
use osm_parser::*;
use visualizers::Visualization;
use walkers::{Plugin, Position, Projector};

pub struct EditorPlugin<'a> {
	pub state: &'a mut EditorPluginState,
	pub osm_data: &'a OsmData,
	#[allow(clippy::borrowed_box)]
	pub visualization: Visualization,
	pub scale_factor: f32,
}

#[derive(Default)]
pub struct EditorPluginState {
	pub hovered: Option<Id>,
	pub selected: Option<Id>,
}

impl Plugin for EditorPlugin<'_> {
	fn run(self: Box<Self>, ui: &mut Ui, response: &Response, projector: &Projector) {
		let mut dist_to_feature = INTERACTION_RANGE * self.scale_factor;
		let mut hovering_feature_id = Option::<Id>::None;

		for way in self.osm_data.ways.values() {
			let mut iter = way.nodes.iter().peekable();

			while let Some(curr_id) = iter.next() {
				if let Some(next_id) = iter.peek() {
					let p1 = projector.project(coordinate_to_pos(&self.osm_data.nodes[curr_id].pos)).to_pos2();
					let p2 = projector.project(coordinate_to_pos(&self.osm_data.nodes[next_id].pos)).to_pos2();

					if let Some(mouse) = response.hover_pos() {
						let mouse_dist = distance_to_segment(mouse, &p1, &p2);
						if mouse_dist < dist_to_feature {
							dist_to_feature = mouse_dist;
							hovering_feature_id = Some(way.id);
						}
					}
					
					let width = visualizers::determine_width_default(way) * self.scale_factor;
					let color = visualizers::determine_color_default(way);
					
					ui.painter().extend(
						match self.visualization {
							Visualization::Default => visualizers::default([p1, p2], color, width),
							Visualization::Sidewalks => visualizers::sidewalks(way, [p1, p2], color, width),
						}
					);				
				}
			}
		}
		
		if let Some(id) = hovering_feature_id {
			let way = &self.osm_data.ways[&id];
			let mut iter = way.nodes.iter().peekable();
			
			while let Some(curr_id) = iter.next() {
				if let Some(next_id) = iter.peek() {
					let p1 = projector.project(coordinate_to_pos(&self.osm_data.nodes[curr_id].pos)).to_pos2();
					let p2 = projector.project(coordinate_to_pos(&self.osm_data.nodes[next_id].pos)).to_pos2();

					let width = visualizers::determine_width_default(way) * self.scale_factor + SELECTION_SIZE_INCREASE;
					ui.painter().extend(
						visualizers::default([p1, p2], SELECTION_COLOR, width)
					);
				}
			}

			self.state.hovered = Some(id);
			self.state.selected = Some(id);
		} else {
			self.state.hovered = None;
			if response.clicked() { self.state.selected = None; }
		}
	}
}

pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::from_lon_lat(c.lon, c.lat)
}

fn distance_to_segment(p: Pos2, x: &Pos2, y: &Pos2) -> f32 {
	let a = p.x - x.x;
	let b = p.y - x.y;
	let c = y.x - x.x;
	let d = y.y - x.y;

	let dot = a * c + b * d;
	let len_sq = c * c + d * d;
	let mut param = -1f32;
	if len_sq != 0f32 {
		param = dot / len_sq;
	}

	let xx;
	let yy;

	if param < 0f32 {
		xx = x.x;
		yy = x.y;
	}
	else if param > 1f32 {
		xx = y.x;
		yy = y.y;
	}
	else {
		xx = x.x + param * c;
		yy = x.y + param * d;
	}

	let dx = p.x - xx;
	let dy = p.y - yy;
	(dx * dx + dy * dy).sqrt()
}
