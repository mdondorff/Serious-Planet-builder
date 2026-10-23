//! Point lists on the sphere (VIEW-027): what the line and region tools edit before any item exists. A line is an open polyline of 2 to 64
//! points; a region is a closed one of 3 to 64 that does not cross itself; both fit the reach of one item (one cell edge, WDEF-007), a
//! longer line is split into parts that share their boundary point (ADR 0069 point 3). Points are latitude and longitude in degrees, the
//! spelling of an item's anchor; the arithmetic is f64 on unit directions with `libm` (CON-11, CON-14); no window, no GPU, no item.

use planet_core::surface::{DEG_TO_RAD, RAD_TO_DEG};
use planet_core::Vec3;
use planet_world_def::cells::CELL_EDGE_M;
use std::fmt;

/// The most points an item holds (ADR 0069 point 2).
pub const MAX_POINTS: usize = 64;
/// The fewest points of a line and of a region.
pub const MIN_LINE_POINTS: usize = 2;
pub const MIN_REGION_POINTS: usize = 3;
/// Metres added to the reach of an item made from a point list, so that it covers its points however the distance is rounded; a line
/// fits one item when its extent, width and this margin are within one cell edge.
pub const REACH_SLACK_M: f64 = 1.0;
/// The share of the reach limit a part of a split line may use, so an edit of a part has room before it must split again.
pub const SPLIT_FRACTION: f64 = 0.8;

/// Two directions closer than this (a few micrometres on the Earth) are one place.
const SAME_PLACE: f64 = 1e-12;

/// A point on the sphere in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoPoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
}

impl GeoPoint {
    pub const fn new(lat_deg: f64, lon_deg: f64) -> GeoPoint {
        GeoPoint { lat_deg, lon_deg }
    }

    /// The point in the spelling an item file takes: longitude in (-180, 180] (so -180 is 180) and 0 at a pole.
    pub fn canonical(self) -> GeoPoint {
        let lat_deg = if self.lat_deg == 0.0 { 0.0 } else { self.lat_deg };
        if lat_deg.abs() == 90.0 {
            return GeoPoint { lat_deg, lon_deg: 0.0 };
        }
        let lon_deg = if self.lon_deg == -180.0 {
            180.0
        } else if self.lon_deg == 0.0 {
            0.0
        } else {
            self.lon_deg
        };
        GeoPoint { lat_deg, lon_deg }
    }

    /// The unit direction of the point.
    pub fn dir(self) -> Vec3 {
        let (lat, lon) = (self.lat_deg * DEG_TO_RAD, self.lon_deg * DEG_TO_RAD);
        let c = libm::cos(lat);
        Vec3::new(c * libm::cos(lon), c * libm::sin(lon), libm::sin(lat))
    }

    /// The point a unit (or any non-zero) direction looks at.
    pub fn of_dir(d: Vec3) -> GeoPoint {
        let rho = (d.x * d.x + d.y * d.y).sqrt();
        GeoPoint { lat_deg: libm::atan2(d.z, rho) * RAD_TO_DEG, lon_deg: libm::atan2(d.y, d.x) * RAD_TO_DEG }
    }
}

/// Why a point list was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum PolylineError {
    /// Fewer or more points than the kind takes.
    Count {
        need_min: usize,
        got: usize,
    },
    TooMany {
        got: usize,
    },
    /// A coordinate that is not finite or outside its range.
    BadCoordinate {
        index: usize,
    },
    /// Two consecutive points at the same place.
    Repeated {
        index: usize,
    },
    /// The width leaves no reach for the line (`width_m` at or above the part budget).
    WidthTooLarge {
        width_m: f64,
        limit_m: f64,
    },
    /// A region reaching farther from its first point than one item reaches.
    RegionTooLarge {
        reach_m: f64,
        limit_m: f64,
    },
    /// A region whose outline crosses itself: the edges at these indices.
    SelfCrossing {
        edge: usize,
        other: usize,
    },
    /// An index outside the list.
    NoSuchPoint {
        index: usize,
        len: usize,
    },
    /// A region whose last point repeats its first: the outline closes by itself.
    ClosedTwice,
    /// A region of no area: all its points on one great circle.
    NoArea,
    /// A region whose two neighbouring edges run back over each other (a spike): the edges.
    FoldedBack {
        edge: usize,
        other: usize,
    },
    /// A width or a radius that is not a finite positive number (a width: not negative).
    BadNumber {
        what: &'static str,
    },
}

