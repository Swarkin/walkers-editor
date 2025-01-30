use osm_parser::{Id, Node, OsmData, Way};
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum Change {
	UpdateNode(Id, Node),
	UpdateWay(Id, Way),
}

impl Display for Change {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Change::UpdateNode(id, _) => write!(f, "Update Node {id}"),
			Change::UpdateWay(id, _) => write!(f, "Update Way {id}"),
		}
	}
}

#[derive(Debug, Default)]
pub struct EditorOsmData {
	pub data: OsmData, // latest state of the osm data
	pub changes: Vec<Change>, // list of changes
}

impl EditorOsmData {
	pub fn apply_change(&mut self, change: Change) {
		match &change {
			Change::UpdateNode(id, updated_node) => {
				self.data.nodes.insert(*id, updated_node.clone());
			}
			Change::UpdateWay(id, updated_way) => {
				self.data.ways.insert(*id, updated_way.clone());
			}
		}
		
		self.changes.push(change);
	}
}
