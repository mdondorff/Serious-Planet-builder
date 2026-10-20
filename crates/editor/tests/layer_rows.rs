//! Tier A: the rows of lines, regions and brushes in the layer list (VIEW-016): a split line or a brush dragged over several items is one row
//! with its part count, a brush row has its stroke count, a line or a region its point count; selecting the row selects the item; hiding and
//! deleting the row take the whole group.

use planet_core::Vec3;
use planet_editor::commands::Editor;
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::lists;
use planet_editor::polyline::GeoPoint;
use planet_editor::session::{Session, Tool};
use planet_world_def::item_store::MemoryStore;
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    GeoPoint::new(lat_deg, lon_deg).dir()
}

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

fn cam() -> Vec3 {
    unit(0.0, 0.0) * (R + 5_000.0)
}

fn line(s: &mut Session<MemoryStore>, kind: &str, from_m: f64, to_m: f64, lat: f64) -> ItemId {
    s.set_tool(Tool::Line(kind.into()));
    let n = 16;
    for k in 0..=n {
        let m = from_m + (to_m - from_m) * f64::from(k) / f64::from(n);
        s.add_draft_point(unit(lat, lon(m))).unwrap();
    }
    s.finish_draft().unwrap().0
}

fn paint(s: &mut Session<MemoryStore>, kind: BrushKind, from: f64, through: &[f64]) {
    s.set_tool(Tool::Brush(kind));
    s.begin_stroke(east(from), 0.0).unwrap();
    for m in through {
        s.extend_stroke(east(*m));
    }
    s.end_stroke().unwrap();
}

fn rows(s: &Session<MemoryStore>) -> Vec<(String, Vec<lists::Row>)> {
    s.layer_groups(cam()).into_iter().map(|g| (g.layer, g.rows)).collect()
}

fn all_rows(s: &Session<MemoryStore>) -> Vec<lists::Row> {
    rows(s).into_iter().flat_map(|(_, r)| r).collect()
}

// spec: VIEW-016
#[test]
fn a_split_line_is_one_row_with_its_part_count_and_a_short_line_has_its_point_count() {
    let mut s = session(1);
    let long = line(&mut s, "ridge", 0.0, 150_000.0, 0.0);
    let parts = s.editor().line_items(long).unwrap().len();
    assert!(parts >= 3, "{parts}");
    line(&mut s, "channel", 0.0, 3_000.0, 0.5);
    let all = all_rows(&s);
    // One row for the long line, not one per part; it is the line's first part, with the count.
    let ridge: Vec<&lists::Row> = all.iter().filter(|r| r.kind == "ridge").collect();
    assert_eq!(ridge.len(), 1, "one row for a split line");
    assert_eq!((ridge[0].id, ridge[0].parts), (s.editor().line_items(long).unwrap()[0], parts));
    assert_eq!(ridge[0].detail(), format!("{parts} parts"));
    let channel = all.iter().find(|r| r.kind == "channel").unwrap();
    assert_eq!((channel.parts, channel.detail()), (1, "17 points".to_string()));
    // The other lists keep every item: the search finds each part, and a part says which one it is.
    let found = s.search("ridge", None, cam());
    assert_eq!(found.len(), parts);
    assert!(found.iter().any(|r| r.detail() == "part 2"), "{:?}", found.iter().map(lists::Row::detail).collect::<Vec<_>>());
    // The row is placed by the layer, in the order the edits apply.
    let layers: Vec<String> = rows(&s).into_iter().map(|(l, _)| l).collect();
    assert_eq!(layers, vec!["stamps".to_string()]);
}

// spec: VIEW-016
#[test]
fn a_region_and_a_brush_have_rows_with_their_points_and_strokes_and_a_dragged_brush_over_several_items_is_one_row() {
    let mut s = session(2);
    s.set_tool(Tool::Region);
    for (n, e) in [(10_000.0, 0.0), (10_000.0, 8_000.0), (16_000.0, 4_000.0)] {
        s.add_draft_point(unit(n / R * 180.0 / std::f64::consts::PI, lon(e))).unwrap();
    }
    s.finish_draft().unwrap();
    s.place("hill", east(30_000.0)).unwrap();
    paint(&mut s, BrushKind::Raise, 0.0, &[300.0]);
    paint(&mut s, BrushKind::Lower, 100.0, &[400.0]);
    let all = all_rows(&s);
    let region = all.iter().find(|r| r.kind == "region").unwrap();
    assert_eq!(region.detail(), "3 points");
    let hill = all.iter().find(|r| r.kind == "hill").unwrap();
    assert_eq!(hill.detail(), "", "a stamp has no detail");
    let brush = all.iter().find(|r| r.kind == "brush").unwrap();
    assert_eq!((brush.strokes, brush.detail()), (2, "2 strokes".to_string()));
    assert_eq!(rows(&s).iter().map(|(l, r)| (l.as_str(), r.len())).collect::<Vec<_>>(), [("stamps", 1), ("brush", 1), ("regions", 1)]);
    // One stroke is "1 stroke"; a long drag is linked items of one group and one row with its parts and all its strokes.
    let mut t = session(3);
    paint(&mut t, BrushKind::Raise, 0.0, &[]);
    assert_eq!(all_rows(&t)[0].detail(), "1 stroke");
    let mut u = session(4);
    u.set_brush_radius(200.0).unwrap();
    let mid: Vec<f64> = (1..=24).map(|k| f64::from(k) * 500.0).collect();
    paint(&mut u, BrushKind::Raise, 0.0, &mid);
    let items = u.editor().items().filter(|i| i.kind == "brush").count();
    assert!(items >= 3);
    let one = all_rows(&u);
    assert_eq!(one.len(), 1, "one row for the group");
    assert_eq!((one[0].parts, one[0].strokes), (items, u.editor().items().map(|i| i.strokes.len()).sum::<usize>()));
    assert_eq!(one[0].detail(), format!("{items} parts, {} strokes", one[0].strokes));
}

// spec: VIEW-016
#[test]
fn selecting_a_row_selects_the_item_hiding_it_hides_the_whole_group_and_deleting_it_is_one_command_for_the_whole_group() {
    let mut s = session(5);
    let long = line(&mut s, "ridge", 0.0, 150_000.0, 0.0);
    let members = s.editor().line_items(long).unwrap();
    s.set_tool(Tool::Select);
    let hash = s.editor().hash();
    let depth = s.editor().undo_depth();
    let row = all_rows(&s).into_iter().find(|r| r.kind == "ridge").unwrap();
    s.select(row.id);
    assert_eq!(s.selected(), Some(row.id));
    // Hiding: every part.
    s.toggle_hidden(row.id);
    assert!(members.iter().all(|m| s.is_hidden(*m)));
    s.toggle_hidden(row.id);
    assert!(members.iter().all(|m| !s.is_hidden(*m)));
    // Deleting: all the parts in one command; one undo brings the line back exactly.
    s.delete_selected().unwrap();
    assert_eq!(s.editor().items().count(), 0, "no part is left");
    assert_eq!(s.editor().undo_depth(), depth + 1);
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), hash);
    // A brush found by its row is deleted, and a stamp is as before.
    paint(&mut s, BrushKind::Raise, 0.0, &[300.0]);
    let brush = all_rows(&s).into_iter().find(|r| r.kind == "brush").unwrap();
    s.set_tool(Tool::Select);
    s.select(brush.id);
    s.delete_selected().unwrap();
    assert!(s.editor().items().all(|i| i.kind != "brush"));
    let (hill, _) = s.place("hill", east(5_000.0)).unwrap();
    s.delete_selected().unwrap();
    assert!(s.item(hill).is_none());
}
