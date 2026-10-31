//! REND-046, tier A: the height clipmap plan. Levels, anchor and snap, per-tile maps and drop fits against the f64 projection and sphere,
//! scroll dirtiness, determinism and the finest-per-region tile lists.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::clipmap::{drop_tolerance_m, tile_uv, BlockMap, ClipmapParams, ClipmapPlan, ClipmapPlanner, TexelRect};
use planet_frame::preview::PreviewStamp;
use planet_frame::{residency_set, LodParams, SelectionCache};
use planet_world_def::stamp::StampKind;

const R: f64 = 6_371_000.0;

fn lod() -> LodParams {
    LodParams { cells: 16, viewport_h_px: 360.0, tau_px: 4.0, max_level: 16, min_level: 0, ..LodParams::earth_1080p() }
}

fn params() -> ClipmapParams {
    ClipmapParams { texels: 256, ..ClipmapParams::new(R, 16) }
}

fn resident_at(dir: Vec3) -> Vec<TileId> {
    let r = residency_set(&TangentWarp, PlanetFixed(dir * (R + 300.0)), &lod(), &mut SelectionCache::new());
    r.tiles.iter().map(|e| e.tile).collect()
}

fn unproject(anchor: Vec3, x: f64, y: f64) -> Vec3 {
    let (e1, e2) = anchor.tangent_basis();
    let rho = libm::hypot(x, y);
    let th = rho / R;
    (anchor * libm::cos(th) + (e1 * x + e2 * y) * (libm::sin(th) / rho)).normalized()
}

fn plan_for(dir: Vec3) -> (ClipmapPlanner, ClipmapPlan, Vec<TileId>) {
    let resident = resident_at(dir);
    let mut planner = ClipmapPlanner::new(params());
    let plan = planner.plan(PlanetFixed(dir * (R + 300.0)), &resident, &[], &[], &TangentWarp);
    (planner, plan, resident)
}

fn check_block(plan: &ClipmapPlan, texel_m: f64, b: &BlockMap) {
    let (cx, cy) = ((b.rect.x0 + b.rect.x1 - 1) / 2, (b.rect.y0 + b.rect.y1 - 1) / 2);
    let pts = [(b.rect.x0, b.rect.y0), (b.rect.x1 - 1, b.rect.y0), (b.rect.x0, b.rect.y1 - 1), (b.rect.x1 - 1, b.rect.y1 - 1), (cx, cy)];
    let step_u = f64::from((b.u[1].abs() + b.u[2].abs()).max(b.v[1].abs() + b.v[2].abs()));
    for (i, j) in pts {
        let (px, py) = ((i as f64 + 0.5) * texel_m, (j as f64 + 0.5) * texel_m);
        let d = unproject(plan.anchor, px, py);
        let (tu, tv) = tile_uv(&TangentWarp, b.tile, d);
        let (u, v) = b.uv_at(i, j);
        assert!(
            (u - tu).abs().max((v - tv).abs()) <= 0.125 * step_u + 1e-9,
            "tile {:?} texel ({i},{j}) level texel {texel_m}: uv ({u},{v}) vs ({tu},{tv})",
            b.tile
        );
        let th = libm::hypot(px, py) / R;
        let drop = 2.0 * R * libm::sin(0.5 * th) * libm::sin(0.5 * th);
        let got = b.drop_at(i, j, texel_m);
        let slack = drop_tolerance_m(texel_m) + 1.0e-6 * drop.max(1.0);
        assert!((got - drop).abs() <= slack, "tile {:?} texel ({i},{j}): drop {got} vs {drop} (slack {slack})", b.tile);
    }
}

// spec: REND-046
#[test]
fn every_tile_map_lands_within_an_eighth_texel_and_the_drop_within_its_tolerance() {
    for dir in [Vec3::new(1.0, 1.0, 1.0).normalized(), Vec3::new(1.0, 0.02, 0.01).normalized(), Vec3::new(0.3, 0.5, 0.8).normalized()] {
        let (_, plan, _) = plan_for(dir);
        assert_eq!(plan.levels.len(), 11);
        for lv in &plan.levels {
            assert!(!lv.blocks.is_empty(), "level texel {} has no blocks", lv.texel_m);
            for b in lv.blocks.iter() {
                assert!(
                    b.fit_error().is_finite()
                        && (f64::from(b.fit_error()) <= 0.125 + 1e-9 || (b.rect.width() <= 4 && b.rect.height() <= 4)),
                    "{b:?}"
                );
                check_block(&plan, lv.texel_m, b);
            }
        }
    }
}

