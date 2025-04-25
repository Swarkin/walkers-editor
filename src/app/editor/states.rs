use super::{changes::EditorOsmData, visual::Visualization, EditorPluginState};
use crate::app::config::TargetServer;
use crate::app::osm::OsmToken;
use crate::app::osmchange::OsmChange;
use crate::app::providers::{providers, Provider, TilesKind};
use crate::app::worker::AnyError;
use eframe::egui::{Context, Vec2};
use std::collections::HashMap;
use std::num::NonZeroU32;
use walkers::{MapMemory, Position};

pub struct EditorState {
	pub providers: HashMap<Provider, TilesKind>,
	pub selected_provider: Option<Provider>,
	pub selected_visualizer: Visualization,
	pub map_memory: MapMemory,
	pub editor_osm: EditorOsmData,
	pub editor_state: EditorPluginState,
	pub hidden_windows: u8,
	pub scale_factor: f32,
	pub zoom_with_ctrl: bool,
	pub prev_size: Vec2,
	pub prev_zoom: f64,
	pub prev_pos: Position,
	pub regenerate_points: bool,
	pub map_download_pending: bool,
}

impl EditorState {
	pub fn default(egui_ctx: Context) -> Self {
		Self {
			providers: providers(egui_ctx),
			selected_provider: Some(Provider::default()),
			selected_visualizer: Visualization::Default,
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
			map_download_pending: false,
		}
	}
}

#[derive(Default)]
pub struct UploaderState {
	pub osmchange: OsmChange,
	pub osmchange_text: String,
	pub changeset_creation: Option<Result<NonZeroU32, AnyError>>,
}

#[derive(Default)]
pub struct AuthenticatorState {
	// todo: currently no way to check which server this belongs to
	pub token: HashMap<TargetServer, Result<OsmToken, AnyError>>,
	pub authorization_code: String,
	pub request_pending: bool,
}
