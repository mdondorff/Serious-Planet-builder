//! Determinism manifest (GEN-003, ADR 0010): one line per checked output, written by `planet generate --manifest`
//! on every CI operating system and diffed by `cargo xtask det-diff`. A line is `<kind> <key> <hash>`; the
//! header lines carry the versions, so a manifest from a different build is reported as such, not as drift.

use planet_core::cube::{face_to_direction, TangentWarp};
use planet_core::hash::{hash_u64s, HASH_VERSION};
use planet_core::{Face, Geo, ReferenceSurface, Sphere, TileId};

use crate::erosion::{ErosionGenerator, EROSION_IMPLEMENTATION_ID, EROSION_VERSION};
use crate::global::{
    bake, bake_blocks, grid_hash, GlobalGrid, GlobalSample, GridSource, GLOBAL_VERSION, IMPLEMENTATION_ID as GLOBAL_IMPLEMENTATION_ID,
};
use crate::terrain::{TerrainGenerator, TERRAIN_IMPLEMENTATION_ID, TERRAIN_VERSION};

use crate::height::{cache_key, generate_tile, GENERATOR_VERSION, IMPLEMENTATION_ID, KEY_FORMAT_VERSION};
use crate::height::{HeightField, HeightGenerator, ReferenceGenerator};
use crate::mesh::{mesh_key, mesh_key_field, tile_mesh, MESH_VERSION};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

/// Seed, grid size and radius of every manifest entry.
pub const MANIFEST_SEED: u64 = 1;
pub const MANIFEST_RES: u32 = 16;
pub const MANIFEST_RADIUS_M: f64 = 6_371_000.0;

/// Tiles in the manifest: every face at level 0, the four corner tiles of every face at level 3 (cube edges and
/// corners), and deep tiles up to level 28.
pub fn manifest_tiles() -> Vec<TileId> {
    let mut ids = Vec::new();
    for f in Face::ALL {
        ids.push(TileId::new(f, 0, 0, 0).expect("level 0"));
        for (x, y) in [(0, 0), (7, 0), (0, 7), (7, 7)] {
            ids.push(TileId::new(f, 3, x, y).expect("level 3"));
        }
    }
    for (f, level, x, y) in [(0, 8, 17, 200), (2, 12, 1000, 77), (4, 16, 40_000, 12_345), (5, 20, 1 << 19, 3), (3, 28, 123_456, 654_321)] {
        ids.push(TileId::new(Face(f), level, x, y).expect("deep tile"));
    }
    ids
}

/// The global bake of the manifest: 64 cells per face edge, hashed whole and as 16-cell blocks of level 2 (16 per face).
pub const MANIFEST_BAKE_RES: u32 = 64;
pub const MANIFEST_BLOCK_LEVEL: u8 = 2;
pub const MANIFEST_BLOCK_CELLS: u32 = 16;
/// Stand-in for the bake's 128-bit stage key (STRM-011, which lives in `cache`): it enters the fields' `inputs_hash`.
pub const MANIFEST_BAKE_KEY: [u8; 16] = [0x5a; 16];

