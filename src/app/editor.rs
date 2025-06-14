pub mod visual;
pub mod cache;
pub mod consts;
pub mod attribute2d;
pub mod states;

use super::osm::Bbox;
use super::places::school;
use cache::{Change, EditorOsmData, ElementRef, MAX_OFFSET};
use consts::{osm::*, *};
use eframe::egui::{Color32, Pos2, Response, Shape, Stroke, Ui};
use eframe::epaint::{CircleShape, ColorMode, PathShape, PathStroke, StrokeKind};
use osm_parser::*;
use states::{CacheFlag, MapState, SelectionFlag};
use std::sync::Arc;
use visual::{FillMode, Visualization};
use walkers::{MapMemory, Plugin, Position, Projector};

/// Data that is passed in every frame
pub struct EditorPlugin<'a> {
	pub editor_state: &'a mut EditorPluginState,
	pub map_state: &'a mut MapState,
	pub osm: &'a mut EditorOsmData,
	pub map_memory: MapMemory,
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
			let current_pos = self.map_memory.detached().unwrap_or_else(school);
			// todo: fix 1 frame delay
			if self.osm.cache_flags & CacheFlag::NodeProjection as u8 != 0 {
				self.osm.refresh_projected_nodes_cache(projector, current_pos);
			} else {
				// update move offset
				let p_start = projector.project(self.osm.node_start);
				let current_projected = projector.project(current_pos);
				let diff = p_start - current_projected;

				if diff.x > MAX_OFFSET || diff.y > MAX_OFFSET {
					// reproject occasionally to minify possible precision errors?
					self.osm.refresh_projected_nodes_cache(projector, current_pos);
				} else if !self.osm.data.nodes.is_empty() {
					self.osm.node_offset_move = diff;
				}
			}

			if self.osm.cache_flags & CacheFlag::NodeOrphan as u8 != 0 {
				self.osm.refresh_orphan_nodes_cache();
			}

			if self.osm.cache_flags & CacheFlag::NodeUsage as u8 != 0 {
				self.osm.refresh_node_usage_cache();
			}

			if self.osm.cache_flags & CacheFlag::WayArea as u8 != 0 {
				self.osm.refresh_way_area_cache();
			}

			if self.osm.cache_flags & CacheFlag::NodeDedup as u8 != 0 {
				self.osm.refresh_way_nodes_dedup_cache();
			}

			if self.osm.cache_flags & CacheFlag::WayMesh as u8 != 0 {
				// it might be possible to use emath::TSTransform for more performance
				self.osm.refresh_way_mesh_cache(current_pos);
			} else if !self.osm.data.ways.is_empty() {
				// update move offset
				let p_start = projector.project(self.osm.mesh_start);
				let current_projected = projector.project(current_pos);
				let diff = p_start - current_projected;

				if diff.x > MAX_OFFSET || diff.y > MAX_OFFSET {
					self.osm.refresh_way_mesh_cache(current_pos);
				} else {
					self.osm.mesh_offset_move = diff;
				}
			}
		}

		let hover = resp.hover_pos();
		self.editor_state.hovered = None;

		// override fill mode
		let mut target_fill = self.map_state.selected_fill_mode;
		if target_fill == FillMode::Partial && self.map_memory.zoom() < PARTIAL_FILL_THRESHOLD {
			target_fill = FillMode::Full
		};

		/* determine last clicked position */ {
			if resp.clicked() {
				self.editor_state.last_click_coords = projector.unproject(resp.interact_pointer_pos().unwrap().to_vec2());
			}
		}

		/* update state.map_bbox */ {
			let tl = projector.unproject(resp.rect.min.to_vec2() + resp.rect.center().to_vec2());
			let br = projector.unproject(resp.rect.max.to_vec2() + resp.rect.center().to_vec2());
			self.editor_state.map_bbox.left = tl.x();
			self.editor_state.map_bbox.bottom = br.y();
			self.editor_state.map_bbox.right = br.x();
			self.editor_state.map_bbox.top = tl.y();
		}

		// minimum capacity
		let capacity = self.osm.node_dedup.len() + self.osm.data.ways.len();
		// todo: cache shapes between frames
		let mut shapes = Vec::with_capacity(capacity);

		/* draw osm data and detect interactions */ {
			let mut detect_interactions = hover.is_some()
				&& self.map_state.selection_mode & SelectionFlag::Ways as u8 != 0;

			for way in self.osm.data.ways.values() {
				let points = self.osm.get_projected_positions_in_way(&way.id);

				if detect_interactions {
					let width = self.way_width(way);
					let mouse = hover.unwrap();

					if is_way_hovered(&points, &mouse, width.powi(2)) {
						self.editor_state.hovered = Some(way.id);
						detect_interactions = false;
					}
				}

				if *self.osm.way_area.get(&way.id).expect("way not found in cache") {
					// draw area
					match target_fill {
						FillMode::Wireframe => shapes.push(self.draw_way_closed(&way.id).into()),
						FillMode::Partial => {
							// outline
							shapes.push(self.draw_way_closed(&way.id).into());

							// partial fill
							// todo: https://github.com/Swarkin/walkers-editor/issues/9
							shapes.push(self.draw_fill_partial_from(
								if let Some(order) = winding_order(&points) {
									if order { points.into_iter().skip(1).collect() } else { points.into_iter().rev().skip(1).collect() }
								} else {
									#[cfg(debug_assertions)]
									eprintln!("winding_order failed for {points:?}");
									points.into_iter().skip(1).collect()
								},
								PARTIAL_FILL_WIDTH,
								self.way_color(way).gamma_multiply(PARTIAL_FILL_GAMMA_MULTIPLY),
							).into());
						}
						FillMode::Full => {
							// draw area
							let color = self.way_color(way);
							let mesh = self.osm.get_way_mesh(&way.id, color.gamma_multiply(PARTIAL_FILL_GAMMA_MULTIPLY));
							shapes.push(Shape::Mesh(Arc::new(mesh)));

							// draw stroke
							shapes.push(PathShape {
								points: points.into_iter().skip(1).collect(),
								closed: true,
								fill: Color32::TRANSPARENT,
								stroke: PathStroke::new(self.way_width(way), color),
							}.into());
						}
					}
				} else {
					// draw way
					let color = self.way_color(way);
					let width = self.way_width(way);

					shapes.extend(match &self.map_state.selected_visualization {
						Visualization::Default => vec![self.draw_way_from(points, width, color).into()],
						Visualization::Sidewalks => visual::sidewalks(&way.tags, points, width, color),
					});
				}
			}

			/* draw nodes and detect hover */ {
				if self.map_state.selection_mode & SelectionFlag::Nodes as u8 != 0 && hover.is_some() {
					let way_nodes = self.osm.node_dedup.way_nodes.iter().map(|id| {
						let pos = self.osm.get_projected_pos(id).expect("id not found in cache");
						let shape = if self.osm.node_usage.get(id).expect("id not found in cache").len() > 1 {
							self.draw_node_connected_at(pos)
						} else {
							self.draw_node_at(pos)
						}.into();
						shapes.push(shape);
						(pos, id)
					}).collect::<Vec<_>>();

					let orphan_nodes = self.osm.node_dedup.orphan_nodes.iter().map(|id| {
						let pos = self.osm.get_projected_pos(id).expect("id not found in cache");
						shapes.push(self.draw_node_orphan_at(pos).into());
						(pos, id)
					}).collect::<Vec<_>>();

					let mouse = hover.unwrap();
					let mut done = false;

					let distance_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);
					for (pos, id) in way_nodes {
						if pos.distance_sq(mouse) < distance_sq {
							self.editor_state.hovered = Some(*id);
							done = true;
							break;
						}
					}

					if !done {
						let distance_sq = (NODE_SIZE_ORPHAN * self.map_state.scale_factor).powi(2);
						for (pos, id) in orphan_nodes {
							if pos.distance_sq(mouse) < distance_sq {
								self.editor_state.hovered = Some(*id);
								break;
							}
						}
					}
				} else { // optimized without hover detection
					for id in &self.osm.node_dedup.way_nodes {
						shapes.push(self.draw_node_dynamic(id).into());
					}

					for id in &self.osm.node_dedup.orphan_nodes {
						shapes.push(self.draw_node_orphan(id).into())
					}
				}
			}
		}

		/* draw hovered element and detect whether it was selected */ {
			if let Some(hover) = self.editor_state.hovered {
				let element = self.osm.data.nodes.get(&hover).map(ElementRef::Node)
					.or_else(|| self.osm.data.ways.get(&hover).map(ElementRef::Way))
					.expect("id not found in data");

				match element {
					ElementRef::Node(node) => {
						shapes.push(self.draw_node_hovered(&node.id).into());

						if resp.clicked() {
							self.editor_state.selected = Some(hover);
						}
					}
					ElementRef::Way(way) => {
						if resp.clicked() { // selected
							if self.is_way_relevant(&way.tags) {
								self.editor_state.selected = Some(hover);
							} else { // deselect when clicking irrelevant way
								self.editor_state.selected = None;
							}
						} else if is_way_closed(way) {
							shapes.push(self.draw_way_closed_hovered(&way.id).into());
							shapes.extend(
								way.nodes.iter().skip(1)
									.map(|id| self.draw_node_dynamic(id).into())
							);
						} else {
							shapes.push(self.draw_way_hovered(&way.id).into());
							shapes.extend(
								way.nodes.iter()
									.map(|id| self.draw_node_dynamic(id).into())
							);
						}
					}
				}
			} else if resp.clicked() { // clicked on empty space
				self.editor_state.selected = None;
			}
		}

		/* draw selected element */ {
			if let Some(id) = self.editor_state.selected {
				let element = self.osm.get(&id).expect("id not found");

				match element {
					ElementRef::Node(node) => shapes.push(self.draw_node_selected(&node.id).into()),
					ElementRef::Way(way) => {
						if is_way_closed(way) {
							shapes.push(self.draw_way_closed_selected(&id).into());
							shapes.extend(
								way.nodes.iter().skip(1)
									.map(|id| self.draw_node_dynamic(id).into())
							);
						} else {
							shapes.push(self.draw_way_selected(&id).into());
							shapes.extend(
								way.nodes.iter()
									.map(|id| self.draw_node_dynamic(id).into())
							);
						}

						// draw editing ui
						if self.is_way_relevant(&way.tags) {
							if let Some(change) = self.way_editing_ui(ui, way.id, projector.project(self.editor_state.last_click_coords).to_pos2()) {
								self.osm.apply_change(change);
							}
						}
					}
				}
			}
		}

		debug_assert!(shapes.len() >= capacity, "overallocated shape buffer: {} - {capacity}", shapes.len());
		ui.painter().extend(shapes);
	}
}

