use super::config::TargetServer;
use super::osmchange::Tag;
use osm_parser::types::*;
use std::{collections::HashMap, ops::Deref, time::Duration};

const CLIENT_ID_DEV: &str = "55c2UqVCKGU_KEhQj4B5wGZHL6fR2dVS5zkwBfkiGd0";
const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const SCOPES: &str = "write_api";

#[derive(Debug, Default)]
pub struct Bbox {
	pub left: f64,
	pub bottom: f64,
	pub right: f64,
	pub top: f64,
}

#[derive(Debug, serde::Deserialize)]
pub struct OsmToken {
	pub access_token: String,
	pub token_type: String, // "Bearer"
	pub scope: String,
	pub created_at: u64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename = "osm")]
pub struct OsmCreateChangeset {
	changeset: RawChangeset,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename = "changeset")]
pub struct RawChangeset {
	tags: Vec<Tag>
}

pub struct OsmClient {
	pub http_client: ureq::Agent,
	pub target_server: TargetServer,
	pub auth_token: HashMap<TargetServer, OsmToken>,
}

impl Deref for OsmClient {
	type Target = ureq::Agent;

	fn deref(&self) -> &Self::Target {
		&self.http_client
	}
}

impl OsmClient {
	pub fn new(target_server: TargetServer) -> Self {
		Self {
			http_client: ureq::Agent::config_builder()
				.user_agent(crate::USER_AGENT)
				.https_only(true)
				.max_redirects(0)
				.timeout_global(Some(Duration::from_secs(30)))
				.build().into(),
			target_server,
			auth_token: HashMap::new(),
		}
	}

	pub fn auth_url(&self) -> String {
		auth_url(self.target_server)
	}

	// todo: error type and unwraps
	// todo: move to xml api calls at some point to get rid of json crates
	pub fn get_map(&self, bbox: &Bbox) -> OsmData {
		let url = format!("https://{}/api/0.6/map.json?bbox={},{},{},{}", self.target_server.base_url(), bbox.left, bbox.bottom, bbox.right, bbox.top);
		let resp = self.get(url).call().unwrap();
		let raw = resp.into_body().read_json::<raw::RawOsmData>().unwrap();
		raw.try_into().unwrap()
	}

	// todo: error type and unwraps
	pub fn create_changeset(&self, tags: Vec<Tag>) -> u32 {
		let url = format!("https://{}/api/0.6/changeset/create", self.target_server.base_url());
		let auth = self.auth_token.get(&self.target_server).unwrap();
		let data = OsmCreateChangeset { changeset: RawChangeset { tags } };
		let body = quick_xml::se::to_string(&data).unwrap();
		let resp = self.put(url).header("authorization", format!("{} {}", auth.token_type, auth.access_token)).send(body).unwrap();
		resp.into_body()
			.read_to_string().unwrap()
			.parse().unwrap()
	}

	// todo: error type and unwraps
	pub fn fetch_token(&self, auth_code: String) -> OsmToken {
		let url = format!("https://{}", self.target_server.base_token_url());
		let body = format!("grant_type=authorization_code&code={auth_code}&redirect_uri={REDIRECT_URI}&client_id={CLIENT_ID_DEV}");
		let resp = self.post(url).header("content-type", "application/x-www-form-urlencoded").send(body).unwrap();
		resp.into_body().read_json::<OsmToken>().unwrap()
	}
}

pub fn auth_url(target_server: TargetServer) -> String {
	if target_server == TargetServer::OpenStreetMap {
		todo!();
	}
	format!("{}?response_type=code&client_id={CLIENT_ID_DEV}&redirect_uri={REDIRECT_URI}&scope={SCOPES}", target_server.base_auth_url())
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
