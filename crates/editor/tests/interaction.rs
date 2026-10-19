//! Tier A: what the pointer does on the planet (VIEW-019): hover, press, drag, release, and the tiles an edit sends to refresh.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_editor::commands::{Affected, Editor};
use planet_editor::feed::{edited_tiles, REFRESH_HALO};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::interaction::{Interaction, PointerView, Pressed};
use planet_editor::overlay::{self, OUTLINE_SEGMENTS};
use planet_editor::relevance::ViewState;
use planet_editor::session::{Session, Tool};
use planet_editor::shape::{self, Handle};
use planet_frame::globe::Viewport;
use planet_frame::pick::SurfaceHit;
use planet_frame::Camera;
use planet_world_def::item_store::MemoryStore;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

fn session(seed: u64) -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R)
}

fn hit(dir: Vec3) -> SurfaceHit {
    SurfaceHit { point: dir * R, dir, height_m: 0.0, distance_m: 1_000.0 }
}

fn view<'a>(cursor: [f64; 2], at: Option<Vec3>, shown: &'a [ItemId], handles: &'a [(Handle, [f64; 2])]) -> PointerView<'a> {
    PointerView { cursor, hit: at.map(hit), shown, handles, metres_per_pixel: 0.5 }
}

const HANDLES: [(Handle, [f64; 2]); 3] =
    [(Handle::Move, [100.0, 100.0]), (Handle::Radius, [160.0, 100.0]), (Handle::Height, [100.0, 40.0])];

// spec: VIEW-019
#[test]
fn a_handle_is_taken_within_its_hit_radius_and_the_nearest_wins() {
    let ix = Interaction::new(10.0);
    let none: [ItemId; 0] = [];
    assert_eq!(ix.handle_under(&view([103.0, 104.0], None, &none, &HANDLES)), Some(Handle::Move));
    assert_eq!(ix.handle_under(&view([100.0, 110.0], None, &none, &HANDLES)), Some(Handle::Move), "the radius is inclusive");
    assert_eq!(ix.handle_under(&view([100.0, 110.1], None, &none, &HANDLES)), None);
    assert_eq!(ix.handle_under(&view([158.0, 98.0], None, &none, &HANDLES)), Some(Handle::Radius));
    assert_eq!(ix.handle_under(&view([100.0, 46.0], None, &none, &HANDLES)), Some(Handle::Height));
    let close = [(Handle::Move, [100.0, 100.0]), (Handle::Radius, [106.0, 100.0])];
    assert_eq!(ix.handle_under(&view([105.0, 100.0], None, &none, &close)), Some(Handle::Radius), "the nearer of two within reach");
    assert_eq!(ix.handle_under(&view([100.0, 100.0], None, &none, &[])), None);
}

// spec: VIEW-019
#[test]
fn hovering_picks_the_edit_under_the_cursor_and_a_handle_or_a_held_drag_clears_it() {
    let mut s = session(1);
    let at = unit(47.2, 8.55);
    let (a, _) = s.place("plateau", at).unwrap();
    s.deselect();
    let mut ix = Interaction::new(10.0);
    let shown = [a];
    ix.hover(&mut s, &view([500.0, 500.0], Some(at), &shown, &[]));
    assert_eq!(s.hovered(), Some(a));
    ix.hover(&mut s, &view([500.0, 500.0], Some(unit(-40.0, 100.0)), &shown, &[]));
    assert_eq!(s.hovered(), None, "off every outline");
    ix.hover(&mut s, &view([500.0, 500.0], None, &shown, &[]));
    assert_eq!(s.hovered(), None, "off the planet");
    ix.hover(&mut s, &view([500.0, 500.0], Some(at), &[], &[]));
    assert_eq!(s.hovered(), None, "an edit that is not shown is not picked");
    ix.hover(&mut s, &view([500.0, 500.0], Some(at), &shown, &[]));
    assert_eq!(s.hovered(), Some(a));
    ix.hover(&mut s, &view([100.0, 100.0], Some(at), &shown, &HANDLES));
    assert_eq!(s.hovered(), None, "over a handle");
    s.select(a);
    ix.press(&mut s, &view([100.0, 100.0], Some(at), &shown, &HANDLES)).unwrap();
    s.set_hover(Some(a));
    ix.hover(&mut s, &view([900.0, 900.0], Some(unit(0.0, 0.0)), &shown, &[]));
    assert_eq!(s.hovered(), Some(a), "while a handle is held the hover does not change");
}

