//! GEN-047: the global network in the chunk raster: one corridor path for neighbouring chunks, zero meander at the coarse cell centres,
//! inflow where a river enters the region, and initial heights from the run plus the base's detail.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::anet::{coarse_z_at, meander_m, segment_points, BaseDetail, CoarseRun, Network, MEANDER_M, NO_NEXT};
use planet_generators::chunk::CHUNK_GRID_LEVEL;
use planet_generators::chunkfield::{region, ChunkLattice};
use planet_generators::coupled::{Lattice, NONE};

const R: f64 = 6_371_000.0;
const N: usize = 2048;

fn lattice() -> &'static Lattice {
    static L: OnceLock<Lattice> = OnceLock::new();
    L.get_or_init(|| Lattice::new(N, R))
}

/// A run with one river on face 2, row 1000, flowing towards +i from column 995 to 1010 with a growing area; heights rise with `i`.
struct River(usize);

const FACE: usize = 2;

fn on_river(row: usize, c: usize) -> Option<usize> {
    let (f, r) = (c / (N * N), c % (N * N));
    let (i, j) = (r % N, r / N);
    (f == FACE && j == row && (995..=1010).contains(&i)).then_some(i)
}

impl CoarseRun for River {
    fn lattice(&self) -> &Lattice {
        lattice()
    }
    fn z(&self, c: usize) -> f32 {
        50.0 + 0.01 * (c % N) as f32
    }
    fn area_m2(&self, c: usize) -> f32 {
        on_river(self.0, c).map_or(1.0e6, |i| 2.0e8 * (1 + i - 995) as f32)
    }
    fn identity(&self) -> u64 {
        7
    }
    fn receiver(&self, c: usize) -> u32 {
        match on_river(self.0, c) {
            Some(i) if i < 1010 => (c + 1) as u32,
            _ => NONE,
        }
    }
}

/// The base is the run's input plus a constant detail of 7 m, so the initial heights are the run's plus 7.
struct Detail(f64);

impl BaseDetail for Detail {
    fn base_height(&self, dir: Vec3) -> f64 {
        self.coarse_input(dir) + self.0
    }
    fn identity(&self) -> u64 {
        11
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        100.0 + 3.0 * dir.x
    }
}

fn chunk(x: u32) -> TileId {
    TileId::new(Face(FACE as u8), CHUNK_GRID_LEVEL, x, 1000).unwrap()
}

// spec: GEN-047
#[test]
fn neighbouring_chunks_find_the_same_corridor_and_the_same_initial_heights_through_their_shared_border() {
    let lat = ChunkLattice::new(32).unwrap();
    let (ra, rb) = (region(&lat, chunk(1000), 40), region(&lat, chunk(1001), 40));
    let (na, nb) = (Network::build(&River(1000), &Detail(7.0), &lat, &ra, R), Network::build(&River(1000), &Detail(7.0), &lat, &rb, R));
    let (mut shared, mut on_path, mut next_checked) = (0, 0, 0);
    for (ia, &cell) in ra.cells.iter().enumerate() {
        let Some(ib) = rb.index_of(cell) else { continue };
        shared += 1;
        assert_eq!(na.corridor[ia], nb.corridor[ib], "corridor at {cell:?}");
        assert_eq!(na.z0[ia].to_bits(), nb.z0[ib].to_bits(), "initial height at {cell:?}");
        on_path += usize::from(na.corridor[ia]);
        if na.next[ia] != NO_NEXT && nb.next[ib] != NO_NEXT {
            assert_eq!(ra.cells[na.next[ia] as usize], rb.cells[nb.next[ib] as usize], "next cell at {cell:?}");
            next_checked += 1;
        }
    }
    assert!(shared > 1000, "the regions overlap ({shared} cells)");
    assert!(on_path >= 20, "the river crosses the overlap ({on_path} corridor cells)");
    assert!(next_checked >= 20);
    // The river runs across both chunks: the corridor cells of the first region reach the shared border.
    let border = ra.cells.iter().enumerate().filter(|(i, c)| na.corridor[*i] && c.i >= 1001 * 32 - 1).count();
    assert!(border > 0, "the corridor of the first chunk reaches the border");
}

// spec: GEN-047
#[test]
fn a_segment_starts_and_ends_at_the_coarse_cell_centres_and_its_meander_stays_within_the_bound() {
    let l = lattice();
    let (c0, c1) = (FACE * N * N + 1000 * N + 1000, FACE * N * N + 1000 * N + 1001);
    let (a, b) = (l.direction(c0), l.direction(c1));
    let pts = segment_points(a, b, c0 as u64, R);
    assert!((pts[0] - a).length() < 1e-12 && (pts[pts.len() - 1] - b).length() < 1e-12, "the ends are the cell centres");
    // One point every 40 m of the chord (the meander moves each sideways, so the steps along the path are longer).
    let chord_m = (b - a).length() * R;
    assert_eq!(pts.len(), (chord_m / 40.0).ceil() as usize + 1, "a point every 40 m of the chord");
    // Distance of every point from the chord line a-b, in metres: at most the meander.
    let along = (b - a).normalized();
    let worst = pts.iter().map(|&p| ((p - a) - along * (p - a).dot(along)).length() * R).fold(0.0, f64::max);
    assert!(worst <= MEANDER_M + 25.0, "the path stays within {worst} m of the chord");
    assert!(worst > 50.0, "and it does wander ({worst} m)");
    assert_eq!(meander_m(7, 0.0, 5000.0), 0.0);
    assert!(meander_m(7, 1.0, 5000.0).abs() < 1e-9);
    // A function of the segment alone: the same call gives the same points.
    assert_eq!(pts, segment_points(a, b, c0 as u64, R));
    assert_ne!(pts[10], segment_points(a, b, c0 as u64 + 1, R)[10], "another segment, another meander");
}

