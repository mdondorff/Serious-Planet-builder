//! SPIKE-13 ONLY (never merged): edit-to-visible latency cases.
//!
//! `cargo run --release -p planet --example spike13 -- --adapter hardware|software [--size WxH] [--out FILE] [--bake-res N | --store DIR]
//!   [--altitudes 1,200,2000] [--radii 50,150,500] [--still-only] [--mode evict|refresh] [--no-preview] [--workers N] [--cache 0|1|2]
//!   [--images DIR] [--settle-limit S]`
//! One rig per altitude is settled once; each (radius, still|moving) case is one edit on it, reverted before the next.

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

fn list(name: &str, default: &[f64]) -> Vec<f64> {
    arg(name).map_or_else(|| default.to_vec(), |s| s.split(',').map(|v| v.parse().unwrap()).collect())
}

fn f(v: Option<f64>) -> String {
    v.map_or("-".into(), |x| format!("{x:.1}"))
}

const HEADER: &str = "| altitude m | radius m | camera | mode | workers | resident | affected by key (levels) / by cells | find keys / cells / apply ms | plan delivered ms | first dispatch ms | preview ms | first new ms | exact ms | all ms | generation sum ms / mean wait ms | upload ms / tiles | substituted max / blank max | generation per level n x mean/max ms | settle, exact frame, frames, host load / error |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|";

fn row(cfg: &AltitudeCfg, radius: f64, moving: bool, r: &CaseResult) -> String {
    let gen: Vec<String> = r.gen.per_level.iter().map(|(l, (n, mean, max))| format!("{l}:{n}x{mean:.1}/{max:.0}")).collect();
    format!(
        "| {} | {} | {} | {:?} | {} | {} | {} ({}) / {} | {:.1} / {:.1} / {:.1} | {} | {} | {} | {} | {} | {} | {:.0} / {:.1} | {:.1} / {} | {} / {} | {} | settle {:.0} s, exact frame {:.1} ms, {} frames; {}{} |",
        cfg.altitude_m,
        radius,
        if moving { "moving" } else { "still" },
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
        r.settle_s,
        r.exact_frame_ms,
        r.frames,
        r.load_line,
        r.error.clone().map_or(String::new(), |e| format!(" ERROR {e}")),
    )
}

fn main() {
    let policy = match arg("--adapter").as_deref() {
        Some("hardware") => AdapterPolicy::HighPerformance,
        _ => AdapterPolicy::Software,
    };
    let size = arg("--size")
        .and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())))
        .unwrap_or((2560u32, 1440u32));
    let ctx = GpuContext::new(policy).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let t = std::time::Instant::now();
    let base = match arg("--store") {
        Some(dir) => {
            base_world_store(std::path::Path::new(&dir), arg("--res").and_then(|v| v.parse().ok()).unwrap_or(16384), arg("--seed").and_then(|v| v.parse().ok()).unwrap_or(1))
                .expect("store world")
        }
        None => base_world(7, arg("--bake-res").and_then(|v| v.parse().ok()).unwrap_or(1024)),
    };
    set_seed(if arg("--store").is_some() { arg("--seed").and_then(|v| v.parse().ok()).unwrap_or(1) } else { 7 });
    println!("base world built in {:.1} s", t.elapsed().as_secs_f64());
    let mode = if arg("--mode").as_deref() == Some("evict") { Mode::Evict } else { Mode::Refresh };
    let images: Option<&'static str> = arg("--images").map(|s| &*Box::leak(s.into_boxed_str()));
    let radii = list("--radii", &[50.0, 150.0, 500.0]);
    let mut md = String::new();
    writeln!(md, "{HEADER}").unwrap();
    for alt in list("--altitudes", &[1.0, 200.0, 2000.0]) {
        let mut cases = Vec::new();
        for &r in &radii {
            cases.push((r, false));
            if !flag("--still-only") {
                cases.push((r, true));
            }
        }
        let cfg = AltitudeCfg {
            altitude_m: alt,
            workers: arg("--workers").and_then(|v| v.parse().ok()).unwrap_or(10),
            size,
            mode,
            preview: !flag("--no-preview"),
            cache_mode: arg("--cache").and_then(|v| v.parse().ok()).unwrap_or(1),
            images,
            cases,
            settle_limit_s: arg("--settle-limit").and_then(|v| v.parse().ok()).unwrap_or(1800),
        };
        for (radius, moving, r) in run_altitude(&ctx, &base, &cfg) {
            let line = row(&cfg, radius, moving, &r);
            println!("{line}");
            writeln!(md, "{line}").unwrap();
            if let Some(out) = arg("--out") {
                std::fs::write(out, &md).unwrap();
            }
        }
    }
}
