//! Tier A, GEN-112 and GEN-102 (m9-bake PR 6): the runtime read of the erosion layer. Off unless `with_layer` is called; on, the field is the relief base plus the layer's change (read C1,
//! faded at the coast), with the crease band of the bake; the bounds hold every height and nest; the keys follow the layer. A synthetic layer on small tiles; no store, no planet bake.

use std::sync::Arc;

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::{hash_u64s, SplitMix64};
use planet_core::{Face, Vec3};
use planet_generators::bake::tile::{TileGrid, TileId as PassTile, TileResult, TileStats};
use planet_generators::erosion::{ErosionField, ErosionGenerator, EROSION_C1_LAYER_IMPLEMENTATION_ID, EROSION_LAYER_IMPLEMENTATION_ID};
use planet_generators::erosion_layer::{coast_fade, ErosionLayer, LayerSource, LayerTile, COAST_FADE_M, MAX_CAP_TILES};
use planet_generators::global::{bake, GlobalGrid, GridSource, LatticeSource};
use planet_generators::height::HeightField;
use planet_generators::relief_channels::ReliefChannels;
use planet_generators::relief_lattice::ReliefLattice;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

const R: f64 = 6_371_000.0;
const SEED: u64 = 7;
const KEY: [u8; 16] = [9; 16];
const CELLS: usize = 64;
/// The layer's grid: 4 x 4 tiles of 64 cells a face.
const LAYER_CELLS: usize = 256;
const TILE: usize = 64;

fn grid() -> GlobalGrid {
    let def = WorldDefinition::single_planet(R, SEED);
    let seed = GenContext::new(&def).body_seed(BodyId::PLANET).unwrap();
    bake(seed, &TangentWarp, 256, 4).unwrap()
}

fn plain<'a>(src: &'a dyn LatticeSource, c1: bool) -> ErosionField<'a> {
    let def = WorldDefinition::single_planet(R, SEED);
    let g = ErosionGenerator;
    if c1 {
        g.field_c1(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
    } else {
        g.field(&GenContext::new(&def), BodyId::PLANET, src, KEY).unwrap()
    }
}

fn channels() -> ReliefChannels {
    let (kf, mf): (&dyn Fn(Vec3) -> f64, &dyn Fn(Vec3) -> f64) =
        (&|d| 1.0 + 3.0 * (0.5 + 0.5 * libm::sin(3.0 * d.x + 1.0) * libm::cos(2.0 * d.y)), &|d| 400.0 * libm::sin(4.0 * d.z) + 150.0);
    let faces = (0..6u8)
        .map(|f| {
            let (mut k, mut m) = (Vec::new(), Vec::new());
            for j in 0..=CELLS {
                for i in 0..=CELLS {
                    let d =
                        face_to_direction(&TangentWarp, Face(f), 2.0 * i as f64 / CELLS as f64 - 1.0, 2.0 * j as f64 / CELLS as f64 - 1.0);
                    k.push(kf(d) as f32);
                    m.push(mf(d) as f32);
                }
            }
            (k, m)
        })
        .collect();
    ReliefChannels::from_planes(CELLS, faces).expect("channels of a synthetic lattice")
}

fn relief<'a>(src: &'a dyn LatticeSource, c1: bool) -> ErosionField<'a> {
    plain(src, c1).with_relief(Arc::new(ReliefLattice::new(channels()).unwrap())).unwrap()
}

fn random_dir(rng: &mut SplitMix64) -> Vec3 {
    Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized()
}

fn offset(c: Vec3, theta: f64, phi: f64) -> Vec3 {
    let a = if c.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let e1 = c.cross(a).normalized();
    let e2 = c.cross(e1);
    (c * libm::cos(theta) + (e1 * libm::cos(phi) + e2 * libm::sin(phi)) * libm::sin(theta)).normalized()
}

fn layer_grid() -> TileGrid {
    TileGrid { cells: LAYER_CELLS, tile: TILE, halo: 8, radius_m: R }
}

