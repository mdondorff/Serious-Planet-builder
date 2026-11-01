//! REND-049, REND-053, tier C: the shadow march over a synthetic clipmap: a wall casts the shadow its geometry predicts with a penumbra
//! that widens with the distance, a sun below the receiver's geometric horizon gives 0 without a march, the cube continues the march, and
//! two runs write the same bytes.

use planet_render::march::{march_shader, MarchParams, ShadowMarchGpu, PENUMBRA_SEARCH_M, SUN_DISC_TAN};
use planet_render::shadow::{ClipmapGpu, CoarseCubeGpu, NO_DATA_M};
use planet_render::{AdapterPolicy, GpuContext};

const TEXELS: u32 = 256;
const CELLS: u32 = 16;
const CUBE: u32 = 16;
const LEVELS: usize = 3;
const FIRST_TEXEL_M: f32 = 8.0;

struct Scene {
    ctx: GpuContext,
    clip: ClipmapGpu,
    cube: CoarseCubeGpu,
    march: ShadowMarchGpu,
}

/// A flat clipmap with a wall one level-0 texel wide at world x in [0, 8), as high as `wall_m`, along all of y.
fn scene(wall_m: f32) -> Scene {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let clip = ClipmapGpu::new(&ctx, TEXELS, LEVELS, CELLS);
    for level in 0..LEVELS {
        let mut heights = vec![0f32; (TEXELS * TEXELS) as usize];
        for j in 0..TEXELS as usize {
            heights[j * TEXELS as usize] = wall_m;
        }
        clip.upload_level(&ctx, level, &heights);
    }
    clip.rebuild_all_mips(&ctx);
    let cube = CoarseCubeGpu::new(&ctx, CUBE, CELLS);
    let march = ShadowMarchGpu::new(&ctx);
    Scene { ctx, clip, cube, march }
}

fn params(sun_elevation_deg: f32, drop_coefficient: f32) -> MarchParams {
    let e = sun_elevation_deg.to_radians();
    MarchParams {
        sun: [-libm::cosf(e), 0.0, libm::sinf(e)],
        sun_tan: SUN_DISC_TAN,
        drop_coefficient,
        max_distance_m: 20_000.0,
        penumbra_search_m: PENUMBRA_SEARCH_M,
        levels: (0..LEVELS)
            .map(|k| ([-(TEXELS as i32) / 2, -(TEXELS as i32) / 2, TEXELS as i32 / 2, TEXELS as i32 / 2], FIRST_TEXEL_M * (1 << k) as f32))
            .collect(),
        use_cube: false,
        anchor: [0.0, 0.0, 1.0],
        east: [1.0, 0.0, 0.0],
        north: [0.0, 1.0, 0.0],
        cube_texel_m: 100.0,
        dirty_since: 0,
    }
}

/// Receivers on the ground at x = 12, 14, ..., y = 0, half a metre above it.
fn row() -> (Vec<[f32; 4]>, Vec<f32>) {
    let xs: Vec<f32> = (0..495).map(|i| 12.0 + 2.0 * i as f32).collect();
    (xs.iter().map(|&x| [x, 0.0, 0.5, 1.0]).collect(), xs)
}

fn wall_visibility(wall_m: f32, elevation_deg: f32) -> (Vec<f32>, Vec<f32>) {
    let s = scene(wall_m);
    let (receivers, xs) = row();
    let r = s.march.run(&s.ctx, &s.clip, &s.cube, &params(elevation_deg, 0.0), &receivers, receivers.len() as u32, 1);
    (xs, r.visibility)
}

