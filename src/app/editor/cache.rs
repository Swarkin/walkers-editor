use crate::app::editor::states::{CacheBitflag, CacheFlag};
use eframe::egui::{Pos2, Vec2};
use osm_parser::{Coordinate, Id, Node, OsmData, Tags, Way};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use walkers::{Position, Projector};

pub const MAX_OFFSET: f32 = 4000.0; // arbitrary threshold, may not be required?

pub type ProjectedNodeCache = HashMap<Id, Pos2>;
pub type OrphanNodeCache = HashSet<Id>;

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
#[derive(Debug, Default)]
pub struct EditorOsmData {
	pub data: OsmData, // latest state of the osm data
	pub changes: Vec<Change>,

	// caches
	projected_nodes: ProjectedNodeCache,
	pub orphan_nodes: OrphanNodeCache,

	pub cache_flags: CacheBitflag,
	pub start_pos: Position,
	pub offset: Vec2,
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

	pub fn get_node_positions_in_way_owned(&self, way: Id) -> Vec<Pos2> {
		self.data.ways.get(&way).expect("way id must be valid")
			.nodes.iter()
			.map(|node_id| self.get_projected_pos_owned(node_id).expect("id not found in cache"))
			.collect()
	}

	pub fn get_projected_pos_owned(&self, node_id: &Id) -> Option<Pos2> {
		self.projected_nodes.get(node_id).map(|pos| pos.to_owned() + self.offset)
	}

	pub fn reproject_nodes(&mut self, projector: &Projector, start_pos: Position) {
		self.projected_nodes.clear();
		self.start_pos = start_pos;
		self.offset = Vec2::ZERO;
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
}

pub fn coordinate_to_pos(c: &Coordinate) -> Position {
	Position::new(c.lon, c.lat)
}
