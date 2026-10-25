//! GEN-048 to GEN-050: the A-net incision: no uphill channel cell after the breach, jittered receivers, the halo of a chunk, determinism.

use std::sync::{Arc, OnceLock};

use planet_core::{Face, TileId, Vec3};
use planet_generators::anet::{ANet, ANetParams, BaseDetail, CoarseRun};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{region, ChunkLattice, ChunkStage};
use planet_generators::coupled::{Lattice, NONE};

const R: f64 = 6_371_000.0;
const N: usize = 2048;
const FACE: usize = 2;
const PER: u32 = 32;

fn lattice() -> &'static Lattice {
    static L: OnceLock<Lattice> = OnceLock::new();
    L.get_or_init(|| Lattice::new(N, R))
}

/// A river on face 2, row 1000, from column 995 to 1010 with a growing area.
struct River;

fn on_river(c: usize) -> Option<usize> {
    let (f, r) = (c / (N * N), c % (N * N));
    let (i, j) = (r % N, r / N);
    (f == FACE && j == 1000 && (995..=1010).contains(&i)).then_some(i)
}

impl CoarseRun for River {
    fn lattice(&self) -> &Lattice {
        lattice()
    }
    fn z(&self, c: usize) -> f32 {
        400.0 - 0.8 * ((c % N) as f32 - 995.0)
    }
    fn area_m2(&self, c: usize) -> f32 {
        on_river(c).map_or(1.0e6, |i| 2.0e8 * (1 + i - 995) as f32)
    }
    fn identity(&self) -> u64 {
        7
    }
    fn receiver(&self, c: usize) -> u32 {
        match on_river(c) {
            Some(i) if i < 1010 => (c + 1) as u32,
            _ => NONE,
        }
    }
}

/// Rolling terrain with hills of a few hundred metres and a ridge across the river's path (so the corridor has to run uphill somewhere).
struct Hills;

impl BaseDetail for Hills {
    fn base_height(&self, dir: Vec3) -> f64 {
        let p = dir * R;
        300.0
            + 45.0 * libm::sin(p.x / 1_900.0 + 0.7 * p.y / 2_600.0 + 1.3)
            + 40.0 * libm::cos(p.y / 1_700.0 - p.z / 2_300.0)
            + 35.0 * libm::sin((p.x + p.z) / 3_100.0)
            + 6.0 * libm::sin(p.x / 230.0 + p.y / 310.0) * libm::cos(p.z / 270.0)
    }
    fn identity(&self) -> u64 {
        11
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        self.base_height(dir)
    }
}

fn stage(jitter: f64, halo_rings: u16) -> ANet {
    let lat = ChunkLattice::new(PER).unwrap();
    let mut params = ANetParams::spike(3_000.0, lat.cell_m(R));
    params.jitter = jitter;
    params.halo_rings = halo_rings;
    ANet { run: Arc::new(River), base: Arc::new(Hills), radius_m: R, params }
}

fn chunk(x: u32) -> TileId {
    TileId::new(Face(FACE as u8), CHUNK_GRID_LEVEL, x, 1000).unwrap()
}

// spec: GEN-048
#[test]
fn after_the_incision_no_channel_cell_has_a_higher_receiver_and_the_breach_did_cut_the_path() {
    let lat = ChunkLattice::new(PER).unwrap();
    let a = stage(0.4, 20);
    let r = region(&lat, chunk(1003), 20);
    let s = a.solve(&lat, &r);
    let (mut channels, mut uphill_before, mut cut) = (0, 0, 0);
    for c in 0..r.cells.len() {
        let rec = s.receiver[c];
        if r.depth[c] >= r.halo || rec == u32::MAX {
            continue;
        }
        let rec = rec as usize;
        if s.z_base[rec] > s.z_base[c] {
            uphill_before += 1;
            cut += usize::from(s.z[rec] < s.z_base[rec] - 1.0e-9);
        }
        if s.area_m2[c] > a.params.channel_area_m2 {
            channels += 1;
            assert!(s.z[rec] <= s.z[c] + 1.0e-6, "channel cell {:?} drains uphill: {} m into {} m", r.cells[c], s.z[c], s.z[rec]);
        }
    }
    assert!(channels > 100, "{channels} channel cells");
    assert!(uphill_before > 5 && cut > 0, "the terrain made the corridor run uphill in {uphill_before} places and the breach cut {cut}");
}

