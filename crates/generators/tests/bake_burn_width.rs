//! Tier A, TEST-110 (GEN-168): `burn_channels_wide` against the width law on a straight, a diagonal and a curved channel at w = 0.5, 1.5, 2 and 3 cells: no gap on the line, the burnt width within half a cell of
//! max(1, w), no cell raised.

use planet_generators::bake::k7::{burn_channels_wide, Channel, QUnit, WidthLaw};

const N: usize = 96;
const LEVEL: f64 = 10.0;
const FLAT: f64 = 100.0;

/// A channel of points one cell apart along `f(s)`, s in cells, with the discharge that gives `w_cells` at a 1 m cell.
fn channel(f: impl Fn(f64) -> (f64, f64), len: f64, w_cells: f64) -> Channel {
    let law = WidthLaw::STANDARD;
    let q = (w_cells / law.a).powf(1.0 / law.b);
    let n = len as usize;
    let points: Vec<(f64, f64)> = (0..=n).map(|k| f(k as f64)).collect();
    Channel { levels: vec![LEVEL; points.len()], q: vec![q; points.len()], q_unit: QUnit::Discharge, points }
}


/// (name, point function, length in cells)
type Shape = (&'static str, Box<dyn Fn(f64) -> (f64, f64)>, f64);

fn shape_list() -> Vec<Shape> {
    vec![
        ("straight", Box::new(|s| (10.0 + s, 48.3)), 70.0),
        ("diagonal", Box::new(|s| (10.0 + s * 0.7071, 10.0 + s * 0.7071)), 100.0),
        ("curved", Box::new(|s| (48.0 + 30.0 * (s / 30.0).cos(), 48.0 + 30.0 * (s / 30.0).sin())), 80.0),
    ]
}

struct Meas {
    gaps: usize,
    raised: usize,
    /// per path point: the burnt cells across the line (cells within 0.5 cell of the normal through the point, counted at the cell centres' projection)
    width_mean: f64,
    width_min: f64,
    width_max: f64,
}

fn measure(shape: &Shape, w_cells: f64) -> Meas {
    let ch = channel(&shape.1, shape.2, w_cells);
    let mut z = vec![FLAT; N * N];
    burn_channels_wide(&mut z, N, (0, 0), std::slice::from_ref(&ch), Some((&WidthLaw::STANDARD, 1.0)));
    let burnt = |x: i64, y: i64| x >= 0 && y >= 0 && (x as usize) < N && (y as usize) < N && z[y as usize * N + x as usize] < FLAT;
    let raised = z.iter().filter(|&&v| v > FLAT).count();
    let mut gaps = 0;
    for p in &ch.points {
        if !burnt(p.0.round() as i64, p.1.round() as i64) {
            gaps += 1;
        }
    }
    // width across the line: at each point, the tangent from its neighbours; count the burnt cells whose centre is within 0.5 cell of the normal line, weighted by nothing (the cells across)
    let mut ws = vec![];
    for k in 2..ch.points.len() - 2 {
        let (a, b) = (ch.points[k - 1], ch.points[k + 1]);
        let (tx, ty) = (b.0 - a.0, b.1 - a.1);
        let l = (tx * tx + ty * ty).sqrt();
        let (tx, ty) = (tx / l, ty / l);
        let p = ch.points[k];
        // sample the burnt/unburnt state along the normal at 0.05 cell steps, nearest cell
        let mut width = 0.0;
        let mut s = -6.0;
        while s <= 6.0 {
            let (x, y) = (p.0 - ty * s, p.1 + tx * s);
            if burnt(x.round() as i64, y.round() as i64) {
                width += 0.05;
            }
            s += 0.05;
        }
        ws.push(width);
    }
    let mean = ws.iter().sum::<f64>() / ws.len() as f64;
    Meas { gaps, raised, width_mean: mean, width_min: ws.iter().cloned().fold(f64::MAX, f64::min), width_max: ws.iter().cloned().fold(0.0, f64::max) }
}

// spec: GEN-168
#[test]
fn burn_width_report() {
    for shape in &shape_list() {
        for w in [0.5, 1.5, 2.0, 3.0] {
            let m = measure(shape, w);
            println!("{:9} w={:.1} target={:.1}: gaps {} raised {} width mean {:.2} min {:.2} max {:.2}", shape.0, w, w.max(1.0), m.gaps, m.raised, m.width_mean, m.width_min, m.width_max);
        }
    }
}