fn assert_windows_covered(plan: &ClipmapPlan, resident: &[TileId], what: &str) {
    let p = params();
    for (k, lv) in plan.levels.iter().enumerate() {
        let deepest = p.deepest_level_for(lv.texel_m);
        assert!(lv.blocks.iter().all(|b| b.tile.level() <= deepest), "{what} level {k}: a tile finer than the texel");
        assert!(lv.blocks.windows(2).all(|w| w[0].tile.level() <= w[1].tile.level()), "{what} level {k}: blocks are not coarse to fine");
        let mut holes = 0;
        for j in (lv.window.y0..lv.window.y1).step_by(5) {
            for i in (lv.window.x0..lv.window.x1).step_by(5) {
                if lv.blocks.iter().any(|b| b.rect.contains(i, j) && b.covers(i, j)) {
                    continue;
                }
                // Only a texel where R holds no tile at all (beyond its horizon cut) may stay empty.
                let d = unproject(plan.anchor, (i as f64 + 0.5) * lv.texel_m, (j as f64 + 0.5) * lv.texel_m);
                if (0..=deepest).any(|l| resident.binary_search(&TileId::from_direction(&TangentWarp, d, l)).is_ok()) {
                    holes += 1;
                }
            }
        }
        assert_eq!(holes, 0, "{what} level {k}: texels of the window no block covers although R has a tile there");
    }
}

// spec: REND-046
#[test]
fn every_window_texel_is_inside_some_block_and_no_tile_is_finer_than_its_level() {
    for dir in [Vec3::new(1.0, 1.0, 1.0).normalized(), Vec3::new(0.3, 0.5, 0.8).normalized()] {
        let (_, plan, resident) = plan_for(dir);
        assert_windows_covered(&plan, &resident, "first plan");
    }
}

// spec: REND-046
#[test]
fn the_windows_stay_covered_after_a_move_that_does_not_re_anchor() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let (mut planner, first, _) = plan_for(dir);
    let reach = 0.24 * f64::from(params().texels) * first.levels[0].texel_m;
    let (e1, e2) = dir.tangent_basis();
    let to = (dir * libm::cos(reach / R) + (e1 + e2).normalized() * libm::sin(reach / R)).normalized();
    let resident = resident_at(to);
    let plan = planner.plan(PlanetFixed(to * (R + 300.0)), &resident, &[], &[], &TangentWarp);
    if !plan.reanchored {
        assert_windows_covered(&plan, &resident, "moved");
    }
}

// spec: REND-046
#[test]
fn a_teleport_to_the_antipode_re_anchors() {
    let a = Vec3::new(1.0, 0.0, 0.0);
    let (mut planner, first, _) = plan_for(a);
    let b = Vec3::new(-1.0, 0.0, 0.0);
    let resident = resident_at(b);
    let plan = planner.plan(PlanetFixed(b * (R + 300.0)), &resident, &[], &[], &TangentWarp);
    assert!(plan.reanchored && plan.generation > first.generation);
    assert!(plan.levels.iter().all(|l| !l.blocks.is_empty()));
}

// spec: REND-046
#[test]
fn the_same_input_gives_the_same_plan() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let (_, a, _) = plan_for(dir);
    let (_, b, _) = plan_for(dir);
    assert_eq!(a, b);
    assert!(a.levels.iter().all(|l| l.dirty == vec![l.window]), "the first plan dirties every window");
}

fn moved(dir: Vec3, meters: f64) -> Vec3 {
    let (e1, _) = dir.tangent_basis();
    let th = meters / R;
    (dir * libm::cos(th) + e1 * libm::sin(th)).normalized()
}

