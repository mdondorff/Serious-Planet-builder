//! Tier A: the editor model (VIEW-015 to VIEW-018): what the camera shows, the lists and the search, the handles and their drags as one
//! command each, the typed values, and the layout as data. No window, no GPU.

use planet_core::{PlanetFixed, TileId, Vec3};
use planet_editor::commands::Editor;
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::layout::{Layout, Side, Tab};
use planet_editor::lists;
use planet_editor::params::{self, Unit};
use planet_editor::relevance::{reach_px, relevant_items, ViewState, MIN_REACH_PX};
use planet_editor::session::{height_delta_m, Session, SessionError, Tool};
use planet_editor::shape::{self, Handle};
use planet_frame::globe::Viewport;
use planet_frame::pick::{cursor_ray, pick_surface, PickLimits, SurfaceHit};
use planet_frame::Camera;
use planet_world_def::cells::{cell_of, CELL_EDGE_M};
use planet_world_def::item::{Anchor, Item};
use planet_world_def::item_store::{ItemStore, MemoryStore};
use planet_world_def::{BodyId, ItemId, WorldDefinition};
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;
const FOV: f64 = std::f64::consts::FRAC_PI_3;
const VIEWPORT: Viewport = Viewport { width_px: 1000.0, height_px: 1000.0 };

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

fn session(seed: u64) -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(seed)));
    let editor = Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2);
    Session::new(editor, BodyId::PLANET, R)
}

/// A camera `altitude_m` above the unit direction `dir`, looking straight down.
fn camera_above(dir: Vec3, altitude_m: f64) -> Camera {
    Camera::at_surface_point(PlanetFixed(dir * (R + altitude_m)), Vec3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2, FOV, 1.0, 1.0)
}

fn view(camera: Camera) -> ViewState {
    ViewState { camera, viewport: VIEWPORT, radius_m: R }
}

fn tile_of(dir: Vec3) -> TileId {
    cell_of(R, dir)
}

// ---- VIEW-015: what the camera shows --------------------------------------------------------------------------------------------

// spec: VIEW-015
#[test]
fn the_reach_on_the_screen_is_the_reach_over_the_distance_in_the_units_of_the_field_of_view() {
    let dir = unit(47.2, 8.55);
    let v = view(camera_above(dir, 1_000.0));
    // 100 m at 1,000 m, a 60 degree field of view over 1,000 pixels: 100 / 1000 * 500 / tan(30 degrees).
    let want = 0.1 * 500.0 / libm::tan(FOV / 2.0);
    assert!((reach_px(&v, dir, 100.0) - want).abs() < 1e-6, "{} vs {want}", reach_px(&v, dir, 100.0));
    // Twice as far, half the size; twice the reach, twice the size.
    let far = view(camera_above(dir, 2_000.0));
    assert!((reach_px(&far, dir, 100.0) - want / 2.0).abs() < 1e-6);
    assert!((reach_px(&v, dir, 200.0) - 2.0 * want).abs() < 1e-6);
    // A camera on the point is inside every reach.
    let on = ViewState { camera: Camera::at_surface_point(PlanetFixed(dir * R), Vec3::new(0.0, 0.0, 1.0), 0.0, FOV, 1.0, 1.0), ..v };
    assert!(reach_px(&on, dir, 1.0).is_infinite());
    assert_eq!(MIN_REACH_PX, 8.0, "the spec value");
}

// spec: VIEW-015
#[test]
fn near_the_ground_small_stamps_show_with_the_large_ones_and_from_high_up_the_small_ones_drop_out() {
    let mut s = session(1);
    let dir = unit(47.2, 8.55);
    let (hill, _) = s.place("hill", dir).unwrap();
    let (plateau, _) = s.place("plateau", dir).unwrap();
    s.deselect();
    let tiles = [tile_of(dir)];
    let near = s.shown(&tiles, &view(camera_above(dir, 2_000.0))).unwrap();
    assert_eq!(near, vec![hill.min(plateau), hill.max(plateau)], "both at 2 km");
    let high = s.shown(&tiles, &view(camera_above(dir, 100_000.0))).unwrap();
    assert_eq!(high, vec![plateau], "the hill covers {:.1} px at 100 km", reach_px(&view(camera_above(dir, 100_000.0)), dir, 500.0));
    // The edge: the hill (reach 500 m) is shown while it covers 8 px and not below.
    let at_edge = 500.0 / MIN_REACH_PX * 0.5 * 1000.0 / libm::tan(FOV / 2.0);
    assert!(s.shown(&tiles, &view(camera_above(dir, at_edge * 0.99))).unwrap().contains(&hill));
    assert!(!s.shown(&tiles, &view(camera_above(dir, at_edge * 1.01))).unwrap().contains(&hill));
}

// spec: VIEW-015
#[test]
fn an_edit_shows_only_where_its_footprint_meets_the_visible_tiles_and_the_selected_edit_always_shows() {
    let mut s = session(2);
    let here = unit(47.2, 8.55);
    let there = unit(-30.0, 120.0);
    let (a, _) = s.place("plateau", here).unwrap();
    let (b, _) = s.place("plateau", there).unwrap();
    s.deselect();
    let v = view(camera_above(here, 50_000.0));
    assert_eq!(s.shown(&[tile_of(here)], &v).unwrap(), vec![a], "b is large but its footprint is not in the visible tiles");
    assert!(s.shown(&[], &v).unwrap().is_empty(), "no visible tile, no edit");
    s.select(b);
    let shown = s.shown(&[tile_of(here)], &v).unwrap();
    assert!(shown.contains(&a) && shown.contains(&b), "the selected edit is always shown: {shown:?}");
    // Small and far away it still is.
    let (c, _) = s.place("hill", there).unwrap();
    let high = view(camera_above(here, 2_000_000.0));
    assert!(s.shown(&[], &high).unwrap() == vec![c], "selected, outside the tiles, a few pixels");
    // A hidden edit is not shown unless it is the selected one.
    s.toggle_hidden(a);
    s.toggle_hidden(c);
    let v2 = view(camera_above(here, 50_000.0));
    let shown = s.shown(&[tile_of(here)], &v2).unwrap();
    assert!(!shown.contains(&a) && shown.contains(&c), "{shown:?}");
}

