//! User preferences, saved by eframe between launches.

use eframe::egui::Key;
use gbade_emu::Keys;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub recent_projects: Vec<PathBuf>,
    pub dark_mode: bool,
    pub editor_font_size: f32,
    /// Integer scale of the game screen (0 = fit the panel).
    pub screen_scale: u32,
    pub volume: f32,
    pub muted: bool,
    /// Rebuild and restart the game every time a file is saved.
    pub run_on_save: bool,
    /// Show emulator warnings (bad memory accesses...) in the Console.
    pub show_emulator_warnings: bool,
    pub keymap: KeyMap,
    pub show_inspector: bool,
    /// Where new projects are created by default.
    pub projects_dir: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            recent_projects: Vec::new(),
            dark_mode: true,
            editor_font_size: 14.0,
            screen_scale: 0,
            volume: 0.8,
            muted: false,
            run_on_save: false,
            show_emulator_warnings: true,
            keymap: KeyMap::default(),
            show_inspector: true,
            projects_dir: None,
        }
    }
}

impl Settings {
    pub fn add_recent(&mut self, p: PathBuf) {
        self.recent_projects.retain(|x| x != &p);
        self.recent_projects.insert(0, p);
        self.recent_projects.truncate(10);
    }

    pub fn projects_dir(&self) -> PathBuf {
        self.projects_dir.clone().unwrap_or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            home.join("GBADE Projects")
        })
    }
}

/// Keyboard key for each GBA button. Stored as key names ("ArrowUp", "X").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KeyMap {
    pub a: String,
    pub b: String,
    pub select: String,
    pub start: String,
    pub right: String,
    pub left: String,
    pub up: String,
    pub down: String,
    pub r: String,
    pub l: String,
}

impl Default for KeyMap {
    fn default() -> Self {
        KeyMap {
            a: "X".into(),
            b: "Z".into(),
            select: "Backspace".into(),
            start: "Enter".into(),
            right: "ArrowRight".into(),
            left: "ArrowLeft".into(),
            up: "ArrowUp".into(),
            down: "ArrowDown".into(),
            r: "S".into(),
            l: "A".into(),
        }
    }
}

impl KeyMap {
    /// (GBA button, keyboard key name) pairs.
    pub fn entries(&self) -> [(Keys, &str); 10] {
        [
            (Keys::A, &self.a),
            (Keys::B, &self.b),
            (Keys::SELECT, &self.select),
            (Keys::START, &self.start),
            (Keys::RIGHT, &self.right),
            (Keys::LEFT, &self.left),
            (Keys::UP, &self.up),
            (Keys::DOWN, &self.down),
            (Keys::R, &self.r),
            (Keys::L, &self.l),
        ]
    }

    pub fn entries_mut(&mut self) -> [(Keys, &mut String); 10] {
        [
            (Keys::A, &mut self.a),
            (Keys::B, &mut self.b),
            (Keys::SELECT, &mut self.select),
            (Keys::START, &mut self.start),
            (Keys::RIGHT, &mut self.right),
            (Keys::LEFT, &mut self.left),
            (Keys::UP, &mut self.up),
            (Keys::DOWN, &mut self.down),
            (Keys::R, &mut self.r),
            (Keys::L, &mut self.l),
        ]
    }

    /// Which GBA buttons are held, given a predicate "is this key down".
    pub fn keys_down(&self, is_down: impl Fn(Key) -> bool) -> Keys {
        let mut k = Keys::NONE;
        for (button, name) in self.entries() {
            if let Some(key) = Key::from_name(name) {
                if is_down(key) {
                    k |= button;
                }
            }
        }
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keymap_is_valid() {
        for (_, name) in KeyMap::default().entries() {
            assert!(Key::from_name(name).is_some(), "invalid key name {name}");
        }
    }

    #[test]
    fn keys_down_maps_buttons() {
        let km = KeyMap::default();
        let k = km.keys_down(|key| key == Key::X || key == Key::ArrowLeft);
        assert_eq!(k, Keys::A | Keys::LEFT);
    }

    #[test]
    fn recent_projects_dedup_and_limit() {
        let mut s = Settings::default();
        for i in 0..12 {
            s.add_recent(PathBuf::from(format!("/p{i}")));
        }
        s.add_recent(PathBuf::from("/p5"));
        assert_eq!(s.recent_projects.len(), 10);
        assert_eq!(s.recent_projects[0], PathBuf::from("/p5"));
        assert_eq!(s.recent_projects.iter().filter(|p| **p == PathBuf::from("/p5")).count(), 1);
    }
}
