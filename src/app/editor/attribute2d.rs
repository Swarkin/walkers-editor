use super::consts::*;
use eframe::egui::Color32;
use osm_parser::Tags;

#[derive(Debug, Default)]
pub struct Attribute2D {
	pub left: TagValue,
	pub right: TagValue,
}

// tag value: sidewalk:left=*
#[derive(Debug, Default, Copy, Clone)]
pub enum TagValue {
	Yes,
	No,
	Separate,
	#[default] Unknown,
}

// tag suffix, sidewalk:*=yes
#[derive(Debug, Default)]
pub enum TagSuffix {
	Left,
	Right,
	Both,
	Separate,
	No,
	#[default] Unknown,
}

impl From<&str> for TagValue {
	fn from(value: &str) -> Self {
		match value {
			"yes" => TagValue::Yes,
			"no" | "none" => TagValue::No,
			"separate" => TagValue::Separate,
			_ => TagValue::Unknown,
		}
	}
}

#[allow(clippy::from_over_into)]
impl Into<Color32> for TagValue {
	fn into(self) -> Color32 {
		match self {
			TagValue::Yes => SIDEWALK_YES_COLOR,
			TagValue::No => SIDEWALK_NO_COLOR,
			TagValue::Separate => SIDEWALK_SEPARATE_COLOR,
			TagValue::Unknown => SIDEWALK_UNKNOWN_COLOR,
		}
	}
}

impl From<&str> for TagSuffix {
	fn from(value: &str) -> Self {
		match value {
			"left" => TagSuffix::Left,
			"right" => TagSuffix::Right,
			"both" => TagSuffix::Both,
			"separate" => TagSuffix::Separate,
			"no" | "none" => TagSuffix::No,
			_ => TagSuffix::Unknown,
		}
	}
}

impl From<TagSuffix> for Attribute2D {
	fn from(value: TagSuffix) -> Self {
		let left: TagValue;
		let right: TagValue;

		match value {
			TagSuffix::Left => {
				left = TagValue::Yes;
				right = TagValue::No;
			},
			TagSuffix::Right => {
				left = TagValue::No;
				right = TagValue::Yes;
			},
			TagSuffix::Both => {
				left = TagValue::Yes;
				right = TagValue::Yes;
			},
			TagSuffix::Separate => {
				left = TagValue::Separate;
				right = TagValue::Separate;
			},
			TagSuffix::No => {
				left = TagValue::No;
				right = TagValue::No;
			},
			TagSuffix::Unknown => {
				left = TagValue::Unknown;
				right = TagValue::Unknown;
			},
		}

		Self { left, right }
	}
}

impl Attribute2D {
	pub fn new(tags: &Tags, tag: &str) -> Self {
		let mut attribute2d = Attribute2D::default();

		if let Some(v) = tags.get("sidewalk") {
			attribute2d = Attribute2D::from(TagSuffix::from(v.as_str()));
		}
		if let Some(v) = tags.get(&format!("{tag}:left")) {
			attribute2d.left = TagValue::from(v.as_str());
		}
		if let Some(v) = tags.get(&format!("{tag}:right")) {
			attribute2d.right = TagValue::from(v.as_str());
		}
		if let Some(v) = tags.get(&format!("{tag}:both")) {
			let v = TagValue::from(v.as_str());
			attribute2d.left = v;
			attribute2d.right = v;
		}

		attribute2d
	}
}