// spec: VIEW-015
#[test]
fn the_shown_set_is_the_footprint_find_and_not_a_scan_of_every_item() {
    // Thousands of items far from the view cost the find nothing: the answer is the same as with none of them.
    let mut s = session(3);
    let here = unit(10.0, 10.0);
    let (a, _) = s.place("plateau", here).unwrap();
    s.deselect();
    let v = view(camera_above(here, 30_000.0));
    let before = s.shown(&[tile_of(here)], &v).unwrap();
    // Forty large plateaus 330 to 700 km away: each covers more than 8 pixels from 30 km up, so only the tiles keep them out.
    for i in 0..40 {
        let far = unit(13.0 + (i % 8) as f64 * 0.5, 10.0 + (i / 8) as f64 * 0.7);
        let (id, _) = s.place("plateau", far).unwrap();
        assert!(reach_px(&v, s.item(id).unwrap().direction(), s.item(id).unwrap().reach_m) > MIN_REACH_PX, "a scan would show it");
    }
    s.deselect();
    assert_eq!(s.shown(&[tile_of(here)], &v).unwrap(), before);
    assert_eq!(before, vec![a]);
    // The function on its own agrees.
    let layers = ["stamps"];
    let direct = relevant_items(s.editor().definition(), BodyId::PLANET, &layers, &[tile_of(here)], &v, MIN_REACH_PX, None).unwrap();
    assert_eq!(direct, vec![a]);
}

// ---- VIEW-016: lists and search -------------------------------------------------------------------------------------------------

fn named(s: &mut Session<MemoryStore>, kind: &str, lat: f64, lon: f64, name: &str) -> ItemId {
    let (id, _) = s.place(kind, unit(lat, lon)).unwrap();
    s.rename_selected(name).unwrap();
    s.deselect();
    id
}

// spec: VIEW-016
#[test]
fn the_nearby_list_has_the_shown_edits_nearest_to_the_camera_first_with_ties_by_id() {
    let mut s = session(4);
    let a = named(&mut s, "hill", 47.20, 8.50, "Hill 7");
    let b = named(&mut s, "crater", 47.19, 8.55, "Ember Crater");
    let c = named(&mut s, "plateau", 47.05, 8.71, "Basalt Plateau");
    let cam = unit(47.19, 8.55) * (R + 5_000.0);
    let rows = s.near(&[a, b, c], cam);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Ember Crater", "Hill 7", "Basalt Plateau"]);
    assert!(rows.windows(2).all(|w| w[0].distance_m <= w[1].distance_m));
    // Only the ids asked for; an id with no item is left out.
    assert_eq!(s.near(&[c, 99], cam).len(), 1);
    // Equal distances order by id.
    let twin_a = named(&mut s, "hill", 10.0, 10.0, "Twin");
    let twin_b = named(&mut s, "hill", 10.0, 10.0, "Twin");
    let rows = s.near(&[twin_b, twin_a], unit(0.0, 0.0) * R);
    assert!((rows[0].distance_m - rows[1].distance_m).abs() == 0.0);
    assert!(rows[0].id < rows[1].id);
}

// spec: VIEW-016
#[test]
fn the_search_is_planet_wide_sorted_by_distance_and_matches_words_in_any_case() {
    let mut s = session(5);
    let a = named(&mut s, "hill", 47.20, 8.50, "Hill 7");
    let b = named(&mut s, "hill", -33.0, 151.0, "Far Hill");
    let c = named(&mut s, "crater", 47.19, 8.55, "Ember Crater");
    let cam = unit(47.2, 8.5) * (R + 1_000.0);
    let hills = s.search("hill", None, cam);
    assert_eq!(hills.iter().map(|r| r.id).collect::<Vec<_>>(), [a, b], "the far one is found too, after the near one");
    assert_eq!(s.search("HILL far", None, cam).iter().map(|r| r.id).collect::<Vec<_>>(), [b], "every word must match, in any case");
    assert_eq!(s.search("", None, cam).len(), 3, "an empty query lists everything");
    assert_eq!(s.search("", Some("crater"), cam).iter().map(|r| r.id).collect::<Vec<_>>(), [c], "the kind filter");
    assert!(s.search("nothing like this", None, cam).is_empty());
    // The id text and the kind match too; an unnamed edit is labelled by its kind and the end of its id.
    let (u, _) = s.place("plateau", unit(0.0, 0.0)).unwrap();
    s.editor_mut().change(u, |it| it.name = None).unwrap();
    let text = planet_world_def::item::ulid_text(u);
    let tail = &text[text.len() - 6..];
    let rows = s.search(tail, None, cam);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, format!("plateau {tail}"));
    // A name is shown trimmed.
    s.editor_mut().change(u, |it| it.name = Some("  Padded  ".into())).unwrap();
    assert_eq!(s.search("padded", None, cam)[0].label, "Padded");
}

