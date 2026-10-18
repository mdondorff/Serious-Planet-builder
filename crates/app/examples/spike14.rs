//! SPIKE-14 ONLY (never merged): the egui panel over the lit frame, offscreen: DPI 100 % and 150 %, cost, and the check that nothing
//! outside the panel changes.
//!
//! `cargo run --release -p planet --example spike14 -- --adapter software|hardware [--out DIR] [--frames N] [--atmosphere]`

#![allow(clippy::disallowed_methods)]

use planet::spike14::{run_pass, PanelState};
use planet_core::cube::TangentWarp;
use planet_frame::look::atmosphere::{Atmosphere, AtmosphereView};
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainRequest, TerrainView};
use planet_generators::compact::tile_data_field;
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame, TerrainRenderer, UiPainter};
use std::fmt::Write as _;

const R: f64 = 6_371_000.0;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 - 1.0) * q).round() as usize]
}

fn write_png(path: &std::path::Path, f: &RgbaFrame) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), f.width, f.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&f.rgba).unwrap();
}

fn main() {
    let policy = if arg("--adapter").as_deref() == Some("hardware") { AdapterPolicy::HighPerformance } else { AdapterPolicy::Software };
    let out_dir = std::path::PathBuf::from(arg("--out").unwrap_or_else(|| "target/spike14".into()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let frames: usize = arg("--frames").and_then(|v| v.parse().ok()).unwrap_or(300);
    let size = (1920u32, 1080u32);
    let ctx = GpuContext::new(policy).expect("adapter");
    let log = ctx.adapter_log_line();
    let tag = if log.contains("Cpu") { "warp" } else { "rtx" };
    println!("{log}");
    // The lit viewer frame (the LDR colour target the window presents).
    let cells = 32u32;
    let lod = LodParams { cells, viewport_h_px: f64::from(size.1), tau_px: 6.0, max_level: planet_frame::production_finest_level(cells), ..LodParams::earth_1080p() };
    let (_, camera) = scripted_views(R, f64::from(size.0) / f64::from(size.1), 1)[3];
    let plan = plan_terrain(&TerrainRequest { camera, size, view: TerrainView::Relief, lod: lod.clone(), face_mapping: "tangent-v1".into(), seed: 1 }).unwrap();
    let pool = compact_format_pool_with(1 << 30, cells, 4096, 1).unwrap();
    let mut renderer = TerrainRenderer::with_compact_pools(&ctx, size, cells, pool);
    renderer.enable_lit(&ctx);
    let atmosphere = flag_atmosphere();
    if atmosphere {
        renderer.enable_atmosphere(&ctx, &Atmosphere::earth(R));
    }
    renderer.warm_up(&ctx);
    for n in &plan.nodes {
        let t = tile_data_field(&Reference(1), &TangentWarp, n.tile, cells, R, lod.max_level);
        renderer.upload_tile(&ctx, &t).unwrap();
    }
    let mut lit = LitPlan::adaptive(11.0, 0.016);
    if atmosphere {
        lit = lit.with_atmosphere(&LookParams::default(), R).unwrap();
        lit.atmosphere.as_mut().unwrap().view = AtmosphereView::Composite;
    }
    let mut p = plan.clone();
    p.lit = Some(lit);
    for _ in 0..4 {
        renderer.render(&ctx, &p, false).unwrap();
    }
    let base = renderer.read_color(&ctx);
    let mut painter = UiPainter::offscreen(&ctx);

    let mut report = String::new();
    writeln!(report, "# SPIKE-14: egui panel over the lit frame ({log})").unwrap();
    writeln!(report, "Lit viewer frame {}x{} ({} nodes{}), panel: 5 sliders, checkbox, combo box, text field, scrolling list of 100 items. Times in ms, median / p90 over {frames} frames.\n", size.0, size.1, plan.nodes.len(), if atmosphere { ", atmosphere" } else { ", no atmosphere" }).unwrap();
    writeln!(report, "| DPI scale | primitives | vertices | egui run + tessellate (CPU) | UI pass record + submit + GPU wait | pixels outside the panel that differ from the blit alone |\n|---|---|---|---|---|---|").unwrap();
    let blit_only = painter.compose(&ctx, &base, size, None);
    for ppp in [1.0f32, 1.5] {
        let egui_ctx = egui::Context::default();
        let mut st = PanelState::default();
        // Every pass's texture changes must reach the painter (the font atlas arrives with the first passes): collect them.
        let mut pending = egui::TexturesDelta::default();
        let mut ui = run_pass(&egui_ctx, &mut st, size, ppp, vec![], 0.0);
        pending.append(std::mem::take(&mut ui.textures_delta));
        for k in 1..8 {
            ui = run_pass(&egui_ctx, &mut st, size, ppp, vec![], f64::from(k) * 0.016);
            println!("pass {k}: {} texture sets, {} primitives", ui.textures_delta.set.len(), ui.primitives.len());
            pending.append(std::mem::take(&mut ui.textures_delta));
        }
        for (id, ds) in &pending.set {
            for d in ds.iter() {
                println!("texture {id:?} delta: size {:?} pos {:?}", d.image.size(), d.pos);
            }
        }
        println!("free: {:?}", pending.free);
        ui.textures_delta = pending;
        // The first frame carries the font texture upload: paint it once with its deltas before the steady frames.
        let composed = painter.compose(&ctx, &base, size, Some(&ui));
        let name = format!("{tag}-lit-panel-{}pct.png", (ppp * 100.0) as u32);
        write_png(&out_dir.join(&name), &composed);
        // Nothing outside the panel (plus a margin for its shadow) differs from the blit alone.
        let panel_px = (st.panel_rect.max.x * ppp).ceil() as u32 + 24;
        let mut outside_diff = 0usize;
        for y in 0..size.1 {
            for x in panel_px.min(size.0)..size.0 {
                let i = ((y * size.0 + x) * 4) as usize;
                if composed.rgba[i..i + 4] != blit_only.rgba[i..i + 4] {
                    outside_diff += 1;
                }
            }
        }
        // CPU: the egui pass and the tessellation, steady state.
        let mut cpu = Vec::with_capacity(frames);
        for k in 0..frames {
            let t = std::time::Instant::now();
            let u = run_pass(&egui_ctx, &mut st, size, ppp, vec![], 1.0 + k as f64 * 0.016);
            cpu.push(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(&u);
        }
        let mut gpu = painter.time_paint(&ctx, size, &ui, frames);
        gpu.remove(0);
        let verts: usize = ui.primitives.iter().map(|p| if let egui::epaint::Primitive::Mesh(m) = &p.primitive { m.vertices.len() } else { 0 }).sum();
        writeln!(
            report,
            "| {:.0} % | {} | {} | {:.3} / {:.3} | {:.3} / {:.3} | {} |",
            ppp * 100.0,
            ui.primitives.len(),
            verts,
            pct(&mut cpu.clone(), 0.5),
            pct(&mut cpu, 0.9),
            pct(&mut gpu.clone(), 0.5),
            pct(&mut gpu, 0.9),
            outside_diff
        )
        .unwrap();
    }
    println!("{report}");
    std::fs::write(out_dir.join(format!("{tag}-report.md")), report).unwrap();
    write_png(&out_dir.join(format!("{tag}-lit-base.png")), &blit_only);
}

fn flag_atmosphere() -> bool {
    std::env::args().any(|a| a == "--atmosphere")
}
