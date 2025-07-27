use super::{cache::EditorOsmData, visual::Visualization, EditorPluginState, FillMode};
use crate::app::osm::TargetServer;
use crate::app::{
	osm::{OsmResult, OsmToken},
	osmchange::OsmChange,
	providers::{Provider, ProviderMap, TilesKind},
	windows::WindowBitflag,
};
use eframe::egui::Vec2;
use std::{
	collections::HashMap,
	fmt::{Display, Formatter},
	num::NonZeroU32
};
use walkers::MapMemory;

pub struct EditorState {
	pub tile_providers: HashMap<Provider, TilesKind>,
	pub map_memory: MapMemory,
	pub map_state: MapState,
	pub plugin_state: EditorPluginState,
	pub osm_data: EditorOsmData,
	pub window_flags: WindowBitflag,
	pub prev_size: Vec2,
}

impl EditorState {
	pub fn new(providers: ProviderMap) -> Self {
		Self {
			tile_providers: providers,
			map_memory: MapMemory::default(),
			map_state: MapState {
				selected_provider: Some(Provider::default()),
				selected_visualization: Visualization::default(),
				selected_fill_mode: FillMode::default(),
				selection_mode: SelectionFlag::Nodes as u8 + SelectionFlag::Ways as u8,
				download: MapDownloadState::Idle(None),
				scale_factor: 1.0,
				zoom_with_ctrl: false,
			},
			osm_data: EditorOsmData::default(),
			plugin_state: EditorPluginState::default(),
			window_flags: WindowBitflag::default(),
			prev_size: Vec2::ZERO,
		}
	}
}

pub struct MapState {
	pub selected_provider: Option<Provider>,
	pub selected_visualization: Visualization,
	pub selected_fill_mode: FillMode,
	pub selection_mode: SelectionBitflag,
	pub download: MapDownloadState,
	pub scale_factor: f32,
	pub zoom_with_ctrl: bool,
}

pub type SelectionBitflag = u8;

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

pub type CacheBitflag = u8;

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
	pub const ALL: CacheBitflag = CacheBitflag::MAX;
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

#[derive(Default)]
pub struct UploaderState {
	pub osmchange: OsmChange,
	pub osmchange_text: String,
	pub changeset_creation: Option<OsmResult<NonZeroU32>>,
}

#[derive(Default)]
pub struct AuthenticatorState {
	// todo: currently no way to check which server this belongs to
	pub token: HashMap<TargetServer, OsmResult<OsmToken>>,
	pub authorization_code: String,
	pub request_pending: bool,
}
