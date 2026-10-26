//! Tier C: the horizon shadows' plumbing (REND-038 part b). A slot written through `set_horizon` reads back as written; a lit frame with a
//! horizon set whose table holds no map (slot -1 everywhere), with the array allocated, equals the frame of a renderer that never heard of it.

use planet_core::cube::TangentWarp;
use planet_frame::horizonview::{HorizonFrame, HorizonNode, SlotUpdate, SLOT_BYTES};
use planet_frame::look::LitPlan;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::{plan_terrain, scripted_views, LodParams, TerrainPlan, TerrainRequest, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};
use planet_streaming::PoolConfig;
use planet_testkit::Image;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (160, 120);
const CELLS: u32 = 16;
const FINEST: u8 = 26;
const EV: f32 = 15.0;

fn plan(index: usize) -> TerrainPlan {
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let (_, camera) = scripted_views(R, aspect, 1)[index];
    let lod = LodParams { cells: CELLS, viewport_h_px: f64::from(SIZE.1), tau_px: 2.5, max_level: FINEST, ..LodParams::earth_1080p() };
    let mut p =
        plan_terrain(&TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod, face_mapping: "tangent-v1".into(), seed: 1 })
            .unwrap();
    p.lit = Some(LitPlan::fixed(EV));
    p
}

fn compact_pool() -> PoolConfig {
    let block_slots = 48 * 48;
    compact_format_pool_with(
        (128 << 20) + 4 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap()
}

fn renderer(ctx: &GpuContext, p: &TerrainPlan) -> TerrainRenderer {
    let mut r = TerrainRenderer::with_pools(ctx, p.size, CELLS, compact_pool());
    r.enable_lit(ctx);
    let field = Reference(1);
    for n in &p.nodes {
        if r.slot_of(n.tile).is_none() {
            r.upload_tile(ctx, &compact_tile_field(&field, &TangentWarp, n.tile, CELLS, R)).unwrap();
        }
    }
    r
}

fn frame(r: &mut TerrainRenderer, ctx: &GpuContext, p: &TerrainPlan) -> Image {
    r.render(ctx, p, false).unwrap();
    let f = r.read_color(ctx);
    Image::new(f.width, f.height, f.rgba)
}

// spec: REND-038
#[test]
fn a_slot_written_through_set_horizon_reads_back_as_written() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &p);
    assert_eq!(r.horizon_bytes(), 0, "the array is a dummy before a capacity is set");
    let none = HorizonFrame { capacity: 3, nodes: Vec::new(), update: SlotUpdate::default() };
    let pattern = |seed: u32| -> Vec<u8> {
        (0..SLOT_BYTES).map(|i| ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8).collect()
    };
    let (a, b) = (pattern(1), pattern(2));
    r.set_horizon(&ctx, &none, &[(0, a.clone()), (2, b.clone())]);
    assert_eq!(r.horizon_bytes(), 3 * SLOT_BYTES as u64);
    assert!(r.read_horizon_slot(&ctx, 0) == a, "slot 0 reads back as written");
    assert!(r.read_horizon_slot(&ctx, 2) == b, "slot 2 reads back as written");
    assert!(r.read_horizon_slot(&ctx, 1).iter().all(|&x| x == 0), "an unwritten slot is zero");
}

// spec: REND-038
#[test]
fn a_frame_with_no_map_in_the_table_is_the_frame_without_the_horizon() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    for index in [0usize, 3, 6] {
        let p = plan(index);
        let mut r = renderer(&ctx, &p);
        let plain = frame(&mut r, &ctx, &p);
        // The array is allocated and written, every node reads slot -1.
        let nodes = vec![HorizonNode { slot: -1, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 0.0, rect: [0.0; 4] }; p.nodes.len()];
        let set = HorizonFrame { capacity: 2, nodes, update: SlotUpdate::default() };
        r.set_horizon(&ctx, &set, &[(0, vec![255u8; SLOT_BYTES])]);
        let with = frame(&mut r, &ctx, &p);
        assert!(plain.pixels().eq(with.pixels()), "view {index}: slot -1 everywhere leaves the frame unchanged");
        r.clear_horizon();
        let cleared = frame(&mut r, &ctx, &p);
        assert!(plain.pixels().eq(cleared.pixels()), "view {index}: a cleared horizon leaves the frame unchanged");
    }
}

