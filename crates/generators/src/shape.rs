//! The shape metrics of SPIKE-91 (S1 to S7) on the preview's equirectangular grid (GEN-103): what the class shares and the hypsometry cannot see, the
//! connectedness, width and length of the high ground and the shape of the land. CPU only, pure functions of the grid.

use crate::classes::EARTH_RADIUS_M;
use planet_core::Vec3;

/// The latitude of the centre of row `y` of `height` rows, in radians.
fn latitude(y: usize, height: usize) -> f64 {
    core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI
}

/// The input of the metrics: a grid of `width x height` cells, `land` and `high` (the high ground, 2 km and up) per cell.
pub struct Grid<'a> {
    pub width: usize,
    pub height: usize,
    pub land: &'a [bool],
    pub high: &'a [bool],
}

/// Connected components of `mask` on the grid, the longitude wrapping; `diagonal` joins the 8 neighbours, else the 4. Labels are 1-based, 0 is outside.
pub fn components(mask: &[bool], w: usize, h: usize, diagonal: bool) -> (Vec<u32>, usize) {
    let mut label = vec![0u32; w * h];
    let mut count = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..w * h {
        if !mask[start] || label[start] != 0 {
            continue;
        }
        count += 1;
        label[start] = count as u32;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    if (dx == 0 && dy == 0) || (!diagonal && dx != 0 && dy != 0) {
                        continue;
                    }
                    let ny = y + dy;
                    if ny < 0 || ny >= h as isize {
                        continue;
                    }
                    let j = ny as usize * w + (x + dx).rem_euclid(w as isize) as usize;
                    if mask[j] && label[j] == 0 {
                        label[j] = count as u32;
                        stack.push(j);
                    }
                }
            }
        }
    }
    (label, count)
}

/// The area of a cell of row `y` in km^2.
fn cell_km2(y: usize, w: usize, h: usize) -> f64 {
    let (dlon, dlat) = (2.0 * core::f64::consts::PI / w as f64, core::f64::consts::PI / h as f64);
    let r = EARTH_RADIUS_M / 1000.0;
    r * r * dlon * dlat * libm::cos(latitude(y, h))
}

/// The distance in km from every cell of `inside` to the nearest cell outside it (a chamfer transform with the cell sizes of each row, the longitude wrapping).
fn edge_distance_km(inside: &[bool], w: usize, h: usize) -> Vec<f64> {
    let r = EARTH_RADIUS_M / 1000.0;
    let mut d: Vec<f64> = inside.iter().map(|&i| if i { f64::INFINITY } else { 0.0 }).collect();
    let step = |y: usize| {
        let dx = r * libm::cos(latitude(y, h)).max(0.02) * 2.0 * core::f64::consts::PI / w as f64;
        let dy = r * core::f64::consts::PI / h as f64;
        (dx, dy, libm::sqrt(dx * dx + dy * dy))
    };
    for _ in 0..2 {
        for y in 0..h {
            let (dx, dy, dg) = step(y);
            for x in 0..w {
                let i = y * w + x;
                let mut best = d[i].min(d[y * w + (x + w - 1) % w] + dx);
                if y > 0 {
                    best =
                        best.min(d[(y - 1) * w + x] + dy).min(d[(y - 1) * w + (x + w - 1) % w] + dg).min(d[(y - 1) * w + (x + 1) % w] + dg);
                }
                d[i] = best;
            }
        }
        for y in (0..h).rev() {
            let (dx, dy, dg) = step(y);
            for x in (0..w).rev() {
                let i = y * w + x;
                let mut best = d[i].min(d[y * w + (x + 1) % w] + dx);
                if y + 1 < h {
                    best =
                        best.min(d[(y + 1) * w + x] + dy).min(d[(y + 1) * w + (x + w - 1) % w] + dg).min(d[(y + 1) * w + (x + 1) % w] + dg);
                }
                d[i] = best;
            }
        }
    }
    d
}

pub struct ShapeMetrics {
    /// S1: the land masses of at least 1 Mkm^2, largest first (Mkm^2).
    pub land_masses: Vec<f64>,
    /// S2: the largest land mass's share of land.
    pub largest_share: f64,
    /// S3: the coastline's box-counting dimension over boxes of 2 to 32 cells.
    pub coast_dimension: f64,
    /// S4: the area-weighted median and the 10th percentile of the high ground's width in km (twice the distance to the edge at the ridge cells of the distance transform).
    pub width_median_km: f64,
    pub width_p10_km: f64,
    /// S5: the spans in km (the greatest distance between two of up to 600 sampled cells, so a lower bound) of the five longest high-ground regions of at least 50,000 km^2.
    pub longest_spans_km: Vec<f64>,
    /// S6: land the high-ground regions surround but do not cover, in Mkm^2 (holes of at least 0.5 Mkm^2 of land).
    pub enclosed_mkm2: f64,
}

