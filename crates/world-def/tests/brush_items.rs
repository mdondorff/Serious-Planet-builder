//! Tier A: brush items (WDEF-013, ADR 0070): the strokes list of the item file (schema 3, AUTH-005), its caps and refusals, the stamp's
//! hash computed independently, and the footprint returning the strokes.

use planet_core::{TangentWarp, TileId, Vec3};
use planet_world_def::footprint::Cap;
use planet_world_def::item::{Anchor, Item, ItemStroke};
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;

fn stroke(kind: BrushKind, radius: f64, strength: f64, target: Option<f64>, points: &[(f64, f64)]) -> ItemStroke {
    ItemStroke { kind, radius_m: radius, strength_m: strength, target_m: target, points: points.to_vec() }
}

/// A brush at (20.0, 30.0) with three strokes: a raise of three points, a smooth of two, a flatten of one point with a target.
fn brush() -> Item {
    Item {
        id: 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "brush".into(),
        priority: 0,
        kind: "brush".into(),
        anchor: Anchor { latitude_deg: 20.0, longitude_deg: 30.0, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 5_000.0,
        max_delta_m: 100.0,
        params: BTreeMap::new(),
        base_hash: 7,
        generator_version: 1,
        stage_version: 1,
        name: Some("a \"brush\"".into()),
        notes: None,
        points: Vec::new(),
        group: Some(0x0aaa_bbbb_cccc_dddd_0000_1111_2222_3333),
        part: Some(2),
        mask_layer: None,
        strokes: vec![
            stroke(BrushKind::Raise, 50.0, 10.0, None, &[(20.0, 30.0), (20.001, 30.001), (20.002, 30.001)]),
            stroke(BrushKind::Smooth, 80.0, 5.0, None, &[(20.0, 30.0), (20.0, 30.002)]),
            stroke(BrushKind::Flatten, 30.0, 4.0, Some(12.5), &[(20.001, 30.0)]),
        ],
    }
}

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
}

// spec: AUTH-005
#[test]
fn a_brush_is_a_schema_3_file_that_round_trips_in_the_fixed_key_order() {
    let item = brush();
    let text = item.to_toml().unwrap();
    assert!(text.starts_with("schema_version = 3\n"), "{text}");
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("`{needle}` in {text}"));
    assert!(at("[anchor]") < at("[params]") && at("[params]") < at("[[strokes]]"));
    let stroke0 = &text[at("[[strokes]]")..];
    let order = ["kind = \"raise\"", "radius_m = 50.0", "strength_m = 10.0", "[[strokes.points]]", "latitude = 20.0", "longitude = 30.0"];
    let mut last = 0;
    for key in order {
        let i = stroke0.find(key).unwrap_or_else(|| panic!("`{key}` in {stroke0}"));
        assert!(i >= last, "`{key}` is out of order in {stroke0}");
        last = i;
    }
    assert!(text.contains("target_m = 12.5\n"), "the flatten's target");
    assert_eq!(text.matches("[[strokes]]").count(), 3);
    assert_eq!(text.matches("[[strokes.points]]").count(), 6);
    let back = Item::from_toml(&text).unwrap();
    assert_eq!(back, item.clone().canonical());
    assert_eq!(back.to_toml().unwrap(), text, "write, read, write gives the same text");
    assert_eq!(back.hash(), item.hash());
    // A hand-written file with another key order, a comment and integer literals parses to the same item.
    let other = text.replace("radius_m = 50.0", "# the radius\nradius_m = 50").replace("strength_m = 10.0\n", "strength_m = 10\n");
    assert_eq!(Item::from_toml(&other).unwrap().hash(), item.hash());
    // Schema 1 and 2 items are written as before.
    let mut plain = brush();
    plain.kind = "hill".into();
    plain.strokes.clear();
    plain.group = None;
    plain.part = None;
    plain.params.insert("height_m".into(), 5.0);
    assert!(plain.to_toml().unwrap().starts_with("schema_version = 1\n"));
}

