//! Tier C, REND-059 and REND-061 (TEST-117, the first part): the river coverage through the lit terrain pass. A river plan over the nodes of a low flight pose draws river water where its coverage is at or under the
//! edge code and changes no other pixel; a frame without rivers is the frame of a plan with an empty river part (the recorded-pixels test of the water frames guards the rest); the layer cap and the dropped count reach the
//! frame's stats; a frame after a frame with rivers is clean again.

use std::sync::Arc;

use planet_core::cube::TangentWarp;
use planet_core::{Face, PlanetFixed, Vec3};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::river::{node_window, plan_rivers, RiverGrid, RiverNode, RiverPlan, RiverRequest, RiverSource};
use planet_frame::{plan_terrain_field, Camera, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::bake::k7::{Channel, QUnit, WidthLaw};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::{HeightField, Reference};
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer, RIVER_LAYERS};
use planet_streaming::PoolConfig;

const R: f64 = 6_371_000.0;
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 14.0;
const SIZE: (u32, u32) = (256, 160);

fn lod() -> LodParams {
    LodParams {
        cells: CELLS,
        viewport_h_px: 2.0 * f64::from(SIZE.1).max(128.0),
        tau_px: 4.0,
        max_level: FINEST,
        max_height_m: 40_000.0,
        ..LodParams::earth_1080p()
    }
}

fn pool() -> PoolConfig {
    let block_slots = 40 * 40;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn highland() -> Vec3 {
    let f = Reference(1);
    for k in 0..4000 {
        let a = f64::from(k) * 0.0137;
        let lat = 0.6 * libm::sin(a * 0.7);
        let lon = 0.5 + a;
        let d = Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat));
        if (400.0..700.0).contains(&f.height(d)) {
            return d;
        }
    }
    panic!("no highland");
}

/// A low pose over a highland point, looking along the ground and pitched down.
fn scene() -> (TerrainPlan, Reference) {
    let field = Reference(1);
    let d = highland();
    let heading = Vec3::new(0.0, 0.0, 1.0).cross(d).normalized();
    let camera =
        Camera::at_surface_point(PlanetFixed(d * (R + field.height(d) + 40.0)), heading, 25f64.to_radians(), 60f64.to_radians(), 1.6, 0.1);
    let plan = plan_terrain_field(
        &TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: "tangent-v1".into(), seed: 1 },
        &field,
    )
    .unwrap();
    (plan, field)
}

/// One channel for every tile asked: the source of a test that places the river by the nodes' own windows.
struct OneRiver {
    grid: RiverGrid,
    channel: Channel,
}

impl RiverSource for OneRiver {
    fn grid(&self) -> RiverGrid {
        self.grid
    }
    fn tile_channels(&self, _: Face, _: u32, _: u32) -> Result<Arc<Vec<Channel>>, String> {
        Ok(Arc::new(vec![self.channel.clone()]))
    }
}

/// A river through the middle of the finest node nearest to the camera, on a layer grid whose cells are that node's own (one sample a cell), 6 cells wide.
fn river_plan(plan: &TerrainPlan, max_layers: usize) -> RiverPlan {
    let finest = plan.nodes.iter().map(|n| n.level).max().unwrap();
    let cells = 1u32 << (finest + 4);
    let grid = RiverGrid { cells, tile: cells / 4, cell_m: 1.0 };
    let law = WidthLaw::STANDARD;
    let pick = plan.nodes.iter().filter(|n| n.level == finest).min_by(|a, b| {
        let d =
            |n: &planet_frame::terrain::TerrainDraw| n.camera_relative_origin.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>();
        d(a).total_cmp(&d(b))
    });
    let tile = pick.expect("a finest node").tile;
    let (origin, _) = node_window(tile, CELLS, cells);
    let q = (6.0 / law.a) * (6.0 / law.a);
    let channel = Channel {
        points: vec![(origin.0 - 120.0, origin.1 + 3.0), (origin.0 - 10.0, origin.1 + 9.0), (origin.0 + 130.0, origin.1 + 2.0)],
        levels: vec![1.0, 1.0, 1.0],
        q: vec![q, q, q],
        q_unit: QUnit::Discharge,
    };
    let src = OneRiver { grid, channel };
    let nodes: Vec<(u32, planet_core::TileId, f64)> = plan
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u32, n.tile, n.camera_relative_origin.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>().sqrt()))
        .collect();
    plan_rivers(&RiverRequest { source: &src, law: &law, res: CELLS, max_step_cells: 2.0, max_layers }, &nodes).unwrap()
}

