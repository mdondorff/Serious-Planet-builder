//! Lead #184 (O8 step 1; owner 2026-11-23, option A with amendments A1-A6; spike only): the trunk's sub-cell centreline.
//! A trunk reach's curve is its lowest path on S0 inside its corridor (KG-trunk's `route_low` on S0, so it exists before KH: wave 1 stays acyclic, A4), smoothed, with
//! meanders on rivers wider than `min_width_m`, then rasterised 4-connected: the raster is the reach's vertex cells (no corner-only step by construction), and each raster
//! cell keeps the curve's sub-cell offset as two signed bytes (A6). Coordinates are L cells of the window: cell `c` has its centre at `(c % w, c / w)`.

/// The curve's rules.
#[derive(Clone, Copy, Debug)]
pub struct CurveParams {
    /// The smoothing's Gaussian sigma along the path (L cells).
    pub sigma: f64,
    /// A smoothed point stands at most this much above its unsmoothed point's cell (m); a higher one is pulled back toward it.
    pub lift_m: f64,
    /// Meanders on (row 6).
    pub meander: bool,
    /// Meanders only where the river is wider than this (m).
    pub min_width_m: f64,
    /// A meander's wavelength in channel widths (10-14 by Leopold and Wolman).
    pub wave_widths: f64,
    /// A meander's amplitude over its wavelength: 0.174 gives a sine's sinuosity of about 1.3.
    pub amp_wave: f64,
    /// A meander point stands at most this much above its unsmoothed point's cell (m): it keeps to the valley floor's 10 m cross-section.
    pub room_m: f64,
    /// An L cell's side (m).
    pub cell_m: f64,
}

impl CurveParams {
    pub fn standard(cell_m: f64, meander: bool) -> CurveParams {
        CurveParams { sigma: 2.0, lift_m: 5.0, meander, min_width_m: 10.0, wave_widths: 12.0, amp_wave: 0.174, room_m: 10.0, cell_m }
    }
}

/// A trunk reach's curve.
#[derive(Clone, Debug, Default)]
pub struct Curve {
    /// The curve's points, about half an L cell apart, from the up anchor's centre to the down anchor's.
    pub pts: Vec<(f64, f64)>,
    /// The 4-connected raster: the reach's vertex cells, up first, no cell twice.
    pub raster: Vec<usize>,
    /// Each raster cell's offset of the curve from its centre, in 1/254 of a cell (A6).
    pub offsets: Vec<(i8, i8)>,
    /// The share of the curve's points that meander (amplitude over zero).
    pub meander_share: f64,
    /// Numerics review F1: 0 the curve's raster; 1 the base path's 4-connected raster, when a curve cell was sea, another node's anchor or above its point's room;
    /// 2 the base path itself (8-connected), when that raster failed too.
    pub fallback: u8,
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    libm::hypot(a.0 - b.0, a.1 - b.1)
}

/// The polyline resampled at `ds` along its length, both ends kept.
pub fn resample(p: &[(f64, f64)], ds: f64) -> Vec<(f64, f64)> {
    if p.len() < 2 {
        return p.to_vec();
    }
    let total: f64 = p.windows(2).map(|s| dist(s[0], s[1])).sum();
    let n = ((total / ds).ceil() as usize).max(1);
    let step = total / n as f64;
    let mut out = Vec::with_capacity(n + 1);
    out.push(p[0]);
    let (mut seg, mut into) = (0usize, 0.0f64);
    for k in 1..n {
        let mut want = step;
        loop {
            let l = dist(p[seg], p[seg + 1]);
            if into + want <= l || seg + 2 >= p.len() {
                into += want;
                let t = if l > 0.0 { (into / l).min(1.0) } else { 0.0 };
                out.push((p[seg].0 + t * (p[seg + 1].0 - p[seg].0), p[seg].1 + t * (p[seg + 1].1 - p[seg].1)));
                break;
            }
            want -= l - into;
            into = 0.0;
            seg += 1;
        }
        let _ = k;
    }
    out.push(p[p.len() - 1]);
    out
}

fn cell_of(q: (f64, f64), w: usize, h: usize) -> Option<usize> {
    let (x, y) = (libm::round(q.0), libm::round(q.1));
    (x >= 0.0 && y >= 0.0 && (x as usize) < w && (y as usize) < h).then(|| y as usize * w + x as usize)
}

