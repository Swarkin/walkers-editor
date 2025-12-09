use super::osm::{Bbox, OrderedTags, OsmClient, OsmResult, OsmToken, TargetServer};
use super::osmchange::{ChangesetId, OsmChange, Tag};
use osm_parser::OsmData;
use std::io;

#[cfg(not(target_family = "wasm"))]
use super::settings;
use super::settings::Config;
use crate::app::editor::theme::Theme;
#[cfg(not(target_family = "wasm"))]
use crossbeam_channel::{Receiver, Sender};
#[cfg(target_family = "wasm")]
use futures::{
	channel::mpsc::{UnboundedReceiver as Receiver, UnboundedSender as Sender},
	stream::StreamExt,
};
#[cfg(not(target_family = "wasm"))]
use std::fs;
#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;
#[cfg(not(target_family = "wasm"))]
use std::thread::JoinHandle;

pub enum Request { // box is used to keep enum size small
	#[cfg(target_family = "wasm")]
	LoadSettings,
	#[cfg(not(target_family = "wasm"))] LoadSettings(Option<PathBuf>, Option<String>),
	#[cfg(not(target_family = "wasm"))] SaveSettings(Option<PathBuf>, Option<String>, Option<Box<Config>>, Option<Box<Theme>>),

	#[cfg(not(target_family = "wasm"))] ExportConfig(Box<Config>),
	#[cfg(not(target_family = "wasm"))] ExportTheme(Box<Theme>),
	#[cfg(not(target_family = "wasm"))] ImportConfig,
	#[cfg(not(target_family = "wasm"))] ImportTheme,

	GetMap(Box<Bbox>),
	SetTargetServer(TargetServer),
	FetchToken(String),
	UploadChanges { tags: Box<OrderedTags>, osmchange: Box<OsmChange> },
}

pub enum Response {
	LoadedSettings(Option<io::Result<Config>>, Option<io::Result<Theme>>),
	#[cfg(not(target_family = "wasm"))] SavedSettings(Option<io::Error>, Option<io::Error>),

	#[cfg(not(target_family = "wasm"))] ExportedConfig(Option<io::Error>),
	#[cfg(not(target_family = "wasm"))] ExportedTheme(Option<io::Error>),
	#[cfg(not(target_family = "wasm"))] ImportedConfig(io::Result<Config>),
	#[cfg(not(target_family = "wasm"))] ImportedTheme(io::Result<Theme>),
	#[cfg(not(target_family = "wasm"))] SettingsIoCancelled,

	Map(OsmResult<OsmData>),
	Token(OsmResult<OsmToken>, TargetServer),
	UploadChangesProgress(UploadChangesProgress),
}

pub enum UploadChangesProgress {
	ChangesetCreated(OsmResult<ChangesetId>),
	DiffUploaded(OsmResult<String>),
	ChangesetClosed(OsmResult<()>),
}

pub struct Worker {
	pub osm_client: OsmClient,
	pub sender: Sender<Response>,
}

impl Worker {
	pub fn spawn(mut self, req_send: Sender<Request>, req_recv: Receiver<Request>, resp_recv: Receiver<Response>) -> WorkerHandle {
		#[cfg(target_family = "wasm")]
		wasm_bindgen_futures::spawn_local(async move {
			self.run(req_recv).await;
		});

		WorkerHandle {
			#[cfg(not(target_family = "wasm"))]
			thread: std::thread::spawn(move || self.run(req_recv)),
			sender: req_send,
			receiver: resp_recv,
		}
	}

	pub fn send_message(&self, msg: Response) {
		#[cfg(not(target_family = "wasm"))]
		self.sender.send(msg).unwrap();
		#[cfg(target_family = "wasm")]
		self.sender.unbounded_send(msg).unwrap();
	}
}

pub struct WorkerHandle {
	#[cfg(not(target_family = "wasm"))]
	#[allow(dead_code)]
	pub thread: JoinHandle<()>,
	pub sender: Sender<Request>,
	pub receiver: Receiver<Response>,
}

impl WorkerHandle {
	pub fn send_message(&self, msg: Request) {
		#[cfg(not(target_family = "wasm"))]
		self.sender.send(msg).unwrap();
		#[cfg(target_family = "wasm")]
		self.sender.unbounded_send(msg).unwrap();
	}

	/// Returns all received messages without blocking.
	#[cfg(not(target_family = "wasm"))]
	pub fn recv_messages(&self) -> Vec<Response> {
		self.receiver.try_iter().collect::<Vec<_>>()
	}