// spec: GEN-047
#[test]
fn a_river_that_enters_the_region_from_outside_brings_its_coarse_area_and_a_chunk_without_one_has_no_corridor() {
    let lat = ChunkLattice::new(32).unwrap();
    // Chunk 1005 with a small halo: the river from column 995 to 1010 passes through it; the region starts inside the river's reach.
    let r = region(&lat, chunk(1005), 6);
    let net = Network::build(&River(1000), &Detail(0.0), &lat, &r, R);
    let entered: f64 = net.inflow_m2.iter().sum();
    assert!(entered > 0.0, "the river enters the region from outside");
    // Every entry carries the coarse area of a cell upstream of the region: one of the areas of the river's cells.
    for &a in net.inflow_m2.iter().filter(|a| **a > 0.0) {
        assert!((995..=1010).any(|i| (a - 2.0e8 * (1 + i - 995) as f64).abs() < 1.0), "inflow {a} m2 is the coarse area of a river cell");
    }
    // A chunk far from the river has no corridor, no inflow and no links.
    let away = region(&lat, TileId::new(Face(FACE as u8), CHUNK_GRID_LEVEL, 1005, 1500).unwrap(), 6);
    let none = Network::build(&River(1000), &Detail(0.0), &lat, &away, R);
    assert!(none.corridor.iter().all(|c| !c) && none.inflow_m2.iter().all(|a| *a == 0.0) && none.next.iter().all(|n| *n == NO_NEXT));
}

// spec: GEN-047
#[test]
fn the_initial_heights_are_the_runs_upsampled_plus_the_base_detail() {
    let lat = ChunkLattice::new(32).unwrap();
    let r = region(&lat, chunk(1003), 4);
    let net = Network::build(&River(1000), &Detail(7.0), &lat, &r, R);
    for k in [0usize, 100, 500, r.cells.len() - 1] {
        let d = lat.direction(r.cells[k]);
        let expected = coarse_z_at(&River(1000), d) + 7.0;
        assert!((f64::from(net.z0[k]) - expected).abs() < 1e-3, "cell {k}: {} against {expected}", net.z0[k]);
    }
    // At a coarse cell centre the upsampled height is the run's own.
    let c = FACE * N * N + 1000 * N + 1003;
    let d = lattice().direction(c);
    assert!((coarse_z_at(&River(1000), d) - f64::from(River(1000).z(c))).abs() < 1e-4);
    let _ = TangentWarp;
}

/// A run whose height is smooth in three dimensions (`1000 y` of the cell's direction), no river.
struct Smooth;

impl CoarseRun for Smooth {
    fn identity(&self) -> u64 {
        5
    }
    fn lattice(&self) -> &Lattice {
        lattice()
    }
    fn z(&self, c: usize) -> f32 {
        (1000.0 * lattice().direction(c).y) as f32
    }
    fn area_m2(&self, _: usize) -> f32 {
        1.0e6
    }
    fn receiver(&self, _: usize) -> u32 {
        NONE
    }
}

// spec: GEN-047
#[test]
fn the_links_of_the_corridor_end_at_an_outlet_join_neighbouring_cells_and_do_not_depend_on_the_region() {
    let lat = ChunkLattice::new(32).unwrap();
    // The links of every corridor cell lead to an outlet: no loop, whatever way the meander turns back inside a cell.
    for (row, x) in [(1000usize, 1000u32), (1000, 1004), (1001, 1000)] {
        let r = region(&lat, chunk(x).tile_at_row(row), 40);
        let net = Network::build(&River(row), &Detail(0.0), &lat, &r, R);
        let mut links = 0;
        for start in 0..r.cells.len() {
            let (mut at, mut steps) = (start, 0);
            while net.next[at] != NO_NEXT {
                let to = net.next[at] as usize;
                assert!(lat.neighbours(r.cells[at]).contains(&r.cells[to]), "a link joins neighbouring cells");
                at = to;
                steps += 1;
                assert!(steps <= r.cells.len(), "the links from {:?} do not end", r.cells[start]);
                links += 1;
            }
        }
        assert!(links > 50, "row {row}: {links} links followed");
    }
    // A river along the next coarse row (2.4 km beyond the chunk's edge) is cut by the region's outer boundary; the links of the cells whose
    // neighbours are all in both regions are the same in both.
    let halo = 20;
    let (a, b) =
        (region(&lat, chunk(1000), halo), region(&lat, TileId::new(Face(FACE as u8), CHUNK_GRID_LEVEL, 1000, 1001).unwrap(), halo));
    let (na, nb) = (Network::build(&River(1001), &Detail(0.0), &lat, &a, R), Network::build(&River(1001), &Detail(0.0), &lat, &b, R));
    let mut compared = 0;
    for (ia, &cell) in a.cells.iter().enumerate() {
        let Some(ib) = b.index_of(cell) else { continue };
        if a.depth[ia] >= halo || b.depth[ib] >= halo {
            continue;
        }
        let to = |net: &Network, r: &planet_generators::chunkfield::Region, i: usize| {
            (net.next[i] != NO_NEXT).then(|| r.cells[net.next[i] as usize])
        };
        assert_eq!(to(&na, &a, ia), to(&nb, &b, ib), "the link out of {cell:?}");
        assert_eq!(na.corridor[ia], nb.corridor[ib]);
        compared += usize::from(na.corridor[ia]);
    }
    assert!(compared > 10, "{compared} corridor cells compared");
}

