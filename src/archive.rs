//! Reads files out of Elden Ring's encrypted `DataN.bhd`/`DataN.bdt` archives.
//! Formats follow SoulsFormats (BHD5) and UXM (keys, path hashes).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use aes::Aes128;
use aes::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPublicKey};

use crate::reader::Reader;

const ARCHIVES: [(&str, &str); 5] = [
    ("Data0", include_str!("keys/data0.pem")),
    ("Data1", include_str!("keys/data1.pem")),
    ("Data2", include_str!("keys/data2.pem")),
    ("Data3", include_str!("keys/data3.pem")),
    ("DLC", include_str!("keys/dlc.pem")),
];

/// One file inside an archive, read lazily from the `.bdt`.
pub struct ArchiveFile {
    bdt: File,
    entry: Entry,
}

struct Entry {
    offset: u64,
    size: usize,
    aes_key: [u8; 16],
    aes_ranges: Vec<(usize, usize)>,
}

/// Finds `path` by searching the game's archives in order. Each archive's header has
/// to be decrypted to be searched, so stopping at the first hit saves seconds.
pub fn open(game_dir: &Path, path: &str) -> io::Result<ArchiveFile> {
    let hash = path_hash(path);
    for (name, key) in ARCHIVES {
        let Ok(bhd) = std::fs::read(game_dir.join(format!("{name}.bhd"))) else {
            continue;
        };
        if let Some(entry) = find_in_bhd5(&rsa_decrypt(&bhd, key)?, hash)? {
            let bdt = File::open(game_dir.join(format!("{name}.bdt")))?;
            return Ok(ArchiveFile { bdt, entry });
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, path.to_owned()))
}

impl ArchiveFile {
    pub fn read_all(&mut self) -> io::Result<Vec<u8>> {
        let mut data = self.read_raw(0, self.entry.size)?;
        let cipher = Aes128::new(GenericArray::from_slice(&self.entry.aes_key));
        for &(start, end) in &self.entry.aes_ranges {
            for block in data[start..end].chunks_exact_mut(16) {
                cipher.decrypt_block(GenericArray::from_mut_slice(block));
            }
        }
        Ok(data)
    }

    /// Reads part of the file. Encrypted ranges have to be read whole with `read_all`.
    pub fn read_at(&mut self, offset: usize, len: usize) -> io::Result<Vec<u8>> {
        if self
            .entry
            .aes_ranges
            .iter()
            .any(|&(start, end)| start < offset + len && offset < end)
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "partial read of an encrypted range",
            ));
        }
        self.read_raw(offset, len)
    }

    fn read_raw(&mut self, offset: usize, len: usize) -> io::Result<Vec<u8>> {
        if offset + len > self.entry.size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "read past end of file",
            ));
        }
        let mut data = vec![0; len];
        self.bdt
            .seek(SeekFrom::Start(self.entry.offset + offset as u64))?;
        self.bdt.read_exact(&mut data)?;
        Ok(data)
    }
}

fn path_hash(path: &str) -> u64 {
    path.to_lowercase()
        .replace('\\', "/")
        .bytes()
        .fold(0u64, |hash, c| {
            hash.wrapping_mul(0x85).wrapping_add(u64::from(c))
        })
}

/// The headers are "decrypted" by raising each 256 byte block to the public exponent,
/// yielding 255 bytes of plaintext per block.
fn rsa_decrypt(data: &[u8], pem: &str) -> io::Result<Vec<u8>> {
    let key = RsaPublicKey::from_pkcs1_pem(pem).map_err(io::Error::other)?;
    let block_size = key.size();
    let mut out = Vec::with_capacity(data.len());
    for block in data.chunks(block_size) {
        let plain = BigUint::from_bytes_be(block)
            .modpow(key.e(), key.n())
            .to_bytes_be();
        out.resize(out.len() + block_size - 1 - plain.len(), 0);
        out.extend_from_slice(&plain);
    }
    Ok(out)
}

fn find_in_bhd5(data: &[u8], hash: u64) -> io::Result<Option<Entry>> {
    let r = Reader(data);
    if r.bytes(0, 4)? != b"BHD5" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a BHD5 header",
        ));
    }
    let bucket_count = r.u32(0x10)? as usize;
    let buckets_offset = r.u32(0x14)? as usize;
    for bucket in 0..bucket_count {
        let at = buckets_offset + bucket * 8;
        let headers = r.u32(at + 4)? as usize;
        for i in 0..r.u32(at)? as usize {
            let at = headers + i * 0x28;
            if r.u64(at)? != hash {
                continue;
            }
            let mut entry = Entry {
                offset: r.u64(at + 0x10)?,
                size: r.u32(at + 8)? as usize,
                aes_key: [0; 16],
                aes_ranges: Vec::new(),
            };
            let aes = r.u64(at + 0x20)? as usize;
            if aes != 0 {
                entry.aes_key = r.bytes(aes, 16)?.try_into().unwrap();
                for j in 0..r.u32(aes + 16)? as usize {
                    let range = aes + 20 + j * 16;
                    let (start, end) = (r.u64(range)? as i64, r.u64(range + 8)? as i64);
                    // Unused range slots are (-1, -1).
                    if start >= 0 && end > start {
                        entry.aes_ranges.push((start as usize, end as usize));
                    }
                }
            }
            return Ok(Some(entry));
        }
    }
    Ok(None)
}
