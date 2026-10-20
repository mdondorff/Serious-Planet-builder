//! Tier A: the line and region tools over the session (VIEW-029): a draft of clicked points that becomes one line or region, one handle per
//! point of a line (however many items it is split into), a point drag that is one command, and adding or removing a point.

use planet_core::Vec3;
use planet_editor::commands::Editor;
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::interaction::{Interaction, PointerView, Pressed};
use planet_editor::polyline::GeoPoint;
use planet_editor::session::{Session, SessionError, Tool};
use planet_editor::shape::Handle;
use planet_frame::pick::SurfaceHit;
use planet_world_def::item_store::MemoryStore;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    GeoPoint::new(lat_deg, lon_deg).dir()
}

/// The longitude in degrees `m` metres east of 0 on the equator.
fn lon(m: f64) -> f64 {
    m / R * 180.0 / std::f64::consts::PI
}

fn session(seed: u64) -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R)
}

fn hit(dir: Vec3) -> SurfaceHit {
    SurfaceHit { point: dir * R, dir, height_m: 0.0, distance_m: 1_000.0 }
}

fn view<'a>(at: Option<Vec3>, shown: &'a [ItemId], handles: &'a [(Handle, [f64; 2])]) -> PointerView<'a> {
    PointerView { cursor: [0.0, 0.0], hit: at.map(hit), shown, handles, metres_per_pixel: 0.5 }
}

/// Click the points of `m_east` (metres east on the equator) with the tool already on.
fn click_all(s: &mut Session<MemoryStore>, ix: &mut Interaction, m_east: &[f64]) {
    for m in m_east {
        let none: [ItemId; 0] = [];
        ix.press(s, &view(Some(unit(0.0, lon(*m))), &none, &[])).unwrap();
    }
}

fn line_session(points_m: &[f64]) -> (Session<MemoryStore>, ItemId) {
    let mut s = session(3);
    let mut ix = Interaction::new(10.0);
    s.set_tool(Tool::Line("ridge".into()));
    click_all(&mut s, &mut ix, points_m);
    let (id, _) = s.finish_draft().unwrap();
    (s, id)
}

// spec: VIEW-029
#[test]
fn clicks_of_the_line_tool_collect_points_and_finishing_puts_the_line_as_one_command_and_selects_it() {
    let mut s = session(1);
    let mut ix = Interaction::new(10.0);
    let none: [ItemId; 0] = [];
    s.set_tool(Tool::Line("ridge".into()));
    assert_eq!(ix.press(&mut s, &view(Some(unit(0.0, lon(0.0))), &none, &[])).unwrap().0, Pressed::DraftPoint(1));
    assert_eq!(ix.press(&mut s, &view(Some(unit(0.0, lon(5_000.0))), &none, &[])).unwrap().0, Pressed::DraftPoint(2));
    // The same place again is a double click, not a point; off the planet is nothing.
    let e = ix.press(&mut s, &view(Some(unit(0.0, lon(5_000.2))), &none, &[])).unwrap_err();
    assert!(e.to_string().contains("last point again"), "{e}");
    assert_eq!(ix.press(&mut s, &view(None, &none, &[])).unwrap().0, Pressed::Nothing);
    assert_eq!(ix.press(&mut s, &view(Some(unit(0.0, lon(9_000.0))), &none, &[])).unwrap().0, Pressed::DraftPoint(3));
    assert_eq!(s.draft().len(), 3);
    assert_eq!(s.undo_draft_point().unwrap(), 2);
    assert_eq!(s.editor().undo_depth(), 0, "a draft is not a command");
    click_all(&mut s, &mut ix, &[8_000.0]);

    let (id, affected) = s.finish_draft().unwrap();
    assert!(!affected.is_empty());
    assert_eq!((s.selected(), s.tool().clone(), s.draft().len()), (Some(id), Tool::Select, 0));
    assert_eq!(s.editor().undo_depth(), 1, "one command");
    let item = s.item(id).unwrap();
    assert_eq!((item.kind.as_str(), item.layer.as_str(), item.points.len()), ("ridge", "stamps", 3));
    assert_eq!(item.name.as_deref(), Some("Ridge 1"));
    assert_eq!((item.params["amount_m"], item.params["width_m"]), (30.0, 200.0));
    s.undo().unwrap();
    assert!(s.item(id).is_none(), "one undo takes the line back");
}

