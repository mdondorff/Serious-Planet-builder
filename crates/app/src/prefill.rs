//! The coarse pre-fill of the tile pack (STRM-021, ADR 0045 point 3): `generate --prefill-dir DIR --global-res N` writes the
//! tiles of levels `F..=--prefill-levels` (default 4; F the render floor, `--floor-level`, default 3, LOD-011) of a stored bake into
//! its pack, and `generate --global-dir` does the same right after the bake. The tiles are the ones the streamer generates on demand (`TileSpec::generate`), so a pre-filled tile
//! equals the generated one bit for bit and a second run writes nothing.

use crate::worldfield::{form_of_format, load_world, WorldTerrain};
use crate::{err, Options};
use planet_cache::meshpack::{prefill, TileSpec, DEFAULT_PREFILL_LEVELS};
use planet_cache::pack::DEFAULT_MAX_PACK_BYTES;
use std::io::Write;
use std::path::Path;

/// Bytes of the pack files (`*.tpack`) of a store directory.
pub fn pack_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "tpack"))
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// `generate --prefill-dir DIR`: pre-fill the stored bake of `--global-res` cells in `DIR`.
pub(crate) fn prefill_dir(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    let dir = o.prefill_dir.as_ref().expect("checked by the caller");
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
        "--compact-cache",
        "--cap-bytes",
    ]
    .into_iter()
    .filter(|f| o.given.contains(*f))
    .collect();
    if !clash.is_empty() {
        return Err(format!("--prefill-dir pre-fills a stored bake and excludes {}", clash.join(", ")));
    }
    if !o.given.contains("--global-res") {
        return Err("--prefill-dir needs --global-res N: the cells per face edge of the stored bake whose tiles it writes".into());
    }
    if !dir.is_dir() {
        return Err(format!("{}: no such store directory (a bake made by `generate --global-dir`)", dir.display()));
    }
    prefill_store(o, dir, out)
}

/// Pre-fill the store in `dir` (a bake of `o.global_res` cells of the definition of `--world` or `--seed`) with levels
/// `F..=--prefill-levels` (F the effective render floor) and print what was done: tiles, bytes and wall time. A `--prefill-levels`
/// below the floor writes nothing and says so.
pub(crate) fn prefill_store(o: &Options, dir: &Path, out: &mut dyn Write) -> Result<(), String> {
    let levels = o.prefill_levels.unwrap_or(DEFAULT_PREFILL_LEVELS);
    if levels == 0 {
        return writeln!(out, "pre-fill {}: disabled (--prefill-levels 0)", dir.display()).map_err(err);
    }
    let def = match &o.world {
        Some(p) => load_world(p)?,
        None => planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
    };
    let world = WorldTerrain::open_store(def, dir, o.global_res)?;
    let field = world.field()?;
    let format = o.tile_format.unwrap_or(planet_render::TileFormat::Compact);
    let max_level = o.finest_level_for(o.cells);
    let floor = o.floor_for(o.cells);
    if levels < floor {
        return writeln!(
            out,
            "pre-fill {}: nothing to write: --prefill-levels {levels} is below the render floor {floor} (no tile of a lower level is drawn, LOD-011)",
            dir.display()
        )
        .map_err(err);
    }
    let spec = TileSpec::new(field.as_ref(), world.radius_m(), o.cells, max_level, form_of_format(format)).with_floor(floor);
    let threads = if o.threads == 0 { std::thread::available_parallelism().map_or(1, |n| n.get()) } else { o.threads };
    let (bytes_before, started) = (pack_bytes(dir), std::time::Instant::now());
    let report = prefill(dir, &spec, levels, threads, DEFAULT_MAX_PACK_BYTES, &|level, n| {
        eprintln!("pre-fill: level {level} done ({n} tiles)");
    })
    .map_err(|e| format!("pre-fill {}: {e}", dir.display()))?;
    let secs = started.elapsed().as_secs_f64();
    let grown = pack_bytes(dir).saturating_sub(bytes_before);
    writeln!(
        out,
        "pre-fill {}: levels {floor}..={levels} of cells {}, {} form, finest level {max_level}: {} tiles, {} written, {} already stored; \
         {} payload bytes, packs grew by {grown} bytes; {secs:.1} s on {threads} threads",
        dir.display(),
        o.cells,
        format.name(),
        report.total,
        report.written,
        report.present,
        report.payload_bytes
    )
    .map_err(err)
}
