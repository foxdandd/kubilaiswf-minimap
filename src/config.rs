//! Settings from `minimap.ini` next to the DLL, read once when the game starts.

use std::path::Path;

use hudhook::imgui::Key;

const DEFAULT_INI: &str = "\
; Elden Ring minimap settings. Restart the game after changing them.

; Draw the path you walked on the minimap (true / false).
trail = true

; Turn the map with the camera. false keeps north up.
rotate = true

; Show the N / E / S / W letters.
compass = true

; Hide the minimap while a boss health bar is on screen.
hide_in_boss_fights = false

; Hide the minimap in catacombs, caves and tunnels, which have no map of their own.
hide_in_caves = true

; Size of the minimap, 1.0 is the default. From 0.5 to 2.5.
size = 1.0

; Distance from the top right corner of the screen, in pixels at 1080p.
margin = 24

; Keys: F1-F12, A-Z, 0-9, Insert, Delete, Home, End, PageUp, PageDown, Keypad0-Keypad9,
; or none to turn the key off.
toggle_key = F9
zoom_key = F10
";

pub struct Config {
    pub trail: bool,
    pub rotate: bool,
    pub compass: bool,
    pub hide_in_boss_fights: bool,
    pub hide_in_caves: bool,
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
        let mut config = Self {
            trail: true,
            rotate: true,
            compass: true,
            hide_in_boss_fights: false,
            hide_in_caves: true,
            size: 1.0,
            margin: 24.0,
            toggle_key: Some(Key::F9),
            zoom_key: Some(Key::F10),
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
                "hide_in_boss_fights" => set_bool(&mut config.hide_in_boss_fights, value),
                "hide_in_caves" => set_bool(&mut config.hide_in_caves, value),
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
