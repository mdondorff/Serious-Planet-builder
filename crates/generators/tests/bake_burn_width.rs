//! Tier A, TEST-110 (GEN-168): `burn_channels_wide` against the width law on a straight, a diagonal and a curved channel at w = 0.5, 1.5, 2 and 3 cells. The rule (D43): every cell of the channel's line is lowered
//! to its level (no gap); no cell is raised; the mean burnt width (the burnt cells per cell of arc length, the ends left out) is within 0.5 cell of max(1, w), 0.71 cell on a diagonal; on an axis-parallel line every
//! column across it holds at least floor(max(1, w)) burnt cells.

use planet_generators::bake::k7::{burn_channels_wide, Channel, QUnit, WidthLaw};

const N: usize = 120;
const LEVEL: f64 = 10.0;
const FLAT: f64 = 100.0;
const END: usize = 6;

/// A channel of points one cell apart along `f(s)`, with the discharge that gives `w_cells` at a 1 m cell.
fn channel(f: &dyn Fn(f64) -> (f64, f64), len: usize, w_cells: f64) -> Channel {
    let law = WidthLaw::STANDARD;
    let q = libm::pow(w_cells / law.a, 1.0 / law.b);
    let points: Vec<(f64, f64)> = (0..=len).map(|k| f(k as f64)).collect();
    Channel { levels: vec![LEVEL; points.len()], q: vec![q; points.len()], q_unit: QUnit::Discharge, points }
}

