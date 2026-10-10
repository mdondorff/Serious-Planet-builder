//! 64-bit tile identifiers and quadtree operations (ADR 0003).
//!
//! Layout: bits 63..61 face, bits 60..56 level (0..=28), low `2·level` bits the Morton path (at each level
//! the pair `(x bit, y bit)`, top level most significant), all other bits zero.

use crate::cube::{direction_to_face, face_to_direction, Face, FaceMapping};
use crate::vec3::Vec3;

pub const MAX_LEVEL: u8 = 28;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileId(u64);

/// Edge of a tile, in face coordinates: East = +s, West = -s, North = +t, South = -t.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    East,
    West,
    North,
    South,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::East, Side::West, Side::North, Side::South];
}

fn spread(v: u32) -> u64 {
    let mut x = u64::from(v);
    x = (x | (x << 16)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x << 8)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333_3333_3333;
    (x | (x << 1)) & 0x5555_5555_5555_5555
}

fn compact(mut x: u64) -> u32 {
    x &= 0x5555_5555_5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333_3333_3333;
    x = (x | (x >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x >> 4)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x >> 8)) & 0x0000_FFFF_0000_FFFF;
    ((x | (x >> 16)) & 0xFFFF_FFFF) as u32
}

impl TileId {
    /// `None` when the face, level or indices are out of range.
    pub fn new(face: Face, level: u8, x: u32, y: u32) -> Option<TileId> {
        if face.0 >= 6 || level > MAX_LEVEL {
            return None;
        }
        let n = 1u64 << level;
        if u64::from(x) >= n || u64::from(y) >= n {
            return None;
        }
        let path = (spread(x) << 1) | spread(y);
        Some(TileId((u64::from(face.0) << 61) | (u64::from(level) << 56) | path))
    }

    /// Validate a raw value: face < 6, level <= 28, and no bits set above the path.
    pub fn from_raw(raw: u64) -> Option<TileId> {
        let (face, level) = ((raw >> 61) as u8, ((raw >> 56) & 31) as u8);
        if face >= 6 || level > MAX_LEVEL {
            return None;
        }
        let path = raw & ((1u64 << 56) - 1);
        (path >> (2 * u32::from(level)) == 0).then_some(TileId(raw))
    }

    pub fn raw(self) -> u64 {
        self.0
    }
    pub fn face(self) -> Face {
        Face((self.0 >> 61) as u8)
    }
    pub fn level(self) -> u8 {
        ((self.0 >> 56) & 31) as u8
    }
    fn path(self) -> u64 {
        self.0 & ((1u64 << 56) - 1)
    }
    pub fn x(self) -> u32 {
        compact(self.path() >> 1)
    }
    pub fn y(self) -> u32 {
        compact(self.path())
    }

    pub fn parent(self) -> Option<TileId> {
        let level = self.level();
        (level > 0).then(|| TileId::new(self.face(), level - 1, self.x() >> 1, self.y() >> 1).expect("parent is valid"))
    }

    /// Children in Morton order: (x0,y0), (x0,y1), (x1,y0), (x1,y1). `None` at the maximum level.
    pub fn children(self) -> Option<[TileId; 4]> {
        let level = self.level();
        (level < MAX_LEVEL).then(|| {
            let (x, y) = (self.x() * 2, self.y() * 2);
            [(0, 0), (0, 1), (1, 0), (1, 1)].map(|(dx, dy)| TileId::new(self.face(), level + 1, x + dx, y + dy).expect("child is valid"))
        })
    }

    /// Tile at `level` containing the direction.
    pub fn from_direction(map: &dyn FaceMapping, dir: Vec3, level: u8) -> TileId {
        let (face, s, t) = direction_to_face(map, dir);
        assert!(level <= MAX_LEVEL, "level {level} exceeds the maximum {MAX_LEVEL}");
        let n = (1u64 << level) as f64;
        // floor(c·n/2) is exact (power-of-two scaling); adding the integer n/2 afterwards avoids the rounding of c + 1.
        let idx = |c: f64| -> u32 {
            if level == 0 {
                return 0;
            }
            let i = (c * 0.5 * n).floor() as i64 + (1i64 << (level - 1));
            i.clamp(0, (1i64 << level) - 1) as u32
        };
        TileId::new(face, level, idx(s), idx(t)).expect("valid level")
    }

