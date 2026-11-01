//! REND-047, tier C: the clipmap gather. Sample texels read the tile height minus the drop; the max mips are the maxima of their
//! dilated footprints; an incremental update writes the bytes of a full rebuild; a preview stamp moves only the flagged tiles' texels.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::clipmap::{BlockMap, ClipmapParams, ClipmapPlan, ClipmapPlanner};
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{residency_set, LodParams, SelectionCache};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::Reference;
use planet_render::shadow::{ClipmapGpu, ClipmapPreview, ClipmapStamp};
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const TEXELS: u32 = 256;
const LEVELS: usize = 11;

fn lod() -> LodParams {
    LodParams { cells: CELLS, viewport_h_px: 360.0, tau_px: 4.0, max_level: 12, ..LodParams::earth_1080p() }
}

fn dir() -> Vec3 {
    Vec3::new(0.3, 0.5, 0.8).normalized()
}

struct World {
    ctx: GpuContext,
    terrain: TerrainRenderer,
    tiles: BTreeMap<TileId, CompactTile>,
    resident: Vec<TileId>,
}

fn world() -> World {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let r = residency_set(&TangentWarp, PlanetFixed(dir() * (R + 300.0)), &lod(), &mut SelectionCache::new());
    let resident: Vec<TileId> = r.tiles.iter().map(|e| e.tile).collect();
    let block_slots = 64;
    let pool = compact_format_pool_with(
        (128 << 20) + 80 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    let mut terrain = TerrainRenderer::with_pools(&ctx, (64, 48), CELLS, pool);
    let field = Reference(1);
    let mut tiles = BTreeMap::new();
    for &t in &resident {
        let data = compact_tile_field(&field, &TangentWarp, t, CELLS, R);
        terrain.upload_tile(&ctx, &data).unwrap();
        tiles.insert(t, data);
    }
    World { ctx, terrain, tiles, resident }
}

fn params() -> ClipmapParams {
    ClipmapParams { texels: TEXELS, ..ClipmapParams::new(R, CELLS) }
}

fn camera() -> PlanetFixed {
    PlanetFixed(dir() * (R + 300.0))
}

/// The finest block covering texel `(i, j)` of a level: the one the gather leaves in it.
fn owner(plan: &ClipmapPlan, level: usize, i: i64, j: i64) -> Option<&BlockMap> {
    plan.levels[level].blocks.iter().rev().find(|b| b.rect.contains(i, j) && b.covers(i, j))
}

fn expected(w: &World, plan: &ClipmapPlan, level: usize, i: i64, j: i64) -> Option<f64> {
    let b = owner(plan, level, i, j)?;
    let tile = &w.tiles[&b.tile];
    let (u, v) = b.uv_at(i, j);
    let (fu, fv) = (u.clamp(0.0, 1.0) * f64::from(CELLS), v.clamp(0.0, 1.0) * f64::from(CELLS));
    let (i0, j0) = ((fu.floor() as usize).min(CELLS as usize - 1), (fv.floor() as usize).min(CELLS as usize - 1));
    let (tu, tv) = (fu - i0 as f64, fv - j0 as f64);
    let h = |a: usize, b: usize| f64::from(tile.rel_height(a, b));
    let rel = (h(i0, j0) * (1.0 - tu) + h(i0 + 1, j0) * tu) * (1.0 - tv) + (h(i0, j0 + 1) * (1.0 - tu) + h(i0 + 1, j0 + 1) * tu) * tv;
    Some(f64::from(tile.h_origin) + rel - b.drop_at(i, j, plan.levels[level].texel_m))
}

fn slot(c: i64) -> usize {
    c.rem_euclid(i64::from(TEXELS)) as usize
}

// spec: REND-047
#[test]
fn sample_texels_read_the_tile_height_minus_the_drop_and_the_mips_are_dilated_maxima() {
    let w = world();
    let mut planner = ClipmapPlanner::new(params());
    let plan = planner.plan(camera(), &w.resident, &[], &[], &TangentWarp);
    let mut gpu = ClipmapGpu::new(&w.ctx, TEXELS, LEVELS, CELLS);
    gpu.gather(&w.ctx, &w.terrain, &plan, &ClipmapPreview::default());
    let mut checked = 0;
    for level in [0usize, 2, 4, 6] {
        let data = gpu.read_level(&w.ctx, level, 0);
        let win = plan.levels[level].window;
        for j in (win.y0..win.y1).step_by(7) {
            for i in (win.x0..win.x1).step_by(7) {
                let Some(want) = expected(&w, &plan, level, i, j) else { continue };
                let got = f64::from(data[slot(j) * TEXELS as usize + slot(i)]);
                // 1 mm, and the f32 rounding of values of a few kilometres (the heights and the drop of a coarse level).
                let tol = 1.0e-3 + 2.0e-7 * want.abs().max(plan.levels[level].texel_m);
                assert!((got - want).abs() <= tol, "level {level} texel ({i},{j}): gpu {got} vs {want} (tol {tol})");
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "only {checked} texels had a covering block");

    // Each mip texel is the maximum of the 2 x 2 footprint dilated by one texel, toroidally.
    for level in [0usize, 5] {
        let mut below = gpu.read_level(&w.ctx, level, 0);
        let mut edge = TEXELS as i64;
        for mip in 1..=3u32 {
            let here = gpu.read_level(&w.ctx, level, mip);
            let child = edge / 2;
            for cy in 0..child {
                for cx in 0..child {
                    let mut best = f32::MIN;
                    for dy in -1..=2 {
                        for dx in -1..=2 {
                            let (px, py) = ((2 * cx + dx).rem_euclid(edge), (2 * cy + dy).rem_euclid(edge));
                            best = best.max(below[(py * edge + px) as usize]);
                        }
                    }
                    assert_eq!(here[(cy * child + cx) as usize], best, "level {level} mip {mip} texel ({cx},{cy})");
                }
            }
            below = here;
            edge = child;
        }
    }
}

// spec: REND-047
#[test]
fn an_incremental_gather_writes_the_bytes_of_a_full_rebuild() {
    let mut w = world();
    let mut planner = ClipmapPlanner::new(params());
    let first = planner.plan(camera(), &w.resident, &[], &[], &TangentWarp);
    let mut incremental = ClipmapGpu::new(&w.ctx, TEXELS, LEVELS, CELLS);
    incremental.gather(&w.ctx, &w.terrain, &first, &ClipmapPreview::default());

    // Refresh the leaf under the camera with other heights, at its slot.
    let leaf =
        (0..=28u8).rev().map(|l| TileId::from_direction(&TangentWarp, dir(), l)).find(|t| w.resident.binary_search(t).is_ok()).unwrap();
    let slot_ref = w.terrain.slot_of(leaf).unwrap();
    let data = compact_tile_field(&Reference(2), &TangentWarp, leaf, CELLS, R);
    w.terrain.upload_tile_at(&w.ctx, &data, slot_ref).unwrap();
    let plan = planner.plan(camera(), &w.resident, &[leaf], &[], &TangentWarp);
    assert!(!plan.reanchored && plan.levels.iter().any(|l| !l.dirty.is_empty()));
    incremental.gather(&w.ctx, &w.terrain, &plan, &ClipmapPreview::default());

    let mut fresh_planner = ClipmapPlanner::new(params());
    let full_plan = fresh_planner.plan(camera(), &w.resident, &[], &[], &TangentWarp);
    let mut full = ClipmapGpu::new(&w.ctx, TEXELS, LEVELS, CELLS);
    full.gather(&w.ctx, &w.terrain, &full_plan, &ClipmapPreview::default());
    for level in 0..LEVELS {
        for mip in [0u32, 1, 3] {
            let (a, b) = (incremental.read_level(&w.ctx, level, mip), full.read_level(&w.ctx, level, mip));
            assert!(
                a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()),
                "level {level} mip {mip}: incremental differs from a full rebuild"
            );
        }
    }
}

// spec: REND-047
#[test]
fn a_preview_stamp_moves_only_the_flagged_tiles_texels() {
    let w = world();
    let mut planner = ClipmapPlanner::new(params());
    let plan = planner.plan(camera(), &w.resident, &[], &[], &TangentWarp);
    let mut gpu = ClipmapGpu::new(&w.ctx, TEXELS, LEVELS, CELLS);
    gpu.gather(&w.ctx, &w.terrain, &plan, &ClipmapPreview::default());
    let before = gpu.read_level(&w.ctx, 0, 0);

    let texel = plan.levels[0].texel_m;
    let reach = 4.0 * texel;
    let stamp = ClipmapStamp { plane_m: [0.0, 0.0], reach_m: reach as f32, amount_m: 10.0, kind: 1.0, sign: 1.0, max_delta_m: 10.0 };
    let flagged_tile = owner(&plan, 0, 0, 0).expect("a block over the anchor").tile;
    let preview = ClipmapPreview { stamps: vec![stamp], delta_count: 1, flagged: BTreeMap::from([(flagged_tile, 1u8)]) };
    // The stamp's footprint is dirty (the planner is told of the stamp), the texels are rewritten with the delta.
    let stamp_dir = dir();
    let with = planner.plan(
        camera(),
        &w.resident,
        &[],
        &[planet_frame::preview::PreviewStamp {
            sign: 1,
            kind: planet_world_def::stamp::StampKind::Hill,
            amount_m: 10.0,
            dir: stamp_dir,
            reach_m: reach,
            max_delta_m: 10.0,
        }],
        &TangentWarp,
    );
    gpu.gather(&w.ctx, &w.terrain, &with, &preview);
    let after = gpu.read_level(&w.ctx, 0, 0);

    let (mut moved, mut flagged_texels, mut other_texels) = (0, 0, 0);
    for j in -8i64..8 {
        for i in -8i64..8 {
            let k = slot(j) * TEXELS as usize + slot(i);
            let delta = f64::from(after[k]) - f64::from(before[k]);
            let (px, py) = ((i as f64 + 0.5) * texel, (j as f64 + 0.5) * texel);
            let x = libm::hypot(px, py) / reach;
            let Some(b) = owner(&with, 0, i, j) else { continue };
            if b.tile == flagged_tile {
                let want = if x < 1.0 { 10.0 * (1.0 - x * x) * (1.0 - x * x) } else { 0.0 };
                assert!((delta - want).abs() <= 2.0e-3 + 2.0e-4 * want, "flagged texel ({i},{j}): delta {delta} vs {want}");
                flagged_texels += 1;
                moved += usize::from(delta.abs() > 1.0e-3);
            } else {
                assert_eq!(delta, 0.0, "unflagged tile {:?} texel ({i},{j}) moved by {delta}", b.tile);
                other_texels += 1;
            }
        }
    }
    assert!(moved > 4 && flagged_texels > 0, "the flagged tile must carry the delta (moved {moved}, flagged {flagged_texels})");
    println!("preview: {flagged_texels} flagged texels ({moved} moved), {other_texels} on other tiles");

    // The same stamp with no flagged tile moves nothing: the vertex stage draws only previewed nodes.
    let mut unflagged = ClipmapGpu::new(&w.ctx, TEXELS, LEVELS, CELLS);
    let no_flags = ClipmapPreview { flagged: BTreeMap::new(), ..preview };
    let mut fresh = ClipmapPlanner::new(params());
    let full = fresh.plan(camera(), &w.resident, &[], &[], &TangentWarp);
    unflagged.gather(&w.ctx, &w.terrain, &full, &no_flags);
    let plain = unflagged.read_level(&w.ctx, 0, 0);
    assert!(plain.iter().zip(&before).all(|(a, b)| a.to_bits() == b.to_bits()), "an unflagged stamp changed the clipmap");
}
