//! SPIKE-102 addendum (spike only, not for merge): the 2 km still's camera of `erosion_stills` (site FACE,TI,TJ at the 0.75 point of the tile, heading AZ deg, 22 deg down, 60 deg fov, eye 2 km above the
//! ground), ray-marched against a dumped tile of heights, so a per-cell map (fill depth) can be drawn over the still. Writes `OUT.hit.f32` (1600 x 900 x 3: the map value at the hit, the hit cell i and j; -1 on a miss).
//! `cargo run --release -p planet --example overlay_cam -- --face 2 --ti 12 --tj 10 --az -45 --h b.h.f32 --map b.depth.f32 --out DIR/b`
use planet_core::cube::{direction_to_face, TangentWarp};
use planet_core::{Face, PlanetFixed, Vec3};
use planet_frame::camera::Camera;
use planet_generators::bake::tile::TileGrid;

const R: f64 = 6_371_000.0;
const TILE: usize = 3072;
const FACE_CELLS: usize = 98_304;
const SIZE: (usize, usize) = (1600, 900);

fn read(path: &str) -> Vec<f32> {
    std::fs::read(path).expect("read").as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone()).unwrap_or_else(|| panic!("{k} needed"));
    let (face, ti, tj): (u8, usize, usize) = (get("--face").parse().unwrap(), get("--ti").parse().unwrap(), get("--tj").parse().unwrap());
    let az: f64 = get("--az").parse().unwrap();
    let alt: f64 = a.iter().position(|x| x == "--alt").map_or(2000.0, |i| a[i + 1].parse().unwrap());
    let pitch: f64 = a.iter().position(|x| x == "--pitch").map_or(22.0, |i| a[i + 1].parse().unwrap());
    let h = read(&get("--h"));
    let map = read(&get("--map"));
    assert_eq!(h.len(), TILE * TILE);
    let grid = TileGrid { cells: FACE_CELLS, tile: TILE, halo: 256, radius_m: R };
    let centre = grid.direction(Face(face), (ti * TILE + 3 * TILE / 4) as i64, (tj * TILE + 3 * TILE / 4) as i64);
    let east = Vec3::new(0.0, 0.0, 1.0).cross(centre).normalized();
    let north = centre.cross(east).normalized();
    let heading = east * libm::sin(az.to_radians()) + north * libm::cos(az.to_radians());
    // Bilinear height of the dump at a direction; None outside the tile.
    let cell = |d: Vec3| -> Option<(f64, f64)> {
        let (f, s, t) = direction_to_face(&TangentWarp, d);
        if f.0 != face {
            return None;
        }
        let n = FACE_CELLS as f64;
        let (x, y) = ((s + 1.0) * 0.5 * n - 0.5 - (ti * TILE) as f64, (t + 1.0) * 0.5 * n - 0.5 - (tj * TILE) as f64);
        (x >= 0.0 && y >= 0.0 && x < (TILE - 1) as f64 && y < (TILE - 1) as f64).then_some((x, y))
    };
    let bil = |v: &[f32], x: f64, y: f64| {
        let (i, j) = (x.floor() as usize, y.floor() as usize);
        let (fx, fy) = (x - i as f64, y - j as f64);
        let g = |a: usize, b: usize| f64::from(v[b * TILE + a]);
        (1.0 - fy) * ((1.0 - fx) * g(i, j) + fx * g(i + 1, j)) + fy * ((1.0 - fx) * g(i, j + 1) + fx * g(i + 1, j + 1))
    };
    let (cx, cy) = cell(centre).expect("centre in the tile");
    let ground = bil(&h, cx, cy);
    println!("camera cell ({cx:.1}, {cy:.1}), ground {ground:.1} m, eye {:.1} m", ground + alt);
    let cam = Camera::at_surface_point(
        PlanetFixed(centre * (R + ground + alt)),
        heading,
        pitch.to_radians(),
        60f64.to_radians(),
        SIZE.0 as f64 / SIZE.1 as f64,
        1.0,
    );
    let (r, u, f) = cam.ray_frame();
    let pos = cam.position.0;
    let mut out = vec![-1.0f32; SIZE.0 * SIZE.1 * 3];
    for py in 0..SIZE.1 {
        for px in 0..SIZE.0 {
            let (ax, by) = (2.0 * (px as f64 + 0.5) / SIZE.0 as f64 - 1.0, 1.0 - 2.0 * (py as f64 + 0.5) / SIZE.1 as f64);
            let dir = (f + r * ax + u * by).normalized();
            let height_at = |t: f64| -> Option<(f64, f64, f64)> {
                let p = pos + dir * t;
                let (x, y) = cell(p.normalized())?;
                Some((p.length() - R - bil(&h, x, y), x, y))
            };
            let (mut t, mut prev) = (5.0, 5.0);
            while t < 120_000.0 {
                match height_at(t) {
                    None => break,
                    Some((above, _, _)) if above < 0.0 => {
                        let (mut lo, mut hi) = (prev, t);
                        for _ in 0..24 {
                            let mid = 0.5 * (lo + hi);
                            if height_at(mid).is_some_and(|v| v.0 < 0.0) {
                                hi = mid;
                            } else {
                                lo = mid;
                            }
                        }
                        let (_, x, y) = height_at(hi).expect("inside");
                        let o = (py * SIZE.0 + px) * 3;
                        out[o] = bil(&map, x, y) as f32;
                        out[o + 1] = x as f32;
                        out[o + 2] = y as f32;
                        break;
                    }
                    Some((above, _, _)) => {
                        prev = t;
                        t += (above * 0.25).clamp(8.0, 300.0).max(t * 0.002);
                    }
                }
            }
        }
    }
    let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(format!("{}.hit.f32", get("--out")), bytes).expect("write");
}
