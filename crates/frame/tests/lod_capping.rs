//! LOD-008 (ADR 0074): a tile whose neighbourhood is too steep to morph is not refined; the plan degrades locally. On a field with a spike
//! of the owner's kind (220 m over 100 m, about 73 degrees) the plan is not refused whole, no leaf is unmorphable, the selection is
//! balanced, the cover equals the plan with every tile of R resident, and the draw set of every view stays inside R. Tier A, no GPU.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::lod::{balance_leaves, morph_room};
use planet_frame::pipeline::{ResidentRecords, ResidentTile};
use planet_frame::{
    cover_selection_bounded, plan_terrain_field, residency_set_field, scripted_views_field, Camera, LodParams, SelectionCache,
    TerrainRequest, TerrainView, TileBounds,
};
use planet_generators::height::{HeightField, Reference};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1280, 720);

/// One hill of `height_m` over `radius_m` of ground: the profile `h (1 - x^2)^2`, whose bounds over a cap are exact (it is monotone in the
/// distance from `at`).
struct Spike {
    at: Vec3,
    height_m: f64,
    radius_m: f64,
}

impl Spike {
    fn x(&self, d_rad: f64) -> f64 {
        d_rad * R / self.radius_m
    }
}

fn angle_between(a: Vec3, b: Vec3) -> f64 {
    libm::atan2(a.cross(b).length(), a.dot(b))
}

fn weight(x: f64) -> f64 {
    if x >= 1.0 {
        0.0
    } else {
        let u = 1.0 - x * x;
        u * u
    }
}

impl HeightField for Spike {
    fn height(&self, dir: Vec3) -> f64 {
        self.height_m * weight(self.x(angle_between(self.at, dir)))
    }
    fn seed(&self) -> u64 {
        1
    }
    fn implementation_id(&self) -> &'static str {
        "test-spike"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        self.height_m
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        let d = angle_between(self.at, centre);
        let near = self.x((d - angular_radius).max(0.0)) * (1.0 - 1e-9);
        let far = self.x(d + angular_radius) * (1.0 + 1e-9);
        (self.height_m * weight(far.min(1.0)) - 1e-9, self.height_m * weight(near) + 1e-9)
    }
    fn inputs_hash(&self) -> u64 {
        7
    }
}

fn lod() -> LodParams {
    LodParams { viewport_h_px: f64::from(SIZE.1), tau_px: 6.0, min_level: 0, max_height_m: 9_000.0, ..LodParams::earth_1080p() }
}

fn request(camera: Camera) -> TerrainRequest {
    TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 1 }
}

fn view(name: &str) -> Camera {
    scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &Reference(1)).into_iter().find(|(n, _)| *n == name).unwrap().1
}

/// A spike under the camera of the view `name`, sized to the levels that view reaches: the owner's 220 m over 100 m at the ground, ten times
/// that (and a little steeper) from 200 m up, so that the tiles the plan reaches are the ones too steep to morph.
fn spike_under(name: &str, camera: &Camera) -> Spike {
    let (height_m, radius_m) = if name == "ground-1m" { (220.0, 100.0) } else { (3_000.0, 1_000.0) };
    Spike { at: camera.position.0.normalized(), height_m, radius_m }
}

fn turned(position: PlanetFixed, yaw: f64, pitch: f64) -> Camera {
    let hint = Vec3::new(libm::cos(yaw), libm::sin(yaw), 0.37 * libm::sin(2.0 * yaw) + 0.2);
    Camera::at_surface_point(position, hint, pitch, 60f64.to_radians(), f64::from(SIZE.0) / f64::from(SIZE.1), 0.1)
}

// spec: LOD-008
#[test]
fn a_spike_too_steep_to_morph_is_capped_not_refused_and_every_leaf_of_the_plan_is_morphable_and_balanced() {
    for name in ["ground-1m", "ground-200m"] {
        let camera = view(name);
        let spike = spike_under(name, &camera);
        let lod = lod();
        // The ground itself: 220 m over 100 m has a slope of 3.4 against the 2.5 the parameters can draw.
        assert!(1.5396 * spike.height_m / spike.radius_m > lod.drawable_slope());
        let plan = plan_terrain_field(&request(camera), &spike).unwrap_or_else(|e| panic!("{name}: {e}"));
        let capped = plan.nodes.iter().filter(|n| n.capped).count();
        assert!(capped > 0, "{name}: the tiles over the spike are capped");
        let floor = lod.floor_level();
        for n in &plan.nodes {
            if n.level > floor {
                assert!(
                    morph_room(f64::from(n.morph_start_m), f64::from(n.morph_end_m), 1e-6),
                    "{name}: tile {:?} at level {} has no morph interval ({} to {})",
                    n.tile,
                    n.level,
                    n.morph_start_m,
                    n.morph_end_m
                );
            }
        }
        let mut leaves: BTreeSet<TileId> = plan.nodes.iter().map(|n| n.tile).collect();
        let before = leaves.clone();
        balance_leaves(&TangentWarp, &mut leaves);
        assert_eq!(leaves, before, "{name}: the selection is balanced");
        // Nothing is capped where nothing is steep.
        let flat = plan_terrain_field(&request(view(name)), &Reference(1)).unwrap();
        assert!(flat.nodes.iter().all(|n| !n.capped), "{name}: the reference terrain has no capped tile");
    }
}