	#[cfg(target_family = "wasm")]
	pub fn recv_messages(&mut self) -> Vec<Response> {
		let mut messages = vec![];
		while let Ok(msg) = self.receiver.try_next() {
			if let Some(msg) = msg {
				messages.push(msg);
			} else { panic!("receiver was closed unexpectedly"); }
		}
		messages
	}
}

impl Worker {
	#[cfg(not(target_family = "wasm"))]
	#[expect(clippy::needless_pass_by_value)]
	fn load_settings(dir: Option<PathBuf>, name: Option<String>, load_config: bool, load_theme: bool) -> (Option<io::Result<Config>>, Option<io::Result<Theme>>) {
		debug_assert!(load_config || load_theme);

		let mut config = None;
		let mut theme = None;

		if let Some(dir) = dir.or_else(|| dirs::config_dir().map(|x| x.join(env!("CARGO_PKG_NAME")))) {
			if name.is_none() && let Err(e) = fs::create_dir_all(&dir) {
				if load_config { config = Some(Err(io::Error::other(format!("{e}")))) }
				if load_theme { theme = Some(Err(e)) }
			} else {
				if load_config {
					config = Some(settings::load_config(dir.join(name.as_ref().map_or(settings::CONFIG_FILE_NAME, |v| v))));
				}
				if load_theme {
					theme = Some(settings::load_theme(dir.join(name.as_ref().map_or(settings::THEME_FILE_NAME, |v| v))));
				}
			}
		}

		(config, theme)
	}

	#[cfg(not(target_family = "wasm"))]
	#[expect(clippy::needless_pass_by_value)]
	fn save_settings(dir: Option<PathBuf>, name: Option<String>, config: Option<Box<Config>>, theme: Option<Box<Theme>>) -> (Option<io::Error>, Option<io::Error>) {
		debug_assert!(config.is_some() || theme.is_some());

		let mut config_result = Ok(());
		let mut theme_result = Ok(());

		if let Some(dir) = dir.or_else(|| dirs::config_dir().map(|x| x.join(env!("CARGO_PKG_NAME")))) {
			if let Some(config) = config {
				config_result = settings::save_config(dir.join(name.as_ref().map_or(settings::CONFIG_FILE_NAME, |v| v)), &config);
			}
			if let Some(theme) = theme {
				theme_result = settings::save_theme(dir.join(name.as_ref().map_or(settings::THEME_FILE_NAME, |v| v)), &theme);
			}
		}

		(config_result.err(), theme_result.err())
	}

