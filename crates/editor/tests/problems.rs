//! Tier A: the problems list of the editor (VIEW-047, M7 A5): the edits too steep to draw fully, with very many strokes stacked and near tiles the plan
//! holds back are listed with what is wrong, and the fixes (reduce the strengths, merge a stack of strokes into one) are one command each that
//! one undo takes back.

use planet_core::Vec3;
use planet_editor::brush::{self, BrushStroke};
use planet_editor::commands::{Editor, NewItem};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::polyline::GeoPoint;
use planet_editor::problems::{self, Issue};
use planet_editor::session::{Session, SessionError};
use planet_editor::slope;
use planet_world_def::item::{Anchor, Item, ItemStroke};
use planet_world_def::item_store::MemoryStore;
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn lon(m: f64) -> f64 {
    m / R * 180.0 / std::f64::consts::PI
}

fn east(m: f64) -> Vec3 {
    GeoPoint::new(0.0, lon(m)).dir()
}

fn session() -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(3)));
    Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R)
}

fn template(name: &str) -> NewItem {
    NewItem {
        body: BodyId::PLANET,
        surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
        layer: brush::BRUSH_LAYER.to_string(),
        priority: 0,
        kind: brush::BRUSH_KIND.to_string(),
        anchor: Anchor { latitude_deg: 0.0, longitude_deg: 0.0, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 1.0,
        max_delta_m: 0.0,
        params: Default::default(),
        name: Some(name.to_string()),
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
        strokes: Vec::new(),
    }
}

/// `n` strokes of `kind` on one point `at` metres east, filed into one item the way a session fills it, without the slope guard.
fn pile(s: &mut Session<MemoryStore>, name: &str, kind: BrushKind, n: usize, strength_m: f64, radius_m: f64, at: f64) -> ItemId {
    let piece = BrushStroke { kind, radius_m, strength_m, target_m: None, points: vec![GeoPoint::new(0.0, lon(at))] };
    let mut open = Vec::new();
    let mut id = None;
    for _ in 0..n {
        let (touched, _) = s.editor_mut().add_stroke(NewItem { name: Some(name.to_string()), ..template(name) }, &piece, &open).unwrap();
        id = touched.first().copied();
        open = touched;
    }
    id.expect("an item")
}

fn camera() -> Vec3 {
    east(0.0) * R
}

// spec: VIEW-047
#[test]
fn the_problems_list_has_the_steep_the_stacked_and_the_capped_with_what_is_wrong_and_nothing_else() {
    let mut s = session();
    assert!(s.problems(&[], camera()).is_empty(), "an empty world has no problem");
    let steep = pile(&mut s, "Steep", BrushKind::Raise, 40, 5.0, 100.0, 0.0);
    // Nine light strokes at one place, 20 km away: stacked, not steep.
    let stacked = pile(&mut s, "Stacked", BrushKind::Raise, 9, 0.1, 100.0, 20_000.0);
    // Three strokes: fine.
    let fine = pile(&mut s, "Fine", BrushKind::Raise, 3, 0.5, 100.0, 40_000.0);
    // A stamp near a capped tile: listed as capped, with nothing to reduce or merge.
    let (hill, _) = s.place("hill", east(60_000.0)).unwrap();
    let list = s.problems(&[hill], camera());
    let ids: Vec<ItemId> = list.iter().map(|p| p.row.id).collect();
    assert!(ids.contains(&steep) && ids.contains(&stacked) && ids.contains(&hill), "{ids:?}");
    assert!(!ids.contains(&fine), "three strokes are not a problem");
    let of = |id: ItemId| list.iter().find(|p| p.row.id == id).unwrap();
    assert!(of(steep).issues.contains(&Issue::TooSteep));
    assert!(
        matches!(of(steep).issues.iter().find(|i| matches!(i, Issue::Stacked { .. })), Some(Issue::Stacked { depth, .. }) if *depth >= problems::STACK_WARN)
    );
    assert_eq!(of(stacked).issues.len(), 1);
    assert!(matches!(of(stacked).issues[0], Issue::Stacked { depth, strokes: 9 } if depth == 9), "{:?}", of(stacked).issues);
    assert_eq!(of(hill).issues, vec![Issue::TilesCapped]);
    assert!(of(steep).can_reduce && of(steep).can_merge && of(stacked).can_reduce && of(stacked).can_merge);
    assert!(!of(hill).can_reduce && !of(hill).can_merge, "a stamp has no strokes");
    // The steepest first, the capped last.
    assert_eq!(list[0].row.id, steep);
    assert_eq!(list.last().unwrap().row.id, hill);
    // The words.
    assert_eq!(Issue::TooSteep.text(), slope::WARNING);
    assert!(
        Issue::Stacked { depth: 9, strokes: 9 }.text().contains("9 strokes stacked"),
        "{}",
        Issue::Stacked { depth: 9, strokes: 9 }.text()
    );
    assert!(Issue::TilesCapped.text().contains("held back"));
    // A capped edit that is not in the definition is left out.
    assert!(!s.problems(&[ItemId::default()], camera()).iter().any(|p| p.row.id == ItemId::default()));
}

// spec: VIEW-047
#[test]
fn a_brush_with_most_of_its_64_strokes_is_listed_even_when_no_place_has_many_of_them() {
    let mut s = session();
    // 50 small strokes a hundred metres apart in one item, as a file from another tool can hold them: never two on one point, but near the cap.
    let strokes: Vec<ItemStroke> = (0..50)
        .map(|k| ItemStroke {
            kind: BrushKind::Smooth,
            radius_m: 20.0,
            strength_m: 1.0,
            target_m: None,
            points: vec![(0.0, lon(f64::from(k) * 100.0))],
        })
        .collect();
    let (id, _) = s.editor_mut().put(NewItem { reach_m: 10_000.0, max_delta_m: 100.0, strokes, ..template("Row") }).unwrap();
    let n = s.editor().item(id).unwrap().strokes.len();
    assert!(n >= problems::STROKES_WARN, "{n} strokes in the item");
    let depth = problems::stack_depths(s.editor().items(), R)[&id];
    assert!(depth < problems::STACK_WARN, "no place has many: {depth}");
    let list = s.problems(&[], camera());
    let p = list.iter().find(|p| p.row.id == id).expect("listed");
    assert!(p.issues.iter().any(|i| matches!(i, Issue::Stacked { strokes, .. } if *strokes >= problems::STROKES_WARN)), "{:?}", p.issues);
    assert!(Issue::Stacked { depth, strokes: n }.text().contains("of 64 strokes"));
}

// spec: VIEW-047
#[test]
fn reducing_a_steep_brush_makes_it_fit_in_one_undoable_command_and_a_stacked_one_is_halved_and_a_stamp_is_refused() {
    let mut s = session();
    let limit = slope::gradient_limit(s.drawable_slope());
    let steep = pile(&mut s, "Steep", BrushKind::Raise, 40, 5.0, 100.0, 0.0);
    assert!(s.steep_items().contains(&steep));
    let before: Vec<f64> = s.editor().item(steep).unwrap().strokes.iter().map(|t| t.strength_m).collect();
    let hash_before = s.editor().hash();
    let (factor, _) = s.reduce_item(steep).unwrap();
    assert!(factor > 0.0 && factor < 1.0, "{factor}");
    assert!(!s.steep_items().contains(&steep), "no longer too steep");
    let after: Vec<f64> = s.editor().item(steep).unwrap().strokes.iter().map(|t| t.strength_m).collect();
    assert!(after.iter().zip(&before).all(|(a, b)| (a - b * factor).abs() < 1e-12), "every strength is scaled by the factor");
    assert!(factor > 0.4 * limit / (40.0 * slope::STROKE_GRADIENT * 5.0 / 100.0), "close to the largest factor that fits: {factor}");
    s.editor().item(steep).unwrap().validate().expect("the item is still valid (the bound holds the sum)");
    assert!(s.status().contains('%'), "{}", s.status());
    // One undo takes it back, to the exact definition.
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), hash_before);
    // A stacked brush that is not too steep is cut by half.
    let stacked = pile(&mut s, "Stacked", BrushKind::Raise, 9, 0.1, 100.0, 20_000.0);
    assert!(!s.steep_items().contains(&stacked));
    let (f, _) = s.reduce_item(stacked).unwrap();
    assert_eq!(f, problems::REDUCE_STACKED);
    assert!(s.editor().item(stacked).unwrap().strokes.iter().all(|t| (t.strength_m - 0.05).abs() < 1e-12));
    // A stamp has nothing to reduce.
    let (hill, _) = s.place("hill", east(80_000.0)).unwrap();
    assert!(matches!(s.reduce_item(hill), Err(SessionError::Refused(m)) if m.contains("no strokes")));
}

