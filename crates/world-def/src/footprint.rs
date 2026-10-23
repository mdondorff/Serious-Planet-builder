//! The footprint query (WDEF-007, ADR 0066 point 2): which authored items can change the output of a tile.
//!
//! An item's `reach_m` is the arc distance on the body's reference sphere beyond which it changes no output. A tile's
//! footprint is a spherical cap around its centre direction that holds the tile and its halo. The item is returned when
//! the angle between its anchor and the cap's centre is at most the cap's angle plus `reach_m / radius`, with a fixed
//! margin, in f64 with `libm` (CON-14). A false positive changes a key and never an output; a false negative is a defect,
//! which `tests/footprint.rs` checks against a scan of every item.

use planet_core::cube::{face_to_direction, FaceMapping};
use planet_core::{TileId, Vec3};

/// The most rings of cells around a tile's cell the index path will look through; a tile that needs more scans the body.
pub const FOOTPRINT_MAX_RINGS: u32 = 16;

/// Rings of cells around the cell of a tile at or below the cell level that hold every item the touch test can accept: the
/// cap's arc plus the longest reach, over the narrowest cell edge, plus one. A cell is no narrower than half the nominal
/// width of its level (the level rounding, a factor of `1/sqrt 2`, and the face warp at a face edge, a factor of about `0.71`).
/// `None` when that is more than [`FOOTPRINT_MAX_RINGS`], so the caller scans the body. The result of the index path is then
/// the set the touch test accepts, the same as the scan's.
pub fn candidate_rings(cap: Cap, radius_m: f64, cell_level: u8) -> Option<u32> {
    let nominal_edge_m = core::f64::consts::FRAC_PI_2 * radius_m / f64::from(1u32 << cell_level);
    let narrowest_edge_m = 0.5 * nominal_edge_m.min(crate::cells::CELL_EDGE_M * 1.5);
    let reach_m = cap.angle * radius_m + crate::cells::CELL_EDGE_M;
    let rings = libm::ceil(reach_m / narrowest_edge_m) + 1.0;
    (rings.is_finite() && rings <= f64::from(FOOTPRINT_MAX_RINGS)).then_some(rings as u32)
}

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
    pub id: crate::item_id::ItemId,
    pub priority: i32,
    pub content: crate::item_id::ItemHash,
    /// Bits of the unit direction of the anchor.
    pub dir: [u64; 3],
    /// Bits of `reach_m` and `max_delta_m`, `-0.0` read as `0.0`.
    pub reach_bits: u64,
    pub max_delta_bits: u64,
    pub layer: String,
    /// The stamp the item is, if it is one (WDEF-010). Its content hash is derived from it, so it is in the item hash through `content`.
    pub stamp: Option<crate::stamp::Stamp>,
}

/// A spherical cap: a unit centre direction and an angle in radians (`PI` is the whole sphere).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cap {
    pub centre: Vec3,
    pub angle: f64,
}