// spec: REND-049
#[test]
fn a_wall_shadows_what_its_geometry_predicts_and_the_penumbra_widens_with_the_distance() {
    let mut widths = Vec::new();
    for wall in [50.0f32, 100.0] {
        let (xs, vis) = wall_visibility(wall, 10.0);
        let tan_e = libm::tanf(10f32.to_radians());
        let tip = 8.0 + wall / tan_e;
        // The sun disc blurs the edge over the distances at which the ray grazes the wall top within the disc; one texel of slack beyond.
        let half = 0.5 * (wall / (tan_e - SUN_DISC_TAN) - wall / (tan_e + SUN_DISC_TAN)) + FIRST_TEXEL_M;
        // The penumbra of the sun disc spans the distances at which the ray grazes the wall top within the disc.
        for (x, v) in xs.iter().zip(&vis) {
            if *x <= tip - half {
                assert_eq!(*v, 0.0, "wall {wall}: receiver at {x} m (tip {tip}) must be in the shadow");
            }
            if *x >= tip + half {
                assert_eq!(*v, 1.0, "wall {wall}: receiver at {x} m (tip {tip}) must be lit");
            }
        }
        widths.push(2.0 * vis.iter().filter(|v| **v > 0.02 && **v < 0.98).count() as f32);
    }
    assert!(widths[1] > 1.5 * widths[0], "the penumbra must widen with the distance from the wall: {widths:?}");
    assert!(widths[0] > 0.0);
}

// spec: REND-049
#[test]
fn a_sun_below_the_geometric_horizon_gives_zero_without_a_march() {
    let s = scene(0.0);
    let c = 0.5 / 6_371_000.0f32;
    let k = 2.0 * c;
    let height = 100.0f32;
    let dip = (2.0 * height * k).sqrt();
    let below = -(dip + libm::sinf(1.0f32.to_radians()));
    let mut p = params(0.0, c);
    p.sun = [-(1.0 - below * below).sqrt(), 0.0, below];
    let r = s.march.run(&s.ctx, &s.clip, &s.cube, &p, &[[0.0, 0.0, height, 1.0]], 1, 1);
    assert_eq!((r.visibility[0], r.steps[0]), (0.0, 0), "1 degree below the horizon");
    // A sun above the horizon on flat ground is lit, and the march took steps.
    let above = -dip + libm::sinf(1.0f32.to_radians());
    p.sun = [-(1.0 - above * above).sqrt(), 0.0, above];
    let r = s.march.run(&s.ctx, &s.clip, &s.cube, &p, &[[0.0, 0.0, height, 1.0]], 1, 1);
    assert_eq!(r.visibility[0], 1.0);
    assert!(r.steps[0] > 0);
}

// spec: REND-049
#[test]
fn the_cube_continues_the_march_beyond_the_outermost_level() {
    let s = scene(0.0);
    // Face 4 (+z) holds a 10 km high plateau; the receiver at 1000 m east marches west out of every level.
    let mut faces = vec![NO_DATA_M; (6 * CUBE * CUBE) as usize];
    for v in &mut faces[(4 * CUBE * CUBE) as usize..(5 * CUBE * CUBE) as usize] {
        *v = 10_000.0;
    }
    s.cube.upload_faces(&s.ctx, &faces);
    s.cube.build_mips(&s.ctx);
    let receivers = [[1000.0f32, 0.0, 0.5, 1.0]];
    let mut p = params(10.0, 0.0);
    let without = s.march.run(&s.ctx, &s.clip, &s.cube, &p, &receivers, 1, 1);
    p.use_cube = true;
    let with = s.march.run(&s.ctx, &s.clip, &s.cube, &p, &receivers, 1, 1);
    assert_eq!(without.visibility[0], 1.0, "without the cube nothing blocks the ray");
    assert_eq!(with.visibility[0], 0.0, "the plateau in the cube blocks it");
    assert!(with.steps[0] > without.steps[0]);
}

// spec: REND-053
#[test]
fn two_runs_write_the_same_bytes_and_the_march_has_no_atomics() {
    let s = scene(100.0);
    let (receivers, _) = row();
    let p = params(10.0, 0.0);
    let a = s.march.run(&s.ctx, &s.clip, &s.cube, &p, &receivers, receivers.len() as u32, 1);
    let b = s.march.run(&s.ctx, &s.clip, &s.cube, &p, &receivers, receivers.len() as u32, 1);
    assert!(a.visibility.iter().zip(&b.visibility).all(|(x, y)| x.to_bits() == y.to_bits()) && a.steps == b.steps);
    assert!(!march_shader().contains("atomic"), "REND-053: no atomics in the march");
}

