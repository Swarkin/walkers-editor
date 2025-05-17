use crate::app::editor::coordinate_to_pos;
use eframe::egui::Pos2;
use osm_parser::{Id, Node, OsmData, Tags, Way};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use walkers::Projector;

type ProjectedNodeCache = HashMap<Id, Pos2>;
type OrphanNodeCache = HashSet<Id>;

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

#[derive(Debug, Default)]
pub struct EditorOsmData {
	pub data: OsmData, // latest state of the osm data
	pub changes: Vec<Change>, // list of changes
	pub projected_nodes: ProjectedNodeCache,
	pub orphan_nodes: OrphanNodeCache,
}

#[derive(Debug)]
pub enum Element<'a> {
	Node(&'a Node),
	Way(&'a Way),
}

impl Element<'_> {
	pub fn id(&self) -> Id {
		match self {
			Element::Node(n) => n.id,
			Element::Way(w) => w.id,
		}
	}

	pub fn tags(&self) -> &Tags {
		match self {
			Element::Node(n) => &n.tags,
			Element::Way(w) => &w.tags,
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

	pub fn get_by_id(&self, id: &Id) -> Option<Element> {
		self.data.nodes.get(id).map(Element::Node)
			.or_else(|| self.data.ways.get(id).map(Element::Way))
	}

	// has to be refreshed whenever the map position changes
	pub fn reproject_nodes(&mut self, projector: &Projector) {
		self.projected_nodes.clear();

		for (id, node) in &self.data.nodes {
			self.projected_nodes.insert(*id, projector.project(coordinate_to_pos(&node.pos)).to_pos2());
		}
	}

	// has to be refreshed whenever the source data changes
	pub fn detect_orphan_nodes(&mut self) {
		self.orphan_nodes.clear();

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
}
