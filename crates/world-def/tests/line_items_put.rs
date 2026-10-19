//! Tier A: the put refusals of a line the item file cannot make (WDEF-011), a region item (AUTH-005) and a point at a pole.

use planet_core::Vec3;
use planet_world_def::item::{Anchor, Item};
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemError, ItemSpec, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

fn line_spec(points: &[Vec3]) -> ItemSpec {
    ItemSpec {
        id: 5,
        layer: "splines".into(),
        priority: 0,
        dir: points[0],
        content_hash: 0,
        reach_m: 5_000.0,
        max_delta_m: 100.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Ridge, [40.0, 200.0, 0.0]).with_points(points))
}

fn path() -> Vec<Vec3> {
    vec![unit(20.0, 30.0), unit(20.01, 30.0), unit(20.01, 30.01)]
}

fn refused(spec: ItemSpec, want: &str) {
    let mut def = WorldDefinition::single_planet(R, 7);
    let before = def.clone();
    let e = def.put_item_spec(BodyId::PLANET, spec).unwrap_err();
    assert!(matches!(e, ItemError::Stamp { id: 5, .. }), "{e:?}");
    assert!(e.to_string().contains("item 5") && e.to_string().contains(want), "{e}");
    assert_eq!(def, before);
}

// spec: WDEF-011
#[test]
fn the_put_refuses_a_line_with_the_wrong_number_of_points_a_repeated_point_and_points_that_are_not_directions_or_start_elsewhere() {
    let mut def = WorldDefinition::single_planet(R, 7);
    def.put_item_spec(BodyId::PLANET, line_spec(&path())).unwrap();
    refused(line_spec(&path()[..1]), "2 to 64 points");
    let many: Vec<Vec3> = (0..65).map(|i| unit(20.0 + 0.00005 * f64::from(i), 30.0)).collect();
    refused(line_spec(&many), "2 to 64 points");
    let mut repeated = path();
    repeated[1] = repeated[0];
    refused(line_spec(&repeated), "coincide");
    let mut off_unit = path();
    off_unit[2] = Vec3::new(off_unit[2].x * 1.01, off_unit[2].y, off_unit[2].z);
    refused(line_spec(&off_unit), "unit directions");
    let mut nan = path();
    nan[1] = Vec3::new(f64::NAN, 0.0, 0.0);
    refused(line_spec(&nan), "unit directions");
    // The first point is not the anchor.
    let mut elsewhere = line_spec(&path());
    elsewhere.dir = unit(20.5, 30.0);
    elsewhere = elsewhere.with_stamp(Stamp::new(StampKind::Ridge, [40.0, 200.0, 0.0]).with_points(&path()));
    refused(elsewhere, "must be the item's anchor");
    // A hill takes no points.
    let hill = ItemSpec {
        id: 5,
        layer: "stamps".into(),
        priority: 0,
        dir: path()[0],
        content_hash: 0,
        reach_m: 500.0,
        max_delta_m: 100.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Hill, [10.0, 0.0, 0.0]).with_points(&path()));
    refused(hill, "takes no points");
    // A line with a reserved slot set.
    let slot = ItemSpec {
        id: 5,
        layer: "splines".into(),
        priority: 0,
        dir: path()[0],
        content_hash: 0,
        reach_m: 5_000.0,
        max_delta_m: 100.0,
        stamp: None,
    }
    .with_stamp(Stamp::new(StampKind::Ridge, [40.0, 200.0, 1.0]).with_points(&path()));
    refused(slot, "reserved");
}

fn region(points: Vec<(f64, f64)>, mask_layer: Option<&str>) -> Item {
    Item {
        id: 9,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "regions".into(),
        priority: 0,
        kind: "region".into(),
        anchor: Anchor { latitude_deg: points[0].0, longitude_deg: points[0].1, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 5_000.0,
        max_delta_m: 0.0,
        params: BTreeMap::from([("surface_type".to_string(), 2.0), ("mask_mode".to_string(), 2.0)]),
        base_hash: 1,
        generator_version: 1,
        stage_version: 1,
        name: None,
        notes: None,
        points,
        group: None,
        part: None,
        mask_layer: mask_layer.map(str::to_string),
    }
}

// spec: AUTH-005
#[test]
fn a_region_needs_three_points_takes_a_mask_layer_and_hashes_it() {
    let tri = vec![(20.0, 30.0), (20.01, 30.0), (20.01, 30.01)];
    let ok = region(tri.clone(), Some("stamps"));
    let text = ok.to_toml().unwrap();
    assert!(text.starts_with("schema_version = 2\n") && text.contains("mask_layer = \"stamps\"\n"), "{text}");
    assert_eq!(Item::from_toml(&text).unwrap(), ok);
    // Two points are a line, not a region.
    let two = region(tri[..2].to_vec(), Some("stamps"));
    assert!(two.validate().unwrap_err().key.contains("points"));
    // A bad layer name.
    assert!(region(tri.clone(), Some("bad name")).validate().unwrap_err().key.contains("mask_layer"));
    // The mask layer is in the hash; the name is not.
    assert_ne!(region(tri.clone(), Some("splines")).hash(), ok.hash());
    assert_ne!(region(tri.clone(), None).hash(), ok.hash());
    // A region is no stamp: it keeps the file hash, which covers its points.
    assert_eq!(ok.spec().stamp, None);
    assert_eq!(ok.spec().content_hash, ok.hash());
    let mut moved = tri;
    moved[2].1 = 30.02;
    assert_ne!(region(moved, Some("stamps")).spec().content_hash, ok.spec().content_hash);
}

// spec: AUTH-005
#[test]
fn a_point_at_a_pole_has_longitude_zero_and_a_line_may_start_there() {
    let line = |lon_at_pole: f64| {
        let mut it = region(vec![(90.0, 0.0), (89.99, 10.0), (89.99, 20.0)], None);
        it.kind = "ridge".into();
        it.layer = "splines".into();
        it.max_delta_m = 100.0;
        it.params = BTreeMap::from([("amount_m".to_string(), 10.0), ("width_m".to_string(), 50.0)]);
        it.points[0].1 = lon_at_pole;
        it.anchor.latitude_deg = 90.0;
        it.anchor.longitude_deg = lon_at_pole;
        it
    };
    let ok = line(0.0);
    ok.validate().unwrap();
    assert_eq!(Item::from_toml(&ok.to_toml().unwrap()).unwrap(), ok);
    let e = line(5.0).validate().unwrap_err();
    assert!(e.key.contains("longitude"), "{e}");
    let mut second = line(0.0);
    second.points[1] = (90.0, 5.0);
    assert!(second.validate().unwrap_err().key.contains("points[1].longitude"));
}