// ---- REND-038 part (c): the shader factor against `horizon_lookup` (GEN-046) ----

use planet_core::cube::{direction_to_face, face_to_direction};
use planet_core::{Face, Vec3};
use planet_frame::horizonview::{pack_coarse, pack_map};
use planet_generators::coarsehorizon::geographic_frame;
use planet_generators::horizon_lookup::{azimuth_in_frame, interpolate, lit_factor, sun_elevation};
use planet_render::{probe_horizon, HorizonQuery};

const AZ: usize = 16;

/// Deterministic bytes below `top` (a realistic horizon: at most `top` * 90/255 degrees).
fn bytes(count: usize, seed: u64, top: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    (0..count)
        .map(|_| {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            ((x >> 33) % top) as u8
        })
        .collect()
}

fn f3(v: Vec3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

/// A unit sun at `elevation` over the horizontal plane of `up`, its horizontal part along `heading` (a unit vector in the tangent plane).
fn sun_at(up: Vec3, heading: Vec3, elevation: f64) -> Vec3 {
    up * libm::sin(elevation) + heading * libm::cos(elevation)
}

fn table_of(nodes: &[HorizonNode]) -> Vec<f32> {
    HorizonFrame { capacity: 2, nodes: nodes.to_vec(), update: SlotUpdate::default() }.table()
}

// spec: REND-038
#[test]
fn the_shader_factor_of_a_chunk_map_equals_the_cpu_reference_at_texel_centres() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let n = 256usize;
    let map = bytes(n * n * AZ, 7, 80);
    let slot_bytes = pack_map(&map, n, AZ).unwrap();
    // Every query is its own node, in slot 1 over the chunk rectangle [0.25, 0.5] x [0.5, 0.75], with its own azimuth.
    let rect = [0.25f32, 0.5, 0.25, 0.25];
    let up = Vec3::new(0.3, -0.5, 0.8).normalized();
    let east = Vec3::new(0.0, 0.0, 1.0).cross(up).normalized();
    let north = up.cross(east).normalized();
    let mut nodes = Vec::new();
    let mut qs = Vec::new();
    let mut expect = Vec::new();
    for q in 0..40usize {
        // A texel inside the rectangle, at its centre, so the bilinear read is the texel.
        let (x, y) = (64 + (q * 7) % 64, 128 + (q * 11) % 64);
        let tile_uv = [((x as f32 + 0.5) / 256.0 - rect[0]) / rect[2], ((y as f32 + 0.5) / 256.0 - rect[1]) / rect[3]];
        let az = -3.0 + 0.4 * q as f64;
        let h = interpolate(&map[(y * n + x) * AZ..(y * n + x + 1) * AZ], az);
        let el = h + (-0.6 + 0.04 * q as f64).to_radians();
        let sun = sun_at(up, north * libm::cos(az) + east * libm::sin(az), el);
        nodes.push(HorizonNode { slot: 1, sun_azimuth: az as f32, sun_elevation: 0.0, fade: 1.0, rect });
        qs.push(HorizonQuery { node: q as u32, tile_uv, up: f3(up), sun: f3(sun), distance_m: 0.0 });
        expect.push((h, lit_factor(sun_elevation(sun, up), h)));
    }
    let got = probe_horizon(&ctx, 2, &[(1, slot_bytes)], None, &table_of(&nodes), &qs);
    let mut worst = (0.0f64, 0.0f64);
    for (g, (h, f)) in got.iter().zip(&expect) {
        assert!(g.present);
        worst = (worst.0.max((f64::from(g.horizon) - h).abs()), worst.1.max((f64::from(g.factor) - f).abs()));
    }
    eprintln!("chunk map: worst horizon error {:.2e} rad, worst factor error {:.2e}", worst.0, worst.1);
    assert!(worst.0 < 2e-5, "horizon error {:.2e} rad", worst.0);
    // The factor is the smoothstep of an f32 asin: a few 1e-5 rad of the adapter's asin is a few 1e-3 of the factor on a 1.25 degree edge.
    assert!(worst.1 < 8e-3, "factor error {:.2e}", worst.1);
    assert!(expect.iter().any(|e| e.1 > 0.05 && e.1 < 0.95), "some queries sit on the soft edge");
}

