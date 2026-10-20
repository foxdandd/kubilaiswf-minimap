//! Map for Goblins DLL's map API (`sdk/mfg_map_api.h`, since 2.1.6): the markers it shows and
//! their icons, read from the mod instead of guessed from its param rows.

use std::io::Read;
use std::sync::OnceLock;

use flate2::read::ZlibDecoder;
use hudhook::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use hudhook::windows::core::{s, w};

const ABI: u32 = 1;
const OK: i32 = 0;

#[repr(C)]
pub struct Point {
    pub id: u64,
    /// Stable across Map for Goblins versions, unlike the world map's icon frames.
    pub icon_id: u32,
    /// Area, grid X and grid Z in the top three bytes, like a block id.
    pub raw_map_id: u32,
    /// Position in that block, the same as a WorldMapPointParam row's.
    pub x: f32,
    pub z: f32,
    pub draw_order: u32,
    reserved: u32,
}

#[repr(C)]
struct RawIcon {
    lossless2: *const u8,
    lossless2_size: u32,
    source_icon_id: u32,
    matrix: *const u8,
    matrix_size: u32,
    reserved: u32,
}

#[repr(C)]
struct Api {
    size: u32,
    abi_version: u32,
    point_size: u32,
    icon_size: u32,
    icon_count: u32,
    reserved: u32,
    view_points: unsafe extern "C" fn(u32, *mut *const Point, *mut u32) -> i32,
    get_icon: unsafe extern "C" fn(u32, *mut RawIcon) -> i32,
}

/// None while Map for Goblins DLL isn't loaded or predates the API. Looked up once, after the
/// game has loaded its DLLs.
fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<&'static Api>> = OnceLock::new();
    *API.get_or_init(|| {
        // SAFETY: the export is the header's MfgGetMapAPI, and the table it returns is static.
        unsafe {
            let module = GetModuleHandleW(w!("MapForGoblins.dll")).ok()?;
            let get = GetProcAddress(module, s!("MfgGetMapAPI"))?;
            let get: unsafe extern "C" fn(u32) -> *const Api = std::mem::transmute(get);
            let api = get(ABI).as_ref()?;
            (api.abi_version == ABI
                && api.point_size as usize == size_of::<Point>()
                && api.icon_size as usize == size_of::<RawIcon>())
            .then_some(api)
        }
    })
}

pub fn available() -> bool {
    api().is_some()
}

/// Calls `visit` with every marker Map for Goblins shows on a map layer (0 surface,
/// 1 underground, 2 DLC). Must run on a game task, as the API requires.
pub fn visit_points(layer: u32, mut visit: impl FnMut(&Point)) {
    let Some(api) = api() else {
        return;
    };
    let mut points = std::ptr::null();
    let mut count = 0;
    // SAFETY: the view stays valid until this thread calls view_points again.
    unsafe {
        if (api.view_points)(layer, &mut points, &mut count) != OK || points.is_null() {
            return;
        }
        std::slice::from_raw_parts(points, count as usize)
            .iter()
            .for_each(&mut visit);
    }
}

pub struct Icon {
    pub source_icon_id: u32,
    pub width: usize,
    pub height: usize,
    /// Straight RGBA.
    pub rgba: Vec<u8>,
}

/// Every icon the API offers, decoded. Icons it can't decode are left out.
pub fn icons() -> Vec<Icon> {
    let Some(api) = api() else {
        return Vec::new();
    };
    (0..api.icon_count)
        .filter_map(|index| {
            let mut raw = RawIcon {
                lossless2: std::ptr::null(),
                lossless2_size: 0,
                source_icon_id: 0,
                matrix: std::ptr::null(),
                matrix_size: 0,
                reserved: 0,
            };
            // SAFETY: the returned tag is immutable for as long as the module stays loaded.
            let body = unsafe {
                if (api.get_icon)(index, &mut raw) != OK || raw.lossless2.is_null() {
                    return None;
                }
                std::slice::from_raw_parts(raw.lossless2, raw.lossless2_size as usize)
            };
            decode_lossless2(body).map(|(width, height, rgba)| Icon {
                source_icon_id: raw.source_icon_id,
                width,
                height,
                rgba,
            })
        })
        .collect()
}

/// A SWF DefineBitsLossless2 body: character id, format, width, height, then zlib data, either
/// a palette of premultiplied RGBA with rows of indices padded to 4 bytes (format 3), or rows
/// of premultiplied ARGB (format 5).
fn decode_lossless2(body: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let format = *body.get(2)?;
    let width = usize::from(u16::from_le_bytes([*body.get(3)?, *body.get(4)?]));
    let height = usize::from(u16::from_le_bytes([*body.get(5)?, *body.get(6)?]));
    let (colours, data) = match format {
        3 => (usize::from(*body.get(7)?) + 1, body.get(8..)?),
        5 => (0, body.get(7..)?),
        _ => return None,
    };
    let mut raw = Vec::new();
    ZlibDecoder::new(data).read_to_end(&mut raw).ok()?;
    let mut rgba = Vec::with_capacity(width * height * 4);
    match format {
        3 => {
            let (table, indices) = raw.split_at_checked(colours * 4)?;
            let stride = (width + 3) & !3;
            for y in 0..height {
                for x in 0..width {
                    let i = usize::from(*indices.get(y * stride + x)?);
                    rgba.extend_from_slice(table.get(i * 4..i * 4 + 4)?);
                }
            }
        }
        _ => {
            for argb in raw.get(..width * height * 4)?.chunks_exact(4) {
                rgba.extend_from_slice(&[argb[1], argb[2], argb[3], argb[0]]);
            }
        }
    }
    for pixel in rgba.chunks_exact_mut(4) {
        let a = u16::from(pixel[3]);
        for c in &mut pixel[..3] {
            if let Some(straight) = (u16::from(*c) * 255 + a / 2).checked_div(a) {
                *c = straight.min(255) as u8;
            }
        }
    }
    Some((width, height, rgba))
}
