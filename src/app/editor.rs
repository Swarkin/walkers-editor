pub mod visual;
mod consts;
mod attribute2d;

use consts::*;
use eframe::egui::{Pos2, Response, Shape, Ui};
use eframe::epaint::PathStroke;
use osm_parser::*;
use visual::Visualization;
use walkers::{Plugin, Position, Projector};

// data received every frame
pub struct EditorPlugin<'a> {
	pub state: &'a mut EditorPluginState,
	pub osm_data: &'a OsmData,
	pub visualization: Visualization,
	pub scale_factor: f32,
}

// data produced every frame
#[derive(Default)]
pub struct EditorPluginState {
	pub hovered: Option<Id>,
	pub selected: Option<Id>,
	pub edit_window_pos: Option<Pos2>,
}

impl Plugin for EditorPlugin<'_> {
	// todo: use Shape::Path to draw lines
	fn run(self: Box<Self>, ui: &mut Ui, resp: &Response, projector: &Projector) {
		let mut shapes_top = Vec::with_capacity(2);
		self.state.hovered = None;

		for way in self.osm_data.ways.values() {
			for v in way.nodes.windows(2) {
				let points = [
					projector.project(coordinate_to_pos(&self.osm_data.nodes[&v[0]].pos)).to_pos2(),
					projector.project(coordinate_to_pos(&self.osm_data.nodes[&v[1]].pos)).to_pos2(),
				];

				let width = visual::determine_width_default(way) * self.scale_factor;
				let color = visual::determine_color_default(way);

				// detect mouse hover
				if let Some(mouse) = resp.hover_pos() {
					let mouse_dist = distance_to_segment(mouse, points);
					if mouse_dist < width {
						self.state.hovered = Some(way.id);
					}
				}

				// draw osm data based on selected visualization method
				let shapes = match self.visualization {
					Visualization::Default => visual::default(points, color, width),
					Visualization::Sidewalks => {
						if let Some(mouse) = resp.hover_pos() {
							if self.state.selected == Some(way.id) && distance_to_segment(mouse, points) < width {
                                self.state.edit_window_pos = Some(mouse);
                            }
						}

						visual::sidewalks(way, points, color, width)
					},
				};

				// draw selection
				if self.state.selected == Some(way.id) {
					shapes_top.push(Shape::LineSegment {
						points,
						stroke: PathStroke::new(width + SELECTION_SIZE_INCREASE, SELECTION_COLOR),
					});
				}

				// submit shapes
				ui.painter().extend(shapes);
			}
		}

		// display editing window
		if let Some(pos) = self.state.edit_window_pos {
			let window_open = match self.visualization {
				Visualization::Sidewalks => visual::sidewalks_ui(ui, pos),
				_ => false,
			};

			if !window_open {
				self.state.edit_window_pos = None;
			}
		}

		// draw hovered way
		if let Some(hover) = self.state.hovered {
			let way = &self.osm_data.ways[&hover];

			for v in way.nodes.windows(2) {
				let p1 = projector.project(coordinate_to_pos(&self.osm_data.nodes[&v[0]].pos)).to_pos2();
				let p2 = projector.project(coordinate_to_pos(&self.osm_data.nodes[&v[1]].pos)).to_pos2();
				let width = visual::determine_width_default(way) * self.scale_factor + HOVER_SIZE_INCREASE;
				
				shapes_top.extend(
					visual::default([p1, p2], HOVER_COLOR, width)
				);
			}

			if resp.clicked() && way_is_relevant(&way.tags, self.visualization) { self.state.selected = Some(hover); }
		} else if resp.clicked() { self.state.selected = None; }
		
		// submit priority shapes
		ui.painter().extend(shapes_top);

	}
}

fn way_is_relevant(tags: &Tags, visualization: Visualization) -> bool {
	match visualization {
		Visualization::Default => true,
		Visualization::Sidewalks => visual::sidewalks_relevant(tags),
	}
}

pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::from_lon_lat(c.lon, c.lat)
}

fn distance_to_segment(p: Pos2, points: [Pos2; 2]) -> f32 {
	let x = points[0];
	let y = points[1];

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
