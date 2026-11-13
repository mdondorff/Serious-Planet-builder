//! Tier A, GEN-185, GEN-186 and TEST-134: the river beds of a committed region are coherent. Along every stored channel line the stored ground (`B + L`, read back through the commit) does not rise
//! downstream, and the stored level of a channel point is the water surface of that ground (neither above nor below it by more than the store's steps). The fixture is the region of TEST-125.

use std::sync::Arc;

use crate::l_writer::{world, Dir};
use planet_cache::l_layer::CommitLayer;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{bake_region, RegionSpec};
use planet_cache::river_read::RiverReader;
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::k7::line_cells;
use planet_generators::erosion_layer::{ErosionLayer, LayerSource};

const R: f64 = 6_371_000.0;

fn spec() -> RegionSpec {
    RegionSpec {
        seed: 3,
        face: Face(2),
        radius_m: R,
        regional: 64,
        region: (16, 39, 16, 39),
        margin: 6,
        tile: 64,
        halo: 24,
        rounds: 2,
        threads: 2,
        res: 64,
        base_id: 7,
        inputs_id: 11,
        height_bounds: (-11_000.0, 9_000.0),
        keep_channels: true,
    }
}

fn base(d: Vec3) -> f64 {
    500.0 - 2400.0 * (d.x - 0.1)
        + 120.0 * libm::sin(40.0 * d.z)
        + 260.0 * libm::sin(173.0 * d.z + 2.0 * libm::sin(97.0 * d.y)) * libm::cos(151.0 * d.y)
        + 90.0 * libm::sin(611.0 * d.y + 433.0 * d.z)
}

fn climate(_: Vec3) -> (f64, f64, f64) {
    (1.0e-8, 0.0, 1.0)
}

/// What the stored lines of a commit show: rises of the stored ground along the line cells, and the stored level against the ground at the points.
#[derive(Debug, Default)]
struct Beds {
    steps: usize,
    rises: Vec<(i64, i64, f64)>,
    points: usize,
    level_minus_ground: Vec<f64>,
}

