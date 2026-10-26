//! `generate --edit-resolve DIR` (M8-7, criterion 6): the measurement of an edit over chunks, repeatable and without a window. Opens the stored
//! world in `DIR` read-only (the chunks are computed in memory and nothing is written beside it), lands the chunks a camera at `--site` wants,
//! then for every radius of `--radii-m` puts a hill of that reach at the site and reports how many of the wanted chunks it re-solves, when the
//! chunk under the edit centre lands and when the last one does, on the real thread pool; the hill is taken away and the chunks settle again.
//! Without `--altitudes-m` the camera is low over the site (its 3 x 3 level-15 tiles); with it, a camera at each altitude looking north and
//! 20 degrees down wants the chunks the level-of-detail selection reads in its view. Never a perf measurement (`cargo xtask perf` is the
//! architect's): a recorded reading, with the machine's load as it was.

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use planet_core::{cube::TangentWarp, PlanetFixed, TileId, Vec3};
use planet_frame::{select_nodes_in_view_field, Camera, LodParams};
use planet_generators::chunkfield::ChunkKey;
use planet_generators::height::HeightField;
use planet_streaming::chunks::ChunkConfig;
use planet_streaming::{SystemClock, ThreadPool};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec};

use crate::err;
use crate::worldfield::WorldTerrain;

/// The level of the tiles whose chunks the low camera wants (the first level that reads chunk detail in the A-net default).
const TILE_LEVEL: u8 = 15;
/// The longest a phase may take before the run gives up (seconds).
const GIVE_UP_S: u64 = 900;

/// The tiles around the site that stand for a camera low over it: the tile under it and its eight neighbours.
fn tiles_at(dir: Vec3) -> Vec<TileId> {
    let map = &TangentWarp;
    let centre = TileId::from_direction(map, dir, TILE_LEVEL);
    use planet_core::Side::{East, North, South, West};
    let mut out = vec![centre];
    for side in [East, West, North, South] {
        out.push(centre.neighbor(map, side));
    }
    for (a, b) in [(East, North), (East, South), (West, North), (West, South)] {
        out.push(centre.neighbor(map, a).neighbor(map, b));
    }
    out
}

