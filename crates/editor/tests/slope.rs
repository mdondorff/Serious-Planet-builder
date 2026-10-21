//! Tier A: the slope guard of the brush (VIEW-043). A raise or lower stroke whose stacked gradient with the strokes already there would pass
//! what the plan can draw is clamped, with a notice; an item that is already too steep wears a warning in the inspector and the layer list.
//! The limit is derived from the LOD parameters through the plan's own morph rule.

use planet_core::Vec3;
use planet_editor::brush;
use planet_editor::commands::Editor;
use planet_editor::ids::{FixedClock, IdMaker, SeededEntropy};
use planet_editor::polyline::GeoPoint;
use planet_editor::session::{Session, Tool};
use planet_editor::slope;
use planet_frame::LodParams;
use planet_world_def::item::Item;
use planet_world_def::item_store::MemoryStore;
use planet_world_def::stamp::BrushKind;
use planet_world_def::{BodyId, WorldDefinition};

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

/// A click of the tool `kind` at `at` metres east (a stroke of one point).
fn click(s: &mut Session<MemoryStore>, kind: BrushKind, at: f64) {
    s.set_tool(Tool::Brush(kind));
    s.begin_stroke(east(at), 0.0).unwrap();
    s.end_stroke().unwrap();
}

fn strokes(s: &Session<MemoryStore>) -> Vec<planet_world_def::item::ItemStroke> {
    s.editor().items().filter(|i| i.kind == "brush").flat_map(|i| i.strokes.clone()).collect()
}

/// The stacked gradient of the raise and lower strokes at `at` metres east, the sum the guard keeps under the limit.
fn stacked(s: &Session<MemoryStore>, at: f64) -> f64 {
    let all = strokes(s);
    slope::gradient_at(&all.iter().collect::<Vec<_>>(), east(at), R, 0.0)
}

// spec: VIEW-043
#[test]
fn the_limit_is_the_drawable_slope_of_the_lod_parameters_less_the_share_left_to_the_base() {
    assert_eq!(slope::gradient_limit(2.5), 2.5 - slope::BASE_SLOPE_SHARE);
    assert_eq!(slope::gradient_limit(0.4), 0.0, "never negative");
    let p = LodParams::earth_1080p();
    let limit = slope::gradient_limit(p.drawable_slope());
    assert!((1.3..1.7).contains(&limit), "about 1.5 at the defaults: {limit}");
    // A session follows the parameters it is given.
    let mut s = session();
    assert_eq!(s.drawable_slope(), p.drawable_slope(), "the default is the production parameters'");
    s.set_drawable_slope(1.2);
    assert_eq!(s.drawable_slope(), 1.2);
    // The gradient of one stroke: at most 1.54 s / r for the profile (1 - x^2)^2, a raise and a lower alike, a flatten and a smooth none.
    let stroke = |kind, s: f64| planet_world_def::item::ItemStroke {
        kind,
        radius_m: 100.0,
        strength_m: s,
        target_m: None,
        points: vec![(0.0, 0.0)],
    };
    let g = slope::stroke_gradient(&stroke(BrushKind::Raise, 5.0));
    assert!((g - slope::STROKE_GRADIENT * 5.0 / 100.0).abs() < 1e-12);
    assert!((slope::STROKE_GRADIENT - 1.5396).abs() < 1e-3, "4 x (1 - x^2) at its largest, x = 1/sqrt(3)");
    assert_eq!(slope::stroke_gradient(&stroke(BrushKind::Lower, 5.0)), g);
    assert_eq!(slope::stroke_gradient(&stroke(BrushKind::Flatten, 5.0)), 0.0);
    assert_eq!(slope::stroke_gradient(&stroke(BrushKind::Smooth, 5.0)), 0.0);
}

// spec: VIEW-043
#[test]
fn a_stack_of_raise_strokes_passes_up_to_the_limit_then_the_next_is_clamped_with_a_notice_and_the_rest_are_refused() {
    let mut s = session();
    let limit = slope::gradient_limit(s.drawable_slope());
    let per_stroke = slope::stroke_gradient(&planet_world_def::item::ItemStroke {
        kind: BrushKind::Raise,
        radius_m: brush::DEFAULT_RADIUS_M,
        strength_m: brush::DEFAULT_STRENGTH_M,
        target_m: None,
        points: vec![(0.0, 0.0)],
    });
    let whole = (limit / per_stroke).floor() as usize;
    assert!((10..40).contains(&whole), "a stack of {whole} strokes of 5 m on 100 m passes");
    for k in 0..whole {
        click(&mut s, BrushKind::Raise, 0.0);
        assert!(!s.status().contains("clamped"), "stroke {k}: {}", s.status());
    }
    assert_eq!(strokes(&s).len(), whole);
    assert!(strokes(&s).iter().all(|t| t.strength_m == brush::DEFAULT_STRENGTH_M), "the first ones are as painted");
    // The next one has room for a part of its strength.
    click(&mut s, BrushKind::Raise, 0.0);
    let all = strokes(&s);
    assert_eq!(all.len(), whole + 1);
    let last = all.last().unwrap();
    assert!(last.strength_m < brush::DEFAULT_STRENGTH_M && last.strength_m > 0.0, "clamped: {}", last.strength_m);
    assert!(s.status().contains("clamped") && s.status().contains("too steep to draw"), "{}", s.status());
    assert!(stacked(&s, 0.0) <= limit + 1e-9, "the stack stays under the limit: {} of {limit}", stacked(&s, 0.0));
    // Then nothing more fits: the stroke is refused with the reason, and nothing is put.
    let before = strokes(&s).len();
    click(&mut s, BrushKind::Raise, 0.0);
    assert_eq!(strokes(&s).len(), before, "no room, no stroke");
    assert!(s.status().contains("too steep to draw"), "{}", s.status());
    // Undo takes the clamped stroke back as it takes any stroke.
    s.undo().unwrap();
    assert_eq!(strokes(&s).len(), whole, "the refused click put nothing, so undo takes the clamped stroke");
}

