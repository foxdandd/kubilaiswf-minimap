//! Streams the game's own world map textures around the player.
//!
//! The menu map is a 41x41 grid of 256px tiles (`MENU_MapTile_Mxx_L0_XX_YY_flags`) at one pixel
//! per meter, with Y growing northwards. Every grid cell has a variant per combination of map
//! fragments; the game's `.mtmsk` lists which fragments touch each cell, and the variant named
//! after that full mask is the cell as it looks with every fragment collected.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::archive;
use crate::formats::{BinderEntry, Oodle, first_tpf_texture, read_binder};
use crate::reader::{Reader, invalid};

pub const TILE: usize = 256;
pub const WINDOW_TILES: usize = 3;
pub const WINDOW: usize = TILE * WINDOW_TILES;
/// Shown for cells with no texture, matching the map's unexplored parchment.
const EMPTY: [u8; 4] = [40, 34, 26, 255];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MapId {
    Overworld,
    Underground,
}

impl MapId {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "M00" => Some(Self::Overworld),
            "M01" => Some(Self::Underground),
            _ => None,
        }
    }
}

/// Converts an overworld-aligned position (meters, see `game.rs`) to menu map pixels, Y up.
/// Map cell (i, j) shows overworld tile m60_{i+28}_{j+24}_00, centers aligned.
pub fn map_pixel([x, z]: [f32; 2]) -> [f32; 2] {
    const HALF: f32 = TILE as f32 / 2.0;
    [x + HALF - 28.0 * TILE as f32, z + HALF - 24.0 * TILE as f32]
}

/// The inverse of `map_pixel` for the game's own world map coordinates, which run Y down
/// over the 41-cell map (checked against the player position the game keeps for its map).
pub fn from_game_map([x, y]: [f32; 2]) -> [f32; 2] {
    const HALF: f32 = TILE as f32 / 2.0;
    let y_up = 41.0 * TILE as f32 - y;
    [
        x - HALF + 28.0 * TILE as f32,
        y_up - HALF + 24.0 * TILE as f32,
    ]
}

/// 3x3 map cells stitched into one RGBA image, rows top (north) first.
pub struct Window {
    pub map: MapId,
    /// Grid cell of the window's south-west corner.
    pub origin: [i32; 2],
    pub rgba: Vec<u8>,
}

pub struct Loader {
    requests: Sender<(MapId, [i32; 2])>,
    // The render loop has to be Sync, which a bare Receiver isn't.
    windows: Mutex<Receiver<Window>>,
}

impl Loader {
    /// Starts reading the map archives on a background thread. If they can't be read the
    /// loader just never produces windows and the minimap stays a plain compass.
    pub fn spawn(game_dir: &Path) -> Self {
        let (requests, request_rx) = channel();
        let (window_tx, windows) = channel();
        let game_dir = game_dir.to_owned();
        std::thread::spawn(move || {
            let _ = serve(&game_dir, &request_rx, &window_tx);
        });
        Self {
            requests,
            windows: Mutex::new(windows),
        }
    }

    pub fn request(&self, map: MapId, origin: [i32; 2]) {
        let _ = self.requests.send((map, origin));
    }

    pub fn poll(&self) -> Option<Window> {
        self.windows.lock().unwrap().try_iter().last()
    }
}

fn serve(
    game_dir: &Path,
    requests: &Receiver<(MapId, [i32; 2])>,
    windows: &Sender<Window>,
) -> io::Result<()> {
    let oodle = Oodle::load(&game_dir.join("oo2core_6_win64.dll"))?;
    let masks =
        read_masks(&oodle.decompress_dcx(
            &archive::open(game_dir, "/menu/71_maptile.mtmskbnd.dcx")?.read_all()?,
        )?)?;
    let tiles = select_tiles(
        read_binder(&archive::open(game_dir, "/menu/71_maptile.tpfbhd")?.read_all()?)?,
        &masks,
    );
    let mut bdt = archive::open(game_dir, "/menu/71_maptile.tpfbdt")?;
    let mut decoded: HashMap<(MapId, [i32; 2]), Vec<u8>> = HashMap::new();

    while let Ok(mut request) = requests.recv() {
        // Only the newest request matters once the player has moved on.
        while let Ok(newer) = requests.try_recv() {
            request = newer;
        }
        let (map, origin) = request;
        // Nine cells are needed at a time; dropping the rest bounds memory to a few MB.
        decoded.retain(|&(m, [x, y]), _| {
            m == map
                && (origin[0] - 1..=origin[0] + 3).contains(&x)
                && (origin[1] - 1..=origin[1] + 3).contains(&y)
        });

        let mut rgba = vec![0u8; WINDOW * WINDOW * 4];
        for dy in 0..WINDOW_TILES {
            for dx in 0..WINDOW_TILES {
                let cell = [origin[0] + dx as i32, origin[1] + dy as i32];
                let tile = match decoded.entry((map, cell)) {
                    Entry::Occupied(tile) => tile.into_mut(),
                    Entry::Vacant(slot) => slot.insert(match tiles.get(&(map, cell)) {
                        Some(entry) => {
                            decode_tile(&oodle, &bdt.read_at(entry.offset, entry.size)?)?
                        }
                        None => EMPTY.repeat(TILE * TILE),
                    }),
                };
                // Cells stack northwards, image rows go southwards.
                let top = (WINDOW_TILES - 1 - dy) * TILE;
                for row in 0..TILE {
                    let at = ((top + row) * WINDOW + dx * TILE) * 4;
                    rgba[at..at + TILE * 4]
                        .copy_from_slice(&tile[row * TILE * 4..(row + 1) * TILE * 4]);
                }
            }
        }
        if windows.send(Window { map, origin, rgba }).is_err() {
            return Ok(());
        }
    }
    Ok(())
}

