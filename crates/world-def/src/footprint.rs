//! The footprint query (WDEF-007, ADR 0066 point 2): which authored items can change the output of a tile.
//!
//! An item's `reach_m` is the arc distance on the body's reference sphere beyond which it changes no output. A tile's
//! footprint is a spherical cap around its centre direction that holds the tile and its halo. The item is returned when
//! the angle between its anchor and the cap's centre is at most the cap's angle plus `reach_m / radius`, with a fixed
//! margin, in f64 with `libm` (CON-14). A false positive changes a key and never an output; a false negative is a defect,
//! which `tests/footprint.rs` checks against a scan of every item.

use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{TileId, Vec3};

/// Candidate cells for a tile at or below the cell level are the tile's cell plus this many rings. The cap of such a tile
/// reaches at most 0.9 tile edges past the tile centre, its halo one tile edge and an item's reach one nominal cell edge, and a
/// cell is no narrower than 0.7 nominal edges (the level rounding) and a little less across the face warp; the margin is
/// checked against a scan of every item by `tests/footprint.rs`, not assumed.
pub const FOOTPRINT_MARGIN_RINGS: u32 = 5;

/// The limit of cells a covering may have (the cells of one tile at or below the cell level are one).
pub const FOOTPRINT_MAX_CELLS: usize = 1024;

/// Relative margin on the touch test (`2^-20`).
pub const FOOTPRINT_REL_MARGIN: f64 = 1.0 / 1_048_576.0;

/// Absolute margin on the touch test in radians.
pub const FOOTPRINT_ABS_MARGIN_RAD: f64 = 1e-12;

/// A halo whose extended face coordinate leaves this bound makes the cap the whole sphere: the warp is not usable far
/// beyond the face.
const MAX_EXTENDED_FACE_COORD: f64 = 1.5;

/// One item as a footprint query returns it: everything the result hash covers.
#[derive(Clone, Debug, PartialEq)]
pub struct FootprintItem {
    pub id: u64,
    pub priority: i32,
    pub content: u64,
    /// Bits of the unit direction of the anchor.
    pub dir: [u64; 3],
    /// Bits of `reach_m` and `max_delta_m`, `-0.0` read as `0.0`.
    pub reach_bits: u64,
    pub max_delta_bits: u64,
    pub layer: String,
}

/// A spherical cap: a unit centre direction and an angle in radians (`PI` is the whole sphere).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cap {
    pub centre: Vec3,
    pub angle: f64,
}

/// The angle in radians between two unit directions, stable at small angles (`atan2` of the cross and dot products).
pub fn angle_between(a: Vec3, b: Vec3) -> f64 {
    let cross = a.cross(b);
    libm::atan2(cross.length(), a.dot(b))
}

/// The cap around `tile` that holds the tile and a halo of `halo` tile edges on every side. `halo` is finite and in
/// `[0, 1]` (the caller checks). The tile centre and the corners and edge midpoints of the extended tile are taken on
/// `map`; the angle is the largest from the centre.
pub fn tile_cap(map: &dyn FaceMapping, tile: TileId, halo: f64) -> Cap {
    let (s0, s1, t0, t1) = tile.bounds();
    let (sm, tm) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
    let (hs, ht) = (halo * (s1 - s0), halo * (t1 - t0));
    let (es0, es1, et0, et1) = (s0 - hs, s1 + hs, t0 - ht, t1 + ht);
    let centre = face_to_direction(map, tile.face(), sm, tm);
    if [es0, es1, et0, et1].iter().any(|c| c.abs() > MAX_EXTENDED_FACE_COORD) {
        return Cap { centre, angle: core::f64::consts::PI };
    }
    let mut angle = 0.0f64;
    for (s, t) in [(es0, et0), (es1, et0), (es0, et1), (es1, et1), (sm, et0), (sm, et1), (es0, tm), (es1, tm)] {
        angle = angle.max(angle_between(centre, face_to_direction(map, tile.face(), s, t)));
    }
    Cap { centre, angle }
}

/// Whether an item at `anchor` with `reach_m` can touch `cap` on a body of `radius_m`. Margins only widen the test.
pub fn touches(cap: Cap, anchor: Vec3, reach_m: f64, radius_m: f64) -> bool {
    let limit = cap.angle + reach_m / radius_m;
    angle_between(cap.centre, anchor) <= limit * (1.0 + FOOTPRINT_REL_MARGIN) + FOOTPRINT_ABS_MARGIN_RAD
}
