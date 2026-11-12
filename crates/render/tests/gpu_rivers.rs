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
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer, RIVER_SLOTS};
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

/// A pose `alt` m over a highland point, looking along the ground and pitched down by `pitch_deg`, with the selection's `tau_px`.
fn scene_at(alt: f64, pitch_deg: f64, tau_px: f64) -> (TerrainPlan, Reference) {
    scene_capped(alt, pitch_deg, tau_px, FINEST)
}

/// [`scene_at`] with the finest level of the selection capped at `max_level`.
fn scene_capped(alt: f64, pitch_deg: f64, tau_px: f64, max_level: u8) -> (TerrainPlan, Reference) {
    let field = Reference(1);
    let d = highland();
    let heading = Vec3::new(0.0, 0.0, 1.0).cross(d).normalized();
    let camera = Camera::at_surface_point(
        PlanetFixed(d * (R + field.height(d) + alt)),
        heading,
        pitch_deg.to_radians(),
        60f64.to_radians(),
        1.6,
        0.1,
    );
    let plan = plan_terrain_field(
        &TerrainRequest {
            camera,
            size: SIZE,
            view: TerrainView::Level,
            lod: LodParams { tau_px, max_level, ..lod() },
            face_mapping: "tangent-v1".into(),
            seed: 1,
        },
        &field,
    )
    .unwrap();
    (plan, field)
}

/// The near pose seen from `k` times as far along its own line of sight: the camera backs away from the point its axis meets on the ground (so the river stays in view), the heading and the pitch kept.
fn scene_backed(alt: f64, pitch_deg: f64, tau_px: f64, k: f64) -> (TerrainPlan, Reference) {
    let field = Reference(1);
    let d = highland();
    let heading = Vec3::new(0.0, 0.0, 1.0).cross(d).normalized();
    let ground = d * (R + field.height(d));
    let target = ground + heading * (alt / libm::tan(pitch_deg.to_radians()));
    let near = d * (R + field.height(d) + alt);
    let pos = target + (near - target) * k;
    let camera = Camera::at_surface_point(PlanetFixed(pos), heading, pitch_deg.to_radians(), 60f64.to_radians(), 1.6, 0.1);
    let plan = plan_terrain_field(
        &TerrainRequest {
            camera,
            size: SIZE,
            view: TerrainView::Level,
            lod: LodParams { tau_px, ..lod() },
            face_mapping: "tangent-v1".into(),
            seed: 1,
        },
        &field,
    )
    .unwrap();
    (plan, field)
}

fn scene() -> (TerrainPlan, Reference) {
    scene_at(40.0, 25.0, 4.0)
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

/// A river through the middle of the finest node nearest to the camera of `plan`, `width_cells` wide, on a layer grid whose cells are that node's own (one sample a cell): the grid and the channel, in face cell-index space.
fn make_river(plan: &TerrainPlan, width_cells: f64) -> OneRiver {
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
    let q = (width_cells / law.a) * (width_cells / law.a);
    let channel = Channel {
        points: vec![(origin.0 - 120.0, origin.1 + 3.0), (origin.0 - 10.0, origin.1 + 9.0), (origin.0 + 130.0, origin.1 + 2.0)],
        levels: vec![1.0, 1.0, 1.0],
        q: vec![q, q, q],
        q_unit: QUnit::Discharge,
    };
    OneRiver { grid, channel }
}

/// The river plan of `plan` over `src`: nodes with a river in range whose samples are at most `max_step` cells apart.
fn plan_river_over(plan: &TerrainPlan, src: &OneRiver, max_layers: usize) -> RiverPlan {
    let law = WidthLaw::STANDARD;
    let nodes: Vec<(u32, planet_core::TileId, f64)> = plan
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u32, n.tile, n.camera_relative_origin.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>().sqrt()))
        .collect();
    plan_rivers(
        &RiverRequest {
            source: src,
            law: &law,
            res: CELLS,
            min_samples: planet_frame::river::MIN_RIVER_SAMPLES,
            full_samples: planet_frame::river::FULL_RIVER_SAMPLES,
            max_layers,
        },
        &nodes,
    )
    .unwrap()
}

