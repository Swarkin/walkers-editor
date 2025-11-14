pub mod visual;
pub mod cache;
pub mod consts;
pub mod attribute2d;
pub mod states;
pub mod r_star;

use crate::app::editor::r_star::WayEntry;
use crate::app::osm::{Bbox, OrderedTags};
use crate::app::windows::{DataViewerModal, OverlapSelectorResult, WindowBitflag};
use cache::Change;
use cache::{EditorOsmData, ElementId, ElementRef, MAX_VIEW_OFFSET};
use consts::{osm::*, *};
use eframe::egui::{Color32, Context, CursorIcon, FontId, Key, Modifiers, Pos2, Response, Shape, Stroke, Ui, Vec2};
use eframe::epaint::{CircleShape, ColorMode, PathShape, PathStroke, RectShape, StrokeKind, TextShape};
use osm_parser::*;
use r_star::{NodeEntry, WebMercatorPoint};
use rstar::primitives::Rectangle;
use rstar::AABB;
use states::SelectionFlag;
use states::{CacheFlag, MapState};
use std::fmt::Display;
use std::sync::Arc;
use visual::{FillMode, Visualization};
use walkers::{MapMemory, Position, Projector};

/// State related to the editor
#[derive(Default)]
pub struct Editor {
	pub map_state: MapState,
	pub osm_data: EditorOsmData,
	pub window_flags: WindowBitflag,
	pub prev_size: Vec2,
	pub prev_zoom: f64,
	pub edit_window: Option<(ElementId, OrderedTags)>,
	pub data_viewer: Option<DataViewerModal>,

	pub mode: EditMode,
	pub operation: EditOperation,
	pub hovered: Vec<ElementId>,
	pub selected: Option<ElementId>,
	pub map_bbox: Bbox,
	pub last_click_coords: Position,
	pub overlap_selector_elements: Vec<ElementId>,
	pub overlap_selector_pos: Pos2,
	pub placeholder_id: Id,

	pub shapes: Vec<Shape>,
	pub shapes_top: Vec<Shape>,
}

#[derive(Default, Copy, Clone, PartialEq, Eq)]
pub enum EditMode {
	#[default] View,
	Edit,
}

impl EditMode {
	pub const fn color(self) -> Color32 {
		match self {
			Self::Edit => EDIT_MODE_COLOR,
			Self::View => VIEW_MODE_COLOR,
		}
	}
}

#[derive(Default, Clone)]
pub enum EditOperation {
	#[default] Idle,
	AddNode,
	AddWay(Vec<Coordinate>),
}

impl Display for EditMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Self::View => "View",
			Self::Edit => "Edit",
		})
	}
}

// drawing nodes
#[allow(clippy::trivially_copy_pass_by_ref)]
impl Editor {
	fn draw_node(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm_data.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH, color: NODE_STROKE_COLOR },
		}
	}

	const fn draw_node_at(&self, center: Pos2) -> CircleShape {
		CircleShape {
			center,
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH, color: NODE_STROKE_COLOR },
		}
	}

	fn draw_node_connected(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm_data.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_CONNECTED_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH, color: NODE_STROKE_COLOR },
		}
	}

	const fn draw_node_connected_at(&self, center: Pos2) -> CircleShape {
		CircleShape {
			center,
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_CONNECTED_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH, color: NODE_STROKE_COLOR },
		}
	}

	fn draw_node_orphan(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm_data.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE_ORPHAN * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH, color: NODE_STROKE_COLOR },
		}
	}

	const fn draw_node_orphan_at(&self, center: Pos2) -> CircleShape {
		CircleShape {
			center,
			radius: NODE_SIZE_ORPHAN * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH, color: NODE_STROKE_COLOR },
		}
	}

	fn draw_node_hovered(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm_data.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH + HOVER_SIZE_INCREASE, color: HOVER_COLOR },
		}
	}

	fn draw_node_selected(&self, id: &Id) -> CircleShape {
		CircleShape {
			center: self.osm_data.get_projected_pos(id).expect("id not found in cache"),
			radius: NODE_SIZE * self.map_state.scale_factor,
			fill: NODE_COLOR,
			stroke: Stroke { width: NODE_STROKE_WIDTH + SELECTION_SIZE_INCREASE, color: SELECTION_COLOR },
		}
	}

	fn draw_node_dynamic(&self, id: &Id) -> CircleShape {
		if self.osm_data.node_usage.get(id).expect("id not found in cache").len() > 1 {
			self.draw_node_connected(id)
		} else {
			self.draw_node(id)
		}
	}
}

