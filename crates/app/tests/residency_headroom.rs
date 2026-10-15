//! STRM-006 (headroom), tier A: on the ten standard views over the default world's eroded field (as `world_view_pins.rs`),
//! the residency set R with its ancestors and the draw set D with its ancestors stay within pinned fractions of the
//! terrain share's slots (ADR 0033 point 2). Counts only, no device.
//!
//! ADR 0033's fractions (R with ancestors at most 10 % of the mid tier's slots, the ground-1m draw set with ancestors at
//! most 6 %) are for the compact tile's slot count (230,640 at the mid share). The vertex format is what is built, and its
//! slot is 12 times larger, so the same tile counts take a larger fraction of fewer slots: both are pinned.

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId};
use planet_frame::pool::tier_pool;
use planet_frame::{residency_set_field, scripted_views_field, select_nodes_in_view_field, LodParams, SelectionCache};
use planet_streaming::budget::GIB;
use planet_streaming::Tier;
use planet_world_def::WorldDefinition;
use std::collections::BTreeSet;

const R: f64 = 6_371_000.0;
const PIN_BAKE_RES: u32 = 256;
const CELLS: u32 = 32;

/// Slots of ADR 0033's compact tile at the mid / high share: 15 / 23 atlases of 124 x 124 (cells 32).
const COMPACT_MID_SLOTS: u64 = 15 * 15_376;
const COMPACT_HIGH_SLOTS: u64 = 23 * 15_376;

/// The same shares in vertex-format slots (cells 32, 53,724 B, blocks of 256): `tier_pool(tier, 32, 2).main_slots()`.
const VERTEX_MID_SLOTS: u64 = 37_376;
const VERTEX_HIGH_SLOTS: u64 = 57_344;

fn closure(tiles: impl IntoIterator<Item = TileId>) -> BTreeSet<TileId> {
    let mut out = BTreeSet::new();
    for t in tiles {
        let mut x = Some(t);
        while let Some(a) = x {
            if !out.insert(a) {
                break;
            }
            x = a.parent();
        }
    }
    out
}

