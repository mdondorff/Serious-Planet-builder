//! REND-039: the change of a landing chunk fades in on the vertex stage: a tile with a change plane is drawn at `h_q - (1 - w) x change`. Weight 1
//! is the final height and the old arithmetic bit for bit, weight 0 the base's height, and between them the blend; the compact renderer agrees with
//! the reference renderer drawing a product mesh of the blended field.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, TileId, Vec3};
use planet_frame::camera::Camera;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{TerrainDraw, TerrainPlan, TerrainView};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{compute_chunk, halo_rings, ChunkField, ChunkLattice, StandIn};
use planet_generators::compact::{compact_tile_field, CompactTile};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::tile_mesh_field;
use planet_render::reference::execute_reference;
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (192, 144);
const CELLS: u32 = 32;
const TOP: u8 = 11;

/// The base field plus `w` of the chunk's change at the tile's level: what the faded tile draws at weight `w`.
struct Blend<'a> {
    base: Reference,
    chunk: &'a ChunkField<Reference>,
    level: u8,
    w: f64,
}

impl HeightField for Blend<'_> {
    fn height(&self, d: Vec3) -> f64 {
        self.base.height(d) + self.w * self.chunk.change_at_level(d, self.level)
    }
    fn seed(&self) -> u64 {
        self.base.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "test-blend"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.base.max_abs_height_m() + 100.0
    }
}

fn differing(a: &RgbaFrame, b: &RgbaFrame, limit: u8) -> usize {
    a.rgba.chunks(4).zip(b.rgba.chunks(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > limit)).count()
}

struct Scene {
    id: TileId,
    final_tile: CompactTile,
    bare: CompactTile,
    plan: TerrainPlan,
    chunk: ChunkField<Reference>,
}

fn scene() -> Scene {
    let lat = ChunkLattice::new(608).unwrap();
    let stage = StandIn { seed: 7, radius_m: R, amplitude_m: 30.0, steps: 8 };
    let chunk_tile = TileId::new(Face(2), CHUNK_GRID_LEVEL, 1000, 1000).unwrap();
    let result = Arc::new(compute_chunk(&lat, &stage, R, TOP, chunk_tile, halo_rings(&stage)).unwrap());
    let chunk = ChunkField::new(Reference(3), lat, R, &stage).with_top_level(TOP).with_ready(result);
    let id = TileId::new(Face(2), 15, 16 * 1000 + 5, 16 * 1000 + 7).unwrap();
    let final_tile = compact_tile_field(&chunk, &TangentWarp, id, CELLS, R);
    assert!(final_tile.change.is_some(), "the tile reads a change");
    let bare = CompactTile { change: None, ..final_tile.clone() };
    let centre = id.center_direction(&TangentWarp);
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let camera = Camera::looking_at_centre(
        PlanetFixed(centre * (R + f64::from(final_tile.h_origin) + 450.0)),
        Vec3::new(0.0, 0.0, 1.0),
        0.9,
        aspect,
        1.0,
    );
    let rel = final_tile.origin.0 - camera.position.0;
    let h = f64::from(final_tile.h_origin);
    let node = TerrainDraw {
        tile: id,
        level: id.level(),
        origin: final_tile.origin,
        camera_relative_origin: [rel.x as f32, rel.y as f32, rel.z as f32],
        morph: 0.0,
        morph_start_m: 1.0e30,
        morph_end_m: 1.0e30,
        color: [200, 60, 60, 255],
        mesh_key: final_tile.key,
        preview: false,
        transient_only: false,
        capped: false,
        stitch: 0,
        fine: 0,
        quadrants: 0b1111,
        state: planet_frame::TileState::default(),
        chunk_weight: 1.0,
    };
    let plan = TerrainPlan {
        size: SIZE,
        view: TerrainView::Height,
        camera,
        view_projection: camera.view_projection(),
        clear_color: [10, 10, 30, 255],
        height_range: [(h - 45.0) as f32, (h + 45.0) as f32],
        cells: CELLS,
        shore_ramp_per_m: 0.0,
        nodes: vec![node],
        lit: None,
        water: None,
    };
    Scene { id, final_tile, bare, plan, chunk }
}

fn with_weight(plan: &TerrainPlan, w: f32) -> TerrainPlan {
    let mut p = plan.clone();
    p.nodes[0].chunk_weight = w;
    p
}

