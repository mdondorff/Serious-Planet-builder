//! The free camera of the viewer window (VIEW-004): one pure function from a pose, the frame's input and `dt` to the next pose
//! (CON-03, CON-04, CON-14). Float64 throughout, `libm` for the transcendentals.
//!
//! The pose is a position, a heading (a unit vector tangent to the sphere at the position) and a pitch. Nothing about the
//! orientation is accumulated in a rotation: each frame the camera is rebuilt from the local vertical (the position's direction),
//! the heading and the pitch, so it cannot roll. Moving along the surface turns the position and the heading together about
//! the same axis, which is parallel transport along a great circle: the heading keeps its angle to the transported north, so
//! there is no singular point at the poles (no latitude or longitude is ever formed). Pitch is clamped short of +-90 degrees.
//!
//! The input carries the keys (axes and look rates, as [`CameraInput`] does) and a look delta in radians (yaw to the right, pitch up). A mouse delta, when
//! the viewer gets one, is `pixels * sensitivity` fed into `look_delta_rad`.

use crate::camera::Camera;
use crate::controller::{CameraInput, ControllerParams};
use planet_core::{PlanetFixed, Vec3};

/// The largest pitch above or below the horizon: 89 degrees.
pub const MAX_PITCH_RAD: f64 = 89.0 * std::f64::consts::PI / 180.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreePose {
    pub position: PlanetFixed,
    /// Unit tangent: perpendicular to the position's direction.
    pub heading: Vec3,
    /// Angle above (+) or below (-) the horizon, radians, within +-[`MAX_PITCH_RAD`].
    pub pitch: f64,
}

/// What one frame of input is: `keys` (move axes in [-1, 1], look rates in rad/s, a speed scale) and the look change in radians
/// that does not come from a rate (a mouse delta later): yaw to the right (+) and pitch up (+).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FreeInput {
    pub keys: CameraInput,
    pub look_delta_rad: [f64; 2],
}

/// `v` projected onto the tangent plane of `up`, as a unit vector; any tangent when `v` is (nearly) parallel to `up`.
fn tangent(v: Vec3, up: Vec3) -> Vec3 {
    let t = v - up * v.dot(up);
    if t.length() > 1e-9 {
        return t.normalized();
    }
    let axis = if up.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    (axis - up * axis.dot(up)).normalized()
}

/// Rotate `v` about the unit `axis` by `angle` (Rodrigues).
fn rotate(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
}

impl FreePose {
    pub fn new(position: PlanetFixed, heading_hint: Vec3, pitch: f64) -> Self {
        let up = position.0.normalized();
        Self { position, heading: tangent(heading_hint, up), pitch: pitch.clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD) }
    }

    /// The pose of a camera: its heading is the view direction projected onto the tangent plane, its pitch the angle above it.
    pub fn from_camera(c: &Camera) -> Self {
        let up = c.position.0.normalized();
        let f = c.forward.normalized();
        let pitch = libm::asin(f.dot(up).clamp(-1.0, 1.0));
        // A view straight along the vertical has no heading of its own: the camera's own up carries it.
        let hint = if (f - up * f.dot(up)).length() > 1e-6 { f } else { c.up };
        Self::new(c.position, hint, pitch)
    }

    pub fn up(&self) -> Vec3 {
        self.position.0.normalized()
    }

    pub fn forward(&self) -> Vec3 {
        let up = self.up();
        (self.heading * libm::cos(self.pitch) + up * libm::sin(self.pitch)).normalized()
    }

    /// The camera, rebuilt from the vertical, the heading and the pitch: its up is the local vertical, so it has no roll.
    pub fn camera(&self, fov_y_rad: f64, aspect: f64, near_m: f64) -> Camera {
        Camera { position: self.position, forward: self.forward(), up: self.up(), fov_y_rad, aspect, near_m }
    }
}

/// The speed in metres per second at `height_m` above the terrain: proportional to the height within the parameters' limits,
/// scaled by the input's boost (0 means 1).
pub fn speed_mps(height_m: f64, scale: f64, p: &ControllerParams) -> f64 {
    let (lo, hi) = p.speed_limits();
    (height_m.max(0.0) * p.heights_per_second).clamp(lo, hi) * if scale > 0.0 { scale } else { 1.0 }
}