pub fn shape_metrics(g: &Grid) -> ShapeMetrics {
    let (w, h) = (g.width, g.height);
    let area = |i: usize| cell_km2(i / w, w, h);

    // S1, S2
    let (lab, n) = components(g.land, w, h, true);
    let mut areas = vec![0.0; n + 1];
    for i in 0..w * h {
        if lab[i] != 0 {
            areas[lab[i] as usize] += area(i);
        }
    }
    let total_land: f64 = areas[1..].iter().sum();
    let mut land_masses: Vec<f64> = areas[1..].iter().map(|a| a / 1e6).filter(|&a| a >= 1.0).collect();
    land_masses.sort_by(|a, b| b.partial_cmp(a).expect("finite areas"));
    let largest_share = if total_land > 0.0 { areas[1..].iter().cloned().fold(0.0, f64::max) / total_land } else { 0.0 };

    // S3: boxes of 2, 4, 8, 16 and 32 cells over the coast cells (land with a sea neighbour).
    let coast: Vec<bool> = (0..w * h)
        .map(|i| {
            if !g.land[i] {
                return false;
            }
            let (x, y) = (i % w, i / w);
            let sea = |j: usize| !g.land[j];
            sea(y * w + (x + 1) % w)
                || sea(y * w + (x + w - 1) % w)
                || (y > 0 && sea((y - 1) * w + x))
                || (y + 1 < h && sea((y + 1) * w + x))
        })
        .collect();
    let mut points: Vec<(f64, f64)> = Vec::new();
    for size in [2usize, 4, 8, 16, 32] {
        let mut boxes = 0usize;
        for by in (0..h).step_by(size) {
            for bx in (0..w).step_by(size) {
                if (by..(by + size).min(h)).any(|y| (bx..(bx + size).min(w)).any(|x| coast[y * w + x])) {
                    boxes += 1;
                }
            }
        }
        points.push((libm::log(size as f64), libm::log(boxes.max(1) as f64)));
    }
    let (mx, my) = (points.iter().map(|p| p.0).sum::<f64>() / 5.0, points.iter().map(|p| p.1).sum::<f64>() / 5.0);
    let slope = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / points.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum::<f64>();

    // S4: the width at the ridge cells of the distance transform.
    let dist = edge_distance_km(g.high, w, h);
    let mut widths: Vec<(f64, f64)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !g.high[i] {
                continue;
            }
            let mut ridge = true;
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let ny = y as isize + dy;
                    if ny < 0 || ny >= h as isize {
                        continue;
                    }
                    if dist[ny as usize * w + (x as isize + dx).rem_euclid(w as isize) as usize] > dist[i] {
                        ridge = false;
                    }
                }
            }
            if ridge {
                widths.push((2.0 * dist[i], area(i)));
            }
        }
    }
    widths.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite widths"));
    let total_w: f64 = widths.iter().map(|p| p.1).sum();
    let quantile = |q: f64| {
        let mut acc = 0.0;
        for &(v, a) in &widths {
            acc += a;
            if acc >= q * total_w {
                return v;
            }
        }
        0.0
    };

    // S5: the spans of the high-ground regions.
    let (hl, hn) = components(g.high, w, h, true);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); hn + 1];
    for i in 0..w * h {
        if hl[i] != 0 {
            members[hl[i] as usize].push(i);
        }
    }
    let r_km = EARTH_RADIUS_M / 1000.0;
    let mut spans: Vec<f64> = members[1..]
        .iter()
        .filter(|m| m.iter().map(|&i| area(i)).sum::<f64>() >= 50_000.0)
        .map(|m| {
            let stride = (m.len() / 600).max(1);
            let dirs: Vec<Vec3> = m
                .iter()
                .step_by(stride)
                .map(|&i| {
                    let lon = -core::f64::consts::PI + ((i % w) as f64 + 0.5) / w as f64 * 2.0 * core::f64::consts::PI;
                    Vec3::from_lat_lon(latitude(i / w, h), lon)
                })
                .collect();
            let mut best = 0.0f64;
            for a in &dirs {
                for b in &dirs {
                    best = best.max(a.angle_to(*b));
                }
            }
            best * r_km
        })
        .collect();
    spans.sort_by(|a, b| b.partial_cmp(a).expect("finite spans"));
    spans.truncate(5);

    // S6: components of the complement of the high ground that touch neither pole row, holding at least 0.5 Mkm^2 of land.
    let not_high: Vec<bool> = g.high.iter().map(|&v| !v).collect();
    let (cl, cn) = components(&not_high, w, h, false);
    let mut touches = vec![false; cn + 1];
    for x in 0..w {
        touches[cl[x] as usize] = true;
        touches[cl[(h - 1) * w + x] as usize] = true;
    }
    let mut land_in = vec![0.0; cn + 1];
    for i in 0..w * h {
        if cl[i] != 0 && g.land[i] {
            land_in[cl[i] as usize] += area(i);
        }
    }
    let enclosed_mkm2 = 0.0f64.max((1..=cn).filter(|&c| !touches[c] && land_in[c] >= 500_000.0).map(|c| land_in[c]).sum::<f64>() / 1e6);

    ShapeMetrics {
        land_masses,
        largest_share,
        coast_dimension: -slope,
        width_median_km: quantile(0.5),
        width_p10_km: quantile(0.1),
        longest_spans_km: spans,
        enclosed_mkm2,
    }
}

