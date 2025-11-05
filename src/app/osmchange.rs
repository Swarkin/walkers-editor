use super::editor::cache::Change;
use crate::HashMap;
use quick_xml::{se::Serializer, SeError};
use serde::{Deserialize, Serialize};

pub type Id = i64;
pub type ChangesetId = u64;
pub type VersionId = u32;

#[derive(Default, Serialize, Deserialize)]
pub struct OsmChange {
	#[serde(rename = "@generator")]
	pub generator: String,
	pub create: Option<Create>,
	pub modify: Option<Modify>,
	pub delete: Option<Delete>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Create {
	pub node: Vec<Node>,
	pub way: Vec<Way>,
}

impl Create {
	pub const fn is_empty(&self) -> bool {
		self.node.is_empty() && self.way.is_empty()
	}
}

#[derive(Default, Serialize, Deserialize)]
pub struct Modify {
	pub node: Vec<Node>,
	pub way: Vec<Way>,
}

impl Modify {
	pub const fn is_empty(&self) -> bool {
		self.node.is_empty() && self.way.is_empty()
	}
}

#[derive(Serialize, Deserialize)]
pub struct Delete {
	#[serde(rename = "@if-unused")]
	if_unused: bool,
	pub node: Vec<DeletedNode>,
	pub way: Vec<Way>,
}

impl Default for Delete {
	fn default() -> Self {
		Self { if_unused: true, node: vec![], way: vec![] }
	}
}

impl Delete {
	pub const fn is_empty(&self) -> bool {
		self.node.is_empty() && self.way.is_empty()
	}
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Node {
	#[serde(rename = "@id")]
	pub id: Id,
	// #[serde(rename = "@changeset", skip_serializing)]
	// pub changeset: ChangesetId,
	#[serde(rename = "@version")]
	pub version: VersionId,
	#[serde(rename = "@lon")]
	#[serde(serialize_with = "serialize_f64_7")]
	pub lon: f64,
	#[serde(rename = "@lat")]
	#[serde(serialize_with = "serialize_f64_7")]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct DeletedNode {
	#[serde(rename = "@id")]
	pub id: Id,
	#[serde(rename = "@version")]
	pub version: VersionId,
	#[serde(rename = "@changeset")]
	pub changeset: ChangesetId,
}

impl From<&osm_parser::Node> for DeletedNode {
	fn from(value: &osm_parser::Node) -> Self {
		Self {
			id: value.id,
			version: value.version,
			changeset: value.changeset,
		}
	}
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Way {
	#[serde(rename = "@id")]
	pub id: Id,
	// #[serde(rename = "@changeset", skip_serializing)]
	// pub changeset: ChangesetId,
	#[serde(rename = "@version")]
	pub version: VersionId,
	#[serde(rename = "nd")]
	pub nd: Vec<Nd>,
	#[serde(rename = "tag")]
	pub tags: Vec<Tag>,
}

impl From<&osm_parser::Way> for Way {
	fn from(value: &osm_parser::Way) -> Self {
		Self {
			id: value.id,
			// changeset: value.changeset,
			version: value.version,
			nd: value.nodes.iter().map(|&r#ref| Nd { r#ref }).collect(),
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Nd {
	#[serde(rename = "@ref")]
	r#ref: Id,
}

impl OsmChange {
	pub fn from(changes: &Vec<Change>) -> Self {
		let mut created_nodes = HashMap::default();
		let mut created_ways = HashMap::default();
		let mut modified_nodes = HashMap::default();
		let mut modified_ways = HashMap::default();
		let mut deleted_nodes = HashMap::default();
		// let mut deleted_ways = HashMap::default();

		let mut create = Create::default();
		let mut modify = Modify::default();
		let mut delete = Delete::default();

		for change in changes {
			match change {
				Change::CreateNode(_, node) => { created_nodes.insert(node.id, node); }
				Change::CreateWay(_, way) => { created_ways.insert(way.id, way); }
				Change::ModifyNode(_, node) => { modified_nodes.insert(node.id, node); }
				Change::ModifyWay(_, way) => { modified_ways.insert(way.id, way); }
				Change::DeleteNode(_, node) => { deleted_nodes.insert(node.id, node); }
			}
		}

		for node in created_nodes.into_values() { create.node.push(node.into()); }
		for way in created_ways.into_values() { create.way.push(way.into()); }

		for node in modified_nodes.into_values() {
			let mut n: Node = node.into();
			n.version += 1;
			modify.node.push(n);
		}
		for way in modified_ways.into_values() {
			let mut w: Way = way.into();
			w.version += 1;
			modify.way.push(w);
		}

		for node in deleted_nodes.into_values() { delete.node.push(node.into()); }

		Self {
			generator: crate::USER_AGENT.into(),
			create: if create.is_empty() { None } else { Some(create) },
			modify: if modify.is_empty() { None } else { Some(modify) },
			delete: if delete.is_empty() { None } else { Some(delete) },
		}
	}

	pub fn clear(&mut self) {
		self.generator.clear();
		self.create = None;
		self.modify = None;
		self.delete = None;
	}

	pub fn to_string_pretty(&self) -> Result<String, SeError> {
		let mut buffer = String::new();
		let mut ser = Serializer::with_root(&mut buffer, Some("osmChange"))?;
		ser.indent('\t', 1);
		self.serialize(ser)?;
		Ok(buffer)
	}
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn serialize_f64_7<S>(val: &f64, serializer: S) -> Result<S::Ok, S::Error>
where S: serde::Serializer {
	serializer.serialize_str(&format!("{val:.7}"))
}
