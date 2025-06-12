use super::states::{CacheBitflag, CacheFlag};
use crate::app::editor::is_way_closed;
use eframe::egui::{Color32, Mesh, Pos2, TextureId, Vec2};
use eframe::epaint::{Vertex, WHITE_UV};
use lyon_tessellation::geom::Point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use osm_parser::{Coordinate, Id, Node, OsmData, Tags, Way};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use walkers::{Position, Projector};

pub const MAX_OFFSET: f32 = 4000.0; // arbitrary threshold, may not be required?

// Stores projected Node positions by Id.
pub type ProjectedNodeCache = HashMap<Id, Pos2>;

// Holds orphan (standalone) Node Ids.
pub type OrphanNodeCache = HashSet<Id>;

// Stores whether a way is detected to be an area.
pub type WayAreaCache = HashMap<Id, bool>;

// Used to avoid rendering Nodes twice when they occupy the same position.
#[derive(Default)]
pub struct NodeDedupCache {
	pub way_nodes: HashSet<Id>,
	pub orphan_nodes: HashSet<Id>,
}

impl NodeDedupCache {
	pub fn clear(&mut self) {
		self.way_nodes.clear();
		self.orphan_nodes.clear();
	}

	pub fn len(&self) -> usize {
		self.way_nodes.len() + self.orphan_nodes.len()
	}
}

// Contains cached MeshData, used by FillMode::Full.
pub type WayMeshCache = HashMap<Id, MeshData>;

pub struct MeshData {
	pub indices: Vec<u32>,
	pub vertices: Vec<Vertex>,
}

#[derive(Debug)]
pub enum Change {
	UpdateWay(Id, Way),
}

impl Display for Change {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Change::UpdateWay(id, way) => {
				if let Some(name) = way.tags.get("name") {
					write!(f, "Updated {name}")
				} else {
					write!(f, "Updated Way {id}")
				}
			},
		}
	}
}

// stores the soure data, changes, and handles caching.
#[derive(Default)]
pub struct EditorOsmData {
	pub data: OsmData, // latest state of the osm data
	pub changes: Vec<Change>,
	pub cache_flags: CacheBitflag,

	// caches
	projected_nodes: ProjectedNodeCache,
	pub orphan_nodes: OrphanNodeCache,
	pub way_area: WayAreaCache,
	pub node_dedup: NodeDedupCache,
	way_mesh: WayMeshCache,

	pub node_start: Position,
	pub mesh_start: Position,
	pub node_offset_move: Vec2,
	pub mesh_offset_move: Vec2,
	pub node_offset_resize: Vec2,
	pub mesh_offset_resize: Vec2,
}

#[derive(Debug)]
pub enum ElementRef<'a> {
	Node(&'a Node),
	Way(&'a Way),
}

impl ElementRef<'_> {
	pub fn tags(&self) -> &Tags {
		match self {
			ElementRef::Node(n) => &n.tags,
			ElementRef::Way(w) => &w.tags,
		}
	}
}

impl EditorOsmData {
	pub fn apply_change(&mut self, change: Change) {
		match change {
			Change::UpdateWay(id, way) => {
				self.data.ways.insert(id, way.clone());

				if let Some(Change::UpdateWay(prev_id, prev_way)) = self.changes.last_mut() {
					if *prev_id == id {
						*prev_way = way;
						return; // do not record a new change
					}
				}

				self.changes.push(Change::UpdateWay(id, way));
			}
		}
	}

	pub fn get(&self, id: &Id) -> Option<ElementRef> {
		self.data.nodes.get(id).map(ElementRef::Node)
			.or_else(|| self.data.ways.get(id).map(ElementRef::Way))
	}

	pub fn get_projected_positions_in_way(&self, way_id: &Id) -> Vec<Pos2> {
		self.data.ways.get(way_id).expect("way id must be valid")
			.nodes.iter()
			.map(|node_id| self.get_projected_pos(node_id).expect("id not found in cache"))
			.collect()
	}

	pub fn get_projected_pos(&self, node_id: &Id) -> Option<Pos2> {
		self.projected_nodes.get(node_id).map(|pos| pos.to_owned() + self.node_offset_move + self.node_offset_resize)
	}

	pub fn get_way_mesh(&self, way_id: &Id, color: Color32) -> Mesh {
		let data = self.way_mesh.get(way_id).expect("id not found in cache");
		Mesh {
			indices: data.indices.clone(),
			vertices: data.vertices.iter().cloned().map(|mut x| {
				x.color = color;
				x.pos += self.mesh_offset_move + self.mesh_offset_resize;
				x
			}).collect(),
			texture_id: TextureId::Managed(0),
		}
	}

	// No required caches
	pub fn refresh_projected_nodes_cache(&mut self, projector: &Projector, start_pos: Position) {
		self.reset_node_offsets(start_pos);
		self.projected_nodes.clear();
		self.cache_flags &= !(CacheFlag::NodeProjection as u8);

		for (id, node) in &self.data.nodes {
			self.projected_nodes.insert(*id, projector.project(coordinate_to_pos(&node.pos)).to_pos2());
		}
	}

