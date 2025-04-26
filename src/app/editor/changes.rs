use osm_parser::{Id, Node, OsmData, Tags, Way};
use std::fmt::{Display, Formatter};

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

				if let Some(Change::UpdateWay(prev_id, prev_way)) = self.last_change_mut() {
					if *prev_id == id {
						*prev_way = way;
						return; // do not record a new change
					}
				}

				self.changes.push(Change::UpdateWay(id, way));
			}
		}
	}

	pub fn last_change_mut(&mut self) -> Option<&mut Change> {
		self.changes.last_mut()
	}

	pub fn get_by_id(&self, id: &Id) -> Option<Element> {
		self.data.nodes.get(id).map(Element::Node)
			.or_else(|| self.data.ways.get(id).map(Element::Way))
	}
}
