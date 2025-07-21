use osm_parser::{Id, OsmData};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, AABB};
use rustc_hash::FxHashMap;

pub type WebMercatorPoint = [f32; 2];
pub type NodeEntry = GeomWithData<WebMercatorPoint, Id>;
pub type WayEntry = GeomWithData<Rectangle<WebMercatorPoint>, Id>;

pub type RTreeNodes = RTree<NodeEntry>;
pub type RTreeWays = RTree<WayEntry>;

#[derive(Default)]
pub struct RStarOsmData {
	pub nodes: RTreeNodes,
	pub ways: RTreeWays,
}

impl From<&OsmData> for RStarOsmData {
	fn from(data: &OsmData) -> Self {
		let mut positions = FxHashMap::<Id, WebMercatorPoint>::from_iter(
			data.nodes.iter()
				.map(|(id, node)| {
					(*id, WebMercatorPoint::from([node.pos.lat as f32, node.pos.lon as f32]))
				})
		);

		let ways = RTreeWays::bulk_load(
			data.ways.iter().map(|(id, way)| {
				WayEntry::new(Rectangle::from_aabb(AABB::from_points(way.nodes.iter().map(|x| positions.get(x).unwrap()))), *id)
			}).collect()
		);

		let nodes = RTreeNodes::bulk_load(
			positions.drain().map(|(id, point)| {
				NodeEntry::new(point, id)
			}).collect()
		);

		Self { nodes, ways }
	}
}
