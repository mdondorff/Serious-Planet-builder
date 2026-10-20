//! SPIKE (lattice-normals): a hillshade of 6 x 6 tiles around a steep spot, shaded with the current normals and with the lattice
//! normals (same sun), and their difference. Writes PPMs (convert with ImageMagick).
use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::compact::{lattice_normals, ring_directions};
use planet_generators::height::HeightField;
use planet_generators::mesh::{normal_from_stencil, normal_stencil};
const R: f64 = 6_371_000.0;
const CELLS: u32 = 32;
fn main() {
    let level: u8 = std::env::var("SPH_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(17);
    let out = std::env::args().nth(1).unwrap_or_else(|| "target/ln/hs".into());
    let world = WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(R, 1), 256).unwrap();
    let field = world.field().unwrap();
    let centre = Vec3::new(-0.444444226, -0.887456451, 0.122026139).normalized();
    let t0 = TileId::from_direction(&TangentWarp, centre, level);
    let (x0, y0) = (t0.x().saturating_sub(3), t0.y().saturating_sub(3));
    let n = 6usize;
    let side = n * CELLS as usize;
    let mut cur = vec![Vec3::new(0.0, 0.0, 1.0); side * side];
    let mut lat = cur.clone();
    let mut up = cur.clone();
    for ty in 0..n {
        for tx in 0..n {
            let id = TileId::new(t0.face(), level, x0 + tx as u32, y0 + ty as u32).unwrap();
            let eps = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level) / f64::from(CELLS);
            let w = (CELLS + 1) as usize;
            let dirs: Vec<Vec3> = (0..=CELLS).flat_map(|j| (0..=CELLS).map(move |i| (i, j))).map(|(i, j)| id.sample_direction(&TangentWarp, CELLS, i, j)).collect();
            let st: Vec<[Vec3; 4]> = dirs.iter().map(|d| normal_stencil(*d, eps)).collect();
            let flat: Vec<Vec3> = st.iter().flatten().copied().collect();
            let h = field.heights(&flat);
            let ring = ring_directions(&TangentWarp, id, CELLS);
            let hr = field.heights(&ring);
            let ln = lattice_normals(&ring, &hr, CELLS, R);
            for j in 0..CELLS as usize {
                for i in 0..CELLS as usize {
                    let k = j * w + i;
                    let idx = (ty * CELLS as usize + j) * side + tx * CELLS as usize + i;
                    cur[idx] = normal_from_stencil(&st[k], &[h[4 * k], h[4 * k + 1], h[4 * k + 2], h[4 * k + 3]], R);
                    lat[idx] = ln[k];
                    up[idx] = dirs[k];
                }
            }
        }
    }
    // a low sun: 20 degrees above the horizon from the local east-south-east of the region centre
    let east = Vec3::new(0.0, 0.0, 1.0).cross(centre).normalized();
    let north = centre.cross(east).normalized();
    let sun = (centre * 0.34 + east * 0.8 + north * -0.5).normalized();
    let shade = |nrm: Vec3, u: Vec3| -> u8 {
        let _ = u;
        (255.0 * (0.12 + 0.88 * nrm.dot(sun).max(0.0))).min(255.0) as u8
    };
    let write = |name: &str, f: &dyn Fn(usize) -> u8| {
        let mut s = format!("P5\n{side} {side}\n255\n").into_bytes();
        for k in 0..side * side { s.push(f(k)); }
        std::fs::write(format!("{out}_{name}.pgm"), s).unwrap();
    };
    write("cur", &|k| shade(cur[k], up[k]));
    write("lat", &|k| shade(lat[k], up[k]));
    write("diff", &|k| ((shade(cur[k], up[k]) as i32 - shade(lat[k], up[k]) as i32).unsigned_abs() * 32).min(255) as u8);
    let mut maxd = 0.0f64;
    let mut sum = 0.0;
    for k in 0..side * side { let a = cur[k].dot(lat[k]).clamp(-1.0, 1.0).acos().to_degrees(); maxd = maxd.max(a); sum += a; }
    println!("level {level}: {side}x{side} vertices, normal angle mean {:.4} max {:.4} degrees; relief of the field here: spacing {:.2} m", sum / (side * side) as f64, maxd, core::f64::consts::FRAC_PI_2 * R / f64::from(1u32 << level) / f64::from(CELLS));
}