// spec: VIEW-047
#[test]
fn a_brush_that_other_edits_make_too_steep_cannot_be_fitted_and_says_so() {
    let mut s = session();
    let _big = pile(&mut s, "Big", BrushKind::Raise, 60, 5.0, 100.0, 0.0);
    // A light stroke of another item on the same place: steep because of the first.
    let layer = |n: &str| NewItem { layer: "other".to_string(), ..template(n) };
    let piece =
        BrushStroke { kind: BrushKind::Raise, radius_m: 100.0, strength_m: 0.1, target_m: None, points: vec![GeoPoint::new(0.0, 0.0)] };
    let (touched, _) = s.editor_mut().add_stroke(layer("Light"), &piece, &[]).unwrap();
    let light = touched[0];
    assert!(s.steep_items().contains(&light));
    let items: Vec<Item> = s.editor().items().cloned().collect();
    assert_eq!(problems::fit_factor(&items, light, slope::gradient_limit(s.drawable_slope()), R), None);
    let hash = s.editor().hash();
    assert!(matches!(s.reduce_item(light), Err(SessionError::Refused(m)) if m.contains("other edits")));
    assert_eq!(s.editor().hash(), hash, "nothing changed");
    // And an item that fits already has the factor 1.
    let mut t = session();
    let fine = pile(&mut t, "Fine", BrushKind::Raise, 3, 1.0, 100.0, 0.0);
    let items: Vec<Item> = t.editor().items().cloned().collect();
    assert_eq!(problems::fit_factor(&items, fine, slope::gradient_limit(t.drawable_slope()), R), Some(1.0));
}

