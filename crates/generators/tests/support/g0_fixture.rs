//! The G0 minimum fixture (TEST-139; river-graph plan, G0 contract addendum section 2.7): a small deterministic landscape on a cube sphere whose H1 cell is 814 m at a face's centre and whose L cell
//! is an eighth of it, with the seven features of the addendum placed by longitude and latitude (degrees; longitude from +x towards +y, latitude towards +z):
//!
//! 1. one trunk and two tributaries (T1 from the north, T2 from the south);
//! 2. one explicit confluence (T1 into the trunk at longitude 5; T2 joins at 35);
//! 3. one antecedent river: the trunk crosses a narrow north-south ridge at longitude 25 that the base keeps and only the regional surface cuts;
//! 4. one closed basin meant as a protected lake (a cone at 55 E, 14 N);
//! 5. one divide close to a region boundary (the trunk's head against river W, the crest near longitude -30.9; the two-region layout splits at the H1 column boundary -30);
//! 6. one reach crossing a tile boundary (tiles of 64 L cells: face 0's boundary at longitude 15);
//! 7. one reach crossing a cube-face edge (faces 0 and 2 meet at longitude 45).
//!
//! The surfaces follow the plan's terms: `B` the base at L, `z_b` the base at the H1 cell centres, `z_s` the regional surface (here `z_b` with the trunk's H1 row lowered to its bed: the cut that makes
//! the river antecedent), `S0 = B + C1(z_s - z_b)` at L (the C1 read of each face's H1 grid, [`RegionField`], clamped at the face's edge). Every value is a function of its cell alone (`libm`), so the
//! fixture has the same bits on any thread count (CON-14). Test support only: no graph types (they wait for the G0 contract).

#![allow(dead_code)]

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::h0::GlobalGrid;
use planet_generators::bake::k7::RegionField;

/// H1 cells a face edge.
pub const H1_N: usize = 24;
/// L cells an H1 cell edge.
pub const L_PER_H1: usize = 8;
/// L cells a face edge.
pub const L_N: usize = H1_N * L_PER_H1;
/// The H1 cell edge at a face's centre (m).
pub const H1_CELL_M: f64 = 814.0;
/// The sphere's radius: a face edge spans a quarter circle of `H1_N` cells of 814 m.
pub const RADIUS_M: f64 = H1_CELL_M * H1_N as f64 * 2.0 / core::f64::consts::PI;
/// L cells a tile edge (3 x 3 tiles a face).
pub const TILE_L: usize = 64;
/// The sea level (m).
pub const SEA_LEVEL_M: f64 = 0.0;

/// The trunk's latitude: inside the H1 row centred at 1.875 N and on an L cell row's centre band, never on a cell boundary.
pub const TRUNK_LAT: f64 = 2.1;
/// Side slope of every valley (m a degree of distance from its line).
const SIDE_M_PER_DEG: f64 = 40.0;
/// The plateau the valleys are cut into (m).
const PLATEAU_M: f64 = 1100.0;
/// The land: longitude -44 to 68.5, latitude -36 to 36; the sea outside it.
const LAND_LON: (f64, f64) = (-44.0, 68.5);
const LAND_LAT: f64 = 36.0;
const SEA_FLOOR_M: f64 = -100.0;
/// The antecedent ridge: longitude, half-width (degrees), height (m).
pub const RIDGE_LON: f64 = 25.0;
const RIDGE_HALF_DEG: f64 = 1.2;
const RIDGE_M: f64 = 300.0;
/// The closed basin: centre (longitude, latitude), radius (degrees), depth of the cone (m).
pub const BOWL: (f64, f64) = (55.0, 14.0);
const BOWL_RADIUS_DEG: f64 = 3.0;
const BOWL_DEPTH_M: f64 = 200.0;
/// The divide between river W and the trunk's head: where W's valley surface (560 m at its head, -30.25) meets the trunk's (490 m at its head, -29.75) on the trunk's row, about -30.9; the
/// two-region layout splits at the H1 column boundary -30 just east of it.
pub const DIVIDE_LON: f64 = -30.9;
pub const REGION_SPLIT_LON: f64 = -30.0;

/// A river line: name, polyline (longitude, latitude) from head to mouth or confluence, bed (m) at the head and at the end.
pub struct Line {
    pub name: &'static str,
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub bed: (f64, f64),
}