fn renderer(ctx: &GpuContext, plan: &TerrainPlan, field: &Reference) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, pool());
    r.enable_lit(ctx);
    for n in &plan.nodes {
        if r.slot_of(n.tile).is_none() {
            r.upload_tile(ctx, &compact_tile_field(field, &TangentWarp, n.tile, CELLS, R)).unwrap();
        }
    }
    r
}

fn frame(
    ctx: &GpuContext,
    r: &mut TerrainRenderer,
    plan: &TerrainPlan,
    rivers: Option<&RiverPlan>,
) -> (Vec<[f32; 4]>, planet_render::TerrainStats) {
    let mut p = plan.clone();
    p.lit = Some(LitPlan::fixed(EV));
    p.rivers = rivers.cloned();
    let (_, stats) = r.render(ctx, &p, false).unwrap();
    (r.lit_path().unwrap().read_hdr(ctx), stats)
}

fn lum(p: &[f32; 4]) -> f32 {
    p[0] + p[1] + p[2]
}

// spec: REND-059, REND-061
#[test]
fn a_river_plan_darkens_the_pixels_of_its_water_and_changes_no_others_and_the_next_frame_is_clean() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let rp = river_plan(&plan, RIVER_LAYERS as usize);
    assert!(!rp.nodes.is_empty(), "the pose has a node with the river in range");
    let mut r = renderer(&ctx, &plan, &field);
    // The ring has more than one set: cycle it so that every set has been drawn with and without rivers.
    let (base, s0) = frame(&ctx, &mut r, &plan, None);
    assert_eq!((s0.river_nodes, s0.river_dropped), (0, 0));
    let (empty, _) = frame(&ctx, &mut r, &plan, Some(&RiverPlan::default()));
    assert_eq!(empty, base, "an empty river part draws the frame of a plan without one");
    let mut changed = 0;
    for _ in 0..3 {
        let (with, stats) = frame(&ctx, &mut r, &plan, Some(&rp));
        assert_eq!(stats.river_nodes as usize, rp.nodes.len(), "every planned river node holds a layer");
        assert_eq!(stats.river_dropped, 0);
        changed = 0;
        for (a, b) in base.iter().zip(&with) {
            if a != b {
                changed += 1;
                assert!(lum(b) < lum(a), "river water darkens the ground it covers: {a:?} -> {b:?}");
            }
        }
    }
    assert!(changed > 20, "the river draws some pixels ({changed})");
    for _ in 0..3 {
        let (after, stats) = frame(&ctx, &mut r, &plan, None);
        assert_eq!(after, base, "a frame without rivers after frames with them is the frame it was");
        assert_eq!(stats.river_nodes, 0);
    }
    println!("river pixels changed: {changed} of {}", base.len());
    // The pictures for the eye (not goldens): the frame without and with the river.
    let dir = planet_testkit::repo_root().join("target").join("review").join("river-first");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, rivers) in [("without", None), ("with", Some(&rp))] {
        frame(&ctx, &mut r, &plan, rivers);
        let f = r.read_color(&ctx);
        planet_testkit::write_png(&dir.join(format!("{name}.png")), &planet_testkit::Image::new(f.width, f.height, f.rgba)).unwrap();
    }
}

// spec: REND-061
#[test]
fn more_river_nodes_than_layers_draw_the_nearest_and_the_stats_count_the_rest() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let all = river_plan(&plan, RIVER_LAYERS as usize);
    assert!(all.nodes.len() >= 2, "the pose has at least two river nodes ({}): the cap case needs two", all.nodes.len());
    let one = river_plan(&plan, 1);
    assert_eq!(one.nodes.len(), 1);
    assert_eq!(one.dropped as usize, all.nodes.len() - 1);
    let mut r = renderer(&ctx, &plan, &field);
    let (_, stats) = frame(&ctx, &mut r, &plan, Some(&one));
    assert_eq!((stats.river_nodes, stats.river_dropped as usize), (1, all.nodes.len() - 1));
}

/// The pixel of the vertex `(i, j)` of a node's lattice (the compact tile's own: the direction of the sample on the reference field), or None behind the camera.
fn project(plan: &TerrainPlan, field: &Reference, tile: planet_core::TileId, i: u32, j: u32) -> Option<(f64, f64)> {
    let dir = tile.sample_direction(&TangentWarp, CELLS, i, j);
    let pos = dir * (R + field.height(dir));
    let cam = plan.camera.position.0;
    let rel = [pos.x - cam.x, pos.y - cam.y, pos.z - cam.z];
    let m = &plan.view_projection;
    let row = |r: usize| f64::from(m[r]) * rel[0] + f64::from(m[4 + r]) * rel[1] + f64::from(m[8 + r]) * rel[2] + f64::from(m[12 + r]);
    let (x, y, w) = (row(0), row(1), row(3));
    (w > 1e-6).then(|| ((x / w * 0.5 + 0.5) * f64::from(SIZE.0), (1.0 - (y / w * 0.5 + 0.5)) * f64::from(SIZE.1)))
}

