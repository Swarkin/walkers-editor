pub mod visual;
pub mod cache;
pub mod consts;
pub mod attribute2d;
pub mod states;

use super::osm::Bbox;
use crate::app::editor::states::CacheFlag;
use cache::*;
use consts::{osm::DEFAULT_NODE_SIZE, *};
use eframe::egui::{Color32, Mesh, Pos2, Response, Shape, Stroke, TextureId, Ui};
use eframe::epaint::{CircleShape, ColorMode, PathShape, PathStroke, StrokeKind, Vertex, WHITE_UV};
use lyon_tessellation::geom::Point;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use osm_parser::*;
use states::{SelectionBitflag, SelectionFlag};
use std::sync::Arc;
use visual::{FillMode, Visualization};
use walkers::{Plugin, Position, Projector};

/// Data that is passed in every frame
pub struct EditorPlugin<'a> {
	pub state: &'a mut EditorPluginState,
	pub osm: &'a mut EditorOsmData,
	pub visualization: Visualization,
	pub selection_mode: SelectionBitflag,
	pub fill_mode: FillMode,
	pub scale_factor: f32,
	pub current_zoom: f64,
	pub current_pos: Position,
}

/// Data that persists or is produced between frames
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
		/* cache invalidation */ {
			if self.osm.cache_flags & CacheFlag::Projection as u8 != 0 {
				self.osm.reproject_nodes(projector, self.current_pos);
			} else {
				// update move offset
				let p_start = projector.project(self.osm.start_pos);
				let p_current = projector.project(self.current_pos);
				let diff = p_start - p_current;
				self.osm.offset = diff;
			}

			if self.osm.cache_flags & CacheFlag::Orphan as u8 != 0 {
				self.osm.redetect_orphan_nodes();
			}
		}

		// setup
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

		/* draw osm data and determine hovered element */ {
			let len = self.osm.data.ways.len();
			let mut shapes = Vec::<Shape>::with_capacity(len);

			for way in self.osm.data.ways.values() {
				let points = self.osm.get_node_positions_in_way_owned(way.id);
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

				// override fill mode
				let mut target_fill = self.fill_mode;
				if target_fill == FillMode::Partial && self.current_zoom < FILL_MODE_THRESHOLD {
					target_fill = FillMode::Full
				};

				// draw logic
				if is_way_area(way) {
					match target_fill {
						FillMode::Wireframe => {
							shapes.push(PathShape::closed_line(
								points.into_iter().skip(1).collect(),
								PathStroke::new(width, color)
							).into());
						}
						FillMode::Partial => {
							shapes.push(PathShape::closed_line(
								points.clone().into_iter().skip(1).collect(),
								PathStroke::new(width, color)
							).into());

							// todo: https://github.com/Swarkin/walkers-editor/issues/9
							shapes.push(PathShape::closed_line(
								if let Some(order) = winding_order(&points) {
									if order { points.into_iter().skip(1).collect() }
									else { points.into_iter().rev().skip(1).collect() }
								} else {
									#[cfg(feature = "debug")]
									panic!("failed to calculate winding order of {points:?}");
									#[cfg(not(feature = "debug"))]
									points.into_iter().skip(1).collect()
								},
								PathStroke {
									width: 12.0,
									color: ColorMode::Solid(color.gamma_multiply(0.5)),
									kind: StrokeKind::Inside,
								}
							).into());
						}
						FillMode::Full => {
							// draw area
							let mut builder = lyon_tessellation::path::Path::builder();
							builder.begin(Point::new(points[0].x, points[0].y));

							for p in points.iter().skip(1) {
								builder.line_to(Point::new(p.x, p.y));
							}

							builder.close();

							// todo: avoid expensive triangulation every frame
							let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
							let mut tessellator = FillTessellator::new();

							// todo: intersection handling
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
							shapes.push(PathShape {
								points: points.into_iter().skip(1).collect(),
								closed: true,
								fill: Color32::TRANSPARENT,
								stroke:  PathStroke::new(width, color),
							}.into());
						}
					}
				} else {
					// draw way
					shapes.extend(match self.visualization {
						Visualization::Default => visual::default(points, color, width),
						Visualization::Sidewalks => visual::sidewalks(way, points, color, width),
					});

					// draw nodes
					shapes.extend(way.nodes.iter().map(|node_id| {
						Shape::Circle(CircleShape {
							center: self.osm.get_projected_pos_owned(node_id).expect("id not found in cache"),
							radius: DEFAULT_NODE_SIZE * self.scale_factor,
							fill: Color32::LIGHT_GRAY,
							stroke: Stroke::new(1.0, Color32::GRAY)
						})
					}));
				};
			}

			debug_assert!(shapes.len() >= len, "overallocated shape buffer: {} - {len}", shapes.len());
			ui.painter().extend(shapes);

			// draw orphan nodes and determine hovered
			ui.painter().extend(self.osm.orphan_nodes.iter().map(|id| {
				let pos = self.osm.get_projected_pos_owned(id).expect("id not found in cache");

				if let Some(mouse) = hover {
					if self.state.hovered.is_none()
						&& (self.selection_mode & (SelectionFlag::Nodes as u8)) != 0
						&& is_node_hovered(&pos, mouse, (DEFAULT_NODE_SIZE * self.scale_factor).powi(2))
					{
						self.state.hovered = Some(*id);
					}
				}

				CircleShape {
					center: pos,
					radius: DEFAULT_NODE_SIZE * self.scale_factor,
					fill: Color32::WHITE,
					stroke: Stroke::new(1.0, Color32::GRAY)
				}.into()
			}));

		}

		/* draw hovered element and determine if it was selected */ {
			if let Some(hover) = self.state.hovered {
				let element = self.osm.data.nodes.get(&hover).map(ElementRef::Node)
					.or_else(|| self.osm.data.ways.get(&hover).map(ElementRef::Way))
					.expect("id not found");

				match element {
					ElementRef::Node(node) => {
						let pos = self.osm.get_projected_pos_owned(&node.id).expect("id not found in cache");

						shapes_top.push(
							CircleShape::stroke(pos, DEFAULT_NODE_SIZE * self.scale_factor, Stroke::new(DEFAULT_NODE_SIZE, HOVER_COLOR)).into()
						);

						if resp.clicked() {
							self.state.selected = Some(hover);
						}
					}
					ElementRef::Way(way) => {
						let points = self.osm.get_node_positions_in_way_owned(way.id);

						shapes_top.push(
							PathShape::line(
								points, PathStroke::new(self.way_width(way) + HOVER_SIZE_INCREASE, HOVER_COLOR)
							).into()
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

		/* draw selected element */ {
			if let Some(selected) = self.state.selected {
				let element = self.osm.get(&selected).expect("id not found");

				match element {
					ElementRef::Node(node) => {
						let point = self.osm.get_projected_pos_owned(&node.id).expect("id not found in cache");

						shapes_top.push(
							Shape::Circle(CircleShape::stroke(
								point, DEFAULT_NODE_SIZE, Stroke::new(DEFAULT_NODE_SIZE + SELECTION_SIZE_INCREASE, SELECTION_COLOR)),
							)
						);
					}
					ElementRef::Way(way) => {
						let points = self.osm.get_node_positions_in_way_owned(way.id);

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

		ui.painter().extend(shapes_top);
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
	if !is_way_closed(way) || way.nodes.len() < 3 || way.tags.is_empty() { return false; }

	for key in ["building", "landuse", "natural", "leisure", "amenity"] {
		if way.tags.contains_key(key) { return true; }
	}

	if way.tags.get("area") == Some(&"yes".into()) { return true; }

	false
}

fn winding_order(points: &[Pos2]) -> Option<bool> {
	let n = points.len();
	if n < 3 {
		return None;
	}

	let mut area = 0.0;
	for i in 0..n {
		let p1 = points[i];
		let p2 = points[(i + 1) % n];
		area += (p1.x * p2.y) - (p2.x * p1.y);
	}

	if area > 0.0 {
		Some(true)
	} else if area < 0.0 {
		Some(false)
	} else {
		None
	}
}