// spec: GEN-050
#[test]
fn jittered_receivers_lean_less_on_the_axes_than_plain_d8_and_are_a_function_of_position_alone() {
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(1003), 20);
    let share = |jitter: f64| {
        let s = stage(jitter, 20).solve(&lat, &r);
        let (mut axis, mut n) = (0usize, 0usize);
        for c in 0..r.cells.len() {
            if r.depth[c] >= r.halo || s.receiver[c] == u32::MAX {
                continue;
            }
            let k = r.neighbours[c].iter().position(|&x| x == s.receiver[c]).expect("a neighbour");
            n += 1;
            axis += usize::from(k < 4);
        }
        (axis as f64 / n as f64, n)
    };
    let (plain, n) = share(0.0);
    let (jittered, _) = share(0.4);
    println!("axis share over {n} receivers: D8 {plain:.3}, jitter 0.4 {jittered:.3}");
    assert!(jittered < plain, "the jitter lowers the axis share");
    assert!(jittered <= 0.56, "axis share with the jitter {jittered}");
    // Two chunks that share cells choose the same receiver for them.
    let other = region(&lat, chunk(1004), 20);
    let (sa, sb) = (stage(0.4, 20).solve(&lat, &r), stage(0.4, 20).solve(&lat, &other));
    let mut compared = 0;
    for (ia, &cell) in r.cells.iter().enumerate() {
        let Some(ib) = other.index_of(cell) else { continue };
        if r.depth[ia] + 8 >= r.halo || other.depth[ib] + 8 >= other.halo {
            continue;
        }
        let to = |s: &planet_generators::anet::Solution, rg: &planet_generators::chunkfield::Region, i: usize| {
            (s.receiver[i] != u32::MAX).then(|| rg.cells[s.receiver[i] as usize])
        };
        assert_eq!(to(&sa, &r, ia), to(&sb, &other, ib), "the receiver of {cell:?}");
        compared += 1;
    }
    assert!(compared > 500, "{compared} shared cells compared");
}

/// The change of the core of chunk `x` with `halo` rings.
fn core_change(x: u32, halo: u16) -> Vec<f32> {
    let lat = ChunkLattice::new(PER).unwrap();
    let a = stage(0.4, halo);
    let r = region(&lat, chunk(x), halo);
    a.compute(&lat, &r)[..r.core].to_vec()
}

// spec: GEN-049
#[test]
fn a_chunk_with_the_stated_halo_equals_the_same_run_with_a_wider_one_in_every_core_cell() {
    let truth = core_change(1003, 70);
    let mut table = Vec::new();
    for halo in [4u16, 10, 20, 40, 55] {
        let c = core_change(1003, halo);
        let differing = c.iter().zip(&truth).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        let worst = c.iter().zip(&truth).map(|(a, b)| f64::from((a - b).abs())).fold(0.0, f64::max);
        table.push((halo, differing, worst));
    }
    println!("halo (rings) / core cells differing of {} / largest difference m: {table:?}", truth.len());
    // The synthetic hills have features of 2 to 3 km, so a catchment can be longer than 20 rings (3 km): the stated halo here is 40 rings.
    let stated = stage(0.4, 40).halo_rings();
    assert_eq!(stated, 40);
    let at_stated = table.iter().find(|t| t.0 == stated).unwrap();
    assert_eq!(at_stated.1, 0, "the stated halo of {stated} rings leaves {} core cells different (up to {} m)", at_stated.1, at_stated.2);
    assert!(table.iter().find(|t| t.0 == 4).unwrap().1 > 0, "a halo of 4 rings is too short to show no difference");
}

// spec: GEN-048, GEN-050
#[test]
fn the_stage_is_a_pure_function_of_the_chunk_and_its_definition_hash_covers_its_parameters() {
    let a = core_change(1003, 20);
    assert_eq!(a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), core_change(1003, 20).iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    assert!(a.iter().any(|v| *v != 0.0), "the incision changes the surface");
    let base = stage(0.4, 20).definition_hash();
    assert_ne!(base, stage(0.5, 20).definition_hash());
    assert_ne!(base, stage(0.4, 21).definition_hash());
    assert_eq!(base, stage(0.4, 20).definition_hash());
}

