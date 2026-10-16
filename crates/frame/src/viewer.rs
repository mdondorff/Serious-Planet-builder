//! The decisions of the viewer window, GPU-free and display-free (CON-03, CON-04): the flight clock that pause and restart act on,
//! the camera of the scripted flight at a time, the key commands, the free camera mode, the frame statistics and the window
//! title with the readiness report (ADR 0042 point 4, ADR 0045 point 5). The `winit` loop in `app` only turns events into
//! [`Command`]s and [`Held`] keys and calls these.

use crate::camera::Camera;
use crate::controller::{CameraInput, ControllerParams};
use crate::freecam::{free_step, keep_clearance, FreeInput, FreePose};
use crate::scenes::{DrawnSet, FRAME_STEP_S};
use planet_core::{PlanetFixed, TileId};

// -------------------------------------------------------------------------------------------------------------------------
// The flight clock

/// The time of the scripted flight, on an injected millisecond clock (CON-04): pause stops it, resume continues from where it
/// stopped, restart starts again from zero. Nothing here reads a real clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightClock {
    /// Flight time accumulated before the current run, milliseconds.
    banked_ms: u64,
    /// The injected time at which the current run started; `None` while paused.
    running_since: Option<u64>,
}

impl FlightClock {
    /// A clock that starts running at `now_ms`.
    pub fn started(now_ms: u64) -> Self {
        Self { banked_ms: 0, running_since: Some(now_ms) }
    }

    pub fn is_paused(&self) -> bool {
        self.running_since.is_none()
    }

    /// Flight time in milliseconds at the injected time `now_ms` (a clock that went backwards counts as not having moved).
    pub fn elapsed_ms(&self, now_ms: u64) -> u64 {
        self.banked_ms + self.running_since.map_or(0, |s| now_ms.saturating_sub(s))
    }

    pub fn time_s(&self, now_ms: u64) -> f64 {
        self.elapsed_ms(now_ms) as f64 / 1000.0
    }

    pub fn pause(&mut self, now_ms: u64) {
        self.banked_ms = self.elapsed_ms(now_ms);
        self.running_since = None;
    }

    pub fn resume(&mut self, now_ms: u64) {
        if self.running_since.is_none() {
            self.running_since = Some(now_ms);
        }
    }

    pub fn toggle(&mut self, now_ms: u64) {
        if self.is_paused() {
            self.resume(now_ms);
        } else {
            self.pause(now_ms);
        }
    }

    /// Start again from zero, running.
    pub fn restart(&mut self, now_ms: u64) {
        *self = Self::started(now_ms);
    }
}

/// The camera of a script at flight time `t_s`: the cameras are one per [`FRAME_STEP_S`], and the camera between two of them is
/// their interpolation, so a window faster than 60 frames per second moves smoothly. The flight loops: after the last camera it
/// continues from the first (the world below is the same, only the streaming starts over, which `restart` also does).
pub fn camera_at(cameras: &[Camera], t_s: f64) -> Camera {
    assert!(!cameras.is_empty(), "a flight has cameras");
    let n = cameras.len();
    let x = (t_s.max(0.0) / FRAME_STEP_S).max(0.0);
    let k = x.floor();
    let frac = x - k;
    let i = (k as u64 % n as u64) as usize;
    // The last camera is not blended with the first: the loop is a cut.
    if i + 1 >= n || frac == 0.0 {
        return cameras[i];
    }
    let (a, b) = (cameras[i], cameras[i + 1]);
    Camera {
        position: PlanetFixed(a.position.0 + (b.position.0 - a.position.0) * frac),
        forward: (a.forward * (1.0 - frac) + b.forward * frac).normalized(),
        up: a.up * (1.0 - frac) + b.up * frac,
        ..a
    }
}

/// Whether `t_s` is past the end of the script, so the next frame is the first of the next loop.
pub fn loops_completed(frames: usize, t_s: f64) -> u64 {
    (t_s.max(0.0) / FRAME_STEP_S).floor() as u64 / frames.max(1) as u64
}

