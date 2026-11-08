//! Tier A, GEN-141 (A3 sections 6.2 to 6.4): the two round kernels (the MFD area clipped to the D8 catchment, the flood visiting by level then cell id) and the round as a
//! pure function of its inputs. G-ROUND: a carve that visits one catchment after the other, in either catchment order, is the same bits as the region-order carve, under today's rules and
//! under the round rules. Closure: under the clip the area at a catchment's outlet is the sum of its members' areas. The default rules are untouched. Small grids; no store read.

use planet_generators::bake::carve::{carve, CarveParams, Schedule};
use planet_generators::bake::flood::{priority_flood, priority_flood_with};
use planet_generators::bake::route::{catchment_roots, route_rules, route_weighted, MfdClip, RoundRules};
use planet_generators::bake::Grid;

const N: usize = 72;

/// A land of ridges with a coast plain; `terraced` quantises it into flats and basins.
fn terrain(salt: u64, terraced: bool) -> Vec<f64> {
    let mut z = vec![0.0; N * N];
    for j in 0..N {
        for i in 0..N {
            let (x, y) = (i as f64, j as f64);
            let h = planet_generators::bake::route::jitter(salt, (j * N + i) as u64, 0);
            let edge = (i.min(j).min(N - 1 - i).min(N - 1 - j) as f64 / 12.0).min(1.0);
            let v = (350.0
                + 500.0 * libm::sin(x * 0.13 + salt as f64) * libm::cos(y * 0.11)
                + 250.0 * libm::sin(0.37 * x + 0.21 * y)
                + 30.0 * h)
                * edge
                + 2.0;
            z[j * N + i] = if terraced { libm::floor(v / 60.0) * 60.0 + 2.0 + 0.3 * h.abs() } else { v };
        }
    }
    z
}

// spec: GEN-141
#[test]
fn the_default_rules_are_todays_kernels() {
    let g = Grid::new(N, N, 800.0);
    let z = terrain(2, true);
    let outlet = g.outlets(&z, 0.0);
    let a = route_weighted(&g, &z, &outlet, 1e-3, 2, 0.0, None);
    let b = route_rules(&g, &z, &outlet, 1e-3, 2, 0.0, None, RoundRules::default());
    assert_eq!(a, b);
    // A digest of the routing and a 20-step carve of this land, taken on the kernels before GEN-141 (origin/main at 70743083): the default path is bit for bit what it was.
    let mut d: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| d = (d ^ v).wrapping_mul(0x100_0000_01b3);
    for p in 0..N * N {
        mix(u64::from(a.receiver[p]));
        mix(u64::from(a.order[p]));
        mix(a.area[p].to_bits());
    }
    let mut zz = z.clone();
    carve(&g, &mut zz, &outlet, &CarveParams { steps: 20, ..CarveParams::default() }, None, 2);
    zz.iter().for_each(|v| mix(v.to_bits()));
    assert_eq!(d, 0xbbde_6e85_47d2_338a, "the default kernels moved: digest {d:016x}");
    assert_eq!(priority_flood(&g, &z, &outlet, 1e-3), priority_flood_with(&g, &z, &outlet, 1e-3, false));
    assert!(RoundRules::default().is_default() && !RoundRules::ROUND.is_default());
    assert_eq!(CarveParams::default().schedule, Schedule::Region);
    assert!(CarveParams::default().round_rules.is_default());
}

// spec: GEN-141
#[test]
fn the_canonical_flood_is_a_valid_fill_that_does_not_depend_on_anything_but_its_input() {
    let g = Grid::new(N, N, 800.0);
    for terraced in [false, true] {
        let z = terrain(3, terraced);
        let outlet = g.outlets(&z, 0.0);
        let f = priority_flood_with(&g, &z, &outlet, 1e-3, true);
        assert_eq!(f, priority_flood_with(&g, &z, &outlet, 1e-3, true), "two runs differ");
        for p in 0..N * N {
            assert!(f[p] >= z[p], "cell {p} is below its own height");
            if !outlet[p] {
                assert!(
                    (0..8).filter_map(|d| g.neighbour(p, d)).any(|q| f[q] < f[p]),
                    "cell {p} has no lower neighbour on the filled surface"
                );
            }
        }
    }
}

