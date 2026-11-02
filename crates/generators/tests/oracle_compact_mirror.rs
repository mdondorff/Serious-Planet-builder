//! Tier B (GEN-019, ADR 0033 point 4): the CPU mirror of the compact vertex shader against the canonical CPU mesh, for every level
//! from 3 to 26, on three fields: position model error (the biquadratic base surface), height step, stored normals (CON-14),
//! the morph target, and seams between neighbouring tiles.

use planet_core::cube::TangentWarp;
use planet_core::hash::SplitMix64;
use planet_core::{Side, TileId, Vec3};
use planet_generators::compact::{compact_tile_field, CompactTile, COMPACT_MIN_LEVEL};
use planet_generators::global::{bake, GridSource};
use planet_generators::height::{HeightField, Reference};
use planet_generators::height_fields::{build_field, HeightFieldKind};
use planet_generators::mesh::{tile_mesh_field, vertex_count, TileMesh};
use planet_generators::stress::Steep;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;

fn eroded() -> Box<dyn HeightField> {
    let grid = bake(7, &TangentWarp, 16, 2).unwrap();
    let def = WorldDefinition::single_planet(R, 7);
    let src = Box::leak(Box::new(GridSource(Box::leak(Box::new(grid)))));
    build_field(HeightFieldKind::Eroded, &GenContext::new(&def), BodyId::PLANET, src, [1; 16]).unwrap()
}

fn random_tile(rng: &mut SplitMix64, level: u8) -> TileId {
    let n = 1u64 << level;
    TileId::new(planet_core::Face((rng.next_u64() % 6) as u8), level, (rng.next_u64() % n) as u32, (rng.next_u64() % n) as u32).unwrap()
}

fn angle_deg(a: [f32; 3], b: [f32; 3]) -> f64 {
    let (a, b) =
        (Vec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2])), Vec3::new(f64::from(b[0]), f64::from(b[1]), f64::from(b[2])));
    libm::atan2(a.cross(b).length(), a.dot(b)).to_degrees()
}

/// Largest position error (position and morph target, every vertex including skirts) as a multiple of its bound, the largest
/// height error beyond the quantisation bound, and the largest normal angle.
fn mirror_error(field: &dyn HeightField, cells: u32, tiles: &[TileId]) -> (f64, f64, f64) {
    let (mut pos, mut height, mut normal) = (0f64, 0f64, 0f64);
    for id in tiles {
        let mesh = tile_mesh_field(field, &TangentWarp, *id, cells, R);
        let built = compact_tile_field(field, &TangentWarp, *id, cells, R);
        // The mirror reads the tile back from its slot bytes at the layout's offsets (GEN-025): one byte string for the CPU, the pack
        // and the shader.
        let ct = CompactTile::from_slot_data(*id, cells, built.key, built.origin, built.relief_m, &built.slot_data())
            .unwrap_or_else(|e| panic!("{id:?}: {e}"));
        assert_eq!(ct, built, "{id:?}: the slot bytes round-trip the tile");
        let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level());
        let shaded = ct.shade_all();
        assert_eq!(shaded.len(), vertex_count(cells));
        let theta = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << id.level());
        for (a, b) in mesh.vertices.iter().zip(&shaded) {
            let dist = |p: [f32; 3], q: [f32; 3]| -> f64 {
                (0..3)
                    .map(|k| {
                        let e = f64::from(p[k]) - f64::from(q[k]);
                        e * e
                    })
                    .sum::<f64>()
                    .sqrt()
            };
            let d = dist(a.pos, b.pos).max(dist(a.coarse, b.coarse));
            // The biquadratic base surface errs by about 0.018 theta^2 of the tile edge, plus f32 rounding of the tile-local numbers,
            // one half height step, and the f32 error of the base normal (about 2e-7 rad) times the tile's relief.
            let bound = edge * (0.03 * theta * theta + 1e-6) + 0.51 * f64::from(ct.hr_step) + 5e-7 * ct.relief_m;
            pos = pos.max(d / bound);
            let slack = 0.51 * f64::from(ct.hr_step) + 2.4e-7 * f64::from(a.height.abs().max(1.0)) + 1.2e-7 * ct.relief_m;
            height = height.max(f64::from((a.height - b.height).abs().max((a.coarse_height - b.coarse_height).abs())) - slack);
            normal = normal.max(angle_deg(a.normal, b.normal));
        }
    }
    (pos, height, normal)
}

// spec: GEN-019, GEN-025, LOD-004, CON-13
#[test]
fn the_cpu_mirror_matches_the_product_mesh_at_every_level_from_3_to_26() {
    let reference = Reference(1);
    let eroded = eroded();
    let mut rng = SplitMix64(31);
    for (name, field, cells) in [("reference", &reference as &dyn HeightField, 16u32), ("eroded", eroded.as_ref(), 32)] {
        let tiles: Vec<TileId> = (COMPACT_MIN_LEVEL..=26).flat_map(|l| [random_tile(&mut rng, l), random_tile(&mut rng, l)]).collect();
        let (pos, height, normal) = mirror_error(field, cells, &tiles);
        eprintln!("{name}: position {pos:.3} x bound, height beyond step {height:.3e} m, normal {normal:.5} degrees");
        assert!(pos < 1.0, "{name}: position error {pos} x the biquadratic bound");
        assert!(height < 1e-6, "{name}: height error beyond the quantisation step: {height}");
        assert!(normal <= 0.01, "{name}: normal error {normal} degrees (CON-14: 0.01)");
    }
    // The steep stress field: kilometres of relief, fine-scale slopes.
    let steep = Steep(1);
    let tiles: Vec<TileId> = (COMPACT_MIN_LEVEL..=22).flat_map(|l| [random_tile(&mut rng, l)]).collect();
    let (pos, height, normal) = mirror_error(&steep, 16, &tiles);
    eprintln!("steep: position {pos:.3} x bound, height beyond step {height:.3e} m, normal {normal:.5} degrees");
    assert!(pos < 1.0 && height < 1e-6, "steep field: position {pos} x bound, height {height}");
    assert!(normal <= 0.01, "steep field: normal error {normal} degrees");
}