// spec: VIEW-029
#[test]
fn a_draft_that_cannot_be_finished_stays_so_the_user_can_fix_it_and_changing_the_tool_drops_it() {
    let mut s = session(2);
    let mut ix = Interaction::new(10.0);
    s.set_tool(Tool::Line("channel".into()));
    click_all(&mut s, &mut ix, &[0.0]);
    let e = s.finish_draft().unwrap_err();
    assert!(e.to_string().contains("at least 2 points"), "{e}");
    assert_eq!(s.draft().len(), 1, "the draft is kept");
    s.set_tool(Tool::Select);
    assert!(s.draft().is_empty(), "another tool drops it");

    // A region needs three points and one reach: 50 km across is refused with the limit, and taking a point back lets it through.
    s.set_tool(Tool::Region);
    let corner = |m_north: f64, m_east: f64| unit(m_north / R * 180.0 / std::f64::consts::PI, lon(m_east));
    let none: [ItemId; 0] = [];
    for (n, e) in [(0.0, 0.0), (0.0, 50_000.0), (50_000.0, 50_000.0), (50_000.0, 0.0)] {
        ix.press(&mut s, &view(Some(corner(n, e)), &none, &[])).unwrap();
    }
    let err = s.finish_draft().unwrap_err();
    assert!(err.to_string().contains("40 km"), "{err}");
    assert_eq!(s.draft().len(), 4);
    s.cancel_draft();
    for (n, e) in [(0.0, 0.0), (0.0, 10_000.0)] {
        ix.press(&mut s, &view(Some(corner(n, e)), &none, &[])).unwrap();
    }
    assert!(s.finish_draft().unwrap_err().to_string().contains("at least 3 points"));
    ix.press(&mut s, &view(Some(corner(10_000.0, 10_000.0)), &none, &[])).unwrap();
    let (id, _) = s.finish_draft().unwrap();
    let region = s.item(id).unwrap();
    assert_eq!((region.kind.as_str(), region.layer.as_str(), region.points.len()), ("region", "regions", 3));
    assert_eq!((region.params["surface_type"], region.params["mask_mode"]), (0.0, 0.0));
    assert!(matches!(s.add_draft_point(unit(0.0, 0.0)), Err(SessionError::Refused(_))), "the tool went back to select");
}

