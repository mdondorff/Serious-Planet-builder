//! Draft of the free-camera pose function (docs/status.md, M5 item 1): GPU-free, pure. The pose is a position on or above a sphere
//! centred at the origin, a heading (a unit vector tangent to the sphere at the position) and a pitch. Yaw and pitch are relative
//! to the local vertical and the basis is rebuilt from them every frame, so roll cannot accumulate; the heading is carried along
//! the motion by parallel transport (the minimal rotation from the old local up to the new one), so the poles are not singular;
//! pitch is clamped short of +-90 degrees.

pub type V3 = [f64; 3];

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}
pub fn norm(a: V3) -> V3 {
    scale(a, 1.0 / len(a))
}

/// Rodrigues rotation of `v` about the unit `axis` by `angle`.
pub fn rotate(v: V3, axis: V3, angle: f64) -> V3 {
    let (s, c) = angle.sin_cos();
    add(add(scale(v, c), scale(cross(axis, v), s)), scale(axis, dot(axis, v) * (1.0 - c)))
}

/// The minimal rotation taking unit `a` to unit `b`, applied to `v`.
pub fn rotate_between(v: V3, a: V3, b: V3) -> V3 {
    let axis = cross(a, b);
    let s = len(axis);
    let c = dot(a, b);
    if s < 1e-15 {
        return v; // parallel (antiparallel is a half-turn of the whole sphere: not reachable in one step)
    }
    rotate(v, scale(axis, 1.0 / s), s.atan2(c))
}

pub const PITCH_LIMIT: f64 = 89.0 * std::f64::consts::PI / 180.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Position relative to the sphere's centre.
    pub pos: V3,
    /// Unit, tangent to the sphere at `pos` (perpendicular to the local up).
    pub heading: V3,
    pub pitch: f64,
}

/// One frame of input, already scaled: angles in radians (mouse and arrow keys, never multiplied by dt by the caller for the
/// mouse), distances in metres (key speed times dt).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub dyaw: f64,
    pub dpitch: f64,
    pub forward: f64,
    pub right: f64,
    pub up: f64,
}

pub struct Basis {
    pub right: V3,
    pub up: V3,
    pub forward: V3,
    /// The local vertical at the position.
    pub vertical: V3,
}

impl Pose {
    /// At latitude/longitude (radians) and altitude above a sphere of `radius`, looking along `heading_east_rad` from north.
    pub fn at(lat: f64, lon: f64, radius: f64, altitude: f64, heading_from_north: f64) -> Pose {
        let up = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
        let east = norm(cross([0.0, 0.0, 1.0], up));
        let north = cross(up, east);
        let heading = add(scale(north, heading_from_north.cos()), scale(east, heading_from_north.sin()));
        Pose { pos: scale(up, radius + altitude), heading, pitch: 0.0 }
    }

    pub fn basis(&self) -> Basis {
        let vertical = norm(self.pos);
        let forward = add(scale(self.heading, self.pitch.cos()), scale(vertical, self.pitch.sin()));
        let right = norm(cross(forward, vertical));
        let up = cross(right, forward);
        Basis { right, up, forward, vertical }
    }

    pub fn altitude(&self, radius: f64) -> f64 {
        len(self.pos) - radius
    }

    /// Total heading angle is not stored; this is the yaw relative to north, for display only.
    pub fn heading_from_north(&self) -> f64 {
        let up = norm(self.pos);
        let east = norm(cross([0.0, 0.0, 1.0], up));
        let north = cross(up, east);
        dot(self.heading, east).atan2(dot(self.heading, north))
    }
}

