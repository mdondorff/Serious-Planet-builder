//! Tier A: the inspector of a line and of a region (VIEW-033): the amount and the half-width typed for a line change every part of it as one
//! command (and make its reach again), the surface type and the mask of a region change in one command each.

use planet_core::Vec3;
use planet_editor::commands::{EditError, Editor};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::polyline::{GeoPoint, PolylineError};
use planet_editor::session::{Session, Tool};
use planet_world_def::item_store::MemoryStore;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    GeoPoint::new(lat_deg, lon_deg).dir()
}

fn lon(m: f64) -> f64 {
    m / R * 180.0 / std::f64::consts::PI
}

fn session(seed: u64) -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R)
}

/// A ridge through the given metres east of (0, 0), drawn with the tool; the first part is selected.
fn ridge(s: &mut Session<MemoryStore>, metres: &[f64]) -> ItemId {
    s.set_tool(Tool::Line("ridge".into()));
    for m in metres {
        s.add_draft_point(unit(0.0, lon(*m))).unwrap();
    }
    s.finish_draft().unwrap().0
}

fn long_ridge(s: &mut Session<MemoryStore>) -> ItemId {
    let metres: Vec<f64> = (0..=15).map(|k| f64::from(k) * 10_000.0).collect();
    ridge(s, &metres)
}

// spec: VIEW-033
#[test]
fn a_line_shows_its_amount_and_half_width_with_their_ranges_and_a_region_its_surface_type_and_mask() {
    let mut s = session(1);
    let id = ridge(&mut s, &[0.0, 5_000.0]);
    let i = s.inspector(Vec3::new(1.0, 0.0, 0.0) * R).unwrap();
    let rows: Vec<_> = i.params.iter().map(|p| (p.key.as_str(), p.label.as_str(), p.range)).collect();
    assert_eq!(rows, vec![("amount_m", "Height", Some((0.0, 1_000.0))), ("width_m", "Half-width", Some((1.0, 5_000.0)))]);
    assert_eq!((i.params[0].text.as_str(), i.params[1].text.as_str()), ("30 m", "200 m"));
    assert!(i.region.is_none(), "a line has no region rows");
    // A channel calls its amount a depth.
    s.set_tool(Tool::Line("channel".into()));
    for m in [0.0, 3_000.0] {
        s.add_draft_point(unit(0.5, lon(m))).unwrap();
    }
    s.finish_draft().unwrap();
    assert_eq!(s.inspector(Vec3::new(1.0, 0.0, 0.0) * R).unwrap().params[0].label, "Depth");
    s.select(id);

    // A region: nine surface types in table order, no mask yet, the layers of the other edits to choose from.
    s.set_tool(Tool::Region);
    for (n, e) in [(10_000.0, 0.0), (10_000.0, 8_000.0), (16_000.0, 4_000.0)] {
        s.add_draft_point(unit(n / R * 180.0 / std::f64::consts::PI, lon(e))).unwrap();
    }
    let (region, _) = s.finish_draft().unwrap();
    let r = s.inspector(Vec3::new(1.0, 0.0, 0.0) * R).unwrap().region.expect("a region has rows");
    assert_eq!(r.surface_names, vec!["rock", "sand", "grass", "snow", "forest", "ice", "desert", "wetland", "urban"]);
    assert_eq!((r.surface_type, r.clips_inside, r.mask_layer.clone()), (0, false, None));
    assert_eq!(r.layers, vec!["stamps".to_string()], "the region's own layer is not offered");
    let i = s.inspector(Vec3::new(1.0, 0.0, 0.0) * R).unwrap();
    assert!(i.params.iter().all(|p| p.key != "surface_type" && p.key != "mask_mode"), "no second, unchecked control for them");
    assert_eq!(s.selected(), Some(region));
}

