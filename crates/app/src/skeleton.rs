//! The M1 walking skeleton: generator -> FramePlan -> GPU executor -> pixels, driven from all three run modes.

use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{Face, PlanetFixed, Side, TileId, Vec3};
use planet_frame::{plan_tiles, DebugView, FramePlan, PlanRequest, ReproBundle};
use planet_generators::{generate_tile, TileData, GENERATOR_VERSION, IMPLEMENTATION_ID};
use planet_render::{execute, AdapterPolicy, ExecStats, GpuContext, RgbaFrame, TileResource};
use std::io::Write;
use std::path::Path;
use std::time::Instant;

const FRAME_SIZE: (u32, u32) = (256, 128);
pub(crate) const RADIUS_M: f64 = 6_371_000.0;

/// `FACE,LEVEL,X,Y`
pub fn parse_tile(s: &str) -> Result<TileId, String> {
    let p: Vec<&str> = s.split(',').collect();
    let [f, l, x, y] = p.as_slice() else {
        return Err(format!("--tile expects FACE,LEVEL,X,Y, got '{s}'"));
    };
    let n = |v: &str| v.trim().parse::<u32>().map_err(|e| format!("--tile '{s}': {e}"));
    TileId::new(Face(n(f)? as u8), n(l)? as u8, n(x)?, n(y)?)
        .ok_or_else(|| format!("--tile '{s}' is not a valid tile (face 0..5, level 0..28, x and y below 2^level)"))
}

fn default_tiles() -> Vec<TileId> {
    let a = TileId::new(Face(3), 3, 5, 2).expect("valid");
    vec![a, a.neighbor(&TangentWarp, Side::East)]
}

fn tiles_of(o: &Options) -> Result<Vec<TileId>, String> {
    if o.tiles.is_empty() {
        Ok(default_tiles())
    } else {
        o.tiles.iter().map(|t| parse_tile(t)).collect()
    }
}

fn request_of(o: &Options) -> Result<PlanRequest, String> {
    let view = DebugView::parse(&o.view).ok_or_else(|| format!("--view must be face, tile-id or height, got '{}'", o.view))?;
    Ok(PlanRequest {
        tiles: tiles_of(o)?,
        // A fixed camera 12,000 km from the centre; the skeleton only needs a defined origin for camera-relative values.
        camera: PlanetFixed(Vec3::new(12.0e6, 1.0e6, -3.0e6)),
        size: FRAME_SIZE,
        view,
        radius_m: RADIUS_M,
        face_mapping: TangentWarp.id().to_string(),
    })
}

fn run_plan(ctx: &GpuContext, plan: &FramePlan, tiles: &[TileData]) -> Result<(RgbaFrame, ExecStats), String> {
    let resources: Vec<TileResource> = tiles.iter().map(|t| TileResource { id: t.id, res: t.res, heights: &t.heights }).collect();
    execute(ctx, plan, &resources).map_err(|e| e.to_string())
}

fn write_frame(frame: &RgbaFrame, path: &Path, out: &mut dyn Write) -> Result<(), String> {
    frame.write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

pub fn render_tiles(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let request = request_of(o)?;
    let plan = plan_tiles(&request).map_err(|e| e.to_string())?;
    let tiles: Vec<TileData> = request.tiles.iter().map(|&id| generate_tile(o.seed, &TangentWarp, id, o.res)).collect();
    let (frame, stats) = run_plan(ctx, &plan, &tiles)?;
    writeln!(
        out,
        "plan {:016x}: {} draws, {} draw calls, {} texture bytes",
        plan.plan_hash(),
        plan.draws.len(),
        stats.draw_calls,
        stats.texture_bytes_uploaded
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        write_frame(&frame, path, out)?;
    }
    if let Some(path) = &o.bundle_out {
        let bundle = ReproBundle {
            generator_version: GENERATOR_VERSION,
            implementation_id: IMPLEMENTATION_ID.to_string(),
            seed: o.seed,
            res: o.res,
            definition_hash: definition_hash(o.seed, RADIUS_M),
            adapter: ctx.adapter_key(),
            request,
            plan,
        };
        std::fs::write(path, bundle.to_text()).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote repro bundle {}", path.display()).map_err(err)?;
    }
    Ok(())
}

/// `editor --offscreen`: the same skeleton as the viewer will run, without a window (the window arrives in M2).
pub fn render_offscreen(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let ctx = GpuContext::new(o.adapter.unwrap_or(AdapterPolicy::Software)).map_err(|e| e.to_string())?;
    writeln!(out, "{}", ctx.adapter_log_line()).map_err(err)?;
    if let Some(bundle) = &o.repro {
        return replay(&ctx, bundle, o.out.as_deref(), out);
    }
    render_tiles(&ctx, o, out)
}

/// Hash of the definition a seed and radius describe: the single planet (WDEF-003). Bundles of the seed-only runs mean the
/// reference field (`cpu-ref-height-v2`) by this hash although the definition names no field: replay re-plans with it. Bundles record it; this build can
/// rebuild only that definition, so replay checks the recorded hash against it.
fn definition_hash(seed: u64, radius_m: f64) -> u64 {
    planet_world_def::WorldDefinition::single_planet(radius_m, seed).hash()
}

/// Refuse a bundle recorded against a definition this build cannot rebuild (CON-19): a hash of zero predates the
/// definition, any other hash must equal the single planet of the recorded seed and radius.
fn check_definition(recorded: u64, seed: u64, radius_m: f64) -> Result<(), String> {
    let rebuilt = definition_hash(seed, radius_m);
    if recorded == rebuilt {
        return Ok(());
    }
    if recorded == 0 {
        return Err(format!(
            "bundle records no definition hash (it predates the WorldDefinition); this build rebuilds the single planet of seed {seed} as {rebuilt:016x}: record the bundle again"
        ));
    }
    Err(format!(
        "bundle was recorded against definition {recorded:016x}, but this build can only rebuild the single planet of seed {seed} and radius {radius_m} m ({rebuilt:016x}); the definition store arrives with authored layers"
    ))
}

/// Replay a repro bundle: refuse on version mismatch, rebuild the plan from the recorded request, compare, render.
pub fn replay(ctx: &GpuContext, path: &Path, png: Option<&Path>, out: &mut dyn Write) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    if text.starts_with("planet-terrain-repro") {
        return replay_terrain(ctx, &text, path, png, out);
    }
    let b = ReproBundle::from_text(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if b.generator_version != GENERATOR_VERSION || b.implementation_id != IMPLEMENTATION_ID {
        return Err(format!(
            "bundle was recorded with generator {} ({}), this build has {} ({}); check out the matching commit to replay it",
            b.generator_version, b.implementation_id, GENERATOR_VERSION, IMPLEMENTATION_ID
        ));
    }
    check_definition(b.definition_hash, b.seed, RADIUS_M)?;
    let rebuilt = plan_tiles(&b.request).map_err(|e| e.to_string())?;
    let diff = rebuilt.diff(&b.plan);
    if diff.is_empty() && rebuilt.plan_hash() != b.plan.plan_hash() {
        return Err(format!(
            "replayed plan hash {:016x} differs from the recorded {:016x} although no field differs: the plan text form changed",
            rebuilt.plan_hash(),
            b.plan.plan_hash()
        ));
    }
    if !diff.is_empty() {
        return Err(format!("replayed plan differs from the recorded plan:\n  {}", diff.join("\n  ")));
    }
    writeln!(out, "replay: plan identical ({:016x}), recorded on adapter {}", rebuilt.plan_hash(), b.adapter).map_err(err)?;
    let map = planet_frame::plan::face_mapping_by_id(&b.request.face_mapping)
        .ok_or_else(|| format!("unknown face mapping '{}'", b.request.face_mapping))?;
    let tiles: Vec<TileData> = b.request.tiles.iter().map(|&id| generate_tile(b.seed, map.as_ref(), id, b.res)).collect();
    let (frame, stats) = run_plan(ctx, &rebuilt, &tiles)?;
    writeln!(out, "replay: {} draw calls", stats.draw_calls).map_err(err)?;
    if let Some(p) = png {
        write_frame(&frame, p, out)?;
    }
    Ok(())
}

/// `generate`: headless CPU generation of one tile with its hash, height map and optionally the face net.
pub fn generate(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    if o.repro.is_some() || o.bundle_out.is_some() {
        return Err("generate has no frames: --repro and --bundle belong to test-render and editor --offscreen".into());
    }
    if let Some(path) = &o.manifest {
        if !o.tiles.is_empty() || o.out.is_some() || o.face_net.is_some() {
            return Err("--manifest writes a fixed tile set; it excludes --tile, --out and --face-net".into());
        }
        if let Some(f) = crate::STORE_OPTIONS.iter().find(|f| o.given.contains(**f)) {
            return Err(format!("--manifest writes a fixed tile set; it excludes {f} (a tile store option)"));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        let text = crate::manifest::full_manifest();
        std::fs::write(path, &text).map_err(|e| format!("writing {}: {e}", path.display()))?;
        return writeln!(out, "wrote {} ({} lines)", path.display(), text.lines().count()).map_err(err);
    }
    if o.compact_cache.is_some() {
        return compact_cache(o, out);
    }
    if o.plan_check.is_some() {
        return crate::plancheck::plan_check(o, out);
    }
    if o.prefill_dir.is_some() {
        if o.global_dir.is_some() {
            return Err("--prefill-dir and --global-dir are separate commands: --global-dir pre-fills after the bake by itself".into());
        }
        return crate::prefill::prefill_dir(o, out);
    }
    if o.prefill_levels.is_some() && o.global_dir.is_none() {
        return Err("--prefill-levels applies to --global-dir (the pre-fill after the bake) and to --prefill-dir".into());
    }
    if o.global_dir.is_some() || o.verify_global.is_some() || o.global_products.is_some() {
        return generate_global(o, out);
    }
    if o.cap_bytes.is_some() {
        return Err("--cap-bytes belongs to --compact-cache".into());
    }
    if o.cache_dir.is_some() || o.verify_cache.is_some() {
        return generate_store(o, out);
    }
    if let Some(f) = crate::STORE_OPTIONS.iter().find(|f| o.given.contains(**f)) {
        return Err(format!("{f} belongs to the tile store commands: use it with --cache-dir (and --region) or --verify-cache"));
    }
    let id = tiles_of(o)?[0];
    let tile = generate_tile(o.seed, &TangentWarp, id, o.res);
    writeln!(
        out,
        "tile {:#018x} face {} level {} ({}, {}): content hash {:016x}, cache key {:016x}",
        id.raw(),
        id.face().0,
        id.level(),
        id.x(),
        id.y(),
        tile.content_hash(),
        planet_generators::cache_key(&TangentWarp, o.seed, id, o.res)
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        planet_generators::dump::tile_height_map(&tile).write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    if let Some(path) = &o.face_net {
        planet_generators::dump::face_net(&TangentWarp, 128, 2).write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote {}", path.display()).map_err(err)?;
    }
    Ok(())
}

/// The definition `generate --cache-dir` and `--verify-cache` work on: a TOML file, or the default single planet.
fn world_of(o: &Options) -> Result<planet_cache::tiles::World, String> {
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => planet_world_def::WorldDefinition::single_planet(RADIUS_M, o.seed),
    };
    planet_cache::tiles::World::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))
}

