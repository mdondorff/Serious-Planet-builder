//! STRM-006 (headroom), tier A: on the ten standard views over the default world's eroded field (as `world_view_pins.rs`),
//! the residency set R with its ancestors and the draw set D with its ancestors stay within pinned fractions of the
//! terrain share's slots (ADR 0033 point 2). Counts only, no device.
//!
//! ADR 0033's fractions (R with ancestors at most 10 % of the mid tier's slots, the ground-1m draw set with ancestors at
//! most 6 %) are for the compact tile's slot count (212,190 at the mid share since the slot of 9,488 B, MESH_VERSION 10; 219,585 at 9,168 B), which is what the product builds (the pools come
//! from `tier_compact_pool`, so the pins follow the real configuration).

use planet::worldfield::WorldTerrain;
use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, TileId};
use planet_frame::pool::tier_compact_pool;
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

/// Slots of the compact tile at the mid / high share: 15 / 23 blocks of 14,639 slots (cells 32, 128 MiB, slot of 9,168 B, STRM-006).
// MESH_VERSION 10, driver block, ADR 0076/GEN-071: blocks of 14,146 slots of 9,488 B (14,639 of 9,168 B before): 212,190 and 325,358 slots.
const COMPACT_MID_SLOTS: u64 = 15 * 14_146;
const COMPACT_HIGH_SLOTS: u64 = 23 * 14_146;

/// The tiles with all their ancestors down to the render `floor` (LOD-011: no tile above the floor is ever a member).
fn closure(tiles: impl IntoIterator<Item = TileId>, floor: u8) -> BTreeSet<TileId> {
    let mut out = BTreeSet::new();
    for t in tiles {
        let mut x = Some(t);
        while let Some(a) = x {
            if !out.insert(a) {
                break;
            }
            x = a.parent().filter(|_| a.level() > floor);
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
        let d_closure = closure(d.nodes.iter().map(|n| n.tile), lod.floor_level());
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
/// Re-pinned 2026-10-18 by `render-floor-3` (LOD-011, ADR 0062 point 3, declared): R and D hold no tile above the render floor (level 3),
/// so the levels 0-2 ancestors drop out of every view (-4 to -34 slots), and orbit-20000km is the 182 level-3 tiles of the visible
/// hemisphere (73 before: 52 level-2 tiles and their ancestors) in both R and D.
/// Re-pinned 2026-10-25 by `m8-relief-band` (GEN-064, declared): the hill band changes the heights the plans read, so a few views plan up to
/// 0.3 % more tiles (ground-1m R 11,489 to 11,496, D 3,333 to 3,337; aerial-200km, low-2km and ground-200m by 1 to 7 slots); the others hold.
/// Re-pinned 2026-10-25 by `m8-relief-creases` (GEN-061, GEN-062, declared): the fine creases follow the landform and the landform widens the
/// field's bounds over wide caps: ground-200m R 8,675 to 8,679, D 2,385 to 2,387; ground-1m R 11,496 to 11,500, D 3,337 to 3,340 (at most +0.05 %).
const PINS: [(&str, usize, usize); 10] = [
    ("orbit-20000km", 182, 182),
    ("high-2000km", 911, 574),
    ("aerial-200km", 2642, 886),
    ("aerial-20km", 4246, 1249),
    ("low-2km", 6359, 1805),
    ("ground-200m", 8679, 2387),
    ("ground-1m", 11500, 3340),
    ("north-pole-1km", 5624, 1598),
    ("cube-corner-1km", 7890, 2182),
    ("face-edge-1km", 7812, 2127),
];

/// The same at the reference finest level 26 (the cap before ADR 0033 point 6, `--finest-level 26`), where it differs: only ground-1m.
/// (14,325 and 4,063 before render-floor-3, which drops the 4 and 3 ancestors above the floor.)
/// Re-pinned 2026-10-25 by `m8-relief-creases` (declared, as `PINS`): R 14,328 to 14,340, D 4,064 to 4,077 (+0.08 and +0.3 %).
const PINS_L26_GROUND_1M: (usize, usize) = (14340, 4077);

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
    let measured: Vec<String> = l22.iter().map(|m| format!("(\"{}\", {}, {}),", m.0, m.1, m.2)).collect();
    assert!(
        failures.is_empty(),
        "slot demand against its pins:\n{}\nmeasured (R, D with ancestors):\n{}",
        failures.join("\n"),
        measured.join("\n")
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
}
