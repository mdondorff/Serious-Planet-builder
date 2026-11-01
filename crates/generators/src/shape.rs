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
    /// S5: the spans in km (the greatest distance between two cells) of the five longest high-ground regions of at least 50,000 km^2.
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
    let largest_share = if total_land > 0.0 { land_masses.first().map_or(0.0, |a| a * 1e6 / total_land) } else { 0.0 };

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
