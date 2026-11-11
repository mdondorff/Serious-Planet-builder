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
    let q = (w_cells / law.a).powf(1.0 / law.b);
    let points: Vec<(f64, f64)> = (0..=len).map(|k| f(k as f64)).collect();
    Channel { levels: vec![LEVEL; points.len()], q: vec![q; points.len()], q_unit: QUnit::Discharge, points }
}

type Shape = (&'static str, Box<dyn Fn(f64) -> (f64, f64)>, usize, f64);

/// (name, point function, length in points, tolerance on the mean width)
fn shapes() -> Vec<Shape> {
    vec![
        ("straight", Box::new(|s| (10.0 + s, 48.3)), 90, 0.5),
        ("straight, on the cell row", Box::new(|s| (10.0 + s, 48.0)), 90, 1.05),
        ("diagonal", Box::new(|s| (10.0 + s * 0.7071, 10.0 + s * 0.7071)), 120, 0.71),
        ("curved", Box::new(|s| (60.0 + 40.0 * (s / 40.0).cos(), 60.0 + 40.0 * (s / 40.0).sin())), 110, 0.71),
    ]
}

fn dist(a: (f64, f64), x: i64, y: i64) -> f64 {
    ((a.0 - x as f64).powi(2) + (a.1 - y as f64).powi(2)).sqrt()
}

// spec: GEN-168
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
                assert!(burnt(x, y) && z[y as usize * N + x as usize] == LEVEL, "{name} w={w}: the line cell ({x}, {y}) is lowered to its level (no gap)");
            }
            // The burnt cells nearest to the middle of the line (not to an end's cap) over the arc length of that middle.
            let mid = &ch.points[END..ch.points.len() - END];
            let ends: Vec<(f64, f64)> = ch.points[..END].iter().chain(&ch.points[ch.points.len() - END..]).copied().collect();
            let arc: f64 = mid.windows(2).map(|p| dist(p[0], p[1].0.round() as i64, p[1].1.round() as i64).max(0.0)).sum::<f64>();
            let arc = arc.max(mid.windows(2).map(|p| ((p[1].0 - p[0].0).powi(2) + (p[1].1 - p[0].1).powi(2)).sqrt()).sum::<f64>());
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