// spec: REND-038
#[test]
fn the_shader_factor_of_the_coarse_map_equals_the_cpu_reference_on_every_face_and_the_larger_horizon_wins() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let m = 8usize;
    // Smooth in the cell, as a real horizon is: a wrong cell is a step of at least 3 bytes (1 degree), the read error of a texture unit a fraction of that.
    let coarse: Vec<u8> = (0..6 * m * m * AZ)
        .map(|n| {
            let (k, cell) = (n % AZ, n / AZ);
            let (face, i, j) = (cell / (m * m), cell % m, (cell / m) % m);
            (10 + 7 * face + 4 * i + 3 * j + 5 * (k % 4) + 2 * (k / 4)) as u8
        })
        .collect();
    let packed = pack_coarse(&coarse, m, AZ).unwrap();
    // A chunk map in slot 0, so for the nodes that have it the larger of the two decides, now one, now the other.
    let n = 256usize;
    let chunk = bytes(n * n * AZ, 5, 160);
    let slot_bytes = pack_map(&chunk, n, AZ).unwrap();
    let rect = [0.0f32, 0.0, 1.0, 1.0];
    let map = planet_core::cube::TangentWarp;
    let (mut qs, mut nodes, mut expect) = (Vec::new(), Vec::new(), Vec::new());
    for face in 0..6u8 {
        for (k, (i, j)) in [(0usize, 0usize), (m - 1, 0), (3, 5), (m - 1, m - 1), (4, 2)].into_iter().enumerate() {
            let (s, t) = (2.0 * (i as f64 + 0.5) / m as f64 - 1.0, 2.0 * (j as f64 + 0.5) / m as f64 - 1.0);
            let up = face_to_direction(&map, Face(face), s, t);
            assert_eq!(direction_to_face(&map, up).0, Face(face));
            let (east, north) = geographic_frame(up);
            let phi = 0.7 * f64::from(face) + 1.3 * k as f64 - 2.0;
            let hc = interpolate(&coarse[(face as usize * m * m + j * m + i) * AZ..][..AZ], phi);
            let with = (face as usize + k).is_multiple_of(2);
            let (x, y) = ((17 * (k + 1)) % n, (29 * (face as usize + 1)) % n);
            let tile_uv = [(x as f32 + 0.5) / 256.0, (y as f32 + 0.5) / 256.0];
            let chunk_az = 0.37 * k as f64 + 0.11 * f64::from(face);
            let hk = if with { interpolate(&chunk[(y * n + x) * AZ..(y * n + x + 1) * AZ], chunk_az) } else { 0.0 };
            let h = hc.max(hk);
            let el = h + (-0.3 + 0.2 * k as f64).to_radians();
            let sun = sun_at(up, north * libm::cos(phi) + east * libm::sin(phi), el);
            assert!(
                (azimuth_in_frame(sun, east, north) - phi).rem_euclid(std::f64::consts::TAU).min(std::f64::consts::TAU) < 1e-9
                    || (phi - azimuth_in_frame(sun, east, north)).rem_euclid(std::f64::consts::TAU) < 1e-9
            );
            nodes.push(HorizonNode { slot: if with { 0 } else { -1 }, sun_azimuth: chunk_az as f32, sun_elevation: 0.0, fade: 1.0, rect });
            qs.push(HorizonQuery { node: qs.len() as u32, tile_uv, up: f3(up), sun: f3(sun), distance_m: 0.0 });
            expect.push((h, lit_factor(sun_elevation(sun, up), h)));
        }
    }
    let got = probe_horizon(&ctx, 1, &[(0, slot_bytes)], Some((m as u32, &packed)), &table_of(&nodes), &qs);
    let mut worst = (0.0f64, 0.0f64);
    for (g, (h, f)) in got.iter().zip(&expect) {
        assert!(g.present);
        worst = (worst.0.max((f64::from(g.horizon) - h).abs()), worst.1.max((f64::from(g.factor) - f).abs()));
    }
    for (q, (g, (h, _))) in got.iter().zip(&expect).enumerate() {
        if (f64::from(g.horizon) - h).abs() > 1e-4 {
            eprintln!("query {q} (face {}, k {}): gpu {} cpu {}", q / 5, q % 5, g.horizon, h);
        }
    }
    eprintln!("coarse + chunk: worst horizon error {:.2e} rad, worst factor error {:.2e}", worst.0, worst.1);
    assert!(worst.0 < 3e-4, "horizon error {:.2e} rad", worst.0);
    assert!(worst.1 < 2e-2, "factor error {:.2e}", worst.1);
}

