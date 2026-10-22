//! Tier A: the edits the camera shows are bounded (VIEW-051, M7 edit-frame-scale): the ones that can be seen, at most `MAX_SHOWN` of them, the
//! largest on the screen first, so a frame of the editor costs the same however many edits the world holds (SPIKE-29).

use planet_core::{PlanetFixed, Vec3};
use planet_editor::relevance::{in_view, reach_px, relevant_items, shown_items, ShownLimits, ViewState, MAX_SHOWN, MIN_REACH_PX};
use planet_frame::globe::Viewport;
use planet_frame::Camera;
use planet_world_def::cells::cell_of;
use planet_world_def::stamp::{Stamp, StampKind};
use planet_world_def::{BodyId, ItemId, ItemSpec, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;
const FOV: f64 = std::f64::consts::FRAC_PI_3;
const VIEWPORT: Viewport = Viewport { width_px: 1000.0, height_px: 1000.0 };

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

/// `n` hills 0 to 2 km east and north of a place, of reaches 40 to 400 m, in one world.
fn world(n: u128, at: Vec3) -> WorldDefinition {
    let mut def = WorldDefinition::single_planet(R, 7);
    let east = Vec3::new(0.0, 0.0, 1.0).cross(at).normalized();
    let north = at.cross(east).normalized();
    for id in 1..=n {
        let (a, b) = (((id * 7919) % 1000) as f64 / 1000.0, ((id * 104_729) % 1000) as f64 / 1000.0);
        let dir = (at * R + east * (a * 2_000.0) + north * (b * 2_000.0)).normalized();
        let reach_m = 40.0 + ((id * 31) % 361) as f64;
        let spec =
            ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir, content_hash: 0, reach_m, max_delta_m: 5.0, stamp: None }
                .with_stamp(Stamp::new(StampKind::Hill, [5.0, 0.0, 0.0]));
        def.put_item_spec(BodyId::PLANET, spec).unwrap();
    }
    def
}

fn view_from(at: Vec3, altitude_m: f64, look: Vec3) -> ViewState {
    let camera = Camera {
        position: PlanetFixed(at * (R + altitude_m)),
        forward: look.normalized(),
        up: at,
        fov_y_rad: FOV,
        aspect: VIEWPORT.width_px / VIEWPORT.height_px,
        near_m: 1.0,
    };
    ViewState { camera, viewport: VIEWPORT, radius_m: R }
}

fn find(def: &WorldDefinition, at: Vec3, v: &ViewState, cap: usize, selected: Option<ItemId>) -> Vec<ItemId> {
    shown_items(
        def,
        BodyId::PLANET,
        &[DEFAULT_ITEM_LAYER],
        &[cell_of(R, at)],
        v,
        ShownLimits { min_px: MIN_REACH_PX, max_shown: cap },
        selected,
    )
    .unwrap()
}