// spec: AUTH-005
#[test]
fn the_item_hash_covers_the_strokes_as_the_layout_says() {
    let item = brush().canonical();
    let mut bits = Vec::new();
    // The schema-2 tail, then the strokes (the hash material of AUTH-005).
    let text = |m: &mut Vec<u8>, s: &str| {
        m.extend_from_slice(&(s.len() as u32).to_le_bytes());
        m.extend_from_slice(s.as_bytes());
    };
    let b = |v: f64| (if v == 0.0 { 0.0f64 } else { v }).to_bits().to_le_bytes();
    bits.extend_from_slice(&3u32.to_le_bytes());
    bits.extend_from_slice(&item.id.to_be_bytes());
    bits.extend_from_slice(&item.body.0.to_le_bytes());
    text(&mut bits, &item.surface);
    text(&mut bits, &item.layer);
    bits.extend_from_slice(&item.priority.to_le_bytes());
    text(&mut bits, &item.kind);
    for v in
        [item.anchor.latitude_deg, item.anchor.longitude_deg, item.anchor.height_m, item.anchor.heading_deg, item.reach_m, item.max_delta_m]
    {
        bits.extend_from_slice(&b(v));
    }
    bits.extend_from_slice(&0u32.to_le_bytes());
    bits.extend_from_slice(&item.base_hash.to_le_bytes());
    bits.extend_from_slice(&item.generator_version.to_le_bytes());
    bits.extend_from_slice(&item.stage_version.to_le_bytes());
    bits.push(1);
    bits.extend_from_slice(&item.group.unwrap().to_be_bytes());
    bits.push(1);
    bits.extend_from_slice(&item.part.unwrap().to_le_bytes());
    text(&mut bits, "");
    bits.extend_from_slice(&0u32.to_le_bytes());
    bits.extend_from_slice(&(item.strokes.len() as u32).to_le_bytes());
    for s in &item.strokes {
        bits.push(s.kind as u8);
        for v in [s.radius_m, s.strength_m, s.target_m.unwrap_or(0.0)] {
            bits.extend_from_slice(&b(v));
        }
        bits.extend_from_slice(&(s.points.len() as u32).to_le_bytes());
        for (la, lo) in &s.points {
            bits.extend_from_slice(&b(*la));
            bits.extend_from_slice(&b(*lo));
        }
    }
    let full = blake3::derive_key("planet-builder item hash v1", &bits);
    assert_eq!(item.hash(), u128::from_be_bytes(full[..16].try_into().unwrap()));
    // Every field of a stroke, a point and the order of two strokes change the hash; the name does not.
    let h = item.hash();
    let mut seen = vec![h];
    let mut check = |name: &str, f: &dyn Fn(&mut Item)| {
        let mut i = item.clone();
        f(&mut i);
        let hash = i.hash();
        assert!(!seen.contains(&hash), "{name} must change the hash");
        seen.push(hash);
    };
    check("a kind", &|i| i.strokes[0].kind = BrushKind::Lower);
    check("a radius", &|i| i.strokes[0].radius_m = 51.0);
    check("a strength", &|i| i.strokes[0].strength_m = 11.0);
    check("a target", &|i| i.strokes[2].target_m = Some(13.0));
    check("a point", &|i| i.strokes[0].points[1].1 = 30.0011);
    check("the order", &|i| i.strokes.swap(0, 1));
    let mut renamed = item.clone();
    renamed.name = Some("another".into());
    renamed.notes = Some("notes".into());
    assert_eq!(renamed.hash(), h, "the name and the notes are not in the hash");
}

fn refused_file(item: &Item, want: &str) {
    let e = item.to_toml().expect_err("the writer must refuse").to_string();
    assert!(e.contains(want), "`{want}` in `{e}`");
}

