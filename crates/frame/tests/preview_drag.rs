//! REND-033, the drag: the delta of the preview runs from the oldest state to the current one, so it is right only for a tile still at the oldest
//! state. A tile refreshed at an intermediate state of a drag, one loaded since the edit began, one a stamp left out at the cap touches and one
//! refreshed ahead of the plan are drawn as they are, never with a delta that is not theirs.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_frame::pipeline::{draw_plan_states, DrawPlan, ResidentTile, TileStates};
use planet_frame::preview::{preview_delta, PreviewStamp};
use planet_frame::{
    plan_terrain_field, scripted_views_field, LodParams, TerrainDraw, TerrainPlan, TerrainRequest, TerrainView, TileBounds,
};
use planet_generators::height::Reference;
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};
use std::collections::{BTreeSet, HashMap};

const R: f64 = 6_371_000.0;
const REACH: f64 = 400.0;
const AMOUNT: f64 = 60.0;

fn east(m: f64) -> Vec3 {
    let a = m / R;
    Vec3::new(libm::cos(a), libm::sin(a), 0.0)
}

fn arc(a: Vec3, b: Vec3) -> f64 {
    R * libm::atan2(a.cross(b).length(), a.dot(b))
}

fn hill_item(id: u128, m: f64, reach: f64, amount: f64) -> ItemSpec {
    ItemSpec {
        id,
        layer: DEFAULT_ITEM_LAYER.into(),
        priority: 0,
        dir: east(m),
        content_hash: 0,
        reach_m: reach,
        max_delta_m: 100.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
}

fn footprint(specs: &[ItemSpec]) -> Vec<FootprintItem> {
    let mut d = WorldDefinition::single_planet(R, 1);
    for s in specs {
        d.put_item_spec(BodyId::PLANET, s.clone()).unwrap();
    }
    d.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre: east(0.0), angle: core::f64::consts::PI }).unwrap()
}

/// The height of a world with the hill at `m` metres east, at `p` (the hill profile of GEN-026).
fn hill_at(m: f64, p: Vec3) -> f64 {
    let x = arc(east(m), p) / REACH;
    if x >= 1.0 {
        0.0
    } else {
        AMOUNT * (1.0 - x * x) * (1.0 - x * x)
    }
}

fn delta_at(stamps: &[PreviewStamp], p: Vec3) -> f64 {
    stamps
        .iter()
        .map(|s| {
            let x = arc(s.dir, p) / s.reach_m;
            if x >= 1.0 {
                0.0
            } else {
                f64::from(s.sign) * s.amount_m * (1.0 - x * x) * (1.0 - x * x)
            }
        })
        .sum()
}

struct Scene {
    req: TerrainRequest,
    plan: TerrainPlan,
    near: TileId,
    far: TileId,
}

fn scene() -> Scene {
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
    let by_height = |n: &&TerrainDraw| n.tile.center_direction(&TangentWarp).dot(camera.position.0.normalized());
    let mut v: Vec<&TerrainDraw> = plan.nodes.iter().collect();
    v.sort_by(|a, b| by_height(b).total_cmp(&by_height(a)));
    let (near, far) = (v[0].tile, v[v.len() - 1].tile);
    Scene { req, plan, near, far }
}

fn stamp_at(tile: TileId) -> PreviewStamp {
    PreviewStamp {
        sign: 1,
        kind: StampKind::Hill,
        amount_m: 30.0,
        dir: tile.center_direction(&TangentWarp),
        reach_m: 300.0,
        max_delta_m: 60.0,
    }
}

impl Scene {
    /// The tiles the frame flags, with the tiles at `keys` (tile, key) resident and every other tile of the plan exact.
    fn flagged(
        &self,
        keys: &[(TileId, u64)],
        oldest: &[(TileId, u64)],
        left_out: &[PreviewStamp],
        edited: &[TileId],
    ) -> (Vec<TileId>, DrawPlan) {
        let resident: HashMap<TileId, ResidentTile> = self
            .plan
            .nodes
            .iter()
            .map(|n| (n.tile, ResidentTile { mesh_key: keys.iter().find(|k| k.0 == n.tile).map_or(n.mesh_key, |k| k.1), origin: n.origin }))
            .collect();
        let oldest: HashMap<TileId, u64> = oldest.iter().copied().collect();
        let edited: BTreeSet<TileId> = edited.iter().copied().collect();
        let states = TileStates { oldest: &oldest, left_out };
        let d = draw_plan_states(
            &self.req,
            Some((&self.req, &self.plan)),
            &resident,
            &TileBounds::default(),
            &[stamp_at(self.near)],
            &edited,
            &[],
            Some(states),
        )
        .unwrap();
        (d.plan.nodes.iter().filter(|n| n.preview).map(|n| n.tile).collect(), d)
    }

    fn key(&self, tile: TileId) -> u64 {
        self.plan.nodes.iter().find(|n| n.tile == tile).unwrap().mesh_key
    }
}

