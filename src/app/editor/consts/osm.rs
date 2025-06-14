#![allow(dead_code)]

use eframe::egui::Color32;

pub const NODE_SIZE: f32 = 3.0;
pub const NODE_SIZE_ORPHAN: f32 = 4.0;
pub const NODE_COLOR: Color32 = Color32::WHITE;
pub const NODE_CONNECTED_COLOR: Color32 = Color32::LIGHT_GRAY;
pub const NODE_STROKE_COLOR: Color32 = Color32::GRAY;

pub const WAY_WIDTH: f32 = 1.0;
pub const WAY_COLOR: Color32 = Color32::GRAY;
pub const NODE_STROKE_WIDTH: f32 = 1.0;

pub const HOVER_COLOR: Color32 = Color32::from_rgb(100, 200, 255);
pub const HOVER_SIZE_INCREASE: f32 = 2.0;
pub const SELECTION_COLOR: Color32 = Color32::from_rgb(40, 180, 255);
pub const SELECTION_SIZE_INCREASE: f32 = 2.5;

pub const PATH_WIDTH: f32 = 2.5;
pub const SERVICE_ROAD_WIDTH: f32 = 4.0;
pub const MINOR_ROAD_WIDTH: f32 = 5.0;
pub const MAJOR_ROAD_WIDTH: f32 = 6.0;
pub const BUILDING_WIDTH: f32 = 2.0;

pub const ATTRIBUTION_URL: &str = "https://www.openstreetmap.org/copyright";

pub const BUILDING_COLOR: Color32 = Color32::from_rgb(224, 110, 95);
pub const PATH_COLOR: Color32 = Color32::from_rgb(221, 204, 170);
pub const FOOTWAY_COLOR: Color32 = Color32::WHITE;
pub const STEPS_COLOR: Color32 = Color32::from_rgb(129, 210, 92);
pub const TRACK_COLOR: Color32 = Color32::from_rgb(197, 181, 159);

//region highway
// roads
pub const MOTORWAY: &str = "motorway";
pub const TRUNK: &str = "trunk";
pub const PRIMARY: &str = "primary";
pub const SECONDARY: &str = "secondary";
pub const TERTIARY: &str = "tertiary";
pub const UNCLASSIFIED: &str = "unclassified";
pub const RESIDENTIAL: &str = "residential";

// link roads
pub const MOTORWAY_LINK: &str = "motorway_link";
pub const TRUNK_LINK: &str = "trunk_link";
pub const PRIMARY_LINK: &str = "primary_link";
pub const SECONDARY_LINK: &str = "secondary_link";
pub const TERTIARY_LINK: &str = "tertiary_link";

// special road types
pub const LIVING_STREET: &str = "living_street";
pub const SERVICE: &str = "service";
pub const PEDESTRIAN: &str = "pedestrian";
pub const TRACK: &str = "track";
pub const BUS_GUIDEWAY: &str = "bus_guideway";
pub const ESCAPE: &str = "escape";
pub const RACEWAY: &str = "raceway";
pub const ROAD: &str = "road";
pub const BUSWAY: &str = "busway";

// paths
pub const FOOTWAY: &str = "footway";
pub const CYCLEWAY: &str = "cycleway";
pub const BRIDLEWAY: &str = "bridleway";
pub const STEPS: &str = "steps";
pub const CORRIDOR: &str = "corridor";
pub const PATH: &str = "path";
pub const VIA_FERRATA: &str = "via_ferrata";

// lifecycle
pub const PROPOSED: &str = "proposed";
pub const CONSTRUCTION: &str = "construction";
//endregion