/// R of a position over `field`, every tile of it resident, with the bounds the residency set carries.
fn resident_r(position: PlanetFixed, field: &dyn HeightField) -> (HashMap<TileId, ResidentTile>, TileBounds, BTreeSet<TileId>) {
    let lod = lod();
    let r = residency_set_field(&TangentWarp, position, field, &lod, &mut SelectionCache::new());
    let records = ResidentRecords::new(field, &TangentWarp, &lod);
    let tiles = r.tiles.iter().map(|e| e.tile).collect();
    (r.tiles.iter().map(|e| (e.tile, records.record(e.tile))).collect(), r.bounds, tiles)
}

fn levels(tiles: impl IntoIterator<Item = TileId>) -> BTreeMap<TileId, u8> {
    tiles.into_iter().map(|t| (t, t.level())).collect()
}

// spec: STRM-015, STRM-016, LOD-008
#[test]
fn with_every_tile_of_r_resident_the_cover_equals_the_plan_node_for_node_on_a_spike_field_and_the_draw_set_stays_inside_r() {
    for name in ["ground-1m", "ground-200m"] {
        let position = view(name).position;
        let spike = spike_under(name, &view(name));
        let (set, bounds, in_r) = resident_r(position, &spike);
        for k in 0..8u32 {
            let camera = turned(position, f64::from(k) * 0.8, [-0.4, -1.0, 0.2][k as usize % 3]);
            let plan = plan_terrain_field(&request(camera), &spike).unwrap();
            let plan_levels = levels(plan.nodes.iter().map(|n| n.tile));
            let cover = cover_selection_bounded(&TangentWarp, &camera, &lod(), &|t| set.get(&t).map(|r| r.origin), &bounds);
            let cover_levels = levels(cover.iter().copied());
            let only = |a: &BTreeMap<TileId, u8>, b: &BTreeMap<TileId, u8>| {
                a.iter().filter(|(t, _)| !b.contains_key(t)).take(6).map(|(t, l)| format!("{t:?}@{l}")).collect::<Vec<_>>()
            };
            assert!(
                cover_levels == plan_levels,
                "{name}, view {k}: cover {} tiles, plan {}; only in the cover {:?}; only in the plan {:?}",
                cover_levels.len(),
                plan_levels.len(),
                only(&cover_levels, &plan_levels),
                only(&plan_levels, &cover_levels)
            );
            // LOD-007: the draw set of any view from the position lies inside R.
            assert!(plan.nodes.iter().all(|n| in_r.contains(&n.tile)), "{name}, view {k}: a drawn tile is not in R");
        }
    }
}

/// A field whose bounds are not a number for every cap that touches a region of 300 m round `at` (a failed bake read), over `relief_m` of
/// heights (the reference's, or none).
struct NanBounds {
    inner: Reference,
    at: Vec3,
    flat: bool,
}

impl HeightField for NanBounds {
    fn height(&self, dir: Vec3) -> f64 {
        if self.flat {
            0.0
        } else {
            self.inner.height(dir)
        }
    }
    fn seed(&self) -> u64 {
        self.inner.seed()
    }
    fn implementation_id(&self) -> &'static str {
        "test-nan-bounds"
    }
    fn version(&self) -> u32 {
        1
    }
    fn max_abs_height_m(&self) -> f64 {
        if self.flat {
            0.0
        } else {
            self.inner.max_abs_height_m()
        }
    }
    fn height_bounds(&self, centre: Vec3, angular_radius: f64) -> (f64, f64) {
        if angle_between(self.at, centre) < 300.0 / R + angular_radius {
            (f64::NAN, f64::NAN)
        } else if self.flat {
            (0.0, 0.0)
        } else {
            self.inner.height_bounds(centre, angular_radius)
        }
    }
    fn inputs_hash(&self) -> u64 {
        9 + u64::from(self.flat)
    }
}

// spec: LOD-008
#[test]
fn bounds_that_are_not_a_number_leave_no_morph_interval_and_no_plan_is_refused_whole_for_them() {
    for flat in [false, true] {
        for name in ["ground-1m", "ground-200m"] {
            let camera = view(name);
            let field = NanBounds { inner: Reference(1), at: camera.position.0.normalized(), flat };
            let plan = plan_terrain_field(&request(camera), &field).unwrap_or_else(|e| panic!("{name}, flat {flat}: {e}"));
            let floor = lod().floor_level();
            for n in plan.nodes.iter().filter(|n| n.level > floor) {
                assert!(
                    morph_room(f64::from(n.morph_start_m), f64::from(n.morph_end_m), 0.0),
                    "{name}, flat {flat}: tile {:?} at level {} was kept with an interval of {} to {}",
                    n.tile,
                    n.level,
                    n.morph_start_m,
                    n.morph_end_m
                );
            }
            assert!(
                plan.nodes.iter().any(|n| n.capped),
                "{name}, flat {flat}: the tiles whose neighbours' bounds are not numbers are capped"
            );
        }
    }
}
