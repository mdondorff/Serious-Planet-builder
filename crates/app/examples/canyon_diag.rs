//! River review (#138, spike branch spike/river-review, never merged): why main's river gate and SPIKE-117 disagree on canyons. On a surface it runs the gate's `measure` unchanged and splits
//! its canyon (valley ring p50 minus the river level) into the visible incision (ring p50 minus the surface at the river cell) and the burn (the surface minus the level), for two rings.
//! `canyon_diag surface <file.f64> <window w> <box x0> <box size> <start.f64>`: a variant dump of erosion_at_l (the start for the erosion depth); counted over the region's box.
//! `canyon_diag glo30 <tile.tif>`: a GLO-30 tile resampled to square 101.8 m cells (box mean of the 1" pixels in each cell), start = surface; counted inside a 10-cell border.
use planet_cache::river_gate::{measure, GateInput, GateParams};
use planet_generators::bake::route::{route_rules, RoundRules, NONE};
use planet_generators::bake::Grid;

const CELL_M: f64 = 101.8;

fn read_f64(path: &str, n: usize) -> Vec<f64> {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(b.len(), n * 8, "{path}: {} bytes, expected {}", b.len(), n * 8);
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

fn ring_p50(z: &[f64], w: usize, h: usize, c: usize, r0: i64, r1: i64) -> f64 {
    let (cx, cy) = ((c % w) as i64, (c / w) as i64);
    let mut v = Vec::new();
    for dy in -r1..=r1 {
        for dx in -r1..=r1 {
            let (x, y) = (cx + dx, cy + dy);
            if dx.abs().max(dy.abs()) >= r0 && x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                v.push(z[(y as usize) * w + x as usize]);
            }
        }
    }
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(z[c])
}

fn share_over(v: &[f64], t: f64) -> String {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let p = |f: f64| s[((s.len() as f64 * f) as usize).min(s.len().saturating_sub(1))];
    format!(
        "{:.2} % over {t} m (p50 {:.1}, p90 {:.1}, p99 {:.1} m)",
        100.0 * s.iter().filter(|&&x| x > t).count() as f64 / s.len().max(1) as f64,
        p(0.5),
        p(0.9),
        p(0.99)
    )
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (w, h, surface, start, inside, label) = match a[0].as_str() {
        "surface" => {
            let (ww, x0, size): (usize, usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap());
            let s = read_f64(&a[1], ww * ww);
            let st = read_f64(&a[5], ww * ww);
            let inside: Vec<bool> =
                (0..ww * ww).map(|c| (x0..x0 + size).contains(&(c % ww)) && (x0..x0 + size).contains(&(c / ww))).collect();
            (ww, ww, s, st, inside, a[1].clone())
        }
        "glo30" => {
            let t = planet_testkit::glo30::read(std::path::Path::new(&a[1])).unwrap_or_else(|e| panic!("{}: {e:?}", a[1]));
            let lat0 = t.geo.origin_lat - 0.5 * t.height as f64 * t.geo.scale_lat;
            let m_per_deg = 111_320.0;
            let (py, px) = (t.geo.scale_lat * m_per_deg, t.geo.scale_lon * m_per_deg * lat0.to_radians().cos());
            let (h, w) = ((t.height as f64 * py / CELL_M) as usize, (t.width as f64 * px / CELL_M) as usize);
            let mut z = vec![f64::NAN; w * h];
            for (c, v) in z.iter_mut().enumerate() {
                let (r0, r1) = ((((c / w) as f64) * CELL_M / py) as usize, ((((c / w) + 1) as f64) * CELL_M / py) as usize);
                let (c0, c1) = ((((c % w) as f64) * CELL_M / px) as usize, ((((c % w) + 1) as f64) * CELL_M / px) as usize);
                let (mut s, mut k) = (0.0, 0usize);
                for r in r0..r1.min(t.height) {
                    for col in c0..c1.min(t.width) {
                        let x = t.sample(r, col);
                        if x.is_finite() {
                            s += f64::from(x);
                            k += 1;
                        }
                    }
                }
                *v = if k > 0 { s / k as f64 } else { -100.0 };
            }
            let inside: Vec<bool> = (0..w * h).map(|c| (10..w - 10).contains(&(c % w)) && (10..h - 10).contains(&(c / w))).collect();
            (w, h, z.clone(), z, inside, format!("{} at {:.1} m ({w} x {h})", a[1], CELL_M))
        }
        m => panic!("mode surface or glo30, not {m}"),
    };
    let g = Grid::new(w, h, CELL_M);
    let outlet = g.outlets(&surface, 0.0);
    let region = vec![true; w * h];
    let params = GateParams::default();
    let report = measure(
        &GateInput {
            grid: &g,
            start: &start,
            surface: &surface,
            outlet: &outlet,
            inside: &inside,
            region: &region,
            h1: &[],
            seed: 7,
            sea_level_m: 0.0,
            outside_drains: false,
        },
        &params,
    );
    println!("CANYON input {label}");
    for l in report.lines() {
        println!("CANYON gate {l}");
    }
    // The split: the river cells by the same rule (D8 on the plain flood, area >= the gate's channel area, land), the level as the gate's breached level (every pit breached).
    let r = route_rules(&g, &surface, &outlet, 1.0e-3, 7, 0.0, None, RoundRules::default());
    let mut area = vec![CELL_M * CELL_M; w * h];
    for &c in r.order.iter().rev() {
        let c = c as usize;
        if r.receiver[c] != NONE {
            area[r.receiver[c] as usize] += area[c];
        }
    }
    let river: Vec<bool> = (0..w * h).map(|c| area[c] >= params.channel_area_m2 && surface[c] > 0.0).collect();
    let mut level = vec![f64::INFINITY; w * h];
    for &c in r.order.iter().rev() {
        let c = c as usize;
        if !river[c] {
            continue;
        }
        let v = level[c].min(surface[c]);
        level[c] = v;
        let rc = r.receiver[c];
        if rc != NONE && river[rc as usize] {
            level[rc as usize] = level[rc as usize].min(v);
        }
    }
    let (mut vis24, mut vis12, mut burn, mut gate24) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for c in 0..w * h {
        if !river[c] || !inside[c] {
            continue;
        }
        let ring24 = ring_p50(&surface, w, h, c, 2, 4);
        vis24.push(ring24 - surface[c]);
        vis12.push(ring_p50(&surface, w, h, c, 1, 2) - surface[c]);
        burn.push(surface[c] - level[c]);
        gate24.push(ring24 - level[c]);
    }
    println!("CANYON split, {} river cells, every pit breached:", vis24.len());
    println!("CANYON   ring 2-4 minus level (the gate's breached canyon): {}", share_over(&gate24, 50.0));
    println!("CANYON   visible incision, ring 2-4 (204-407 m) minus the surface: {}", share_over(&vis24, 50.0));
    println!("CANYON   visible incision, ring 1-2 (102-204 m) minus the surface: {}", share_over(&vis12, 50.0));
    println!("CANYON   burn, the surface minus the level (SPIKE-117's canyon at D = inf): {}", share_over(&burn, 50.0));
}
