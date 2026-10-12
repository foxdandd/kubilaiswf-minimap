use std::sync::{Mutex, OnceLock};

use eldenring::cs::{
    BlockId, BonfireWarpParam, CSCamera, CSEventFlagMan, CSFade, CSFeManHudState, CSFeManImp,
    CSMenuManImp, SoloParamRepository, WorldChrMan, WorldMapLegacyConvParam, WorldMapPointParam,
};
use fromsoftware_shared::FromStatic;

use crate::maptiles::{self, MapId};

/// Copied out on a game task so the render thread never touches live game objects.
#[derive(Clone)]
pub struct Snapshot {
    /// Meters on the overworld grid where the world map can place the player, otherwise local
    /// to `space`.
    pub position: [f32; 2],
    /// Positions are only comparable within the same space.
    pub space: i32,
    /// The menu map showing `position`, if any.
    pub map: Option<MapId>,
    pub facing: [f32; 2],
    pub camera_right: [f32; 2],
    pub camera_forward: [f32; 2],
    /// Screen fade from 0 to 1; loading screens and warps sit behind a full one.
    pub fade: f32,
    /// False while a menu has the HUD hidden or changed.
    pub hud_visible: bool,
    pub graces: Vec<[f32; 2]>,
    pub beacons: Vec<Beacon>,
    /// Includes whatever map mods like Map for Goblins add.
    pub points: Vec<MapPoint>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PointKind {
    Landmark,
    Npc,
    Invasion,
    Boss,
    /// A grace shown before it's discovered.
    Grace,
    /// Anything else; map mods mostly add item pickups.
    Item,
}

#[derive(Clone, Copy)]
pub struct MapPoint {
    pub kind: PointKind,
    pub position: [f32; 2],
    /// Defeated bosses and the like, which the game marks as done.
    pub cleared: bool,
}

/// A beacon placed on the world map, numbered like the game numbers it.
#[derive(Clone, Copy)]
pub struct Beacon {
    pub number: u8,
    pub position: [f32; 2],
}

pub static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);

pub fn update() {
    *LATEST.lock().unwrap() = read();
}

fn read() -> Option<Snapshot> {
    // SAFETY: game tasks run in step with the game's own updates of these singletons.
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let camera = unsafe { CSCamera::instance() }.ok()?;
    let player = world_chr_man.main_player.as_ref()?;
    let fade = unsafe { CSFade::instance() }.map_or(0.0, |fade| {
        fade.fade_plates
            .iter()
            .map(|plate| plate.current_color.a)
            .fold(0.0, f32::max)
    });
    let hud_visible = unsafe { CSFeManImp::instance() }
        .map_or(true, |fe| fe.hud_state == CSFeManHudState::Default);

    // Havok space, which the camera and physics matrices use, shares axes and
    // scale with block space, so their direction vectors apply to block positions.
    let view = &camera.pers_cam_1.matrix;
    let model = &player.chr_ins.chr_ctrl.physics_model_matrix;
    let (position, space, map) = locate(
        player.current_block_id,
        [player.block_position.x, player.block_position.z],
    );
    Some(Snapshot {
        position,
        space,
        map,
        // Characters face down their model's -Z axis (verified in game).
        facing: [-model.2.0, -model.2.2],
        camera_right: [view.0.0, view.0.2],
        camera_forward: [view.2.0, view.2.2],
        fade,
        hud_visible,
        graces: discovered_graces(position, space),
        beacons: map.map_or_else(Vec::new, placed_beacons),
        points: map.map_or_else(Vec::new, |map| visible_points(map, position, space)),
    })
}

/// Converts a block-local position to the minimap's coordinates: (position, space, map).
fn locate(block: BlockId, local: [f32; 2]) -> ([f32; 2], i32, Option<MapId>) {
    if block.is_overworld() {
        // Overworld blocks are mAA_XX_ZZ_LL grid tiles of (256 << LL) meters with positions
        // relative to the tile center. Larger tiles are centered on their 2x2 or 4x4 group of
        // small tiles, hence the extra half-tile offsets.
        let tile = (256u32 << (block.index() & 0x3)) as f32;
        let center = (tile - 256.0) / 2.0;
        let position = [
            local[0] + f32::from(block.block()) * tile + center,
            local[1] + f32::from(block.region()) * tile + center,
        ];
        // The Shadow of the Erdtree map (area 61) isn't calibrated yet.
        let map = (block.area() == 60).then_some(MapId::Overworld);
        return (position, i32::from(block.area()), map);
    }
    let Some(conversion) = legacy_conversion(block, local) else {
        return (local, i32::from(block), None);
    };
    let position = [
        local[0] + conversion.offset[0],
        local[1] + conversion.offset[1],
    ];
    // Underground areas share the overworld's coordinates but have their own map.
    let (space, map) = match (conversion.dst_area, block.area()) {
        (60, 12) => (12, Some(MapId::Underground)),
        (60, _) => (60, Some(MapId::Overworld)),
        (area, _) => (i32::from(area), None),
    };
    (position, space, map)
}