/// The cost of one chunk at 8 m cells with the 3 km halo (design.md decision 6): `cargo test --release -p planet-generators --test it
/// the_cost_of_a_chunk -- --ignored --nocapture`. A measurement, never a test (CON-20).
#[test]
#[ignore]
fn the_cost_of_a_chunk_at_8_m_cells_and_a_3_km_halo() {
    use std::time::Instant;
    let lat = ChunkLattice::new(608).unwrap();
    let mut params = ANetParams::spike(3_000.0, lat.cell_m(R));
    params.jitter = 0.4;
    let a = ANet { run: Arc::new(River), base: Arc::new(Hills), radius_m: R, params };
    let t = Instant::now();
    let r = region(&lat, chunk(1003), a.halo_rings());
    let t_region = t.elapsed();
    let t = Instant::now();
    let net = planet_generators::anet::Network::build(&River, &Hills, &lat, &r, R);
    let t_net = t.elapsed();
    let t = Instant::now();
    let change = a.compute(&lat, &r);
    let t_all = t.elapsed();
    println!(
        "cell {:.2} m, halo {} rings, {} cells: region {:?}, network {:?}, whole compute (network again included) {:?}, {} corridor cells",
        lat.cell_m(R),
        a.halo_rings(),
        r.cells.len(),
        t_region,
        t_net,
        t_all,
        net.corridor.iter().filter(|c| **c).count()
    );
    assert_eq!(change.len(), r.cells.len());
}

// spec: GEN-048, GEN-049
#[test]
fn a_chunk_of_the_synthetic_scene_keeps_its_known_answer() {
    let fnv = |v: &[f32]| {
        v.iter()
            .flat_map(|x| x.to_bits().to_le_bytes())
            .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
    };
    assert_eq!(fnv(&core_change(1003, 40)), KNOWN_ANSWER, "the stage changed: platform, order or arithmetic");
}

const KNOWN_ANSWER: u64 = 15_453_545_016_494_209_173;

/// A gentle valley along the river (slope 0.001, below the breach slope) plus a little noise: every stream runs to the river.
struct Plane;

impl BaseDetail for Plane {
    fn base_height(&self, dir: Vec3) -> f64 {
        let p = dir * R;
        let (_, _, t) = planet_core::cube::direction_to_face(&planet_core::cube::TangentWarp, dir);
        let axis = -1.0 + (2.0 * 1000.5) / N as f64;
        300.0 + 0.001 * (t - axis).abs() * R * core::f64::consts::FRAC_PI_4 + 2.0 * libm::sin(p.y / 140.0) * libm::cos(p.z / 170.0)
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        self.base_height(dir)
    }
    fn identity(&self) -> u64 {
        13
    }
}

/// The base is the run plus 50 m: smooth, so anchors are the run's heights and the base is above them.
struct AboveRun;

impl BaseDetail for AboveRun {
    fn base_height(&self, dir: Vec3) -> f64 {
        let c = lattice().cell_at(dir);
        f64::from(River.z(c)) + 50.0
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        self.base_height(dir)
    }
    fn identity(&self) -> u64 {
        17
    }
}

/// A 600 m ridge across the river's path.
struct Ridge;

impl BaseDetail for Ridge {
    fn base_height(&self, dir: Vec3) -> f64 {
        // A ridge one coarse cell wide at column 1003: 600 m high.
        let across = ((lattice().cell_at(dir) % N) as f64 - 1003.0) / 0.4;
        100.0 + 600.0 * libm::exp(-(across * across))
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        self.base_height(dir)
    }
    fn identity(&self) -> u64 {
        19
    }
}

fn stage_over(base: Arc<dyn BaseDetail>, halo_rings: u16) -> ANet {
    let lat = ChunkLattice::new(PER).unwrap();
    let mut params = ANetParams::spike(3_000.0, lat.cell_m(R));
    params.halo_rings = halo_rings;
    ANet { run: Arc::new(River), base, radius_m: R, params }
}

fn max_difference(a: &ANet, b: &ANet, x: u32, ha: u16, hb: u16) -> f64 {
    let lat = ChunkLattice::new(PER).unwrap();
    let (ra, rb) = (region(&lat, chunk(x), ha), region(&lat, chunk(x), hb));
    let (ca, cb) = (a.compute(&lat, &ra), b.compute(&lat, &rb));
    ca[..ra.core].iter().zip(&cb[..rb.core]).map(|(p, q)| f64::from((p - q).abs())).fold(0.0, f64::max)
}

