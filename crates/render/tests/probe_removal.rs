//! Tier C: the lit path with the atmosphere through the terrain renderer (ADR 0040, ADR 0042 points 1-3 and 5; ATMO-002, ATMO-011):
//! sky and aerial perspective over the terrain on both tile formats, the frame targets equal to the compiled graph's count, nothing
//! created after the atmosphere is enabled, the lit path without an atmosphere unchanged, goldens of the sky at a fixed exposure.

use planet_core::cube::TangentWarp;
use planet_frame::look::atmosphere::{Atmosphere, AtmosphereView};
use planet_frame::look::file::LookParams;
use planet_frame::look::{self, LitPlan};
use planet_frame::pool::{compact_format_pool_with, vertex_slot_bytes};
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::{tile_data_field, vertex_tile_field};
use planet_generators::height::Reference;
use planet_generators::mesh::grid_indices;
use planet_render::graph::{frame_target_share_bytes, lit_frame_graph, LitShape};
use planet_render::{AdapterPolicy, ExecError, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::{GoldenChecks, Image, Tolerance};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (128, 96);
const CELLS: u32 = 16;
const FINEST: u8 = 26;
/// The pre-exposure and fixed exposure of the lit goldens: a sunny day, EV100 15.
const EV: f32 = 15.0;

fn plan(index: usize) -> TerrainPlan {
    let (_, camera) = scripted_views(R, 4.0 / 3.0, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: 96.0, tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 }).unwrap()
}

fn lit(mut p: TerrainPlan, l: LitPlan) -> TerrainPlan {
    p.lit = Some(l);
    p
}

fn vertex_pool() -> PoolConfig {
    let slot = vertex_slot_bytes(CELLS);
    PoolConfig {
        slot_bytes: slot,
        block_slots: 512,
        main_blocks: 4,
        vertex_slot_bytes: slot,
        vertex_slots: 64,
        index_bytes: grid_indices(CELLS).len() as u64 * 4,
        node_record_bytes: 4096 * 512,
        frames_in_flight: 1,
        block_alloc_bytes: 0,
        slot_record_bytes: 0,
        fallback_slots: 0,
    }
}

fn compact_pool() -> PoolConfig {
    let side: u32 = std::env::var("PROBE_ATLAS").map(|v| v.parse().unwrap()).unwrap_or(24 * (CELLS + 1));
    let share: u64 = std::env::var("PROBE_SHARE_MB").map(|v| v.parse::<u64>().unwrap() << 20).unwrap_or((128 << 20) + 4 * u64::from(side) * u64::from(side) * 8);
    let one = compact_format_pool_with(share, CELLS, side, 1).unwrap();
    if std::env::var("PROBE_ATLAS").is_ok() {
        return one;
    }
    PoolConfig { vertex_slots: 256, fallback_slots: 192, ..one }
}

/// A renderer in `compact` or vertex format holding the tiles of `plans`.
fn renderer(ctx: &GpuContext, compact: bool, plans: &[&TerrainPlan], enable_lit: bool) -> TerrainRenderer {
    let mut r = if compact {
        TerrainRenderer::with_compact_pools(ctx, SIZE, CELLS, compact_pool())
    } else {
        TerrainRenderer::with_pools(ctx, SIZE, CELLS, vertex_pool())
    };
    if enable_lit {
        r.enable_lit(ctx);
    }
    let field = Reference(1);
    for p in plans {
        for n in &p.nodes {
            if r.slot_of(n.tile).is_some() {
                continue;
            }
            if compact {
                r.upload_tile(ctx, &tile_data_field(&field, &TangentWarp, n.tile, CELLS, R, FINEST)).unwrap();
            } else {
                r.upload(ctx, &vertex_tile_field(&field, &TangentWarp, n.tile, CELLS, R, FINEST)).unwrap();
            }
        }
    }
    r
}

fn image(r: &TerrainRenderer, ctx: &GpuContext) -> Image {
    let f = r.read_color(ctx);
    Image::new(f.width, f.height, f.rgba)
}

fn rgb64(p: &[f32; 4]) -> [f64; 3] {
    [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]
}

fn atmosphere_plan(view: AtmosphereView) -> LitPlan {
    let mut l = LitPlan::fixed(EV).with_atmosphere(&LookParams::default(), R).unwrap();
    l.atmosphere.as_mut().unwrap().view = view;
    l
}

fn renderer_atmosphere(ctx: &GpuContext, compact: bool, plans: &[&TerrainPlan]) -> TerrainRenderer {
    let mut r = renderer(ctx, compact, plans, true);
    r.enable_atmosphere(ctx, &Atmosphere::earth(R));
    r
}


fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Exploration (WARP device removal with compact + atmosphere): a renderer-level loop with switches.
/// PROBE_COMPACT=1|0, PROBE_ATMO=1|0, PROBE_READ=exposure|hdr|color|none, PROBE_FRAMES=n, PROBE_ADAPTIVE=1|0
#[test]
fn probe_device_removal() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let compact = env("PROBE_COMPACT", "1") == "1";
    let atmo = env("PROBE_ATMO", "1") == "1";
    let adaptive = env("PROBE_ADAPTIVE", "1") == "1";
    let read = env("PROBE_READ", "exposure");
    let frames: usize = env("PROBE_FRAMES", "30").parse().unwrap();
    let base = plan(env("PROBE_VIEW", "3").parse().unwrap());
    let viewer_plan = env("PROBE_PLAN", "viewer") == "viewer";
    let (_, cam) = scripted_views(R, 4.0 / 3.0, 1)[3];
    let mut r = if atmo { renderer_atmosphere(&ctx, compact, &[&base]) } else { renderer(&ctx, compact, &[&base], true) };
    if env("PROBE_WARM", "1") == "1" && atmo {
        let warm = planet_frame::viewer::viewer_lit_plan(&LookParams::default(), R, 0.0, true, true).unwrap();
        r.warm_up_lit(&ctx, &planet_frame::viewer::lit_warm_plan(cam, SIZE, CELLS, warm)).unwrap();
        println!("warmed");
    }
    for frame in 0..frames {
        let mut l = if adaptive { LitPlan::adaptive(11.0, 0.016) } else { LitPlan::fixed(EV) };
        if atmo {
            l = l.with_atmosphere(&LookParams::default(), R).unwrap();
            l.atmosphere.as_mut().unwrap().view = AtmosphereView::Composite;
        }
        let p = if viewer_plan && atmo {
            lit(base.clone(), planet_frame::viewer::viewer_lit_plan(&LookParams::default(), R, 0.016, frame == 0, true).unwrap())
        } else {
            lit(base.clone(), l)
        };
        match env("PROBE_MODE", "render").as_str() {
            "timed" => {
                r.render(&ctx, &p, true).unwrap();
            }
            "submit" => {
                r.submit(&ctx, &p).unwrap();
            }
            _ => {
                r.render(&ctx, &p, false).unwrap();
            }
        }
        match read.as_str() {
            "exposure" => {
                let s = r.lit_path().unwrap().read_exposure(&ctx);
                if frame == 0 || frame + 1 == frames {
                    println!("frame {frame}: ev {}", s.ev100);
                }
            }
            "hdr" => {
                let _ = r.lit_path().unwrap().read_hdr(&ctx);
            }
            "color" => {
                let _ = r.read_color(&ctx);
            }
            _ => {}
        }
    }
    println!("completed {frames} frames: compact {compact} atmosphere {atmo} adaptive {adaptive} read {read}");
}
