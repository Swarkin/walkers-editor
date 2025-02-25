//todo: clean up the MyApp struct and move editor config related things into this file

#[derive(Default)]
pub struct UploaderConfig {
	target_server: TargetServer,
	//uploader_state: UploaderState,
}

#[derive(Default, Copy, Clone, PartialEq)]
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

	pub fn url(&self) -> &'static str {
		match self {
			TargetServer::OpenStreetMap => "www.openstreetmap.org",
			TargetServer::OpenStreetMapDev => "master.apis.dev.openstreetmap.org",
		}
	}
}
