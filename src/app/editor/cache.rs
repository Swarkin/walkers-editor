use super::r_star::*;
use super::states::{CacheBitflag, CacheFlag};
use crate::app::editor::is_way_closed;
use crate::app::icons::*;
use eframe::egui::{Color32, ImageSource, Mesh, Pos2, TextureId, Vec2};
use eframe::epaint::{Vertex, WHITE_UV};
use indexmap::IndexMap;
use lyon_tessellation::geom::Point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};
use osm_parser::{Coordinate, Id, Node, OsmData, Tags, Way};
use rstar::AABB;
use rustc_hash::FxBuildHasher;
use std::fmt::{Display, Formatter};
use walkers::{Position, Projector};

type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
type HashSet<K> = rustc_hash::FxHashSet<K>;

pub const MAX_VIEW_OFFSET: f32 = 100.0; // arbitrary threshold, may not be required?

// Stores projected Node positions by Id.
pub type ProjectedNodeCache = HashMap<Id, Pos2>;

// Holds orphan (standalone) Node Ids.
pub type OrphanNodeCache = HashSet<Id>;

// Stores separate way and area IDs.
#[derive(Default)]
pub struct WayAreaCache {
	pub ways: HashSet<Id>,
	pub areas: HashSet<Id>,
}

// Used to avoid rendering Nodes twice when they occupy the same position.
#[derive(Default)]
pub struct NodeDedupCache {
	pub way_nodes: HashSet<Id>,
	pub orphan_nodes: HashSet<Id>,
}

// Maps Node IDs to Way IDs
pub type NodeUsageCache = HashMap<Id, Vec<Id>>;

// Contains cached MeshData, used by FillMode::Full.
pub type WayMeshCache = HashMap<Id, MeshData>;

/// Stores a list of area IDs ordered by the area size, used for rendering.
pub type AreaSizeOrderedCache = IndexMap<Id, f32, FxBuildHasher>; // can easily be refactored to use indexmap if the size is needed

#[cfg(feature = "debug")]
#[derive(Default)]
pub struct CacheDebug(pub [(u32, u32); CacheFlag::SIZE + 1]);

#[cfg(feature = "debug")]
impl CacheDebug {
	pub const fn update(&mut self, flag: CacheFlag, time: u32) {
		let entry = &mut self.0[(flag as u8).trailing_zeros() as usize];
		entry.0 = time;
		entry.1 += 1;
	}
}

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
			Self::UpdateWay(id, way) => {
				if let Some(name) = way.tags.get("name") {
					write!(f, "Updated {name}")
				} else {
					write!(f, "Updated Way {id}")
				}
			},
		}
	}
}

// stores the source data, changes, and caches.
#[derive(Default)]
pub struct EditorOsmData {
	pub data: OsmData, // latest state of the osm data
	pub rtree_data: RStarOsmData,

	pub view_start: Position,
	pub nodes_in_view: Vec<Id>,
	pub ways_in_view: Vec<Id>,
	#[cfg(feature = "debug")]
	pub view_timing: u32,
	pub refresh_in_view_flag: bool,

	pub changes: Vec<Change>,
	pub cache_flags: CacheBitflag,
	#[cfg(feature = "debug")]
	pub cache_debug: CacheDebug,

	// caches
	projected_nodes: ProjectedNodeCache,
	pub orphan_nodes: OrphanNodeCache,
	pub way_area: WayAreaCache,
	pub node_dedup: NodeDedupCache,
	pub node_usage: NodeUsageCache,
	way_mesh: WayMeshCache,
	pub area_size_ordered: AreaSizeOrderedCache,

	pub node_start: Position,
	pub mesh_start: Position,
	pub node_offset_move: Vec2,
	pub mesh_offset_move: Vec2,
	pub node_offset_resize: Vec2,
	pub mesh_offset_resize: Vec2,
}

