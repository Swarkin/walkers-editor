use super::osmchange::Tag;

#[cfg(not(target_family = "wasm"))]
pub use native::OsmClient;

#[cfg(target_family = "wasm")]
pub use web::OsmClient;

const REDIRECT_URI: &str = "urn:ietf:wg:oauth:2.0:oob";
const SCOPES: &str = "write_api";
const AUTHORIZATION_HEADER: &str = "authorization";
const CONTENT_TYPE_HEADER: &str = "content-type";
const APPLICATION_XML: &str = "application/xml";
const TIMEOUT_GLOBAL: u64 = 120;

type AnyError = Box<dyn std::error::Error + Sync + Send>;
pub type OsmResult<T> = Result<T, AnyError>;

pub type OrderedTags = indexmap::IndexMap<String, String>;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TargetServer {
	#[default] OpenStreetMap,
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

	pub const fn base_user_url(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "www.openstreetmap.org/user",
			Self::OpenStreetMapDev => "master.apis.dev.openstreetmap.org/user",
		}
	}

	pub const fn base_changeset_url(self) -> &'static str {
		match self {
			Self::OpenStreetMap => "www.openstreetmap.org/changeset",
			Self::OpenStreetMapDev => "master.apis.dev.openstreetmap.org/changeset",
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
	tag: Vec<Tag>
}

fn api_url(path: impl AsRef<str>, target_server: TargetServer) -> String {
	debug_assert!(path.as_ref().starts_with('/'));
	format!("https://{}/api/0.6{}", target_server.base_url(), path.as_ref())
}

pub fn client_auth_url(server: TargetServer) -> String {
	format!("https://{}?response_type=code&client_id={}&redirect_uri={REDIRECT_URI}&scope={SCOPES}", server.base_auth_url(), server.client_id())
}

#[cfg(not(target_family = "wasm"))]
mod native {
	use super::*;
	use crate::app::osm::TargetServer;
	use crate::app::osmchange::ChangesetId;
	use osm_parser::types::raw;
	use osm_parser::OsmData;
	use std::time::Duration;
	use ureq::http::{HeaderName, HeaderValue, Response};
	use ureq::Body;

	const DOWNLOAD_LIMIT: u64 = 100_000_000;
	const CONTENT_LENGTH_HEADER: &str = "content-length";

	pub struct OsmClient {
		pub http_client: ureq::Agent,
		pub target_server: TargetServer,
		pub auth_token: [Option<OsmToken>; TargetServer::SIZE],
	}

	impl OsmClient {
		pub fn new(target_server: TargetServer) -> Self {
			Self {
				http_client: ureq::Agent::config_builder()
					.user_agent(crate::USER_AGENT)
					.https_only(true)
					.max_redirects(0)
					.timeout_connect(Some(Duration::from_secs(30)))
					.timeout_global(Some(Duration::from_secs(TIMEOUT_GLOBAL)))
					.build().into(),
				target_server,
				auth_token: Default::default(),
			}
		}

		pub fn post_with_auth(&self, url: String, data: Vec<u8>) -> OsmResult<Response<Body>> {
			self.http_client.post(url)
				.header(AUTHORIZATION_HEADER, self.get_auth()?)
				.header(CONTENT_LENGTH_HEADER, data.len())
				.send(data)
				.map_err(Box::from)
		}

		pub fn put_with_auth(&self, url: String, data: Vec<u8>, headers: Vec<(HeaderName, HeaderValue)>) -> OsmResult<Response<Body>> {
			let mut req = self.http_client.put(url);

			for (name, value) in headers {
				req = req.header(name, value);
			}

			req.header(AUTHORIZATION_HEADER, self.get_auth()?)
				.header(CONTENT_LENGTH_HEADER, data.len())
				.send(data)
				.map_err(Box::from)
		}

		pub fn put_with_auth_empty(&self, url: String) -> OsmResult<Response<Body>> {
			self.http_client.put(url)
				.header(AUTHORIZATION_HEADER, self.get_auth()?)
				.send_empty()
				.map_err(Box::from)
		}

		pub fn get_auth(&self) -> OsmResult<String> {
			let auth = self.auth_token.get(self.target_server as usize).unwrap().as_ref().ok_or("missing auth token")?;
			Ok(format!("{} {}", auth.token_type, auth.access_token))
		}

		// todo: error type
		pub fn get_map(&self, bbox: &Bbox) -> OsmResult<OsmData> {
			let url = api_url(format!("/map.json?bbox={},{},{},{}", bbox.left, bbox.bottom, bbox.right, bbox.top), self.target_server);

			let mut req = self.http_client.get(url);
			if let Ok(auth) = self.get_auth() {
				req = req.header(AUTHORIZATION_HEADER, auth);
			}

			req.call()?
				.into_body().into_with_config().limit(DOWNLOAD_LIMIT)
				.read_json::<raw::RawOsmData>()?
				.try_into()
		}