// -------------------------------------------------------------------------------------------------------------------------
// Commands and keys

/// What a key press asks of the viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    TogglePause,
    Restart,
    NextView,
    ToggleVsync,
    ToggleCamera,
    Quit,
}

/// Keys that act while they are held (the free camera).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub down: bool,
    pub up: bool,
    pub look_left: bool,
    pub look_right: bool,
    pub look_up: bool,
    pub look_down: bool,
    pub boost: bool,
}

/// Key look speed, radians per second.
pub const LOOK_RATE_RAD_S: f64 = 1.2;
/// Speed multiplier while the boost key is held.
pub const BOOST: f64 = 5.0;

impl Held {
    fn axis(plus: bool, minus: bool) -> f64 {
        f64::from(u8::from(plus)) - f64::from(u8::from(minus))
    }

    /// The held keys as the free camera's input.
    pub fn input(&self) -> FreeInput {
        FreeInput {
            keys: CameraInput {
                forward: Self::axis(self.forward, self.back),
                right: Self::axis(self.right, self.left),
                up: Self::axis(self.up, self.down),
                yaw_rate: LOOK_RATE_RAD_S * Self::axis(self.look_right, self.look_left),
                pitch_rate: LOOK_RATE_RAD_S * Self::axis(self.look_up, self.look_down),
                speed_scale: if self.boost { BOOST } else { 1.0 },
            },
            look_delta_rad: [0.0, 0.0],
        }
    }
}

/// The key bindings, as text for the usage message and the log.
pub const KEYS_HELP: &str = "P or Space pause, R restart the flight, V next debug view, T vsync on/off, C flight or free camera, \
     free camera: WASD move, Q/E down/up, arrows look, Shift boost; Esc quits";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraMode {
    /// The scripted flight on the flight clock.
    Flight,
    /// The free camera, from where the flight was when it was chosen.
    Free(FreePose),
}

/// The viewer's state: the flight clock, the debug view, vsync and the camera mode. No GPU, no window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewerState {
    pub clock: FlightClock,
    /// Index into the list of debug views the window cycles.
    pub view_index: usize,
    pub views: usize,
    pub vsync: bool,
    pub mode: CameraMode,
    pub held: Held,
    /// The user paused the flight (as against the free camera holding the clock).
    user_paused: bool,
}

impl ViewerState {
    pub fn new(now_ms: u64, views: usize, view_index: usize, vsync: bool) -> Self {
        assert!(views > 0 && view_index < views);
        Self {
            clock: FlightClock::started(now_ms),
            view_index,
            views,
            vsync,
            mode: CameraMode::Flight,
            held: Held::default(),
            user_paused: false,
        }
    }

    pub fn is_free(&self) -> bool {
        matches!(self.mode, CameraMode::Free(_))
    }

    /// Apply a command at the injected time `now_ms`; `flight_camera` is the camera of the flight now (where the free camera
    /// starts). Returns `true` for `Quit`.
    pub fn command(&mut self, cmd: Command, now_ms: u64, flight_camera: &Camera) -> bool {
        match cmd {
            Command::TogglePause => {
                // While the free camera holds the clock, pause is the user's wish for when the flight comes back.
                self.user_paused = !self.user_paused;
                if !self.is_free() {
                    self.clock.toggle(now_ms);
                }
            }
            Command::Restart => {
                self.user_paused = false;
                self.mode = CameraMode::Flight;
                self.clock.restart(now_ms);
            }
            Command::NextView => self.view_index = (self.view_index + 1) % self.views,
            Command::ToggleVsync => self.vsync = !self.vsync,
            Command::ToggleCamera => match self.mode {
                CameraMode::Flight => {
                    self.clock.pause(now_ms);
                    self.mode = CameraMode::Free(FreePose::from_camera(flight_camera));
                }
                CameraMode::Free(_) => {
                    self.mode = CameraMode::Flight;
                    if !self.user_paused {
                        self.clock.resume(now_ms);
                    }
                }
            },
            Command::Quit => return true,
        }
        false
    }

