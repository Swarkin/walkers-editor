use super::translations;
use crate::app::editor::{theme, Editor};
use crate::app::osm::OrderedTags;
use crate::app::osmchange::ChangesetId;
use crate::app::providers::TilesKind;
use crate::app::{
	editor::visual::{FillMode, Visualization},
	osm::{OsmResult, OsmToken, TargetServer},
	osmchange::OsmChange,
	providers::Provider,
};
use crate::HashMap;
use std::fmt::{Display, Formatter};
use walkers::MapMemory;

#[cfg(not(target_family = "wasm"))]
pub type SettingsIOResult = (Option<std::io::Error>, Option<std::io::Error>);

#[derive(Default)]
pub struct AppState {
	pub view: View,
	pub language: translations::Language,
	pub theme: theme::Theme,
	pub target_server_ui: TargetServer,
	pub open_modals: u8,
	pub top_bar_disabled: bool,
	#[cfg(not(target_family = "wasm"))]
	pub settings_load_result: Option<SettingsIOResult>,
	#[cfg(not(target_family = "wasm"))]
	pub settings_save_result: Option<SettingsIOResult>,
	pub debug_redraw_continuously: bool,
}

#[derive(Default, PartialEq, Eq)]
pub enum View {
	#[default]
	Edit,
	Upload,
	Auth,
}

pub struct EditorState {
	pub editor: Editor,
	pub map_memory: MapMemory,
	pub tile_providers: HashMap<Provider, TilesKind>,
}

pub struct MapState {
	pub selected_provider: Option<Provider>,
	pub selected_visualization: Visualization,
	pub selected_fill_mode: FillMode,
	pub selection_mode: u8,
	pub download: MapDownloadState,
	pub zoom_with_ctrl: bool,
}

impl Default for MapState {
	fn default() -> Self {
		Self {
			selected_provider: Some(Provider::default()),
			selected_visualization: Visualization::default(),
			selected_fill_mode: FillMode::default(),
			selection_mode: SelectionFlag::Nodes as u8 + SelectionFlag::Ways as u8,
			download: MapDownloadState::Idle(None),
			zoom_with_ctrl: false,
		}
	}
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum SelectionFlag {
	Nodes = 1 << 0,
	Ways = 1 << 1,
	Areas = 1 << 2, // todo: implement
}

impl Display for SelectionFlag {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Self::Nodes => "Nodes",
			Self::Ways => "Ways",
			Self::Areas => "Areas",
		})
	}
}

impl SelectionFlag {
	pub const ITER: [Self; 3] = [Self::Nodes, Self::Ways, Self::Areas];
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ModalFlag {
	Licenses = 1 << 0,
	DataViewer = 1 << 1,
	#[cfg(target_family = "wasm")]
	FirefoxNotice = 1 << 7,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum CacheFlag {
	NodeProjection = 1 << 0,
	NodeOrphan = 1 << 1,
	NodeDedup = 1 << 2,
	NodeUsage = 1 << 3,
	WayArea = 1 << 4,
	WayMeshAndAreaSize = 1 << 5,
	AreaSizeOrdered = 1 << 6,
}

impl CacheFlag {
	pub const ALL: u8 = u8::MAX;
}

#[cfg(feature = "debug")]
impl CacheFlag {
	pub const SIZE: usize = 7;
	pub const ITER: [Self; Self::SIZE] = [
		Self::NodeProjection,
		Self::NodeOrphan,
		Self::NodeDedup,
		Self::NodeUsage,
		Self::WayArea,
		Self::WayMeshAndAreaSize,
		Self::AreaSizeOrdered,
	];
}

pub enum MapDownloadState {
	Idle(Option<(OsmResult<()>, f64)>),
	Downloading,
}

impl Default for MapDownloadState {
	fn default() -> Self {
		Self::Idle(None)
	}
}

/// State related to the upload tab
#[derive(Default)]
pub struct UploaderState {
	pub osmchange: OsmChange,
	pub osmchange_text: String,
	pub changeset_upload: ChangesetUpload,
}

impl UploaderState {
	pub fn clear_osmchange(&mut self) {
		self.osmchange.clear();
		self.osmchange_text.clear();
	}
}

#[derive(Default)]
pub struct ChangesetUpload {
	pub tags: OrderedTags,
	pub target_server: TargetServer,
	pub state: ChangesetUploadState,
	pub creation: Option<OsmResult<ChangesetId>>,
	pub diff_upload: Option<OsmResult<String>>,
	pub close: Option<OsmResult<()>>,
}

impl ChangesetUpload {
	pub fn clear(&mut self) {
		self.tags.clear();
		self.state = ChangesetUploadState::Idle;
		self.creation = None;
		self.diff_upload = None;
		self.close = None;
	}

	pub fn is_empty(&self) -> bool {
		matches!(
			(&self.creation, &self.diff_upload, &self.close),
			(None, None, None)
		)
	}

	pub fn all_successful(&self) -> bool {
		matches!(
			(&self.creation, &self.diff_upload, &self.close),
			(Some(Ok(_)), Some(Ok(_)), Some(Ok(())))
		)
	}

	pub fn any_unsuccessful(&self) -> bool {
		matches!(
			(&self.creation, &self.diff_upload, &self.close),
			(Some(Err(_)), _, _) |
			(_, Some(Err(_)), _) |
			(_, _, Some(Err(_)))
		)
	}
}

#[derive(Default)]
pub enum ChangesetUploadState {
	#[default] Idle,
	Creating,
	Uploading,
	Closing,
}

impl Display for ChangesetUploadState {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Self::Idle => "Idle",
			Self::Creating => "Creating changeset",
			Self::Uploading => "Uploading changes",
			Self::Closing => "Closing changeset",
		})
	}
}

