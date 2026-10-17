//! Tier C, REND-016: the lit water pass and the atmosphere over compact tiles in storage-buffer blocks draw exactly the pixels the
//! atlas textures drew (ADR 0054). TRANSITIONAL: exists while both paths exist; it records the atlas path's hash.

use planet_core::cube::TangentWarp;
use planet_core::{PlanetFixed, Vec3};
use planet_frame::look::atmosphere::Atmosphere;
use planet_frame::look::file::LookParams;
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::water::{plan_water, WaterPlan, WaterRequest, WaterSource, WaterTileCache, WaterView};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::tile_data_field;
use planet_generators::global::ReadError;
use planet_generators::height::{HeightField, Reference};
use planet_generators::water::{WaterBodyId, WaterSample};
use planet_render::{set_buffer_blocks, AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_world_def::{LiquidKindId, LiquidTable};

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 14.0;
const FOV: f64 = 60.0;

fn lod_for(size: (u32, u32)) -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: 2.0 * f64::from(size.1).max(128.0),
        tau_px: 4.0,
        max_level: FINEST,
        max_height_m: 40_000.0,
        ..LodParams::earth_1080p()
    }
}

fn compact_pool() -> PoolConfig {
    let side = 40 * (CELLS + 1);
    let one = compact_format_pool_with((128 << 20) + 4 * u64::from(side) * u64::from(side) * 8, CELLS, side, 1).unwrap();
    PoolConfig { vertex_slots: 768, fallback_slots: 512, ..one }
}

/// The sea of the tests: water wherever the reference terrain is at or below the level (a test-only producer: the sea of the
/// production bake needs a bake, and the pass reads W through the same trait).
struct HeightSea<'a> {
    field: &'a dyn HeightField,
    level: f64,
}

impl WaterSource for HeightSea<'_> {
    fn water_at(&self, dir: Vec3) -> Result<Option<WaterSample>, ReadError> {
        Ok((self.field.height(dir) <= self.level).then_some(WaterSample {
            level_m: self.level,
            body: WaterBodyId::SEA,
            kind: LiquidKindId::WATER,
        }))
    }
}

/// The first coast east of 30 E on 20 N whose sea lies to the west: the direction of the coast and the heading from the sea to the land.
fn coast(field: &dyn HeightField, level: f64) -> (Vec3, Vec3) {
    let at = |s: f64| {
        let lon = 30f64.to_radians() + s / (R * libm::cos(20f64.to_radians()));
        let la = 20f64.to_radians();
        Vec3::new(libm::cos(la) * libm::cos(lon), libm::cos(la) * libm::sin(lon), libm::sin(la))
    };
    let mut s = 0.0;
    let mut prev = field.height(at(s)) - level;
    loop {
        s += 50.0;
        let cur = field.height(at(s)) - level;
        if prev < 0.0 && cur >= 0.0 {
            let (mut a, mut b) = (s - 50.0, s);
            for _ in 0..40 {
                let m = 0.5 * (a + b);
                if field.height(at(m)) - level < 0.0 {
                    a = m
                } else {
                    b = m
                }
            }
            return (at(b), (at(b + 10.0) - at(b - 10.0)).normalized());
        }
        prev = cur;
        assert!(s < 4.0e6, "no coast found");
    }
}

/// A camera `alt` m above the water `back` m out to sea from the coast, looking along the heading to the land pitched down.
#[allow(clippy::too_many_arguments)]
fn coast_camera(c: Vec3, heading: Vec3, level: f64, alt: f64, back: f64, along: f64, pitch_deg: f64, aspect: f64) -> Camera {
    let pos_dir = (c + heading * ((along - back) / R)).normalized();
    Camera::at_surface_point(PlanetFixed(pos_dir * (R + level + alt)), heading, pitch_deg.to_radians(), FOV.to_radians(), aspect, 0.1)
}

struct Scene<'a> {
    camera: Camera,
    size: (u32, u32),
    field: &'a dyn HeightField,
}

