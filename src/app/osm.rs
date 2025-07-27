use super::osmchange::Tag;

#[cfg(not(target_family = "wasm"))]
pub use native::OsmClient;

#[cfg(target_family = "wasm")]
pub use web::OsmClient;

const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const SCOPES: &str = "write_api";

type AnyError = Box<dyn std::error::Error + Sync + Send>;
pub type OsmResult<T> = Result<T, AnyError>;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TargetServer {
	OpenStreetMap,
	#[default]
	OpenStreetMapDev,
}

impl TargetServer {
	pub const ITER: [Self; 2] = [Self::OpenStreetMap, Self::OpenStreetMapDev];
	pub const SIZE: usize = Self::ITER.len();

	pub const fn description(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "OpenStreetMap main instance",
			Self::OpenStreetMapDev => "OpenStreetMap test instance",
		}
	}

	pub const fn base_url(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "www.openstreetmap.org",
			Self::OpenStreetMapDev => "master.apis.dev.openstreetmap.org",
		}
	}

	pub const fn base_token_url(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "www.openstreetmap.org/oauth2/token",
			Self::OpenStreetMapDev => "master.apis.dev.openstreetmap.org/oauth2/token",
		}
	}

	pub const fn base_auth_url(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "www.openstreetmap.org/oauth2/authorize",
			Self::OpenStreetMapDev => "master.apis.dev.openstreetmap.org/oauth2/authorize",
		}
	}

	pub const fn client_id(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "",
			Self::OpenStreetMapDev => "55c2UqVCKGU_KEhQj4B5wGZHL6fR2dVS5zkwBfkiGd0",
		}
	}
}

#[derive(Debug, Default, Clone)]
pub struct Bbox {
	pub left: f64,
	pub bottom: f64,
	pub right: f64,
	pub top: f64,
}

impl Bbox {
	pub fn area(&self) -> f64 {
		(self.right - self.left) * (self.top - self.bottom)
	}
}

#[allow(dead_code)]
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

fn api_url(path: impl AsRef<str>, target_server: TargetServer) -> String {
	debug_assert!(path.as_ref().starts_with('/'));
	format!("https://{}/api/0.6{}", target_server.base_url(), path.as_ref())
}

// used in OsmClient::get_map
fn api_url_override(path: impl AsRef<str>, server: TargetServer) -> String {
	debug_assert!(path.as_ref().starts_with('/'));
	format!("https://{}/api/0.6{}", server.base_url(), path.as_ref())
}

pub fn client_auth_url(server: TargetServer) -> String {
	format!("https://{}?response_type=code&client_id={}&redirect_uri={REDIRECT_URI}&scope={SCOPES}", server.base_auth_url(), server.client_id())
}

#[cfg(not(target_family = "wasm"))]
mod native {
	use super::*;
	use crate::app::osm::TargetServer;
	use osm_parser::types::raw;
	use osm_parser::OsmData;
	use std::num::NonZeroU32;
	use std::time::Duration;

	pub struct OsmClient {
		pub http_client: ureq::Agent,
		pub target_server: TargetServer,
		pub auth_token: [Option<OsmToken>; TargetServer::SIZE],
	}

	// todo: auto-add authorization token if available
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
				auth_token: Default::default(),
			}
		}

		// todo: error type
		// todo: move to xml api calls at some point to get rid of json crates
		pub fn get_map(&self, bbox: &Bbox) -> OsmResult<OsmData> {
			// always use the main osm instance to fetch map data
			let url = api_url_override(format!("/map.json?bbox={},{},{},{}", bbox.left, bbox.bottom, bbox.right, bbox.top), TargetServer::OpenStreetMap);
			let resp = self.http_client.get(url).call()?;
			let raw = resp.into_body().read_json::<raw::RawOsmData>()?;
			raw.try_into()
		}

		// todo: error type
		pub fn create_changeset(&self, tags: Vec<Tag>) -> OsmResult<NonZeroU32> {
			let url = api_url("/changeset/create", self.target_server);
			let auth = self.auth_token.get(self.target_server as usize).unwrap().as_ref().ok_or("missing auth token")?;
			let data = OsmCreateChangeset { changeset: RawChangeset { tags } };
			let body = quick_xml::se::to_string(&data)?;
			let resp = self.http_client.put(url)
				.header("authorization", format!("{} {}", auth.token_type, auth.access_token))
				.send(body)?;
			resp.into_body()
				.read_to_string()?
				.parse().map_err(Box::from)
		}

		// todo: error type
		pub fn close_changeset(&self, id: NonZeroU32) -> OsmResult<NonZeroU32> {
			let url = api_url(format!("/changeset/{id}/close"), self.target_server);
			let auth = self.auth_token.get(self.target_server as usize).unwrap().as_ref().ok_or("missing auth token")?;
			self.http_client.put(url)
				.header("authorization", format!("{} {}", auth.token_type, auth.access_token))
				.send_empty()
				.map(|_| id)
				.map_err(Box::from)
		}

		// todo: error type
		pub fn fetch_token(&self, auth_code: impl AsRef<str>) -> OsmResult<OsmToken> {
			let url = format!("https://{}", self.target_server.base_token_url());
			let body = format!("grant_type=authorization_code&code={}&redirect_uri={REDIRECT_URI}&client_id={}", auth_code.as_ref(), self.target_server.client_id());
			let resp = self.http_client.post(url).header("content-type", "application/x-www-form-urlencoded").send(body)?;
			resp.into_body().read_json::<OsmToken>()
				.map_err(Box::from)
		}
	}
}

