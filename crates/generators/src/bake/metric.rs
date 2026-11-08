//! The physical cell metric of a cube-face grid (GEN-138, SPIKE-105 F5): the true area of a cell and the true distance between two cells of the equiangular cube face (`TangentWarp`),
//! instead of the uniform `dx^2` and `dx` (`sqrt 2 dx`). The cell of the face plane `(i, j)` spans the tangent coordinates `x = tan(pi/4 s)` between the edges `s = 2 i / n - 1` and
//! `2 (i + 1) / n - 1`; the direction of a point is `(x, y, 1)` normalised, so area and distance need no rotation. The area of a cell is the exact spherical quad (the solid angle of the
//! gnomonic patch `[x0, x1] x [y0, y1]`: `G(x1, y1) - G(x0, y1) - G(x1, y0) + G(x0, y0)` with `G(x, y) = atan(x y / sqrt(1 + x^2 + y^2))`), so the cells of a face sum to a sixth of the sphere.
//! The tables are per column and row: the grid is a window of the face plane and `offset` is the plane index of its cell 0 (`-halo`, a halo continuing the plane past the edge).

use std::sync::Arc;

/// The metric of a window of one cube face's plane.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    radius_m: f64,
    /// Tangent coordinate of the cell edges: `w + 1` and `h + 1` entries.
    edge_x: Arc<[f64]>,
    edge_y: Arc<[f64]>,
    /// Unit direction of each cell centre in the face frame, per column and row: `(x, 1) / sqrt(1 + x^2 ...)` is rebuilt from the tangent of the centre.
    centre_x: Arc<[f64]>,
    centre_y: Arc<[f64]>,
}

/// The tangent coordinate of the plane position `s` (`-1` to `1` over the face, beyond it the extended plane): the mapping of `TangentWarp`.
fn tangent(s: f64) -> f64 {
    use planet_core::cube::{FaceMapping, TangentWarp};
    TangentWarp.face_to_plane(s)
}

/// The solid-angle antiderivative of the gnomonic patch from the origin to `(x, y)`.
fn patch(x: f64, y: f64) -> f64 {
    libm::atan(x * y / libm::sqrt(1.0 + x * x + y * y))
}

impl Metric {
    /// The metric of a `w x h` window of a face of `cells` cells an edge on a sphere of `radius_m`, whose cell `(0, 0)` is the plane cell `(offset.0, offset.1)`.
    pub fn cube_face(cells: usize, radius_m: f64, offset: (i64, i64), w: usize, h: usize) -> Metric {
        assert!(cells > 0 && radius_m.is_finite() && radius_m > 0.0, "a metric needs cells and a radius");
        let n = cells as f64;
        let edge = |o: i64, len: usize| -> Arc<[f64]> { (0..=len).map(|k| tangent(2.0 * ((o + k as i64) as f64) / n - 1.0)).collect() };
        let centre =
            |o: i64, len: usize| -> Arc<[f64]> { (0..len).map(|k| tangent(2.0 * ((o + k as i64) as f64 + 0.5) / n - 1.0)).collect() };
        Metric {
            radius_m,
            edge_x: edge(offset.0, w),
            edge_y: edge(offset.1, h),
            centre_x: centre(offset.0, w),
            centre_y: centre(offset.1, h),
        }
    }

    /// The area of cell `(i, j)` of the window in square metres. The four-term difference loses digits to cancellation: a relative error of about 1e-11 at 257 cells an edge and about 3e-8 at the 12,288 cells of pass 1 (measured), far under any use of an area.
    pub fn area(&self, i: usize, j: usize) -> f64 {
        let (x0, x1, y0, y1) = (self.edge_x[i], self.edge_x[i + 1], self.edge_y[j], self.edge_y[j + 1]);
        self.radius_m * self.radius_m * (patch(x1, y1) - patch(x0, y1) - patch(x1, y0) + patch(x0, y0))
    }

    /// The great-circle distance between the centres of cells `(i, j)` and `(k, l)` of the window in metres.
    pub fn distance(&self, i: usize, j: usize, k: usize, l: usize) -> f64 {
        let unit = |x: f64, y: f64| {
            let s = 1.0 / libm::sqrt(1.0 + x * x + y * y);
            [x * s, y * s, s]
        };
        let (a, b) = (unit(self.centre_x[i], self.centre_y[j]), unit(self.centre_x[k], self.centre_y[l]));
        let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
        let chord = libm::sqrt(dx * dx + dy * dy + dz * dz);
        self.radius_m * 2.0 * libm::asin((0.5 * chord).min(1.0))
    }
}