impl Scene<'_> {
    fn terrain(&self) -> TerrainPlan {
        // The plans of the lit path bound their tiles up to the sea level (`WaterBounded`): the water above a culled seabed is drawn.
        let field = planet_generators::water::WaterBounded::borrowed(self.field, 0.0);
        let field: &dyn HeightField = &field;
        plan_terrain_field(
            &TerrainRequest {
                camera: self.camera,
                size: self.size,
                view: TerrainView::Level,
                lod: lod_for(self.size),
                face_mapping: "tangent-v1".into(),
                seed: self.field.seed(),
            },
            field,
        )
        .unwrap()
    }
}

fn water_plan(plan: &TerrainPlan, source: &dyn WaterSource, table: &LiquidTable, size: (u32, u32), t: f64, view: WaterView) -> WaterPlan {
    let look = LookParams::default();
    plan_water(
        &WaterRequest { source, map: &TangentWarp, liquids: table, look: &look.water, lod: &lod_for(size), time_s: t, view },
        &mut WaterTileCache::new(),
        plan,
    )
    .unwrap()
}

fn lit_plan(atmosphere: bool) -> LitPlan {
    let mut l = LitPlan::fixed(EV);
    if atmosphere {
        l = l.with_atmosphere(&LookParams::default(), R).unwrap();
    }
    l
}

fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

/// The lit frames (water with and without the atmosphere, the class view) of the aerial coast, as the HDR target holds them.
fn frames(ctx: &GpuContext, buffer: bool) -> (Vec<Vec<[f32; 4]>>, Vec<Vec<u8>>) {
    set_buffer_blocks(buffer);
    let field = Reference(1);
    let size = (256, 160);
    let (c, heading) = coast(&field, 0.0);
    let table = LiquidTable::earth();
    let src = HeightSea { field: &field, level: 0.0 };
    let camera = coast_camera(c, heading, 0.0, 120.0, 800.0, 0.0, 12.0, 1.6);
    let plan = Scene { camera, size, field: &field }.terrain();
    let lit_water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Lit);
    let class_water = water_plan(&plan, &src, &table, size, 2.0, WaterView::Class);
    let mut r = TerrainRenderer::with_compact_pools(ctx, size, CELLS, compact_pool());
    r.enable_lit(ctx);
    for n in &plan.nodes {
        r.upload_tile(ctx, &tile_data_field(&field, &TangentWarp, n.tile, CELLS, R, FINEST)).unwrap();
    }
    r.enable_atmosphere(ctx, &Atmosphere::earth(R));
    r.enable_water(ctx);
    let (mut hdr, mut ldr) = (Vec::new(), Vec::new());
    for (water, atmosphere) in
        [(None, false), (None, true), (Some(&class_water), false), (Some(&lit_water), false), (Some(&lit_water), true)]
    {
        let mut p = plan.clone();
        p.lit = Some(if atmosphere { LitPlan::fixed(EV).with_atmosphere(&LookParams::default(), R).unwrap() } else { LitPlan::fixed(EV) });
        p.water = water.cloned();
        r.render(ctx, &p, false).unwrap();
        hdr.push(r.lit_path().unwrap().read_hdr(ctx));
        ldr.push(r.read_color(ctx).rgba);
    }
    set_buffer_blocks(false);
    (hdr, ldr)
}

// spec: REND-016, ATMO-004
#[test]
fn the_lit_water_and_atmosphere_frames_are_identical_with_blocks_in_buffers() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    println!("{}", ctx.adapter_log_line());
    let (ha, la) = frames(&ctx, false);
    let (hb, lb) = frames(&ctx, true);
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for (k, ((a, b), (x, y))) in ha.iter().zip(&hb).zip(la.iter().zip(&lb)).enumerate() {
        assert!(a.iter().zip(b).all(|(p, q)| p.map(f32::to_bits) == q.map(f32::to_bits)), "frame {k}: the HDR target differs");
        assert!(x == y, "frame {k}: the colour target differs");
        for p in a {
            for v in p {
                hash = fnv(hash, &v.to_bits().to_le_bytes());
            }
        }
        hash = fnv(hash, x);
    }
    println!("DIFFERENTIAL water identical: {} frames, atlas-path hash {hash:016x}", ha.len());
}