trait AtRow {
    fn tile_at_row(self, row: usize) -> TileId;
}

impl AtRow for TileId {
    fn tile_at_row(self, row: usize) -> TileId {
        TileId::new(self.face(), CHUNK_GRID_LEVEL, self.x(), row as u32).unwrap()
    }
}

// spec: GEN-047
#[test]
fn the_upsampled_run_has_no_step_across_a_face_edge_or_at_a_cube_corner() {
    use planet_core::cube::face_to_direction;
    let at = |s: f64, t: f64, face: u8| face_to_direction(&TangentWarp, Face(face), s, t);
    // Across the +s edge of face 2 and at the corner where three faces meet: 1.2 m apart, a smooth surface differs by a little.
    let pairs = [(at(1.0 - 1e-7, 0.3, 2), at(1.0 + 1e-7, 0.3, 2)), (at(1.0 - 1e-7, 1.0 - 1e-7, 2), at(1.0 + 1e-7, 1.0 + 1e-7, 2))];
    for (a, b) in pairs {
        let step = (coarse_z_at(&Smooth, a) - coarse_z_at(&Smooth, b)).abs();
        assert!(step < 3.0, "a {step} m step between points {} m apart", (a - b).length() * R);
    }
}

/// A base with a narrow ridge across the river between the coarse cells 1002 and 1003 (80 m high, 500 m across), on the 7 m plateau of `Detail`.
struct Ridge;

fn ridge_centre() -> Vec3 {
    let l = lattice();
    let (a, b) = (l.direction(FACE * N * N + 1000 * N + 1002), l.direction(FACE * N * N + 1000 * N + 1003));
    ((a + b) * 0.5).normalized()
}

impl BaseDetail for Ridge {
    fn base_height(&self, dir: Vec3) -> f64 {
        // The ridge is a wall along the line through its centre across the river, so the way round it is to the side, not through it.
        let along =
            (lattice().direction(FACE * N * N + 1000 * N + 1003) - lattice().direction(FACE * N * N + 1000 * N + 1002)).normalized();
        let across = ridge_centre().cross(along).normalized();
        let (d_along, d_across) = ((dir - ridge_centre()).dot(along) * R, (dir - ridge_centre()).dot(across) * R);
        let wall = 80.0 * libm::exp(-(d_along / 250.0) * (d_along / 250.0)) * (1.0 - 0.9 * smooth(d_across.abs() / 1200.0));
        self.coarse_input(dir) + 7.0 + wall
    }
    fn identity(&self) -> u64 {
        13
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        100.0 + 3.0 * dir.x
    }
}