    /// The camera of this frame. In flight mode it is the script's at the clock's time; in free mode the pose moves by `dt` seconds
    /// of the held keys first: `height_m` is the camera's height above the terrain now, `ground_radius_m` the distance from the
    /// planet's centre to the terrain at the camera's direction after the move, supplied by `ground` for the new pose.
    pub fn camera(
        &mut self,
        cameras: &[Camera],
        now_ms: u64,
        dt_s: f64,
        params: &ControllerParams,
        height_m: &dyn Fn(PlanetFixed) -> f64,
        ground_radius_m: &dyn Fn(PlanetFixed) -> f64,
    ) -> Camera {
        match &mut self.mode {
            CameraMode::Flight => camera_at(cameras, self.clock.time_s(now_ms)),
            CameraMode::Free(pose) => {
                let shape = cameras[0];
                let moved = free_step(*pose, &self.held.input(), dt_s, height_m(pose.position), params);
                *pose = keep_clearance(moved, ground_radius_m(moved.position), params.min_clearance_m);
                pose.camera(shape.fov_y_rad, shape.aspect, shape.near_m)
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------------
// Readiness and the title

/// How much of the view the streaming has at the level the plan wants (ADR 0045 point 5): of the tiles the newest plan wants,
/// those drawn at their own level, and the mean number of levels the others lag (the unweighted form of the detail lag, TEST-018).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewCoverage {
    pub wanted: usize,
    pub exact: usize,
    pub mean_lag: f64,
}

/// Coverage of `wanted` by what is drawn. A wanted tile with finer tiles drawn below it counts as exact; one with nothing drawn at,
/// above or below it counts as lagging by its whole level plus one, as the detail lag does.
pub fn view_coverage(wanted: &[TileId], drawn: &DrawnSet) -> ViewCoverage {
    let (mut exact, mut lag) = (0usize, 0.0f64);
    for &t in wanted {
        match drawn.drawn_level_of(t) {
            Some(l) if l >= t.level() => exact += 1,
            Some(l) => lag += f64::from(t.level() - l),
            None if drawn.covered_below(t) => exact += 1,
            None => lag += f64::from(t.level()) + 1.0,
        }
    }
    ViewCoverage { wanted: wanted.len(), exact, mean_lag: if wanted.is_empty() { 0.0 } else { lag / wanted.len() as f64 } }
}

/// The GPU-free readiness report (ADR 0045 point 5): bake blocks done and total, pre-fill tiles done and total, and the coverage
/// of the current view. An application shows it as a loading screen or starts early on coarse terrain; the M5 viewer shows it in
/// the title. A source without a pre-fill (an in-memory bake) reports `0 of 0`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Readiness {
    pub bake_done: u64,
    pub bake_total: u64,
    pub prefill_done: u64,
    pub prefill_total: u64,
    pub view: ViewCoverage,
}

impl Readiness {
    /// Share of the current view at its wanted level, in percent (100 when nothing is wanted).
    pub fn view_percent(&self) -> f64 {
        if self.view.wanted == 0 {
            100.0
        } else {
            100.0 * self.view.exact as f64 / self.view.wanted as f64
        }
    }

    /// Everything is there: the bake, the pre-fill and the view at its level.
    pub fn is_ready(&self) -> bool {
        self.bake_done == self.bake_total && self.prefill_done == self.prefill_total && self.view.exact == self.view.wanted
    }

    pub fn summary(&self) -> String {
        let prefill = if self.prefill_total == 0 {
            "prefill n/a".to_string()
        } else {
            format!("prefill {}/{}", self.prefill_done, self.prefill_total)
        };
        format!(
            "{} - bake {}/{}, {prefill}, view {:.0}% (lag {:.2})",
            if self.is_ready() { "ready" } else { "loading" },
            self.bake_done,
            self.bake_total,
            self.view_percent(),
            self.view.mean_lag
        )
    }
}

/// Frame statistics over a window of injected time: the frame rate and the mean CPU frame time of the frames since the last
/// report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameStats {
    since_ms: u64,
    frames: u32,
    cpu_ms_sum: f64,
    period_ms: u64,
}

/// What a window of frames came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatsReport {
    pub fps: f64,
    pub cpu_ms: f64,
}