fn river_plan(plan: &TerrainPlan, max_layers: usize) -> RiverPlan {
    plan_river_over(plan, &make_river(plan, 6.0), max_layers)
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
    let rp = river_plan(&plan, RIVER_SLOTS as usize);
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
    let all = river_plan(&plan, RIVER_SLOTS as usize);
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
    let rp = river_plan(&plan, RIVER_SLOTS as usize);
    let node = rp.nodes[0].node;
    let tile = plan.nodes[node as usize].tile;
    let mut r = renderer(&ctx, &plan, &field);
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    let size = (CELLS + 1) as usize;
    // The strip coverage: water (code 0) on the samples with the low index along one axis, far outside elsewhere.
    for axis in 0..2usize {
        let coverage: Vec<u8> = (0..size * size).map(|k| if [k % size, k / size][axis] < 4 { 0 } else { 255 }).collect();
        let strip = RiverPlan { nodes: vec![RiverNode { node, size: size as u32, coverage, fade: 255 }], dropped: 0 };
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
    let rp = river_plan(&plan, RIVER_SLOTS as usize);
    let mut r = renderer(&ctx, &plan, &field);
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    let mut p = plan.clone();
    p.lit = Some(LitPlan::fixed(EV));
    let size = (CELLS + 1) as usize;
    let node = |n: u32, side: usize| RiverNode { node: n, size: side as u32, coverage: vec![0; side * side], fade: 255 };
    for (bad, what) in [
        (RiverPlan { nodes: vec![node(0, size + 1)], dropped: 0 }, "a coverage of another size"),
        (RiverPlan { nodes: vec![node(plan.nodes.len() as u32, size)], dropped: 0 }, "a node the plan does not have"),
        (RiverPlan { nodes: vec![node(0, size), node(0, size)], dropped: 0 }, "a node twice"),
        (RiverPlan { nodes: (0..=RIVER_SLOTS).map(|_| node(0, size)).collect(), dropped: 0 }, "more nodes than layers"),
        (
            RiverPlan { nodes: vec![RiverNode { node: 0, size: size as u32, coverage: vec![0; 3], fade: 255 }], dropped: 0 },
            "a short coverage",
        ),
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

// spec: REND-060
#[test]
fn river_water_adds_no_geometry_the_frame_submits_the_draws_and_triangles_it_did_without_rivers() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let rp = river_plan(&plan, RIVER_SLOTS as usize);
    let mut r = renderer(&ctx, &plan, &field);
    let (_, without) = frame(&ctx, &mut r, &plan, None);
    let (_, with) = frame(&ctx, &mut r, &plan, Some(&rp));
    assert!(with.river_nodes > 0, "the frame has river nodes");
    // The river is the ground where the coverage is wet (the burned bed): no surface of its own, so the same draws, triangles, vertex bytes and node records.
    assert_eq!(
        (
            with.draw_calls,
            with.triangles,
            with.vertex_bytes,
            with.encoded_draws,
            with.node_record_bytes,
            with.tile_ref_bytes,
            with.water_draws
        ),
        (
            without.draw_calls,
            without.triangles,
            without.vertex_bytes,
            without.encoded_draws,
            without.node_record_bytes,
            without.tile_ref_bytes,
            without.water_draws
        )
    );
}

/// The wetness of every pixel: how much the river darkened it, from 0 (the frame without) to about 1 (the full tint), from the HDR luminance of the frame with and without the river.
fn wetness(base: &[[f32; 4]], with: &[[f32; 4]]) -> Vec<f32> {
    let d: Vec<f32> = base.iter().zip(with).map(|(a, b)| (lum(a) - lum(b)).max(0.0) / lum(a).max(1e-6)).collect();
    let top = d.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    d.iter().map(|v| v / top).collect()
}

/// The sub-pixel positions where the wetness crosses a half along each row (`axis` 0) or column (`axis` 1): per line the first and the last crossing, which are the banks of a river that crosses the line.
fn bank_positions(w: &[f32], axis: usize) -> Vec<(usize, f64, f64)> {
    let (n_lines, len) = if axis == 0 { (SIZE.1 as usize, SIZE.0 as usize) } else { (SIZE.0 as usize, SIZE.1 as usize) };
    let at = |line: usize, k: usize| if axis == 0 { w[line * SIZE.0 as usize + k] } else { w[k * SIZE.0 as usize + line] };
    let mut out = Vec::new();
    for line in 0..n_lines {
        let mut crossings = Vec::new();
        for k in 0..len - 1 {
            let (a, b) = (at(line, k), at(line, k + 1));
            if (a - 0.5) * (b - 0.5) < 0.0 {
                crossings.push(k as f64 + f64::from((0.5 - a) / (b - a)));
            }
        }
        if crossings.len() >= 2 {
            out.push((line, crossings[0], crossings[crossings.len() - 1]));
        }
    }
    out
}

/// The largest second difference of a bank's position along consecutive lines: a cell staircase moves the bank by whole texels (several pixels) at a step; a smooth bank moves it by a fraction of a pixel.
fn staircase_px(banks: &[(usize, f64, f64)]) -> f64 {
    let mut worst = 0.0f64;
    for w in banks.windows(3) {
        if w[1].0 == w[0].0 + 1 && w[2].0 == w[1].0 + 1 {
            worst = worst.max((w[0].1 - 2.0 * w[1].1 + w[2].1).abs()).max((w[0].2 - 2.0 * w[1].2 + w[2].2).abs());
        }
    }
    worst
}

// spec: TEST-117, REND-059
#[test]
fn the_river_scene_has_smooth_banks_at_the_near_pose_and_no_pop_between_two_levels_of_detail() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = planet_testkit::GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    // The near pose: low, pitched down, a river 10 cells wide through the finest node nearest to the camera.
    let (plan, field) = scene_at(25.0, 40.0, 4.0);
    let src = make_river(&plan, 10.0);
    let rp = plan_river_over(&plan, &src, RIVER_SLOTS as usize);
    assert!(!rp.nodes.is_empty(), "the near pose has river nodes");
    let mut r = renderer(&ctx, &plan, &field);
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    // The picture without the river, for the eye next to the golden candidate (not a golden).
    let dir = planet_testkit::repo_root().join("target").join("review").join("river-near");
    std::fs::create_dir_all(&dir).unwrap();
    let without_img = {
        let f = r.read_color(&ctx);
        planet_testkit::Image::new(f.width, f.height, f.rgba)
    };
    planet_testkit::write_png(&dir.join("without.png"), &without_img).unwrap();
    let (with, stats) = frame(&ctx, &mut r, &plan, Some(&rp));
    let near_img = {
        let f = r.read_color(&ctx);
        planet_testkit::Image::new(f.width, f.height, f.rgba)
    };
    let w = wetness(&base, &with);
    let wet_px = w.iter().filter(|v| **v > 0.5).count();
    assert!(wet_px > 200, "the river shows at the near pose ({wet_px} wet pixels, {} river nodes)", stats.river_nodes);
    // No staircase bank: along the rows (or the columns, whichever the river crosses more of) the bank moves by a fraction of a pixel from line to line.
    let (rows, cols) = (bank_positions(&w, 0), bank_positions(&w, 1));
    let banks = if rows.len() >= cols.len() { rows } else { cols };
    assert!(banks.len() >= 8, "the river crosses enough lines to judge its banks ({})", banks.len());
    let stair = staircase_px(&banks);
    println!("near pose: {wet_px} wet pixels, {} bank lines, largest bank second difference {stair:.3} px", banks.len());
    assert!(stair <= 1.5, "the bank has a staircase: its position changes by {stair} px from one line to the next beyond a straight line");
    goldens.check("river_lit_near", &near_img, planet_testkit::Tolerance::Exact);
    // Two levels of detail: the same camera and the same river over a plan whose finest level is one coarser (the nearest tiles are the parents of the near pose's): the river is drawn from coverages of half the resolution.
    let finest = plan.nodes.iter().map(|n| n.level).max().unwrap();
    let (coarse_plan, _) = scene_capped(25.0, 40.0, 4.0, finest - 1);
    let coarse_nodes = coarse_plan.nodes.len();
    assert_eq!(coarse_plan.nodes.iter().map(|n| n.level).max().unwrap(), finest - 1);
    let rp_coarse = plan_river_over(&coarse_plan, &src, RIVER_SLOTS as usize);
    let mut rc = renderer(&ctx, &coarse_plan, &field);
    let (base_c, _) = frame(&ctx, &mut rc, &coarse_plan, None);
    let (with_c, _) = frame(&ctx, &mut rc, &coarse_plan, Some(&rp_coarse));
    let wc = wetness(&base_c, &with_c);
    let wet_c = wc.iter().filter(|v| **v > 0.5).count();
    let differ = w.iter().zip(&wc).filter(|(a, b)| (**a > 0.5) != (**b > 0.5)).count();
    let share = differ as f64 / wet_px.max(1) as f64;
    println!("two levels: {wet_px} wet pixels (finest level {finest}, {} nodes) and {wet_c} (finest level {}, {coarse_nodes} nodes); {differ} pixels change wetness: {:.1} % of the wet pixels", plan.nodes.len(), finest - 1, share * 100.0);
    assert!(wet_c > 100, "the coarser plan still draws the river ({wet_c})");
    assert!(share <= 0.1, "the river pops between two levels of detail: {:.1} % of its pixels change wetness", share * 100.0);
}

// spec: TEST-117, REND-059
#[test]
fn flying_out_a_river_fades_with_its_width_in_samples_and_its_wet_pixels_fall() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (near, field) = scene_at(25.0, 40.0, 4.0);
    let src = make_river(&near, 10.0);
    let mut series: Vec<(f64, u8, usize, u8, f64)> = Vec::new();
    for k in 0..11 {
        let scale = (0..k).fold(1.0, |a, _| a * 1.4);
        let alt = 25.0 * scale;
        let (plan, _) = scene_backed(25.0, 40.0, 4.0, scale);
        let rp = plan_river_over(&plan, &src, RIVER_SLOTS as usize);
        let mut r = renderer(&ctx, &plan, &field);
        let (base, _) = frame(&ctx, &mut r, &plan, None);
        let (with, _) = frame(&ctx, &mut r, &plan, Some(&rp));
        let mass: f64 = wetness_mass(&base, &with);
        let finest = plan.nodes.iter().map(|n| n.level).max().unwrap();
        let fade = rp.nodes.iter().map(|n| n.fade).max().unwrap_or(0);
        let (parts, gaps) = connected_parts(&wetness(&base, &with));
        println!("flight-out: altitude {alt:8.1} m: {parts} connected part(s) with one pixel of gap allowed, {gaps} without");
        if mass > 20.0 {
            assert_eq!(
                parts, 1,
                "the river is broken into {parts} parts at {alt} m (wet mass {mass:.1}, {gaps} parts without the one pixel of gap)"
            );
        }
        series.push((alt, finest, rp.nodes.len(), fade, mass));
    }
    for (alt, finest, nodes, fade, mass) in &series {
        println!(
            "flight-out: altitude {alt:8.1} m, finest level {finest}, {nodes:3} river nodes, strongest fade {fade:3}, wet mass {mass:9.1}"
        );
    }
    // The river leaves the picture as the camera rises: its wet mass falls (a tolerance of 5 % for the pixel grid), the plan's strongest fade never grows, and the river is seen at the first pose and gone at the last.
    for w in series.windows(2) {
        assert!(w[1].4 <= w[0].4 * 1.05 + 1.0, "the wet mass grows from {} to {} px between {} m and {} m", w[0].4, w[1].4, w[0].0, w[1].0);
        assert!(
            w[1].3 <= w[0].3 || w[0].2 == 0,
            "the strongest fade grows from {} to {} between {} m and {} m",
            w[0].3,
            w[1].3,
            w[0].0,
            w[1].0
        );
    }
    assert!(series[0].4 > 200.0 && series[series.len() - 1].4 == 0.0);
    // Normalised by the perspective (the pixels of a river fall with the square of the altitude): while the plan's strongest fade is full no step lowers it by more than half, and inside the fade it falls (architect, 2026-11-12).
    let norm: Vec<f64> = series.iter().map(|s| s.4 * s.0 * s.0).collect();
    println!("flight-out: wet mass times altitude squared: {}", norm.iter().map(|v| format!("{v:.2e}")).collect::<Vec<_>>().join(" "));
    for (w, n) in series.windows(2).zip(norm.windows(2)) {
        // While the river is resolved (at least 50 wet pixels at the nearer pose: a river under about two pixels wide moves by whole pixels from pose to pose, 188 m to 263 m measures a fall of 56 % on the pixel grid alone).
        if w[0].3 == 255 && w[1].3 == 255 && w[0].4 >= 50.0 {
            assert!(
                n[1] >= 0.5 * n[0],
                "at full strength the normalised mass falls from {:.3e} to {:.3e} between {} m and {} m",
                n[0],
                n[1],
                w[0].0,
                w[1].0
            );
        }
        if w[0].3 < 255 && w[0].3 > 0 && w[1].3 > 0 {
            assert!(
                n[1] <= n[0] * 1.05 + 1.0,
                "inside the fade the normalised mass grows from {:.3e} to {:.3e} between {} m and {} m",
                n[0],
                n[1],
                w[0].0,
                w[1].0
            );
        }
    }
}

