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

    /// Like `sample_direction` but allows indices from -1 to `res + 1` (a one-sample halo). Beyond a face
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

/// One of the four corners of a tile in face coordinates `(s, t)`: `(s0, t0)`, `(s1, t0)`, `(s0, t1)`, `(s1, t1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    SouthWest,
    SouthEast,
    NorthWest,
    NorthEast,
}

impl Corner {
    pub const ALL: [Corner; 4] = [Corner::SouthWest, Corner::SouthEast, Corner::NorthWest, Corner::NorthEast];

    /// Lattice offsets `(k, l)` (0 or 1) of the corner within the tile.
    fn offsets(self) -> (u32, u32) {
        match self {
            Corner::SouthWest => (0, 0),
            Corner::SouthEast => (1, 0),
            Corner::NorthWest => (0, 1),
            Corner::NorthEast => (1, 1),
        }
    }
}

impl TileId {
    /// Direction of one corner of this tile, bit-identical to the same corner point seen from any tile sharing it.
    pub fn corner_direction(self, map: &dyn FaceMapping, corner: Corner) -> Vec3 {
        let (k, l) = corner.offsets();
        self.sample_direction(map, 1, k, l)
    }

    /// The other tiles of the same level that share `corner` with this tile: the two edge neighbours and the diagonal
    /// one (three tiles) at an ordinary corner, and the two edge neighbours (two tiles, no diagonal) where the corner is
    /// one of the eight cube corners. Sorted by id.
    pub fn corner_neighbors(self, map: &dyn FaceMapping, corner: Corner) -> Vec<TileId> {
        let c = self.corner_direction(map, corner);
        // A tangent frame at the corner point, and probe directions all around it, much closer than a tile.
        let axis = if c.x.abs() <= c.y.abs() && c.x.abs() <= c.z.abs() {
            Vec3::new(1.0, 0.0, 0.0)
        } else if c.y.abs() <= c.z.abs() {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let t1 = (axis - c * axis.dot(c)).normalized();
        let t2 = c.cross(t1);
        let delta = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << self.level()) * 1e-3;
        let mut found: Vec<TileId> = Vec::new();
        for k in 0..8 {
            let a = f64::from(k) * core::f64::consts::FRAC_PI_4;
            let probe = (c + (t1 * libm::cos(a) + t2 * libm::sin(a)) * delta).normalized();
            let t = TileId::from_direction(map, probe, self.level());
            if t != self && !found.contains(&t) {
                found.push(t);
            }
        }
        found.sort();
        found
    }
}

// ---- The lattice normal stencil (ADR 0073) ----

fn int_axes(face: Face) -> ([i64; 3], [i64; 3], [i64; 3]) {
    let r = |v: Vec3| [v.x as i64, v.y as i64, v.z as i64];
    (r(face.normal()), r(face.u_axis()), r(face.v_axis()))
}

/// The faces holding the cube point `p` (in units of half a cell, `n` cells per face edge: a face holds it when its normal coordinate
/// is `n`), smallest first, with the lattice indices of the point on each.
fn faces_of_point(p: [i64; 3], n: i64) -> Vec<(Face, i64, i64)> {
    let mut out = Vec::with_capacity(3);
    for g in Face::ALL {
        let (gn, gu, gv) = int_axes(g);
        let dot = |a: [i64; 3]| p[0] * a[0] + p[1] * a[1] + p[2] * a[2];
        if dot(gn) == n {
            out.push((g, (dot(gu) + n) / 2, (dot(gv) + n) / 2));
        }
    }
    out
}

/// The exact integer cube point (half-cell units) of the lattice point `(i, j)` of `face`, for one coordinate at most one step outside
/// `0..=n` (the other inside): the point itself inside the face; beyond a face edge the point on the shared edge moved one cell back
/// against the face's normal, which is the neighbour face's own lattice point one cell inside it, on the row that continues the shared
/// edge (never found by projecting the extended plane back).
fn lattice_cube_point(face: Face, n: i64, i: i64, j: i64) -> [i64; 3] {
    let inside = |a: i64| (0..=n).contains(&a);
    assert!(inside(i) || inside(j), "a ring point beyond a face edge has the other coordinate inside the face");
    assert!((-1..=n + 1).contains(&i) && (-1..=n + 1).contains(&j), "ring of one sample only");
    let (nrm, u, v) = int_axes(face);
    let (ci, cj) = (i.clamp(0, n), j.clamp(0, n));
    let back = if inside(i) && inside(j) { 0 } else { 2 };
    [0, 1, 2].map(|k| (n - back) * nrm[k] + (2 * ci - n) * u[k] + (2 * cj - n) * v[k])
}

