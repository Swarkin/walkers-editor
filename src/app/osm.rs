use osm_parser::types::*;
use std::error::Error;
use std::ops::Deref;
use super::config::TargetServer;

pub struct OsmClient {
	client: reqwest::Client,
	target_server: TargetServer,
}

impl Deref for OsmClient {
	type Target = reqwest::Client;

	fn deref(&self) -> &Self::Target {
		&self.client
	}
}


#[derive(Debug, Default)]
pub struct Bbox {
	pub left: f64,
	pub bottom: f64,
	pub right: f64,
	pub top: f64,
}

impl Bbox {
	pub fn tuple(&self) -> (f64, f64, f64, f64) {
		(self.left, self.bottom, self.right, self.top)
	}
}

// todo: error type
impl OsmClient {
	pub async fn get_map(&self, left: f64, bottom: f64, right: f64, top: f64) -> Result<OsmData, Box<dyn Error>> {
		let url = format!("{}/api/0.6/map.json?bbox={left},{bottom},{right},{top}", self.target_server.url());
		let resp = self.get(&url).send().await?.error_for_status()?;
		let raw = resp.json::<raw::RawOsmData>().await?;
		raw.try_into()
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