// spec: VIEW-016
#[test]
fn the_layers_list_follows_the_layer_stack_and_within_a_layer_the_order_the_edits_apply() {
    let mut s = session(6);
    let ids: Vec<ItemId> = (0..4)
        .map(|i| {
            let (id, _) = s.place(if i % 2 == 0 { "hill" } else { "crater" }, unit(10.0 + i as f64, 20.0)).unwrap();
            id
        })
        .collect();
    s.editor_mut().change(ids[0], |it| it.priority = 5).unwrap();
    s.editor_mut().change(ids[1], |it| it.priority = -1).unwrap();
    s.editor_mut().change(ids[2], |it| it.layer = "splines".into()).unwrap();
    s.editor_mut().change(ids[3], |it| it.layer = "my-layer".into()).unwrap();
    let cam = unit(10.0, 20.0) * R;
    let groups = s.layer_groups(cam);
    assert_eq!(
        groups.iter().map(|g| g.layer.as_str()).collect::<Vec<_>>(),
        ["stamps", "splines", "my-layer"],
        "the stack bottom to top, a layer outside the stack after it"
    );
    let stamps: Vec<ItemId> = groups[0].rows.iter().map(|r| r.id).collect();
    assert_eq!(stamps, [ids[1], ids[0]], "priority -1 then 5");
    let chain: Vec<ItemId> = s.chain(cam).iter().map(|r| r.id).collect();
    assert_eq!(
        chain,
        [ids[1], ids[0], ids[2], ids[3]],
        "layer first, then priority, then id: a low priority in a higher layer still applies later"
    );
    s.editor_mut().change(ids[2], |it| it.priority = -100).unwrap();
    let chain: Vec<ItemId> = s.chain(cam).iter().map(|r| r.id).collect();
    assert_eq!(chain, [ids[1], ids[0], ids[2], ids[3]], "priority never reorders layers");
    assert_eq!(lists::LAYER_STACK, ["base", "imported", "intent", "stamps", "splines", "placed", "frozen"]);
}

// ---- VIEW-017: selection, handles, drags, typed values ---------------------------------------------------------------------------

fn item_at(lat: f64, lon: f64, radius_m: f64, height_m: Option<f64>, heading: f64) -> Item {
    let mut params = BTreeMap::from([("radius_m".to_string(), radius_m)]);
    if let Some(h) = height_m {
        params.insert("height_m".to_string(), h);
    }
    Item {
        id: 1,
        body: BodyId::PLANET,
        surface: "sphere".into(),
        layer: "stamps".into(),
        priority: 0,
        kind: "hill".into(),
        anchor: Anchor { latitude_deg: lat, longitude_deg: lon, height_m: 0.0, heading_deg: heading },
        reach_m: radius_m * 1.25,
        max_delta_m: height_m.unwrap_or(0.0),
        params,
        base_hash: 0,
        generator_version: 4,
        stage_version: 2,
        name: None,
        notes: None,
    }
}

// spec: VIEW-017
#[test]
fn the_outline_is_the_radius_parameter_or_the_reach_and_the_handles_follow_the_parameters() {
    let it = item_at(10.0, 20.0, 400.0, Some(80.0), 0.0);
    assert_eq!(shape::outline_radius_m(&it), 400.0);
    assert_eq!(shape::handles_of(&it), [Handle::Move, Handle::Radius, Handle::Height]);
    let mut crater = it.clone();
    crater.params.remove("height_m");
    assert_eq!(shape::handles_of(&crater), [Handle::Move, Handle::Radius], "no height parameter, no height handle");
    let mut bare = crater.clone();
    bare.params.clear();
    assert_eq!(shape::outline_radius_m(&bare), bare.reach_m, "without a radius the reach is the outline");
    bare.reach_m = 0.0;
    assert_eq!(shape::handles_of(&bare), [Handle::Move], "a point has only its anchor");
    let mut bad = it.clone();
    bad.params.insert("radius_m".into(), -3.0);
    assert_eq!(shape::outline_radius_m(&bad), bad.reach_m, "a radius that is not positive is not an outline");
}

// spec: VIEW-017
#[test]
fn the_handles_sit_on_the_anchor_on_the_outline_along_the_heading_and_above_by_the_height() {
    for heading in [0.0, 90.0, 217.0] {
        let it = item_at(47.2, 8.55, 400.0, Some(80.0), heading);
        let centre = it.direction();
        let mv = shape::handle_point(&it, Handle::Move, R, 12.0);
        assert!((mv.length() - (R + 12.0)).abs() < 1e-6 && mv.normalized().dot(centre) > 1.0 - 1e-15);
        let rad = shape::handle_point(&it, Handle::Radius, R, 12.0);
        let arc = R * libm::atan2(centre.cross(rad.normalized()).length(), centre.dot(rad.normalized()));
        assert!((arc - 400.0).abs() < 1e-3, "heading {heading}: arc {arc}");
        let h = shape::handle_point(&it, Handle::Height, R, 12.0);
        assert!((h.length() - (R + 12.0 + 80.0)).abs() < 1e-6);
    }
    // Heading 0 is north (latitude grows), 90 east (longitude grows).
    let it = item_at(0.0, 0.0, 1_000.0, None, 0.0);
    assert!(shape::lat_lon_of(shape::point_at(it.direction(), 0.0, 1_000.0, R)).0 > 0.0);
    let east = shape::lat_lon_of(shape::point_at(it.direction(), 90.0, 1_000.0, R));
    assert!(east.1 > 0.0 && east.0.abs() < 1e-9, "{east:?}");
}