fn measure(s: &RegionSpec, dir: &std::path::Path) -> Beds {
    let w = world();
    let commit = planet_cache::l_writer::Commit::read(dir).unwrap().expect("a commit");
    let store = Arc::new(TileStore::open(dir, w.params()).unwrap());
    let reader = RiverReader::new(store, commit.clone(), 64 << 20).expect("a reader of the commit");
    let src: Arc<dyn LayerSource> = Arc::new(CommitLayer::open(dir, Arc::new(world()), 16).expect("the layer"));
    let layer = ErosionLayer::new(src).expect("the layer");
    let n = s.local() as f64;
    let face = s.face;
    let ground = |x: i64, y: i64| {
        let d = face_to_direction(&TangentWarp, face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
        base(d) + layer.change_at(d).expect("the change")
    };
    let t = s.tile as i64;
    let mut out = Beds::default();
    for &(f, ti, tj) in commit.tiles.keys() {
        let (x0, y0) = (i64::from(ti) * t, i64::from(tj) * t);
        let core = |x: i64, y: i64| x >= x0 && y >= y0 && x < x0 + t && y < y0 + t;
        for ch in reader.tile_channels(Face(f), ti, tj).expect("the tile's channels").iter() {
            let line = line_cells(ch);
            for k in 1..line.len() {
                let ((px, py), (x, y)) = (line[k - 1], line[k]);
                if !core(x, y) {
                    continue;
                }
                let (p, v) = (ground(px, py), ground(x, y));
                if p <= 0.0 || v <= 0.0 {
                    continue;
                }
                out.steps += 1;
                if v - p > 0.25 {
                    out.rises.push((x, y, v - p));
                }
            }
            for (k, &(px, py)) in ch.points.iter().enumerate() {
                let (x, y) = (libm::round(px) as i64, libm::round(py) as i64);
                if !core(x, y) {
                    continue;
                }
                let g = ground(x, y);
                if g <= 0.0 {
                    continue;
                }
                out.points += 1;
                out.level_minus_ground.push(ch.levels[k] - g);
            }
        }
    }
    out.level_minus_ground.sort_by(f64::total_cmp);
    out
}

// spec: GEN-185, GEN-186, TEST-134
#[test]
fn the_stored_ground_does_not_rise_along_a_stored_line_and_the_stored_level_is_its_water_surface() {
    let s = spec();
    let d = Dir::new("k7-beds");
    bake_region(&world(), &s, &base, &climate, d.path()).expect("the bake");
    let b = measure(&s, d.path());
    let lg = &b.level_minus_ground;
    let at = |q: f64| lg[((lg.len() as f64 * q) as usize).min(lg.len() - 1)];
    let worst = b.rises.iter().map(|r| r.2).fold(0.0, f64::max);
    println!(
        "TEST-134: {} land steps, {} rise over 0.25 m (largest {worst:.2} m); {} points, level - ground p1 {:.2} p50 {:.2} p99 {:.2} m",
        b.steps,
        b.rises.len(),
        b.points,
        at(0.01),
        at(0.5),
        at(0.99)
    );
    assert!(b.steps > 100 && b.points > 20, "the fixture has rivers on land: {} steps, {} points", b.steps, b.points);
    assert!(b.rises.is_empty(), "{} steps rise over 0.25 m along the stored lines, the first {:?}", b.rises.len(), &b.rises[..b.rises.len().min(8)]);
    assert!(at(0.01) >= -0.5 && at(0.99) <= 0.5, "the stored level is not the water surface of the stored ground: level - ground p1 {:.2}, p99 {:.2} m", at(0.01), at(0.99));
}

#[test]
#[ignore]
fn diag_rises() {
    let s = spec();
    let d = Dir::new("k7-beds-diag");
    let (_, report) = bake_region(&world(), &s, &base, &climate, d.path()).expect("the bake");
    let w = world();
    let commit = planet_cache::l_writer::Commit::read(d.path()).unwrap().unwrap();
    let store = Arc::new(TileStore::open(d.path(), w.params()).unwrap());
    let reader = RiverReader::new(store, commit.clone(), 64 << 20).unwrap();
    let src: Arc<dyn LayerSource> = Arc::new(CommitLayer::open(d.path(), Arc::new(world()), 16).unwrap());
    let layer = ErosionLayer::new(src).unwrap();
    let n = s.local() as f64;
    let ground = |x: i64, y: i64| {
        let dd = face_to_direction(&TangentWarp, s.face, 2.0 * (x as f64 + 0.5) / n - 1.0, 2.0 * (y as f64 + 0.5) / n - 1.0);
        base(dd) + layer.change_at(dd).unwrap()
    };
    let t = s.tile as i64;
    let mem: std::collections::BTreeSet<(i64, i64)> = report.channels.iter().flat_map(line_cells).collect();
    let mut worst = (f64::INFINITY, 0usize, 0usize, 0u32, 0u32);
    for &(_, ti, tj) in commit.tiles.keys() {
        let (x0, y0) = (i64::from(ti) * t, i64::from(tj) * t);
        for (ci, ch) in reader.tile_channels(s.face, ti, tj).unwrap().iter().enumerate() {
            for (k, &(px, py)) in ch.points.iter().enumerate() {
                let (x, y) = (libm::round(px) as i64, libm::round(py) as i64);
                if x < x0 || y < y0 || x >= x0 + t || y >= y0 + t { continue; }
                let g = ground(x, y);
                if g > 0.0 && ch.levels[k] - g < worst.0 { worst = (ch.levels[k] - g, ci, k, ti, tj); }
            }
        }
    }
    println!("worst level - ground {:.2} at tile {} {} channel {} point {}", worst.0, worst.3, worst.4, worst.1, worst.2);
    let chs = reader.tile_channels(s.face, worst.3, worst.4).unwrap();
    let ch = &chs[worst.1];
    for (k, &(px, py)) in ch.points.iter().enumerate() {
        let (x, y) = (libm::round(px) as i64, libm::round(py) as i64);
        println!("  point {k} ({x},{y}) level {:.2} ground {:.2} q {:.1}", ch.levels[k], ground(x, y), ch.q[k]);
    }
    let line = line_cells(ch);
    for (k, &(x, y)) in line.iter().enumerate() { println!("  cell {k} ({x},{y}) ground {:.2} mem {}", ground(x, y), mem.contains(&(x, y))); }
    let _ = &report;
}
