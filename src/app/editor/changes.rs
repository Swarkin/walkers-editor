use osm_parser::{Id, Node, OsmData, Way};
use std::collections::HashMap;

pub type NodeChanges = HashMap<Id, (Change, Option<Node>)>;
pub type WayChanges = HashMap<Id, (Change, Option<Way>)>;

#[derive(Debug)]
pub enum Change {
	Created,
	Modified,
	Deleted,
}

#[derive(Debug, Default)]
pub struct EditorOsmData {
	pub original: OsmData,
	pub changes: EditorOsmDiff,
}

#[derive(Debug, Default)]
pub struct EditorOsmDiff {
	pub nodes: NodeChanges,
	pub ways: WayChanges,
}

impl EditorOsmData {
	pub fn from(osm_data: OsmData) -> Self {
		Self {
			original: osm_data,
			changes: EditorOsmDiff::default(),
		}
	}

	pub fn node(&self, id: Id) -> Option<&Node> {
		if let Some((change, node)) = self.changes.nodes.get(&id) {
			match change {
				Change::Deleted => None,
				_ => node.as_ref(),
			}
		} else {
			self.original.nodes.get(&id)
		}
	}

	pub fn way(&self, id: Id) -> Option<&Way> {
		if let Some((change, way)) = self.changes.ways.get(&id) {
			match change {
				Change::Deleted => None,
				_ => way.as_ref(),
			}
		} else {
			self.original.ways.get(&id)
		}
	}

	pub fn node_mut(&mut self, id: Id) -> Option<&mut Node> {
		if let Some((change, node)) = self.changes.nodes.get_mut(&id) {
			match change {
				Change::Deleted => panic!("attempted to modify deleted node"),
				_ => node.as_mut(),
			}
		} else { None }
	}

	pub fn way_mut(&mut self, id: Id) -> Option<&mut Way> {
		if let Some((change, way)) = self.changes.ways.get_mut(&id) {
			match change {
				Change::Deleted => panic!("attempted to get a mutable reference to a deleted way"),
				_ => way.as_mut(),
			}
		} else { None }
	}

	// get all nodes while respecting changes
	pub fn nodes(&self) -> Vec<&Node> {
		let mut nodes: Vec<_> = self.original.nodes.iter()
			.filter_map(|(id, node)| {
				if let Some((change, node)) = self.changes.nodes.get(id) {
					match change {
						Change::Created | Change::Modified => node.as_ref(),
						_ => None, // do not include deleted elements
					}
				} else { Some(node) } // use unmodified element
			}).collect();

		// append newly created elements
		nodes.append(&mut self.changes.created_nodes());

		nodes
	}

	// get all ways while respecting changes
	pub fn ways(&self) -> Vec<&Way> {
		let mut ways: Vec<_> = self.original.ways.iter()
			.filter_map(|(id, way)| {
				if let Some((change, way)) = self.changes.ways.get(id) {
					match change {
						Change::Created | Change::Modified => way.as_ref(),
						_ => None, // do not include deleted elements
					}
				} else { Some(way) } // use unmodified element
			}).collect();

		// append newly created elements
		ways.append(&mut self.changes.created_ways());

		ways
	}
}

impl EditorOsmDiff {
	// return references to created nodes
	pub fn created_nodes(&self) -> Vec<&Node> {
		self.nodes.iter().filter_map(|(_, (change, node))| {
			match change {
				Change::Created => node.as_ref(),
				_ => None,
			}
		}).collect()
	}

	// return references to created ways
	pub fn created_ways(&self) -> Vec<&Way> {
		self.ways.iter().filter_map(|(_, (change, way))| {
			match change {
				Change::Created => way.as_ref(),
				_ => None,
			}
		}).collect()
	}

/*	pub fn modified_nodes(&self) -> Vec<&Node> {
		self.nodes.iter().filter_map(|(_, (change, node))| {
			match change {
				Change::Modified => node.as_ref(),
				_ => None,
			}
		}).collect()
	}*/

/*	pub fn modified_ways(&self) -> Vec<&Way> {
		self.ways.iter().filter_map(|(_, (change, way))| {
			match change {
				Change::Modified => way.as_ref(),
				_ => None,
			}
		}).collect()
	}*/

/*	pub fn deleted_nodes(&self) -> Vec<Id> {
		self.nodes.iter().filter_map(|(id, (change, _))| {
			match change {
				Change::Deleted => Some(id.to_owned()),
				_ => None,
			}
		}).collect()
	}*/

/*	pub fn deleted_ways(&self) -> Vec<Id> {
		self.ways.iter().filter_map(|(id, (change, _))| {
			match change {
				Change::Deleted => Some(id.to_owned()),
				_ => None,
			}
		}).collect()
	}*/
}
