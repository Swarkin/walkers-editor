//todo: clean up the MyApp struct and move editor config related things into this file

#[derive(Default)]
pub struct UploaderConfig {
	target_server: TargetServer,
	//uploader_state: UploaderState,
}

#[derive(Default, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TargetServer {
	OpenStreetMap,
	#[default]
	OpenStreetMapDev,
}

#[derive(Default, Copy, Clone, PartialEq)]
pub enum UploaderState {
	#[default]
	Viewing,
	//Authenticating,
	//Uploading,
}

impl TargetServer {
	pub const ITER: [TargetServer; 2] = [TargetServer::OpenStreetMap, TargetServer::OpenStreetMapDev];

	pub fn description(&self) -> &'static str {
		match self {
			TargetServer::OpenStreetMap => "OpenStreetMap main instance",
			TargetServer::OpenStreetMapDev => "OpenStreetMap test instance",
		}
	}

	pub fn base_url(&self) -> &'static str {
		match self {
			TargetServer::OpenStreetMap => "www.openstreetmap.org",
			TargetServer::OpenStreetMapDev => "master.apis.dev.openstreetmap.org",
		}
	}

	pub fn base_token_url(&self) -> &'static str {
		match self {
			TargetServer::OpenStreetMap => "www.openstreetmap.org/oauth2/token",
			TargetServer::OpenStreetMapDev => "master.apis.dev.openstreetmap.org/oauth2/token",
		}
	}

	pub fn base_auth_url(&self) -> &'static str {
		match self {
			TargetServer::OpenStreetMap => "www.openstreetmap.org/oauth2/authorize",
			TargetServer::OpenStreetMapDev => "master.apis.dev.openstreetmap.org/oauth2/authorize",
		}
	}
}
