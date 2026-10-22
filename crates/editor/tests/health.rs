//! Tier A: the edit-health lists kept incrementally (VIEW-048, M7 edit-health-scale): they equal the full scans after any sequence of edits, and
//! the work a change costs is the items near it, not the items there are (SPIKE-29).

use planet_editor::brush;
use planet_editor::commands::{Editor, NewItem};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::polyline::GeoPoint;
use planet_editor::problems;
use planet_editor::session::Session;
use planet_editor::session::Tool;
use planet_editor::slope;
use planet_world_def::item::{Anchor, Item, ItemStroke};
use planet_world_def::item_store::{ItemStore, MemoryStore};
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, ItemId, WorldDefinition};

const R: f64 = 6_371_000.0;

fn session() -> Session<MemoryStore> {
    let ids = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(5)));
    Session::new(Editor::new(WorldDefinition::single_planet(R, 7), MemoryStore::new(), ids, 4, 2), BodyId::PLANET, R)
}

fn deg(m: f64) -> f64 {
    m / R * 180.0 / std::f64::consts::PI
}

fn stroke(kind: BrushKind, strength_m: f64, radius_m: f64, north_m: f64, east_m: f64) -> ItemStroke {
    ItemStroke { kind, radius_m, strength_m, target_m: None, points: vec![(deg(north_m), deg(east_m))] }
}

fn new_item(strokes: Vec<ItemStroke>) -> NewItem {
    // The first point of the first stroke is the anchor (the file's rule).
    let (latitude_deg, longitude_deg) = strokes[0].points[0];
    NewItem {
        body: BodyId::PLANET,
        surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
        layer: brush::BRUSH_LAYER.to_string(),
        priority: 0,
        kind: brush::BRUSH_KIND.to_string(),
        anchor: Anchor { latitude_deg, longitude_deg, height_m: 0.0, heading_deg: 0.0 },
        reach_m: 10_000.0,
        max_delta_m: 100.0,
        params: Default::default(),
        name: Some("b".to_string()),
        notes: None,
        points: Vec::new(),
        group: None,
        part: None,
        mask_layer: None,
        strokes,
    }
}

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn assert_equal_to_full_scans(s: &Session<MemoryStore>, what: &str) {
    let limit = slope::gradient_limit(s.drawable_slope());
    assert_eq!(s.steep_items(), slope::steep_items(s.editor().items(), limit, R), "steep after {what}");
    assert_eq!(s.stacked_items(), problems::stacked_items(s.editor().items(), R), "stacked after {what}");
}

// spec: VIEW-048
#[test]
fn the_incremental_lists_equal_the_full_scans_after_every_edit_of_a_random_sequence() {
    let mut s = session();
    let mut rng = Rng(9);
    let mut ids: Vec<ItemId> = Vec::new();
    // Brushes in a few small areas (so that they stack, some steeply), and some with many strokes.
    let make = |rng: &mut Rng| -> Vec<ItemStroke> {
        let area = (rng.next_u64() % 4) as f64 * 30_000.0;
        let n = 1 + (rng.next_u64() % 6) as usize;
        (0..n)
            .map(|_| {
                let kind = if rng.f() < 0.2 { BrushKind::Smooth } else { BrushKind::Raise };
                stroke(kind, 1.0 + 6.0 * rng.f(), 60.0 + 200.0 * rng.f(), area + 600.0 * rng.f(), 600.0 * rng.f())
            })
            .collect()
    };
    for _ in 0..30 {
        let strokes = make(&mut rng);
        ids.push(s.editor_mut().put(new_item(strokes)).unwrap().0);
    }
    assert_equal_to_full_scans(&s, "30 puts");
    let mut steps = [0usize; 4];
    for step in 0..60 {
        // Undo may have taken an item away: the ids are those the editor has.
        ids = s.editor().item_ids();
        let what = match rng.next_u64() % 4 {
            0 => {
                let strokes = make(&mut rng);
                s.editor_mut().put(new_item(strokes)).unwrap();
                0
            }
            1 if !ids.is_empty() => {
                let id = ids[(rng.next_u64() as usize) % ids.len()];
                let strokes = make(&mut rng);
                let _ = s.editor_mut().change(id, |it| {
                    it.strokes = strokes;
                    it.strokes[0].points[0] = (it.anchor.latitude_deg, it.anchor.longitude_deg);
                });
                1
            }
            2 if !ids.is_empty() => {
                let id = ids[(rng.next_u64() as usize) % ids.len()];
                s.editor_mut().delete(id).unwrap();
                2
            }
            _ => {
                let _ = s.editor_mut().undo();
                3
            }
        };
        steps[what] += 1;
        assert_equal_to_full_scans(&s, &format!("step {step} (kind {what})"));
    }
    assert!(steps.iter().all(|n| *n > 0), "every kind of step was made: {steps:?}");
    assert!(!s.stacked_items().is_empty() || !s.steep_items().is_empty(), "the scene has something to list");
    // A change of the drawable slope starts again from all the items.
    s.set_drawable_slope(1.2);
    assert_equal_to_full_scans(&s, "a new drawable slope");
}

