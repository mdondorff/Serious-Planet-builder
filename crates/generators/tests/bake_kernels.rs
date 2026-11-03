//! Tier A, GEN-100 (m9-bake PR 3a): the routing and carve kernels on synthetic surfaces: a pit, a ramp, a cone, a dam and a coast, with the expected results, the order and area
//! conserved, the outlets kept, land kept above the sea, windows that route a cell alike, bad input refused, and the same bits on every run and platform. Small grids; no planet bake.

use planet_generators::bake::carve::{carve, CarveParams};
use planet_generators::bake::flood::priority_flood;
use planet_generators::bake::route::{route, NONE};
use planet_generators::bake::Grid;

const DX: f64 = 800.0;

fn grid(n: usize) -> Grid {
    Grid::new(n, n, DX)
}

/// A tilted plane draining to the west: height `10 i` metres over a sea-level column at `i = 0`.
fn ramp(n: usize) -> Vec<f64> {
    (0..n * n).map(|p| 10.0 * (p % n) as f64).collect()
}

/// A plateau at 100 m with a pit of 20 m in its middle.
fn bowl(n: usize) -> Vec<f64> {
    let c = n as f64 / 2.0;
    (0..n * n)
        .map(|p| {
            let (i, j) = ((p % n) as f64, (p / n) as f64);
            if libm::hypot(i - c, j - c) < 3.0 {
                20.0
            } else {
                100.0
            }
        })
        .collect()
}

/// A cone: 3000 m at the middle, falling to 0 at the edge.
fn cone(n: usize) -> Vec<f64> {
    let c = (n as f64 - 1.0) / 2.0;
    (0..n * n)
        .map(|p| {
            let r = libm::hypot((p % n) as f64 - c, (p / n) as f64 - c);
            (3000.0 * (1.0 - r / c)).max(0.0)
        })
        .collect()
}

fn bits(z: &[f64]) -> Vec<u64> {
    z.iter().map(|v| v.to_bits()).collect()
}

fn fnv(v: &[u64]) -> u64 {
    v.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, x| (h ^ x).wrapping_mul(0x0000_0100_0000_01b3))
}

// spec: GEN-100
#[test]
fn the_flood_fills_a_pit_to_its_pour_point_and_every_cell_drains() {
    let g = grid(24);
    let mut z = bowl(24);
    // The pour point: a channel from the pit to the west edge at 60 m.
    for i in 0..10 {
        z[12 * 24 + i] = 60.0;
    }
    let outlet = g.outlets(&z, -1.0);
    let filled = priority_flood(&g, &z, &outlet, 1e-3);
    for p in 0..z.len() {
        assert!(filled[p] >= z[p], "the flood lowered cell {p}");
        if outlet[p] {
            assert_eq!(filled[p].to_bits(), z[p].to_bits(), "an outlet moved");
        } else {
            let lower = (0..8).filter_map(|d| g.neighbour(p, d)).any(|q| filled[q] < filled[p]);
            assert!(lower, "cell {p} at {} has no lower neighbour: a pit is left", filled[p]);
        }
    }
    let pit = 12 * 24 + 12;
    assert!(filled[pit] > 59.9 && filled[pit] < 61.0, "the pit fills to {}", filled[pit]);
}

// spec: GEN-100
#[test]
fn a_ramp_drains_downhill_the_order_follows_the_receivers_and_the_area_is_conserved() {
    let n = 20;
    let g = grid(n);
    let z = ramp(n);
    let outlet = g.outlets(&z, 0.0);
    let r = route(&g, &z, &outlet, 1e-3, 7, 0.0);
    let filled = priority_flood(&g, &z, &outlet, 1e-3);
    for (p, &is_outlet) in outlet.iter().enumerate() {
        if is_outlet {
            assert_eq!(r.receiver[p], NONE);
        } else {
            let rec = r.receiver[p];
            assert!(rec != NONE && filled[rec as usize] < filled[p], "cell {p} drains uphill or nowhere");
        }
    }
    // Each cell comes after its receiver.
    let mut at = vec![0usize; n * n];
    for (k, &p) in r.order.iter().enumerate() {
        at[p as usize] = k;
    }
    assert_eq!(r.order.len(), n * n);
    for p in 0..n * n {
        if r.receiver[p] != NONE {
            assert!(at[r.receiver[p] as usize] < at[p], "cell {p} comes before its receiver");
        }
    }
    // The area leaving the outlets is the area of the grid (each cell's area is shared, none is lost or counted twice; the shares are not integer multiples of the cell area, so
    // the sum is judged to rounding).
    let total: f64 = (0..n * n).filter(|&p| outlet[p]).map(|p| r.area[p]).sum();
    let whole = (n * n) as f64 * DX * DX;
    assert!((total - whole).abs() <= 1e-9 * whole, "{total} against {whole}");
    let capped = route(&g, &z, &outlet, 1e-3, 7, 4.0 * DX * DX);
    assert!(capped.area.iter().all(|&a| a <= 4.0 * DX * DX));
}