// spec: AUTH-005, WDEF-013
#[test]
fn the_writer_and_the_loader_refuse_a_brush_that_breaks_a_cap_or_a_rule() {
    let base = brush();
    let with = |f: &dyn Fn(&mut Item)| {
        let mut i = base.clone();
        f(&mut i);
        i
    };
    refused_file(&with(&|i| i.strokes.clear()), "1 to 64 strokes");
    refused_file(&with(&|i| i.strokes = vec![i.strokes[0].clone(); 65]), "1 to 64 strokes");
    refused_file(
        &with(&|i| {
            i.strokes[0].points =
                vec![(20.0, 30.0); 1].into_iter().chain((0..64).map(|k| (20.0, 30.0 + 0.0001 * f64::from(k + 1)))).collect()
        }),
        "1 to 64 points",
    );
    refused_file(
        &with(&|i| {
            // 64 strokes of 65 points are over the per-stroke cap first; 64 of 64 are 4,096 (allowed), so add one more point elsewhere.
            let long: Vec<(f64, f64)> = (0..64).map(|k| (20.0, 30.0 + 0.0001 * f64::from(k))).collect();
            i.strokes = (0..64).map(|_| stroke(BrushKind::Raise, 50.0, 1.0, None, &long)).collect();
            i.strokes[63].points.push((20.0, 30.01));
            i.max_delta_m = 100.0;
        }),
        "4096 points in all",
    );
    refused_file(&with(&|i| i.strokes[0].radius_m = 0.5), "radius_m");
    refused_file(&with(&|i| i.strokes[0].strength_m = 0.0), "strength_m");
    refused_file(&with(&|i| i.strokes[2].target_m = None), "target_m");
    refused_file(&with(&|i| i.strokes[0].target_m = Some(1.0)), "no target");
    refused_file(&with(&|i| i.strokes[0].radius_m = f64::NAN), "radius_m");
    refused_file(&with(&|i| i.strokes[0].points[1] = i.strokes[0].points[0]), "repeats");
    refused_file(&with(&|i| i.strokes[0].points[0] = (21.0, 30.0)), "anchor");
    refused_file(&with(&|i| i.strokes[0].points[1].0 = 91.0), "latitude");
    refused_file(&with(&|i| i.max_delta_m = 18.0), "sum of the strokes' strengths");
    refused_file(&with(&|i| i.points = vec![(20.0, 30.0), (20.001, 30.0)]), "takes strokes, not points");
    refused_file(
        &with(&|i| {
            i.kind = "hill".into();
            i.params.insert("height_m".into(), 5.0);
        }),
        "takes no strokes",
    );
    refused_file(&with(&|i| i.group = None), "come together");
    // The loader: the version and the keys.
    let text = base.to_toml().unwrap();
    let load = |t: &str| Item::from_toml(t).expect_err("the loader must refuse").to_string();
    assert!(load(&text.replacen("schema_version = 3", "schema_version = 2", 1)).contains("[[strokes]]"));
    assert!(load(&text.replacen("schema_version = 3", "schema_version = 4", 1)).contains("newer"));
    assert!(load(&text.replacen("radius_m = 50.0", "radius_m = 50.0\nsparkles = 1.0", 1)).contains("strokes[0].sparkles"));
    assert!(load(&text.replacen("kind = \"raise\"", "kind = \"melt\"", 1)).contains("strokes[0].kind"));
    assert!(load(&text.replacen("strength_m = 10.0\n", "", 1)).contains("strokes[0].strength_m"));
    let mut one = base.clone();
    one.strokes.truncate(1);
    one.max_delta_m = 10.0;
    assert!(load(&one.to_toml().unwrap().replacen("schema_version = 3", "schema_version = 1", 1)).contains("[[strokes]]"));
}

fn put(item: &Item) -> Result<WorldDefinition, String> {
    let mut def = WorldDefinition::single_planet(R, 7);
    let before = def.clone();
    match item.put_into(&mut def) {
        Ok(_) => Ok(def),
        Err(e) => {
            assert_eq!(def, before, "a refused put changes nothing");
            Err(e.to_string())
        }
    }
}