/// The continent-stage metrics of SPIKE-92 (S9 to S11 and the polar rule, TEST-088) of a land mask.
pub struct ContinentMetrics {
    /// S9: the area-weighted median over the masses of at least 1 Mkm^2 of the compactness 4 pi A / P^2, P the contour length of the mass's land fraction on a 1 degree grid.
    pub compactness: f64,
    /// S10: the median over those masses of sqrt(l1 / l2), the eigenvalues of the area-weighted second moment in the tangent plane at the mass's mean direction.
    pub elongation: f64,
    /// S11: the number of land masses of 10^4 to 10^6 km^2, and their share of all land.
    pub islands: usize,
    pub island_share: f64,
    /// The masses of at least 1 Mkm^2 whose area-weighted mean direction is poleward of 70 degrees.
    pub polar_masses: usize,
}

/// The contour length in km of `f` (rows x cols, row 0 north, one degree cells, one row of padding above and below) at level 0.5, by marching squares: the sum over the cells
/// of the length of the segments between the linearly interpolated edge crossings, east-west steps scaled by the cosine of the segment's latitude.
fn contour_km(f: &[f64], rows: usize, cols: usize) -> f64 {
    let d = EARTH_RADIUS_M / 1000.0 * core::f64::consts::PI / 180.0;
    let at = |i: usize, j: usize| f[i * cols + j];
    let mut total = 0.0;
    for i in 0..rows - 1 {
        for j in 0..cols - 1 {
            let (a, b, c, e) = (at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j));
            let case = usize::from(a >= 0.5) | usize::from(b >= 0.5) << 1 | usize::from(c >= 0.5) << 2 | usize::from(e >= 0.5) << 3;
            // Crossing points (row, column) on the edges 0 top, 1 right, 2 bottom, 3 left.
            let point = |edge: usize| -> (f64, f64) {
                let t = |p: f64, q: f64| (0.5 - p) / (q - p);
                match edge {
                    0 => (i as f64, j as f64 + t(a, b)),
                    1 => (i as f64 + t(b, c), j as f64 + 1.0),
                    2 => (i as f64 + 1.0, j as f64 + t(e, c)),
                    _ => (i as f64 + t(a, e), j as f64),
                }
            };
            let centre_in = 0.25 * (a + b + c + e) >= 0.5;
            let segments: &[(usize, usize)] = match case {
                1 | 14 => &[(3, 0)],
                2 | 13 => &[(0, 1)],
                3 | 12 => &[(3, 1)],
                4 | 11 => &[(1, 2)],
                5 if centre_in => &[(0, 1), (2, 3)],
                5 => &[(3, 0), (1, 2)],
                6 | 9 => &[(0, 2)],
                7 | 8 => &[(2, 3)],
                10 if centre_in => &[(3, 0), (1, 2)],
                10 => &[(0, 1), (2, 3)],
                _ => &[],
            };
            for &(p, q) in segments {
                let ((y1, x1), (y2, x2)) = (point(p), point(q));
                // Padded row y is row y - 1 of the grid; the latitude of the segment's middle.
                let lat = (90.0 - ((0.5 * (y1 + y2) - 1.0) + 0.5)).to_radians();
                total += d * libm::hypot(y2 - y1, (x2 - x1) * libm::cos(lat));
            }
        }
    }
    total
}

