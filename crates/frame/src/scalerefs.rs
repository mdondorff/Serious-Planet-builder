//! Scale references (VIEW-050, REND-036): plain boxes of known size dropped on the ground in front of the camera, so the eye can judge the
//! size of what the ground shows (the ground detail, the relief, the distances). Decided here, GPU-free (CON-03): the renderer draws the
//! boxes it is given.
//!
//! Each box is one named size: a chair 0.9 m high, a house 8 m wide, a boat 10 m long, a tower 291 m high, and a block of 100 boxes on 1 km. They
//! stand in a row ahead of the camera at distances that suit their sizes, each on the lowest ground under its footprint.

use planet_core::{PlanetFixed, Vec3};

/// The most boxes one drop makes.
pub const MAX_BOXES: usize = 128;

/// One box of a drop: its centre in the planet-fixed axes (f64), its three unit axes (across, along the view, up) and its half sizes.
#[derive(Clone, Debug, PartialEq)]
pub struct RefBox {
    pub name: &'static str,
    pub centre: PlanetFixed,
    /// Unit axes: across (to the camera's right), along (the camera's heading), up (away from the planet's centre).
    pub axes: [Vec3; 3],
    /// Half sizes along `axes`, metres.
    pub half_m: [f32; 3],
    /// Linear albedo.
    pub colour: [f32; 3],
}

/// A named size: width across the view, length along it and height, in metres; where it stands (metres ahead of the camera, metres to its
/// right); its albedo.
struct Spec {
    name: &'static str,
    size_m: [f64; 3],
    ahead_m: f64,
    right_m: f64,
    colour: [f32; 3],
}

const OBJECTS: [Spec; 4] = [
    Spec { name: "chair", size_m: [0.45, 0.45, 0.9], ahead_m: 6.0, right_m: 1.5, colour: [0.95, 0.55, 0.05] },
    Spec { name: "house", size_m: [8.0, 6.0, 5.0], ahead_m: 30.0, right_m: -9.0, colour: [0.60, 0.12, 0.10] },
    Spec { name: "boat", size_m: [3.0, 10.0, 2.5], ahead_m: 55.0, right_m: 14.0, colour: [0.15, 0.30, 0.65] },
    Spec { name: "tower", size_m: [20.0, 20.0, 291.0], ahead_m: 700.0, right_m: -90.0, colour: [0.70, 0.70, 0.72] },
];

/// The block: `BLOCK_SIDE` x `BLOCK_SIDE` boxes `BLOCK_PITCH_M` apart (a block 1 km on a side less one pitch), starting `BLOCK_AHEAD_M` ahead.
pub const BLOCK_SIDE: usize = 10;
pub const BLOCK_PITCH_M: f64 = 100.0;
pub const BLOCK_AHEAD_M: f64 = 1_500.0;
/// A box of the block: 50 m across and along, 30 to 90 m high.
pub const BLOCK_BASE_M: f64 = 50.0;