/// One row of WorldMapLegacyConvParam: a point in a dungeon and where the world map draws it.
struct Conversion {
    src: (u8, u8, u8),
    src_pos: [f32; 2],
    dst_area: u8,
    /// Added to a dungeon position to get the overworld-aligned position.
    offset: [f32; 2],
}

static CONVERSIONS: OnceLock<Vec<Conversion>> = OnceLock::new();

/// Dungeons that are drawn on the world map can list several reference points, each valid
/// around itself, so the one nearest the player is used.
fn legacy_conversion(block: BlockId, local: [f32; 2]) -> Option<&'static Conversion> {
    let src = (block.area(), block.block(), block.region());
    conversions()?
        .iter()
        .filter(|c| c.src == src)
        .min_by(|a, b| {
            distance_squared(a.src_pos, local).total_cmp(&distance_squared(b.src_pos, local))
        })
}

fn conversions() -> Option<&'static [Conversion]> {
    if let Some(conversions) = CONVERSIONS.get() {
        return Some(conversions);
    }
    // SAFETY: the param repository is only read here, on a game task.
    let params = unsafe { SoloParamRepository::instance() }.ok()?;
    let conversions: Vec<Conversion> = (0..)
        .map_while(|i| params.get_row_by_index::<WorldMapLegacyConvParam>(i))
        .map(|row| {
            let src_pos = [row.src_pos_x(), row.src_pos_z()];
            let dst = [
                f32::from(row.dst_grid_x_no()) * 256.0 + row.dst_pos_x(),
                f32::from(row.dst_grid_z_no()) * 256.0 + row.dst_pos_z(),
            ];
            Conversion {
                src: (row.src_area_no(), row.src_grid_x_no(), row.src_grid_z_no()),
                src_pos,
                dst_area: row.dst_area_no(),
                offset: [dst[0] - src_pos[0], dst[1] - src_pos[1]],
            }
        })
        .collect();
    // The params load after the player can already exist; try again next frame.
    if conversions.is_empty() {
        return None;
    }
    Some(CONVERSIONS.get_or_init(|| conversions))
}

fn distance_squared(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

/// Graces further than this aren't drawn even at the widest zoom.
const GRACE_RANGE: f32 = 400.0;

struct Grace {
    event_flag: u32,
    position: [f32; 2],
    space: i32,
}

static GRACES: OnceLock<Vec<Grace>> = OnceLock::new();

fn discovered_graces(position: [f32; 2], space: i32) -> Vec<[f32; 2]> {
    let (Some(graces), Ok(flags)) = (graces(), unsafe { CSEventFlagMan::instance() }) else {
        return Vec::new();
    };
    graces
        .iter()
        .filter(|g| {
            g.space == space && distance_squared(g.position, position) < GRACE_RANGE * GRACE_RANGE
        })
        .filter(|g| flags.virtual_memory_flag.get_flag(g.event_flag))
        .map(|g| g.position)
        .collect()
}

fn graces() -> Option<&'static [Grace]> {
    if let Some(graces) = GRACES.get() {
        return Some(graces);
    }
    conversions()?;
    // SAFETY: the param repository is only read here, on a game task.
    let params = unsafe { SoloParamRepository::instance() }.ok()?;
    let graces: Vec<Grace> = (0..)
        .map_while(|i| params.get_row_by_index::<BonfireWarpParam>(i))
        .filter(|row| row.eventflag_id() != 0)
        .map(|row| {
            let block = BlockId::from_parts(row.area_no(), row.grid_x_no(), row.grid_z_no(), 0);
            let (position, space, _) = locate(block, [row.pos_x(), row.pos_z()]);
            Grace {
                event_flag: row.eventflag_id(),
                position,
                space,
            }
        })
        .collect();
    Some(GRACES.get_or_init(|| graces))
}

