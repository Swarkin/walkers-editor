use crate::app::editor::is_way_area;
use crate::app::editor::states::{CacheBitflag, CacheFlag};
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

pub type ProjectedNodeCache = HashMap<Id, Pos2>;
pub type OrphanNodeCache = HashSet<Id>;
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

	// caches
	projected_nodes: ProjectedNodeCache,
	pub orphan_nodes: OrphanNodeCache,
	way_meshes: WayMeshCache,

	pub cache_flags: CacheBitflag,
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

	pub fn get_projected_positions_in_way(&self, way: Id) -> Vec<Pos2> {
		self.data.ways.get(&way).expect("way id must be valid")
			.nodes.iter()
			.map(|node_id| self.get_projected_pos(node_id).expect("id not found in cache"))
			.collect()
	}

	pub fn get_projected_pos(&self, node_id: &Id) -> Option<Pos2> {
		self.projected_nodes.get(node_id).map(|pos| pos.to_owned() + self.node_offset_move + self.node_offset_resize)
	}

	pub fn get_way_mesh(&self, way_id: &Id, color: Color32) -> Mesh {
		let data = self.way_meshes.get(way_id).expect("id not found in cache");
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

	pub fn reproject_nodes(&mut self, projector: &Projector, start_pos: Position) {
		self.reset_node_offsets(start_pos);
		self.projected_nodes.clear();
		self.cache_flags &= !(CacheFlag::Projection as u8);

		for (id, node) in &self.data.nodes {
			self.projected_nodes.insert(*id, projector.project(coordinate_to_pos(&node.pos)).to_pos2());
		}
	}

	pub fn redetect_orphan_nodes(&mut self) {
		self.orphan_nodes.clear();
		self.cache_flags &= !(CacheFlag::Orphan as u8);

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

	pub fn retriangulate_way_meshes(&mut self, start_pos: Position) {
		self.reset_mesh_offsets(start_pos);
		self.way_meshes.clear();
		self.cache_flags &= !(CacheFlag::Triangulation as u8);

		for way in self.data.ways.values() {
			if is_way_area(way) {
				let points = self.get_projected_positions_in_way(way.id);
				let mut builder = Path::builder();
				builder.begin(Point::new(points[0].x, points[0].y));

				for p in points.iter().skip(1) {
					builder.line_to(Point::new(p.x, p.y));
				}

				builder.close();

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

				self.way_meshes.insert(way.id, MeshData {
					indices: geometry.indices,
					vertices: geometry.vertices,
				});
			}
		}
	}

	pub fn append_new_nodes_ways(&mut self, from: OsmData) {
		if from.is_empty() { return; }

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

		self.cache_flags ^= CacheFlag::Projection as u8;
		self.cache_flags ^= CacheFlag::Orphan as u8;
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
