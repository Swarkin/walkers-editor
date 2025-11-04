use crate::app::editor::Editor;
use crate::app::providers::TilesKind;
use crate::app::{
	editor::{FillMode, Visualization},
	osm::{OsmResult, OsmToken, TargetServer},
	osmchange::OsmChange,
	providers::Provider,
};
use std::{
	collections::HashMap,
	fmt::{Display, Formatter},
	num::NonZeroU32,
};
use walkers::MapMemory;

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
	pub scale_factor: f32,
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
			scale_factor: 1.,
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
	pub changeset_creation: Option<OsmResult<NonZeroU32>>,
	pub diff_upload: Option<OsmResult<String>>,
	pub changeset_closure: Option<OsmResult<()>>,
	pub request_pending: bool,
}

/// State related to the auth tab
#[derive(Default)]
pub struct AuthenticatorState {
	pub token: HashMap<TargetServer, OsmResult<OsmToken>>,
	pub authorization_code: String,
	pub request_pending: bool,
}