// spec: GEN-049
#[test]
fn gentle_terrain_below_the_breach_slope_does_not_make_a_core_depend_on_how_far_the_region_reaches() {
    let a = stage_over(Arc::new(Plane), 20);
    let b = stage_over(Arc::new(Plane), 40);
    let worst = max_difference(&a, &b, 1003, 20, 40);
    assert!(worst <= 0.02, "halo 20 against 40 on a valley: {worst} m");
}

// spec: GEN-048
#[test]
fn a_river_over_a_smooth_base_keeps_its_links_and_its_area_flows_on_past_the_anchors() {
    let lat = ChunkLattice::new(PER).unwrap();
    let a = stage_over(Arc::new(AboveRun), 40);
    let r = region(&lat, chunk(1003), 40);
    let net = planet_generators::anet::Network::build(&River, &AboveRun, &lat, &r, R);
    let s = a.solve(&lat, &r);
    let mut with_link = 0;
    for (c, &on) in net.corridor.iter().enumerate() {
        if on && r.depth[c] < r.halo - 1 && net.next[c] != u32::MAX && s.receiver[c] != u32::MAX {
            with_link += 1;
        } else if on && r.depth[c] + 2 < r.halo && net.next[c] != u32::MAX {
            panic!("corridor cell {:?} lost its receiver (a pit)", r.cells[c]);
        }
    }
    // The area along the corridor never falls downstream (a link joins a cell to the next one on the path), and it reaches the river's size.
    let mut max_area = 0.0f64;
    for c in 0..r.cells.len() {
        if net.corridor[c] && net.next[c] != u32::MAX && r.depth[c] + 2 < r.halo {
            let next = net.next[c] as usize;
            assert!(
                s.area_m2[next] >= s.area_m2[c] * 0.99,
                "area fell from {} to {} between {:?} and {:?}",
                s.area_m2[c],
                s.area_m2[next],
                r.cells[c],
                r.cells[next]
            );
            max_area = max_area.max(s.area_m2[c]);
        }
    }
    let prev_area = max_area;
    assert!(with_link > 100, "{with_link} corridor cells with a link");
    assert!(prev_area > 1.0e8, "the river's area {prev_area} m2 reaches the downstream end");
}

// spec: GEN-039, GEN-048
#[test]
fn the_change_never_exceeds_the_stages_stated_bound_even_across_a_high_ridge() {
    let a = stage_over(Arc::new(Ridge), 30);
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(1003), 30);
    let change = a.compute(&lat, &r);
    let bound = a.bound_m() as f32;
    assert!(change.iter().all(|c| c.abs() <= bound), "a change beyond {bound} m");
    assert!(
        change.iter().any(|c| c.abs() > 50.0),
        "the ridge is cut: the largest change is {}",
        change.iter().fold(0.0f32, |m, c| m.max(c.abs()))
    );
}

// spec: GEN-051
#[test]
fn slumping_moves_the_excess_to_the_lower_neighbours_and_conserves_what_it_moves() {
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(1003), 6);
    let (z0, dist) = (vec![0.0f64; r.cells.len()], vec![[1.0f64; 8]; r.cells.len()]);
    // A spike of 50 over a unit grid is far over the angle of repose (0.9).
    let spike = r.index_of(planet_generators::chunkfield::Cell { face: 2, i: 32110, j: 32010 }).unwrap();
    let mut z = z0.clone();
    z[spike] = 50.0;
    let cells = r.cells.len().min(r.depth.partition_point(|&d| d <= 4));
    let delta = planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|_| false, &|c| c as u64);
    assert!(delta[spike] < 0.0, "the spike gives");
    for k in 0..8 {
        assert!(delta[r.neighbours[spike][k] as usize] > 0.0, "neighbour {k} takes");
    }
    assert!(delta.iter().sum::<f64>().abs() < 1e-9, "what one cell gives the others take: {}", delta.iter().sum::<f64>());
    // A protected neighbour takes nothing, and the pair with it is skipped for the spike too.
    let q = r.neighbours[spike][2] as usize;
    let delta = planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|c| c == q, &|c| c as u64);
    assert_eq!(delta[q], 0.0);
    assert!(delta.iter().sum::<f64>().abs() < 1e-9);
    // Below the angle of repose nothing moves.
    z[spike] = 0.5;
    assert!(planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|_| false, &|c| c as u64)
        .iter()
        .all(|d| *d == 0.0));
}

