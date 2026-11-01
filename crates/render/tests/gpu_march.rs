//! REND-049, REND-053, tier C: the shadow march over a synthetic clipmap: a wall casts the shadow its geometry predicts with a penumbra
//! that widens with the distance, a sun below the receiver's geometric horizon gives 0 without a march, the cube continues the march, and
//! two runs write the same bytes.

use planet_render::march::{march_shader, MarchParams, ShadowMarchGpu, SUN_DISC_TAN};
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

fn params(sun_elevation_deg: f32, curvature: f32) -> MarchParams {
    let e = sun_elevation_deg.to_radians();
    MarchParams {
        sun: [-libm::cosf(e), 0.0, libm::sinf(e)],
        sun_tan: SUN_DISC_TAN,
        curvature,
        max_distance_m: 20_000.0,
        penumbra_search_m: 2_000.0,
        levels: (0..LEVELS)
            .map(|k| ([-(TEXELS as i32) / 2, -(TEXELS as i32) / 2, TEXELS as i32 / 2, TEXELS as i32 / 2], FIRST_TEXEL_M * (1 << k) as f32))
            .collect(),
        use_cube: false,
        anchor: [0.0, 0.0, 1.0],
        east: [1.0, 0.0, 0.0],
        north: [0.0, 1.0, 0.0],
        cube_texel_m: 100.0,
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
        let tip = 8.0 + wall / libm::tanf(10f32.to_radians());
        // The penumbra of the sun disc spans the distances at which the ray grazes the wall top within the disc.
        for (x, v) in xs.iter().zip(&vis) {
            if *x <= tip - 24.0 {
                assert_eq!(*v, 0.0, "wall {wall}: receiver at {x} m (tip {tip}) must be in the shadow");
            }
            if *x >= tip + 24.0 {
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
    let k = 1.0 / 6_371_000.0f32;
    let height = 100.0f32;
    let dip = (2.0 * height * k).sqrt();
    let below = -(dip + libm::sinf(1.0f32.to_radians()));
    let mut p = params(0.0, k);
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