/// The next pose after `dt` seconds of `input` at `height_m` above the terrain (the caller measures it: the terrain is not the
/// function's business). Non-finite `dt` or input counts as 0.
///
/// The look is applied first (yaw about the local vertical, then pitch, clamped), the move after it along the new view: the
/// horizontal part as an arc of a great circle turning position and heading together, the vertical part as a change of the
/// distance from the centre. Over a frame the arc advances by the integral of `speed / r` while `r` changes at a constant
/// rate, so two half frames of the same input land exactly where one whole frame does, and the opposite move undoes a move.
pub fn free_step(pose: FreePose, input: &FreeInput, dt: f64, height_m: f64, p: &ControllerParams) -> FreePose {
    let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
    let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
    let k = input.keys;
    let (fwd, right, vert) = (fin(k.forward).clamp(-1.0, 1.0), fin(k.right).clamp(-1.0, 1.0), fin(k.up).clamp(-1.0, 1.0));
    // Yaw to the right and pitch up are positive, for a rate and for a delta alike.
    let yaw_right = fin(k.yaw_rate) * dt + fin(input.look_delta_rad[0]);
    let dpitch = fin(k.pitch_rate) * dt + fin(input.look_delta_rad[1]);

    // Look. An angle about the outward vertical turns to the left, so a turn to the right is its negative.
    let up = pose.up();
    let heading = if yaw_right == 0.0 { pose.heading } else { tangent(rotate(pose.heading, up, -yaw_right), up) };
    let pitch = (pose.pitch + dpitch).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);

    // Move: along the view (W, S), to the side (A, D) and along the vertical (Q, E).
    let pose = FreePose { position: pose.position, heading, pitch };
    let speed = speed_mps(height_m, k.speed_scale, p);
    let right_dir = heading.cross(up);
    let (cp, sp) = (libm::cos(pitch), libm::sin(pitch));
    let horizontal = heading * (fwd * cp) + right_dir * right;
    let h_len = horizontal.length();
    let v_rate = fwd * sp + vert;
    let norm = (h_len * h_len + v_rate * v_rate).sqrt();
    // Diagonal keys are not faster than straight ones.
    let scale = if norm > 1.0 { 1.0 / norm } else { 1.0 };
    let (h_m, v_m) = (h_len * scale * speed * dt, v_rate * scale * speed * dt);
    if h_m == 0.0 && v_m == 0.0 {
        return pose;
    }
    let r0 = pose.position.0.length();
    let r1 = (r0 + v_m).max(1.0);
    // Arc angle: integral of h_m / (r0 + v t) over the frame = (h_m / v_m) ln(r1 / r0); h_m / r0 as v_m goes to 0.
    let x = (r1 - r0) / r0;
    let ratio = if x.abs() < 1e-9 { 1.0 - x / 2.0 } else { libm::log1p(x) / x };
    let theta = h_m / r0 * ratio;
    if h_m == 0.0 {
        return FreePose { position: PlanetFixed(up * r1), ..pose };
    }
    let dir = horizontal * (1.0 / h_len);
    let axis = up.cross(dir).normalized();
    let new_up = rotate(up, axis, theta).normalized();
    let new_heading = tangent(rotate(heading, axis, theta), new_up);
    FreePose { position: PlanetFixed(new_up * r1), heading: new_heading, pitch }
}

