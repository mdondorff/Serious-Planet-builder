//! The cover draws the parent of a missing in-view tile over the missing quadrant only (STRM-016, STRM-002, SPIKE-18): the quadrant masks
//! of the cover and of the plan text. Tier A, no GPU.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId};
use planet_frame::pipeline::{draw_plan_with, ResidentRecords, ResidentTile};
use planet_frame::{
    child_index_toward, cover_selection_masked, quadrant_bit, residency_set_field, scripted_views_field, Camera, LodParams, SelectionCache,
    TerrainPlan, TerrainRequest, TerrainView, TileBounds, ALL_QUADRANTS,
};
use planet_generators::height::{HeightField, Reference};
use std::collections::{BTreeMap, HashMap};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1280, 720);

fn lod() -> LodParams {
    LodParams {
        viewport_h_px: f64::from(SIZE.1),
        tau_px: 6.0,
        min_level: 0,
        max_height_m: Reference(1).max_abs_height_m(),
        ..LodParams::earth_1080p()
    }
}

fn view(name: &str) -> Camera {
    scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &Reference(1)).into_iter().find(|(n, _)| *n == name).unwrap().1
}

fn resident_r(position: PlanetFixed) -> (HashMap<TileId, ResidentTile>, TileBounds) {
    let field = Reference(1);
    let lod = lod();
    let r = residency_set_field(&TangentWarp, position, &field, &lod, &mut SelectionCache::new());
    let records = ResidentRecords::new(&field, &TangentWarp, &lod);
    (r.tiles.iter().map(|e| (e.tile, records.record(e.tile))).collect(), r.bounds)
}

fn masked_cover(camera: &Camera, set: &HashMap<TileId, ResidentTile>, bounds: &TileBounds) -> Vec<(TileId, u8)> {
    cover_selection_masked(&TangentWarp, camera, &lod(), &|t| set.get(&t).map(|r| r.origin), bounds)
}

/// `full` without `s` and everything under it: the tile that has not streamed in yet.
fn without(full: &HashMap<TileId, ResidentTile>, s: TileId) -> HashMap<TileId, ResidentTile> {
    full.iter().filter(|(t, _)| !std::iter::successors(Some(**t), |a| a.parent()).any(|a| a == s)).map(|(t, r)| (*t, *r)).collect()
}

// spec: STRM-016, STRM-002
#[test]
fn the_parent_of_a_missing_in_view_tile_is_drawn_over_its_quadrant_only_and_a_complete_residency_draws_whole_tiles() {
    for name in ["aerial-20km", "low-2km", "ground-200m"] {
        let camera = view(name);
        let (full, bounds) = resident_r(camera.position);
        let complete = masked_cover(&camera, &full, &bounds);
        assert!(complete.iter().all(|(_, m)| *m == ALL_QUADRANTS), "{name}: every tile whole when R is resident");
        let leaves: Vec<TileId> = complete.iter().map(|(t, _)| *t).collect();
        let mut checked = 0;
        for s in leaves.iter().copied().filter(|t| t.level() >= 3).take(40) {
            let Some(parent) = s.parent() else { continue };
            let siblings: Vec<TileId> = parent.children().unwrap().into_iter().filter(|c| *c != s && leaves.contains(c)).collect();
            if siblings.is_empty() {
                continue;
            }
            let masked: BTreeMap<TileId, u8> = masked_cover(&camera, &without(&full, s), &bounds).into_iter().collect();
            let mask = masked.get(&parent).copied().unwrap_or_else(|| panic!("{name}: the parent of {s:?} is drawn"));
            assert!(mask & quadrant_bit(child_index_toward(parent, s)) != 0, "{name}: the parent draws the quadrant of the missing {s:?}");
            for sib in &siblings {
                assert_eq!(mask & quadrant_bit(child_index_toward(parent, *sib)), 0, "{name}: not the quadrant of the resident {sib:?}");
                assert!(masked.contains_key(sib), "{name}: the resident sibling {sib:?} is drawn");
            }
            checked += 1;
        }
        assert!(checked > 0, "{name}: no leaf with a leaf sibling");
    }
}

