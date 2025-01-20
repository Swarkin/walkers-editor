use osm_parser::types::*;
use std::error::Error;

pub const BASE_URL: &str = "https://api.openstreetmap.com";

pub fn get_map(left: f64, bottom: f64, right: f64, top: f64) -> Result<OsmData, Box<dyn Error>> {
	let url = format!("{BASE_URL}/api/0.6/map.json?bbox={left},{bottom},{right},{top}");
	let client = reqwest::blocking::Client::builder()
		.user_agent(crate::USER_AGENT)
		.build()?;
	let resp = client.get(&url).send()?.error_for_status()?;
	let raw = resp.json::<raw::RawOsmData>()?;
	raw.try_into()
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
