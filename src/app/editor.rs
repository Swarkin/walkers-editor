pub mod visual;
pub mod changes;
pub mod consts;
pub mod attribute2d;
pub mod states;

use super::osm::Bbox;
use changes::*;
use consts::*;
use eframe::egui::{Color32, Pos2, Response, Shape, Stroke, Ui};
use eframe::epaint::{CircleShape, PathShape, PathStroke};
use osm_parser::*;
use states::SelectionMode;
use std::collections::HashMap;
use visual::Visualization;
use walkers::{Plugin, Position, Projector};

use crate::app::editor::consts::osm::DEFAULT_NODE_SIZE;
#[cfg(feature = "debug")]
use {super::DebugTimes, std::time::Instant};

type ProjectedPointsCache = HashMap<Id, Pos2>;

/// Data that is passed in every frame
pub struct EditorPlugin<'a> {
	pub state: &'a mut EditorPluginState,
	pub osm: &'a mut EditorOsmData,
	pub visualization: Visualization,
	pub selection_mode: SelectionMode,
	pub scale_factor: f32,
	pub regenerate_points: bool,
	#[cfg(feature = "debug")]
	pub debug_times: &'a mut DebugTimes,
}

/// Data that persists between frames
#[derive(Default)]
pub struct EditorPluginState {
	pub hovered: Option<Id>,
	pub selected: Option<Id>,
	pub map_bbox: Bbox,
	pub last_click_coords: Position,
	pub node_cache: ProjectedPointsCache,
}