impl fmt::Display for PolylineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolylineError::Count { need_min, got } => write!(f, "{got} point(s), at least {need_min} are needed"),
            PolylineError::TooMany { got } => write!(f, "{got} points, at most {MAX_POINTS} fit one item"),
            PolylineError::BadCoordinate { index } => write!(f, "point {index} has a coordinate that is not finite or out of range"),
            PolylineError::Repeated { index } => write!(f, "points {} and {index} are at the same place", index - 1),
            PolylineError::WidthTooLarge { width_m, limit_m } => {
                write!(f, "a width of {width_m} m leaves no room in the reach of {limit_m} m of one item")
            }
            PolylineError::RegionTooLarge { reach_m, limit_m } => {
                write!(
                    f,
                    "the region reaches {reach_m:.0} m from its first point, one item reaches {limit_m:.0} m (about {:.0} km across)",
                    limit_m / 1000.0
                )
            }
            PolylineError::SelfCrossing { edge, other } => write!(f, "the outline crosses itself: edge {edge} meets edge {other}"),
            PolylineError::NoSuchPoint { index, len } => write!(f, "no point {index} in a list of {len}"),
            PolylineError::ClosedTwice => write!(f, "the last point repeats the first: a region closes its outline by itself"),
            PolylineError::NoArea => write!(f, "the outline has no area: its points lie on one line"),
            PolylineError::FoldedBack { edge, other } => write!(f, "edges {edge} and {other} run back over each other"),
            PolylineError::BadNumber { what } => write!(f, "{what} is not a usable number"),
        }
    }
}

/// The arc distance in metres between two unit directions on a sphere of `radius_m`.
pub fn arc_m(a: Vec3, b: Vec3, radius_m: f64) -> f64 {
    radius_m * a.angle_to(b)
}

/// The point at `t` (0 to 1) along the great circle from `a` to `b`.
pub fn slerp(a: Vec3, b: Vec3, t: f64) -> Vec3 {
    let omega = a.angle_to(b);
    if omega < 1e-12 {
        return a;
    }
    let s = libm::sin(omega);
    (a * (libm::sin((1.0 - t) * omega) / s) + b * (libm::sin(t * omega) / s)).normalized()
}

fn check_coordinates(points: &[GeoPoint]) -> Result<(), PolylineError> {
    for (index, p) in points.iter().enumerate() {
        let ok = p.lat_deg.is_finite() && p.lon_deg.is_finite() && p.lat_deg.abs() <= 90.0 && p.lon_deg.abs() <= 180.0;
        if !ok {
            return Err(PolylineError::BadCoordinate { index });
        }
    }
    for index in 1..points.len() {
        // The same place, not the same spelling: (90, 0) and (90, 45) are one point, so are longitude 180 and -180.
        if points[index].dir().distance(points[index - 1].dir()) < SAME_PLACE {
            return Err(PolylineError::Repeated { index });
        }
    }
    Ok(())
}

/// A line as one item holds it: 2 to 64 points, finite and in range, no two consecutive points equal.
pub fn validate_line(points: &[GeoPoint]) -> Result<(), PolylineError> {
    if points.len() < MIN_LINE_POINTS {
        return Err(PolylineError::Count { need_min: MIN_LINE_POINTS, got: points.len() });
    }
    if points.len() > MAX_POINTS {
        return Err(PolylineError::TooMany { got: points.len() });
    }
    check_coordinates(points)
}

/// The farthest distance, metres, of a point from the first one: the extent that an item's reach covers beyond its anchor.
pub fn extent_m(points: &[GeoPoint], radius_m: f64) -> f64 {
    let Some(first) = points.first() else { return 0.0 };
    let a = first.dir();
    points.iter().map(|p| arc_m(a, p.dir(), radius_m)).fold(0.0, f64::max)
}

/// Whether a line of `width_m` fits the reach of one item: its extent plus the width within one cell edge.
pub fn line_fits_one_item(points: &[GeoPoint], width_m: f64, radius_m: f64) -> bool {
    points.len() <= MAX_POINTS && extent_m(points, radius_m) + width_m + REACH_SLACK_M <= CELL_EDGE_M
}

