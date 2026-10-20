//! Tier A: the brush tool (VIEW-035): a drag is recorded as points and put as one command; strokes of one area join one brush item, a long
//! stroke is cut at 64 points and at the item's reach; a flatten levels towards the ground under its start; the numbers are metric and
//! checked; undo takes a stroke back.

use planet_core::Vec3;
use planet_editor::brush::{self, BrushStroke};
use planet_editor::commands::Editor;
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::interaction::{Interaction, PointerView, Pressed};
use planet_editor::polyline::GeoPoint;
use planet_editor::session::{Session, Tool};
use planet_frame::pick::SurfaceHit;
use planet_world_def::item_store::MemoryStore;
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    GeoPoint::new(lat_deg, lon_deg).dir()
}

/// The longitude in degrees `m` metres east of 0 on the equator.
fn lon(m: f64) -> f64 {
    m / R * 180.0 / std::f64::consts::PI
}

fn east(m: f64) -> Vec3 {
    unit(0.0, lon(m))
}

fn session(seed: u64) -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R)
}

/// One stroke of the tool `kind`: pressed at `from`, moved through `through`, released.
fn paint(s: &mut Session<MemoryStore>, kind: BrushKind, from: f64, through: &[f64], ground_m: f64) {
    s.set_tool(Tool::Brush(kind));
    s.begin_stroke(east(from), ground_m).unwrap();
    for m in through {
        s.extend_stroke(east(*m));
    }
    s.end_stroke().unwrap();
}

fn brushes(s: &Session<MemoryStore>) -> Vec<planet_world_def::item::Item> {
    s.editor().items().filter(|i| i.kind == "brush").cloned().collect()
}

// spec: VIEW-035
#[test]
fn a_drag_is_resampled_to_a_quarter_of_the_radius_and_cut_at_64_points_sharing_the_point_where_it_is_cut() {
    let stroke = |points: Vec<GeoPoint>| BrushStroke { kind: BrushKind::Raise, radius_m: 100.0, strength_m: 5.0, target_m: None, points };
    // A straight 1 km drag given by its two ends: points every 25 m, the ends kept.
    let line = [GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, lon(1_000.0))];
    let points = brush::resample(&line, 25.0, R);
    assert_eq!(points.len(), 41);
    assert_eq!((points[0], *points.last().unwrap()), (line[0], line[1].canonical()));
    for w in points.windows(2) {
        let d = planet_editor::polyline::arc_m(w[0].dir(), w[1].dir(), R);
        assert!((d - 25.0).abs() < 1e-3, "{d}");
    }
    // A click is one stroke of one point; a repeated place adds nothing.
    assert_eq!(brush::split(&stroke(vec![GeoPoint::new(10.0, 20.0), GeoPoint::new(10.0, 20.0)]), R).len(), 1);
    // 5 km at a 25 m spacing is 201 points: cut at 64, consecutive strokes share the point.
    let long = brush::split(&stroke(vec![GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, lon(5_000.0))]), R);
    assert!(long.len() >= 4 && long.iter().all(|p| p.points.len() <= 64), "{:?}", long.iter().map(|p| p.points.len()).collect::<Vec<_>>());
    for pair in long.windows(2) {
        assert_eq!(pair[0].points.last(), pair[1].points.first(), "the cut point is shared");
    }
    let covered: usize = long.iter().map(|p| p.points.len() - 1).sum::<usize>() + 1;
    assert_eq!(covered, 201, "no point lost, none doubled");
    // A stroke longer than one item can reach is cut where the span would pass a cell edge less the radius.
    let huge = brush::split(&stroke(vec![GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, lon(100_000.0))]), R);
    for p in &huge {
        let far = p.points.iter().map(|q| planet_editor::polyline::arc_m(p.points[0].dir(), q.dir(), R)).fold(0.0, f64::max);
        assert!(far + 100.0 <= planet_world_def::cells::CELL_EDGE_M, "{far}");
    }
}