/// S9 to S11 and the polar rule of `land` on a `width x height` equirectangular grid (the longitude wrapping). The perimeter of each mass is taken on a 360 x 180 grid of its land fractions
/// (a pixel counts for the one degree cell it lies in), cut at the meridian where the mass is thinnest.
pub fn continent_metrics(land: &[bool], width: usize, height: usize) -> ContinentMetrics {
    let (w, h) = (width, height);
    let (label, n) = components(land, w, h, true);
    let mut areas = vec![0.0f64; n + 1];
    for i in 0..w * h {
        areas[label[i] as usize] += cell_km2(i / w, w, h);
    }
    let total_land: f64 = areas[1..].iter().sum();
    let bin = |i: usize| ((i / w) * 180 / h) * 360 + (i % w) * 360 / w;
    let mut per_bin = vec![0.0f64; 360 * 180];
    for i in 0..w * h {
        per_bin[bin(i)] += 1.0;
    }
    let (mut s9, mut s10): (Vec<(f64, f64)>, Vec<f64>) = (Vec::new(), Vec::new());
    let (mut islands, mut island_area, mut polar) = (0usize, 0.0, 0usize);
    for (m, &a) in areas.iter().enumerate().skip(1) {
        if (1e4..1e6).contains(&a) {
            islands += 1;
            island_area += a;
        }
        if a < 1e6 {
            continue;
        }
        let dirs: Vec<(usize, Vec3, f64)> = (0..w * h)
            .filter(|&i| label[i] as usize == m)
            .map(|i| {
                let lon = -core::f64::consts::PI + ((i % w) as f64 + 0.5) / w as f64 * 2.0 * core::f64::consts::PI;
                (i, Vec3::from_lat_lon(latitude(i / w, h), lon), cell_km2(i / w, w, h))
            })
            .collect();
        // S9: the land fraction of the mass on the one degree grid, cut at the emptiest meridian, padded with sea.
        let mut frac = vec![0.0f64; 360 * 180];
        for (i, _, _) in &dirs {
            frac[bin(*i)] += 1.0;
        }
        for (f, t) in frac.iter_mut().zip(&per_bin) {
            *f = if *t > 0.0 { *f / *t } else { 0.0 };
        }
        let column = |c: usize| (0..180).map(|r| frac[r * 360 + c]).sum::<f64>();
        let cut = (0..360).fold(0, |best, c| if column(c) < column(best) { c } else { best });
        let (rows, cols) = (182, 362);
        let mut padded = vec![0.0f64; rows * cols];
        for r in 0..180 {
            for c in 0..360 {
                padded[(r + 1) * cols + c + 1] = frac[r * 360 + (c + cut) % 360];
            }
        }
        let p = contour_km(&padded, rows, cols);
        s9.push((4.0 * core::f64::consts::PI * a / (p * p), a));
        // S10 and the polar rule: the second moment in the tangent plane at the area-weighted mean direction.
        let sum = dirs.iter().fold(Vec3::new(0.0, 0.0, 0.0), |s, (_, v, wgt)| s + *v * *wgt);
        let nrm = sum.normalized();
        if libm::asin(nrm.z.clamp(-1.0, 1.0)).to_degrees().abs() >= 70.0 {
            polar += 1;
        }
        let helper = if nrm.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
        let e1 = nrm.cross(helper).normalized();
        let e2 = nrm.cross(e1);
        let wsum: f64 = dirs.iter().map(|d| d.2).sum();
        let (mu, mt) =
            (dirs.iter().map(|d| d.2 * d.1.dot(e1)).sum::<f64>() / wsum, dirs.iter().map(|d| d.2 * d.1.dot(e2)).sum::<f64>() / wsum);
        let (mut cuu, mut ctt, mut cut_) = (0.0, 0.0, 0.0);
        for (_, v, wgt) in &dirs {
            let (u, t) = (v.dot(e1) - mu, v.dot(e2) - mt);
            cuu += wgt * u * u;
            ctt += wgt * t * t;
            cut_ += wgt * u * t;
        }
        let (cuu, ctt, cut_) = (cuu / wsum, ctt / wsum, cut_ / wsum);
        let mean = 0.5 * (cuu + ctt);
        let disc = libm::sqrt((0.5 * (cuu - ctt)) * (0.5 * (cuu - ctt)) + cut_ * cut_);
        s10.push(libm::sqrt((mean + disc) / (mean - disc).max(1e-300)));
    }
    s9.sort_by(|a, b| a.0.total_cmp(&b.0));
    let half = 0.5 * s9.iter().map(|p| p.1).sum::<f64>();
    let mut acc = 0.0;
    let compactness = s9
        .iter()
        .find(|p| {
            acc += p.1;
            acc >= half
        })
        .map_or(0.0, |p| p.0);
    s10.sort_by(f64::total_cmp);
    let elongation = match s10.len() {
        0 => 0.0,
        k if k % 2 == 1 => s10[k / 2],
        k => 0.5 * (s10[k / 2 - 1] + s10[k / 2]),
    };
    ContinentMetrics {
        compactness,
        elongation,
        islands,
        island_share: if total_land > 0.0 { island_area / total_land } else { 0.0 },
        polar_masses: polar,
    }
}
