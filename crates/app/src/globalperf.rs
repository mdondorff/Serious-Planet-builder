//! The global-bake performance scene (GEN-014): `test-render --perf --scene global-bake`.

use crate::{err, Options};
use planet_render::GpuContext;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

/// The GEN-014 budgets (CON-20, CON-21): a bake of a 1 km grid takes at most ten minutes and holds at most 8 GB. Returns the
/// violated budgets, empty when both hold.
pub(crate) fn budget_failures(secs: f64, peak_resident_bytes: u64) -> Vec<String> {
    let mut v = Vec::new();
    if !secs.is_finite() || secs > 600.0 {
        v.push(format!("bake time {secs:.1} s exceeds the ten minute budget (GEN-014)"));
    }
    if peak_resident_bytes > 8 << 30 {
        v.push(format!("peak resident blocks {peak_resident_bytes} bytes exceed the 8 GB budget (CON-21)"));
    }
    v
}

// spec: GEN-014
/// Bake the global-coarse field into a scratch store under `target/perf/global-bake` (the real pipeline: bake, encode, pack)
/// after the ADR 0020 readiness gate, and report wall time, the most blocks alive at once with the bytes they take, and the
/// store size, against the GEN-014 budgets. The resident figure is derived from the block counter times the block size (the
/// blocks are the memory the clause bounds), not a process measurement. The default resolution is the 1 km grid of GEN-014
/// (16384 cells per face edge); `--global-res`, `--threads`, `--seed`/`--world` and `--out` (the JSON log) apply.
pub fn perf_global_bake(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    let res = if o.given.contains("--global-res") { o.global_res } else { 16384 };
    let world = crate::skeleton::global_world_of(o)?;
    let (level, cells) = world.layout(res).map_err(|e| e.to_string())?;
    let threads = if o.threads == 0 { std::thread::available_parallelism().map_or(1, |n| n.get()) } else { o.threads };
    let dir = Path::new("target").join("perf").join("global-bake");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("clearing {}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).map_err(err)?;
    writeln!(out, "global bake perf: {res} cells per face edge, blocks of {cells} (level {level}), {threads} threads").map_err(err)?;
    let start = Instant::now();
    let report = planet_cache::global_store::build_global(&dir, &world, res, threads, planet_cache::pack::DEFAULT_MAX_PACK_BYTES)
        .map_err(|e| e.to_string())?;
    let secs = start.elapsed().as_secs_f64();
    let block_bytes = u64::from(cells + 1) * u64::from(cells + 1) * 16;
    let peak = report.peak_resident_blocks as u64 * block_bytes;
    let store_bytes: u64 = std::fs::read_dir(&dir)
        .map_err(err)?
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum();
    let failures = budget_failures(secs, peak);
    writeln!(
        out,
        "bake {secs:.1} s, {} blocks, at most {} blocks alive ({:.2} GB), store {:.2} GB",
        report.written,
        report.peak_resident_blocks,
        peak as f64 / 1e9,
        store_bytes as f64 / 1e9
    )
    .map_err(err)?;
    let json = format!(
        "{{\n  \"scene\": \"global-bake\",\n  \"adapter\": \"{}\",\n  \"owner_override\": {:?},\n  \"res\": {res},\n  \"cells\": {cells},\n  \"threads\": {threads},\n  \"seconds\": {secs:.3},\n  \"blocks\": {},\n  \"peak_resident_blocks\": {},\n  \"peak_resident_bytes\": {peak},\n  \"store_bytes\": {store_bytes},\n  \"budget_failures\": {:?}\n}}\n",
        ctx.adapter_name().replace('"', "'"),
        owner_override,
        report.written,
        report.peak_resident_blocks,
        failures
    );
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("global-bake.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)?;
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// The GEN-042 budget (M8 exit criterion 4): the coupled run takes at most ten minutes. Returns the violated budgets, empty when it holds.
pub(crate) fn coupled_budget_failures(secs: f64) -> Vec<String> {
    if secs.is_finite() && secs <= 600.0 {
        Vec::new()
    } else {
        vec![format!("coupled run time {secs:.1} s exceeds the ten minute budget (GEN-042)")]
    }
}

/// Steps of the timed coupled run: the top of the 70 to 100 the world version is expected to take.
pub(crate) const COUPLED_STEPS: usize = 100;

/// The fixed input of the timed run: a smooth world of about 60 % land (the heights from the direction), uplift and erodibility constant.
fn coupled_input(lat: &planet_generators::coupled::Lattice) -> planet_generators::coupled::CoupledInput {
    let z: Vec<f32> = (0..lat.len())
        .map(|c| {
            let d = lat.direction(c);
            (1200.0 * (libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y) + 0.5 * libm::sin(5.0 * d.z)) + 200.0) as f32
        })
        .collect();
    planet_generators::coupled::CoupledInput { uplift: vec![1.0e-3; z.len()], erodibility: vec![2.0e-5; z.len()], z }
}

/// Run the coupled run once from the fixed input and the default parameters (seed 0): the wall seconds and the run.
pub(crate) fn time_coupled(res: usize, steps: usize) -> (f64, planet_generators::coupled::CoupledState) {
    use planet_generators::coupled::{run, CoupledParams, Lattice};
    let lat = Lattice::new(res, planet_core::EARTH_RADIUS_M);
    let input = coupled_input(&lat);
    let start = Instant::now();
    let state = run(&lat, &input, &CoupledParams::default(), steps);
    (start.elapsed().as_secs_f64(), state)
}

// spec: GEN-042
/// Time the global coupled run (GEN-040) at `--global-res` cells per face edge (default 2048) for [`COUPLED_STEPS`] steps after the ADR 0020
/// readiness gate, and report wall time, steps, seconds per step and threads (the run is single-threaded) against the GEN-042 budget.
pub fn perf_coupled(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    if let Some(into) = &o.remake_into {
        return perf_coupled_make(o, ctx, out, &owner_override, into);
    }
    let res = if o.given.contains("--global-res") { o.global_res } else { 2048 };
    writeln!(out, "coupled run perf: {res} cells per face edge, {COUPLED_STEPS} steps, 1 thread").map_err(err)?;
    let (secs, state) = time_coupled(res as usize, COUPLED_STEPS);
    let per_step = secs / COUPLED_STEPS as f64;
    let last = state.steps.last().map_or(f64::NAN, |s| s.mean_dz_m);
    let failures = coupled_budget_failures(secs);
    writeln!(out, "coupled {secs:.1} s, {COUPLED_STEPS} steps, {per_step:.2} s per step, 1 thread (last step mean change {last:.4} m)")
        .map_err(err)?;
    let json = format!(
        "{{\n  \"scene\": \"coupled\",\n  \"adapter\": \"{}\",\n  \"owner_override\": {:?},\n  \"res\": {res},\n  \"steps\": {COUPLED_STEPS},\n  \"threads\": 1,\n  \"seconds\": {secs:.3},\n  \"seconds_per_step\": {per_step:.4},\n  \"budget_failures\": {:?}\n}}\n",
        ctx.adapter_name().replace('"', "'"),
        owner_override,
        failures
    );
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("coupled.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)?;
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

// spec: GEN-042
/// The make of the coupled run from the stored bake of `--store` (the viewer's make: reduction of the bake, then the steps), written to `into` under
/// `target/` and never into the store, after the readiness gate: wall time against the GEN-042 budget, log `perf/logs/coupled-make.json`.
fn perf_coupled_make(o: &Options, ctx: &GpuContext, out: &mut dyn Write, owner_override: &[String], into: &Path) -> Result<(), String> {
    let store = o.store.clone().ok_or("--remake-into needs --store DIR, the stored bake to read")?;
    writeln!(out, "coupled make perf: bake of {} in {}, written to {}", o.global_res, store.display(), into.display()).map_err(err)?;
    let (secs, res) = crate::coupledworld::timed_remake(o, &store, into)?;
    let failures = coupled_budget_failures(secs);
    writeln!(out, "coupled make {secs:.1} s, {res} cells per face edge, {COUPLED_STEPS} steps").map_err(err)?;
    let json = format!(
        "{{
  \"scene\": \"coupled-make\",
  \"adapter\": \"{}\",
  \"owner_override\": {:?},
  \"bake_res\": {},
  \"res\": {res},
  \"steps\": {COUPLED_STEPS},
  \"seconds\": {secs:.3},
  \"budget_failures\": {:?}
}}
",
        ctx.adapter_name().replace('"', "'"),
        owner_override,
        o.global_res,
        failures
    );
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("coupled-make.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)?;
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// The GEN-052 budget: an A-net chunk of 24 km2 takes at most 4 s at 8 m cells on the stage's production thread setting.
pub(crate) fn anet_chunk_budget_failures(cell_m: f64, secs: f64) -> Vec<String> {
    if cell_m == 8.0 && !(secs.is_finite() && secs <= 4.0) {
        vec![format!("A-net chunk time {secs:.2} s at 8 m exceeds 4 s (GEN-052)")]
    } else {
        Vec::new()
    }
}

// spec: GEN-052
/// Time one A-net chunk (the chunk of the coupled run's largest land river) at 8, 4 and 10 m with the 3 km halo after the ADR 0020 readiness gate, on the
/// stage's production thread setting (`WorldBase`'s default), and once more at 8 m on one thread (recorded, not judged). The world is the default eroded one
/// over a bake of `--global-res` cells (default 256, which makes the coarse segments 8 times longer than the production run's) or the stored production
/// bake of `--store DIR` (then `--global-res 16384`: the geometry of the viewer).
pub fn perf_anet_chunk(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    use crate::anetworld::ANET_TOP_LEVEL;
    use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_HALO_M};
    use planet_generators::anet::{ANet, ANetParams};
    use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkLattice};
    use std::sync::Arc;
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    let res = if o.given.contains("--global-res") { o.global_res } else { 256 };
    let store = o.store.clone().or_else(|| std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from));
    let (world, maps, tile) = anet_scene(res, o.seed, store, out)?;
    let radius = world.radius_m();
    let mut failures = Vec::new();
    let mut rows = Vec::new();
    for (cell_m, one_thread) in [(8.0, false), (8.0, true), (4.0, false), (10.0, false)] {
        let per = cells_per_chunk_for(radius, cell_m);
        let lat = ChunkLattice::new(per).ok_or("no lattice")?;
        let stage = ANet {
            run: maps.clone(),
            base: Arc::new(if one_thread { WorldBase::new(&world).with_threads(1) } else { WorldBase::new(&world) }),
            radius_m: radius,
            params: {
                let mut params = ANetParams::spike(ANET_HALO_M, lat.cell_m(radius));
                params.drivers.seed = world.base_seed();
                params
            },
            memo: Default::default(),
        };
        let halo = halo_rings(&stage);
        // The share of the base field: its heights over the region, on the same thread setting.
        let reg = planet_generators::chunkfield::region(&lat, tile, halo);
        let dirs: Vec<planet_core::Vec3> = reg.cells.iter().map(|&c| lat.direction(c)).collect();
        let t0 = Instant::now();
        let _ = planet_generators::anet::BaseDetail::base_heights(&*stage.base, &dirs);
        let base_secs = t0.elapsed().as_secs_f64();
        let start = Instant::now();
        let r = compute_chunk(&lat, &stage, radius, ANET_TOP_LEVEL, tile, halo).ok_or("the chunk was refused")?;
        let secs = start.elapsed().as_secs_f64();
        let cells = (per as usize + 2 * halo as usize).pow(2);
        writeln!(
            out,
            "{cell_m} m, {}: {per} cells per chunk, halo {halo} rings, about {cells} cells with the halo: {secs:.2} s, the base field of it {base_secs:.2} s (hash {:016x})",
            if one_thread { "one thread (recorded)" } else { "production threads" },
            r.hash()
        )
        .map_err(err)?;
        if cell_m == 8.0 && !one_thread {
            anet_steps(&world, &maps, &stage, &lat, tile, out)?;
        }
        if !one_thread {
            failures.extend(anet_chunk_budget_failures(cell_m, secs));
        }
        rows.push(format!(
            "{{\"cell_m\": {cell_m}, \"threads\": \"{}\", \"cells_per_chunk\": {per}, \"halo_rings\": {halo}, \"seconds\": {secs:.3}}}",
            if one_thread { "1" } else { "production" }
        ));
    }
    let json = format!(
        "{{\n  \"scene\": \"anet-chunk\",\n  \"adapter\": \"{}\",\n  \"owner_override\": {:?},\n  \"res\": {res},\n  \"chunks\": [{}],\n  \"budget_failures\": {:?}\n}}\n",
        ctx.adapter_name().replace('"', "'"),
        owner_override,
        rows.join(", "),
        failures
    );
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("anet-chunk.json"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)?;
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// The world of the chunk scenes: the default eroded world over a bake of `res` cells, its coupled run, and the chunk of the land cell with the
/// largest drainage area that drains on, with that area in m2.
fn anet_scene(
    res: u32,
    seed: u64,
    store: Option<std::path::PathBuf>,
    out: &mut dyn Write,
) -> Result<
    (std::sync::Arc<crate::worldfield::WorldTerrain>, std::sync::Arc<planet_generators::coupled::CoupledMaps>, planet_core::TileId),
    String,
> {
    use crate::coupledworld::Standing;
    use crate::worldfield::WorldTerrain;
    use std::sync::Arc;
    // A store (`--store DIR`, or `PLANET_ANET_STORE=DIR`) is read for the bake and the stored coupled run of a production store (seed 1, the viewer's
    // default) instead of baking in memory: the measurements of the exit criteria are made on the shared store.
    let def = planet_world_def::WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, if store.is_some() { 1 } else { seed });
    let world = Arc::new(match &store {
        Some(dir) => WorldTerrain::open_store(def, dir, res)?,
        None => WorldTerrain::open(def, res)?,
    });
    writeln!(
        out,
        "A-net chunk scene: bake {res} cells per face edge{}, coupled run {} steps",
        store.as_ref().map_or(String::new(), |d| format!(" of the store {}", d.display())),
        crate::coupledworld::COUPLED_STEPS
    )
    .map_err(err)?;
    world.request_coupled();
    let maps = loop {
        match world.coupled_standing() {
            Standing::Ready(m, _) => break m,
            Standing::Failed(e) => return Err(e),
            _ => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    // `PLANET_ANET_MIN_HEIGHT_M` keeps the river inland (the look review's anchor takes 150 m: the largest river over the sea is a river mouth).
    let min_height_m: f32 = std::env::var("PLANET_ANET_MIN_HEIGHT_M").ok().and_then(|r| r.parse().ok()).unwrap_or(0.0);
    let lat_c = maps.lattice();
    let rec = maps.receivers();
    let mut best: Option<(usize, f32)> = None;
    for (c, &r) in rec.iter().enumerate() {
        let a = maps.network_area_m2(c);
        if maps.z(c) > min_height_m && r != planet_generators::coupled::NONE && best.is_none_or(|(_, b)| a > b) {
            best = Some((c, a));
        }
    }
    let (cell, area) = best.ok_or("the coupled run has no land river")?;
    let tile = planet_core::TileId::from_direction(
        &planet_core::cube::TangentWarp,
        lat_c.direction(cell),
        planet_generators::chunk::CHUNK_GRID_LEVEL,
    );
    writeln!(out, "chunk {:#018x} (face {}), the river of {:.0} km2 at its coarse cell", tile.raw(), tile.face().0, f64::from(area) / 1e6)
        .map_err(err)?;
    Ok((world, maps, tile))
}

/// The steps of the A-net chunk against the parent, per tile level (criterion 3): for each tile level `L` from 11 to 16, over the vertices of the
/// tiles of level `L - 1` inside the chunk, the difference of the change a tile of level `L` reads and the one its parent reads (the base for the
/// level above the pyramid's top), as p99 and maximum in metres, and the fade time a limit of 0.25 m per frame at 60 frames needs.
fn anet_steps(
    world: &std::sync::Arc<crate::worldfield::WorldTerrain>,
    _maps: &std::sync::Arc<planet_generators::coupled::CoupledMaps>,
    stage: &planet_generators::anet::ANet,
    lat: &planet_generators::chunkfield::ChunkLattice,
    tile: planet_core::TileId,
    out: &mut dyn Write,
) -> Result<(), String> {
    use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField};
    let radius = world.radius_m();
    let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
    let r = compute_chunk(lat, stage, radius, top, tile, halo_rings(stage)).ok_or("the chunk for the steps was refused")?;
    let base = world.base_field()?;
    let field = ChunkField::new(base, *lat, radius, stage).with_top_level(top).with_ready(std::sync::Arc::new(r));
    let map = &planet_core::cube::TangentWarp;
    let res = planet_generators::chunk::TILE_CELLS;
    writeln!(out, "steps against the parent (tile level L vs its parent, at the parent's vertices, inside the chunk), 8 m cells:")
        .map_err(err)?;
    for level in top..=top + 5 {
        let mut tiles = vec![tile];
        for _ in top..level.saturating_sub(1).max(top) {
            tiles = tiles.iter().flat_map(|t| t.children().expect("children")).collect();
        }
        let mut d: Vec<f64> = Vec::new();
        let mut abs: Vec<f64> = Vec::new();
        for t in &tiles {
            for l in 0..=res {
                for k in 0..=res {
                    let dir = t.sample_direction(map, res, k, l);
                    let own = field.change_at_level(dir, level);
                    abs.push(own.abs());
                    let parent = if level == top { 0.0 } else { field.change_at_level(dir, level - 1) };
                    d.push((own - parent).abs());
                }
            }
        }
        d.sort_by(f64::total_cmp);
        abs.sort_by(f64::total_cmp);
        let (p99, max) = (d[d.len() * 99 / 100], d[d.len() - 1]);
        let abs99 = abs[abs.len() * 99 / 100];
        writeln!(
            out,
            "  level {level}: against the parent p99 {p99:.3} m, max {max:.3} m (the change itself at this level: p99 {abs99:.3} m) over {} vertices, fade of the max at 0.25 m per frame, 60 fps: {:.2} s",
            d.len(),
            max / 15.0
        )
        .map_err(err)?;
    }
    // The entry step of the A-net's own top level: a pyramid reaching up to ANET_TOP_LEVEL only, so the parent of a tile of that level is the
    // base (no pyramid above it): the change a tile of the top level reads, at the vertices of the tiles of the level above.
    let entry = crate::anetworld::ANET_TOP_LEVEL;
    let r = compute_chunk(lat, stage, radius, entry, tile, halo_rings(stage)).ok_or("the chunk for the entry step was refused")?;
    let field = ChunkField::new(world.base_field()?, *lat, radius, stage).with_top_level(entry).with_ready(std::sync::Arc::new(r));
    let mut tiles = vec![tile];
    for _ in top..entry - 1 {
        tiles = tiles.iter().flat_map(|t| t.children().expect("children")).collect();
    }
    let mut d: Vec<f64> = Vec::new();
    let mut signed: Vec<f64> = Vec::new();
    for t in &tiles {
        for l in 0..=res {
            for k in 0..=res {
                let c = field.change_at_level(t.sample_direction(map, res, k, l), entry);
                signed.push(c);
                d.push(c.abs());
            }
        }
    }
    signed.sort_by(f64::total_cmp);
    let q = |f: usize| signed[(signed.len() - 1) * f / 100];
    writeln!(
        out,
        "the change at the entry level, signed: p0 {:.2}, p1 {:.2}, p10 {:.2}, p50 {:.2}, p90 {:.2}, p99 {:.2}, p100 {:.2} m",
        q(0),
        q(1),
        q(10),
        q(50),
        q(90),
        q(99),
        q(100)
    )
    .map_err(err)?;
    d.sort_by(f64::total_cmp);
    writeln!(
        out,
        "entry step at the A-net top level {entry} against the base (pyramid up to {entry} only): p99 {:.3} m, max {:.3} m over {} vertices",
        d[d.len() * 99 / 100],
        d[d.len() - 1],
        d.len()
    )
    .map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-042
    #[test]
    fn the_coupled_budget_is_ten_minutes_and_a_small_run_is_deterministic() {
        assert!(coupled_budget_failures(599.9).is_empty());
        assert_eq!(coupled_budget_failures(600.1).len(), 1);
        assert_eq!(coupled_budget_failures(f64::NAN).len(), 1, "a missing time never passes");
        let (secs, a) = time_coupled(8, 3);
        let (_, b) = time_coupled(8, 3);
        assert!(secs.is_finite() && secs >= 0.0);
        assert_eq!(a.steps.len(), 3);
        assert!(a.z.iter().zip(&b.z).all(|(x, y)| x.to_bits() == y.to_bits()), "the fixed input gives the same run");
    }

    // spec: GEN-037
    /// The steps of the A-net chunk against the parent at the tile levels 11 to 16 (criterion 3), printed: `cargo test -p planet --release --lib
    /// anet_steps_report -- --ignored --nocapture`. A measurement of values, not of time.
    #[test]
    #[ignore = "a measurement: it prints the steps of one real chunk"]
    fn anet_steps_report() {
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, maps, tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let lat = planet_generators::chunkfield::ChunkLattice::new(crate::anetworld::cells_per_chunk_for(radius, 8.0)).unwrap();
        let stage = planet_generators::anet::ANet {
            run: maps.clone(),
            base: std::sync::Arc::new(crate::anetworld::WorldBase::new(&world)),
            radius_m: radius,
            params: {
                let mut params = planet_generators::anet::ANetParams::spike(crate::anetworld::ANET_HALO_M, lat.cell_m(radius));
                params.drivers.seed = world.base_seed();
                params
            },
            memo: Default::default(),
        };
        anet_steps(&world, &maps, &stage, &lat, tile, &mut out).unwrap();
    }

    // spec: TEST-064
    /// The steps of the base field alone (no chunk stage) between a tile level and its parent (0 by construction at shared vertices: a child tile's vertices include the parent's and the base height does not depend on the level; the measurement guards that) at the levels 11 to 16, over the vertices of the
    /// parents inside the same chunk as `anet_steps_report`, printed: `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib
    /// base_steps_report -- --ignored --nocapture`. A measurement of values, read only.
    #[test]
    #[ignore = "a measurement: it prints the steps of the base field in one real chunk"]
    fn base_steps_report() {
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let base = world.base_field().unwrap();
        let map = &planet_core::cube::TangentWarp;
        let cells = planet_generators::chunk::TILE_CELLS;
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        writeln!(
            out,
            "steps of the base field alone against the parent (tile level L vs L - 1, at the parent's vertices, inside the chunk):"
        )
        .unwrap();
        for level in top + 1..=top + 5 {
            let mut tiles = vec![tile];
            for _ in top..level - 1 {
                tiles = tiles.iter().flat_map(|t| t.children().expect("children")).collect();
            }
            let mut d: Vec<f64> = Vec::new();
            for t in &tiles {
                for l in 0..=cells {
                    for k in 0..=cells {
                        let dir = t.sample_direction(map, cells, k, l);
                        d.push((base.height_at_level(dir, level) - base.height_at_level(dir, level - 1)).abs());
                    }
                }
            }
            d.sort_by(f64::total_cmp);
            writeln!(
                out,
                "  level {level}: base field against the parent p99 {:.3} m, max {:.3} m over {} vertices",
                d[d.len() * 99 / 100],
                d[d.len() - 1],
                d.len()
            )
            .unwrap();
        }
    }

    // spec: GEN-037
    /// The change the A-net chunks make, by tile level, over a sample of land chunks of the store (the stated bound of the stage, GEN-039): the
    /// maximum and the p99.9 of its absolute value at each level 11 to 16, and the signed extremes. `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N
    /// PLANET_ANET_CHUNKS=48 cargo test -p planet --release --lib anet_change_report -- --ignored --nocapture`. A measurement of values, read only.
    #[test]
    #[ignore = "a measurement: it solves a sample of real chunks"]
    fn anet_change_report() {
        use planet_core::hash::SplitMix64;
        use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField};
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let count: usize = std::env::var("PLANET_ANET_CHUNKS").ok().and_then(|r| r.parse().ok()).unwrap_or(48);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, maps, _) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let lat = planet_generators::chunkfield::ChunkLattice::new(crate::anetworld::cells_per_chunk_for(radius, 8.0)).unwrap();
        let stage = planet_generators::anet::ANet {
            run: maps.clone(),
            base: std::sync::Arc::new(crate::anetworld::WorldBase::new(&world)),
            radius_m: radius,
            params: {
                let mut params = planet_generators::anet::ANetParams::spike(crate::anetworld::ANET_HALO_M, lat.cell_m(radius));
                params.drivers.seed = world.base_seed();
                params
            },
            memo: Default::default(),
        };
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        let per_face = 1u32 << top;
        let base = world.base_field().unwrap();
        // A sample of land chunks: random tiles of the chunk grid whose centre is above sea level, the same ones on every run.
        let mut rng = SplitMix64(0x6368_616e_6765);
        let mut tiles: Vec<planet_core::TileId> = Vec::new();
        while tiles.len() < count {
            let face = planet_core::Face::ALL[(rng.next_u64() % 6) as usize];
            let (x, y) = ((rng.next_u64() % u64::from(per_face)) as u32, (rng.next_u64() % u64::from(per_face)) as u32);
            let t = planet_core::TileId::new(face, top, x, y).expect("a tile of the chunk grid");
            if base.height(t.center_direction(&planet_core::cube::TangentWarp)) > 0.0 {
                tiles.push(t);
            }
        }
        let map = &planet_core::cube::TangentWarp;
        let samples = 65u32;
        // Per level: every absolute change seen, and the signed extremes.
        let levels: Vec<u8> = (top..=top + 5).collect();
        let mut abs: Vec<Vec<f64>> = vec![Vec::new(); levels.len()];
        let mut lo_hi: Vec<(f64, f64)> = vec![(0.0, 0.0); levels.len()];
        let mut worst: Vec<(f64, planet_core::TileId)> = vec![(0.0, tiles[0]); levels.len()];
        for (n, &tile) in tiles.iter().enumerate() {
            let Some(r) = compute_chunk(&lat, &stage, radius, top, tile, halo_rings(&stage)) else { continue };
            let field =
                ChunkField::new(world.base_field().unwrap(), lat, radius, &stage).with_top_level(top).with_ready(std::sync::Arc::new(r));
            for (i, &level) in levels.iter().enumerate() {
                for l in 0..samples {
                    for k in 0..samples {
                        let c = field.change_at_level(tile.sample_direction(map, samples - 1, k, l), level);
                        abs[i].push(c.abs());
                        lo_hi[i].0 = lo_hi[i].0.min(c);
                        lo_hi[i].1 = lo_hi[i].1.max(c);
                        if c.abs() > worst[i].0 {
                            worst[i] = (c.abs(), tile);
                        }
                    }
                }
            }
            writeln!(out, "chunk {}/{} {:?} done", n + 1, tiles.len(), (tile.face().0, tile.x(), tile.y())).map_err(err).unwrap();
        }
        writeln!(out, "the change of the A-net chunks over {} land chunks of the store, {samples} x {samples} samples each:", tiles.len())
            .map_err(err)
            .unwrap();
        for (i, level) in levels.iter().enumerate() {
            abs[i].sort_by(f64::total_cmp);
            let a = &abs[i];
            let q = |p: f64| a[(((a.len() - 1) as f64) * p) as usize];
            writeln!(
                out,
                "  level {level}: |change| p50 {:.2}, p99 {:.2}, p99.9 {:.2}, p99.99 {:.2}, max {:.2} m (signed {:.1} to {:.1} m; the largest at {:?})",
                q(0.5),
                q(0.99),
                q(0.999),
                q(0.9999),
                a[a.len() - 1],
                lo_hi[i].0,
                lo_hi[i].1,
                (worst[i].1.face().0, worst[i].1.x(), worst[i].1.y())
            )
            .map_err(err)
            .unwrap();
        }
    }

    // spec: GEN-052
    /// What building one tile costs with the chunk layer (chunks ready under it, an edit in the definition) and without it (base and stamps only),
    /// per level (M8-7, criterion 6: the tile refresh of an edit). `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib
    /// tile_cost_report -- --ignored --nocapture`. Times of one thread, a probe, not a perf measurement.
    #[test]
    #[ignore = "a measurement: it solves two real chunks and builds tiles"]
    fn tile_cost_report() {
        use planet_generators::chunkfield::compute_chunk;
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let _ = compute_chunk;
        // The viewer's chunk stage on this world, the chunks around the site landed on a manual spawner.
        let spawner = std::sync::Arc::new(planet_streaming::ThreadPool::new(6));
        let cfg = planet_streaming::chunks::ChunkConfig { workers: 6, ..Default::default() };
        assert_eq!(
            world.enable_anet_from_store_with(
                crate::anetworld::ANET_CELL_M,
                spawner,
                std::sync::Arc::new(planet_streaming::SystemClock::new()),
                cfg,
                false
            ),
            Ok(true)
        );
        let manager = world.chunk_manager().unwrap();
        let map = &planet_core::cube::TangentWarp;
        let site = tile.center_direction(map);
        let t15 = planet_core::TileId::from_direction(map, site, 15);
        let camera = planet_core::PlanetFixed(site * world.radius_m());
        let start = std::time::Instant::now();
        while manager.in_flight() > 0 || manager.result_hash(manager.reads(t15)[0]).is_none() {
            world.chunk_frame(&[t15], camera, &[]);
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert!(start.elapsed().as_secs() < 300, "the chunks did not land");
        }
        let def = world.definition();
        let cells = 64u32;
        fn time<'a>(f: &dyn Fn() -> Box<dyn planet_generators::height::HeightField + 'a>, id: planet_core::TileId, cells: u32) -> f64 {
            let map = &planet_core::cube::TangentWarp;
            // The mesh path of a real tile build (batch heights and climate over the lattice and its ring), as the streamer's job runs it.
            let t = std::time::Instant::now();
            for _ in 0..3 {
                let field = f();
                std::hint::black_box(planet_generators::mesh::tile_mesh_field(&*field, map, id, cells, 6_371_000.0));
            }
            t.elapsed().as_secs_f64() * 1000.0 / 3.0
        }
        for level in [15u8, 13, 11, 9, 7, 5, 4, 3] {
            let id = planet_core::TileId::from_direction(map, site, level);
            let with = time(&|| world.field().unwrap(), id, cells);
            let without = time(&|| planet_generators::chain::with_stamps(world.base_field().unwrap(), &def).unwrap(), id, cells);
            writeln!(out, "tile level {level} ({cells} cells, mesh path): {with:.0} ms with the chunk layer, {without:.0} ms without")
                .map_err(err)
                .unwrap();
        }
    }
    // spec: GEN-052
    /// What one `chunk_frame` call costs the calling thread (the main thread of the window) while the camera moves over new chunks, without and
    /// with an item in a stamp layer under the chunk stage (M8 owner test: stalls of 1 to 15 s in the editor's chunk frame). `PLANET_ANET_STORE=DIR
    /// PLANET_ANET_RES=N cargo test -p planet --release --lib chunk_frame_cost_report -- --ignored --nocapture`. Times of the call only, a probe.
    #[test]
    #[ignore = "a measurement: it flies over chunks and times the chunk frame"]
    fn chunk_frame_cost_report() {
        use planet_core::cube::TangentWarp;
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let spawner = std::sync::Arc::new(planet_streaming::ThreadPool::new(6));
        let cfg = planet_streaming::chunks::ChunkConfig { workers: 6, ..Default::default() };
        assert_eq!(
            world.enable_anet_from_store_with(
                crate::anetworld::ANET_CELL_M,
                spawner,
                std::sync::Arc::new(planet_streaming::SystemClock::new()),
                cfg,
                false
            ),
            Ok(true)
        );
        let radius = world.radius_m();
        let site = tile.center_direction(&TangentWarp);
        let east = planet_core::Vec3::new(0.0, 0.0, 1.0).cross(site).normalized();
        let north = site.cross(east).normalized();
        // 3 x 3 level-15 tiles round a point of the path: what a low camera wants.
        let tiles_at = |dir: planet_core::Vec3| -> Vec<planet_core::TileId> {
            use planet_core::Side::{East, North, South, West};
            let c = planet_core::TileId::from_direction(&TangentWarp, dir, 15);
            let mut t = vec![c];
            for side in [East, West, North, South] {
                t.push(c.neighbor(&TangentWarp, side));
            }
            for (a, b) in [(East, North), (East, South), (West, North), (West, South)] {
                t.push(c.neighbor(&TangentWarp, a).neighbor(&TangentWarp, b));
            }
            t
        };
        let fly = |label: &str, origin: planet_core::Vec3, along: planet_core::Vec3, out: &mut dyn Write| {
            let mut times = Vec::new();
            for k in 0..30 {
                let dir = (origin * libm::cos(k as f64 * 3_000.0 / radius) + along * libm::sin(k as f64 * 3_000.0 / radius)).normalized();
                let tiles = tiles_at(dir);
                let camera = planet_core::PlanetFixed(dir * radius);
                let t = std::time::Instant::now();
                world.chunk_frame(&tiles, camera, &[]);
                times.push(t.elapsed().as_secs_f64() * 1000.0);
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            let max = times.iter().cloned().fold(0.0, f64::max);
            let over: Vec<String> = times.iter().filter(|t| **t > 20.0).map(|t| format!("{t:.0}")).collect();
            writeln!(out, "{label}: 30 frames 3 km apart, max {max:.0} ms, frames over 20 ms: [{}]", over.join(", ")).map_err(err).unwrap();
        };
        fly("no items, east", site, east, &mut out);
        let mut def = (*world.definition()).clone();
        let item = planet_world_def::ItemSpec {
            id: 1,
            layer: "stamps".into(),
            priority: 0,
            dir: site,
            content_hash: 0,
            reach_m: 500.0,
            max_delta_m: 60.0,
            stamp: None,
        }
        .with_stamp(planet_world_def::stamp::Stamp::new(planet_world_def::stamp::StampKind::Hill, [30.0, 0.0, 0.0]));
        def.put_item_spec(planet_world_def::BodyId::PLANET, item).map_err(|e| format!("{e:?}")).unwrap();
        world.set_definition(std::sync::Arc::new(def)).unwrap();
        fly("one hill in the stamp layer, north (new chunks)", site, north, &mut out);
        fly("one hill in the stamp layer, west (new chunks)", site, east * -1.0, &mut out);
    }

    // spec: VIEW-056
    /// What the editor's hover costs a frame (`pick_surface` through the world's field, once a frame under the cursor) with the camera at several
    /// altitudes over the site, cursors on a grid across the viewport (M8 owner test: editor frames of up to 1.3 s at the full-planet view).
    /// `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib pick_cost_report -- --ignored --nocapture`. One thread, a probe.
    #[test]
    #[ignore = "a measurement: it ray-marches the field under a grid of cursors"]
    fn pick_cost_report() {
        use planet_core::cube::TangentWarp;
        use planet_frame::pick::{cursor_ray, pick_surface, PickLimits};
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let site = tile.center_direction(&TangentWarp);
        let field = world.field().unwrap();
        let limits = PickLimits::new(radius, planet_frame::LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()));
        // The first height reads of a field made now, and of one made after an edit of the definition (what the window's ground height under the camera
        // meets on the first frame after either).
        let first_reads = |label: &str, field: &dyn planet_generators::height::HeightField, out: &mut dyn Write| {
            let times: Vec<String> = (0..4)
                .map(|k| {
                    let d = (site + planet_core::Vec3::new(0.0, 0.0, 1.0) * (f64::from(k) * 0.01)).normalized();
                    let t = std::time::Instant::now();
                    std::hint::black_box(field.height(d));
                    format!("{:.1}", t.elapsed().as_secs_f64() * 1000.0)
                })
                .collect();
            writeln!(out, "{label}: first four height reads (ms): [{}]", times.join(", ")).map_err(err).unwrap();
        };
        first_reads("a field made now", &*field, &mut out);
        let mut def = (*world.definition()).clone();
        let item = planet_world_def::ItemSpec {
            id: 1,
            layer: "stamps".into(),
            priority: 0,
            dir: site,
            content_hash: 0,
            reach_m: 500.0,
            max_delta_m: 60.0,
            stamp: None,
        }
        .with_stamp(planet_world_def::stamp::Stamp::new(planet_world_def::stamp::StampKind::Hill, [30.0, 0.0, 0.0]));
        def.put_item_spec(planet_world_def::BodyId::PLANET, item).map_err(|e| format!("{e:?}")).unwrap();
        world.set_definition(std::sync::Arc::new(def)).unwrap();
        let edited = world.field().unwrap();
        first_reads("a field made after an edit", &*edited, &mut out);
        let (w, h) = (2560.0, 1440.0);
        for altitude in [20_000_000.0, 5_000_000.0, 1_000_000.0, 100_000.0, 10_000.0] {
            let position = planet_core::PlanetFixed(site * (radius + altitude));
            let camera = planet_frame::Camera::at_surface_point(
                position,
                planet_core::Vec3::new(0.0, 0.0, 1.0),
                90f64.to_radians(),
                1.0,
                w / h,
                0.5,
            );
            let (mut worst, mut sum, mut n, mut hits) = (0.0f64, 0.0f64, 0u32, 0u32);
            for gx in 0..9 {
                for gy in 0..5 {
                    let ray = cursor_ray(&camera, w * (f64::from(gx) + 0.5) / 9.0, h * (f64::from(gy) + 0.5) / 5.0, w, h);
                    let calls = std::cell::Cell::new(0u64);
                    let t = std::time::Instant::now();
                    let hit = pick_surface(&ray, &limits, &|d| {
                        calls.set(calls.get() + 1);
                        field.height(d)
                    });
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    worst = worst.max(ms);
                    sum += ms;
                    n += 1;
                    hits += u32::from(hit.is_some());
                    if ms > 100.0 {
                        writeln!(
                            out,
                            "    slow pick at cursor ({gx},{gy}): {ms:.0} ms, {} height calls, hit {}",
                            calls.get(),
                            hit.is_some()
                        )
                        .map_err(err)
                        .unwrap();
                    }
                }
            }
            writeln!(out, "camera {altitude:.0} m up: {n} picks, {hits} hits, mean {:.1} ms, worst {worst:.0} ms", sum / f64::from(n))
                .map_err(err)
                .unwrap();
            // The same picks with the height of the tile level whose vertex spacing is about a pixel's footprint at the camera's distance to the surface.
            let (mut worst_l, mut sum_l, mut hits_l) = (0.0f64, 0.0f64, 0u32);
            let footprint_m = altitude * 2.0 * libm::tan(45f64.to_radians()) / h;
            let level =
                (libm::ceil(libm::log2(radius * core::f64::consts::FRAC_PI_2 / 32.0 / footprint_m.max(1e-3))).clamp(3.0, 22.0)) as u8;
            for gx in 0..9 {
                for gy in 0..5 {
                    let ray = cursor_ray(&camera, w * (f64::from(gx) + 0.5) / 9.0, h * (f64::from(gy) + 0.5) / 5.0, w, h);
                    let t = std::time::Instant::now();
                    let hit = pick_surface(&ray, &limits, &|d| field.height_at_level(d, level));
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    worst_l = worst_l.max(ms);
                    sum_l += ms;
                    hits_l += u32::from(hit.is_some());
                }
            }
            writeln!(out, "  at level {level}: {hits_l} hits, mean {:.1} ms, worst {worst_l:.0} ms", sum_l / f64::from(n))
                .map_err(err)
                .unwrap();
        }
    }

    // spec: GEN-052
    #[test]
    fn the_chunk_budget_is_four_seconds_at_8_m_and_a_missing_time_never_passes() {
        assert!(anet_chunk_budget_failures(8.0, 3.9).is_empty());
        assert_eq!(anet_chunk_budget_failures(8.0, 4.1).len(), 1);
        assert_eq!(anet_chunk_budget_failures(8.0, f64::NAN).len(), 1);
        assert!(anet_chunk_budget_failures(4.0, 41.0).is_empty(), "the times at 4 and 10 m are recorded, not budgeted");
    }

    // spec: GEN-042
    #[test]
    fn the_coupled_scene_takes_the_global_res() {
        let ok = run_str(&["test-render", "--perf", "--scene", "coupled", "--global-res", "16", "--smoke"]);
        assert!(ok.is_ok(), "{ok:?}");
    }

    fn run_str(args: &[&str]) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut buf = Vec::new();
        crate::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
    }

    // spec: GEN-014
    #[test]
    fn the_scene_takes_the_options_xtask_forwards_and_other_runs_still_refuse_them() {
        let ok = run_str(&["test-render", "--perf", "--scene", "global-bake", "--global-res", "1024", "--threads", "2", "--smoke"]);
        assert!(ok.is_ok(), "{ok:?}");
        for args in [&["test-render", "--global-res", "8"][..], &["test-render", "--perf", "--threads", "2"], &["editor", "--threads", "2"]]
        {
            let e = run_str(args).unwrap_err();
            assert!(e.contains("only applies to `generate`"), "{args:?}: {e}");
        }
    }

    // spec: GEN-014
    #[test]
    fn the_budgets_are_ten_minutes_and_eight_gigabytes() {
        assert!(budget_failures(599.9, 8 << 30).is_empty());
        assert_eq!(budget_failures(600.1, 0).len(), 1);
        assert_eq!(budget_failures(1.0, (8 << 30) + 1).len(), 1);
        assert_eq!(budget_failures(f64::NAN, 0).len(), 1, "a missing time never passes");
    }

    /// SPIKE-61 (spike code, never merged): M9 R3 bake pre-run. Tiles of one cube face at about 100 m (98,304 cells per face edge, 48 per coupled
    /// cell), each sampled from the store's field, routed on its own ground (priority flood, steepest descent on the filled surface, D8 area with
    /// the coupled network's inflow at the window's edge) and carved by implicit stream power capped by local relief; timing per phase and the
    /// deflated size of the carve delta. `PLANET_ANET_STORE=DIR PLANET_ANET_RES=16384 PRERUN_FACE=f PRERUN_STRIDE=k PRERUN_PAR=p PRERUN_STEPS=s
    /// PRERUN_OUT=FILE cargo test -p planet --release --lib bake_prerun_report -- --ignored --nocapture`. Read only on the store.
    #[test]
    #[ignore = "a spike measurement"]
    fn bake_prerun_report() {
        use planet_core::cube::{face_to_direction, Face, TangentWarp};
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        use std::io::Write as _;
        use std::time::Instant;
        let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let face = env("PRERUN_FACE", 0);
        let stride = env("PRERUN_STRIDE", 8);
        let par = env("PRERUN_PAR", 4);
        let sample_threads = env("PRERUN_SAMPLE_THREADS", 4);
        let steps = env("PRERUN_STEPS", 10);
        let max_tiles = env("PRERUN_MAX_TILES", usize::MAX);
        let offset = env("PRERUN_OFFSET", 0);
        let out_path = std::env::var("PRERUN_OUT").unwrap_or_else(|_| "../../target/spike61/prerun.txt".into());
        let dump_dir = std::env::var("PRERUN_DUMP_DIR").ok();
        let dump: Vec<usize> = std::env::var("PRERUN_DUMP").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        let res = env("PLANET_ANET_RES", 256) as u32;
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let mut so = std::io::stdout();
        let (world, maps, _tile) = anet_scene(res, 11, store, &mut so).unwrap();
        let field = world.field().unwrap();
        let lat_c = maps.lattice();
        let nc = lat_c.n();
        const PER: usize = 48; // fine cells per coupled cell
        let m = nc * PER; // fine cells per face edge
        const CORE: usize = 3072; // 64 coupled cells
        const HALO: usize = 256;
        let tiles_per_edge = m / CORE;
        let rec_c = maps.receivers();
        let fc0 = face * nc * nc;
        // Tiles: every `stride`-th along both axes, from `offset`.
        let mut tiles: Vec<(usize, usize)> = Vec::new();
        for tj in (offset..tiles_per_edge).step_by(stride) {
            for ti in (offset..tiles_per_edge).step_by(stride) {
                tiles.push((ti, tj));
            }
        }
        tiles.truncate(max_tiles);
        let mut out = std::fs::File::create(&out_path).unwrap();
        writeln!(out, "face {face} fine {m} per edge ({:.1} m mean), core {CORE} halo {HALO}, {} tiles of {} per face, stride {stride}, par {par}, sample threads {sample_threads}, steps {steps}", 2.0 * std::f64::consts::FRAC_PI_4 * 2.0 * world.radius_m() / m as f64 / 2.0, tiles.len(), tiles_per_edge * tiles_per_edge).unwrap();
        // Land tiles of every face from the coupled run (a tile with any coupled cell above 0 in its window counts).
        for f in 0..6 {
            let (mut any, mut frac) = (0usize, 0.0f64);
            let cpt = CORE / PER;
            for tj in 0..tiles_per_edge {
                for ti in 0..tiles_per_edge {
                    let mut land = 0usize;
                    for cj in tj * cpt..(tj + 1) * cpt {
                        for ci in ti * cpt..(ti + 1) * cpt {
                            land += usize::from(maps.z(f * nc * nc + cj * nc + ci) > 0.0);
                        }
                    }
                    any += usize::from(land > 0);
                    frac += land as f64 / (cpt * cpt) as f64;
                }
            }
            writeln!(out, "face {f}: {any} of {} tiles hold land; land share {:.3}", tiles_per_edge * tiles_per_edge, frac / (tiles_per_edge * tiles_per_edge) as f64).unwrap();
        }
        writeln!(out, "tile ti tj land_frac skip dirs_s sample_s route_s carve_s total_s max_cut_m p50_cut_chan_m chan_cells core_cells delta_bytes h16_bytes inflow_cells").unwrap();
        out.flush().unwrap();
        let out = std::sync::Mutex::new(out);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let dir_of = |i: usize, j: usize| {
            face_to_direction(&TangentWarp, Face(face as u8), -1.0 + (2.0 * i as f64 + 1.0) / m as f64, -1.0 + (2.0 * j as f64 + 1.0) / m as f64)
        };
        std::thread::scope(|sc| {
            for _ in 0..par {
                sc.spawn(|| loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if k >= tiles.len() {
                        break;
                    }
                    let (ti, tj) = tiles[k];
                    let t_all = Instant::now();
                    let (ci0, cj0) = (ti * CORE, tj * CORE);
                    let wi0 = ci0.saturating_sub(HALO);
                    let wj0 = cj0.saturating_sub(HALO);
                    let wi1 = (ci0 + CORE + HALO).min(m);
                    let wj1 = (cj0 + CORE + HALO).min(m);
                    let (w, hgt) = (wi1 - wi0, wj1 - wj0);
                    // Ocean tiles (every coupled cell of the window at or below 0) are skipped.
                    let (cwi0, cwj0, cwi1, cwj1) = (wi0 / PER, wj0 / PER, wi1.div_ceil(PER), wj1.div_ceil(PER));
                    let mut land = 0usize;
                    let mut tot = 0usize;
                    for cj in cwj0..cwj1 {
                        for ci in cwi0..cwi1 {
                            tot += 1;
                            land += usize::from(maps.z(fc0 + cj * nc + ci) > 0.0);
                        }
                    }
                    let land_frac = land as f64 / tot as f64;
                    if land == 0 {
                        writeln!(out.lock().unwrap(), "{k} {ti} {tj} {land_frac:.3} 1 0 0 0 0 {:.3} 0 0 0 {} 0 0 0", t_all.elapsed().as_secs_f64(), CORE * CORE).unwrap();
                        continue;
                    }
                    let n = w * hgt;
                    let t = Instant::now();
                    let mut dirs = Vec::with_capacity(n);
                    for j in wj0..wj1 {
                        for i in wi0..wi1 {
                            dirs.push(dir_of(i, j));
                        }
                    }
                    let dirs_s = t.elapsed().as_secs_f64();
                    let t = Instant::now();
                    let orig: Vec<f32> = field.heights_threaded(&dirs, sample_threads).into_iter().map(|h| h as f32).collect();
                    let sample_s = t.elapsed().as_secs_f64();
                    // Local spacing at the window's centre.
                    let (mi, mj) = ((wi0 + wi1) / 2, (wj0 + wj1) / 2);
                    let dx = world.radius_m() * libm::acos(dir_of(mi, mj).dot(dir_of(mi + 1, mj)).min(1.0));
                    let cell_a = dx * dx;
                    drop(dirs);
                    // Inflow: coupled cells outside the window whose receiver lies inside it, at the window cell nearest them.
                    let mut inflow: Vec<(usize, f64)> = Vec::new();
                    let margin = 3;
                    for cj in cwj0.saturating_sub(margin)..(cwj1 + margin).min(nc) {
                        for ci in cwi0.saturating_sub(margin)..(cwi1 + margin).min(nc) {
                            let c = fc0 + cj * nc + ci;
                            let r = rec_c[c];
                            if r == planet_generators::coupled::NONE || (r as usize) / (nc * nc) != face {
                                continue;
                            }
                            let (fi, fj) = (ci * PER + PER / 2, cj * PER + PER / 2);
                            let inside = |a: usize, b: usize| a >= wi0 && a < wi1 && b >= wj0 && b < wj1;
                            let rr = r as usize - fc0;
                            let (ri, rj) = ((rr % nc) * PER + PER / 2, (rr / nc) * PER + PER / 2);
                            if !inside(fi, fj) && inside(ri, rj) && maps.z(c) > 0.0 {
                                let (ei, ej) = (fi.clamp(wi0, wi1 - 1) - wi0, fj.clamp(wj0, wj1 - 1) - wj0);
                                inflow.push((ej * w + ei, maps.network_area_m2(c) as f64));
                            }
                        }
                    }
                    // Cap: half the 5 km block relief, at most 100 m.
                    let blk = (5000.0 / dx).round().max(1.0) as usize;
                    let (bw, bh) = (w.div_ceil(blk), hgt.div_ceil(blk));
                    let mut bmin = vec![f32::MAX; bw * bh];
                    let mut bmax = vec![f32::MIN; bw * bh];
                    for j in 0..hgt {
                        for i in 0..w {
                            let b = (j / blk) * bw + i / blk;
                            let h = orig[j * w + i];
                            bmin[b] = bmin[b].min(h);
                            bmax[b] = bmax[b].max(h);
                        }
                    }
                    let cap = |p: usize| -> f32 {
                        let b = ((p / w) / blk) * bw + (p % w) / blk;
                        (0.5 * (bmax[b] - bmin[b])).min(100.0)
                    };
                    let key = |h: f32| -> u32 {
                        let b = h.to_bits();
                        if b & 0x8000_0000 != 0 { !b } else { b | 0x8000_0000 }
                    };
                    const NB: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
                    let mut h = orig.clone();
                    let mut route_s = 0.0;
                    let mut carve_s = 0.0;
                    let kdt = 0.01f64;
                    let mut filled = vec![0f32; n];
                    let mut order: Vec<u32> = Vec::with_capacity(n);
                    let mut rec = vec![u32::MAX; n];
                    let mut area = vec![0f64; n];
                    let mut seen = vec![false; n];
                    for _step in 0..steps {
                        let t = Instant::now();
                        // Priority flood with epsilon from the window border and the sea.
                        order.clear();
                        seen.iter_mut().for_each(|s| *s = false);
                        let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
                        for p in 0..n {
                            let (i, j) = (p % w, p / w);
                            if i == 0 || j == 0 || i == w - 1 || j == hgt - 1 || h[p] <= 0.0 {
                                filled[p] = h[p];
                                seen[p] = true;
                                heap.push(Reverse((key(h[p]), p as u32)));
                            }
                        }
                        while let Some(Reverse((_, p))) = heap.pop() {
                            let p = p as usize;
                            order.push(p as u32);
                            let (i, j) = ((p % w) as i64, (p / w) as i64);
                            for (di, dj) in NB {
                                let (a, b) = (i + di, j + dj);
                                if a < 0 || b < 0 || a >= w as i64 || b >= hgt as i64 {
                                    continue;
                                }
                                let q = b as usize * w + a as usize;
                                if seen[q] {
                                    continue;
                                }
                                seen[q] = true;
                                let lo = filled[p] + 1e-3;
                                filled[q] = h[q].max(lo);
                                heap.push(Reverse((key(filled[q]), q as u32)));
                            }
                        }
                        // Steepest descent on the filled surface; outlets have none.
                        for p in 0..n {
                            let (i, j) = ((p % w) as i64, (p / w) as i64);
                            if i == 0 || j == 0 || i == w as i64 - 1 || j == hgt as i64 - 1 || h[p] <= 0.0 {
                                rec[p] = u32::MAX;
                                continue;
                            }
                            let mut best = (0.0f64, u32::MAX);
                            for (k2, (di, dj)) in NB.iter().enumerate() {
                                let q = (j + dj) as usize * w + (i + di) as usize;
                                let d = if k2 < 4 { dx } else { dx * std::f64::consts::SQRT_2 };
                                let s = f64::from(filled[p] - filled[q]) / d;
                                if s > best.0 {
                                    best = (s, q as u32);
                                }
                            }
                            rec[p] = best.1;
                        }
                        area.iter_mut().for_each(|a| *a = cell_a);
                        for &(p, a) in &inflow {
                            area[p] += a;
                        }
                        for &p in order.iter().rev() {
                            let r = rec[p as usize];
                            if r != u32::MAX {
                                area[r as usize] += area[p as usize];
                            }
                        }
                        route_s += t.elapsed().as_secs_f64();
                        // Implicit stream power (n = 1, m = 0.5), outlets first, capped by local relief.
                        let t = Instant::now();
                        for &p in &order {
                            let p = p as usize;
                            let r = rec[p];
                            if r == u32::MAX {
                                continue;
                            }
                            let r = r as usize;
                            let d = if (p % w) == (r % w) || (p / w) == (r / w) { dx } else { dx * std::f64::consts::SQRT_2 };
                            let f = kdt * area[p].sqrt() / d;
                            let hn = ((f64::from(h[p]) + f * f64::from(h[r])) / (1.0 + f)) as f32;
                            h[p] = hn.max(orig[p] - cap(p)).min(h[p]);
                        }
                        carve_s += t.elapsed().as_secs_f64();
                    }
                    // Core statistics and the deflated carve delta (0.25 m steps, u16) and heights (0.5 m, i16).
                    let mut cuts: Vec<f32> = Vec::new();
                    let mut max_cut = 0f32;
                    let mut delta = Vec::with_capacity(CORE * CORE * 2);
                    let mut h16 = Vec::with_capacity(CORE * CORE * 2);
                    let mut core_cells = 0usize;
                    for j in (cj0 - wj0)..(cj0 - wj0 + CORE).min(hgt) {
                        for i in (ci0 - wi0)..(ci0 - wi0 + CORE).min(w) {
                            let p = j * w + i;
                            core_cells += 1;
                            let cut = orig[p] - h[p];
                            max_cut = max_cut.max(cut);
                            if area[p] >= 1e6 {
                                cuts.push(cut);
                            }
                            delta.extend_from_slice(&((cut / 0.25).round().clamp(0.0, 65535.0) as u16).to_le_bytes());
                            h16.extend_from_slice(&((h[p] / 0.5).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes());
                        }
                    }
                    cuts.sort_by(f32::total_cmp);
                    let p50 = cuts.get(cuts.len() / 2).copied().unwrap_or(0.0);
                    let delta_bytes = miniz_oxide::deflate::compress_to_vec(&delta, 6).len();
                    let h16_bytes = miniz_oxide::deflate::compress_to_vec(&h16, 6).len();
                    if let Some(dd) = &dump_dir {
                        if dump.contains(&k) {
                            let wr = |name: &str, v: &[f32]| {
                                let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
                                std::fs::write(format!("{dd}/tile{k}-{name}-{w}x{hgt}.f32"), b).unwrap();
                            };
                            wr("orig", &orig);
                            wr("carved", &h);
                            let a: Vec<f32> = area.iter().map(|&x| x as f32).collect();
                            wr("area", &a);
                        }
                    }
                    writeln!(
                        out.lock().unwrap(),
                        "{k} {ti} {tj} {land_frac:.3} 0 {dirs_s:.2} {sample_s:.2} {route_s:.2} {carve_s:.2} {:.2} {max_cut:.1} {p50:.2} {} {core_cells} {delta_bytes} {h16_bytes} {}",
                        t_all.elapsed().as_secs_f64(),
                        cuts.len(),
                        inflow.len()
                    )
                    .unwrap();
                    out.lock().unwrap().flush().unwrap();
                });
            }
        });
    }

    /// SPIKE-62 (spike code, never merged): R4 routing cost and flow direction, on the tiles of SPIKE-61's grid. Per tile: (a) re-routing every
    /// k of 10 stream-power steps against k = 1; (b) the heap flood against Barnes' improved flood (a plain queue for depression cells) and a
    /// bucket queue at 5 cm, with the topological order from the receivers; (c) D8, jittered D8 and MFD: a straight-run metric of the channels.
    /// `PLANET_ANET_STORE=DIR PLANET_ANET_RES=16384 CADENCE_TILES=f:ti:tj,... CADENCE_PAR=4 CADENCE_OUT=FILE [CADENCE_DUMP_DIR=DIR]
    /// cargo test -p planet --release --lib route_cadence_report -- --ignored --nocapture`. Read only on the store.
    #[test]
    #[ignore = "a spike measurement"]
    fn route_cadence_report() {
        use planet_core::cube::{face_to_direction, Face, TangentWarp};
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, VecDeque};
        use std::io::Write as _;
        use std::time::Instant;
        let par: usize = std::env::var("CADENCE_PAR").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
        let tiles: Vec<(usize, usize, usize)> = std::env::var("CADENCE_TILES")
            .expect("CADENCE_TILES=f:ti:tj,...")
            .split(',')
            .map(|t| {
                let v: Vec<usize> = t.split(':').map(|x| x.parse().unwrap()).collect();
                (v[0], v[1], v[2])
            })
            .collect();
        let out_path = std::env::var("CADENCE_OUT").unwrap();
        let dump_dir = std::env::var("CADENCE_DUMP_DIR").ok();
        let res: u32 = std::env::var("PLANET_ANET_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let mut so = std::io::stdout();
        let (world, maps, _tile) = anet_scene(res, 11, store, &mut so).unwrap();
        let field = world.field().unwrap();
        let nc = maps.lattice().n();
        const PER: usize = 48;
        const CORE: usize = 3072;
        const HALO: usize = 256;
        const STEPS: usize = 10;
        let m = nc * PER;
        let rec_c = maps.receivers();
        let out = std::sync::Mutex::new(std::fs::File::create(&out_path).unwrap());
        writeln!(out.lock().unwrap(), "SPIKE-62: {} tiles, par {par}, {STEPS} steps; rows: tile f ti tj relief5_p50 then per item", tiles.len()).unwrap();
        let next = std::sync::atomic::AtomicUsize::new(0);
        const NB: [(i64, i64); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
        let key = |h: f32| -> u32 {
            let b = h.to_bits();
            if b & 0x8000_0000 != 0 { !b } else { b | 0x8000_0000 }
        };
        std::thread::scope(|sc| {
            for _ in 0..par {
                sc.spawn(|| loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if k >= tiles.len() {
                        break;
                    }
                    let (face, ti, tj) = tiles[k];
                    let fc0 = face * nc * nc;
                    let dir_of = |i: usize, j: usize| {
                        face_to_direction(&TangentWarp, Face(face as u8), -1.0 + (2.0 * i as f64 + 1.0) / m as f64, -1.0 + (2.0 * j as f64 + 1.0) / m as f64)
                    };
                    let (ci0, cj0) = (ti * CORE, tj * CORE);
                    let (wi0, wj0) = (ci0.saturating_sub(HALO), cj0.saturating_sub(HALO));
                    let (wi1, wj1) = ((ci0 + CORE + HALO).min(m), (cj0 + CORE + HALO).min(m));
                    let (w, hgt) = (wi1 - wi0, wj1 - wj0);
                    let n = w * hgt;
                    let mut dirs = Vec::with_capacity(n);
                    for j in wj0..wj1 {
                        for i in wi0..wi1 {
                            dirs.push(dir_of(i, j));
                        }
                    }
                    let orig: Vec<f32> = field.heights_threaded(&dirs, 4).into_iter().map(|h| h as f32).collect();
                    drop(dirs);
                    let (mi, mj) = ((wi0 + wi1) / 2, (wj0 + wj1) / 2);
                    let dx = world.radius_m() * libm::acos(dir_of(mi, mj).dot(dir_of(mi + 1, mj)).min(1.0));
                    let cell_a = dx * dx;
                    let (cwi0, cwj0, cwi1, cwj1) = (wi0 / PER, wj0 / PER, wi1.div_ceil(PER), wj1.div_ceil(PER));
                    let mut inflow: Vec<(usize, f64)> = Vec::new();
                    for cj in cwj0.saturating_sub(3)..(cwj1 + 3).min(nc) {
                        for ci in cwi0.saturating_sub(3)..(cwi1 + 3).min(nc) {
                            let c = fc0 + cj * nc + ci;
                            let r = rec_c[c];
                            if r == planet_generators::coupled::NONE || (r as usize) / (nc * nc) != face {
                                continue;
                            }
                            let (fi, fj) = (ci * PER + PER / 2, cj * PER + PER / 2);
                            let inside = |a: usize, b: usize| a >= wi0 && a < wi1 && b >= wj0 && b < wj1;
                            let rr = r as usize - fc0;
                            let (ri, rj) = ((rr % nc) * PER + PER / 2, (rr / nc) * PER + PER / 2);
                            if !inside(fi, fj) && inside(ri, rj) && maps.z(c) > 0.0 {
                                inflow.push(((fj.clamp(wj0, wj1 - 1) - wj0) * w + fi.clamp(wi0, wi1 - 1) - wi0, maps.network_area_m2(c) as f64));
                            }
                        }
                    }
                    let blk = (5000.0 / dx).round().max(1.0) as usize;
                    let (bw, bh) = (w.div_ceil(blk), hgt.div_ceil(blk));
                    let (mut bmin, mut bmax) = (vec![f32::MAX; bw * bh], vec![f32::MIN; bw * bh]);
                    for p in 0..n {
                        let b = ((p / w) / blk) * bw + (p % w) / blk;
                        bmin[b] = bmin[b].min(orig[p]);
                        bmax[b] = bmax[b].max(orig[p]);
                    }
                    let mut rel: Vec<f32> = (0..bw * bh).filter(|&b| bmax[b] > 0.0).map(|b| bmax[b] - bmin[b]).collect();
                    rel.sort_by(f32::total_cmp);
                    let relief_p50 = rel.get(rel.len() / 2).copied().unwrap_or(0.0);
                    let cap = |p: usize| -> f32 {
                        let b = ((p / w) / blk) * bw + (p % w) / blk;
                        (0.5 * (bmax[b] - bmin[b])).min(100.0)
                    };
                    let border = |p: usize, h: &[f32]| {
                        let (i, j) = (p % w, p / w);
                        i == 0 || j == 0 || i == w - 1 || j == hgt - 1 || h[p] <= 0.0
                    };
                    let nbr = |p: usize, d: usize| -> Option<usize> {
                        let (i, j) = ((p % w) as i64 + NB[d].0, (p / w) as i64 + NB[d].1);
                        (i >= 0 && j >= 0 && i < w as i64 && j < hgt as i64).then(|| j as usize * w + i as usize)
                    };
                    // Floods. 0: heap; 1: Barnes improved (plain queue for cells raised to the spill level); 2: bucket queue at 5 cm.
                    let flood = |h: &[f32], kind: u8| -> Vec<f32> {
                        let mut filled = vec![0f32; n];
                        let mut seen = vec![false; n];
                        match kind {
                            0 | 1 => {
                                let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
                                let mut pit: VecDeque<u32> = VecDeque::new();
                                for p in 0..n {
                                    if border(p, h) {
                                        filled[p] = h[p];
                                        seen[p] = true;
                                        heap.push(Reverse((key(h[p]), p as u32)));
                                    }
                                }
                                loop {
                                    let p = if let Some(p) = pit.pop_front() {
                                        p as usize
                                    } else if let Some(Reverse((_, p))) = heap.pop() {
                                        p as usize
                                    } else {
                                        break;
                                    };
                                    for d in 0..8 {
                                        let Some(q) = nbr(p, d) else { continue };
                                        if seen[q] {
                                            continue;
                                        }
                                        seen[q] = true;
                                        let lo = filled[p] + 1e-3;
                                        if kind == 1 && h[q] <= lo {
                                            filled[q] = lo;
                                            pit.push_back(q as u32);
                                        } else {
                                            filled[q] = h[q].max(lo);
                                            heap.push(Reverse((key(filled[q]), q as u32)));
                                        }
                                    }
                                }
                            }
                            _ => {
                                const B: f32 = 0.05;
                                let lo_h = h.iter().fold(f32::MAX, |a, &b| a.min(b)).min(0.0);
                                let hi_h = h.iter().fold(f32::MIN, |a, &b| a.max(b));
                                let nb = ((hi_h - lo_h) / B) as usize + 2 + (n as f32 * 1e-3 / B) as usize;
                                let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); nb];
                                let bi = |v: f32| (((v - lo_h) / B) as usize).min(nb - 1);
                                for p in 0..n {
                                    if border(p, h) {
                                        filled[p] = h[p];
                                        seen[p] = true;
                                        buckets[bi(h[p])].push(p as u32);
                                    }
                                }
                                let mut b = 0;
                                while b < nb {
                                    let Some(p) = buckets[b].pop() else {
                                        b += 1;
                                        continue;
                                    };
                                    let p = p as usize;
                                    for d in 0..8 {
                                        let Some(q) = nbr(p, d) else { continue };
                                        if seen[q] {
                                            continue;
                                        }
                                        seen[q] = true;
                                        filled[q] = h[q].max(filled[p] + 1e-3);
                                        let qb = bi(filled[q]).max(b);
                                        buckets[qb].push(q as u32);
                                    }
                                }
                            }
                        }
                        filled
                    };
                    // Receivers on the filled surface: 0 steepest descent, 1 jittered (slope x (1 + 0.5 u), u a hash in [-1, 1]).
                    let receivers = |h: &[f32], filled: &[f32], jitter: bool| -> Vec<u32> {
                        let mut rec = vec![u32::MAX; n];
                        for p in 0..n {
                            if border(p, h) {
                                continue;
                            }
                            let mut best = (0.0f64, u32::MAX);
                            for d in 0..8 {
                                let q = nbr(p, d).unwrap();
                                let dd = if d < 4 { dx } else { dx * std::f64::consts::SQRT_2 };
                                let mut s = f64::from(filled[p] - filled[q]) / dd;
                                if jitter && s > 0.0 {
                                    let x = (p as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (d as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
                                    let x = (x ^ (x >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                                    let u = (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
                                    s *= 1.0 + 0.5 * u;
                                }
                                if s > best.0 {
                                    best = (s, q as u32);
                                }
                            }
                            rec[p] = best.1;
                        }
                        rec
                    };
                    // Topological order (outlets first) from the receivers: a stack over donor lists (Braun and Willett).
                    let topo = |rec: &[u32]| -> Vec<u32> {
                        let mut ndon = vec![0u32; n + 1];
                        for &r in rec {
                            if r != u32::MAX {
                                ndon[r as usize + 1] += 1;
                            }
                        }
                        for i in 0..n {
                            ndon[i + 1] += ndon[i];
                        }
                        let mut fill = ndon.clone();
                        let mut don = vec![0u32; n];
                        for (p, &r) in rec.iter().enumerate() {
                            if r != u32::MAX {
                                don[fill[r as usize] as usize] = p as u32;
                                fill[r as usize] += 1;
                            }
                        }
                        let mut order = Vec::with_capacity(n);
                        let mut stack: Vec<u32> = (0..n as u32).filter(|&p| rec[p as usize] == u32::MAX).collect();
                        while let Some(p) = stack.pop() {
                            order.push(p);
                            for &d in &don[ndon[p as usize] as usize..ndon[p as usize + 1] as usize] {
                                stack.push(d);
                            }
                        }
                        order
                    };
                    let accumulate = |rec: &[u32], order: &[u32]| -> Vec<f64> {
                        let mut area = vec![cell_a; n];
                        for &(p, a) in &inflow {
                            area[p] += a;
                        }
                        for &p in order.iter().rev() {
                            let r = rec[p as usize];
                            if r != u32::MAX {
                                area[r as usize] += area[p as usize];
                            }
                        }
                        area
                    };
                    let carve = |h: &mut [f32], rec: &[u32], order: &[u32], area: &[f64]| {
                        for &p in order {
                            let p = p as usize;
                            let r = rec[p];
                            if r == u32::MAX {
                                continue;
                            }
                            let r = r as usize;
                            let d = if (p % w) == (r % w) || (p / w) == (r / w) { dx } else { dx * std::f64::consts::SQRT_2 };
                            let f = 0.01 * area[p].sqrt() / d;
                            let hn = ((f64::from(h[p]) + f * f64::from(h[r])) / (1.0 + f)) as f32;
                            h[p] = hn.max(orig[p] - cap(p)).min(h[p]);
                        }
                    };
                    let mut line = format!("{k} {face} {ti} {tj} {relief_p50:.0}");
                    // (b) Floods on the original ground.
                    let mut fl = Vec::new();
                    for kind in 0..3u8 {
                        let t = Instant::now();
                        let f = flood(&orig, kind);
                        fl.push((t.elapsed().as_secs_f64(), f));
                    }
                    let maxd = |a: &[f32], b: &[f32]| a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()));
                    line += &format!(" | flood heap {:.2} s barnes {:.2} s (max diff {:.4} m) bucket5cm {:.2} s (max diff {:.3} m)", fl[0].0, fl[1].0, maxd(&fl[0].1, &fl[1].1), fl[2].0, maxd(&fl[0].1, &fl[2].1));
                    let t = Instant::now();
                    let rec0 = receivers(&orig, &fl[0].1, false);
                    let rs = t.elapsed().as_secs_f64();
                    let t = Instant::now();
                    let ord0 = topo(&rec0);
                    let ts = t.elapsed().as_secs_f64();
                    let t = Instant::now();
                    let _a0 = accumulate(&rec0, &ord0);
                    let acs = t.elapsed().as_secs_f64();
                    line += &format!(" | receivers {rs:.2} s topo {ts:.2} s accumulate {acs:.2} s");
                    drop(fl);
                    // (a) Cadence, with the Barnes flood and the topological order from the receivers.
                    let mut ref_h: Option<Vec<f32>> = None;
                    for kk in [1usize, 3, 5, 10] {
                        let mut h = orig.clone();
                        let (mut route_s, mut carve_s) = (0.0, 0.0);
                        let (mut rec, mut ord, mut area) = (Vec::new(), Vec::new(), Vec::new());
                        for step in 0..STEPS {
                            if step % kk == 0 {
                                let t = Instant::now();
                                let f = flood(&h, 1);
                                rec = receivers(&h, &f, false);
                                ord = topo(&rec);
                                area = accumulate(&rec, &ord);
                                route_s += t.elapsed().as_secs_f64();
                            }
                            let t = Instant::now();
                            carve(&mut h, &rec, &ord, &area);
                            carve_s += t.elapsed().as_secs_f64();
                        }
                        match &ref_h {
                            None => {
                                line += &format!(" | k1 route {route_s:.2} carve {carve_s:.2}");
                                ref_h = Some(h);
                            }
                            Some(r) => {
                                // Difference over cells that k = 1 cut by more than 1 cm, in the core.
                                let mut d: Vec<f32> = Vec::new();
                                for j in (cj0 - wj0)..(cj0 - wj0 + CORE).min(hgt) {
                                    for i in (ci0 - wi0)..(ci0 - wi0 + CORE).min(w) {
                                        let p = j * w + i;
                                        if orig[p] - r[p] > 0.01 {
                                            d.push((h[p] - r[p]).abs());
                                        }
                                    }
                                }
                                d.sort_by(f32::total_cmp);
                                let q = |f: f64| d.get(((d.len() as f64 - 1.0) * f) as usize).copied().unwrap_or(0.0);
                                line += &format!(" | k{kk} route {route_s:.2} carve {carve_s:.2} diff p50 {:.3} p95 {:.3} max {:.2} over {} cells", q(0.5), q(0.95), q(1.0), d.len());
                            }
                        }
                    }
                    // (c) Flow direction: straight-run share of channel cells (area at least 1e7 m2) in the core: a channel cell with exactly two
                    // channel neighbours, opposite along an axis, counts as straight; runs of at least 20 such cells (2 km) counted separately.
                    let f = flood(&orig, 1);
                    let mut chans: Vec<(String, Vec<bool>)> = Vec::new();
                    for (name, jit) in [("d8", false), ("jd8", true)] {
                        let rec = receivers(&orig, &f, jit);
                        let ord = topo(&rec);
                        let a = accumulate(&rec, &ord);
                        chans.push((name.into(), a.iter().map(|&x| x >= 1e7).collect()));
                    }
                    {
                        // MFD (Quinn): area to every lower neighbour by slope^1.1 x contour length, in decreasing filled height.
                        let mut idx: Vec<u32> = (0..n as u32).collect();
                        idx.sort_by(|&a, &b| f[b as usize].total_cmp(&f[a as usize]).then(a.cmp(&b)));
                        let mut a = vec![cell_a; n];
                        for &(p, x) in &inflow {
                            a[p] += x;
                        }
                        for &p in &idx {
                            let p = p as usize;
                            if border(p, &orig) {
                                continue;
                            }
                            let mut ws = [0f64; 8];
                            let mut tot = 0.0;
                            for d in 0..8 {
                                let q = nbr(p, d).unwrap();
                                let dd = if d < 4 { dx } else { dx * std::f64::consts::SQRT_2 };
                                let s = f64::from(f[p] - f[q]) / dd;
                                if s > 0.0 {
                                    ws[d] = libm::pow(s, 1.1) * if d < 4 { 0.5 } else { 0.354 };
                                    tot += ws[d];
                                }
                            }
                            if tot > 0.0 {
                                for d in 0..8 {
                                    if ws[d] > 0.0 {
                                        let q = nbr(p, d).unwrap();
                                        a[q] += a[p] * ws[d] / tot;
                                    }
                                }
                            }
                        }
                        chans.push(("mfd".into(), a.iter().map(|&x| x >= 1e7).collect()));
                    }
                    for (name, ch) in &chans {
                        let (mut cells, mut straight) = (0usize, 0usize);
                        let mut st = vec![false; n];
                        for j in (cj0 - wj0)..(cj0 - wj0 + CORE).min(hgt) {
                            for i in (ci0 - wi0)..(ci0 - wi0 + CORE).min(w) {
                                let p = j * w + i;
                                if !ch[p] || border(p, &orig) {
                                    continue;
                                }
                                cells += 1;
                                let c: Vec<usize> = (0..8).filter(|&d| ch[nbr(p, d).unwrap()]).collect();
                                if c == [0, 1] || c == [2, 3] {
                                    straight += 1;
                                    st[p] = true;
                                }
                            }
                        }
                        // Long runs: straight cells in axis lines of at least 20.
                        let mut long = 0usize;
                        for j in 0..hgt {
                            let mut run = 0;
                            for i in 0..=w {
                                if i < w && st[j * w + i] {
                                    run += 1;
                                } else {
                                    if run >= 20 {
                                        long += run;
                                    }
                                    run = 0;
                                }
                            }
                        }
                        for i in 0..w {
                            let mut run = 0;
                            for j in 0..=hgt {
                                if j < hgt && st[j * w + i] {
                                    run += 1;
                                } else {
                                    if run >= 20 {
                                        long += run;
                                    }
                                    run = 0;
                                }
                            }
                        }
                        line += &format!(" | {name} chan {cells} straight {:.3} long {:.3}", straight as f64 / cells.max(1) as f64, long as f64 / cells.max(1) as f64);
                        if let Some(dd) = &dump_dir {
                            let b: Vec<u8> = ch.iter().map(|&x| u8::from(x)).collect();
                            std::fs::write(format!("{dd}/t{face}-{ti}-{tj}-{name}-{w}x{hgt}.u8"), b).unwrap();
                        }
                    }
                    if let Some(dd) = &dump_dir {
                        let b: Vec<u8> = orig.iter().flat_map(|x| x.to_le_bytes()).collect();
                        std::fs::write(format!("{dd}/t{face}-{ti}-{tj}-orig-{w}x{hgt}.f32"), b).unwrap();
                    }
                    let mut o = out.lock().unwrap();
                    writeln!(o, "{line}").unwrap();
                    o.flush().unwrap();
                });
            }
        });
    }
}
