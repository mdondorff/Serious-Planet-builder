//! The solid angle and area of a cube cell (COORD-013, lead #197; G3-5's quadrature metric). The cube's projection is equiangular (`TangentWarp`): the
//! face coordinate `s` in `[-1, 1]` is the angle `alpha = pi/4 s`, the tangent-plane coordinate `x = tan(alpha)`, and the cells of a level with `n` cells
//! per face edge are equal steps of `alpha` (`pi / (2 n)`). On the unit sphere, `d Omega = sec^2(alpha) sec^2(beta) / (1 + tan^2(alpha) + tan^2(beta))^(3/2)
//! d alpha d beta`, a smooth integrand that is even in `alpha` and in `beta` and symmetric under their swap: the face square's eight symmetries keep it.
//!
//! The solid angle of a cell is that integral by a 2 x 2 Gauss-Legendre rule on each of `k x k` equal subcells, `k = ceil(512 / n)`, in the angle
//! coordinates. No difference of nearly equal values is taken, so there is no cancellation at any `n`. It is computed from the cell folded into
//! one octant of its face (`(min(i, n-1-i), min(j, n-1-j))`, sorted), so all the cells that a symmetry of the cube maps onto each other get the same
//! bits. Pure `libm`, no thread or platform input (CON-14). Additive: no caller reads it yet.

/// Subcells an edge are `ceil(SUBCELL_LEVEL / n)`: the rule always runs on steps of at most `pi / (2 SUBCELL_LEVEL)`.
pub const SUBCELL_LEVEL: i64 = 512;

/// The solid angle (steradians) of cell `(i, j)` of a level with `n` cells per face edge (`0 <= i, j < n`, `n >= 1`).
pub fn cell_solid_angle(n: i64, i: i64, j: i64) -> f64 {
    assert!(n >= 1 && (0..n).contains(&i) && (0..n).contains(&j), "cell ({i}, {j}) of a level with {n} cells per face edge");
    // the octant fold: the integrand is even in each angle and symmetric under their swap
    let (a, b) = (i.min(n - 1 - i), j.min(n - 1 - j));
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let k = (SUBCELL_LEVEL + n - 1) / n;
    // node offsets of the 2-point rule inside a subcell, in units of the subcell: 1/2 -+ 1/(2 sqrt 3)
    let h = 0.5 / libm::sqrt(3.0);
    let offsets = [0.5 - h, 0.5 + h];
    let steps = (n * k) as f64;
    // the angle of a point `t` subcells into the face: pi/4 (2 t / (n k) - 1)
    let angle = |t: f64| core::f64::consts::FRAC_PI_4 * (2.0 * t / steps - 1.0);
    let sec2 = |t: f64| {
        let x = libm::tan(angle(t));
        (1.0 + x * x, x * x)
    };
    let mut rows = 0.0;
    for sv in 0..k {
        for ov in offsets {
            let (qv, yy) = sec2((b * k + sv) as f64 + ov);
            let mut row = 0.0;
            for su in 0..k {
                for ou in offsets {
                    let (pu, xx) = sec2((a * k + su) as f64 + ou);
                    let s = 1.0 + xx + yy;
                    row += pu * qv / (s * libm::sqrt(s));
                }
            }
            rows += row;
        }
    }
    // each node weighs (d alpha / 2)^2, d alpha the subcell's angle step pi / (2 n k)
    let half = core::f64::consts::FRAC_PI_4 / steps;
    half * half * rows
}

/// The pairwise sum of `xs` (halves until at most 8 values, then in order): the fixed reduction of COORD-013's sums of cell areas.
pub fn pairwise_sum(xs: &[f64]) -> f64 {
    if xs.len() <= 8 {
        return xs.iter().fold(0.0, |s, &x| s + x);
    }
    let m = xs.len() / 2;
    pairwise_sum(&xs[..m]) + pairwise_sum(&xs[m..])
}