// spec: REND-033
#[test]
fn a_tile_refreshed_at_an_intermediate_state_of_a_drag_is_drawn_as_it_is_and_one_at_the_oldest_state_gets_the_delta() {
    let s = scene();
    let (oldest_key, middle_key) = (s.key(s.near) ^ 1, s.key(s.near) ^ 2);
    let oldest = [(s.near, oldest_key), (s.far, s.key(s.far) ^ 1)];
    // At the oldest state: previewed.
    let (flagged, d) = s.flagged(&[(s.near, oldest_key)], &oldest, &[], &[]);
    assert_eq!(flagged, [s.near]);
    assert_eq!(d.substituted, 1);
    // Refreshed at the middle state of the drag (its key is neither the oldest nor the plan's): drawn as it is, stale, not previewed.
    let (flagged, d) = s.flagged(&[(s.near, middle_key)], &oldest, &[], &[]);
    assert!(flagged.is_empty(), "a tile at an intermediate state is not given the delta of the oldest state: {flagged:?}");
    assert_eq!(d.substituted, 1, "it is still stale: the frame is not exact");
    let node = d.plan.nodes.iter().find(|n| n.tile == s.near).unwrap();
    assert_eq!(node.state, planet_frame::TileState::Stale);
    // A tile loaded since the edit began (no entry in the oldest states) has an unknown state: not previewed.
    let (flagged, _) = s.flagged(&[(s.near, oldest_key)], &[], &[], &[]);
    assert!(flagged.is_empty());
    // Named by the edit and refreshed ahead of the plan (the single-edit window): it holds the edit already, so no delta.
    let (flagged, _) = s.flagged(&[(s.near, middle_key)], &oldest, &[], &[s.near]);
    assert!(flagged.is_empty());
    // The exact tile (the plan's key) ends it.
    let (flagged, d) = s.flagged(&[], &oldest, &[], &[]);
    assert!(flagged.is_empty() && d.substituted == 0);
}

// spec: REND-033
#[test]
fn a_tile_a_stamp_left_out_at_the_cap_touches_is_drawn_without_the_preview() {
    let s = scene();
    let oldest = [(s.near, s.key(s.near) ^ 1)];
    let (flagged, _) = s.flagged(&[(s.near, s.key(s.near) ^ 1)], &oldest, &[], &[]);
    assert_eq!(flagged, [s.near], "without a left-out stamp it is previewed");
    // A stamp the delta had to leave out reaches the tile: its delta would be incomplete, so none is applied.
    let (flagged, _) = s.flagged(&[(s.near, s.key(s.near) ^ 1)], &oldest, &[stamp_at(s.near)], &[]);
    assert!(flagged.is_empty());
    // One that lies away from the tile changes nothing.
    let (flagged, _) = s.flagged(&[(s.near, s.key(s.near) ^ 1)], &oldest, &[stamp_at(s.far)], &[]);
    assert_eq!(flagged, [s.near]);
}

// spec: REND-033
#[test]
fn the_cap_reports_the_stamps_it_leaves_out() {
    let specs: Vec<ItemSpec> = (1..=40u128).map(|id| hill_item(id, 100.0 * id as f64, 50.0, 5.0)).collect();
    let list = preview_delta(&[], &footprint(&specs), east(0.0));
    assert_eq!(list.stamps.len() + list.left_out.len(), 40);
    assert_eq!(list.left_out.len(), list.dropped);
    assert!(list.dropped > 0 && list.stamps.iter().all(|k| !list.left_out.contains(k)));
}

// spec: REND-033
#[test]
fn every_drawn_stale_tile_shows_its_own_state_or_the_current_one_never_a_mixture() {
    // A hill dragged in three steps (0, 300, 500 m east); the delta runs from the oldest state (hill at 0) to the current one (hill at 500).
    let s = scene();
    let delta =
        preview_delta(&footprint(&[hill_item(1, 0.0, REACH, AMOUNT)]), &footprint(&[hill_item(1, 500.0, REACH, AMOUNT)]), east(0.0)).stamps;
    for (what, tile_state_m, key_is_oldest) in [("oldest", 0.0, true), ("middle", 300.0, false)] {
        let key = s.key(s.near) ^ if key_is_oldest { 1 } else { 2 };
        let oldest = [(s.near, s.key(s.near) ^ 1)];
        let (flagged, _) = s.flagged(&[(s.near, key)], &oldest, &[], &[]);
        let previewed = flagged.contains(&s.near);
        for m in [0.0, 300.0, 500.0] {
            let p = east(m);
            let exact = hill_at(500.0, p);
            let own = hill_at(tile_state_m, p);
            let shown = own + if previewed { delta_at(&delta, p) } else { 0.0 };
            let is_current = (shown - exact).abs() < 1e-6;
            let is_own = (shown - own).abs() < 1e-6;
            assert!(is_current || is_own, "{what} tile at {m} m shows {shown:.2} m: current {exact:.2}, own {own:.2}");
            if previewed {
                assert!(is_current, "{what}: a previewed tile shows the current state");
            }
        }
    }
}
