//! SPIKE (lattice-normals): the angle between the current 4-evaluation normal and the lattice normal per level, on real worlds, and the
//! seams: the normals of the shared vertices of two neighbouring tiles of one level, in one face and across a cube edge.
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{Side, TileId, Vec3};
use planet_generators::compact::{lattice_normals, ring_directions};
use planet_generators::height::HeightField;
use planet_generators::mesh::{normal_from_stencil, normal_stencil};
const R: f64 = 6_371_000.0;
const CELLS: u32 = 32;
fn stencil_normals(field: &dyn HeightField, id: TileId) -> Vec<Vec3> {
    let eps = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level()) / f64::from(CELLS);
    let mut dirs = Vec::new();
    for j in 0..=CELLS { for i in 0..=CELLS { dirs.push(id.sample_direction(&TangentWarp, CELLS, i, j)); } }
    let mut out = Vec::new();
    let st: Vec<[Vec3; 4]> = dirs.iter().map(|d| normal_stencil(*d, eps)).collect();
    let flat: Vec<Vec3> = st.iter().flatten().copied().collect();
    let h = field.heights(&flat);
    for (k, s) in st.iter().enumerate() { out.push(normal_from_stencil(s, &[h[4 * k], h[4 * k + 1], h[4 * k + 2], h[4 * k + 3]], R)); }
    out
}
fn lattice(field: &dyn HeightField, id: TileId) -> Vec<Vec3> {
    let d = ring_directions(&TangentWarp, id, CELLS);
    let h = field.heights(&d);
    lattice_normals(&d, &h, CELLS, R)
}
fn angle(a: Vec3, b: Vec3) -> f64 { a.dot(b).clamp(-1.0, 1.0).acos().to_degrees() }
fn main() {
    let def = planet_world_def::WorldDefinition::single_planet(R, 1);
    let world = match std::env::var("SPN_STORE") {
        Ok(s) => WorldTerrain::open_store(def, std::path::Path::new(&s), 16384).unwrap(),
        Err(_) => WorldTerrain::open(def, 256).unwrap(),
    };
    let field = world.field().unwrap();
    let mut s = 0x9e3779b97f4a7c15u64;
    let mut next = move || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; (s >> 11) as f64 / (1u64 << 53) as f64 };
    println!("| level | spacing m | tiles | vertices | mean deg | p99 deg | max deg | max deg, 4 steepest tiles |\n|---|---|---|---|---|---|---|---|");
    for level in [6u8, 8, 10, 12, 14, 16, 18, 20, 22] {
        let mut tiles: Vec<(f64, TileId)> = Vec::new();
        for _ in 0..400 {
            let (z, a) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
            let r = libm::sqrt(1.0 - z * z);
            let t = TileId::from_direction(&TangentWarp, Vec3::new(r * libm::cos(a), r * libm::sin(a), z), level);
            let e = planet_frame::lod::tile_extent(&TangentWarp, t, &planet_frame::LodParams::earth_1080p());
            let (lo, hi) = field.height_bounds(e.dir, e.angular_radius);
            tiles.push((hi - lo, t));
        }
        tiles.sort_by(|a, b| b.0.total_cmp(&a.0));
        let pick: Vec<TileId> = tiles.iter().take(4).map(|t| t.1).chain(tiles.iter().skip(4).step_by(20).take(20).map(|t| t.1)).collect();
        let mut errs: Vec<f64> = Vec::new();
        let mut steep_max = 0.0f64;
        for (n, t) in pick.iter().enumerate() {
            let (a, b) = (stencil_normals(field.as_ref(), *t), lattice(field.as_ref(), *t));
            for (x, y) in a.iter().zip(&b) {
                let e = angle(*x, *y);
                errs.push(e);
                if n < 4 { steep_max = steep_max.max(e); }
            }
        }
        errs.sort_by(|a, b| a.total_cmp(b));
        let spacing = std::f64::consts::FRAC_PI_2 * R / f64::from(1u32 << level) / f64::from(CELLS);
        println!("| {level} | {spacing:.3} | {} | {} | {:.4} | {:.4} | {:.4} | {:.4} |", pick.len(), errs.len(), errs.iter().sum::<f64>() / errs.len() as f64, errs[(errs.len() as f64 * 0.99) as usize], errs[errs.len() - 1], steep_max);
    }
    println!("
cube-edge seams, chosen tiles (largest angle over the shared vertices)");
    println!("| level | tile | lattice deg | current stencil deg |
|---|---|---|---|");
    for level in [8u8, 12, 14, 16, 18, 20, 22] {
        let n = 1u32 << level;
        for (x, y, side) in [(0u32, n / 3, Side::West), (n - 1, n / 2 + 1, Side::East), (n / 5, 0, Side::South), (n / 3, n - 1, Side::North)] {
            let t = TileId::new(planet_core::Face(2), level, x, y).unwrap();
            if t.neighbor(&TangentWarp, side).face() == t.face() { continue; }
            let (l, s) = seam(field.as_ref(), t, side);
            println!("| {level} | {t:?} {side:?} | {l:.5} | {s:.5} |");
        }
    }
    // seams
    println!("\nseams: the largest angle between the normals the two tiles compute for the same vertex");
    println!("| level | kind | method | pairs | max deg |\n|---|---|---|---|---|");
    for level in [8u8, 12, 16, 20] {
        for (kind, face_edge) in [("same face", false), ("across a cube edge", true)] {
            let (mut worst_l, mut worst_s, mut pairs) = (0.0f64, 0.0f64, 0);
            let mut tries = 0;
            while pairs < 6 && tries < 4000 {
                tries += 1;
                let (z, a) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
                let r = libm::sqrt(1.0 - z * z);
                let t = TileId::from_direction(&TangentWarp, Vec3::new(r * libm::cos(a), r * libm::sin(a), z), level);
                let n = 1u32 << level;
                let on_edge = t.x() == 0 || t.y() == 0 || t.x() + 1 == n || t.y() + 1 == n;
                if on_edge != face_edge { continue; }
                for side in Side::ALL {
                    let o = t.neighbor(&TangentWarp, side);
                    if (o.face() != t.face()) != face_edge { continue; }
                    // the shared vertices: the points of `t` along `side` that are also lattice points of `o`
                    let (lt, lo) = (lattice(field.as_ref(), t), lattice(field.as_ref(), o));
                    let (st_t, st_o) = (stencil_normals(field.as_ref(), t), stencil_normals(field.as_ref(), o));
                    let w = (CELLS + 1) as usize;
                    for k in 0..w {
                        let dt: Vec3 = match side { Side::East => t.sample_direction(&TangentWarp, CELLS, CELLS, k as u32), Side::West => t.sample_direction(&TangentWarp, CELLS, 0, k as u32), Side::North => t.sample_direction(&TangentWarp, CELLS, k as u32, CELLS), Side::South => t.sample_direction(&TangentWarp, CELLS, k as u32, 0) };
                        // the vertex of `o` with the same direction
                        let mut found = None;
                        for jj in 0..w { for ii in 0..w {
                            if (o.sample_direction(&TangentWarp, CELLS, ii as u32, jj as u32) - dt).length() < 1e-12 { found = Some(jj * w + ii); }
                        } }
                        let Some(idx) = found else { continue };
                        let (ti, _) = match side { Side::East => (k * w + CELLS as usize, 0), Side::West => (k * w, 0), Side::North => (CELLS as usize * w + k, 0), Side::South => (k, 0) };
                        worst_l = worst_l.max(angle(lt[ti], lo[idx]));
                        worst_s = worst_s.max(angle(st_t[ti], st_o[idx]));
                    }
                    pairs += 1;
                    if pairs >= 6 { break; }
                }
            }
            println!("| {level} | {kind} | current stencil | {pairs} | {worst_s:.5} |\n| {level} | {kind} | lattice | {pairs} | {worst_l:.5} |");
        }
    }
}

/// The seam check of one tile and side: `(lattice, stencil)` largest angles over the shared vertices.
#[allow(dead_code)]
pub fn seam(field: &dyn HeightField, t: TileId, side: Side) -> (f64, f64) {
    let o = t.neighbor(&TangentWarp, side);
    let w = (CELLS + 1) as usize;
    let (lt, lo) = (lattice(field, t), lattice(field, o));
    let (st_t, st_o) = (stencil_normals(field, t), stencil_normals(field, o));
    let (mut wl, mut ws) = (0.0f64, 0.0f64);
    for k in 0..w {
        let (dt, ti) = match side {
            Side::East => (t.sample_direction(&TangentWarp, CELLS, CELLS, k as u32), k * w + CELLS as usize),
            Side::West => (t.sample_direction(&TangentWarp, CELLS, 0, k as u32), k * w),
            Side::North => (t.sample_direction(&TangentWarp, CELLS, k as u32, CELLS), CELLS as usize * w + k),
            Side::South => (t.sample_direction(&TangentWarp, CELLS, k as u32, 0), k),
        };
        for jj in 0..w { for ii in 0..w {
            if (o.sample_direction(&TangentWarp, CELLS, ii as u32, jj as u32) - dt).length() < 1e-12 {
                wl = wl.max(angle(lt[ti], lo[jj * w + ii]));
                ws = ws.max(angle(st_t[ti], st_o[jj * w + ii]));
            }
        } }
    }
    (wl, ws)
}
