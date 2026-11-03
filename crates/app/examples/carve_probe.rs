//! Diagnosis probe (inbox #51, not committed): the bake slope that drives the crease amplitude (`carve(slope)`), v1 (bilinear base) against v2 (C1 base), over
//! a window around a site: histograms of the slope and the carve factor, the share of cells whose carve changes by more than 10 %, and the `CARVE_TO` that
//! would restore v1's mean carve on v2. CPU only. `cargo run --release -p planet --example carve_probe -- --lat 19.1589 --lon 155.3686`

use std::path::PathBuf;

use planet::worldfield::WorldTerrain;
use planet_core::Vec3;
use planet_world_def::WorldDefinition;

const FLOOR: f64 = 0.15;
const FROM: f64 = 0.01;
const TO: f64 = 0.2;
const SLOPE_CELLS: f64 = 3.0;
const RES: f64 = 16384.0;
const R: f64 = 6_371_000.0;

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn carve_with(slope: f64, from: f64, to: f64) -> f64 {
    FLOOR + (1.0 - FLOOR) * smoothstep(from, to, slope)
}

fn frame(lat: f64, lon: f64) -> (Vec3, Vec3, Vec3) {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    let (sla, cla, slo, clo) = (libm::sin(la), libm::cos(la), libm::sin(lo), libm::cos(lo));
    (Vec3::new(cla * clo, cla * slo, sla), Vec3::new(-slo, clo, 0.0), Vec3::new(-sla * clo, -sla * slo, cla))
}

/// The bake's gradient magnitude in metres per radian by the field's own stencil (a central difference over `SLOPE_CELLS` cells along each axis' tangent part).
fn gradient(base: &dyn Fn(Vec3) -> f64, d: Vec3) -> f64 {
    let delta = SLOPE_CELLS * core::f64::consts::FRAC_PI_2 / RES;
    let mut g = Vec3::new(0.0, 0.0, 0.0);
    for a in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
        let t = a - d * a.dot(d);
        let rise = base((d + t * delta).normalized()) - base((d - t * delta).normalized());
        g = g + t * (rise / (2.0 * delta));
    }
    g.length()
}

fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((s.len() as f64 - 1.0) * q) as usize]
}

fn hist(v: &[f64], lo: f64, hi: f64, bins: usize) -> Vec<f64> {
    let mut h = vec![0.0; bins];
    for x in v {
        let k = (((x - lo) / (hi - lo)) * bins as f64).floor().clamp(0.0, bins as f64 - 1.0) as usize;
        h[k] += 1.0;
    }
    h.iter().map(|c| c / v.len() as f64 * 100.0).collect()
}