// spec: WDEF-013
#[test]
fn a_put_refuses_a_brush_whose_reach_or_bound_cannot_hold_its_strokes() {
    let base = brush();
    assert!(put(&base).is_ok());
    let mut short = base.clone();
    short.reach_m = 100.0;
    assert!(put(&short).unwrap_err().contains("above its reach"), "a reach under the strokes' extent plus the radius");
    let mut zero = base.clone();
    zero.reach_m = 0.0;
    assert!(put(&zero).is_err());
    let mut far = base.clone();
    far.strokes[1].points[1] = (20.1, 30.0);
    assert!(put(&far).unwrap_err().contains("reach"), "a point 11 km away with a 5 km reach");
}

// spec: WDEF-013
#[test]
fn the_footprint_returns_the_brush_with_its_strokes_and_the_stamp_hash_is_the_layout() {
    let item = brush();
    let def = put(&item).unwrap();
    let all = Cap { centre: unit(20.0, 30.0), angle: 0.01 };
    let items = def.footprint_items_cap(BodyId::PLANET, "brush", all).unwrap();
    assert_eq!(items.len(), 1);
    let stamp = items[0].stamp.as_ref().expect("a brush is a stamp");
    assert_eq!(stamp.kind, planet_world_def::stamp::StampKind::Brush);
    assert_eq!(stamp.strokes().len(), 3);
    assert_eq!(stamp.strokes()[2].kind, BrushKind::Flatten);
    assert_eq!(stamp.strokes()[2].target_m(), 12.5);
    assert_eq!(stamp.strokes()[0].points_bits().len(), 3);
    assert!(stamp.points().is_empty());
    // The stamp hash: the kind byte, the three parameters (0), the point count is absent, then the strokes.
    let mut m: Vec<u8> = vec![7];
    m.extend_from_slice(&[0u8; 24]);
    m.extend_from_slice(&3u32.to_le_bytes());
    for s in stamp.strokes() {
        m.push(s.kind as u8);
        for v in [s.radius_m(), s.strength_m(), s.target_m()] {
            m.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        m.extend_from_slice(&(s.points_bits().len() as u32).to_le_bytes());
        for p in s.points_bits() {
            for b in p {
                m.extend_from_slice(&b.to_le_bytes());
            }
        }
    }
    let full = blake3::derive_key("planet-builder stamp v1", &m);
    assert_eq!(stamp.content_hash(), u128::from_be_bytes(full[..16].try_into().unwrap()));
    assert_eq!(items[0].content, stamp.content_hash(), "the footprint carries the derived hash");
    // A tile away from the brush does not read it; one under it does.
    let under = TileId::from_direction(&TangentWarp, unit(20.0, 30.0), 13);
    let away = TileId::from_direction(&TangentWarp, unit(-20.0, 210.0), 13);
    let ctx = planet_world_def::GenContext::new(&def);
    assert_eq!(ctx.query_footprint(BodyId::PLANET, "brush", under, 0.25).unwrap().len(), 1);
    assert!(ctx.query_footprint(BodyId::PLANET, "brush", away, 0.25).unwrap().is_empty());
}

// spec: WDEF-013
#[test]
fn a_stamp_without_strokes_hashes_as_it_did_and_a_stroke_change_rekeys_only_what_it_touches() {
    use planet_world_def::stamp::{Stamp, StampKind};
    // The hill's hash is the layout without any stroke part.
    let hill = Stamp::new(StampKind::Hill, [5.0, 0.0, 0.0]);
    let mut m: Vec<u8> = vec![1];
    for v in [5.0f64, 0.0, 0.0] {
        m.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    let full = blake3::derive_key("planet-builder stamp v1", &m);
    assert_eq!(hill.content_hash(), u128::from_be_bytes(full[..16].try_into().unwrap()));
    assert!(hill.strokes().is_empty());
    let a = brush();
    let mut b = brush();
    b.strokes[0].strength_m = 11.0;
    b.max_delta_m = 100.0;
    let (da, db) = (put(&a).unwrap(), put(&b).unwrap());
    let under = TileId::from_direction(&TangentWarp, unit(20.0, 30.0), 13);
    let away = TileId::from_direction(&TangentWarp, unit(-20.0, 210.0), 13);
    let hash = |d: &WorldDefinition, t: TileId| {
        let ctx = planet_world_def::GenContext::new(d);
        planet_world_def::footprint_hash(&ctx.query_footprint(BodyId::PLANET, "brush", t, 0.25).unwrap())
    };
    assert_ne!(hash(&da, under), hash(&db, under), "a stroke strength re-keys the tile under the brush");
    assert_eq!(hash(&da, away), hash(&db, away), "and not a tile away from it");
}

// spec: WDEF-013
#[test]
fn the_index_refuses_a_brush_stamp_built_by_hand_naming_the_stroke_and_the_reason_and_changes_nothing() {
    use planet_world_def::stamp::{Stamp, StampKind, Stroke};
    use planet_world_def::ItemSpec;
    let at = unit(20.0, 30.0);
    let stroke_of = |kind: BrushKind, radius: f64, strength: f64, target: Option<f64>, points: Vec<Vec3>| Stroke {
        kind,
        radius_m: radius,
        strength_m: strength,
        target_m: target,
        points,
    };
    let near = |dlon: f64| unit(20.0, 30.0 + dlon);
    let spec_of = |strokes: &[Stroke], reach: f64, bound: f64| {
        let stamp = Stamp::new(StampKind::Brush, [0.0; 3]).with_strokes(strokes);
        ItemSpec { id: 1, layer: "brush".into(), priority: 0, dir: at, content_hash: 0, reach_m: reach, max_delta_m: bound, stamp: None }
            .with_stamp(stamp)
    };
    let refuse = |strokes: Vec<Stroke>, reach: f64, bound: f64, want: &str| {
        let mut def = WorldDefinition::single_planet(R, 7);
        let before = def.clone();
        let e = format!("{:?}", def.put_item_spec(BodyId::PLANET, spec_of(&strokes, reach, bound)).expect_err("must be refused"));
        assert!(e.contains(want), "`{want}` in `{e}`");
        assert_eq!(def, before);
    };
    let good = || stroke_of(BrushKind::Raise, 50.0, 10.0, None, vec![at, near(0.001)]);
    let mut def = WorldDefinition::single_planet(R, 7);
    def.put_item_spec(BodyId::PLANET, spec_of(&[good()], 5_000.0, 10.0)).unwrap();
    refuse(vec![], 5_000.0, 10.0, "1 to 64 strokes");
    refuse(vec![good(); 65], 5_000.0, 1000.0, "1 to 64 strokes");
    refuse(vec![stroke_of(BrushKind::Raise, 0.5, 10.0, None, vec![at])], 5_000.0, 10.0, "stroke 0 has a radius");
    refuse(vec![good(), stroke_of(BrushKind::Lower, 50.0, 0.0, None, vec![at])], 5_000.0, 10.0, "stroke 1 has a strength");
    refuse(vec![stroke_of(BrushKind::Raise, 50.0, 10.0, Some(3.0), vec![at])], 5_000.0, 10.0, "has a target");
    refuse(vec![stroke_of(BrushKind::Raise, 50.0, 10.0, None, vec![at, at])], 5_000.0, 10.0, "stroke 0 coincide");
    refuse(vec![stroke_of(BrushKind::Raise, 50.0, 10.0, None, vec![near(0.001)])], 5_000.0, 10.0, "must be the item's anchor");
    refuse(vec![stroke_of(BrushKind::Raise, 50.0, 10.0, None, vec![at, Vec3::new(2.0, 0.0, 0.0)])], 5_000.0, 10.0, "unit directions");
    refuse(vec![good()], 100.0, 10.0, "above its reach");
    refuse(vec![good()], 5_000.0, 9.0, "below the sum of its strengths");
    refuse(vec![stroke_of(BrushKind::Raise, 50.0, f64::NAN, None, vec![at])], 5_000.0, 10.0, "not finite");
}

// spec: AUTH-005, WDEF-013
#[test]
fn a_brush_takes_no_parameters() {
    let mut item = brush();
    item.params.insert("x".into(), 1.0);
    let e = item.to_toml().expect_err("a parameter on a brush").to_string();
    assert!(e.contains("params.x") && e.contains("no parameters"), "{e}");
}