// spec: VIEW-035
#[test]
fn a_click_is_one_brush_item_with_one_stroke_of_one_point_and_one_undo_takes_it_back() {
    let mut s = session(1);
    let hash = s.editor().hash();
    paint(&mut s, BrushKind::Raise, 0.0, &[], 0.0);
    assert_eq!(s.editor().undo_depth(), 1, "one command");
    let items = brushes(&s);
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!((item.layer.as_str(), item.priority, item.name.as_deref()), ("brush", 0, Some("Brush 1")));
    assert!(item.params.is_empty() && item.points.is_empty() && item.group.is_none() && item.mask_layer.is_none());
    assert_eq!(item.strokes.len(), 1);
    let stroke = &item.strokes[0];
    assert_eq!((stroke.kind, stroke.radius_m, stroke.strength_m, stroke.target_m), (BrushKind::Raise, 100.0, 5.0, None));
    assert_eq!(stroke.points, vec![(item.anchor.latitude_deg, item.anchor.longitude_deg)], "the first point is the anchor");
    assert_eq!(item.max_delta_m, 5.0, "the exact sum of the strengths");
    assert!(item.reach_m >= brush::MIN_AREA_M && item.reach_m <= planet_world_def::cells::CELL_EDGE_M);
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), hash);
    s.redo().unwrap();
    assert_eq!(brushes(&s).len(), 1);
}

// spec: VIEW-035
#[test]
fn strokes_of_one_area_join_one_item_a_far_one_starts_another_and_undo_takes_back_one_stroke_only() {
    let mut s = session(2);
    paint(&mut s, BrushKind::Raise, 0.0, &[300.0], 0.0);
    let one = s.editor().hash();
    paint(&mut s, BrushKind::Lower, 100.0, &[200.0, 400.0], 0.0);
    let items = brushes(&s);
    assert_eq!(items.len(), 1, "the second stroke joined the first item");
    assert_eq!(items[0].strokes.len(), 2);
    assert_eq!((items[0].strokes[1].kind, items[0].strokes[1].target_m), (BrushKind::Lower, None));
    assert_eq!(items[0].max_delta_m, 10.0, "raised to the sum of both strengths");
    assert_eq!(s.editor().undo_depth(), 2);
    // A stroke 30 km away is outside the area: another item.
    paint(&mut s, BrushKind::Smooth, 30_000.0, &[30_200.0], 0.0);
    assert_eq!(brushes(&s).len(), 2);
    // Undo takes the last stroke back, then the one that joined.
    s.undo().unwrap();
    assert_eq!(brushes(&s).len(), 1);
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), one, "the item is as it was with its first stroke");
    assert_eq!(brushes(&s)[0].strokes.len(), 1);
    s.redo().unwrap();
    assert_eq!(brushes(&s)[0].strokes.len(), 2, "a redone join is the join");
}

// spec: VIEW-035
#[test]
fn a_stroke_is_not_added_to_an_item_the_session_did_not_paint_and_a_full_item_starts_another() {
    let mut s = session(3);
    s.set_brush_radius(1.0).unwrap();
    s.set_brush_strength(0.5).unwrap();
    for _ in 0..64 {
        paint(&mut s, BrushKind::Raise, 0.0, &[], 0.0);
    }
    assert_eq!((brushes(&s).len(), brushes(&s)[0].strokes.len()), (1, 64), "64 strokes fill an item");
    assert_eq!(brushes(&s)[0].max_delta_m, 32.0);
    paint(&mut s, BrushKind::Raise, 0.0, &[], 0.0);
    assert_eq!(brushes(&s).len(), 2, "the 65th stroke starts a new item");
    // The command layer joins only what it is told is open.
    let template = {
        let it = &brushes(&s)[0];
        planet_editor::commands::NewItem {
            body: it.body,
            surface: it.surface.clone(),
            layer: it.layer.clone(),
            priority: 0,
            kind: "brush".into(),
            anchor: it.anchor,
            reach_m: 1.0,
            max_delta_m: 0.0,
            params: Default::default(),
            name: None,
            notes: None,
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: Vec::new(),
        }
    };
    let stroke =
        BrushStroke { kind: BrushKind::Raise, radius_m: 100.0, strength_m: 2.0, target_m: None, points: vec![GeoPoint::new(0.0, 0.0)] };
    let before = brushes(&s).len();
    let (touched, _) = s.editor_mut().add_stroke(template.clone(), &stroke, &[]).unwrap();
    assert_eq!((touched.len(), brushes(&s).len()), (1, before + 1), "nothing open: a new item");
    let open = [brushes(&s).into_iter().find(|i| i.strokes.len() == 1 && i.strokes[0].radius_m == 100.0).unwrap().id];
    let (touched, _) = s.editor_mut().add_stroke(template, &stroke, &open).unwrap();
    assert_eq!((touched, brushes(&s).len()), (open.to_vec(), before + 1), "the open item took it");
}

