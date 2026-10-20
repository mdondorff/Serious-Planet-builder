//! Regions (WDEF-012, ADR 0069): the table of surface types and the geometry of a region's polygon. A region is a closed polygon of unit
//! directions; it is tested on the gnomonic projection onto the tangent plane at its first point (the polygon fits one reach, 40 km, so
//! the projection is monotone and has no wrap). f64 throughout, no transcendental (CON-14).

use planet_core::Vec3;

/// One surface type of the table: its name and the debug tint of its overlay (sRGB).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceType {
    pub name: &'static str,
    pub tint: [u8; 3],
}

/// The surface types, in the order the owner named them (2026-10-19); a region stores the index.
pub const SURFACE_TYPES: [SurfaceType; 9] = [
    SurfaceType { name: "rock", tint: [128, 128, 128] },
    SurfaceType { name: "sand", tint: [219, 196, 135] },
    SurfaceType { name: "grass", tint: [106, 168, 79] },
    SurfaceType { name: "snow", tint: [240, 244, 250] },
    SurfaceType { name: "forest", tint: [34, 100, 52] },
    SurfaceType { name: "ice", tint: [170, 214, 235] },
    SurfaceType { name: "desert", tint: [214, 165, 92] },
    SurfaceType { name: "wetland", tint: [86, 120, 98] },
    SurfaceType { name: "urban", tint: [150, 140, 150] },
];

/// The surface type with the name `name`, as an index.
pub fn surface_type_index(name: &str) -> Option<usize> {
    SURFACE_TYPES.iter().position(|s| s.name == name)
}

/// The basis `(e1, e2)` of the tangent plane at the unit vector `a`.
fn basis(a: Vec3) -> (Vec3, Vec3) {
    let helper = if a.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    let e1 = helper.cross(a).normalized();
    (e1, a.cross(e1))
}

/// The gnomonic projection of `points` onto the tangent plane at `anchor`; none when a point is not in front of the plane.
fn project(anchor: Vec3, points: &[Vec3]) -> Option<Vec<[f64; 2]>> {
    let (e1, e2) = basis(anchor);
    points
        .iter()
        .map(|q| {
            let d = q.dot(anchor);
            (d > 1e-9).then(|| [q.dot(e1) / d, q.dot(e2) / d])
        })
        .collect()
}

fn dist_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (dx, dy) = (p[0] - (a[0] + t * abx), p[1] - (a[1] + t * aby));
    (dx * dx + dy * dy).sqrt()
}

/// Whether the unit direction `p` lies inside or on the closed polygon `points` (at least 3 unit directions, the first the anchor): the
/// even-odd rule on the gnomonic projection at the first point, the boundary counted inside to 1e-12 of the projection's unit (a few
/// micrometres). A point behind the tangent plane is outside.
pub fn polygon_contains(points: &[Vec3], p: Vec3) -> bool {
    let bits: Vec<[u64; 3]> = points.iter().map(|q| [q.x.to_bits(), q.y.to_bits(), q.z.to_bits()]).collect();
    polygon_contains_bits(&bits, p)
}

/// [`polygon_contains`] for a polygon held as the bits of its unit directions (a stamp's points, at most [`crate::stamp::MAX_POINTS`]):
/// no allocation, the projected polygon lives on the stack (the stamp stage asks it for every sample).
pub fn polygon_contains_bits(points: &[[u64; 3]], p: Vec3) -> bool {
    let n = points.len();
    if !(3..=crate::stamp::MAX_POINTS).contains(&n) {
        return false;
    }
    let vec = |b: &[u64; 3]| Vec3::new(f64::from_bits(b[0]), f64::from_bits(b[1]), f64::from_bits(b[2]));
    let anchor = vec(&points[0]);
    let (e1, e2) = basis(anchor);
    let mut poly = [[0.0f64; 2]; crate::stamp::MAX_POINTS];
    for (slot, b) in poly.iter_mut().zip(points) {
        let q = vec(b);
        let d = q.dot(anchor);
        if d <= 1e-9 {
            return false;
        }
        *slot = [q.dot(e1) / d, q.dot(e2) / d];
    }
    let d = p.dot(anchor);
    if d <= 1e-9 {
        return false;
    }
    let q = [p.dot(e1) / d, p.dot(e2) / d];
    let poly = &poly[..n];
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if dist_to_segment(q, a, b) <= 1e-12 {
            return true;
        }
        if (a[1] > q[1]) != (b[1] > q[1]) {
            let x = a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if q[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn on_segment(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    c[0] >= a[0].min(b[0]) && c[0] <= a[0].max(b[0]) && c[1] >= a[1].min(b[1]) && c[1] <= a[1].max(b[1])
}

fn segments_meet(p1: [f64; 2], p2: [f64; 2], p3: [f64; 2], p4: [f64; 2]) -> bool {
    let (d1, d2, d3, d4) = (orient(p3, p4, p1), orient(p3, p4, p2), orient(p1, p2, p3), orient(p1, p2, p4));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    (d1 == 0.0 && on_segment(p3, p4, p1))
        || (d2 == 0.0 && on_segment(p3, p4, p2))
        || (d3 == 0.0 && on_segment(p1, p2, p3))
        || (d4 == 0.0 && on_segment(p1, p2, p4))
}

/// Whether the closed polygon `points` crosses itself (two edges that are not neighbours meet) or has no area (collinear points, a spike
/// that doubles back; a signed area under 1e-15 of the projection's unit squared). A polygon with a point behind the tangent
/// plane at its first point is reported as crossing (it does not fit one reach).
pub fn polygon_self_intersects(points: &[Vec3]) -> bool {
    let Some(poly) = project(points[0], points) else { return true };
    let n = poly.len();
    // A polygon with no area (collinear points, a spike doubling back) is not a region: its signed area, in the projection's unit, is
    // far below that of one square metre (about 2.5e-14).
    let area: f64 = (0..n).map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1]).sum::<f64>() * 0.5;
    if area.abs() < 1e-15 {
        return true;
    }
    for i in 0..n {
        for j in i + 1..n {
            // Neighbouring edges share a vertex; the last edge neighbours the first.
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if segments_meet(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n]) {
                return true;
            }
        }
    }
    false
}
