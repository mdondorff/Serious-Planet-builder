//! Tier A: the edges of a split line (VIEW-033, VIEW-016): the typed amount and width as the editor takes them without the inspector's
//! range check, the width at which no part fits, and hover and redo after a split line is deleted whole.

use planet_core::Vec3;
use planet_editor::commands::Editor;
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::polyline::{split_line, GeoPoint, PolylineError, SPLIT_FRACTION};
use planet_editor::session::{Session, Tool};
use planet_world_def::cells::CELL_EDGE_M;
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

/// A ridge of 150 km east of (0, 0), drawn with the tool: it is split into several linked parts. The first part is returned.
fn long_ridge(s: &mut Session<MemoryStore>) -> ItemId {
    s.set_tool(Tool::Line("ridge".into()));
    for k in 0..=15 {
        s.add_draft_point(unit(0.0, lon(f64::from(k) * 10_000.0))).unwrap();
    }
    s.finish_draft().unwrap().0
}

// spec: VIEW-033
#[test]
fn the_editor_refuses_an_amount_or_a_width_that_is_not_a_number_or_out_of_range_and_changes_nothing() {
    let mut s = session(1);
    let first = long_ridge(&mut s);
    let hash = s.editor().hash();
    let depth = s.editor().undo_depth();
    // The inspector checks the kind's range; the editor under it must not take what the item rules refuse either.
    for (amount, width, why) in [
        (Some(f64::NAN), None, "amount NaN"),
        (Some(f64::INFINITY), None, "amount infinite"),
        (Some(-5.0), None, "amount negative"),
        (None, Some(f64::NAN), "width NaN"),
        (None, Some(f64::INFINITY), "width infinite"),
        (None, Some(-1.0), "width negative"),
        (None, Some(0.0), "width zero"),
        (None, Some(1.0e9), "width a million metres"),
    ] {
        let r = s.editor_mut().set_line_params(first, amount, width);
        assert!(r.is_err(), "{why} was taken: {r:?}");
        assert_eq!(s.editor().hash(), hash, "{why}: a refused change changes nothing");
        assert_eq!(s.editor().undo_depth(), depth, "{why}: and is not a command");
    }
}

// spec: VIEW-033
#[test]
fn a_line_wider_than_the_part_budget_is_refused_at_the_edge_and_one_just_inside_it_is_split() {
    // A line that does not fit one item (9 km and a wide half-width), so it must be split into parts of a reach of SPLIT_FRACTION of the cell.
    let pts: Vec<GeoPoint> = [0.0, 4_500.0, 9_000.0].iter().map(|m| GeoPoint::new(0.0, lon(*m))).collect();
    let edge = SPLIT_FRACTION * CELL_EDGE_M - 1.0;
    let e = split_line(&pts, edge, R).unwrap_err();
    assert_eq!(e, PolylineError::WidthTooLarge { width_m: edge, limit_m: CELL_EDGE_M }, "no room for a part at the budget");
    let e = split_line(&pts, edge + 100.0, R).unwrap_err();
    assert!(matches!(e, PolylineError::WidthTooLarge { .. }), "{e:?}");
    // Just inside, a part still has a metre or more of reach: the line is split and every part holds two points or more.
    let parts = split_line(&pts, edge - 0.5, R).unwrap();
    assert!(parts.len() > 1 && parts.iter().all(|p| p.len() >= 2), "{} parts", parts.len());
    assert_eq!(parts.first().unwrap()[0], pts[0]);
    assert_eq!(parts.last().unwrap().last().unwrap(), pts.last().unwrap());
    for w in parts.windows(2) {
        assert_eq!(w[0].last(), w[1].first(), "the last point of a part is the first of the next");
    }
}

// spec: VIEW-016
#[test]
fn deleting_a_split_line_clears_the_hover_of_any_of_its_parts_and_an_undo_and_redo_of_it_agree() {
    let mut s = session(2);
    let first = long_ridge(&mut s);
    let parts = s.editor().line_items(first).unwrap();
    assert!(parts.len() >= 4);
    let before = s.editor().hash();

    // The cursor is over a part that is not the selected one when the line is deleted.
    s.select(parts[0]);
    s.set_hover(Some(parts[3]));
    s.delete_selected().unwrap();
    assert_eq!(s.hovered(), None, "the hovered part went with the line");
    assert_eq!(s.selected(), None);
    assert!(parts.iter().all(|p| s.item(*p).is_none()));
    let deleted = s.editor().hash();
    assert_ne!(deleted, before);

    // Undo brings every part back; redo takes them all again, as one command each time, and the hover stays clear.
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), before);
    assert_eq!(s.editor().line_items(first).unwrap(), parts);
    s.set_hover(Some(parts[2]));
    assert!(s.redo().unwrap().is_some());
    assert_eq!(s.editor().hash(), deleted, "redo is the deletion again, whole");
    assert!(parts.iter().all(|p| s.item(*p).is_none()), "no part is left behind");
    assert_eq!(s.hovered(), None, "the hover of a deleted part is dropped by the redo too");
    assert!(s.redo().unwrap().is_none(), "nothing more to redo");
    // And the line can be drawn and deleted again after that.
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), before);
}
