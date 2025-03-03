use super::config::TargetServer;
use osm_parser::types::*;
use std::ops::Deref;

pub struct OsmClient {
	pub http_client: ureq::Agent,
	pub target_server: TargetServer,
}

impl Deref for OsmClient {
	type Target = ureq::Agent;

	fn deref(&self) -> &Self::Target {
		&self.http_client
	}
}

#[derive(Debug, Default)]
pub struct Bbox {
	pub left: f64,
	pub bottom: f64,
	pub right: f64,
	pub top: f64,
}

// todo: error type and unwraps
// todo: move to xml api calls at some point to get rid of json crates
impl OsmClient {
	pub fn get_map(&self, bbox: &Bbox) -> OsmData {
		let url = format!("https://{}/api/0.6/map.json?bbox={},{},{},{}", self.target_server.url(), bbox.left, bbox.bottom, bbox.right, bbox.top);
		let resp = self.get(url).call().unwrap();
		let raw = resp.into_body().read_json::<raw::RawOsmData>().unwrap();
		raw.try_into().unwrap()
	}
}

pub fn append_new_nodes_ways(to: &mut OsmData, from: OsmData) {
	for (id, way) in from.ways.into_iter() {
		// skip existing keys
		if to.ways.contains_key(&id) {
			continue;
		}

		to.ways.insert(id, way);
	}

	for (id, node) in from.nodes.into_iter() {
		// skip existing keys
		if to.nodes.contains_key(&id) {
			continue;
		}

		to.nodes.insert(id, node);
	}
}
