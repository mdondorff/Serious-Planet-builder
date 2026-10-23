//! Scalar helpers shared by the generators and the look. Only IEEE-exact operations, so results are identical on every platform.

/// The cubic Hermite step from 0 at `e0` to 1 at `e1`: `t * t * (3 - 2 t)` with `t = (x - e0) / (e1 - e0)` clamped to `[0, 1]`. The
/// arithmetic is part of the generated fields (CON-14): do not reorder it.
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
