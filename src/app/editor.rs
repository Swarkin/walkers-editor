pub mod visual;
mod consts;
mod attribute2d;

use consts::*;
use eframe::egui::{Color32, Pos2, Response, Shape, Ui};
use eframe::epaint::{PathShape, PathStroke};
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
	pub map_bbox: (f64, f64, f64, f64),
	pub last_click_coords: Position, // depends on https://github.com/podusowski/walkers/issues/246
}

impl Plugin for EditorPlugin<'_> {
	fn run(self: Box<Self>, ui: &mut Ui, resp: &Response, projector: &Projector) {
		let mut shapes_top = Vec::with_capacity(2);
		self.state.hovered = None;

		if resp.clicked() {
			let pos = resp.interact_pointer_pos().unwrap() - resp.rect.center();
			self.state.last_click_coords = projector.unproject(pos);
		}

		for way in self.osm_data.ways.values() {
			let points = self.project_way_to_points(way, projector);
			let width = self.way_width(way);
			let color = self.way_color(way);

			// detect hover
			if self.state.hovered.is_none() {
				if let Some(mouse) = resp.hover_pos() {
					if points.windows(2).any(|p| distance_to_segment(mouse, &[p[0], p[1]]) < width) {
						self.state.hovered = Some(way.id);
					}
				}
			}

			// draw way using selected visualization
			let shapes = match self.visualization {
				Visualization::Default => visual::default(points, color, width),
				Visualization::Sidewalks => visual::sidewalks(way, points, color, width),
			};

			ui.painter().extend(shapes);
		}

		// draw hovered way and handle logic
		if let Some(hover) = self.state.hovered {
			let way = &self.osm_data.ways[&hover];
			let points = self.project_way_to_points(way, projector);

			shapes_top.push(
				Shape::Path(PathShape::line(
					points, PathStroke::new(self.way_width(way) + HOVER_SIZE_INCREASE, HOVER_COLOR)
				))
			);

			if resp.clicked() {
				if self.is_way_relevant(&way.tags) {
					self.state.selected = Some(hover);
				} else { // deselect when clicking irrelevant object
					self.state.selected = None;
				}
			}
		} else if resp.clicked() { // discard hovered way
			self.state.selected = None;
		}

		// draw selected way
		if let Some(selected) = self.state.selected {
			let way = &self.osm_data.ways[&selected];
			let points = self.project_way_to_points(way, projector);

			shapes_top.push(
				Shape::Path(PathShape::line(
					points,
					PathStroke::new(self.way_width(way) + SELECTION_SIZE_INCREASE, SELECTION_COLOR),
				))
			)
		}

		ui.painter().extend(shapes_top);

		// update state.map_bbox
		let tl = projector.unproject(resp.rect.min.to_vec2());
		let br = projector.unproject(resp.rect.max.to_vec2());
		let left = tl.lon();
		let bottom = br.lat();
		let right = br.lon();
		let top = tl.lat();
		self.state.map_bbox = (left, bottom, right, top);

		// draw editing ui
		if let Some(selected) = self.state.selected {
			if self.is_way_relevant(&self.osm_data.ways[&selected].tags) {
				self.display_editing_ui(ui, projector.project(self.state.last_click_coords).to_pos2());
			}
		}
	}
}

impl EditorPlugin<'_> {
	fn way_width(&self, way: &Way) -> f32 {
		match self.visualization {
			Visualization::Default => visual::width_default(way) * self.scale_factor,
			Visualization::Sidewalks => visual::width_sidewalk(way) * self.scale_factor,
		}
	}

	fn way_color(&self, way: &Way) -> Color32 {
		match self.visualization {
			Visualization::Default => visual::color_default(way),
			Visualization::Sidewalks => visual::color_sidewalk(way),
		}
	}

	fn project_way_to_points(&self, way: &Way, projector: &Projector) -> Vec<Pos2> {
		way.nodes.iter()
			.map(|id| &self.osm_data.nodes[id])
			.map(|n| projector.project(coordinate_to_pos(&n.pos)).to_pos2())
			.collect()
	}

	fn is_way_relevant(&self, tags: &Tags) -> bool {
		match self.visualization {
			Visualization::Default => true,
			Visualization::Sidewalks => visual::sidewalks_relevant(tags),
		}
	}

	fn display_editing_ui(&self, ui: &mut Ui, pos: Pos2) {
		match self.visualization {
			Visualization::Default => return,
			Visualization::Sidewalks => visual::sidewalks_ui(ui, pos),
		};
	}
}


pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::from_lon_lat(c.lon, c.lat)
}

fn distance_to_segment(p: Pos2, points: &[Pos2; 2]) -> f32 {
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