/// The curve of a reach whose lowest path is `base` (window cells, up first): `water` is S0 with the ocean at the sea level, `width_m(f)` the river's width at the share `f`
/// of the reach's length.
/// `avoid(c)`: a cell no raster cell but the ends may take (other nodes' anchors).
pub fn build(
    base: &[usize],
    w: usize,
    h: usize,
    water: &[f64],
    ocean: &[bool],
    avoid: &dyn Fn(usize) -> bool,
    width_m: &dyn Fn(f64) -> f64,
    p: &CurveParams,
) -> Curve {
    let centre = |c: usize| ((c % w) as f64, (c / w) as f64);
    if base.len() < 2 {
        return Curve {
            pts: base.iter().map(|&c| centre(c)).collect(),
            raster: base.to_vec(),
            offsets: vec![(0, 0); base.len()],
            meander_share: 0.0,
            fallback: 0,
        };
    }
    let raw = resample(&base.iter().map(|&c| centre(c)).collect::<Vec<_>>(), 0.5);
    let n = raw.len();
    // numerics review F7: the samples' actual spacing (half a cell or a little less)
    let ds = raw.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>() / (n - 1) as f64;
    let zref: Vec<f64> = raw.iter().map(|&q| cell_of(q, w, h).map_or(f64::INFINITY, |c| water[c])).collect();
    let ok = |q: (f64, f64), i: usize, room: f64| cell_of(q, w, h).is_some_and(|c| !ocean[c] && water[c] <= zref[i] + room);
    // the smoothing: a symmetric Gaussian window, narrowed near the ends so both ends stay put
    let sn = p.sigma / ds;
    let half = (3.0 * sn).ceil() as usize;
    let kern: Vec<f64> = (0..=half).map(|k| libm::exp(-0.5 * (k as f64 / sn) * (k as f64 / sn))).collect();
    let mut sm = raw.clone();
    for i in 1..n - 1 {
        let r = half.min(i).min(n - 1 - i);
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for k in 0..=r {
            let wt = kern[k];
            for j in if k == 0 { vec![i] } else { vec![i - k, i + k] } {
                sx += wt * raw[j].0;
                sy += wt * raw[j].1;
                sw += wt;
            }
        }
        let mut q = (sx / sw, sy / sw);
        // a smoothed point that climbs out of the valley is pulled back toward its path
        let mut tries = 0;
        while !ok(q, i, p.lift_m) && tries < 4 {
            q = (0.5 * (q.0 + raw[i].0), 0.5 * (q.1 + raw[i].1));
            tries += 1;
        }
        sm[i] = if ok(q, i, p.lift_m) || cell_of(q, w, h) == cell_of(raw[i], w, h) { q } else { raw[i] };
    }
    // meanders (row 6): a sine along the smoothed curve's normal, its wavelength wave_widths channel widths, tapered over half a wavelength at both ends and held to
    // the valley floor (room_m above the path); the amplitude's factor is the largest of 1, 3/4, 1/2, 1/4, 0 that keeps the point there, smoothed along the curve
    let total = ds * (n - 1) as f64;
    let lam: Vec<f64> = (0..n).map(|i| p.wave_widths * width_m(i as f64 / (n - 1) as f64) / p.cell_m).collect();
    let wide: Vec<bool> = (0..n).map(|i| p.meander && width_m(i as f64 / (n - 1) as f64) > p.min_width_m).collect();
    let mut phase = vec![0.0f64; n];
    for i in 1..n {
        phase[i] = phase[i - 1] + if wide[i] { 2.0 * std::f64::consts::PI * ds / lam[i].max(1e-9) } else { 0.0 };
    }
    let win = |i: usize| ((0.25 * lam[i] / ds).ceil() as usize).max(1);
    // numerics review F1, then lead #184's first row (a whole-curve fallback on one failing cell took the meanders off a third of the reaches): a failing raster cell
    // halves the meander's cap within a quarter wavelength of its point, and from the third pass also pulls the smoothed curve halfway back to the path within two
    // cells; up to five passes, then the fallbacks
    let mut cap = vec![1.0f64; n];
    let mut smc = sm.clone();
    let bad = |c: usize| ocean[c] || avoid(c);
    // a raster cell's point: its dense sample's share of the curve, at the path's sample; its room taken over the path's samples within two cells (the share is approximate
    // once the curve meanders)
    let room_at = |i: usize| (i.saturating_sub(4)..=(i + 4).min(n - 1)).map(|j| zref[j]).fold(f64::NEG_INFINITY, f64::max) + p.room_m;
    let failing = |cells: &[usize], at: &[usize], dn: usize, height: bool| -> Vec<usize> {
        let last = cells.len().saturating_sub(1);
        cells
            .iter()
            .enumerate()
            .filter_map(|(k, &c)| {
                let i = if dn > 1 { (libm::round(at[k] as f64 / (dn - 1) as f64 * (n - 1) as f64) as usize).min(n - 1) } else { 0 };
                (k != 0 && k != last && (bad(c) || (height && water[c] > room_at(i)))).then_some(i)
            })
            .collect()
    };
    for pass in 0..5 {
        let normal = |i: usize| {
            let (a, b) = (smc[i.saturating_sub(1)], smc[(i + 1).min(n - 1)]);
            let (tx, ty) = (b.0 - a.0, b.1 - a.1);
            let l = libm::hypot(tx, ty).max(1e-12);
            (-ty / l, tx / l)
        };
        let disp = |i: usize, m: f64| {
            if !wide[i] {
                return smc[i];
            }
            let s = ds * i as f64;
            let t = ((s.min(total - s)) / (0.5 * lam[i])).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            let a = m * cap[i] * p.amp_wave * lam[i] * t * libm::sin(phase[i]);
            let nv = normal(i);
            (smc[i].0 + a * nv.0, smc[i].1 + a * nv.1)
        };
        let mut pts = smc.clone();
        let mut share = 0.0;
        if p.meander {
            let fac: Vec<f64> = (0..n)
                .map(|i| [1.0, 0.75, 0.5, 0.25, 0.0].into_iter().find(|&m| m == 0.0 || ok(disp(i, m), i, p.room_m)).unwrap_or(0.0))
                .collect();
            // a quarter wavelength's moving minimum, then its moving mean, so the amplitude changes smoothly
            let mn: Vec<f64> =
                (0..n).map(|i| (i.saturating_sub(win(i))..=(i + win(i)).min(n - 1)).map(|j| fac[j]).fold(1.0, f64::min)).collect();
            let fm: Vec<f64> = (0..n)
                .map(|i| {
                    let r = i.saturating_sub(win(i))..=(i + win(i)).min(n - 1);
                    let k = r.clone().count() as f64;
                    r.map(|j| mn[j]).sum::<f64>() / k
                })
                .collect();
            let mut wavy = 0usize;
            for i in 1..n - 1 {
                let q = disp(i, fm[i]);
                // numerics review F6: a smoothed factor that leaves the room falls back to the point's own checked factor, not to no meander
                pts[i] = if ok(q, i, p.room_m) {
                    q
                } else if ok(disp(i, fac[i]), i, p.room_m) {
                    disp(i, fac[i])
                } else {
                    smc[i]
                };
                wavy += usize::from(wide[i] && fm[i] * cap[i] > 0.0);
            }
            share = wavy as f64 / n as f64;
        }
        let (raster, offsets, at, dn) = rasterise(&pts, w, h, water, &bad);
        let fail = failing(&raster, &at, dn, true);
        if fail.is_empty() {
            return Curve { pts, raster, offsets, meander_share: share, fallback: 0 };
        }
        for &i in &fail {
            for j in i.saturating_sub(win(i))..=(i + win(i)).min(n - 1) {
                cap[j] *= 0.5;
            }
            if pass >= 2 {
                for j in i.saturating_sub(4)..=(i + 4).min(n - 1) {
                    smc[j] = (0.5 * (smc[j].0 + raw[j].0), 0.5 * (smc[j].1 + raw[j].1));
                }
            }
        }
    }
    let (r4, o4, a4, d4) = rasterise(&raw, w, h, water, &bad);
    if failing(&r4, &a4, d4, false).is_empty() {
        return Curve { pts: raw, raster: r4, offsets: o4, meander_share: 0.0, fallback: 1 };
    }
    Curve { pts: raw, raster: base.to_vec(), offsets: vec![(0, 0); base.len()], meander_share: 0.0, fallback: 2 }
}