#[derive(Debug, Clone)]
pub enum ElementRef<'a> {
	Node(&'a Node),
	Way(&'a Way),
}

impl ElementRef<'_> {
	pub const fn element_id(&self) -> ElementId {
		match self {
			ElementRef::Node(n) => ElementId::Node(n.id),
			ElementRef::Way(w) => ElementId::Way(w.id),
		}
	}

	pub const fn element_icon(&self) -> ImageSource {
		match self {
			ElementRef::Node(_) => PRIMITIVE_NODE_ICON,
			ElementRef::Way(_) => PRIMITIVE_WAY_ICON,
		}
	}

	pub const fn id_ref(&self) -> &Id {
		match self {
			ElementRef::Node(n) => &n.id,
			ElementRef::Way(w) => &w.id,
		}
	}

	pub const fn tags(&self) -> &Tags {
		match self {
			ElementRef::Node(n) => &n.tags,
			ElementRef::Way(w) => &w.tags,
		}
	}

	pub fn name(&self) -> Option<&str> {
		match self {
			ElementRef::Node(n) => n.tags.get("name").map(String::as_str),
			ElementRef::Way(w) => w.tags.get("name").map(String::as_str),
		}
	}

	pub const fn type_str(&self) -> &'static str {
		match self {
			ElementRef::Node(_) => "Node",
			ElementRef::Way(_) => "Way",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementId {
	Node(Id),
	Way(Id),
}

impl ElementId {
	pub const fn id_ref(&self) -> &Id {
		match self {
			Self::Node(id) | Self::Way(id) => id,
		}
	}
}

#[allow(clippy::trivially_copy_pass_by_ref, clippy::cast_possible_truncation)]
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

	pub fn get_projected_origin_positions_in_way(&self, way_id: &Id) -> Vec<Pos2> {
		self.data.ways.get(way_id).expect("way id must be valid")
			.nodes.iter()
			.map(|node_id| self.get_projected_origin_pos(node_id).expect("id not found in cache"))
			.collect()
	}

	pub fn get_projected_origin_pos(&self, node_id: &Id) -> Option<Pos2> {
		self.projected_nodes.get(node_id).map(ToOwned::to_owned)
	}

	pub fn get_way_mesh(&self, way_id: &Id, color: Color32) -> Mesh {
		let data = self.way_mesh.get(way_id).expect("id not found in cache");
		Mesh {
			indices: data.indices.clone(),
			vertices: data.vertices.iter().copied().map(|mut x| {
				x.color = color;
				x.pos += self.mesh_offset_move + self.mesh_offset_resize;
				x
			}).collect(),
			texture_id: TextureId::Managed(0),
		}
	}

	// Required caches:
	// - NodeDedup
	pub fn refresh_projected_nodes_cache(&mut self, projector: &Projector, start_pos: Position) {
		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		debug_assert_eq!(self.cache_flags & CacheFlag::NodeDedup as u8, 0);

		self.reset_node_offsets(start_pos);
		self.projected_nodes.clear();
		self.cache_flags &= !(CacheFlag::NodeProjection as u8);

		for node in self.ways_in_view.iter()
			.flat_map(|x| {
				let way = self.data.ways.get(x).expect("way not found in data");
				way.nodes.iter().map(|x| self.data.nodes.get(x).expect("id not found in data"))
			})
			.chain(self.node_dedup.orphan_nodes.iter().map(|x| self.data.nodes.get(x).expect("id not found in data")))
		{
			self.projected_nodes.insert(node.id, projector.project(coordinate_to_pos(&node.pos)).to_pos2());
		}

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::NodeProjection, t.elapsed().as_micros() as u32);
	}

	// No required caches
	pub fn refresh_orphan_nodes_cache(&mut self) {
		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.orphan_nodes.clear();
		self.cache_flags &= !(CacheFlag::NodeOrphan as u8);

		let mut orphans = self.nodes_in_view.iter().copied().collect::<OrphanNodeCache>();
		let mut parented = HashSet::default();

		for way in self.data.ways.values() {
			for id in &way.nodes {
				parented.insert(*id);
			}
		}

		orphans.retain(|x| !parented.contains(x));
		self.orphan_nodes = orphans;

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::NodeOrphan, t.elapsed().as_micros() as u32);
	}

	// No required caches
	pub fn refresh_way_area_cache(&mut self) {
		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.way_area.ways.clear();
		self.way_area.areas.clear();
		self.cache_flags &= !(CacheFlag::WayArea as u8);

		for way in self.ways_in_view.iter()
			.map(|x| self.data.ways.get(x).expect("way not found in data"))
		{
			if is_way_area(way) { // todo: cache is_way_area
				self.way_area.areas.insert(way.id);
			} else {
				self.way_area.ways.insert(way.id);
			}
		}

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::WayArea, t.elapsed().as_micros() as u32);
	}

	// Required caches:
	// - NodeOrphan
	// - WayArea
	pub fn refresh_node_dedup_cache(&mut self) {
		#[allow(clippy::cast_possible_truncation)]
		fn quantize_and_insert(positions: &mut HashSet<(i64, i64)>, pos: &Coordinate, amount: f64) -> bool {
			let pos_quantized = ((pos.lat * amount) as i64, (pos.lon * amount) as i64);
			positions.insert(pos_quantized)
		}

		debug_assert_eq!(self.cache_flags & (CacheFlag::NodeOrphan as u8 | CacheFlag::WayArea as u8), 0);

		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.node_dedup.way_nodes.clear();
		self.node_dedup.orphan_nodes.clear();
		self.cache_flags &= !(CacheFlag::NodeDedup as u8);

		let mut positions = HashSet::default();
		self.node_dedup.way_nodes = self.way_area.ways.iter()
			.flat_map(|id| {
				let way = self.data.ways.get(id).expect("way not found in cache");
				match way.nodes.len() {
					0 => vec![],
					1 => vec![way.nodes[0]],
					len => {
						let first = way.nodes[0];
						let last = way.nodes[len - 1];
						vec![first, last]
					}
				}
			})
			.filter(|id| quantize_and_insert(&mut positions, &self.data.nodes.get(id).expect("id not found in data").pos, 10_000_000.0))
			.collect();

		positions.clear();
		self.node_dedup.orphan_nodes = self.orphan_nodes.iter()
			.filter(|id| quantize_and_insert(&mut positions, &self.data.nodes.get(id).expect("id not found in data").pos, 10_000_000.0))
			.copied()
			.collect();

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::NodeDedup, t.elapsed().as_micros() as u32);
	}

	// No required caches
	pub fn refresh_node_usage_cache(&mut self) {
		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.node_usage.clear();
		self.cache_flags &= !(CacheFlag::NodeUsage as u8);

		for way in self.ways_in_view.iter()
			.map(|x| self.data.ways.get(x).expect("way not found in data"))
		{
			for node_id in &way.nodes {
				self.node_usage
					.entry(*node_id)
					.or_default()
					.push(way.id);
			}
		}

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::NodeUsage, t.elapsed().as_micros() as u32);
	}

	// This cache would greatly benefit from https://github.com/Swarkin/walkers-editor/issues/38
	// Required caches:
	// - WayArea
	pub fn refresh_way_mesh_and_area_size_cache(&mut self, start_pos: Position) {
		debug_assert_eq!(self.cache_flags & CacheFlag::WayArea as u8, 0);

		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.reset_mesh_offsets(start_pos);
		self.way_mesh.clear();
		self.cache_flags &= !(CacheFlag::WayMeshAndAreaSize as u8);

		for id in &self.way_area.areas {
			// next 15 lines take ~10% of the total time
			// todo: separate cache to eliminate doing this twice
			let mut points = self.get_projected_positions_in_way(id).into_iter();

			if let Some(first) = points.next() { // skip empty ways
				let mut builder = Path::builder();
				builder.begin(Point::new(first.x, first.y));

				for p in points {
					builder.line_to(Point::new(p.x, p.y));
				}

				builder.close();
				let path = builder.build();

				// next 15 lines take ~70% of the total time
				// todo: re-use vertexbuffers allocation
				let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
				let mut tessellator = FillTessellator::new();

				// todo: intersection handling
				tessellator.tessellate_path(
					&path,
					&FillOptions::default().with_intersections(false),
					&mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
						Vertex {
							pos: Pos2::from(vertex.position().to_array()),
							uv: WHITE_UV,
							color: Color32::WHITE,
						}
					}),
				).expect("path tesselation failed");

				self.way_mesh.insert(*id, MeshData {
					indices: geometry.indices,
					vertices: geometry.vertices,
				});
			}
		}

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::WayMeshAndAreaSize, t.elapsed().as_micros() as u32);
	}

	// Builds off of the WayArea cache.
	// Required caches:
	// - NodeProjection
	// - WayArea
	pub fn refresh_area_size_ordered_cache(&mut self) {
		// Shoelace formula for area calculation, returns twice the area.
		fn area_size(points: &[Pos2]) -> f32 {
			let n = points.len();
			if n < 3 {
				0.0
			} else {
				let mut area = 0.0;
				for i in 0..n {
					let p1 = points[i];
					let p2 = points[(i + 1) % n];
					area += p1.x.mul_add(p2.y, -(p2.x * p1.y));
				}

				area
			}
		}

		debug_assert_eq!(self.cache_flags & (CacheFlag::NodeProjection as u8 | CacheFlag::WayArea as u8), 0);

		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.area_size_ordered.clear();
		self.cache_flags &= !(CacheFlag::AreaSizeOrdered as u8);

		let mut area_sizes = self.way_area.areas.iter()
			.map(|area_id| {
				let points = self.get_projected_origin_positions_in_way(area_id);
				(area_id, area_size(&points))
			})
			.collect::<Vec<_>>();

		area_sizes.sort_unstable_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());

		// Does it make a difference to sort the IndexMap directly instead of the intermediate Vec?

		for (k, v) in area_sizes {
			self.area_size_ordered.insert(*k, v);
		}

		#[cfg(feature = "debug")]
		self.cache_debug.update(CacheFlag::AreaSizeOrdered, t.elapsed().as_micros() as u32);
	}

	pub fn append_new_nodes_ways(&mut self, from: OsmData) {
		if from.is_empty() { return; }

		if !from.ways.is_empty() {
			self.cache_flags |= CacheFlag::WayArea as u8 | CacheFlag::WayMeshAndAreaSize as u8 | CacheFlag::AreaSizeOrdered as u8;
		}

		if !from.nodes.is_empty() {
			self.cache_flags |= CacheFlag::NodeProjection as u8 | CacheFlag::NodeOrphan as u8 | CacheFlag::NodeDedup as u8 | CacheFlag::NodeUsage as u8;
		}

		for (id, way) in from.ways {
			// todo: handle new versions
			if self.data.ways.contains_key(&id) {
				continue;
			}

			self.data.ways.insert(id, way);
		}

		for (id, node) in from.nodes {
			// todo: handle new versions
			if self.data.nodes.contains_key(&id) {
				continue;
			}

			self.data.nodes.insert(id, node);
		}

		self.rtree_data = RStarOsmData::from(&self.data);
	}

	pub fn refresh_elements_in_view(&mut self, aabb: &AABB<WebMercatorPoint>) {
		#[cfg(feature = "debug")]
		let t = std::time::Instant::now();

		self.nodes_in_view = self.rtree_data.nodes.locate_in_envelope_intersecting(aabb)
			.map(|x| x.data)
			.collect();

		self.ways_in_view = self.rtree_data.ways.locate_in_envelope_intersecting(aabb)
			.map(|x| x.data)
			.collect();

		#[cfg(feature = "debug")] {
			self.view_timing = t.elapsed().as_micros() as u32;
		}
	}

	const fn reset_node_offsets(&mut self, start: Position) {
		self.node_offset_move = Vec2::ZERO;
		self.node_offset_resize = Vec2::ZERO;
		self.node_start = start;
	}

	const fn reset_mesh_offsets(&mut self, start: Position) {
		self.mesh_offset_move = Vec2::ZERO;
		self.mesh_offset_resize = Vec2::ZERO;
		self.mesh_start = start;
	}
}

pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::new(c.lon, c.lat)
}

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