// spec: GEN-100
#[test]
fn a_cell_routes_alike_in_every_window_that_holds_it_because_the_jitter_follows_the_global_cell() {
    // A smooth surface without pits, so that the filled surface is the surface: the receivers of the interior of an inner window equal those of the big window.
    let (n, m, off) = (30usize, 16usize, 7usize);
    let z: Vec<f64> = (0..n * n).map(|p| 10.0 * (p % n) as f64 + 3.0 * libm::sin(0.7 * (p / n) as f64)).collect();
    let big = grid(n);
    let big_r = route(&big, &z, &big.outlets(&z, -1.0), 1e-3, 9, 0.0);
    let inner_z: Vec<f64> = (0..m * m).map(|q| z[(off + q / m) * n + off + q % m]).collect();
    let inner = Grid::new(m, m, DX).with_origin((off as u32, off as u32));
    let inner_r = route(&inner, &inner_z, &inner.outlets(&inner_z, -1.0), 1e-3, 9, 0.0);
    let mut compared = 0;
    for q in 0..m * m {
        let (i, j) = (q % m, q / m);
        if i == 0 || j == 0 || i == m - 1 || j == m - 1 {
            continue;
        }
        let (bp, ip) = ((off + j) * n + off + i, q);
        let (br, ir) = (big_r.receiver[bp] as usize, inner_r.receiver[ip] as usize);
        assert_eq!((br % n - off, br / n - off), (ir % m, ir / m), "cell ({i}, {j}) routes differently in the other window");
        compared += 1;
    }
    assert!(compared > 100);
}

// spec: GEN-100
#[test]
fn the_carve_keeps_the_outlets_uplifts_the_land_stays_above_the_receivers_and_is_the_same_bits_on_every_run() {
    let n = 32;
    let g = grid(n);
    let cone = cone(n);
    let outlet = g.outlets(&cone, 0.0);
    let params = CarveParams::default();
    let run = || {
        let mut z = cone.clone();
        let r = carve(&g, &mut z, &outlet, &params, None, 11);
        (z, r)
    };
    let (z, r) = run();
    let (z2, r2) = run();
    assert_eq!(bits(&z), bits(&z2), "the carve differs between runs");
    assert_eq!(r, r2);
    // A pinned fingerprint of the output bits: the same on every platform (CON-14); re-record it when the kernels or their parameters change on purpose.
    println!("cone carve fingerprint {}", fnv(&bits(&z)));
    assert_eq!(fnv(&bits(&z)), CONE_FINGERPRINT, "the carve's output changed (or differs on this platform)");
    let uplift = params.u_over_k * params.k_dt * params.steps as f64;
    let mut rose = 0;
    for p in 0..n * n {
        if outlet[p] {
            assert_eq!(z[p].to_bits(), cone[p].to_bits(), "an outlet moved");
        } else {
            assert!(z[p] <= cone[p] + uplift + 1e-9, "cell {p} rose by more than the uplift");
            rose += usize::from(z[p] > cone[p]);
            let rec = r.receiver[p];
            if rec != NONE {
                assert!(z[p] > z[rec as usize], "cell {p} is not above its receiver after the carve");
            }
        }
    }
    // With the area shared among the lower neighbours (the multiple-flow-direction area, kernel version 2) the default uplift of 20 m is out-incised on this cone; an uplift ten times
    // the default is not, so the uplift term is still applied and shows.
    let mut z_up = cone.clone();
    carve(&g, &mut z_up, &outlet, &CarveParams { u_over_k: 10.0 * params.u_over_k, ..params }, None, 11);
    rose += (0..n * n).filter(|&p| !outlet[p] && z_up[p] > cone[p]).count();
    assert!(rose > 0, "no cell was uplifted");
    let (trunk, _) = (0..n * n).filter(|&p| !outlet[p]).map(|p| (p, r.area[p])).fold((0, 0.0), |b, x| if x.1 > b.1 { x } else { b });
    assert!(
        r.area[trunk] > 10.0 * DX * DX && cone[trunk] - z[trunk] > 0.0,
        "the trunk of {} cells was not incised",
        r.area[trunk] / (DX * DX)
    );
}

const CONE_FINGERPRINT: u64 = 1974223322930457283;