/// The curve's 4-connected raster: the cells its points fall in, a diagonal step filled with the edge cell not `bad` first, then the one nearer the curve (ties the lower on
/// `water`, then the lower id), loops cut at the first return to a cell; each cell's offset is the mean of its points' offsets from its centre, in 1/254 of a cell (a fill
/// cell has none: 0, 0). Also each cell's dense-sample index and the dense samples' count.
pub fn rasterise(
    pts: &[(f64, f64)],
    w: usize,
    h: usize,
    water: &[f64],
    bad: &dyn Fn(usize) -> bool,
) -> (Vec<usize>, Vec<(i8, i8)>, Vec<usize>, usize) {
    let dense = resample(pts, 0.2);
    let mut idx: Vec<usize> = Vec::new();
    let mut cells: Vec<usize> = Vec::new();
    let mut sums: Vec<(f64, f64, f64)> = Vec::new();
    let mut at: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut push =
        |c: usize, q: Option<(f64, f64)>, cells: &mut Vec<usize>, sums: &mut Vec<(f64, f64, f64)>, idx: &mut Vec<usize>, s: usize| {
            if cells.last() == Some(&c) {
                if let Some(q) = q {
                    let s = sums.last_mut().expect("a cell");
                    *s = (s.0 + q.0, s.1 + q.1, s.2 + 1.0);
                }
                return;
            }
            if let Some(&k) = at.get(&c) {
                // a loop: back to the first visit
                for d in cells.drain(k + 1..) {
                    at.remove(&d);
                }
                sums.truncate(k + 1);
                idx.truncate(k + 1);
            } else {
                at.insert(c, cells.len());
                cells.push(c);
                sums.push((0.0, 0.0, 0.0));
                idx.push(s);
            }
            if let Some(q) = q {
                let s = sums.last_mut().expect("a cell");
                *s = (s.0 + q.0, s.1 + q.1, s.2 + 1.0);
            }
        };
    let mut prev: Option<(usize, (f64, f64))> = None;
    for (si, &q) in dense.iter().enumerate() {
        let Some(c) = cell_of(q, w, h) else { continue };
        if let Some((pc, pq)) = prev {
            let (mut cx, mut cy) = ((pc % w) as i64, (pc / w) as i64);
            let (tx, ty) = ((c % w) as i64, (c / w) as i64);
            // walk 4-connected from the previous cell to this one
            while (cx, cy) != (tx, ty) {
                let (dx, dy) = ((tx - cx).signum(), (ty - cy).signum());
                let (ax, ay) = (cx + dx, cy);
                let (bx, by) = (cx, cy + dy);
                let step = if dx == 0 {
                    (bx, by)
                } else if dy == 0 {
                    (ax, ay)
                } else {
                    // the edge cell nearer the segment pq -> q, ties the lower, then the lower id
                    let seg = |x: i64, y: i64| {
                        let (px, py) = (x as f64, y as f64);
                        let (sx, sy, ex, ey) = (pq.0, pq.1, q.0, q.1);
                        let (vx, vy) = (ex - sx, ey - sy);
                        let l2 = vx * vx + vy * vy;
                        let t = if l2 > 0.0 { (((px - sx) * vx + (py - sy) * vy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                        libm::hypot(px - sx - t * vx, py - sy - t * vy)
                    };
                    let (da, db) = (seg(ax, ay), seg(bx, by));
                    let (ia, ib) = (ay as usize * w + ax as usize, by as usize * w + bx as usize);
                    let (ba, bb) = (bad(ia), bad(ib));
                    if (!ba && bb)
                        || (ba == bb && (da < db || (da == db && (water[ia] < water[ib] || (water[ia] == water[ib] && ia < ib)))))
                    {
                        (ax, ay)
                    } else {
                        (bx, by)
                    }
                };
                (cx, cy) = step;
                let sc = cy as usize * w + cx as usize;
                push(sc, if sc == c { Some(q) } else { None }, &mut cells, &mut sums, &mut idx, si);
            }
            if pc == c {
                push(c, Some(q), &mut cells, &mut sums, &mut idx, si);
            }
        } else {
            push(c, Some(q), &mut cells, &mut sums, &mut idx, si);
        }
        prev = Some((c, q));
    }
    let offsets = cells
        .iter()
        .zip(&sums)
        .map(|(&c, &(sx, sy, k))| {
            if k == 0.0 {
                return (0, 0);
            }
            let (ox, oy) = (sx / k - (c % w) as f64, sy / k - (c / w) as f64);
            let code = |v: f64| libm::round(v * 254.0).clamp(-127.0, 127.0) as i8;
            (code(ox), code(oy))
        })
        .collect();
    (cells, offsets, idx, dense.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: lead #184 (spike)
    #[test]
    fn the_raster_is_4_connected_without_repeats_and_keeps_both_ends() {
        let (w, h) = (40usize, 40usize);
        let water = vec![0.0; w * h];
        // a diagonal line and a loop back
        let pts = vec![(2.0, 2.0), (20.0, 20.0), (25.0, 18.0), (21.0, 21.0), (35.0, 30.0)];
        let (r, off, _, _) = rasterise(&pts, w, h, &water, &|_| false);
        assert_eq!(r[0], 2 * w + 2);
        assert_eq!(*r.last().unwrap(), 30 * w + 35);
        assert_eq!(r.len(), off.len());
        for s in r.windows(2) {
            let (ax, ay, bx, by) = ((s[0] % w) as i64, (s[0] / w) as i64, (s[1] % w) as i64, (s[1] / w) as i64);
            assert_eq!((ax - bx).abs() + (ay - by).abs(), 1, "a 4-connected step");
        }
        let mut seen = std::collections::HashSet::new();
        assert!(r.iter().all(|c| seen.insert(*c)), "no cell twice");
    }

    // spec: lead #184 (spike)
    #[test]
    fn a_wide_river_meanders_on_a_flat_floor_and_a_narrow_one_does_not() {
        let (w, h) = (300usize, 60usize);
        let water = vec![10.0; w * h];
        let ocean = vec![false; w * h];
        let base: Vec<usize> = (5..295).map(|x| 30 * w + x).collect();
        let p = CurveParams::standard(100.0, true);
        let wide = build(&base, w, h, &water, &ocean, &|_| false, &|_| 100.0, &p);
        let narrow = build(&base, w, h, &water, &ocean, &|_| false, &|_| 5.0, &p);
        let len = |c: &Curve| c.pts.windows(2).map(|s| dist(s[0], s[1])).sum::<f64>();
        let chord = 289.0;
        let (sw, sn) = (len(&wide) / chord, len(&narrow) / chord);
        assert!((1.15..1.45).contains(&sw), "a 100 m river's sinuosity {sw}");
        assert!((sn - 1.0).abs() < 1e-6, "a 5 m river stays straight: {sn}");
        assert!(wide.meander_share > 0.8);
        // a wall: the floor two cells wide, everything else 30 m higher: the meander is held to the floor
        let mut walled = vec![40.0; w * h];
        for x in 0..w {
            for y in 29..=31 {
                walled[y * w + x] = 10.0;
            }
        }
        let held = build(&base, w, h, &walled, &ocean, &|_| false, &|_| 100.0, &p);
        assert!(held.pts.iter().all(|&q| walled[cell_of(q, w, h).unwrap()] <= 20.0), "every point stays within room_m of the floor");
        assert!(held.raster.iter().all(|&c| walled[c] <= 20.0), "and so does every raster cell (F1)");
    }

    // spec: lead #184 (spike; numerics review F1)
    #[test]
    fn a_diagonal_fill_takes_the_land_cell_and_no_raster_cell_is_sea_or_taken() {
        let (w, h) = (40usize, 40usize);
        let water = vec![0.0; w * h];
        let pts = vec![(2.0, 2.0), (30.0, 30.0)];
        // every cell below the diagonal is "sea": the fill must take the cells above it
        let sea = |c: usize| (c / w) > (c % w);
        let (r, _, _, _) = rasterise(&pts, w, h, &water, &sea);
        assert!(r[1..r.len() - 1].iter().all(|&c| !sea(c)), "no fill cell is sea");
        let ocean: Vec<bool> = (0..w * h).map(sea).collect();
        let base: Vec<usize> = (2..=30).map(|i| i * w + i).collect();
        let p = CurveParams::standard(100.0, false);
        let c = build(&base, w, h, &water, &ocean, &|_| false, &|_| 5.0, &p);
        assert!(c.raster[1..c.raster.len() - 1].iter().all(|&x| !ocean[x]), "fallback {}", c.fallback);
    }
}
