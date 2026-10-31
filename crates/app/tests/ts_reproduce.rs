//! The terrain scorecard reproduces SPIKE-55's generator rows (TEST-079). Read only; needs the production store.

use planet::terrainscore::{analyse, sample_window, GRID};
use planet::worldfield::WorldTerrain;
use planet_world_def::WorldDefinition;

const STORE: &str = "C:/DEV/SeriousPlanetData/global16384";
const RES: u32 = 16384;
const TOLERANCE: f64 = 0.02;

/// The 18 generator sites of the scorecard through the run's own sampling and analysis, against SPIKE-55's rows (`data/spike55-generator-rows.toml`):
/// slope and local relief at 1 km and 5 km, p50 and p99, within 2 %. The real rows are not compared (the scorecard reads the central 0.5-degree
/// square of a tile, SPIKE-55 the whole tile). `PLANET_REAL_STORE=DIR` names another store. `cargo test -p planet --release --test it
/// the_scorecard_reproduces_spike_55 -- --ignored --nocapture`; prints `SPIKE-55 reproduced: ...`, or `NOT RUN (reason)` and fails when the store is absent.
// spec: TEST-079
#[test]
#[ignore = "samples 18 windows of the real store C:/DEV/SeriousPlanetData/global16384 (read only, about 40 s in release); NOT RUN when it is absent"]
fn the_scorecard_reproduces_spike_55() {
    let given = std::env::var_os("PLANET_REAL_STORE");
    let dir = given.as_deref().map_or(std::path::Path::new(STORE), std::path::Path::new);
    if !dir.join("pack-00000001.tpack").is_file() {
        println!("SPIKE-55 reproduction: NOT RUN ({} has no pack: the check needs the production store)", dir.display());
        panic!("SPIKE-55 reproduction: NOT RUN ({} has no pack)", dir.display());
    }
    let rows: toml::Table = include_str!("../data/spike55-generator-rows.toml").parse().expect("the rows file parses");
    let rows = rows["row"].as_array().expect("rows");
    assert_eq!(rows.len(), 18);
    let def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
    let world = WorldTerrain::open_store(def, dir, RES).unwrap_or_else(|e| panic!("SPIKE-55 reproduction: the store does not open: {e}"));
    let field = world.field().expect("the field");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let (mut worst, mut failures) = (0.0f64, Vec::new());
    for row in rows {
        let t = row.as_table().expect("a row");
        let (tile, lat, lon) = (t["tile"].as_str().unwrap(), t["lat"].as_float().unwrap(), t["lon"].as_float().unwrap());
        let z = sample_window(&*field, lat, lon, world.radius_m(), threads);
        let (stats, _) = analyse(&z, GRID, GRID, threads, false);
        for (k, key) in [(0, "slope"), (3, "relief_1km"), (4, "relief_5km")] {
            let want: Vec<f64> = t[key].as_array().unwrap().iter().map(|v| v.as_float().unwrap()).collect();
            for (q, w) in [50usize, 99].into_iter().zip(want) {
                let got = stats.series[k].p(q);
                let rel = (got / w - 1.0).abs();
                worst = worst.max(rel);
                if rel > TOLERANCE {
                    failures.push(format!(
                        "{tile} ({lat}, {lon}) {key} p{q}: {got:.4} against SPIKE-55 {w:.4} ({:+.2} %)",
                        (got / w - 1.0) * 100.0
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "SPIKE-55 not reproduced within 2 %:\n  {}", failures.join("\n  "));
    println!("SPIKE-55 reproduced: 18 sites, slope and relief 1 km and 5 km at p50 and p99, worst difference {:.3} %", worst * 100.0);
}
