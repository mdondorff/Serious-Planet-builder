//! Tier A, GEN-146 (A3 section 6.5, gates G-EQ and G-INC at kernel scale): the round solver with drivers is the carve of the same drivers; the memoised re-solve of an edited surface, of edited
//! runoff weights and of edited drivers is the solve of the edited definition bit for bit (G-EQ) with only the work of what changed (G-INC); the basin objects keep their identity across an edit
//! (merge, split, an intentional basin) and the continuity map follows the overlap of cells (A3 section 5.4). Small grids; no store read.

use planet_generators::bake::carve::{carve_rock, CarveParams, Rock};
use planet_generators::bake::flood::priority_flood_with;
use planet_generators::bake::hydro::{basin_continuity, basin_id, hydrology_with, BasinClass, HydroParams};
use planet_generators::bake::metric::Metric;
use planet_generators::bake::partition::{partitioned_route_weighted, PartCache};
use planet_generators::bake::round::{solve_rounds_memo, solve_rounds_with, Drivers, RoundCache};
use planet_generators::bake::route::{route_rules, RoundRules};
use planet_generators::bake::Grid;

const N: usize = 72;
const R: f64 = 6_371_000.0;
const EPS: f64 = 1e-3;

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

fn grids() -> Vec<(&'static str, Grid)> {
    let dx = R * core::f64::consts::FRAC_PI_2 / 12288.0;
    vec![
        ("uniform", Grid::new(N, N, 800.0)),
        ("metric", Grid::new(N, N, dx).with_origin((8100, 900)).with_metric(Metric::cube_face(12288, R, (8100, 900), N, N))),
    ]
}

fn params(rounds: usize) -> CarveParams {
    CarveParams { steps: rounds * 5, reroute_every: 5, round_rules: RoundRules::ROUND, ..CarveParams::default() }
}

struct Fields {
    k: Vec<f64>,
    uplift: Vec<f32>,
    runoff: Vec<f32>,
}

