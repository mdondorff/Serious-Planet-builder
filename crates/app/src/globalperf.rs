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
