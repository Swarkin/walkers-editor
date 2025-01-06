use std::collections::HashMap;
use eframe::egui::Context;
use walkers::sources::{Attribution, TileSource};
use walkers::{HttpOptions, HttpTiles, TileId, Tiles};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
	#[default] OpenStreetMap,
	Geoportal,
	MapboxSatellite,
	EsriWorldImagery,
}

// https://services.arcgisonline.com/arcgis/rest/services/World_Imagery/MapServer/tile/0/0/0
pub struct EsriWorldImagery {}

impl TileSource for EsriWorldImagery {
	fn tile_url(&self, tile_id: TileId) -> String {
		format!("https://services.arcgisonline.com/arcgis/rest/services/World_Imagery/MapServer/tile/{}/{}/{}", tile_id.zoom, tile_id.y, tile_id.x)
	}

	fn attribution(&self) -> Attribution {
		Attribution {
			text: "Esri, Maxar, Earthstar Geographics, and the GIS User Community",
			url: "https://services.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer",
			logo_light: None, logo_dark: None,
		}
	}

	fn tile_size(&self) -> u32 { 256 }
	
	fn max_zoom(&self) -> u8 { 19 }
}

pub fn http_options() -> HttpOptions {
	HttpOptions {
		// Not sure where to put cache on Android, so it will be disabled for now.
		cache: if cfg!(target_os = "android") || std::env::var("NO_HTTP_CACHE").is_ok() {
			None
		} else {
			Some(".cache".into())
		},
		..Default::default()
	}
}

pub fn providers(egui_ctx: Context) -> HashMap<Provider, Box<dyn Tiles + Send>> {
	let mut providers: HashMap<Provider, Box<dyn Tiles + Send>> = HashMap::default();

	providers.insert(
		Provider::OpenStreetMap,
		Box::new(HttpTiles::with_options(
			walkers::sources::OpenStreetMap,
			http_options(),
			egui_ctx.to_owned(),
		)),
	);

	providers.insert(
		Provider::EsriWorldImagery,
		Box::new(HttpTiles::with_options(
			EsriWorldImagery {},
			http_options(),
			egui_ctx.to_owned(),
		)),
	);

	providers.insert(
		Provider::Geoportal,
		Box::new(HttpTiles::with_options(
			walkers::sources::Geoportal,
			http_options(),
			egui_ctx.to_owned(),
		)),
	);

	// We only show the mapbox map if we have an access token
	if let Some(token) = option_env!("MAPBOX_ACCESS_TOKEN") {
		providers.insert(
			Provider::MapboxSatellite,
			Box::new(HttpTiles::with_options(
				walkers::sources::Mapbox {
					style: walkers::sources::MapboxStyle::Satellite,
					access_token: token.to_string(),
					high_resolution: true,
				},
				http_options(),
				egui_ctx.to_owned(),
			)),
		);
	}
	
	providers
}