// spec: REND-038
#[test]
fn a_node_with_no_chunk_map_and_no_coarse_map_is_lit_in_full_and_a_coarse_map_alone_shadows_it() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let none = HorizonNode { slot: -1, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 1.0, rect: [0.0; 4] };
    let table = table_of(&[none]);
    let up = Vec3::new(0.0, 1.0, 0.0);
    let q = |el: f64| HorizonQuery {
        node: 0,
        tile_uv: [0.5, 0.5],
        up: f3(up),
        sun: f3(sun_at(up, Vec3::new(1.0, 0.0, 0.0), el)),
        distance_m: 0.0,
    };
    let got = probe_horizon(&ctx, 1, &[], None, &table, &[q(-0.5), q(0.0), q(0.3), q(1.5)]);
    for g in &got {
        assert!(!g.present && g.factor == 1.0, "no map is the identity, even for a sun below the horizon: {g:?}");
    }
    // A coarse map whose horizon is 10 degrees everywhere: the sun under it is in shadow, the sun well above it is lit.
    let m = 4usize;
    let byte = (10.0f64 / (90.0 / 255.0)).round() as u8;
    let packed = pack_coarse(&vec![byte; 6 * m * m * AZ], m, AZ).unwrap();
    let got = probe_horizon(&ctx, 1, &[], Some((m as u32, &packed)), &table, &[q(5f64.to_radians()), q(30f64.to_radians())]);
    assert!(got[0].present && got[0].factor == 0.0, "{:?}", got[0]);
    assert!(got[1].factor == 1.0, "{:?}", got[1]);
}

// spec: REND-038
#[test]
fn a_frame_whose_chunk_horizon_is_the_zenith_loses_its_direct_light() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let p = plan(3);
    let mut r = renderer(&ctx, &p);
    let plain = frame(&mut r, &ctx, &p);
    let nodes = vec![HorizonNode { slot: 0, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 1.0, rect: [0.0, 0.0, 1.0, 1.0] }; p.nodes.len()];
    let set = HorizonFrame { capacity: 1, nodes, update: SlotUpdate::default() };
    // Every texel of every azimuth at the largest byte: a horizon at the zenith, so no direct light reaches any fragment.
    r.set_horizon(&ctx, &set, &[(0, vec![255u8; SLOT_BYTES])]);
    let shadowed = frame(&mut r, &ctx, &p);
    let sum = |i: &Image| i.pixels().map(|px| px.iter().take(3).map(|&c| u64::from(c)).sum::<u64>()).sum::<u64>();
    eprintln!("plain {} shadowed {}", sum(&plain), sum(&shadowed));
    assert!(sum(&plain) > 1000, "the plain frame has lit terrain");
    assert!(sum(&shadowed) * 20 < sum(&plain), "the shadowed frame is dark");
}