/// 0 at 0, 1 at 1 and beyond.
fn smooth(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn routed_law() -> planet_generators::anet::AnchorLaw {
    planet_generators::anet::AnchorLaw {
        route_band_m: 1500.0,
        route_lambda: 0.02,
        route_stride: 1,
        slide_m: 0.0,
        slide_steep_m: 0.0,
        head_m: 0.0,
        band_cap_m: 0.0,
        ..planet_generators::anet::AnchorLaw::default()
    }
}

// spec: GEN-066, GEN-067
#[test]
fn neighbouring_chunks_find_the_same_routed_corridor_and_beds_through_their_shared_border() {
    let lat = ChunkLattice::new(32).unwrap();
    let mut law = routed_law();
    law.slide_m = 400.0;
    law.slide_steep_m = 400.0;
    law.head_m = 400.0;
    let (ra, rb) = (region(&lat, chunk(1002), 40), region(&lat, chunk(1003), 40));
    let (na, nb) =
        (Network::build_with(&River(1000), &Ridge, &lat, &ra, R, &law), Network::build_with(&River(1000), &Ridge, &lat, &rb, R, &law));
    let (mut shared, mut on_path) = (0, 0);
    for (ia, &cell) in ra.cells.iter().enumerate() {
        let Some(ib) = rb.index_of(cell) else { continue };
        shared += 1;
        assert_eq!(na.corridor[ia], nb.corridor[ib], "corridor at {cell:?}");
        assert_eq!(na.anchor_z[ia].to_bits(), nb.anchor_z[ib].to_bits(), "bed at {cell:?}");
        if na.next[ia] != NO_NEXT && nb.next[ib] != NO_NEXT {
            assert_eq!(ra.cells[na.next[ia] as usize], rb.cells[nb.next[ib] as usize], "next cell at {cell:?}");
        }
        on_path += usize::from(na.corridor[ia]);
    }
    assert!(shared > 1000 && on_path >= 15, "{shared} shared cells, {on_path} on the corridor");
}

// spec: GEN-066
#[test]
fn the_routed_path_goes_round_a_ridge_the_coarse_path_crosses_and_the_nodes_of_all_segments_through_a_cell_agree() {
    use planet_generators::route::Router;
    let lat = ChunkLattice::new(32).unwrap();
    let region_ = region(&lat, chunk(1002), 40);
    let top = |net: &Network, region: &planet_generators::chunkfield::Region| {
        (0..region.cells.len())
            .filter(|&c| net.corridor[c])
            .map(|c| Ridge.base_height(lat.direction(region.cells[c])))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let coarse = Network::build_with(&River(1000), &Ridge, &lat, &region_, R, &planet_generators::anet::AnchorLaw::default());
    let routed = Network::build_with(&River(1000), &Ridge, &lat, &region_, R, &routed_law());
    let (high, low) = (top(&coarse, &region_), top(&routed, &region_));
    assert!(high > 160.0, "the coarse path crosses the ridge ({high} m)");
    assert!(low < high - 30.0, "the routed path keeps lower: {low} m against {high} m");
    // The node of a cell is one direction for every segment that meets there (a function of the cell alone).
    let mut law = routed_law();
    law.slide_m = 600.0;
    law.slide_steep_m = 600.0;
    let run = River(1000);
    let memo = planet_generators::route::RouteMemo::default();
    let router = Router::new(&run, &Ridge, &lat, R, &law, &memo);
    let c = FACE * N * N + 1000 * N + 1002;
    let (up, down) = (router.polyline(c - 1, c), router.polyline(c, c + 1));
    assert_eq!(up[up.len() - 1], down[0], "the segments meet at the same node");
    assert_eq!(router.node(c), down[0]);
    // The node slid to ground no higher than the cell's centre, within the radius.
    let centre = lattice().direction(c);
    assert!(Ridge.base_height(router.node(c)) <= Ridge.base_height(centre) + 1e-9);
    assert!((router.node(c) - centre).length() * R <= 600.0 + 1.0);
}

// spec: GEN-059
#[test]
fn the_bed_never_lies_over_the_ground_and_never_climbs_downstream_even_where_a_ridge_forces_a_cut() {
    let lat = ChunkLattice::new(32).unwrap();
    let r = region(&lat, chunk(1002), 40);
    let mut law = routed_law();
    law.cut_cap_m = 200.0;
    let net = Network::build_with(&River(1000), &Ridge, &lat, &r, R, &law);
    let (mut checked, mut cut) = (0, 0.0f64);
    for c in 0..r.cells.len() {
        if !net.corridor[c] {
            continue;
        }
        assert!(net.anchor_z[c] <= net.base_z[c] + 1e-6, "the bed of {:?} is over the ground", r.cells[c]);
        cut = cut.max(net.base_z[c] - net.anchor_z[c]);
        let n = net.next[c];
        if n != NO_NEXT {
            assert!(net.anchor_z[n as usize] <= net.anchor_z[c] + 0.05, "the bed climbs from {:?}", r.cells[c]);
            checked += 1;
        }
    }
    assert!(checked > 30, "{checked} links checked");
    assert!(cut <= 200.0 + 100.0, "the cut stays within the cap and the valley depth ({cut} m)");
}

/// The river of `River(1000)` with a tributary down column 1002 from row 995 to the river at row 1000 (a confluence at the coarse cell (1002, 1000)).
struct Fork;

fn on_trib(c: usize) -> Option<usize> {
    let (f, r) = (c / (N * N), c % (N * N));
    let (i, j) = (r % N, r / N);
    (f == FACE && i == 1002 && (995..1000).contains(&j)).then_some(j)
}

impl CoarseRun for Fork {
    fn lattice(&self) -> &Lattice {
        lattice()
    }
    fn z(&self, c: usize) -> f32 {
        River(1000).z(c)
    }
    fn area_m2(&self, c: usize) -> f32 {
        on_trib(c).map_or_else(|| River(1000).area_m2(c), |_| 3.0e8)
    }
    fn identity(&self) -> u64 {
        17
    }
    fn receiver(&self, c: usize) -> u32 {
        if on_trib(c).is_some() {
            (c + N) as u32
        } else {
            River(1000).receiver(c)
        }
    }
}

/// `Detail` with a hollow `depth` m deep on the tributary, two coarse cells above the confluence.
struct Hollow(f64);

impl BaseDetail for Hollow {
    fn base_height(&self, dir: Vec3) -> f64 {
        let at = lattice().direction(FACE * N * N + 998 * N + 1002);
        let d = (dir - at).length() * R;
        self.coarse_input(dir) + 7.0 - self.0 * libm::exp(-(d / 300.0) * (d / 300.0))
    }
    fn identity(&self) -> u64 {
        19
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        100.0 + 3.0 * dir.x
    }
}

// spec: GEN-059, GEN-069
#[test]
fn a_tributary_hangs_above_the_stem_at_a_confluence_with_no_climb_and_never_over_the_ground() {
    let lat = ChunkLattice::new(32).unwrap();
    for (depth, climbs_allowed) in [(0.0, false), (30.0, true)] {
        let base = Hollow(depth);
        let r = region(&lat, chunk(1002), 40);
        let net = Network::build_with(&Fork, &base, &lat, &r, R, &routed_law());
        let (mut links, mut worst_climb) = (0, 0.0f64);
        for c in 0..r.cells.len() {
            if !net.corridor[c] {
                continue;
            }
            assert!(net.anchor_z[c] <= net.base_z[c] + 1e-6, "depth {depth}: the bed of {:?} is over the ground", r.cells[c]);
            let n = net.next[c];
            if n != NO_NEXT {
                worst_climb = worst_climb.max(net.anchor_z[n as usize] - net.anchor_z[c]);
                links += 1;
            }
        }
        assert!(links > 30, "{links} links");
        // Where the ground is smooth no link climbs; in a hollow the bed is held at the ground, a flat or a short rise out of it at worst.
        assert!(climbs_allowed || worst_climb <= 0.05, "depth {depth}: a link climbs by {worst_climb} m");
    }
}

// spec: GEN-067
#[test]
fn the_routed_corridor_beds_and_links_of_a_fork_do_not_depend_on_the_halo_or_on_the_thread() {
    let lat = ChunkLattice::new(32).unwrap();
    let mut law = routed_law();
    law.slide_m = 400.0;
    law.slide_steep_m = 400.0;
    law.head_m = 400.0;
    // The same chunk at two halos: every shared cell has the same corridor flag, bed bits and next cell, across the confluence.
    let (small, wide) = (region(&lat, chunk(1002), 20), region(&lat, chunk(1002), 40));
    let (ns, nw) =
        (Network::build_with(&Fork, &Hollow(0.0), &lat, &small, R, &law), Network::build_with(&Fork, &Hollow(0.0), &lat, &wide, R, &law));
    let mut compared = 0;
    for (i, &cell) in small.cells.iter().enumerate() {
        let j = wide.index_of(cell).expect("the small region lies in the wide one");
        assert_eq!(ns.corridor[i], nw.corridor[j], "corridor at {cell:?}");
        if ns.corridor[i] {
            assert_eq!(ns.anchor_z[i].to_bits(), nw.anchor_z[j].to_bits(), "bed at {cell:?}");
            if ns.next[i] != NO_NEXT && nw.next[j] != NO_NEXT {
                assert_eq!(small.cells[ns.next[i] as usize], wide.cells[nw.next[j] as usize], "next cell at {cell:?}");
            }
            compared += 1;
        }
    }
    assert!(compared > 30, "{compared} corridor cells compared");
    // The same network built on four threads at once is the same bits: the router holds no shared state.
    let reference: Vec<u64> = wide.cells.iter().enumerate().map(|(i, _)| nw.anchor_z[i].to_bits()).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let n = Network::build_with(&Fork, &Hollow(0.0), &lat, &wide, R, &law);
                    n.anchor_z.iter().map(|z| z.to_bits()).collect::<Vec<u64>>()
                })
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().expect("a thread"), reference, "a build on another thread differs");
        }
    });
}

