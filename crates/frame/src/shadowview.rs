//! The shadow pass's view of a frame (REND-050, ADR 0085 points 1 and 5), GPU-free: the camera and the sun in the clipmap anchor's tangent
//! frame (x east, y north, z up, Euclidean offsets from the anchor's surface point), rounded once to f32 (CON-11). The clipmap is indexed
//! differently: by the azimuthal equidistant plane around the anchor (arc length, not chord), holding height minus the sphere's drop. The
//! receivers pass converts a Euclidean point to that frame in the shader from the drop coefficient alone (`to_plane`), so no planet radius
//! reaches a shader; the camera and the reprojection stay Euclidean, where a rigid motion is exact.

use planet_core::Vec3;

use crate::camera::Camera;

/// The camera and sun of one frame in the anchor's tangent frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowView {
    /// The camera: position (metres from the anchor point), forward, right and up (unit), the tangents of the half fields of view
    /// (horizontal, vertical) and the near plane.
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub tan_half_fov: [f32; 2],
    pub near_m: f32,
    /// The unit direction to the sun in the same frame.
    pub sun: [f32; 3],
    /// The sphere's drop per squared plane distance, `1 / (2 R)`, rounded once to f32 (never R itself in a shader).
    pub drop_coefficient: f32,
    /// The anchor's unit direction and its east and north in the planet-fixed axes (the coarse cube's lookup).
    pub anchor: [f32; 3],
    pub east: [f32; 3],
    pub north: [f32; 3],
}

fn f3(v: Vec3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

impl ShadowView {
    /// `anchor` is the unit direction of the clipmap plan's anchor, `camera` the frame's camera (planet-fixed, f64), `sun_dir` the unit
    /// direction to the sun in the planet-fixed axes, `radius_m` the body's radius.
    pub fn new(anchor: Vec3, camera: &Camera, sun_dir: Vec3, radius_m: f64) -> ShadowView {
        let (east, north) = anchor.tangent_basis();
        // The camera's own basis (the one the depth buffer's projection uses), with the tangents of the field of view taken off it.
        let (rt, ut, f) = camera.ray_frame();
        let (tx, ty) = (rt.length(), ut.length());
        let (r, u) = (rt * (1.0 / tx), ut * (1.0 / ty));
        let local = |v: Vec3| f3(Vec3::new(v.dot(east), v.dot(north), v.dot(anchor)));
        let offset = camera.position.0 - anchor * radius_m;
        ShadowView {
            // The offset is a difference of two planet-radius vectors: formed in f64, rounded once.
            position: local(offset),
            forward: local(f),
            right: local(r),
            up: local(u),
            tan_half_fov: [tx as f32, ty as f32],
            near_m: camera.near_m as f32,
            sun: local(sun_dir.normalized()),
            drop_coefficient: (0.5 / radius_m) as f32,
            anchor: f3(anchor),
            east: f3(east),
            north: f3(north),
        }
    }
}
