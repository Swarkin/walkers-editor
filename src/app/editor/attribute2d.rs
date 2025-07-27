use super::visual::{SIDEWALK_NO_COLOR, SIDEWALK_SEPARATE_COLOR, SIDEWALK_UNKNOWN_COLOR, SIDEWALK_YES_COLOR};
use eframe::egui::Color32;
use osm_parser::Tags;
use std::fmt::{Display, Formatter};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Attribute2D {
	pub left: TagValue,
	pub right: TagValue,
}

impl Attribute2D {
	pub fn new(tags: &Tags, tag: &str) -> Self {
		let mut attr = Self::default();

		if let Some(v) = tags.get("sidewalk") {
			attr = Self::from(TagSuffix::from(v.as_str()));
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
		let mut tags = Tags::default();

		match self.left {
			TagValue::Yes | TagValue::No | TagValue::Separate => {
				if self.left == self.right {
					tags.insert(format!("{tag}:both"), self.left.to_string());
					return tags;
				}
				tags.insert(format!("{tag}:left"), self.left.to_string());
			},
			TagValue::Unknown => {},
		}

		match self.right {
			TagValue::Yes | TagValue::No | TagValue::Separate => {
				tags.insert(format!("{tag}:right"), self.right.to_string());
			},
			TagValue::Unknown => {},
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
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum TagValue {
	Yes,
	No,
	Separate,
	#[default] Unknown,
}

impl From<&str> for TagValue {
	fn from(value: &str) -> Self {
		match value {
			"yes" => Self::Yes,
			"no" | "none" => Self::No,
			"separate" => Self::Separate,
			_ => Self::Unknown,
		}
	}
}

impl Display for TagValue {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", match self {
			Self::Yes => "yes",
			Self::No => "no",
			Self::Separate => "separate",
			Self::Unknown => "unknown",
		})
	}
}

#[allow(clippy::from_over_into)]
impl Into<Color32> for TagValue {
	fn into(self) -> Color32 {
		match self {
			Self::Yes => SIDEWALK_YES_COLOR,
			Self::No => SIDEWALK_NO_COLOR,
			Self::Separate => SIDEWALK_SEPARATE_COLOR,
			Self::Unknown => SIDEWALK_UNKNOWN_COLOR,
		}
	}
}


// tag suffix, sidewalk:*=yes
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
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
			"left" => Self::Left,
			"right" => Self::Right,
			"both" => Self::Both,
			"separate" => Self::Separate,
			"no" | "none" => Self::No,
			_ => Self::Unknown,
		}
	}
}