// spec: GEN-068
#[test]
fn a_tributary_of_the_coarse_run_is_routed_like_a_stem_only_above_the_river_threshold() {
    let lat = ChunkLattice::new(32).unwrap();
    let r = region(&lat, chunk(1002), 40);
    // 12 cells (1.8 km) north of the main river's row (the middle of coarse row 1000), only the tributary can have a corridor.
    let north = |law: &planet_generators::anet::AnchorLaw| {
        let net = Network::build_with(&Fork, &Hollow(0.0), &lat, &r, R, law);
        (0..r.cells.len()).filter(|&c| net.corridor[c] && r.cells[c].j + 12 < 1000 * 32 + 16).count()
    };
    let mut law = routed_law();
    law.river_area_m2 = 1.0e8;
    let with = north(&law);
    law.river_area_m2 = 5.0e8;
    let without = north(&law);
    assert!(with > 20, "the tributary of 3e8 m2 has {with} corridor cells above a threshold of 1e8");
    assert_eq!(without, 0, "and none above a threshold of 5e8");
}

// spec: GEN-067
#[test]
fn a_stage_memo_of_routed_segments_gives_the_bits_of_a_fresh_build_and_empties_when_the_law_changes() {
    use planet_generators::route::RouteMemo;
    let lat = ChunkLattice::new(32).unwrap();
    let mut law = routed_law();
    law.slide_m = 400.0;
    law.slide_steep_m = 400.0;
    let (ra, rb) = (region(&lat, chunk(1002), 40), region(&lat, chunk(1003), 40));
    let bits = |n: &Network| n.anchor_z.iter().map(|z| z.to_bits()).collect::<Vec<u64>>();
    let memo = RouteMemo::default();
    let first = Network::build_with_memo(&Fork, &Ridge, &lat, &ra, R, &law, &memo);
    // A second chunk through the same memo is the fresh build, and so is the first again (all hits).
    let second = Network::build_with_memo(&Fork, &Ridge, &lat, &rb, R, &law, &memo);
    assert_eq!(bits(&second), bits(&Network::build_with(&Fork, &Ridge, &lat, &rb, R, &law)));
    assert_eq!(bits(&first), bits(&Network::build_with_memo(&Fork, &Ridge, &lat, &ra, R, &law, &memo)));
    // Another law through the same memo is that law's fresh build, not the memo's old paths.
    let mut other = law;
    other.route_band_m = 800.0;
    other.slide_m = 200.0;
    let changed = Network::build_with_memo(&Fork, &Ridge, &lat, &ra, R, &other, &memo);
    assert_eq!(bits(&changed), bits(&Network::build_with(&Fork, &Ridge, &lat, &ra, R, &other)));
    assert_ne!(bits(&changed), bits(&first), "the law changes the beds");
}