fn measure(world: &WorldTerrain, max_level: u8) -> Vec<(String, usize, usize, usize)> {
    let field = world.field().unwrap();
    let size = (2560u32, 1440u32);
    let lod = LodParams {
        cells: CELLS,
        viewport_h_px: f64::from(size.1),
        tau_px: 6.0,
        max_level,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    let views = scripted_views_field(R, f64::from(size.0) / f64::from(size.1), field.as_ref());
    let mut cache = SelectionCache::new();
    let mut out = Vec::new();
    for (name, camera) in views {
        let position: PlanetFixed = camera.position;
        let r = residency_set_field(&TangentWarp, position, field.as_ref(), &lod, &mut cache);
        let d = select_nodes_in_view_field(&TangentWarp, &camera, field.as_ref(), &lod);
        let d_closure = closure(d.nodes.iter().map(|n| n.tile));
        assert!(d_closure.iter().all(|t| r.contains(*t)), "{name}: D with its ancestors is inside R");
        out.push((name.to_string(), r.len(), d_closure.len(), d.nodes.len()));
    }
    out
}

/// `(view, |R|, |D with ancestors|)` per standard view, cells 32, tau 6 px, 1440p, the default world over a 256-cell bake, the
/// finest level 26 (what the pinned node counts of `world_view_pins.rs` use). R is the position-based set with all its
/// ancestors. Pinned 2026-10-15 by `residency-manager`; raising a pin is a decision: say why in the change that does it
/// (CON-20).
const PINS_L26: [(&str, usize, usize); 10] = [
    ("orbit-20000km", 73, 73),
    ("high-2000km", 945, 597),
    ("aerial-200km", 2650, 893),
    ("aerial-20km", 4252, 1252),
    ("low-2km", 6362, 1807),
    ("ground-200m", 8677, 2384),
    ("ground-1m", 14325, 4063),
    ("north-pole-1km", 5633, 1603),
    ("cube-corner-1km", 7899, 2185),
    ("face-edge-1km", 7822, 2133),
];

/// The same at the finest level 22 (ADR 0033 point 6), where it differs: only ground-1m.
const PINS_L22_GROUND_1M: (usize, usize) = (11493, 3336);

fn check_pins(measured: &[(String, usize, usize, usize)], pins: &[(&str, usize, usize)]) -> Vec<String> {
    let mut failures = Vec::new();
    for ((name, r, dc, _), (pinned_name, r_pin, d_pin)) in measured.iter().zip(pins) {
        assert_eq!(name, pinned_name, "the pins are in the order of the standard views");
        for (what, got, pin) in [("R", *r, *r_pin), ("D with ancestors", *dc, *d_pin)] {
            if got > pin {
                failures.push(format!("{name}: {what} is {got} slots against its pin {pin}"));
            }
            // A pin stays tight: more than 10 % under its pin, the pin is lowered with it.
            if pin > got * 10 / 9 {
                failures.push(format!("{name}: {what} is {got} slots, more than 10 % under its pin {pin}: lower the pin"));
            }
        }
    }
    failures
}

// spec: STRM-006
#[test]
fn the_slots_every_standard_view_needs_stay_within_their_pins_and_the_share() {
    // The two finest-level measurements are independent: run them side by side over one bake (opening it is most of the cost).
    let world = WorldTerrain::open(WorldDefinition::single_planet(R, 1), PIN_BAKE_RES).unwrap();
    let (l26, l22) = std::thread::scope(|s| {
        let a = s.spawn(|| measure(&world, 26));
        let b = s.spawn(|| measure(&world, 22));
        (a.join().unwrap_or_else(|e| std::panic::resume_unwind(e)), b.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
    });
    assert_eq!(l26.len(), PINS_L26.len(), "a standard view was added or removed: pin it");
    let mut failures = check_pins(&l26, &PINS_L26);
    for (a, b) in l26.iter().zip(&l22) {
        if a.0 != "ground-1m" && (a.1, a.2) != (b.1, b.2) {
            failures.push(format!("{}: the finest level 22 changed a view that has no tile finer than 22 ({} -> {})", a.0, a.1, b.1));
        }
    }
    let g = l22.iter().find(|m| m.0 == "ground-1m").unwrap();
    if (g.1, g.2) != PINS_L22_GROUND_1M {
        failures.push(format!("ground-1m at level 22: R {} and D {} against pins {:?}", g.1, g.2, PINS_L22_GROUND_1M));
    }
    assert!(
        failures.is_empty(),
        "slot demand against its pins:
{}",
        failures.join(
            "
"
        )
    );

    // ADR 0033 point 2, for the compact tile's slot count (the format the ADR sizes): R with ancestors at most 10 % of the
    // mid tier's slots at every view, the ground-1m draw set with ancestors at most 6 %.
    let r_max = l26.iter().map(|m| m.1).max().unwrap() as u64;
    let g26 = l26.iter().find(|m| m.0 == "ground-1m").unwrap();
    assert!(r_max * 10 <= COMPACT_MID_SLOTS, "R {r_max} slots is over 10 % of the mid tier's {COMPACT_MID_SLOTS} compact slots");
    assert!(g26.2 as u64 * 100 <= COMPACT_MID_SLOTS * 6, "ground-1m D {} is over 6 % of {COMPACT_MID_SLOTS}", g26.2);
    assert!(r_max * 100 <= COMPACT_HIGH_SLOTS * 10, "and over 10 % of the high tier's {COMPACT_HIGH_SLOTS}");

    // What is built: the vertex format's slot (53,724 B at cells 32). The share is a byte ceiling, so the same demand takes a
    // larger fraction of fewer slots; both tiers hold the worst R with room to spare (no degradation in normal use).
    let mid = tier_pool(Tier::Mid, CELLS, 2).unwrap();
    let high = tier_pool(Tier::High, CELLS, 2).unwrap();
    assert!(mid.allocated_bytes() <= 2 * GIB && high.allocated_bytes() <= 3 * GIB);
    let (mid_slots, high_slots) = (u64::from(mid.main_slots()), u64::from(high.main_slots()));
    assert_eq!((mid_slots, high_slots), (VERTEX_MID_SLOTS, VERTEX_HIGH_SLOTS), "the share in vertex-format slots changed");
    assert!(r_max * 100 <= mid_slots * 40, "R {r_max} is over 40 % of the mid tier's {mid_slots} vertex-format slots");
    assert!(g26.2 as u64 * 100 <= mid_slots * 12, "ground-1m D {} is over 12 % of {mid_slots}", g26.2);
    assert!(r_max * 100 <= high_slots * 26, "R {r_max} is over 26 % of the high tier's {high_slots}");
}
