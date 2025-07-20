pub mod visual;
pub mod cache;
pub mod consts;
pub mod attribute2d;
pub mod states;

use super::osm::Bbox;
use super::places::school;
use crate::app::windows::OverlapSelectorResult;
use cache::{Change, EditorOsmData, ElementId, ElementRef, MAX_OFFSET};
use consts::{osm::*, *};
use eframe::egui::{Color32, FontId, PointerButton, Pos2, Response, Stroke, Ui};
use eframe::epaint::{CircleShape, ColorMode, PathShape, PathStroke, RectShape, StrokeKind, TextShape};
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
}

/// Data that persists or is produced between frames
#[derive(Default)]
pub struct EditorPluginState {
	pub hovered: Vec<ElementId>,
	pub selected: Option<ElementId>,
	pub map_bbox: Bbox,
	pub last_click_coords: Position,
	pub overlap_selector_elements: Vec<ElementId>,
	pub overlap_selector_pos: Pos2,
}

impl Plugin for EditorPlugin<'_> {
	// todo(optimization): cache results of way_width and way_color
	fn run(mut self: Box<Self>, ui: &mut Ui, resp: &Response, projector: &Projector, map_memory: &MapMemory) {
		/* cache invalidation */ {
			let current_pos = map_memory.detached().unwrap_or_else(school);
			if self.osm.cache_flags & CacheFlag::NodeProjection as u8 != 0 {
				self.osm.refresh_projected_nodes_cache(projector, current_pos);
			} else if !self.osm.data.nodes.is_empty() {
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
				self.osm.refresh_node_dedup_cache();
			}

			if self.osm.cache_flags & CacheFlag::WayMeshAndAreaSize as u8 != 0 {
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

			if self.osm.cache_flags & CacheFlag::AreaSizeOrdered as u8 != 0 {
				self.osm.refresh_area_size_ordered_cache();
			}
		}

		let mouse = ui.ctx().pointer_hover_pos(); // todo: touchscreen
		let should_draw_nodes = map_memory.zoom() > NODE_MIN_ZOOM;
		self.editor_state.hovered.clear();

		// override fill mode
		let mut target_fill = self.map_state.selected_fill_mode;
		if target_fill == FillMode::Partial && map_memory.zoom() < PARTIAL_FILL_THRESHOLD {
			target_fill = FillMode::Full;
		}

		/* update editor state */ {
			if resp.clicked() {
				self.editor_state.last_click_coords = projector.unproject(resp.interact_pointer_pos().unwrap().to_vec2());
			}

			let tl = projector.unproject(resp.rect.min.to_vec2());
			let br = projector.unproject(resp.rect.max.to_vec2());
			self.editor_state.map_bbox.left = tl.x();
			self.editor_state.map_bbox.bottom = br.y();
			self.editor_state.map_bbox.right = br.x();
			self.editor_state.map_bbox.top = tl.y();
		}

		// minimum capacity to prevent most reallocations
		let capacity = self.osm.area_size_ordered.len()
			+ if should_draw_nodes { self.osm.node_dedup.way_nodes.len() + self.osm.node_dedup.orphan_nodes.len() } else { 0 };

		let mut skipped = 0;
		// todo: https://github.com/Swarkin/walkers-editor/issues/20
		let mut shapes = Vec::with_capacity(capacity);

		/* draw osm data and detect interactions */ {
			let detect_interactions = self.should_detect_interactions(&mouse, SelectionFlag::Ways);

			// 1. draw areas
			// todo: is it faster to iterate over the key-value pairs directly?
			for area_id in self.osm.area_size_ordered.keys() {
				let way = self.osm.data.ways.get(area_id).expect("id not found in data");
				let points = self.osm.get_projected_positions_in_way(area_id);
				let width = self.way_width(way);
				let color = self.way_color(way);

				if detect_interactions && distance_to_way(&points, mouse.as_ref().unwrap()) < width.powi(2) {
					self.editor_state.hovered.push(ElementId::Way(*area_id));
				}

				match target_fill {
					FillMode::Wireframe => shapes.push(self.draw_way_closed_from(points, width, color).into()),
					FillMode::Partial => {
						// outline
						shapes.push(self.draw_way_closed_from(points.clone(), width, color).into());

						// partial fill
						// todo: https://github.com/Swarkin/walkers-editor/issues/9
						let area = *self.osm.area_size_ordered.get(area_id).unwrap();
						let points = if area > 0.0 {
							points.into_iter().skip(1).collect()
						} else if area < 0.0 {
							points.into_iter().rev().skip(1).collect()
						} else {
							skipped += 1;
							continue;
						};

						shapes.push(self.draw_fill_partial_from(
							points,
							PARTIAL_FILL_WIDTH,
							color.gamma_multiply(PARTIAL_FILL_GAMMA_MULTIPLY),
						).into());
					}
					FillMode::Full => {
						// draw area
						let mesh = self.osm.get_way_mesh(&way.id, color.gamma_multiply(PARTIAL_FILL_GAMMA_MULTIPLY));
						shapes.push(Arc::new(mesh).into());

						// draw stroke
						shapes.push(PathShape {
							points: points.into_iter().skip(1).collect(),
							closed: true,
							fill: Color32::TRANSPARENT,
							stroke: PathStroke::new(width, color),
						}.into());
					}
				}
			}

			// 2. draw ways
			for way_id in &self.osm.way_area.ways {
				let way = self.osm.data.ways.get(way_id).expect("id not found in data");
				let points = self.osm.get_projected_positions_in_way(way_id);
				let width = self.way_width(way);
				let color = self.way_color(way);

				if detect_interactions && distance_to_way(&points, mouse.as_ref().unwrap()) < width.powi(2) {
					self.editor_state.hovered.push(ElementId::Way(*way_id));
				}

				if self.is_way_relevant(&way.tags) {
					shapes.push(self.draw_way_from(points.clone(), width, color).into());

					#[allow(clippy::single_match)] // there will be more visualizations in the future
					match &self.map_state.selected_visualization {
						Visualization::Sidewalks => shapes.extend(visual::sidewalks(&way.tags, points, width)),
						_ => unreachable!("is_way_relevant returned invalid information"),
					}
				} else {
					shapes.push(self.draw_way_from(points, width, color).into());
				}
			}

			// 3. draw nodes
			if should_draw_nodes {
				if self.should_detect_interactions(&mouse, SelectionFlag::Nodes) {
					let way_nodes = self.osm.node_dedup.way_nodes.iter().map(|id| {
						let pos = self.osm.get_projected_pos(id).expect("id not found in cache");
						let shape = if self.osm.node_usage.get(id).expect("id not found in cache").len() > 1 {
							self.draw_node_connected_at(pos)
						} else {
							self.draw_node_at(pos)
						}.into();
						shapes.push(shape);
						(id, pos)
					}).collect::<Vec<_>>();

					let orphan_nodes = self.osm.node_dedup.orphan_nodes.iter().map(|id| {
						let pos = self.osm.get_projected_pos(id).expect("id not found in cache");
						shapes.push(self.draw_node_orphan_at(pos).into());
						(pos, id)
					}).collect::<Vec<_>>();

					let mouse = mouse.unwrap();

					// node hover detection
					let distance_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);
					for (id, pos) in way_nodes {
						if pos.distance_sq(mouse) < distance_sq {
							self.editor_state.hovered.insert(0, ElementId::Node(*id));
						}
					}

					let distance_sq = (NODE_SIZE_ORPHAN * self.map_state.scale_factor).powi(2);
					for (pos, id) in orphan_nodes {
						if pos.distance_sq(mouse) < distance_sq {
							self.editor_state.hovered.insert(0, ElementId::Node(*id));
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

		/* detect interactions on selected element */ {
			if let Some(element) = &self.editor_state.selected
				&& let ElementId::Way(w) = element
				&& self.should_detect_interactions(&mouse, SelectionFlag::Nodes)
			{
				let range_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);

				let points = self.osm.data.ways
					.get(w).expect("id not found in data")
					.nodes.iter()
					.map(|id| (id, self.osm.get_projected_pos(id).expect("id not found in cache")))
					.collect::<Vec<_>>();

				for (id, pos) in points {
					if pos.distance_sq(mouse.unwrap()) < range_sq {
						self.editor_state.hovered.insert(0, ElementId::Node(*id));
					}
				}
			}
		}

		/* draw overlap selector */ {
			if ui.ctx().input(|i| i.pointer.button_clicked(PointerButton::Middle)) {
				self.editor_state.overlap_selector_elements = self.editor_state.hovered.clone();
				self.editor_state.overlap_selector_pos = mouse.unwrap();
			}

			if !self.editor_state.overlap_selector_elements.is_empty() {
				let resolved_elements = self.editor_state.overlap_selector_elements.iter()
					.filter_map(|id| self.osm.get(id.id_ref()))
					.collect::<Vec<_>>();

				let resp = super::windows::overlap_selector(
					ui,
					self.editor_state.overlap_selector_pos,
					resolved_elements,
				);

				match resp.inner.unwrap() {
					OverlapSelectorResult::None => self.editor_state.hovered.clear(),
					OverlapSelectorResult::Hovered(e) => self.editor_state.hovered = vec![e.element_id()],
					OverlapSelectorResult::Selected(e) => self.editor_state.selected = Some(e.element_id()),
				}

				if let Some(mouse) = mouse
					&& ui.ctx().input(|i| i.pointer.any_pressed())
					&& !resp.response.rect.contains(mouse)
				{
					self.editor_state.overlap_selector_elements.clear();
				}
			}
		}

		let mut shapes_hover_tooltip = Vec::new();

		/* draw hovered element and detect whether it was selected */ {
			if let Some(hovered_element) = self.editor_state.hovered.first() && self.editor_state.hovered.first() != self.editor_state.selected.as_ref() {
				let element = self.osm.get(hovered_element.id_ref())
					.expect("id not found in data");

				// draw hovered element name tooltip
				if let Some(name) = element.tags().get("name") && self.editor_state.overlap_selector_elements.is_empty() {
					let hover = mouse.unwrap();
					let galley = ui.fonts(|f| {
						f.layout_no_wrap(name.to_owned(), FontId::proportional(HOVER_TOOLTIP_FONT_SIZE), Color32::LIGHT_GRAY)
					});
					let rect = galley.rect
						.translate(hover.to_vec2() + HOVER_TOOLTIP_OFFSET)
						.expand(4.0);

					shapes_hover_tooltip.push(RectShape::filled(rect, 4.0, HOVER_TOOLTIP_COLOR).into());
					shapes_hover_tooltip.push(TextShape::new(hover + HOVER_TOOLTIP_OFFSET, galley, Color32::PLACEHOLDER).into());
				}

				match element {
					ElementRef::Node(node) => {
						shapes.push(self.draw_node_hovered(&node.id).into());

						if resp.clicked() {
							self.editor_state.selected = Some(hovered_element.to_owned());
						}
					}
					ElementRef::Way(way) => {
						if resp.clicked() { // selected
							if self.is_way_relevant(&way.tags) || self.map_state.selected_visualization == Visualization::Default {
								self.editor_state.selected = Some(hovered_element.to_owned());
							} else { // deselect when clicking irrelevant way
								self.editor_state.selected = None;
							}
						} else {
							let closed = is_way_closed(way);
							let mut newly_hovered_node = None;

							/* detect interactions and draw nodes on hovered way */ {
								if self.should_detect_interactions(&mouse, SelectionFlag::Nodes) {
									let range_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);

									let points = way.nodes.iter()
										.skip(if closed { 1 } else { 0 })
										.map(|id| (id, self.osm.get_projected_pos(id).expect("id not found in cache")))
										.collect::<Vec<_>>();

									for (id, pos) in &points {
										if pos.distance_sq(mouse.unwrap()) < range_sq {
											newly_hovered_node = Some(*id);
										}
									}
								}
							}

							if let Some(id) = newly_hovered_node { // only draw the newly hovered node
								// todo(performance): re-use the existing points
								shapes.push(self.draw_node_selected(id).into());
							} else {
								shapes.push(if closed { self.draw_way_closed_hovered(&way.id) } else { self.draw_way_hovered(&way.id) }.into());
								shapes.extend( // draw nodes again above the selection
									way.nodes.iter().skip(if closed { 1 } else { 0 })
										.map(|id| self.draw_node_dynamic(id).into())
								);
							}
						}
					}
				}
			} else if resp.clicked() { // clicked on empty space
				self.editor_state.selected = None;
			}
		}

		/* draw selected element */ {
			if let Some(element_id) = &self.editor_state.selected {
				let element = self.osm.get(element_id.id_ref()).expect("id not found in data");

				match element {
					ElementRef::Node(node) => shapes.push(self.draw_node_selected(&node.id).into()),
					ElementRef::Way(way) => {
						let points = self.osm.get_projected_positions_in_way(&way.id);
						let width = self.way_width(way);

						if is_way_closed(way) {
							shapes.push(self.draw_way_closed_selected_from(points.iter().skip(1).copied().collect(), width).into());
							shapes.extend(
								way.nodes.iter().skip(1)
									.map(|id| self.draw_node_dynamic(id).into())
							);
						} else {
							shapes.push(self.draw_way_selected_from(points, width).into());
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

		shapes.extend(shapes_hover_tooltip);

		// we want to preallocate as much memory as possible without overallocating
		debug_assert!(shapes.len() >= capacity - skipped, "overallocated shape buffer: {} > ({capacity} - {skipped})", shapes.len());
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

	//noinspection DuplicatedCode
	fn draw_way_closed_from(&self, points: Vec<Pos2>, width: f32, color: Color32) -> PathShape {
		PathShape {
			points,
			closed: true,
			fill: Color32::default(),
			stroke: PathStroke {
				width,
				color: ColorMode::Solid(color),
				kind: StrokeKind::Middle,
			}
		}
	}

	//noinspection DuplicatedCode
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

	//noinspection DuplicatedCode
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

	//noinspection DuplicatedCode
	fn draw_way_selected_from(&self, points: Vec<Pos2>, width: f32) -> PathShape {
		PathShape {
			points,
			closed: false,
			fill: Color32::default(),
			stroke: PathStroke {
				width: width + SELECTION_SIZE_INCREASE,
				color: ColorMode::Solid(SELECTION_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	//noinspection DuplicatedCode
	fn draw_way_closed_selected_from(&self, points: Vec<Pos2>, width: f32) -> PathShape {
		PathShape {
			points,
			closed: true,
			fill: Color32::default(),
			stroke: PathStroke {
				width: width + SELECTION_SIZE_INCREASE,
				color: ColorMode::Solid(SELECTION_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	//noinspection DuplicatedCode
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
			Visualization::Default | Visualization::Sidewalks => visual::width_default(way) * self.map_state.scale_factor,
		}
	}

	fn way_color(&self, way: &Way) -> Color32 {
		match self.map_state.selected_visualization {
			Visualization::Default | Visualization::Sidewalks => visual::color_default(way),
		}
	}

	// returns whether the way is relevant for the current visualization, or false if none selected.
	fn is_way_relevant(&self, tags: &Tags) -> bool {
		match self.map_state.selected_visualization {
			Visualization::Default => false,
			Visualization::Sidewalks => visual::sidewalks_relevant(tags),
		}
	}

	fn way_editing_ui(&mut self, ui: &mut Ui, id: Id, pos: Pos2) -> Option<Change> {
		match self.map_state.selected_visualization {
			Visualization::Default => None,
			Visualization::Sidewalks => visual::sidewalks_ui(ui, self.osm.data.ways.get(&id).unwrap(), pos),
		}
	}

	fn should_detect_interactions(&self, mouse: &Option<Pos2>, selection_flag: SelectionFlag) -> bool {
		mouse.is_some()
			&& self.map_state.selection_mode & selection_flag as u8 != 0
			&& self.editor_state.overlap_selector_elements.is_empty()
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

fn distance_to_way(points: &[Pos2], mouse: &Pos2) -> f32 {
	points
		.windows(2)
		.map(|p| distance_to_segment_sq(mouse, &[p[0], p[1]]))
		.min_by(|a, b| a.partial_cmp(b).unwrap())
		.unwrap_or(f32::INFINITY)
}

fn is_way_closed(way: &Way) -> bool {
	way.nodes.first() == way.nodes.last()
}