// spec: GEN-051
#[test]
fn channels_the_corridor_and_fixed_cells_are_untouched_by_the_hillslope_processes_and_the_slopes_are_not() {
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(1003), 40);
    let with = stage(0.4, 40);
    let mut off = stage(0.4, 40);
    off.params.diffusion = 0.0;
    off.params.slump_rate = 0.0;
    let (a, b) = (with.solve(&lat, &r), off.solve(&lat, &r));
    let net = planet_generators::anet::Network::build(&River, &Hills, &lat, &r, R);
    let (mut changed, mut kept) = (0, 0);
    for c in 0..r.cells.len() {
        if r.depth[c] > 30 {
            continue;
        }
        let exempt = net.corridor[c] || !net.anchor_z[c].is_nan() || a.area_m2[c] > with.params.channel_area_m2;
        if exempt {
            assert_eq!(a.z[c].to_bits(), b.z[c].to_bits(), "an exempt cell {:?} moved", r.cells[c]);
            kept += 1;
        } else if a.z[c] != b.z[c] {
            changed += 1;
        }
    }
    assert!(kept > 100 && changed > 100, "{kept} cells kept, {changed} slopes smoothed");
}

// spec: GEN-051
#[test]
fn slumping_conserves_what_it_moves_next_to_a_cube_corner_where_the_neighbour_table_is_not_symmetric() {
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, TileId::new(Face(0), CHUNK_GRID_LEVEL, 0, 0).unwrap(), 6);
    let (mut z, dist) = (vec![0.0f64; r.cells.len()], vec![[1.0f64; 8]; r.cells.len()]);
    z[0] = 50.0;
    let cells = r.depth.partition_point(|&d| d <= 4);
    let delta = planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|_| false, &|c| r.cells[c].key());
    assert!(delta[0] < 0.0 && delta.iter().sum::<f64>().abs() < 1e-9, "sum {}", delta.iter().sum::<f64>());
}

// spec: GEN-051
#[test]
fn a_chunk_with_no_halo_does_not_index_a_missing_neighbour() {
    let lat = ChunkLattice::new(PER).unwrap();
    for halo in [0u16, 1] {
        let r = region(&lat, chunk(1003), halo);
        let s = stage(0.4, halo.max(1)).solve(&lat, &r);
        assert_eq!(s.z.len(), r.cells.len());
    }
}

/// The hashes of the changes of `tiles`, computed on `threads` threads.
fn hashes_on(threads: usize, tiles: &[TileId]) -> Vec<u64> {
    let lat = ChunkLattice::new(PER).unwrap();
    let per = tiles.len().div_ceil(threads);
    let mut out = vec![0u64; tiles.len()];
    std::thread::scope(|s| {
        for (lane, slot) in out.chunks_mut(per).enumerate() {
            s.spawn(move || {
                let a = stage(0.4, 24);
                for (k, h) in slot.iter_mut().enumerate() {
                    let r = region(&lat, tiles[lane * per + k], 24);
                    let change = a.compute(&lat, &r);
                    *h = change[..r.core]
                        .iter()
                        .flat_map(|x| x.to_bits().to_le_bytes())
                        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3));
                }
            });
        }
    });
    out
}

// spec: GEN-051, GEN-038
#[test]
fn the_changes_of_eight_chunks_are_the_same_on_one_two_and_eight_threads() {
    let tiles: Vec<TileId> = (1000..1008).map(chunk).collect();
    let one = hashes_on(1, &tiles);
    assert_eq!(one, hashes_on(2, &tiles), "1 against 2 threads");
    assert_eq!(one, hashes_on(8, &tiles), "1 against 8 threads");
    assert!(one.windows(2).any(|w| w[0] != w[1]), "the chunks differ");
}

/// The real high ground: the base 2,600 m above the run (the run lies 1.2 to 2.7 km under the base where the relief is high).
struct HighBase;

