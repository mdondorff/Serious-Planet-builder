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

const KNOWN_ANSWER: u64 = 2_924_459_866_857_506_201;

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
    let delta = planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|_| false);
    assert!(delta[spike] < 0.0, "the spike gives");
    for k in 0..8 {
        assert!(delta[r.neighbours[spike][k] as usize] > 0.0, "neighbour {k} takes");
    }
    assert!(delta.iter().sum::<f64>().abs() < 1e-9, "what one cell gives the others take: {}", delta.iter().sum::<f64>());
    // A protected neighbour takes nothing, and the pair with it is skipped for the spike too.
    let q = r.neighbours[spike][2] as usize;
    let delta = planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|c| c == q);
    assert_eq!(delta[q], 0.0);
    assert!(delta.iter().sum::<f64>().abs() < 1e-9);
    // Below the angle of repose nothing moves.
    z[spike] = 0.5;
    assert!(planet_generators::anet::slump_deltas(&z, &r.neighbours, &dist, cells, 0.9, 0.09, &|_| false).iter().all(|d| *d == 0.0));
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
