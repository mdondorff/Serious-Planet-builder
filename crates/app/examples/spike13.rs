//! SPIKE-13 ONLY (never merged): edit-to-visible latency cases.
//!
//! `cargo run --release -p planet --example spike13 -- --adapter hardware|software [--size WxH] [--out DIR]
//!   [--altitude M --radius M --moving --mode evict|refresh --preview --workers N --cache 0|1|2 --images DIR]`
//! With no case options the matrix of the spike runs (3 altitudes x 3 radii x still/moving, refresh mode, preview on).

#![allow(clippy::disallowed_methods)]

use planet::spike13::*;
use planet_render::{AdapterPolicy, GpuContext};
use std::fmt::Write as _;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn flag(name: &str) -> bool {
    std::env::args().any(|x| x == name)
}

fn f(v: Option<f64>) -> String {
    v.map_or("-".into(), |x| format!("{x:.1}"))
}

fn row(cfg: &CaseCfg, r: &CaseResult) -> String {
    let gen: Vec<String> = r.gen.per_level.iter().map(|(l, (n, mean, max))| format!("{l}:{n}x{mean:.1}/{max:.0}")).collect();
    format!(
        "| {} | {} | {} | {:?} | {} | {} | {} ({}) / {} | {:.1} / {:.1} / {:.1} | {} | {} | {} | {} | {} | {} | {:.0} / {:.1} | {:.1} / {} | {} / {} | {} | {} |",
        cfg.altitude_m,
        cfg.radius_m,
        if cfg.moving { "moving" } else { "still" },
        cfg.mode,
        cfg.workers,
        r.resident_before,
        r.affected.total(),
        r.affected.text(),
        r.affected_cells_method,
        r.find_keys_ms,
        r.find_cells_ms,
        r.apply_ms,
        format!("{} (worker {:.0})", f(r.plan_delivered_ms), r.plan_worker_ms),
        f(r.first_dispatch_ms),
        f(r.preview_ms),
        f(r.first_new_ms),
        f(r.exact_ms),
        f(r.all_ms),
        r.gen.total_ms,
        r.gen.wait_mean_ms,
        r.upload_ms_total,
        r.tiles_uploaded,
        r.substituted_max,
        r.blank_max,
        gen.join(" "),
        format!("settle {:.0} s, exact frame {:.1} ms, {} frames; {}{}", r.settle_s, r.exact_frame_ms, r.frames, r.load_line, r.error.clone().map_or(String::new(), |e| format!(" ERROR {e}"))),
    )
}

const HEADER: &str = "| altitude m | radius m | camera | mode | workers | resident | affected by key (levels) / by cells | find keys / cells / apply ms | plan delivered ms | first dispatch ms | preview ms | first new ms | exact ms | all ms | generation sum ms / mean wait ms | upload ms / tiles | substituted max / blank max | generation per level n x mean/max ms | load or error |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|";

fn main() {
    let policy = match arg("--adapter").as_deref() {
        Some("hardware") => AdapterPolicy::HighPerformance,
        _ => AdapterPolicy::Software,
    };
    let size = arg("--size")
        .and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())))
        .unwrap_or((1280u32, 720u32));
    let ctx = GpuContext::new(policy).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let t = std::time::Instant::now();
    let base = base_world(7);
    println!("base world built in {:.1} s", t.elapsed().as_secs_f64());
    let workers: usize = arg("--workers").and_then(|v| v.parse().ok()).unwrap_or(10);
    let mode = if arg("--mode").as_deref() == Some("evict") { Mode::Evict } else { Mode::Refresh };
    let cache_mode: u8 = arg("--cache").and_then(|v| v.parse().ok()).unwrap_or(1);
    let images: Option<&'static str> = arg("--images").map(|s| &*Box::leak(s.into_boxed_str()));
    let mut cases: Vec<CaseCfg> = Vec::new();
    if let Some(a) = arg("--altitude") {
        cases.push(CaseCfg {
            altitude_m: a.parse().unwrap(),
            radius_m: arg("--radius").and_then(|v| v.parse().ok()).unwrap_or(150.0),
            moving: flag("--moving"),
            workers,
            mode,
            preview: !flag("--no-preview"),
            cache_mode,
            size,
            images,
        });
    } else {
        for &alt in &[1.0, 200.0, 2000.0] {
            for &radius in &[50.0, 150.0, 500.0] {
                for moving in [false, true] {
                    cases.push(CaseCfg { altitude_m: alt, radius_m: radius, moving, workers, mode, preview: true, cache_mode, size, images: None });
                }
            }
        }
    }
    let mut md = String::new();
    writeln!(md, "{HEADER}").unwrap();
    for c in &cases {
        let r = run_case(&ctx, &base, *c);
        let line = row(c, &r);
        println!("{line}");
        writeln!(md, "{line}").unwrap();
    }
    if let Some(out) = arg("--out") {
        std::fs::write(out, md).unwrap();
    }
}
