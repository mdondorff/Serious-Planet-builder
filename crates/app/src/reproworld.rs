//! The world of a repro bundle when it has authored items (VIEW-024): the base definition as TOML and every item as its item-file TOML
//! (AUTH-001), so a replay builds the same definition, items included, from the bundle alone. The definition text cannot carry items
//! (`LoadError::ItemsNotSupported`), so the two travel side by side.

use planet_world_def::item::Item;
use planet_world_def::toml_io::{parse_definition, to_toml};
use planet_world_def::{BodyId, WorldDefinition};

/// The definition text without items and the text of each item. The result is rebuilt and compared with `def` before it is returned: a
/// definition whose items are not exactly `items` (an edit made after the frame's definition was handed over) is refused, so a bundle
/// never records a world that loads to another definition.
pub fn record_world(def: &WorldDefinition, items: &[Item]) -> Result<(String, Vec<String>), String> {
    let mut base = def.clone();
    for id in def.item_ids(BodyId::PLANET) {
        base.remove_item(BodyId::PLANET, id);
    }
    let toml = to_toml(&base).map_err(|e| format!("recording the world: {e}"))?;
    let texts = items.iter().map(|i| i.to_toml().map_err(|e| format!("recording item {}: {e}", i.id))).collect::<Result<Vec<_>, _>>()?;
    let rebuilt = rebuild_world(&toml, &texts)?;
    if rebuilt.hash() != def.hash() {
        return Err(format!(
            "recording the world: the {} items of the editor give definition {:016x}, the frame was planned from {:016x} (an edit is in flight; press F9 again)",
            items.len(),
            rebuilt.hash(),
            def.hash()
        ));
    }
    Ok((toml, texts))
}

/// The definition a bundle recorded: the base text, then every item put into it.
pub fn rebuild_world(definition_toml: &str, items: &[String]) -> Result<WorldDefinition, String> {
    let mut def = parse_definition(definition_toml).map_err(|e| format!("the world recorded in the bundle cannot be read: {e}"))?;
    for text in items {
        let item = Item::from_toml(text).map_err(|e| format!("an item recorded in the bundle cannot be read: {e}"))?;
        item.put_into(&mut def).map_err(|e| format!("an item recorded in the bundle cannot be put into its world: {e}"))?;
    }
    Ok(def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_editor::brush::BrushStroke;
    use planet_editor::commands::{Editor, NewItem};
    use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
    use planet_editor::polyline::GeoPoint;
    use planet_world_def::item::Anchor;
    use planet_world_def::item_store::MemoryStore;
    use planet_world_def::stamp::BrushKind;
    use std::collections::BTreeMap;

    const R: f64 = 6_371_000.0;

    fn east(m: f64) -> GeoPoint {
        GeoPoint::new(0.0, m / R * 180.0 / std::f64::consts::PI)
    }

    fn template(kind: &str, params: &[(&str, f64)]) -> NewItem {
        NewItem {
            body: BodyId::PLANET,
            surface: "sphere".into(),
            layer: "items".into(),
            priority: 0,
            kind: kind.into(),
            anchor: Anchor { latitude_deg: 0.0, longitude_deg: 0.0, height_m: 0.0, heading_deg: 0.0 },
            reach_m: 1.0,
            max_delta_m: 40.0,
            params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
            name: Some(format!("a {kind}")),
            notes: None,
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: Vec::new(),
        }
    }

    fn editor_with_items() -> Editor<MemoryStore> {
        let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(5)));
        let mut def = WorldDefinition::single_planet(R, 7);
        def.set_height_field("cpu-ref-height-v2");
        let mut ed = Editor::new(def, MemoryStore::new(), ids, planet_generators::GENERATOR_VERSION, crate::editwin::EDIT_STAGE_VERSION);
        let mut hill = template("hill", &[("radius_m", 300.0), ("height_m", 25.0)]);
        hill.anchor.longitude_deg = 0.01;
        ed.put(hill).unwrap();
        ed.put_line(template("ridge", &[("amount_m", 30.0), ("width_m", 200.0)]), &[east(1_000.0), east(3_000.0), east(5_000.0)]).unwrap();
        let stroke = BrushStroke {
            kind: BrushKind::Raise,
            radius_m: 100.0,
            strength_m: 2.0,
            target_m: None,
            points: vec![east(8_000.0), east(8_400.0)],
        };
        ed.add_stroke(template("brush", &[]), &stroke, &[]).unwrap();
        ed
    }

    // spec: VIEW-024
    #[test]
    fn a_definition_with_stamps_a_line_and_a_brush_stroke_round_trips_through_the_bundle_world_to_an_equal_definition() {
        let ed = editor_with_items();
        let items: Vec<Item> = ed.items().cloned().collect();
        assert!(
            items.iter().any(|i| i.kind == "hill")
                && items.iter().any(|i| i.kind == "ridge")
                && items.iter().any(|i| !i.strokes.is_empty())
        );
        let (definition_toml, texts) = record_world(ed.definition(), &items).unwrap();
        let rebuilt = rebuild_world(&definition_toml, &texts).unwrap();
        assert_eq!(&rebuilt, ed.definition(), "the replay's definition equals the editor's");
        assert_eq!(rebuilt.hash(), ed.definition().hash());
    }

    // spec: VIEW-024
    #[test]
    fn a_definition_that_is_not_the_editors_items_is_refused_with_both_hashes() {
        let ed = editor_with_items();
        let items: Vec<Item> = ed.items().skip(1).cloned().collect();
        let e = record_world(ed.definition(), &items).unwrap_err();
        assert!(e.contains("an edit is in flight") && e.contains("items of the editor give definition"), "{e}");
    }
}
