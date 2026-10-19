//! Stamp items (WDEF-010): the kind and parameters of an item, the refusals, the derived content hash, and the cap form of the footprint
//! query a stage reads its items with (GEN-026).

use planet_core::{TangentWarp, TileId, Vec3};
use planet_world_def::footprint::{angle_between, touches, Cap};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::summary::cell_hash128;
use planet_world_def::{footprint_hash, BodyId, GenContext, ItemError, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

fn stamp_spec(id: u128, at: Vec3, reach_m: f64, max_delta_m: f64, stamp: Stamp) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir: at, content_hash: 0, reach_m, max_delta_m, stamp: None }
        .with_stamp(stamp)
}

// spec: WDEF-010
#[test]
fn a_stamp_that_cannot_be_one_is_refused_naming_the_item_and_the_reason_and_changes_nothing() {
    let mut d = WorldDefinition::single_planet(R, 7);
    let at = dir(1.0, 0.2, 0.1);
    d.put_item_spec(BodyId::PLANET, stamp_spec(1, at, 500.0, 50.0, Stamp::new(StampKind::Hill, [30.0, 0.0, 0.0]))).unwrap();
    let before = d.clone();
    let refuse = |d: &mut WorldDefinition, spec: ItemSpec, want: &str| {
        let err = d.put_item_spec(BodyId::PLANET, spec).unwrap_err();
        assert!(matches!(err, ItemError::Stamp { id: 1, .. }), "{err:?}");
        assert!(err.to_string().contains("item 1") && err.to_string().contains(want), "{err}");
    };
    let hill = |amount: f64| Stamp::new(StampKind::Hill, [amount, 0.0, 0.0]);
    refuse(&mut d, stamp_spec(1, at, 0.0, 50.0, hill(10.0)), "reach above 0");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, hill(f64::NAN)), "not finite");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, hill(f64::INFINITY)), "not finite");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, hill(50.5)), "above its bound");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, hill(-50.5)), "above its bound");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, Stamp::new(StampKind::Crater, [-1.0, 0.0, 0.0])), "between 0 and");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, Stamp::new(StampKind::Crater, [60.0, 0.0, 0.0])), "between 0 and");
    refuse(&mut d, stamp_spec(1, at, 500.0, 50.0, Stamp::new(StampKind::Plateau, [10.0, 1.0, 0.0])), "reserved");
    // A given content hash that is not the stamp's.
    let mut forged = stamp_spec(1, at, 500.0, 50.0, hill(10.0));
    forged.content_hash += 1;
    refuse(&mut d, forged, "content hash");
    assert_eq!(d, before, "refused stamps change nothing, the stamp of the same id stays");
    // The accepted ones: a negative hill (a pit), a deep crater at the bound, a plateau at any finite height (its change is clamped).
    d.put_item_spec(BodyId::PLANET, stamp_spec(2, at, 500.0, 50.0, hill(-50.0))).unwrap();
    d.put_item_spec(BodyId::PLANET, stamp_spec(3, at, 500.0, 50.0, Stamp::new(StampKind::Crater, [50.0, 0.0, 0.0]))).unwrap();
    d.put_item_spec(BodyId::PLANET, stamp_spec(4, at, 500.0, 50.0, Stamp::new(StampKind::Plateau, [-1234.5, 0.0, 0.0]))).unwrap();
}

// spec: WDEF-010
#[test]
fn the_content_hash_of_a_stamp_is_derived_from_its_kind_and_parameters_and_reaches_every_hash() {
    let at = dir(1.0, 0.2, 0.1);
    let tile = TileId::from_direction(&TangentWarp, at, 12);
    let make = |stamp: Stamp| {
        let mut d = WorldDefinition::single_planet(R, 7);
        d.put_item_spec(BodyId::PLANET, stamp_spec(1, at, 500.0, 50.0, stamp)).unwrap();
        let ctx = GenContext::new(&d);
        let items = ctx.query_footprint(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 0.25).unwrap();
        (
            d.hash(),
            ctx.read_set_hash(),
            footprint_hash(&items),
            cell_hash128(&items),
            items,
            d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER),
        )
    };
    let base = make(Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0]));
    assert_eq!(base.4[0].stamp, Some(Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0])), "the query returns the stamp");
    for other in [
        Stamp::new(StampKind::Hill, [10.5, 0.0, 0.0]),
        Stamp::new(StampKind::Crater, [10.0, 0.0, 0.0]),
        Stamp::new(StampKind::Plateau, [10.0, 0.0, 0.0]),
    ] {
        let o = make(other.clone());
        assert_ne!(base.0, o.0, "the definition hash follows kind and parameters: {other:?}");
        assert_ne!(base.1, o.1, "the read set follows them");
        assert_ne!(base.2, o.2, "the 64-bit footprint hash follows them");
        assert_ne!(base.3, o.3, "the summary leaf follows them");
        assert_ne!(base.5, o.5, "the summary tree follows them");
    }
    // Negative zero is zero: the same stamp, the same hashes.
    let zero = make(Stamp::new(StampKind::Hill, [0.0, 0.0, 0.0]));
    let neg_zero = make(Stamp::new(StampKind::Hill, [-0.0, 0.0, 0.0]));
    assert_eq!((zero.0, zero.1, zero.2, zero.3), (neg_zero.0, neg_zero.1, neg_zero.2, neg_zero.3));
    assert_eq!(Stamp::new(StampKind::Hill, [-0.0, -0.0, -0.0]), Stamp::new(StampKind::Hill, [0.0; 3]));
}