impl Plugin for EditorPlugin<'_> {
	// todo(optimization): cache results of way_width and way_color
	fn run(mut self: Box<Self>, ui: &mut Ui, resp: &Response, projector: &Projector) {
		let mut shapes_top = Vec::with_capacity(2);
		self.state.hovered = None;

		/* determine last clicked position */ {
			if resp.clicked() {
				self.state.last_click_coords = projector.unproject(resp.interact_pointer_pos().unwrap().to_vec2());
			}
		}

		/* update state.map_bbox */ {
			let tl = projector.unproject(resp.rect.min.to_vec2() + resp.rect.center().to_vec2());
			let br = projector.unproject(resp.rect.max.to_vec2() + resp.rect.center().to_vec2());
			let left = tl.x();
			let bottom = br.y();
			let right = br.x();
			let top = tl.y();
			self.state.map_bbox = Bbox { left, bottom, right, top };
		}

		#[cfg(feature = "debug")]
		let instant = Instant::now();

		/* generate points cache */ {
			if self.regenerate_points {
				self.state.node_cache.clear();
				self.generate_points_cache(projector);
			}
		}

		#[cfg(feature = "debug")]
		let instant = {
			self.debug_times.push(("generate points cache", instant.elapsed().as_micros() as u32));
			Instant::now()
		};

		/* draw osm data and determine hovered element */ {
			match self.selection_mode {
				SelectionMode::Nodes => {
					let mut shapes = vec![];
					for node in self.osm.data.nodes.values() {
						let pos = *self.state.node_cache.get(&node.id).expect("id not found in cache");

						// determine hover
						if self.state.hovered.is_none() {
							if let Some(mouse) = resp.hover_pos() {
								if pos.distance_sq(mouse) < 25.0 {
									self.state.hovered = Some(node.id);
								}
							}
						}

						// draw
						shapes.push(Shape::Circle(CircleShape {
							center: pos,
							radius: DEFAULT_NODE_SIZE * self.scale_factor,
							fill: Color32::WHITE,
							stroke: Stroke::new(1.0, Color32::BLACK)
						}));
					}

					ui.painter().extend(shapes);
				}
				SelectionMode::Ways => {
					for way in self.osm.data.ways.values() {
						let points = self.get_nodes_in_way_cloned(way.id);
						let width = self.way_width(way);
						let color = self.way_color(way);

						// determine hover
						if self.state.hovered.is_none() {
							if let Some(mouse) = resp.hover_pos() {
								if points.windows(2).any(|p| distance_to_segment(mouse, &[p[0], p[1]]) < width) {
									self.state.hovered = Some(way.id);
								}
							}
						}

						// draw using selected visualization
						let shapes = match self.visualization {
							Visualization::Default => visual::default(points.clone(), color, width),
							Visualization::Sidewalks => visual::sidewalks(way, points, color, width),
						};

						ui.painter().extend(shapes);
					}
				}
			}
		}

		#[cfg(feature = "debug")]
		let instant = {
			self.debug_times.push(("draw and hover", instant.elapsed().as_micros() as u32));
			Instant::now()
		};

		/* draw hovered element and determine if it was selected */ {
			if let Some(hover) = self.state.hovered {
				let element = self.osm.data.nodes.get(&hover);
				if let Some(element) = element {
					let pos = *self.state.node_cache.get(&element.id).expect("id not found in cache");

					shapes_top.push(
						Shape::Circle(CircleShape::stroke(pos, DEFAULT_NODE_SIZE * self.scale_factor, Stroke::new(DEFAULT_NODE_SIZE, HOVER_COLOR)))
					);

					if resp.clicked() {
						self.state.selected = Some(hover);
					}
				} else if let Some(element) = self.osm.data.ways.get(&hover) {
					let points = self.get_nodes_in_way_cloned(element.id);

					shapes_top.push(
						Shape::Path(PathShape::line(
							points, PathStroke::new(self.way_width(element) + HOVER_SIZE_INCREASE, HOVER_COLOR)
						))
					);

					if resp.clicked() { // selected
						if self.is_way_relevant(&element.tags) {
							self.state.selected = Some(hover);
						} else { // deselect when clicking irrelevant object
							self.state.selected = None;
						}
					}
				} else {
					panic!("invalid element id");
				}

			} else if resp.clicked() { // discard hovered way
				self.state.selected = None;
			}
		}

		#[cfg(feature = "debug")]
		let instant = {
			self.debug_times.push(("draw hovered, determine selected", instant.elapsed().as_micros() as u32));
			Instant::now()
		};

		/* draw selected element */ {
			if let Some(selected) = self.state.selected {
				let element = self.osm.get_by_id(&selected).expect("id not found");

				match element {
					Element::Node(node) => {
						let point = self.state.node_cache.get(&node.id).expect("id not found");

						shapes_top.push(
							Shape::Circle(CircleShape::stroke(
								*point, DEFAULT_NODE_SIZE, Stroke::new(DEFAULT_NODE_SIZE + SELECTION_SIZE_INCREASE, SELECTION_COLOR)),
							)
						);
					}
					Element::Way(way) => {
						let points = self.get_nodes_in_way_cloned(way.id);

						shapes_top.push(
							Shape::Path(PathShape::line(
								points, PathStroke::new(self.way_width(way) + SELECTION_SIZE_INCREASE, SELECTION_COLOR),
							))
						);

						// draw editing ui
						if self.is_way_relevant(&way.tags) {
							if let Some(change) = self.way_editing_ui(ui, way.id, projector.project(self.state.last_click_coords).to_pos2()) {
								self.osm.apply_change(change);
							}
						}
					}
				}

			}
		}

		#[cfg(feature = "debug")]
		self.debug_times.push(("draw selected way", instant.elapsed().as_micros() as u32));

		ui.painter().extend(shapes_top);
	}
}

impl EditorPlugin<'_> {
	fn get_nodes_in_way_cloned(&self, way: Id) -> Vec<Pos2> {
		self.osm.data.ways.get(&way).expect("way id must be valid").nodes.iter().map(|node_id| {
			self.state.node_cache.get(node_id).expect("node id must be valid and cached").to_owned()
		}).collect()
	}

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

	fn generate_points_cache(&mut self, projector: &Projector) {
		debug_assert!(self.state.node_cache.is_empty());
		for (id, node) in &self.osm.data.nodes {
			self.state.node_cache.insert(*id, projector.project(coordinate_to_pos(&node.pos)).to_pos2());
		}
	}

	fn is_way_relevant(&self, tags: &Tags) -> bool {
		match self.visualization {
			Visualization::Default => true,
			Visualization::Sidewalks => visual::sidewalks_relevant(tags),
		}
	}

	fn way_editing_ui(&mut self, ui: &mut Ui, id: Id, pos: Pos2) -> Option<Change> {
		match self.visualization {
			Visualization::Default => None,
			Visualization::Sidewalks => visual::sidewalks_ui(ui, self.osm.data.ways.get(&id).unwrap(), pos),
		}
	}
}


pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::new(c.lon, c.lat)
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