/// Lift the pose to `min_clearance_m` above a terrain whose surface here is `ground_radius_m` from the centre (sphere radius plus
/// height), when it is lower: the caller reads the terrain at the pose's direction.
pub fn keep_clearance(pose: FreePose, ground_radius_m: f64, min_clearance_m: f64) -> FreePose {
    let floor = ground_radius_m + min_clearance_m;
    let r = pose.position.0.length();
    if r >= floor {
        return pose;
    }
    FreePose { position: PlanetFixed(pose.up() * floor), ..pose }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0;

    fn params() -> ControllerParams {
        ControllerParams::default()
    }

    fn pose_at(dir: Vec3, height: f64, heading: Vec3, pitch: f64) -> FreePose {
        FreePose::new(PlanetFixed(dir.normalized() * (R + height)), heading, pitch)
    }

    fn keys(forward: f64, right: f64, up: f64, yaw_rate: f64, pitch_rate: f64) -> FreeInput {
        FreeInput { keys: CameraInput { forward, right, up, yaw_rate, pitch_rate, speed_scale: 1.0 }, look_delta_rad: [0.0, 0.0] }
    }

    fn height(p: &FreePose) -> f64 {
        p.position.0.length() - R
    }

    fn roll(p: &FreePose) -> f64 {
        // The camera's right-hand axis has no vertical component when there is no roll.
        let cam = p.camera(1.0, 1.0, 0.1);
        cam.forward.cross(cam.up).normalized().dot(p.up()).abs()
    }

    fn sane(p: &FreePose) {
        assert!(p.position.0.length().is_finite());
        assert!((p.heading.length() - 1.0).abs() < 1e-12, "heading length {}", p.heading.length());
        assert!(p.heading.dot(p.up()).abs() < 1e-12, "heading leaves the tangent plane: {}", p.heading.dot(p.up()));
        assert!(p.pitch.abs() <= MAX_PITCH_RAD + 1e-15);
    }

    /// A fixed pseudo-random input sequence (SplitMix64-style, self-contained).
    fn sequence(n: usize) -> Vec<(FreeInput, f64)> {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) as f64 / u64::MAX as f64
        };
        (0..n)
            .map(|_| {
                let axis = |u: f64| (u * 3.0).floor() - 1.0;
                let i = FreeInput {
                    keys: CameraInput {
                        forward: axis(next()),
                        right: axis(next()),
                        up: axis(next()),
                        yaw_rate: (next() - 0.5) * 4.0,
                        pitch_rate: (next() - 0.5) * 4.0,
                        speed_scale: 1.0,
                    },
                    look_delta_rad: [(next() - 0.5) * 0.2, (next() - 0.5) * 0.2],
                };
                (i, 0.005 + next() * 0.03)
            })
            .collect()
    }

    // spec: VIEW-004
    #[test]
    fn no_input_sequence_produces_roll_or_leaves_the_tangent_plane() {
        // At 1 m, at 3 km and in orbit, from the equator, a pole and a diagonal, 2000 random frames each.
        for (dir, h) in [
            (Vec3::new(1.0, 0.0, 0.0), 1.0),
            (Vec3::new(0.0, 0.0, 1.0), 1.0),
            (Vec3::new(0.3, -0.4, 0.86), 3000.0),
            (Vec3::new(-0.5, 0.5, -0.7), 2.0e7),
        ] {
            let mut p = pose_at(dir, h, Vec3::new(0.0, 1.0, 0.0), 0.0);
            for (input, dt) in sequence(2000) {
                p = free_step(p, &input, dt, height(&p), &params());
                p = keep_clearance(p, R, 1.0);
                sane(&p);
                assert!(roll(&p) < 1e-12, "roll {} at {dir:?}", roll(&p));
            }
        }
    }

    // spec: VIEW-004
    #[test]
    fn the_pitch_is_clamped_short_of_the_poles() {
        let p = pose_at(Vec3::new(1.0, 0.2, 0.1), 1000.0, Vec3::new(0.0, 0.0, 1.0), 0.0);
        let up = free_step(p, &keys(0.0, 0.0, 0.0, 0.0, 1.0), 100.0, 1000.0, &params());
        assert_eq!(up.pitch, MAX_PITCH_RAD);
        const { assert!(MAX_PITCH_RAD < std::f64::consts::FRAC_PI_2) };
        let down = free_step(p, &keys(0.0, 0.0, 0.0, 0.0, -1.0), 100.0, 1000.0, &params());
        assert_eq!(down.pitch, -MAX_PITCH_RAD);
        // A mouse-sized delta clamps the same way, and the camera still has a defined, roll-free basis at the clamp.
        let m = FreeInput { look_delta_rad: [0.0, 10.0], ..Default::default() };
        let c = free_step(p, &m, 0.016, 1000.0, &params());
        assert_eq!(c.pitch, MAX_PITCH_RAD);
        assert!(roll(&c) < 1e-12);
        // Constructing a pose clamps too.
        assert_eq!(FreePose::new(p.position, p.heading, 3.0).pitch, MAX_PITCH_RAD);
    }

    // spec: VIEW-004
    #[test]
    fn a_move_and_its_opposite_return_to_the_start() {
        let start = pose_at(Vec3::new(0.2, 0.7, 0.4), 5000.0, Vec3::new(0.0, 0.0, 1.0), 0.3);
        for (f, r, u) in [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0), (1.0, 1.0, 1.0)] {
            let mut p = start;
            for _ in 0..50 {
                p = free_step(p, &keys(f, r, u, 0.0, 0.0), 0.02, 5000.0, &params());
            }
            assert!((p.position.0 - start.position.0).length() > 1.0, "the move moved");
            for _ in 0..50 {
                p = free_step(p, &keys(-f, -r, -u, 0.0, 0.0), 0.02, 5000.0, &params());
            }
            let d = (p.position.0 - start.position.0).length();
            // Reversing a move of a km is exact to rounding for a straight run; the diagonal's heading drifts by nothing.
            assert!(d < 1e-6, "keys ({f},{r},{u}): back {d} m from the start");
            assert!(p.heading.cross(start.heading).length() < 1e-9);
        }
        // A look change undoes itself too.
        let turned = free_step(start, &keys(0.0, 0.0, 0.0, 1.0, 0.5), 0.5, 5000.0, &params());
        let back = free_step(turned, &keys(0.0, 0.0, 0.0, -1.0, -0.5), 0.5, 5000.0, &params());
        assert!(back.heading.cross(start.heading).length() < 1e-12 && (back.pitch - start.pitch).abs() < 1e-12);
    }

    // spec: VIEW-004
    #[test]
    fn the_result_does_not_depend_on_how_the_input_is_split_across_frames() {
        let start = pose_at(Vec3::new(-0.3, 0.5, 0.8), 800.0, Vec3::new(1.0, 0.0, 0.0), 0.4);
        // Motion: one second as 1, 3 and 60 frames (the camera is held at the same height input, as the function is pure in it).
        for (f, r, u) in [(1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (0.5, 0.5, 1.0)] {
            let run = |frames: u32| {
                let mut p = start;
                for _ in 0..frames {
                    p = free_step(p, &keys(f, r, u, 0.0, 0.0), 1.0 / f64::from(frames), 800.0, &params());
                }
                p
            };
            let whole = run(1);
            for n in [3, 60] {
                let split = run(n);
                let d = (split.position.0 - whole.position.0).length();
                assert!(d < 1e-6, "keys ({f},{r},{u}) in {n} frames: {d} m apart");
                assert!(split.heading.cross(whole.heading).length() < 1e-9);
            }
        }
        // Look: the same total angle however it is cut, by rate or by delta.
        let whole = free_step(start, &keys(0.0, 0.0, 0.0, 0.8, 0.2), 1.0, 800.0, &params());
        let mut p = start;
        for _ in 0..40 {
            p = free_step(p, &keys(0.0, 0.0, 0.0, 0.8, 0.2), 1.0 / 40.0, 800.0, &params());
        }
        assert!(p.heading.cross(whole.heading).length() < 1e-12 && (p.pitch - whole.pitch).abs() < 1e-12);
        let by_delta = free_step(start, &FreeInput { look_delta_rad: [0.8, 0.2], ..Default::default() }, 1.0, 800.0, &params());
        assert!(by_delta.heading.cross(whole.heading).length() < 1e-12, "a delta of 0.8 is the rate 0.8 for one second");
        assert!((by_delta.pitch - whole.pitch).abs() < 1e-12);
    }

    // spec: VIEW-004
    #[test]
    fn the_poles_are_not_singular() {
        // Fly over the north pole at 1 m: straight on, heading kept (the transported heading points on across the pole).
        let mut p = pose_at(Vec3::new(0.0, 1.0e-6, 1.0), 1.0, Vec3::new(0.0, -1.0, 0.0), 0.0);
        let crossing = Vec3::new(0.0, 0.0, 1.0);
        let mut min_angle = f64::MAX;
        let mut last_heading = p.heading;
        for _ in 0..4000 {
            p = free_step(p, &keys(1.0, 0.0, 0.0, 0.0, 0.0), 0.01, 1.0, &params());
            sane(&p);
            assert!(roll(&p) < 1e-12);
            // The heading turns smoothly: no jump between frames, also at the pole.
            assert!(p.heading.cross(last_heading).length() < 1e-4, "heading jumped by {}", p.heading.cross(last_heading).length());
            last_heading = p.heading;
            min_angle = min_angle.min(libm::acos(p.up().dot(crossing).clamp(-1.0, 1.0)));
        }
        // 1 m/s * 40 s = 40 m, from 6.4 m before the pole: the run goes over it.
        assert!(min_angle * R < 25.0, "closest approach {} m", min_angle * R);
        // Looking and moving exactly at the pole works as anywhere else.
        let at_pole = pose_at(crossing, 1.0, Vec3::new(1.0, 0.0, 0.0), 0.2);
        let moved = free_step(at_pole, &keys(1.0, 1.0, 0.0, 0.5, 0.1), 0.1, 1.0, &params());
        sane(&moved);
        assert!(roll(&moved) < 1e-12);
        assert!((moved.position.0 - at_pole.position.0).length() > 0.01);
    }

    // spec: VIEW-004
    #[test]
    fn speed_follows_the_height_from_one_metre_to_orbit() {
        let p = params();
        assert_eq!(speed_mps(0.1, 0.0, &p), p.min_speed_mps);
        assert_eq!(speed_mps(1.0, 0.0, &p), 1.0);
        assert_eq!(speed_mps(10_000.0, 0.0, &p), 10_000.0);
        assert_eq!(speed_mps(1.0e9, 0.0, &p), p.max_speed_mps);
        assert_eq!(speed_mps(10_000.0, 2.0, &p), 20_000.0);
        // One second forward at 10 km and at 2000 km covers that many metres (flat view, no pitch).
        for h in [10_000.0, 2.0e6] {
            let start = pose_at(Vec3::new(0.0, 1.0, 0.0), h, Vec3::new(0.0, 0.0, 1.0), 0.0);
            let moved = free_step(start, &keys(1.0, 0.0, 0.0, 0.0, 0.0), 1.0, h, &p);
            let arc = libm::acos(start.up().dot(moved.up()).clamp(-1.0, 1.0)) * (R + h);
            assert!((arc - h).abs() < h * 1e-9, "height {h}: arc {arc}");
        }
    }

    // spec: VIEW-004
    #[test]
    fn non_finite_input_counts_as_nothing_and_the_clearance_lifts() {
        let p = pose_at(Vec3::new(1.0, 1.0, 1.0), 50.0, Vec3::new(0.0, 0.0, 1.0), 0.1);
        let nan = FreeInput {
            keys: CameraInput { forward: f64::NAN, yaw_rate: f64::INFINITY, ..Default::default() },
            look_delta_rad: [f64::NAN, 0.0],
        };
        assert_eq!(free_step(p, &nan, f64::NAN, 50.0, &params()), p);
        assert_eq!(free_step(p, &keys(1.0, 0.0, 0.0, 0.0, 0.0), -1.0, 50.0, &params()), p);
        let low = pose_at(Vec3::new(1.0, 1.0, 1.0), 0.2, Vec3::new(0.0, 0.0, 1.0), 0.0);
        let lifted = keep_clearance(low, R + 300.0, 1.0);
        assert!((lifted.position.0.length() - (R + 301.0)).abs() < 1e-6);
        assert_eq!(keep_clearance(p, R, 1.0), p);
    }

    /// The speed before the limits scaled with the radius, copied here so that Earth's speeds are pinned to it bit for bit.
    fn speed_before_radius_scaling(height_m: f64, scale: f64, p: &ControllerParams) -> f64 {
        (height_m.max(0.0) * p.heights_per_second).clamp(p.min_speed_mps, p.max_speed_mps) * if scale > 0.0 { scale } else { 1.0 }
    }

    // spec: VIEW-004
    #[test]
    fn an_earth_sized_body_moves_exactly_as_before_the_speed_scaled_with_the_radius() {
        let earth = ControllerParams::for_radius(crate::controller::EARTH_RADIUS_M);
        assert_eq!(earth, ControllerParams::default());
        for h in [-5.0, 0.0, 0.3, 1.0, 1.7, 40.0, 3_000.0, 12_345.678, 5.0e5, 2.0e6, 3.0e6, 9.9e6, 1.0e9, f64::INFINITY] {
            for scale in [0.0, 1.0, 2.5, 8.0] {
                assert_eq!(
                    speed_mps(h, scale, &earth).to_bits(),
                    speed_before_radius_scaling(h, scale, &earth).to_bits(),
                    "height {h} boost {scale}"
                );
            }
        }
        // The same through a whole step: the pose after a second of forward flight is bit-identical to a pose moved at the old speed.
        let p = pose_at(Vec3::new(1.0, 1.0, 1.0), 3_000.0, Vec3::new(0.0, 0.0, 1.0), 0.0);
        let fast = free_step(p, &keys(1.0, 0.0, 0.0, 0.0, 0.0), 1.0, 3_000.0, &earth);
        let old = ControllerParams { min_speed_mps: 0.5, max_speed_mps: 3.0e6, ..earth };
        assert_eq!(fast, free_step(p, &keys(1.0, 0.0, 0.0, 0.0, 0.0), 1.0, 3_000.0, &old));
    }

    // spec: VIEW-004
    #[test]
    fn a_body_of_half_the_radius_has_half_the_speed_limits_and_the_same_speed_between_them() {
        let half = ControllerParams::for_radius(crate::controller::EARTH_RADIUS_M / 2.0);
        let earth = ControllerParams::default();
        assert_eq!(half.speed_limits(), (0.25, 1.5e6));
        // Between the limits the speed follows the height alone (a body of half the radius has half the height in orbit).
        for h in [1.0, 40.0, 3_000.0, 5.0e5] {
            assert_eq!(speed_mps(h, 1.0, &half), speed_mps(h, 1.0, &earth), "height {h}");
        }
        // At the limits it is half: standing still near the ground, and the top speed far out.
        assert_eq!(speed_mps(0.0, 1.0, &half), speed_mps(0.0, 1.0, &earth) / 2.0);
        assert_eq!(speed_mps(1.0e9, 1.0, &half), speed_mps(1.0e9, 1.0, &earth) / 2.0);
        assert_eq!(speed_mps(1.0e9, 2.0, &half), 3.0e6, "the boost multiplies the scaled limit");
        // A body of twice the radius: twice the limits. An unusable radius counts as the Earth's.
        assert_eq!(ControllerParams::for_radius(2.0 * crate::controller::EARTH_RADIUS_M).speed_limits(), (1.0, 6.0e6));
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(ControllerParams::for_radius(bad).speed_limits(), earth.speed_limits(), "radius {bad}");
        }
        // The old controller scales with the same limits.
        let c = crate::controller::CameraController::new(PlanetFixed(Vec3::new(0.0, 0.0, R + 10.0)), Vec3::new(1.0, 0.0, 0.0), half);
        assert_eq!(c.speed_for_height(0.0), 0.25);
        assert_eq!(c.speed_for_height(1.0e12), 1.5e6);
    }

    // spec: VIEW-004
    #[test]
    fn the_eye_never_ends_below_the_terrain_plus_the_margin_whichever_way_it_moves() {
        // A 300 m hill ahead: flying down and forward into it, 20 seconds in ten steps, the eye stays at or above hill + margin
        // (the clamp reads the terrain at the pose AFTER the move, as the viewer does).
        let hill = |p: PlanetFixed| if p.0.normalized().dot(Vec3::new(0.0, 0.0, 1.0)) > 0.0 { R + 300.0 } else { R };
        let params = ControllerParams::default();
        let mut p = pose_at(Vec3::new(1.0, 0.0, 0.5), 20.0, Vec3::new(0.0, 0.0, 1.0), -0.5);
        for _ in 0..40 {
            let h = p.position.0.length() - hill(p.position);
            let moved = free_step(p, &keys(1.0, 0.0, -1.0, 0.0, 0.0), 0.5, h, &params);
            p = keep_clearance(moved, hill(moved.position), params.min_clearance_m);
            assert!(
                p.position.0.length() >= hill(p.position) + params.min_clearance_m - 1e-9,
                "{}",
                p.position.0.length() - hill(p.position)
            );
        }
        // It ended on the ground (the clamp engaged), not flying: within a metre of terrain + margin.
        assert!(p.position.0.length() - hill(p.position) < params.min_clearance_m + 1.0);
    }

    // spec: VIEW-004
    #[test]
    fn a_camera_gives_its_pose_back() {
        let cam = crate::camera::Camera::at_surface_point(
            PlanetFixed(Vec3::new(0.3, 0.2, 0.9).normalized() * (R + 100.0)),
            Vec3::new(0.0, 0.0, 1.0),
            0.4,
            1.0,
            1.5,
            0.1,
        );
        let pose = FreePose::from_camera(&cam);
        // `at_surface_point` pitches down by 0.4; the pose's pitch is above the horizon.
        assert!((pose.pitch + 0.4).abs() < 1e-12, "pitch {}", pose.pitch);
        let again = pose.camera(1.0, 1.5, 0.1);
        assert!((again.forward - cam.forward).length() < 1e-12);
        // A camera looking straight down still has a heading (from its up).
        let down =
            crate::camera::Camera::looking_at_centre(PlanetFixed(Vec3::new(0.0, 0.0, R + 1.0e6)), Vec3::new(0.0, 1.0, 0.0), 1.0, 1.0, 0.1);
        let pose = FreePose::from_camera(&down);
        sane(&pose);
        assert!((pose.pitch + MAX_PITCH_RAD).abs() < 1e-12 || pose.pitch < -1.5, "pitch {}", pose.pitch);
    }
}
