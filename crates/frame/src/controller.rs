//! Adaptive camera (report §10): speed proportional to the height above the terrain, with orbit and fly modes. A pure
//! state machine in float64 over planet-fixed positions; input is plain data and time is a step, so it is
//! deterministic and testable (CON-04).

use crate::camera::Camera;
use planet_core::{PlanetFixed, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraMode {
    /// Rotate around the planet centre at the current distance; the view always points at the centre.
    Orbit,
    /// Free flight along the view direction and sideways.
    Fly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// Forward (+) / backward (-), sideways right (+) / left (-), up (+) / down (-), each in [-1, 1].
    pub forward: f64,
    pub right: f64,
    pub up: f64,
    /// Turn rates in radians per second (yaw to the right, pitch up).
    pub yaw_rate: f64,
    pub pitch_rate: f64,
    /// Multiplies the adaptive speed (boost); 0 means 1.
    pub speed_scale: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerParams {
    /// Speed in "heights per second": at 1 km above the ground and 1.0, the camera moves 1 km/s.
    pub heights_per_second: f64,
    pub min_speed_mps: f64,
    pub max_speed_mps: f64,
    /// The camera never gets closer to the terrain than this, metres.
    pub min_clearance_m: f64,
    pub radius_m: f64,
}

impl Default for ControllerParams {
    fn default() -> Self {
        Self { heights_per_second: 1.0, min_speed_mps: 0.5, max_speed_mps: 3.0e6, min_clearance_m: 1.0, radius_m: 6_371_000.0 }
    }
}

/// Maximum pitch above or below the horizon, radians (85 degrees).
const MAX_PITCH: f64 = 1.4835;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraController {
    pub mode: CameraMode,
    pub position: PlanetFixed,
    /// Unit tangent direction the camera faces on the ground.
    pub heading: Vec3,
    /// Angle above (+) or below (-) the horizon, radians.
    pub pitch: f64,
    pub params: ControllerParams,
}

/// `v` projected onto the tangent plane of `up`; falls back to any tangent when `v` is (nearly) parallel to `up`.
fn tangent(v: Vec3, up: Vec3) -> Vec3 {
    let t = v - up * v.dot(up);
    if t.length() > 1e-9 {
        return t.normalized();
    }
    let axis = if up.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    (axis - up * axis.dot(up)).normalized()
}

impl CameraController {
    pub fn new(position: PlanetFixed, forward_hint: Vec3, params: ControllerParams) -> Self {
        let up = position.0.normalized();
        Self { mode: CameraMode::Fly, position, heading: tangent(forward_hint, up), pitch: 0.0, params }
    }

    pub fn up(&self) -> Vec3 {
        self.position.0.normalized()
    }

    /// View direction: the heading tilted by the pitch, or straight at the centre in orbit mode.
    pub fn forward(&self) -> Vec3 {
        let up = self.up();
        match self.mode {
            CameraMode::Orbit => -up,
            CameraMode::Fly => (self.heading * libm::cos(self.pitch) + up * libm::sin(self.pitch)).normalized(),
        }
    }

    /// Speed in metres per second for a given height above the terrain.
    pub fn speed_for_height(&self, height_m: f64) -> f64 {
        (height_m.max(0.0) * self.params.heights_per_second).clamp(self.params.min_speed_mps, self.params.max_speed_mps)
    }

    /// Height above the terrain surface: distance from the centre minus the radius minus the terrain height below.
    pub fn height_above_terrain(&self, terrain_height_m: &dyn Fn(Vec3) -> f64) -> f64 {
        let d = self.position.0.length();
        d - self.params.radius_m - terrain_height_m(self.position.0 * (1.0 / d))
    }

    /// Advance by `dt` seconds (negative or non-finite `dt` counts as 0; NaN inputs count as 0). Long steps are split so that
    /// the camera never moves more than half its height above the terrain per sub-step and cannot tunnel through the planet.
    /// `terrain_height_m` gives the terrain height above the sphere for a unit direction.
    pub fn step(&mut self, input: CameraInput, dt: f64, terrain_height_m: &dyn Fn(Vec3) -> f64) {
        let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
        let input = CameraInput {
            forward: fin(input.forward).clamp(-1.0, 1.0),
            right: fin(input.right).clamp(-1.0, 1.0),
            up: fin(input.up).clamp(-1.0, 1.0),
            yaw_rate: fin(input.yaw_rate),
            pitch_rate: fin(input.pitch_rate),
            speed_scale: fin(input.speed_scale),
        };
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let height = self.height_above_terrain(terrain_height_m).max(self.params.min_clearance_m);
        let speed = self.speed_for_height(height) * if input.speed_scale > 0.0 { input.speed_scale } else { 1.0 };
        let travel = speed * dt * (input.forward.abs() + input.right.abs() + input.up.abs()).max(1.0);
        let n = ((travel / (0.5 * height)).ceil() as usize).clamp(1, 20_000);
        for _ in 0..n {
            self.step_once(input, dt / n as f64, terrain_height_m);
        }
    }

    fn step_once(&mut self, input: CameraInput, dt: f64, terrain_height_m: &dyn Fn(Vec3) -> f64) {
        let height = self.height_above_terrain(terrain_height_m);
        let speed = self.speed_for_height(height) * if input.speed_scale > 0.0 { input.speed_scale } else { 1.0 };
        let up = self.up();
        // Turn: yaw rotates the heading about the local up; pitch accumulates and is limited.
        let right = self.heading.cross(up).normalized();
        let yaw = input.yaw_rate * dt;
        self.heading = tangent(self.heading * libm::cos(yaw) + right * libm::sin(yaw), up);
        if self.mode == CameraMode::Fly {
            self.pitch = (self.pitch + input.pitch_rate * dt).clamp(-MAX_PITCH, MAX_PITCH);
        }
        let right = self.heading.cross(up).normalized();
        match self.mode {
            CameraMode::Fly => {
                let mv = self.forward() * input.forward + right * input.right + up * input.up;
                self.position = PlanetFixed(self.position.0 + mv * (speed * dt));
            }
            CameraMode::Orbit => {
                // Forward/back changes the distance; right/up move over the sphere around the centre.
                let d = self.position.0.length();
                // Screen-up in orbit mode is the heading.
                let north = self.heading;
                let angle = speed * dt / d;
                let dir = (up + right * (input.right * angle) + north * (input.up * angle)).normalized();
                let nd = (d - input.forward * speed * dt).max(self.params.radius_m * 0.5);
                self.position = PlanetFixed(dir * nd);
            }
        }
        // Never closer to the terrain than the clearance.
        let d = self.position.0.length();
        let dir = self.position.0 * (1.0 / d);
        let floor = self.params.radius_m + terrain_height_m(dir) + self.params.min_clearance_m;
        if d < floor {
            self.position = PlanetFixed(dir * floor);
        }
        self.heading = tangent(self.heading, self.up());
    }

    pub fn camera(&self, fov_y_rad: f64, aspect: f64, near_m: f64) -> Camera {
        // Orbit looks straight down, so use the heading as the up hint to keep the basis well defined.
        let up_hint = if self.mode == CameraMode::Orbit { self.heading } else { self.up() };
        Camera { position: self.position, forward: self.forward(), up: up_hint, fov_y_rad, aspect, near_m }
    }
}