/// Splits a line into the parts of the linked items it becomes (ADR 0069 point 3): a line that fits one item stays whole; a longer one
/// is cut into parts that each reach at most [`SPLIT_FRACTION`] of the limit (width included) from their first point and hold at most 64
/// points, and the last point of a part is the first of the next. Segments longer than half a part are cut at their great-circle midpoints
/// first, so every part spans at least one segment. The input needs no more than the finite, in-range, no-repeat rule (any length).
pub fn split_line(points: &[GeoPoint], width_m: f64, radius_m: f64) -> Result<Vec<Vec<GeoPoint>>, PolylineError> {
    if points.len() < MIN_LINE_POINTS {
        return Err(PolylineError::Count { need_min: MIN_LINE_POINTS, got: points.len() });
    }
    check_coordinates(points)?;
    if !(width_m.is_finite() && width_m >= 0.0) {
        return Err(PolylineError::BadNumber { what: "the width" });
    }
    if !(radius_m.is_finite() && radius_m > 0.0) {
        return Err(PolylineError::BadNumber { what: "the radius" });
    }
    if line_fits_one_item(points, width_m, radius_m) {
        return Ok(vec![points.to_vec()]);
    }
    let budget = SPLIT_FRACTION * CELL_EDGE_M - width_m;
    if budget <= 1.0 {
        return Err(PolylineError::WidthTooLarge { width_m, limit_m: CELL_EDGE_M });
    }
    // Cut long segments so that one segment is never more than half a part.
    let max_segment = budget / 2.0;
    let mut dense: Vec<GeoPoint> = vec![points[0]];
    for w in points.windows(2) {
        let (a, b) = (w[0].dir(), w[1].dir());
        let pieces = (arc_m(a, b, radius_m) / max_segment).ceil().max(1.0) as usize;
        for k in 1..pieces {
            dense.push(GeoPoint::of_dir(slerp(a, b, k as f64 / pieces as f64)));
        }
        dense.push(w[1]);
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while start < dense.len() - 1 {
        let anchor = dense[start].dir();
        let mut end = start + 1;
        while end + 1 < dense.len() && end + 1 - start < MAX_POINTS && arc_m(anchor, dense[end + 1].dir(), radius_m) <= budget {
            end += 1;
        }
        parts.push(dense[start..=end].to_vec());
        start = end;
    }
    Ok(parts)
}

/// Whether the closed outline crosses itself, projecting the points onto the tangent plane at their mean direction (a region fits one
/// item, so the projection is faithful to a fraction of a percent). Returns the two edges that meet; an edge `i` runs from point `i` to
/// point `i + 1`, the last closes the outline.
fn outline_fault(points: &[GeoPoint]) -> Option<PolylineError> {
    let dirs: Vec<Vec3> = points.iter().map(|p| p.dir()).collect();
    let centre = dirs.iter().fold(Vec3::ZERO, |s, d| s + *d);
    if centre.length() < 1e-9 {
        return None;
    }
    let c = centre.normalized();
    let (e1, e2) = c.tangent_basis();
    let flat: Vec<(f64, f64)> = dirs
        .iter()
        .map(|d| {
            let k = d.dot(c);
            (d.dot(e1) / k, d.dot(e2) / k)
        })
        .collect();
    let n = flat.len();
    // No area: the shoelace sum against the squared perimeter.
    let area2: f64 = (0..n).map(|i| flat[i].0 * flat[(i + 1) % n].1 - flat[(i + 1) % n].0 * flat[i].1).sum();
    let length = |a: (f64, f64), b: (f64, f64)| ((b.0 - a.0) * (b.0 - a.0) + (b.1 - a.1) * (b.1 - a.1)).sqrt();
    let perimeter: f64 = (0..n).map(|i| length(flat[i], flat[(i + 1) % n])).sum();
    if area2.abs() <= 1e-9 * perimeter * perimeter {
        return Some(PolylineError::NoArea);
    }
    let orient = |a: (f64, f64), b: (f64, f64), p: (f64, f64)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    // Neighbouring edges meet at their shared point by construction; they fold back when they run along one line in opposite directions.
    for i in 0..n {
        let (a, b, c) = (flat[i], flat[(i + 1) % n], flat[(i + 2) % n]);
        let turn = orient(a, b, c);
        let backwards = (b.0 - a.0) * (c.0 - b.0) + (b.1 - a.1) * (c.1 - b.1) < 0.0;
        if backwards && turn.abs() <= 1e-9 * length(a, b) * length(b, c) {
            return Some(PolylineError::FoldedBack { edge: i, other: (i + 1) % n });
        }
    }
    for i in 0..n {
        let (a, b) = (flat[i], flat[(i + 1) % n]);
        for j in (i + 1)..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (p, q) = (flat[j], flat[(j + 1) % n]);
            let (d1, d2) = (orient(a, b, p), orient(a, b, q));
            let (d3, d4) = (orient(p, q, a), orient(p, q, b));
            if d1 * d2 <= 0.0 && d3 * d4 <= 0.0 {
                return Some(PolylineError::SelfCrossing { edge: i, other: j });
            }
        }
    }
    None
}