// spec: REND-046
#[test]
fn a_small_move_dirties_only_the_uncovered_strips_and_a_large_one_re_anchors() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let (mut planner, first, resident) = plan_for(dir);
    let texel0 = first.levels[0].texel_m;
    let n = i64::from(params().texels);

    let second = planner.plan(PlanetFixed(moved(dir, 3.0 * texel0) * (R + 300.0)), &resident, &[], &[], &TangentWarp);
    assert!(!second.reanchored);
    assert_eq!(second.generation, first.generation);
    let area0: i64 = second.levels[0].dirty.iter().map(TexelRect::area).sum();
    assert_eq!(area0, 3 * n, "level 0 dirties a strip of 3 texels");
    for (k, lv) in second.levels.iter().enumerate() {
        let moved_texels = (lv.window.x0 - first.levels[k].window.x0).abs() + (lv.window.y0 - first.levels[k].window.y0).abs();
        let area: i64 = lv.dirty.iter().map(TexelRect::area).sum();
        assert_eq!(area, moved_texels * n, "level {k}: dirty area is the uncovered strips only");
    }

    let third = planner.plan(PlanetFixed(moved(dir, 0.5 * n as f64 * texel0) * (R + 300.0)), &resident, &[], &[], &TangentWarp);
    assert!(third.reanchored && third.generation > second.generation);
    assert!(third.levels.iter().all(|l| l.dirty == vec![l.window]), "a re-anchor dirties every level");
}

// spec: REND-046
#[test]
fn an_evicted_tile_and_a_preview_stamp_dirty_their_footprints() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let (mut planner, _first, resident) = plan_for(dir);
    let under = (0..=28u8).rev().map(|l| TileId::from_direction(&TangentWarp, dir, l)).find(|t| resident.binary_search(t).is_ok()).unwrap();
    // A neighbour of the tile under the camera: the finest level under the camera (and so the texel) stays.
    let finest = under.neighbors8().into_iter().find(|t| resident.binary_search(t).is_ok()).expect("a resident neighbour");
    let rest: Vec<TileId> = resident.iter().copied().filter(|t| *t != finest).collect();
    let cam = PlanetFixed(dir * (R + 300.0));
    let evicted = planner.plan(cam, &rest, &[], &[], &TangentWarp);
    assert!(!evicted.reanchored);
    let covered: i64 = evicted.levels.iter().flat_map(|l| l.dirty.iter()).map(TexelRect::area).sum();
    assert!(covered > 0 && evicted.levels.iter().all(|l| l.blocks.iter().all(|b| b.tile != finest)));

    let steady = planner.plan(cam, &rest, &[], &[], &TangentWarp);
    assert!(steady.levels.iter().all(|l| l.dirty.is_empty()), "nothing changed: nothing is dirty");

    let stamp = PreviewStamp { sign: 1, kind: StampKind::Hill, amount_m: 10.0, dir, reach_m: 20.0, max_delta_m: 10.0 };
    let with = planner.plan(cam, &rest, &[], &[stamp], &TangentWarp);
    let l0 = &with.levels[0];
    let reach = (20.0 / l0.texel_m) as i64;
    assert!(l0.dirty.iter().any(|r| r.x0 <= -reach && r.x1 >= reach && r.y0 <= -reach && r.y1 >= reach));
    // The frame after the stamp is gone still rewrites its footprint (the previous frame's stamp is undone); then nothing is dirty.
    let gone = planner.plan(cam, &rest, &[], &[], &TangentWarp);
    assert!(!gone.levels[0].dirty.is_empty());
    let after = planner.plan(cam, &rest, &[], &[], &TangentWarp);
    assert!(after.levels.iter().all(|l| l.dirty.is_empty()));
}

// spec: REND-046
#[test]
#[ignore = "timing probe: prints the plan time for a large R"]
fn plan_time_for_a_large_r() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let p = LodParams { max_level: 20, cells: 32, ..lod() };
    let r = residency_set(&TangentWarp, PlanetFixed(dir * (R + 300.0)), &p, &mut SelectionCache::new());
    let resident: Vec<TileId> = r.tiles.iter().map(|e| e.tile).collect();
    let mut planner = ClipmapPlanner::new(ClipmapParams::new(R, 16));
    let cam = PlanetFixed(dir * (R + 300.0));
    let t = std::time::Instant::now();
    planner.plan(cam, &resident, &[], &[], &TangentWarp);
    let first = t.elapsed();
    let t = std::time::Instant::now();
    for _ in 0..100 {
        planner.plan(cam, &resident, &[], &[], &TangentWarp);
    }
    println!("R = {} tiles: first plan {first:?}, steady plan {:?}", resident.len(), t.elapsed() / 100);
}

