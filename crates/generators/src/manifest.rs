//! Determinism manifest (GEN-003, ADR 0010): one line per checked output, written by `planet generate --manifest`
//! on every CI operating system and diffed by `cargo xtask det-diff`. A line is `<kind> <key> <hash>`; the
//! header lines carry the versions, so a manifest from a different build is reported as such, not as drift.

use planet_core::cube::TangentWarp;
use planet_core::hash::{hash_u64s, HASH_VERSION};
use planet_core::{Face, Geo, ReferenceSurface, Sphere, TileId};

use crate::height::{cache_key, generate_tile, GENERATOR_VERSION, IMPLEMENTATION_ID, KEY_FORMAT_VERSION};
use crate::mesh::{mesh_key, tile_mesh, MESH_VERSION};

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
    let mut s = String::new();
    s.push_str(&format!("version generator {GENERATOR_VERSION}\nversion hash {HASH_VERSION}\nversion mesh {MESH_VERSION}\nversion key {KEY_FORMAT_VERSION}\n"));
    s.push_str(&format!("version implementation {IMPLEMENTATION_ID}\n"));
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
    }
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