fn main() -> Result<(), String> {
    let (mut lat, mut lon, mut half_km, mut step_m) = (19.1589, 155.3686, 4.0, 100.0);
    let mut store = PathBuf::from("C:/DEV/SeriousPlanetData/global16384-m9r2");
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut v = |n: &str| it.next().ok_or(format!("{n} needs a value"));
        match a.as_str() {
            "--lat" => lat = v("--lat")?.parse().map_err(|e| format!("{e}"))?,
            "--lon" => lon = v("--lon")?.parse().map_err(|e| format!("{e}"))?,
            "--half-km" => half_km = v("--half-km")?.parse().map_err(|e| format!("{e}"))?,
            "--step-m" => step_m = v("--step-m")?.parse().map_err(|e| format!("{e}"))?,
            "--store" => store = v("--store")?.into(),
            o => return Err(format!("unknown {o}")),
        }
    }
    let def = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 1);
    let world = WorldTerrain::open_store(def, &store, 16384)?;
    let v1 = |d: Vec3| world.bake_probe(d).map_or(f64::NAN, |p| f64::from(p.elevation_m));
    let v2 = |d: Vec3| world.base_elevation_c1(d).unwrap_or(f64::NAN);
    let f = frame(lat, lon);
    let n = (2.0 * half_km * 1000.0 / step_m) as i64;
    let (mut s1, mut s2, mut c1, mut c2, mut elev) = (vec![], vec![], vec![], vec![], vec![]);
    for iy in 0..n {
        for ix in 0..n {
            let (x, y) = (-half_km * 1000.0 + ix as f64 * step_m, -half_km * 1000.0 + iy as f64 * step_m);
            let p = f.0 * R + f.1 * x + f.2 * y;
            let d = p * (1.0 / libm::sqrt(p.x * p.x + p.y * p.y + p.z * p.z));
            let (a, b) = (gradient(&v1, d) / R, gradient(&v2, d) / R);
            s1.push(a);
            s2.push(b);
            c1.push(carve_with(a, FROM, TO));
            c2.push(carve_with(b, FROM, TO));
            elev.push(v2(d));
        }
    }
    println!("window {n} x {n} cells of {step_m} m around {lat} N {lon} E; base elevation median {:.0} m", quantile(&elev, 0.5));
    println!(
        "slope (rise over run) quantiles p10 / p50 / p90 / p99: v1 {:.4} {:.4} {:.4} {:.4}; v2 {:.4} {:.4} {:.4} {:.4}",
        quantile(&s1, 0.1),
        quantile(&s1, 0.5),
        quantile(&s1, 0.9),
        quantile(&s1, 0.99),
        quantile(&s2, 0.1),
        quantile(&s2, 0.5),
        quantile(&s2, 0.9),
        quantile(&s2, 0.99)
    );
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "carve factor mean: v1 {:.4}, v2 {:.4}; quantiles p10 / p50 / p90: v1 {:.3} {:.3} {:.3}; v2 {:.3} {:.3} {:.3}",
        mean(&c1),
        mean(&c2),
        quantile(&c1, 0.1),
        quantile(&c1, 0.5),
        quantile(&c1, 0.9),
        quantile(&c2, 0.1),
        quantile(&c2, 0.5),
        quantile(&c2, 0.9)
    );
    let changed = c1.iter().zip(&c2).filter(|(a, b)| (**b - **a).abs() > 0.1 * **a).count();
    let lower = c1.iter().zip(&c2).filter(|(a, b)| **b < **a * 0.9).count();
    println!(
        "cells whose carve changes by more than 10 %: {:.1} % (lower by more than 10 %: {:.1} %)",
        changed as f64 / c1.len() as f64 * 100.0,
        lower as f64 / c1.len() as f64 * 100.0
    );
    println!(
        "carve histogram (% of cells, 10 bins from 0.15 to 1.0):\n  v1 {:?}\n  v2 {:?}",
        hist(&c1, 0.15, 1.0001, 10).iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>(),
        hist(&c2, 0.15, 1.0001, 10).iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>()
    );
    println!(
        "slope histogram (% of cells, 10 bins from 0 to 0.4):\n  v1 {:?}\n  v2 {:?}",
        hist(&s1, 0.0, 0.4, 10).iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>(),
        hist(&s2, 0.0, 0.4, 10).iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>()
    );
    // The smallest change that restores v1's carve distribution on v2: lower CARVE_TO (the slope where the factor saturates), then CARVE_FROM, one parameter each, by the mean and the p10/p50/p90 gap.
    let target = (mean(&c1), quantile(&c1, 0.1), quantile(&c1, 0.5), quantile(&c1, 0.9));
    let score = |c: &[f64]| {
        (mean(c) - target.0).abs()
            + (quantile(c, 0.1) - target.1).abs()
            + (quantile(c, 0.5) - target.2).abs()
            + (quantile(c, 0.9) - target.3).abs()
    };
    let (mut best_to, mut best) = (TO, f64::MAX);
    for k in 0..=200 {
        let to = 0.05 + 0.2 * f64::from(k) / 200.0;
        let c: Vec<f64> = s2.iter().map(|s| carve_with(*s, FROM, to)).collect();
        let sc = score(&c);
        if sc < best {
            best = sc;
            best_to = to;
        }
    }
    let (mut best_from, mut best_f) = (FROM, f64::MAX);
    for k in 0..=200 {
        let from = 0.0 + 0.1 * f64::from(k) / 200.0;
        let c: Vec<f64> = s2.iter().map(|s| carve_with(*s, from, TO)).collect();
        let sc = score(&c);
        if sc < best_f {
            best_f = sc;
            best_from = from;
        }
    }
    let at = |from: f64, to: f64| -> (f64, f64) {
        let c: Vec<f64> = s2.iter().map(|s| carve_with(*s, from, to)).collect();
        (mean(&c), c.iter().zip(&c1).filter(|(a, b)| (**a - **b).abs() > 0.1 * **b).count() as f64 / c.len() as f64 * 100.0)
    };
    println!("restore v1's carve distribution on v2: CARVE_TO {TO} -> {best_to:.3} (mean {:.4}, {:.1} % of cells still off by more than 10 %); or CARVE_FROM {FROM} -> {best_from:.4} (mean {:.4}, {:.1} % off)",
        at(FROM, best_to).0, at(FROM, best_to).1, at(best_from, TO).0, at(best_from, TO).1);
    Ok(())
}
