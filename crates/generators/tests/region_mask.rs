//! GEN-029: the region mask. A region of mode 2 cuts every delta of its layer inside it, whatever the order of the items; a region of mode 0
//! and a region of another layer change nothing; the bounds still enclose the masked heights; the region re-keys the tiles it touches.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::height::{HeightField, Reference};
use planet_generators::stamp::{ItemReader, StampStage};
use planet_world_def::footprint::angle_between;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;
const LAYER: &str = "stamps";

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

fn at_distance(anchor: Vec3, hint: Vec3, d_m: f64) -> Vec3 {
    let t = (hint - anchor * anchor.dot(hint)).normalized();
    let a = d_m / R;
    (anchor * libm::cos(a) + t * libm::sin(a)).normalized()
}

fn hill(id: u128, layer: &str, at: Vec3, reach_m: f64, amount: f64) -> ItemSpec {
    ItemSpec { id, layer: layer.into(), priority: 0, dir: at, content_hash: 0, reach_m, max_delta_m: amount.abs(), stamp: None }
        .with_stamp(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]))
}

fn ridge(id: u128, layer: &str, amount: f64, width: f64, points: &[(f64, f64)]) -> ItemSpec {
    let dirs: Vec<Vec3> = points.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let far = dirs.iter().map(|d| angle_between(dirs[0], *d) * R).fold(0.0f64, f64::max);
    ItemSpec { id, layer: layer.into(), priority: 0, dir: dirs[0], content_hash: 0, reach_m: far + width, max_delta_m: amount, stamp: None }
        .with_stamp(Stamp::new(StampKind::Ridge, [amount, width, 0.0]).with_points(&dirs))
}

/// A region around (20.0, 30.0), about 4 km a side, clipping `layer` with `mode` (the item's layer is the clipped one for mode 2).
fn region(id: u128, layer: &str, mode: f64, corners: &[(f64, f64)]) -> ItemSpec {
    let dirs: Vec<Vec3> = corners.iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let far = dirs.iter().map(|d| angle_between(dirs[0], *d) * R).fold(0.0f64, f64::max);
    ItemSpec { id, layer: layer.into(), priority: 0, dir: dirs[0], content_hash: 0, reach_m: far + 10.0, max_delta_m: 0.0, stamp: None }
        .with_stamp(Stamp::new(StampKind::Region, [2.0, mode, 0.0]).with_points(&dirs))
}

fn square() -> Vec<(f64, f64)> {
    vec![(19.98, 29.98), (19.98, 30.02), (20.02, 30.02), (20.02, 29.98)]
}

fn world(items: &[ItemSpec]) -> Arc<WorldDefinition> {
    let mut d = WorldDefinition::single_planet(R, 1);
    for i in items {
        d.put_item_spec(BodyId::PLANET, i.clone()).unwrap();
    }
    Arc::new(d)
}

fn stage(def: &Arc<WorldDefinition>, layer: &str) -> StampStage<'static> {
    StampStage::new(Arc::new(Reference(1)), Arc::clone(def) as Arc<dyn ItemReader>, BodyId::PLANET, layer, 0, 3).unwrap()
}