/// A region as one item holds it: 3 to 64 points, every point within one cell edge of the first (the item's reach, WDEF-007, which a
/// mask cannot be split across), and an outline that does not cross itself.
pub fn validate_region(points: &[GeoPoint], radius_m: f64) -> Result<(), PolylineError> {
    if points.len() < MIN_REGION_POINTS {
        return Err(PolylineError::Count { need_min: MIN_REGION_POINTS, got: points.len() });
    }
    if points.len() > MAX_POINTS {
        return Err(PolylineError::TooMany { got: points.len() });
    }
    check_coordinates(points)?;
    if points[0].dir().distance(points[points.len() - 1].dir()) < SAME_PLACE {
        return Err(PolylineError::ClosedTwice);
    }
    let reach_m = extent_m(points, radius_m);
    if reach_m > CELL_EDGE_M {
        return Err(PolylineError::RegionTooLarge { reach_m, limit_m: CELL_EDGE_M });
    }
    match outline_fault(points) {
        Some(fault) => Err(fault),
        None => Ok(()),
    }
}

/// Whether the unit direction `dir` is inside the closed outline `points` (even-odd, in the tangent plane at the mean direction). A
/// region fits one item, so the projection is faithful; a direction on the far side of the mean, or a degenerate list, is not inside.
pub fn contains(points: &[GeoPoint], dir: Vec3) -> bool {
    if points.len() < MIN_REGION_POINTS {
        return false;
    }
    let dirs: Vec<Vec3> = points.iter().map(|p| p.dir()).collect();
    let centre = dirs.iter().fold(Vec3::ZERO, |s, d| s + *d);
    if centre.length() < 1e-9 || !(dir.x.is_finite() && dir.y.is_finite() && dir.z.is_finite()) {
        return false;
    }
    let c = centre.normalized();
    let k = dir.dot(c);
    if k <= 1e-9 {
        return false;
    }
    let (e1, e2) = c.tangent_basis();
    let flat = |d: Vec3| {
        let k = d.dot(c);
        (d.dot(e1) / k, d.dot(e2) / k)
    };
    let (px, py) = flat(dir);
    let poly: Vec<(f64, f64)> = dirs.iter().map(|d| flat(*d)).collect();
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let ((xi, yi), (xj, yj)) = (poly[i], poly[j]);
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The points of the path through `points` (closed: back to the first) with every segment cut into `pieces` along its great circle, so
/// that a path drawn on the sphere follows it. The original points are kept.
pub fn sample_path(points: &[GeoPoint], closed: bool, pieces: usize) -> Vec<Vec3> {
    let n = points.len();
    let mut out = Vec::new();
    if n == 0 {
        return out;
    }
    let segments = if closed { n } else { n.saturating_sub(1) };
    for s in 0..segments {
        let (a, b) = (points[s].dir(), points[(s + 1) % n].dir());
        for k in 0..pieces.max(1) {
            out.push(slerp(a, b, k as f64 / pieces.max(1) as f64));
        }
    }
    out.push(points[if closed { 0 } else { n - 1 }].dir());
    out
}

/// The index of the point nearest to `hit` (a unit direction) within `max_arc_m`, if any.
pub fn nearest_point(points: &[GeoPoint], hit: Vec3, radius_m: f64, max_arc_m: f64) -> Option<usize> {
    if !(hit.x.is_finite() && hit.y.is_finite() && hit.z.is_finite()) {
        return None;
    }
    points
        .iter()
        .enumerate()
        .map(|(i, p)| (i, arc_m(hit, p.dir(), radius_m)))
        .filter(|(_, d)| *d <= max_arc_m)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Where a click lands on a list's segments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentHit {
    /// The segment that runs from point `segment` to point `segment + 1` (`closed`: the last runs back to the first).
    pub segment: usize,
    /// The foot of the click on that segment (an end point when the click is beyond it).
    pub foot: GeoPoint,
    pub distance_m: f64,
}

/// The nearest segment to `hit` and the foot on it. `closed` adds the segment from the last point to the first (a region's outline).
pub fn nearest_segment(points: &[GeoPoint], closed: bool, hit: Vec3, radius_m: f64) -> Option<SegmentHit> {
    let n = points.len();
    if n < 2 || !(hit.x.is_finite() && hit.y.is_finite() && hit.z.is_finite()) {
        return None;
    }
    let segments = if closed { n } else { n - 1 };
    let mut best: Option<SegmentHit> = None;
    for s in 0..segments {
        let (a, b) = (points[s].dir(), points[(s + 1) % n].dir());
        let normal = a.cross(b);
        let foot = if normal.length() < 1e-15 {
            a
        } else {
            let nrm = normal.normalized();
            let q = hit - nrm * hit.dot(nrm);
            if q.length() < 1e-15 {
                a
            } else {
                let q = q.normalized();
                // Inside the arc a..b when q is on b's side of a and on a's side of b.
                if a.cross(q).dot(nrm) >= 0.0 && q.cross(b).dot(nrm) >= 0.0 {
                    q
                } else if arc_m(hit, a, radius_m) <= arc_m(hit, b, radius_m) {
                    a
                } else {
                    b
                }
            }
        };
        let distance_m = arc_m(hit, foot, radius_m);
        if best.is_none_or(|h| distance_m < h.distance_m) {
            best = Some(SegmentHit { segment: s, foot: GeoPoint::of_dir(foot), distance_m });
        }
    }
    best
}

/// The list with `at` inserted after point `segment`.
pub fn insert_point(points: &[GeoPoint], segment: usize, at: GeoPoint) -> Result<Vec<GeoPoint>, PolylineError> {
    if segment >= points.len() {
        return Err(PolylineError::NoSuchPoint { index: segment, len: points.len() });
    }
    let mut v = points.to_vec();
    v.insert(segment + 1, at);
    Ok(v)
}

/// The list with point `index` moved to `to`.
pub fn move_point(points: &[GeoPoint], index: usize, to: GeoPoint) -> Result<Vec<GeoPoint>, PolylineError> {
    if index >= points.len() {
        return Err(PolylineError::NoSuchPoint { index, len: points.len() });
    }
    let mut v = points.to_vec();
    v[index] = to;
    Ok(v)
}

/// The list without point `index`, refused when fewer than `min_points` would remain (2 for a line, 3 for a region).
pub fn remove_point(points: &[GeoPoint], index: usize, min_points: usize) -> Result<Vec<GeoPoint>, PolylineError> {
    if index >= points.len() {
        return Err(PolylineError::NoSuchPoint { index, len: points.len() });
    }
    if points.len() - 1 < min_points {
        return Err(PolylineError::Count { need_min: min_points, got: points.len() - 1 });
    }
    let mut v = points.to_vec();
    v.remove(index);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;

    /// A point `east_m` metres east of (0, 0) on the equator.
    fn east(east_m: f64) -> GeoPoint {
        GeoPoint::new(0.0, east_m / R * RAD_TO_DEG)
    }

    fn line_of(east_m: &[f64]) -> Vec<GeoPoint> {
        east_m.iter().map(|m| east(*m)).collect()
    }

    // spec: VIEW-027
    #[test]
    fn a_line_needs_two_to_sixty_four_distinct_points_in_range() {
        assert!(validate_line(&line_of(&[0.0, 10.0])).is_ok());
        assert_eq!(validate_line(&line_of(&[0.0])), Err(PolylineError::Count { need_min: 2, got: 1 }));
        let many: Vec<GeoPoint> = (0..65).map(|k| east(k as f64 * 10.0)).collect();
        assert_eq!(validate_line(&many), Err(PolylineError::TooMany { got: 65 }));
        assert_eq!(validate_line(&many[..64]), Ok(()));
        assert_eq!(validate_line(&line_of(&[0.0, 10.0, 10.0])), Err(PolylineError::Repeated { index: 2 }));
        assert_eq!(validate_line(&[GeoPoint::new(0.0, 0.0), GeoPoint::new(91.0, 0.0)]), Err(PolylineError::BadCoordinate { index: 1 }));
        assert_eq!(validate_line(&[GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, f64::NAN)]), Err(PolylineError::BadCoordinate { index: 1 }));
        let e = validate_line(&line_of(&[0.0, 10.0, 10.0])).unwrap_err().to_string();
        assert!(e.contains("points 1 and 2"), "{e}");
    }

    // spec: VIEW-027
    #[test]
    fn a_line_that_fits_one_item_stays_whole_and_a_longer_one_splits_into_parts_that_share_their_boundary_point() {
        let short = line_of(&[0.0, 10_000.0, 25_000.0]);
        assert_eq!(split_line(&short, 50.0, R).unwrap(), vec![short.clone()], "within the reach: one item");

        // 150 km along the equator in 10 km steps, width 200 m.
        let long: Vec<GeoPoint> = (0..=15).map(|k| east(k as f64 * 10_000.0)).collect();
        let parts = split_line(&long, 200.0, R).unwrap();
        assert!(parts.len() >= 4, "{} parts", parts.len());
        for (k, part) in parts.iter().enumerate() {
            assert!((MIN_LINE_POINTS..=MAX_POINTS).contains(&part.len()), "part {k} has {} points", part.len());
            let reach = extent_m(part, R) + 200.0;
            assert!(reach <= SPLIT_FRACTION * CELL_EDGE_M, "part {k} reaches {reach} m");
            assert_eq!(validate_line(part), Ok(()));
        }
        for w in parts.windows(2) {
            assert_eq!(w[0].last(), w[1].first(), "consecutive parts share their boundary point exactly");
        }
        assert_eq!(parts.first().unwrap().first(), long.first());
        assert_eq!(parts.last().unwrap().last(), long.last());
        // Every original point is a point of some part (the parts only add cut points).
        for p in &long {
            assert!(parts.iter().flatten().any(|q| q == p), "{p:?} lost");
        }
    }

    // spec: VIEW-027
    #[test]
    fn a_single_long_segment_is_cut_and_a_dense_line_is_cut_at_sixty_four_points_and_a_wide_line_is_refused() {
        // One segment of 100 km: cut at great-circle points, none of the parts longer than the budget.
        let one = line_of(&[0.0, 100_000.0]);
        let parts = split_line(&one, 100.0, R).unwrap();
        assert!(parts.len() >= 3, "{}", parts.len());
        let total: f64 = parts.iter().map(|p| p.windows(2).map(|w| arc_m(w[0].dir(), w[1].dir(), R)).sum::<f64>()).sum();
        assert!((total - 100_000.0).abs() < 1.0, "the parts add up to the segment: {total}");

        // 200 points 100 m apart (20 km): fits the reach but not 64 points, so it splits by count.
        let dense: Vec<GeoPoint> = (0..200).map(|k| east(k as f64 * 100.0)).collect();
        assert!(!line_fits_one_item(&dense, 10.0, R));
        let parts = split_line(&dense, 10.0, R).unwrap();
        assert_eq!(parts.len(), 4, "199 segments in parts of at most 63");
        assert!(parts.iter().all(|p| p.len() <= MAX_POINTS));

        // A width that leaves no budget.
        let e = split_line(&line_of(&[0.0, 100_000.0]), 33_000.0, R).unwrap_err();
        assert_eq!(e, PolylineError::WidthTooLarge { width_m: 33_000.0, limit_m: CELL_EDGE_M });
    }

    fn square(side_m: f64) -> Vec<GeoPoint> {
        let d = side_m / R * RAD_TO_DEG;
        vec![GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, d), GeoPoint::new(d, d), GeoPoint::new(d, 0.0)]
    }

    // spec: VIEW-027
    #[test]
    fn a_region_is_refused_beyond_one_reach_or_when_its_outline_crosses_itself() {
        assert_eq!(validate_region(&square(10_000.0), R), Ok(()));
        assert_eq!(validate_region(&square(10_000.0)[..2], R), Err(PolylineError::Count { need_min: 3, got: 2 }));
        // 50 km sides: the far corner is 70 km from the first point.
        let e = validate_region(&square(50_000.0), R).unwrap_err();
        assert!(matches!(e, PolylineError::RegionTooLarge { limit_m, .. } if limit_m == CELL_EDGE_M), "{e:?}");
        assert!(e.to_string().contains("40 km"), "the limit is told: {e}");
        // A bow tie: the corners 2 and 3 swapped.
        let mut bow = square(10_000.0);
        bow.swap(2, 3);
        assert_eq!(validate_region(&bow, R), Err(PolylineError::SelfCrossing { edge: 1, other: 3 }));
        // A concave arrow is fine.
        let d = 10_000.0 / R * RAD_TO_DEG;
        let arrow = vec![
            GeoPoint::new(0.0, 0.0),
            GeoPoint::new(0.0, d),
            GeoPoint::new(d, d),
            GeoPoint::new(d * 0.5, d * 0.4),
            GeoPoint::new(d, 0.0),
        ];
        assert_eq!(validate_region(&arrow, R), Ok(()));
        // Across the antimeridian and next to the pole.
        let wrap: Vec<GeoPoint> =
            vec![GeoPoint::new(0.0, 179.99), GeoPoint::new(0.0, -179.99), GeoPoint::new(0.1, -179.99), GeoPoint::new(0.1, 179.99)];
        assert_eq!(validate_region(&wrap, R), Ok(()));
        let pole: Vec<GeoPoint> =
            vec![GeoPoint::new(89.9, 0.0), GeoPoint::new(89.9, 90.0), GeoPoint::new(89.9, 180.0), GeoPoint::new(89.9, -90.0)];
        assert_eq!(validate_region(&pole, R), Ok(()));
    }

    // spec: VIEW-027
    #[test]
    fn a_degenerate_region_the_same_place_spelled_twice_and_unusable_numbers_are_refused() {
        // Three points on one line, a spike that goes out and back, and a last point equal to the first.
        assert_eq!(validate_region(&line_of(&[0.0, 5_000.0, 10_000.0]), R), Err(PolylineError::NoArea));
        let d = 10_000.0 / R * RAD_TO_DEG;
        let spike = vec![GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, d), GeoPoint::new(0.0, d * 0.5), GeoPoint::new(d, d * 0.5)];
        assert_eq!(validate_region(&spike, R), Err(PolylineError::FoldedBack { edge: 0, other: 1 }));
        let mut closed = square(10_000.0);
        closed.push(closed[0]);
        assert_eq!(validate_region(&closed, R), Err(PolylineError::ClosedTwice));
        // Longitude 180 and -180, and the pole at any longitude, are one place each.
        let e = validate_line(&[GeoPoint::new(0.0, 180.0), GeoPoint::new(0.0, -180.0)]);
        assert_eq!(e, Err(PolylineError::Repeated { index: 1 }));
        assert_eq!(validate_line(&[GeoPoint::new(90.0, 0.0), GeoPoint::new(90.0, 45.0)]), Err(PolylineError::Repeated { index: 1 }));
        // Width and radius.
        let l = line_of(&[0.0, 100_000.0]);
        assert_eq!(split_line(&l, f64::NAN, R), Err(PolylineError::BadNumber { what: "the width" }));
        assert_eq!(split_line(&l, -5.0, R), Err(PolylineError::BadNumber { what: "the width" }));
        assert_eq!(split_line(&l, 5.0, 0.0), Err(PolylineError::BadNumber { what: "the radius" }));
        // A click that is not a direction picks nothing.
        let nan = Vec3::new(f64::NAN, 0.0, 0.0);
        assert_eq!(nearest_point(&l, nan, R, 1e9), None);
        assert!(nearest_segment(&l, false, nan, R).is_none());
    }

    // spec: VIEW-027
    #[test]
    fn a_click_picks_the_nearest_point_within_reach_and_the_nearest_segment_with_its_foot() {
        let line = line_of(&[0.0, 10_000.0, 20_000.0]);
        let click = |east_m: f64, north_m: f64| GeoPoint::new(north_m / R * RAD_TO_DEG, east_m / R * RAD_TO_DEG).dir();
        assert_eq!(nearest_point(&line, click(10_100.0, 50.0), R, 200.0), Some(1));
        assert_eq!(nearest_point(&line, click(5_000.0, 0.0), R, 200.0), None, "between points, out of reach");
        // Click 300 m north of the middle of the second segment: the foot is on the line, 300 m away.
        let hit = nearest_segment(&line, false, click(15_000.0, 300.0), R).unwrap();
        assert_eq!(hit.segment, 1);
        assert!((hit.distance_m - 300.0).abs() < 1.0, "{}", hit.distance_m);
        assert!(hit.foot.lat_deg.abs() < 1e-6 && (hit.foot.lon_deg * DEG_TO_RAD * R - 15_000.0).abs() < 1.0, "{:?}", hit.foot);
        // Beyond the end of an open line the foot is the end point; on a closed outline the closing segment is there.
        let beyond = nearest_segment(&line, false, click(25_000.0, 0.0), R).unwrap();
        assert_eq!((beyond.segment, beyond.foot), (1, line[2]));
        assert!((beyond.distance_m - 5_000.0).abs() < 1.0);
        let sq = square(10_000.0);
        let d = 10_000.0 / R * RAD_TO_DEG;
        let closing = nearest_segment(&sq, true, GeoPoint::new(d * 0.5, -d * 0.1).dir(), R).unwrap();
        assert_eq!(closing.segment, 3, "the segment from the last point back to the first");
        assert!(nearest_segment(&line[..1], false, click(0.0, 0.0), R).is_none());
    }

    // spec: VIEW-027
    #[test]
    fn inserting_moving_and_removing_a_point_returns_a_new_list_and_refuses_what_leaves_too_few() {
        let line = line_of(&[0.0, 10_000.0, 20_000.0]);
        let with = insert_point(&line, 0, east(4_000.0)).unwrap();
        assert_eq!(with, vec![line[0], east(4_000.0), line[1], line[2]]);
        let moved = move_point(&line, 1, east(12_000.0)).unwrap();
        assert_eq!(moved, vec![line[0], east(12_000.0), line[2]]);
        assert_eq!(remove_point(&line, 1, 2).unwrap(), vec![line[0], line[2]]);
        assert_eq!(remove_point(&line[..2], 0, 2), Err(PolylineError::Count { need_min: 2, got: 1 }));
        assert_eq!(move_point(&line, 3, line[0]), Err(PolylineError::NoSuchPoint { index: 3, len: 3 }));
        assert_eq!(insert_point(&line, 3, line[0]), Err(PolylineError::NoSuchPoint { index: 3, len: 3 }));
        // The input is untouched.
        assert_eq!(line.len(), 3);
    }

    // spec: VIEW-028
    #[test]
    fn a_point_takes_the_spelling_of_an_item_file() {
        assert_eq!(GeoPoint::new(10.0, -180.0).canonical(), GeoPoint::new(10.0, 180.0));
        assert_eq!(GeoPoint::new(90.0, 33.0).canonical(), GeoPoint::new(90.0, 0.0));
        assert_eq!(GeoPoint::new(-90.0, -180.0).canonical(), GeoPoint::new(-90.0, 0.0));
        assert_eq!(GeoPoint::new(-0.0, -0.0).canonical().lat_deg.to_bits(), 0.0f64.to_bits());
        assert_eq!(GeoPoint::new(-5.5, 120.25).canonical(), GeoPoint::new(-5.5, 120.25));
    }

    // spec: VIEW-031
    #[test]
    fn a_direction_is_inside_a_region_by_the_even_odd_rule_and_a_path_is_sampled_along_its_great_circles() {
        let d = |m: f64| m / R * 180.0 / std::f64::consts::PI;
        let sq = vec![
            GeoPoint::new(0.0, 0.0),
            GeoPoint::new(0.0, d(10_000.0)),
            GeoPoint::new(d(10_000.0), d(10_000.0)),
            GeoPoint::new(d(10_000.0), 0.0),
        ];
        assert!(contains(&sq, GeoPoint::new(d(5_000.0), d(5_000.0)).dir()));
        assert!(!contains(&sq, GeoPoint::new(d(5_000.0), d(15_000.0)).dir()));
        assert!(!contains(&sq, GeoPoint::new(d(-1.0), d(5_000.0)).dir()), "just south of the south edge");
        assert!(!contains(&sq, GeoPoint::new(0.0, 180.0).dir()), "the far side");
        // A concave arrow: the notch is outside.
        let arrow = vec![
            GeoPoint::new(0.0, 0.0),
            GeoPoint::new(0.0, d(10_000.0)),
            GeoPoint::new(d(10_000.0), d(10_000.0)),
            GeoPoint::new(d(5_000.0), d(4_000.0)),
            GeoPoint::new(d(10_000.0), 0.0),
        ];
        assert!(!contains(&arrow, GeoPoint::new(d(8_000.0), d(2_000.0)).dir()), "in the notch");
        assert!(contains(&arrow, GeoPoint::new(d(1_000.0), d(2_000.0)).dir()));
        assert!(!contains(&sq[..2], sq[0].dir()), "two points enclose nothing");

        let line = vec![GeoPoint::new(0.0, 0.0), GeoPoint::new(0.0, d(10_000.0))];
        let path = sample_path(&line, false, 4);
        assert_eq!(path.len(), 5);
        assert!((arc_m(path[0], path[4], R) - 10_000.0).abs() < 1e-3 && (arc_m(path[0], path[2], R) - 5_000.0).abs() < 1e-3);
        assert_eq!(sample_path(&sq, true, 2).len(), 9, "four segments in two pieces and the first point again");
        assert!(sample_path(&[], false, 4).is_empty());
    }

    // spec: VIEW-027
    #[test]
    fn a_point_round_trips_through_its_direction_and_slerp_stays_on_the_great_circle() {
        for p in [GeoPoint::new(12.5, -170.25), GeoPoint::new(-89.0, 3.0), GeoPoint::new(0.0, 179.999)] {
            let back = GeoPoint::of_dir(p.dir());
            assert!((back.lat_deg - p.lat_deg).abs() < 1e-12 && (back.lon_deg - p.lon_deg).abs() < 1e-12, "{p:?} {back:?}");
        }
        let (a, b) = (GeoPoint::new(10.0, 10.0).dir(), GeoPoint::new(40.0, 70.0).dir());
        let mid = slerp(a, b, 0.5);
        assert!((arc_m(a, mid, R) - arc_m(mid, b, R)).abs() < 1e-3, "the middle is halfway");
        assert!((arc_m(a, mid, R) + arc_m(mid, b, R) - arc_m(a, b, R)).abs() < 1e-3, "and on the arc");
        assert_eq!(slerp(a, a, 0.3), a);
    }
}