// spec: VIEW-029
#[test]
fn a_line_has_one_handle_per_point_and_a_point_drag_is_held_in_the_session_until_one_command_at_the_release() {
    let (mut s, id) = line_session(&[0.0, 5_000.0, 9_000.0]);
    let handles = s.handles(0.0);
    assert_eq!(handles.iter().map(|(h, _)| *h).collect::<Vec<_>>(), vec![Handle::Point(0), Handle::Point(1), Handle::Point(2)]);
    assert!((handles[1].1 - unit(0.0, lon(5_000.0)) * R).length() < 0.01, "a handle sits on its point");

    let (stored, depth) = (s.item(id).unwrap().clone(), s.editor().undo_depth());
    s.begin_drag(Handle::Point(1)).unwrap();
    assert_eq!(s.dragging(), Some(Handle::Point(1)));
    s.drag_surface(unit(0.01, lon(5_000.0))).unwrap();
    s.drag_surface(unit(0.02, lon(5_500.0))).unwrap();
    assert_eq!(s.item(id).unwrap(), &stored, "the editor has heard nothing during the drag");
    assert_eq!(s.editor().undo_depth(), depth);
    let drawn = s.drawn_points(id);
    assert!((drawn[1].lat_deg - 0.02).abs() < 1e-9 && (drawn[0].lat_deg, drawn[2].lat_deg) == (0.0, 0.0), "{drawn:?}");
    assert!((s.handles(0.0)[1].1 - unit(0.02, lon(5_500.0)) * R).length() < 0.01, "the handle follows the pointer");

    let affected = s.end_drag().unwrap();
    assert!(!affected.is_empty());
    assert_eq!(s.editor().undo_depth(), depth + 1, "one command for all the moves");
    assert!((s.item(id).unwrap().points[1].0 - 0.02).abs() < 1e-9);
    assert_eq!(s.dragging(), None);
    s.undo().unwrap();
    assert_eq!(s.item(id).unwrap(), &stored, "one undo puts the point back");

    // Esc: nothing is told to the editor and the drawn points go back.
    s.begin_drag(Handle::Point(2)).unwrap();
    s.drag_surface(unit(0.5, 0.0)).unwrap();
    s.cancel_drag().unwrap();
    assert_eq!((s.item(id).unwrap(), s.editor().undo_depth()), (&stored, depth));
    assert_eq!(s.drawn_points(id).len(), 3);
    assert!((s.drawn_points(id)[2].lat_deg).abs() < 1e-12);
    assert!(s.begin_drag(Handle::Point(3)).is_err(), "no such point");
}

// spec: VIEW-029
#[test]
fn a_drag_the_item_refuses_leaves_it_as_it_was_and_the_pointer_drives_the_whole_flow() {
    let (mut s, id) = line_session(&[0.0, 5_000.0, 9_000.0]);
    let stored = s.item(id).unwrap().clone();
    // Onto the next point: a repeated point, refused at the release.
    s.begin_drag(Handle::Point(0)).unwrap();
    s.drag_surface(unit(0.0, lon(5_000.0))).unwrap();
    let e = s.end_drag().unwrap_err();
    assert!(e.to_string().contains("same place") || e.to_string().contains("repeats"), "{e}");
    assert_eq!((s.item(id).unwrap(), s.dragging()), (&stored, None), "the item is as it was and no drag is open");

    // The same through the pointer: press on a handle, move, release.
    let mut ix = Interaction::new(10.0);
    let shown = [id];
    let handles = [(Handle::Point(0), [100.0, 100.0]), (Handle::Point(1), [200.0, 100.0]), (Handle::Point(2), [300.0, 100.0])];
    let mut v = view(Some(unit(0.0, lon(5_000.0))), &shown, &handles);
    v.cursor = [202.0, 101.0];
    assert_eq!(ix.press(&mut s, &v).unwrap().0, Pressed::Handle(Handle::Point(1)));
    v.hit = Some(hit(unit(0.003, lon(5_100.0))));
    ix.moved(&mut s, &v).unwrap();
    v.hit = Some(hit(unit(0.004, lon(5_200.0))));
    ix.moved(&mut s, &v).unwrap();
    let depth = s.editor().undo_depth();
    assert!(ix.release(&mut s).unwrap().is_some());
    assert_eq!(s.editor().undo_depth(), depth + 1);
    assert!((s.item(id).unwrap().points[1].0 - 0.004).abs() < 1e-9);
}

