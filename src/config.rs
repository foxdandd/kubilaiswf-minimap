//! Settings from `minimap.ini` next to the DLL, read once when the game starts.

use std::path::Path;

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
";

pub struct Config {
    pub trail: bool,
    pub rotate: bool,
    pub compass: bool,
    pub hide_in_boss_fights: bool,
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
        };
        for line in text.lines().map(str::trim) {
            if line.starts_with([';', '#']) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let setting = match key.trim().to_ascii_lowercase().as_str() {
                "trail" => &mut config.trail,
                "rotate" => &mut config.rotate,
                "compass" => &mut config.compass,
                "hide_in_boss_fights" => &mut config.hide_in_boss_fights,
                _ => continue,
            };
            *setting = parse_bool(value).unwrap_or(*setting);
        }
        config
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    let value = value.split([';', '#']).next().unwrap_or_default();
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}