/// The change of the synthetic layer at cell `(i, j)` of `face`: smooth swells and a hashed cell-scale noise, metres; the tiles with `(ti + tj).is_multiple_of(5)` on face 2 are absent (0).
fn synthetic(scale: f64, seed: u64, face: u8, i: usize, j: usize) -> f64 {
    if face == 2 && (i / TILE + j / TILE).is_multiple_of(5) {
        return 0.0;
    }
    let (x, y) = (i as f64 / LAYER_CELLS as f64, j as f64 / LAYER_CELLS as f64);
    let noise = (hash_u64s(seed, &[u64::from(face), i as u64, j as u64]) >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
    scale * (40.0 * libm::sin(9.0 * x + f64::from(face)) * libm::cos(7.0 * y) + 25.0 * noise)
}

fn zero_stats() -> TileStats {
    TileStats {
        land_cells: 0,
        thermal_passes: 0,
        thermal_moving_at_cap: 0,
        eroded_m3: 0.0,
        deposited_m3: 0.0,
        exported_m3: 0.0,
        clamped_cells: 0,
    }
}

type Value = Arc<dyn Fn(u8, usize, usize) -> f64 + Send + Sync>;

/// A layer held in memory, built tile by tile on demand from a function of the cell; the noisy layer leaves some tiles of face 2 absent.
struct MemoryLayer {
    grid: TileGrid,
    value: Value,
    absent: bool,
    range: (f64, f64),
    id: u64,
    held: std::sync::Mutex<std::collections::HashMap<(u8, u32, u32), Arc<LayerTile>>>,
}

impl MemoryLayer {
    fn of(value: Value, absent: bool, id: u64) -> MemoryLayer {
        let (mut lo, mut hi) = (0.0f64, 0.0f64);
        for f in 0..6u8 {
            for j in 0..LAYER_CELLS {
                for i in 0..LAYER_CELLS {
                    let v = f64::from(value(f, i, j) as f32);
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
        MemoryLayer { grid: layer_grid(), value, absent, range: (lo, hi), id, held: Default::default() }
    }

    fn new(scale: f64, seed: u64) -> MemoryLayer {
        MemoryLayer::of(Arc::new(move |f, i, j| synthetic(scale, seed, f, i, j)), true, hash_u64s(0x6d65_6d6c, &[scale.to_bits(), seed]))
    }
}

impl LayerSource for MemoryLayer {
    fn grid(&self) -> &TileGrid {
        &self.grid
    }
    fn tile(&self, face: Face, ti: u32, tj: u32) -> Result<Option<Arc<LayerTile>>, String> {
        if self.absent && face.0 == 2 && (ti + tj).is_multiple_of(5) {
            return Ok(None);
        }
        if let Some(t) = self.held.lock().unwrap().get(&(face.0, ti, tj)) {
            return Ok(Some(t.clone()));
        }
        let core: Vec<f32> =
            (0..TILE * TILE).map(|k| (self.value)(face.0, ti as usize * TILE + k % TILE, tj as usize * TILE + k / TILE) as f32).collect();
        let result = Arc::new(TileResult {
            id: PassTile { face, ti, tj },
            core,
            deposited: Vec::new(),
            fill: Vec::new(),
            heights: Vec::new(),
            stats: zero_stats(),
        });
        let tile = Arc::new(LayerTile::new(result, TILE)?);
        self.held.lock().unwrap().insert((face.0, ti, tj), tile.clone());
        Ok(Some(tile))
    }
    fn global_range(&self) -> (f64, f64) {
        self.range
    }
    fn id(&self) -> u64 {
        self.id
    }
}

/// A smooth layer: a function of the cell's direction, so it is continuous across the cube's edges (no noise, no absent tile).
fn smooth_value(face: u8, i: usize, j: usize) -> f64 {
    let d = layer_grid().direction(Face(face), i as i64, j as i64);
    80.0 * libm::sin(5.0 * d.x + 1.0) * libm::cos(4.0 * d.y) + 60.0 * libm::sin(3.0 * d.z + 0.5)
}

fn smooth_layer() -> Arc<ErosionLayer> {
    Arc::new(ErosionLayer::new(Arc::new(MemoryLayer::of(Arc::new(smooth_value), false, 77))).unwrap())
}

/// A layer of zeros with one cell of 1000 m.
fn spike_layer(face: u8, i: usize, j: usize) -> Arc<ErosionLayer> {
    let value: Value = Arc::new(move |f, a, b| if (f, a, b) == (face, i, j) { 1000.0 } else { 0.0 });
    Arc::new(ErosionLayer::new(Arc::new(MemoryLayer::of(value, false, 78))).unwrap())
}

/// The direction at fractional cell coordinates `(x, y)` of `face` (a cell's centre at its integer coordinates).
fn at_cell(face: u8, x: f64, y: f64) -> Vec3 {
    face_to_direction(&TangentWarp, Face(face), 2.0 * (x + 0.5) / LAYER_CELLS as f64 - 1.0, 2.0 * (y + 0.5) / LAYER_CELLS as f64 - 1.0)
}

fn layer(scale: f64, seed: u64) -> Arc<ErosionLayer> {
    Arc::new(ErosionLayer::new(Arc::new(MemoryLayer::new(scale, seed))).unwrap())
}

/// The face coordinates of the centre of cell `(i, j)`.
fn cell_dir(face: u8, i: usize, j: usize) -> Vec3 {
    face_to_direction(
        &TangentWarp,
        Face(face),
        2.0 * (i as f64 + 0.5) / LAYER_CELLS as f64 - 1.0,
        2.0 * (j as f64 + 0.5) / LAYER_CELLS as f64 - 1.0,
    )
}

// spec: GEN-112, GEN-188
#[test]
fn the_read_is_the_cell_value_at_a_cell_centre_and_within_the_four_cells_around_a_point() {
    let l = layer(1.0, 3);
    let mut rng = SplitMix64(1);
    for _ in 0..300 {
        let (f, i, j) = ((rng.next_f64() * 6.0) as u8, 3 + (rng.next_f64() * 250.0) as usize, 3 + (rng.next_f64() * 250.0) as usize);
        let v = l.change_at(cell_dir(f, i, j)).unwrap();
        let want = f64::from(synthetic(1.0, 3, f, i, j) as f32);
        assert!((v - want).abs() < 1e-6, "face {f} cell ({i}, {j}): read {v}, cell {want}");
    }
    for _ in 0..2000 {
        let d = random_dir(&mut rng);
        let (face, s, t) = planet_core::cube::direction_to_face(&TangentWarp, d);
        let (x, y) = (
            ((s + 1.0) * 0.5 * LAYER_CELLS as f64 - 0.5).clamp(0.0, 254.999),
            ((t + 1.0) * 0.5 * LAYER_CELLS as f64 - 0.5).clamp(0.0, 254.999),
        );
        // Within two cells of a cube edge the read is cross-faded with the neighbouring face's: the blend is tested on its own.
        if x < 3.0 || y < 3.0 || x > 252.0 || y > 252.0 {
            continue;
        }
        let (i, j) = (x as usize, y as usize);
        let c = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)].map(|(a, b)| f64::from(synthetic(1.0, 3, face.0, a, b) as f32));
        let (lo, hi) = (c.iter().cloned().fold(f64::INFINITY, f64::min), c.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        let v = l.change_at(d).unwrap();
        assert!(v >= lo - 1e-9 && v <= hi + 1e-9, "read {v} outside the four cells [{lo}, {hi}] at {d:?}");
    }
}

// spec: GEN-112
#[test]
fn the_read_has_no_crease_across_a_cell_line_or_a_tile_seam() {
    // A smooth layer: the one-sided slopes at a cell centre (where the Catmull-Rom segments meet) agree, along both axes, at the cells around the seams of the tiles (cells 62 to 65 and
    // 126 to 129). A bilinear read or a stencil clamped at a tile seam leaves a kink of the layer's second difference (about 1 m per cell here).
    let l = smooth_layer();
    let h = 1e-4;
    let slope = |face: u8, x: f64, y: f64, axis: usize, side: f64| {
        let (dx, dy) = if axis == 0 { (side * h, 0.0) } else { (0.0, side * h) };
        (l.change_at(at_cell(face, x + dx, y + dy)).unwrap() - l.change_at(at_cell(face, x, y)).unwrap()) / h * side
    };
    let mut rng = SplitMix64(21);
    let mut tested = 0;
    for _ in 0..400 {
        let face = (rng.next_f64() * 6.0) as u8;
        let seam = [62.0, 63.0, 64.0, 65.0, 126.0, 127.0, 128.0, 129.0][(rng.next_f64() * 8.0) as usize];
        let along = 5.0 + (rng.next_f64() * 240.0).floor();
        let (x, y) = if rng.next_f64() < 0.5 { (seam, along) } else { (along, seam) };
        for axis in 0..2 {
            let (right, left) = (slope(face, x, y, axis, 1.0), slope(face, x, y, axis, -1.0));
            // At an extremum the read is clamped to its four cells and is C1 only otherwise.
            if right.abs().min(left.abs()) < 0.5 {
                continue;
            }
            tested += 1;
            assert!((right - left).abs() < 0.02, "face {face} cell ({x}, {y}) axis {axis}: slopes {left} and {right} m per cell");
        }
    }
    assert!(tested > 300, "enough knots away from an extremum ({tested})");
}

// spec: GEN-112
#[test]
fn the_read_is_continuous_across_every_cube_edge_and_around_every_corner() {
    // A layer that is a smooth function of the direction has no step at an edge once the two faces' reads are cross-faded; each face alone would step by about the cell-to-cell change.
    let l = smooth_layer();
    let eps = 1e-9;
    let mut rng = SplitMix64(22);
    let mut worst: f64 = 0.0;
    for _ in 0..600 {
        let face = (rng.next_f64() * 6.0) as u8;
        let along = rng.next_f64() * 2.0 - 1.0;
        let (sa, ta) = [(1.0, along), (-1.0, along), (along, 1.0), (along, -1.0)][(rng.next_f64() * 4.0) as usize];
        let inside = face_to_direction(&TangentWarp, Face(face), sa * (1.0 - eps), ta * (1.0 - eps));
        let outside = face_to_direction(&TangentWarp, Face(face), sa * (1.0 + eps), ta * (1.0 + eps));
        worst = worst.max((l.change_at(inside).unwrap() - l.change_at(outside).unwrap()).abs());
    }
    assert!(worst < 1e-3, "a step of {worst} m across a cube edge");
    for corner in [
        (1.0, 1.0, 1.0),
        (1.0, 1.0, -1.0),
        (1.0, -1.0, 1.0),
        (-1.0, 1.0, 1.0),
        (-1.0, -1.0, 1.0),
        (-1.0, 1.0, -1.0),
        (1.0, -1.0, -1.0),
        (-1.0, -1.0, -1.0),
    ] {
        let c = Vec3::new(corner.0, corner.1, corner.2).normalized();
        let reads: Vec<f64> = (0..12).map(|k| l.change_at(offset(c, 1e-9, f64::from(k) * std::f64::consts::FRAC_PI_6)).unwrap()).collect();
        let (lo, hi) = (reads.iter().cloned().fold(f64::INFINITY, f64::min), reads.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        assert!(hi - lo < 1e-3, "around the corner {corner:?} the reads span {} m", hi - lo);
    }
}

// spec: GEN-112
#[test]
fn an_absent_tile_reads_as_zero_and_the_neighbours_stencil_sees_it() {
    let l = layer(1.0, 3);
    // Face 2 tile (0, 0) is absent: its cell centres read 0, a cell beside it reads its own value, and between the last cell of the absent tile and the first of the next the read lies
    // between 0 and that cell's value (the stencil reads the absent cells as 0).
    assert_eq!(l.change_at(cell_dir(2, 10, 10)).unwrap(), 0.0);
    let beside = l.change_at(cell_dir(2, 64 + 10, 10)).unwrap();
    assert!((beside - f64::from(synthetic(1.0, 3, 2, 74, 10) as f32)).abs() < 1e-6);
    let edge = f64::from(synthetic(1.0, 3, 2, 64, 10) as f32);
    assert!(edge.abs() > 1.0, "a cell with a change to compare");
    for x in [63.1, 63.5, 63.9] {
        let v = l.change_at(at_cell(2, x, 10.0)).unwrap();
        assert!(v.abs() <= edge.abs() + 1e-9 && v * edge >= -1e-9, "x {x}: read {v} not between 0 and {edge}");
        assert!(v != edge, "x {x}: the read ignores the absent tile");
    }
}

// spec: GEN-113
#[test]
fn the_coast_fade_is_zero_at_the_sea_and_one_three_metres_above_it() {
    assert_eq!(COAST_FADE_M, 3.0);
    assert_eq!((coast_fade(-5.0, 0.0), coast_fade(0.0, 0.0), coast_fade(3.0, 0.0), coast_fade(80.0, 0.0)), (0.0, 0.0, 1.0, 1.0));
    assert!((coast_fade(1.5, 0.0) - 0.5).abs() < 1e-12);
    assert_eq!(coast_fade(103.0, 100.0), 1.0, "the sea level of the definition, not 0");
    let mut prev = 0.0;
    for k in 0..=60 {
        let f = coast_fade(f64::from(k) * 0.1, 0.0);
        assert!(f >= prev && (0.0..=1.0).contains(&f));
        prev = f;
    }
}

fn layered<'a>(src: &'a dyn LatticeSource, c1: bool, scale: f64) -> ErosionField<'a> {
    relief(src, c1).with_layer(layer(scale, 3), 0.0).unwrap()
}

// spec: GEN-112
#[test]
fn off_is_the_field_with_the_band_and_a_layer_of_zeros_changes_nothing() {
    let g = grid();
    let src = GridSource(&g);
    for c1 in [false, true] {
        let band = relief(&src, c1).with_crease_band(planet_generators::erosion::CreaseBand::of_the_bake(0.0)).unwrap();
        let zero = relief(&src, c1).with_layer(layer(0.0, 3), 0.0).unwrap();
        let mut rng = SplitMix64(11);
        for _ in 0..300 {
            let d = random_dir(&mut rng);
            assert_eq!(band.height(d).to_bits(), zero.height(d).to_bits(), "c1 {c1}: a layer of zeros adds nothing, bit for bit");
        }
        assert!(layered(&src, c1, 1.0).layer_on() && !relief(&src, c1).layer_on());
    }
}

// spec: GEN-113
#[test]
fn the_layer_is_added_after_the_relief_base_faded_at_the_coast_and_not_scaled_by_k() {
    let g = grid();
    let src = GridSource(&g);
    let l = layer(1.0, 3);
    let on = layered(&src, true, 1.0);
    let band = relief(&src, true).with_crease_band(planet_generators::erosion::CreaseBand::of_the_bake(0.0)).unwrap();
    let mut rng = SplitMix64(12);
    let (mut high, mut coast, mut sea) = (0, 0, 0);
    for _ in 0..3000 {
        let d = random_dir(&mut rng);
        let e = on.base_elevation(d);
        let want = band.height(d) + coast_fade(e, 0.0) * l.change_at(d).unwrap();
        assert_eq!(on.height(d).to_bits(), want.to_bits());
        if e > 3.0 {
            high += 1;
        } else if e > 0.0 {
            coast += 1;
        } else {
            sea += 1;
            assert_eq!(on.height(d).to_bits(), band.height(d).to_bits(), "no change below the sea level");
        }
    }
    assert!(high > 100 && sea > 100, "the sample covers land and sea ({high} high, {coast} coast, {sea} sea)");
    // The first 3 m above the sea: directions whose bake elevation lies strictly inside the fade, read through the field.
    let mut coast = Vec::new();
    for _ in 0..400_000 {
        let d = random_dir(&mut rng);
        let e = on.base_elevation(d);
        if e > 0.1 && e < 2.9 {
            coast.push((d, e));
            if coast.len() == 5 {
                break;
            }
        }
    }
    assert!(coast.len() >= 3, "directions on the first 3 m above the sea ({})", coast.len());
    for (d, e) in coast {
        let fade = coast_fade(e, 0.0);
        let change = l.change_at(d).unwrap();
        assert!(fade > 0.0 && fade < 1.0 && change.abs() > 1e-3, "a coast point with a share of a change (fade {fade}, change {change})");
        assert_eq!(on.height(d).to_bits(), (band.height(d) + fade * change).to_bits(), "the coast adds the fade's share of the change");
    }
}

// spec: GEN-112
#[test]
fn a_batch_of_heights_is_the_single_reads_bit_for_bit() {
    let g = grid();
    let src = GridSource(&g);
    let f = layered(&src, true, 1.0);
    let mut rng = SplitMix64(13);
    let dirs: Vec<Vec3> = (0..400).map(|_| random_dir(&mut rng)).collect();
    let batch = f.heights(&dirs);
    for (d, h) in dirs.iter().zip(&batch) {
        assert_eq!(f.height(*d).to_bits(), h.to_bits());
    }
    assert!(f.error().is_none());
}

// spec: GEN-114
#[test]
fn the_keys_follow_the_layer_and_the_order_of_the_calls() {
    let g = grid();
    let src = GridSource(&g);
    let a = layered(&src, false, 1.0);
    let b = layered(&src, false, 2.0);
    let c = relief(&src, false).with_layer(layer(1.0, 3), 5.0).unwrap();
    let band = relief(&src, false).with_crease_band(planet_generators::erosion::CreaseBand::of_the_bake(0.0)).unwrap();
    assert_ne!(a.inputs_hash(), b.inputs_hash(), "another layer");
    assert_ne!(a.inputs_hash(), c.inputs_hash(), "another sea level");
    assert_ne!(a.inputs_hash(), band.inputs_hash(), "the layer is part of the key");
    assert_eq!(a.implementation_id(), EROSION_LAYER_IMPLEMENTATION_ID);
    assert_eq!(layered(&src, true, 1.0).implementation_id(), EROSION_C1_LAYER_IMPLEMENTATION_ID);
    assert_eq!(a.version(), band.version(), "the field version does not change");
    // Without the relief channels the layer still applies (`k` 1, `m` unread), under its own ids.
    let alone = plain(&src, false).with_layer(layer(1.0, 3), 0.0).unwrap();
    assert_eq!(alone.implementation_id(), planet_generators::erosion::EROSION_LAYER_ONLY_IMPLEMENTATION_ID);
    assert_eq!(
        plain(&src, true).with_layer(layer(1.0, 3), 0.0).unwrap().implementation_id(),
        planet_generators::erosion::EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID
    );
    assert_ne!(alone.inputs_hash(), a.inputs_hash(), "the channels are part of the key when on");
    let e = relief(&src, false)
        .with_crease_band(planet_generators::erosion::CreaseBand::of_the_bake(0.0))
        .unwrap()
        .with_layer(layer(1.0, 3), 0.0)
        .err()
        .unwrap();
    assert!(e.contains("crease band"), "{e}");
    let e = a.with_relief(Arc::new(ReliefLattice::new(channels()).unwrap())).err().unwrap();
    assert!(e.contains("before the crease band"), "{e}");
}

// spec: GEN-102
#[test]
fn the_range_over_a_cap_holds_every_read_in_it_and_nests() {
    let l = layer(1.0, 3);
    let mut rng = SplitMix64(14);
    let mut taken = 0;
    for n in 0..300 {
        let c = random_dir(&mut rng);
        let r = 1e-5 + rng.next_f64() * 0.04 * (f64::from(n % 3) + 0.2);
        let Some((lo, hi)) = l.range_over_cap(c, r).unwrap() else { continue };
        taken += 1;
        for _ in 0..40 {
            let d = offset(c, r * rng.next_f64().sqrt(), rng.next_f64() * std::f64::consts::TAU);
            let v = l.change_at(d).unwrap();
            assert!(v >= lo - 1e-6 && v <= hi + 1e-6, "read {v} outside [{lo}, {hi}] in a cap of {r} at {c:?}");
        }
        let (ir, ic) = (r * 0.3, offset(c, 0.6 * r, rng.next_f64() * std::f64::consts::TAU));
        let (ilo, ihi) = l.range_over_cap(ic, ir).unwrap().expect("an inner cap reaches no more tiles");
        assert!(ilo >= lo - 1e-9 && ihi <= hi + 1e-9, "inner [{ilo}, {ihi}] not within outer [{lo}, {hi}]");
    }
    assert!(taken > 150, "most caps read the pyramids ({taken})");
}

// spec: GEN-102
#[test]
fn a_cap_on_a_face_edge_or_a_cube_corner_reads_both_faces_and_a_wide_cap_takes_the_global_range() {
    let l = layer(1.0, 3);
    let mut rng = SplitMix64(15);
    for centre in [Vec3::new(1.0, 1.0, 1.0).normalized(), Vec3::new(1.0, 0.0, 1.0).normalized(), Vec3::new(0.0, -1.0, 1.0).normalized()] {
        let (lo, hi) = l.range_over_cap(centre, 0.01).unwrap().expect("a small cap");
        for _ in 0..400 {
            let d = offset(centre, 0.01 * rng.next_f64().sqrt(), rng.next_f64() * std::f64::consts::TAU);
            let v = l.change_at(d).unwrap();
            assert!(v >= lo - 1e-6 && v <= hi + 1e-6, "read {v} outside [{lo}, {hi}] on the edge at {centre:?}");
        }
    }
    assert!(
        l.range_over_cap(Vec3::new(0.3, 0.2, 0.9).normalized(), 1.6).unwrap().is_none(),
        "more than {MAX_CAP_TILES} tiles: the caller takes the global range"
    );
    let (glo, ghi) = l.global_range();
    assert!(glo < 0.0 && ghi > 0.0);
}

// spec: STRM-066
#[test]
fn the_pyramid_holds_every_cell_and_each_level_its_children() {
    let src = MemoryLayer::new(1.0, 3);
    let t = src.tile(Face(1), 1, 2).unwrap().expect("a tile");
    let [lo, hi] = t.range();
    for j in 0..TILE {
        for i in 0..TILE {
            let v = t.cell(i, j);
            assert!(v >= lo && v <= hi);
        }
    }
    let core_min = t.result().core.iter().cloned().fold(f32::INFINITY, f32::min);
    let core_max = t.result().core.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert_eq!((lo, hi), (core_min, core_max), "the top node is the tile's own range");
    // A damaged tile is refused, naming it.
    let mut core = t.result().core.clone();
    core[5] = f32::NAN;
    let bad = TileResult { id: t.result().id, core, deposited: Vec::new(), fill: Vec::new(), heights: Vec::new(), stats: zero_stats() };
    let e = LayerTile::new(Arc::new(bad), TILE).err().unwrap();
    assert!(e.contains("layer tile 1/1/2") && e.contains("not finite"), "{e}");
}

// spec: GEN-102
#[test]
fn bounds_hold_every_height_with_the_layer_on_and_nest_on_both_bases() {
    let g = grid();
    let src = GridSource(&g);
    for c1 in [false, true] {
        let f = layered(&src, c1, 1.0);
        let mut rng = SplitMix64(16);
        for n in 0..300 {
            let c = random_dir(&mut rng);
            let r = 1e-4 + rng.next_f64() * 0.02 * (f64::from(n % 4) + 0.3);
            let (lo, hi) = f.height_bounds(c, r);
            for _ in 0..30 {
                let d = offset(c, r * rng.next_f64().sqrt(), rng.next_f64() * std::f64::consts::TAU);
                let h = f.height(d);
                assert!(h >= lo - 1e-3 && h <= hi + 1e-3, "c1 {c1}: height {h} outside [{lo}, {hi}] (cap {r} at {c:?})");
                let h32 = f64::from(h as f32);
                assert!(h32 >= lo - 1e-3 && h32 <= hi + 1e-3, "c1 {c1}: f32 height {h32} outside [{lo}, {hi}]");
            }
            let (ir, ic) = (r * 0.3, offset(c, 0.6 * r, rng.next_f64() * std::f64::consts::TAU));
            let (ilo, ihi) = f.height_bounds(ic, ir);
            assert!(ilo >= lo - 1e-3 && ihi <= hi + 1e-3, "c1 {c1}: inner [{ilo}, {ihi}] not within outer [{lo}, {hi}]");
        }
        assert!(f.error().is_none());
        let global = f.max_abs_height_m();
        let (lo, hi) = f.height_bounds(Vec3::new(0.3, 0.2, 0.9).normalized(), 0.9);
        assert!(lo >= -global && hi <= global, "a wide cap stays inside the planet-wide bound");
    }
}

// spec: GEN-102
#[test]
fn a_spike_within_a_cell_of_a_cap_is_inside_the_cap_s_range_inside_a_face_on_an_edge_and_at_a_corner() {
    // A layer of zeros with one 1000 m cell: a cap of 1e-9 rad around a point within a cell of the spike (whose read the spike moves) must hold that read. A cap that missed the nodes
    // within a cell of it, or the neighbouring face's border cells near an edge, fails here.
    let cases: [(u8, usize, usize); 4] = [(0, 100, 100), (0, 0, 100), (0, 255, 255), (4, 0, 0)];
    for (face, i, j) in cases {
        let l = spike_layer(face, i, j);
        let (ci, cj) = (i as f64, j as f64);
        let mut tested = 0;
        for (dx, dy) in [
            (0.7, 0.0),
            (-0.7, 0.0),
            (0.0, 0.9),
            (0.0, -0.9),
            (0.6, 0.6),
            (-0.6, -0.6),
            (0.9, -0.2),
            (1.4, 0.0),
            (0.0, -1.4),
            (-0.3, 0.3),
            (-1.0, -1.0),
            (-2.5, 0.0),
            (0.0, 2.5),
        ] {
            for beyond in [false, true] {
                let (x, y) = (ci + dx, cj + dy);
                // Past a face edge the point is read on the neighbouring face's plane.
                let x = if beyond && i == 0 { -0.3 + dx.min(0.0) * 0.1 } else { x };
                let d = at_cell(face, x, y);
                let (lo, hi) = l.range_over_cap(d, 1e-9).unwrap().expect("a small cap");
                let v = l.change_at(d).unwrap();
                assert!(v >= lo - 1e-9 && v <= hi + 1e-9, "face {face} spike ({i}, {j}) read {v} at ({x}, {y}) outside [{lo}, {hi}]");
                tested += usize::from(v.abs() > 1.0);
            }
        }
        assert!(tested >= 3, "face {face} spike ({i}, {j}): the cases include reads the spike moves ({tested})");
    }
}

// spec: STRM-066
#[test]
fn the_pyramid_of_a_tile_of_odd_sizes_holds_exactly_its_nodes_at_every_level() {
    // 96 cells (3 blocks: 3, 2, 1 nodes a side) and 100 cells (4 blocks with a short last block), against the range of each node's cells taken directly.
    for cells in [96usize, 100, 40] {
        let core: Vec<f32> =
            (0..cells * cells).map(|k| ((hash_u64s(5, &[cells as u64, k as u64]) >> 40) as f32 / 1000.0) - 8000.0).collect();
        let result = Arc::new(TileResult {
            id: PassTile { face: Face(0), ti: 0, tj: 0 },
            core: core.clone(),
            deposited: Vec::new(),
            fill: Vec::new(),
            heights: Vec::new(),
            stats: zero_stats(),
        });
        let t = LayerTile::new(result, cells).unwrap();
        let block = t.block_cells();
        let mut seen_levels = 0;
        for level in 0..t.levels() {
            let size = block << level;
            let side = cells.div_ceil(size);
            for y in 0..side {
                for x in 0..side {
                    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                    for j in (y * size)..((y + 1) * size).min(cells) {
                        for i in (x * size)..((x + 1) * size).min(cells) {
                            lo = lo.min(core[j * cells + i]);
                            hi = hi.max(core[j * cells + i]);
                        }
                    }
                    assert_eq!(t.node_range(level, x, y), [lo, hi], "{cells} cells, level {level}, node ({x}, {y})");
                }
            }
            seen_levels += 1;
        }
        assert!(
            seen_levels >= 2 && t.node_range(t.levels() - 1, 0, 0) == t.range(),
            "{cells} cells: the last level is one node, the tile's range"
        );
    }
}

// spec: GEN-112
#[test]
fn without_the_relief_channels_the_layer_is_added_to_the_lattice_read_with_the_band_and_the_bounds_hold() {
    let g = grid();
    let src = GridSource(&g);
    let l = layer(1.0, 3);
    for c1 in [false, true] {
        let on = plain(&src, c1).with_layer(l.clone(), 0.0).unwrap();
        let band = plain(&src, c1).with_crease_band(planet_generators::erosion::CreaseBand::of_the_bake(0.0)).unwrap();
        assert!(on.layer_on() && on.crease_band_on() && !on.relief_on());
        let mut rng = SplitMix64(31);
        for _ in 0..1500 {
            let d = random_dir(&mut rng);
            let want = band.height(d) + coast_fade(on.base_elevation(d), 0.0) * l.change_at(d).unwrap();
            assert_eq!(on.height(d).to_bits(), want.to_bits(), "c1 {c1}: k is 1, m unread: the band's height plus the faded change");
        }
        let dirs: Vec<Vec3> = (0..300).map(|_| random_dir(&mut rng)).collect();
        for (d, h) in dirs.iter().zip(on.heights(&dirs)) {
            assert_eq!(on.height(*d).to_bits(), h.to_bits());
        }
        for n in 0..200 {
            let c = random_dir(&mut rng);
            let r = 1e-4 + rng.next_f64() * 0.02 * (f64::from(n % 4) + 0.3);
            let (lo, hi) = on.height_bounds(c, r);
            for _ in 0..20 {
                let d = offset(c, r * rng.next_f64().sqrt(), rng.next_f64() * std::f64::consts::TAU);
                let h = on.height(d);
                assert!(h >= lo - 1e-3 && h <= hi + 1e-3, "c1 {c1}: height {h} outside [{lo}, {hi}]");
            }
            let (ilo, ihi) = on.height_bounds(offset(c, 0.6 * r, rng.next_f64() * std::f64::consts::TAU), r * 0.3);
            assert!(ilo >= lo - 1e-3 && ihi <= hi + 1e-3, "c1 {c1}: inner [{ilo}, {ihi}] not within outer [{lo}, {hi}]");
        }
        assert!(on.error().is_none());
    }
}

/// A channel source of made-up numbers: the deposited depth follows z, the area x, and it can fail at one direction.
struct FakeChannels(u64, bool);

impl planet_generators::channels::ChannelSource for FakeChannels {
    fn identity(&self) -> u64 {
        self.0
    }
    fn area_log2_cells(&self, d: Vec3) -> Result<Option<f32>, String> {
        if self.1 {
            return Err("the area of a face could not be read".into());
        }
        Ok(Some((2.0 + 6.0 * d.x.abs()) as f32))
    }
    fn deposited_depth_m(&self, d: Vec3) -> Result<Option<f32>, String> {
        Ok(if d.z < 0.0 { None } else { Some((0.5 + 3.0 * d.z) as f32) })
    }
    fn fill_depth_m(&self, d: Vec3) -> Result<Option<f32>, String> {
        Ok(if d.z < 0.0 { None } else { Some((0.2 + d.x.abs()) as f32) })
    }
    fn erosion_depth_m(&self, d: Vec3) -> Result<Option<f32>, String> {
        Ok(if d.z < 0.0 { None } else { Some((1.0 + d.y.abs()) as f32) })
    }
}

// spec: GEN-119, GEN-126
#[test]
fn the_eroded_field_reads_the_channels_for_the_driver_block_and_changes_nothing_else_but_its_version_word() {
    let grid = grid();
    let g = GridSource(&grid);
    let (off, on) = (layered(&g, true, 1.0), layered(&g, true, 1.0).with_channels(Arc::new(FakeChannels(5, false))));
    let dirs: Vec<Vec3> = (0..40)
        .map(|k| face_to_direction(&TangentWarp, Face((k % 6) as u8), -0.9 + 0.045 * f64::from(k), 0.3 - 0.03 * f64::from(k)))
        .collect();
    assert_eq!(off.driver_channels(&dirs), None, "a field without channels fills no driver block");
    let samples = on.driver_channels(&dirs).expect("channels");
    assert_eq!(samples.len(), dirs.len());
    for (d, s) in dirs.iter().zip(&samples) {
        assert_eq!(s.area_log2_cells, Some((2.0 + 6.0 * d.x.abs()) as f32));
        assert_eq!(s.soil_m.is_some(), d.z >= 0.0, "a point the store holds no depth for has none");
        // GEN-126: the fill depth and the erosion depth are read from their own channels, not swapped and not the deposit.
        let (fill, erosion) = (((0.2 + d.x.abs()) as f32), ((1.0 + d.y.abs()) as f32));
        assert_eq!((s.fill_m, s.erosion_m), if d.z >= 0.0 { (Some(fill), Some(erosion)) } else { (None, None) });
    }
    // The heights, their keys and the climate are the field's own; the version word (so every mesh key) and nothing else changes with the channels, and with their identity.
    assert_eq!(off.heights(&dirs), on.heights(&dirs));
    assert_eq!((off.inputs_hash(), off.implementation_id(), off.version()), (on.inputs_hash(), on.implementation_id(), on.version()));
    assert_ne!(off.climate_version_word(), on.climate_version_word());
    let other = layered(&g, true, 1.0).with_channels(Arc::new(FakeChannels(6, false)));
    assert_ne!(on.climate_version_word(), other.climate_version_word());
    // A failed read is recorded like a height's and leaves the texel without that channel.
    let broken = layered(&g, true, 1.0).with_channels(Arc::new(FakeChannels(5, true)));
    let s = broken.driver_channels(&dirs[..3]).unwrap();
    assert!(s.iter().all(|s| s.area_log2_cells.is_none()));
    assert!(broken.error().is_some_and(|e| e.contains("could not be read")), "{:?}", broken.error());
}

// spec: GEN-188
#[test]
fn the_read_has_no_step_across_a_cell_centre_line() {
    // The noisy layer (cell-scale noise of 25 m): a read clamped to the four cells around the point stepped where the set of four changes, at every cell-centre line where the
    // interpolant along the line overshoots on one side only (the #108 finding: 1.5 % of crossings over 0.1 m on R3a, up to 12.5 m).
    let l = layer(1.0, 3);
    let mut rng = SplitMix64(108);
    let e = 1e-7;
    let mut worst = (0.0f64, 0u8, 0.0f64, 0.0f64);
    for k in 0..20_000 {
        let face = (rng.next_f64() * 6.0) as u8;
        // Away from the cube edges (the cross-fade with the neighbouring face is tested on its own).
        let (x, y) = (4.0 + rng.next_f64() * 247.0, 4.0 + rng.next_f64() * 247.0);
        let jump = if k % 2 == 0 {
            let yi = libm::floor(y);
            (l.change_at(at_cell(face, x, yi + e)).unwrap() - l.change_at(at_cell(face, x, yi - e)).unwrap()).abs()
        } else {
            let xi = libm::floor(x);
            (l.change_at(at_cell(face, xi + e, y)).unwrap() - l.change_at(at_cell(face, xi - e, y)).unwrap()).abs()
        };
        if jump > worst.0 {
            worst = (jump, face, x, y);
        }
    }
    assert!(worst.0 <= 1e-3, "a jump of {} m across a cell-centre line on face {} near cell ({}, {})", worst.0, worst.1, worst.2, worst.3);
}

// spec: GEN-188
#[test]
fn the_read_at_a_cell_centre_is_the_stored_sample_bit_for_bit() {
    let l = layer(1.0, 3);
    let mut rng = SplitMix64(1880);
    for _ in 0..2000 {
        let (f, i, j) = ((rng.next_f64() * 6.0) as u8, 3 + (rng.next_f64() * 250.0) as usize, 3 + (rng.next_f64() * 250.0) as usize);
        let v = l.change_at(cell_dir(f, i, j)).unwrap();
        let want = f64::from(synthetic(1.0, 3, f, i, j) as f32);
        assert_eq!(v.to_bits(), want.to_bits(), "face {f} cell ({i}, {j}): read {v}, sample {want}");
    }
}