impl BaseDetail for HighBase {
    fn base_height(&self, dir: Vec3) -> f64 {
        f64::from(River.z(lattice().cell_at(dir))) + 2_600.0
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        self.base_height(dir)
    }
    fn identity(&self) -> u64 {
        23
    }
}

// spec: GEN-057
#[test]
fn the_anchors_lie_a_depth_under_the_drawn_ground_that_grows_with_area_and_no_core_cell_reaches_the_clamp() {
    use planet_generators::anet::{AnchorLaw, Network};
    let law = AnchorLaw::default();
    // The law: shallow for a small river, deeper for a larger one, capped well inside the stage's bound.
    let depths: Vec<f64> = [1.0e8, 1.0e9, 1.0e10, 1.0e11, 1.0e13].iter().map(|&a| law.depth_m(a)).collect();
    assert!(depths.windows(2).all(|w| w[1] >= w[0]) && depths[3] > depths[0] + 20.0, "{depths:?}");
    assert_eq!(depths[4], law.cap_m);
    let lat = ChunkLattice::new(PER).unwrap();
    for x in [1000, 1008] {
        let a = stage_over(Arc::new(HighBase), 30);
        let r = region(&lat, chunk(x), 30);
        assert!(law.cap_m * 2.0 < a.bound_m(), "the cap is well inside the bound");
        let net = Network::build(&River, &HighBase, &lat, &r, R);
        let anchored: Vec<usize> = (0..r.cells.len()).filter(|&c| !net.anchor_z[c].is_nan()).collect();
        assert!(!anchored.is_empty(), "chunk {x} has anchors");
        for &c in &anchored {
            // The base falls with the river here, so the running maximum is each anchor's own value: under the ground by the law, at most the cap.
            let depth = net.base_z[c] - net.anchor_z[c];
            assert!(depth >= -1e-9 && depth <= law.cap_m + 1e-9, "anchor {depth} m under the ground, cap {}", law.cap_m);
        }
        let s = a.solve(&lat, &r);
        let deepest = (0..r.core).map(|c| s.z_base[c] - s.z[c]).fold(0.0f64, f64::max);
        assert!(deepest < a.bound_m() * 0.5, "chunk {x}: the deepest cut is {deepest} m, the clamp {}", a.bound_m());
    }
}

// spec: GEN-049, GEN-057
#[test]
fn the_anchors_of_a_cell_are_the_same_whatever_the_halo_over_ground_that_is_not_monotone() {
    use planet_generators::anet::Network;
    let lat = ChunkLattice::new(PER).unwrap();
    for x in [1000, 1003, 1008] {
        let (ra, rb) = (region(&lat, chunk(x), 10), region(&lat, chunk(x), 30));
        let (na, nb) = (Network::build(&River, &Hills, &lat, &ra, R), Network::build(&River, &Hills, &lat, &rb, R));
        let mut shared = 0;
        for (i, cell) in ra.cells.iter().enumerate() {
            let Some(j) = rb.index_of(*cell) else { continue };
            assert_eq!(na.anchor_z[i].to_bits(), nb.anchor_z[j].to_bits(), "chunk {x}, cell {cell:?}");
            shared += usize::from(!na.anchor_z[i].is_nan());
        }
        assert!(shared > 0 || x == 1000, "chunk {x}: anchors compared: {shared}");
    }
}

/// The steep edge pairs of a solved chunk: (pairs of a protected cell and a free cell, those over 0.9, pairs of two free cells over 0.9).
fn bank_census(a: &ANet, base: &dyn BaseDetail, x: u32) -> (usize, usize, usize) {
    use planet_generators::anet::Network;
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(x), 30);
    let net = Network::build_with(&River, base, &lat, &r, R, &a.params.anchor);
    let s = a.solve(&lat, &r);
    let cell_m = lat.cell_m(R);
    let protected = |c: usize| s.area_m2[c] > a.params.channel_area_m2 || net.corridor[c] || !net.anchor_z[c].is_nan();
    let (mut banks, mut steep_banks, mut steep_free, mut under_banks) = (0, 0, 0, 0);
    for c in 0..r.core {
        for k in 0..4 {
            let q = r.neighbours[c][k] as usize;
            if q >= r.core {
                continue;
            }
            let steep = (s.z[c] - s.z[q]).abs() / cell_m > 0.9;
            match (protected(c), protected(q)) {
                (true, false) | (false, true) => {
                    // A bank: the free cell stands over the protected one (a free cell under a dammed channel is not a wall to relax).
                    let (free, fixed) = if protected(c) { (q, c) } else { (c, q) };
                    if s.z[free] > s.z[fixed] {
                        banks += 1;
                        steep_banks += usize::from(steep);
                    } else {
                        under_banks += usize::from(steep);
                    }
                }
                (false, false) => steep_free += usize::from(steep),
                (true, true) => {}
            }
        }
    }
    eprintln!("free cells under a protected one over 0.9: {under_banks}");
    (banks, steep_banks, steep_free)
}