// spec: REND-038
#[test]
fn the_shader_azimuth_frame_agrees_with_the_reference_at_a_pole_and_with_the_sun_overhead() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let none = HorizonNode { slot: -1, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 1.0, rect: [0.0; 4] };
    let table = table_of(&[none]);
    // A coarse map that depends on the azimuth only: 10 degrees to the east half of the compass, 0 to the rest, so a wrong frame flips the result.
    let m = 4usize;
    let east_byte = (10.0f64 / (90.0 / 255.0)).round() as u8;
    let cell: Vec<u8> = (0..AZ).map(|k| if (1..=7).contains(&k) { east_byte } else { 0 }).collect();
    let packed = pack_coarse(&cell.repeat(6 * m * m), m, AZ).unwrap();
    let mut qs = Vec::new();
    let mut expect = Vec::new();
    // Near a pole (inside the 1e-6 of the old threshold), at the pole's neighbours, and at an ordinary point.
    for up in [Vec3::new(1e-7, 0.0, 1.0), Vec3::new(0.0, 3e-8, -1.0), Vec3::new(0.02, 0.01, 1.0), Vec3::new(0.4, 0.5, 0.2)] {
        let up = up.normalized();
        let (east, north) = geographic_frame(up);
        for phi in [0.5f64, 1.5, 4.0] {
            let sun = sun_at(up, north * libm::cos(phi) + east * libm::sin(phi), 5f64.to_radians());
            let h = interpolate(&cell, azimuth_in_frame(sun, east, north));
            qs.push(HorizonQuery { node: 0, tile_uv: [0.5, 0.5], up: f3(up), sun: f3(sun), distance_m: 0.0 });
            expect.push(lit_factor(sun_elevation(sun, up), h));
        }
    }
    // The sun straight overhead: no horizontal direction, the reference azimuth is 0 and the answer is finite.
    let up = Vec3::new(1.0, 0.0, 0.0);
    qs.push(HorizonQuery { node: 0, tile_uv: [0.5, 0.5], up: f3(up), sun: f3(up), distance_m: 0.0 });
    expect.push(1.0);
    let got = probe_horizon(&ctx, 1, &[], Some((m as u32, &packed)), &table, &qs);
    for (i, (g, e)) in got.iter().zip(&expect).enumerate() {
        assert!(g.factor.is_finite() && g.horizon.is_finite(), "query {i}: {g:?}");
        assert!((f64::from(g.factor) - e).abs() < 1e-2, "query {i}: gpu {} cpu {e}", g.factor);
    }
}

