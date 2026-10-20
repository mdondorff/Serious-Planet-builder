//! Tier A: the preview list leaves out the line kinds (REND-033, WDEF-011): the preview shader draws the radial stamps, a line shows when
//! its exact tiles arrive.

use planet_core::Vec3;
use planet_frame::preview::preview_delta;
use planet_world_def::footprint::Cap;
use planet_world_def::item::{Anchor, Item};
use planet_world_def::{BodyId, WorldDefinition};
use std::collections::BTreeMap;

// spec: REND-033, WDEF-011
#[test]
fn a_line_added_to_the_world_is_not_in_the_preview_list_and_a_hill_beside_it_is() {
    let item = |id: u128, kind: &str, params: &[(&str, f64)], points: Vec<(f64, f64)>| Item {
        id,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: 0,
        kind: kind.into(),
        anchor: Anchor { latitude_deg: points[0].0, longitude_deg: points[0].1, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 5_000.0,
        max_delta_m: 100.0,
        params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
        base_hash: 1,
        generator_version: 1,
        stage_version: 1,
        name: None,
        notes: None,
        points,
        group: None,
        part: None,
        mask_layer: None,
        strokes: Vec::new(),
    };
    let mut def = WorldDefinition::single_planet(6_371_000.0, 1);
    let empty = def.clone();
    item(1, "ridge", &[("amount_m", 40.0), ("width_m", 200.0)], vec![(20.0, 30.0), (20.01, 30.0)]).put_into(&mut def).unwrap();
    let at = Vec3::new(0.8, 0.45, 0.35).normalized();
    let read =
        |d: &WorldDefinition| d.footprint_items_cap(BodyId::PLANET, "items", Cap { centre: at, angle: core::f64::consts::PI }).unwrap();
    assert_eq!(read(&def).len(), 1, "the ridge is in the footprint");
    assert!(preview_delta(&read(&empty), &read(&def), at).stamps.is_empty(), "and not in the preview");
    item(2, "hill", &[("height_m", 30.0)], vec![(20.0, 30.0)]).canonical_hill().put_into(&mut def).unwrap();
    assert_eq!(preview_delta(&read(&empty), &read(&def), at).stamps.len(), 1, "a hill is");
}

// spec: REND-033, WDEF-013
#[test]
fn a_brush_added_to_the_world_is_not_in_the_preview_list() {
    use planet_world_def::item::ItemStroke;
    use planet_world_def::stamp::BrushKind;
    let brush = Item {
        id: 3,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "items".into(),
        priority: 0,
        kind: "brush".into(),
        anchor: Anchor { latitude_deg: 20.0, longitude_deg: 30.0, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 5_000.0,
        max_delta_m: 10.0,
        params: BTreeMap::new(),
        base_hash: 1,
        generator_version: 1,
        stage_version: 1,
        name: None,
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
        strokes: vec![ItemStroke {
            kind: BrushKind::Raise,
            radius_m: 50.0,
            strength_m: 10.0,
            target_m: None,
            points: vec![(20.0, 30.0), (20.001, 30.0)],
        }],
    };
    let mut def = WorldDefinition::single_planet(6_371_000.0, 1);
    let empty = def.clone();
    brush.put_into(&mut def).unwrap();
    let at = Vec3::new(0.8, 0.45, 0.35).normalized();
    let read =
        |d: &WorldDefinition| d.footprint_items_cap(BodyId::PLANET, "items", Cap { centre: at, angle: core::f64::consts::PI }).unwrap();
    assert_eq!(read(&def).len(), 1, "the brush is in the footprint");
    assert!(preview_delta(&read(&empty), &read(&def), at).stamps.is_empty(), "and not in the preview");
}

trait CanonicalHill {
    fn canonical_hill(self) -> Item;
}

impl CanonicalHill for Item {
    /// A hill has no points: the file form of the item without them.
    fn canonical_hill(mut self) -> Item {
        self.points.clear();
        self
    }
}