// spec: VIEW-048
#[test]
fn a_change_re_checks_the_items_near_it_and_no_others_whatever_the_number_of_items() {
    // The same changes cost the same checks at 625 and at 2,500 items (a count that grew with the items would differ).
    bounded_at(25);
    bounded_at(50);
}

fn bounded_at(side: u32) {
    // side x side brushes on a grid 1 km apart, each a small raise of 100 m radius: none meets another. They are made as an item store holds
    // them and loaded in one step (the test counts items checked, not time).
    let mut scratch = session();
    let (template_id, _) = scratch.editor_mut().put(new_item(vec![stroke(BrushKind::Raise, 2.0, 100.0, 0.0, 0.0)])).unwrap();
    let template = scratch.editor().item(template_id).unwrap().clone();
    let mut store = MemoryStore::new();
    let mut ids = Vec::new();
    for row in 0..side {
        for col in 0..side {
            let id = ItemId::from(row * side + col + 1);
            let strokes = vec![stroke(BrushKind::Raise, 2.0, 100.0, f64::from(row) * 1000.0, f64::from(col) * 1000.0)];
            let anchor = Anchor { latitude_deg: strokes[0].points[0].0, longitude_deg: strokes[0].points[0].1, ..template.anchor };
            store.put(&Item { id, anchor, strokes, ..template.clone() }).unwrap();
            ids.push(id);
        }
    }
    let idm = IdMaker::new(Box::new(FixedClock(1_760_000_000_000)), Box::new(SeededEntropy(6)));
    let mut editor = Editor::new(WorldDefinition::single_planet(R, 7), store, idm, 4, 2);
    editor.load(BodyId::PLANET).unwrap();
    let mut s = Session::new(editor, BodyId::PLANET, R);
    let (first, middle) = (ids[0], ids[(side / 2 * side + side / 2) as usize]);
    let total = (side * side) as usize;
    assert_eq!(s.editor().items().count(), total);
    assert!(s.steep_items().is_empty());
    assert_eq!(s.health_checked(), total, "the first time, every item");
    let pile_at = f64::from(side) * 500.0 + 500.0 * 0.5;
    // Changing one: only itself.
    s.editor_mut().change(middle, |it| it.strokes[0].strength_m = 3.0).unwrap();
    assert!(s.steep_items().is_empty());
    assert_eq!(s.health_checked(), 1, "one isolated item changed");
    // Nothing changed: nothing checked again.
    let _ = s.steep_items();
    assert_eq!(s.health_checked(), 1, "no change, no new work");
    // A new brush on top of another: the two.
    let point = s.editor().item(first).unwrap().strokes[0].points[0];
    let on_top = vec![ItemStroke { kind: BrushKind::Raise, radius_m: 100.0, strength_m: 2.0, target_m: None, points: vec![point] }];
    let top = s.editor_mut().put(new_item(on_top)).unwrap().0;
    let _ = s.stacked_items();
    assert_eq!(s.health_checked(), 2, "the new item and the one it covers");
    // Deleting it: the one it covered.
    s.editor_mut().delete(top).unwrap();
    let _ = s.stacked_items();
    assert_eq!(s.health_checked(), 1, "the one that was covered");
    // A pile of 40 on one place: a change to one of them re-checks the pile (40 in all), not the thousands.
    let mut pile = Vec::new();
    for _ in 0..40 {
        pile.push(s.editor_mut().put(new_item(vec![stroke(BrushKind::Raise, 5.0, 100.0, pile_at, pile_at)])).unwrap().0);
    }
    let _ = s.steep_items();
    s.editor_mut().change(pile[0], |it| it.strokes[0].strength_m = 6.0).unwrap();
    let _ = s.steep_items();
    assert!(s.health_checked() <= 42, "{}", s.health_checked());
    assert!(s.steep_items().contains(&pile[0]), "forty strokes on a place are too steep");
    // The full scans are quadratic in the items (seconds at 5,000): the equality is held by the random sequence above; here, what they must find.
    assert!(s.steep_items().iter().all(|id| pile.contains(id)), "only the pile is steep");
    assert!(s.stacked_items().keys().all(|id| pile.contains(id)), "only the pile is stacked");
}