// spec: GEN-029
#[test]
fn inside_a_region_that_clips_the_height_is_the_bases_and_outside_it_carries_the_deltas_whatever_the_order() {
    // A hill and a ridge both reach across the region's edge.
    let items = [
        hill(1, LAYER, unit(20.0, 30.0), 6_000.0, 50.0),
        ridge(2, LAYER, 40.0, 3_000.0, &[(19.99, 29.95), (20.01, 30.06)]),
        region(3, LAYER, 2.0, &square()),
    ];
    let straight = world(&items);
    let flipped = world(&[items[2].clone(), items[1].clone(), items[0].clone()]);
    let with = stage(&straight, LAYER);
    let base = Reference(1);
    // A grid over and around the region: inside = base bit for bit, outside near the stamps = changed.
    let (mut inside, mut outside_changed) = (0, 0);
    for i in -12..=12 {
        for j in -12..=12 {
            let p = unit(20.0 + 0.005 * f64::from(i), 30.0 + 0.005 * f64::from(j));
            let h = with.height(p);
            let in_square =
                (19.98..=20.02).contains(&(20.0 + 0.005 * f64::from(i))) && (29.98..=30.02).contains(&(30.0 + 0.005 * f64::from(j)));
            let clearly_inside =
                (19.981..20.019).contains(&(20.0 + 0.005 * f64::from(i))) && (29.981..30.019).contains(&(30.0 + 0.005 * f64::from(j)));
            if clearly_inside {
                assert_eq!(h.to_bits(), base.height(p).to_bits(), "inside the region at ({i}, {j})");
                inside += 1;
            } else if !in_square && h != base.height(p) {
                outside_changed += 1;
            }
            assert_eq!(h.to_bits(), stage(&flipped, LAYER).height(p).to_bits(), "the order of the items does not matter at ({i}, {j})");
        }
    }
    assert!(inside >= 49 && outside_changed > 20, "inside {inside}, changed outside {outside_changed}");
}

// spec: GEN-029
#[test]
fn a_region_of_mode_zero_and_a_region_of_another_layer_change_nothing() {
    let hills = [hill(1, LAYER, unit(20.0, 30.0), 6_000.0, 50.0)];
    let plain = world(&hills);
    let label = world(&[hills[0].clone(), region(2, "regions", 0.0, &square())]);
    let other = world(&[hills[0].clone(), region(3, "other", 2.0, &square())]);
    let (a, b, c) = (stage(&plain, LAYER), stage(&label, LAYER), stage(&other, LAYER));
    for k in 0..80 {
        let p = at_distance(unit(20.0, 30.0), unit(20.5, 30.3), f64::from(k) * 90.0 - 3_000.0);
        assert_eq!(a.height(p).to_bits(), b.height(p).to_bits(), "a label at {k}");
        assert_eq!(a.height(p).to_bits(), c.height(p).to_bits(), "a region of another layer at {k}");
    }
    // The clipped layer itself: the other layer's stage cuts its own items inside the region, the stamps layer's stage does not.
    let in_other = world(&[hill(4, "other", unit(20.0, 30.0), 6_000.0, 50.0), region(5, "other", 2.0, &square())]);
    assert_eq!(stage(&in_other, "other").height(unit(20.0, 30.0)).to_bits(), Reference(1).height(unit(20.0, 30.0)).to_bits());
    assert_eq!(
        stage(&in_other, LAYER).height(unit(20.0, 30.0)).to_bits(),
        Reference(1).height(unit(20.0, 30.0)).to_bits(),
        "the stamps layer reads nothing of it"
    );
}

// spec: GEN-029
#[test]
fn the_bounds_over_a_cap_enclose_the_masked_heights() {
    let items = [
        hill(1, LAYER, unit(20.0, 30.0), 6_000.0, 50.0),
        ridge(2, LAYER, 40.0, 3_000.0, &[(19.99, 29.95), (20.01, 30.06)]),
        region(3, LAYER, 2.0, &square()),
    ];
    let def = world(&items);
    let s = stage(&def, LAYER);
    let mut seed = 777u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for k in 0..60 {
        let centre = at_distance(unit(20.0, 30.0), Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized(), rnd() * 5_000.0);
        let angle = (20.0 + rnd() * 3_000.0) / R;
        let (lo, hi) = s.height_bounds(centre, angle);
        for n in 0..60 {
            let p = at_distance(centre, Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5).normalized(), rnd() * angle * R);
            let h = s.height(p);
            assert!(h >= lo && h <= hi, "cap {k} sample {n}: {h} outside [{lo}, {hi}]");
        }
    }
}