// spec: REND-059
#[test]
fn the_coverage_lands_where_its_samples_are_along_both_axes_of_the_node() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let rp = river_plan(&plan, RIVER_LAYERS as usize);
    let node = rp.nodes[0].node;
    let tile = plan.nodes[node as usize].tile;
    let mut r = renderer(&ctx, &plan, &field);
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    let size = (CELLS + 1) as usize;
    // The strip coverage: water (code 0) on the samples with the low index along one axis, far outside elsewhere.
    for axis in 0..2usize {
        let coverage: Vec<u8> = (0..size * size).map(|k| if [k % size, k / size][axis] < 4 { 0 } else { 255 }).collect();
        let strip = RiverPlan { nodes: vec![RiverNode { node, size: size as u32, coverage }], dropped: 0 };
        let (with, _) = frame(&ctx, &mut r, &plan, Some(&strip));
        let verts: Vec<((u32, u32), (f64, f64))> = (0..=CELLS)
            .flat_map(|j| (0..=CELLS).map(move |i| (i, j)))
            .filter_map(|(i, j)| project(&plan, &field, tile, i, j).map(|p| ((i, j), p)))
            .collect();
        let mut changed = 0;
        for (k, (a, b)) in base.iter().zip(&with).enumerate() {
            if a == b {
                continue;
            }
            changed += 1;
            let (x, y) = ((k as u32 % SIZE.0) as f64 + 0.5, (k as u32 / SIZE.0) as f64 + 0.5);
            let d2 = |p: &((u32, u32), (f64, f64))| (p.1 .0 - x) * (p.1 .0 - x) + (p.1 .1 - y) * (p.1 .1 - y);
            let nearest = verts.iter().min_by(|p, q| d2(p).total_cmp(&d2(q))).unwrap();
            let index = [nearest.0 .0, nearest.0 .1][axis];
            assert!(index <= 5, "axis {axis}: the pixel ({x}, {y}) darkened where the nearest lattice vertex has index {index} along the axis (the strip is the indices 0 to 3)");
        }
        assert!(changed > 5, "axis {axis}: the strip draws some pixels ({changed})");
    }
}

// spec: REND-061
#[test]
fn a_river_plan_the_array_cannot_hold_is_an_error_and_a_later_frame_without_rivers_never_shows_a_stale_layer() {
    use planet_render::ExecError;
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let rp = river_plan(&plan, RIVER_LAYERS as usize);
    let mut r = renderer(&ctx, &plan, &field);
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    let mut p = plan.clone();
    p.lit = Some(LitPlan::fixed(EV));
    let size = (CELLS + 1) as usize;
    let node = |n: u32, side: usize| RiverNode { node: n, size: side as u32, coverage: vec![0; side * side] };
    for (bad, what) in [
        (RiverPlan { nodes: vec![node(0, size + 1)], dropped: 0 }, "a coverage of another size"),
        (RiverPlan { nodes: vec![node(plan.nodes.len() as u32, size)], dropped: 0 }, "a node the plan does not have"),
        (RiverPlan { nodes: vec![node(0, size), node(0, size)], dropped: 0 }, "a node twice"),
        (RiverPlan { nodes: (0..=RIVER_LAYERS).map(|_| node(0, size)).collect(), dropped: 0 }, "more nodes than layers"),
        (RiverPlan { nodes: vec![RiverNode { node: 0, size: size as u32, coverage: vec![0; 3] }], dropped: 0 }, "a short coverage"),
    ] {
        p.rivers = Some(bad);
        assert!(matches!(r.render(&ctx, &p, false), Err(ExecError::BadRiverPlan(_))), "{what} is a plan error, not a panic");
    }
    // Stale tails: rivers on a large plan, then a smaller plan without rivers, then the large plan again without, on every set of the ring.
    let small = {
        let mut s = plan.clone();
        s.nodes.truncate(plan.nodes.len() / 2);
        s
    };
    for _ in 0..planet_render::RING_SETS {
        frame(&ctx, &mut r, &plan, Some(&rp));
    }
    for _ in 0..planet_render::RING_SETS {
        frame(&ctx, &mut r, &small, None);
    }
    for _ in 0..planet_render::RING_SETS {
        let (after, stats) = frame(&ctx, &mut r, &plan, None);
        assert_eq!(after, base, "no river of an earlier frame is drawn");
        assert_eq!(stats.river_nodes, 0);
    }
}
