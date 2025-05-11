use super::{changes::EditorOsmData, visual::Visualization, EditorPluginState};
use crate::app::config::TargetServer;
use crate::app::osm::OsmToken;
use crate::app::osm::Result;
use crate::app::osmchange::OsmChange;
use crate::app::providers::{providers, Provider, TilesKind};
use eframe::egui::{Context, Vec2};
use std::collections::HashMap;
use std::num::NonZeroU32;
use walkers::{MapMemory, Position};

pub struct EditorState {
	pub providers: HashMap<Provider, TilesKind>,
	pub selected_provider: Option<Provider>,
	pub selected_visualizer: Visualization,
	pub selection_mode: SelectionMode,
	pub map_memory: MapMemory,
	pub editor_osm: EditorOsmData,
	pub editor_state: EditorPluginState,
	pub hidden_windows: WindowBitflag,
	pub scale_factor: f32,
	pub zoom_with_ctrl: bool,
	pub prev_size: Vec2,
	pub prev_zoom: f64,
	pub prev_pos: Position,
	pub regenerate_points: bool,
	pub regenerate_orphan: bool,
	pub map_download: MapDownloadState,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum SelectionMode {
	Nodes,
	#[default]
	Ways,
	//Areas, // todo
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
	pub fn new(egui_ctx: &Context) -> Self {
		Self {
			providers: providers(egui_ctx),
			selected_provider: Some(Provider::default()),
			selected_visualizer: Visualization::Default,
			selection_mode: SelectionMode::default(),
			map_memory: MapMemory::default(),
			editor_osm: EditorOsmData::default(),
			editor_state: EditorPluginState::default(),
			hidden_windows: 0,
			scale_factor: 1.0,
			zoom_with_ctrl: true,
			prev_size: Vec2::ZERO,
			prev_zoom: 0.0,
			prev_pos: Position::default(),
			regenerate_points: false,
			regenerate_orphan: false,
			map_download: MapDownloadState::Idle(None),
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
