pub mod visual;
pub mod changes;
pub mod consts;
pub mod attribute2d;
pub mod states;

use super::osm::Bbox;
use changes::*;
use consts::*;
use eframe::egui::{Color32, Mesh, Pos2, Response, Shape, Stroke, TextureId, Ui};
use eframe::epaint::{CircleShape, PathShape, PathStroke, Vertex, WHITE_UV};
use lyon_tessellation::geom::Point;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use osm::DEFAULT_NODE_SIZE;
use osm_parser::*;
use states::{SelectionBitflag, SelectionFlag};
use std::sync::Arc;
use visual::Visualization;
use walkers::{Plugin, Position, Projector};
#[cfg(feature = "debug")]
use {super::DebugTimes, std::time::Instant};

/// Data that is passed in every frame
pub struct EditorPlugin<'a> {
	pub state: &'a mut EditorPluginState,
	pub osm: &'a mut EditorOsmData,
	pub visualization: Visualization,
	pub selection_mode: SelectionBitflag,
	pub scale_factor: f32,
	pub regenerate_points: bool,
	pub regenerate_orphan: bool,
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
}

impl Plugin for EditorPlugin<'_> {
	// todo(optimization): cache results of way_width and way_color
	fn run(mut self: Box<Self>, ui: &mut Ui, resp: &Response, projector: &Projector) {
		let mut shapes_top = Vec::with_capacity(2);
		let hover = resp.hover_pos();
		self.state.hovered = None;

		/* determine last clicked position */ {
			if resp.clicked() {
				self.state.last_click_coords = projector.unproject(resp.interact_pointer_pos().unwrap().to_vec2());
			}
		}

		/* update state.map_bbox */ {
			let tl = projector.unproject(resp.rect.min.to_vec2() + resp.rect.center().to_vec2());
			let br = projector.unproject(resp.rect.max.to_vec2() + resp.rect.center().to_vec2());
			self.state.map_bbox.left = tl.x();
			self.state.map_bbox.bottom = br.y();
			self.state.map_bbox.right = br.x();
			self.state.map_bbox.top = tl.y();
		}

		#[cfg(feature = "debug")]
		let instant = Instant::now();

		/* (re)generate caches */ {
			if self.regenerate_points {
				#[cfg(feature = "debug")]
				dbg!("points");
				self.osm.reproject_nodes(projector);
			}

			if self.regenerate_orphan {
				#[cfg(feature = "debug")]
				dbg!("orphan");
				self.osm.detect_orphan_nodes();
			}
		}

		#[cfg(feature = "debug")]
		let instant = {
			self.debug_times.push(("generate points cache", instant.elapsed().as_micros() as u32));
			Instant::now()
		};

		/* draw osm data and determine hovered element */ {
			for way in self.osm.data.ways.values() {
				let points = self.get_nodes_in_way_cloned(way.id);
				let width = self.way_width(way);
				let color = self.way_color(way);

				// hover logic
				if let Some(mouse) = hover {
					if self.state.hovered.is_none() {
						if (self.selection_mode & SelectionFlag::Nodes as u8) != 0 {
							for (pos, id) in points.iter().zip(&way.nodes) {
								if is_node_hovered(pos, mouse, width.powi(2)) {
									self.state.hovered = Some(*id);
								}
							}
						}
						if (self.selection_mode & SelectionFlag::Ways as u8) != 0 && is_way_hovered(&points, &mouse, width) {
                            self.state.hovered = Some(way.id);
                        }
					}
				}

				// draw logic
				let shapes = if is_way_area(way) {
					debug_assert!(!points.is_empty());
					let mut shapes = Vec::with_capacity(2);

					// draw area
					let mut builder = lyon_tessellation::path::Path::builder();
					builder.begin(Point::new(points[0].x, points[0].y));

					for p in points.iter().skip(1) {
						builder.line_to(Point::new(p.x, p.y));
					}

					builder.close();

					// todo(performance): implement a cache that only reprojects elements on zoom
					// this is very important to avoid expensive triangulation on every frame
					let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
					let mut tessellator = FillTessellator::new();

					// todo: re-enable intersection handling
					tessellator.tessellate_path(
						&builder.build(),
						&FillOptions::default().with_intersections(false),
						&mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
							Vertex {
								pos: Pos2::from(vertex.position().to_array()),
								uv: WHITE_UV,
								color: color.gamma_multiply(0.7),
							}
						}),
					).expect("path tesselation failed");

					shapes.push(Shape::Mesh(Arc::new(Mesh {
						indices: geometry.indices,
						vertices: geometry.vertices,
						texture_id: TextureId::Managed(0),
					})));

					// draw stroke
					shapes.push(Shape::Path(PathShape {
						points: points.into_iter().skip(1).collect(),
						closed: true,
						fill: Color32::TRANSPARENT,
						stroke:  PathStroke::new(width, color),
					}));

					shapes
				} else {
					let mut shapes = Vec::with_capacity(way.nodes.len() + 1); // node count + at least 1

					// draw way
					shapes.extend(match self.visualization {
						Visualization::Default => visual::default(points, color, width),
						Visualization::Sidewalks => visual::sidewalks(way, points, color, width),
					});

					// draw nodes
					shapes.extend(way.nodes.iter().map(|node_id| {
						Shape::Circle(CircleShape {
							center: *self.osm.projected_nodes.get(node_id).expect("id not found in cache"),
							radius: DEFAULT_NODE_SIZE * self.scale_factor,
							fill: Color32::LIGHT_GRAY,
							stroke: Stroke::new(1.0, Color32::GRAY)
						})
					}));

					shapes
				};

				ui.painter().extend(shapes);
			}

			// draw orphan nodes and determine hovered
			ui.painter().extend(self.osm.orphan_nodes.iter().map(|id| {
				let pos = *self.osm.projected_nodes.get(id).expect("id not found in cache");

				if let Some(mouse) = hover {
					if self.state.hovered.is_none() && (self.selection_mode & (SelectionFlag::Nodes as u8)) != 0 && is_node_hovered(&pos, mouse, (DEFAULT_NODE_SIZE * self.scale_factor).powi(2)) {
						self.state.hovered = Some(*id);
					}
				}

				Shape::Circle(CircleShape {
					center: pos,
					radius: DEFAULT_NODE_SIZE * self.scale_factor,
					fill: Color32::WHITE,
					stroke: Stroke::new(1.0, Color32::GRAY)
				})
			}));
		}

		#[cfg(feature = "debug")]
		let instant = {
			self.debug_times.push(("draw and hover", instant.elapsed().as_micros() as u32));
			Instant::now()
		};

		/* draw hovered element and determine if it was selected */ {
			if let Some(hover) = self.state.hovered {
				let element = self.osm.data.nodes.get(&hover).map(Element::Node)
					.or_else(|| self.osm.data.ways.get(&hover).map(Element::Way))
					.expect("id not found");

				match element {
					Element::Node(node) => {
						let pos = *self.osm.projected_nodes.get(&node.id).expect("id not found in cache");

						shapes_top.push(
							Shape::Circle(CircleShape::stroke(pos, DEFAULT_NODE_SIZE * self.scale_factor, Stroke::new(DEFAULT_NODE_SIZE, HOVER_COLOR)))
						);

						if resp.clicked() {
							self.state.selected = Some(hover);
						}
					}
					Element::Way(way) => {
						let points = self.get_nodes_in_way_cloned(way.id);

						shapes_top.push(
							Shape::Path(PathShape::line(
								points, PathStroke::new(self.way_width(way) + HOVER_SIZE_INCREASE, HOVER_COLOR)
							))
						);

						if resp.clicked() { // selected
							if self.is_way_relevant(&way.tags) {
								self.state.selected = Some(hover);
							} else { // deselect when clicking irrelevant object
								self.state.selected = None;
							}
						}
					}
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
						let point = self.osm.projected_nodes.get(&node.id).expect("id not found in cache");

						shapes_top.push(
							Shape::Circle(CircleShape::stroke(
								*point, DEFAULT_NODE_SIZE, Stroke::new(DEFAULT_NODE_SIZE + SELECTION_SIZE_INCREASE, SELECTION_COLOR)),
							)
						);
					}
					Element::Way(way) => {
						let points = self.get_nodes_in_way_cloned(way.id);

						if is_way_closed(way) {
							shapes_top.push(
								Shape::Path(PathShape::closed_line(
									points.into_iter().skip(1).collect(), PathStroke::new(self.way_width(way) + SELECTION_SIZE_INCREASE, SELECTION_COLOR),
								))
							);
						} else {
							shapes_top.push(
								Shape::Path(PathShape::line(
									points, PathStroke::new(self.way_width(way) + SELECTION_SIZE_INCREASE, SELECTION_COLOR),
								))
							);
						}

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
			self.osm.projected_nodes.get(node_id).expect("id not found in cache").to_owned()
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

fn distance_to_segment(p: &Pos2, points: &[Pos2; 2]) -> f32 {
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

fn is_node_hovered(point: &Pos2, mouse: Pos2, distance_squared: f32) -> bool {
	point.distance_sq(mouse) < distance_squared
}

fn is_way_hovered(points: &[Pos2], mouse: &Pos2, width: f32) -> bool {
	points.windows(2).any(|p| distance_to_segment(mouse, &[p[0], p[1]]) < width)
}

fn is_way_closed(way: &Way) -> bool {
	way.nodes.first() == way.nodes.last()
}

fn is_way_area(way: &Way) -> bool {
	if !is_way_closed(way) { return false; }

	if !way.tags.is_empty() {
		for key in ["building", "landuse", "natural", "leisure", "amenity"] {
			if way.tags.contains_key(key) { return true; }
		}

		if way.tags.get("area") == Some(&"yes".into()) {
			return true;
		}

		false
	} else { false }
}
