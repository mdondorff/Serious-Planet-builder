//! `cargo xtask perf preview` runs this (TEST-099): the latency of the level-1 preview (A3 11.1) at the H0 grid. The resident base of a session is built once from the default class source
//! (heights, rainfall and temperature of every H0 cell, the baseline routing and discharge); then a height stamp, a rainfall region and both are previewed, each after a warm run and over
//! three runs, with the phases (stamps, route, discharge, overlay), the process peak working set (PowerShell, Windows) and the resident bytes.
//!
//! Usage: `preview_latency [n] [--threads N] [--seed N] [--smoke]`. `n` is the cells a face edge (default 3,072, the H0 grid of 3.26 km); `--smoke` is `n` = 96 and the table says it is no
//! measurement. A timing read needs a quiet machine and a perf window: this file only measures.

use planet_core::Vec3;
use planet_generators::bake::preview::{ClimateRegion, PreviewBase, PreviewEdit};
use planet_world_def::footprint::FootprintItem;
use planet_world_def::stamp::{Stamp, StampKind};
use std::time::Instant;

const SEA_LEVEL: f64 = 0.0;
const RUNS: usize = 3;

fn fail(msg: &str) -> ! {
    eprintln!("preview_latency: {msg}");
    std::process::exit(2)
}

/// The peak and the current working set of this process in MiB (PowerShell; NaN elsewhere).
fn memory_mb() -> (f64, f64) {
    let ps = |field: &str| -> f64 {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).{field}", std::process::id())])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
            .map_or(f64::NAN, |b| b / 1_048_576.0)
    };
    (ps("PeakWorkingSet64"), ps("WorkingSet64"))
}

/// A hill of `amount` metres reaching `reach_m` at `dir`.
fn hill(dir: Vec3, reach_m: f64, amount: f64) -> FootprintItem {
    FootprintItem {
        id: 1,
        priority: 0,
        content: 0x70_7265_7669_6577,
        dir: [dir.x.to_bits(), dir.y.to_bits(), dir.z.to_bits()],
        reach_bits: reach_m.to_bits(),
        max_delta_bits: amount.to_bits(),
        layer: "preview".into(),
        stamp: Some(Stamp::new(StampKind::Hill, [amount, 0.0, 0.0])),
    }
}

struct Row {
    name: &'static str,
    totals: Vec<f64>,
    phases: [f64; 4],
    changed: usize,
    trunk: usize,
    peak_mb: f64,
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}

/// One preview with its phases.
fn timed(base: &PreviewBase, edit: &PreviewEdit, trunk_discharge: f64) -> (f64, [f64; 4], usize, usize) {
    let t = Instant::now();
    let (p, ph) = base.preview_timed(edit, trunk_discharge).unwrap_or_else(|e| fail(&format!("the preview failed: {e}")));
    (t.elapsed().as_secs_f64(), [ph.stamps_s, ph.route_s, ph.discharge_s, ph.overlay_s], p.overlay.changed.len(), p.overlay.trunk.len())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut n, mut threads, mut seed, mut smoke) = (3072usize, 0usize, 3u64, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--smoke" => smoke = true,
            "--threads" => threads = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| fail("--threads needs an integer")),
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| fail("--seed needs a non-negative integer")),
            v if v.parse::<usize>().is_ok() => n = v.parse().unwrap_or(n),
            other => fail(&format!("unknown argument '{other}' (known: N, --threads N, --seed N, --smoke)")),
        }
    }
    if smoke {
        n = 96;
    }
    if n < 8 {
        fail("n is at least 8 cells a face edge");
    }
    if threads == 0 {
        threads = std::thread::available_parallelism().map_or(1, |k| k.get());
    }
    let label = if smoke {
        "NOT A MEASUREMENT (smoke, n = 96)"
    } else {
        "untimed until a perf window authorises it: read after the display changes of 2026-11-09"
    };
    println!("preview_latency: n {n} ({} cells), {threads} threads, seed {seed}; {label}", 6 * n * n);

    let t0 = Instant::now();
    let base = PreviewBase::from_class_source(n, seed, threads);
    let base_s = t0.elapsed().as_secs_f64();
    let (z0, p_mm) = (base.heights(), base.rainfall());
    let land = z0.iter().filter(|z| **z > SEA_LEVEL).count();
    // The anchors: the highest land cell for the hill, the wettest land cell's neighbourhood for the rain.
    let top = (0..z0.len())
        .filter(|&c| z0[c] > SEA_LEVEL)
        .max_by(|a, b| z0[*a].total_cmp(&z0[*b]))
        .unwrap_or_else(|| fail("the base has no land"));
    let wet = (0..p_mm.len()).filter(|&c| z0[c] > SEA_LEVEL).max_by(|a, b| p_mm[*a].total_cmp(&p_mm[*b])).unwrap_or(top);
    let (top_dir, wet_dir) = (base.grid().direction(top as u32), base.grid().direction(wet as u32));
    let (peak_base, now_base) = memory_mb();
    let cells = base.grid().len();
    let resident_mb = (cells * (8 + 4 + 4)) as f64 / 1_048_576.0 + (cells * (4 + 8 + 4 + 8)) as f64 / 1_048_576.0;
    println!("base: {base_s:.1} s once (sampling and the baseline route), {land} land cells ({:.1} %), peak {peak_base:.0} MiB, now {now_base:.0} MiB, session about {resident_mb:.0} MiB", 100.0 * land as f64 / cells as f64);

    let reach = if smoke { 800_000.0 } else { 400_000.0 };
    let stamp = vec![hill(top_dir, reach, 300.0)];
    let rain = vec![ClimateRegion { dir: wet_dir, reach_m: 2.0 * reach, rain_factor: 2.0, temp_offset_c: 0.0 }];
    let edits = [
        ("height stamp", PreviewEdit { stamps: stamp.clone(), climate: vec![] }),
        ("rainfall region", PreviewEdit { stamps: vec![], climate: rain.clone() }),
        ("both", PreviewEdit { stamps: stamp, climate: rain }),
    ];
    let trunk_discharge = 1.0e12;
    let mut rows = Vec::new();
    for (name, edit) in &edits {
        let _warm = timed(&base, edit, trunk_discharge);
        let (mut totals, mut phases, mut changed, mut trunk) = (Vec::new(), [0.0; 4], 0, 0);
        for _ in 0..RUNS {
            let (total, ph, c, t) = timed(&base, edit, trunk_discharge);
            totals.push(total);
            for (a, b) in phases.iter_mut().zip(ph) {
                *a += b / RUNS as f64;
            }
            (changed, trunk) = (c, t);
        }
        let (peak, _) = memory_mb();
        rows.push(Row { name, totals, phases, changed, trunk, peak_mb: peak });
    }
    println!("\n| edit | median s | max s | stamps s | route s | discharge s | overlay s | changed cells | trunk cells | peak MiB |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        let max = r.totals.iter().cloned().fold(0.0, f64::max);
        println!(
            "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} | {:.0} |",
            r.name,
            median(&r.totals),
            max,
            r.phases[0],
            r.phases[1],
            r.phases[2],
            r.phases[3],
            r.changed,
            r.trunk,
            r.peak_mb
        );
    }
    println!("\n{label}");
}