// spec: VIEW-019
#[test]
fn a_press_selects_the_edit_under_it_or_clears_the_selection_and_the_stamp_tool_places() {
    let mut s = session(2);
    let at = unit(47.2, 8.55);
    let (a, _) = s.place("hill", at).unwrap();
    s.deselect();
    let mut ix = Interaction::new(10.0);
    let shown = [a];
    let (p, aff) = ix.press(&mut s, &view([500.0, 500.0], Some(at), &shown, &[])).unwrap();
    assert_eq!((p, aff.len(), s.selected()), (Pressed::Selected(a), 0, Some(a)), "a selection is not a command");
    let (p, _) = ix.press(&mut s, &view([500.0, 500.0], Some(unit(-40.0, 100.0)), &shown, &[])).unwrap();
    assert_eq!((p, s.selected()), (Pressed::Cleared, None));
    let (p, _) = ix.press(&mut s, &view([500.0, 500.0], None, &shown, &[])).unwrap();
    assert_eq!(p, Pressed::Cleared, "the sky clears it too");
    // The stamp tool places where the cursor meets the surface, even over another edit; off the planet it does nothing.
    let depth = s.editor().undo_depth();
    s.set_tool(Tool::Stamp("crater".into()));
    let (p, aff) = ix.press(&mut s, &view([500.0, 500.0], None, &shown, &[])).unwrap();
    assert_eq!((p, aff.len(), s.editor().undo_depth()), (Pressed::Nothing, 0, depth));
    assert_eq!(s.tool(), &Tool::Stamp("crater".into()), "the tool stays until a stamp is placed");
    let (p, aff) = ix.press(&mut s, &view([500.0, 500.0], Some(at), &shown, &[])).unwrap();
    let Pressed::Placed(id) = p else { panic!("{p:?}") };
    assert_eq!((aff.len(), s.editor().undo_depth(), s.selected(), s.tool()), (1, depth + 1, Some(id), &Tool::Select));
    assert_eq!(s.item(id).unwrap().kind, "crater");
}

// spec: VIEW-019
#[test]
fn a_press_on_a_handle_holds_it_and_the_moves_make_one_command_on_release() {
    let mut s = session(3);
    let start = unit(47.2, 8.55);
    let (a, _) = s.place("hill", start).unwrap();
    let h0 = s.editor().hash();
    let depth = s.editor().undo_depth();
    let mut ix = Interaction::new(10.0);
    let shown = [a];
    // Move: press on the move handle, drag over the surface, release.
    let (p, aff) = ix.press(&mut s, &view([101.0, 99.0], Some(start), &shown, &HANDLES)).unwrap();
    assert_eq!((p, aff.len(), s.dragging()), (Pressed::Handle(Handle::Move), 0, Some(Handle::Move)));
    let mut last = start;
    for k in 1..=50 {
        last = unit(47.2 + k as f64 * 0.0005, 8.55 + k as f64 * 0.001);
        let aff = ix.moved(&mut s, &view([101.0 + k as f64, 99.0], Some(last), &shown, &HANDLES)).unwrap();
        assert_eq!(aff.len(), 2);
    }
    assert!(ix.moved(&mut s, &view([300.0, 300.0], None, &shown, &HANDLES)).unwrap().is_empty(), "off the planet the handle stays");
    let aff = ix.release(&mut s).unwrap().expect("a drag ended");
    assert_eq!(aff.len(), 2);
    assert_eq!((s.dragging(), s.editor().undo_depth()), (None, depth + 1), "fifty moves, one command");
    assert!(s.item(a).unwrap().direction().dot(last) > 1.0 - 1e-14);
    assert_eq!(ix.release(&mut s).unwrap(), None, "a release with nothing held does nothing");
    // Height: the cursor going up raises by the pixels times the metres per pixel.
    let h1 = s.item(a).unwrap().params["height_m"];
    ix.press(&mut s, &view([100.0, 40.0], Some(start), &shown, &HANDLES)).unwrap();
    assert_eq!(s.dragging(), Some(Handle::Height));
    ix.moved(&mut s, &view([100.0, 30.0], Some(start), &shown, &HANDLES)).unwrap();
    ix.moved(&mut s, &view([100.0, 20.0], Some(start), &shown, &HANDLES)).unwrap();
    assert!(ix.moved(&mut s, &view([100.0, 20.0], Some(start), &shown, &HANDLES)).unwrap().is_empty(), "no vertical move, no change");
    ix.release(&mut s).unwrap();
    assert_eq!(s.item(a).unwrap().params["height_m"], h1 + 10.0, "20 pixels up at half a metre a pixel");
    ix.moved(&mut s, &view([100.0, 10.0], Some(start), &shown, &HANDLES)).unwrap();
    assert_eq!(s.item(a).unwrap().params["height_m"], h1 + 10.0, "moves with no handle held change nothing");
    // Radius.
    ix.press(&mut s, &view([160.0, 100.0], Some(start), &shown, &HANDLES)).unwrap();
    assert_eq!(s.dragging(), Some(Handle::Radius));
    let c = s.item(a).unwrap().direction();
    ix.moved(&mut s, &view([200.0, 100.0], Some(shape::point_at(c, 0.0, 1_000.0, R)), &shown, &HANDLES)).unwrap();
    ix.release(&mut s).unwrap();
    assert!((s.item(a).unwrap().params["radius_m"] - 1_000.0).abs() < 1e-3);
    assert_eq!(s.editor().undo_depth(), depth + 3);
    for _ in 0..3 {
        s.undo().unwrap().unwrap();
    }
    assert_eq!(s.editor().hash(), h0, "three drags, three undos");
}

