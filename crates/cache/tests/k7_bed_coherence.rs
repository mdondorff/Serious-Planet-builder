//! Tier A, GEN-185 and TEST-134: the river beds of a committed region are coherent. The burn levels come from the whole network (the running minimum down the channel tree of the H1 level and of K7's
//! start surface at the line cells), so along every stored channel line the stored ground (`B + L`, read back through the commit) does not rise downstream, the burn reaches the stored level, and no
//! tile's burn has a cutoff. The fixture is the region of TEST-125 on a rugged base; before GEN-185 it had 44 steps rising up to 5 m and stored levels 4.9 m (p50) to 194 m (p99) over the ground.
//! The fixture does not reproduce R3a's larger rises (r3a-e1: up to 42 m in K7's own heights, from the sliding minimum of the bounded carry): the evidence on production geometry is `k7_line_check`
//! on R3a (cache example), whose numbers the change's design records.

use std::sync::Arc;

use crate::l_writer::{world, Dir};
use planet_cache::l_layer::CommitLayer;
use planet_cache::pack::TileStore;
use planet_cache::region_bake::{bake_region, region_front, RegionSpec};
use planet_cache::river_read::RiverReader;
use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};
use planet_generators::bake::k7::{k7_tile, line_cells, K7Params};
use planet_generators::bake::tile::{TileGrid, TileId};
use planet_generators::erosion_layer::{ErosionLayer, LayerSource};
use planet_cache::l_writer::ChannelFeed;

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
    ground_over_level: Vec<f64>,
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
                if ch.levels[k] > 0.0 {
                    out.ground_over_level.push(g - ch.levels[k]);
                }
            }
        }
    }
    out.level_minus_ground.sort_by(f64::total_cmp);
    out
}

// spec: GEN-185, TEST-134
#[test]
fn the_stored_ground_does_not_rise_along_a_stored_line_and_the_burn_reaches_the_stored_level() {
    let s = spec();
    let d = Dir::new("k7-beds");
    let (_, report) = bake_region(&world(), &s, &base, &climate, d.path()).expect("the bake");
    let b = measure(&s, d.path());
    let lg = &b.level_minus_ground;
    let at = |q: f64| lg[((lg.len() as f64 * q) as usize).min(lg.len() - 1)];
    let worst = b.rises.iter().map(|r| r.2).fold(0.0, f64::max);
    println!(
        "TEST-134: {} land steps, {} rise over 0.25 m (largest {worst:.2} m); {} points, level - ground p1 {:.2} p50 {:.2} p99 {:.2} m; {}",
        b.steps,
        b.rises.len(),
        b.points,
        at(0.01),
        at(0.5),
        at(0.99),
        report.bed_cut.line()
    );
    assert!(b.steps > 100 && b.points > 20, "the fixture has rivers on land: {} steps, {} points", b.steps, b.points);
    assert!(b.rises.is_empty(), "{} steps rise over 0.25 m along the stored lines, the first {:?}", b.rises.len(), &b.rises[..b.rises.len().min(8)]);
    // The burn reaches the stored level wherever the level is on land: the ground stands at most the 0.5 m code of the level and the 0.25 m step of L over it (it may lie lower: incision and the final pass
    // cut under the burn; the read-time water level of REND-066 follows the ground). A level under the sea is floored at the sea level on land (GEN-176): those points are not compared.
    let over = b.ground_over_level.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(over <= 0.75, "the stored ground stands {over:.2} m over the stored level: the burn did not reach the level (GEN-185)");
}

// spec: GEN-185, TEST-134
#[test]
fn with_the_burn_levels_of_the_whole_network_no_tile_s_burn_has_a_cutoff() {
    let s = spec();
    let front = region_front(&s, &base, &climate).expect("the front");
    let regional = |d: Vec3| front.regional_field.at(d);
    let grid = TileGrid { cells: s.local(), tile: s.tile, halo: s.halo, radius_m: s.radius_m };
    let k7 = K7Params::standard(s.seed, 0.0);
    let (t, h, w) = (s.tile as i64, s.halo as i64, grid.window() as i64);
    let (mut tiles, mut burned, mut binds) = (0usize, 0usize, 0usize);
    for id in s.tiles() {
        let (i0, j0) = (i64::from(id.ti) * t - h, i64::from(id.tj) * t - h);
        let channels = front.feed.channels_near(id.face, i0, j0, w);
        if channels.is_empty() {
            continue;
        }
        let r = k7_tile(&grid, TileId { face: id.face, ti: id.ti, tj: id.tj }, &base, &regional, &channels, &k7, 2);
        tiles += 1;
        burned += r.burned;
        binds += r.burn_cutoff_binds;
        assert_eq!(r.burn_cutoff_binds, 0, "tile ({}, {}): the burn's cutoff binds {} cells (diff max {:.2} m)", id.ti, id.tj, r.burn_cutoff_binds, r.burn_cutoff_diff.max);
    }
    println!("TEST-134: {tiles} tiles with channels, {burned} cells burned, burn cutoff binds {binds}");
    assert!(tiles > 4 && burned > 100, "the fixture burns channels: {tiles} tiles, {burned} cells");
}