fn stroke(kind: BrushKind, radius_m: f64, strength_m: f64, at: f64) -> ItemStroke {
    ItemStroke {
        kind,
        radius_m,
        strength_m,
        target_m: if kind == BrushKind::Flatten { Some(0.0) } else { None },
        points: vec![(0.0, lon(at))],
    }
}

// spec: VIEW-047
#[test]
fn merging_a_stack_makes_one_stroke_of_the_summed_strength_in_the_place_of_the_first_and_leaves_the_rest_alone() {
    // The pure function: three raises on one place, a flatten on it, a raise far away, a lower on it.
    let strokes = vec![
        stroke(BrushKind::Raise, 100.0, 1.0, 0.0),
        stroke(BrushKind::Flatten, 100.0, 1.0, 0.0),
        stroke(BrushKind::Raise, 150.0, 2.0, 30.0),
        stroke(BrushKind::Raise, 100.0, 3.0, 0.0),
        stroke(BrushKind::Raise, 100.0, 4.0, 50_000.0),
        stroke(BrushKind::Lower, 100.0, 5.0, 0.0),
    ];
    let (merged, n) = problems::merge_stack(&strokes, R).expect("a stack");
    assert_eq!(n, 3, "the three raises on the place");
    assert_eq!(merged.len(), 4, "6 - 3 + 1");
    assert_eq!(merged[0].kind, BrushKind::Raise);
    assert_eq!(merged[0].strength_m, 6.0, "1 + 2 + 3");
    assert_eq!(merged[0].radius_m, 150.0, "the largest radius");
    assert_eq!(merged[0].points, strokes[0].points, "the first stroke's path, so the item's anchor stays the first point");
    assert_eq!(merged[1], strokes[1], "the flatten is not a raise");
    assert_eq!(merged[2], strokes[4], "the far raise");
    assert_eq!(merged[3], strokes[5], "the lower is another kind");
    // Nothing overlaps: nothing to merge. A kind never merges with another.
    assert!(problems::merge_stack(&[stroke(BrushKind::Raise, 100.0, 1.0, 0.0), stroke(BrushKind::Raise, 100.0, 1.0, 5_000.0)], R).is_none());
    assert!(problems::merge_stack(&[stroke(BrushKind::Raise, 100.0, 1.0, 0.0), stroke(BrushKind::Lower, 100.0, 1.0, 0.0)], R).is_none());
    // A flatten and a smooth are not added up.
    assert!(problems::merge_stack(&[stroke(BrushKind::Smooth, 100.0, 1.0, 0.0), stroke(BrushKind::Smooth, 100.0, 1.0, 0.0)], R).is_none());
}

// spec: VIEW-047
#[test]
fn merging_in_a_session_is_one_command_keeps_the_item_valid_and_one_undo_takes_it_back() {
    let mut s = session();
    let id = pile(&mut s, "Pile", BrushKind::Raise, 12, 0.5, 100.0, 0.0);
    let hash = s.editor().hash();
    let sum: f64 = s.editor().item(id).unwrap().strokes.iter().map(|t| t.strength_m).sum();
    let (n, _) = s.merge_item(id).unwrap();
    assert_eq!(n, 12);
    let item = s.editor().item(id).unwrap();
    assert_eq!(item.strokes.len(), 1);
    assert!((item.strokes[0].strength_m - sum).abs() < 1e-9);
    item.validate().expect("the merged item is valid");
    assert!(s.status().contains("12 strokes merged"), "{}", s.status());
    assert!(
        !s.problems(&[], camera()).iter().any(|p| p.row.id == id && p.issues.iter().any(|i| matches!(i, Issue::Stacked { .. }))),
        "no longer stacked"
    );
    // Nothing left to merge.
    assert!(matches!(s.merge_item(id), Err(SessionError::Refused(m)) if m.contains("overlap")));
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), hash, "one undo, the exact definition");
    // A stamp is refused.
    let (hill, _) = s.place("hill", east(80_000.0)).unwrap();
    assert!(matches!(s.merge_item(hill), Err(SessionError::Refused(m)) if m.contains("no strokes")));
}