/// State related to the auth tab
#[derive(Default)]
pub struct AuthenticatorState {
	pub token: HashMap<TargetServer, OsmResult<OsmToken>>,
	pub authorization_code: String,
	pub request_pending: bool,
}

#[cfg(not(target_family = "wasm"))]
#[derive(Default)]
pub enum BootState {
	#[default] Starting,
	Idle,
	Saving,
	Finished,
}

pub mod settings {
	#[cfg(not(target_family = "wasm"))]
	use super::theme::Theme;
	use super::translations;
	use crate::app::windows::{Window, WindowBitflag};
	use serde::{Deserialize, Serialize};
	#[cfg(not(target_family = "wasm"))]
	use std::path::Path;

	#[cfg(not(target_family = "wasm"))]
	const CONFIG_FILE_NAME: &str = "config.toml";
	#[cfg(not(target_family = "wasm"))]
	const THEME_FILE_NAME: &str = "theme.toml";

	#[derive(Clone, Serialize, Deserialize)]
	pub struct Config {
		pub language: translations::Language,
		pub window_flags: WindowBitflag,
		pub zoom_with_ctrl: bool,
		pub debug_redraw_continuously: bool,
	}

	impl Default for Config {
		fn default() -> Self {
			Self {
				language: translations::Language::EN,
				window_flags: Window::Settings as u8,
				zoom_with_ctrl: false,
				debug_redraw_continuously: false,
			}
		}
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn load_config(path: &Path) -> std::io::Result<Config> {
		let path = path.join(CONFIG_FILE_NAME);
		let content = match std::fs::read_to_string(path) {
			Ok(c) => c,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
			Err(e) => return Err(e),
		};

		toml::from_str(&content)
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn load_theme(path: &Path) -> std::io::Result<Theme> {
		let path = path.join(THEME_FILE_NAME);
		let content = match std::fs::read_to_string(path) {
			Ok(c) => c,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Theme::default()),
			Err(e) => return Err(e),
		};

		toml::from_str(&content)
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn save_config(path: &Path, config: &Config) -> std::io::Result<()> {
		let path = path.join(CONFIG_FILE_NAME);
		let content = toml::to_string(config)
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
		std::fs::write(path, content)
	}

	#[cfg(not(target_family = "wasm"))]
	pub fn save_theme(path: &Path, theme: &Theme) -> std::io::Result<()> {
		let path = path.join(THEME_FILE_NAME);
		let content = toml::to_string(theme)
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
		std::fs::write(path, content)
	}
}
