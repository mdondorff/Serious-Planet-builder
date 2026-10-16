//! STRM-006 (headroom), tier A: on the ten standard views over the default world's eroded field (as `world_view_pins.rs`),
//! the residency set R with its ancestors and the draw set D with its ancestors stay within pinned fractions of the
//! terrain share's slots (ADR 0033 point 2). Counts only, no device.
//!
//! ADR 0033's fractions (R with ancestors at most 10 % of the mid tier's slots, the ground-1m draw set with ancestors at
//! most 6 %) are for the compact tile's slot count (230,640 at the mid share), which is what the product builds (the pools come
//! from `tier_compact_pool`, so the pins follow the real configuration). The vertex format (the reference, `--tile-format vertex`)
//! has slots 6 times larger, so the same tile counts take a larger fraction of fewer slots: both are pinned.

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId};
use planet_frame::pool::{tier_compact_pool, tier_pool};
use planet_frame::{
    production_finest_level, residency_set_field, scripted_views_field, select_nodes_in_view_field, LodParams, SelectionCache,
    REFERENCE_FINEST_LEVEL,
};
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

/// The same shares in vertex-format slots (cells 32, 58,608 B since MESH_VERSION 7: the 44-byte vertex and the 4-byte climate of each of
/// the 1,221 vertices, ADR 0039 point 2; blocks of 256): `tier_pool(tier, 32, 2).main_slots()`. Under MESH_VERSION 6 these were 37,376 and
/// 57,344 slots of 53,724 B.
const VERTEX_MID_SLOTS: u64 = 34_304;
const VERTEX_HIGH_SLOTS: u64 = 52_480;

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
/// production finest level 22 (`production_finest_level(32)`; what the pinned node counts of `world_view_pins.rs` use). R is the
/// position-based set with all its ancestors. Pinned 2026-10-15 by `residency-manager`; re-pinned 2026-10-15 by `finest-level-cap`
/// (LOD-010, declared: finest level 22): only ground-1m reaches level 22, R 14,325 to 11,493 (-19.8 %) and D with ancestors 4,063 to
/// 3,336 (-17.9 %). Raising a pin is a decision: say why in the change that does it (CON-20).
const PINS: [(&str, usize, usize); 10] = [
    ("orbit-20000km", 73, 73),
    ("high-2000km", 945, 597),
    ("aerial-200km", 2650, 893),
    ("aerial-20km", 4252, 1252),
    ("low-2km", 6362, 1807),
    ("ground-200m", 8677, 2384),
    ("ground-1m", 11493, 3336),
    ("north-pole-1km", 5633, 1603),
    ("cube-corner-1km", 7899, 2185),
    ("face-edge-1km", 7822, 2133),
];

/// The same at the reference finest level 26 (the cap before ADR 0033 point 6, `--finest-level 26`), where it differs: only ground-1m.
const PINS_L26_GROUND_1M: (usize, usize) = (14325, 4063);

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
        let a = s.spawn(|| measure(&world, REFERENCE_FINEST_LEVEL));
        let b = s.spawn(|| measure(&world, production_finest_level(CELLS)));
        (a.join().unwrap_or_else(|e| std::panic::resume_unwind(e)), b.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
    });
    assert_eq!(production_finest_level(CELLS), 22, "the production cap at cells 32 (LOD-010)");
    assert_eq!(l22.len(), PINS.len(), "a standard view was added or removed: pin it");
    let mut failures = check_pins(&l22, &PINS);
    for (a, b) in l26.iter().zip(&l22) {
        if a.0 != "ground-1m" && (a.1, a.2) != (b.1, b.2) {
            failures.push(format!("{}: the finest level 22 changed a view that has no tile finer than 22 ({} -> {})", a.0, a.1, b.1));
        }
    }
    let g = l26.iter().find(|m| m.0 == "ground-1m").unwrap();
    if (g.1, g.2) != PINS_L26_GROUND_1M {
        failures.push(format!("ground-1m at the reference level 26: R {} and D {} against pins {:?}", g.1, g.2, PINS_L26_GROUND_1M));
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

    // ADR 0033 point 2 for the compact tile's slot count, from the pools the renderer allocates: R with ancestors at most 10 % of the
    // mid tier's slots at every view, the ground-1m draw set with ancestors at most 6 %; the allocation model stays inside both shares.
    let cmid = tier_compact_pool(Tier::Mid, CELLS, 2).unwrap();
    let chigh = tier_compact_pool(Tier::High, CELLS, 2).unwrap();
    assert!(cmid.allocated_bytes() <= 2 * GIB && chigh.allocated_bytes() <= 3 * GIB, "the compact pools exceed the terrain share");
    let (compact_mid, compact_high) = (u64::from(cmid.main_slots()), u64::from(chigh.main_slots()));
    assert_eq!((compact_mid, compact_high), (COMPACT_MID_SLOTS, COMPACT_HIGH_SLOTS), "the share in compact slots changed");
    // The bounds hold at the production cap and at the reference cap 26 (`--finest-level 26` stays supported): the larger of the two.
    let r_max = l22.iter().chain(&l26).map(|m| m.1).max().unwrap() as u64;
    let g_d = l22.iter().chain(&l26).filter(|m| m.0 == "ground-1m").map(|m| m.2).max().unwrap() as u64;
    assert!(r_max * 10 <= compact_mid, "R {r_max} slots is over 10 % of the mid tier's {compact_mid} compact slots");
    assert!(g_d * 100 <= compact_mid * 6, "ground-1m D {} is over 6 % of {compact_mid}", g_d);
    assert!(r_max * 100 <= compact_high * 10, "R {r_max} is over 10 % of the high tier's {compact_high}");

    // The reference format: the vertex format's slot (58,608 B at cells 32). The share is a byte ceiling, so the same demand takes a
    // larger fraction of fewer slots; both tiers hold the worst R with room to spare (no degradation in normal use).
    let mid = tier_pool(Tier::Mid, CELLS, 2).unwrap();
    let high = tier_pool(Tier::High, CELLS, 2).unwrap();
    assert!(mid.allocated_bytes() <= 2 * GIB && high.allocated_bytes() <= 3 * GIB);
    let (mid_slots, high_slots) = (u64::from(mid.main_slots()), u64::from(high.main_slots()));
    assert_eq!((mid_slots, high_slots), (VERTEX_MID_SLOTS, VERTEX_HIGH_SLOTS), "the share in vertex-format slots changed");
    // Re-pinned with the climate (MESH_VERSION 7, ADR 0039 point 2): the vertex slot grew 9.1 % (4 B of climate per vertex), so the same R
    // is 41.8 % of the mid tier's slots (it was 38.3 %); the bounds move from 40 % / 26 % to 45 % / 30 %. The compact tile, the
    // production format, did not grow: the climate rides in the spare channel of its texels, and its pins above are unchanged.
    assert!(r_max * 100 <= mid_slots * 45, "R {r_max} is over 45 % of the mid tier's {mid_slots} vertex-format slots");
    assert!(g_d * 100 <= mid_slots * 12, "ground-1m D {} is over 12 % of {mid_slots}", g_d);
    assert!(r_max * 100 <= high_slots * 30, "R {r_max} is over 30 % of the high tier's {high_slots}");
}