/// The 8-connected parts of the wet pixels (wetness over a half): with a gap of one pixel allowed (the mask grown by one pixel) and without. A river that is one line has one part.
fn connected_parts(w: &[f32]) -> (usize, usize) {
    let (width, height) = (SIZE.0 as usize, SIZE.1 as usize);
    let wet: Vec<bool> = w.iter().map(|v| *v > 0.5).collect();
    let grown: Vec<bool> = (0..width * height)
        .map(|k| {
            let (x, y) = ((k % width) as i64, (k / width) as i64);
            (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (px, py) = (x + dx, y + dy);
                    px >= 0 && py >= 0 && (px as usize) < width && (py as usize) < height && wet[py as usize * width + px as usize]
                })
            })
        })
        .collect();
    let count = |mask: &[bool]| {
        let mut seen = vec![false; mask.len()];
        let mut parts = 0;
        for start in 0..mask.len() {
            if !mask[start] || seen[start] {
                continue;
            }
            parts += 1;
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(k) = stack.pop() {
                let (x, y) = ((k % width) as i64, (k / width) as i64);
                for dy in -1..=1i64 {
                    for dx in -1..=1i64 {
                        let (px, py) = (x + dx, y + dy);
                        if px >= 0 && py >= 0 && (px as usize) < width && (py as usize) < height {
                            let n = py as usize * width + px as usize;
                            if mask[n] && !seen[n] {
                                seen[n] = true;
                                stack.push(n);
                            }
                        }
                    }
                }
            }
        }
        parts
    };
    (count(&grown), count(&wet))
}