// spec: WDEF-010
#[test]
fn the_content_hash_of_a_stamp_is_pinned() {
    // BLAKE3-128 under `planet-builder stamp v1` over the kind byte and the parameter bits (little endian).
    let h = Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0]).content_hash();
    assert_eq!(format!("{h:032x}"), KNOWN_HILL_HASH);
    assert_ne!(h, Stamp::new(StampKind::Crater, [10.0, 0.0, 0.0]).content_hash());
}

const KNOWN_HILL_HASH: &str = "c3fa08a26a7a8e3889d14e73793053b9";

// spec: WDEF-010, WDEF-007
#[test]
fn the_cap_form_of_the_footprint_query_returns_the_items_that_touch_the_cap_as_a_scan_of_every_item_does() {
    let mut d = WorldDefinition::single_planet(R, 7);
    // Items scattered around a few places, with reaches up to the limit.
    let mut s = 12345u64;
    let mut next = || {
        s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as f64 / u64::MAX as f64
    };
    let centres = [dir(1.0, 0.2, 0.1), dir(1.0, 1.0, 1.0), dir(-0.3, 1.0, 0.4), dir(0.0, 0.0, 1.0)];
    let mut id = 0u128;
    for c in centres {
        for _ in 0..60 {
            id += 1;
            let off = Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5) * (300_000.0 / R);
            let at = (c + off - c * c.dot(off)).normalized();
            let reach = 100.0 + next() * 39_000.0;
            d.put_item_spec(
                BodyId::PLANET,
                ItemSpec {
                    id,
                    layer: DEFAULT_ITEM_LAYER.into(),
                    priority: (id % 3) as i32,
                    dir: at,
                    content_hash: id,
                    reach_m: reach,
                    max_delta_m: 10.0,
                    stamp: None,
                },
            )
            .unwrap();
        }
    }
    let mut positives = 0;
    for c in centres {
        for angle_m in [0.0, 50.0, 2_000.0, 25_000.0, 150_000.0, 900_000.0, 5_000_000.0] {
            let cap = Cap { centre: c, angle: angle_m / R };
            let got: Vec<u128> = d.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, cap).unwrap().iter().map(|i| i.id).collect();
            let mut want: Vec<(i32, u128)> = (1..=id)
                .filter(|i| {
                    let (anchor, reach) = d.item_anchor(BodyId::PLANET, *i).unwrap();
                    touches(cap, anchor, reach, R)
                })
                .map(|i| ((i % 3) as i32, i))
                .collect();
            want.sort_unstable();
            let want: Vec<u128> = want.into_iter().map(|(_, i)| i).collect();
            positives += want.len();
            assert_eq!(got, want, "cap {angle_m} m at {c:?}: application order (priority, id), same set as the scan");
        }
    }
    assert!(positives > 200, "the setup must return items, {positives}");
    // A point is a cap of angle 0: the items whose reach contains it.
    let p = centres[0];
    let at_point = d.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre: p, angle: 0.0 }).unwrap();
    for i in &at_point {
        let (anchor, reach) = d.item_anchor(BodyId::PLANET, i.id).unwrap();
        assert!(R * angle_between(anchor, p) <= reach * (1.0 + 1e-5) + 1e-3);
    }
    assert!(d.footprint_items_cap(BodyId::PLANET, DEFAULT_ITEM_LAYER, Cap { centre: p, angle: f64::NAN }).is_err());
}