// spec: GEN-029, WDEF-012
#[test]
fn a_region_rekeys_the_tiles_of_its_layer_it_touches_and_no_others() {
    let key = |def: &Arc<WorldDefinition>, layer: &str, tile: TileId| stage(def, layer).inputs_hash_for(tile, &TangentWarp);
    let under = TileId::from_direction(&TangentWarp, unit(20.0, 30.0), 13);
    let far = TileId::from_direction(&TangentWarp, unit(-20.0, 210.0), 13);
    let base_items = [hill(1, LAYER, unit(20.0, 30.0), 6_000.0, 50.0)];
    let without = world(&base_items);
    let with = world(&[base_items[0].clone(), region(2, LAYER, 2.0, &square())]);
    let mut moved = square();
    moved[2].1 = 30.03;
    let moved = world(&[base_items[0].clone(), region(2, LAYER, 2.0, &moved)]);
    assert_ne!(key(&without, LAYER, under), key(&with, LAYER, under), "adding the region re-keys the tile under it");
    assert_ne!(key(&with, LAYER, under), key(&moved, LAYER, under), "moving a point of it does");
    assert_eq!(key(&without, LAYER, far), key(&with, LAYER, far), "a tile away from it keeps its key");
    assert_eq!(key(&without, "other", under), key(&with, "other", under), "so does the same tile in another layer");
}

/// The footprint items around `p` of the world, as the stage reads them.
fn items_at(def: &Arc<WorldDefinition>, layer: &str, p: Vec3) -> Vec<planet_world_def::footprint::FootprintItem> {
    def.footprint_items_cap(BodyId::PLANET, layer, planet_world_def::footprint::Cap { centre: p, angle: 1e-6 }).unwrap()
}

// spec: GEN-029
#[test]
fn a_region_that_does_not_cut_leaves_the_bits_of_the_height_and_the_edge_of_its_reach_is_cut() {
    use planet_generators::stamp::apply_stamps;
    // A point inside the reach of the region but outside its polygon, a base of -0.0: nothing may touch the sign, for a clip and a label.
    for (layer, mode) in [(LAYER, 2.0), (LAYER, 0.0)] {
        let def = world(&[region(1, layer, mode, &square())]);
        let p = at_distance(unit(20.0, 30.0), unit(20.5, 30.0), 2_450.0); // north of the square's 2.2 km half side, inside the reach
        let items = items_at(&def, layer, p);
        assert_eq!(items.len(), 1, "the region is read at the point");
        assert_eq!(apply_stamps(&items, R, p, -0.0).to_bits(), (-0.0f64).to_bits(), "mode {mode}: the sign of the base is kept");
        assert_eq!(apply_stamps(&items, R, p, 12.5), 12.5);
    }
    // A region whose reach is exactly the distance to its farthest vertex cuts at that vertex (the reach is inclusive).
    let dirs: Vec<Vec3> = square().iter().map(|(la, lo)| unit(*la, *lo)).collect();
    let far = dirs.iter().map(|d| angle_between(dirs[0], *d) * R).fold(0.0f64, f64::max);
    let vertex = dirs.iter().copied().max_by(|a, b| angle_between(dirs[0], *a).total_cmp(&angle_between(dirs[0], *b))).unwrap();
    let exact =
        ItemSpec { id: 1, layer: LAYER.into(), priority: 0, dir: dirs[0], content_hash: 0, reach_m: far, max_delta_m: 0.0, stamp: None }
            .with_stamp(Stamp::new(StampKind::Region, [2.0, 2.0, 0.0]).with_points(&dirs));
    let def = world(&[exact, hill(2, LAYER, vertex, 3_000.0, 40.0)]);
    let items = items_at(&def, LAYER, vertex);
    assert_eq!(items.len(), 2);
    assert_eq!(apply_stamps(&items, R, vertex, 7.0), 7.0, "the farthest vertex is inside the region and its hill is cut");
}