// spec: VIEW-019
#[test]
fn an_abandoned_drag_goes_back_and_records_nothing() {
    let mut s = session(4);
    let at = unit(47.2, 8.55);
    let (a, _) = s.place("hill", at).unwrap();
    let (h0, depth) = (s.editor().hash(), s.editor().undo_depth());
    let mut ix = Interaction::new(10.0);
    let shown = [a];
    assert_eq!(ix.cancel(&mut s).unwrap(), None, "nothing held, nothing to cancel");
    ix.press(&mut s, &view([100.0, 100.0], Some(at), &shown, &HANDLES)).unwrap();
    ix.moved(&mut s, &view([120.0, 100.0], Some(unit(10.0, 10.0)), &shown, &HANDLES)).unwrap();
    assert_ne!(s.editor().hash(), h0);
    let aff = ix.cancel(&mut s).unwrap().expect("a drag was open");
    assert_eq!(aff.len(), 2);
    assert_eq!((s.editor().hash(), s.editor().undo_depth(), s.dragging()), (h0, depth, None));
}

fn all_tiles(level: u8) -> Vec<TileId> {
    use planet_core::Face;
    let n = 1u32 << level;
    let mut v = Vec::new();
    for face in Face::ALL {
        for x in 0..n {
            for y in 0..n {
                v.push(TileId::new(face, level, x, y).unwrap());
            }
        }
    }
    v
}

// spec: VIEW-019
#[test]
fn the_tiles_an_edit_sends_to_refresh_are_those_its_reach_can_touch() {
    let map = TangentWarp;
    let level = 8;
    let tiles = all_tiles(level);
    let at = unit(47.2, 8.55);
    let aff = [Affected { body: BodyId::PLANET, dir: at, reach_m: 5_000.0 }];
    let found = edited_tiles(&aff, BodyId::PLANET, &tiles, R, &map);
    assert!(!found.is_empty() && found.len() < 40, "a 5 km reach touches a handful of level-8 tiles, got {}", found.len());
    // No false negative: every tile that holds a point of the disc is in.
    for k in 0..64 {
        let p = shape::point_at(at, k as f64 * 360.0 / 64.0, 4_900.0, R);
        let t = TileId::from_direction(&map, p, level);
        assert!(found.contains(&t), "the tile of a point 4.9 km away is in");
    }
    assert!(found.contains(&TileId::from_direction(&map, at, level)));
    // A tile far away is out; the result is sorted and without repeats.
    let far = TileId::from_direction(&map, unit(-40.0, 100.0), level);
    assert!(!found.contains(&far));
    assert!(found.windows(2).all(|w| w[0].raw() < w[1].raw()));
    // The place before and the place after of a move both count.
    let moved = [aff[0], Affected { body: BodyId::PLANET, dir: unit(-40.0, 100.0), reach_m: 5_000.0 }];
    let both = edited_tiles(&moved, BodyId::PLANET, &tiles, R, &map);
    assert!(both.contains(&far) && both.contains(&TileId::from_direction(&map, at, level)));
    // Another body's places do not count; a bigger reach touches more; an empty list finds nothing.
    assert!(edited_tiles(&aff, BodyId(1), &tiles, R, &map).is_empty());
    let big = [Affected { reach_m: 40_000.0, ..aff[0] }];
    assert!(edited_tiles(&big, BodyId::PLANET, &tiles, R, &map).len() > found.len());
    assert!(edited_tiles(&[], BodyId::PLANET, &tiles, R, &map).is_empty());
    assert!(edited_tiles(&aff, BodyId::PLANET, &[], R, &map).is_empty());
    assert!(REFRESH_HALO > 0.0 && !map.id().is_empty());
}

// ---- VIEW-020: the overlay -----------------------------------------------------------------------------------------------------

fn view_above(dir: Vec3, altitude_m: f64) -> ViewState {
    let camera = Camera::at_surface_point(
        PlanetFixed(dir * (R + altitude_m)),
        Vec3::new(0.0, 0.0, 1.0),
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::FRAC_PI_3,
        1.0,
        1.0,
    );
    ViewState { camera, viewport: Viewport { width_px: 1000.0, height_px: 1000.0 }, radius_m: R }
}

