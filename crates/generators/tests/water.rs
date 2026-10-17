//! GEN-023: the water-level field W (ADR 0041 point 1), its sea producer and the retrofit test (ADR 0041 point 7).

use std::collections::HashSet;

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::hash_u64s;
use planet_core::{Face, Vec3};
use planet_generators::global::{bake, locate_direction, GlobalGrid, GlobalSample, GridSource, ReadError, SEA_LEVEL_C};
use planet_generators::ocean::{tiled_ocean, CubeIndex, OceanGrid, OceanLattice, EARTH_OCEAN_MIN_AREA_FRACTION};
use planet_generators::water::{
    PlanWater, SeaProducer, WaterBodyId, WaterField, WaterProducer, WaterQuery, WaterSample, WATER_IMPLEMENTATION_ID, WATER_VERSION,
};
use planet_world_def::{BodyId, LiquidKind, LiquidKindId, SeaParams, WorldDefinition};

const SEED: u64 = 1;
const RES: u32 = 128;

fn world_bake() -> (GlobalGrid, OceanGrid) {
    let grid = bake(SEED, &TangentWarp, RES, 4).unwrap();
    let (_, ocean) = tiled_ocean(&grid, &TangentWarp, 32, 3, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    (grid, ocean)
}

/// A reproducible spread of directions over the whole sphere.
fn directions(n: u64) -> Vec<Vec3> {
    (0..n)
        .map(|k| {
            let c = |a: u64| (hash_u64s(0x7761_7465, &[k, a]) >> 11) as f64 / (1u64 << 52) as f64 * 2.0 - 1.0;
            Vec3::new(c(0), c(1), c(2)).normalized()
        })
        .collect()
}

fn word(w: Option<WaterSample>) -> [u64; 3] {
    w.map_or([0; 3], |w| [w.level_m.to_bits(), w.body.0, u64::from(w.kind.0) + 1])
}

// spec: GEN-023
#[test]
fn the_sea_is_ocean_connected_low_terrain_and_everything_else_has_no_water() {
    let (grid, ocean) = world_bake();
    let src = GridSource(&grid);
    let sea = SeaProducer::new(&ocean, SeaParams::EARTH);
    let field = WaterField::new(&TangentWarp, &src, vec![&sea]);
    let mut counts = [0usize; 4]; // wet, dry sea (a basin), land, edge cases
    for d in directions(6000) {
        let w = field.water_at(d).unwrap();
        let ground = planet_generators::global::read_direction(&src, &TangentWarp, d).unwrap();
        let low = f64::from(ground.continent) <= SEA_LEVEL_C;
        match w {
            Some(w) => {
                assert!(low, "water only where the interpolated continentalness says sea");
                assert_eq!((w.level_m, w.body, w.kind), (0.0, WaterBodyId::SEA, LiquidKindId::WATER));
                counts[0] += 1;
            }
            None if low => {
                // Low terrain without ocean-connected vertices: a basin, or the strip next to land the rule drops.
                let pos = locate_direction(RES, &TangentWarp, d).unwrap();
                let (v, n) = pos.weighted_vertices(RES);
                assert!(v[..n].iter().all(|&(i, j)| !ocean.get(pos.face, i, j)));
                counts[1] += 1;
            }
            None => counts[2] += 1,
        }
    }
    assert!(counts[0] > 1000 && counts[2] > 1000, "the world has an ocean and land: {counts:?}");
    // The vertices themselves: an ocean vertex is water, a land vertex is not, at weight zero.
    let mut checked = (0, 0);
    for f in [0u8, 3, 4] {
        for j in (0..=RES).step_by(4) {
            for i in (0..=RES).step_by(4) {
                let s = |k: u32| 2.0 * f64::from(k) / f64::from(RES) - 1.0;
                let d = face_to_direction(&TangentWarp, Face(f), s(i), s(j));
                let w = field.water_at(d).unwrap();
                let is_ocean = ocean.get(Face(f), i, j);
                // At a vertex the interpolation is the stored sample: ocean vertices are sea, so wet.
                if is_ocean {
                    assert!(w.is_some(), "face {f} vertex ({i}, {j}) is ocean");
                    checked.0 += 1;
                } else if f64::from(grid.at(Face(f), i, j).continent) > SEA_LEVEL_C {
                    assert!(w.is_none(), "face {f} vertex ({i}, {j}) is land");
                    checked.1 += 1;
                }
            }
        }
    }
    assert!(checked.0 > 50 && checked.1 > 50, "{checked:?}");
}

/// A bake whose sea is a set of nodes (continentalness -1 sea, +1 land), as in the ocean tests.
fn grid_from_nodes(res: u32, sea_nodes: &HashSet<u64>) -> GlobalGrid {
    let index = CubeIndex::new(res);
    let n = (res + 1) as usize;
    let faces = std::array::from_fn(|f| {
        let mut v = Vec::with_capacity(n * n);
        for j in 0..=res {
            for i in 0..=res {
                let c = if sea_nodes.contains(&index.node_id(Face(f as u8), i, j)) { -1.0 } else { 1.0 };
                v.push(GlobalSample { continent: c, elevation_m: 10.0 * c, temperature_c: 0.0, precipitation_mm: 0.0 });
            }
        }
        v
    });
    GlobalGrid { seed: 0, res, faces }
}

// spec: GEN-023
#[test]
fn a_dry_basin_one_vertex_from_the_ocean_stays_dry_and_the_strip_beside_the_ocean_is_wet() {
    let res = 32;
    let index = CubeIndex::new(res);
    // Face 4: the open sea is the band i <= 10; the wall i = 11 is land; the basin is 12 <= i <= 15 and 10 <= j <= 20.
    let mut set = HashSet::new();
    for j in 0..=res {
        for i in 0..=res {
            if i <= 10 || ((12..=15).contains(&i) && (10..=20).contains(&j)) {
                set.insert(index.node_id(Face(4), i, j));
            }
        }
    }
    let grid = grid_from_nodes(res, &set);
    // The area threshold sits between the basin and the band.
    let fraction = 0.02;
    let (table, ocean) = tiled_ocean(&grid, &TangentWarp, 8, 2, fraction).unwrap();
    assert_eq!(table.ocean_components().count(), 1);
    let src = GridSource(&grid);
    let sea = SeaProducer::new(&ocean, SeaParams::EARTH);
    let field = WaterField::new(&TangentWarp, &src, vec![&sea]);
    let at = |i: f64, j: f64| {
        let s = |k: f64| 2.0 * k / f64::from(res) - 1.0;
        field.water_at(face_to_direction(&TangentWarp, Face(4), s(i), s(j))).unwrap()
    };
    assert!(at(5.0, 15.0).is_some(), "open sea");
    assert!(at(10.0, 15.0).is_some(), "the last ocean vertex");
    // Between the ocean vertex 10 and the wall 11 the continentalness is 0, at or below the sea level: sea, and the ocean vertex
    // carries weight, so it is wet. This is the strip beside the ocean.
    assert!(at(10.5, 15.0).is_some(), "the strip next to the ocean is water");
    // Between the wall (land) and the basin vertex 12: also low terrain, but no vertex with weight is ocean-connected: dry.
    assert!(at(11.5, 15.0).is_none(), "low terrain between the wall and the basin is dry");
    assert!(at(13.0, 15.0).is_none(), "the basin is dry although it is below sea level");
    assert!(at(11.0, 15.0).is_none(), "the wall is land");
    assert!(at(20.0, 15.0).is_none(), "land");
    // A basin vertex is low terrain; the rule reads the ocean flag and not the continentalness alone.
    let ground = planet_generators::global::read_direction(
        &src,
        &TangentWarp,
        face_to_direction(&TangentWarp, Face(4), 2.0 * 13.0 / 32.0 - 1.0, 2.0 * 15.0 / 32.0 - 1.0),
    )
    .unwrap();
    assert!(f64::from(ground.continent) <= SEA_LEVEL_C);
}

// spec: GEN-023
#[test]
fn the_weighted_vertices_are_the_same_nodes_from_either_face_of_a_shared_edge() {
    use planet_generators::global::LatticePos;
    let res = 16u32;
    let index = CubeIndex::new(res);
    // A point on the edge between face 0 (+x) and face 4 (+z), a quarter of the way between vertices k and k + 1 of the edge.
    // Face 0 has axes (U, V) = (y, z): its edge j = res runs along i. Face 4 has (x, y): its edge i = res runs along j, the
    // same y direction, so the lattice index is the same along the edge.
    for k in 0..res {
        for w in [0.0, 0.25, 0.5] {
            let a = LatticePos { res, face: Face(0), i: k, j: res, wu: w, wv: 0.0 };
            let b = LatticePos { res, face: Face(4), i: res, j: k, wu: 0.0, wv: w };
            let nodes = |p: &LatticePos| {
                let (v, n) = p.weighted_vertices(res);
                let mut ids: Vec<u64> = v[..n].iter().map(|&(i, j)| index.node_id(p.face, i, j)).collect();
                ids.sort_unstable();
                ids
            };
            assert_eq!(nodes(&a), nodes(&b), "edge vertex {k} + {w}");
            assert_eq!(nodes(&a).len(), if w == 0.0 { 1 } else { 2 });
        }
    }
}

/// A basin with a fixed level: the second producer of the retrofit test (ADR 0041 point 7). It lives in the test, not in the library.
struct SyntheticLake {
    centre: Vec3,
    cos_radius: f64,
    level_m: f64,
    body: WaterBodyId,
    kind: LiquidKindId,
}

impl WaterProducer for SyntheticLake {
    fn water_at(&self, q: &WaterQuery) -> Result<Option<WaterSample>, ReadError> {
        let inside = q.dir.dot(self.centre) >= self.cos_radius;
        let low = f64::from(q.ground.elevation_m) <= self.level_m && f64::from(q.ground.continent) > SEA_LEVEL_C;
        Ok((inside && low).then_some(WaterSample { level_m: self.level_m, body: self.body, kind: self.kind }))
    }
}

// spec: GEN-023
#[test]
fn a_synthetic_lake_goes_through_the_same_path_as_the_sea() {
    let (grid, ocean) = world_bake();
    let src = GridSource(&grid);
    // A land vertex with some relief, found on the bake.
    let (face, i, j) = (0..6u8)
        .flat_map(|f| (8..RES - 8).flat_map(move |j| (8..RES - 8).map(move |i| (f, i, j))))
        .find(|&(f, i, j)| {
            let s = grid.at(Face(f), i, j);
            s.continent > 0.5 && (200.0..800.0).contains(&s.elevation_m)
        })
        .expect("the bake has a highland");
    let s = |k: u32| 2.0 * f64::from(k) / f64::from(RES) - 1.0;
    let centre = face_to_direction(&TangentWarp, Face(face), s(i), s(j));
    let level_m = f64::from(grid.at(Face(face), i, j).elevation_m) + 400.0;

    // The definition: two liquids (water, lava), the sea of water, the lake of lava.
    let mut def = WorldDefinition::single_planet(6_371_000.0, SEED);
    def.set_liquid(LiquidKindId(0), &LiquidKind::WATER).unwrap();
    let lava = LiquidKind {
        colour: [0.8, 0.1, 0.02],
        emission: [40.0, 9.0, 0.5],
        viscosity_pa_s: 1.0e3,
        wave_amplitude_m: 0.0,
        wave_length_m: 0.0,
    };
    def.set_liquid(LiquidKindId(1), &lava).unwrap();
    let liquids = def.liquids().unwrap();
    let sea_params = def.sea(BodyId::PLANET).unwrap();

    let sea = SeaProducer::new(&ocean, sea_params);
    let lake = SyntheticLake { centre, cos_radius: libm::cos(0.05), level_m, body: WaterBodyId(2), kind: LiquidKindId(1) };
    let only_sea = WaterField::new(&TangentWarp, &src, vec![&sea]);
    let both = WaterField::new(&TangentWarp, &src, vec![&lake, &sea]);

    // The lake: found at its centre through the same query, with its level, body and kind; its kind is a row of the table.
    let w = both.water_at(centre).unwrap().expect("water at the centre of the lake");
    assert_eq!((w.level_m, w.body, w.kind), (level_m, WaterBodyId(2), LiquidKindId(1)));
    assert!(liquids.get(w.kind).unwrap().emission[0] > 0.0, "the lake is lava: it emits");
    assert!(only_sea.water_at(centre).unwrap().is_none(), "without the producer the centre is dry land");
    assert_eq!(both.level_at(centre).unwrap(), Some(level_m));

    // Everywhere else the answers are the same: the sea is untouched by the lake, and the lake adds water only on land.
    let mut lake_points = 0;
    let mut sea_points = 0;
    for d in directions(6000) {
        let (a, b) = (only_sea.water_at(d).unwrap(), both.water_at(d).unwrap());
        match b {
            Some(w) if w.body == WaterBodyId(2) => {
                lake_points += 1;
                assert!(a.is_none(), "a lake never replaces sea");
                assert!(d.dot(centre) >= libm::cos(0.05));
            }
            Some(w) => {
                sea_points += 1;
                assert_eq!(Some(w), a);
                assert_eq!(w.kind, LiquidKindId::WATER);
            }
            None => assert!(a.is_none()),
        }
    }
    assert!(sea_points > 1000, "{sea_points}");
    assert!(lake_points >= 1, "the lake holds some of the sampled directions: {lake_points}");
    // A footprint sampled on a ring around the centre has water somewhere and dry land somewhere else.
    let wet_ring = (0..64)
        .filter(|k| {
            let a = f64::from(*k) / 64.0 * core::f64::consts::TAU;
            let tangent = centre.cross(Vec3::new(0.3, 0.5, 0.8)).normalized();
            let other = centre.cross(tangent);
            let d = (centre * libm::cos(0.03) + (tangent * libm::cos(a) + other * libm::sin(a)) * libm::sin(0.03)).normalized();
            both.water_at(d).unwrap().is_some()
        })
        .count();
    assert!(wet_ring > 0 && wet_ring < 64, "the lake covers part of its footprint: {wet_ring} of 64");
    // Far from the lake the lake is absent.
    assert!(both.water_at(centre * -1.0).unwrap().is_none_or(|w| w.body == WaterBodyId::SEA));

    // The plan side (GPU-free): the plan's water at the camera is the lake's level over the lake, the sea's level elsewhere.
    let over_lake = both.plan_water(centre).unwrap();
    assert_eq!(over_lake, PlanWater { level_m, water: Some((WaterBodyId(2), LiquidKindId(1))) });
    let far_land = directions(400).into_iter().find(|d| both.water_at(*d).unwrap().is_none()).unwrap();
    assert_eq!(both.plan_water(far_land).unwrap(), PlanWater { level_m: 0.0, water: None }, "dry places shade against the sea level");
    let wet = directions(400).into_iter().find(|d| matches!(both.water_at(*d).unwrap(), Some(w) if w.body == WaterBodyId::SEA)).unwrap();
    assert_eq!(both.plan_water(wet).unwrap(), PlanWater { level_m: 0.0, water: Some((WaterBodyId::SEA, LiquidKindId::WATER)) });
    // A sea at another level moves the plan's reference with it, and no producer at all shades against 0.
    let high_sea = SeaProducer::new(&ocean, SeaParams { level_m: 12.5, ..SeaParams::EARTH });
    let field = WaterField::new(&TangentWarp, &src, vec![&high_sea]);
    assert_eq!(field.plan_water(far_land).unwrap().level_m, 12.5);
    assert_eq!(field.water_at(wet).unwrap().unwrap().level_m, 12.5);
    assert_eq!(WaterField::new(&TangentWarp, &src, vec![]).plan_water(far_land).unwrap().level_m, 0.0);
}

// spec: GEN-023
#[test]
fn the_field_is_a_function_of_the_direction_alone_and_reads_a_bake_of_the_resolution_it_was_made_for() {
    let (grid, ocean) = world_bake();
    let src = GridSource(&grid);
    let sea = SeaProducer::new(&ocean, SeaParams::EARTH);
    let field = WaterField::new(&TangentWarp, &src, vec![&sea]);
    let dirs = directions(3000);
    let one: Vec<[u64; 3]> = dirs.iter().map(|d| word(field.water_at(*d).unwrap())).collect();
    for threads in [2usize, 7] {
        let parts: Vec<Vec<[u64; 3]>> = std::thread::scope(|s| {
            let chunk = dirs.len().div_ceil(threads);
            let hs: Vec<_> = dirs
                .chunks(chunk)
                .map(|c| {
                    let field = &field;
                    s.spawn(move || c.iter().map(|d| word(field.water_at(*d).unwrap())).collect::<Vec<_>>())
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert_eq!(one, parts.into_iter().flatten().collect::<Vec<_>>(), "{threads} threads");
    }
    // An ocean channel of another resolution is an error naming both, not a wrong answer.
    let coarse_grid = bake(SEED, &TangentWarp, 64, 2).unwrap();
    let (_, coarse_ocean) = tiled_ocean(&coarse_grid, &TangentWarp, 32, 1, EARTH_OCEAN_MIN_AREA_FRACTION).unwrap();
    let wrong = SeaProducer::new(&coarse_ocean, SeaParams::EARTH);
    let e = WaterField::new(&TangentWarp, &src, vec![&wrong]).water_at(dirs[0]).unwrap_err().to_string();
    assert!(e.contains("64-cell") && e.contains("128-cell"), "{e}");
    assert_eq!(OceanLattice::res(&ocean), RES);
}

// spec: GEN-023
#[test]
fn known_answers_of_the_field_are_pinned() {
    assert_eq!((WATER_VERSION, WATER_IMPLEMENTATION_ID), (1, "cpu-water-level-v1"));
    let (grid, ocean) = world_bake();
    let src = GridSource(&grid);
    let sea = SeaProducer::new(&ocean, SeaParams::EARTH);
    let field = WaterField::new(&TangentWarp, &src, vec![&sea]);
    let dirs = directions(4096);
    let words: Vec<u64> = dirs.iter().flat_map(|d| word(field.water_at(*d).unwrap())).collect();
    let wet = dirs.iter().filter(|d| field.water_at(**d).unwrap().is_some()).count();
    let h = hash_u64s(0x7761_7465_6b61, &words);
    assert_eq!((wet, h), (KNOWN_WET, KNOWN_HASH));
}

const KNOWN_WET: usize = 3068;
const KNOWN_HASH: u64 = 0xc93a_0500_e6ca_f961;

// spec: GEN-023
#[test]
fn directions_on_face_edges_and_cube_corners_are_pinned_and_agree_with_the_vertex_rule() {
    let (grid, ocean) = world_bake();
    let src = GridSource(&grid);
    let sea = SeaProducer::new(&ocean, SeaParams::EARTH);
    let field = WaterField::new(&TangentWarp, &src, vec![&sea]);
    let mut words = Vec::new();
    let mut wet = 0;
    for f in 0..6u8 {
        for (s, t) in
            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (-1.0, 0.3), (1.0, -0.7), (0.4, -1.0), (-0.2, 1.0), (-1.0, 1.0 / 128.0)]
        {
            let d = face_to_direction(&TangentWarp, Face(f), s, t);
            let w = field.water_at(d).unwrap();
            // On the lattice (a corner, or a vertex on an edge) the rule is the vertex's own ocean bit when it is sea.
            let pos = locate_direction(RES, &TangentWarp, d).unwrap();
            if pos.wu == 0.0 && pos.wv == 0.0 {
                let sea_vertex = f64::from(grid.at(pos.face, pos.i, pos.j).continent) <= SEA_LEVEL_C;
                assert_eq!(w.is_some(), sea_vertex && ocean.get(pos.face, pos.i, pos.j), "face {f} ({s}, {t})");
            }
            wet += usize::from(w.is_some());
            words.extend(word(w));
        }
    }
    let h = hash_u64s(0x6564_6765, &words);
    assert_eq!((wet, h), (KNOWN_EDGE_WET, KNOWN_EDGE_HASH));
}

const KNOWN_EDGE_WET: usize = 42;
const KNOWN_EDGE_HASH: u64 = 17169129414116492182;
