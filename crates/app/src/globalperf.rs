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

    // spec: LOD-013
    /// The tiles the plan holds back as too steep to morph (ADR 0074) at the pose of an F9 repro bundle, over the stored bake: with no chunk stage, with the
    /// chunk stage's widening of 300 m both ways, and with an asymmetric one (`PLANET_HELD_DOWN` and `PLANET_HELD_UP`, default 250 and 10 m). The chunks
    /// need not be landed: the widening of the bounds is independent of readiness (LOD-013). `PLANET_BUNDLE=FILE PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo
    /// test -p planet --release --lib held_back_report -- --ignored --nocapture`. Read only, one thread.
    #[test]
    #[ignore = "a measurement: it plans the view of a bundle three times"]
    fn held_back_report() {
        use planet_core::cube::TangentWarp;
        let mut out = std::io::stdout();
        let bundle = std::env::var_os("PLANET_BUNDLE").map(std::path::PathBuf::from).expect("PLANET_BUNDLE=FILE");
        let text = std::fs::read_to_string(&bundle).expect("the bundle reads");
        let b = planet_frame::TerrainBundle::from_text(&text).expect("a terrain bundle");
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, _tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let env_m = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let (down, up) = (env_m("PLANET_HELD_DOWN", 250.0), env_m("PLANET_HELD_UP", 10.0));
        let spawner = std::sync::Arc::new(planet_streaming::ThreadPool::new(2));
        let cfg = planet_streaming::chunks::ChunkConfig { workers: 2, ..Default::default() };
        let plain = world.field().unwrap();
        let lod = b.request.lod.clone();
        let count = |field: &dyn planet_generators::height::HeightField| -> (usize, usize) {
            let sel = planet_frame::select_nodes_in_view_field(&TangentWarp, &b.request.camera, field, &lod);
            (sel.nodes.iter().filter(|n| n.capped).count(), sel.nodes.len())
        };
        let (c0, n0) = count(&*plain);
        writeln!(out, "pose of {}: no chunk stage: {c0} of {n0} nodes held back", bundle.display()).map_err(err).unwrap();
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
        let both = world.field().unwrap();
        let (c1, n1) = count(&*both);
        writeln!(out, "  chunk stage, widening 300 m both ways: {c1} of {n1}").map_err(err).unwrap();
        let asym = world.field_with_bounds(Some((down, up))).unwrap();
        let (c2, n2) = count(&*asym);
        writeln!(out, "  asymmetric bound (down {down} m, up {up} m): {c2} of {n2}").map_err(err).unwrap();
    }

    // spec: LOD-013
    /// The step at the chunk's apply level at the pose of an F9 repro bundle (owner test finding 3, the coast): the chunks the plan's tiles of level 15 and
    /// finer read are landed, then the change they make at the entry level (`ANET_TOP_LEVEL`, the coarsest tile level that reads chunk detail) is sampled at
    /// the 33 x 33 vertices of every plan node of that level: the step between such a tile and its parent, which reads the base. `PLANET_BUNDLE=FILE
    /// PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib entry_step_report -- --ignored --nocapture`. Lands chunks: minutes.
    #[test]
    #[ignore = "a measurement: it solves the chunks under a view and samples their change"]
    fn entry_step_report() {
        use planet_core::cube::TangentWarp;
        let mut out = std::io::stdout();
        let bundle = std::env::var_os("PLANET_BUNDLE").map(std::path::PathBuf::from).expect("PLANET_BUNDLE=FILE");
        let text = std::fs::read_to_string(&bundle).expect("the bundle reads");
        let b = planet_frame::TerrainBundle::from_text(&text).expect("a terrain bundle");
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, _tile) = anet_scene(res, 11, store, &mut out).unwrap();
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
        let entry = crate::anetworld::ANET_TOP_LEVEL;
        let lod = b.request.lod.clone();
        let plan_nodes = |field: &dyn planet_generators::height::HeightField| -> Vec<planet_core::TileId> {
            planet_frame::select_nodes_in_view_field(&TangentWarp, &b.request.camera, field, &lod).nodes.iter().map(|n| n.tile).collect()
        };
        let tiles = plan_nodes(&*world.field().unwrap());
        let fine: Vec<planet_core::TileId> = tiles.iter().copied().filter(|t| t.level() >= entry).collect();
        writeln!(
            out,
            "pose of {}: {} plan nodes, {} of level {entry} or finer, {} of them level {entry}",
            bundle.display(),
            tiles.len(),
            fine.len(),
            fine.iter().filter(|t| t.level() == entry).count()
        )
        .map_err(err)
        .unwrap();
        let manager = world.chunk_manager().unwrap();
        let camera = b.request.camera.position;
        let start = std::time::Instant::now();
        loop {
            world.chunk_frame(&fine, camera, &[]);
            std::thread::sleep(std::time::Duration::from_millis(100));
            let all_ready = fine.iter().all(|t| manager.reads(*t).iter().all(|k| manager.is_ready(*k)));
            if all_ready && manager.in_flight() == 0 {
                break;
            }
            assert!(start.elapsed().as_secs() < 1_800, "the chunks did not land");
        }
        writeln!(out, "  {} chunks landed in {:.0} s", manager.stats().landed, start.elapsed().as_secs_f64()).map_err(err).unwrap();
        let field = world.field().unwrap();
        let mut steps: Vec<(f64, planet_core::TileId)> = Vec::new();
        let mut all: Vec<f64> = Vec::new();
        for t in tiles.iter().filter(|t| t.level() == entry) {
            let mut worst = 0.0f64;
            for l in 0..=32u32 {
                for k in 0..=32u32 {
                    let d = t.sample_direction(&TangentWarp, 32, k, l);
                    let c = (field.height_at_level(d, entry) - field.height_at_level(d, entry - 1)).abs();
                    all.push(c);
                    worst = worst.max(c);
                }
            }
            steps.push((worst, *t));
        }
        all.sort_by(f64::total_cmp);
        steps.sort_by(|a, b| b.0.total_cmp(&a.0));
        if all.is_empty() {
            writeln!(out, "  no plan node of level {entry} at this pose").map_err(err).unwrap();
            return;
        }
        writeln!(
            out,
            "  step at the entry level {entry} (a tile against its parent, at its vertices): p99 {:.2} m, max {:.2} m over {} vertices of {} nodes",
            all[all.len() * 99 / 100],
            all[all.len() - 1],
            all.len(),
            steps.len()
        )
        .map_err(err)
        .unwrap();
        for (w, t) in steps.iter().take(3) {
            writeln!(out, "    worst tile {:?}: {w:.2} m", (t.face().0, t.x(), t.y())).map_err(err).unwrap();
        }
    }

    // spec: LOD-013
    /// The held-back tiles at the pose of an F9 bundle, whether they sit on a boundary between two levels of the plan, and the base's own step across the
    /// bake's block seams nearest the pose (owner test finding 3). `PLANET_BUNDLE=FILE PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release
    /// --lib coast_pose_report -- --ignored --nocapture`. Read only, one thread, no chunks landed (the held count is independent of them, LOD-013).
    #[test]
    #[ignore = "a measurement: it plans the view of a bundle twice and samples the base across block seams"]
    fn coast_pose_report() {
        use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
        use planet_core::{Side, TileId};
        let mut out = std::io::stdout();
        let bundle = std::env::var_os("PLANET_BUNDLE").map(std::path::PathBuf::from).expect("PLANET_BUNDLE=FILE");
        let text = std::fs::read_to_string(&bundle).expect("the bundle reads");
        let b = planet_frame::TerrainBundle::from_text(&text).expect("a terrain bundle");
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, _tile) = anet_scene(res, 11, store, &mut out).unwrap();
        let plain = world.field().unwrap();
        let lod = b.request.lod.clone();
        let sel_plain = planet_frame::select_nodes_in_view_field(&TangentWarp, &b.request.camera, &*plain, &lod);
        let held_plain = sel_plain.nodes.iter().filter(|n| n.capped).count();
        // The seam step first, on the plain base (before the chunk stage is on).
        let radius = world.radius_m();
        let pos = b.request.camera.position.0.normalized();
        let (face, s0, t0) = direction_to_face(&TangentWarp, pos);
        let cell = 2.0 / f64::from(res);
        let block = 1024.0f64.min(f64::from(res));
        let seam_near = |x: f64| -> f64 {
            let i = (x + 1.0) / cell;
            ((i / block).round() * block) * cell - 1.0
        };
        let (s_seam, t_seam) = (seam_near(s0), seam_near(t0));
        let metres_per_s = {
            let a = face_to_direction(&TangentWarp, face, s0, t0);
            let c = face_to_direction(&TangentWarp, face, s0 + 1e-6, t0);
            radius * a.angle_to(c) / 1e-6
        };
        let h = |s: f64, t: f64| plain.height(face_to_direction(&TangentWarp, face, s, t));
        // Across the seam: the height difference over 2 m (+-1 m) along the line, at 41 points 2 km apart along the seam, against the same on the control
        // line half a block away (an interior line).
        let eps = 1.0 / metres_per_s;
        let along = 2_000.0 / metres_per_s;
        let step_at = |line: f64, vertical: bool| -> (f64, f64) {
            let (mut max, mut sum) = (0.0f64, 0.0f64);
            for k in -20..=20 {
                let a = f64::from(k) * along;
                let d = if vertical {
                    (h(line + eps, t0 + a) - h(line - eps, t0 + a)).abs()
                } else {
                    (h(s0 + a, line + eps) - h(s0 + a, line - eps)).abs()
                };
                max = max.max(d);
                sum += d;
            }
            (max, sum / 41.0)
        };
        let (sm, ss) = step_at(s_seam, true);
        let (tm, ts) = step_at(t_seam, false);
        let (cm, cs) = step_at(seam_near(s0) + 0.5 * block * cell, true);
        writeln!(
            out,
            "pose of {}: nearest seams at {:.1} km (s) and {:.1} km (t); across the s seam over 2 m: max {sm:.3} m, mean {ss:.3}; t seam: max {tm:.3}, mean {ts:.3}; interior control line: max {cm:.3}, mean {cs:.3}",
            bundle.display(),
            (s0 - s_seam).abs() * metres_per_s / 1e3,
            (t0 - t_seam).abs() * metres_per_s / 1e3
        )
        .map_err(err)
        .unwrap();
        // The held tiles with the chunk stage's widening on.
        let spawner = std::sync::Arc::new(planet_streaming::ThreadPool::new(2));
        let cfg = planet_streaming::chunks::ChunkConfig { workers: 2, ..Default::default() };
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
        let both = world.field().unwrap();
        let sel = planet_frame::select_nodes_in_view_field(&TangentWarp, &b.request.camera, &*both, &lod);
        let held: Vec<TileId> = sel.nodes.iter().filter(|n| n.capped).map(|n| n.tile).collect();
        // A node is on a level boundary when one of its four side neighbours at its level is not a node of the plan but its parent is (a coarser
        // neighbour) or its children are (a finer one).
        let set: std::collections::HashSet<TileId> = sel.nodes.iter().map(|n| n.tile).collect();
        let on_boundary = |t: &TileId| -> bool {
            [Side::East, Side::West, Side::North, Side::South].iter().any(|&side| {
                let nb = t.neighbor(&TangentWarp, side);
                !set.contains(&nb)
                    && (nb.parent().is_some_and(|p| set.contains(&p)) || nb.children().is_some_and(|c| c.iter().any(|k| set.contains(k))))
            })
        };
        let all_b = sel.nodes.iter().filter(|n| on_boundary(&n.tile)).count();
        let held_b = held.iter().filter(|t| on_boundary(t)).count();
        let mut by_level = std::collections::BTreeMap::<u8, usize>::new();
        for t in &held {
            *by_level.entry(t.level()).or_default() += 1;
        }
        writeln!(
            out,
            "  held back: {} of {} nodes with the widening ({held_plain} of {} without the chunk stage); {held_b} of the held and {all_b} of all {} nodes sit on a level boundary; held by level {:?}",
            held.len(),
            sel.nodes.len(),
            sel_plain.nodes.len(),
            sel.nodes.len(),
            by_level
        )
        .map_err(err)
        .unwrap();
    }

    // spec: GEN-037
    /// The change the A-net chunks make against a base with a valley carve along the run's trunk chains (ADR 0084 option 1), for thresholds of the
    /// run's drainage area (`PLANET_CARVE_THRESHOLDS`, default 1e9 and 1e10 m2) and a scale of GEN-057's depth law (`PLANET_CARVE_SCALE`, default 1:
    /// `d = min(3 m (A / 1e8)^0.5, 100 m)`), over a sample of land chunks (`PLANET_ANET_CHUNKS`, default 24, the same sample as `anet_change_report`):
    /// the carve is made in memory on the bake's lattice along the polylines from every run cell over the threshold to its receiver (area interpolated
    /// along it), read bilinearly, and subtracted from the base the stage solves on; the report gives the cells carved, the share of the chunks' cells
    /// it lowers by more than 1 m, and the change against the carved base (p50, p99, p99.9, max, signed range) beside the one with no carve.
    /// `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib carve_threshold_report -- --ignored --nocapture`. Read only.
    #[test]
    #[ignore = "a measurement: it solves a sample of real chunks several times"]
    fn carve_threshold_report() {
        use planet_core::cube::{direction_to_face, TangentWarp};
        use planet_core::hash::SplitMix64;
        use planet_generators::anet::{BaseDetail, CoarseRun};
        use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField};
        use std::collections::HashMap;
        use std::sync::Arc;
        let mut out = std::io::stdout();
        let res: u32 = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let count: usize = std::env::var("PLANET_ANET_CHUNKS").ok().and_then(|r| r.parse().ok()).unwrap_or(24);
        let scale: f64 = std::env::var("PLANET_CARVE_SCALE").ok().and_then(|r| r.parse().ok()).unwrap_or(1.0);
        let thresholds: Vec<f64> = std::env::var("PLANET_CARVE_THRESHOLDS")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
            .unwrap_or_else(|| vec![1e9, 1e10]);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, maps, _) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let lat = planet_generators::chunkfield::ChunkLattice::new(crate::anetworld::cells_per_chunk_for(radius, 8.0)).unwrap();
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        let per_face = 1u32 << top;
        let base = world.base_field().unwrap();
        let mut rng = SplitMix64(0x6368_616e_6765);
        let mut tiles: Vec<planet_core::TileId> = Vec::new();
        while tiles.len() < count {
            let face = planet_core::Face::ALL[(rng.next_u64() % 6) as usize];
            let (x, y) = ((rng.next_u64() % u64::from(per_face)) as u32, (rng.next_u64() % u64::from(per_face)) as u32);
            let t = planet_core::TileId::new(face, top, x, y).expect("a tile of the chunk grid");
            // `PLANET_CARVE_ON_TRUNKS=1`: only chunks whose centre run cell or one of its neighbours drains over 1e9 m2 (the chunks the carve touches).
            let on_trunk = std::env::var_os("PLANET_CARVE_ON_TRUNKS").is_none() || {
                let c = maps.lattice().cell_at(t.center_direction(&TangentWarp));
                std::iter::once(c)
                    .chain(maps.lattice().neighbours(c).into_iter().map(|n| n as usize))
                    .any(|n| n < maps.lattice().len() && f64::from(maps.area_m2(n)) >= 1.0e9)
            };
            if on_trunk && base.height(t.center_direction(&TangentWarp)) > 0.0 {
                tiles.push(t);
            }
        }
        let map = &TangentWarp;
        let coarse = maps.lattice();
        let depth = |area: f64| (scale * 3.0 * libm::sqrt(area / 1.0e8)).min(100.0 * scale.max(1.0));
        let cell_key = |res: u32, f: u8, i: i64, j: i64| {
            (u64::from(f) << 56)
                | ((i.clamp(0, i64::from(res) - 1) as u64 & 0x0fff_ffff) << 28)
                | (j.clamp(0, i64::from(res) - 1) as u64 & 0x0fff_ffff)
        };
        // The carve grid of a threshold: depth in metres on the cells of the bake's lattice that the polylines of the run's trunks touch, for the
        // windows round the sample chunks (four rings of run cells round the cell holding each chunk's centre, and every segment those cells start).
        let carve_grid = |thr: f64| -> HashMap<u64, f32> {
            let mut grid: HashMap<u64, f32> = HashMap::new();
            let mut seen = std::collections::HashSet::new();
            for t in &tiles {
                let mut ring = vec![coarse.cell_at(t.center_direction(map))];
                let mut all = ring.clone();
                for _ in 0..4 {
                    let mut next: Vec<usize> = Vec::new();
                    for &c in &ring {
                        for n in coarse.neighbours(c) {
                            let n = n as usize;
                            if n != planet_generators::coupled::NONE as usize && !all.contains(&n) && !next.contains(&n) {
                                next.push(n);
                            }
                        }
                    }
                    all.extend(next.iter().copied());
                    ring = next;
                }
                for c in all {
                    if !seen.insert(c) || f64::from(maps.area_m2(c)) < thr {
                        continue;
                    }
                    let r = maps.receiver(c);
                    if r == planet_generators::coupled::NONE {
                        continue;
                    }
                    let (a, b) = (coarse.direction(c), coarse.direction(r as usize));
                    let (area_a, area_b) = (f64::from(maps.area_m2(c)), f64::from(maps.area_m2(r as usize)));
                    let steps = (radius * a.angle_to(b) / 150.0).ceil().max(1.0) as u32;
                    for k in 0..=steps {
                        let l = f64::from(k) / f64::from(steps);
                        let p = (a * (1.0 - l) + b * l).normalized();
                        let (f, s, t2) = direction_to_face(map, p);
                        let n = f64::from(res);
                        let (i, j) = (((s + 1.0) * 0.5 * n) as i64, ((t2 + 1.0) * 0.5 * n) as i64);
                        let d = depth(area_a * (1.0 - l) + area_b * l) as f32;
                        let e = grid.entry(cell_key(res, f.0, i, j)).or_insert(0.0);
                        *e = e.max(d);
                    }
                }
            }
            grid
        };
        struct Carved {
            inner: crate::anetworld::WorldBase,
            grid: HashMap<u64, f32>,
            res: u32,
            thr: f64,
        }
        impl Carved {
            fn carve(&self, d: planet_core::Vec3) -> f64 {
                if self.grid.is_empty() {
                    return 0.0;
                }
                let (f, s, t) = direction_to_face(&TangentWarp, d);
                let n = f64::from(self.res);
                let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
                let (i0, j0) = (libm::floor(x) as i64, libm::floor(y) as i64);
                let (wx, wy) = (x - i0 as f64, y - j0 as f64);
                let key = |i: i64, j: i64| {
                    (u64::from(f.0) << 56)
                        | ((i.clamp(0, i64::from(self.res) - 1) as u64 & 0x0fff_ffff) << 28)
                        | (j.clamp(0, i64::from(self.res) - 1) as u64 & 0x0fff_ffff)
                };
                let at = |i: i64, j: i64| f64::from(self.grid.get(&key(i, j)).copied().unwrap_or(0.0));
                at(i0, j0) * (1.0 - wx) * (1.0 - wy)
                    + at(i0 + 1, j0) * wx * (1.0 - wy)
                    + at(i0, j0 + 1) * (1.0 - wx) * wy
                    + at(i0 + 1, j0 + 1) * wx * wy
            }
        }
        impl BaseDetail for Carved {
            fn base_height(&self, dir: planet_core::Vec3) -> f64 {
                self.inner.base_height(dir) - self.carve(dir)
            }
            fn base_heights(&self, dirs: &[planet_core::Vec3]) -> Vec<f64> {
                self.inner.base_heights(dirs).into_iter().zip(dirs).map(|(h, d)| h - self.carve(*d)).collect()
            }
            fn coarse_input(&self, dir: planet_core::Vec3) -> f64 {
                self.inner.coarse_input(dir)
            }
            fn region_heights(&self, dirs: &[planet_core::Vec3]) -> Vec<f64> {
                self.inner.region_heights(dirs).into_iter().zip(dirs).map(|(h, d)| h - self.carve(*d)).collect()
            }
            fn identity(&self) -> u64 {
                self.inner.identity() ^ self.thr.to_bits() ^ u64::from(self.res)
            }
        }
        let mut variants: Vec<(String, HashMap<u64, f32>, f64)> = vec![("no carve".into(), HashMap::new(), 0.0)];
        // Where the solve's own channel lies against the carve line (the reference line: threshold 1e9, the law as set): the share of the incision (the sum
        // of the changes below -5 m) at points the line lowers by more than 1 m, and at points within about two bake cells of it.
        let ref_line = Carved { inner: crate::anetworld::WorldBase::new(&world), grid: carve_grid(1.0e9), res, thr: 1.0e9 };
        for &thr in &thresholds {
            let t = std::time::Instant::now();
            let g = carve_grid(thr);
            writeln!(
                out,
                "carve at threshold {thr:.0e} m2: {} bake cells carved over the {count} windows ({:.1} s)",
                g.len(),
                t.elapsed().as_secs_f64()
            )
            .map_err(err)
            .unwrap();
            variants.push((format!("threshold {thr:.0e} m2, scale {scale}"), g, thr));
        }
        for (label, grid, thr) in variants {
            let carved = Arc::new(Carved { inner: crate::anetworld::WorldBase::new(&world), grid, res, thr });
            let mut params = planet_generators::anet::ANetParams::spike(crate::anetworld::ANET_HALO_M, lat.cell_m(radius));
            params.drivers.seed = world.base_seed();
            let stage = planet_generators::anet::ANet {
                run: maps.clone(),
                base: carved.clone() as Arc<dyn BaseDetail>,
                radius_m: radius,
                params,
                memo: Default::default(),
            };
            let (mut abs, mut lowered) = (Vec::<f64>::new(), 0usize);
            let (mut lo, mut hi) = (0.0f64, 0.0f64);
            let (mut mass_all, mut mass_on, mut mass_near, mut n_on, mut n_near) = (0.0f64, 0.0f64, 0.0f64, 0usize, 0usize);
            for tile in &tiles {
                let Some(r) = compute_chunk(&lat, &stage, radius, top, *tile, halo_rings(&stage)) else { continue };
                let field = ChunkField::new(world.base_field().unwrap(), lat, radius, &stage).with_top_level(top).with_ready(Arc::new(r));
                for l in 0..65u32 {
                    for k in 0..65u32 {
                        let d = tile.sample_direction(map, 64, k, l);
                        let c = field.change_at_level(d, top + 5);
                        abs.push(c.abs());
                        lo = lo.min(c);
                        hi = hi.max(c);
                        lowered += usize::from(carved.carve(d) > 1.0);
                        if thr == 0.0 {
                            let on = ref_line.carve(d) > 1.0;
                            let near = !on && {
                                let (f, s0, t0) = direction_to_face(map, d);
                                let off = 2.0 * 2.0 / f64::from(res);
                                [(off, 0.0), (-off, 0.0), (0.0, off), (0.0, -off)]
                                    .iter()
                                    .any(|(a, b2)| ref_line.carve(planet_core::cube::face_to_direction(map, f, s0 + a, t0 + b2)) > 1.0)
                            };
                            let mass = if c < -5.0 { -c } else { 0.0 };
                            mass_all += mass;
                            if on {
                                mass_on += mass;
                                n_on += 1;
                            } else if near {
                                mass_near += mass;
                                n_near += 1;
                            }
                        }
                    }
                }
            }
            abs.sort_by(f64::total_cmp);
            let q = |p: f64| abs[(((abs.len() - 1) as f64) * p) as usize];
            if thr == 0.0 && mass_all > 0.0 {
                writeln!(
                    out,
                    "  the solve's own incision (sum of the change below -5 m) lies {:.1} % on the carve line ({n_on} points) and {:.1} % within about two bake cells of it ({n_near} points)",
                    100.0 * mass_on / mass_all,
                    100.0 * mass_near / mass_all
                )
                .map_err(err)
                .unwrap();
            }
            writeln!(
                out,
                "{label}: the carve lowers {:.1} % of the chunks' sample points by more than 1 m; |change| over {} points: p50 {:.2}, p99 {:.2}, p99.9 {:.2}, max {:.2} m; signed {lo:.1} to {hi:+.1} m",
                100.0 * lowered as f64 / abs.len() as f64,
                abs.len(),
                q(0.5),
                q(0.99),
                q(0.999),
                abs[abs.len() - 1]
            )
            .map_err(err)
            .unwrap();
        }
    }

    // spec: GEN-037
    /// "Replace, not stack" (ADR 0084 option 1): the chunk keeps the absolute height of its bed and anchors (the stage solves on the uncarved base), and
    /// the carve lowers the base only down to the depth the chunk cuts at that place, so the change against the carved base is `change + min(carve,
    /// max(0, -change))`. Over the 24 trunk chunks of `carve_threshold_report` it gives the change at level 16 for the depth law at scale 1 and at a scale
    /// tuned to the chunks' median incision on the carve line; and at the site of an F9 bundle (`PLANET_BUNDLE`, the findings 2 site) the height step along
    /// the river between the coarsest level that has the chunk change (`ANET_TOP_LEVEL`) and the level above it, for no carve, the stacked carve (the
    /// stage solves on the carved base) and the replace variant. `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N PLANET_BUNDLE=FILE cargo test -p planet --release
    /// --lib carve_replace_report -- --ignored --nocapture`. Read only.
    #[test]
    #[ignore = "a measurement: it solves chunks several times"]
    fn carve_replace_report() {
        use planet_core::cube::{direction_to_face, TangentWarp};
        use planet_core::hash::SplitMix64;
        use planet_generators::anet::{BaseDetail, CoarseRun};
        use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField};
        use std::collections::HashMap;
        use std::sync::Arc;
        let mut out = std::io::stdout();
        let res: u32 = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let count: usize = std::env::var("PLANET_ANET_CHUNKS").ok().and_then(|r| r.parse().ok()).unwrap_or(24);
        let bundle = std::env::var_os("PLANET_BUNDLE").map(std::path::PathBuf::from);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, maps, _) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let lat = planet_generators::chunkfield::ChunkLattice::new(crate::anetworld::cells_per_chunk_for(radius, 8.0)).unwrap();
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        let entry = crate::anetworld::ANET_TOP_LEVEL;
        let map = &TangentWarp;
        let coarse = maps.lattice();
        let thr = 1.0e9;
        let depth = |scale: f64, area: f64| (scale * 3.0 * libm::sqrt(area / 1.0e8)).min(100.0 * scale.max(1.0));
        let cell_key = |f: u8, i: i64, j: i64| {
            (u64::from(f) << 56)
                | ((i.clamp(0, i64::from(res) - 1) as u64 & 0x0fff_ffff) << 28)
                | (j.clamp(0, i64::from(res) - 1) as u64 & 0x0fff_ffff)
        };
        // The carve grid of a scale round the given chunks: depth on the bake's lattice along the run's polylines from every cell over `thr`.
        let carve_grid = |tiles: &[planet_core::TileId], scale: f64, thr: f64| -> HashMap<u64, f32> {
            let mut grid: HashMap<u64, f32> = HashMap::new();
            let mut seen = std::collections::HashSet::new();
            for t in tiles {
                let mut ring = vec![coarse.cell_at(t.center_direction(map))];
                let mut all = ring.clone();
                for _ in 0..4 {
                    let mut next: Vec<usize> = Vec::new();
                    for &c in &ring {
                        for n in coarse.neighbours(c) {
                            let n = n as usize;
                            if n != planet_generators::coupled::NONE as usize && !all.contains(&n) && !next.contains(&n) {
                                next.push(n);
                            }
                        }
                    }
                    all.extend(next.iter().copied());
                    ring = next;
                }
                for c in all {
                    if !seen.insert(c) || f64::from(maps.area_m2(c)) < thr {
                        continue;
                    }
                    let r = maps.receiver(c);
                    if r == planet_generators::coupled::NONE {
                        continue;
                    }
                    let (a, b) = (coarse.direction(c), coarse.direction(r as usize));
                    let (area_a, area_b) = (f64::from(maps.area_m2(c)), f64::from(maps.area_m2(r as usize)));
                    let steps = (radius * a.angle_to(b) / 150.0).ceil().max(1.0) as u32;
                    for k in 0..=steps {
                        let l = f64::from(k) / f64::from(steps);
                        let p = (a * (1.0 - l) + b * l).normalized();
                        let (f, s, t2) = direction_to_face(map, p);
                        let n = f64::from(res);
                        let (i, j) = (((s + 1.0) * 0.5 * n) as i64, ((t2 + 1.0) * 0.5 * n) as i64);
                        let d = depth(scale, area_a * (1.0 - l) + area_b * l) as f32;
                        let e = grid.entry(cell_key(f.0, i, j)).or_insert(0.0);
                        *e = e.max(d);
                    }
                }
            }
            grid
        };
        let carve_at = |grid: &HashMap<u64, f32>, d: planet_core::Vec3| -> f64 {
            if grid.is_empty() {
                return 0.0;
            }
            let (f, s, t) = direction_to_face(map, d);
            let n = f64::from(res);
            let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
            let (i0, j0) = (libm::floor(x) as i64, libm::floor(y) as i64);
            let (wx, wy) = (x - i0 as f64, y - j0 as f64);
            let at = |i: i64, j: i64| f64::from(grid.get(&cell_key(f.0, i, j)).copied().unwrap_or(0.0));
            at(i0, j0) * (1.0 - wx) * (1.0 - wy)
                + at(i0 + 1, j0) * wx * (1.0 - wy)
                + at(i0, j0 + 1) * (1.0 - wx) * wy
                + at(i0 + 1, j0 + 1) * wx * wy
        };
        struct Carved {
            inner: crate::anetworld::WorldBase,
            grid: HashMap<u64, f32>,
            res: u32,
        }
        impl Carved {
            fn carve(&self, d: planet_core::Vec3) -> f64 {
                if self.grid.is_empty() {
                    return 0.0;
                }
                let (f, s, t) = direction_to_face(&TangentWarp, d);
                let n = f64::from(self.res);
                let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
                let (i0, j0) = (libm::floor(x) as i64, libm::floor(y) as i64);
                let (wx, wy) = (x - i0 as f64, y - j0 as f64);
                let key = |i: i64, j: i64| {
                    (u64::from(f.0) << 56)
                        | ((i.clamp(0, i64::from(self.res) - 1) as u64 & 0x0fff_ffff) << 28)
                        | (j.clamp(0, i64::from(self.res) - 1) as u64 & 0x0fff_ffff)
                };
                let at = |i: i64, j: i64| f64::from(self.grid.get(&key(i, j)).copied().unwrap_or(0.0));
                at(i0, j0) * (1.0 - wx) * (1.0 - wy)
                    + at(i0 + 1, j0) * wx * (1.0 - wy)
                    + at(i0, j0 + 1) * (1.0 - wx) * wy
                    + at(i0 + 1, j0 + 1) * wx * wy
            }
        }
        impl BaseDetail for Carved {
            fn base_height(&self, dir: planet_core::Vec3) -> f64 {
                self.inner.base_height(dir) - self.carve(dir)
            }
            fn base_heights(&self, dirs: &[planet_core::Vec3]) -> Vec<f64> {
                self.inner.base_heights(dirs).into_iter().zip(dirs).map(|(h, d)| h - self.carve(*d)).collect()
            }
            fn coarse_input(&self, dir: planet_core::Vec3) -> f64 {
                self.inner.coarse_input(dir)
            }
            fn region_heights(&self, dirs: &[planet_core::Vec3]) -> Vec<f64> {
                self.inner.region_heights(dirs).into_iter().zip(dirs).map(|(h, d)| h - self.carve(*d)).collect()
            }
            fn identity(&self) -> u64 {
                self.inner.identity() ^ u64::from(self.res)
            }
        }
        let stage_of = |grid: HashMap<u64, f32>| -> planet_generators::anet::ANet {
            let mut params = planet_generators::anet::ANetParams::spike(crate::anetworld::ANET_HALO_M, lat.cell_m(radius));
            params.drivers.seed = world.base_seed();
            planet_generators::anet::ANet {
                run: maps.clone(),
                base: Arc::new(Carved { inner: crate::anetworld::WorldBase::new(&world), grid, res }) as Arc<dyn BaseDetail>,
                radius_m: radius,
                params,
                memo: Default::default(),
            }
        };
        let stats = |v: &mut Vec<f64>| -> String {
            v.sort_by(f64::total_cmp);
            let q = |p: f64| v[(((v.len() - 1) as f64) * p) as usize];
            format!("p50 {:.2}, p99 {:.2}, p99.9 {:.2}, max {:.2} m", q(0.5), q(0.99), q(0.999), v[v.len() - 1])
        };
        // Part A: the 24 trunk chunks (the sample of `carve_threshold_report`).
        let per_face = 1u32 << top;
        let base = world.base_field().unwrap();
        let mut rng = SplitMix64(0x6368_616e_6765);
        let mut tiles: Vec<planet_core::TileId> = Vec::new();
        while tiles.len() < count {
            let face = planet_core::Face::ALL[(rng.next_u64() % 6) as usize];
            let (x, y) = ((rng.next_u64() % u64::from(per_face)) as u32, (rng.next_u64() % u64::from(per_face)) as u32);
            let t = planet_core::TileId::new(face, top, x, y).expect("a tile of the chunk grid");
            let c = coarse.cell_at(t.center_direction(map));
            let on_trunk = std::iter::once(c)
                .chain(coarse.neighbours(c).into_iter().map(|n| n as usize))
                .any(|n| n < coarse.len() && f64::from(maps.area_m2(n)) >= thr);
            if on_trunk && base.height(t.center_direction(map)) > 0.0 {
                tiles.push(t);
            }
        }
        let plain = stage_of(HashMap::new());
        let grid1 = carve_grid(&tiles, 1.0, thr);
        let mut points: Vec<(planet_core::Vec3, f64)> = Vec::new();
        for tile in &tiles {
            let Some(r) = compute_chunk(&lat, &plain, radius, top, *tile, halo_rings(&plain)) else { continue };
            let field = ChunkField::new(world.base_field().unwrap(), lat, radius, &plain).with_top_level(top).with_ready(Arc::new(r));
            for l in 0..65u32 {
                for k in 0..65u32 {
                    let d = tile.sample_direction(map, 64, k, l);
                    points.push((d, field.change_at_level(d, top + 5)));
                }
            }
        }
        // The median incision of the chunks on the carve line against the median of the law at scale 1 there.
        let mut inc: Vec<f64> = points.iter().filter(|(d, c)| *c < -5.0 && carve_at(&grid1, *d) > 1.0).map(|(_, c)| -c).collect();
        let mut law: Vec<f64> =
            points.iter().filter(|(d, c)| *c < -5.0 && carve_at(&grid1, *d) > 1.0).map(|(d, _)| carve_at(&grid1, *d)).collect();
        inc.sort_by(f64::total_cmp);
        law.sort_by(f64::total_cmp);
        let (m_inc, m_law) = (inc[inc.len() / 2], law[law.len() / 2].max(1e-6));
        let tuned = m_inc / m_law;
        writeln!(
            out,
            "median incision on the carve line {m_inc:.1} m, median depth of the law at scale 1 there {m_law:.1} m: tuned scale {tuned:.1}"
        )
        .map_err(err)
        .unwrap();
        let mut none: Vec<f64> = points.iter().map(|(_, c)| c.abs()).collect();
        writeln!(out, "A no carve: change {}", stats(&mut none)).map_err(err).unwrap();
        for (label, scale) in [("scale 1", 1.0), ("tuned scale", tuned)] {
            let grid = carve_grid(&tiles, scale, thr);
            let mut v: Vec<f64> = points.iter().map(|(d, c)| (c + carve_at(&grid, *d).min((-c).max(0.0))).abs()).collect();
            writeln!(out, "A replace, {label}: change {}", stats(&mut v)).map_err(err).unwrap();
        }
        // Part B: the step along the river at the bundle's site between level `entry` (with the chunk change) and `entry - 1` (without).
        let Some(bundle) = bundle else { return };
        let b =
            planet_frame::TerrainBundle::from_text(&std::fs::read_to_string(&bundle).expect("the bundle reads")).expect("a terrain bundle");
        let lod = b.request.lod.clone();
        let nodes: Vec<planet_core::TileId> =
            planet_frame::select_nodes_in_view_field(map, &b.request.camera, &*world.field().unwrap(), &lod)
                .nodes
                .iter()
                .map(|n| n.tile)
                .collect();
        let nodes15: Vec<planet_core::TileId> = nodes.iter().copied().filter(|t| t.level() == entry).collect();
        let mut chunks: Vec<planet_core::TileId> = Vec::new();
        for t in &nodes15 {
            let c = planet_core::TileId::from_direction(map, t.center_direction(map), top);
            if !chunks.contains(&c) {
                chunks.push(c);
            }
        }
        writeln!(out, "B site of {}: {} level-{entry} nodes over {} chunks", bundle.display(), nodes15.len(), chunks.len())
            .map_err(err)
            .unwrap();
        let thr_b: f64 = std::env::var("PLANET_CARVE_B_THRESHOLD").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0e8);
        writeln!(out, "B carve threshold {thr_b:.0e} m2 at this site").map_err(err).unwrap();
        let (gridb1, gridbt) = (carve_grid(&chunks, 1.0, thr_b), carve_grid(&chunks, tuned, thr_b));
        let stacked = stage_of(gridb1.clone());
        let mut river: Vec<(planet_core::Vec3, f64, f64)> = Vec::new();
        for t in &chunks {
            let (Some(r0), Some(r1)) = (
                compute_chunk(&lat, &plain, radius, entry, *t, halo_rings(&plain)),
                compute_chunk(&lat, &stacked, radius, entry, *t, halo_rings(&stacked)),
            ) else {
                continue;
            };
            let f0 = ChunkField::new(world.base_field().unwrap(), lat, radius, &plain).with_top_level(entry).with_ready(Arc::new(r0));
            let f1 = ChunkField::new(world.base_field().unwrap(), lat, radius, &stacked).with_top_level(entry).with_ready(Arc::new(r1));
            for nd in nodes15.iter().filter(|n| planet_core::TileId::from_direction(map, n.center_direction(map), top) == *t) {
                for l in 0..=32u32 {
                    for k in 0..=32u32 {
                        let d = nd.sample_direction(map, 32, k, l);
                        if carve_at(&gridb1, d) > 1.0 {
                            river.push((d, f0.change_at_level(d, entry), f1.change_at_level(d, entry)));
                        }
                    }
                }
            }
        }
        if river.is_empty() {
            writeln!(out, "B no river vertex (a vertex the carve line lowers by more than 1 m) among the level-{entry} nodes")
                .map_err(err)
                .unwrap();
            return;
        }
        let mut s_none: Vec<f64> = river.iter().map(|r| r.1.abs()).collect();
        let mut s_stacked: Vec<f64> = river.iter().map(|r| r.2.abs()).collect();
        let mut s_rep1: Vec<f64> = river.iter().map(|r| (r.1 + carve_at(&gridb1, r.0).min((-r.1).max(0.0))).abs()).collect();
        let mut s_rept: Vec<f64> = river.iter().map(|r| (r.1 + carve_at(&gridbt, r.0).min((-r.1).max(0.0))).abs()).collect();
        writeln!(out, "B step level {entry} against {} along the river ({} vertices):", entry - 1, river.len()).map_err(err).unwrap();
        writeln!(out, "B   no carve: {}", stats(&mut s_none)).map_err(err).unwrap();
        writeln!(out, "B   stacked carve (scale 1): {}", stats(&mut s_stacked)).map_err(err).unwrap();
        writeln!(out, "B   replace (scale 1): {}", stats(&mut s_rep1)).map_err(err).unwrap();
        writeln!(out, "B   replace (tuned scale {tuned:.1}): {}", stats(&mut s_rept)).map_err(err).unwrap();
    }

    // spec: GEN-037
    /// One scale of the depth law for every site, replace mode (ADR 0084 option 1, no pre-solve): the scale of the sweep that minimises the pooled p99 of the height step
    /// along the rivers between the entry level and the level above, over the 24 trunk chunks, the findings 2 site and the findings 3 sites together, for the
    /// thresholds 1e8 and 1e7 m2 of the run's drainage area; the step p99 and max per group and the share of river vertices over 2 m.
    /// `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib carve_tuned_report -- --ignored --nocapture`. Read only.
    #[test]
    #[ignore = "a measurement: it solves chunks several times"]
    fn carve_tuned_report() {
        use planet_core::cube::{direction_to_face, TangentWarp};
        use planet_core::hash::SplitMix64;
        use planet_generators::anet::{BaseDetail, CoarseRun};
        use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField};
        use std::collections::HashMap;
        use std::sync::Arc;
        let mut out = std::io::stdout();
        let res: u32 = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let count: usize = std::env::var("PLANET_ANET_CHUNKS").ok().and_then(|r| r.parse().ok()).unwrap_or(24);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, maps, _) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let lat = planet_generators::chunkfield::ChunkLattice::new(crate::anetworld::cells_per_chunk_for(radius, 8.0)).unwrap();
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        let entry = crate::anetworld::ANET_TOP_LEVEL;
        let map = &TangentWarp;
        let coarse = maps.lattice();
        let depth = |scale: f64, area: f64| (scale * 3.0 * libm::sqrt(area / 1.0e8)).min(100.0 * scale.max(1.0));
        let cell_key = |f: u8, i: i64, j: i64| {
            (u64::from(f) << 56)
                | ((i.clamp(0, i64::from(res) - 1) as u64 & 0x0fff_ffff) << 28)
                | (j.clamp(0, i64::from(res) - 1) as u64 & 0x0fff_ffff)
        };
        // The carve grid of a scale round the given chunks: depth on the bake's lattice along the run's polylines from every cell over `thr`.
        let carve_grid = |tiles: &[planet_core::TileId], scale: f64, thr: f64| -> HashMap<u64, f32> {
            let mut grid: HashMap<u64, f32> = HashMap::new();
            let mut seen = std::collections::HashSet::new();
            for t in tiles {
                let mut ring = vec![coarse.cell_at(t.center_direction(map))];
                let mut all = ring.clone();
                for _ in 0..4 {
                    let mut next: Vec<usize> = Vec::new();
                    for &c in &ring {
                        for n in coarse.neighbours(c) {
                            let n = n as usize;
                            if n != planet_generators::coupled::NONE as usize && !all.contains(&n) && !next.contains(&n) {
                                next.push(n);
                            }
                        }
                    }
                    all.extend(next.iter().copied());
                    ring = next;
                }
                for c in all {
                    if !seen.insert(c) || f64::from(maps.area_m2(c)) < thr {
                        continue;
                    }
                    let r = maps.receiver(c);
                    if r == planet_generators::coupled::NONE {
                        continue;
                    }
                    let (a, b) = (coarse.direction(c), coarse.direction(r as usize));
                    let (area_a, area_b) = (f64::from(maps.area_m2(c)), f64::from(maps.area_m2(r as usize)));
                    let steps = (radius * a.angle_to(b) / 150.0).ceil().max(1.0) as u32;
                    for k in 0..=steps {
                        let l = f64::from(k) / f64::from(steps);
                        let p = (a * (1.0 - l) + b * l).normalized();
                        let (f, s, t2) = direction_to_face(map, p);
                        let n = f64::from(res);
                        let (i, j) = (((s + 1.0) * 0.5 * n) as i64, ((t2 + 1.0) * 0.5 * n) as i64);
                        let d = depth(scale, area_a * (1.0 - l) + area_b * l) as f32;
                        let e = grid.entry(cell_key(f.0, i, j)).or_insert(0.0);
                        *e = e.max(d);
                    }
                }
            }
            grid
        };
        let carve_at = |grid: &HashMap<u64, f32>, d: planet_core::Vec3| -> f64 {
            if grid.is_empty() {
                return 0.0;
            }
            let (f, s, t) = direction_to_face(map, d);
            let n = f64::from(res);
            let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
            let (i0, j0) = (libm::floor(x) as i64, libm::floor(y) as i64);
            let (wx, wy) = (x - i0 as f64, y - j0 as f64);
            let at = |i: i64, j: i64| f64::from(grid.get(&cell_key(f.0, i, j)).copied().unwrap_or(0.0));
            at(i0, j0) * (1.0 - wx) * (1.0 - wy)
                + at(i0 + 1, j0) * wx * (1.0 - wy)
                + at(i0, j0 + 1) * (1.0 - wx) * wy
                + at(i0 + 1, j0 + 1) * wx * wy
        };
        struct Carved {
            inner: crate::anetworld::WorldBase,
            grid: HashMap<u64, f32>,
            res: u32,
        }
        impl Carved {
            fn carve(&self, d: planet_core::Vec3) -> f64 {
                if self.grid.is_empty() {
                    return 0.0;
                }
                let (f, s, t) = direction_to_face(&TangentWarp, d);
                let n = f64::from(self.res);
                let (x, y) = ((s + 1.0) * 0.5 * n - 0.5, (t + 1.0) * 0.5 * n - 0.5);
                let (i0, j0) = (libm::floor(x) as i64, libm::floor(y) as i64);
                let (wx, wy) = (x - i0 as f64, y - j0 as f64);
                let key = |i: i64, j: i64| {
                    (u64::from(f.0) << 56)
                        | ((i.clamp(0, i64::from(self.res) - 1) as u64 & 0x0fff_ffff) << 28)
                        | (j.clamp(0, i64::from(self.res) - 1) as u64 & 0x0fff_ffff)
                };
                let at = |i: i64, j: i64| f64::from(self.grid.get(&key(i, j)).copied().unwrap_or(0.0));
                at(i0, j0) * (1.0 - wx) * (1.0 - wy)
                    + at(i0 + 1, j0) * wx * (1.0 - wy)
                    + at(i0, j0 + 1) * (1.0 - wx) * wy
                    + at(i0 + 1, j0 + 1) * wx * wy
            }
        }
        impl BaseDetail for Carved {
            fn base_height(&self, dir: planet_core::Vec3) -> f64 {
                self.inner.base_height(dir) - self.carve(dir)
            }
            fn base_heights(&self, dirs: &[planet_core::Vec3]) -> Vec<f64> {
                self.inner.base_heights(dirs).into_iter().zip(dirs).map(|(h, d)| h - self.carve(*d)).collect()
            }
            fn coarse_input(&self, dir: planet_core::Vec3) -> f64 {
                self.inner.coarse_input(dir)
            }
            fn region_heights(&self, dirs: &[planet_core::Vec3]) -> Vec<f64> {
                self.inner.region_heights(dirs).into_iter().zip(dirs).map(|(h, d)| h - self.carve(*d)).collect()
            }
            fn identity(&self) -> u64 {
                self.inner.identity() ^ u64::from(self.res)
            }
        }
        let stage_of = |grid: HashMap<u64, f32>| -> planet_generators::anet::ANet {
            let mut params = planet_generators::anet::ANetParams::spike(crate::anetworld::ANET_HALO_M, lat.cell_m(radius));
            params.drivers.seed = world.base_seed();
            planet_generators::anet::ANet {
                run: maps.clone(),
                base: Arc::new(Carved { inner: crate::anetworld::WorldBase::new(&world), grid, res }) as Arc<dyn BaseDetail>,
                radius_m: radius,
                params,
                memo: Default::default(),
            }
        };
        let stats = |v: &mut Vec<f64>| -> String {
            v.sort_by(f64::total_cmp);
            let q = |p: f64| v[(((v.len() - 1) as f64) * p) as usize];
            format!("p50 {:.2}, p99 {:.2}, p99.9 {:.2}, max {:.2} m", q(0.5), q(0.99), q(0.999), v[v.len() - 1])
        };
        // The groups: the 24 trunk chunks, the findings 2 site (frame-31358: the 3 chunks under its level-15 nodes) and the findings 3 sites (frames 41356 and
        // 41595). Per chunk the no-carve change at the entry level (the stage solved once, the pyramid up to `entry`) on a 129 x 129 grid; the variants are
        // then computed from it: the step against the level above is `|change + min(carve, max(0, -change))|` (replace mode).
        let per_face = 1u32 << top;
        let base = world.base_field().unwrap();
        let mut rng = SplitMix64(0x6368_616e_6765);
        let mut trunk: Vec<planet_core::TileId> = Vec::new();
        while trunk.len() < count {
            let face = planet_core::Face::ALL[(rng.next_u64() % 6) as usize];
            let (x, y) = ((rng.next_u64() % u64::from(per_face)) as u32, (rng.next_u64() % u64::from(per_face)) as u32);
            let t = planet_core::TileId::new(face, top, x, y).expect("a tile of the chunk grid");
            let c = coarse.cell_at(t.center_direction(map));
            let on_trunk = std::iter::once(c)
                .chain(coarse.neighbours(c).into_iter().map(|n| n as usize))
                .any(|n| n < coarse.len() && f64::from(maps.area_m2(n)) >= 1.0e9);
            if on_trunk && base.height(t.center_direction(map)) > 0.0 {
                trunk.push(t);
            }
        }
        let chunks_of_bundle = |name: &str| -> Vec<planet_core::TileId> {
            let path = std::path::Path::new("C:/DEV/SeriousPlanetData/owner-test-2026-10-27").join(name);
            let b = planet_frame::TerrainBundle::from_text(&std::fs::read_to_string(&path).expect("the bundle reads"))
                .expect("a terrain bundle");
            let nodes = planet_frame::select_nodes_in_view_field(map, &b.request.camera, &*world.field().unwrap(), &b.request.lod).nodes;
            let mut v: Vec<planet_core::TileId> = Vec::new();
            for n in nodes.iter().filter(|n| n.tile.level() == entry) {
                let c = planet_core::TileId::from_direction(map, n.tile.center_direction(map), top);
                if !v.contains(&c) {
                    v.push(c);
                }
            }
            v
        };
        let site2 = chunks_of_bundle("frame-31358.bundle");
        let mut site3 = chunks_of_bundle("frame-41356.bundle");
        for c in chunks_of_bundle("frame-41595.bundle") {
            if !site3.contains(&c) {
                site3.push(c);
            }
        }
        let groups: Vec<(&str, Vec<planet_core::TileId>)> =
            vec![("24 trunk chunks", trunk), ("findings 2 site", site2), ("findings 3 sites", site3)];
        let plain = stage_of(HashMap::new());
        let mut changes: Vec<Vec<(planet_core::Vec3, f64)>> = Vec::new();
        for (name, chunks) in &groups {
            let mut pts = Vec::new();
            for t in chunks {
                let Some(r) = compute_chunk(&lat, &plain, radius, entry, *t, halo_rings(&plain)) else { continue };
                let field = ChunkField::new(world.base_field().unwrap(), lat, radius, &plain).with_top_level(entry).with_ready(Arc::new(r));
                for l in 0..129u32 {
                    for k in 0..129u32 {
                        let d = t.sample_direction(map, 128, k, l);
                        pts.push((d, field.change_at_level(d, entry)));
                    }
                }
            }
            writeln!(out, "group {name}: {} chunks, {} points", chunks.len(), pts.len()).map_err(err).unwrap();
            changes.push(pts);
        }
        let all_chunks: Vec<planet_core::TileId> = groups.iter().flat_map(|g| g.1.iter().copied()).collect();
        let scales = [1.0, 2.0, 3.0, 4.8, 6.0, 8.0, 12.0, 16.0];
        for thr_v in [1.0e8, 1.0e7] {
            let line = carve_grid(&all_chunks, 1.0, thr_v);
            let grids: Vec<HashMap<u64, f32>> = scales.iter().map(|&s| carve_grid(&all_chunks, s, thr_v)).collect();
            // The river vertices of a group: the points the line of the threshold passes (any depth over 0.1 m at scale 1).
            let river: Vec<Vec<usize>> = changes
                .iter()
                .map(|pts| pts.iter().enumerate().filter(|(_, (d, _))| carve_at(&line, *d) > 0.1).map(|(i, _)| i).collect())
                .collect();
            let step = |g: usize, si: Option<usize>| -> Vec<f64> {
                river[g]
                    .iter()
                    .map(|&i| {
                        let (d, c) = changes[g][i];
                        match si {
                            None => c.abs(),
                            Some(si) => (c + carve_at(&grids[si], d).min((-c).max(0.0))).abs(),
                        }
                    })
                    .collect()
            };
            let pooled_p99 = |si: Option<usize>| -> f64 {
                let mut v: Vec<f64> = (0..groups.len()).flat_map(|g| step(g, si)).collect();
                v.sort_by(f64::total_cmp);
                v[(((v.len() - 1) as f64) * 0.99) as usize]
            };
            let mut best = (f64::MAX, 0usize);
            for si in 0..scales.len() {
                let p = pooled_p99(Some(si));
                writeln!(out, "  threshold {thr_v:.0e}: scale {:.1}: pooled step p99 {p:.2} m", scales[si]).map_err(err).unwrap();
                if p < best.0 {
                    best = (p, si);
                }
            }
            writeln!(
                out,
                "threshold {thr_v:.0e}: pooled no carve p99 {:.2} m; best single scale {:.1} (p99 {:.2} m)",
                pooled_p99(None),
                scales[best.1],
                best.0
            )
            .map_err(err)
            .unwrap();
            for g in 0..groups.len() {
                for (label, si) in [("no carve", None), ("replace, best scale", Some(best.1))] {
                    let mut v = step(g, si);
                    let over2 = v.iter().filter(|x| **x > 2.0).count();
                    writeln!(
                        out,
                        "RESULT threshold {thr_v:.0e} {} [{label}]: {} river vertices, step {}; over 2 m: {:.1} %",
                        groups[g].0,
                        v.len(),
                        stats(&mut v),
                        100.0 * over2 as f64 / v.len().max(1) as f64
                    )
                    .map_err(err)
                    .unwrap();
                }
            }
        }
    }

    // spec: GEN-052
    /// How many chunks of the chunk grid (tile level 11) a river of the coupled run crosses, for thresholds of the drainage area (`PLANET_RIVER_THRESHOLDS`,
    /// default 1e7, 1e8 and 1e9 m2): every land cell of the run over the threshold, its polyline to its receiver stepped every 2.4 km (half a chunk) and the
    /// chunks it passes counted once; with the run cells over the threshold. For the cost of a stored incision channel (world-build hours = chunks times the
    /// solve time over the chunk workers). `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib river_chunk_count_report -- --ignored
    /// --nocapture`. Read only.
    #[test]
    #[ignore = "a measurement: it walks the run"]
    fn river_chunk_count_report() {
        use planet_core::cube::TangentWarp;
        use planet_generators::anet::CoarseRun;
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, maps, _) = anet_scene(res, 11, store, &mut out).unwrap();
        let radius = world.radius_m();
        let coarse = maps.lattice();
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        let thresholds: Vec<f64> = std::env::var("PLANET_RIVER_THRESHOLDS")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
            .unwrap_or_else(|| vec![1e7, 1e8, 1e9]);
        let per_face = 1u64 << top;
        let land = (0..coarse.len()).filter(|&c| maps.z(c) > 0.0).count();
        writeln!(out, "the run: {} cells, {land} land cells; {} chunks in the grid", coarse.len(), 6 * per_face * per_face)
            .map_err(err)
            .unwrap();
        for thr in thresholds {
            let mut chunks = std::collections::HashSet::new();
            let mut cells = 0usize;
            let mut length_km = 0.0f64;
            for c in 0..coarse.len() {
                if maps.z(c) <= 0.0 || f64::from(maps.area_m2(c)) < thr {
                    continue;
                }
                let r = maps.receiver(c);
                if r == planet_generators::coupled::NONE {
                    continue;
                }
                cells += 1;
                let (a, b) = (coarse.direction(c), coarse.direction(r as usize));
                let len = radius * a.angle_to(b);
                length_km += len / 1e3;
                let steps = (len / 2_400.0).ceil().max(1.0) as u32;
                for k in 0..=steps {
                    let l = f64::from(k) / f64::from(steps);
                    let t = planet_core::TileId::from_direction(&TangentWarp, (a * (1.0 - l) + b * l).normalized(), top);
                    chunks.insert((t.face().0, t.x(), t.y()));
                }
            }
            writeln!(out, "RESULT rivers over {thr:.0e} m2: {cells} run cells, {length_km:.0} km of polyline, {} chunks", chunks.len())
                .map_err(err)
                .unwrap();
        }
    }

    // spec: LOD-013
    /// The boundary between the chunk's entry level (`ANET_TOP_LEVEL`, level 15: `level_color` (250, 140, 130), the light red of the LOD debug view) and the level
    /// above it at the pose of an F9 bundle (owner test finding 3, frame-50119): for every level-15 node of the plan with a level-14 neighbour, along the shared
    /// edge at the level-15 vertices, the difference of the two surfaces (the level-14 tile's linear interpolation of its own vertices, the level-15 tile's
    /// vertices): the chunk's change (level 15 only), the base's interpolation error (the base is the same field at both levels, a coarser tile samples it
    /// twice as far apart) and their sum, and the edge samples where the two surfaces lie on different sides of sea level (a shoreline that moves). The chunks
    /// under the level-15 nodes are landed first. `PLANET_BUNDLE=FILE PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib boundary_report
    /// -- --ignored --nocapture`. Read only.
    #[test]
    #[ignore = "a measurement: it solves the chunks under a view"]
    fn boundary_report() {
        use planet_core::cube::TangentWarp;
        use planet_core::Side;
        let mut out = std::io::stdout();
        let bundle = std::env::var_os("PLANET_BUNDLE").map(std::path::PathBuf::from).expect("PLANET_BUNDLE=FILE");
        let b =
            planet_frame::TerrainBundle::from_text(&std::fs::read_to_string(&bundle).expect("the bundle reads")).expect("a terrain bundle");
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, _tile) = anet_scene(res, 11, store, &mut out).unwrap();
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
        let entry = crate::anetworld::ANET_TOP_LEVEL;
        let lod = b.request.lod.clone();
        let sel = planet_frame::select_nodes_in_view_field(&TangentWarp, &b.request.camera, &*world.field().unwrap(), &lod);
        let nodes: std::collections::HashSet<planet_core::TileId> = sel.nodes.iter().map(|n| n.tile).collect();
        let fine: Vec<planet_core::TileId> = nodes.iter().copied().filter(|t| t.level() >= entry).collect();
        let manager = world.chunk_manager().unwrap();
        let camera = b.request.camera.position;
        let start = std::time::Instant::now();
        loop {
            world.chunk_frame(&fine, camera, &[]);
            std::thread::sleep(std::time::Duration::from_millis(100));
            if fine.iter().all(|t| manager.reads(*t).iter().all(|k| manager.is_ready(*k))) && manager.in_flight() == 0 {
                break;
            }
            assert!(start.elapsed().as_secs() < 1_800, "the chunks did not land");
        }
        let field = world.field().unwrap();
        let base = world.base_field().unwrap();
        let level15: Vec<planet_core::TileId> = nodes.iter().copied().filter(|t| t.level() == entry).collect();
        let (mut geo, mut chunk, mut total) = (Vec::<f64>::new(), Vec::<f64>::new(), Vec::<f64>::new());
        let (mut edges, mut crossings) = (0usize, 0usize);
        for t in &level15 {
            for side in [Side::East, Side::West, Side::North, Side::South] {
                let nb = t.neighbor(&TangentWarp, side);
                if nodes.contains(&nb) || !nb.parent().is_some_and(|p| nodes.contains(&p)) {
                    continue;
                }
                edges += 1;
                // The 33 vertices of the shared edge of `t`.
                let pts: Vec<planet_core::Vec3> = (0..=32u32)
                    .map(|k| match side {
                        Side::West => t.sample_direction(&TangentWarp, 32, 0, k),
                        Side::East => t.sample_direction(&TangentWarp, 32, 32, k),
                        Side::North => t.sample_direction(&TangentWarp, 32, k, 0),
                        Side::South => t.sample_direction(&TangentWarp, 32, k, 32),
                    })
                    .collect();
                let h15: Vec<f64> = pts.iter().map(|d| field.height_at_level(*d, entry)).collect();
                let h14: Vec<f64> = pts.iter().map(|d| field.height_at_level(*d, entry - 1)).collect();
                let hb: Vec<f64> = pts.iter().map(|d| base.height(*d)).collect();
                for k in 0..=32usize {
                    let (s14, geo_k) =
                        if k % 2 == 0 { (h14[k], 0.0) } else { (0.5 * (h14[k - 1] + h14[k + 1]), hb[k] - 0.5 * (hb[k - 1] + hb[k + 1])) };
                    geo.push(geo_k.abs());
                    chunk.push((h15[k] - h14[k]).abs());
                    total.push((h15[k] - s14).abs());
                    if (h15[k] > 0.5 && s14 < -0.5) || (h15[k] < -0.5 && s14 > 0.5) {
                        crossings += 1;
                    }
                }
            }
        }
        writeln!(
            out,
            "pose of {}: {} level-{entry} nodes, {edges} edges against a level-{} neighbour",
            bundle.display(),
            level15.len(),
            entry - 1
        )
        .map_err(err)
        .unwrap();
        let line = |name: &str, v: &mut Vec<f64>| -> String {
            if v.is_empty() {
                return format!("{name}: none");
            }
            v.sort_by(f64::total_cmp);
            format!("{name}: p50 {:.3}, p99 {:.3}, max {:.3} m", v[v.len() / 2], v[(v.len() - 1) * 99 / 100], v[v.len() - 1])
        };
        writeln!(out, "RESULT  chunk change at the edge: {}", line("step", &mut chunk)).map_err(err).unwrap();
        writeln!(out, "RESULT  base interpolation error of the coarser tile (odd vertices): {}", line("error", &mut geo))
            .map_err(err)
            .unwrap();
        writeln!(out, "RESULT  total surface difference at the edge vertices: {}", line("total", &mut total)).map_err(err).unwrap();
        writeln!(
            out,
            "RESULT  edge samples where the two surfaces lie on different sides of sea level (beyond 0.5 m): {crossings} of {}",
            total.len()
        )
        .map_err(err)
        .unwrap();
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
}