// Beacon storage in game version 1.17.1, from soarqin's EROverlay notes:
// CSMenuMan -> owner -> map view -> WorldMapMarkerDataList (+0x300) -> CSMenuMarkersSaveData (+0x38).
const MENU_MAP_OWNER: usize = 0x80;
const OWNER_MAP_VIEW: usize = 0x250;
const VIEW_MARKER_SAVE: usize = 0x300 + 0x38;
const SAVE_SLOTS: usize = 0x08;
const SAVE_CAPACITY: usize = 0x10;
/// The save has 10 slots; the game only allows 5 beacons at a time.
const MAX_SLOTS: usize = 10;

#[repr(C)]
#[derive(Clone, Copy)]
struct MarkerSlot {
    /// Negative for an empty slot.
    id: i32,
    /// Game world map coordinates, see `maptiles::from_game_map`.
    x: f32,
    y: f32,
    /// 0 for the overworld map, 1 for the underground map.
    map: u8,
    icon: u8,
    _pad: [u8; 2],
}

fn placed_beacons(map: MapId) -> Vec<Beacon> {
    let wanted_map = match map {
        MapId::Overworld => 0,
        MapId::Underground => 1,
    };
    // SAFETY: read on a game task, so the menu objects aren't being torn down meanwhile;
    // every pointer in the chain is checked before it's followed.
    unsafe {
        let Ok(menu) = CSMenuManImp::instance_ptr() else {
            return Vec::new();
        };
        let Some(save) = follow(
            menu as usize,
            &[MENU_MAP_OWNER, OWNER_MAP_VIEW, VIEW_MARKER_SAVE],
        ) else {
            return Vec::new();
        };
        let Some(slots) = follow(save, &[SAVE_SLOTS]) else {
            return Vec::new();
        };
        let capacity = ((save + SAVE_CAPACITY) as *const u32).read_unaligned() as usize;
        (0..capacity.min(MAX_SLOTS))
            .filter_map(|i| {
                let slot =
                    ((slots + i * size_of::<MarkerSlot>()) as *const MarkerSlot).read_unaligned();
                (slot.id >= 0 && slot.map == wanted_map && slot.x.is_finite() && slot.y.is_finite())
                    .then(|| Beacon {
                        // The game shows the slot number, not the id.
                        number: i as u8 + 1,
                        position: maptiles::from_game_map([slot.x, slot.y]),
                    })
            })
            .collect()
    }
}

/// Reads a pointer at each offset in turn, stopping at null.
unsafe fn follow(mut address: usize, offsets: &[usize]) -> Option<usize> {
    for offset in offsets {
        // SAFETY: the caller guarantees `address` points at a live object with a pointer here.
        address = unsafe { ((address + offset) as *const usize).read_unaligned() };
        if address == 0 {
            return None;
        }
    }
    Some(address)
}

/// Points further than this aren't drawn even at the widest zoom.
const POINT_RANGE: f32 = 300.0;

/// One WorldMapPointParam row, reduced to what deciding its visibility needs.
struct PointRow {
    position: [f32; 2],
    space: i32,
    on_overworld: bool,
    on_underground: bool,
    icon: u16,
    alt_icon: u16,
    open_flag: u32,
    cleared_flag: u32,
    /// Rows allowed without text skip the text condition.
    needs_text: bool,
    texts: [PointText; 8],
}

#[derive(Clone, Copy)]
struct PointText {
    id: i32,
    enable: [u32; 2],
    disable: [u32; 2],
    kind: u8,
}

static POINTS: OnceLock<Vec<PointRow>> = OnceLock::new();

/// Same rule as the game (per soarqin's EROverlay notes): the open flag must be set and an
/// ordinary point needs an active text line. Map mods hide looted items by disabling the text.
fn visible_points(map: MapId, position: [f32; 2], space: i32) -> Vec<MapPoint> {
    let (Some(rows), Ok(flags)) = (points(), unsafe { CSEventFlagMan::instance() }) else {
        return Vec::new();
    };
    let flags = &flags.virtual_memory_flag;
    let on = |flag: u32| flag != 0 && flags.get_flag(flag);
    rows.iter()
        .filter(|row| match map {
            MapId::Overworld => row.on_overworld,
            MapId::Underground => row.on_underground,
        })
        .filter(|row| {
            row.space == space
                && distance_squared(row.position, position) < POINT_RANGE * POINT_RANGE
        })
        .filter(|row| row.open_flag == 0 || on(row.open_flag))
        .filter_map(|row| {
            let active = |text: &&PointText| {
                text.id > 0
                    && text.enable.iter().all(|&flag| flag == 0 || on(flag))
                    && !text.disable.iter().any(|&flag| on(flag))
            };
            let mut active_texts = row.texts.iter().filter(active).peekable();
            if row.needs_text && active_texts.peek().is_none() {
                return None;
            }
            let icon = if active_texts.any(|text| text.kind == 1) && row.alt_icon != 0 {
                row.alt_icon
            } else {
                row.icon
            };
            Some(MapPoint {
                kind: point_kind(icon)?,
                position: row.position,
                cleared: row.cleared_flag != u32::MAX && on(row.cleared_flag),
            })
        })
        .collect()
}

