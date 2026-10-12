use std::io;

/// Bounds-checked little-endian reads over the offset tables FromSoftware's formats are made of.
pub struct Reader<'a>(pub &'a [u8]);

impl<'a> Reader<'a> {
    pub fn bytes(&self, at: usize, len: usize) -> io::Result<&'a [u8]> {
        self.0
            .get(at..at + len)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "read past end of data"))
    }

    pub fn u32(&self, at: usize) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()))
    }

    pub fn u32_be(&self, at: usize) -> io::Result<u32> {
        Ok(u32::from_be_bytes(self.bytes(at, 4)?.try_into().unwrap()))
    }

    pub fn u64(&self, at: usize) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(at, 8)?.try_into().unwrap()))
    }

    /// Reads a null-terminated UTF-16 string.
    pub fn utf16(&self, at: usize) -> io::Result<String> {
        let units: Vec<u16> = self.0[at.min(self.0.len())..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        String::from_utf16(&units).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}