/// The angle in radians between two unit directions, stable at small angles (`atan2` of the cross and dot products).
pub fn angle_between(a: Vec3, b: Vec3) -> f64 {
    a.angle_to(b)
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

/// The 16-byte id of an item, big-endian, so the byte order is the numeric order (an id below `2^64` is zero-extended, as it was when
/// ids were 64 bits).
pub fn id_bytes(id: crate::item_id::ItemId) -> [u8; 16] {
    id.to_be_bytes()
}

/// 128-bit hash of one item as the footprint and summary reads see it (`planet-builder footprint item v1`): the layer, the
/// priority, the content hash, the exact bits of the anchor, the reach and the height bound. The anchor is inside it, so a
/// result needs no separate direction. (The item file's own content hash, E2, keeps `planet-builder item hash v1`.)
pub fn item_hash128(item: &FootprintItem) -> [u8; 16] {
    let mut m = Vec::with_capacity(96);
    m.extend_from_slice(&u32::try_from(item.layer.len()).expect("a layer name under 4 GiB").to_le_bytes());
    m.extend_from_slice(item.layer.as_bytes());
    m.extend_from_slice(&item.priority.to_le_bytes());
    // The content hash as 8 bytes when it fits (the encoding from before it was 128 bits) and as 16 bytes otherwise: the input is
    // then 8 bytes longer, so the two never give the same material.
    match u64::try_from(item.content) {
        Ok(narrow) => m.extend_from_slice(&narrow.to_le_bytes()),
        Err(_) => m.extend_from_slice(&item.content.to_le_bytes()),
    }
    for d in item.dir {
        m.extend_from_slice(&d.to_le_bytes());
    }
    m.extend_from_slice(&item.reach_bits.to_le_bytes());
    m.extend_from_slice(&item.max_delta_bits.to_le_bytes());
    let full = blake3::derive_key("planet-builder footprint item v1", &m);
    full[..16].try_into().expect("16 of 32 bytes")
}

/// The angle in radians between the unit vector `p` and the great-circle arc `a..b` (unit vectors): the distance to the arc where the foot
/// of `p` on its great circle lies on it, else to the nearer end. `libm` for the transcendental, f64 throughout (CON-14).
pub fn arc_angle(a: Vec3, b: Vec3, p: Vec3) -> f64 {
    let ends = || angle_between(p, a).min(angle_between(p, b));
    // The normal of the arc's great circle as `(a + b) x (b - a) / 2` (the same vector as `a x b`): `b - a` is exact, so the error is
    // relative to the arc's length, not to 1e-16 of a unit, and a one-centimetre arc keeps its great circle to a nanometre.
    let c = (a + b).cross(b - a) * 0.5;
    let cl = c.length();
    if cl.is_nan() || cl <= 1e-12 {
        return ends();
    }
    let n = c * (1.0 / cl);
    let s = p.dot(n);
    let f = p - n * s;
    let fl = f.length();
    if fl.is_nan() || fl <= 1e-12 {
        return ends();
    }
    let f = f * (1.0 / fl);
    if a.cross(f).dot(n) >= 0.0 && f.cross(b).dot(n) >= 0.0 {
        libm::atan2(s.abs(), fl)
    } else {
        ends()
    }
}

/// The unit vector of the bits of a stored direction.
pub fn point_of(bits: &[u64; 3]) -> Vec3 {
    Vec3::new(f64::from_bits(bits[0]), f64::from_bits(bits[1]), f64::from_bits(bits[2]))
}

/// The angle in radians between the unit vector `p` and the polyline through `points` (bits of unit vectors, at least two): the minimum
/// over its arcs (GEN-028).
pub fn polyline_angle(points: &[[u64; 3]], p: Vec3) -> f64 {
    points.windows(2).map(|w| arc_angle(point_of(&w[0]), point_of(&w[1]), p)).fold(f64::INFINITY, f64::min)
}

/// The angle in radians between the unit vector `p` and a brush stroke's polyline (bits of unit vectors, at least one): the angle to the point
/// for a one-point stroke (a dab), else the minimum over its arcs.
pub fn stroke_angle(points: &[[u64; 3]], p: Vec3) -> f64 {
    match points {
        [only] => angle_between(point_of(only), p),
        _ => polyline_angle(points, p),
    }
}

/// Whether a brush's `strokes` can change an output inside `cap` on a body of `radius_m` (WDEF-007, GEN-030): a stroke changes a point only
/// within its radius of its polyline, and the distance to a polyline changes by at most the distance moved, so a stroke touches the cap when
/// its polyline is within the stroke's radius plus the cap's angle of the cap's centre. The stroke's radius, not the item's reach, bounds it:
/// the reach is a disc round the first point that holds the whole stroke, several times the stroke's own area. Margins only widen the test.
pub fn strokes_touch(cap: Cap, strokes: &[crate::stamp::StrokeBits], radius_m: f64) -> bool {
    strokes.iter().any(|s| {
        let limit = cap.angle + s.radius_m() / radius_m;
        stroke_angle(s.points_bits(), cap.centre) <= limit * (1.0 + FOOTPRINT_REL_MARGIN) + FOOTPRINT_ABS_MARGIN_RAD
    })
}