// spec: GEN-100
#[test]
fn a_breach_lowers_the_receiver_of_a_dam_and_land_stays_above_the_sea() {
    // A basin at 200 m east of a dam (30 m above the floor), a rim of the dam's height on its other three sides, the sea (0 m) west of it.
    let n = 24;
    let g = grid(n);
    let make = |dam: f64| -> Vec<f64> {
        (0..n * n)
            .map(|p| {
                let (i, j) = (p % n, p / n);
                if i < 3 {
                    0.0
                } else if i == 5 || i == n - 1 || j == 0 || j == n - 1 {
                    dam
                } else if i < 5 {
                    50.0
                } else {
                    200.0
                }
            })
            .collect()
    };
    let one_step = |dam: f64, breach_max: f64| {
        let mut z = make(dam);
        let outlet = g.outlets(&z, 0.0);
        let params = CarveParams { steps: 1, breach_max_m: breach_max, ..CarveParams::default() };
        carve(&g, &mut z, &outlet, &params, None, 3);
        z
    };
    let crest = 12 * n + 5;
    let (low, none) = (one_step(230.0, 50.0), one_step(230.0, 0.0));
    assert!(low[crest] < none[crest], "the breach changed nothing at the crest: {} against {}", low[crest], none[crest]);
    let (tall, tall_none) = (one_step(400.0, 50.0), one_step(400.0, 0.0));
    assert!(tall[crest] < tall_none[crest], "a tall dam was not breached");
    for p in 0..n * n {
        if !g.is_edge(p) && p % n >= 3 {
            assert!(tall[p] > 0.0 && low[p] > 0.0, "land cell {p} fell to the sea level");
        }
    }
}

// spec: GEN-100
#[test]
fn land_next_to_a_deep_sea_stays_above_the_sea_level_after_the_carve() {
    let n = 12;
    let g = grid(n);
    let mut z: Vec<f64> = (0..n * n).map(|p| if p % n < 2 { -3000.0 } else { 5.0 }).collect();
    let outlet = g.outlets(&z, 0.0);
    carve(&g, &mut z, &outlet, &CarveParams::default(), None, 5);
    for (p, &h) in z.iter().enumerate() {
        if !outlet[p] {
            assert!(h > 0.0, "land cell {p} is at {h} m after the carve beside a sea at -3000 m");
        }
    }
}

// spec: GEN-100
#[test]
fn the_carve_factor_scales_the_uplift_and_input_that_cannot_be_routed_is_refused() {
    let n = 16;
    let g = grid(n);
    let flat = vec![100.0f64; n * n];
    let outlet = g.outlets(&flat, 0.0);
    let params = CarveParams { steps: 1, ..CarveParams::default() };
    let rise = |factor: f64| {
        let mut z = flat.clone();
        let f = vec![factor; n * n];
        carve(&g, &mut z, &outlet, &params, Some(&f), 1);
        z.iter().zip(&flat).map(|(a, b)| a - b).fold(f64::MIN, f64::max)
    };
    assert!(rise(0.5) < rise(1.0), "half the factor, less uplift");
    let refused = |f: fn()| std::panic::catch_unwind(f).is_err();
    assert!(
        refused(|| {
            let g3 = Grid::new(3, 3, DX);
            let mut z = vec![f64::NAN; 9];
            carve(&g3, &mut z, &g3.outlets(&[0.0; 9], 0.0), &CarveParams::default(), None, 1);
        }),
        "a NaN height is refused"
    );
    assert!(
        refused(|| {
            let g3 = Grid::new(3, 3, DX);
            priority_flood(&g3, &[1.0; 9], &[false; 9], 1e-3);
        }),
        "a flood without an outlet is refused"
    );
    assert!(
        refused(|| {
            let g3 = Grid::new(3, 3, DX);
            priority_flood(&g3, &[1.0; 9], &g3.outlets(&[1.0; 9], 0.0), 0.0);
        }),
        "an epsilon of 0 is refused"
    );
    assert!(
        refused(|| {
            let g3 = Grid::new(3, 3, DX);
            let mut z = vec![5.0; 9];
            let outlet = g3.outlets(&z.clone(), 0.0);
            carve(&g3, &mut z, &outlet, &CarveParams::default(), Some(&[1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0]), 1);
        }),
        "a negative carve factor is refused"
    );
}

/// The flood with every outlet seeded into the heap (the textbook seeding), for the comparison below.
fn flood_seeding_every_outlet(g: &Grid, z: &[f64], outlet: &[bool], eps: f64) -> Vec<f64> {
    use std::cmp::Reverse;
    use std::collections::{BinaryHeap, VecDeque};
    let key = |v: f64| {
        let b = v.to_bits();
        if b & (1 << 63) != 0 {
            !b
        } else {
            b | (1 << 63)
        }
    };
    let n = g.len();
    let (mut filled, mut seen) = (vec![0.0; n], vec![false; n]);
    let (mut heap, mut flat) = (BinaryHeap::new(), VecDeque::new());
    for p in 0..n {
        if outlet[p] {
            filled[p] = z[p];
            seen[p] = true;
            heap.push(Reverse((key(z[p]), p as u32)));
        }
    }
    loop {
        let p = if let Some(p) = flat.pop_front() {
            p as usize
        } else if let Some(Reverse((_, p))) = heap.pop() {
            p as usize
        } else {
            break;
        };
        for d in 0..8 {
            let Some(q) = g.neighbour(p, d) else { continue };
            if seen[q] {
                continue;
            }
            seen[q] = true;
            let lowest = filled[p] + eps;
            if z[q] <= lowest {
                filled[q] = lowest;
                flat.push_back(q as u32);
            } else {
                filled[q] = z[q];
                heap.push(Reverse((key(z[q]), q as u32)));
            }
        }
    }
    filled
}