// spec: VIEW-029
#[test]
fn a_long_line_has_handles_over_all_its_parts_and_a_drag_in_a_later_part_re_splits_it_in_one_command() {
    // 150 km along the equator in 10 km steps: clicked as the draft, split into parts when finished.
    let metres: Vec<f64> = (0..=15).map(|k| f64::from(k) * 10_000.0).collect();
    let (mut s, id) = line_session(&metres);
    let parts = s.editor().line_items(id).unwrap();
    assert!(parts.len() >= 4, "{} parts", parts.len());
    assert_eq!(s.selected(), Some(parts[0]), "the first part is selected");
    let joined = s.editor().line_points(id).unwrap();
    assert!(joined.len() >= metres.len());
    assert_eq!(s.handles(0.0).len(), joined.len(), "a handle for every point of every part");

    let depth = s.editor().undo_depth();
    let hash = s.editor().hash();
    let last = joined.len() - 3;
    s.begin_drag(Handle::Point(last)).unwrap();
    s.drag_surface(unit(0.05, lon(135_000.0))).unwrap();
    s.end_drag().unwrap();
    assert_eq!(s.editor().undo_depth(), depth + 1, "one command for the line, however many parts changed");
    assert!(s.editor().line_points(id).unwrap().len() >= metres.len(), "the line still holds its points");
    assert!(
        s.editor().line_points(id).unwrap().iter().any(|p| (p.lat_deg - 0.05).abs() < 1e-9 && (p.lon_deg - lon(135_000.0)).abs() < 1e-9),
        "the moved point is in the line"
    );
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), hash, "one undo puts every part back");
}

// spec: VIEW-029
#[test]
fn drafts_have_limits_names_count_lines_not_parts_and_history_or_a_new_tool_closes_a_held_point_drag() {
    // 640 points on a line draft, 64 on a region draft; each 3 m from the last.
    let mut s = session(4);
    s.set_tool(Tool::Line("ridge".into()));
    for k in 0..640 {
        s.add_draft_point(unit(0.0, lon(3.0 * f64::from(k)))).unwrap();
    }
    let e = s.add_draft_point(unit(0.0, lon(3.0 * 640.0))).unwrap_err();
    assert!(e.to_string().contains("at most 640"), "{e}");
    s.set_tool(Tool::Region);
    assert!(s.draft().is_empty());
    for k in 0..64 {
        s.add_draft_point(unit(0.0, lon(3.0 * f64::from(k)))).unwrap();
    }
    assert!(s.add_draft_point(unit(0.0, lon(3.0 * 64.0))).unwrap_err().to_string().contains("at most 64"));
    s.cancel_draft();

    // A 150 km ridge is several items but one line: the next ridge is Ridge 2.
    let metres: Vec<f64> = (0..=15).map(|k| f64::from(k) * 10_000.0).collect();
    let (mut s, id) = line_session(&metres);
    assert!(s.editor().line_items(id).unwrap().len() >= 4);
    assert_eq!(s.item(id).unwrap().name.as_deref(), Some("Ridge 1"));
    s.set_tool(Tool::Line("ridge".into()));
    s.add_draft_point(unit(20.0, 0.0)).unwrap();
    s.add_draft_point(unit(20.0, 0.05)).unwrap();
    let (second, _) = s.finish_draft().unwrap();
    assert_eq!(s.item(second).unwrap().name.as_deref(), Some("Ridge 2"));

    // Undo, delete, a deselect or another tool while a point is held close the drag: the held points never reach another state.
    for how in 0..4 {
        s.select(second);
        s.begin_drag(Handle::Point(0)).unwrap();
        s.drag_surface(unit(21.0, 0.0)).unwrap();
        match how {
            0 => drop(s.undo().unwrap()),
            1 => drop(s.delete_selected().unwrap()),
            2 => s.deselect(),
            _ => s.set_tool(Tool::Region),
        }
        assert_eq!(s.dragging(), None, "case {how}");
        assert!(s.end_drag().is_err(), "no drag is left to end (case {how})");
        s.set_tool(Tool::Select);
        // Bring the second line back for the next case.
        if how < 2 {
            s.redo().ok();
            if s.item(second).is_none() {
                s.undo().ok();
            }
        }
        if s.item(second).is_none() {
            break;
        }
    }
}

