use eframe::egui::{Key, KeyboardShortcut, Modifiers};

const CTRL_SHIFT: Modifiers = Modifiers { ctrl: true, shift: true, alt: false, command: false, mac_cmd: false };

#[cfg(not(feature = "kiosk"))]
pub static FULLSCREEN: &KeyboardShortcut = &KeyboardShortcut::new(Modifiers::NONE, Key::F11);

pub static DOWNLOAD: &KeyboardShortcut = &KeyboardShortcut::new(CTRL_SHIFT, Key::ArrowDown);
pub static WIREFRAME: &KeyboardShortcut = &KeyboardShortcut::new(Modifiers::NONE, Key::W);
