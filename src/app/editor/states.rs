use super::{cache::EditorOsmData, visual::Visualization, EditorPluginState};
use crate::app::config::TargetServer;
use crate::app::editor::visual::FillMode;
use crate::app::osm::{OsmToken, Result};
use crate::app::osmchange::OsmChange;
use crate::app::providers::{Provider, ProviderMap, TilesKind};
use crate::app::windows::WindowBitflag;
use eframe::egui::Vec2;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;
use walkers::MapMemory;

pub struct EditorState {
	pub tile_providers: HashMap<Provider, TilesKind>,
	pub map_memory: MapMemory,
	pub map_state: MapState,
	pub plugin_state: EditorPluginState,
	pub osm_data: EditorOsmData,
	pub window_flags: WindowBitflag,
	pub prev_size: Vec2,
	pub prev_zoom: f64,
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

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum SelectionFlag {
	Nodes = 1 << 0,
	Ways = 1 << 1,
	Areas = 1 << 2, // todo: implement
}

impl Display for SelectionFlag {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			SelectionFlag::Nodes => "Nodes",
			SelectionFlag::Ways => "Ways",
			SelectionFlag::Areas => "Areas",
		})
	}
}

impl SelectionFlag {
	pub const ITER: [SelectionFlag; 3] = [SelectionFlag::Nodes, SelectionFlag::Ways, SelectionFlag::Areas];
}

pub type CacheBitflag = u8;

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum CacheFlag {
	Projection = 1 << 0,
	Orphan = 1 << 1,
	WayNodesDedup = 1 << 2,
	Triangulation = 1 << 3,
}

pub enum MapDownloadState {
	Idle(Option<Result<()>>),
	Downloading,
}

impl MapDownloadState {
	pub fn is_busy(&self) -> bool {
		match self {
			MapDownloadState::Idle(_) => false,
			MapDownloadState::Downloading => true,
		}
	}
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
			prev_zoom: 0.0,
		}
	}
}

#[derive(Default)]
pub struct UploaderState {
	pub osmchange: OsmChange,
	pub osmchange_text: String,
	pub changeset_creation: Option<Result<NonZeroU32>>,
}

#[derive(Default)]
pub struct AuthenticatorState {
	// todo: currently no way to check which server this belongs to
	pub token: HashMap<TargetServer, Result<OsmToken>>,
	pub authorization_code: String,
	pub request_pending: bool,
}