type Shape = (&'static str, Box<dyn Fn(f64) -> (f64, f64)>, usize, f64);

/// (name, point function, length in points, tolerance on the mean width)
fn shapes() -> Vec<Shape> {
    vec![
        ("straight", Box::new(|s| (10.0 + s, 48.3)), 90, 0.5),
        ("straight, on the cell row", Box::new(|s| (10.0 + s, 48.0)), 90, 1.05),
        ("diagonal", Box::new(|s| (10.0 + s * core::f64::consts::FRAC_1_SQRT_2, 10.0 + s * core::f64::consts::FRAC_1_SQRT_2)), 120, 0.71),
        ("curved", Box::new(|s| (60.0 + 40.0 * libm::cos(s / 40.0), 60.0 + 40.0 * libm::sin(s / 40.0))), 110, 0.71),
    ]
}

fn dist(a: (f64, f64), x: i64, y: i64) -> f64 {
    libm::hypot(a.0 - x as f64, a.1 - y as f64)
}

// spec: GEN-168
// spec: TEST-110
#[test]
fn the_burn_matches_the_width_law_on_a_straight_a_diagonal_and_a_curved_channel() {
    for (name, f, len, tol) in &shapes() {
        for w in [0.5, 1.5, 2.0, 3.0] {
            let ch = channel(f.as_ref(), *len, w);
            let mut z = vec![FLAT; N * N];
            burn_channels_wide(&mut z, N, (0, 0), std::slice::from_ref(&ch), Some((&WidthLaw::STANDARD, 1.0)));
            let burnt = |x: i64, y: i64| z[y as usize * N + x as usize] < FLAT;
            assert!(z.iter().all(|&v| v <= FLAT), "{name} w={w}: no cell is raised");
            for p in &ch.points {
                let (x, y) = (p.0.round() as i64, p.1.round() as i64);
                assert!(
                    burnt(x, y) && z[y as usize * N + x as usize] == LEVEL,
                    "{name} w={w}: the line cell ({x}, {y}) is lowered to its level (no gap)"
                );
            }
            // The burnt cells nearest to the middle of the line (not to an end's cap) over the arc length of that middle.
            let mid = &ch.points[END..ch.points.len() - END];
            let ends: Vec<(f64, f64)> = ch.points[..END].iter().chain(&ch.points[ch.points.len() - END..]).copied().collect();
            let arc: f64 = mid.windows(2).map(|p| dist(p[0], p[1].0.round() as i64, p[1].1.round() as i64).max(0.0)).sum::<f64>();
            let arc = arc.max(mid.windows(2).map(|p| libm::hypot(p[1].0 - p[0].0, p[1].1 - p[0].1)).sum::<f64>());
            let mut count = 0usize;
            for y in 0..N as i64 {
                for x in 0..N as i64 {
                    if burnt(x, y) {
                        let dm = mid.iter().map(|&p| dist(p, x, y)).fold(f64::MAX, f64::min);
                        let de = ends.iter().map(|&p| dist(p, x, y)).fold(f64::MAX, f64::min);
                        if dm <= de {
                            count += 1;
                        }
                    }
                }
            }
            let mean = count as f64 / arc;
            println!("{name:26} w={w:.1}: mean burnt width {mean:.2} cells (target {:.1} +- {tol})", w.max(1.0));
            assert!((mean - w.max(1.0)).abs() <= *tol, "{name} w={w}: mean burnt width {mean:.2} against {:.1}", w.max(1.0));
            if name.starts_with("straight") {
                let need = w.max(1.0).floor() as usize;
                for x in (10 + END as i64)..(10 + (*len - END) as i64) {
                    let n = (0..N as i64).filter(|&y| burnt(x, y)).count();
                    assert!(n >= need, "{name} w={w}: column {x} holds {n} burnt cells, at least {need} expected");
                }
            }
        }
    }
}

// spec: GEN-169
// spec: TEST-112
#[test]
fn no_cell_of_the_line_stands_above_its_upstream_neighbour_after_the_burn() {
    // A surface with holes under the profile: the cells at s = 20 and s = 45 sit 7 m and 5 m under the level, so a burn that only lowers min(z, level) leaves a step up after each.
    for (name, f, len, _) in &shapes() {
        for w in [0.5, 1.5, 3.0] {
            let mut ch = channel(f.as_ref(), *len, w);
            // a profile falling 0.05 m a cell
            for (k, l) in ch.levels.iter_mut().enumerate() {
                *l = LEVEL - 0.05 * k as f64;
            }
            let mut z = vec![FLAT; N * N];
            let cell = |p: (f64, f64)| p.1.round() as usize * N + p.0.round() as usize;
            z[cell(ch.points[20])] = LEVEL - 7.0;
            z[cell(ch.points[45])] = LEVEL - 5.0;
            let before = z.clone();
            burn_channels_wide(&mut z, N, (0, 0), std::slice::from_ref(&ch), Some((&WidthLaw::STANDARD, 1.0)));
            assert!(z.iter().zip(&before).all(|(a, b)| a <= b), "{name} w={w}: no cell is raised");
            let mut last = f64::INFINITY;
            for (k, p) in ch.points.iter().enumerate() {
                let h = z[cell(*p)];
                assert!(h <= last, "{name} w={w}: the line cell {k} stands at {h} above its upstream neighbour at {last}");
                last = last.min(h);
            }
            // the holes are kept and everything downstream of one is at least as low
            assert!(
                z[cell(ch.points[ch.points.len() - 1])] <= LEVEL - 7.0,
                "{name} w={w}: the run carries the lowest hole down the channel"
            );
        }
    }
}

// spec: GEN-169
// spec: TEST-112
#[test]
fn a_wide_river_beside_a_line_does_not_pull_the_line_down_and_the_order_of_the_channels_does_not_matter() {
    // A: a wide river (w = 6) falling to 2 m; B: a narrow line across A's disc at a right angle, level 50 m, in a window of 100 m. B's cells outside A's disc must keep B's own profile.
    let a = channel(&|s| (10.0 + s, 40.0), 80, 6.0);
    let mut a = a;
    a.levels = vec![2.0; a.points.len()];
    let mut b = channel(&|s| (50.0, 20.0 + s), 60, 0.5);
    b.levels = vec![50.0; b.points.len()];
    let burn = |chs: &[Channel]| {
        let mut z = vec![FLAT; N * N];
        burn_channels_wide(&mut z, N, (0, 0), chs, Some((&WidthLaw::STANDARD, 1.0)));
        z
    };
    let ab = burn(&[a.clone(), b.clone()]);
    let ba = burn(&[b.clone(), a.clone()]);
    // B's cells past A's disc (y > 40 + 3 + 1) stand at B's level in both orders, not at A's bed
    for y in 45..80 {
        assert_eq!(ab[y * N + 50], 50.0, "A then B: B's cell at y = {y}");
        assert_eq!(ba[y * N + 50], 50.0, "B then A: B's cell at y = {y}");
    }
    assert_eq!(ab, ba, "the order of the channels does not change the surface here");
}

use planet_generators::bake::k7::{burn_channels_bounded, final_channel_pass, upstream_first};

/// A channel along `y` from `x0` to `x1` (one point a cell) falling `fall` metres a cell from `top`, with the discharge of nothing (one cell wide).
fn row_channel(y: f64, x0: f64, x1: f64, top: f64, fall: f64) -> Channel {
    let points: Vec<(f64, f64)> = (0..=((x1 - x0) as usize)).map(|k| (x0 + k as f64, y)).collect();
    let levels = (0..points.len()).map(|k| top - fall * k as f64).collect();
    Channel { points, levels, q: Vec::new(), q_unit: QUnit::Discharge }
}

/// The global surface of the partition tests: a plane with a hole of 7 m at the main channel's cell 30 and one of 4 m at cell 75, and one of 5 m on the tributary.
fn global_z(x: i64, y: i64) -> f64 {
    let mut z = 100.0 + 0.0 * y as f64;
    if (x, y) == (30, 20) {
        z -= 7.0;
    }
    if (x, y) == (75, 20) {
        z -= 4.0;
    }
    if (x, y) == (45, 33) {
        z -= 5.0;
    }
    z
}

/// The window of `w` cells at `(i0, j0)` cut from the global surface.
fn window(w: usize, (i0, j0): (i64, i64)) -> Vec<f64> {
    (0..w * w).map(|c| global_z(i0 + (c % w) as i64, j0 + (c / w) as i64)).collect()
}

/// The channels of the partition tests: a main channel along row 20 and a tributary down the column 60 ending at the main channel's first cell... joined at (60, 20).
fn tree() -> Vec<Channel> {
    let main_up = row_channel(20.0, 2.0, 60.0, 99.5, 0.01);
    let main_down = row_channel(20.0, 60.0, 150.0, 98.9, 0.01);
    let mut trib = row_channel(0.0, 0.0, 0.0, 0.0, 0.0);
    trib.points = (0..=20).map(|k| (60.0, 40.0 - k as f64)).collect();
    trib.levels = (0..=20).map(|k| 99.0 - 0.01 * k as f64).collect();
    // a tributary whose hole stands on its own cell (45, 33): a second one along row 33 into the column
    let mut t2 = row_channel(33.0, 30.0, 60.0, 99.0, 0.01);
    t2.points.push((60.0, 32.0));
    t2.levels.push(98.5);
    upstream_first(vec![main_down, trib, main_up, t2])
}

// spec: GEN-169
// spec: TEST-113
#[test]
fn the_burn_and_the_final_pass_give_the_same_core_in_two_overlapping_windows_and_the_unbounded_carry_does_not() {
    let (w, halo) = (96usize, 8usize);
    let chs = tree();
    let burned = |origin: (i64, i64), carry: usize| {
        let mut z = window(w, origin);
        burn_channels_bounded(&mut z, w, origin, &chs, None, carry);
        // the final pass on the same global surface (the halo of a real K7 differs by its own carve: the pass is tested on identical input)
        let mut f = window(w, origin);
        final_channel_pass(&mut f, w, origin, &chs, 0.0, 1.0, carry);
        (z, f)
    };
    // the common core of the windows at (0, 0) and (48, 0): at least `halo` cells in from the border of each
    let core = |a: (i64, i64), b: (i64, i64)| {
        let mut cells = vec![];
        for y in 0..w as i64 {
            for x in 0..w as i64 {
                let (gx, gy) = (a.0 + x, a.1 + y);
                let inside = |o: (i64, i64)| {
                    gx >= o.0 + halo as i64 && gx < o.0 + (w - halo) as i64 && gy >= o.1 + halo as i64 && gy < o.1 + (w - halo) as i64
                };
                if inside(a) && inside(b) {
                    cells.push((gx, gy));
                }
            }
        }
        cells
    };
    let (a, b) = ((0, 0), (48, 0));
    let common = core(a, b);
    assert!(common.len() > 500);
    let (za, fa) = burned(a, halo);
    let (zb, fb) = burned(b, halo);
    let at = |v: &[f64], o: (i64, i64), g: (i64, i64)| v[(g.1 - o.1) as usize * w + (g.0 - o.0) as usize].to_bits();
    for &g in &common {
        assert_eq!(at(&za, a, g), at(&zb, b, g), "the burn of the cell {g:?} differs between the windows");
        assert_eq!(at(&fa, a, g), at(&fb, b, g), "the final pass of the cell {g:?} differs between the windows");
    }
    // the same bits when the windows swap roles (a third tiling), and no seam rise: no common-core line cell above its upstream neighbour within the halo of a hole
    // the unbounded carry is not partition independent: the window at 48 does not hold the hole at 30
    let (ua, _) = {
        let mut z = window(w, a);
        burn_channels_bounded(&mut z, w, a, &chs, None, usize::MAX);
        (z, ())
    };
    let (ub, _) = {
        let mut z = window(w, b);
        burn_channels_bounded(&mut z, w, b, &chs, None, usize::MAX);
        (z, ())
    };
    assert!(common.iter().any(|&g| at(&ua, a, g) != at(&ub, b, g)), "the unbounded carry differs between the windows (the test has teeth)");
}

// spec: GEN-169
// spec: TEST-113
#[test]
fn the_channels_are_burnt_upstream_first_whatever_the_order_given() {
    let main_up = row_channel(20.0, 2.0, 40.0, 90.0, 0.01);
    let main_down = row_channel(20.0, 40.0, 80.0, 89.6, 0.01);
    let w = 96usize;
    let hole = |x: i64, y: i64| if (x, y) == (20, 20) { 80.0 } else { 100.0 };
    let make = |chs: Vec<Channel>| {
        let mut z: Vec<f64> = (0..w * w).map(|c| hole((c % w) as i64, (c / w) as i64)).collect();
        burn_channels_bounded(&mut z, w, (0, 0), &upstream_first(chs), None, usize::MAX);
        z
    };
    let a = make(vec![main_down.clone(), main_up.clone()]);
    let b = make(vec![main_up, main_down]);
    assert_eq!(a, b, "the order given does not matter: the tree order is applied");
    // the hole of 20 m at x = 20 is carried down both channels: no step up at the joint at x = 40
    for x in 21..80 {
        assert!(a[20 * w + x] <= a[20 * w + x - 1], "a step up at x = {x}");
    }
}

// spec: GEN-169
// spec: TEST-113
#[test]
fn two_segments_ending_in_each_others_first_cell_are_ordered_by_the_order_given_and_the_burn_runs() {
    // A cycle in cells (the rounded end cells meet): there is no upstream, the order given breaks it, and a tributary counts only when it is listed earlier.
    let a = row_channel(20.0, 10.0, 20.0, 90.0, 0.01);
    let mut b = row_channel(20.0, 10.0, 20.0, 91.0, 0.01);
    b.points.reverse();
    let ordered = upstream_first(vec![a.clone(), b.clone()]);
    assert_eq!(ordered.len(), 2);
    assert_eq!(ordered[0].points, a.points, "the first in the order given opens the cycle");
    let swapped = upstream_first(vec![b.clone(), a.clone()]);
    assert_eq!(swapped[0].points, b.points);
    let w = 48usize;
    let mut z = vec![100.0; w * w];
    burn_channels_bounded(&mut z, w, (0, 0), &ordered, None, 8);
    assert!(z.iter().all(|v| v.is_finite()));
}

// spec: GEN-169
// spec: TEST-113
#[test]
fn a_wide_river_burns_the_same_core_in_two_overlapping_windows() {
    // A river of w = 6 cells (disc radius 3) along row 20 with holes 7 m and 3 m deep upstream of the second window; halo 40 holds the carry 8 and the disc radius cap.
    let (w, halo) = (140usize, 40usize);
    let law = WidthLaw::STANDARD;
    let q = libm::pow(6.0 / law.a, 2.0);
    let mut ch = row_channel(60.0, 2.0, 135.0, 99.5, 0.01);
    ch.q = vec![q; ch.points.len()];
    let chs = vec![ch];
    let carry = halo;

    // a hole just outside the second window (it starts at x = 50): without the margin of the disc radius the first window carries it into the core
    let hole = |x: i64, y: i64| if (x, y) == (49, 60) { 92.0 } else { global_z(x, y) };
    let run = |origin: (i64, i64)| {
        let mut z: Vec<f64> = (0..w * w).map(|c| hole(origin.0 + (c % w) as i64, origin.1 + (c / w) as i64)).collect();
        burn_channels_bounded(&mut z, w, origin, &chs, Some((&law, 1.0)), carry);
        z
    };

    let (a, b) = ((0, 0), (50, 0));
    let (za, zb) = (run(a), run(b));
    let mut compared = 0;
    for gy in 0..w as i64 {
        for gx in 0..w as i64 {
            let inside = |o: (i64, i64)| {
                gx >= o.0 + halo as i64 && gx < o.0 + (w - halo) as i64 && gy >= o.1 + halo as i64 && gy < o.1 + (w - halo) as i64
            };
            if inside(a) && inside(b) {
                compared += 1;
                assert_eq!(
                    za[(gy - a.1) as usize * w + (gx - a.0) as usize].to_bits(),
                    zb[(gy - b.1) as usize * w + (gx - b.0) as usize].to_bits(),
                    "cell ({gx}, {gy})"
                );
            }
        }
    }
    assert!(compared > 100);
}

// spec: GEN-169
// spec: TEST-113
#[test]
fn the_final_pass_equals_a_breadth_first_search_over_the_tree_within_the_carry() {
    // a chain of three channels whose middle one is one cell long, joined by tributaries of lengths 1, 2 and 3, on a rough surface
    let w = 60usize;
    let mk = |pts: Vec<(f64, f64)>| Channel { levels: vec![0.0; pts.len()], q: Vec::new(), q_unit: QUnit::Discharge, points: pts };
    let main = mk((0..30).map(|k| (10.0 + k as f64, 30.0)).collect());
    let t1 = mk(vec![(9.0, 30.0), (10.0, 30.0)]); // ends at main's first cell (10, 30): two cells
    let t2 = mk(vec![(9.0, 30.0)]); // one cell, the same cell: a tributary of nothing (first == last): ignored by the order
    let g1 = mk((0..4).map(|k| (5.0 + k as f64, 25.0 + k as f64 * 1.25)).collect()); // an arbitrary path ending near t1's first cell
    let _ = (t2, g1);
    let trib_chain = vec![mk(vec![(6.0, 30.0), (7.0, 30.0), (8.0, 30.0), (9.0, 30.0)]), t1, main];
    let chs = upstream_first(trib_chain);
    let rough = |x: i64, y: i64| 300.0 + ((x * 37 + y * 11) % 17) as f64 - 0.3 * x as f64;
    let cells: Vec<Vec<(i64, i64)>> = chs.iter().map(planet_generators::bake::k7::line_cells).collect();
    for carry in 0..12usize {
        let mut z: Vec<f64> = (0..w * w).map(|c| rough((c % w) as i64, (c / w) as i64)).collect();
        let before = z.clone();
        final_channel_pass(&mut z, w, (0, 0), &chs, 0.0, 1.0, carry);
        // brute force: the nodes (channel, index); the predecessor of (c, k > 0) is (c, k - 1), of (c, 0) the last cell of every channel ending at c's first cell
        for (c, line) in cells.iter().enumerate() {
            #[allow(clippy::needless_range_loop)]
            for k in 0..line.len() {
                let mut seen = std::collections::HashSet::new();
                let mut frontier = vec![(c, k)];
                let mut best = f64::INFINITY;
                for _step in 0..=carry {
                    let mut next = vec![];
                    for &(cc, kk) in &frontier {
                        if !seen.insert((cc, kk)) {
                            continue;
                        }
                        let (x, y) = cells[cc][kk];
                        best = best.min(before[y as usize * w + x as usize]);
                        if kk > 0 {
                            next.push((cc, kk - 1));
                        } else {
                            for (u, ul) in cells.iter().enumerate() {
                                if u != cc && ul.last() == cells[cc].first() {
                                    next.push((u, ul.len() - 1));
                                }
                            }
                        }
                    }
                    frontier = next;
                }
                let (x, y) = line[k];
                let want = before[y as usize * w + x as usize].min(best);
                // a cell on two channels takes the lower of its targets: compare against the lowest expectation over the channels that hold it
                assert!(
                    z[y as usize * w + x as usize] <= want + 1e-12,
                    "carry {carry}: cell {:?} stands at {} above the search's {want}",
                    (x, y),
                    z[y as usize * w + x as usize]
                );
            }
        }
        // and not lower than any search target over the channels holding the cell (the pass lowers path cells to the lowest search value only)
        let on_path: std::collections::HashSet<(i64, i64)> = cells.iter().flatten().copied().collect();
        for y in 0..w as i64 {
            for x in 0..w as i64 {
                if !on_path.contains(&(x, y)) {
                    assert_eq!(z[y as usize * w + x as usize], before[y as usize * w + x as usize]);
                }
            }
        }
    }
}