// spec: STRM-016
#[test]
fn a_node_drawn_over_some_quadrants_writes_and_reads_back_with_them_and_a_whole_node_has_no_trace_in_the_text() {
    let camera = view("low-2km");
    let (full, bounds) = resident_r(camera.position);
    let req =
        TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 1 };
    let whole = draw_plan_with(&req, None, &full, &bounds).unwrap();
    assert!(!whole.plan.to_text().contains("quadrants"), "a plan of whole tiles has no quadrants token");
    let leaves: Vec<TileId> = masked_cover(&camera, &full, &bounds).into_iter().map(|(t, _)| t).collect();
    let s = leaves
        .iter()
        .copied()
        .find(|t| t.level() >= 3 && t.parent().is_some_and(|p| p.children().unwrap().iter().filter(|c| leaves.contains(c)).count() > 1))
        .unwrap();
    let d = draw_plan_with(&req, None, &without(&full, s), &bounds).unwrap();
    let text = d.plan.to_text();
    assert!(text.contains(" quadrants "), "a node over some quadrants says so");
    let back = TerrainPlan::from_text(&text).unwrap();
    let pairs = |p: &TerrainPlan| p.nodes.iter().map(|n| (n.tile, n.quadrants)).collect::<Vec<_>>();
    assert_eq!(pairs(&back), pairs(&d.plan));
    assert!(d.plan.nodes.iter().any(|n| n.quadrants != ALL_QUADRANTS));
    assert_ne!(d.plan.plan_hash(), whole.plan.plan_hash());
    assert!(TerrainPlan::from_text(&text.replacen(" quadrants ", " quadrants 9", 1)).is_err(), "a mask above 14 is refused");
    // The bounds: 0 and 15 are not written (a node drawn over nothing is not drawn, a whole node has no token), and a legacy
    // `shore_ramp` token before the mask still reads.
    let masked = text.lines().find(|l| l.contains(" quadrants ")).unwrap().to_string();
    let stem = masked.split(" quadrants ").next().unwrap();
    for bad in ["0", "15", "-1", "x"] {
        let line = text.replacen(&masked, &format!("{stem} quadrants {bad}"), 1);
        assert!(TerrainPlan::from_text(&line).is_err(), "quadrants {bad} is refused");
    }
    let legacy = text.replacen(&masked, &format!("{stem} shore_ramp 3f800000 quadrants {}", masked.rsplit(' ').next().unwrap()), 1);
    assert_eq!(pairs(&TerrainPlan::from_text(&legacy).unwrap()), pairs(&d.plan));
}

fn turned(camera: Camera, angle_rad: f64) -> Camera {
    let up = camera.up.normalized();
    let f = camera.forward.normalized();
    Camera { forward: f * libm::cos(angle_rad) + up.cross(f) * libm::sin(angle_rad), ..camera }
}