// spec: GEN-141
#[test]
fn under_the_local_clip_a_catchments_outlet_holds_exactly_the_area_of_its_members() {
    let g = Grid::new(N, N, 800.0);
    for terraced in [false, true] {
        let z = terrain(4, terraced);
        let outlet = g.outlets(&z, 0.0);
        let mut moved_by_off = false;
        for (rules, closed) in [
            (RoundRules::ROUND, true),
            (RoundRules { clip_mfd: MfdClip::ToReceiver, canonical_flood: true }, true),
            (RoundRules { canonical_flood: true, ..Default::default() }, false),
        ] {
            let r = route_rules(&g, &z, &outlet, 1e-3, 4, 0.0, None, rules);
            let root = catchment_roots(&r.receiver, &r.order);
            let mut members = vec![0.0f64; N * N];
            for p in 0..N * N {
                members[root[p] as usize] += g.cell_area(p);
            }
            for p in (0..N * N).filter(|&p| root[p] as usize == p) {
                let ok = (r.area[p] - members[p]).abs() <= 1e-9 * members[p];
                if closed {
                    assert!(ok, "{rules:?}: the outlet {p} holds {} of {} m2", r.area[p], members[p]);
                } else {
                    moved_by_off |= !ok;
                }
            }
        }
        assert!(moved_by_off, "terraced {terraced}: without the clip no area crosses a divide, the fixture does not test the clip");
    }
}

// spec: GEN-141
#[test]
fn on_a_land_without_flats_a_catchment_routed_alone_is_its_part_of_the_region_routing() {
    // The local clip makes the accumulation closed under the catchment; the flood's reach across a divide shows only on flats (the measured limit, design.md), so this fixture has none.
    let g = Grid::new(N, N, 800.0);
    let z = terrain(5, false);
    let outlet = g.outlets(&z, 0.0);
    let r = route_rules(&g, &z, &outlet, 1e-3, 5, 0.0, None, RoundRules::ROUND);
    let root = catchment_roots(&r.receiver, &r.order);
    let mut roots = root.clone();
    roots.sort_unstable();
    roots.dedup();
    let mut checked = 0;
    for &rt in &roots {
        let active: Vec<bool> = root.iter().map(|&x| x == rt).collect();
        if active.iter().filter(|&&a| a).count() < 20 {
            continue;
        }
        let ga = g.clone().with_active(active.clone());
        let mut out = vec![false; N * N];
        out[rt as usize] = true;
        let a = route_rules(&ga, &z, &out, 1e-3, 5, 0.0, None, RoundRules::ROUND);
        for p in (0..N * N).filter(|&p| active[p]) {
            assert_eq!(a.receiver[p], r.receiver[p], "catchment {rt} cell {p}: the receiver");
            assert_eq!(a.area[p].to_bits(), r.area[p].to_bits(), "catchment {rt} cell {p}: the area");
        }
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} catchments checked");
}

// spec: GEN-141
#[test]
fn g_round_a_round_carved_one_catchment_after_the_other_is_the_region_carve() {
    let g = Grid::new(N, N, 800.0);
    for terraced in [false, true] {
        let z = terrain(6, terraced);
        let outlet = g.outlets(&z, 0.0);
        for rules in [RoundRules::default(), RoundRules::ROUND] {
            let run = |schedule| {
                let mut zz = z.clone();
                carve(&g, &mut zz, &outlet, &CarveParams { steps: 20, round_rules: rules, schedule, ..CarveParams::default() }, None, 6);
                zz.iter().map(|v| v.to_bits()).collect::<Vec<u64>>()
            };
            let region = run(Schedule::Region);
            let moved = region.iter().zip(&z).filter(|(a, b)| **a != b.to_bits()).count();
            assert!(moved > N * N / 4, "the carve moved only {moved} cells");
            for descending in [false, true] {
                let by = run(Schedule::Catchments { descending });
                let differ = by.iter().zip(&region).filter(|(a, b)| a != b).count();
                assert_eq!(differ, 0, "terraced {terraced} {rules:?} descending {descending}: {differ} cells differ from the region carve");
            }
        }
    }
}

// spec: GEN-141
#[test]
fn a_masked_grid_shows_the_kernels_only_its_cells() {
    let g = Grid::new(6, 6, 100.0);
    let mut active = vec![true; 36];
    active[7] = false;
    let m = g.with_active(active);
    assert!(!m.is_active(7) && m.is_active(8));
    assert_eq!(m.neighbour(7, 0), None, "a masked cell has no neighbours");
    assert_eq!(m.neighbour(8, 0), None, "no cell has a masked cell as a neighbour");
    assert_eq!(m.neighbour(8, 1), Some(9));
}
