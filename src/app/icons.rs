use eframe::egui::{include_image, ImageSource};

pub const PRIMITIVE_NODE_ICON: ImageSource = include_image!("../../assets/primitives/node24.svg");
pub const PRIMITIVE_WAY_ICON: ImageSource = include_image!("../../assets/primitives/way24.svg");
//pub const PRIMITIVE_AREA_ICON: ImageSource = include_image!("../../assets/primitives/area24.svg");
//pub const PRIMITIVE_RELATION_ICON: ImageSource = include_image!("../../assets/primitives/relation24.svg");

pub const CHECK: ImageSource = include_image!("../../assets/ui/check.svg");
pub const CLOCK: ImageSource = include_image!("../../assets/ui/clock.svg");
pub const COMMIT: ImageSource = include_image!("../../assets/ui/commit.svg");
pub const CROSS: ImageSource = include_image!("../../assets/ui/cross.svg");
pub const DOWNLOAD: ImageSource = include_image!("../../assets/ui/download.svg");
pub const EXTERNAL: ImageSource = include_image!("../../assets/ui/external.svg");
pub const FILE_ARROW_LEFT: ImageSource = include_image!("../../assets/ui/file_arrow_left.svg");
pub const FILE_ARROW_RIGHT: ImageSource = include_image!("../../assets/ui/file_arrow_right.svg");
pub const HASHTAG: ImageSource = include_image!("../../assets/ui/hashtag.svg");
pub const LAYOUT: ImageSource = include_image!("../../assets/ui/layout.svg");
pub const LOGIN: ImageSource = include_image!("../../assets/ui/login.svg");
pub const LOGOUT: ImageSource = include_image!("../../assets/ui/logout.svg");
pub const MOON: ImageSource = include_image!("../../assets/ui/moon.svg");
#[cfg(not(target_family = "wasm"))]
pub const RELOAD: ImageSource = include_image!("../../assets/ui/reload.svg");
pub const SQUARE_X: ImageSource = include_image!("../../assets/ui/square_x.svg");
pub const SUN: ImageSource = include_image!("../../assets/ui/sun.svg");
pub const UPLOAD: ImageSource = include_image!("../../assets/ui/upload.svg");
pub const USER: ImageSource = include_image!("../../assets/ui/user.svg");
pub const WARNING: ImageSource = include_image!("../../assets/ui/warning.svg");

pub const MISC_ARROW: ImageSource = include_image!("../../assets/misc/arrow.svg");
pub const MISC_CHECK: ImageSource = include_image!("../../assets/misc/check.svg");
pub const MISC_CROSS: ImageSource = include_image!("../../assets/misc/cross.svg");
pub const MISC_QUESTION_MARK: ImageSource = include_image!("../../assets/misc/question_mark.svg");

pub const SIDEWALK_YES: ImageSource = include_image!("../../assets/overlays/sidewalk/yes.svg");
pub const SIDEWALK_NO: ImageSource = include_image!("../../assets/overlays/sidewalk/no.svg");
pub const SIDEWALK_SEPARATE: ImageSource = include_image!("../../assets/overlays/sidewalk/separate.svg");
pub const SIDEWALK_UNKNOWN: ImageSource = include_image!("../../assets/overlays/sidewalk/unknown.svg");
