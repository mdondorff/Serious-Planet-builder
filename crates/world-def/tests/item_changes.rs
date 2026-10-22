//! Tier A: where the items that differ between two definitions were and are (WDEF-016, STRM-034).

use planet_core::Vec3;
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

fn spec(id: u128, dir: Vec3, reach_m: f64, priority: i32) -> ItemSpec {
    ItemSpec { id, layer: "stamps".into(), priority, dir, content_hash: id, reach_m, max_delta_m: 1.0, stamp: None }
}

fn sorted(mut v: Vec<(Vec3, f64)>) -> Vec<(u64, u64, u64, u64)> {
    let mut out: Vec<_> = v.drain(..).map(|(d, r)| (d.x.to_bits(), d.y.to_bits(), d.z.to_bits(), r.to_bits())).collect();
    out.sort();
    out
}

// spec: WDEF-016
#[test]
fn the_changes_are_the_places_of_every_item_that_differs_before_and_after_and_none_for_the_rest() {
    let mut old = WorldDefinition::single_planet(R, 3);
    for id in 1..=40u128 {
        old.put_item_spec(BodyId::PLANET, spec(id, unit(f64::from(id as u32), 2.0 * f64::from(id as u32)), 100.0 + id as f64, 0)).unwrap();
    }
    assert!(old.clone().item_changes_from(&old).is_empty(), "the same definition: no change");
    let mut new = old.clone();
    // Added (id 100), removed (id 3), moved (id 5), the reach changed (id 7), the priority changed (id 9), and the same item put again (id 11).
    new.put_item_spec(BodyId::PLANET, spec(100, unit(-30.0, 100.0), 900.0, 0)).unwrap();
    assert!(new.remove_item(BodyId::PLANET, 3));
    new.put_item_spec(BodyId::PLANET, spec(5, unit(80.0, -10.0), 105.0, 0)).unwrap();
    new.put_item_spec(BodyId::PLANET, spec(7, unit(7.0, 14.0), 5_000.0, 0)).unwrap();
    new.put_item_spec(BodyId::PLANET, spec(9, unit(9.0, 18.0), 109.0, 2)).unwrap();
    new.put_item_spec(BodyId::PLANET, spec(11, unit(11.0, 22.0), 111.0, 0)).unwrap();
    // The places are the normalised directions the definition holds, so compare through the definition's own anchors.
    let anchor = |def: &WorldDefinition, id: u128| def.item_anchor(BodyId::PLANET, id);
    let want = [
        anchor(&new, 100).into_iter().collect::<Vec<_>>(),
        anchor(&old, 3).into_iter().collect(),
        anchor(&new, 5).into_iter().chain(anchor(&old, 5)).collect(),
        anchor(&new, 7).into_iter().chain(anchor(&old, 7)).collect(),
        anchor(&new, 9).into_iter().chain(anchor(&old, 9)).collect(),
    ]
    .concat();
    let got = new.item_changes_from(&old);
    assert_eq!(
        sorted(got),
        sorted(want),
        "added, removed, moved, resized and re-prioritised items, once as they were and once as they are"
    );
    // The other way round gives the same places.
    assert_eq!(sorted(old.item_changes_from(&new)), sorted(new.item_changes_from(&old)));
}