// spec: VIEW-035
#[test]
fn a_long_stroke_becomes_linked_items_of_one_group_in_one_command_and_one_undo_takes_them_all_back() {
    let mut s = session(4);
    let hash = s.editor().hash();
    s.set_brush_radius(200.0).unwrap();
    let mid: Vec<f64> = (1..=24).map(|k| f64::from(k) * 500.0).collect();
    paint(&mut s, BrushKind::Raise, 0.0, &mid, 0.0);
    assert_eq!(s.editor().undo_depth(), 1, "one command for a 12 km drag");
    let mut items = brushes(&s);
    assert!(items.len() >= 3, "{}", items.len());
    items.sort_by_key(|i| i.part);
    let group = items[0].group.expect("linked items share a group");
    assert!(items.iter().all(|i| i.group == Some(group)));
    assert_eq!(items.iter().map(|i| i.part).collect::<Vec<_>>(), (0..items.len() as u32).map(Some).collect::<Vec<_>>());
    for i in &items {
        assert!(i.strokes.iter().all(|st| st.points.len() <= 64));
        i.validate().unwrap();
    }
    // The joined strokes run on: each piece starts at the point where the one before ended.
    for pair in items.windows(2) {
        assert_eq!(pair[0].strokes.last().unwrap().points.last(), pair[1].strokes[0].points.first());
    }
    s.undo().unwrap();
    assert_eq!((s.editor().hash(), brushes(&s).len()), (hash, 0));
}

// spec: VIEW-035
#[test]
fn a_flatten_levels_towards_the_ground_under_its_start_or_the_typed_height_and_the_others_have_no_target() {
    let mut s = session(5);
    paint(&mut s, BrushKind::Flatten, 0.0, &[200.0], 812.5);
    assert_eq!(brushes(&s)[0].strokes[0].target_m, Some(812.5), "the terrain height under the start");
    s.set_brush_target(Some(1_000.0)).unwrap();
    paint(&mut s, BrushKind::Flatten, 50.0, &[250.0], 812.5);
    assert_eq!(brushes(&s)[0].strokes[1].target_m, Some(1_000.0), "the typed height wins");
    paint(&mut s, BrushKind::Raise, 80.0, &[], 812.5);
    paint(&mut s, BrushKind::Smooth, 90.0, &[], 812.5);
    let strokes = &brushes(&s)[0].strokes;
    assert_eq!((strokes[2].target_m, strokes[3].target_m), (None, None));
    assert!(s.set_brush_target(Some(f64::NAN)).is_err());
    s.set_brush_target(None).unwrap();
    assert_eq!(s.brush_settings().target_m, None);
}

// spec: VIEW-035
#[test]
fn the_radius_and_the_strength_are_metres_within_their_ranges_and_a_refused_value_changes_nothing() {
    let mut s = session(6);
    let before = s.brush_settings();
    for r in [0.5, 5_000.1, f64::NAN, f64::INFINITY, -3.0] {
        let e = s.set_brush_radius(r).unwrap_err();
        assert!(e.to_string().contains("radius must be between 1 m and 5 km"), "{r}: {e}");
    }
    for v in [0.0, 0.009, 100.1, f64::NAN, -1.0] {
        let e = s.set_brush_strength(v).unwrap_err();
        assert!(e.to_string().contains("strength must be between 0.01 m and 100 m"), "{v}: {e}");
    }
    assert_eq!(s.brush_settings(), before);
    s.set_brush_radius(5_000.0).unwrap();
    s.set_brush_strength(0.01).unwrap();
    paint(&mut s, BrushKind::Raise, 0.0, &[], 0.0);
    let st = &brushes(&s)[0].strokes[0];
    assert_eq!((st.radius_m, st.strength_m), (5_000.0, 0.01));
    // A stroke keeps the numbers it started with when the settings change under the drag.
    s.set_tool(Tool::Brush(BrushKind::Lower));
    s.begin_stroke(east(100.0), 0.0).unwrap();
    s.set_brush_radius(50.0).unwrap();
    s.extend_stroke(east(300.0));
    assert_eq!(s.stroke_in_progress().unwrap().radius_m, 5_000.0);
    s.end_stroke().unwrap();
    // (A 5 km brush fills its item's reach, so the second stroke is in another item.)
    let radii: Vec<f64> = brushes(&s).iter().flat_map(|i| i.strokes.iter().map(|st| st.radius_m)).collect();
    assert_eq!(radii, vec![5_000.0, 5_000.0]);
}

