use super::config::TargetServer;
use super::osmchange::Tag;
use osm_parser::types::*;
use std::num::NonZeroU32;
use std::{collections::HashMap, ops::Deref, time::Duration};

const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const SCOPES: &str = "write_api";

type AnyError = Box<dyn std::error::Error + Sync + Send>;
pub type Result<T> = core::result::Result<T, AnyError>;

#[derive(Debug, Default)]
pub struct Bbox {
	pub left: f64,
	pub bottom: f64,
	pub right: f64,
	pub top: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
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

// todo(26.04.2025): auto-add authorizazion token if available
impl OsmClient {
	/// Always use `https`.
	const PROTOCOL: &'static str = "https";

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

	fn api_url(&self, path: impl AsRef<str>) -> String {
		debug_assert!(path.as_ref().starts_with('/'));
		format!("{}://{}/api/0.6{}", Self::PROTOCOL, self.target_server.base_url(), path.as_ref())
	}

	// used in OsmClient::get_map
	fn api_url_override(path: impl AsRef<str>, server: TargetServer) -> String {
		debug_assert!(path.as_ref().starts_with('/'));
		format!("{}://{}/api/0.6{}", Self::PROTOCOL, server.base_url(), path.as_ref())
	}

	// todo: error type
	// todo: move to xml api calls at some point to get rid of json crates
	pub fn get_map(&self, bbox: &Bbox) -> Result<Box<OsmData>> {
		// always use the main osm instance to fetch map data
		let url = Self::api_url_override(format!("/map.json?bbox={},{},{},{}", bbox.left, bbox.bottom, bbox.right, bbox.top), TargetServer::OpenStreetMap);
		let resp = self.get(url).call()?;
		let raw = resp.into_body().read_json::<raw::RawOsmData>()?;
		let a: OsmData = raw.try_into()?;
		Ok(Box::new(a))
	}

	// todo: error type
	pub fn create_changeset(&self, tags: Vec<Tag>) -> Result<NonZeroU32> {
		let url = self.api_url("/changeset/create");
		let auth = self.auth_token.get(&self.target_server).ok_or("missing auth token")?;
		let data = OsmCreateChangeset { changeset: RawChangeset { tags } };
		let body = quick_xml::se::to_string(&data)?;
		let resp = self.put(url)
			.header("authorization", format!("{} {}", auth.token_type, auth.access_token))
			.send(body)?;
		resp.into_body()
			.read_to_string()?
			.parse().map_err(Box::from)
	}

	// todo: error type
	pub fn close_changeset(&self, id: NonZeroU32) -> Result<()> {
		let url = self.api_url(format!("/changeset/{id}/close"));
		let auth = self.auth_token.get(&self.target_server).ok_or("missing auth token")?;
		self.put(url)
			.header("authorization", format!("{} {}", auth.token_type, auth.access_token))
			.send_empty()
			.map(|_| ())
			.map_err(Box::from)
	}

	// todo: error type
	pub fn fetch_token(&self, auth_code: String) -> Result<OsmToken> {
		let url = format!("{}://{}", Self::PROTOCOL, self.target_server.base_token_url());
		let body = format!("grant_type=authorization_code&code={auth_code}&redirect_uri={REDIRECT_URI}&client_id={}", self.target_server.client_id());
		let resp = self.post(url).header("content-type", "application/x-www-form-urlencoded").send(body)?;
		resp.into_body().read_json::<OsmToken>()
			.map_err(Box::from)
	}
}

// this isnt inside the OsmClient impl for now, since the ui code has no access to it yet
pub fn client_auth_url(server: TargetServer) -> String {
	format!("{}://{}?response_type=code&client_id={}&redirect_uri={REDIRECT_URI}&scope={SCOPES}", OsmClient::PROTOCOL, server.base_auth_url(), server.client_id())
}

pub fn append_new_nodes_ways(to: &mut OsmData, from: Box<OsmData>) {
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