// spec: VIEW-020
#[test]
fn the_outline_of_a_shown_edit_is_a_ring_of_pixels_at_its_radius_around_its_centre() {
    let mut s = session(5);
    let at = unit(47.2, 8.55);
    let (a, _) = s.place("hill", at).unwrap();
    let (b, _) = s.place("plateau", unit(47.2, 8.59)).unwrap();
    s.select(a);
    s.set_hover(Some(b));
    let v = view_above(at, 2_000.0);
    let items = overlay::build(&s, &[a, b], &v, &|_| 0.0);
    assert_eq!(items.len(), 2);
    assert_eq!(items.last().unwrap().id, a, "the selected edit is drawn last, on top");
    let sel = items.iter().find(|o| o.id == a).unwrap();
    let other = items.iter().find(|o| o.id == b).unwrap();
    assert_eq!((sel.selected, sel.hovered, other.selected, other.hovered), (true, false, false, true));
    assert_eq!((sel.label.as_str(), sel.kind.as_str()), ("Hill 1", "hill"));
    // 400 m at 2 km up is 400 / 2000 * 500 / tan(30 degrees) pixels from the centre, all the way round.
    let want = 400.0 / 2_000.0 * 500.0 / libm::tan(std::f64::consts::FRAC_PI_6);
    let c = sel.centre.expect("the centre is on the screen");
    assert!((c[0] - 500.0).abs() < 1e-6 && (c[1] - 500.0).abs() < 1e-6, "the camera looks at it: {c:?}");
    assert_eq!(sel.outline.len(), OUTLINE_SEGMENTS);
    for p in &sel.outline {
        let p = p.expect("the whole ring is on the screen");
        let d = libm::hypot(p[0] - c[0], p[1] - c[1]);
        assert!((d - want).abs() < 0.01 * want, "{d} vs {want}");
    }
    // Only the selected edit has handles, and they are on the screen where the shape puts them.
    assert!(other.handles.is_empty());
    assert_eq!(sel.handles.iter().map(|(h, _)| *h).collect::<Vec<_>>(), [Handle::Move, Handle::Radius, Handle::Height]);
    let px = overlay::handle_pixels(&items);
    assert_eq!(px.len(), 3);
    assert!(libm::hypot(px[0].1[0] - c[0], px[0].1[1] - c[1]) < 1e-6, "the move handle is on the centre");
    assert!(libm::hypot(px[1].1[0] - c[0], px[1].1[1] - c[1]) > 0.9 * want, "the radius handle is on the outline");
    assert!(libm::hypot(px[2].1[0] - c[0], px[2].1[1] - c[1]) < 1e-6, "straight down the height handle is behind the move handle");
    // From an oblique view (2 km south of it and 2 km up, looking north-down) the height handle is above the centre on the screen.
    let south = shape::point_at(at, 180.0, 2_000.0, R);
    let oblique = ViewState {
        camera: Camera::at_surface_point(
            PlanetFixed(south * (R + 2_000.0)),
            Vec3::new(0.0, 0.0, 1.0),
            std::f64::consts::FRAC_PI_4,
            std::f64::consts::FRAC_PI_3,
            1.0,
            1.0,
        ),
        ..v
    };
    let o = overlay::build(&s, &[a], &oblique, &|_| 0.0);
    let (oc, oh) = (o[0].centre.unwrap(), o[0].handles.iter().find(|(h, _)| *h == Handle::Height).unwrap().1.unwrap());
    assert!(oh[1] < oc[1] - 5.0, "the height handle is above the centre on the screen: {oc:?} {oh:?}");
    // The ground lifts the outline toward the camera: it grows.
    let high = overlay::build(&s, &[a], &v, &|_| 500.0);
    let grown = high[0].outline[0].unwrap();
    assert!(libm::hypot(grown[0] - c[0], grown[1] - c[1]) > 1.2 * want);
}

// spec: VIEW-020
#[test]
fn points_behind_the_camera_or_off_the_screen_are_none_and_an_unknown_id_is_skipped() {
    let mut s = session(6);
    let at = unit(47.2, 8.55);
    let (a, _) = s.place("plateau", at).unwrap();
    // A camera so near that the 15 km ring leaves the screen on every side.
    let items = overlay::build(&s, &[a, 999], &view_above(at, 300.0), &|_| 0.0);
    assert_eq!(items.len(), 1, "an id with no edit is skipped");
    assert!(items[0].outline.iter().all(|p| p.is_none()), "the ring is outside the screen");
    assert!(items[0].centre.is_some());
    // The other side of the planet is behind the camera.
    let far = overlay::build(&s, &[a], &view_above(unit(-47.2, -171.45), 2_000.0), &|_| 0.0);
    assert!(far[0].centre.is_none() && far[0].outline.iter().all(|p| p.is_none()));
    assert!(overlay::handle_pixels(&far).is_empty());
    assert!(overlay::build(&s, &[], &view_above(at, 2_000.0), &|_| 0.0).is_empty());
}