	// No required caches
	pub fn refresh_orphan_nodes_cache(&mut self) {
		self.orphan_nodes.clear();
		self.cache_flags &= !(CacheFlag::NodeOrphan as u8);

		let mut orphans = self.data.nodes.keys().copied().collect::<OrphanNodeCache>();
		let mut parented = HashSet::new();

		for way in self.data.ways.values() {
			for id in &way.nodes {
				parented.insert(id);
			}
		}

		orphans.retain(|x| !parented.contains(x));
		self.orphan_nodes = orphans;
	}

	// No required caches
	pub fn refresh_way_area_cache(&mut self) {
		self.way_area.clear();
		self.cache_flags &= !(CacheFlag::WayArea as u8);

		for way in self.data.ways.values() {
			self.way_area.insert(way.id, is_way_area(way));
		}
	}

	// Required caches:
	// - NodeOrphan
	// - WayArea
	pub fn refresh_way_nodes_dedup_cache(&mut self) {
		#[cfg(debug_assertions)] {
			assert_eq!(self.cache_flags & (CacheFlag::NodeOrphan as u8 | CacheFlag::WayArea as u8), 0);
		}

		self.node_dedup.clear();
		self.cache_flags &= !(CacheFlag::NodeDedup as u8);

		let mut positions = HashSet::new();
		self.node_dedup.way_nodes = self.data.ways.values()
			.flat_map(|way| {
				if !*self.way_area.get(&way.id).expect("way not found in cache") {
					match way.nodes.len() {
						0 => vec![],
						1 => vec![way.nodes[0]],
						len => {
							let first = way.nodes[0];
							let last = way.nodes[len - 1];
							vec![first, last]
						}
					}
				} else { vec![] }
			})
			.filter(|id| {
				let pos_quantized = coordinate_quantized(&self.data.nodes.get(id).unwrap().pos, 10000000.0);
				positions.insert(pos_quantized)
			})
			.collect();

		positions.clear();
		self.node_dedup.orphan_nodes = self.orphan_nodes.iter()
			.filter(|id| {
				let pos_quantized = coordinate_quantized(&self.data.nodes.get(id).unwrap().pos, 10000000.0);
				positions.insert(pos_quantized)
			})
			.copied()
			.collect();
	}

	// Required caches:
	// - WayArea
	pub fn refresh_way_mesh_cache(&mut self, start_pos: Position) {
		#[cfg(debug_assertions)] {
			assert_eq!(self.cache_flags & CacheFlag::WayArea as u8, 0);
		}

		self.reset_mesh_offsets(start_pos);
		self.way_mesh.clear();
		self.cache_flags &= !(CacheFlag::WayMesh as u8);

		for way in self.data.ways.values() {
			if *self.way_area.get(&way.id).expect("way not found in cache") {
				let points = self.get_projected_positions_in_way(&way.id);
				let mut builder = Path::builder();
				builder.begin(Point::new(points[0].x, points[0].y));

				for p in points.iter().skip(1) {
					builder.line_to(Point::new(p.x, p.y));
				}

				builder.close();

				// todo: re-use vertexbuffers allocation
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
							color: Color32::WHITE,
						}
					}),
				).expect("path tesselation failed");

				self.way_mesh.insert(way.id, MeshData {
					indices: geometry.indices,
					vertices: geometry.vertices,
				});
			}
		}
	}

	pub fn append_new_nodes_ways(&mut self, from: OsmData) {
		if from.is_empty() { return; }

		if !from.ways.is_empty() {
			self.cache_flags |= CacheFlag::WayArea as u8 | CacheFlag::WayMesh as u8;
		}

		if !from.nodes.is_empty() {
			self.cache_flags |= CacheFlag::NodeProjection as u8 | CacheFlag::NodeOrphan as u8 | CacheFlag::NodeDedup as u8;
		}

		for (id, way) in from.ways {
			// todo: handle new versions
			if self.data.ways.contains_key(&id) {
				continue;
			}

			self.data.ways.insert(id, way);
		}

		for (id, node) in from.nodes.into_iter() {
			// todo: handle new versions
			if self.data.nodes.contains_key(&id) {
				continue;
			}

			self.data.nodes.insert(id, node);
		}
	}

	fn reset_node_offsets(&mut self, start: Position) {
		self.node_offset_move = Vec2::ZERO;
		self.node_offset_resize = Vec2::ZERO;
		self.node_start = start;
	}

	fn reset_mesh_offsets(&mut self, start: Position) {
		self.mesh_offset_move = Vec2::ZERO;
		self.mesh_offset_resize = Vec2::ZERO;
		self.mesh_start = start;
	}
}

pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::new(c.lon, c.lat)
}

// Workaround to use Eq and Hash for Coordinates
pub fn coordinate_quantized(c: &Coordinate, scale: f64) -> (u64, u64) { ((c.lat * scale) as u64, (c.lon * scale) as u64) }

// Primitive area detection
fn is_way_area(way: &Way) -> bool {
	if !is_way_closed(way) || way.nodes.len() < 3 || way.tags.is_empty() { return false; }

	if let Some(area) = way.tags.get("area") {
		match area.as_str() {
			"yes" => return true,
			"no" => return false,
			_ => {},
		}
	}

	for key in ["building", "landuse", "natural", "leisure", "amenity", "playground"] {
		if way.tags.contains_key(key) { return true; }
	}

	false
}