/// The definition of the global commands: a TOML file, or the default single planet.
pub(crate) fn global_world_of(o: &Options) -> Result<planet_cache::global_store::GlobalWorld, String> {
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => planet_world_def::WorldDefinition::single_planet(RADIUS_M, o.seed),
    };
    planet_cache::global_store::GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))
}

/// `generate --global-dir DIR`: bake the global-coarse field (continents, elevation, climate) into a tile store as blocks and
/// print the time per sample. `--verify-global DIR`: regenerate a sample of the stored blocks and compare.
/// `--global-products DIR`: write cube-net and map images of an in-memory bake for review.
fn generate_global(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let modes = [o.global_dir.is_some(), o.verify_global.is_some(), o.global_products.is_some()];
    if modes.iter().filter(|m| **m).count() > 1 {
        return Err("--global-dir, --verify-global and --global-products are separate commands: give one".into());
    }
    let clash: Vec<&str> = ["--tile", "--out", "--face-net", "--manifest", "--smoke", "--cache-dir", "--verify-cache", "--region", "--res"]
        .into_iter()
        .filter(|f| o.given.contains(*f))
        .collect();
    if !clash.is_empty() {
        return Err(format!("the global bake commands exclude {}", clash.join(", ")));
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    let threads = if o.threads == 0 { std::thread::available_parallelism().map_or(1, |n| n.get()) } else { o.threads };
    let world = global_world_of(o)?;
    if let Some(dir) = &o.verify_global {
        if o.given.contains("--global-res") {
            return Err("--verify-global reads the blocks' own grid: it excludes --global-res".into());
        }
        if o.given.contains("--threads") {
            return Err("--verify-global regenerates blocks one by one: it excludes --threads".into());
        }
        if !dir.is_dir() {
            return Err(format!("{}: no such cache directory (check the path; an empty store would verify nothing)", dir.display()));
        }
        let store = planet_cache::pack::TileStore::open(dir, world.params()).map_err(|e| e.to_string())?;
        for p in store.problems() {
            writeln!(out, "WARNING: {p}").map_err(err)?;
        }
        let report = planet_cache::global_store::verify_global(&store, &world, o.sample);
        writeln!(
            out,
            "verify-global {}: checked {} of {} entries: {} match, {} stale (versions or node set changed), {} stale without a known reason, {} of another kind, {} differ, {} unreadable",
            dir.display(),
            report.checked,
            store.len(),
            report.matching,
            report.stale,
            report.unexplained_stale,
            report.unsupported,
            report.mismatches.len(),
            report.unreadable.len()
        )
        .map_err(err)?;
        for m in &report.mismatches {
            writeln!(out, "MISMATCH block {:#018x} key {} in {} at offset {}: {}", m.tile.raw(), m.key, m.pack, m.offset, m.detail)
                .map_err(err)?;
        }
        for u in &report.unreadable {
            writeln!(out, "UNREADABLE {u}").map_err(err)?;
        }
        let stale = report.stale + report.unexplained_stale;
        if report.clean() && report.compared_nothing() {
            return Err(if stale == 0 {
                format!("verify-global compared nothing: {} entries looked at, none of them a block of this definition", report.checked)
            } else if !o.allow_stale {
                format!("verify-global compared nothing: all {stale} entries are stale; check --world/--seed, or pass --allow-stale if the definition changed on purpose")
            } else {
                return Ok(());
            });
        }
        return if report.passed_allowing_stale() {
            Ok(())
        } else {
            Err(format!("verify-global found {} differing and {} unreadable entries", report.mismatches.len(), report.unreadable.len()))
        };
    }
    if o.given.contains("--sample") || o.given.contains("--allow-stale") {
        return Err("--sample and --allow-stale only apply to --verify-global".into());
    }
    if let Some(dir) = &o.global_products {
        let grid = planet_generators::global::bake(world.seed(), &TangentWarp, o.global_res, threads).map_err(|e| e.to_string())?;
        for ch in planet_generators::global::Channel::ALL {
            let path = dir.join(format!("{}-net.png", ch.name()));
            planet_generators::global::product(&grid, ch).write_png(&path).map_err(|e| format!("writing {}: {e}", path.display()))?;
            let map = dir.join(format!("{}-map.png", ch.name()));
            planet_generators::global::equirect_product(world.seed(), (o.global_res * 4).min(2048), ch)
                .write_png(&map)
                .map_err(|e| format!("writing {}: {e}", map.display()))?;
        }
        return writeln!(out, "wrote 4 cube nets and 4 maps of a {}-cell bake to {}", o.global_res, dir.display()).map_err(err);
    }
    let dir = o.global_dir.as_ref().expect("checked by the caller");
    let (level, cells) = world.layout(o.global_res).map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    let report = planet_cache::global_store::build_global(dir, &world, o.global_res, threads, planet_cache::pack::DEFAULT_MAX_PACK_BYTES)
        .map_err(|e| e.to_string())?;
    let secs = start.elapsed().as_secs_f64();
    let edge = f64::from(cells) + 1.0;
    let samples = report.written as f64 * edge * edge;
    writeln!(
        out,
        "global bake {}: {} cells per face edge in blocks of {} (level {}): {} blocks written, {} already stored, at most {} blocks in memory; {:.1} s on {} threads, {:.2} us per sample per core",
        dir.display(),
        o.global_res,
        cells,
        level,
        report.written,
        report.reused,
        report.peak_resident_blocks,
        secs,
        threads,
        if samples > 0.0 { secs * threads as f64 / samples * 1e6 } else { 0.0 }
    )
    .map_err(err)?;
    // The ocean-connectivity channel (ADR 0041 point 2, STRM-023) is part of the bake.
    if report.ocean_components > 0 || report.ocean_written > 0 {
        writeln!(
            out,
            "global ocean channel: {} blocks written, {} already stored; {} sea components, {} of them ocean (area threshold {:.4} % of the surface); {:.1} s beyond the bake, {:.1} s of analysis thread time inside it, the merger held {} bytes",
            report.ocean_written,
            report.ocean_reused,
            report.ocean_components,
            report.ocean_oceans,
            world.sea().min_area_fraction * 100.0,
            report.ocean_seconds,
            report.ocean_in_bake_thread_seconds,
            report.ocean_merger_bytes
        )
        .map_err(err)?;
    } else {
        writeln!(out, "global ocean channel: {} blocks already stored under their current key, nothing recomputed", report.ocean_reused)
            .map_err(err)?;
    }
    // The coarse tiles go into the same pack (STRM-021, ADR 0045 point 3): levels 0..=4 unless `--prefill-levels` says otherwise.
    crate::prefill::prefill_store(o, dir, out)
}

/// `generate --compact-cache DIR [--cap-bytes N]`: rewrite the store keeping the records the definition still produces (a
/// height tile or a global block of its current versions and node set), newest packs first when a cap applies (STRM-013).
/// Records of a kind this build does not know are kept. Takes the writer lock: an editor or a generator writing to the
/// directory makes it refuse.
fn compact_cache(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.compact_cache.as_ref().expect("checked by the caller");
    let clash: Vec<&str> = [
        "--tile",
        "--out",
        "--face-net",
        "--manifest",
        "--smoke",
        "--cache-dir",
        "--verify-cache",
        "--region",
        "--res",
        "--sample",
        "--allow-stale",
        "--global-dir",
        "--verify-global",
        "--global-products",
        "--plan-check",
        "--global-res",
        "--threads",
    ]
    .into_iter()
    .filter(|f| o.given.contains(*f))
    .collect();
    if !clash.is_empty() {
        return Err(format!("--compact-cache takes only --world or --seed and --cap-bytes, not {}", clash.join(", ")));
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if !dir.is_dir() {
        return Err(format!("{}: no such cache directory", dir.display()));
    }
    // Liveness is judged against one definition: records of any other definition's read set are dropped, so name it.
    if !o.given.contains("--world") && !o.given.contains("--seed") {
        return Err(
            "--compact-cache judges liveness against one definition and drops the records of any other: give --world <file> or --seed N"
                .into(),
        );
    }
    let world = world_of(o)?;
    let global = global_world_of(o)?;
    let which = o.world.as_ref().map_or_else(|| format!("seed {}", o.seed), |p| format!("definition {}", p.display()));
    writeln!(out, "cache-compact: live records are those of {which}").map_err(err)?;
    let live = |r: &planet_cache::pack::Record| world.is_live(r) && global.is_live(r);
    let policy = planet_cache::pack::CompactPolicy {
        live: &live,
        cap_bytes: o.cap_bytes,
        rank: &planet_cache::pack::rank_by_pack_age,
        max_pack_bytes: planet_cache::pack::DEFAULT_MAX_PACK_BYTES,
    };
    let r = planet_cache::pack::compact(dir, world.params(), &policy).map_err(|e| e.to_string())?;
    for p in &r.problems {
        writeln!(out, "WARNING: {p}").map_err(err)?;
    }
    writeln!(
        out,
        "cache-compact {}: kept {} records in {} pack(s) ({} bytes, was {} in {} pack(s) and {} of another hash version); dropped {} dead, {} superseded, {} corrupt, {} evicted for the cap; output hash {}",
        dir.display(),
        r.kept,
        r.packs_out,
        r.bytes_after,
        r.bytes_before,
        r.packs_in,
        r.dead_packs,
        r.dead,
        r.superseded,
        r.corrupt,
        r.evicted,
        r.output_hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
    .map_err(err)
}

/// `generate --cache-dir DIR --region F,L`: generate a face region into a tile store. `generate --verify-cache DIR`:
/// regenerate a sample of a store from the definition and compare; a mismatch fails the command.
fn generate_store(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let clash: Vec<&str> = [
        "--tile",
        "--out",
        "--face-net",
        "--manifest",
        "--smoke",
        "--global-dir",
        "--verify-global",
        "--global-products",
        "--global-res",
        "--threads",
    ]
    .into_iter()
    .filter(|f| o.given.contains(*f))
    .collect();
    if !clash.is_empty() {
        return Err(format!("--cache-dir and --verify-cache work on a tile store and exclude {}", clash.join(", ")));
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    let world = world_of(o)?;
    if let Some(dir) = &o.verify_cache {
        if o.cache_dir.is_some() || o.given.contains("--res") {
            return Err("--verify-cache only reads: it excludes --cache-dir, --region and --res (a tile carries its own resolution)".into());
        }
        if o.given.contains("--cache-dir") || o.given.contains("--region") {
            return Err("--verify-cache only reads: it excludes --cache-dir and --region".into());
        }
        if !dir.is_dir() {
            return Err(format!("{}: no such cache directory (check the path; an empty store would verify nothing)", dir.display()));
        }
        let store = planet_cache::pack::TileStore::open(dir, world.params()).map_err(|e| e.to_string())?;
        for p in store.problems() {
            writeln!(out, "WARNING: {p}").map_err(err)?;
        }
        let report = planet_cache::tiles::verify_store(&store, &world, o.sample);
        writeln!(
            out,
            "cache-verify {}: checked {} of {} entries: {} match, {} stale (versions or node set changed), {} stale without a known reason, {} of another kind, {} differ, {} unreadable",
            dir.display(),
            report.checked,
            store.len(),
            report.matching,
            report.stale,
            report.unexplained_stale,
            report.unsupported,
            report.mismatches.len(),
            report.unreadable.len()
        )
        .map_err(err)?;
        for m in &report.mismatches {
            writeln!(
                out,
                "MISMATCH tile {:#018x} (face {} level {} x {} y {}) key {} in {} at offset {}: {}",
                m.tile.raw(),
                m.tile.face().0,
                m.tile.level(),
                m.tile.x(),
                m.tile.y(),
                m.key,
                m.pack,
                m.offset,
                m.detail
            )
            .map_err(err)?;
        }
        for u in &report.unreadable {
            writeln!(out, "UNREADABLE {u}").map_err(err)?;
        }
        let stale = report.stale + report.unexplained_stale;
        if report.clean() && report.compared_nothing() {
            if stale == 0 {
                return Err(format!(
                    "cache-verify compared nothing: {} entries looked at, none of them a height tile of this definition (an empty store, packs of another version, or only entries of another kind); --allow-stale does not help when nothing is stale",
                    report.checked
                ));
            }
            if !o.allow_stale {
                return Err(format!(
                    "cache-verify compared nothing: all {stale} entries are stale, so the store was built from another definition (or seed) than the one given; check --world/--seed, or pass --allow-stale if the definition changed on purpose"
                ));
            }
        }
        return if report.passed_allowing_stale() {
            Ok(())
        } else {
            Err(format!("cache-verify found {} differing and {} unreadable entries", report.mismatches.len(), report.unreadable.len()))
        };
    }
    let dir = o.cache_dir.as_ref().expect("checked by the caller");
    if o.given.contains("--sample") || o.given.contains("--allow-stale") {
        return Err("--sample and --allow-stale only apply to --verify-cache".into());
    }
    let (face, level) = o.region.ok_or("--cache-dir needs --region FACE,LEVEL (which tiles to generate)")?;
    let tiles = planet_cache::tiles::region_tiles(planet_core::Face(face), level)?;
    let report = planet_cache::tiles::build_tiles(dir, &world, &tiles, o.res, planet_cache::pack::DEFAULT_MAX_PACK_BYTES)
        .map_err(|e| e.to_string())?;
    writeln!(
        out,
        "cache {}: face {face} level {level}, {} tiles at {} cells per edge: {} generated, {} already stored",
        dir.display(),
        tiles.len(),
        o.res,
        report.written,
        report.reused
    )
    .map_err(err)
}

use planet_generators::compact::RenderTile;

const TERRAIN_SIZE: (u32, u32) = (256, 192);
const TERRAIN_CELLS: u32 = 16;

/// `--scene terrain`: node selection -> TerrainPlan -> meshes -> GPU, one of the ten scripted cameras.
pub fn render_terrain(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{scripted_views_field, LodParams, TerrainRequest, TerrainView};
    let view = TerrainView::parse(&o.view).ok_or_else(|| {
        format!("--view must be one of face, tile-id, level, morph, height, normals, depth, relief, morph-height, got '{}'", o.view)
    })?;
    // The world's field (GEN-018); without `--world` the seed-only world, which names the reference field.
    let world = std::sync::Arc::new(WorldTerrain::of_options(o, RADIUS_M)?);
    let field = world.field()?;
    let radius_m = world.radius_m();
    if o.world.is_some() {
        writeln!(out, "{}", world.describe()).map_err(err)?;
    }
    let views = scripted_views_field(radius_m, f64::from(TERRAIN_SIZE.0) / f64::from(TERRAIN_SIZE.1), field.as_ref());
    let index = match o.script.parse::<usize>() {
        Ok(i) => i,
        Err(_) => views
            .iter()
            .position(|(n, _)| *n == o.script)
            .ok_or_else(|| format!("--script '{}' is neither an index nor one of the scripted names", o.script))?,
    };
    let (name, camera) = *views.get(index).ok_or_else(|| format!("--script {index} is out of range (0..{})", views.len()))?;
    let lod = LodParams {
        cells: TERRAIN_CELLS,
        viewport_h_px: f64::from(TERRAIN_SIZE.1),
        tau_px: 4.0,
        max_level: o.finest_level_for(TERRAIN_CELLS),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let request = TerrainRequest { camera, size: TERRAIN_SIZE, view, lod, face_mapping: TangentWarp.id().to_string(), seed: field.seed() };
    if o.pipelined {
        // The camera script: the view cold, then (with --then) a jump to another view. The loop settles on each in turn.
        let mut requests = vec![request.clone()];
        let mut shown = name;
        if let Some(then) = &o.then {
            let idx = match then.parse::<usize>() {
                Ok(i) => i,
                Err(_) => views
                    .iter()
                    .position(|(n, _)| n == then)
                    .ok_or_else(|| format!("--then '{then}' is neither an index nor one of the scripted names"))?,
            };
            let (then_name, then_camera) = *views.get(idx).ok_or_else(|| format!("--then {idx} is out of range (0..{})", views.len()))?;
            requests.push(TerrainRequest { camera: then_camera, ..request.clone() });
            shown = then_name;
        }
        let (frame, log) = crate::pipelined::run_script(ctx, &world, &requests, o.gen_workers(), o.lockstep, out)?;
        writeln!(out, "terrain {shown} ({}) pipelined: {} frames drawn", view.name(), log.len()).map_err(err)?;
        if let Some(path) = &o.out {
            write_frame(&frame, path, out)?;
        }
        if let Some(dir) = &o.out_dir {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
            let path = dir.join("pipelined.txt");
            std::fs::write(&path, crate::pipelined::log_text(&log)).map_err(|e| format!("writing {}: {e}", path.display()))?;
        }
        return Ok(());
    }
    let mut cache = planet_frame::SelectionCache::new();
    let plan = planet_frame::plan_terrain_cached(&request, field.as_ref(), &mut cache).map_err(|e| e.to_string())?;
    let format = ctx.default_tile_format();
    let meshes: Vec<std::sync::Arc<RenderTile>> = if o.stream {
        // R is selected on the plan's cache (its tile geometry and height volumes are hits), from the position only.
        let residency = planet_frame::pipeline::residency_of(&request, field.as_ref(), &mut cache).map_err(|e| e.to_string())?;
        streamed_meshes(&world, &plan, &residency, format, request.lod.max_level)?
    } else {
        plan.nodes
            .iter()
            .map(|n| std::sync::Arc::new(tile_for(format, field.as_ref(), n.tile, TERRAIN_CELLS, radius_m, request.lod.max_level)))
            .collect()
    };
    let refs: Vec<&RenderTile> = meshes.iter().map(|m| m.as_ref()).collect();
    let (frame, stats) = planet_render::execute_terrain_tiles(ctx, &plan, &refs).map_err(|e| e.to_string())?;
    writeln!(
        out,
        "terrain {name} ({}): {}; plan {:016x}; {} draw calls, {} triangles",
        view.name(),
        plan.summary(),
        plan.plan_hash(),
        stats.draw_calls,
        stats.triangles
    )
    .map_err(err)?;
    if let Some(path) = &o.out {
        write_frame(&frame, path, out)?;
    }
    if let Some(path) = &o.bundle_out {
        // A run that named a world records it whole (definition text, field id, bake size): the replay rebuilds the field
        // from the bundle alone. The seed-only run records none and means the reference field of its seed.
        let (definition_hash, world_section) = if o.world.is_some() {
            let toml = planet_world_def::toml_io::to_toml(world.definition()).map_err(|e| format!("recording the world: {e}"))?;
            let section =
                planet_frame::BundleWorld { height_field: world.id().to_string(), global_res: world.global_res(), definition_toml: toml };
            (world.definition().hash(), Some(section))
        } else {
            (definition_hash(request.seed, request.lod.radius_m), None)
        };
        let bundle = planet_frame::TerrainBundle {
            generator_version: GENERATOR_VERSION,
            implementation_id: IMPLEMENTATION_ID.to_string(),
            mesh_version: planet_generators::mesh::MESH_VERSION,
            definition_hash,
            adapter: ctx.adapter_key(),
            world: world_section,
            request,
            plan,
        };
        std::fs::write(path, bundle.to_text()).map_err(|e| format!("writing {}: {e}", path.display()))?;
        writeln!(out, "wrote repro bundle {}", path.display()).map_err(err)?;
    }
    Ok(())
}

/// Replay a terrain repro bundle: refuse on version mismatch, replan from the recorded request, compare every field of
/// the plan, then render with meshes generated from the recorded seed.
fn replay_terrain(ctx: &GpuContext, text: &str, path: &Path, png: Option<&Path>, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{plan_terrain, TerrainBundle};
    let b = TerrainBundle::from_text(text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mesh_version = planet_generators::mesh::MESH_VERSION;
    if b.generator_version != GENERATOR_VERSION || b.implementation_id != IMPLEMENTATION_ID || b.mesh_version != mesh_version {
        return Err(format!(
            "bundle was recorded with generator {} ({}) and mesh version {}, this build has {} ({}) and {}; check out the matching commit to replay it",
            b.generator_version, b.implementation_id, b.mesh_version, GENERATOR_VERSION, IMPLEMENTATION_ID, mesh_version
        ));
    }
    // A bundle that recorded a world rebuilds its field from the recorded definition and bake size; one that recorded none is
    // the seed-only reference field of its seed (version 1 bundles and runs without `--world`).
    let world = match &b.world {
        Some(w) => Some(world_of_bundle(w, &b)?),
        None => {
            check_definition(b.definition_hash, b.request.seed, b.request.lod.radius_m)?;
            None
        }
    };
    let field = world.as_ref().map(|w| w.field()).transpose()?;
    let rebuilt = match &field {
        Some(f) => planet_frame::plan_terrain_field(&b.request, f.as_ref()),
        None => plan_terrain(&b.request),
    }
    .map_err(|e| e.to_string())?;
    let diff = rebuilt.diff(&b.plan);
    if !diff.is_empty() {
        return Err(format!(
            "replayed terrain plan differs from the recorded plan:
  {}",
            diff.join(
                "
  "
            )
        ));
    }
    writeln!(
        out,
        "replay: terrain plan identical ({:016x}), {} nodes, recorded on adapter {}",
        rebuilt.plan_hash(),
        rebuilt.nodes.len(),
        b.adapter
    )
    .map_err(err)?;
    let map = planet_frame::plan::face_mapping_by_id(&b.request.face_mapping)
        .ok_or_else(|| format!("unknown face mapping '{}'", b.request.face_mapping))?;
    let (seed, cells, radius) = (b.request.seed, b.request.lod.cells, b.request.lod.radius_m);
    let format = ctx.default_tile_format();
    let max_level = b.request.lod.max_level;
    let reference = planet_generators::height::Reference(seed);
    let meshes: Vec<RenderTile> = rebuilt
        .nodes
        .iter()
        .map(|n| match &field {
            Some(f) => tile_with(format, f.as_ref(), map.as_ref(), n.tile, cells, radius, max_level),
            None => tile_with(format, &reference, map.as_ref(), n.tile, cells, radius, max_level),
        })
        .collect();
    let refs: Vec<&RenderTile> = meshes.iter().collect();
    let (frame, stats) = planet_render::execute_terrain_tiles(ctx, &rebuilt, &refs).map_err(|e| e.to_string())?;
    writeln!(out, "replay: {} draw calls, {} triangles", stats.draw_calls, stats.triangles).map_err(err)?;
    if let Some(p) = png {
        write_frame(&frame, p, out)?;
    }
    Ok(())
}

/// Rebuild the terrain a bundle recorded: parse the recorded definition, check it against the recorded hash, field id and
/// radius, and bake the field's input again at the recorded size. Everything is refused with the recorded and the rebuilt
/// values, before any replanning (CON-19).
fn world_of_bundle(w: &planet_frame::BundleWorld, b: &planet_frame::TerrainBundle) -> Result<WorldTerrain, String> {
    let def = planet_world_def::toml_io::parse_definition(&w.definition_toml)
        .map_err(|e| format!("the world recorded in the bundle cannot be read: {e}"))?;
    if def.hash() != b.definition_hash {
        return Err(format!(
            "bundle records definition hash {:016x}, but its recorded world hashes to {:016x}: the bundle was edited or its world was written by another build",
            b.definition_hash,
            def.hash()
        ));
    }
    let terrain = WorldTerrain::open(def, w.global_res)?;
    if terrain.id() != w.height_field {
        return Err(format!(
            "bundle records height field {}, but its recorded world resolves to {} in this build",
            w.height_field,
            terrain.id()
        ));
    }
    if terrain.radius_m() != b.request.lod.radius_m {
        return Err(format!("bundle records radius {} m, its recorded world has {} m", b.request.lod.radius_m, terrain.radius_m()));
    }
    Ok(terrain)
}

/// Tile meshes of the run's world field. Each load builds the field afresh (cheap: it only reads the definition and holds the
/// bake by reference), so the source owns the world and no borrowed field has to outlive the streamer's workers.
pub(crate) struct MeshSource {
    pub(crate) world: std::sync::Arc<WorldTerrain>,
    pub(crate) format: planet_render::TileFormat,
    pub(crate) max_level: u8,
    /// Cells per tile edge: the run's tile size (the product 32, LOD-010; the reference views 16).
    pub(crate) cells: u32,
}

impl planet_streaming::TileSource<RenderTile> for MeshSource {
    fn load(&self, tile: TileId) -> Result<RenderTile, String> {
        let field = self.world.field()?;
        // A pre-filled coarse tile is read from the pack instead of generated (STRM-021): the same tile, bit for bit.
        if let Some(t) = self.world.prefilled_tile(field.as_ref(), tile, self.cells, self.max_level, self.format) {
            self.world.fill_water(tile);
            return Ok(t);
        }
        let made = tile_for(self.format, field.as_ref(), tile, self.cells, self.world.radius_m(), self.max_level);
        self.world.fill_water(tile);
        Ok(made)
    }

    /// The stage boundary of a tile is every height sample: once the token is set the field answers 0 without reading the
    /// store, so the generation runs out in microseconds and the garbage tile is dropped, never returned (STRM-005: a job
    /// stops at its next stage boundary; a teardown or a teleport does not wait for a cold tile to read its blocks).
    fn load_staged(&self, tile: TileId, cancel: &planet_streaming::CancelToken) -> Result<Option<RenderTile>, String> {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        self.world.timed_load(|| self.load_staged_timed(tile, cancel))
    }
}

impl MeshSource {
    fn load_staged_timed(&self, tile: TileId, cancel: &planet_streaming::CancelToken) -> Result<Option<RenderTile>, String> {
        let field = self.world.field()?;
        if let Some(t) = self.world.prefilled_tile(field.as_ref(), tile, self.cells, self.max_level, self.format) {
            if !cancel.is_cancelled() {
                self.world.fill_water(tile);
            }
            return Ok(Some(t));
        }
        let guarded = CancelGuarded { inner: field.as_ref(), cancel };
        let made = tile_for(self.format, &guarded, tile, self.cells, self.world.radius_m(), self.max_level);
        if cancel.is_cancelled() {
            return Ok(None);
        }
        // The tile's water (ADR 0056) is computed here, in the job, so the plan of a frame only looks it up.
        self.world.fill_water(tile);
        Ok(Some(made))
    }
}

/// A height field that stops reading once its job is cancelled: every sample polls the token and answers 0 afterwards. Every
/// other method is the inner field's (so the keys of a tile built through it are the inner field's).
pub(crate) struct CancelGuarded<'a> {
    pub(crate) inner: &'a dyn planet_generators::height::HeightField,
    pub(crate) cancel: &'a planet_streaming::CancelToken,
}

// Maintenance: a new `HeightField` method with a default body must be delegated here too, or a guarded tile would key differently.
impl planet_generators::height::HeightField for CancelGuarded<'_> {
    fn height(&self, dir: Vec3) -> f64 {
        if self.cancel.is_cancelled() {
            0.0
        } else {
            self.inner.height(dir)
        }
    }
    /// The inner field's batch (it may order the samples by block, GEN-020), stopped by the token or the caller's flag.
    // Invariant: once `stop` or the cancel flag answers true the batch returns 0.0 for the unfinished directions; whoever cancels
    // must discard the results of the batch (the plan of a cancelled job is never used or memoised).
    fn heights_until(&self, dirs: &[Vec3], stop: &dyn Fn() -> bool) -> Vec<f64> {
        self.inner.heights_until(dirs, &|| stop() || self.cancel.is_cancelled())
    }
    fn heights_threaded_until(&self, dirs: &[Vec3], stop: &(dyn Fn() -> bool + Sync), threads: usize) -> Vec<f64> {
        self.inner.heights_threaded_until(dirs, &|| stop() || self.cancel.is_cancelled(), threads)
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.inner.implementation_id()
    }
    fn version(&self) -> u32 {
        self.inner.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.inner.max_abs_height_m()
    }
    fn inputs_hash(&self) -> u64 {
        self.inner.inputs_hash()
    }
    fn read_records(&self) -> Vec<planet_world_def::ReadRecord> {
        self.inner.read_records()
    }
    fn read_records_for(&self, tile: TileId, map: &dyn FaceMapping) -> Vec<planet_world_def::ReadRecord> {
        self.inner.read_records_for(tile, map)
    }
    fn reads_cells(&self) -> bool {
        self.inner.reads_cells()
    }
    fn inputs_hash_for(&self, tile: TileId, map: &dyn FaceMapping) -> u64 {
        self.inner.inputs_hash_for(tile, map)
    }
    fn body(&self) -> planet_world_def::BodyId {
        self.inner.body()
    }
    fn source_key(&self) -> Option<[u8; 16]> {
        self.inner.source_key()
    }
    fn surface_id(&self) -> &'static str {
        self.inner.surface_id()
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        self.inner.height_bounds(centre, angular_radius)
    }
    fn climate(&self, map: &dyn FaceMapping, id: TileId) -> planet_generators::climate::ClimateTexture {
        self.inner.climate(map, id)
    }
    fn climate_version_word(&self) -> u64 {
        self.inner.climate_version_word()
    }
    fn heights_and_climate(
        &self,
        map: &dyn FaceMapping,
        id: TileId,
        dirs: &[Vec3],
        stop: &dyn Fn() -> bool,
    ) -> (Vec<f64>, planet_generators::climate::ClimateTexture) {
        self.inner.heights_and_climate(map, id, dirs, &|| stop() || self.cancel.is_cancelled())
    }
}

/// The tile in the format the run uses: the compact format for levels 3 and finer with the vertex fallback (GEN-019), or the
/// vertex format for every tile (`--tile-format vertex`, the reference).
pub(crate) fn tile_for(
    format: planet_render::TileFormat,
    field: &dyn planet_generators::height::HeightField,
    tile: TileId,
    cells: u32,
    radius_m: f64,
    max_level: u8,
) -> RenderTile {
    tile_with(format, field, &TangentWarp, tile, cells, radius_m, max_level)
}

fn tile_with(
    format: planet_render::TileFormat,
    field: &dyn planet_generators::height::HeightField,
    map: &dyn planet_core::cube::FaceMapping,
    tile: TileId,
    cells: u32,
    radius_m: f64,
    max_level: u8,
) -> RenderTile {
    match format {
        planet_render::TileFormat::Compact => planet_generators::compact::tile_data_field(field, map, tile, cells, radius_m, max_level),
        planet_render::TileFormat::Vertex => {
            RenderTile::Vertex(planet_generators::compact::vertex_tile_field(field, map, tile, cells, radius_m, max_level))
        }
    }
}

/// Generate the plan's tile meshes through the asynchronous streaming path and wait, frame by frame, until every wanted tile
/// is resident. The result equals the synchronous path. The residency manager drives the requests as in the interactive
/// loop (STRM-005, STRM-006): the plan's draw set D first (ancestors before descendants), then the rest of the residency set R
/// of the camera position (selected on the plan's own cache), within the slots of the mid tier's share; the run ends when D is
/// resident, whatever of R is still on its way is dropped with the streamer.
fn streamed_meshes(
    world: &std::sync::Arc<WorldTerrain>,
    plan: &planet_frame::TerrainPlan,
    residency: &planet_frame::ResidencySet,
    format: planet_render::TileFormat,
    max_level: u8,
) -> Result<Vec<std::sync::Arc<RenderTile>>, String> {
    use planet_streaming::{ResidencyManager, StreamConfig, Streamer, SystemClock, Tier, Wants};
    // Four tile threads and four queued behind them (STRM-020): the pool size and the limit are one number.
    let gen = planet_streaming::GenWorkers::new(4);
    let pool = gen.pool();
    let share = match format {
        planet_render::TileFormat::Compact => planet_frame::pool::tier_compact_pool(Tier::Mid, TERRAIN_CELLS, 1)?,
        planet_render::TileFormat::Vertex => planet_frame::pool::tier_pool(Tier::Mid, TERRAIN_CELLS, 1)?,
    };
    let config = gen.apply(StreamConfig {
        capacity: share.main_slots() as usize,
        vertex_pool_slots: share.managed_vertex_slots() as usize,
        retry_after_ms: 1000,
        record_dispatches: false,
        ..Default::default()
    });
    let mut streamer = Streamer::new(
        std::sync::Arc::new(MeshSource { world: std::sync::Arc::clone(world), format, max_level, cells: TERRAIN_CELLS }),
        pool,
        std::sync::Arc::new(SystemClock::new()),
        config,
    );
    streamer.load_roots_blocking()?;
    let mut manager = ResidencyManager::new(streamer);
    let wants = Wants {
        draw: plan.nodes.iter().enumerate().map(|(i, n)| (n.tile, i as f64)).collect(),
        residency: residency.wants(),
        prefetch: Vec::new(),
    };
    let started = Instant::now();
    loop {
        let frame = manager.begin_frame(&wants);
        if frame.resolved.iter().all(|r| r.exact()) {
            return Ok(wants.draw.iter().map(|(t, _)| manager.streamer().get(*t).expect("resident")).collect());
        }
        if started.elapsed().as_secs() > 120 {
            return Err(format!("streaming did not finish within 120 s ({:?})", manager.streamer().stats()));
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Generate the meshes `plan` needs that `renderer` does not hold (by tile and mesh key) in parallel, then upload them.
/// Returns the number of new meshes.
pub(crate) fn upload_missing_meshes(
    ctx: &GpuContext,
    renderer: &mut planet_render::TerrainRenderer,
    plan: &planet_frame::TerrainPlan,
    field: &dyn planet_generators::height::HeightField,
    radius_m: f64,
    cells: u32,
    max_level: u8,
) -> Result<usize, String> {
    let format = ctx.default_tile_format();
    let missing: Vec<TileId> = plan.nodes.iter().filter(|n| !renderer.has_mesh(n.tile, n.mesh_key)).map(|n| n.tile).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = missing.len().div_ceil(threads).max(1);
    let meshes: Vec<RenderTile> = std::thread::scope(|s| {
        let handles: Vec<_> = missing
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|&t| tile_for(format, field, t, cells, radius_m, max_level)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("mesh thread")).collect()
    });
    for m in &meshes {
        renderer.upload_tile(ctx, m).map_err(|e| e.to_string())?;
    }
    Ok(meshes.len())
}

/// Most meshes a flight keeps resident; the renderer is rebuilt (dropping every mesh) when a frame would exceed it.
const FLIGHT_RESIDENT_CAP: usize = 12_000;
/// The flight's contact sheet: this many frames from orbit to 1 m, in this many columns, thumbnails at most this wide.
const SHEET_FRAMES: usize = 12;
const SHEET_COLUMNS: usize = 4;
const SHEET_THUMB_W: u32 = 320;

/// `test-render --scene terrain --flight N --out-dir DIR`: the scripted orbit-to-1 m flight (TEST-013) rendered frame by
/// frame through the persistent terrain renderer with production LOD parameters, for the owner's real-GPU jitter review
/// (tier D). Writes `frame_NNNN.png` and `flight.txt` (frame, altitude above terrain, nodes, plan hash).
pub fn render_flight(ctx: &GpuContext, o: &Options, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{flight_path_field, plan_terrain_field, LodParams, TerrainRequest, TerrainView};
    use planet_render::TerrainRenderer;
    let world = WorldTerrain::of_options(o, RADIUS_M)?;
    let field = world.field()?;
    let radius_m = world.radius_m();
    if o.world.is_some() {
        writeln!(out, "{}", world.describe()).map_err(err)?;
    }
    let frames = o.flight.unwrap_or(0);
    if frames < 10 {
        return Err(format!("--flight needs at least 10 frames, got {frames}"));
    }
    // Option conflicts are rejected in `check_flight_options` before the GPU starts.
    let dir = o.out_dir.as_ref().ok_or("--flight needs --out-dir")?;
    // The flat-colour views are one colour per tile, which is a solid frame once the camera is a metre above the ground,
    // so a flight defaults to the relief view unless `--view` says otherwise (TEST-013).
    let view_name = if o.given.contains("--view") { o.view.as_str() } else { "relief" };
    let view = TerrainView::parse(view_name).ok_or_else(|| {
        format!("--view must be one of face, tile-id, level, morph, height, normals, depth, relief, morph-height, got '{view_name}'")
    })?;
    let size = o.size.unwrap_or((1280, 720));
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: o.tau,
        max_level: o.finest_level_for(o.cells),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, o.tau))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let cameras = flight_path_field(radius_m, f64::from(size.0) / f64::from(size.1), field.as_ref(), frames);
    let mut renderer = TerrainRenderer::for_context(ctx, size, o.cells, 1);
    let mut index = String::from("# frame altitude_above_terrain_m nodes plan_hash finest_level\n");
    let on_sheet = crate::sheet::sheet_indices(frames, SHEET_FRAMES);
    let mut sheet = Vec::new();
    for (i, camera) in cameras.into_iter().enumerate() {
        let req = TerrainRequest { camera, size, view, lod: lod.clone(), face_mapping: TangentWarp.id().to_string(), seed: field.seed() };
        let plan = plan_terrain_field(&req, field.as_ref()).map_err(|e| format!("frame {i}: {e}"))?;
        let missing = plan.nodes.iter().filter(|n| !renderer.has_mesh(n.tile, n.mesh_key)).count();
        if renderer.resident_tiles() > 0 && renderer.resident_tiles() + missing > FLIGHT_RESIDENT_CAP {
            renderer = TerrainRenderer::for_context(ctx, size, o.cells, 1);
        }
        upload_missing_meshes(ctx, &mut renderer, &plan, field.as_ref(), radius_m, o.cells, lod.max_level)?;
        renderer.render(ctx, &plan, false).map_err(|e| format!("frame {i}: {e}"))?;
        let path = dir.join(format!("frame_{i:04}.png"));
        let image = renderer.read_color(ctx);
        image.write_png(&path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        let p = camera.position.0;
        let altitude = p.length() - radius_m - field.height(p.normalized());
        let finest = plan.nodes.iter().map(|n| n.level).max().unwrap_or(0);
        index.push_str(&format!("{i:04} {altitude:.3} {} {:016x} {finest}\n", plan.nodes.len(), plan.plan_hash()));
        if on_sheet.contains(&i) {
            sheet.push((image, crate::sheet::label(i, altitude)));
        }
    }
    let sheet_path = dir.join("contact_sheet.png");
    crate::sheet::contact_sheet(&sheet, SHEET_COLUMNS, SHEET_THUMB_W)
        .write_png(&sheet_path)
        .map_err(|e| format!("writing {}: {e}", sheet_path.display()))?;
    std::fs::write(dir.join("flight.txt"), &index).map_err(|e| format!("writing flight.txt: {e}"))?;
    writeln!(
        out,
        "flight: wrote {frames} frames to {} ({}x{}, cells {}, tau {} px, view {}, finest level {})",
        dir.display(),
        size.0,
        size.1,
        o.cells,
        o.tau,
        view.name(),
        lod.max_level
    )
    .map_err(err)
}

/// `test-render --perf --scene terrain`: the terrain frame workload at 1440p on the reference GPU (CON-21). Meshes are
/// generated and uploaded once per view; frames then only update two small buffers and draw. Reports GPU time (timestamp
/// queries), CPU record/submit time, wall time, CPU planning time and the deterministic counts, per run, and checks the
/// terrain budget (median GPU <= 8 ms, p99 frame <= 20 ms).
pub fn perf_terrain(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    use planet_frame::{scripted_views_field, LodParams, TerrainRequest, TerrainView};
    use planet_perf::{summarize, Protocol, RunReport, Stats};
    use planet_render::TerrainRenderer;
    // `--smoke` runs the same code path briefly on any adapter to prove it works; it is labelled and never a measurement.
    let smoke = o.perf_smoke;
    // The world's field (GEN-018): the default world (eroded field over an in-memory bake of --global-res) unless --world
    // names another. Baked before the readiness gate, so the bake's load is not measured as the machine's.
    let world = WorldTerrain::of_perf_options(o, RADIUS_M)?;
    let field = world.field()?;
    let radius_m = world.radius_m();
    let measured_field = world.describe();
    let mut owner_override: Vec<String> = Vec::new();
    if !smoke {
        planet_perf::check_reference_adapter(&crate::facts(ctx)).map_err(|r| r.to_string())?;
        owner_override = crate::readiness_gate(o, ctx, out)?;
    }
    let size = if smoke { (1280u32, 720u32) } else { (2560u32, 1440u32) };
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: o.tau,
        max_level: o.finest_level_for(o.cells),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, o.tau))?;
    let (warmup, measured, runs) = if smoke { (2usize, 5usize, 1usize) } else { (60usize, o.frames.max(10), 3usize) };
    let protocol = Protocol {
        warmup_frames: warmup,
        measured_frames: measured,
        runs,
        internal_resolution: size,
        field: Some(measured_field.clone()),
        smoke,
        ..Protocol::default()
    };
    let views = scripted_views_field(radius_m, f64::from(size.0) / f64::from(size.1), field.as_ref());
    let mut renderer = TerrainRenderer::for_context(ctx, size, o.cells, 1);
    let mut reports: Vec<RunReport> = Vec::new();
    let mut budget_lines = Vec::new();
    let mut plan_lines: Vec<(&str, f64)> = Vec::new();
    let mut noisy: Vec<(String, f64)> = Vec::new();
    if smoke {
        writeln!(out, "SMOKE RUN: exercises the perf path only; the numbers are not a measurement").map_err(err)?;
    }
    writeln!(
        out,
        "terrain perf: {}x{}, cells {}, tau {} px, finest level {}, timestamps {}, {measured_field}",
        size.0,
        size.1,
        o.cells,
        o.tau,
        lod.max_level,
        ctx.timestamps_supported()
    )
    .map_err(err)?;
    // Heaviest first: the light orbit view never raises idle clocks by itself, so it runs after the GPU is busy.
    let mut warmups: Vec<(String, planet_perf::clock::WarmUpOutcome)> = Vec::new();
    let mut loads: Vec<(String, planet_perf::load::CaseLoad)> = Vec::new();
    for index in [6usize, 5, 3, 0] {
        let (name, camera) = views[index];
        let req = TerrainRequest {
            camera,
            size,
            view: TerrainView::Level,
            lod: lod.clone(),
            face_mapping: TangentWarp.id().to_string(),
            seed: field.seed(),
        };
        // CPU planning time (selection + plan), measured separately: the camera is static during frame measurements. A
        // renderer plans every frame with a warm per-tile cache, so the budget applies to steady state: the camera
        // steps 5 cm per plan (a slow walk), the cold first plan is reported beside it.
        let mut cache = planet_frame::SelectionCache::new();
        let t_cold = Instant::now();
        planet_frame::plan_terrain_cached(&req, field.as_ref(), &mut cache).map_err(|e| e.to_string())?;
        let cold_ms = t_cold.elapsed().as_secs_f64() * 1000.0;
        let mut plan_samples = Vec::new();
        let mut moving = req.clone();
        let up = camera.position.0.normalized();
        let axis = if up.z.abs() > 0.9 { planet_core::Vec3::new(1.0, 0.0, 0.0) } else { planet_core::Vec3::new(0.0, 0.0, 1.0) };
        let step = up.cross(axis).normalized() * 0.05;
        for _ in 0..20 {
            moving.camera.position = planet_core::PlanetFixed(moving.camera.position.0 + step);
            let t = Instant::now();
            planet_frame::plan_terrain_cached(&moving, field.as_ref(), &mut cache).map_err(|e| e.to_string())?;
            plan_samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let plan = planet_frame::plan_terrain_cached(&req, field.as_ref(), &mut cache).map_err(|e| e.to_string())?;
        let started = Instant::now();
        let new_meshes = upload_missing_meshes(ctx, &mut renderer, &plan, field.as_ref(), radius_m, o.cells, lod.max_level)?;
        let plan_median = summarize(&plan_samples, 0).map_or(f64::NAN, |s| s.median);
        plan_lines.push((name, plan_median));
        writeln!(
            out,
            "{name}: {} nodes ({} new meshes in {:.1} s), plan {plan_median:.2} ms warm, {cold_ms:.2} ms cold",
            plan.nodes.len(),
            new_meshes,
            started.elapsed().as_secs_f64(),
        )
        .map_err(err)?;
        for _ in 0..warmup {
            renderer.render(ctx, &plan, true).map_err(|e| e.to_string())?;
        }
        // Bring the GPU to its loaded state before measuring (ADR 0034): chunks of frames until the graphics clock reads the floor (ADR 0036), with a
        // measured cap. Not in a smoke run (it is no measurement and may be on a software adapter without `nvidia-smi`).
        if !smoke {
            let started = Instant::now();
            let outcome = planet_perf::clock::warm_up(
                planet_perf::clock::WARMUP_CAP_S,
                || {
                    for _ in 0..planet_perf::clock::WARMUP_CHUNK_FRAMES {
                        renderer.render(ctx, &plan, true).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                },
                || planet_perf::query_nvidia_smi().as_ref().map(planet_perf::clock::ClockPoint::from),
                || started.elapsed().as_secs_f64(),
            )?;
            writeln!(out, "{}", outcome.line(name)).map_err(err)?;
            warmups.push((name.to_string(), outcome));
        }
        // Run-to-run noise above the limit gets one retry of this view's runs (ADR 0020); still noisy: the session is invalid.
        let mut attempt = 0;
        let batch_start = reports.len();
        let budget_start = budget_lines.len();
        // The host's load beside this view's runs (TEST-023): recorded, never judged.
        let load_sampler = (!smoke).then(|| planet_perf::load::LoadSampler::start(planet_perf::load::SAMPLE_PERIOD_S));
        loop {
            for _ in 0..runs {
                let before = planet_perf::query_nvidia_smi();
                let (mut total, mut cpu, mut gpu) = (Vec::new(), Vec::new(), Vec::new());
                let mut counts = planet_render::TerrainStats::default();
                for _ in 0..measured {
                    let (t, s) = renderer.render(ctx, &plan, true).map_err(|e| e.to_string())?;
                    total.push(t.total_ms);
                    cpu.push(t.cpu_ms);
                    gpu.extend(t.gpu_ms);
                    counts = s;
                }
                let stat = |v: &[f64]| summarize(v, 0).ok_or_else(|| "no valid samples".to_string());
                let mut metrics: Vec<(String, Stats)> = vec![("cpu_ms".into(), stat(&cpu)?), ("plan_ms".into(), stat(&plan_samples)?)];
                if !gpu.is_empty() {
                    metrics.push(("gpu_ms".into(), stat(&gpu)?));
                }
                let (t_stats, g_stats) = (stat(&total)?, metrics.iter().find(|(n, _)| n == "gpu_ms").map(|(_, s)| s.clone()));
                budget_lines.push((name, t_stats.clone(), g_stats));
                reports.push(RunReport {
                    path_name: format!("terrain/{name}"),
                    vram_cap_gb: 8,
                    stats: t_stats,
                    clocks_before: before,
                    clocks_after: planet_perf::query_nvidia_smi(),
                    metrics,
                    counts: vec![
                        ("nodes".into(), plan.nodes.len() as u64),
                        ("draw_calls".into(), u64::from(counts.draw_calls)),
                        ("triangles".into(), counts.triangles),
                        ("vertex_bytes_resident".into(), renderer.uploaded_bytes),
                        ("resident_tiles".into(), renderer.resident_tiles() as u64),
                    ],
                });
            }
            let medians: Vec<f64> = reports[batch_start..].iter().map(|r| r.stats.median).collect();
            if smoke || !planet_perf::gate::noise_needs_retry(&medians) || attempt == 1 {
                if !smoke && planet_perf::gate::noise_needs_retry(&medians) {
                    noisy.push((format!("terrain/{name}"), planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN)));
                }
                break;
            }
            writeln!(
                out,
                "{name}: run-to-run noise {:.3} above {}: one retry",
                planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN),
                planet_perf::gate::MAX_NOISE
            )
            .map_err(err)?;
            reports.truncate(batch_start);
            budget_lines.truncate(budget_start);
            attempt += 1;
        }
        let view_load = load_sampler.map_or_else(planet_perf::load::CaseLoad::not_measured, planet_perf::load::LoadSampler::stop);
        writeln!(out, "{name}: {}", view_load.line()).map_err(err)?;
        for n in view_load.notes() {
            writeln!(out, "  {name}: LOAD NOTE (recorded, the session's validity is unchanged): {n}").map_err(err)?;
        }
        loads.push((format!("terrain/{name}"), view_load));
    }
    let scheme = planet_perf::query_power_scheme();
    let throttled: Vec<String> = reports.iter().filter(|r| crate::run_throttled(r)).map(|r| r.path_name.clone()).collect();
    let validity = if smoke { planet_perf::gate::Validity::default() } else { planet_perf::gate::validity(&throttled, &noisy) };
    let verdict =
        planet_perf::validity::SessionVerdict::assess(&crate::facts(ctx), scheme.as_deref(), &reports, smoke, &validity, &owner_override);
    if !smoke {
        crate::session_verdict_line(out, &verdict)?;
    }
    for (name, median) in &plan_lines {
        let word = if planet_perf::plan_budget_ok(*median) { "ok" } else { "OVER" };
        writeln!(out, "  {name}: CPU plan median {median:.2} ms (budget {}) {word}", planet_perf::PLAN_CPU_BUDGET_MS).map_err(err)?;
    }
    if smoke {
        // A smoke run prints no session verdict above; its one cause is still named.
        for w in verdict.warnings() {
            writeln!(out, "WARNING (not valid for time budgets): {w}").map_err(err)?;
        }
    }
    if !smoke {
        let off: Vec<String> = budget_lines
            .iter()
            .map(|(n, _, _)| *n)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|n| !planet_perf::view_gpu_validity(&reports, &format!("terrain/{n}")).is_valid())
            .map(str::to_string)
            .collect();
        if !off.is_empty() {
            writeln!(
                out,
                "NOT VALID for GPU budgets (a graphics clock sample below {} MHz, ADR 0036; frame verdicts stand): {}",
                planet_perf::clock::GPU_CLOCK_FLOOR_MHZ,
                off.join(", ")
            )
            .map_err(err)?;
        }
    }
    for (name, total, gpu) in &budget_lines {
        let clock_state = planet_perf::view_gpu_validity(&reports, &format!("terrain/{name}"));
        // ADR 0034 as amended: the frame verdict holds at any clock state; only the GPU verdict needs the clock floor (ADR 0036).
        let power_cap = planet_perf::view_power_cap(&reports, &format!("terrain/{name}"));
        writeln!(out, "{}", planet_perf::terrain_view_line(name, total, gpu.as_ref(), &clock_state, power_cap, smoke)).map_err(err)?;
    }
    // Run-to-run noise within each view (its three runs), reported as the worst view.
    let mut worst_noise = 0.0f64;
    let names: std::collections::BTreeSet<&str> = budget_lines.iter().map(|(n, _, _)| *n).collect();
    for name in names {
        let medians: Vec<f64> = reports.iter().filter(|r| r.path_name == format!("terrain/{name}")).map(|r| r.stats.median).collect();
        worst_noise = worst_noise.max(planet_perf::run_to_run_noise(&medians).unwrap_or(0.0));
    }
    writeln!(out, "worst run-to-run noise within a view (max-min)/median: {worst_noise:.3}").map_err(err)?;
    let json = planet_perf::report_json_full(
        &crate::facts(ctx),
        scheme.as_deref(),
        &owner_override,
        &validity,
        &protocol,
        &reports,
        &warmups,
        &loads,
    );
    if smoke && o.out.is_none() {
        return writeln!(out, "smoke run: no log written (pass --out to keep it)").map_err(err);
    }
    let path = o.out.clone().unwrap_or_else(|| std::path::Path::new("perf").join("logs").join("terrain.json"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    // Compare with the last valid session of this scene (report only), then keep this one as the baseline if it is valid.
    let baseline = planet_perf::compare::baseline_path(&path);
    // Without a baseline file yet, the previous log of this scene stands in (it is used only if it was valid).
    let last_valid = planet_perf::compare::choose_baseline(std::fs::read(&baseline).ok(), std::fs::read(&path).ok());
    let gpu_baseline = planet_perf::compare::gpu_baseline_path(&path);
    let last_gpu_valid = planet_perf::compare::choose_baseline(std::fs::read(&gpu_baseline).ok(), std::fs::read(&path).ok());
    let lines = planet_perf::compare::comparison_lines_gpu(last_valid.as_deref(), last_gpu_valid.as_deref(), json.as_bytes());
    for line in &lines {
        writeln!(out, "{line}").map_err(err)?;
    }
    std::fs::write(&path, &json).map_err(err)?;
    if planet_perf::compare::replaces_baseline(json.as_bytes()) {
        std::fs::write(&baseline, &json).map_err(err)?;
    }
    if planet_perf::compare::replaces_gpu_baseline(json.as_bytes()) {
        std::fs::write(&gpu_baseline, &json).map_err(err)?;
    }
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: STRM-005
    #[test]
    fn the_mesh_source_stops_a_cancelled_job_and_builds_the_same_tile_when_live() {
        use planet_streaming::{CancelToken, TileSource};
        let world =
            std::sync::Arc::new(WorldTerrain::open(crate::worldfield::legacy_world(5, RADIUS_M), 8).expect("a small in-memory world"));
        let source = MeshSource { world, format: planet_render::TileFormat::Compact, max_level: 22, cells: 16 };
        let tile = TileId::new(Face(2), 5, 3, 4).unwrap();
        assert!(source.load_staged(tile, &CancelToken::cancelled()).unwrap().is_none(), "cancelled before it starts: no tile");
        let live = source.load_staged(tile, &CancelToken::never()).unwrap().expect("a live job returns its tile");
        assert_eq!(live, source.load(tile).unwrap(), "the staged path builds the tile of the plain one");
    }

    // spec: ATMO-012
    #[test]
    fn a_tile_job_computes_the_water_of_its_tile_and_the_plan_then_asks_the_field_nothing() {
        use planet_frame::water::WaterTileCache;
        use planet_streaming::{CancelToken, TileSource};
        let world = std::sync::Arc::new(
            WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(RADIUS_M, 5), 128)
                .expect("a small in-memory world")
                .bounding_water(),
        );
        let source =
            MeshSource { world: std::sync::Arc::clone(&world), format: planet_render::TileFormat::Vertex, max_level: 22, cells: 16 };
        let tiles: Vec<TileId> =
            (0..6u8).flat_map(|f| [TileId::new(Face(f), 2, 1, 2).unwrap(), TileId::new(Face(f), 4, 7, 9).unwrap()]).collect();
        // A cancelled job leaves nothing; the jobs of the live tiles fill the book.
        assert!(source.load_staged(tiles[0], &CancelToken::cancelled()).unwrap().is_none());
        assert_eq!(world.water_book().filled(), 0, "a cancelled job computes no water");
        for t in &tiles {
            source.load_staged(*t, &CancelToken::never()).unwrap().unwrap();
        }
        assert_eq!(world.water_book().filled(), tiles.len() as u64);
        // The plan's cache answers every tile from the book: no field query, no miss (no store block read on the plan thread).
        let mut cache = WaterTileCache::with_book(world.water_book());
        let from_book: Vec<_> = world
            .with_water(|f| {
                tiles.iter().map(|t| cache.tile_water(f, &planet_core::cube::TangentWarp, *t, RADIUS_M).unwrap()).collect::<Vec<_>>()
            })
            .unwrap()
            .unwrap();
        assert_eq!((cache.queries, cache.tile_misses, cache.book_hits), (0, 0, tiles.len() as u64));
        // The same answers as the plan thread's own query (the answer is a pure function of the tile and the field, CON-14).
        let mut own = WaterTileCache::new();
        let direct: Vec<_> = world
            .with_water(|f| {
                tiles.iter().map(|t| own.tile_water(f, &planet_core::cube::TangentWarp, *t, RADIUS_M).unwrap()).collect::<Vec<_>>()
            })
            .unwrap()
            .unwrap();
        assert_eq!(from_book, direct);
        assert!(
            own.queries > 0 && from_book.iter().any(Option::is_some) && from_book.iter().any(Option::is_none),
            "the tiles are a mix of sea and land"
        );
        // A world without the lit path computes no water in its jobs (the debug path pays nothing).
        let debug =
            std::sync::Arc::new(WorldTerrain::open(planet_world_def::WorldDefinition::single_planet(RADIUS_M, 5), 128).expect("world"));
        MeshSource { world: std::sync::Arc::clone(&debug), format: planet_render::TileFormat::Vertex, max_level: 22, cells: 16 }
            .load(tiles[1])
            .unwrap();
        assert_eq!(debug.water_book().filled(), 0);
    }

    /// A height field that counts the samples it is asked for.
    struct Sampled<'a> {
        inner: planet_generators::height::Reference,
        calls: &'a std::sync::atomic::AtomicUsize,
    }

    impl planet_generators::height::HeightField for Sampled<'_> {
        fn height(&self, dir: Vec3) -> f64 {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.height(dir)
        }
        fn seed(&self) -> u64 {
            self.inner.seed()
        }
        fn implementation_id(&self) -> &'static str {
            self.inner.implementation_id()
        }
        fn version(&self) -> u32 {
            self.inner.version()
        }
        fn max_abs_height_m(&self) -> f64 {
            self.inner.max_abs_height_m()
        }
    }

    // spec: STRM-005
    #[test]
    fn a_cancelled_tile_job_stops_reading_at_its_next_sample_and_a_live_one_builds_the_same_tile() {
        use planet_generators::height::HeightField;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tile = TileId::new(Face(1), 6, 5, 9).unwrap();
        for format in [planet_render::TileFormat::Compact, planet_render::TileFormat::Vertex] {
            let (live_calls, cancelled_calls, direct_calls) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0));
            fn make(calls: &AtomicUsize) -> Sampled<'_> {
                Sampled { inner: planet_generators::height::Reference(7), calls }
            }
            let direct = tile_for(format, &make(&direct_calls), tile, 16, RADIUS_M, 22);
            let (inner, never) = (make(&live_calls), planet_streaming::CancelToken::never());
            let guarded = CancelGuarded { inner: &inner, cancel: &never };
            assert_eq!(tile_for(format, &guarded, tile, 16, RADIUS_M, 22), direct, "{format:?}: the guard changes nothing while live");
            assert_eq!(live_calls.load(Ordering::SeqCst), direct_calls.load(Ordering::SeqCst));
            assert_eq!(guarded.inputs_hash(), inner.inputs_hash(), "keys are the inner field's");
            let (inner, cancelled) = (make(&cancelled_calls), planet_streaming::CancelToken::cancelled());
            let guarded = CancelGuarded { inner: &inner, cancel: &cancelled };
            let _ = tile_for(format, &guarded, tile, 16, RADIUS_M, 22);
            assert_eq!(cancelled_calls.load(Ordering::SeqCst), 0, "{format:?}: a cancelled job reads nothing more");
            assert!(direct_calls.load(Ordering::SeqCst) > 100, "the tile does read the field");
        }
    }

    // spec: GEN-021, STRM-021
    #[test]
    fn a_guarded_field_over_a_bake_builds_the_tile_and_key_of_the_unguarded_field_with_the_bakes_climate() {
        use planet_core::cube::TangentWarp;
        use planet_generators::climate::analytic_climate;
        use planet_generators::erosion::ErosionGenerator;
        use planet_generators::global::{bake, GridSource};
        use planet_generators::height::HeightField;
        use planet_generators::terrain::TerrainGenerator;
        use planet_world_def::{BodyId, GenContext, WorldDefinition};
        let g = bake(7, &TangentWarp, 64, 2).unwrap();
        let src = GridSource(&g);
        let def = WorldDefinition::single_planet(RADIUS_M, 7);
        let (terrain, eroded) = (
            TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, [3; 16]).unwrap(),
            ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, [3; 16]).unwrap(),
        );
        let tile = TileId::new(Face(2), 5, 9, 14).unwrap();
        let never = planet_streaming::CancelToken::never();
        for (name, inner) in [("terrain", &terrain as &dyn HeightField), ("eroded", &eroded as &dyn HeightField)] {
            let guarded = CancelGuarded { inner, cancel: &never };
            assert_eq!(guarded.climate_version_word(), inner.climate_version_word(), "{name}");
            assert_eq!(guarded.climate(&TangentWarp, tile), inner.climate(&TangentWarp, tile), "{name}");
            for format in [planet_render::TileFormat::Compact, planet_render::TileFormat::Vertex] {
                let (a, b) = (tile_for(format, &guarded, tile, 16, RADIUS_M, 22), tile_for(format, inner, tile, 16, RADIUS_M, 22));
                assert_eq!(a, b, "{name} {format:?}: the guard changes nothing, the key and the climate included");
            }
            assert_ne!(inner.climate(&TangentWarp, tile), analytic_climate(7, &TangentWarp, tile), "{name}: it is the bake's climate");
        }
    }

    // spec: TEST-008
    #[test]
    fn a_bundle_definition_hash_is_checked_against_the_single_planet_it_names() {
        let right = definition_hash(3, RADIUS_M);
        assert!(check_definition(right, 3, RADIUS_M).is_ok());
        let none = check_definition(0, 3, RADIUS_M).unwrap_err();
        assert!(none.contains("predates the WorldDefinition") && none.contains(&format!("{right:016x}")), "{none}");
        let other = check_definition(right, 4, RADIUS_M).unwrap_err();
        assert!(other.contains("can only rebuild") && other.contains(&format!("{right:016x}")), "{other}");
        assert!(check_definition(right, 3, RADIUS_M + 1.0).is_err(), "the radius is part of the definition");
    }

    // spec: TEST-008
    #[test]
    fn a_recorded_world_is_refused_when_its_hash_field_or_radius_differs() {
        use planet_frame::{plan_terrain, BundleWorld, TerrainBundle, TerrainRequest, TerrainView};
        let def = planet_world_def::WorldDefinition::single_planet(RADIUS_M, 5);
        let toml = planet_world_def::toml_io::to_toml(&def).unwrap();
        let (_, camera) = planet_frame::scripted_views(RADIUS_M, 4.0 / 3.0, 5)[3];
        let lod = planet_frame::LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 4.0, ..planet_frame::LodParams::earth_1080p() };
        let request =
            TerrainRequest { camera, size: (256, 192), view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 5 };
        let plan = plan_terrain(&request).unwrap();
        let world = BundleWorld { height_field: "cpu-erosion-v1".into(), global_res: 8, definition_toml: toml };
        let bundle = TerrainBundle {
            generator_version: 2,
            implementation_id: "x".into(),
            mesh_version: 1,
            definition_hash: def.hash(),
            adapter: "a".into(),
            world: Some(world.clone()),
            request,
            plan,
        };
        assert!(world_of_bundle(&world, &bundle).is_ok(), "the canonical text round-trips to the recorded hash");
        let e = world_of_bundle(&world, &TerrainBundle { definition_hash: def.hash() ^ 1, ..bundle.clone() }).err().unwrap();
        assert!(e.contains("its recorded world hashes to"), "{e}");
        let wrong_field = BundleWorld { height_field: "cpu-terrain-v1".into(), ..world.clone() };
        assert!(world_of_bundle(&wrong_field, &bundle).err().unwrap().contains("cpu-erosion-v1"));
        let mut other_radius = bundle.clone();
        other_radius.request.lod.radius_m += 1.0;
        assert!(world_of_bundle(&world, &other_radius).err().unwrap().contains("radius"));
    }
}
