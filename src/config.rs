//! Settings from `minimap.ini` next to the DLL, read once when the game starts.

use std::path::Path;

const DEFAULT_INI: &str = "\
; Elden Ring minimap settings. Restart the game after changing them.

; Draw the path you walked on the minimap (true / false).
trail = true
";

pub struct Config {
    pub trail: bool,
}

impl Config {
    /// Writes the default file on first run so there is something to edit.
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|_| {
            let _ = std::fs::write(path, DEFAULT_INI);
            DEFAULT_INI.to_owned()
        });
        let mut config = Self { trail: true };
        for line in text.lines().map(str::trim) {
            if line.starts_with([';', '#']) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim().eq_ignore_ascii_case("trail") {
                config.trail = parse_bool(value).unwrap_or(config.trail);
            }
        }
        config
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}
