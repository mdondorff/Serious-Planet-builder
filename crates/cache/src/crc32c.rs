//! CRC-32C (Castagnoli) for payload validation (ADR 0019). Implemented in-tree: a 256-entry table over the reflected
//! polynomial `0x82F63B78`, no unsafe code and no dependency.

const fn table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0x82F6_3B78 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

static TABLE: [u32; 256] = table();

/// The CRC-32C of `data`.
pub fn crc32c(data: &[u8]) -> u32 {
    update(0, data)
}

/// Continue a CRC-32C over more data: `update(crc32c(a), b) == crc32c(a ++ b)`.
pub fn update(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for b in data {
        c = TABLE[((c ^ u32::from(*b)) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}