// spec: GEN-066
#[test]
fn without_a_band_or_a_slide_the_polyline_is_the_coarse_path_and_a_head_moves_to_the_highest_ground() {
    use planet_generators::route::{RouteMemo, Router};
    let lat = ChunkLattice::new(32).unwrap();
    let c0 = FACE * N * N + 1000 * N + 1001;
    let run = River(1000);
    let memo = RouteMemo::default();
    let off = planet_generators::anet::AnchorLaw::default();
    let router = Router::new(&run, &Ridge, &lat, R, &off, &memo);
    let coarse = segment_points(lattice().direction(c0), lattice().direction(c0 + 1), c0 as u64, R);
    assert_eq!(*router.polyline(c0, c0 + 1), coarse, "no band and no slide is the coarse path");
    // The first cell of the river (995) is a head: with head_m set, its node is the highest ground within the radius, not lower than the centre.
    let head = FACE * N * N + 1000 * N + 995;
    let mut law = routed_law();
    law.head_m = 500.0;
    let memo = RouteMemo::default();
    let router = Router::new(&run, &Ridge, &lat, R, &law, &memo);
    let centre = lattice().direction(head);
    assert!(Ridge.base_height(router.node(head)) >= Ridge.base_height(centre) - 1e-9);
    assert!((router.node(head) - centre).length() * R <= 500.0 + 1.0);
}

// spec: GEN-067
#[test]
fn chunks_built_at_once_on_several_threads_through_one_stage_memo_equal_fresh_builds() {
    use planet_generators::route::RouteMemo;
    let lat = ChunkLattice::new(32).unwrap();
    let mut law = routed_law();
    law.slide_m = 400.0;
    law.slide_steep_m = 400.0;
    let memo = RouteMemo::default();
    let chunks = [1001u32, 1002, 1003, 1004];
    let bits = |n: &Network| n.anchor_z.iter().map(|z| z.to_bits()).collect::<Vec<u64>>();
    let fresh: Vec<Vec<u64>> =
        chunks.iter().map(|&x| bits(&Network::build_with(&Fork, &Ridge, &lat, &region(&lat, chunk(x), 40), R, &law))).collect();
    let shared: Vec<Vec<u64>> = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|&x| {
                let (lat, law, memo) = (&lat, &law, &memo);
                scope.spawn(move || bits(&Network::build_with_memo(&Fork, &Ridge, lat, &region(lat, chunk(x), 40), R, law, memo)))
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("a thread")).collect()
    });
    assert_eq!(shared, fresh, "chunks built concurrently through one memo differ from fresh builds");
}

/// A base that counts the directions it is asked for (the raster of the routing reads it only where the region and the memo do not).
struct Counting(AtomicUsize);

impl BaseDetail for Counting {
    fn base_height(&self, dir: Vec3) -> f64 {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ridge.base_height(dir)
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        Ridge.coarse_input(dir)
    }
    fn identity(&self) -> u64 {
        Ridge.identity()
    }
}

fn stride_law(stride: u32) -> planet_generators::anet::AnchorLaw {
    let mut law = routed_law();
    law.route_stride = stride;
    law.slide_m = 400.0;
    law.slide_steep_m = 400.0;
    law
}