// drawing nodes
impl EditorPlugin<'_> {
	fn draw_node(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH, NODE_STROKE_COLOR),
		}
	}

	fn draw_node_at(&self, center: Pos2) -> CircleShape {
		CircleShape {
			center,
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH, NODE_STROKE_COLOR),
		}
	}

	fn draw_node_connected(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_CONNECTED_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH, NODE_STROKE_COLOR),
		}
	}

	fn draw_node_connected_at(&self, center: Pos2) -> CircleShape {
		CircleShape {
			center,
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_CONNECTED_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH, NODE_STROKE_COLOR),
		}
	}

	fn draw_node_orphan(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE_ORPHAN * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH, NODE_STROKE_COLOR),
		}
	}

	fn draw_node_orphan_at(&self, center: Pos2) -> CircleShape {
		CircleShape {
			center,
			radius: NODE_SIZE_ORPHAN * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH, NODE_STROKE_COLOR),
		}
	}

	fn draw_node_hovered(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke::new(NODE_STROKE_WIDTH + HOVER_SIZE_INCREASE, HOVER_COLOR),
		}
	}

	fn draw_node_selected(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH + SELECTION_SIZE_INCREASE, color: SELECTION_COLOR },
		}
	}

	fn draw_node_dynamic(&self, id: &Id) -> CircleShape {
		if self.osm.node_usage.get(id).expect("id not found in cache").len() > 1 {
			self.draw_node_connected(id)
		} else {
			self.draw_node(id)
		}
	}
}