// spec: VIEW-033
#[test]
fn a_typed_width_or_amount_changes_every_part_of_a_split_line_in_one_command_and_a_refused_one_changes_nothing() {
    let mut s = session(2);
    let first = long_ridge(&mut s);
    let parts = s.editor().line_items(first).unwrap();
    assert!(parts.len() >= 4);
    // The selected part may be a later one; the whole line changes.
    s.select(parts[2]);
    let (depth, hash) = (s.editor().undo_depth(), s.editor().hash());

    s.set_param_text("width_m", "800").unwrap();
    assert_eq!(s.editor().undo_depth(), depth + 1, "one command for the line");
    let parts_after = s.editor().line_items(first).unwrap();
    assert!(parts_after.len() >= parts.len(), "a wider line needs at least as many parts");
    for p in &parts_after {
        let item = s.item(*p).unwrap();
        assert_eq!(item.params["width_m"], 800.0);
        assert!(item.reach_m >= 800.0 + 1.0 && item.reach_m <= planet_world_def::cells::CELL_EDGE_M, "reach {}", item.reach_m);
    }
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), hash, "one undo takes the whole change back");

    // The amount raises the bound on the change where needed, in every part.
    s.set_param_text("amount_m", "90 m").unwrap();
    for p in s.editor().line_items(first).unwrap() {
        let item = s.item(p).unwrap();
        assert_eq!((item.params["amount_m"], item.max_delta_m >= 90.0), (90.0, true));
    }
    s.undo().unwrap();

    // Refused: out of the kind's range, not a number, and a width that leaves no room for a part; nothing changes.
    let hash = s.editor().hash();
    for (key, text, why) in [
        ("width_m", "9 km", "Half-width must be between"),
        ("amount_m", "-5", "Height must be between"),
        ("width_m", "wide", "not a number"),
    ] {
        let e = s.set_param_text(key, text).unwrap_err();
        assert!(e.to_string().contains(why), "{key} {text}: {e}");
    }
    assert_eq!(s.editor().hash(), hash);
    let e = s.editor_mut().set_line_params(first, None, Some(33_000.0)).unwrap_err();
    assert!(matches!(e, EditError::Geometry(PolylineError::WidthTooLarge { .. })), "{e:?}");
    assert_eq!(s.editor().hash(), hash);
    // A stamp's radius still goes through the single-item path.
    let (hill, _) = s.place("hill", unit(30.0, 30.0)).unwrap();
    s.set_param_text("radius_m", "300").unwrap();
    assert_eq!(s.item(hill).unwrap().reach_m, 300.0);
    // Lines are not stamps to place.
    assert!(s.place("ridge", unit(0.0, 0.0)).is_err());
}

// spec: VIEW-033
#[test]
fn a_region_takes_a_surface_type_and_a_mask_each_in_one_change_and_the_mask_is_never_half_set() {
    let mut s = session(3);
    ridge(&mut s, &[0.0, 5_000.0]);
    s.set_tool(Tool::Region);
    for (n, e) in [(10_000.0, 0.0), (10_000.0, 8_000.0), (16_000.0, 4_000.0)] {
        s.add_draft_point(unit(n / R * 180.0 / std::f64::consts::PI, lon(e))).unwrap();
    }
    let (region, _) = s.finish_draft().unwrap();
    let depth = s.editor().undo_depth();

    s.set_surface_type(5).unwrap();
    assert_eq!((s.item(region).unwrap().params["surface_type"], s.editor().undo_depth()), (5.0, depth + 1));
    assert!(s.status().contains("ice"), "{}", s.status());
    let e = s.set_surface_type(9).unwrap_err();
    assert!(e.to_string().contains("no surface type 9"), "{e}");

    // Inside needs a layer, and not the region's own; with one, mode and layer change together.
    assert!(s.set_mask(true, None).unwrap_err().to_string().contains("choose the layer"));
    assert!(s.set_mask(true, Some("regions")).unwrap_err().to_string().contains("own layer"));
    assert_eq!(s.item(region).unwrap().params["mask_mode"], 0.0, "refused: nothing half done");
    s.set_mask(true, Some("stamps")).unwrap();
    let item = s.item(region).unwrap().clone();
    assert_eq!((item.params["mask_mode"], item.mask_layer.as_deref(), item.layer.as_str()), (2.0, Some("stamps"), "regions"));
    let r = s.inspector(Vec3::new(1.0, 0.0, 0.0) * R).unwrap().region.unwrap();
    assert_eq!((r.clips_inside, r.mask_layer.as_deref()), (true, Some("stamps")));
    // And back to labelling: mode 0 and no layer together.
    s.set_mask(false, None).unwrap();
    let item = s.item(region).unwrap();
    assert_eq!((item.params["mask_mode"], item.mask_layer.clone()), (0.0, None));
    // One undo takes the last change back (the item is then clipping again).
    s.undo().unwrap();
    assert_eq!(s.item(region).unwrap().mask_layer.as_deref(), Some("stamps"));

    // A line is not a region.
    let ridge_id = s.editor().items().find(|i| i.kind == "ridge").unwrap().id;
    s.select(ridge_id);
    assert!(s.set_surface_type(1).unwrap_err().to_string().contains("not a region"));
    assert!(s.set_mask(false, None).unwrap_err().to_string().contains("not a region"));
}
