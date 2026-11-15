//! River review (#138, spike branch spike/river-review, never merged): the terrain scorecard's descriptors (the functions `terrain-score` calls, with its scales in metres) on a dumped
//! variant surface at L, over the region's box. `variant_descriptors <file.f64> <window w> <box x0> <box size> [--threads T]`; prints one CSV line:
//! slope p50,p90 (deg); relief 1 km and 5 km p50,p90 (m); TPI 300 m and 2 km p10,p90 (m); geomorphon shares (flat, ridge, slope, valley, pit, %); drainage density (km/km2).
use planet_generators::descriptors as desc;

const CELL_M: f64 = 101.8;

fn cells_for(metres: f64) -> usize {
    ((metres / CELL_M).round() as usize).max(1)
}

fn pct(mut v: Vec<f64>, q: &[f64]) -> Vec<f64> {
    v.retain(|x| x.is_finite());
    v.sort_by(f64::total_cmp);
    q.iter().map(|&p| if v.is_empty() { f64::NAN } else { v[((p / 100.0) * (v.len() - 1) as f64).round() as usize] }).collect()
}

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let file = &a[0];
    // `<window w>` may be `WxH` (a GLO-30 dump); the box is `size` square from (x0, x0), or with `size` 0 the whole raster less `x0` cells a side.
    let (ww, wh) = match a[1].split_once('x') {
        Some((x, y)) => (x.parse::<usize>().unwrap(), y.parse::<usize>().unwrap()),
        None => (a[1].parse().unwrap(), a[1].parse().unwrap()),
    };
    let (x0, size): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let (bw, bh) = if size == 0 { (ww - 2 * x0, wh - 2 * x0) } else { (size, size) };
    let threads: usize = a.iter().position(|x| x == "--threads").map_or(8, |k| a[k + 1].parse().unwrap());
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    if bytes.len() != ww * wh * 8 {
        return Err(format!("{file}: {} bytes, a {ww} x {wh} window needs {}", bytes.len(), ww * wh * 8));
    }
    let mut z = vec![0f64; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            let c = (x0 + y) * ww + x0 + x;
            z[y * bw + x] = f64::from_le_bytes(bytes[c * 8..c * 8 + 8].try_into().unwrap());
        }
    }
    drop(bytes);
    let r = desc::Raster::new(&z, bw, bh, CELL_M);
    let slope = pct(desc::slope_degrees(&r, threads), &[50.0, 90.0]);
    let r1 = pct(desc::local_relief(&r, cells_for(1000.0), threads).values, &[50.0, 90.0]);
    let r5 = pct(desc::local_relief(&r, cells_for(5000.0), threads).values, &[50.0, 90.0]);
    let t3 = pct(desc::tpi(&r, cells_for(300.0), threads), &[10.0, 90.0]);
    let t2 = pct(desc::tpi(&r, cells_for(2000.0), threads), &[10.0, 90.0]);
    let forms = desc::geomorphons(&r, cells_for(1000.0), 1.0, threads);
    let mut counts = [0usize; 10];
    for &f in &forms {
        if f != desc::UNCOUNTED {
            counts[usize::from(f)] += 1;
        }
    }
    drop(forms);
    let total: usize = counts.iter().sum::<usize>().max(1);
    let share = |k: usize| 100.0 * counts[k] as f64 / total as f64;
    let d = desc::drainage(&r);
    let density = desc::drainage_density(&r, &d, desc::CHANNEL_AREA_M2);
    use desc::form::*;
    println!(
        "DESC {file},{:.2},{:.2},{:.1},{:.1},{:.1},{:.1},{:.2},{:.2},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.2},{:.3}",
        slope[0],
        slope[1],
        r1[0],
        r1[1],
        r5[0],
        r5[1],
        t3[0],
        t3[1],
        t2[0],
        t2[1],
        share(FLAT as usize),
        share(RIDGE as usize),
        share(SLOPE as usize),
        share(VALLEY as usize),
        share(PIT as usize),
        density
    );
    Ok(())
}
