//! The container formats between an archive file and a texture: BXF4 binders, DCX
//! compression and TPF texture packs. Layouts follow SoulsFormats.

use std::io;

use crate::reader::{Reader, invalid};

pub struct BinderEntry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

/// Lists the files of a BND4 binder, or of a split binder from its `.*bhd` header. Both share
/// one header and entry table; entry offsets point into the BND4 itself or into the `.*bdt`.
pub fn read_binder(data: &[u8]) -> io::Result<Vec<BinderEntry>> {
    let r = Reader(data);
    if !matches!(r.bytes(0, 4)?, b"BHF4" | b"BND4") {
        return Err(invalid("not a BHF4/BND4 binder"));
    }
    // Entry layout depends on the binder's format flags; the map tiles use IDs, names and
    // compression with 32-bit offsets, the only layout read here.
    if r.u64(0x20)? != 0x24 {
        return Err(invalid("unsupported BHF4 entry layout"));
    }
    (0..r.u32(0xC)? as usize)
        .map(|i| {
            let at = 0x40 + i * 0x24;
            Ok(BinderEntry {
                size: r.u64(at + 0x8)? as usize,
                offset: r.u32(at + 0x18)? as usize,
                name: r.utf16(r.u32(at + 0x20)? as usize)?,
            })
        })
        .collect()
}

type OodleDecompress = unsafe extern "system" fn(
    compressed: *const u8,
    compressed_len: i64,
    raw: *mut u8,
    raw_len: i64,
    fuzz_safe: i32,
    check_crc: i32,
    verbosity: i32,
    decode_buffer: *mut u8,
    decode_buffer_len: i64,
    callback: *const (),
    callback_data: *const (),
    decoder_memory: *mut u8,
    decoder_memory_len: i64,
    thread_phase: i32,
) -> i64;

/// Kraken decompression through the game's own copy of Oodle.
pub struct Oodle {
    _library: libloading::Library,
    decompress: OodleDecompress,
}

impl Oodle {
    pub fn load(path: &std::path::Path) -> io::Result<Self> {
        // SAFETY: oo2core is the game's own library and OodleLZ_Decompress has this signature.
        unsafe {
            let library = libloading::Library::new(path).map_err(io::Error::other)?;
            let decompress = *library
                .get::<OodleDecompress>(b"OodleLZ_Decompress\0")
                .map_err(io::Error::other)?;
            Ok(Self {
                _library: library,
                decompress,
            })
        }
    }

    pub fn decompress_dcx(&self, data: &[u8]) -> io::Result<Vec<u8>> {
        let r = Reader(data);
        if r.bytes(0, 4)? != b"DCX\0" || r.bytes(0x28, 4)? != b"KRAK" {
            return Err(invalid("not a DCX_KRAK file"));
        }
        let raw_len = r.u32_be(0x1C)? as usize;
        let compressed = r.bytes(0x4C, r.u32_be(0x20)? as usize)?;
        let mut raw = vec![0u8; raw_len];
        const FUZZ_SAFE_YES: i32 = 1;
        const DECODE_UNTHREADED: i32 = 3;
        // SAFETY: both buffers are valid for the lengths passed.
        let written = unsafe {
            (self.decompress)(
                compressed.as_ptr(),
                compressed.len() as i64,
                raw.as_mut_ptr(),
                raw_len as i64,
                FUZZ_SAFE_YES,
                0,
                0,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                0,
                DECODE_UNTHREADED,
            )
        };
        if written as usize != raw_len {
            return Err(invalid("Oodle decompression failed"));
        }
        Ok(raw)
    }
}

/// Returns the DDS file of the first texture in a PC TPF.
pub fn first_tpf_texture(data: &[u8]) -> io::Result<&[u8]> {
    let r = Reader(data);
    if r.bytes(0, 4)? != b"TPF\0" || r.u32(0x8)? == 0 {
        return Err(invalid("not a TPF with textures"));
    }
    r.bytes(r.u32(0x10)? as usize, r.u32(0x14)? as usize)
}
