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

/// The GEN-052 budget: an A-net chunk of 24 km2 takes at most 4 s at 8 m cells (one thread).
pub(crate) fn anet_chunk_budget_failures(cell_m: f64, secs: f64) -> Vec<String> {
    if cell_m == 8.0 && !(secs.is_finite() && secs <= 4.0) {
        vec![format!("A-net chunk time {secs:.2} s at 8 m exceeds 4 s (GEN-052)")]
    } else {
        Vec::new()
    }
}

// spec: GEN-052
/// Time one A-net chunk (the chunk of the coupled run's largest land river) at 8, 4 and 10 m with the 3 km halo on one thread after the ADR 0020
/// readiness gate. The world is the default eroded one over a bake of `--global-res` cells (default 256) and its coupled run; the chunk time
/// does not depend on the run's resolution, only on the cells of the chunk and its rivers.
pub fn perf_anet_chunk(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    use crate::anetworld::{cells_per_chunk_for, WorldBase, ANET_HALO_M};
    use crate::coupledworld::Standing;
    use crate::worldfield::WorldTerrain;
    use planet_generators::anet::{ANet, ANetParams};
    use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkLattice, PYRAMID_TOP_LEVEL};
    use std::sync::Arc;
    planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = crate::readiness_gate(o, ctx, out)?;
    let res = if o.given.contains("--global-res") { o.global_res } else { 256 };
    let def = planet_world_def::WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, o.seed);
    let world = Arc::new(WorldTerrain::open(def, res)?);
    let radius = world.radius_m();
    writeln!(out, "A-net chunk perf: bake {res} cells per face edge, coupled run {} steps, 1 thread", crate::coupledworld::COUPLED_STEPS)
        .map_err(err)?;
    world.request_coupled();
    let maps = loop {
        match world.coupled_standing() {
            Standing::Ready(m, _) => break m,
            Standing::Failed(e) => return Err(e),
            _ => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    // The chunk of the land cell with the largest drainage area that drains on.
    let lat_c = maps.lattice();
    let rec = maps.receivers();
    let mut best: Option<(usize, f32)> = None;
    for c in 0..lat_c.len() {
        let a = maps.network_area_m2(c);
        if maps.z(c) > 0.0 && rec[c] != planet_generators::coupled::NONE && best.map_or(true, |(_, b)| a > b) {
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
    let mut failures = Vec::new();
    let mut rows = Vec::new();
    for cell_m in [8.0, 4.0, 10.0] {
        let per = cells_per_chunk_for(radius, cell_m);
        let lat = ChunkLattice::new(per).ok_or("no lattice")?;
        let stage = ANet {
            run: maps.clone(),
            base: Arc::new(WorldBase::new(&world).with_threads(1)),
            radius_m: radius,
            params: ANetParams::spike(ANET_HALO_M, lat.cell_m(radius)),
        };
        let halo = halo_rings(&stage);
        // The share of the base field: its heights over the region, one thread.
        let reg = planet_generators::chunkfield::region(&lat, tile, halo);
        let dirs: Vec<planet_core::Vec3> = reg.cells.iter().map(|&c| lat.direction(c)).collect();
        let t0 = Instant::now();
        let _ = planet_generators::anet::BaseDetail::base_heights(&*stage.base, &dirs);
        let base_secs = t0.elapsed().as_secs_f64();
        let start = Instant::now();
        let r = compute_chunk(&lat, &stage, radius, PYRAMID_TOP_LEVEL, tile, halo).ok_or("the chunk was refused")?;
        let secs = start.elapsed().as_secs_f64();
        let cells = (per as usize + 2 * halo as usize).pow(2);
        writeln!(
            out,
            "{cell_m} m: {per} cells per chunk, halo {halo} rings, about {cells} cells with the halo: {secs:.2} s, the base field of it {base_secs:.2} s (hash {:016x})",
            r.hash()
        )
        .map_err(err)?;
        failures.extend(anet_chunk_budget_failures(cell_m, secs));
        rows.push(format!("{{\"cell_m\": {cell_m}, \"cells_per_chunk\": {per}, \"halo_rings\": {halo}, \"seconds\": {secs:.3}}}"));
    }
    let json = format!(
        "{{\n  \"scene\": \"anet-chunk\",\n  \"adapter\": \"{}\",\n  \"owner_override\": {:?},\n  \"res\": {res},\n  \"threads\": 1,\n  \"chunks\": [{}],\n  \"budget_failures\": {:?}\n}}\n",
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