// spec: REND-038, CON-18
#[test]
fn the_horizon_and_shadow_views_show_the_ridge_that_casts_the_shadow_and_match_the_adapters_goldens() {
    use planet_frame::look::shading::ShadeView;
    use planet_testkit::{GoldenChecks, Tolerance};
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let mut goldens = GoldenChecks::new(&ctx.adapter_key(), &ctx.driver_string());
    let p = plan(3);
    let mut r = renderer(&ctx, &p);
    // A synthetic chunk map shared by every node: the horizon climbs from 0 to 90 degrees across the chunk's i axis in every azimuth, so each
    // tile shows a ridge line where the horizon passes the sun's elevation. One slot, the whole chunk per tile.
    let n = 256usize;
    let map: Vec<u8> = (0..n * n * AZ).map(|c| ((c / AZ) % n) as u8).collect();
    let nodes = vec![HorizonNode { slot: 0, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 1.0, rect: [0.0, 0.0, 1.0, 1.0] }; p.nodes.len()];
    let set = HorizonFrame { capacity: 1, nodes, update: SlotUpdate::default() };
    r.set_horizon(&ctx, &set, &[(0, pack_map(&map, n, AZ).unwrap())]);
    let mut grey = Vec::new();
    for (view, golden) in
        [(ShadeView::Horizon, "lit_ground_horizon_synthetic_ev15"), (ShadeView::Shadow, "lit_ground_shadow_synthetic_ev15")]
    {
        let mut q = p.clone();
        let mut l = LitPlan::fixed(EV);
        l.view = view;
        q.lit = Some(l);
        r.render(&ctx, &q, false).unwrap();
        let f = r.read_color(&ctx);
        let img = Image::new(f.width, f.height, f.rgba);
        let covered: Vec<bool> = r.lit_path().unwrap().read_hdr(&ctx).iter().map(|px| px[3] > 0.0).collect();
        assert!(covered.iter().filter(|c| **c).count() > 100, "{golden}: the terrain is there");
        // The view is grey: red, green and blue agree on the terrain pixels.
        let values: Vec<u8> = img.pixels().zip(&covered).filter(|(_, c)| **c).map(|(px, _)| px[0]).collect();
        for (px, c) in img.pixels().zip(&covered) {
            if *c {
                assert!(px[0].abs_diff(px[1]) <= 1 && px[1].abs_diff(px[2]) <= 1, "{golden}: {px:?} is not grey");
            }
        }
        grey.push(values);
        goldens.check(golden, &img, Tolerance::Exact);
    }
    // The horizon view climbs across the tile (several greys); the shadow view has both lit and shadowed terrain.
    let distinct: std::collections::BTreeSet<u8> = grey[0].iter().copied().collect();
    assert!(distinct.len() >= 8, "the horizon view shows a gradient ({} greys)", distinct.len());
    let (lit, dark) = (grey[1].iter().filter(|&&v| v > 200).count(), grey[1].iter().filter(|&&v| v < 20).count());
    assert!(lit > 50 && dark > 50, "the shadow view shows lit ({lit}) and shadowed ({dark}) terrain");
    goldens.finish();
}

// spec: REND-043
#[test]
fn the_coarse_map_fades_out_with_the_distance_of_the_camera_the_way_the_cpu_function_says() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let none = HorizonNode { slot: -1, sun_azimuth: 0.0, sun_elevation: 0.0, fade: 1.0, rect: [0.0; 4] };
    let table = table_of(&[none]);
    let up = Vec3::new(0.0, 1.0, 0.0);
    let sun = sun_at(up, Vec3::new(1.0, 0.0, 0.0), 5f64.to_radians());
    // A coarse map whose horizon is 10 degrees everywhere, the sun at 5 degrees: in shadow near, lit far, in between as the fade says.
    let m = 4usize;
    let byte = (10.0f64 / (90.0 / 255.0)).round() as u8;
    let packed = pack_coarse(&vec![byte; 6 * m * m * AZ], m, AZ).unwrap();
    let h10 = planet_generators::horizon::dequantize(byte);
    let distances = [0.0f64, 5.0e5, 1.0e6, 1.5e6, 2.0e6, 2.5e6, 3.0e6, 6.0e6];
    let qs: Vec<HorizonQuery> =
        distances.iter().map(|&d| HorizonQuery { node: 0, tile_uv: [0.5, 0.5], up: f3(up), sun: f3(sun), distance_m: d as f32 }).collect();
    let got = probe_horizon(&ctx, 1, &[], Some((m as u32, &packed)), &table, &qs);
    let el = sun_elevation(up, sun);
    let mut worst = 0.0f64;
    for (g, &d) in got.iter().zip(&distances) {
        let want = lit_factor(el, h10 * planet_generators::horizon::coarse_fade(d));
        worst = worst.max((f64::from(g.factor) - want).abs());
        assert!((f64::from(g.factor) - want).abs() < 2e-2, "{d} m: gpu {} cpu {want}", g.factor);
    }
    eprintln!("coarse fade: worst factor error {worst:.2e}");
    assert_eq!(got[0].factor, 0.0, "near: in shadow");
    assert_eq!(got.last().unwrap().factor, 1.0, "far: the coarse map casts nothing");
    assert!(got.windows(2).all(|w| w[1].factor >= w[0].factor - 1e-6), "monotone in the distance");
}