/// The trunk's head and mouth longitudes.
pub const TRUNK_LON: (f64, f64) = (-29.75, 68.0);

/// The trunk's bed at a longitude: 490 m at its head falling linearly to 1 m at its mouth.
pub fn trunk_bed(lon: f64) -> f64 {
    490.0 - 489.0 * (lon - TRUNK_LON.0) / (TRUNK_LON.1 - TRUNK_LON.0)
}

/// The four lines: the trunk, its tributaries T1 (into the trunk at 5 E, the explicit confluence) and T2 (at 35 E), and river W west of the divide.
pub fn lines() -> [Line; 4] {
    [
        Line {
            name: "trunk",
            from: (TRUNK_LON.0, TRUNK_LAT),
            to: (TRUNK_LON.1, TRUNK_LAT),
            bed: (trunk_bed(TRUNK_LON.0), trunk_bed(TRUNK_LON.1)),
        },
        Line { name: "T1", from: (-5.0, 30.0), to: (5.0, TRUNK_LAT), bed: (800.0, trunk_bed(5.0)) },
        Line { name: "T2", from: (40.0, -28.0), to: (35.0, TRUNK_LAT), bed: (800.0, trunk_bed(35.0)) },
        Line { name: "W", from: (-30.25, TRUNK_LAT), to: (-44.0, TRUNK_LAT), bed: (560.0, 1.0) },
    ]
}

/// Longitude and latitude of a unit direction (degrees).
pub fn lon_lat(d: Vec3) -> (f64, f64) {
    (libm::atan2(d.y, d.x).to_degrees(), libm::asin(d.z.clamp(-1.0, 1.0)).to_degrees())
}

/// The base `B` (m) at a unit direction: valleys cut into a plateau along the four lines, the antecedent ridge across them all, the closed basin, and the sea outside the land.
pub fn base_m(d: Vec3) -> f64 {
    let (lon, lat) = lon_lat(d);
    if lon < LAND_LON.0 || lon > LAND_LON.1 || lat.abs() > LAND_LAT {
        return SEA_FLOOR_M;
    }
    let mut z = PLATEAU_M;
    for l in lines() {
        let (ax, ay, bx, by) = (l.from.0, l.from.1, l.to.0, l.to.1);
        let (dx, dy) = (bx - ax, by - ay);
        let t = (((lon - ax) * dx + (lat - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        let dist = libm::hypot(lon - ax - t * dx, lat - ay - t * dy);
        z = z.min(l.bed.0 + t * (l.bed.1 - l.bed.0) + SIDE_M_PER_DEG * dist);
    }
    let q = (lon - RIDGE_LON) / RIDGE_HALF_DEG;
    z += RIDGE_M * (1.0 - q * q).max(0.0);
    let r = libm::hypot(lon - BOWL.0, lat - BOWL.1);
    z -= BOWL_DEPTH_M * (1.0 - r / BOWL_RADIUS_DEG).max(0.0);
    z
}

/// The fixture's surfaces.
pub struct Fixture {
    /// `B` at the L cell centres (`6 L_N^2`, the cube grid's order: face, row, column).
    pub b: Vec<f64>,
    /// `z_b` and `z_s` at the H1 cell centres (`6 H1_N^2`).
    pub z_b: Vec<f64>,
    pub z_s: Vec<f64>,
    /// `S0 = B + C1(z_s - z_b)` at L.
    pub s0: Vec<f64>,
}

/// Fills `out` by `f(cell)` over `threads` scoped threads in contiguous chunks: each value depends on its cell alone, so any thread count gives the same bits.
fn par_fill(out: &mut [f64], threads: usize, f: &(dyn Fn(usize) -> f64 + Sync)) {
    let chunk = out.len().div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, part) in out.chunks_mut(chunk).enumerate() {
            s.spawn(move || {
                for (c, v) in part.iter_mut().enumerate() {
                    *v = f(k * chunk + c);
                }
            });
        }
    });
}