		// todo: error type
		pub fn create_changeset(&self, tags: Vec<Tag>) -> OsmResult<ChangesetId> {
			let url = api_url("/changeset/create", self.target_server);
			let data = OsmCreateChangeset { changeset: RawChangeset { tag: tags } };
			let body = quick_xml::se::to_string(&data)?;

			self.put_with_auth(url, body.into_bytes(), vec![(HeaderName::from_static(CONTENT_TYPE_HEADER), HeaderValue::from_static(APPLICATION_XML))])?
				.into_body()
				.read_to_string()?
				.parse::<ChangesetId>()
				.map_err(Box::from)
		}

		// todo: error type
		/// <https://wiki.openstreetmap.org/wiki/API_v0.6#Diff_upload:_POST_/api/0.6/changeset/#id/upload>
		pub fn diff_upload(&self, id: ChangesetId, osmchange_str: String) -> OsmResult<String> {
			let url = api_url(format!("/changeset/{id}/upload"), self.target_server);
			self.post_with_auth(url, osmchange_str.into_bytes())?
				.into_body()
				.read_to_string()
				.map_err(Box::from)
		}

		// todo: error type
		pub fn close_changeset(&self, id: ChangesetId) -> OsmResult<()> {
			let url = api_url(format!("/changeset/{id}/close"), self.target_server);
			self.put_with_auth_empty(url)?;
			Ok(())
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
	use crate::app::osmchange::ChangesetId;
	use crate::USER_AGENT;
	use ehttp::Request;
	use osm_parser::types::raw;
	use osm_parser::OsmData;
	use std::time::Duration;

	const GET: &str = "GET";
	const PUT: &str = "PUT";
	const POST: &str = "POST";

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

		pub async fn send_request(&self, method: String, url: String, body: Vec<u8>, headers: &[(&str, &str)]) -> ehttp::Result<ehttp::Response> {
			let auth = self.auth_token.get(self.target_server as usize).unwrap().as_ref();
			let mut headers = ehttp::Headers::new(headers);
			headers.insert("x-requested-with", USER_AGENT);
			if let Some(auth) = auth {
				headers.insert(AUTHORIZATION_HEADER, format!("{} {}", auth.token_type, auth.access_token));
			}

			ehttp::fetch_async(Request { method, url, body, headers, mode: ehttp::Mode::Cors, timeout: Some(Duration::from_secs(TIMEOUT_GLOBAL)) })
				.await
				.map(|x| if x.ok { Ok(x) } else { Err(format!("Request failed with status code {}", x.status)) })?
		}

		pub async fn get_map(&self, bbox: &Bbox) -> OsmResult<OsmData> {
			let url = api_url(format!("/map.json?bbox={},{},{},{}", bbox.left, bbox.bottom, bbox.right, bbox.top), self.target_server);

			let resp = self.send_request(GET.into(), url, vec![], &[]).await?;
			let raw = resp.json::<raw::RawOsmData>()?;
			raw.try_into()
		}

		pub async fn create_changeset(&self, tags: Vec<Tag>) -> OsmResult<ChangesetId> {
			let url = api_url("/changeset/create", self.target_server);
			let data = OsmCreateChangeset { changeset: RawChangeset { tag: tags } };
			let body = quick_xml::se::to_string(&data)?;

			let resp = self.send_request(PUT.into(), url, body.into_bytes(), &[(CONTENT_TYPE_HEADER, APPLICATION_XML)]).await?;
			String::from_utf8(resp.bytes)?
				.parse().map_err(Box::from)
		}

		pub async fn diff_upload(&self, id: ChangesetId, osmchange_str: String) -> OsmResult<String> {
			let url = api_url(format!("/changeset/{id}/upload"), self.target_server);

			let resp = self.send_request(POST.into(), url, osmchange_str.into_bytes(), &[]).await?;
			String::from_utf8(resp.bytes).map_err(Box::from)
		}

		pub async fn close_changeset(&self, id: ChangesetId) -> OsmResult<()> {
			let url = api_url(format!("/changeset/{id}/close"), self.target_server);

			self.send_request(PUT.into(), url, vec![], &[]).await
				.map(|_| ())
				.map_err(Box::from)
		}

		pub async fn fetch_token(&self, auth_code: impl AsRef<str>) -> OsmResult<OsmToken> {
			let url = format!("https://{}", self.target_server.base_token_url());
			let body = format!("grant_type=authorization_code&code={}&redirect_uri={REDIRECT_URI}&client_id={}", auth_code.as_ref(), self.target_server.client_id());

			let resp = self.send_request(POST.into(), url, body.into_bytes(), &[("content-type", "application/x-www-form-urlencoded")]).await?;
			resp.json::<OsmToken>()
				.map_err(Box::from)
		}
	}
}
