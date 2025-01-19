use eframe::egui::Context;
use osm_parser::convert::{Convert, Projection};
use osm_parser::Coordinate;
use std::collections::HashMap;
use walkers::sources::{Attribution, TileSource};
use walkers::{HttpOptions, HttpTiles, TileId, Tiles};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
	#[default] OpenStreetMap,
	Geoportal,
	MapboxSatellite,
	EsriWorldImagery,
	Bavaria20cm,
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

pub struct Bavaria20cm {}

impl TileSource for Bavaria20cm {
	fn tile_url(&self, tile_id: TileId) -> String {
		let tile_tl = webmercator_tiles::tile2lonlat(tile_id.x, tile_id.y, tile_id.zoom);
		let min = Coordinate::new(tile_tl.1, tile_tl.0);
		let tile_br = webmercator_tiles::tile2lonlat(tile_id.x + 1, tile_id.y + 1, tile_id.zoom);
		let max = Coordinate::new(tile_br.1, tile_br.0);

		let mut tr = Coordinate::new(max.lat, min.lon);
		let mut bl = Coordinate::new(min.lat, max.lon);
		tr.convert_to(Projection::WebMercator);
		bl.convert_to(Projection::WebMercator);

		format!("https://geoservices.bayern.de/od/wms/dop/v1/dop20?FORMAT=image%2Fpng&VERSION=1.1.1&SERVICE=WMS&REQUEST=GetMap&LAYERS=by_dop20c&SRS=EPSG%3A3857&WIDTH=256&HEIGHT=256&BBOX={},{},{},{}", tr.lon, tr.lat, bl.lon, bl.lat)
	}

	fn attribution(&self) -> Attribution {
		Attribution {
			text: "Digitales Orthophoto RGB 20cm\nKostenfreie Geodaten der Bayerischen Vermessungsverwaltung\nLicensed under CC BY 4.0",
			url: "https://geodaten.bayern.de/opengeodata/OpenDataDetail.html?pn=dop20rgb&active=SERVICE",
			logo_light: None, logo_dark: None,
		}
	}

	fn tile_size(&self) -> u32 { 256 }

	fn max_zoom(&self) -> u8 { 20 }
}

pub fn http_options() -> HttpOptions {
	HttpOptions {
		cache: if std::env::var("NO_HTTP_CACHE").is_ok() {
			None
		} else {
			Some(".cache".into())
		},
		user_agent: Some(
			walkers::HeaderValue::from_static(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))),
		),
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
		Provider::Geoportal,
		Box::new(HttpTiles::with_options(
			walkers::sources::Geoportal,
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
		Provider::Bavaria20cm,
		Box::new(HttpTiles::with_options(
			Bavaria20cm {},
			http_options(),
			egui_ctx.to_owned(),
		)),
	);

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