/// Whether an H1 cell is on the trunk's row (the H1 cell holding the trunk's latitude) within the trunk's longitudes: the cells `z_s` lowers to the bed.
fn trunk_row(d: Vec3, h1: &GlobalGrid, id: u32) -> bool {
    let (lon, lat) = lon_lat(d);
    let (f, i, j) = h1.split(id);
    if !(TRUNK_LON.0..=TRUNK_LON.1).contains(&lon) || (lat - TRUNK_LAT).abs() > 2.0 {
        return false;
    }
    // the row whose span holds the trunk's latitude at this column
    let face = Face(f);
    let n = H1_N as f64;
    let s = 2.0 * (i as f64 + 0.5) / n - 1.0;
    let row_of = |jj: usize| {
        let (lo, hi) = (2.0 * jj as f64 / n - 1.0, 2.0 * (jj as f64 + 1.0) / n - 1.0);
        (lon_lat(face_to_direction(&TangentWarp, face, s, lo)).1, lon_lat(face_to_direction(&TangentWarp, face, s, hi)).1)
    };
    let (a, b) = row_of(j);
    a.min(b) <= TRUNK_LAT && TRUNK_LAT < a.max(b)
}

/// Builds the fixture on `threads` threads.
pub fn build(threads: usize) -> Fixture {
    let h1 = GlobalGrid::new(H1_N, RADIUS_M);
    let l = GlobalGrid::new(L_N, RADIUS_M);
    let mut z_b = vec![0.0; 6 * H1_N * H1_N];
    par_fill(&mut z_b, threads, &|p| base_m(h1.direction(p as u32)));
    let z_s: Vec<f64> = (0..z_b.len())
        .map(|p| {
            let d = h1.direction(p as u32);
            if trunk_row(d, &h1, p as u32) {
                z_b[p].min(trunk_bed(lon_lat(d).0))
            } else {
                z_b[p]
            }
        })
        .collect();
    let field = |v: &[f64], f: usize| {
        let nn = H1_N * H1_N;
        RegionField::new(Face(f as u8), H1_N, (0, 0), H1_N, H1_N, v[f * nn..(f + 1) * nn].iter().map(|&x| x as f32).collect())
    };
    let change: Vec<f64> = z_s.iter().zip(&z_b).map(|(s, b)| s - b).collect();
    let fields: Vec<RegionField> = (0..6).map(|f| field(&change, f)).collect();
    let mut b = vec![0.0; 6 * L_N * L_N];
    par_fill(&mut b, threads, &|p| base_m(l.direction(p as u32)));
    let mut s0 = vec![0.0; b.len()];
    par_fill(&mut s0, threads, &|p| {
        let f = p / (L_N * L_N);
        b[p] + fields[f].at(l.direction(p as u32))
    });
    Fixture { b, z_b, z_s, s0 }
}

/// A partition layout: each H1 cell's region (L cells take their H1 cell's region).
pub struct Layout {
    pub name: &'static str,
    pub regions: usize,
    pub region_of_h1: Vec<u8>,
}

/// The two layouts: one region; two regions split at longitude -30 (a column boundary of face 0's H1 grid, about 0.9 degrees east of the divide): region 0 west of it, 1 east.
pub fn layouts() -> [Layout; 2] {
    let h1 = GlobalGrid::new(H1_N, RADIUS_M);
    let split = (0..6 * H1_N * H1_N)
        .map(|p| {
            let (lon, _) = lon_lat(h1.direction(p as u32));
            u8::from(lon >= REGION_SPLIT_LON)
        })
        .collect();
    [
        Layout { name: "one-region", regions: 1, region_of_h1: vec![0; 6 * H1_N * H1_N] },
        Layout { name: "two-regions", regions: 2, region_of_h1: split },
    ]
}

/// The H1 cell of an L cell.
pub fn h1_of_l(p: usize) -> usize {
    let nn = L_N * L_N;
    let (f, i, j) = (p / nn, p % L_N, (p % nn) / L_N);
    f * H1_N * H1_N + (j / L_PER_H1) * H1_N + i / L_PER_H1
}

/// The tile of an L cell: (face, tile column, tile row).
pub fn tile_of_l(p: usize) -> (usize, usize, usize) {
    let nn = L_N * L_N;
    (p / nn, (p % L_N) / TILE_L, (p % nn) / L_N / TILE_L)
}

/// The cell of a grid of `n` cells a face edge nearest a longitude and latitude.
pub fn cell_at(n: usize, lon: f64, lat: f64) -> usize {
    let (lo, la) = (lon.to_radians(), lat.to_radians());
    let d = Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    let (face, s, t) = planet_core::cube::direction_to_face(&TangentWarp, d);
    let k = |x: f64| ((((x + 1.0) * 0.5) * n as f64) as usize).min(n - 1);
    face.0 as usize * n * n + k(t) * n + k(s)
}