impl FrameStats {
    pub fn new(now_ms: u64, period_ms: u64) -> Self {
        Self { since_ms: now_ms, frames: 0, cpu_ms_sum: 0.0, period_ms }
    }

    /// Record a frame that took `cpu_ms` of CPU time, finished at `now_ms`; reports and starts a new window once the period passed.
    pub fn frame(&mut self, now_ms: u64, cpu_ms: f64) -> Option<StatsReport> {
        self.frames += 1;
        self.cpu_ms_sum += cpu_ms;
        let span = now_ms.saturating_sub(self.since_ms);
        if span < self.period_ms {
            return None;
        }
        let report = StatsReport { fps: f64::from(self.frames) * 1000.0 / span as f64, cpu_ms: self.cpu_ms_sum / f64::from(self.frames) };
        *self = Self::new(now_ms, self.period_ms);
        Some(report)
    }
}

/// What the title shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleInfo {
    pub fps: f64,
    pub cpu_ms: f64,
    /// GPU time of the terrain pass; `None` when the device has no timestamps or none was taken yet.
    pub gpu_ms: Option<f64>,
    pub vsync: bool,
    pub present_mode: String,
    pub view: String,
    pub free: bool,
    pub paused: bool,
    pub flight_s: f64,
    pub altitude_m: f64,
    pub readiness: Readiness,
}