// spec: GEN-077
#[test]
fn the_routing_reads_the_regions_heights_and_the_memo_and_evaluates_only_the_rest_with_the_bits_of_evaluating_everything() {
    use planet_generators::route::{RouteMemo, Router};
    let lat = ChunkLattice::new(32).unwrap();
    for stride in [1u32, 4] {
        let law = stride_law(stride);
        // A router that reads the base at every raster point (no region heights, no memo) against one that reads the region's heights.
        let ra = region(&lat, chunk(1002), 40);
        let dirs: Vec<Vec3> = ra.cells.iter().map(|&c| lat.direction(c)).collect();
        let heights = Ridge.base_heights(&dirs);
        let plain_memo = RouteMemo::with_height_cap(0);
        let plain = Router::new(&River(1000), &Ridge, &lat, R, &law, &plain_memo);
        let known_memo = RouteMemo::default();
        let known = Router::new(&River(1000), &Ridge, &lat, R, &law, &known_memo).with_known(&ra, &heights);
        let c = FACE * N * N + 1000 * N + 1002;
        for (from, to) in [(c - 1, c), (c, c + 1), (c + 1, c + 2)] {
            let (a, b) = (plain.polyline(from, to), known.polyline(from, to));
            assert_eq!(a.len(), b.len(), "stride {stride}: the path differs in length");
            assert!(
                a.iter().zip(b.iter()).all(|(p, q)| p == q),
                "stride {stride}: the path differs from the one that evaluates every point"
            );
        }
        let [from_region, from_memo, evaluated] = known_memo.raster_counts();
        let [_, _, plain_evaluated] = plain_memo.raster_counts();
        assert!(from_region > 0, "stride {stride}: no raster point read the region's heights");
        assert_eq!(from_region + from_memo + evaluated, plain_evaluated, "stride {stride}: every raster point is served once");
        assert!(evaluated < plain_evaluated, "stride {stride}: {evaluated} of {plain_evaluated} points still evaluated");
    }
}

// spec: GEN-077
#[test]
fn chunks_have_the_same_bits_with_the_memo_cold_warm_filled_in_another_order_or_capped_at_one_and_eight_workers() {
    use planet_generators::route::RouteMemo;
    let lat = ChunkLattice::new(32).unwrap();
    let law = stride_law(4);
    let chunks: Vec<u32> = (1001..=1008).collect();
    let bits = |n: &Network| n.anchor_z.iter().map(|z| z.to_bits()).collect::<Vec<u64>>();
    let build = |x: u32, memo: &RouteMemo| {
        let base = Counting(AtomicUsize::new(0));
        let net = Network::build_with_memo(&Fork, &base, &lat, &region(&lat, chunk(x), 40), R, &law, memo);
        (bits(&net), base.0.load(Ordering::Relaxed))
    };
    // Cold: a fresh memo for every chunk.
    let cold: Vec<Vec<u64>> = chunks.iter().map(|&x| build(x, &RouteMemo::default()).0).collect();
    // Warm and in reverse order: one memo, the chunks in reverse order, then again (the second time every raster point from the memo).
    let memo = RouteMemo::default();
    let reversed: Vec<Vec<u64>> = chunks.iter().rev().map(|&x| build(x, &memo).0).collect::<Vec<_>>().into_iter().rev().collect();
    assert_eq!(reversed, cold, "the memo filled in reverse order changes a chunk");
    let evaluated_once = memo.raster_counts()[2];
    let again: Vec<Vec<u64>> = chunks.iter().map(|&x| build(x, &memo).0).collect();
    assert_eq!(again, cold, "a warm memo changes a chunk");
    assert_eq!(memo.raster_counts()[2], evaluated_once, "the warm builds still evaluated raster points");
    // Capped far under what the chunks read: the memo clears and refills, and never holds more than the cap.
    let capped = RouteMemo::with_height_cap(500);
    let through_cap: Vec<Vec<u64>> = chunks.iter().map(|&x| build(x, &capped).0).collect();
    assert_eq!(through_cap, cold, "a memo that clears often changes a chunk");
    assert!(capped.raster_heights_held() <= 500, "the memo holds {} heights over its cap of 500", capped.raster_heights_held());
    // One and eight workers through one memo.
    for workers in [1usize, 8] {
        let shared = RouteMemo::default();
        let next = AtomicUsize::new(0);
        let results = std::sync::Mutex::new(vec![Vec::new(); chunks.len()]);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let k = next.fetch_add(1, Ordering::SeqCst);
                    if k >= chunks.len() {
                        break;
                    }
                    let b = build(chunks[k], &shared).0;
                    results.lock().expect("results")[k] = b;
                });
            }
        });
        assert_eq!(results.into_inner().expect("results"), cold, "{workers} workers through one memo change a chunk");
    }
}

// spec: GEN-078
#[test]
fn a_chunk_routes_only_the_segments_that_can_reach_its_region_and_has_the_bits_of_routing_them_all() {
    use planet_generators::route::RouteMemo;
    let lat = ChunkLattice::new(32).unwrap();
    let bits = |n: &Network| {
        let z: Vec<u64> = n.anchor_z.iter().map(|z| z.to_bits()).collect();
        let a: Vec<u64> = n.river_area_m2.iter().map(|a| a.to_bits()).collect();
        let inflow: Vec<u64> = n.inflow_m2.iter().map(|a| a.to_bits()).collect();
        (z, a, inflow, n.corridor.clone(), n.next.clone())
    };
    let (mut pruned_total, mut all_total) = (0, 0);
    for law in [stride_law(1), stride_law(4), planet_generators::anet::AnchorLaw::default()] {
        for x in 1001..=1008u32 {
            for halo in [10u16, 40] {
                let r = region(&lat, chunk(x), halo);
                let (m_pruned, m_all) = (RouteMemo::default(), RouteMemo::default());
                let pruned = Network::build_with_memo(&Fork, &Ridge, &lat, &r, R, &law, &m_pruned);
                let all = Network::build_unpruned(&Fork, &Ridge, &lat, &r, R, &law, &m_all);
                assert_eq!(bits(&pruned), bits(&all), "chunk {x}, halo {halo}, stride {}: pruning changed the network", law.route_stride);
                assert!(m_pruned.routed_segments() <= m_all.routed_segments());
                pruned_total += m_pruned.routed_segments();
                all_total += m_all.routed_segments();
            }
        }
    }
    assert!(pruned_total < all_total, "pruning saved nothing: {pruned_total} of {all_total} segments routed");
}

