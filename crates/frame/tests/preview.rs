//! REND-033: the preview list of an edit, from the footprint items before and after it.

use planet_core::Vec3;
use planet_frame::preview::{preview_delta, PreviewStamp, MAX_PREVIEW_STAMPS};
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn spec(id: u128, priority: i32, at: Vec3, kind: StampKind, amount: f64) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority, dir: at, content_hash: 0, reach_m: 2_000.0, max_delta_m: 100.0, stamp: None }
        .with_stamp(Stamp::new(kind, [amount, 0.0, 0.0]))
}

fn world(items: &[ItemSpec]) -> WorldDefinition {
    let mut d = WorldDefinition::single_planet(R, 1);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i.clone()).unwrap();
    }
    d
}

fn all(d: &WorldDefinition) -> Vec<FootprintItem> {
    d.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre: dir(1.0, 0.0, 0.0), angle: core::f64::consts::PI }).unwrap()
}

fn signs(list: &[PreviewStamp]) -> Vec<(i8, StampKind, f64)> {
    list.iter().map(|s| (s.sign, s.kind, s.amount_m)).collect()
}

// spec: REND-033
#[test]
fn an_edit_is_the_removals_and_additions_that_differ_in_application_order_and_nothing_for_an_unchanged_item() {
    let cam = dir(1.0, 0.0, 0.0);
    let keep = spec(7, 0, dir(1.0, 0.3, 0.0), StampKind::Plateau, 300.0);
    let a = spec(1, 1, dir(1.0, 0.05, 0.0), StampKind::Hill, 40.0);
    let before = world(std::slice::from_ref(&keep));
    // Added.
    let added = preview_delta(&all(&before), &all(&world(&[keep.clone(), a.clone()])), cam);
    assert_eq!(signs(&added.stamps), [(1, StampKind::Hill, 40.0)]);
    assert_eq!(added.dropped, 0);
    // Removed.
    let removed = preview_delta(&all(&world(&[keep.clone(), a.clone()])), &all(&before), cam);
    assert_eq!(signs(&removed.stamps), [(-1, StampKind::Hill, 40.0)]);
    // Changed in size: the old state out, the new one in, in that order.
    let bigger = spec(1, 1, dir(1.0, 0.05, 0.0), StampKind::Hill, 70.0);
    let changed = preview_delta(&all(&world(&[keep.clone(), a.clone()])), &all(&world(&[keep.clone(), bigger])), cam);
    assert_eq!(signs(&changed.stamps), [(-1, StampKind::Hill, 40.0), (1, StampKind::Hill, 70.0)]);
    // Moved: the old place out, the new place in.
    let moved = spec(1, 1, dir(1.0, 0.08, 0.0), StampKind::Hill, 40.0);
    let m = preview_delta(&all(&world(&[keep.clone(), a.clone()])), &all(&world(&[keep.clone(), moved])), cam);
    assert_eq!(m.stamps.iter().map(|s| s.sign).collect::<Vec<_>>(), [-1, 1]);
    assert!((m.stamps[0].dir - dir(1.0, 0.05, 0.0)).length() < 1e-12 && (m.stamps[1].dir - dir(1.0, 0.08, 0.0)).length() < 1e-12);
    // Nothing changed: nothing to preview. The unchanged item never appears.
    assert!(preview_delta(&all(&before), &all(&before), cam).stamps.is_empty());
    // Application order: priority, then id, whatever the order of discovery.
    let late = spec(2, 5, dir(1.0, 0.04, 0.0), StampKind::Crater, 30.0);
    let early = spec(9, -1, dir(1.0, 0.06, 0.0), StampKind::Hill, 10.0);
    let both = preview_delta(&all(&before), &all(&world(&[keep, late, early])), cam);
    assert_eq!(signs(&both.stamps), [(1, StampKind::Hill, 10.0), (1, StampKind::Crater, 30.0)], "-1 first, then 5");
    // The carried numbers are the item's: reach, bound, unit anchor.
    assert_eq!((both.stamps[1].reach_m, both.stamps[1].max_delta_m), (2_000.0, 100.0));
    assert!((both.stamps[1].dir.length() - 1.0).abs() < 1e-12);
}