pub fn title(i: &TitleInfo) -> String {
    let gpu = i.gpu_ms.map_or("gpu n/a".to_string(), |g| format!("gpu {g:.2} ms"));
    let camera = if i.free {
        "free camera".to_string()
    } else if i.paused {
        format!("flight {:.1} s (paused)", i.flight_s)
    } else {
        format!("flight {:.1} s", i.flight_s)
    };
    let alt = if i.altitude_m >= 10_000.0 { format!("{:.0} km", i.altitude_m / 1000.0) } else { format!("{:.1} m", i.altitude_m) };
    format!(
        "planet | {:.0} fps | cpu {:.2} ms | {gpu} | vsync {} ({}) | {} | {camera} | alt {alt} | {}",
        i.fps,
        i.cpu_ms,
        if i.vsync { "on" } else { "off" },
        i.present_mode,
        i.view,
        i.readiness.summary()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::Vec3;

    fn cam(x: f64) -> Camera {
        Camera {
            position: PlanetFixed(Vec3::new(6.4e6 + x, 0.0, 0.0)),
            forward: Vec3::new(0.0, 1.0, 0.0),
            up: Vec3::new(1.0, 0.0, 0.0),
            fov_y_rad: 1.0,
            aspect: 1.5,
            near_m: 0.1,
        }
    }

    fn script(n: usize) -> Vec<Camera> {
        (0..n).map(|k| cam(k as f64)).collect()
    }

    // spec: VIEW-002
    #[test]
    fn pause_stops_the_flight_and_resume_continues_from_there() {
        let mut c = FlightClock::started(1000);
        assert_eq!(c.elapsed_ms(1000), 0);
        assert_eq!(c.elapsed_ms(3500), 2500);
        c.pause(3500);
        assert!(c.is_paused());
        // Time passes while paused; the flight does not.
        assert_eq!(c.elapsed_ms(60_000), 2500);
        c.resume(60_000);
        assert_eq!(c.elapsed_ms(61_000), 3500);
        c.toggle(61_000);
        assert!(c.is_paused() && c.elapsed_ms(99_999) == 3500);
        c.toggle(70_000);
        assert!(!c.is_paused() && c.elapsed_ms(71_000) == 4500);
        // Pausing twice and resuming twice change nothing more.
        c.pause(71_000);
        c.pause(80_000);
        assert_eq!(c.elapsed_ms(90_000), 4500);
        c.resume(90_000);
        c.resume(95_000);
        assert_eq!(c.elapsed_ms(91_000), 5500);
        // A clock that goes backwards does not rewind the flight.
        assert_eq!(c.elapsed_ms(80_000), 4500);
    }

    // spec: VIEW-002
    #[test]
    fn restart_starts_again_from_zero_and_runs() {
        let mut c = FlightClock::started(0);
        c.pause(5000);
        c.restart(9000);
        assert!(!c.is_paused());
        assert_eq!(c.elapsed_ms(9000), 0);
        assert_eq!(c.elapsed_ms(9250), 250);
    }

    // spec: VIEW-002
    #[test]
    fn the_camera_at_a_time_is_the_scripts_and_the_same_flight_replays() {
        let cams = script(100);
        // On a frame step it is the script's camera exactly; the same time always gives the same camera.
        for k in [0usize, 1, 50, 99] {
            assert_eq!(camera_at(&cams, k as f64 * FRAME_STEP_S), cams[k]);
        }
        assert_eq!(camera_at(&cams, 0.7321), camera_at(&cams, 0.7321));
        // Between two cameras it is their blend: half a step is the midpoint.
        let mid = camera_at(&cams, 10.5 * FRAME_STEP_S);
        assert!((mid.position.0.x - (6.4e6 + 10.5)).abs() < 1e-6);
        assert!((mid.forward.length() - 1.0).abs() < 1e-12);
        // Past the end the flight loops: the last camera is not blended into the first.
        let end = 99.0 * FRAME_STEP_S;
        assert_eq!(camera_at(&cams, end + 0.5 * FRAME_STEP_S), cams[99]);
        assert_eq!(camera_at(&cams, 100.0 * FRAME_STEP_S), cams[0]);
        assert_eq!(camera_at(&cams, 205.0 * FRAME_STEP_S), cams[5]);
        assert_eq!(loops_completed(100, 99.9 * FRAME_STEP_S), 0);
        assert_eq!(loops_completed(100, 100.0 * FRAME_STEP_S), 1);
        // Time before zero is the first camera.
        assert_eq!(camera_at(&cams, -3.0), cams[0]);
    }

    // spec: VIEW-002
    #[test]
    fn the_camera_follows_the_clock_through_a_pause_and_a_restart() {
        let cams = script(600);
        let mut v = ViewerState::new(0, 9, 7, true);
        let p = ControllerParams::default();
        let h = |_: PlanetFixed| 100.0;
        let g = |_: PlanetFixed| 6.0e6;
        let at = |v: &mut ViewerState, now: u64| v.camera(&cams, now, 0.016, &p, &h, &g).position.0.x - 6.4e6;
        assert!((at(&mut v, 2000) - 120.0).abs() < 1e-6, "2 s is frame 120");
        v.command(Command::TogglePause, 2000, &cams[120]);
        assert!((at(&mut v, 8000) - 120.0).abs() < 1e-6, "paused");
        v.command(Command::TogglePause, 8000, &cams[120]);
        assert!((at(&mut v, 9000) - 180.0).abs() < 1e-6, "one more second");
        v.command(Command::Restart, 9500, &cams[0]);
        assert!((at(&mut v, 9500) - 0.0).abs() < 1e-6 && (at(&mut v, 10_000) - 30.0).abs() < 1e-6);
    }

    // spec: VIEW-003
    #[test]
    fn the_view_cycles_through_every_view_and_back_and_vsync_toggles() {
        let mut v = ViewerState::new(0, 9, 7, true);
        let c = cam(0.0);
        let mut seen = vec![v.view_index];
        for _ in 0..9 {
            v.command(Command::NextView, 0, &c);
            seen.push(v.view_index);
        }
        assert_eq!(seen.first(), seen.last());
        seen.pop();
        seen.sort_unstable();
        assert_eq!(seen, (0..9).collect::<Vec<_>>());
        assert!(v.vsync);
        v.command(Command::ToggleVsync, 0, &c);
        assert!(!v.vsync);
        v.command(Command::ToggleVsync, 0, &c);
        assert!(v.vsync);
        assert!(v.command(Command::Quit, 0, &c));
        assert!(!v.command(Command::NextView, 0, &c));
    }

    // spec: VIEW-004
    #[test]
    fn the_free_camera_takes_over_from_the_flight_and_gives_it_back() {
        let cams = script(600);
        let p = ControllerParams::default();
        let h = |_: PlanetFixed| 5000.0;
        let g = |_: PlanetFixed| 6.0e6;
        let mut v = ViewerState::new(0, 9, 7, true);
        let before = v.camera(&cams, 1000, 0.016, &p, &h, &g);
        v.command(Command::ToggleCamera, 1000, &before);
        assert!(v.is_free());
        // The flight clock holds while the free camera flies; with nothing held the camera stays where the flight left it.
        let still = v.camera(&cams, 5000, 0.016, &p, &h, &g);
        assert!((still.position.0 - before.position.0).length() < 1e-9);
        v.held.forward = true;
        let moved = v.camera(&cams, 5100, 0.1, &p, &h, &g);
        assert!((moved.position.0 - before.position.0).length() > 100.0, "W moves at 5 km a second");
        v.held = Held::default();
        // Back to the flight: it resumes at its time (1 s), not at the 5 s of the wall.
        v.command(Command::ToggleCamera, 6000, &moved);
        assert!(!v.is_free());
        let back = v.camera(&cams, 6000, 0.016, &p, &h, &g);
        assert_eq!(back, camera_at(&cams, 1.0));
        // A pause chosen while free is kept when the flight returns.
        v.command(Command::ToggleCamera, 7000, &back);
        v.command(Command::TogglePause, 7500, &back);
        v.command(Command::ToggleCamera, 8000, &back);
        assert!(v.clock.is_paused());
        let held = v.camera(&cams, 20_000, 0.016, &p, &h, &g);
        assert_eq!(held, camera_at(&cams, v.clock.time_s(20_000)));
        // Restart leaves the free camera.
        v.command(Command::ToggleCamera, 21_000, &held);
        v.command(Command::Restart, 22_000, &held);
        assert!(!v.is_free() && !v.clock.is_paused());
    }

    // spec: VIEW-004
    #[test]
    fn held_keys_become_the_free_cameras_input() {
        let i = Held { forward: true, left: true, up: true, look_right: true, look_down: true, boost: true, ..Held::default() }.input();
        assert_eq!((i.keys.forward, i.keys.right, i.keys.up), (1.0, -1.0, 1.0));
        assert_eq!(i.keys.yaw_rate, LOOK_RATE_RAD_S);
        assert_eq!(i.keys.pitch_rate, -LOOK_RATE_RAD_S);
        assert_eq!(i.keys.speed_scale, BOOST);
        // Opposite keys cancel.
        let i = Held { forward: true, back: true, ..Held::default() }.input();
        assert_eq!(i.keys.forward, 0.0);
        assert_eq!(i.look_delta_rad, [0.0, 0.0]);
    }

    fn tile(level: u8, x: u32, y: u32) -> TileId {
        TileId::new(planet_core::Face(0), level, x, y).expect("tile")
    }

    // spec: VIEW-003
    #[test]
    fn coverage_counts_the_wanted_tiles_drawn_at_their_level() {
        let root = tile(0, 0, 0);
        let kids: Vec<TileId> = root.children().expect("children").to_vec();
        let wanted = kids.clone();
        // Everything wanted is drawn.
        let all = DrawnSet::new(kids.clone());
        let c = view_coverage(&wanted, &all);
        assert_eq!((c.wanted, c.exact, c.mean_lag), (4, 4, 0.0));
        // Only the root is drawn: nothing exact, each wanted tile lags one level.
        let coarse = DrawnSet::new([root]);
        let c = view_coverage(&wanted, &coarse);
        assert_eq!((c.exact, c.mean_lag), (0, 1.0));
        // Half drawn.
        let half = DrawnSet::new(kids[..2].to_vec());
        let c = view_coverage(&wanted, &half);
        assert_eq!(c.exact, 2);
        // A wanted root with finer tiles drawn below counts as exact.
        let c = view_coverage(&[root], &all);
        assert_eq!(c.exact, 1);
        // Nothing drawn at, above or below: lag is the level plus one.
        let c = view_coverage(&[kids[0]], &DrawnSet::new([]));
        assert_eq!((c.exact, c.mean_lag), (0, 2.0));
        // Nothing wanted is ready.
        let r = Readiness { view: view_coverage(&[], &all), ..Default::default() };
        assert!(r.is_ready() && r.view_percent() == 100.0);
    }

    // spec: VIEW-003
    #[test]
    fn the_readiness_report_counts_and_says_whether_it_is_ready() {
        let mut r = Readiness {
            bake_done: 3,
            bake_total: 6,
            prefill_done: 0,
            prefill_total: 340,
            view: ViewCoverage { wanted: 10, exact: 5, mean_lag: 0.75 },
        };
        assert!(!r.is_ready());
        assert_eq!(r.summary(), "loading - bake 3/6, prefill 0/340, view 50% (lag 0.75)");
        r.bake_done = 6;
        r.prefill_done = 340;
        r.view = ViewCoverage { wanted: 10, exact: 10, mean_lag: 0.0 };
        assert!(r.is_ready());
        assert_eq!(r.summary(), "ready - bake 6/6, prefill 340/340, view 100% (lag 0.00)");
        let none = Readiness { bake_done: 6, bake_total: 6, ..Default::default() };
        assert!(none.summary().contains("prefill n/a"));
    }

    // spec: VIEW-003
    #[test]
    fn frame_statistics_report_once_per_period_against_the_injected_clock() {
        let mut s = FrameStats::new(0, 500);
        let mut reports = Vec::new();
        // 100 frames at 10 ms each, 3 ms of CPU time: 100 fps.
        for k in 1..=100u64 {
            if let Some(r) = s.frame(k * 10, 3.0) {
                reports.push((k, r));
            }
        }
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].0, 50);
        assert!((reports[0].1.fps - 100.0).abs() < 1e-9 && (reports[0].1.cpu_ms - 3.0).abs() < 1e-12);
        // A stall shows: five frames in the next 500 ms is 10 fps.
        let mut s = FrameStats::new(0, 500);
        let r = (1..=5u64).filter_map(|k| s.frame(k * 100, 1.0)).next().expect("a report");
        assert!((r.fps - 10.0).abs() < 1e-9);
    }

    // spec: VIEW-003
    #[test]
    fn the_title_shows_the_rate_the_times_the_state_and_the_readiness() {
        let mut i = TitleInfo {
            fps: 143.6,
            cpu_ms: 3.214,
            gpu_ms: Some(2.05),
            vsync: false,
            present_mode: "Immediate".into(),
            view: "relief".into(),
            free: false,
            paused: false,
            flight_s: 12.34,
            altitude_m: 1.0,
            readiness: Readiness {
                bake_done: 6,
                bake_total: 6,
                prefill_done: 340,
                prefill_total: 340,
                view: ViewCoverage { wanted: 4, exact: 4, mean_lag: 0.0 },
            },
        };
        let t = title(&i);
        assert_eq!(
            t,
            "planet | 144 fps | cpu 3.21 ms | gpu 2.05 ms | vsync off (Immediate) | relief | flight 12.3 s | alt 1.0 m | ready - bake 6/6, prefill 340/340, view 100% (lag 0.00)"
        );
        i.gpu_ms = None;
        i.paused = true;
        i.altitude_m = 2.0e7;
        i.vsync = true;
        let t = title(&i);
        assert!(
            t.contains("gpu n/a") && t.contains("flight 12.3 s (paused)") && t.contains("alt 20000 km") && t.contains("vsync on"),
            "{t}"
        );
        i.free = true;
        assert!(title(&i).contains("free camera"));
    }
}