// spec: VIEW-035
#[test]
fn the_pointer_paints_with_a_brush_tool_a_press_off_the_planet_starts_nothing_and_cancel_puts_nothing() {
    let mut s = session(7);
    let mut ix = Interaction::new(10.0);
    let none: [ItemId; 0] = [];
    let hit = |m: f64| SurfaceHit { point: east(m) * R, dir: east(m), height_m: 40.0, distance_m: 1_000.0 };
    let view = |at: Option<f64>| PointerView { cursor: [0.0, 0.0], hit: at.map(hit), shown: &none, handles: &[], metres_per_pixel: 0.5 };
    s.set_tool(Tool::Brush(BrushKind::Flatten));
    assert!(s.status().contains("flatten brush"), "{}", s.status());
    assert_eq!(ix.press(&mut s, &view(None)).unwrap().0, Pressed::Nothing);
    assert!(s.stroke_in_progress().is_none());
    assert_eq!(ix.press(&mut s, &view(Some(0.0))).unwrap().0, Pressed::Stroke);
    ix.moved(&mut s, &view(Some(100.0))).unwrap();
    ix.moved(&mut s, &view(None)).unwrap();
    ix.moved(&mut s, &view(Some(200.0))).unwrap();
    assert_eq!(s.stroke_in_progress().unwrap().points.len(), 3);
    let affected = ix.release(&mut s).unwrap().expect("the release ends the stroke");
    assert!(!affected.is_empty());
    assert_eq!(s.editor().undo_depth(), 1);
    assert_eq!(brushes(&s)[0].strokes[0].target_m, Some(40.0), "the hit's height is the flatten target");
    // Esc: a stroke begun and cancelled is nothing.
    ix.press(&mut s, &view(Some(0.0))).unwrap();
    ix.moved(&mut s, &view(Some(100.0))).unwrap();
    assert!(ix.cancel(&mut s).unwrap().is_some());
    assert!(s.stroke_in_progress().is_none());
    assert_eq!(s.editor().undo_depth(), 1);
    // Switching tools drops a stroke too.
    ix.press(&mut s, &view(Some(0.0))).unwrap();
    s.set_tool(Tool::Select);
    assert!(s.stroke_in_progress().is_none());
}

// spec: VIEW-035
#[test]
fn a_brush_item_is_no_stamp_to_pick_and_has_no_handle() {
    let mut s = session(8);
    paint(&mut s, BrushKind::Raise, 0.0, &[300.0], 0.0);
    let id = brushes(&s)[0].id;
    let hit = SurfaceHit { point: east(100.0) * R, dir: east(100.0), height_m: 0.0, distance_m: 1_000.0 };
    assert_eq!(s.pick(&hit, &[id]), None, "a brush is picked from the list, not from the planet");
    s.select(id);
    assert!(s.handles(0.0).is_empty(), "the anchor of a brush cannot move");
}

// spec: VIEW-035
#[test]
fn what_an_item_holds_is_kept_incrementally_and_equals_what_reading_every_point_gives() {
    let stroke = |m: f64, radius: f64, strength: f64| {
        brush::to_item_stroke(&BrushStroke {
            kind: BrushKind::Raise,
            radius_m: radius,
            strength_m: strength,
            target_m: None,
            points: vec![GeoPoint::new(0.0, lon(m)), GeoPoint::new(0.0, lon(m + 40.0))],
        })
    };
    let anchor = (0.0, lon(0.0));
    let mut strokes = vec![stroke(0.0, 100.0, 5.0)];
    let mut fill = brush::Fill::of(&strokes, anchor, R);
    for (m, radius, strength) in [(300.0, 50.0, 2.0), (900.0, 400.0, 7.5), (100.0, 1.0, 0.5)] {
        let piece = stroke(m, radius, strength);
        fill = fill.with(&piece, anchor, R);
        strokes.push(piece);
        let again = brush::Fill::of(&strokes, anchor, R);
        assert_eq!((fill.strokes, fill.points), (again.strokes, again.points));
        for (a, b) in [(fill.strength_m, again.strength_m), (fill.far_m, again.far_m), (fill.radius_m, again.radius_m)] {
            assert!((a - b).abs() < 1e-9, "{a} {b}");
        }
    }
    // The decision is the same as `fits` over the strokes.
    let piece = stroke(1_200.0, 100.0, 1.0);
    for reach in [500.0, 1_000.0, 1_341.0, 1_342.0, 5_000.0] {
        assert_eq!(fill.with(&piece, anchor, R).fits(reach), brush::fits(&strokes, anchor, reach, &piece, R), "reach {reach}");
    }
}