    pub fn contains(self, map: &dyn FaceMapping, dir: Vec3) -> bool {
        TileId::from_direction(map, dir, self.level()) == self
    }

    /// Face-coordinate bounds `(s0, s1, t0, t1)`; exact in f64 (dyadic rationals).
    pub fn bounds(self) -> (f64, f64, f64, f64) {
        let n = (1u64 << self.level()) as f64;
        let c = |i: u32| 2.0 * f64::from(i) / n - 1.0;
        (c(self.x()), c(self.x() + 1), c(self.y()), c(self.y() + 1))
    }

    pub fn center_direction(self, map: &dyn FaceMapping) -> Vec3 {
        let (s0, s1, t0, t1) = self.bounds();
        face_to_direction(map, self.face(), 0.5 * (s0 + s1), 0.5 * (t0 + t1))
    }

    /// Direction of sample `(k, l)` on a `res`×`res` cell grid over the tile (`0..=res` inclusive; `res` a power
    /// of two). Computed from the face lattice index, so equal border samples of neighbouring tiles, also across
    /// faces, give bit-identical directions (COORD-007).
    pub fn sample_direction(self, map: &dyn FaceMapping, res: u32, k: u32, l: u32) -> Vec3 {
        assert!(k <= res && l <= res, "sample indices must be within the tile");
        self.sample_direction_ext(map, res, i64::from(k), i64::from(l))
    }

    /// Like `sample_direction` but allows indices from -1 to `res + 1` (a one-sample halo, used for normals). Beyond a face
    /// edge the point continues on the extended cube plane.
    pub fn sample_direction_ext(self, map: &dyn FaceMapping, res: u32, k: i64, l: i64) -> Vec3 {
        assert!(res.is_power_of_two(), "sample grid must be a power of two");
        assert!((-1..=i64::from(res) + 1).contains(&k) && (-1..=i64::from(res) + 1).contains(&l), "halo of one sample only");
        assert!(
            u32::from(self.level()) + res.trailing_zeros() <= 52,
            "level + log2(res) must be <= 52 so lattice indices stay exact in f64"
        );
        let n = (1u64 << self.level()) as f64 * f64::from(res);
        let lat = |tile_index: u32, i: i64| 2.0 * ((i64::from(tile_index) * i64::from(res) + i) as f64) / n - 1.0;
        face_to_direction(map, self.face(), lat(self.x(), k), lat(self.y(), l))
    }

    /// Neighbour across `side` at the same level; crosses face edges transparently. Computed by stepping just
    /// past the middle of the edge and locating the containing tile, so the result is the unique edge-adjacent tile.
    pub fn neighbor(self, map: &dyn FaceMapping, side: Side) -> TileId {
        let n = 1u32 << self.level();
        let (x, y) = (self.x(), self.y());
        let inside = match side {
            Side::East => x + 1 < n,
            Side::West => x > 0,
            Side::North => y + 1 < n,
            Side::South => y > 0,
        };
        if inside {
            let (nx, ny) = match side {
                Side::East => (x + 1, y),
                Side::West => (x - 1, y),
                Side::North => (x, y + 1),
                Side::South => (x, y - 1),
            };
            return TileId::new(self.face(), self.level(), nx, ny).expect("in range");
        }
        let (s0, s1, t0, t1) = self.bounds();
        let d = (s1 - s0) * 1e-3;
        let (sm, tm) = (0.5 * (s0 + s1), 0.5 * (t0 + t1));
        let (s, t) = match side {
            Side::East => (s1 + d, tm),
            Side::West => (s0 - d, tm),
            Side::North => (sm, t1 + d),
            Side::South => (sm, t0 - d),
        };
        TileId::from_direction(map, face_to_direction(map, self.face(), s, t), self.level())
    }
}
