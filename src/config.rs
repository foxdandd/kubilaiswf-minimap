//! Settings from `minimap.ini` next to the DLL, read once when the game starts.

use std::path::Path;

use hudhook::imgui::Key;

const DEFAULT_INI: &str = include_str!("../package/minimap.ini");

pub struct Config {
    pub trail: bool,
    pub rotate: bool,
    pub compass: bool,
    pub coop_players: bool,
    pub hide_in_boss_fights: bool,
    pub hide_in_caves: bool,
    pub dungeon_mode: bool,
    pub outdoor_layers: bool,
    pub parchment_maps: bool,
    pub dungeon_height_band: f32,
    pub dungeon_floor_key: Option<Key>,
    pub size: f32,
    pub margin: f32,
    pub toggle_key: Option<Key>,
    pub zoom_key: Option<Key>,
}

impl Config {
    /// Writes the default file on first run so there is something to edit.
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
            let _ = std::fs::write(path, DEFAULT_INI);
            DEFAULT_INI.to_owned()
        });
        Self::from_text(&text)
    }
    fn from_text(text:&str)->Self {
        let mut config = Self {
            trail: true,
            rotate: true,
            compass: true,
            coop_players: true,
            hide_in_boss_fights: false,
            hide_in_caves: true,
            dungeon_mode: false,
            outdoor_layers: true,
            parchment_maps: true,
            dungeon_height_band: 2.0,
            dungeon_floor_key: Some(Key::F11),
            size: 1.0,
            margin: 24.0,
            toggle_key: Some(Key::F7),
            zoom_key: Some(Key::F8),
        };
        for line in text.lines().map(str::trim) {
            if line.starts_with([';', '#']) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.split([';', '#']).next().unwrap_or_default().trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "trail" => set_bool(&mut config.trail, value),
                "rotate" => set_bool(&mut config.rotate, value),
                "compass" => set_bool(&mut config.compass, value),
                "coop_players" => set_bool(&mut config.coop_players, value),
                "hide_in_boss_fights" => set_bool(&mut config.hide_in_boss_fights, value),
                "hide_in_caves" => set_bool(&mut config.hide_in_caves, value),
                "dungeon_mode" => set_bool(&mut config.dungeon_mode, value),
                "outdoor_layers" => set_bool(&mut config.outdoor_layers, value),
                "parchment_maps" => set_bool(&mut config.parchment_maps, value),
                "dungeon_height_band" => set_number(&mut config.dungeon_height_band, value, 0.5..=5.0),
                "dungeon_floor_key" => set_key(&mut config.dungeon_floor_key, value),
                "size" => set_number(&mut config.size, value, 0.5..=2.5),
                "margin" => set_number(&mut config.margin, value, 0.0..=1000.0),
                "toggle_key" => set_key(&mut config.toggle_key, value),
                "zoom_key" => set_key(&mut config.zoom_key, value),
                _ => {}
            }
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_defaults_preserve_explicit_pilot_keys_and_new_coop_setting() {
        let old=Config::from_text("toggle_key = F6\nzoom_key = F7\ndungeon_floor_key = F8\ndungeon_mode = true\n");
        assert_eq!(old.toggle_key,Some(Key::F6));assert_eq!(old.zoom_key,Some(Key::F7));
        assert_eq!(old.dungeon_floor_key,Some(Key::F8));assert!(old.dungeon_mode && old.coop_players);
        let new=Config::from_text("coop_players=false\n");assert!(!new.coop_players);
        assert_eq!(new.toggle_key,Some(Key::F7));assert_eq!(new.zoom_key,Some(Key::F8));
    }
}

// Values that don't parse keep the default.

fn set_bool(setting: &mut bool, value: &str) {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => *setting = true,
        "false" | "0" | "no" | "off" => *setting = false,
        _ => {}
    }
}

fn set_number(setting: &mut f32, value: &str, range: std::ops::RangeInclusive<f32>) {
    if let Ok(number) = value.parse::<f32>() {
        *setting = number.clamp(*range.start(), *range.end());
    }
}

/// Key names as imgui spells them; digits can be written on their own.
fn set_key(setting: &mut Option<Key>, value: &str) {
    if value.eq_ignore_ascii_case("none") {
        *setting = None;
        return;
    }
    let name = if value.len() == 1 && value.as_bytes()[0].is_ascii_digit() {
        format!("Alpha{value}")
    } else {
        value.to_owned()
    };
    if let Some(key) = Key::VARIANTS
        .into_iter()
        .find(|key| format!("{key:?}").eq_ignore_ascii_case(&name))
    {
        *setting = Some(key);
    }
}
