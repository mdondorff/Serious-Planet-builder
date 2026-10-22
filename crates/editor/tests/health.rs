//! Tier A: the edit-health lists kept incrementally (VIEW-048, M7 edit-health-scale): they equal the full scans after any sequence of edits, and
//! the work a change costs is the items near it, not the items there are (SPIKE-29).

use planet_editor::brush;
use planet_editor::commands::{Editor, NewItem};
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::problems;
use planet_editor::session::Session;
use planet_editor::slope;
use planet_world_def::item::{Anchor, ItemStroke};
use planet_world_def::item_store::MemoryStore;
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
fn a_change_re_checks_the_items_near_it_and_no_others_at_5000_items() {
    let mut s = session();
    // 5,041 brushes on a grid 1 km apart, each a small raise of 100 m radius: none meets another.
    let mut ids = Vec::new();
    for row in 0..71 {
        for col in 0..71 {
            let it = new_item(vec![stroke(BrushKind::Raise, 2.0, 100.0, f64::from(row) * 1000.0, f64::from(col) * 1000.0)]);
            ids.push(s.editor_mut().put(it).unwrap().0);
        }
    }
    let (first, middle) = (ids[0], ids[35 * 71 + 35]);
    assert!(s.editor().items().count() >= 5000);
    assert!(s.steep_items().is_empty());
    assert_eq!(s.health_checked(), 5041, "the first time, every item");
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
    // A pile of 40 on one place: a change to one of them re-checks the pile (40 in all), not the 5,000.
    let mut pile = Vec::new();
    for _ in 0..40 {
        pile.push(s.editor_mut().put(new_item(vec![stroke(BrushKind::Raise, 5.0, 100.0, 36_500.0, 36_500.0)])).unwrap().0);
    }
    let _ = s.steep_items();
    s.editor_mut().change(pile[0], |it| it.strokes[0].strength_m = 6.0).unwrap();
    let _ = s.steep_items();
    assert!(s.health_checked() <= 42, "{}", s.health_checked());
    assert!(s.steep_items().contains(&pile[0]), "forty strokes on a place are too steep");
    assert_equal_to_full_scans(&s, "the 5,000 item scene");
}
