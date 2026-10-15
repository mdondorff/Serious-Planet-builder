//! CRC-32C (Castagnoli) for payload validation (ADR 0019). Implemented in-tree: tables over the reflected polynomial
//! `0x82F63B78` (slicing-by-8: eight bytes per step, the same value as the bytewise definition, which the tests check), no
//! unsafe code, no dependency and no CPU feature to detect.

const fn tables() -> [[u32; 256]; 8] {
    let mut t = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0x82F6_3B78 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[0][i] = c;
        i += 1;
    }
    let mut s = 1;
    while s < 8 {
        let mut i = 0;
        while i < 256 {
            let prev = t[s - 1][i];
            t[s][i] = (prev >> 8) ^ t[0][(prev & 0xff) as usize];
            i += 1;
        }
        s += 1;
    }
    t
}

static TABLES: [[u32; 256]; 8] = tables();

/// The CRC-32C of `data`.
pub fn crc32c(data: &[u8]) -> u32 {
    update(0, data)
}

/// Continue a CRC-32C over more data: `update(crc32c(a), b) == crc32c(a ++ b)`.
pub fn update(crc: u32, data: &[u8]) -> u32 {
    let t = &TABLES;
    let mut c = !crc;
    let (chunks, rest) = data.as_chunks::<8>();
    for chunk in chunks {
        let lo = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) ^ c;
        let hi = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        c = t[7][(lo & 0xff) as usize]
            ^ t[6][((lo >> 8) & 0xff) as usize]
            ^ t[5][((lo >> 16) & 0xff) as usize]
            ^ t[4][(lo >> 24) as usize]
            ^ t[3][(hi & 0xff) as usize]
            ^ t[2][((hi >> 8) & 0xff) as usize]
            ^ t[1][((hi >> 16) & 0xff) as usize]
            ^ t[0][(hi >> 24) as usize];
    }
    for b in rest {
        c = t[0][((c ^ u32::from(*b)) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}