// spec: GEN-078
#[test]
fn the_reach_of_a_routed_path_covers_the_charts_stretch_at_a_cube_corner() {
    use planet_generators::anet::{routing_reach_m, AnchorLaw};
    // The production law: band 1 km, slide and head 1 km, 8 m cells at stride 8. The equal-angle chart stretches a grid step by 2/sqrt(3) at a cube corner,
    // so the band disc and the slide disc reach 1.1547 km there along the diagonal, and the meander (700 m, true metres) adds to both.
    let law =
        AnchorLaw { route_band_m: 1000.0, slide_m: 500.0, slide_steep_m: 1000.0, head_m: 1000.0, route_stride: 8, ..AnchorLaw::default() };
    let worst = 2.0 / 3.0f64.sqrt() * (1000.0 + 1000.0) + planet_generators::anet::MEANDER_M + 2.0 * 64.0;
    assert!(routing_reach_m(&law, 8.0) >= worst, "the reach {} m is under the corner's {worst} m", routing_reach_m(&law, 8.0));
    // Without routing nothing but the meander (and the raster's rounding) is left.
    let off = AnchorLaw { route_band_m: 0.0, slide_m: 0.0, slide_steep_m: 0.0, head_m: 0.0, ..AnchorLaw::default() };
    assert!(routing_reach_m(&off, 8.0) >= planet_generators::anet::MEANDER_M);
}

/// The `Ridge` base with a bump of `dh` metres within `radius_m` of `at`: an item that raises the ground.
struct Bumped {
    at: Vec3,
    radius_m: f64,
    dh: f64,
}

impl BaseDetail for Bumped {
    fn base_height(&self, dir: Vec3) -> f64 {
        let bump = if (dir - self.at).length() * R <= self.radius_m { self.dh } else { 0.0 };
        Ridge.base_height(dir) + bump
    }
    fn coarse_input(&self, dir: Vec3) -> f64 {
        Ridge.coarse_input(dir)
    }
    fn identity(&self) -> u64 {
        14
    }
}

// spec: GEN-079
#[test]
fn an_item_outside_the_read_set_changes_no_bit_of_a_chunk_and_one_inside_does() {
    use planet_generators::anet::chunk_read_set;
    use planet_generators::route::RouteMemo;
    let lat = ChunkLattice::new(32).unwrap();
    let bits = |n: &Network| {
        let z: Vec<u64> = n.anchor_z.iter().map(|z| z.to_bits()).collect();
        let a: Vec<u64> = n.river_area_m2.iter().map(|a| a.to_bits()).collect();
        let inflow: Vec<u64> = n.inflow_m2.iter().map(|a| a.to_bits()).collect();
        (z, a, inflow, n.corridor.clone(), n.next.clone())
    };
    // The default window and a short one (2 segments back): a bump beyond the short window must change nothing.
    let short = planet_generators::anet::AnchorLaw { window: 2, ..stride_law(4) };
    // Slide and head differ from the steep slide: the relief factor of a start cell moves the node, so the base at its eight neighbours is read.
    let slide = planet_generators::anet::AnchorLaw { slide_m: 500.0, slide_steep_m: 1000.0, head_m: 1000.0, ..stride_law(4) };
    let (bump_m, mut changed_inside, mut tested_outside) = (300.0, 0, 0);
    for (x, law) in
        [(1003u32, stride_law(4)), (1006, stride_law(4)), (1008, short), (1004, short), (1002, short), (1005, slide), (1007, slide)]
    {
        let r = region(&lat, chunk(x), 40);
        let discs = chunk_read_set(&Fork, &lat, &r, R, &law);
        // The set is a function of the chunk, the run and the law only: the same twice.
        assert_eq!(discs, chunk_read_set(&Fork, &lat, &r, R, &law));
        let plain = Network::build_with_memo(&Fork, &Ridge, &lat, &r, R, &law, &RouteMemo::default());
        let plain = bits(&plain);
        for i in (990..=1020).step_by(3) {
            for j in 994..=1006 {
                let at = lattice().direction(FACE * N * N + j * N + i);
                let inside = discs.iter().any(|d| d.centre.angle_to(at) <= d.angle + bump_m / R);
                let with =
                    Network::build_with_memo(&Fork, &Bumped { at, radius_m: bump_m, dh: -60.0 }, &lat, &r, R, &law, &RouteMemo::default());
                if inside {
                    changed_inside += usize::from(bits(&with) != plain);
                } else {
                    tested_outside += 1;
                    assert_eq!(bits(&with), plain, "chunk {x}: a bump at ({i}, {j}) outside every disc changed the network");
                }
            }
        }
    }
    assert!(tested_outside > 0, "no bump lay outside the read set");
    assert!(changed_inside > 0, "no bump inside the read set changed a network");
}
