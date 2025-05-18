use super::{changes::EditorOsmData, visual::Visualization, EditorPluginState};
use crate::app::config::TargetServer;
use crate::app::editor::visual::FillMode;
use crate::app::osm::{OsmToken, Result};
use crate::app::osmchange::OsmChange;
use crate::app::providers::{providers, Provider, TilesKind};
use crate::app::windows::WindowBitflag;
use eframe::egui::{Context, Vec2};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;
use walkers::{MapMemory, Position};

pub struct EditorState {
	pub providers: HashMap<Provider, TilesKind>,
	pub selected_provider: Option<Provider>,
	pub selected_visualizer: Visualization,
	pub selected_fill_mode: FillMode,
	pub selection_mode: SelectionBitflag,
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
			selected_visualizer: Visualization::default(),
			selected_fill_mode: FillMode::default(),
			selection_mode: SelectionFlag::Nodes as u8 + SelectionFlag::Ways as u8,
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