#[cfg(target_family = "wasm")]
mod web {
	use super::*;
	use crate::app::osm::TargetServer;
	use ehttp::Request;
	use osm_parser::types::raw;
	use osm_parser::OsmData;
	use std::num::NonZeroU32;

	pub struct OsmClient {
		pub target_server: TargetServer,
		pub auth_token: [Option<OsmToken>; TargetServer::SIZE],
	}

	#[allow(clippy::future_not_send)]
	impl OsmClient {
		pub fn new(target_server: TargetServer) -> Self {
			Self {
				target_server,
				auth_token: Default::default(),
			}
		}

		pub async fn get_map(&self, bbox: &Bbox) -> OsmResult<OsmData> {
			let url = api_url_override(format!("/map.json?bbox={},{},{},{}", bbox.left, bbox.bottom, bbox.right, bbox.top), TargetServer::OpenStreetMap);
			let resp = ehttp::fetch_async(Request::get(url)).await
				.map(|x| if x.ok { Ok(x) } else { Err(format!("request failed with status code {}", x.status)) })??;

			let raw = resp.json::<raw::RawOsmData>()?;
			raw.try_into()
		}

		pub async fn create_changeset(&self, tags: Vec<Tag>) -> OsmResult<NonZeroU32> {
			let url = api_url("/changeset/create", self.target_server);
			let auth = self.auth_token.get(self.target_server as usize).unwrap().as_ref().ok_or("missing auth token")?;
			let data = OsmCreateChangeset { changeset: RawChangeset { tags } };
			let body = quick_xml::se::to_string(&data)?;
			let resp = ehttp::fetch_async(Request {
				method: "PUT".into(),
				url,
				body: body.into_bytes(),
				headers: ehttp::Headers::new(&[("authorization", &format!("{} {}", auth.token_type, auth.access_token))]),
				mode: ehttp::Mode::default(),
			}).await
				.map(|x| if x.ok { Ok(x) } else { Err(format!("request failed with status code {}", x.status)) })??;

			String::from_utf8(resp.bytes)?
				.parse().map_err(Box::from)
		}

		pub async fn close_changeset(&self, id: NonZeroU32) -> OsmResult<NonZeroU32> {
			let url = api_url(format!("/changeset/{id}/close"), self.target_server);
			let auth = self.auth_token.get(self.target_server as usize).unwrap().as_ref().ok_or("missing auth token")?;
			ehttp::fetch_async(Request {
				method: "PUT".into(),
				url,
				body: vec![],
				headers: ehttp::Headers::new(&[("authorization", &format!("{} {}", auth.token_type, auth.access_token))]),
				mode: ehttp::Mode::default(),
			}).await
				.map(|x| if x.ok { Ok(id) } else { Err(format!("request failed with status code {}", x.status)) })?
				.map_err(Box::from)
		}

		pub async fn fetch_token(&self, auth_code: impl AsRef<str>) -> OsmResult<OsmToken> {
			let url = format!("https://{}", self.target_server.base_token_url());
			let body = format!("grant_type=authorization_code&code={}&redirect_uri={REDIRECT_URI}&client_id={}", auth_code.as_ref(), self.target_server.client_id());
			let resp = ehttp::fetch_async(Request {
				method: "POST".into(),
				url,
				body: body.into_bytes(),
				headers: ehttp::Headers::new(&[("content-type", "application/x-www-form-urlencoded")]),
				mode: ehttp::Mode::default(),
			}).await
				.map(|x| if x.ok { Ok(x) } else { Err(format!("request failed with status code {}", x.status)) })??;

			resp.json::<OsmToken>()
				.map_err(Box::from)
		}
	}
}