/// Groups the game's map icon ids (names from Smithbox's WORLD_MAP_ICON_ID enum).
fn point_kind(icon: u16) -> Option<PointKind> {
    match icon {
        0 => None,
        1 | 2 => Some(PointKind::Grace),
        // Light beams guiding toward graces and the like, not places.
        83..=85 => None,
        80 | 86 => Some(PointKind::Npc),
        87 | 88 => Some(PointKind::Invasion),
        3..=99 => Some(PointKind::Landmark),
        499..=502 => Some(PointKind::Boss),
        _ => Some(PointKind::Item),
    }
}

fn points() -> Option<&'static [PointRow]> {
    if let Some(points) = POINTS.get() {
        return Some(points);
    }
    conversions()?;
    // SAFETY: the param repository is only read here, on a game task.
    let params = unsafe { SoloParamRepository::instance() }.ok()?;
    let points: Vec<PointRow> = (0..)
        .map_while(|i| params.get_row_by_index::<WorldMapPointParam>(i))
        // Area labels name whole regions rather than places.
        .filter(|row| !row.is_area_icon())
        .map(|row| {
            let block = BlockId::from_parts(row.area_no(), row.grid_x_no(), row.grid_z_no(), 0);
            let (position, space, _) = locate(block, [row.pos_x(), row.pos_z()]);
            // The second set of text flags is signed; non-positive means unused.
            let flag2 = |flag: i32| u32::try_from(flag).unwrap_or(0);
            let texts = [
                (
                    row.text_id1(),
                    row.text_enable_flag_id1(),
                    row.text_disable_flag_id1(),
                    row.text_enable_flag2_id1(),
                    row.text_disable_flag2_id1(),
                    row.text_type1(),
                ),
                (
                    row.text_id2(),
                    row.text_enable_flag_id2(),
                    row.text_disable_flag_id2(),
                    row.text_enable_flag2_id2(),
                    row.text_disable_flag2_id2(),
                    row.text_type2(),
                ),
                (
                    row.text_id3(),
                    row.text_enable_flag_id3(),
                    row.text_disable_flag_id3(),
                    row.text_enable_flag2_id3(),
                    row.text_disable_flag2_id3(),
                    row.text_type3(),
                ),
                (
                    row.text_id4(),
                    row.text_enable_flag_id4(),
                    row.text_disable_flag_id4(),
                    row.text_enable_flag2_id4(),
                    row.text_disable_flag2_id4(),
                    row.text_type4(),
                ),
                (
                    row.text_id5(),
                    row.text_enable_flag_id5(),
                    row.text_disable_flag_id5(),
                    row.text_enable_flag2_id5(),
                    row.text_disable_flag2_id5(),
                    row.text_type5(),
                ),
                (
                    row.text_id6(),
                    row.text_enable_flag_id6(),
                    row.text_disable_flag_id6(),
                    row.text_enable_flag2_id6(),
                    row.text_disable_flag2_id6(),
                    row.text_type6(),
                ),
                (
                    row.text_id7(),
                    row.text_enable_flag_id7(),
                    row.text_disable_flag_id7(),
                    row.text_enable_flag2_id7(),
                    row.text_disable_flag2_id7(),
                    row.text_type7(),
                ),
                (
                    row.text_id8(),
                    row.text_enable_flag_id8(),
                    row.text_disable_flag_id8(),
                    row.text_enable_flag2_id8(),
                    row.text_disable_flag2_id8(),
                    row.text_type8(),
                ),
            ]
            .map(|(id, enable, disable, enable2, disable2, kind)| PointText {
                id,
                enable: [enable, flag2(enable2)],
                disable: [disable, flag2(disable2)],
                kind,
            });
            PointRow {
                position,
                space,
                on_overworld: row.disp_mask00(),
                on_underground: row.disp_mask01(),
                icon: row.icon_id(),
                alt_icon: row.alt_icon_id(),
                open_flag: row.event_flag_id(),
                cleared_flag: row.cleared_event_flag_id(),
                needs_text: !row.is_enable_no_text(),
                texts,
            }
        })
        .collect();
    Some(POINTS.get_or_init(|| points))
}
