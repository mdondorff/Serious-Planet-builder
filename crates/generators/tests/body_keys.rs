//! GEN-012: the body (id, seed, radius, reference surface) is part of every height and mesh key (ADR 0001, 0007).

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId, Vec3};
use planet_generators::height::{cache_key_field, HeightField, Reference, KEY_FORMAT_VERSION};
use planet_generators::mesh::{mesh_key_field, MeshKeyer};
use planet_world_def::{body_params_hash, BodyId, SPHERE_SURFACE_ID};

/// A field identical to `Reference(seed)` in everything but the reference surface it names.
struct OnSurface(Reference, &'static str);

impl HeightField for OnSurface {
    fn height(&self, dir: Vec3) -> f64 {
        self.0.height(dir)
    }
    fn seed(&self) -> u64 {
        self.0.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.0.implementation_id()
    }
    fn version(&self) -> u32 {
        self.0.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.max_abs_height_m()
    }
    fn surface_id(&self) -> &'static str {
        self.1
    }
}

// spec: GEN-012
#[test]
fn the_reference_surface_alone_changes_the_height_key_and_the_mesh_key() {
    let sphere = OnSurface(Reference(7), SPHERE_SURFACE_ID);
    let ellipsoid = OnSurface(Reference(7), "ellipsoid");
    let hk = |f: &dyn HeightField| cache_key_field(f, &TangentWarp, tile(), 16);
    assert_eq!(hk(&sphere), hk(&Reference(7)));
    assert_ne!(hk(&sphere), hk(&ellipsoid));
    assert_ne!(mesh_key_field(&sphere, &TangentWarp, tile(), 16, R), mesh_key_field(&ellipsoid, &TangentWarp, tile(), 16, R));
}

const R: f64 = 6_371_000.0;

fn tile() -> TileId {
    TileId::new(Face(1), 6, 9, 4).unwrap()
}

/// A field identical to `Reference(seed)` in everything but the body it claims to describe.
struct AsBody(Reference, BodyId);

impl HeightField for AsBody {
    fn height(&self, dir: Vec3) -> f64 {
        self.0.height(dir)
    }
    fn seed(&self) -> u64 {
        self.0.seed()
    }
    fn implementation_id(&self) -> &'static str {
        self.0.implementation_id()
    }
    fn version(&self) -> u32 {
        self.0.version()
    }
    fn max_abs_height_m(&self) -> f64 {
        self.0.max_abs_height_m()
    }
    fn body(&self) -> BodyId {
        self.1
    }
}

// spec: GEN-012
#[test]
fn the_body_id_alone_changes_the_height_key_and_the_mesh_key() {
    let planet = AsBody(Reference(7), BodyId::PLANET);
    let moon = AsBody(Reference(7), BodyId(1));
    let other_moon = AsBody(Reference(7), BodyId(2));
    // Same seed, same inputs, same tile, same radius: only the body differs.
    assert_eq!(planet.inputs_hash(), moon.inputs_hash());
    let hk = |f: &dyn HeightField| cache_key_field(f, &TangentWarp, tile(), 16);
    let mk = |f: &dyn HeightField| mesh_key_field(f, &TangentWarp, tile(), 16, R);
    assert_ne!(hk(&planet), hk(&moon));
    assert_ne!(hk(&moon), hk(&other_moon));
    assert_ne!(mk(&planet), mk(&moon));
    assert_ne!(mk(&moon), mk(&other_moon));
    // The planet through the trait default is the planet field built directly from the seed.
    assert_eq!(hk(&planet), hk(&Reference(7)));
    assert_eq!(mk(&planet), mk(&Reference(7)));
}

// spec: GEN-012
#[test]
fn every_body_parameter_changes_the_body_parameter_hash() {
    let base = body_params_hash(BodyId(1), 5, R, SPHERE_SURFACE_ID);
    assert_eq!(base, body_params_hash(BodyId(1), 5, R, SPHERE_SURFACE_ID));
    assert_ne!(base, body_params_hash(BodyId(2), 5, R, SPHERE_SURFACE_ID));
    assert_ne!(base, body_params_hash(BodyId(1), 6, R, SPHERE_SURFACE_ID));
    assert_ne!(base, body_params_hash(BodyId(1), 5, R + 1.0, SPHERE_SURFACE_ID));
    assert_ne!(base, body_params_hash(BodyId(1), 5, -R, SPHERE_SURFACE_ID));
    assert_ne!(base, body_params_hash(BodyId(1), 5, R, "ellipsoid"));
}

// spec: GEN-012
#[test]
fn the_mesh_key_follows_the_radius_through_the_body_parameter_hash() {
    let f = AsBody(Reference(7), BodyId(1));
    let a = mesh_key_field(&f, &TangentWarp, tile(), 16, R);
    assert_eq!(a, mesh_key_field(&f, &TangentWarp, tile(), 16, R));
    assert_ne!(a, mesh_key_field(&f, &TangentWarp, tile(), 16, 1_737_400.0));
}

// spec: GEN-012
#[test]
fn the_batch_keyer_matches_the_single_key_for_other_bodies() {
    for body in [BodyId::PLANET, BodyId(1), BodyId(7)] {
        let f = AsBody(Reference(3), body);
        let keyer = MeshKeyer::new(&f, &TangentWarp, 16, R);
        for t in [tile(), TileId::new(Face(4), 2, 1, 3).unwrap()] {
            assert_eq!(keyer.key(t), mesh_key_field(&f, &TangentWarp, t, 16, R), "{body:?} {t:?}");
        }
    }
}

// spec: GEN-012
#[test]
fn the_key_format_version_is_the_one_the_body_fields_belong_to() {
    // New inputs in a key bump the version; a known answer pins the composition for the planet.
    assert_eq!(KEY_FORMAT_VERSION, 2, "a new key composition needs a new version and new known answers");
    let planet = Reference(7);
    let expect_h = cache_key_field(&planet, &TangentWarp, tile(), 16);
    let expect_m = mesh_key_field(&planet, &TangentWarp, tile(), 16, R);
    assert_eq!((expect_h, expect_m), (KNOWN_HEIGHT_KEY, KNOWN_MESH_KEY));
}

const KNOWN_HEIGHT_KEY: u64 = 16898139709241435207;
const KNOWN_MESH_KEY: u64 = 17514838993888437677;