// spec: GEN-051, GEN-058
#[test]
fn the_bank_of_a_channel_is_held_to_the_angle_of_repose_relative_to_the_channel_cell() {
    // Without the settle the breach leaves walls over the repose slope next to the channel cells ...
    let mut off = stage_over(Arc::new(Ridge), 30);
    off.params.bank_rounds = 0;
    let (banks_off, steep_off, _) = bank_census(&off, &Ridge, 1003);
    assert!(banks_off > 100 && steep_off > 10, "without the settle: {steep_off} of {banks_off} bank pairs are over the repose slope");
    // ... and with it no bank is over it; the channel cells did not move.
    let on = stage_over(Arc::new(Ridge), 30);
    let (banks, steep, _) = bank_census(&on, &Ridge, 1003);
    assert!(banks > 100, "{banks} bank pairs");
    // A free cell between a high bed and a low one stays over the low one's cone (it is not lowered under the high bed by more than the repose slope).
    assert!(steep * 20 <= banks, "{steep} of {banks} bank pairs are over the angle of repose");
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(1003), 30);
    let (a, b) = (on.solve(&lat, &r), off.solve(&lat, &r));
    let net = planet_generators::anet::Network::build_with(&River, &Ridge, &lat, &r, R, &on.params.anchor);
    let mut moved = 0;
    for c in 0..r.core {
        let exempt = net.corridor[c] || !net.anchor_z[c].is_nan() || a.area_m2[c] > on.params.channel_area_m2;
        if exempt {
            assert_eq!(a.z[c].to_bits(), b.z[c].to_bits(), "a protected cell {:?} moved", r.cells[c]);
        } else if a.z[c] < b.z[c] {
            moved += 1;
        }
        assert!(a.z[c] <= b.z[c], "the settle only lowers: {:?}", r.cells[c]);
    }
    assert!(moved > 50, "{moved} bank cells came down");
}

// spec: GEN-049, GEN-058
#[test]
fn the_settled_banks_do_not_depend_on_how_far_the_region_reaches_and_make_no_new_wall_below_a_channel() {
    // The same stated halo's reach over a wider region: bit for bit the same core.
    let (mut a, mut b) = (stage_over(Arc::new(Ridge), 30), stage_over(Arc::new(Ridge), 30));
    a.params.bank_rounds = 14;
    b.params.bank_rounds = 14;
    assert_eq!(max_difference(&a, &b, 1003, 30, 70), 0.0);
    // The settle makes no wall under a protected cell that the incision did not leave: free cells more than 0.9 under a protected neighbour.
    let lat = ChunkLattice::new(PER).unwrap();
    let r = region(&lat, chunk(1003), 30);
    let (on, mut off) = (stage_over(Arc::new(Ridge), 30), stage_over(Arc::new(Ridge), 30));
    off.params.bank_rounds = 0;
    let net = planet_generators::anet::Network::build_with(&River, &Ridge, &lat, &r, R, &on.params.anchor);
    let under = |a: &ANet| {
        let s = a.solve(&lat, &r);
        let protected = |c: usize| s.area_m2[c] > a.params.channel_area_m2 || net.corridor[c] || !net.anchor_z[c].is_nan();
        let mut n = 0;
        for c in 0..r.core {
            for k in 0..4 {
                let q = r.neighbours[c][k] as usize;
                if q < r.core && protected(q) && !protected(c) && s.z[q] - s.z[c] > 0.9 * lat.cell_m(R) {
                    n += 1;
                }
            }
        }
        n
    };
    assert!(under(&on) <= under(&off), "the settle made walls below protected cells: {} against {}", under(&on), under(&off));
}