fn slot(i: i32) -> usize {
    i.rem_euclid(TEXELS as i32) as usize
}

/// A scene whose level `k` holds `height(x_m)` at the texel centres, along all of y.
fn ramp_scene(height: impl Fn(f32) -> f32) -> Scene {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let clip = ClipmapGpu::new(&ctx, TEXELS, LEVELS, CELLS);
    for level in 0..LEVELS {
        let t = FIRST_TEXEL_M * (1 << level) as f32;
        let mut heights = vec![0f32; (TEXELS * TEXELS) as usize];
        for i in -(TEXELS as i32) / 2..TEXELS as i32 / 2 {
            let h = height((i as f32 + 0.5) * t);
            for j in 0..TEXELS as usize {
                heights[j * TEXELS as usize + slot(i)] = h;
            }
        }
        clip.upload_level(&ctx, level, &heights);
    }
    clip.rebuild_all_mips(&ctx);
    let cube = CoarseCubeGpu::new(&ctx, CUBE, CELLS);
    let march = ShadowMarchGpu::new(&ctx);
    Scene { ctx, clip, cube, march }
}

// spec: REND-049
#[test]
fn a_receiver_on_the_ground_or_on_a_slope_facing_the_sun_is_not_its_own_occluder() {
    // Flat ground, receivers on it and a few millimetres under the texel sample.
    let s = scene(0.0);
    let flat: Vec<[f32; 4]> =
        [0.0f32, -0.003, 0.5].iter().flat_map(|&z| (0..40).map(move |i| [100.0 + 20.0 * i as f32, 0.0, z, 1.0])).collect();
    for elevation in [10.0f32, 45.0] {
        let r = s.march.run(&s.ctx, &s.clip, &s.cube, &params(elevation, 0.0), &flat, flat.len() as u32, 1);
        assert!(r.visibility.iter().all(|v| *v == 1.0), "flat ground, sun {elevation} degrees: {:?}", &r.visibility[..8]);
    }
    // A plane rising toward the sun (20 degrees) under a sun 30 degrees above the horizon: 10 degrees above the slope.
    let tan20 = libm::tanf(20.0f32.to_radians());
    let s = ramp_scene(|x| -tan20 * x);
    let on_slope: Vec<[f32; 4]> = (0..40)
        .map(|i| {
            let x = (((-400 + 20 * i) as f32) / FIRST_TEXEL_M).floor() * FIRST_TEXEL_M + 0.5 * FIRST_TEXEL_M;
            [x, 0.0, -tan20 * x, 1.0]
        })
        .collect();
    let r = s.march.run(&s.ctx, &s.clip, &s.cube, &params(30.0, 0.0), &on_slope, on_slope.len() as u32, 1);
    assert!(r.visibility.iter().all(|v| *v == 1.0), "a slope facing the sun must be lit: {:?}", r.visibility);
}

// spec: REND-049
#[test]
fn a_ray_is_judged_by_the_next_level_where_a_coarse_cell_would_pass_the_edge_of_the_window() {
    let blocked = |wall: f32| {
        let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
        let clip = ClipmapGpu::new(&ctx, TEXELS, LEVELS, CELLS);
        for level in 0..LEVELS {
            let mut heights = vec![0f32; (TEXELS * TEXELS) as usize];
            if level == 1 {
                // A wall 16 m wide at world x in [-1104, -1088) in level 1 only, beyond level 0's window.
                for j in 0..TEXELS as usize {
                    heights[j * TEXELS as usize + slot(-69)] = wall;
                }
            }
            clip.upload_level(&ctx, level, &heights);
        }
        clip.rebuild_all_mips(&ctx);
        let cube = CoarseCubeGpu::new(&ctx, CUBE, CELLS);
        let march = ShadowMarchGpu::new(&ctx);
        let mut p = params(5.0, 0.0);
        // Level 0's window is not aligned to any mip: [-129, 127) texels of 8 m.
        p.levels[0].0 = [-129, -128, 127, 128];
        march.run(&ctx, &clip, &cube, &p, &[[900.0, 0.0, 0.5, 1.0]], 1, 1).visibility[0]
    };
    assert_eq!(blocked(0.0), 1.0, "nothing in the way");
    assert_eq!(blocked(300.0), 0.0, "the wall in the next level blocks the ray");
}