// spec: REND-033
#[test]
fn a_list_beyond_eight_keeps_the_eight_nearest_to_the_camera_and_counts_the_rest() {
    let cam = dir(1.0, 0.0, 0.0);
    let items: Vec<ItemSpec> =
        (0..11u128).map(|k| spec(k + 1, 0, dir(1.0, 0.02 * (k as f64 + 1.0), 0.0), StampKind::Hill, 10.0 + k as f64)).collect();
    let list = preview_delta(&[], &all(&world(&items)), cam);
    assert_eq!(list.stamps.len(), MAX_PREVIEW_STAMPS);
    assert_eq!(list.dropped, 3);
    // The nearest are ids 1 to 8 (the anchors drift away from the camera as the id grows); the list is in application order.
    let amounts: Vec<f64> = list.stamps.iter().map(|s| s.amount_m).collect();
    assert_eq!(amounts, (0..8).map(|k| 10.0 + f64::from(k)).collect::<Vec<_>>());
}

// spec: REND-033
#[test]
fn only_a_node_drawn_from_a_stale_tile_under_a_stamp_is_previewed_and_the_exact_tile_ends_it() {
    use planet_core::cube::TangentWarp;
    use planet_frame::pipeline::{draw_plan_preview, draw_plan_with, ResidentTile};
    use planet_frame::TileBounds;
    use planet_frame::{plan_terrain_field, scripted_views_field, LodParams, TerrainRequest, TerrainView};
    use planet_generators::height::Reference;
    use std::collections::HashMap;

    let (_, camera) = scripted_views_field(R, 4.0 / 3.0, &Reference(1))[6];
    let req = TerrainRequest {
        camera,
        size: (128, 96),
        view: TerrainView::Level,
        lod: LodParams { cells: 16, viewport_h_px: 192.0, tau_px: 2.5, max_level: 26, ..LodParams::earth_1080p() },
        face_mapping: "tangent-v1".into(),
        seed: 1,
    };
    let plan = plan_terrain_field(&req, &Reference(1)).unwrap();
    let (near, far) = {
        let by_height = |n: &&planet_frame::TerrainDraw| n.tile.center_direction(&TangentWarp).dot(camera.position.0.normalized());
        let mut v: Vec<&planet_frame::TerrainDraw> = plan.nodes.iter().collect();
        v.sort_by(|a, b| by_height(b).total_cmp(&by_height(a)));
        (v[0].tile, v[v.len() - 1].tile)
    };
    let stamp = |tile: planet_core::TileId| PreviewStamp {
        sign: 1,
        kind: StampKind::Hill,
        amount_m: 30.0,
        dir: tile.center_direction(&TangentWarp),
        reach_m: 300.0,
        max_delta_m: 60.0,
    };
    let resident = |stale: &[planet_core::TileId]| -> HashMap<planet_core::TileId, ResidentTile> {
        plan.nodes
            .iter()
            .map(|n| {
                (n.tile, ResidentTile { mesh_key: if stale.contains(&n.tile) { n.mesh_key ^ 1 } else { n.mesh_key }, origin: n.origin })
            })
            .collect()
    };
    let bounds = TileBounds::default();
    let flagged = |d: &planet_frame::pipeline::DrawPlan| -> Vec<planet_core::TileId> {
        d.plan.nodes.iter().filter(|n| n.preview).map(|n| n.tile).collect()
    };
    let base = Some((&req, &plan));
    // A stale tile under the stamp is previewed; a stale tile far away and a current tile under the stamp are not.
    let set = resident(&[near, far]);
    let d = draw_plan_preview(&req, base, &set, &bounds, &[stamp(near)]).unwrap();
    assert_eq!(flagged(&d), [near]);
    assert_eq!(d.preview.len(), 1);
    assert_eq!(d.substituted, 2, "both stale tiles are drawn as they are");
    // The exact tile arrives (the plan's key): nothing is previewed.
    let d = draw_plan_preview(&req, base, &resident(&[far]), &bounds, &[stamp(near)]).unwrap();
    assert!(flagged(&d).is_empty());
    // An empty list is the plain draw plan.
    let plain = draw_plan_with(&req, base, &set, &bounds).unwrap();
    let none = draw_plan_preview(&req, base, &set, &bounds, &[]).unwrap();
    assert_eq!(plain, none);
    assert!(flagged(&none).is_empty() && none.preview.is_empty());
}