// drawing ways
impl EditorPlugin<'_> {
	fn draw_way_from(&self, points: Vec<Pos2>, width: f32, color: Color32) -> PathShape {
		PathShape {
			points,
			closed: false,
			fill: Color32::default(),
			stroke: PathStroke {
				width,
				color: ColorMode::Solid(color),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_closed(&self, id: &Id) -> PathShape {
		let way = self.osm.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm.get_projected_positions_in_way(id).into_iter().skip(1).collect(),
			closed: true,
			fill: Color32::default(),
			stroke: PathStroke {
				width: self.way_width(way),
				color: ColorMode::Solid(self.way_color(way)),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_hovered(&self, id: &Id) -> PathShape {
		let way = self.osm.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm.get_projected_positions_in_way(id),
			closed: false,
			fill: Color32::default(),
			stroke: PathStroke {
				width: self.way_width(way) + HOVER_SIZE_INCREASE,
				color: ColorMode::Solid(HOVER_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_closed_hovered(&self, id: &Id) -> PathShape {
		let way = self.osm.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm.get_projected_positions_in_way(id).into_iter().skip(1).collect(),
			closed: true,
			fill: Color32::default(),
			stroke: PathStroke {
				width: self.way_width(way) + HOVER_SIZE_INCREASE,
				color: ColorMode::Solid(HOVER_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_selected(&self, id: &Id) -> PathShape {
		let way = self.osm.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm.get_projected_positions_in_way(id),
			closed: false,
			fill: Color32::default(),
			stroke: PathStroke {
				width: self.way_width(way) + SELECTION_SIZE_INCREASE,
				color: ColorMode::Solid(SELECTION_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_closed_selected(&self, id: &Id) -> PathShape {
		let way = self.osm.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm.get_projected_positions_in_way(id).into_iter().skip(1).collect(),
			closed: true,
			fill: Color32::default(),
			stroke: PathStroke {
				width: self.way_width(way) + SELECTION_SIZE_INCREASE,
				color: ColorMode::Solid(SELECTION_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_fill_partial_from(&self, points: Vec<Pos2>, width: f32, color: Color32) -> PathShape {
		PathShape {
			points,
			closed: true,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width,
				color: ColorMode::Solid(color),
				kind: StrokeKind::Inside,
			}
		}
	}
}

// logic
impl EditorPlugin<'_> {
	fn way_width(&self, way: &Way) -> f32 {
		match self.map_state.selected_visualization {
			Visualization::Default => visual::width_default(way) * self.map_state.scale_factor,
			Visualization::Sidewalks => visual::width_sidewalk(way) * self.map_state.scale_factor,
		}
	}

	fn way_color(&self, way: &Way) -> Color32 {
		match self.map_state.selected_visualization {
			Visualization::Default => visual::color_default(way),
			Visualization::Sidewalks => visual::color_sidewalk(way),
		}
	}

	fn is_way_relevant(&self, tags: &Tags) -> bool {
		match self.map_state.selected_visualization {
			Visualization::Default => true,
			Visualization::Sidewalks => visual::sidewalks_relevant(tags),
		}
	}

	fn way_editing_ui(&mut self, ui: &mut Ui, id: Id, pos: Pos2) -> Option<Change> {
		match self.map_state.selected_visualization {
			Visualization::Default => None,
			Visualization::Sidewalks => visual::sidewalks_ui(ui, self.osm.data.ways.get(&id).unwrap(), pos),
		}
	}
}

fn distance_to_segment_sq(p: &Pos2, points: &[Pos2; 2]) -> f32 {
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
	} else if param > 1f32 {
		xx = y.x;
		yy = y.y;
	} else {
		xx = x.x + param * c;
		yy = x.y + param * d;
	}

	let dx = p.x - xx;
	let dy = p.y - yy;
	dx * dx + dy * dy
}

fn is_way_hovered(points: &[Pos2], mouse: &Pos2, distance_sq: f32) -> bool {
	points.windows(2).any(|p| distance_to_segment_sq(mouse, &[p[0], p[1]]) < distance_sq)
}

fn is_way_closed(way: &Way) -> bool {
	way.nodes.first() == way.nodes.last()
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