// spec: GEN-100
#[test]
fn seeding_only_the_outlets_that_expand_gives_the_same_filled_surface_as_seeding_every_outlet() {
    let n = 40;
    let g = grid(n);
    // A face that is mostly sea: a cone island in a sea of -50 m, and the bowl with a sea of its own.
    for z in [
        (0..n * n)
            .map(|p| {
                let r = libm::hypot((p % n) as f64 - 20.0, (p / n) as f64 - 20.0);
                if r < 8.0 {
                    400.0 * (1.0 - r / 8.0) + 3.0 * libm::sin(1.3 * (p % n) as f64)
                } else {
                    -50.0
                }
            })
            .collect::<Vec<f64>>(),
        bowl(n),
        ramp(n),
    ] {
        let outlet = g.outlets(&z, 0.0);
        let (a, b) = (priority_flood(&g, &z, &outlet, 1e-3), flood_seeding_every_outlet(&g, &z, &outlet, 1e-3));
        assert_eq!(bits(&a), bits(&b), "the two seedings fill differently");
    }
}

// spec: GEN-100
#[test]
fn land_stays_above_a_sea_level_that_is_not_zero_and_the_jitter_lies_in_its_range() {
    let n = 12;
    let g = grid(n);
    let mut z: Vec<f64> = (0..n * n).map(|p| if p % n < 2 { 40.0 } else { 150.0 }).collect();
    let outlet = g.outlets(&z, 100.0);
    let params = CarveParams { sea_level: 100.0, ..CarveParams::default() };
    carve(&g, &mut z, &outlet, &params, None, 5);
    for (p, &h) in z.iter().enumerate() {
        if !outlet[p] {
            assert!(h > 100.0, "land cell {p} is at {h} m, below the sea level of 100 m");
        }
    }
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for id in 0..5_000u64 {
        for d in 0..8 {
            let u = planet_generators::bake::route::jitter(3, id | (id << 33), d);
            lo = lo.min(u);
            hi = hi.max(u);
        }
    }
    assert!(lo >= -1.0 && hi < 1.0 && lo < -0.99 && hi > 0.99, "the jitter spans [{lo}, {hi}]");
}

// spec: GEN-100
#[test]
fn the_drainage_area_is_shared_among_the_lower_neighbours_whatever_the_jitter_and_the_grid_axes() {
    // A plane falling along the diagonal towards the (0, 0) corner, its other edges the outlets: heights `10 (i + j) + 5`.
    let n = 24;
    let g = grid(n);
    let z: Vec<f64> = (0..n * n).map(|p| 10.0 * ((p % n) + (p / n)) as f64 + 5.0).collect();
    let outlet: Vec<bool> = (0..n * n).map(|p| p % n == 0 || p / n == 0 || p % n == n - 1 || p / n == n - 1).collect();
    let (a, b) = (route(&g, &z, &outlet, 1e-3, 1, 0.0), route(&g, &z, &outlet, 1e-3, 2, 0.0));
    // The receivers follow the jitter, the area does not.
    assert!(a.receiver != b.receiver, "the two jitter streams routed alike: the test proves nothing");
    assert_eq!(
        a.area.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        b.area.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        "the area depends on the jitter"
    );
    // Mirror symmetry about the diagonal: the area does not prefer the i axis to the j axis.
    for (i, j) in [(3, 6), (10, 4), (7, 15), (12, 12)] {
        let (p, q) = (j * n + i, i * n + j);
        assert!(
            (a.area[p] - a.area[q]).abs() <= 1e-9 * a.area[p],
            "cells ({i}, {j}) and ({j}, {i}) differ: {} against {}",
            a.area[p],
            a.area[q]
        );
    }
    // A cell off the diagonal receives from the cells above it, which a D8 tree sends elsewhere; every interior cell holds more than its own cell.
    let own = DX * DX;
    assert!(a.area[6 * n + 3] > 1.5 * own, "cell (3, 6) holds {} cells of area", a.area[6 * n + 3] / own);
    let total: f64 = (0..n * n).filter(|&p| outlet[p]).map(|p| a.area[p]).sum();
    assert!((total - (n * n) as f64 * own).abs() <= 1e-9 * total, "the area is not conserved: {total}");
}