// spec: VIEW-017
#[test]
fn a_direction_has_one_latitude_and_longitude_spelling_that_the_item_file_takes() {
    for (lat, lon) in [(47.2, 8.55), (-33.0, 151.0), (0.0, 0.0), (10.0, 180.0), (-80.0, -179.999), (89.9999, 45.0)] {
        let (la, lo) = shape::lat_lon_of(unit(lat, lon));
        assert!((la - lat).abs() < 1e-9 && (lo - lon).abs() < 1e-9, "({lat}, {lon}) -> ({la}, {lo})");
    }
    assert_eq!(shape::lat_lon_of(unit(90.0, 33.0)), (90.0, 0.0), "0 at a pole");
    assert_eq!(shape::lat_lon_of(unit(-90.0, 33.0)), (-90.0, 0.0));
    let anti = shape::lat_lon_of(Vec3::new(-1.0, 0.0, 0.0));
    assert_eq!(anti, (0.0, 180.0), "the antimeridian is +180");
    let anti2 = shape::lat_lon_of(Vec3::new(-1.0, -0.0, 0.0));
    assert_eq!(anti2.1, 180.0);
    // Whatever the drag made is a valid item.
    for (lat, lon) in [(90.0, 0.0), (0.0, 180.0), (-90.0, 0.0), (12.5, -179.5)] {
        let moved = shape::moved(&item_at(1.0, 1.0, 400.0, Some(80.0), 10.0), unit(lat, lon));
        moved.validate().unwrap_or_else(|e| panic!("({lat}, {lon}): {e}"));
    }
}

// spec: VIEW-017
#[test]
fn a_handle_drag_makes_the_item_the_pointer_says_and_keeps_the_bounds_true() {
    let it = item_at(47.2, 8.55, 400.0, Some(80.0), 0.0);
    // Move: the anchor is under the pointer; nothing else changes.
    let p = unit(47.21, 8.56);
    let moved = shape::moved(&it, p);
    assert!(moved.direction().dot(p) > 1.0 - 1e-14);
    assert_eq!((moved.params.clone(), moved.reach_m, moved.anchor.heading_deg), (it.params.clone(), it.reach_m, it.anchor.heading_deg));
    // Resize (a hill: 50 m to 2 km): the radius is the arc to the pointer; the reach grows to hold it with its margin and does not shrink.
    let out = shape::point_at(it.direction(), 33.0, 1_500.0, R);
    let big = shape::resized(&it, out, R);
    assert!((big.params["radius_m"] - 1_500.0).abs() < 1e-3);
    assert!((big.reach_m - big.params["radius_m"] * shape::REACH_FACTOR).abs() < 1e-9, "the reach holds the outline with the margin");
    let small = shape::resized(&it, shape::point_at(it.direction(), 33.0, 100.0, R), R);
    assert!((small.params["radius_m"] - 100.0).abs() < 1e-3 && small.reach_m == it.reach_m, "a smaller outline keeps the reach");
    let none = shape::resized(&it, it.direction(), R);
    assert_eq!(none.params["radius_m"], 50.0, "never below the kind's least radius");
    let huge = shape::resized(&it, shape::point_at(it.direction(), 0.0, 90_000.0, R), R);
    assert_eq!(huge.params["radius_m"], 2_000.0, "never beyond the kind's greatest radius, the range a typed value is held to");
    assert!(huge.reach_m >= huge.params["radius_m"]);
    // Raise (a hill: 5 m to 400 m): the height stays in the range and the bound on the change holds it, never shrinking.
    let up = shape::raised(&it, 40.0);
    assert_eq!((up.params["height_m"], up.max_delta_m), (120.0, 120.0));
    let down = shape::raised(&it, -200.0);
    assert_eq!((down.params["height_m"], down.max_delta_m), (5.0, 80.0), "the least height of the kind; the bound does not shrink");
    let top = shape::raised(&it, 1.0e9);
    assert_eq!((top.params["height_m"], top.max_delta_m), (400.0, 400.0));
    // A kind the table does not know: 1 m to one cell edge, and a height within the limit on the change, whose bound holds it.
    let mut mesa = it.clone();
    mesa.kind = "mesa".into();
    let huge = shape::resized(&mesa, shape::point_at(mesa.direction(), 0.0, 90_000.0, R), R);
    assert_eq!((huge.params["radius_m"], huge.reach_m), (CELL_EDGE_M, CELL_EDGE_M));
    assert_eq!(shape::resized(&mesa, mesa.direction(), R).params["radius_m"], shape::MIN_RADIUS_M);
    for delta in [1.0e9, -1.0e9] {
        let r = shape::raised(&mesa, delta);
        assert_eq!(r.params["height_m"].abs(), planet_world_def::item::MAX_DELTA_LIMIT_M);
        assert!(r.params["height_m"].abs() <= r.max_delta_m, "the bound holds the height");
        r.validate().unwrap();
    }
}

// spec: VIEW-017
#[test]
fn one_pixel_of_the_screen_is_a_known_length_at_a_distance_and_dragging_up_raises() {
    let dir = unit(47.2, 8.55);
    let cam = camera_above(dir, 1_000.0);
    let mpp = shape::metres_per_pixel(&cam, 1000.0, dir * R);
    assert!((mpp - 2.0 * 1_000.0 * libm::tan(FOV / 2.0) / 1000.0).abs() < 1e-9, "{mpp}");
    assert!(shape::metres_per_pixel(&cam, 1000.0, cam.position.0).is_finite(), "a point at the camera is one metre away");
    assert!(height_delta_m(-10.0, mpp) > 0.0 && height_delta_m(10.0, mpp) < 0.0);
    assert!((height_delta_m(-10.0, 1.5) - 15.0).abs() < 1e-12);
}