// spec: REND-046
#[test]
fn an_incremental_plan_has_the_blocks_of_a_plan_from_scratch() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let (mut planner, first, resident) = plan_for(dir);
    let under = (0..=28u8).rev().map(|l| TileId::from_direction(&TangentWarp, dir, l)).find(|t| resident.binary_search(t).is_ok()).unwrap();
    let neighbour = under.neighbors8().into_iter().find(|t| resident.binary_search(t).is_ok()).expect("a resident neighbour");
    let rest: Vec<TileId> = resident.iter().copied().filter(|t| *t != neighbour).collect();
    let to = moved(dir, 7.0 * first.levels[0].texel_m);
    let cam = PlanetFixed(to * (R + 300.0));
    let incremental = planner.plan(cam, &rest, &[], &[], &TangentWarp);
    assert!(!incremental.reanchored);
    // From scratch around the same anchor: a planner that was first asked at the anchor, then at the same pose, with the same R.
    let mut fresh = ClipmapPlanner::new(params());
    fresh.plan(PlanetFixed(dir * (R + 300.0)), &rest, &[], &[], &TangentWarp);
    let scratch = fresh.plan(cam, &rest, &[], &[], &TangentWarp);
    for (k, (a, b)) in incremental.levels.iter().zip(&scratch.levels).enumerate() {
        assert_eq!(a.window, b.window, "level {k}");
        assert_eq!(a.blocks, b.blocks, "level {k}: incremental blocks differ from a plan from scratch");
    }
}

// spec: REND-046
#[test]
fn a_camera_hovering_on_a_level_boundary_does_not_thrash_the_anchor() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let (mut planner, first, resident) = plan_for(dir);
    let under = (0..=28u8).rev().map(|l| TileId::from_direction(&TangentWarp, dir, l)).find(|t| resident.binary_search(t).is_ok()).unwrap();
    // The finest tile under the camera comes and goes every frame: its level, and so the wanted texel, flips.
    let without: Vec<TileId> = resident.iter().copied().filter(|t| *t != under).collect();
    let cam = PlanetFixed(dir * (R + 300.0));
    for frame in 0..40 {
        let plan = planner.plan(cam, if frame % 2 == 0 { &without } else { &resident }, &[], &[], &TangentWarp);
        assert!(!plan.reanchored && plan.generation == first.generation, "frame {frame}: the anchor thrashed");
    }
    // A change that holds re-anchors once, after the hysteresis.
    let mut generations = std::collections::BTreeSet::new();
    for _ in 0..planet_frame::clipmap::TEXEL_CHANGE_HYSTERESIS_FRAMES + 2 {
        generations.insert(planner.plan(cam, &without, &[], &[], &TangentWarp).generation);
    }
    assert_eq!(generations.len(), 2, "one re-anchor once the change held: {generations:?}");
}

// spec: REND-046
#[test]
#[ignore = "timing probe: prints the incremental plan times (one tile landing, a scroll of a texel)"]
fn incremental_plan_times() {
    let dir = Vec3::new(0.3, 0.5, 0.8).normalized();
    let p = LodParams { max_level: 20, cells: 32, ..lod() };
    let r = residency_set(&TangentWarp, PlanetFixed(dir * (R + 300.0)), &p, &mut SelectionCache::new());
    let resident: Vec<TileId> = r.tiles.iter().map(|e| e.tile).collect();
    let mut planner = ClipmapPlanner::new(ClipmapParams::new(R, 32));
    let cam = PlanetFixed(dir * (R + 300.0));
    let t = std::time::Instant::now();
    let first = planner.plan(cam, &resident, &[], &[], &TangentWarp);
    println!("first plan {:?}", t.elapsed());
    let texel0 = first.levels[0].texel_m;
    let leaf = resident.iter().rev().copied().find(|t| t.level() > 8 && t.level() < 14).unwrap();
    let without: Vec<TileId> = resident.iter().copied().filter(|t| *t != leaf).collect();
    let t = std::time::Instant::now();
    planner.plan(cam, &without, &[], &[], &TangentWarp);
    println!("one tile evicted {:?}", t.elapsed());
    let t = std::time::Instant::now();
    planner.plan(cam, &resident, &[], &[], &TangentWarp);
    println!("one tile landed {:?}", t.elapsed());
    let t = std::time::Instant::now();
    for i in 1..=20 {
        planner.plan(PlanetFixed(moved(dir, f64::from(i) * texel0) * (R + 300.0)), &resident, &[], &[], &TangentWarp);
    }
    println!("scroll by a texel, mean of 20 {:?}", t.elapsed() / 20);
}