/// The river's wet mass of a frame: the sum over its pixels of how much the river darkened them, in pixels (a pixel half darkened counts a half).
fn wetness_mass(base: &[[f32; 4]], with: &[[f32; 4]]) -> f64 {
    let w = wetness(base, with);
    w.iter().map(|v| f64::from(*v)).sum()
}

// spec: REND-059
#[test]
fn the_strength_of_a_node_scales_its_river_in_the_shader_and_a_river_narrower_than_the_coarse_level_is_reported_across_levels() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let full = river_plan(&plan, RIVER_SLOTS as usize);
    let half = RiverPlan { nodes: full.nodes.iter().map(|n| RiverNode { fade: 128, ..n.clone() }).collect(), dropped: 0 };
    let mut r = renderer(&ctx, &plan, &field);
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    let (with_full, _) = frame(&ctx, &mut r, &plan, Some(&full));
    let (with_half, _) = frame(&ctx, &mut r, &plan, Some(&half));
    let (m_full, m_half) = (darkening(&base, &with_full), darkening(&base, &with_half));
    let ratio = m_half / m_full;
    println!("strength 128 against 255: darkening {m_half:.1} against {m_full:.1}: ratio {ratio:.3}");
    assert!((ratio - 128.0 / 255.0).abs() < 0.03, "half strength darkens {ratio:.3} of full strength");
    // Across two levels with a river narrower than the coarse level's step (a width of 1.2 times a coarse sample, 0.6 of it at the fine level's): reported, not gated (the fade is per node and is not morphed).
    let finest = plan.nodes.iter().map(|n| n.level).max().unwrap();
    let (coarse_plan, _) = scene_capped(25.0, 40.0, 4.0, finest - 1);
    let src = make_river(&plan, 2.4);
    let (fine, coarse) = (plan_river_over(&plan, &src, RIVER_SLOTS as usize), plan_river_over(&coarse_plan, &src, RIVER_SLOTS as usize));
    let mut rc = renderer(&ctx, &coarse_plan, &field);
    let (base_c, _) = frame(&ctx, &mut rc, &coarse_plan, None);
    let (with_c, _) = frame(&ctx, &mut rc, &coarse_plan, Some(&coarse));
    let (base_f, _) = frame(&ctx, &mut r, &plan, None);
    let (with_f, _) = frame(&ctx, &mut r, &plan, Some(&fine));
    let (df, dc) = (darkening(&base_f, &with_f), darkening(&base_c, &with_c));
    println!(
        "level pop of a thin river (2.4 cells): fine level fade {:?}, coarse level fade {:?}; darkening {df:.1} (fine) against {dc:.1} (coarse): ratio {:.2}",
        fine.nodes.iter().map(|n| n.fade).max(),
        coarse.nodes.iter().map(|n| n.fade).max(),
        dc / df.max(1e-6)
    );
}

