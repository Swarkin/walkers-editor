// osmchange data structures
// todo: find a way to reduce number of structs and conversions

use super::editor::cache::Change;
use quick_xml::{se::Serializer, SeError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type Id = i64;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OsmChange {
	#[serde(rename = "@generator")]
	pub generator: String,
	pub create: Option<Create>,
	pub modify: Option<Modify>,
	//pub delete: Option<Delete>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Create {
	pub node: Vec<Node>,
	pub way: Vec<Way>,
}

impl Create {
	pub const fn is_empty(&self) -> bool {
		self.node.is_empty() && self.way.is_empty()
	}
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Modify {
	pub node: Vec<Node>,
	pub way: Vec<Way>,
}

impl Modify {
	pub const fn is_empty(&self) -> bool {
		self.node.is_empty() && self.way.is_empty()
	}
}

/*#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Delete {
	pub node: Vec<Node>,
	pub way: Vec<Way>,
}

impl Delete {
	pub fn is_empty(&self) -> bool {
		self.node.is_empty() && self.way.is_empty()
	}
}*/

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Node {
	#[serde(rename = "@id")]
	pub id: Id,
	// #[serde(rename = "@changeset")]
	// #[serde(skip_serializing)]
	// pub changeset: u64,
	#[serde(rename = "@version")]
	pub version: u32,
	#[serde(rename = "@lon")]
	pub lon: f64,
	#[serde(rename = "@lat")]
	pub lat: f64,
	#[serde(rename = "tag")]
	pub tags: Vec<Tag>,
}

impl From<&osm_parser::Node> for Node {
	fn from(value: &osm_parser::Node) -> Self {
		Self {
			id: value.id,
			// changeset: value.changeset,
			version: value.version,
			lon: value.pos.lon,
			lat: value.pos.lat,
			tags: value.tags.iter().map(Into::into).collect(),
		}
	}
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Way {
	#[serde(rename = "@id")]
	pub id: Id,
	// #[serde(rename = "@changeset")]
	// #[serde(skip_serializing)]
	// pub changeset: u64,
	#[serde(rename = "@version")]
	pub version: u32,
	#[serde(rename = "tag")]
	pub tags: Vec<Tag>,
}

impl From<&osm_parser::Way> for Way {
	fn from(value: &osm_parser::Way) -> Self {
		Self {
			id: value.id,
			// changeset: value.changeset,
			version: value.version,
			tags: value.tags.iter().map(Into::into).collect(),
		}
	}
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Tag {
	#[serde(rename = "@k")]
	pub k: String,
	#[serde(rename = "@v")]
	pub v: String,
}

impl From<(&String, &String)> for Tag {
	fn from(value: (&String, &String)) -> Self {
		Self { k: value.0.to_owned(), v: value.1.to_owned() }
	}
}

#[allow(unused)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Nd {
	r#ref: Id,
}

impl OsmChange {
	pub fn from(changes: &Vec<Change>) -> Self {
		let mut created_nodes = HashMap::new();
		let mut modified_ways = HashMap::new();

		let mut create = Create::default();
		let mut modify = Modify::default();
		//let delete = Delete::default();

		for change in changes {
			match change {
				Change::CreateNode(_, node) => {
					created_nodes.insert(node.id, node);
				}
				Change::ModifyWay(_, way) => {
					modified_ways.insert(way.id, way);
				}
			}
		}

		for node in created_nodes.into_values() {
			create.node.push(node.into());
		}

		for way in modified_ways.into_values() {
			let mut w: Way = way.into();
			w.version += 1;
			modify.way.push(w);
		}

		Self {
			generator: crate::USER_AGENT.into(),
			create: if create.is_empty() { None } else { Some(create) },
			modify: if modify.is_empty() { None } else { Some(modify) },
			//delete: if delete.is_empty() { None } else { Some(delete) },
		}
	}

	pub fn to_string_pretty(&self) -> Result<String, SeError> {
		let mut buffer = String::new();
		let mut ser = Serializer::with_root(&mut buffer, Some("osmChange"))?;
		ser.indent(' ', 2);
		self.serialize(ser)?;
		Ok(buffer)
	}
}