/// Reads `<MapTileMask id="..." mask="..."/>` entries, where id is level*10000 + x*100 + y.
fn read_masks(bnd: &[u8]) -> io::Result<HashMap<(MapId, [i32; 2]), u32>> {
    let mut masks = HashMap::new();
    for entry in read_binder(bnd)? {
        let Some(map) = entry
            .name
            .rsplit('\\')
            .next()
            .and_then(|f| f.get(13..16))
            .and_then(MapId::from_name)
        else {
            continue;
        };
        let xml =
            String::from_utf8_lossy(Reader(bnd).bytes(entry.offset, entry.size)?).into_owned();
        for line in xml.lines() {
            let (Some(id), Some(mask)) = (attribute(line, "id"), attribute(line, "mask")) else {
                continue;
            };
            if id < 10000 {
                masks.insert((map, [(id / 100) as i32, (id % 100) as i32]), mask);
            }
        }
    }
    Ok(masks)
}

fn attribute(line: &str, name: &str) -> Option<u32> {
    let start = line.find(&format!(" {name}=\""))? + name.len() + 3;
    line[start..].split('"').next()?.parse().ok()
}

/// Keeps, for every L0 cell, the variant whose flags equal the cell's full fragment mask.
fn select_tiles(
    entries: Vec<BinderEntry>,
    masks: &HashMap<(MapId, [i32; 2]), u32>,
) -> HashMap<(MapId, [i32; 2]), BinderEntry> {
    let mut tiles = HashMap::new();
    for entry in entries {
        // e.g. 71_MapTile\MENU_MapTile_M00_L0_12_31_00000100.tpf.dcx
        let Some(stem) = entry
            .name
            .rsplit('\\')
            .next()
            .and_then(|f| f.strip_suffix(".tpf.dcx"))
        else {
            continue;
        };
        let parts: Vec<&str> = stem.split('_').collect();
        let [_, _, map, "L0", x, y, flags] = parts[..] else {
            continue;
        };
        let (Some(map), Ok(x), Ok(y), Ok(flags)) = (
            MapId::from_name(map),
            x.parse(),
            y.parse(),
            u32::from_str_radix(flags, 16),
        ) else {
            continue;
        };
        if masks.get(&(map, [x, y])).copied().unwrap_or(0) == flags {
            tiles.insert((map, [x, y]), entry);
        }
    }
    tiles
}

fn decode_tile(oodle: &Oodle, dcx: &[u8]) -> io::Result<Vec<u8>> {
    let tpf = oodle.decompress_dcx(dcx)?;
    let dds = Reader(first_tpf_texture(&tpf)?);
    let (height, width) = (dds.u32(0xC)? as usize, dds.u32(0x10)? as usize);
    if (width, height) != (TILE, TILE) {
        return Err(invalid("unexpected map tile size"));
    }
    let mut pixels = vec![0u32; TILE * TILE];
    let result = match (dds.bytes(0x54, 4)?, dds.u32(0x80)?) {
        // DXGI_FORMAT_BC7_UNORM(_SRGB)
        (b"DX10", 98 | 99) => texture2ddecoder::decode_bc7(&dds.0[0x94..], TILE, TILE, &mut pixels),
        (b"DXT1", _) => texture2ddecoder::decode_bc1(&dds.0[0x80..], TILE, TILE, &mut pixels),
        _ => return Err(invalid("unsupported map tile format")),
    };
    result.map_err(invalid)?;
    // The decoder packs BGRA; textures are uploaded as RGBA.
    Ok(pixels
        .iter()
        .flat_map(|p| {
            let [b, g, r, a] = p.to_le_bytes();
            [r, g, b, a]
        })
        .collect())
}