// drawing ways
#[allow(clippy::trivially_copy_pass_by_ref)]
impl Editor {
	const fn draw_way_from(points: Vec<Pos2>, width: f32, color: Color32) -> PathShape {
		PathShape {
			points,
			closed: false,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width,
				color: ColorMode::Solid(color),
				kind: StrokeKind::Middle,
			}
		}
	}

	const fn draw_way_closed_from(points: Vec<Pos2>, width: f32, color: Color32) -> PathShape {
		PathShape {
			points,
			closed: true,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width,
				color: ColorMode::Solid(color),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_hovered(&self, id: &Id) -> PathShape {
		let way = self.osm_data.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm_data.get_projected_positions_in_way(id),
			closed: false,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width: self.way_width(way) + HOVER_SIZE_INCREASE,
				color: ColorMode::Solid(HOVER_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_closed_hovered(&self, id: &Id) -> PathShape {
		let way = self.osm_data.data.ways.get(id).expect("id not found in cache");
		PathShape {
			points: self.osm_data.get_projected_positions_in_way(id).into_iter().skip(1).collect(),
			closed: true,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width: self.way_width(way) + HOVER_SIZE_INCREASE,
				color: ColorMode::Solid(HOVER_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	const fn draw_way_selected_from(points: Vec<Pos2>, width: f32) -> PathShape {
		PathShape {
			points,
			closed: false,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width: width + SELECTION_SIZE_INCREASE,
				color: ColorMode::Solid(SELECTION_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	fn draw_way_closed_selected_from(points: Vec<Pos2>, width: f32) -> PathShape {
		PathShape {
			points,
			closed: true,
			fill: Color32::TRANSPARENT,
			stroke: PathStroke {
				width: width + SELECTION_SIZE_INCREASE,
				color: ColorMode::Solid(SELECTION_COLOR),
				kind: StrokeKind::Middle,
			}
		}
	}

	const fn draw_fill_partial_from(points: Vec<Pos2>, width: f32, color: Color32) -> PathShape {
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
impl Editor {
	#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
	pub fn run(&mut self, ui: &Ui, response: &Response, projector: &Projector, map_memory: &MapMemory) {
		 #[cfg(feature = "debug")] /* update frame timing */ {
			let (i, vec) = &mut self.osm_data.frame_timing;
			vec[*i] = ui.input(|i| i.unstable_dt);
			*i = (*i + 1) % vec.len();
		}

		let curr_zoom = map_memory.zoom();

		// todo: https://github.com/Swarkin/walkers-editor/issues/20
		self.shapes.clear();
		self.shapes_top.clear();

		#[allow(clippy::float_cmp)]
		if self.prev_zoom != curr_zoom {
			self.osm_data.refresh_in_view_flag = true;
		}

		let mouse = response.hover_pos();
		let clicked = response.clicked();

		let should_draw_nodes = curr_zoom > NODE_MIN_ZOOM;

		let interact_nodes = self.should_detect_interactions(mouse, SelectionFlag::Nodes);
		let interact_ways = self.should_detect_interactions(mouse, SelectionFlag::Ways);

		let current_pos = map_memory.detached().unwrap_or_default();
		let current_pos_projected = projector.project(current_pos);

		// override fill mode
		let mut target_fill = self.map_state.selected_fill_mode;
		if target_fill == FillMode::Partial && curr_zoom < PARTIAL_FILL_THRESHOLD {
			target_fill = FillMode::Full;
		}

		self.hovered.clear();

		/* update editor state */ {
			self.prev_zoom = curr_zoom;
			if clicked {
				self.last_click_coords = projector.unproject(response.interact_pointer_pos().unwrap().to_vec2());
			}

			let tl = projector.unproject(response.rect.min.to_vec2());
			let br = projector.unproject(response.rect.max.to_vec2());
			self.map_bbox.left = tl.x();
			self.map_bbox.bottom = br.y();
			self.map_bbox.right = br.x();
			self.map_bbox.top = tl.y();
		}

		/* update elements in view */ {
			if !self.osm_data.data.nodes.is_empty() || self.osm_data.refresh_in_view_flag {
				let p_start = projector.project(self.osm_data.view_start);
				let diff = p_start - current_pos_projected;

				if diff.x.abs() > MAX_VIEW_OFFSET || diff.y.abs() > MAX_VIEW_OFFSET || self.osm_data.refresh_in_view_flag {
					#[allow(clippy::cast_possible_truncation)]
					let aabb = &AABB::from_corners(
						WebMercatorPoint::from((self.map_bbox.top as f32, self.map_bbox.left as f32)),
						WebMercatorPoint::from((self.map_bbox.bottom as f32, self.map_bbox.right as f32))
					);

					self.osm_data.refresh_elements_in_view(aabb);
					self.osm_data.view_start = current_pos;
					self.osm_data.refresh_in_view_flag = false;

					self.osm_data.cache_flags = CacheFlag::ALL;
				}
			}
		}

		/* cache invalidation */ {
			if self.osm_data.cache_flags & CacheFlag::NodeUsage as u8 != 0 {
				self.osm_data.refresh_node_usage_cache();
			}

			if self.osm_data.cache_flags & CacheFlag::NodeOrphan as u8 != 0 {
				self.osm_data.refresh_orphan_nodes_cache();
			}

			if self.osm_data.cache_flags & CacheFlag::WayArea as u8 != 0 {
				self.osm_data.refresh_way_area_cache();
			}

			if self.osm_data.cache_flags & CacheFlag::NodeDedup as u8 != 0 {
				self.osm_data.refresh_node_dedup_cache();
			}

			if self.osm_data.cache_flags & CacheFlag::NodeProjection as u8 != 0 {
				self.osm_data.refresh_projected_nodes_cache(projector, current_pos);
			} else if !self.osm_data.data.nodes.is_empty() {
				let p_start = projector.project(self.osm_data.node_start);
				let diff = p_start - current_pos_projected;

				self.osm_data.node_offset_move = diff;
			}

			if self.osm_data.cache_flags & CacheFlag::WayMeshAndAreaSize as u8 != 0 {
				// it might be possible to use emath::TSTransform for more performance
				self.osm_data.refresh_way_mesh_and_area_size_cache(current_pos);
			} else if !self.osm_data.data.ways.is_empty() {
				// update move offset
				let p_start = projector.project(self.osm_data.mesh_start);
				let diff = p_start - current_pos_projected;

				self.osm_data.mesh_offset_move = diff;
			}

			if self.osm_data.cache_flags & CacheFlag::AreaSizeOrdered as u8 != 0 {
				self.osm_data.refresh_area_size_ordered_cache();
			}
		}

		/* handle edit operation */ {
			match &mut self.operation {
				EditOperation::Idle => {}
				EditOperation::AddNode => {
					if consume_key(ui.ctx(), Key::Escape, Modifiers::NONE) {
						self.operation = EditOperation::Idle;
					} else {
						ui.ctx().set_cursor_icon(CursorIcon::Crosshair);

						if clicked {
							#[allow(clippy::collapsible_if)]
							if let Some(mouse) = mouse {
								let id = self.next_placeholder_id();
								let pos = projector.unproject(mouse.to_vec2());
								let coord = Coordinate::new(pos.0.y, pos.0.x);

								#[allow(clippy::cast_possible_truncation)]
								self.osm_data.rtree_data.nodes.insert(NodeEntry::new([coord.lat as f32, coord.lon as f32], id));
								let change = Change::CreateNode(id, Node { id, pos: coord, ..Default::default() });
								self.osm_data.apply_change(change);
								self.osm_data.refresh_in_view_flag = true;

								self.operation = EditOperation::Idle;
								self.selected = Some(ElementId::Node(id));
							}
						}
					}
				}
				EditOperation::AddWay(node_coords) => {
					if consume_key(ui.ctx(), Key::Escape, Modifiers::NONE) { // cancel
						self.operation = EditOperation::Idle;
					} else {
						ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
						let mut end_way = false;

						if clicked && let Some(mouse) = mouse {
							let pos = projector.unproject(mouse.to_vec2());
							let coord = Coordinate::new(pos.0.y, pos.0.x);

							if node_coords.len() > 2 {
								let first_coord = &node_coords[0];
								let first_pos = projector.project(Position::new(first_coord.lon, first_coord.lat)).to_pos2();
								if first_pos.distance_sq(mouse) < (NODE_SIZE * self.map_state.scale_factor).powi(2) {
									end_way = true;
									node_coords.push(first_coord.clone());
								}
							}

							if !end_way { node_coords.push(coord); }
						}

						if node_coords.len() > 1 && (end_way || consume_key(ui.ctx(), Key::Enter, Modifiers::NONE)) {
							let closed_way = node_coords.first() == node_coords.last();

							let mut nodes = Vec::with_capacity(node_coords.len());
							let coords = std::mem::take(node_coords).into_iter().skip(closed_way.into()).collect::<Vec<_>>();

							#[allow(clippy::cast_possible_truncation)]
							let temp = coords.iter().map(|x| [x.lat as f32, x.lon as f32]).collect::<Vec<WebMercatorPoint>>();
							let aabb = AABB::from_points(&temp);
							drop(temp);

							for coord in coords {
								let id = self.next_placeholder_id();
								let node = Node { id, pos: coord, ..Default::default() };

								nodes.push(node.id);
								#[allow(clippy::cast_possible_truncation)]
								self.osm_data.rtree_data.nodes.insert(NodeEntry::new([node.pos.lat as f32, node.pos.lon as f32], id));
								self.osm_data.apply_change(Change::CreateNode(id, node));
							}

							if closed_way { nodes.push(nodes[0]); }

							let id = self.next_placeholder_id();
							self.osm_data.apply_change(Change::CreateWay(id, Way { id, nodes, ..Default::default() }));
							self.osm_data.rtree_data.ways.insert(WayEntry::new(Rectangle::from_aabb(aabb), id));

							self.osm_data.refresh_in_view_flag = true;
							self.operation = EditOperation::Idle;
						}
					}
				}
			}
		}

		/* draw edit operation */ {
			#[allow(clippy::single_match)]
			match &self.operation {
				EditOperation::AddWay(node_coords) => {
					let node_pos = node_coords.iter().map(|x| {
						let pos = projector.project(Position::new(x.lon, x.lat));
						pos.to_pos2()
					}).collect::<Vec<Pos2>>();

					if node_coords.len() > 1 {
						self.shapes_top.push(Self::draw_way_from(node_pos.clone(), WAY_TEMP_WIDTH, WAY_TEMP_COLOR).into());
					}

					let node_shapes = node_pos.into_iter()
						.map(|x| self.draw_node_at(x).into())
						.collect::<Vec<Shape>>();
					self.shapes_top.extend(node_shapes);
				}
				_ => {}
			}
		}

		/* draw osm data and detect interactions */ {
			// 1. draw areas
			// todo: is it faster to iterate over the key-value pairs directly?
			for area_id in self.osm_data.area_size_ordered.keys() {
				let way = self.osm_data.data.ways.get(area_id).expect("id not found in data");
				let points = self.osm_data.get_projected_positions_in_way(area_id);
				let width = self.way_width(way);
				let color = self.way_color(way);

				if interact_ways && distance_to_way(&points, mouse.unwrap()) < width.powi(2) {
					self.hovered.push(ElementId::Way(*area_id));
				}

				match target_fill {
					FillMode::Wireframe => self.shapes.push(Self::draw_way_closed_from(points, width, color).into()),
					FillMode::Partial => {
						// outline
						self.shapes.push(Self::draw_way_closed_from(points.clone(), width, color).into());

						// partial fill
						// todo: https://github.com/Swarkin/walkers-editor/issues/9
						let area = *self.osm_data.area_size_ordered.get(area_id).unwrap();
						let points = if area > 0.0 {
							points.into_iter().skip(1).collect()
						} else if area < 0.0 {
							points.into_iter().rev().skip(1).collect()
						} else {
							continue;
						};

						self.shapes.push(Self::draw_fill_partial_from(
							points,
							PARTIAL_FILL_WIDTH,
							color.gamma_multiply(PARTIAL_FILL_GAMMA_MULTIPLY),
						).into());
					}
					FillMode::Full => {
						// draw area
						let mesh = self.osm_data.get_way_mesh(&way.id, color.gamma_multiply(PARTIAL_FILL_GAMMA_MULTIPLY));
						self.shapes.push(Arc::new(mesh).into());

						// draw stroke
						self.shapes.push(PathShape {
							points: points.into_iter().skip(1).collect(),
							closed: true,
							fill: Color32::TRANSPARENT,
							stroke: PathStroke::new(width, color),
						}.into());
					}
				}
			}

			// 2. draw ways
			for way_id in &self.osm_data.way_area.ways {
				let way = self.osm_data.data.ways.get(way_id).expect("id not found in data");
				let points = self.osm_data.get_projected_positions_in_way(way_id);
				let width = self.way_width(way);
				let color = self.way_color(way);

				if interact_ways && distance_to_way(&points, mouse.unwrap()) < width.powi(2) {
					self.hovered.push(ElementId::Way(*way_id));

					if interact_nodes {
						let range_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);

						for (pos, id) in points.iter().zip(way.nodes.iter()) {
							if pos.distance_sq(mouse.unwrap()) < range_sq {
								self.hovered.insert(0, ElementId::Node(*id));
							}
						}
					}
				}

				match &self.map_state.selected_visualization {
					Visualization::Sidewalks => {
						if visual::sidewalks_relevant(&way.tags) { // todo: this can be cached
							self.shapes.extend(visual::sidewalks(&way.tags, &points, width, self.map_state.scale_factor));
						}
					},
					Visualization::Default => {},
				}

				self.shapes.push(Self::draw_way_from(points, width, color).into());
			}

			// 3. draw nodes
			if should_draw_nodes {
				if interact_nodes {
					let way_nodes = self.osm_data.node_dedup.way_nodes.iter().map(|id| {
						let pos = self.osm_data.get_projected_pos(id).expect("id not found in cache");
						(id, pos)
					}).collect::<Vec<_>>();

					let orphan_nodes = self.osm_data.node_dedup.orphan_nodes.iter().map(|id| {
						let pos = self.osm_data.get_projected_pos(id).expect("id not found in cache");
						(pos, id)
					}).collect::<Vec<_>>();

					let mouse = mouse.unwrap();

					let distance_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);
					for (id, pos) in way_nodes {
						// hit detection
						if pos.distance_sq(mouse) < distance_sq {
							self.hovered.insert(0, ElementId::Node(*id));
						}

						// drawing
						let shape = if self.osm_data.node_usage.get(id).expect("id not found in cache").len() > 1 {
							self.draw_node_connected_at(pos)
						} else {
							self.draw_node_at(pos)
						}.into();
						self.shapes.push(shape);
					}

					let distance_sq = (NODE_SIZE_ORPHAN * self.map_state.scale_factor).powi(2);
					for (pos, id) in orphan_nodes {
						// hit detection
						if pos.distance_sq(mouse) < distance_sq {
							self.hovered.insert(0, ElementId::Node(*id));
						}

						// drawing
						self.shapes.push(self.draw_node_orphan_at(pos).into());
					}
				} else { // optimized without hover detection
					for id in &self.osm_data.node_dedup.way_nodes {
						self.shapes.push(self.draw_node_dynamic(id).into());
					}

					for id in &self.osm_data.node_dedup.orphan_nodes {
						self.shapes.push(self.draw_node_orphan(id).into());
					}
				}
			}
		}

		/* draw overlap selector */ {
			if response.middle_clicked() {
				self.overlap_selector_elements.clone_from(&self.hovered);
				self.overlap_selector_pos = mouse.unwrap();
			}

			if !self.overlap_selector_elements.is_empty() {
				let resolved_elements = self.overlap_selector_elements.iter()
					.filter_map(|id| self.osm_data.get(id.id_ref()))
					.collect::<Vec<_>>();

				let resp = super::windows::overlap_selector(
					ui,
					self.overlap_selector_pos,
					resolved_elements,
				);

				match resp.inner.unwrap() {
					OverlapSelectorResult::None => self.hovered.clear(),
					OverlapSelectorResult::Hovered(e) => self.hovered = vec![e.element_id()],
					OverlapSelectorResult::Selected(e) => self.selected = Some(e.element_id()),
				}

				if clicked	&& !resp.response.contains_pointer() {
					self.overlap_selector_elements.clear();
				}
			}
		}

		/* draw hovered element and detect whether it was selected */ {
			if let Some(hovered_element) = self.hovered.first() && self.hovered.first() != self.selected.as_ref() {
				let element = self.osm_data.get(hovered_element.id_ref())
					.expect("id not found in data");

				// draw hovered element name tooltip
				if self.overlap_selector_elements.is_empty()
					&& let Some(mouse) = mouse
					&& let Some(name) = element.tags().get("name")
				{
					let galley = ui.fonts_mut(|f| {
						f.layout_no_wrap(name.to_owned(), FontId::proportional(HOVER_TOOLTIP_FONT_SIZE), Color32::LIGHT_GRAY)
					});
					let rect = galley.rect
						.translate(mouse.to_vec2() + HOVER_TOOLTIP_OFFSET)
						.expand(4.0);

					self.shapes_top.push(RectShape::filled(rect, 4.0, HOVER_TOOLTIP_COLOR).into());
					self.shapes_top.push(TextShape::new(mouse + HOVER_TOOLTIP_OFFSET, galley, Color32::PLACEHOLDER).into());
				}

				match element {
					ElementRef::Node(node) => {
						self.shapes.push(self.draw_node_hovered(&node.id).into());

						if clicked {
							self.selected = Some(hovered_element.to_owned());
						}
					}
					ElementRef::Way(way) => {
						if clicked { // selected
							if self.is_way_relevant(&way.tags) || self.map_state.selected_visualization == Visualization::Default {
								self.selected = Some(hovered_element.to_owned());
							} else { // deselect when clicking irrelevant way
								self.selected = None;
							}
						} else {
							let closed = is_way_closed(way);
							let mut newly_hovered_node = None;

							/* detect interactions and draw nodes on hovered way */ {
								if interact_nodes {
									let range_sq = (NODE_SIZE * self.map_state.scale_factor).powi(2);

									let points = way.nodes.iter()
										.skip(closed.into())
										.map(|id| (id, self.osm_data.get_projected_pos(id).expect("id not found in cache")))
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
								self.shapes.push(self.draw_node_selected(id).into());
							} else {
								self.shapes.push(if closed { self.draw_way_closed_hovered(&way.id) } else { self.draw_way_hovered(&way.id) }.into());

								let shapes = way.nodes.iter().skip(closed.into())
									.map(|id| self.draw_node_dynamic(id).into())
									.collect::<Vec<Shape>>();
								self.shapes.extend(shapes); // draw nodes again above the selection
							}
						}
					}
				}
			} else if clicked { // on empty space
				self.selected = None;
			}
		}

		/* draw selected element */
		let is_selected_element_visible = {
			if let Some(element_id) = &self.selected {
				let element = self.osm_data.get(element_id.id_ref()).expect("id not found in data");
				match element {
					ElementRef::Node(node) => {
						if self.osm_data.nodes_in_view.contains(&node.id) {
							self.shapes.push(self.draw_node_selected(&node.id).into());
							true
						} else { false }
					},
					ElementRef::Way(way) => {
						if self.osm_data.ways_in_view.contains(&way.id) {
							let points = self.osm_data.get_projected_positions_in_way(&way.id);
							let width = self.way_width(way);

							if is_way_closed(way) {
								self.shapes.push(Self::draw_way_closed_selected_from(points.iter().skip(1).copied().collect(), width).into());

								let shapes = way.nodes.iter().skip(1)
									.map(|id| self.draw_node_dynamic(id).into())
									.collect::<Vec<Shape>>();
								self.shapes.extend(shapes);
							} else {
								self.shapes.push(Self::draw_way_selected_from(points, width).into());

								let shapes = way.nodes.iter()
									.map(|id| self.draw_node_dynamic(id).into())
									.collect::<Vec<Shape>>();
								self.shapes.extend(shapes);
							}

							// draw editing ui
							if self.is_way_relevant(&way.tags)
								&& let Some(change) = self.way_editing_ui(ui, way.id, projector.project(self.last_click_coords).to_pos2())
							{
								self.osm_data.apply_change(change);
							}
							true
						} else { false }
					}
				}
			} else { false }
		};

		/* draw direction of way */ {
			if is_selected_element_visible && let Some(element) = self.selected.as_ref().or_else(|| self.hovered.first()) {
				let element = self.osm_data.get(element.id_ref()).expect("id not found in data");
				if let ElementRef::Way(w) = element {
					for section in self.osm_data.get_projected_positions_in_way(&w.id).windows(2) {
						let way_width = self.way_width(w);

						let arrow_length = way_width.mul_add(0.75, 6.5) * self.map_state.scale_factor;
						let arrow_width = way_width.mul_add(0.75, 5.0) * self.map_state.scale_factor;
						let (p1, p2) = (section[0], section[1]);
						let length = (p2 - p1).length_sq().abs();
						if length < arrow_length * 5.0 { continue; } // skip short segments

						let direction = (p2 - p1).normalized();
						let center = (p1 + p2.to_vec2()) / 2.0;

						let tip = center + direction * arrow_length;
						let side = center + direction.rot90() * arrow_width / 2.0;
						let side2 = center + direction.rot90().rot90().rot90() * arrow_width / 2.0;

						self.shapes.push(PathShape::convex_polygon(vec![side, tip, side2], Color32::WHITE, PathStroke::new(0.5 * self.map_state.scale_factor, Color32::DARK_GRAY)).into());
					}
				}
			}
		}

		/* handle delete key */ {
			if matches!(self.operation, EditOperation::Idle)
				&& consume_key(ui.ctx(), Key::Delete, Modifiers::NONE)
			{
				#[allow(clippy::collapsible_if)]
				if let Some(selected) = &self.selected
					&& let ElementId::Node(node_id) = selected
					&& self.osm_data.orphan_nodes.contains(node_id)
				{
					let node = self.osm_data.data.nodes.get(node_id).expect("id not found in data");

					#[allow(clippy::cast_possible_truncation)]
					self.osm_data.rtree_data.nodes.remove(&NodeEntry::new([node.pos.lat as f32, node.pos.lon as f32], *node_id)).unwrap();
					self.osm_data.apply_change(Change::DeleteNode(*node_id, node.to_owned()));
					self.osm_data.refresh_in_view_flag = true;

					self.hovered.clear();
					self.selected = None;
				}
			}
		}

		ui.painter().extend(self.shapes.drain(..));
		ui.painter().extend(self.shapes_top.drain(..));
	}

	const fn next_placeholder_id(&mut self) -> Id {
		self.placeholder_id -= 1;
		self.placeholder_id
	}

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

	fn way_editing_ui(&self, ui: &Ui, id: Id, pos: Pos2) -> Option<Change> {
		match self.map_state.selected_visualization {
			Visualization::Default => None,
			Visualization::Sidewalks => visual::sidewalks_ui(ui, self.osm_data.data.ways.get(&id).unwrap(), pos),
		}
	}

	const fn should_detect_interactions(&self, mouse: Option<Pos2>, selection_flag: SelectionFlag) -> bool {
		matches!(self.operation, EditOperation::Idle)
			&& mouse.is_some()
			&& self.map_state.selection_mode & selection_flag as u8 != 0
			&& self.overlap_selector_elements.is_empty()
	}
}

#[allow(clippy::many_single_char_names)]
fn distance_to_segment_sq(p: Pos2, points: &[Pos2; 2]) -> f32 {
	let x = points[0];
	let y = points[1];

	let a = p.x - x.x;
	let b = p.y - x.y;
	let c = y.x - x.x;
	let d = y.y - x.y;

	let dot = a.mul_add(c, b * d);
	let len_sq = c.mul_add(c, d * d);
	let param = if len_sq == 0f32 { -1f32 } else { dot / len_sq };

	let xx;
	let yy;

	if param < 0f32 {
		xx = x.x;
		yy = x.y;
	} else if param > 1f32 {
		xx = y.x;
		yy = y.y;
	} else {
		xx = param.mul_add(c, x.x);
		yy = param.mul_add(d, x.y);
	}

	let dx = p.x - xx;
	let dy = p.y - yy;
	dx.mul_add(dx, dy * dy)
}

fn distance_to_way(points: &[Pos2], mouse: Pos2) -> f32 {
	points
		.windows(2)
		.map(|p| distance_to_segment_sq(mouse, &[p[0], p[1]]))
		.min_by(|a, b| a.partial_cmp(b).unwrap())
		.unwrap_or(f32::INFINITY)
}

fn is_way_closed(way: &Way) -> bool {
	way.nodes.first() == way.nodes.last()
}

pub fn consume_key(ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
	ctx.input_mut(|i| i.consume_key(modifiers, key))
}