fn world(m: &TileMesh, c: &CompactTile, i: u32, j: u32, coarse: bool) -> ([f64; 3], [f64; 3]) {
    let v = m.vertices[(j * (m.cells + 1) + i) as usize];
    let s = c.shade_vertex((j * (c.cells + 1) + i) as usize);
    let (a, b) = if coarse { (v.coarse, s.coarse) } else { (v.pos, s.pos) };
    let o = m.origin.0;
    (
        [o.x + f64::from(a[0]), o.y + f64::from(a[1]), o.z + f64::from(a[2])],
        [c.origin.0.x + f64::from(b[0]), c.origin.0.y + f64::from(b[1]), c.origin.0.z + f64::from(b[2])],
    )
}

// spec: GEN-019, LOD-005
#[test]
fn neighbouring_compact_tiles_meet_within_millimetres_where_the_tiles_are_small_and_far_under_the_skirt_elsewhere() {
    let field = eroded();
    let cells = 32u32;
    let mut rng = SplitMix64(77);
    let mut by_level = Vec::new();
    for level in COMPACT_MIN_LEVEL..=24u8 {
        let mut worst = 0f64;
        for _ in 0..2 {
            let t = random_tile(&mut rng, level);
            let side = [Side::East, Side::North][(rng.next_u64() % 2) as usize];
            let nb = t.neighbor(&TangentWarp, side);
            let (ma, ca) =
                (tile_mesh_field(field.as_ref(), &TangentWarp, t, cells, R), compact_tile_field(field.as_ref(), &TangentWarp, t, cells, R));
            let (mb, cb) = (
                tile_mesh_field(field.as_ref(), &TangentWarp, nb, cells, R),
                compact_tile_field(field.as_ref(), &TangentWarp, nb, cells, R),
            );
            // Every border vertex of `t` on the shared side has a partner on `nb`: compare the compact positions of the partners that
            // coincide in the product mesh (within 1 mm there), i.e. the compact seam is the product seam plus the model errors.
            let border = |m: &TileMesh, c: &CompactTile, k: u32, s: Side| -> [f64; 3] {
                let (i, j) = match s {
                    Side::East => (cells, k),
                    Side::West => (0, k),
                    Side::North => (k, cells),
                    Side::South => (k, 0),
                };
                world(m, c, i, j, false).1
            };
            let (sa, sb) = (side, [Side::West, Side::South][if side == Side::East { 0 } else { 1 }]);
            // Across a cube-face edge the lattice runs differently; only same-face pairs are compared by index.
            if nb.face() != t.face() {
                continue;
            }
            for k in 0..=cells {
                let (p, q) = (border(&ma, &ca, k, sa), border(&mb, &cb, k, sb));
                let (dx, dy, dz) = (p[0] - q[0], p[1] - q[1], p[2] - q[2]);
                let d = (dx * dx + dy * dy + dz * dz).sqrt();
                worst = worst.max(d);
            }
        }
        by_level.push((level, worst));
        let edge = R * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level);
        let theta = core::f64::consts::FRAC_PI_2 / f64::from(1u32 << level);
        // Product seam error (f32 rounding) plus twice the base-surface model error of the two tiles.
        let bound = edge * (0.002 * theta * theta + 2e-7) + 1e-3;
        assert!(worst <= bound, "level {level}: seam mismatch {worst} m beyond {bound}");
    }
    eprintln!("seam mismatch by level: {by_level:?}");
    // From level 12 (edges under 5 km) the mismatch is at the model-error floor: millimetres.
    for (level, worst) in by_level.iter().filter(|(l, _)| *l >= 12) {
        assert!(*worst <= 0.003, "level {level}: seam mismatch {worst} m");
    }
}

// spec: GEN-019
#[test]
fn the_exact_morph_target_equals_the_product_mesh_at_every_vertex_of_a_steep_tile() {
    // The approximate target (neighbour offsets along the vertex's own normal) fails here by metres; the exact one agrees to the bound
    // `mirror_error` states. This pins the morph target on the field where the difference shows (cells 16, level 6-8).
    let steep = Steep(1);
    let mut rng = SplitMix64(3);
    let tiles: Vec<TileId> = (6..=8u8).flat_map(|l| [random_tile(&mut rng, l), random_tile(&mut rng, l)]).collect();
    let (pos, height, _) = mirror_error(&steep, 16, &tiles);
    assert!(pos < 1.0 && height < 1e-6, "exact morph target off the product by {pos} x bound, height {height}");
}
