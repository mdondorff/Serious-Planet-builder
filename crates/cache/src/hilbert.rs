//! The Hilbert-ordered sort key of the tile store (ADR 0019, ADR 0003).
//!
//! Index entries sort by `(kind, face, level, Hilbert index of (x, y) at that level)`. The standard 2D Hilbert curve
//! keeps a tile's four children together (the children of a cell take the four consecutive indices `4d .. 4d + 3`) and
//! puts siblings and neighbours near each other, so a streaming consumer asks for runs. The 64-bit `TileId` (Morton
//! path) stays the in-memory identity; this key is derived from it by a pure function.

use planet_core::TileId;

/// The sort key of a stored record: `kind`, `face`, `level` and the Hilbert index, 11 bytes, compared bytewise.
pub type SortKey = [u8; 11];

/// Hilbert index of cell `(x, y)` on the `2^bits x 2^bits` grid (`bits <= 31`).
pub fn xy_to_d(bits: u8, x: u32, y: u32) -> u64 {
    assert!(bits <= 31, "a Hilbert grid of 2^{bits} cells per side is too large");
    let n = 1u64 << bits;
    assert!(u64::from(x) < n && u64::from(y) < n, "cell ({x}, {y}) is outside the 2^{bits} grid");
    let (mut x, mut y) = (u64::from(x), u64::from(y));
    let mut d = 0u64;
    let mut s = n >> 1;
    while s > 0 {
        let rx = u64::from(x & s > 0);
        let ry = u64::from(y & s > 0);
        d += s * s * ((3 * rx) ^ ry);
        // Rotate the quadrant so the curve continues in the same orientation.
        if ry == 0 {
            if rx == 1 {
                x = n - 1 - x;
                y = n - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s >>= 1;
    }
    d
}

/// The cell `(x, y)` at Hilbert index `d` on the `2^bits x 2^bits` grid: the inverse of [`xy_to_d`].
pub fn d_to_xy(bits: u8, d: u64) -> (u32, u32) {
    assert!(bits <= 31);
    let n = 1u64 << bits;
    assert!(d < n * n, "index {d} is outside the 2^{bits} grid");
    let (mut x, mut y) = (0u64, 0u64);
    let mut t = d;
    let mut s = 1u64;
    while s < n {
        let rx = 1 & (t / 2);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        t /= 4;
        s <<= 1;
    }
    (x as u32, y as u32)
}

/// The 11-byte sort key of a record of `kind` for `tile`: `kind | face | level | Hilbert index (u64 big endian)`.
pub fn sort_key(kind: u8, tile: TileId) -> SortKey {
    let d = xy_to_d(tile.level(), tile.x(), tile.y());
    let mut k = [0u8; 11];
    k[0] = kind;
    k[1] = tile.face().0;
    k[2] = tile.level();
    k[3..].copy_from_slice(&d.to_be_bytes());
    k
}
