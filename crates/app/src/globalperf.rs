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
    let json = planet_perf::stamp_one_off_json(
        &json,
        &planet_perf::run_validity(),
        &[planet_perf::OneOffReading { metric: "bake seconds (GEN-014)", reading: secs, budget: 600.0 }],
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
    let json = planet_perf::stamp_one_off_json(
        &json,
        &planet_perf::run_validity(),
        &[planet_perf::OneOffReading { metric: "coupled seconds (GEN-042)", reading: secs, budget: 600.0 }],
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
    let json = planet_perf::stamp_one_off_json(
        &json,
        &planet_perf::run_validity(),
        &[planet_perf::OneOffReading { metric: "coupled make seconds (GEN-042)", reading: secs, budget: 600.0 }],
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

/// The world of the reports: the default eroded world over a bake of `res` cells, its coupled run, and the chunk of the land cell with the
/// largest drainage area that drains on, with that area in m2.
#[cfg(test)]
fn river_scene(
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
        "report scene: bake {res} cells per face edge{}, coupled run {} steps",
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

    // spec: TEST-064
    /// Measured on the base field: the step between a tile level and its parent. For each level 12 to 16 the tiles under the tile of the largest
    /// river are generated as the field gives them (`generate_tile_field`, heights rounded to f32) and every vertex of a tile that is also a vertex
    /// of its parent (the even `k`, `l`) is compared with the parent's height there. A vertex has one direction (COORD-007) and the base height
    /// does not depend on the level, so the steps are 0 or rounding; a value over 0.25 m fails the gate (TEST-064). Printed:
    /// `PLANET_ANET_STORE=DIR PLANET_ANET_RES=N cargo test -p planet --release --lib base_steps_report -- --ignored --nocapture`.
    /// A measurement of values, read only.
    #[test]
    #[ignore = "a measurement: it prints the steps of the base field between a level and its parent around one river"]
    fn base_steps_report() {
        use planet_generators::height::generate_tile_field;
        let mut out = std::io::stdout();
        let res = std::env::var("PLANET_ANET_RES").ok().and_then(|r| r.parse().ok()).unwrap_or(256);
        let store = std::env::var_os("PLANET_ANET_STORE").map(std::path::PathBuf::from);
        let (world, _maps, tile) = river_scene(res, 11, store, &mut out).unwrap();
        let base = world.field().unwrap();
        let map = &planet_core::cube::TangentWarp;
        let cells = planet_generators::chunk::TILE_CELLS;
        let top = planet_generators::chunk::CHUNK_GRID_LEVEL;
        writeln!(
            out,
            "steps of the base field alone against the parent (tile level L vs L - 1, at the parent's vertices, under the tile {tile:?}):"
        )
        .unwrap();
        let mut parents = std::collections::HashMap::new();
        for level in top + 1..=top + 5 {
            let mut tiles = vec![tile];
            for _ in top..level {
                tiles = tiles.iter().flat_map(|t| t.children().expect("children")).collect();
            }
            let mut d: Vec<f64> = Vec::new();
            for t in &tiles {
                let parent = t.parent().expect("parent");
                let p = parents.entry(parent).or_insert_with(|| generate_tile_field(&*base, map, parent, cells));
                let (qx, qy) = ((t.x() & 1) * (cells / 2), (t.y() & 1) * (cells / 2));
                let child = generate_tile_field(&*base, map, *t, cells);
                for l in (0..=cells).step_by(2) {
                    for k in (0..=cells).step_by(2) {
                        d.push(f64::from(child.at(k, l) - p.at(qx + k / 2, qy + l / 2)).abs());
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
            parents.clear();
        }
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
        let (world, _maps, tile) = river_scene(res, 11, store, &mut out).unwrap();
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
        }
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