fn hit_at(dir: Vec3) -> SurfaceHit {
    // A hit on the bare sphere: the real picker through a camera looking at `dir`.
    let cam = camera_above(dir, 10_000.0);
    let ray = cursor_ray(&cam, 500.0, 500.0, 1000.0, 1000.0);
    pick_surface(&ray, &PickLimits::new(R, 100.0), &|_| 0.0).expect("the ray meets the sphere")
}

// spec: VIEW-017
#[test]
fn the_item_under_the_pointer_is_the_nearest_in_its_own_radius_among_the_shown_ones() {
    let mut s = session(7);
    let centre = unit(47.2, 8.55);
    let (big, _) = s.place("plateau", centre).unwrap();
    let hill_at = shape::point_at(centre, 90.0, 100.0, R);
    let (small, _) = s.place("hill", hill_at).unwrap();
    s.deselect();
    let shown = [big, small];
    assert_eq!(s.pick(&hit_at(hill_at), &shown), Some(small), "a small item inside a large one wins near its centre");
    let edge = shape::point_at(centre, 0.0, 5_000.0, R);
    assert_eq!(s.pick(&hit_at(edge), &shown), Some(big), "beyond the small one the large one is under the pointer");
    assert_eq!(s.pick(&hit_at(unit(-10.0, 100.0)), &shown), None);
    assert_eq!(s.pick(&hit_at(hill_at), &[big]), Some(big), "an item that is not shown cannot be picked");
    assert_eq!(s.pick(&hit_at(hill_at), &[]), None);
    assert_eq!(s.pick(&hit_at(hill_at), &[12345]), None, "an id with no item is skipped");
}

// spec: VIEW-017
#[test]
fn placing_a_stamp_makes_one_selected_command_with_the_kinds_defaults_and_undo_takes_it_away() {
    let mut s = session(8);
    let h0 = s.editor().hash();
    s.set_tool(Tool::Stamp("crater".into()));
    let dir = unit(47.19, 8.55);
    let (_, affected) = s.place("crater", dir).unwrap();
    assert_eq!(affected.len(), 1, "a put has only the place after");
    let id = s.selected().expect("the new stamp is selected");
    assert_eq!(s.tool(), &Tool::Select, "the tool goes back to select");
    let it = s.item(id).unwrap().clone();
    assert_eq!((it.kind.as_str(), it.layer.as_str()), ("crater", "stamps"));
    assert_eq!(it.params["radius_m"], 2_000.0);
    assert_eq!((it.params["depth_m"], it.params["rim_m"]), (250.0, 40.0));
    assert_eq!(it.reach_m, 2_500.0, "the reach holds the outline with a margin");
    assert_eq!(it.max_delta_m, 250.0, "the bound holds the deepest parameter");
    assert_eq!(it.name.as_deref(), Some("Crater 1"));
    assert!(it.direction().dot(dir) > 1.0 - 1e-14);
    assert_eq!(s.editor().store().get(BodyId::PLANET, id).unwrap(), Some(it.clone()), "written through");
    assert_ne!(s.editor().hash(), h0);
    assert_eq!(s.status(), "Crater 1 placed");
    let (second, _) = s.place("crater", dir).unwrap();
    assert_eq!(s.item(second).unwrap().name.as_deref(), Some("Crater 2"));
    s.undo().unwrap().unwrap();
    s.undo().unwrap().unwrap();
    assert_eq!(s.editor().hash(), h0, "the exact starting definition");
    assert_eq!(s.selected(), None, "the selected edit is gone, so is the selection");
    assert!(matches!(s.place("volcano", dir), Err(SessionError::Refused(_))));
    assert_eq!(s.editor().hash(), h0);
}