// spec: VIEW-029
#[test]
fn a_region_point_drag_the_item_refuses_is_closed_and_a_click_on_an_existing_point_adds_nothing() {
    let mut s = session(6);
    let mut ix = Interaction::new(10.0);
    s.set_tool(Tool::Region);
    let d = |m: f64| m / R * 180.0 / std::f64::consts::PI;
    let none: [ItemId; 0] = [];
    for (n, e) in [(0.0, 0.0), (0.0, 10_000.0), (10_000.0, 10_000.0)] {
        ix.press(&mut s, &view(Some(unit(d(n), lon(e))), &none, &[])).unwrap();
    }
    let (id, _) = s.finish_draft().unwrap();
    let stored = s.item(id).unwrap().clone();
    // Point 1 dragged beyond the reach of one item (50 km away): refused at the release, the region as it was.
    s.begin_drag(Handle::Point(1)).unwrap();
    s.drag_surface(unit(0.0, lon(50_000.0))).unwrap();
    let e = s.end_drag().unwrap_err();
    assert!(e.to_string().contains("40 km"), "{e}");
    assert_eq!((s.item(id).unwrap(), s.dragging()), (&stored, None));
    // A click on a point of the outline is not a new point.
    let e = s.insert_point_at(unit(0.0, lon(10_000.0)), 200.0).unwrap_err();
    assert!(e.to_string().contains("point here already"), "{e}");
    assert_eq!(s.item(id).unwrap().points.len(), 3);
}

// spec: VIEW-029
#[test]
fn a_point_is_added_on_the_outline_and_removed_each_as_one_command_and_a_region_keeps_three() {
    let (mut s, id) = line_session(&[0.0, 10_000.0]);
    let depth = s.editor().undo_depth();
    // 40 m beside the middle of the segment: within 100 m, the foot is inserted after point 0.
    let (index, _) = s.insert_point_at(unit(40.0 / R * 180.0 / std::f64::consts::PI, lon(5_000.0)), 100.0).unwrap();
    assert_eq!(index, 1);
    assert_eq!(s.item(id).unwrap().points.len(), 3);
    assert!(s.item(id).unwrap().points[1].0.abs() < 1e-9 && (s.item(id).unwrap().points[1].1 - lon(5_000.0)).abs() < 1e-6, "on the line");
    assert_eq!(s.editor().undo_depth(), depth + 1);
    // 1 km away is not on the outline.
    let e = s.insert_point_at(unit(1_000.0 / R * 180.0 / std::f64::consts::PI, lon(2_000.0)), 100.0).unwrap_err();
    assert!(e.to_string().contains("not on the outline"), "{e}");
    assert_eq!(s.item(id).unwrap().points.len(), 3);

    s.remove_point(1).unwrap();
    assert_eq!(s.item(id).unwrap().points.len(), 2);
    assert_eq!(s.editor().undo_depth(), depth + 2);
    let e = s.remove_point(0).unwrap_err();
    assert!(e.to_string().contains("at least 2"), "{e}");
    assert!(s.remove_point(7).is_err());

    // A region: the closing segment takes a point too, and three points stay.
    let mut r = session(9);
    r.set_tool(Tool::Region);
    let mut ix = Interaction::new(10.0);
    let none: [ItemId; 0] = [];
    let d = |m: f64| m / R * 180.0 / std::f64::consts::PI;
    for (n, e) in [(0.0, 0.0), (0.0, 10_000.0), (10_000.0, 10_000.0)] {
        ix.press(&mut r, &view(Some(unit(d(n), lon(e))), &none, &[])).unwrap();
    }
    let (rid, _) = r.finish_draft().unwrap();
    // The closing segment runs from the last point back to the first; 30 m outside its middle.
    let mid = unit(d(5_000.0), lon(5_000.0) - d(30.0));
    let (index, _) = r.insert_point_at(mid, 200.0).unwrap();
    assert_eq!((index, r.item(rid).unwrap().points.len()), (3, 4));
    r.remove_point(3).unwrap();
    let e = r.remove_point(0).unwrap_err();
    assert!(e.to_string().contains("at least 3"), "{e}");
    assert_eq!(r.item(rid).unwrap().points.len(), 3);
}