/// How much the river darkened a frame: the sum over its pixels of the relative luminance lost.
fn darkening(base: &[[f32; 4]], with: &[[f32; 4]]) -> f64 {
    base.iter().zip(with).map(|(a, b)| f64::from((lum(a) - lum(b)).max(0.0) / lum(a).max(1e-6))).sum()
}

// spec: REND-061
/// A node's slot is its place in the plan's river list: a river in a late slot (past the first row and column of the atlas, where the slot's origin has both a column and a row) draws the frame of the same river in slot
/// zero. A swapped column count or row stride of the shader or the upload would pass every other test, which only ever fill the first row.
#[test]
fn a_river_in_a_late_slot_of_the_atlas_draws_the_frame_of_the_same_river_in_slot_zero() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let (plan, field) = scene();
    let rp = river_plan(&plan, RIVER_SLOTS as usize);
    assert!(!rp.nodes.is_empty());
    let size = CELLS + 1;
    let wanted = 60usize;
    let taken: std::collections::HashSet<u32> = rp.nodes.iter().map(|n| n.node).collect();
    let dry: Vec<planet_frame::river::RiverNode> = (0..plan.nodes.len() as u32)
        .filter(|i| !taken.contains(i))
        .map(|node| planet_frame::river::RiverNode { node, size, coverage: vec![255; (size * size) as usize], fade: 255 })
        .collect();
    // Slots 0.. are dry nodes (far from any river: no water), the real river nodes follow: the first lands past slot 46, the atlas's first row.
    let need = wanted.min(dry.len());
    assert!(
        need >= 47,
        "the pose plans {} nodes: not enough to fill the first row of the atlas ({} dry ones)",
        plan.nodes.len(),
        dry.len()
    );
    let mut late = RiverPlan { nodes: dry.into_iter().take(need).collect(), dropped: 0 };
    late.nodes.extend(rp.nodes.iter().cloned());
    let mut r = renderer(&ctx, &plan, &field);
    let (first, s1) = frame(&ctx, &mut r, &plan, Some(&rp));
    let (second, s2) = frame(&ctx, &mut r, &plan, Some(&late));
    assert_eq!((s1.river_dropped, s2.river_dropped), (0, 0));
    assert!(s2.river_nodes as usize >= need + rp.nodes.len());
    let (base, _) = frame(&ctx, &mut r, &plan, None);
    assert!(first.iter().zip(&base).any(|(a, b)| a != b), "the river draws");
    assert_eq!(second, first, "the same river in slot {need} draws the frame of slot 0");
}