	#[cfg(not(target_family = "wasm"))]
	fn handle_message(&mut self, request: Request) {
		match request {
			Request::LoadSettings(dir, name) => {
				let (config, theme) = Self::load_settings(dir, name, true, true);
				self.send_message(Response::LoadedSettings(config, theme));
			}
			Request::SaveSettings(path, name, config, theme) => {
				let (config, theme) = Self::save_settings(path, name, config, theme);
				self.send_message(Response::SavedSettings(config, theme));
			}

			Request::ExportConfig(config) => {
				if let Some(path) = rfd::FileDialog::new().set_file_name("config.toml").add_filter("toml", &["toml"]).save_file() {
					let dir = path.parent().unwrap().to_owned();
					let file = path.file_name().unwrap().to_str().unwrap().into();
					let (config, theme) = Self::save_settings(Some(dir), Some(file), Some(config), None);
					debug_assert!(theme.is_none());
					self.send_message(Response::ExportedConfig(config));
				} else {
					self.send_message(Response::SettingsIoCancelled);
				}
			}
			Request::ExportTheme(theme) => {
				if let Some(path) = rfd::FileDialog::new().set_file_name("theme.toml").add_filter("toml", &["toml"]).save_file() {
					let dir = path.parent().unwrap().to_owned();
					let file = path.file_name().unwrap().to_str().unwrap().into();
					let (config, theme) = Self::save_settings(Some(dir), Some(file), None, Some(theme));
					debug_assert!(config.is_none());
					self.send_message(Response::ExportedTheme(theme));
				} else {
					self.send_message(Response::SettingsIoCancelled);
				}
			}
			Request::ImportConfig => {
				if let Some(path) = rfd::FileDialog::new().add_filter("toml", &["toml"]).pick_file() {
					let dir = path.parent().unwrap().to_owned();
					let file = path.file_name().unwrap().to_str().unwrap().into();
					let (config, theme) = Self::load_settings(Some(dir), Some(file), true, false);
					debug_assert!(theme.is_none());
					self.send_message(Response::ImportedConfig(config.unwrap()));
				} else {
					self.send_message(Response::SettingsIoCancelled);
				}
			}
			Request::ImportTheme => {
				if let Some(path) = rfd::FileDialog::new().add_filter("toml", &["toml"]).pick_file() {
					let dir = path.parent().unwrap().to_owned();
					let file = path.file_name().unwrap().to_str().unwrap().into();
					let (config, theme) = Self::load_settings(Some(dir), Some(file), false, true);
					debug_assert!(config.is_none());
					self.send_message(Response::ImportedTheme(theme.unwrap()));
				} else {
					self.send_message(Response::SettingsIoCancelled);
				}
			}

			Request::GetMap(bbox) => {
				let result = self.osm_client.get_map(&bbox);
				self.send_message(Response::Map(result));
			}
			Request::SetTargetServer(target) => {
				self.osm_client.target_server = target;
			}
			Request::FetchToken(auth_code) => {
				let result = self.osm_client.fetch_token(auth_code);
				let target_server = self.osm_client.target_server;

				if let Ok(token) = result.as_ref() {
					self.osm_client.auth_token[target_server as usize] = Some(token.to_owned());
				}

				self.send_message(Response::Token(result, target_server));
			}
			Request::UploadChanges { tags, mut osmchange } => {
				let tags = tags.into_iter().map(|(k, v)| Tag { k, v }).collect();

				let changeset_result = self.osm_client.create_changeset(tags);
				let changeset_id = changeset_result.as_ref().ok().copied();

				self.send_message(Response::UploadChangesProgress(
					UploadChangesProgress::ChangesetCreated(changeset_result),
				));

				if let Some(changeset_id) = changeset_id {
					osmchange.prepare_upload(changeset_id);
					let diff_result = self.osm_client.diff_upload(changeset_id, osmchange.to_string_pretty().unwrap());
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::DiffUploaded(diff_result),
					));

					let close_result = self.osm_client.close_changeset(changeset_id);
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::ChangesetClosed(close_result),
					));
				}
			}
		}
	}

	#[cfg(target_family = "wasm")]
	#[expect(clippy::future_not_send)]
	async fn handle_message(&mut self, request: Request) {
		match request {
			Request::LoadSettings => {
				self.send_message(Response::LoadedSettings(Some(Ok(Config::default())), Some(Ok(Theme::default()))));
			}
			Request::GetMap(bbox) => {
				let result = self.osm_client.get_map(&bbox).await;
				self.send_message(Response::Map(result));
			}
			Request::SetTargetServer(target) => {
				self.osm_client.target_server = target;
			}
			Request::FetchToken(auth_code) => {
				let result = self.osm_client.fetch_token(auth_code).await;
				let target_server = self.osm_client.target_server;

				if let Ok(token) = result.as_ref() {
					self.osm_client.auth_token[target_server as usize] = Some(token.to_owned());
				}

				self.send_message(Response::Token(result, target_server));
			}
			Request::UploadChanges { tags, mut osmchange } => {
				let tags = tags.into_iter().map(|(k, v)| Tag { k, v }).collect();

				let changeset_result = self.osm_client.create_changeset(tags).await;
				let changeset_id = changeset_result.as_ref().ok().copied();

				self.send_message(Response::UploadChangesProgress(
					UploadChangesProgress::ChangesetCreated(changeset_result),
				));

				if let Some(changeset_id) = changeset_id {
					osmchange.prepare_upload(changeset_id);
					let diff_result = self.osm_client.diff_upload(changeset_id, osmchange.to_string_pretty().unwrap()).await;
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::DiffUploaded(diff_result),
					));

					let close_result = self.osm_client.close_changeset(changeset_id).await;
					self.send_message(Response::UploadChangesProgress(
						UploadChangesProgress::ChangesetClosed(close_result),
					));
				}
			}
		}
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn run(&mut self, receiver: Receiver<Request>) {
		for msg in receiver {
			self.handle_message(msg);
		}
	}

	#[cfg(target_family = "wasm")]
	#[expect(clippy::future_not_send)]
	pub async fn run(&mut self, mut receiver: Receiver<Request>) {
		while let Some(msg) = receiver.next().await {
			self.handle_message(msg).await;
		}
	}
}
