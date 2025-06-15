use super::visual::{SIDEWALK_NO_COLOR, SIDEWALK_SEPARATE_COLOR, SIDEWALK_UNKNOWN_COLOR, SIDEWALK_YES_COLOR};
use eframe::egui::Color32;
use osm_parser::Tags;
use std::fmt::{Display, Formatter};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Attribute2D {
	pub left: TagValue,
	pub right: TagValue,
}

impl Attribute2D {
	pub fn new(tags: &Tags, tag: &str) -> Self {
		let mut attr = Attribute2D::default();

		if let Some(v) = tags.get("sidewalk") {
			attr = Attribute2D::from(TagSuffix::from(v.as_str()));
		}
		if let Some(v) = tags.get(&format!("{tag}:left")) {
			attr.left = TagValue::from(v.as_str());
		}
		if let Some(v) = tags.get(&format!("{tag}:right")) {
			attr.right = TagValue::from(v.as_str());
		}
		if let Some(v) = tags.get(&format!("{tag}:both")) {
			let v = TagValue::from(v.as_str());
			attr.left = v;
			attr.right = v;
		}

		attr
	}

	pub fn into_tags(self, tag: &str) -> Tags {
		let mut tags = Tags::new();

		match self.left {
			TagValue::Yes | TagValue::No | TagValue::Separate => {
				if self.left == self.right {
					tags.insert(format!("{tag}:both"), self.left.to_string());
					return tags;
				} else {
					tags.insert(format!("{tag}:left"), self.left.to_string());
				}
			},
			_ => {},
		}

		match self.right {
			TagValue::Yes | TagValue::No | TagValue::Separate => {
				tags.insert(format!("{tag}:right"), self.right.to_string());
			},
			_ => {},
		}

		tags
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

// tag value: sidewalk:left=*
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum TagValue {
	Yes,
	No,
	Separate,
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

impl Display for TagValue {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			TagValue::Yes => "yes",
			TagValue::No => "no",
			TagValue::Separate => "separate",
			TagValue::Unknown => "unknown",
		})
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


// tag suffix, sidewalk:*=yes
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum TagSuffix {
	Left,
	Right,
	Both,
	Separate,
	No,
	#[default] Unknown,
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
