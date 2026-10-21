//! ADR 0074 (change 2): the edge masks of the leaves next to a capped tile. A capped leaf has finer leaves across some of its edges: its edge is
//! `fine` and the edges of the finer leaves that face it are `stitch`; the vertex stage holds a stitched edge to the coarse grid and a fine edge to
//! its own, so the vertices of the finer leaf lie on the capped leaf's edge. Tier A, no GPU.

use planet_core::cube::{FaceMapping, TangentWarp};
use planet_core::{Side, TileId, Vec3};
use planet_frame::terrain::side_bit;
use planet_frame::{plan_terrain_field, scripted_views_field, Camera, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_generators::mesh::tile_mesh_field;
use std::collections::BTreeMap;

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (1280, 720);

/// A spike as in `lod_capping.rs`: `height_m` over `radius_m` with exact bounds.
struct Spike {
    at: Vec3,
    height_m: f64,
    radius_m: f64,
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
        self.height_m * weight(angle_between(self.at, dir) * R / self.radius_m)
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
    fn height_bounds(&self, centre: Vec3, a: f64) -> (f64, f64) {
        let d = angle_between(self.at, centre);
        let near = (d - a).max(0.0) * R / self.radius_m * (1.0 - 1e-9);
        let far = ((d + a) * R / self.radius_m * (1.0 + 1e-9)).min(1.0);
        (self.height_m * weight(far) - 1e-9, self.height_m * weight(near) + 1e-9)
    }
    fn inputs_hash(&self) -> u64 {
        7
    }
}

fn lod() -> LodParams {
    LodParams { viewport_h_px: f64::from(SIZE.1), tau_px: 6.0, min_level: 0, max_height_m: 9_000.0, ..LodParams::earth_1080p() }
}

fn ground_camera() -> Camera {
    scripted_views_field(R, f64::from(SIZE.0) / f64::from(SIZE.1), &Reference(1)).into_iter().find(|(n, _)| *n == "ground-1m").unwrap().1
}

fn plan_over_a_spike() -> (planet_frame::TerrainPlan, Spike) {
    let camera = ground_camera();
    let spike = Spike { at: camera.position.0.normalized(), height_m: 220.0, radius_m: 100.0 };
    let req = TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 1 };
    (plan_terrain_field(&req, &spike).unwrap(), spike)
}

// spec: LOD-008, REND-016
#[test]
fn a_capped_leaf_has_its_finer_neighbours_stitched_to_it_and_no_other_leaf_has_a_mask() {
    let (plan, _) = plan_over_a_spike();
    let by_tile: BTreeMap<TileId, &planet_frame::terrain::TerrainDraw> = plan.nodes.iter().map(|n| (n.tile, n)).collect();
    let (mut stitched, mut fines) = (0, 0);
    for n in &plan.nodes {
        for side in Side::ALL {
            let bit = side_bit(side);
            let across = n.tile.neighbor(&TangentWarp, side);
            if n.stitch & bit != 0 {
                stitched += 1;
                // The leaf is finer than the tile across, whose parent is a capped leaf of the plan with the facing edge `fine`.
                let capped = across.parent().and_then(|p| by_tile.get(&p)).expect("a stitched edge faces a leaf one level coarser");
                assert!(capped.capped, "{:?} is stitched to {:?}, which is not capped", n.tile, capped.tile);
                let facing = Side::ALL.into_iter().find(|s| capped.tile.neighbor(&TangentWarp, *s) == n.tile.parent().unwrap());
                assert!(facing.is_some_and(|s| capped.fine & side_bit(s) != 0), "the capped leaf's facing edge is fine");
            }
            if n.fine & bit != 0 {
                fines += 1;
                assert!(n.capped, "only a capped leaf has a fine edge");
                // The tile across is split into leaves one level finer, and the one that touches this edge is stitched.
                let kids = across.children().unwrap();
                assert!(
                    kids.iter().any(|k| by_tile.get(k).is_some_and(|f| f.stitch != 0)),
                    "{:?}: the fine edge {side:?} faces no stitched leaf",
                    n.tile
                );
            }
            if n.stitch == 0 && n.fine == 0 {
                continue;
            }
        }
    }
    assert!(stitched > 0 && fines > 0, "the spike's capped tiles have finer neighbours: {stitched} stitched, {fines} fine edges");
    // Nothing is masked on ground with no capped tile.
    let camera = ground_camera();
    let req = TerrainRequest { camera, size: SIZE, view: TerrainView::Level, lod: lod(), face_mapping: TangentWarp.id().to_string(), seed: 1 };
    let flat = plan_terrain_field(&req, &Reference(1)).unwrap();
    assert!(flat.nodes.iter().all(|n| n.stitch == 0 && n.fine == 0 && !n.capped));
}

// spec: LOD-008, REND-016
#[test]
fn the_vertices_of_a_finer_leaf_on_a_stitched_edge_morph_onto_the_capped_leafs_edge() {
    // The vertex stage morphs a vertex of a stitched edge to the parent grid (the mesh's `coarse` position): the odd vertices of the edge go to
    // the middle of their two even neighbours, which are the capped leaf's vertices (the same lattice points, COORD-007). Every such vertex must
    // then lie on the capped leaf's edge, the polyline of its edge vertices, to a millimetre.
    let (plan, spike) = plan_over_a_spike();
    let cells = plan.cells;
    let by_tile: BTreeMap<TileId, &planet_frame::terrain::TerrainDraw> = plan.nodes.iter().map(|n| (n.tile, n)).collect();
    let mut checked = 0;
    for f in plan.nodes.iter().filter(|n| n.stitch != 0) {
        let fine = tile_mesh_field(&spike, &TangentWarp, f.tile, cells, R);
        for side in Side::ALL.into_iter().filter(|s| f.stitch & side_bit(*s) != 0) {
            let across = f.tile.neighbor(&TangentWarp, side);
            let t = by_tile[&across.parent().unwrap()];
            let coarse = tile_mesh_field(&spike, &TangentWarp, t.tile, cells, R);
            // The grid vertices of an edge, in order along it (the east and west edges run along j, north and south along i).
            let edge = |mesh: &planet_generators::mesh::TileMesh, side: Side, pick: &dyn Fn(&planet_generators::mesh::MeshVertex) -> [f32; 3]| {
                let w = (cells + 1) as usize;
                (0..w)
                    .map(|k| {
                        let (i, j) = match side {
                            Side::East => (w - 1, k),
                            Side::West => (0, k),
                            Side::North => (k, w - 1),
                            Side::South => (k, 0),
                        };
                        let v = pick(&mesh.vertices[j * w + i]);
                        Vec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2])) + mesh.origin.0
                    })
                    .collect::<Vec<Vec3>>()
            };
            // The capped leaf's edge facing the finer leaf.
            let facing = Side::ALL.into_iter().find(|s| t.tile.neighbor(&TangentWarp, *s) == f.tile.parent().unwrap()).unwrap();
            let target = edge(&coarse, facing, &|v| v.pos);
            let morphed = edge(&fine, side, &|v| v.coarse);
            for p in &morphed {
                let d = target.windows(2).map(|s| distance_to_segment(*p, s[0], s[1])).fold(f64::INFINITY, f64::min);
                // Only the part of the edge that the capped leaf's edge covers: the finer leaf spans half of it.
                if d < 0.5 {
                    assert!(d < 1e-3, "{:?} {side:?}: a morphed vertex is {d} m off the capped leaf's edge", f.tile);
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 20, "the oracle looked at {checked} vertices");
}

fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}