/// Advance a pose by one frame of input. Positive `dyaw` turns right (clockwise seen from above), positive `dpitch` looks up.
pub fn step(p: &Pose, i: &Input) -> Pose {
    let up_old = norm(p.pos);
    // Yaw about the old vertical, pitch clamped.
    let heading = rotate(p.heading, up_old, -i.dyaw);
    let pitch = (p.pitch + i.dpitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    // Move along the great circle in the direction of the requested tangent displacement: one rotation about the axis
    // (up x direction) carries the position and, as parallel transport, the heading.
    let right = norm(cross(heading, up_old));
    let flat = add(scale(heading, i.forward), scale(right, i.right));
    let dist = len(flat);
    let r_old = len(p.pos);
    let radius = (r_old + i.up).max(1e-9);
    let (up_new, heading) = if dist < 1e-12 {
        (up_old, heading)
    } else {
        let axis = norm(cross(up_old, scale(flat, 1.0 / dist)));
        let angle = dist / r_old;
        (norm(rotate(up_old, axis, angle)), norm(rotate(heading, axis, angle)))
    };
    // Re-orthogonalise against the new vertical (rounding only).
    let heading = norm(add(heading, scale(up_new, -dot(heading, up_new))));
    let pos = scale(up_new, radius);
    Pose { pos, heading, pitch }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 1000.0;

    fn start() -> Pose {
        Pose::at(0.3, 0.7, R, 1.7, 0.4)
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    fn orthonormal(p: &Pose) {
        let b = p.basis();
        assert!(close(len(b.forward), 1.0, 1e-12) && close(len(b.right), 1.0, 1e-12) && close(len(b.up), 1.0, 1e-12));
        assert!(close(dot(b.forward, b.right), 0.0, 1e-12) && close(dot(b.forward, b.up), 0.0, 1e-12) && close(dot(b.right, b.up), 0.0, 1e-12));
        // No roll: the right vector is horizontal (perpendicular to the local vertical), whatever the input history.
        assert!(close(dot(b.right, b.vertical), 0.0, 1e-12), "roll {}", dot(b.right, b.vertical));
        assert!(close(dot(p.heading, b.vertical), 0.0, 1e-12) && close(len(p.heading), 1.0, 1e-12));
    }

    fn lcg(s: &mut u64) -> f64 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }

    #[test]
    fn no_roll_after_any_input_sequence() {
        let mut p = start();
        let mut s = 7u64;
        for _ in 0..20000 {
            let i = Input { dyaw: lcg(&mut s) * 0.2, dpitch: lcg(&mut s) * 0.2, forward: lcg(&mut s) * 30.0, right: lcg(&mut s) * 30.0, up: lcg(&mut s) * 0.5 };
            p = step(&p, &i);
            orthonormal(&p);
        }
    }

    #[test]
    fn pitch_is_clamped_short_of_the_poles_of_the_view() {
        let mut p = start();
        for _ in 0..10 {
            p = step(&p, &Input { dpitch: 1.0, ..Default::default() });
        }
        assert_eq!(p.pitch, PITCH_LIMIT);
        for _ in 0..10 {
            p = step(&p, &Input { dpitch: -1.0, ..Default::default() });
        }
        assert_eq!(p.pitch, -PITCH_LIMIT);
        orthonormal(&p);
    }

    #[test]
    fn yaw_is_reversible_and_split_invariant() {
        let p = start();
        let a = step(&step(&p, &Input { dyaw: 0.8, ..Default::default() }), &Input { dyaw: -0.8, ..Default::default() });
        assert!(close(len(add(a.heading, scale(p.heading, -1.0))), 0.0, 1e-12));
        let whole = step(&p, &Input { dyaw: 1.0, dpitch: 0.3, ..Default::default() });
        let halves = step(&step(&p, &Input { dyaw: 0.5, dpitch: 0.15, ..Default::default() }), &Input { dyaw: 0.5, dpitch: 0.15, ..Default::default() });
        assert!(len(add(whole.heading, scale(halves.heading, -1.0))) < 1e-12 && close(whole.pitch, halves.pitch, 1e-12));
    }

    #[test]
    fn the_same_distance_split_across_frames_arrives_at_the_same_place() {
        let p = start();
        let whole = step(&p, &Input { forward: 100.0, ..Default::default() });
        let mut q = p;
        for _ in 0..100 {
            q = step(&q, &Input { forward: 1.0, ..Default::default() });
        }
        // Great-circle motion: one long step and many short ones arrive at the same point.
        assert!(len(add(whole.pos, scale(q.pos, -1.0))) < 1.0e-9, "{}", len(add(whole.pos, scale(q.pos, -1.0))));
        assert!(close(whole.altitude(R), q.altitude(R), 1e-9));
    }

    #[test]
    fn walking_over_the_north_pole_keeps_the_heading_continuous() {
        // Start 100 m from the pole heading north, walk through the pole and beyond, 1 m per frame.
        let lat = std::f64::consts::FRAC_PI_2 - 100.0 / R;
        let mut p = Pose::at(lat, 0.3, R, 1.7, 0.0);
        let mut prev = p.heading;
        for k in 0..300 {
            p = step(&p, &Input { forward: 1.0, ..Default::default() });
            // The heading changes by well under a degree per metre of travel everywhere, including at the pole.
            assert!(dot(prev, p.heading) > 0.9999, "frame {k}: dot {}", dot(prev, p.heading));
            prev = p.heading;
            orthonormal(&p);
        }
        // After walking 300 m (more than the 100 m to the pole) we are past it: on the far meridian, heading roughly south.
        let from_north = p.heading_from_north().abs();
        assert!((from_north - std::f64::consts::PI).abs() < 1e-3, "heading after the pole {from_north}");
    }

    #[test]
    fn works_at_one_metre_and_in_orbit() {
        for alt in [1.0, 1.0e3, 2.0e4 * R] {
            let mut p = Pose::at(-0.4, 2.0, R, alt, 1.0);
            for _ in 0..1000 {
                p = step(&p, &Input { dyaw: 0.01, dpitch: 0.003, forward: 0.01 * (1.0 + alt), right: -0.003 * (1.0 + alt), up: 0.0 });
            }
            orthonormal(&p);
            assert!(close(p.altitude(R), alt, 1e-6 * (1.0 + alt)), "altitude {} vs {alt}", p.altitude(R));
        }
    }
}