// spec: VIEW-051
#[test]
fn at_most_the_cap_is_shown_the_largest_on_the_screen_first_and_the_selected_edit_besides() {
    let at = unit(30.0, 40.0);
    let def = world(800, at);
    // 400 m up looking straight down: every hill is in view and covers more than 8 pixels.
    let v = view_from(at, 400.0, at * -1.0);
    let all = relevant_items(&def, BodyId::PLANET, &[DEFAULT_ITEM_LAYER], &[cell_of(R, at)], &v, MIN_REACH_PX, None).unwrap();
    assert!(all.len() > 700, "the scan finds them all: {}", all.len());
    let shown = find(&def, at, &v, MAX_SHOWN, None);
    assert_eq!(shown.len(), MAX_SHOWN);
    let px = |id: ItemId| {
        let (dir, reach) = def.item_anchor(BodyId::PLANET, id).unwrap();
        reach_px(&v, dir, reach)
    };
    // Every shown edit is at least as large on the screen as every edit left out.
    let smallest_shown = shown.iter().map(|id| px(*id)).fold(f64::INFINITY, f64::min);
    let largest_left = all.iter().filter(|id| !shown.contains(id)).map(|id| px(*id)).fold(0.0, f64::max);
    assert!(smallest_shown >= largest_left, "{smallest_shown} px shown against {largest_left} px left out");
    assert!(shown.windows(2).all(|w| w[0] < w[1]), "ascending by id");
    assert_eq!(find(&def, at, &v, MAX_SHOWN, None), shown, "the same camera shows the same edits");
    // The selected edit is shown whatever its size.
    let left_out = *all.iter().find(|id| !shown.contains(id)).unwrap();
    let with = find(&def, at, &v, MAX_SHOWN, Some(left_out));
    assert_eq!(with.len(), MAX_SHOWN + 1);
    assert!(with.contains(&left_out));
    // A world of 30 edits shows the same ones as the scan.
    let few = world(30, at);
    assert_eq!(
        find(&few, at, &v, MAX_SHOWN, None),
        relevant_items(&few, BodyId::PLANET, &[DEFAULT_ITEM_LAYER], &[cell_of(R, at)], &v, MIN_REACH_PX, None).unwrap()
    );
}

// spec: VIEW-051
#[test]
fn edits_behind_the_camera_or_far_off_the_screen_are_not_shown() {
    let at = unit(30.0, 40.0);
    let def = world(450, at);
    let down = view_from(at, 400.0, at * -1.0);
    assert!(find(&def, at, &down, usize::MAX, None).len() > 400, "in view from above");
    // The ground may lie `HEIGHT_MARGIN_M` off the sphere, so an edit nearer than that is never left out for its place: look from farther.
    let east = Vec3::new(0.0, 0.0, 1.0).cross(at).normalized();
    let ahead = view_from(at - east * (12_000.0 / R), 400.0, east);
    let in_front = find(&def, at, &ahead, usize::MAX, None);
    let away = view_from(at + east * (12_000.0 / R), 400.0, east);
    let behind = find(&def, at, &away, usize::MAX, Some(7));
    assert!(!in_front.is_empty(), "looking at them");
    assert_eq!(behind, vec![7], "looking away from them shows only the selected one");
    // The test itself: a wide disc that touches the view is in view, one wholly beside it is not.
    let dir = (at * R + east * 100.0).normalized();
    assert!(in_view(&ahead, dir, 50.0, 0.0));
    assert!(!in_view(&away, dir, 50.0, 0.0));
}

// spec: VIEW-051
#[test]
fn the_session_reuses_the_footprint_find_while_nothing_is_edited_and_sees_an_edit_at_once() {
    use planet_editor::commands::Editor;
    use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
    use planet_editor::session::Session;
    use planet_world_def::item_store::MemoryStore;
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(2)));
    let editor = Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2);
    let mut s = Session::new(editor, BodyId::PLANET, R);
    let at = unit(30.0, 40.0);
    let (first, _) = s.place("hill", at).unwrap();
    s.deselect();
    let tiles = [cell_of(R, at)];
    let near = view_from(at, 300.0, at * -1.0);
    let moved = view_from(unit(30.0, 40.01), 300.0, at * -1.0);
    // The same answer as the function on its own, for two cameras in a row (the second reuses the find) and back again.
    for v in [&near, &moved, &near] {
        let direct = shown_items(s.editor().definition(), BodyId::PLANET, &["stamps"], &tiles, v, ShownLimits::default(), None).unwrap();
        assert_eq!(s.shown(&tiles, v).unwrap(), direct);
    }
    assert_eq!(s.shown(&tiles, &near).unwrap(), vec![first]);
    // An edit is seen at the next call: the find is not kept past a change of the definition.
    let (second, _) = s.place("crater", unit(30.0, 40.0001)).unwrap();
    s.deselect();
    let shown = s.shown(&tiles, &near).unwrap();
    assert!(shown.contains(&first) && shown.contains(&second), "{shown:?}");
}