// spec: STRM-016, STRM-002
#[test]
fn a_stale_plan_with_a_tile_drawn_over_some_quadrants_leaves_no_hole_where_the_camera_turned_to() {
    use planet_frame::plan_terrain_field;
    let field = Reference(1);
    let request = |camera: Camera| TerrainRequest {
        camera,
        size: SIZE,
        view: TerrainView::Level,
        lod: lod(),
        face_mapping: TangentWarp.id().to_string(),
        seed: 1,
    };
    let mut frames_with_masks = 0;
    for name in ["low-2km", "ground-200m"] {
        let a = view(name);
        let (full, bounds) = resident_r(a.position);
        let plan_a = plan_terrain_field(&request(a), &field).unwrap();
        for turn in [0.0f64, 0.05, 0.12, 0.3, 0.8] {
            let b = turned(a, turn);
            let plan_b = plan_terrain_field(&request(b), &field).unwrap();
            // A tile of A's plan missing with everything under it: the stand-ins draw their quadrants.
            for gone in plan_a.nodes.iter().map(|n| n.tile).filter(|t| t.level() >= 3).step_by(7).take(12) {
                let set = without(&full, gone);
                let d = draw_plan_with(&request(b), Some((&request(a), &plan_a)), &set, &bounds).unwrap();
                frames_with_masks += usize::from(d.plan.nodes.iter().any(|n| n.quadrants != ALL_QUADRANTS));
                let drawn: BTreeMap<TileId, u8> = d.plan.nodes.iter().map(|n| (n.tile, n.quadrants)).collect();
                for m in &plan_b.nodes {
                    // Something draws the node's tile: itself whole, an ancestor over the quadrant that holds it, or finer tiles under it.
                    let above = std::iter::successors(Some(m.tile), |t| t.parent()).any(|t| {
                        drawn.get(&t).is_some_and(|mask| {
                            if t == m.tile {
                                *mask == ALL_QUADRANTS
                            } else {
                                mask & quadrant_bit(child_index_toward(t, m.tile)) != 0
                            }
                        })
                    });
                    let below = drawn.keys().any(|d| std::iter::successors(Some(*d), |t| t.parent()).any(|t| t == m.tile));
                    assert!(
                        above || below,
                        "{name}, turn {turn}, without {gone:?}: the plan's node {:?} has nothing drawn over it",
                        m.tile
                    );
                }
            }
        }
    }
    assert!(frames_with_masks > 0, "some frame must draw a stand-in over some quadrants");
}

// spec: STRM-016, STRM-002
#[test]
fn a_stale_plan_is_not_taken_for_a_tile_it_draws_over_some_quadrants_whose_other_quadrants_the_frame_sees() {
    use planet_frame::plan_terrain_field;
    let field = Reference(1);
    let request = |camera: Camera| TerrainRequest {
        camera,
        size: SIZE,
        view: TerrainView::Level,
        lod: lod(),
        face_mapping: TangentWarp.id().to_string(),
        seed: 1,
    };
    let a = view("low-2km");
    let (full, bounds) = resident_r(a.position);
    let plan_a = plan_terrain_field(&request(a), &field).unwrap();
    let nodes: Vec<TileId> = plan_a.nodes.iter().map(|n| n.tile).collect();
    // A parent whose four children are all nodes of the plan: the plan draws the first two and the stale plan does not hold the others (the
    // camera that made it had them out of its frustum); the first child is the tile that has not streamed in.
    let x = nodes
        .iter()
        .filter_map(|t| t.parent())
        .find(|p| p.children().is_some_and(|c| c.iter().all(|k| nodes.contains(k))))
        .expect("a parent with four children in the plan");
    let kids = x.children().unwrap();
    let stale =
        TerrainPlan { nodes: plan_a.nodes.iter().filter(|n| n.tile != kids[2] && n.tile != kids[3]).cloned().collect(), ..plan_a.clone() };
    let b = turned(a, 0.002);
    // Only the second child is resident: the cover draws the parent over the other three quadrants, the stale plan over the first only.
    let set = without(&without(&without(&full, kids[0]), kids[2]), kids[3]);
    let d = draw_plan_with(&request(b), Some((&request(a), &stale)), &set, &bounds).unwrap();
    let drawn: BTreeMap<TileId, u8> = d.plan.nodes.iter().map(|n| (n.tile, n.quadrants)).collect();
    for k in [kids[2], kids[3]] {
        let covered = drawn.contains_key(&k)
            || std::iter::successors(k.parent(), |t| t.parent())
                .any(|t| drawn.get(&t).is_some_and(|mask| mask & quadrant_bit(child_index_toward(t, k)) != 0))
            || drawn.keys().any(|d| std::iter::successors(Some(*d), |t| t.parent()).any(|t| t == k));
        assert!(covered, "the frame sees {k:?}, which the stale plan did not draw and its stand-in does not cover");
    }
}