// spec: VIEW-017
#[test]
fn a_typed_value_is_one_command_in_its_unit_and_a_value_out_of_range_or_not_a_number_changes_nothing() {
    let mut s = session(9);
    s.place("crater", unit(47.19, 8.55)).unwrap();
    let id = s.selected().unwrap();
    let h0 = s.editor().hash();
    let depth0 = s.editor().undo_depth();
    s.set_param_text("radius_m", "3.5 km").unwrap();
    assert_eq!(s.item(id).unwrap().params["radius_m"], 3_500.0);
    assert!(s.item(id).unwrap().reach_m >= 3_500.0, "the reach follows the radius");
    s.set_param_text("depth_m", "300").unwrap();
    assert_eq!(s.item(id).unwrap().params["depth_m"], 300.0, "a bare number is in the unit shown");
    assert_eq!(s.item(id).unwrap().max_delta_m, 300.0);
    assert_eq!(s.editor().undo_depth(), depth0 + 2);
    let h1 = s.editor().hash();
    for (key, text) in [
        ("radius_m", "20 km"),
        ("radius_m", "10"),
        ("depth_m", "-5"),
        ("radius_m", "abc"),
        ("radius_m", ""),
        ("radius_m", "5 kg"),
        ("nope", "1"),
        ("depth_m", "NaN"),
    ] {
        let r = s.set_param_text(key, text);
        assert!(matches!(r, Err(SessionError::Refused(_))), "{key}={text}: {r:?}");
        assert_eq!(s.editor().hash(), h1, "{key}={text} changed nothing");
    }
    assert_eq!(s.editor().undo_depth(), depth0 + 2);
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(s.editor().hash(), h0);
    // Nothing selected: refused.
    s.deselect();
    assert!(matches!(s.set_param_text("radius_m", "1 km"), Err(SessionError::Refused(_))));
    // A parameter the table does not know is edited too, without a range.
    s.select(id);
    s.editor_mut()
        .change(id, |it| {
            it.params.insert("wobble".into(), 2.0);
        })
        .unwrap();
    s.set_param_text("wobble", "7.5").unwrap();
    assert_eq!(s.item(id).unwrap().params["wobble"], 7.5);
    // The limits of the item file hold where the table has no range: the outline within one cell edge, a change within the bound.
    s.editor_mut().change(id, |it| it.kind = "mesa".into()).unwrap();
    for (key, text) in [("radius_m", "100 km"), ("radius_m", "0"), ("radius_m", "-3"), ("wobble_m", "1")] {
        let r = s.set_param_text(key, text);
        assert!(matches!(r, Err(SessionError::Refused(_))), "{key}={text}: {r:?}");
    }
    s.editor_mut()
        .change(id, |it| {
            it.params.insert("lift_m".into(), 1.0);
        })
        .unwrap();
    let h = s.editor().hash();
    assert!(matches!(s.set_param_text("lift_m", "1000000"), Err(SessionError::Refused(_))));
    assert_eq!(s.editor().hash(), h);
    s.set_param_text("radius_m", "30 km").unwrap();
    s.set_param_text("lift_m", "5000").unwrap();
    let it = s.item(id).unwrap();
    assert_eq!((it.params["radius_m"], it.reach_m, it.max_delta_m), (30_000.0, 37_500.0, 5_000.0));
    assert!(shape::outline_radius_m(it) <= it.reach_m && it.params["lift_m"].abs() <= it.max_delta_m);
}

// spec: VIEW-017
#[test]
fn a_whole_drag_of_a_handle_is_one_undo_step_and_a_cancelled_drag_is_none() {
    let mut s = session(10);
    s.place("hill", unit(47.2, 8.55)).unwrap();
    let id = s.selected().unwrap();
    let h0 = s.editor().hash();
    let depth0 = s.editor().undo_depth();
    // Move: a hundred pointer moves.
    s.begin_drag(Handle::Move).unwrap();
    assert_eq!(s.dragging(), Some(Handle::Move));
    let mut last = unit(47.2, 8.55);
    for k in 1..=100 {
        last = unit(47.2 + k as f64 * 0.001, 8.55 + k as f64 * 0.002);
        let affected = s.drag_surface(last).unwrap();
        assert_eq!(affected.len(), 2, "the place before and the place after");
    }
    assert!(s.undo().is_err() && s.place("hill", last).is_err(), "no other command while a handle is held");
    s.end_drag().unwrap();
    assert_eq!(s.editor().undo_depth(), depth0 + 1, "one command");
    assert!(s.item(id).unwrap().direction().dot(last) > 1.0 - 1e-14);
    assert_eq!(s.status(), "Hill 1 moved");
    // Resize and raise as well.
    let c = s.item(id).unwrap().direction();
    s.begin_drag(Handle::Radius).unwrap();
    s.drag_surface(shape::point_at(c, 10.0, 700.0, R)).unwrap();
    s.drag_surface(shape::point_at(c, 10.0, 900.0, R)).unwrap();
    s.end_drag().unwrap();
    assert!((s.item(id).unwrap().params["radius_m"] - 900.0).abs() < 1e-3);
    s.begin_drag(Handle::Height).unwrap();
    assert!(s.drag_surface(c).is_err(), "the height handle does not follow the surface");
    s.drag_height(10.0).unwrap();
    s.drag_height(20.0).unwrap();
    s.end_drag().unwrap();
    assert_eq!(s.item(id).unwrap().params["height_m"], 110.0, "80 and the 30 the drag raised it by");
    assert_eq!(s.editor().undo_depth(), depth0 + 3);
    for _ in 0..3 {
        s.undo().unwrap().unwrap();
    }
    assert_eq!(s.editor().hash(), h0, "three drags, three undos, the exact start");
    // A cancelled drag goes back and records nothing.
    let depth = s.editor().undo_depth();
    s.begin_drag(Handle::Move).unwrap();
    s.drag_surface(unit(10.0, 10.0)).unwrap();
    s.cancel_drag().unwrap();
    assert_eq!((s.editor().hash(), s.editor().undo_depth(), s.dragging()), (h0, depth, None));
    // Refusals.
    assert!(s.end_drag().is_err() && s.cancel_drag().is_err() && s.drag_surface(c).is_err() && s.drag_height(1.0).is_err());
    s.editor_mut()
        .change(id, |it| {
            it.params.remove("height_m");
        })
        .unwrap();
    assert!(matches!(s.begin_drag(Handle::Height), Err(SessionError::Refused(_))), "this edit has no height handle");
    s.deselect();
    assert!(matches!(s.begin_drag(Handle::Move), Err(SessionError::Refused(_))));
}