// spec: REND-049
#[test]
fn a_low_sun_over_flat_ground_ends_the_march_in_a_bounded_number_of_steps() {
    let s = scene(0.0);
    let r = s.march.run(&s.ctx, &s.clip, &s.cube, &params(0.2, 0.0), &[[500.0, 0.0, 0.5, 1.0]], 1, 1);
    assert!(r.steps[0] < 600, "a low sun over flat ground took {} steps", r.steps[0]);
    assert!(r.visibility[0] > 0.0);
}

/// The cube with `heights` on the faces listed, `NO_DATA_M` elsewhere, mips built.
fn cube_with(s: &Scene, high_faces: &[usize]) {
    let mut faces = vec![NO_DATA_M; (6 * CUBE * CUBE) as usize];
    for f in high_faces {
        for v in &mut faces[f * (CUBE * CUBE) as usize..(f + 1) * (CUBE * CUBE) as usize] {
            *v = 10_000.0;
        }
    }
    s.cube.upload_faces(&s.ctx, &faces);
    s.cube.build_mips(&s.ctx);
}

/// A march from an anchor `degrees` short of the face edge between the +z face and the +x face (or of the corner of the +x, +y and +z
/// faces), the sun in the direction of the neighbouring face.
fn edge_pose(corner: bool, high_faces: &[usize]) -> f32 {
    let s = scene(0.0);
    cube_with(&s, high_faces);
    let c = 0.5 / 6_371_000.0f32;
    let (anchor, east, north) = if corner {
        // Near the corner (1, 1, 1) / sqrt 3, on the +z side of it.
        let a: [f32; 3] = [0.5773, 0.5773, 0.5780];
        let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let a = [a[0] / n, a[1] / n, a[2] / n];
        // East toward -y +x: perpendicular to a within the plane of x, y.
        let e = [std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2, 0.0];
        let nn = [a[1] * e[2] - a[2] * e[1], a[2] * e[0] - a[0] * e[2], a[0] * e[1] - a[1] * e[0]];
        (a, e, nn)
    } else {
        let beta = 44.9f32.to_radians();
        ([libm::sinf(beta), 0.0, libm::cosf(beta)], [libm::cosf(beta), 0.0, -libm::sinf(beta)], [0.0, 1.0, 0.0])
    };
    let mut p = params(10.0, c);
    p.sun = [libm::cosf(10f32.to_radians()), 0.0, libm::sinf(10f32.to_radians())];
    p.use_cube = true;
    p.anchor = anchor;
    p.east = east;
    p.north = north;
    p.max_distance_m = 60_000.0;
    s.march.run(&s.ctx, &s.clip, &s.cube, &p, &[[0.0, 0.0, 0.5, 1.0]], 1, 1).visibility[0]
}

// spec: REND-049
#[test]
fn a_ridge_just_across_a_cube_edge_or_corner_blocks_the_ray() {
    // The edge pose: the anchor is on the +z face about 11 km from the edge, the sun toward the +x face whose texels hold the ridge.
    assert_eq!(edge_pose(false, &[0]), 0.0, "the ridge on the face across the edge must block");
    assert_eq!(edge_pose(false, &[]), 1.0, "without a ridge nothing blocks");
    // The corner pose: the ridge on one of the other two faces that meet at the corner.
    assert_eq!(edge_pose(true, &[0, 2]), 0.0, "the ridge on the faces around the corner must block");
    assert_eq!(edge_pose(true, &[]), 1.0);
}