/// The tiles of the plan of a camera `altitude_m` over `site`, looking north and 20 degrees down, over the world's current field.
fn plan_tiles(world: &Arc<WorldTerrain>, site: Vec3, altitude_m: f64) -> Result<Vec<TileId>, String> {
    let field = world.field()?;
    let radius = world.radius_m();
    let params = LodParams {
        radius_m: radius,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let position = PlanetFixed(site * (radius + altitude_m + field.height(site)));
    let camera = Camera::at_surface_point(position, Vec3::new(0.0, 0.0, 1.0), 20f64.to_radians(), params.fov_y_rad, 16.0 / 9.0, 0.5);
    let sel = select_nodes_in_view_field(&TangentWarp, &camera, &*field, &params);
    Ok(sel.nodes.iter().map(|n| n.tile).collect())
}

/// What one settling of the chunks took.
struct Settled {
    total: Duration,
    landed: u64,
    /// When the chunk named `centre` landed (None if it did not land in this settling).
    centre: Option<Duration>,
}

/// Drive the chunk frames until every chunk the tiles read is ready and nothing is in flight.
fn settle(world: &Arc<WorldTerrain>, tiles: &[TileId], camera: PlanetFixed, centre: ChunkKey) -> Result<Settled, String> {
    let manager = world.chunk_manager().ok_or("the world has no chunk stage")?;
    let before = manager.stats().landed;
    let start = Instant::now();
    let (mut quiet, mut centre_at) = (0, None);
    loop {
        let up = world.chunk_frame(tiles, camera, &[]);
        if centre_at.is_none() && up.landed.contains(&centre) {
            centre_at = Some(start.elapsed());
        }
        if up.started == 0 && manager.in_flight() == 0 {
            quiet += 1;
            if quiet >= 3 {
                return Ok(Settled { total: start.elapsed(), landed: manager.stats().landed - before, centre: centre_at });
            }
        } else {
            quiet = 0;
        }
        if start.elapsed().as_secs() > GIVE_UP_S {
            return Err(format!("the chunks did not settle in {GIVE_UP_S} s"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// The per-chunk-job times of the phases of the chunks solved since the last call: the network (base heights, routing, beds), the solve and the
/// drivers, summed over the jobs and divided by their number; the jobs overlap in wall time when they run in parallel.
fn phases() -> String {
    let [network, solve, drivers, solved] = planet_generators::anet::phase_totals_ms();
    if solved < 1.0 {
        return String::new();
    }
    let per = |ms: f64| ms / solved / 1000.0;
    format!(" [per chunk job: network {:.1} s, solve {:.1} s, drivers {:.1} s, {solved:.0} jobs]", per(network), per(solve), per(drivers))
}

fn fmt_opt(d: Option<Duration>) -> String {
    d.map_or_else(|| "(not re-solved)".to_string(), |d| format!("{:.1} s", secs(d)))
}

fn list(s: &Option<String>, default: &[f64], name: &str) -> Result<Vec<f64>, String> {
    match s {
        Some(s) => {
            s.split(',').map(|v| v.trim().parse::<f64>().map_err(|_| format!("{name} {s}: expected metres separated by commas"))).collect()
        }
        None => Ok(default.to_vec()),
    }
}

pub fn edit_resolve(o: &crate::Options, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.edit_resolve.as_ref().expect("checked by the caller");
    let (lat, lon) = o.site.unwrap_or((45.0, -100.0));
    let radii = list(&o.radii_m, &[150.0, 500.0, 2_000.0, 5_000.0], "--radii-m")?;
    // No altitude given: the low camera (0 stands for it).
    let altitudes = list(&o.altitudes_m, &[0.0], "--altitudes-m")?;
    let workers = o.threads.max(1);
    let def = planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed);
    let world = Arc::new(WorldTerrain::open_store(def, dir, o.global_res)?);
    let pool = Arc::new(ThreadPool::new(workers));
    let cfg = {
        let d = ChunkConfig::default();
        ChunkConfig { workers, stale_workers: o.stale_workers.unwrap_or(d.stale_workers), ..d }
    };
    if !world.enable_anet_from_store_with(crate::anetworld::ANET_CELL_M, pool, Arc::new(SystemClock::new()), cfg, false)? {
        return Err("the store holds no coupled run: make it first (the viewer or `test-render --scene coupled`)".into());
    }
    let manager = world.chunk_manager().ok_or("the world has no chunk stage")?;
    let site = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
    let centre_tile = TileId::from_direction(&TangentWarp, site, planet_generators::chunk::CHUNK_GRID_LEVEL);
    let centre: ChunkKey = (centre_tile.face().0, centre_tile.x(), centre_tile.y());
    writeln!(
        out,
        "edit-resolve: site {lat}, {lon}; {workers} chunk workers (at most {} on stale chunks); store read-only: chunks computed in memory, nothing written; centre chunk {centre:?}",
        cfg.stale_workers
    )
    .map_err(err)?;
    for (n, altitude) in altitudes.into_iter().enumerate() {
        let (tiles, camera, label) = if altitude <= 0.0 {
            (tiles_at(site), PlanetFixed(site * world.radius_m()), "low camera (3 x 3 level-15 tiles)".to_string())
        } else {
            (
                plan_tiles(&world, site, altitude)?,
                PlanetFixed(site * (world.radius_m() + altitude)),
                format!("camera {altitude} m up, in view"),
            )
        };
        let wanted: usize = {
            let mut seen = std::collections::HashSet::new();
            tiles.iter().flat_map(|&t| manager.reads(t)).filter(|k| seen.insert(*k)).count()
        };
        writeln!(out, "{label}: {} tiles read {wanted} chunks", tiles.len()).map_err(err)?;
        let s = settle(&world, &tiles, camera, centre)?;
        writeln!(out, "  landed: {} chunks in {:.1} s (centre chunk after {}){}", s.landed, secs(s.total), fmt_opt(s.centre), phases())
            .map_err(err)?;
        for (k, &r) in radii.iter().enumerate() {
            let id = (n * radii.len() + k + 1) as u128;
            let mut def = (*world.definition()).clone();
            let item = ItemSpec {
                id,
                layer: "stamps".into(),
                priority: 0,
                dir: site,
                content_hash: 0,
                reach_m: r,
                max_delta_m: 60.0,
                stamp: None,
            }
            .with_stamp(Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]));
            def.put_item_spec(BodyId::PLANET, item).map_err(|e| format!("{e:?}"))?;
            world.set_definition(Arc::new(def.clone()))?;
            let s = settle(&world, &tiles, camera, centre)?;
            writeln!(
                out,
                "  hill reach {r} m: {} of {wanted} wanted chunks re-solved; the centre chunk landed after {}, the last after {:.1} s{}",
                s.landed,
                fmt_opt(s.centre),
                secs(s.total),
                phases()
            )
            .map_err(err)?;
            def.remove_item(BodyId::PLANET, id);
            world.set_definition(Arc::new(def))?;
            let back = settle(&world, &tiles, camera, centre)?;
            writeln!(out, "    removed: {} chunks back in {:.1} s", back.landed, secs(back.total)).map_err(err)?;
        }
    }
    Ok(())
}