/// Face coordinates `(s, t)` at which both terrain fields are evaluated on every face: the four corners, the four edge
/// midpoints and the centre (cube corners and face edges, where the lattice and the tangent bases change), an interior
/// point, a point on an edge off its middle, and a point a hair inside a corner.
pub const MANIFEST_FIELD_POINTS: [(f64, f64); 12] = [
    (-1.0, -1.0),
    (1.0, -1.0),
    (-1.0, 1.0),
    (1.0, 1.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (0.0, 0.0),
    (0.3, -0.7),
    (-1.0, 0.123_456_789),
    (0.999_999, 0.999_999),
];

/// A float one ulp away (the lowest bit flipped): the smallest change a mutation check can make.
fn one_ulp_f64(x: f64) -> f64 {
    f64::from_bits(x.to_bits() ^ 1)
}

fn one_ulp_f32(x: f32) -> f32 {
    f32::from_bits(x.to_bits() ^ 1)
}

/// One ulp of one channel (`variant % 4`: continent, elevation, temperature, precipitation) of the first (`variant < 4`) or
/// the last sample of a block.
fn perturb_sample(block: &mut [GlobalSample], variant: usize) {
    let i = if variant < 4 { 0 } else { block.len() - 1 };
    let sm = &mut block[i];
    let c = match variant % 4 {
        0 => &mut sm.continent,
        1 => &mut sm.elevation_m,
        2 => &mut sm.temperature_c,
        _ => &mut sm.precipitation_mm,
    };
    *c = one_ulp_f32(*c);
}

/// Hash of every bit of every channel of a block, in order.
fn block_hash(block: &[GlobalSample]) -> u64 {
    let words: Vec<u64> = block
        .iter()
        .flat_map(|s| {
            [
                u64::from(s.continent.to_bits()) << 32 | u64::from(s.elevation_m.to_bits()),
                u64::from(s.temperature_c.to_bits()) << 32 | u64::from(s.precipitation_mm.to_bits()),
            ]
        })
        .collect();
    hash_u64s(0x626c_6f63, &words)
}

/// Geo points for the reference-surface round trip: poles, the antimeridian, and a coarse lat/lon grid.
fn manifest_geos() -> Vec<Geo> {
    let mut g = vec![Geo::new(90.0, 0.0, 0.0), Geo::new(-90.0, 0.0, 0.0), Geo::new(0.0, 180.0, 0.0), Geo::new(0.0, -180.0, 8_848.0)];
    for lat in (-75..=75).step_by(25) {
        for lon in (-165..=165).step_by(30) {
            g.push(Geo::new(f64::from(lat) + 0.123_456_789, f64::from(lon) - 0.987_654_321, f64::from(lat * 37)));
        }
    }
    g
}

/// The manifest text. Pure and single-threaded: the same build gives the same bytes on every platform (CON-14).
pub fn determinism_manifest() -> String {
    determinism_manifest_perturbed(&|_, _| None)
}

/// [`determinism_manifest`] with a one-ulp change in the value behind every new entry (`globalblock`, `globalgrid`,
/// `terrain`, `erosion`) for which `perturb(kind, key)` is `Some`: the mutation check of GEN-003 (a test shows that a one-ulp
/// change in any of these values changes the manifest line, so the hash really covers the value's bits).
pub fn determinism_manifest_perturbed(perturb: &dyn Fn(&str, &str) -> Option<usize>) -> String {
    let mut s = String::new();
    s.push_str(&format!("version generator {GENERATOR_VERSION}\nversion hash {HASH_VERSION}\nversion mesh {MESH_VERSION}\nversion key {KEY_FORMAT_VERSION}\n"));
    s.push_str(&format!("version implementation {IMPLEMENTATION_ID}\n"));
    s.push_str(&format!(
        "version global {GLOBAL_IMPLEMENTATION_ID} {GLOBAL_VERSION}\nversion terrain {TERRAIN_IMPLEMENTATION_ID} {TERRAIN_VERSION}\n"
    ));
    s.push_str(&format!("version erosion {EROSION_IMPLEMENTATION_ID} {EROSION_VERSION}\n"));
    // A moon built through the generator: the body id, the derived seed and the read set are part of its keys.
    let mut def = WorldDefinition::single_planet(MANIFEST_RADIUS_M, MANIFEST_SEED);
    def.set_body_radius(BodyId(1), 1_737_400.0);
    let moon: Box<dyn HeightField> =
        ReferenceGenerator.field(&GenContext::new(&def), BodyId(1)).expect("the moon's seed derives from the world seed");
    for id in manifest_tiles() {
        let key = format!("{}/{}/{}/{}", id.face().0, id.level(), id.x(), id.y());
        let h = generate_tile(MANIFEST_SEED, &TangentWarp, id, MANIFEST_RES).content_hash();
        s.push_str(&format!("height {key} {h:016x}\n"));
        let m = tile_mesh(MANIFEST_SEED, &TangentWarp, id, MANIFEST_RES, MANIFEST_RADIUS_M).content_hash();
        s.push_str(&format!("mesh {key} {m:016x}\n"));
        // Cache keys (ADR 0006, 0018): the persistent key's inputs must agree on every platform too.
        let hk = cache_key(&TangentWarp, MANIFEST_SEED, id, MANIFEST_RES);
        s.push_str(&format!("heightkey {key} {hk:016x}\n"));
        let mk = mesh_key(&TangentWarp, MANIFEST_SEED, id, MANIFEST_RES, MANIFEST_RADIUS_M);
        s.push_str(&format!("meshkey {key} {mk:016x}\n"));
        let mhk = crate::height::cache_key_field(moon.as_ref(), &TangentWarp, id, MANIFEST_RES);
        s.push_str(&format!("moonheightkey {key} {mhk:016x}\n"));
        let mmk = mesh_key_field(moon.as_ref(), &TangentWarp, id, MANIFEST_RES, 1_737_400.0);
        s.push_str(&format!("moonmeshkey {key} {mmk:016x}\n"));
    }
    s.push_str(&GlobalFieldValues::compute().entries(perturb));
    let sphere = Sphere { radius_m: MANIFEST_RADIUS_M };
    for (i, g) in manifest_geos().into_iter().enumerate() {
        let p = sphere.geo_to_planet_fixed(g);
        let back = sphere.planet_fixed_to_geo(p);
        let n = sphere.normal(p);
        let bits = [p.0.x, p.0.y, p.0.z, back.lat_deg, back.lon_deg, back.height_m, n.x, n.y, n.z].map(f64::to_bits);
        s.push_str(&format!("geo {i} {:016x}\n", hash_u64s(0x0067_656f, &bits)));
    }
    s
}

/// The values behind the global bake and field entries, computed once so that a mutation check can render the entries many
/// times without baking again.
pub struct GlobalFieldValues {
    grid: GlobalGrid,
    blocks: Vec<(TileId, Vec<GlobalSample>)>,
    /// `(kind, key, height)` of the terrain field and then the eroded field.
    heights: Vec<(&'static str, String, f64)>,
}

impl GlobalFieldValues {
    /// The global bake and the two height fields over it (GEN-014, GEN-016, GEN-017), single-threaded.
    pub fn compute() -> GlobalFieldValues {
        let def = WorldDefinition::single_planet(MANIFEST_RADIUS_M, MANIFEST_SEED);
        let seed = GenContext::new(&def).body_seed(BodyId::PLANET).expect("the planet's seed");
        let grid = bake(seed, &TangentWarp, MANIFEST_BAKE_RES, 1).expect("a power-of-two resolution");
        let mut blocks = Vec::new();
        bake_blocks(seed, &TangentWarp, MANIFEST_BLOCK_LEVEL, MANIFEST_BLOCK_CELLS, 1, &mut |id, b| blocks.push((id, b)))
            .expect("a valid tiled bake");
        let src = GridSource(&grid);
        let terrain = TerrainGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, MANIFEST_BAKE_KEY).expect("seed");
        let eroded = ErosionGenerator.field(&GenContext::new(&def), BodyId::PLANET, &src, MANIFEST_BAKE_KEY).expect("seed and radius");
        let mut heights = Vec::new();
        for (kind, field) in [("terrain", &terrain as &dyn HeightField), ("erosion", &eroded as &dyn HeightField)] {
            for f in 0..6u8 {
                for (st, tt) in MANIFEST_FIELD_POINTS {
                    heights.push((kind, format!("{f}/{st:?}/{tt:?}"), field.height(face_to_direction(&TangentWarp, Face(f), st, tt))));
                }
            }
        }
        assert!(terrain.error().is_none() && eroded.error().is_none(), "the manifest's bake reads must succeed");
        assert!(heights.iter().all(|h| h.2.is_finite()), "a NaN height would pass the cross-OS diff silently");
        GlobalFieldValues { grid, blocks, heights }
    }

    /// The manifest lines: the bake hashed whole (`globalgrid`) and block by block (`globalblock`), then the heights
    /// (`terrain`, `erosion`) as the raw bits of the `f64`. Where `perturb(kind, key)` returns a variant, the value behind
    /// that line is changed by one ulp first (grid and blocks: variant 0 to 7 pick a channel and the first or last sample) (the mutation check of GEN-003; the other lines are unaffected).
    pub fn entries(&self, perturb: &dyn Fn(&str, &str) -> Option<usize>) -> String {
        let mut s = String::new();
        let grid_key = format!("res{MANIFEST_BAKE_RES}");
        let h = if let Some(v) = perturb("globalgrid", &grid_key) {
            let mut g = self.grid.clone();
            let face = if v < 4 { 0 } else { 5 };
            perturb_sample(&mut g.faces[face], v);
            grid_hash(&g)
        } else {
            grid_hash(&self.grid)
        };
        s.push_str(&format!("globalgrid {grid_key} {h:016x}\n"));
        for (id, block) in &self.blocks {
            let key = format!("{}/{}/{}/{}", id.face().0, id.level(), id.x(), id.y());
            let h = if let Some(v) = perturb("globalblock", &key) {
                let mut b = block.clone();
                perturb_sample(&mut b, v);
                block_hash(&b)
            } else {
                block_hash(block)
            };
            s.push_str(&format!("globalblock {key} {h:016x}\n"));
        }
        for (kind, key, h) in &self.heights {
            let h = if perturb(kind, key).is_some() { one_ulp_f64(*h) } else { *h };
            s.push_str(&format!("{kind} {key} {:016x}\n", h.to_bits()));
        }
        s
    }
}