// spec: VIEW-017
#[test]
fn delete_and_undo_and_the_handles_of_the_selected_edit() {
    let mut s = session(11);
    s.place("plateau", unit(47.2, 8.55)).unwrap();
    let id = s.selected().unwrap();
    let hs = s.handles(0.0);
    assert_eq!(hs.iter().map(|(h, _)| *h).collect::<Vec<_>>(), [Handle::Move, Handle::Radius, Handle::Height]);
    let h_before = s.editor().hash();
    let removed = s.delete_selected().unwrap();
    assert_eq!(removed.len(), 1, "a delete has only the place before");
    assert_eq!(s.selected(), None);
    assert!(s.handles(0.0).is_empty());
    assert!(s.status().contains("deleted"));
    assert!(matches!(s.delete_selected(), Err(SessionError::Refused(_))));
    s.undo().unwrap().unwrap();
    assert!(s.item(id).is_some());
    assert_eq!(s.editor().hash(), h_before);
    s.redo().unwrap().unwrap();
    assert!(s.item(id).is_none() && s.status() == "redone");
}

// spec: VIEW-017
#[test]
fn the_inspector_shows_the_selected_edit_in_metric_values_with_the_ranges_of_its_kind() {
    let mut s = session(12);
    s.place("hill", unit(47.2, 8.55)).unwrap();
    let hill = s.selected().unwrap();
    s.place("hill", unit(47.3, 8.55)).unwrap();
    s.select(hill);
    s.editor_mut()
        .change(hill, |it| {
            it.params.insert("zzz_m".into(), 12.0);
            it.params.insert("aaa".into(), 0.5);
        })
        .unwrap();
    assert!(s.inspector(Vec3::new(R, 0.0, 0.0)).is_some());
    s.deselect();
    assert_eq!(s.inspector(Vec3::new(R, 0.0, 0.0)), None, "nothing selected, no inspector");
    s.select(hill);
    let cam = unit(47.2, 8.55) * (R + 2_400.0);
    let i = s.inspector(cam).unwrap();
    assert_eq!((i.kind.as_str(), i.layer.as_str(), i.title.as_str()), ("Hill", "stamps", "Hill 1"));
    assert_eq!(i.latitude, "47.20000° N");
    assert_eq!(i.longitude, "8.55000° E");
    assert_eq!(i.distance, "2.4 km");
    assert_eq!(i.order, "1 of 2 in its layer");
    assert_eq!(i.reach, "500 m");
    let rows: Vec<(&str, &str)> = i.params.iter().map(|p| (p.key.as_str(), p.text.as_str())).collect();
    assert_eq!(
        rows,
        [("radius_m", "400 m"), ("height_m", "80 m"), ("aaa", "0.5"), ("zzz_m", "12 m")],
        "the table's order, then the others by name"
    );
    assert_eq!(i.params[0].range, Some((50.0, 2_000.0)));
    assert_eq!(i.params[2].range, None);
    assert_eq!((i.params[0].unit, i.params[2].unit, i.params[3].unit), (Unit::Metres, Unit::Plain, Unit::Metres));
}

// spec: VIEW-017
#[test]
fn lengths_are_typed_and_shown_in_metres_or_kilometres_and_a_bad_input_says_why() {
    for (text, unit, want) in [
        ("450", Unit::Metres, 450.0),
        ("450 m", Unit::Metres, 450.0),
        ("2.4 km", Unit::Metres, 2_400.0),
        ("2.4KM", Unit::Metres, 2_400.0),
        ("  12 ", Unit::Metres, 12.0),
        ("-3.5", Unit::Metres, -3.5),
        ("40 %", Unit::Percent, 40.0),
        ("7", Unit::Plain, 7.0),
    ] {
        assert_eq!(params::parse_value(text, unit), Ok(want), "'{text}'");
    }
    for (text, unit) in [
        ("", Unit::Metres),
        ("abc", Unit::Metres),
        ("5 kg", Unit::Metres),
        ("5 %", Unit::Metres),
        ("5 m", Unit::Percent),
        ("1e3", Unit::Metres),
        ("NaN", Unit::Metres),
        ("inf", Unit::Metres),
        ("--5", Unit::Metres),
    ] {
        assert!(params::parse_value(text, unit).is_err(), "'{text}' should be refused");
    }
    assert!(params::parse_value("5 kg", Unit::Metres).unwrap_err().contains("kg"));
    for (v, want) in [
        (450.0, "450 m"),
        (0.0, "0 m"),
        (12.34, "12.3 m"),
        (1_000.0, "1 km"),
        (2_400.0, "2.4 km"),
        (1_234.5, "1.23 km"),
        (-2_400.0, "-2.4 km"),
        (999.96, "1 km"),
        (-0.04, "0 m"),
    ] {
        assert_eq!(params::format_metres(v), want);
    }
    assert_eq!(params::format_value(40.0, Unit::Percent), "40 %");
    assert_eq!(params::kind_title("crater"), "Crater");
    assert_eq!(params::kind_title(""), "");
    // The table: three kinds, each with a radius.
    for kind in params::STAMP_KINDS {
        let defs = params::defs_for(kind);
        assert!(defs.iter().any(|d| d.key == "radius_m"), "{kind}");
        for d in defs {
            assert!(d.min <= d.default && d.default <= d.max && d.step > 0.0, "{kind}.{}", d.key);
        }
    }
    assert!(params::defs_for("volcano").is_empty() && params::def_of("hill", "nope").is_none());
}