fn piece_copy(p: &planet_editor::brush::BrushStroke) -> planet_editor::brush::BrushStroke {
    planet_editor::brush::BrushStroke {
        kind: p.kind,
        radius_m: p.radius_m,
        strength_m: p.strength_m,
        target_m: p.target_m,
        points: p.points.clone(),
    }
}

// spec: VIEW-049
#[test]
fn the_problems_list_and_the_slope_guard_give_what_the_scans_over_all_items_give() {
    let mut s = session();
    let limit = slope::gradient_limit(s.drawable_slope());
    // Isolated brushes all round (so that the near-item filter has something to leave out), and a pile of 5 m strokes that fills most of the room.
    for k in 0..200 {
        let it = new_item(vec![stroke(BrushKind::Raise, 2.0, 100.0, 3_000.0 + f64::from(k) * 700.0, 20_000.0)]);
        s.editor_mut().put(it).unwrap();
    }
    let one = slope::STROKE_GRADIENT * 5.0 / 100.0;
    let n = (limit / one).floor() as usize - 1;
    // One more stroke that leaves room for half of a 5 m stroke.
    let rest_m = (limit - n as f64 * one - 0.5 * one) * 100.0 / slope::STROKE_GRADIENT;
    let piece = planet_editor::brush::BrushStroke {
        kind: BrushKind::Raise,
        radius_m: 100.0,
        strength_m: 5.0,
        target_m: None,
        points: vec![GeoPoint::new(0.0, deg(500.0))],
    };
    let mut open = Vec::new();
    for _ in 0..n {
        open = s.editor_mut().add_stroke(new_item(vec![stroke(BrushKind::Raise, 5.0, 100.0, 0.0, 500.0)]), &piece, &open).unwrap().0;
    }
    let rest = planet_editor::brush::BrushStroke { strength_m: rest_m, ..piece_copy(&piece) };
    s.editor_mut().add_stroke(new_item(vec![stroke(BrushKind::Raise, rest_m, 100.0, 0.0, 500.0)]), &rest, &open).unwrap();
    let camera = GeoPoint::new(0.0, deg(500.0)).dir() * R;
    // The list holds exactly the steep and the stacked, as the scans find them.
    let all: Vec<_> = s.editor().items().cloned().collect();
    let mut want: Vec<ItemId> =
        slope::steep_items(&all, limit, R).into_iter().chain(problems::stacked_items(&all, R).into_keys()).collect();
    want.sort();
    want.dedup();
    let mut got: Vec<ItemId> = s.problems(&[], camera).iter().map(|p| p.row.id).collect();
    got.sort();
    assert_eq!(got, want);
    // A stroke painted on the pile is cut to what the scan over every stroke allows.
    let existing: Vec<_> = s.editor().items().flat_map(|i| i.strokes.iter().cloned()).collect();
    let refs: Vec<&ItemStroke> = existing.iter().collect();
    let expected = slope::allowed_strength(&piece, &refs, R, limit);
    assert!(expected < 5.0 && expected >= planet_editor::brush::STRENGTH_RANGE_M.0, "the pile leaves some room: {expected}");
    s.set_tool(Tool::Brush(BrushKind::Raise));
    s.set_brush_radius(100.0).unwrap();
    s.set_brush_strength(5.0).unwrap();
    s.begin_stroke(GeoPoint::new(0.0, deg(500.0)).dir(), 0.0).unwrap();
    s.end_stroke().unwrap();
    let painted = s
        .editor()
        .items()
        .flat_map(|i| i.strokes.iter())
        .map(|t| t.strength_m)
        .filter(|m| *m != 5.0 && *m != 2.0 && *m != rest_m)
        .collect::<Vec<_>>();
    assert_eq!(painted, vec![expected], "the stroke has the strength the full scan allows");
}