/// The cell of the neighbour face that the extended cell `(i, j)` of `face` is (`n` cells per face edge, exactly one coordinate one step
/// outside `0..n`): the cube point of its centre in half-cell units, folded over the shared edge (the cell lies `e` half-cells beyond the
/// edge on the extended plane, so on the neighbour face it lies `e` below the edge), read on the neighbour face. Integers only.
fn fold_cell(face: Face, n: i64, i: i64, j: i64) -> (Face, i64, i64) {
    let (nrm, u, v) = int_axes(face);
    let (a, b) = (2 * i + 1 - n, 2 * j + 1 - n);
    let beyond = |c: i64| c.abs() - n;
    let p: [i64; 3] = if a.abs() > n {
        let (s, e) = (a.signum(), beyond(a));
        [0, 1, 2].map(|k| (n - e) * nrm[k] + s * n * u[k] + b * v[k])
    } else {
        let (s, e) = (b.signum(), beyond(b));
        [0, 1, 2].map(|k| (n - e) * nrm[k] + a * u[k] + s * n * v[k])
    };
    for g in Face::ALL {
        let (gn, gu, gv) = int_axes(g);
        let dot = |w: [i64; 3]| p[0] * w[0] + p[1] * w[1] + p[2] * w[2];
        if dot(gn) == n {
            // The centre's coordinates along the face axes are `2 * index + 1 - n`.
            return (g, (dot(gu) + n - 1) / 2, (dot(gv) + n - 1) / 2);
        }
    }
    unreachable!("a folded cell centre lies on a face")
}

impl TileId {
    /// The tiles of the same level that share an edge or a corner with this one, across cube faces, computed with integers only (no
    /// projection): eight inside a face, and at a cube corner seven, because the diagonal there does not exist. Unsorted.
    pub fn neighbors8(self) -> Vec<TileId> {
        let n = 1i64 << self.level();
        let (x, y) = (i64::from(self.x()), i64::from(self.y()));
        let mut out = Vec::with_capacity(8);
        for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (i, j) = (x + dx, y + dy);
            let (in_i, in_j) = ((0..n).contains(&i), (0..n).contains(&j));
            let cell = match (in_i, in_j) {
                (true, true) => (self.face(), i, j),
                (false, false) => continue,
                _ => fold_cell(self.face(), n, i, j),
            };
            out.push(TileId::new(cell.0, self.level(), cell.1 as u32, cell.2 as u32).expect("a cell of the face"));
        }
        out
    }

    /// Direction of the lattice point `(k, l)` of the tile's `res`-cell grid for `k`, `l` from -1 to `res + 1`, one at most outside
    /// `0..=res` (ADR 0073): the point itself inside the tile and across a tile edge inside a face (the same lattice integers), and
    /// beyond a face edge the neighbour tile's own lattice direction one cell inside it. On a cube edge both faces hold the point and
    /// the smaller face is used. Bit-identical to the direction any other tile reads for the same point (COORD-007).
    pub fn ring_direction(self, map: &dyn FaceMapping, res: u32, k: i64, l: i64) -> Vec3 {
        assert!(res.is_power_of_two(), "sample grid must be a power of two");
        assert!(
            u32::from(self.level()) + res.trailing_zeros() <= 52,
            "level + log2(res) must be <= 52 so lattice indices stay exact in f64"
        );
        let n = (1i64 << self.level()) * i64::from(res);
        let (gi, gj) = (i64::from(self.x()) * i64::from(res) + k, i64::from(self.y()) * i64::from(res) + l);
        if (0..=n).contains(&gi) && (0..=n).contains(&gj) {
            // Inside the face: the tile's own halo expression.
            return self.sample_direction_ext(map, res, k, l);
        }
        let p = lattice_cube_point(self.face(), n, gi, gj);
        let (face, i, j) = faces_of_point(p, n)[0];
        let nf = n as f64;
        face_to_direction(map, face, 2.0 * (i as f64) / nf - 1.0, 2.0 * (j as f64) / nf - 1.0)
    }

    /// For a vertex `(k, l)` of the tile that is one of the eight cube corners: the three of its four stencil neighbours (slots 0 = east
    /// `(k + 1, l)`, 1 = west `(k - 1, l)`, 2 = north `(k, l + 1)`, 3 = south `(k, l - 1)`) that are three different points, in the
    /// canonical order of the cube edges they lie on (by the pair of faces holding the point), so that the three tiles at a corner
    /// form `(p1 - p0) x (p2 - p0)` from the same operands (ADR 0073 point 3). `None` for any other vertex.
    pub fn corner_stencil(self, res: u32, k: u32, l: u32) -> Option<[u8; 3]> {
        let n = (1i64 << self.level()) * i64::from(res);
        let (gi, gj) = (i64::from(self.x()) * i64::from(res) + i64::from(k), i64::from(self.y()) * i64::from(res) + i64::from(l));
        if !((gi == 0 || gi == n) && (gj == 0 || gj == n)) {
            return None;
        }
        // The two neighbours along the face's own edges, and one of the two that point outside (they are the same cube point).
        let along_i: u8 = if gi == 0 { 0 } else { 1 };
        let along_j: u8 = if gj == 0 { 2 } else { 3 };
        let outside: u8 = if gi == 0 { 1 } else { 0 };
        let point = |slot: u8| -> (i64, i64) {
            match slot {
                0 => (gi + 1, gj),
                1 => (gi - 1, gj),
                2 => (gi, gj + 1),
                _ => (gi, gj - 1),
            }
        };
        let key = |slot: u8| -> (u8, u8) {
            let (i, j) = point(slot);
            let held = faces_of_point(lattice_cube_point(self.face(), n, i, j), n);
            // A point strictly inside a cube edge is held by two faces; the pair orders the three edges of the corner.
            assert!(held.len() >= 2, "the neighbour of a cube corner lies on a cube edge");
            (held[0].0 .0, held[1].0 .0)
        };
        let mut ordered = [along_i, along_j, outside];
        ordered.sort_by_key(|s| key(*s));
        Some(ordered)
    }
}