// spec: VIEW-043
#[test]
fn a_lower_counts_by_its_size_a_flatten_and_a_smooth_never_clamp_and_a_stroke_far_away_is_not_held_back() {
    let mut s = session();
    let limit = slope::gradient_limit(s.drawable_slope());
    // Fill the room with raises, then a lower of the same size has none, a lower elsewhere does.
    while stacked(&s, 0.0) + slope::STROKE_GRADIENT * 5.0 / 100.0 <= limit {
        click(&mut s, BrushKind::Raise, 0.0);
    }
    let held = strokes(&s).len();
    click(&mut s, BrushKind::Lower, 0.0);
    let after = strokes(&s);
    assert!(after.len() == held || after.last().unwrap().strength_m < 5.0, "a lower on the stack is clamped as a raise is");
    let count = strokes(&s).len();
    click(&mut s, BrushKind::Lower, 10_000.0);
    assert_eq!(strokes(&s).len(), count + 1);
    assert_eq!(strokes(&s).last().unwrap().strength_m, 5.0, "10 km away nothing holds it back");
    // They add no height of their own: a flatten and a smooth on the full stack are put as painted.
    for kind in [BrushKind::Flatten, BrushKind::Smooth] {
        let n = strokes(&s).len();
        click(&mut s, kind, 0.0);
        assert_eq!(strokes(&s).len(), n + 1, "{kind:?}");
        assert_eq!(strokes(&s).last().unwrap().strength_m, 5.0, "{kind:?} as painted");
        assert!(!s.status().contains("clamped"), "{}", s.status());
    }
}

// spec: VIEW-043
#[test]
fn an_item_already_too_steep_wears_the_warning_in_the_inspector_and_the_layer_row_and_one_within_the_limit_does_not() {
    // Strokes filed without the guard (items of an older version or other tools): 30 raises of 5 m on 100 m is over the limit.
    let mut s = session();
    let limit = slope::gradient_limit(s.drawable_slope());
    let template = |name: &str| planet_editor::commands::NewItem {
        body: BodyId::PLANET,
        surface: planet_world_def::item::DEFAULT_SURFACE.to_string(),
        layer: brush::BRUSH_LAYER.to_string(),
        priority: 0,
        kind: brush::BRUSH_KIND.to_string(),
        anchor: planet_world_def::item::Anchor { latitude_deg: 0.0, longitude_deg: 0.0, height_m: 0.0, heading_deg: 0.0 },
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
    };
    let piece = brush::BrushStroke {
        kind: BrushKind::Raise,
        radius_m: 100.0,
        strength_m: 5.0,
        target_m: None,
        points: vec![GeoPoint::new(0.0, 0.0)],
    };
    let mut open = Vec::new();
    for _ in 0..40 {
        let (touched, _) = s.editor_mut().add_stroke(template("Steep"), &piece, &open).unwrap();
        open = touched;
    }
    assert!(stacked(&s, 0.0) > limit, "the stack is over the limit: {}", stacked(&s, 0.0));
    let steep: Vec<_> = s.steep_items().iter().copied().collect();
    assert!(!steep.is_empty(), "the stack is flagged");
    let id = steep[0];
    s.select(id);
    let cam = east(0.0) * R;
    let inspector = s.inspector(cam).unwrap();
    assert_eq!(inspector.warning.as_deref(), Some(planet_editor::slope::WARNING), "{:?}", inspector.warning);
    let groups = s.layer_groups(cam);
    assert!(groups.iter().flat_map(|g| g.rows.iter()).any(|r| r.id == id && r.steep), "the layer row carries it");
    // A single small stroke elsewhere is not.
    click(&mut s, BrushKind::Raise, 50_000.0);
    let other = s.editor().items().find(|i: &&Item| i.strokes.iter().any(|t| t.points[0].1 > 0.1)).unwrap().id;
    s.select(other);
    assert!(s.inspector(cam).unwrap().warning.is_none());
    assert!(!s.layer_groups(cam).iter().flat_map(|g| g.rows.iter()).any(|r| r.id == other && r.steep));
}