fn fields() -> Fields {
    Fields {
        k: (0..N * N).map(|p| 0.5 + (p % 11) as f64 * 0.25).collect(),
        uplift: (0..N * N).map(|p| 10.0 + (p % 13) as f32 * 2.5).collect(),
        runoff: (0..N * N).map(|p| 0.3 + (p % 7) as f32 * 0.4).collect(),
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

// spec: GEN-146
#[test]
fn the_round_solver_with_drivers_is_the_carve_of_the_same_drivers() {
    let f = fields();
    for (name, g) in grids() {
        for terraced in [false, true] {
            let z0 = terrain(3, terraced);
            let outlet = g.outlets(&z0, 0.0);
            let p = params(2);
            let dr = Drivers { k_scale: Some(&f.k), uplift: Some(&f.uplift), runoff: Some(&f.runoff) };
            let mut want = z0.clone();
            let rock = Rock { par: &1usize, k: None, uplift: Some(&f.uplift), runoff: Some(&f.runoff) };
            carve_rock(&g, &mut want, &outlet, &p, Some(&f.k), &rock, 3);
            for threads in [1usize, 4] {
                let mut got = z0.clone();
                solve_rounds_with(&g, &mut got, &outlet, &p, 2, &threads, 3, &dr);
                assert_eq!(bits(&got), bits(&want), "G-ROUND with drivers ({name}, terraced {terraced}, {threads} threads)");
            }
            // The class factor alone, and the weights alone.
            for (label, dr) in [
                ("class factor", Drivers { k_scale: Some(&f.k), ..Drivers::default() }),
                ("runoff weights", Drivers { runoff: Some(&f.runoff), ..Drivers::default() }),
                ("uplift field", Drivers { uplift: Some(&f.uplift), ..Drivers::default() }),
            ] {
                let mut want = z0.clone();
                let rock = Rock { par: &1usize, k: None, uplift: dr.uplift, runoff: dr.runoff };
                carve_rock(&g, &mut want, &outlet, &p, dr.k_scale, &rock, 3);
                let mut got = z0.clone();
                solve_rounds_with(&g, &mut got, &outlet, &p, 2, &2usize, 3, &dr);
                assert_eq!(bits(&got), bits(&want), "{label} ({name}, terraced {terraced})");
            }
        }
    }
}

// spec: GEN-146
#[test]
fn the_partitioned_route_takes_the_runoff_weights() {
    let f = fields();
    for (name, g) in grids() {
        let z = terrain(4, true);
        let outlet = g.outlets(&z, 0.0);
        let want = route_rules(&g, &z, &outlet, EPS, 4, 0.0, Some(&f.runoff), RoundRules::ROUND);
        for tile in [8usize, 13] {
            let got =
                partitioned_route_weighted(&g, &z, &outlet, EPS, 4, tile, RoundRules::ROUND, Some(&f.runoff), &mut PartCache::default());
            assert_eq!(got.routing.receiver, want.receiver, "receivers ({name}, tile {tile})");
            assert_eq!(bits(&got.routing.area), bits(&want.area), "A_d ({name}, tile {tile})");
        }
    }
}

/// The solve of the same inputs without a cache: the definition of the edited world (a fresh build).
fn fresh(g: &Grid, z0: &[f64], outlet: &[bool], p: &CarveParams, dr: &Drivers, rounds: usize) -> Vec<f64> {
    let mut z = z0.to_vec();
    solve_rounds_with(g, &mut z, outlet, p, rounds, &1usize, 3, dr);
    z
}

// spec: GEN-146
#[test]
fn a_memoised_re_solve_is_the_solve_of_the_edited_definition_with_the_work_of_the_change() {
    let f = fields();
    for (name, g) in grids() {
        let z0 = terrain(5, true);
        let outlet = g.outlets(&z0, 0.0);
        let p = params(4);
        let base_dr = Drivers { k_scale: Some(&f.k), uplift: Some(&f.uplift), runoff: Some(&f.runoff) };
        let tile = 12;
        let mut cache = RoundCache::default();
        let mut base = z0.clone();
        let (_, w0) = solve_rounds_memo(&g, &mut base, &outlet, &p, 4, &1usize, 3, &base_dr, tile, &mut cache);
        assert_eq!(bits(&base), bits(&fresh(&g, &z0, &outlet, &p, &base_dr, 4)), "the first memoised solve is the fresh solve ({name})");
        assert!(w0.iter().all(|w| w.carved > 0 && w.tile_floods > 0));
        // The same definition again: nothing is flooded or carved.
        let mut again = z0.clone();
        let (_, w1) = solve_rounds_memo(&g, &mut again, &outlet, &p, 4, &3usize, 3, &base_dr, tile, &mut cache);
        assert_eq!(bits(&again), bits(&base));
        assert!(
            w1.iter().all(|w| w.carved == 0 && w.tile_floods == 0 && w.memo_hits == w.catchments),
            "no work on an unchanged definition ({name}): {w1:?}"
        );
        let total_carved: usize = w0.iter().map(|w| w.carved).sum();

        // E1 and E2 type edits of the surface: a raised block, a lowered pass.
        let mut edited = z0.clone();
        for j in 30..36 {
            for i in 40..46 {
                edited[j * N + i] += 400.0;
            }
        }
        edited[20 * N + 20] -= 60.0;
        let mut got = edited.clone();
        let (_, w2) = solve_rounds_memo(&g, &mut got, &outlet, &p, 4, &1usize, 3, &base_dr, tile, &mut cache);
        assert_eq!(bits(&got), bits(&fresh(&g, &edited, &outlet, &p, &base_dr, 4)), "G-EQ after a surface edit ({name})");
        let carved: usize = w2.iter().map(|w| w.carved).sum();
        assert!(
            w2[0].tiles_filled_changed >= 1 && w2[0].tiles_filled_changed < w2[0].tiles,
            "the change counts read against the same round of the previous solve ({name}): {:?}",
            w2[0]
        );
        assert!(
            w1.iter().all(|w| w.tiles_filled_changed + w.tiles_receiver_changed + w.tiles_area_changed == 0),
            "no tile changed on an unchanged definition ({name})"
        );
        assert!(
            carved > 0 && carved < total_carved,
            "the edit carves fewer catchments than a fresh build ({carved} of {total_carved}, {name})"
        );
        assert!(
            w2[0].tile_floods < w0[0].tile_floods,
            "the edit floods fewer tiles in round 1 ({} of {}, {name})",
            w2[0].tile_floods,
            w0[0].tile_floods
        );

        // E5 type: the runoff over a window doubled, no height change: no tile is flooded in round 1, the catchments of the window are carved again.
        let mut wet = f.runoff.clone();
        for j in 10..30 {
            for i in 10..30 {
                wet[j * N + i] *= 2.0;
            }
        }
        let dr_wet = Drivers { runoff: Some(&wet), ..base_dr };
        let mut got = z0.clone();
        let (_, w3) = solve_rounds_memo(&g, &mut got, &outlet, &p, 4, &1usize, 3, &dr_wet, tile, &mut cache);
        assert_eq!(bits(&got), bits(&fresh(&g, &z0, &outlet, &p, &dr_wet, 4)), "G-EQ after a rainfall edit ({name})");
        assert_eq!(w3[0].tile_floods, 0, "round 1 floods no tile when only the runoff changed ({name}): {:?}", w3[0]);
        assert!(w3.iter().map(|w| w.carved).sum::<usize>() > 0);

        // E6 type: a softer rock and twice the uplift over a block.
        let (mut k2, mut u2) = (f.k.clone(), f.uplift.clone());
        for j in 40..52 {
            for i in 40..52 {
                k2[j * N + i] *= 2.9;
                u2[j * N + i] *= 2.0;
            }
        }
        let dr_rock = Drivers { k_scale: Some(&k2), uplift: Some(&u2), runoff: Some(&f.runoff) };
        let mut got = z0.clone();
        let (_, w4) = solve_rounds_memo(&g, &mut got, &outlet, &p, 4, &1usize, 3, &dr_rock, tile, &mut cache);
        assert_eq!(bits(&got), bits(&fresh(&g, &z0, &outlet, &p, &dr_rock, 4)), "G-EQ after a rock and uplift edit ({name})");
        assert!(
            w4.iter().map(|w| w.carved).sum::<usize>() < total_carved,
            "the rock edit carves fewer catchments than a fresh build ({name})"
        );
    }
}

/// A 40 x 40 slope rising to the east with two bowls, floors 280 at (12, 12) and 270 at (12, 26), 6 x 6 cells each.
fn two_bowls() -> Vec<f64> {
    let w = 40;
    let mut z: Vec<f64> = (0..w * w).map(|p| 500.0 + 10.0 * (p % w) as f64).collect();
    for (j0, floor) in [(10usize, (12usize, 12usize, 280.0)), (24, (12, 26, 270.0))] {
        for j in j0..j0 + 6 {
            for i in 10..16 {
                z[j * w + i] = 300.0;
            }
        }
        z[floor.1 * w + floor.0] = floor.2;
    }
    z
}

// spec: GEN-146
#[test]
fn basins_merge_split_and_stay_intentional_with_their_identity_and_a_continuity_map() {
    let w = 40;
    let g = Grid::new(w, w, 800.0);
    let h1 = |z: &[f64], flags: Option<&[bool]>| {
        let outlet = g.outlets(z, 0.0);
        let routing = route_rules(&g, z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
        let filled = priority_flood_with(&g, z, &outlet, EPS, true);
        hydrology_with(&g, z, &filled, &routing, &outlet, None, None, flags, &HydroParams::default())
    };
    let z = two_bowls();
    let base = h1(&z, None);
    assert_eq!(base.basins.len(), 2);
    let (id_a, id_b) = (basin_id(0, g.global_id(12 * w + 12)), basin_id(0, g.global_id(26 * w + 12)));
    assert!(base.basins.iter().any(|b| b.id == id_a) && base.basins.iter().any(|b| b.id == id_b));

    // E3b: a trench joins them. One basin; the deeper floor's cell names it; both old ids map to it.
    let mut merged = z.clone();
    for j in 16..24 {
        for i in 11..14 {
            merged[j * w + i] = merged[j * w + i].min(300.0);
        }
    }
    let m = h1(&merged, None);
    assert_eq!(m.basins.len(), 1);
    assert_eq!(m.basins[0].id, id_b, "the deeper floor's cell survives");
    for (old, news) in basin_continuity(&base, &m) {
        assert_eq!(news.first().map(|x| x.0), Some(id_b), "old basin {old:x} maps to the merged basin");
    }

    // E3c: a sill raised across the trench splits it again; the parts have their old ids, and the continuity map follows the overlap.
    let mut split = merged.clone();
    for j in 19..21 {
        for i in 11..14 {
            split[j * w + i] = z[j * w + i] + 30.0;
        }
    }
    let s = h1(&split, None);
    assert_eq!(s.basins.len(), 2);
    assert!(
        s.basins.iter().any(|b| b.id == id_a) && s.basins.iter().any(|b| b.id == id_b),
        "the parts keep their lowest cells and their ids"
    );
    let map = basin_continuity(&m, &s);
    assert_eq!(map.len(), 1);
    assert_eq!(map[0].1.len(), 2, "the merged basin maps to both parts, by overlap");
    assert!(map[0].1[0].1 >= map[0].1[1].1, "in order of the cells shared");

    // E3d: a small dent flagged as intentional is kept whatever its size, through a raise of the whole land by 1,000 m.
    let mut dent = z.clone();
    dent[30 * w + 30] = 789.0;
    dent[30 * w + 29] = 790.0;
    let mut flags = vec![false; w * w];
    flags[30 * w + 30] = true;
    let plain = h1(&dent, None);
    assert_eq!(plain.basins.len(), 2, "the dent is an artefact without the flag");
    let named = h1(&dent, Some(&flags));
    assert_eq!(named.basins.len(), 3);
    let raised: Vec<f64> = dent.iter().map(|v| v + 1000.0).collect();
    let after = h1(&raised, Some(&flags));
    let id_d = basin_id(0, g.global_id(30 * w + 30));
    for h in [&named, &after] {
        let b = h.basins.iter().find(|b| b.id == id_d).expect("the intentional basin is kept");
        assert_eq!(b.class, BasinClass::Intentional);
    }
}

fn same_hydro(a: &planet_generators::bake::hydro::Hydro, b: &planet_generators::bake::hydro::Hydro) -> bool {
    bits(&a.q) == bits(&b.q)
        && bits(&a.qw) == bits(&b.qw)
        && a.channel == b.channel
        && a.segments == b.segments
        && a.basins == b.basins
        && a.label == b.label
        && a.stats == b.stats
}

// spec: GEN-146
#[test]
fn a_basin_resolves_after_the_losses_upstream_of_it_and_q_w_closes() {
    use planet_generators::bake::hydro::{hydrology_with, verify, HydroParams};
    let w = 40;
    let g = Grid::new(w, w, 800.0);
    // Two bowls in series on a slope falling west: the upper one (i 22..28) drains into the lower one (i 10..16).
    let mut z: Vec<f64> = (0..w * w).map(|p| 500.0 + 10.0 * (p % w) as f64).collect();
    for (i0, floor) in [(10usize, 280.0), (22, 270.0)] {
        for j in 10..16 {
            for i in i0..i0 + 6 {
                z[j * w + i] = 300.0;
            }
        }
        z[12 * w + i0 + 2] = floor;
    }
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let runoff = vec![1.0e-8f32; w * w];
    let run = |evap: Option<&[f32]>| hydrology_with(&g, &z, &filled, &routing, &outlet, Some(&runoff), evap, None, &HydroParams::default());
    let wet = run(None);
    assert_eq!(wet.basins.len(), 2);
    let find = |h: &planet_generators::bake::hydro::Hydro, i: usize| {
        h.basins.iter().position(|b| (b.floor_cell as usize % w) >= i && (b.floor_cell as usize % w) < i + 6).unwrap()
    };
    let (up, low) = (find(&wet, 22), find(&wet, 10));
    // Without evaporation both overflow and the lower basin receives everything above it.
    assert!(wet.basins[low].q_in > wet.basins[up].q_in, "the lower basin receives what the upper one passes on");
    // Strong evaporation over the upper basin only: it is endorheic and passes nothing; the lower one loses its inflow.
    let mut evap = vec![0.0f32; w * w];
    for j in 10..16 {
        for i in 22..28 {
            evap[j * w + i] = 1.0e-5;
        }
    }
    let dry = run(Some(&evap));
    let (up2, low2) = (find(&dry, 22), find(&dry, 10));
    assert_eq!(dry.basins[up2].class, BasinClass::Endorheic);
    assert_eq!(dry.basins[up2].q_out, 0.0);
    assert_eq!(dry.basins[up2].q_in.to_bits(), wet.basins[up].q_in.to_bits(), "the upper basin sees the same inflow");
    let want = wet.basins[low].q_in - dry.basins[up2].q_in;
    assert!(
        (dry.basins[low2].q_in - want).abs() <= 1e-9 * want,
        "inflow {} against the wet inflow less the upper loss {want}",
        dry.basins[low2].q_in
    );
    for h in [&wet, &dry] {
        let v = verify(&g, h, &routing, &filled, &outlet, Some(&runoff), None);
        assert!(v.qw_closure_error <= 1e-9 && v.qw_confluence_error <= 1e-9 && v.q_outlets_error <= 1e-9, "{v:?}");
    }
    // A lake in the chain passes `Q_in - A(spill) E_open` on.
    let mut some = vec![0.0f32; w * w];
    for j in 10..16 {
        for i in 10..16 {
            some[j * w + i] = 1.0e-9;
        }
    }
    let lake = run(Some(&some));
    let l = find(&lake, 10);
    assert_eq!(lake.basins[l].class, BasinClass::Lake);
    let a = lake.basins[l].area_m2[15];
    assert!((lake.basins[l].q_out - (lake.basins[l].q_in - a * f64::from(1.0e-9f32))).abs() <= 1e-12 * lake.basins[l].q_in);
    let v = verify(&g, &lake, &routing, &filled, &outlet, Some(&runoff), None);
    assert!(v.qw_closure_error <= 1e-9 && v.qw_confluence_error <= 1e-9, "{v:?}");
}

/// Two islands in a sea, a strait between them.
fn islands() -> Vec<f64> {
    let mut z: Vec<f64> = vec![-10.0; N * N];
    for j in 0..N {
        for i in 0..N {
            for (ci, cj) in [(18.0f64, 36.0f64), (54.0, 36.0)] {
                let (dx, dy) = (i as f64 - ci, j as f64 - cj);
                let d = libm::sqrt(dx * dx + dy * dy);
                if d < 13.0 {
                    let v = 600.0 * (1.0 - d / 13.0) + 3.0 * planet_generators::bake::route::jitter(9, (j * N + i) as u64, 0) + 2.0;
                    z[j * N + i] = z[j * N + i].max(v);
                }
            }
        }
    }
    z
}

// spec: GEN-146
#[test]
fn a_bridge_joins_two_landmasses_and_taking_it_away_gives_the_first_result_back() {
    use planet_generators::bake::hydro::{hydrology_with, HydroParams};
    let f = fields();
    for (name, g) in grids() {
        let z0 = islands();
        let outlet = g.outlets(&z0, 0.0);
        let p = params(4);
        let dr = Drivers { k_scale: Some(&f.k), uplift: Some(&f.uplift), runoff: Some(&f.runoff) };
        let tile = 12;
        let mut cache = RoundCache::default();
        let mut base = z0.clone();
        let (_, w0) = solve_rounds_memo(&g, &mut base, &outlet, &p, 4, &1usize, 3, &dr, tile, &mut cache);
        // A bridge of land across the strait: 3 cells wide, 25 m above the sea.
        let mut bridged = z0.clone();
        for i in 28..44 {
            for j in 35..38 {
                bridged[j * N + i] = 25.0;
            }
        }
        let outlet2 = g.outlets(&bridged, 0.0);
        let mut joined = bridged.clone();
        let (_, w1) = solve_rounds_memo(&g, &mut joined, &outlet2, &p, 4, &1usize, 3, &dr, tile, &mut cache);
        let mut fresh_joined = bridged.clone();
        solve_rounds_with(&g, &mut fresh_joined, &outlet2, &p, 4, &1usize, 3, &dr);
        assert_eq!(bits(&joined), bits(&fresh_joined), "G-EQ with the bridge ({name})");
        assert!(w1.iter().map(|w| w.carved).sum::<usize>() > 0);
        // The bridge taken away: the first result, with no work at all.
        let mut back = z0.clone();
        let (_, w2) = solve_rounds_memo(&g, &mut back, &outlet, &p, 4, &1usize, 3, &dr, tile, &mut cache);
        assert_eq!(bits(&back), bits(&base), "separating them gives the first result bit for bit ({name})");
        assert!(w2.iter().all(|w| w.carved == 0 && w.tile_floods == 0), "the memo answers all of it ({name}): {w2:?}");
        assert!(w0.iter().all(|w| w.carved > 0));
        // The complete result: H1 on the memoised partitioned routing equals H1 on the resident routing, before and after.
        for (surface, flags) in [(&base, &outlet), (&joined, &outlet2)] {
            let resident = route_rules(&g, surface, flags, EPS, 3, 0.0, Some(&f.runoff), RoundRules::ROUND);
            let filled = priority_flood_with(&g, surface, flags, EPS, true);
            let part = partitioned_route_weighted(&g, surface, flags, EPS, 3, tile, RoundRules::ROUND, Some(&f.runoff), &mut cache.part);
            let evap = vec![2.0e-9f32; N * N];
            let a = hydrology_with(&g, surface, &filled, &resident, flags, Some(&f.runoff), Some(&evap), None, &HydroParams::default());
            let b = hydrology_with(
                &g,
                surface,
                &part.filled,
                &part.routing,
                flags,
                Some(&f.runoff),
                Some(&evap),
                None,
                &HydroParams::default(),
            );
            assert!(same_hydro(&a, &b), "the complete H1 result is the same from the partitioned and the resident routing ({name})");
        }
    }
}

// spec: GEN-146
#[test]
#[should_panic(expected = "not negative")]
fn a_negative_open_water_evaporation_is_refused() {
    use planet_generators::bake::hydro::{hydrology_with, HydroParams};
    let g = Grid::new(24, 24, 800.0);
    let z: Vec<f64> = (0..24 * 24).map(|p| 100.0 + (p % 24) as f64 * 5.0).collect();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let filled = priority_flood_with(&g, &z, &outlet, EPS, true);
    let runoff = vec![1.0e-8f32; 24 * 24];
    let mut evap = vec![0.0f32; 24 * 24];
    evap[100] = -1.0e-9;
    hydrology_with(&g, &z, &filled, &routing, &outlet, Some(&runoff), Some(&evap), None, &HydroParams::default());
}

// spec: GEN-146
#[test]
fn the_taper_follows_the_uplift_field_when_the_uniform_ratio_is_zero_and_a_cache_serves_other_parameters() {
    let f = fields();
    for (name, g) in grids() {
        let z0 = terrain(8, true);
        let outlet = g.outlets(&z0, 0.0);
        let p0 = CarveParams { u_over_k: 0.0, ..params(2) };
        let dr = Drivers { uplift: Some(&f.uplift), ..Drivers::default() };
        let rock = Rock { par: &1usize, k: None, uplift: Some(&f.uplift), runoff: None };
        let mut want = z0.clone();
        carve_rock(&g, &mut want, &outlet, &p0, None, &rock, 3);
        let mut got = z0.clone();
        solve_rounds_with(&g, &mut got, &outlet, &p0, 2, &1usize, 3, &dr);
        assert_eq!(bits(&got), bits(&want), "an uplift field with u_over_k 0 ({name})");
        // One cache, three definitions of the carve: a changed k_dt and a height-scaled breach are other keys, never stale entries.
        let mut cache = RoundCache::default();
        for p in [
            params(2),
            CarveParams { k_dt: 0.03, ..params(2) },
            CarveParams { breach_height_scale_m: 300.0, breach_max_m: 300.0, ..params(2) },
            params(2),
        ] {
            let mut memo = z0.clone();
            solve_rounds_memo(&g, &mut memo, &outlet, &p, 2, &2usize, 3, &Drivers::default(), 12, &mut cache);
            assert_eq!(bits(&memo), bits(&fresh(&g, &z0, &outlet, &p, &Drivers::default(), 2)), "G-EQ across parameters ({name})");
        }
    }
}

// spec: GEN-146
#[test]
#[should_panic(expected = "does not stand above its receiver")]
fn the_cascade_refuses_a_surface_that_is_not_the_one_the_routing_was_made_on() {
    use planet_generators::bake::hydro::{hydrology_with, HydroParams};
    let g = Grid::new(24, 24, 800.0);
    let z: Vec<f64> = (0..24 * 24).map(|p| 100.0 + (p % 24) as f64 * 5.0).collect();
    let outlet = g.outlets(&z, 0.0);
    let routing = route_rules(&g, &z, &outlet, EPS, 3, 0.0, None, RoundRules::ROUND);
    let flat = vec![50.0f64; 24 * 24];
    hydrology_with(&g, &z, &flat, &routing, &outlet, None, None, None, &HydroParams::default());
}