fn compact_image(ctx: &GpuContext, tile: &CompactTile, plan: &TerrainPlan) -> (RgbaFrame, planet_render::TerrainStats) {
    let block_slots = 8 * 8;
    let pool = compact_format_pool_with(
        (128 << 20) + 40 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, pool);
    r.upload_tile(ctx, tile).unwrap();
    let (_, stats) = r.render(ctx, plan, false).unwrap();
    (r.read_color(ctx), stats)
}

// spec: REND-039
#[test]
fn a_tile_with_a_change_plane_is_drawn_at_the_base_at_weight_zero_final_at_one_and_the_old_arithmetic_without_a_plane() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let s = scene();
    let (at_one, stats_one) = compact_image(&ctx, &s.final_tile, &with_weight(&s.plan, 1.0));
    let (bare, stats_bare) = compact_image(&ctx, &s.bare, &with_weight(&s.plan, 1.0));
    assert_eq!(at_one.rgba, bare.rgba, "weight 1 draws the old arithmetic bit for bit, with a plane or without");
    assert_eq!((stats_one.faded_tiles, stats_bare.faded_tiles), (0, 0), "a settled node sends no plane");
    // Without a plane the weight changes nothing (the bare tile has no change to take off).
    let (bare_zero, _) = compact_image(&ctx, &s.bare, &with_weight(&s.plan, 0.0));
    assert_eq!(bare_zero.rgba, bare.rgba, "no plane: any weight draws the final height");
    let mut worst = 0.0f64;
    let mut fades = Vec::new();
    for w in [0.0f32, 0.5, 1.0] {
        let plan = with_weight(&s.plan, w);
        let (image, stats) = compact_image(&ctx, &s.final_tile, &plan);
        assert_eq!(stats.faded_tiles, u32::from(w < 1.0), "weight {w}");
        let blend = Blend { base: Reference(3), chunk: &s.chunk, level: s.id.level(), w: f64::from(w) };
        let mesh = tile_mesh_field(&blend, &TangentWarp, s.id, CELLS, R);
        // The reference draws a mesh of its own field, with its own key: the plan it executes names that key.
        let mut ref_plan = plan.clone();
        ref_plan.nodes[0].mesh_key = mesh.key;
        // Its vertices are relative to its own origin (the blended centre), not the final tile's.
        let rel = mesh.origin.0 - plan.camera.position.0;
        ref_plan.nodes[0].origin = mesh.origin;
        ref_plan.nodes[0].camera_relative_origin = [rel.x as f32, rel.y as f32, rel.z as f32];
        let reference = execute_reference(&ctx, &ref_plan, &[&mesh]).unwrap();
        let wrong = differing(&reference, &image, 12) as f64 / (SIZE.0 * SIZE.1) as f64;
        println!("weight {w}: compact against the reference of the blended field: {wrong:.5} of the pixels differ");
        if std::env::var_os("CHUNK_FADE_DUMP").is_some() {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review/chunk-fade");
            std::fs::create_dir_all(&dir).unwrap();
            let ppm = |f: &RgbaFrame| {
                let mut v = format!(
                    "P6
{} {}
255
",
                    SIZE.0, SIZE.1
                )
                .into_bytes();
                v.extend(f.rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]));
                v
            };
            let diff: Vec<u8> = reference
                .rgba
                .chunks(4)
                .zip(image.rgba.chunks(4))
                .flat_map(|(a, b)| {
                    let d = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
                    [d.saturating_mul(8), d.saturating_mul(8), d.saturating_mul(8), 255]
                })
                .collect();
            std::fs::write(dir.join(format!("w{w}-compact.ppm")), ppm(&image)).unwrap();
            std::fs::write(dir.join(format!("w{w}-reference.ppm")), ppm(&reference)).unwrap();
            std::fs::write(dir.join(format!("w{w}-diff.ppm")), ppm(&RgbaFrame { rgba: diff, ..image.clone() })).unwrap();
        }
        worst = worst.max(wrong);
        fades.push(image);
    }
    assert!(worst <= 0.004, "the faded tile agrees with the reference renderer: {worst}");
    let moved = differing(&fades[0], &fades[2], 12) as f64 / (SIZE.0 * SIZE.1) as f64;
    println!("weight 0 against weight 1: {moved:.4} of the pixels differ");
    assert!(moved > 0.02, "the fade changes the picture ({moved}): the change is tens of metres on a tile of 300 m");
    assert_eq!(fades[2].rgba, at_one.rgba, "the weight-1 frame of the loop is the same frame");
}