/// The boxes of a drop at `camera` looking along `forward` (any direction; only its heading is used), on a planet of radius `radius_m` whose
/// ground height above the reference surface at a unit direction is `ground`.
pub fn drop_references(camera: PlanetFixed, forward: Vec3, radius_m: f64, ground: &dyn Fn(Vec3) -> f64) -> Vec<RefBox> {
    let up = camera.0.normalized();
    let mut heading = forward - up * forward.dot(up);
    if heading.length() < 1e-9 {
        // Looking straight up or down: any heading will do, but the same one for the same place.
        heading = up.cross(Vec3::new(0.0, 0.0, 1.0));
        if heading.length() < 1e-9 {
            heading = up.cross(Vec3::new(1.0, 0.0, 0.0));
        }
    }
    let heading = heading.normalized();
    let right = heading.cross(up).normalized();
    let stand = |name: &'static str, size_m: [f64; 3], ahead_m: f64, right_m: f64, colour: [f32; 3]| -> RefBox {
        // The place on the sphere, then the local axes there (the ground normal is the planet's radial).
        let at = |a: f64, r: f64| (up * radius_m + heading * a + right * r).normalized();
        let dir = at(ahead_m, right_m);
        let local_up = dir;
        let local_along = (heading - local_up * heading.dot(local_up)).normalized();
        let local_across = local_along.cross(local_up).normalized();
        // The lowest ground under the footprint: the box stands on it (the high side sinks into the ground).
        let (hw, hl) = (0.5 * size_m[0], 0.5 * size_m[1]);
        let low = [(-hw, -hl), (hw, -hl), (-hw, hl), (hw, hl), (0.0, 0.0)]
            .iter()
            .map(|(dx, dy)| ground((up * radius_m + heading * (ahead_m + dy) + right * (right_m + dx)).normalized()))
            .fold(f64::INFINITY, f64::min);
        let centre = dir * (radius_m + low + 0.5 * size_m[2]);
        RefBox {
            name,
            centre: PlanetFixed(centre),
            axes: [local_across, local_along, local_up],
            half_m: [(0.5 * size_m[0]) as f32, (0.5 * size_m[1]) as f32, (0.5 * size_m[2]) as f32],
            colour,
        }
    };
    let mut out: Vec<RefBox> = OBJECTS.iter().map(|o| stand(o.name, o.size_m, o.ahead_m, o.right_m, o.colour)).collect();
    for i in 0..BLOCK_SIDE {
        for j in 0..BLOCK_SIDE {
            let height_m = 30.0 + 6.0 * ((i * 7 + j * 13) % 11) as f64;
            let ahead = BLOCK_AHEAD_M + i as f64 * BLOCK_PITCH_M;
            let across = (j as f64 - 0.5 * (BLOCK_SIDE - 1) as f64) * BLOCK_PITCH_M;
            let shade = 0.35 + 0.04 * ((i + j) % 4) as f32;
            out.push(stand("block", [BLOCK_BASE_M, BLOCK_BASE_M, height_m], ahead, across, [shade, shade, shade + 0.05]));
        }
    }
    debug_assert!(out.len() <= MAX_BOXES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;

    fn camera_at(lat_deg: f64, lon_deg: f64, alt_m: f64) -> PlanetFixed {
        let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
        PlanetFixed(Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la)) * (R + alt_m))
    }

    fn east(cam: PlanetFixed) -> Vec3 {
        Vec3::new(0.0, 0.0, 1.0).cross(cam.0.normalized()).normalized()
    }

    // spec: VIEW-050
    #[test]
    fn the_named_sizes_are_what_they_say_and_the_block_is_a_kilometre_of_a_hundred_boxes() {
        let cam = camera_at(30.0, 40.0, 1.8);
        let boxes = drop_references(cam, east(cam), R, &|_| 0.0);
        let full = |name: &str| boxes.iter().find(|b| b.name == name).map(|b| b.half_m.map(|h| 2.0 * f64::from(h))).expect(name);
        let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-5, "{a} against {b}");
        near(full("chair")[2], 0.9);
        near(full("house")[0], 8.0);
        near(full("boat")[1], 10.0);
        near(full("tower")[2], 291.0);
        let block: Vec<&RefBox> = boxes.iter().filter(|b| b.name == "block").collect();
        assert_eq!(block.len(), BLOCK_SIDE * BLOCK_SIDE);
        assert_eq!(boxes.len(), 4 + BLOCK_SIDE * BLOCK_SIDE);
        assert!(boxes.len() <= MAX_BOXES);
        // The block spans 1 km less one pitch between the centres of its first and last rows (900 m) plus a box: 950 m.
        let along: Vec<f64> = block.iter().map(|b| (b.centre.0 - cam.0).dot(east(cam))).collect();
        let span = along.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - along.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!((span + BLOCK_BASE_M - 950.0).abs() < 2.0, "the block is 1 km across: {span}");
    }

    // spec: VIEW-050
    #[test]
    fn the_boxes_stand_on_the_lowest_ground_ahead_of_the_camera_on_their_own_places_and_do_not_overlap() {
        let cam = camera_at(-20.0, 100.0, 5.0);
        let heading = east(cam);
        // A ground that rises towards the heading: 1 m per 10 m ahead.
        let origin = cam.0.normalized();
        let ground = move |d: Vec3| 0.1 * (d - origin).dot(heading) * R;
        let boxes = drop_references(cam, heading + cam.0.normalized() * 0.4, R, &ground);
        for b in &boxes {
            assert!((b.centre.0 - cam.0).dot(heading) > 0.0, "{} is ahead of the camera", b.name);
            let up = b.axes[2];
            assert!(
                (up.dot(b.centre.0.normalized()) - 1.0).abs() < 1e-12
                    && (b.axes[0].dot(up)).abs() < 1e-9
                    && (b.axes[1].dot(up)).abs() < 1e-9
            );
            // The lowest of the footprint's corners and its middle is the ground the box stands on.
            let bottom = (b.centre.0 - up * f64::from(b.half_m[2])).length() - R;
            let corner = |sx: f64, sy: f64| {
                let p = b.centre.0 + b.axes[0] * (sx * f64::from(b.half_m[0])) + b.axes[1] * (sy * f64::from(b.half_m[1]));
                ground(p.normalized())
            };
            let low = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, 1.0), corner(0.0, 0.0)]
                .into_iter()
                .fold(f64::INFINITY, f64::min);
            assert!((bottom - low).abs() < 0.05, "{}: bottom {bottom} against the lowest ground {low}", b.name);
        }
        // No two boxes overlap in the ground plane (the circles round their footprints are apart).
        for (i, a) in boxes.iter().enumerate() {
            for b in boxes.iter().skip(i + 1) {
                let ra = libm::hypot(f64::from(a.half_m[0]), f64::from(a.half_m[1]));
                let rb = libm::hypot(f64::from(b.half_m[0]), f64::from(b.half_m[1]));
                assert!((a.centre.0 - b.centre.0).length() > ra + rb, "{} and {} overlap", a.name, b.name);
            }
        }
        // The same camera gives the same drop, and looking straight down still gives one.
        assert_eq!(boxes, drop_references(cam, heading + cam.0.normalized() * 0.4, R, &ground));
        assert_eq!(drop_references(cam, cam.0.normalized() * -1.0, R, &ground).len(), boxes.len());
    }
}
