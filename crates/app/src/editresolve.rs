//! `generate --edit-resolve DIR` (M8-7, criterion 6): the first measurement of an edit over chunks, repeatable and without a window. Opens the
//! stored world in `DIR` read-only (the chunks are computed in memory and nothing is written beside it), lands the chunks under a camera at `--site`, then for every radius of `--radii-m` puts a hill of that reach at the
//! site, counts how many chunks are re-solved and how long they take on the real thread pool, takes the hill away and settles. Never a perf
//! measurement (`cargo xtask perf` is the architect's); a recorded reading with the machine's load as it was.

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use planet_core::{cube::TangentWarp, PlanetFixed, TileId, Vec3};
use planet_streaming::chunks::ChunkConfig;
use planet_streaming::{SystemClock, ThreadPool};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec};

use crate::err;
use crate::worldfield::WorldTerrain;

/// The level of the tiles whose chunks are wanted (the first level that reads chunk detail in the A-net default).
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

/// Drive the chunk frames until every chunk the tiles read is ready and nothing is in flight; returns the time and the chunks landed.
fn settle(world: &Arc<WorldTerrain>, tiles: &[TileId], camera: PlanetFixed) -> Result<(Duration, u64), String> {
    let manager = world.chunk_manager().ok_or("the world has no chunk stage")?;
    let before = manager.stats().landed;
    let start = Instant::now();
    let mut quiet = 0;
    loop {
        let up = world.chunk_frame(tiles, camera, &[]);
        if up.started == 0 && manager.in_flight() == 0 {
            quiet += 1;
            if quiet >= 3 {
                return Ok((start.elapsed(), manager.stats().landed - before));
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

pub fn edit_resolve(o: &crate::Options, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.edit_resolve.as_ref().expect("checked by the caller");
    let (lat, lon) = o.site.unwrap_or((45.0, -100.0));
    let radii: Vec<f64> = match &o.radii_m {
        Some(s) => s
            .split(',')
            .map(|v| v.trim().parse::<f64>().map_err(|_| format!("--radii-m {s}: expected metres separated by commas")))
            .collect::<Result<_, _>>()?,
        None => vec![150.0, 500.0, 2_000.0, 5_000.0],
    };
    let workers = o.threads.max(1);
    let def = planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed);
    let world = Arc::new(WorldTerrain::open_store(def, dir, o.global_res)?);
    let pool = Arc::new(ThreadPool::new(workers));
    let cfg = ChunkConfig { workers, ..ChunkConfig::default() };
    if !world.enable_anet_from_store_with(crate::anetworld::ANET_CELL_M, pool, Arc::new(SystemClock::new()), cfg, false)? {
        return Err("the store holds no coupled run: make it first (the viewer or `test-render --scene coupled`)".into());
    }
    let site = Vec3::from_lat_lon(lat.to_radians(), lon.to_radians());
    let camera = PlanetFixed(site * world.radius_m());
    let tiles = tiles_at(site);
    writeln!(
        out,
        "edit-resolve: site {lat}, {lon}; {} tiles; {workers} chunk workers; store read-only: chunks computed in memory, nothing written",
        tiles.len()
    )
    .map_err(err)?;
    let (t, n) = settle(&world, &tiles, camera)?;
    writeln!(
        out,
        "landed: {n} chunks in {:.1} s ({:.1} s a chunk, {workers} workers)",
        t.as_secs_f64(),
        t.as_secs_f64() * workers as f64 / n.max(1) as f64
    )
    .map_err(err)?;
    let mut id = 1u128;
    for r in radii {
        let mut def = (*world.definition()).clone();
        let item =
            ItemSpec { id, layer: "stamps".into(), priority: 0, dir: site, content_hash: 0, reach_m: r, max_delta_m: 60.0, stamp: None }
                .with_stamp(Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]));
        def.put_item_spec(BodyId::PLANET, item).map_err(|e| format!("{e:?}"))?;
        world.set_definition(Arc::new(def.clone()))?;
        let (t, n) = settle(&world, &tiles, camera)?;
        writeln!(
            out,
            "hill reach {r} m: {n} chunks re-solved in {:.1} s ({:.1} s a chunk on {workers} workers)",
            t.as_secs_f64(),
            t.as_secs_f64() * workers as f64 / n.max(1) as f64
        )
        .map_err(err)?;
        def.remove_item(BodyId::PLANET, id);
        world.set_definition(Arc::new(def))?;
        let (t, n) = settle(&world, &tiles, camera)?;
        writeln!(out, "  removed: {n} chunks back in {:.1} s", t.as_secs_f64()).map_err(err)?;
        id += 1;
    }
    Ok(())
}