// spec: VIEW-017
#[test]
fn a_place_that_is_not_a_direction_is_refused_and_a_selection_waits_for_the_drag() {
    let mut s = session(13);
    let (a, _) = s.place("hill", unit(47.2, 8.55)).unwrap();
    let (b, _) = s.place("hill", unit(47.3, 8.55)).unwrap();
    let h = s.editor().hash();
    for bad in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(f64::NAN, 0.0, 0.0), Vec3::new(f64::INFINITY, 0.0, 1.0)] {
        assert!(matches!(s.place("hill", bad), Err(SessionError::Refused(_))), "{bad:?}");
    }
    s.select(a);
    s.begin_drag(Handle::Move).unwrap();
    for bad in [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, f64::NAN, 0.0)] {
        assert!(matches!(s.drag_surface(bad), Err(SessionError::Refused(_))), "{bad:?}");
    }
    s.select(b);
    assert_eq!(s.selected(), Some(a), "the selection does not change while a handle is held");
    s.cancel_drag().unwrap();
    s.select(b);
    assert_eq!(s.selected(), Some(b));
    assert_eq!(s.editor().hash(), h);
}

// spec: VIEW-017
#[test]
fn heading_zero_at_a_pole_is_the_limit_along_longitude_zero_and_does_not_jump() {
    for (lat, near) in [(90.0, 90.0 - 1e-6), (-90.0, -90.0 + 1e-6)] {
        let at = shape::tangent_frame(unit(lat, 0.0)).0;
        let next = shape::tangent_frame(unit(near, 0.0)).0;
        assert!(at.dot(next) > 0.999_999, "lat {lat}: {at:?} vs {next:?}");
    }
    assert!(shape::tangent_frame(unit(90.0, 0.0)).0.dot(Vec3::new(-1.0, 0.0, 0.0)) > 1.0 - 1e-12, "north at the north pole is -x");
    assert!(shape::tangent_frame(unit(-90.0, 0.0)).0.dot(Vec3::new(1.0, 0.0, 0.0)) > 1.0 - 1e-12, "north at the south pole is +x");
    let (north, east) = shape::tangent_frame(unit(89.999_999_99, 137.0));
    assert!(north.dot(unit(89.999_999_99, 137.0)).abs() < 1e-9 && east.dot(north).abs() < 1e-9, "a frame, a hair from the pole");
}

// ---- VIEW-018: the layout as data ------------------------------------------------------------------------------------------------

// spec: VIEW-018
#[test]
fn the_ends_of_every_range_are_taken_and_a_hair_beyond_them_is_refused() {
    for (key, lo, hi) in
        [("rail_width", 0.0, 200.0), ("left_width", 120.0, 800.0), ("inspector_width", 120.0, 800.0), ("handle_hit_px", 3.0, 60.0)]
    {
        for v in [lo, hi] {
            assert!(Layout::from_toml(&format!("{key} = {v}\n")).is_ok(), "{key} = {v}");
        }
        for v in [lo - 0.001, hi + 0.001] {
            assert_eq!(Layout::from_toml(&format!("{key} = {v}\n")).unwrap_err().key, key, "{key} = {v}");
        }
    }
    assert_eq!(Layout::from_toml("rail_width = 0\n").unwrap().rail_width, 0.0, "0 hides the rail");
}

// spec: VIEW-018
#[test]
fn the_layout_has_defaults_and_a_file_changes_only_the_keys_it_names() {
    let d = Layout::default();
    assert_eq!((d.rail_width, d.left_width, d.inspector_width), (56.0, 300.0, 320.0));
    assert_eq!((d.inspector_side, d.status_bar, d.handle_hit_px), (Side::Right, true, 10.0));
    assert_eq!(d.tabs, [Tab::Near, Tab::Layers, Tab::Graph]);
    assert_eq!(Layout::from_toml("").unwrap(), d);
    let l = Layout::from_toml(
        "left_width = 360\ninspector_side = \"left\"\ntabs = [\"graph\", \"near\"]\nstatus_bar = false\nhandle_hit_px = 14.5\nrail_width = 0\n",
    )
    .unwrap();
    assert_eq!((l.left_width, l.inspector_width, l.rail_width), (360.0, 320.0, 0.0));
    assert_eq!((l.inspector_side, l.status_bar, l.handle_hit_px), (Side::Left, false, 14.5));
    assert_eq!(l.tabs, [Tab::Graph, Tab::Near]);
    assert_eq!((Tab::Near.title(), Tab::Layers.key(), Tab::Graph.title()), ("Near", "layers", "Graph"));
}

// spec: VIEW-018
#[test]
fn a_layout_file_with_an_unknown_key_a_bad_value_or_a_bad_tab_list_is_refused_naming_the_key() {
    for (text, key) in [
        ("colour = 3\n", "colour"),
        ("left_width = 50\n", "left_width"),
        ("left_width = 5000\n", "left_width"),
        ("left_width = \"wide\"\n", "left_width"),
        ("inspector_width = nan\n", "inspector_width"),
        ("rail_width = -1\n", "rail_width"),
        ("handle_hit_px = 1\n", "handle_hit_px"),
        ("inspector_side = \"top\"\n", "inspector_side"),
        ("inspector_side = 3\n", "inspector_side"),
        ("status_bar = \"yes\"\n", "status_bar"),
        ("tabs = []\n", "tabs"),
        ("tabs = [\"near\", \"near\"]\n", "tabs"),
        ("tabs = [\"map\"]\n", "tabs"),
        ("tabs = [1]\n", "tabs"),
        ("tabs = \"near\"\n", "tabs"),
        ("left_width = = 3\n", "(file)"),
    ] {
        let e = Layout::from_toml(text).unwrap_err();
        assert_eq!(e.key, key, "{text:?}: {e}");
        assert!(e.to_string().contains(key));
    }
}
