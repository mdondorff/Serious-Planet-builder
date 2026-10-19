//! Tier A: the map from an item file's kind and parameters to a stamp (WDEF-010, AUTH-001): `hill` with `height_m`, `crater` with `depth_m`,
//! `plateau` with `target_m`; the radius is the file's `reach_m`, the bound its `max_delta_m`.

use planet_world_def::item::{Anchor, Item};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;

fn item(kind: &str, params: &[(&str, f64)]) -> Item {
    Item {
        id: 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: 1,
        kind: kind.into(),
        anchor: Anchor { latitude_deg: 48.1, longitude_deg: 11.5, height_m: 500.0, heading_deg: 0.0 },
        reach_m: 900.0,
        max_delta_m: 80.0,
        params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
        base_hash: 7,
        generator_version: 1,
        stage_version: 1,
        name: Some("a name".into()),
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
    }
}

// spec: WDEF-010
#[test]
fn a_stamp_survives_the_file_unchanged_and_its_hash_is_stable() {
    for (kind, key, value) in [
        (StampKind::Hill, "height_m", 30.0),
        (StampKind::Hill, "height_m", -12.5),
        (StampKind::Crater, "depth_m", 24.0),
        (StampKind::Plateau, "target_m", 450.0),
    ] {
        assert_eq!(kind.param_keys(), [key]);
        let stamp = Stamp::new(kind, [value, 0.0, 0.0]);
        // Stamp -> file spelling -> Stamp.
        let (name, params) = stamp.to_file();
        assert_eq!(Stamp::from_file(name, &params), Ok(Some(stamp.clone())), "{name} {value}");
        // The item as a file: text, parsed again, the same item, the same spec, the same stamp and hash.
        let it = item(name, &[(key, value)]);
        let text = it.to_toml().unwrap();
        let back = Item::from_toml(&text).unwrap();
        assert_eq!(back, it);
        let spec = back.spec();
        assert_eq!(spec.stamp, Some(stamp.clone()));
        assert_eq!(spec.content_hash, stamp.content_hash(), "the content hash is the stamp's");
        assert_eq!(spec, it.spec(), "the spec of the parsed file is the spec of the item");
        // The definition takes it.
        let mut def = WorldDefinition::single_planet(R, 1);
        back.put_into(&mut def).unwrap();
    }
}

// spec: WDEF-010
#[test]
fn what_changes_no_output_changes_no_hash_and_what_does_changes_it() {
    let a = item("crater", &[("depth_m", 24.0)]).spec();
    let mut other = item("crater", &[("depth_m", 24.0), ("note_m", 5.0)]);
    other.name = Some("another".into());
    other.base_hash = 99;
    other.generator_version = 9;
    assert_eq!(
        other.spec().content_hash,
        a.content_hash,
        "the name, the recorded versions and a parameter nothing reads are not in the hash"
    );
    assert_ne!(item("crater", &[("depth_m", 25.0)]).spec().content_hash, a.content_hash);
    assert_ne!(item("hill", &[("height_m", 24.0)]).spec().content_hash, a.content_hash, "the kind is in the hash");
}

// spec: WDEF-010
#[test]
fn a_stamp_kind_without_its_parameter_is_refused_and_a_kind_that_is_no_stamp_has_no_height_output() {
    let bad = item("crater", &[("radius_m", 310.0)]);
    let e = bad.validate().unwrap_err();
    assert!(format!("{e}").contains("depth_m"), "the message names the missing parameter: {e}");
    assert!(item("hill", &[]).validate().is_err());
    // A kind the stage does not know is a plain item.
    let tree = item("tree", &[("trunk_m", 0.4)]);
    assert!(tree.validate().is_ok());
    assert_eq!(tree.spec().stamp, None);
    assert_eq!(tree.spec().content_hash, tree.hash(), "a plain item keeps the file hash");
}
