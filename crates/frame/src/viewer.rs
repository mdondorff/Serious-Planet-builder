//! The decisions of the viewer window, GPU-free and display-free (CON-03, CON-04): the flight clock that pause and restart act on,
//! the camera of the scripted flight at a time, the key commands, the free camera mode, the frame statistics and the window
//! title with the readiness report (ADR 0042 point 4, ADR 0045 point 5). The `winit` loop in `app` only turns events into
//! [`Command`]s and [`Held`] keys and calls these.

use crate::camera::Camera;
use crate::controller::{CameraInput, ControllerParams};
use crate::freecam::{free_step, keep_clearance, FreeInput, FreePose};
use crate::globe::{landing, GlobeSession};
use crate::lod::LodParams;
use crate::look::Overlay;
use crate::mouselook::MouseLook;
use crate::scenes::{DrawnSet, FRAME_STEP_S};
use planet_core::{PlanetFixed, TileId, Vec3};

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
    /// L: the lit path with the atmosphere, or the debug path (VIEW-007).
    ToggleLit,
    /// F3: the next debug overlay over the lit terrain: off, tiles, mesh, regions (VIEW-014, VIEW-030).
    CycleOverlay,
    /// F4: the coarse-LOD inspection view, the error bound times four (VIEW-014).
    ToggleCoarseLod,
    /// H: the editing light, a sun fixed in camera space over the viewer's shoulder, or back to the look's sun (VIEW-042).
    ToggleEditingLight,
    /// G: the globe overview over the point under the camera, or back (VIEW-013).
    ToggleGlobe,
    /// F9: write the camera path of the last minute and the repro bundle of this frame (VIEW-024). Changes nothing in the viewer; the window
    /// takes the request with [`ViewerState::take_repro_request`].
    SaveRepro,
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

/// The LOD parameters a frame is planned with: the base ones, or with the error bound times four in the coarse-LOD inspection view
/// (VIEW-014).
pub fn effective_lod(base: &LodParams, coarse: bool) -> LodParams {
    if coarse {
        base.coarse()
    } else {
        base.clone()
    }
}

/// Plan a frame with `attempt(coarse)`, `coarse` being the state of the coarse-LOD view (VIEW-014). When the coarse view is on and the
/// attempt fails (the coarser error bound does not fit the terrain here: a steep neighbourhood leaves no morph room), the switch is
/// turned off, the refusal is counted and the attempt is made again with the base parameters, so the window does not stop for it.
/// A failure with the coarse view off, or the second one, is returned as it is.
pub fn with_coarse_fallback<T, E>(coarse: &mut bool, refusals: &mut u64, mut attempt: impl FnMut(bool) -> Result<T, E>) -> Result<T, E> {
    match attempt(*coarse) {
        Err(_) if *coarse => {
            *coarse = false;
            *refusals += 1;
            attempt(false)
        }
        other => other,
    }
}

/// The corner label of the editor: the active overlay and the LOD error-bound factor (1 off, [`LodParams::coarse_factor`] on), e.g.
/// `Overlay: Mesh, LOD x4` or `Overlay: Tiles, LOD x2.6` (VIEW-014).
pub fn overlay_status(overlay: Overlay, lod_factor: f64) -> String {
    let overlay = match overlay {
        Overlay::None => "off",
        Overlay::Tiles => "Tiles",
        Overlay::Mesh => "Mesh",
        Overlay::Regions => "Regions",
        Overlay::Stale => "Stale tiles",
    };
    let lod = if (lod_factor - lod_factor.round()).abs() < 0.05 {
        format!("LOD x{}", lod_factor.round())
    } else {
        format!("LOD x{lod_factor:.1}")
    };
    format!("Overlay: {overlay}, {lod}")
}

/// The colours of the stale-tile overlay by [`TileState`](crate::terrain::TileState) code (the shader's palette, VIEW-044): exact green, stale
/// amber, stand-in red, previewed violet, capped grey.
pub const STATE_COLOURS: [[u8; 3]; 5] = [[38, 217, 77], [255, 153, 26], [242, 51, 51], [179, 77, 255], [153, 153, 153]];

/// How many drawn nodes are in each state, by state code (VIEW-044).
pub fn state_counts(nodes: &[crate::terrain::TerrainDraw]) -> [usize; 5] {
    let mut c = [0usize; 5];
    for n in nodes {
        c[n.state.code() as usize] += 1;
    }
    c
}

/// The counts for the title while the stale-tile overlay is on, e.g. `tiles 120 exact, 4 stale, 2 stand-in, 1 previewed`; capped is named only
/// when some tile is (ADR 0074).
pub fn state_title(c: [usize; 5]) -> String {
    let mut s = format!("tiles {} exact, {} stale, {} stand-in, {} previewed", c[0], c[1], c[2], c[3]);
    if c[4] > 0 {
        s += &format!(", {} capped", c[4]);
    }
    s
}

/// The legend of the overlay: a colour and what it says, in state order (capped marked as reserved while no tile is ever capped).
pub fn state_legend() -> Vec<([u8; 3], String)> {
    let words = [
        "exact: resident with the current key",
        "stale: an old version, its refresh is pending",
        "stand-in: a coarser tile covers a missing one",
        "previewed: a preview stamp displaces it",
        "capped: slope limit (reserved)",
    ];
    STATE_COLOURS.iter().copied().zip(words.map(String::from)).collect()
}

/// The key bindings, as text for the usage message and the log.
pub const KEYS_HELP: &str = "P or Space pause, R restart the flight, L lit path (with the atmosphere) or debug path, G globe overview (drag turns it, wheel zooms, a click on a marker or the surface flies there and lands in the free camera), F3 cycles the overlay (off, tiles, mesh, regions, stale tiles; lit path), F4 coarse LOD (error bound x4), F9 writes the camera path of the last minute and a repro bundle (target/review), V next debug view (debug path), T vsync on/off, C flight or free camera, \
     H editing light (a sun fixed over your shoulder, 35 degrees up, so no slope is black; off by default), \
     free camera: WASD move, Q/E down/up, arrows look, click captures the mouse to look (Esc releases it), Shift boost; Esc quits";

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
    /// Mouse look of the free camera (VIEW-010): the grab state and the raw motion since the last frame.
    pub mouse: MouseLook,
    /// The lit path is shown (VIEW-007); the debug path otherwise.
    pub lit: bool,
    /// The debug overlay drawn over the lit terrain (VIEW-014); off at the start.
    pub overlay: Overlay,
    /// The coarse-LOD inspection view (VIEW-014): the plan's error bound times [`COARSE_LOD_FACTOR`](crate::lod::COARSE_LOD_FACTOR); off at
    /// the start. It changes the view, not the data.
    pub coarse_lod: bool,
    /// The editing light (VIEW-042): the lit frames take [`editing_sun_dir`] for the sun; off at the start, so the look's sun and every golden stay.
    pub editing_light: bool,
    /// The globe overview while it is shown (VIEW-013): its view is the camera, and a click's flight ends in the free camera.
    pub globe: Option<GlobeSession>,
    /// The planet radius the overview is built on, metres.
    pub radius_m: f64,
    /// The next lit frame snaps the exposure (a first lit frame, a return to the lit path, a restart of the flight).
    exposure_reset: bool,
    repro_requested: bool,
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
            mouse: MouseLook::new(),
            lit: false,
            overlay: Overlay::None,
            coarse_lod: false,
            editing_light: false,
            globe: None,
            radius_m: 6_371_000.0,
            exposure_reset: true,
            repro_requested: false,
            user_paused: false,
        }
    }

    /// A viewer on a planet of `radius_m` (the globe overview's sphere).
    pub fn with_radius_m(mut self, radius_m: f64) -> Self {
        self.radius_m = radius_m;
        self
    }

    /// A viewer that starts on the lit path (the editor window) or on the debug path.
    pub fn with_lit(mut self, lit: bool) -> Self {
        self.lit = lit;
        self
    }

    /// Whether the next lit frame snaps the exposure to its target, once: true for the first lit frame, after the lit path was
    /// switched back on and after a restart of the flight (a cut).
    pub fn take_exposure_reset(&mut self) -> bool {
        std::mem::take(&mut self.exposure_reset)
    }

    /// True once after F9 was pressed (VIEW-024).
    pub fn take_repro_request(&mut self) -> bool {
        std::mem::take(&mut self.repro_requested)
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
                self.exposure_reset = true;
                self.user_paused = false;
                self.mode = CameraMode::Flight;
                self.mouse.release();
                self.clock.restart(now_ms);
            }
            // The debug views belong to the debug path; on the lit path V does nothing.
            Command::NextView => {
                if !self.lit {
                    self.view_index = (self.view_index + 1) % self.views;
                }
            }
            Command::ToggleLit => {
                self.lit = !self.lit;
                self.exposure_reset |= self.lit;
            }
            Command::CycleOverlay => self.overlay = self.overlay.next(),
            Command::SaveRepro => self.repro_requested = true,
            Command::ToggleCoarseLod => self.coarse_lod = !self.coarse_lod,
            Command::ToggleEditingLight => self.editing_light = !self.editing_light,
            Command::ToggleGlobe => {
                self.globe = match self.globe {
                    Some(_) => None,
                    None => {
                        // Over the point under the camera now: the free camera's, or the flight's.
                        let from = match &self.mode {
                            CameraMode::Free(pose) => pose.position.0,
                            CameraMode::Flight => flight_camera.position.0,
                        };
                        self.mouse.release();
                        Some(GlobeSession::enter(from, self.radius_m))
                    }
                };
            }
            Command::ToggleVsync => self.vsync = !self.vsync,
            Command::ToggleCamera => match self.mode {
                CameraMode::Flight => {
                    self.clock.pause(now_ms);
                    self.mode = CameraMode::Free(FreePose::from_camera(flight_camera));
                }
                CameraMode::Free(_) => {
                    self.mode = CameraMode::Flight;
                    self.mouse.release();
                    if !self.user_paused {
                        self.clock.resume(now_ms);
                    }
                }
            },
            Command::Quit => return true,
        }
        false
    }

    /// The window lost the focus (VIEW-010): the mouse is released and no key stays down.
    pub fn focus_lost(&mut self) {
        self.mouse.focus_lost();
        self.held = Held::default();
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
        if let Some(globe) = &mut self.globe {
            // The overview is the camera; the frame a click's flight ends the free camera takes over where it landed.
            let shape = cameras[0];
            match globe.step(dt_s) {
                Some(end) => {
                    let (position, heading, pitch) = landing(&end, globe.radius_m);
                    let pose = FreePose::new(position, heading, pitch);
                    self.globe = None;
                    if !self.is_free() {
                        self.clock.pause(now_ms);
                    }
                    self.mode = CameraMode::Free(pose);
                    return pose.camera(shape.fov_y_rad, shape.aspect, shape.near_m);
                }
                None => return globe.view.camera(globe.radius_m, shape.fov_y_rad, shape.aspect, shape.near_m),
            }
        }
        match &mut self.mode {
            CameraMode::Flight => camera_at(cameras, self.clock.time_s(now_ms)),
            CameraMode::Free(pose) => {
                let shape = cameras[0];
                // The mouse's turn of this frame rides on the held keys' input: counts x sensitivity, not scaled by `dt_s`.
                let input = FreeInput { look_delta_rad: self.mouse.take_delta(), ..self.held.input() };
                let moved = free_step(*pose, &input, dt_s, height_m(pose.position), params);
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

/// The window title while the roots (the tiles of the render floor, 384 at floor 3) load before frame 0: the readiness report's
/// `loading` with the roots loaded so far (STRM-001, ADR 0045 point 5).
pub fn roots_title(done: usize, total: usize) -> String {
    format!("planet | loading - roots {done}/{total} (the tiles of the render floor)")
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

/// The input state the title ends with, so that a camera that does not answer the keys can be told apart from keys that do not arrive:
/// the keys the viewer holds down (`W` `A` `S` `D` `Q` `E`, arrows as `<` `>` `^` `v`, `+` for the boost; `-` when none), whether the
/// pointer is captured for looking around and whether the globe overview is shown.
pub fn input_text(held: &Held, captured: bool, globe: bool) -> String {
    let keys: String = [
        (held.forward, 'W'),
        (held.left, 'A'),
        (held.back, 'S'),
        (held.right, 'D'),
        (held.down, 'Q'),
        (held.up, 'E'),
        (held.look_left, '<'),
        (held.look_right, '>'),
        (held.look_up, '^'),
        (held.look_down, 'v'),
        (held.boost, '+'),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, c)| *c)
    .collect();
    format!(
        "keys {} | pointer {}{}",
        if keys.is_empty() { "-".to_string() } else { keys },
        if captured { "captured" } else { "free" },
        if globe { " | overview" } else { "" }
    )
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
    /// The exposure the last sampled lit frame was shown at; `None` on the debug path (VIEW-007).
    pub exposure: Option<ExposureReading>,
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
    let exposure = i.exposure.map_or(String::new(), |e| format!(" | {}", exposure_text(&e)));
    let alt = if i.altitude_m >= 10_000.0 { format!("{:.0} km", i.altitude_m / 1000.0) } else { format!("{:.1} m", i.altitude_m) };
    format!(
        "planet | {:.0} fps | cpu {:.2} ms | {gpu} | vsync {} ({}) | {}{exposure} | {camera} | alt {alt} | {}",
        i.fps,
        i.cpu_ms,
        if i.vsync { "on" } else { "off" },
        i.present_mode,
        i.view,
        i.readiness.summary()
    )
}

// -------------------------------------------------------------------------------------------------------------------------
// The lit path in the window (VIEW-007)

use crate::look::file::LookParams;
use crate::look::{LitPlan, SOLAR_ILLUMINANCE_LUX};
use crate::terrain::{TerrainPlan, TerrainView};

/// The pre-exposure EV100 the viewer writes the scene target at on Earth (ADR 0048 point 3: chosen by the view, never read back):
/// f16 then holds about +-12 stops around it, so a sunlit surface (EV 14 to 15) and a moonless night's scene alike stay inside it.
pub const VIEWER_PRE_EV100: f32 = 12.0;

/// The pre-exposure of a body: the Earth's, moved by the stops its sun differs by (the scene unit is the sun's illuminance, so a
/// dimmer sun lowers every scene value by the same factor).
pub fn viewer_pre_ev100(look: &LookParams) -> f32 {
    let lux = look.units.solar_illuminance_lux;
    if !(lux.is_finite() && lux > 0.0) {
        return VIEWER_PRE_EV100;
    }
    VIEWER_PRE_EV100 + libm::log2(lux / SOLAR_ILLUMINANCE_LUX) as f32
}

/// The elevation of the editing sun above the horizon of the viewer's place, and its azimuth off the direction behind the viewer (to the
/// right), degrees (VIEW-042).
pub const EDITING_SUN_ELEVATION_DEG: f64 = 35.0;
pub const EDITING_SUN_AZIMUTH_DEG: f64 = 40.0;

/// The unit vector towards the editing sun in planet-fixed axes (VIEW-042): a sun fixed in camera space over the viewer's shoulder, so the
/// view is lit from behind and a little to the right, high enough to give slopes a shade, and no part of what the camera sees faces away
/// from it. At the viewer's place: up is the direction from the planet's centre; the heading is the view direction along the ground (the
/// camera's up when the view is straight up or down); the sun is `EDITING_SUN_ELEVATION_DEG` above the horizon, `EDITING_SUN_AZIMUTH_DEG`
/// to the right of the direction behind the viewer. A pure function of the camera.
pub fn editing_sun_dir(camera: &crate::camera::Camera) -> [f32; 3] {
    let up = camera.position.0.normalized();
    let flat = |v: Vec3| v - up * v.dot(up);
    let mut heading = flat(camera.forward);
    if heading.length() < 1e-6 {
        heading = flat(camera.up);
    }
    if heading.length() < 1e-6 || !heading.length().is_finite() {
        // A degenerate camera (at the centre, or up parallel to everything): any horizontal direction.
        heading = flat(if up.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) });
    }
    let heading = heading.normalized();
    let right = heading.cross(up).normalized();
    let back = heading * -1.0;
    let (el, az) = (EDITING_SUN_ELEVATION_DEG.to_radians(), EDITING_SUN_AZIMUTH_DEG.to_radians());
    let horizontal = back * libm::cos(az) + right * libm::sin(az);
    let sun = horizontal * libm::cos(el) + up * libm::sin(el);
    let s = sun.normalized();
    [s.x as f32, s.y as f32, s.z as f32]
}

/// The lit plan of one viewer frame: the body's look (sun, tone curve, units, histogram, albedo), the adaptive exposure with `dt_s`
/// seconds of the frame clock and, when `atmosphere` is set, the atmosphere of a body of `radius_m`. A pure function of its
/// arguments, so the exposure a run reaches is decided by the plans alone (CON-04). `reset` snaps the exposure (see
/// [`ViewerState::take_exposure_reset`]).
pub fn viewer_lit_plan(look: &LookParams, radius_m: f64, dt_s: f32, reset: bool, atmosphere: bool) -> Result<LitPlan, String> {
    let mut plan = LitPlan::adaptive_with(look, viewer_pre_ev100(look), dt_s);
    if atmosphere {
        plan = plan.with_atmosphere(look, radius_m)?;
    }
    plan.reset_exposure = reset;
    Ok(plan)
}

/// The plan of the throwaway lit frames that warm the lit and atmosphere passes before the first measured frame (REND-017): the
/// first camera of the flight, no node, the same lit plan the first frame will carry.
pub fn lit_warm_plan(camera: crate::camera::Camera, size: (u32, u32), cells: u32, lit: LitPlan) -> TerrainPlan {
    TerrainPlan {
        size,
        view: TerrainView::Relief,
        camera,
        view_projection: camera.view_projection(),
        clear_color: [0, 0, 0, 255],
        height_range: [0.0, 1.0],
        cells,
        shore_ramp_per_m: 0.0,
        nodes: Vec::new(),
        lit: Some(lit),
        water: None,
    }
}

/// The exposure of a lit frame as the GPU holds it: the EV100 shown, and the EV100 the histogram asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureReading {
    pub ev100: f32,
    pub target_ev100: f32,
}

/// Stops within which the exposure counts as adapted to its target.
pub const ADAPTED_WITHIN_STOPS: f32 = 0.05;

/// The exposure as the title shows it: `EV 12.3 (adapted)`, or `EV 12.3 (adapting to 14.0)` while it moves.
pub fn exposure_text(e: &ExposureReading) -> String {
    if (e.ev100 - e.target_ev100).abs() <= ADAPTED_WITHIN_STOPS {
        format!("EV {:.1} (adapted)", e.ev100)
    } else {
        format!("EV {:.1} (adapting to {:.1})", e.ev100, e.target_ev100)
    }
}

/// What the title names where it names the debug view: the lit path (with the atmosphere where it runs) or the debug view.
pub fn mode_label(lit: bool, atmosphere: bool, debug_view: &str) -> String {
    match (lit, atmosphere) {
        (true, true) => "lit + atmosphere".to_string(),
        (true, false) => "lit".to_string(),
        (false, _) => debug_view.to_string(),
    }
}

/// What a smoke run counted, frame by frame (no clock): the verdict reads only these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmokeCounts {
    /// Frames drawn with a lit plan, and of them those whose plan carried the atmosphere.
    pub lit_frames: u64,
    pub atmosphere_frames: u64,
    /// Frames drawn on the debug path.
    pub debug_frames: u64,
    /// Frames presented to the window, by path.
    pub lit_presented: u64,
    pub debug_presented: u64,
    /// Exposure readings taken, and whether the renderer itself reports having run the lit and atmosphere passes.
    pub exposure_readings: u64,
    pub renderer_ran_lit: bool,
    pub renderer_ran_atmosphere: bool,
    /// Lit frames whose plan had water nodes, and whether the renderer reports having run the water pass (a world and a flight that
    /// never see water leave both at zero: the verdict does not require them, the summary reports them).
    pub water_frames: u64,
    pub renderer_ran_water: bool,
    /// Lit frames drawn with a debug overlay, and with the mesh overlay among them (VIEW-014).
    pub overlay_frames: u64,
    pub mesh_overlay_frames: u64,
    /// Frames planned with the coarse-LOD inspection view (VIEW-014).
    pub coarse_lod_frames: u64,
    /// Times the coarse view was refused because the plan could not be made with it (a steep neighbourhood leaves no morph room) and the
    /// frame fell back to the base parameters.
    pub coarse_lod_refusals: u64,
    /// Lit frames drawn with the editing light (VIEW-042); the smoke run switches it on for a few frames and the verdict wants them.
    pub editing_light_frames: u64,
    /// Lit frames drawn with the stale-tile overlay (VIEW-044); the smoke run's fifth F3 press shows it and the verdict wants them.
    pub stale_overlay_frames: u64,
}

/// Frames a smoke run must have drawn lit.
pub const SMOKE_MIN_LIT_FRAMES: u64 = 16;

/// The smoke run's verdict (VIEW-005, VIEW-007): at least [`SMOKE_MIN_LIT_FRAMES`] lit frames drawn and one presented, the
/// atmosphere on every lit frame when it was wanted, the debug path drawn and presented too (the key switch worked), an exposure
/// reading taken, and the renderer's own counters agreeing that the lit passes ran. An error names the count that fell short.
pub fn smoke_verdict(c: &SmokeCounts, atmosphere_wanted: bool) -> Result<(), String> {
    let short = |what: &str, got: String, want: &str| Err(format!("smoke: {what}: {got}, wanted {want}"));
    if c.lit_frames < SMOKE_MIN_LIT_FRAMES {
        return short("lit frames drawn", c.lit_frames.to_string(), &format!("at least {SMOKE_MIN_LIT_FRAMES}"));
    }
    if c.lit_presented == 0 {
        return short("lit frames presented", "0".into(), "at least 1");
    }
    if atmosphere_wanted && c.atmosphere_frames != c.lit_frames {
        return short("lit frames with the atmosphere", c.atmosphere_frames.to_string(), &format!("all {}", c.lit_frames));
    }
    if !atmosphere_wanted && c.atmosphere_frames != 0 {
        return short("lit frames with the atmosphere", c.atmosphere_frames.to_string(), "none");
    }
    if c.debug_frames == 0 || c.debug_presented == 0 {
        return short(
            "debug-path frames drawn and presented",
            format!("{} and {}", c.debug_frames, c.debug_presented),
            "at least 1 each (the L key must switch the path)",
        );
    }
    if c.exposure_readings == 0 {
        return short("exposure readings", "0".into(), "at least 1");
    }
    if c.stale_overlay_frames == 0 {
        return short("lit frames with the stale-tile overlay", "0".into(), "at least 1 (the fourth F3 press must show it)");
    }
    if c.editing_light_frames == 0 {
        return short("lit frames with the editing light", "0".into(), "at least 1 (the H key must switch it on)");
    }
    if !c.renderer_ran_lit || (atmosphere_wanted && !c.renderer_ran_atmosphere) {
        return Err(format!(
            "smoke: the renderer reports lit passes run: {}, atmosphere passes run: {} (wanted lit, and the atmosphere when asked)",
            c.renderer_ran_lit, c.renderer_ran_atmosphere
        ));
    }
    Ok(())
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

    // spec: VIEW-010
    #[test]
    fn the_mouse_turns_the_free_camera_by_counts_times_sensitivity_and_not_the_flight() {
        let cams = script(600);
        let p = ControllerParams::default();
        let h = |_: PlanetFixed| 5000.0;
        let g = |_: PlanetFixed| 6.0e6;
        let mut v = ViewerState::new(0, 9, 7, true);
        let before = v.camera(&cams, 1000, 0.016, &p, &h, &g);
        // In the scripted flight a click does not capture, and motion changes nothing.
        v.mouse.click(v.is_free());
        v.mouse.motion(300.0, 0.0);
        assert!(!v.mouse.captured());
        v.command(Command::ToggleCamera, 1000, &before);
        v.mouse.click(v.is_free());
        assert!(v.mouse.captured());
        let b = v.camera(&cams, 1000, 0.016, &p, &h, &g);
        v.mouse.motion(100.0, -50.0);
        let turned = v.camera(&cams, 1016, 0.016, &p, &h, &g);
        // The camera turned right by 100 counts and up by 50, whatever the frame time.
        let (pb, pt) = (FreePose::from_camera(&b), FreePose::from_camera(&turned));
        let yaw = libm::acos(pb.heading.dot(pt.heading).clamp(-1.0, 1.0));
        assert!((yaw - 100.0 * crate::mouselook::SENSITIVITY_RAD_PER_COUNT).abs() < 1e-9, "yaw {yaw}");
        assert!((pt.pitch - pb.pitch - 50.0 * crate::mouselook::SENSITIVITY_RAD_PER_COUNT).abs() < 1e-9, "pitch {}", pt.pitch - pb.pitch);
        assert!(pb.heading.cross(pt.heading).dot(pb.up()).is_sign_negative(), "a turn to the right is clockwise seen from above");
        // The same counts over a frame of 1 ms and of 1 s turn by the same angle.
        let turn_for = |dt: f64| {
            let mut w = v;
            w.mouse.motion(40.0, 0.0);
            let before = FreePose::from_camera(&w.camera(&cams, 2000, 0.0, &p, &h, &g));
            w.mouse.motion(40.0, 0.0);
            let after = FreePose::from_camera(&w.camera(&cams, 2000, dt, &p, &h, &g));
            libm::acos(before.heading.dot(after.heading).clamp(-1.0, 1.0))
        };
        assert!((turn_for(0.001) - turn_for(1.0)).abs() < 1e-9);
        // Leaving the free camera (C) releases the mouse, and a restart does too.
        let free_camera = v.camera(&cams, 3000, 0.016, &p, &h, &g);
        v.command(Command::ToggleCamera, 3000, &free_camera);
        assert!(!v.mouse.captured() && !v.is_free());
        v.mouse.click(true);
        v.command(Command::Restart, 4000, &free_camera);
        assert!(!v.mouse.captured());
    }

    // spec: VIEW-010
    #[test]
    fn focus_loss_releases_the_mouse_and_lets_go_of_every_held_key() {
        let mut v = ViewerState::new(0, 9, 7, true);
        v.held = Held { forward: true, look_left: true, boost: true, ..Held::default() };
        v.mouse.click(true);
        v.mouse.take_grab_change();
        v.focus_lost();
        assert_eq!(v.held, Held::default(), "no key stays down after Alt-Tab");
        assert!(!v.mouse.captured());
        assert_eq!(v.mouse.take_grab_change(), Some(crate::mouselook::GrabChange::Release));
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

    // spec: STRM-001, VIEW-003
    #[test]
    fn the_title_shows_the_roots_loaded_so_far_while_they_load() {
        assert_eq!(roots_title(0, 384), "planet | loading - roots 0/384 (the tiles of the render floor)");
        assert!(roots_title(120, 384).contains("roots 120/384") && roots_title(384, 384).contains("roots 384/384"));
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
            exposure: None,
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

    fn reading(ev: f32, target: f32) -> ExposureReading {
        ExposureReading { ev100: ev, target_ev100: target }
    }

    // spec: VIEW-007
    #[test]
    fn the_light_key_switches_the_path_and_the_debug_views_belong_to_the_debug_path() {
        let c = cam(0.0);
        let mut v = ViewerState::new(0, 9, 7, true).with_lit(true);
        assert!(v.lit);
        // V does nothing on the lit path.
        v.command(Command::NextView, 10, &c);
        assert_eq!(v.view_index, 7);
        assert!(!v.command(Command::ToggleLit, 20, &c) && !v.lit, "L leaves the lit path");
        v.command(Command::NextView, 30, &c);
        assert_eq!(v.view_index, 8, "V cycles the debug views on the debug path");
        v.command(Command::ToggleLit, 40, &c);
        assert!(v.lit);
        v.command(Command::NextView, 50, &c);
        assert_eq!(v.view_index, 8, "and again does nothing on the lit path");
        // The debug path is where tests and the editor's older behaviour start.
        assert!(!ViewerState::new(0, 9, 0, true).lit);
    }

    // spec: VIEW-007
    #[test]
    fn the_exposure_snaps_on_the_first_lit_frame_a_return_and_a_restart_and_adapts_between() {
        let c = cam(0.0);
        let mut v = ViewerState::new(0, 9, 0, true).with_lit(true);
        assert!(v.take_exposure_reset(), "the first lit frame snaps");
        assert!(!v.take_exposure_reset(), "once");
        v.command(Command::ToggleLit, 10, &c);
        assert!(!v.take_exposure_reset(), "leaving the lit path asks for nothing");
        v.command(Command::ToggleLit, 20, &c);
        assert!(v.take_exposure_reset(), "returning to it snaps");
        v.command(Command::TogglePause, 30, &c);
        v.command(Command::ToggleCamera, 40, &c);
        assert!(!v.take_exposure_reset(), "pause and the free camera do not");
        v.command(Command::Restart, 50, &c);
        assert!(v.take_exposure_reset(), "a restart is a cut");
    }

    // spec: VIEW-007
    #[test]
    fn the_viewer_lit_plan_is_the_looks_sun_with_adaptive_exposure_and_the_atmosphere_and_is_deterministic() {
        let look = LookParams::default();
        let plan = viewer_lit_plan(&look, 6_371_000.0, 0.016, false, true).unwrap();
        assert_eq!(plan, viewer_lit_plan(&look, 6_371_000.0, 0.016, false, true).unwrap(), "a pure function of its arguments");
        assert_eq!(plan.to_text(), viewer_lit_plan(&look, 6_371_000.0, 0.016, false, true).unwrap().to_text());
        assert!(plan.exposure.is_adaptive() && plan.dt_s == 0.016 && !plan.reset_exposure);
        assert_eq!(plan.exposure.pre_ev100(), VIEWER_PRE_EV100);
        assert_eq!(plan.sun_dir, look.sun.direction.map(|v| v as f32), "the sun is the look's");
        assert!(plan.atmosphere.is_some());
        plan.validate().unwrap();
        // The look's sun and the frame clock's dt reach the plan.
        let mut moon = LookParams::default();
        moon.sun.direction = [0.0, 0.0, 1.0];
        moon.units.solar_illuminance_lux = SOLAR_ILLUMINANCE_LUX / 4.0;
        let p = viewer_lit_plan(&moon, 6_371_000.0, 0.25, true, false).unwrap();
        assert_eq!(p.sun_dir, [0.0, 0.0, 1.0]);
        assert!(p.reset_exposure && p.atmosphere.is_none() && p.dt_s == 0.25);
        assert_eq!(p.exposure.pre_ev100(), VIEWER_PRE_EV100 - 2.0, "a sun a quarter as bright is two stops lower");
        // A body whose radius the atmosphere cannot sit on is an error naming it, not a panic.
        assert!(viewer_lit_plan(&look, 500.0, 0.016, false, true).is_err());
    }

    // spec: VIEW-007
    #[test]
    fn the_warm_plan_is_an_empty_valid_lit_plan_at_the_first_camera() {
        let lit = viewer_lit_plan(&LookParams::default(), 6_371_000.0, 0.0, true, true).unwrap();
        let plan = lit_warm_plan(cam(0.0), (1280, 720), 32, lit);
        assert!(plan.nodes.is_empty() && plan.lit == Some(lit) && plan.camera == cam(0.0));
        assert_eq!(plan.view_projection, cam(0.0).view_projection());
    }

    // spec: VIEW-003
    #[test]
    fn the_title_shows_the_path_and_the_exposure_on_the_lit_path_only() {
        let mut i = TitleInfo {
            fps: 60.0,
            cpu_ms: 1.0,
            gpu_ms: None,
            vsync: true,
            present_mode: "Fifo".into(),
            view: mode_label(true, true, "relief"),
            free: false,
            paused: false,
            flight_s: 1.0,
            altitude_m: 100.0,
            exposure: Some(reading(12.34, 12.34)),
            readiness: Readiness::default(),
        };
        let t = title(&i);
        assert!(t.contains("| lit + atmosphere | EV 12.3 (adapted) | flight 1.0 s |"), "{t}");
        i.exposure = Some(reading(12.0, 14.04));
        assert!(title(&i).contains("EV 12.0 (adapting to 14.0)"), "{}", title(&i));
        i.exposure = None;
        i.view = mode_label(false, true, "relief");
        let t = title(&i);
        assert!(t.contains("| relief | flight") && !t.contains("EV"), "{t}");
        assert_eq!(mode_label(true, false, "relief"), "lit");
        assert_eq!(exposure_text(&reading(5.0, 5.04)), "EV 5.0 (adapted)");
        assert_eq!(exposure_text(&reading(5.0, 5.2)), "EV 5.0 (adapting to 5.2)");
    }

    fn good_counts() -> SmokeCounts {
        SmokeCounts {
            lit_frames: 40,
            atmosphere_frames: 40,
            debug_frames: 6,
            lit_presented: 40,
            debug_presented: 6,
            exposure_readings: 3,
            renderer_ran_lit: true,
            renderer_ran_atmosphere: true,
            water_frames: 0,
            renderer_ran_water: false,
            overlay_frames: 0,
            mesh_overlay_frames: 0,
            coarse_lod_frames: 0,
            coarse_lod_refusals: 0,
            editing_light_frames: 4,
            stale_overlay_frames: 3,
        }
    }

    // spec: VIEW-014, VIEW-030
    #[test]
    fn f3_cycles_the_overlay_off_tiles_mesh_regions_off_and_f4_toggles_the_coarse_lod_both_off_at_the_start() {
        let c = camera_for_tests();
        let mut v = ViewerState::new(0, 3, 0, true).with_lit(true);
        assert_eq!((v.overlay, v.coarse_lod), (Overlay::None, false), "off at the start");
        assert!(!v.command(Command::CycleOverlay, 10, &c));
        assert_eq!(v.overlay, Overlay::Tiles);
        v.command(Command::ToggleLit, 20, &c);
        assert_eq!(v.overlay, Overlay::Tiles, "leaving the lit path keeps the choice, the debug path draws no overlay");
        v.command(Command::CycleOverlay, 30, &c);
        assert_eq!(v.overlay, Overlay::Mesh);
        v.command(Command::CycleOverlay, 40, &c);
        assert_eq!(v.overlay, Overlay::Regions, "the third press shows the regions (VIEW-030)");
        v.command(Command::CycleOverlay, 44, &c);
        assert_eq!(v.overlay, Overlay::Stale, "the fourth press shows the state of the tiles (VIEW-044)");
        v.command(Command::CycleOverlay, 45, &c);
        assert_eq!(v.overlay, Overlay::None, "the fifth press is back to off");
        assert!(!v.command(Command::ToggleCoarseLod, 50, &c) && v.coarse_lod);
        assert_eq!(v.overlay, Overlay::None, "the two switches are independent");
        v.command(Command::ToggleCoarseLod, 60, &c);
        assert!(!v.coarse_lod);
        for word in ["F3", "tiles", "mesh", "regions", "F4", "coarse LOD"] {
            assert!(KEYS_HELP.contains(word), "the help names {word}: {KEYS_HELP}");
        }
    }

    // spec: VIEW-024
    #[test]
    fn f9_asks_once_for_the_repro_files_and_changes_nothing_in_the_viewer() {
        let c = camera_for_tests();
        let mut v = ViewerState::new(0, 3, 0, true).with_lit(true);
        let (lit, mode, overlay, paused) = (v.lit, v.mode, v.overlay, v.user_paused);
        assert!(!v.take_repro_request(), "nobody asked yet");
        assert!(!v.command(Command::SaveRepro, 10, &c), "F9 does not quit");
        assert!(v.take_repro_request(), "the window takes the request");
        assert!(!v.take_repro_request(), "once");
        assert_eq!((v.lit, v.mode, v.overlay, v.user_paused), (lit, mode, overlay, paused), "the viewer is as it was");
        for word in ["F9", "camera path", "repro bundle"] {
            assert!(KEYS_HELP.contains(word), "the help names {word}: {KEYS_HELP}");
        }
    }

    // spec: VIEW-013
    #[test]
    fn g_toggles_the_globe_overview_which_is_the_camera_and_a_click_flight_lands_in_the_free_camera() {
        use crate::globe::{Click, Marker, Viewport};
        use crate::pick::PickLimits;
        const R: f64 = 6_371_000.0;
        let c = camera_for_tests();
        let cameras = [c];
        let params = ControllerParams::for_radius(R);
        let (height, ground) = (|_: planet_core::PlanetFixed| 1.0e6, |_: planet_core::PlanetFixed| R);
        let mut v = ViewerState::new(0, 3, 0, true).with_lit(true).with_radius_m(R);
        assert!(v.globe.is_none(), "off at the start");
        assert!(!v.command(Command::ToggleGlobe, 10, &c));
        let session = v.globe.expect("G shows the overview");
        assert!((session.view.altitude_m - 1.5 * R).abs() < 1e-6);
        // The overview is the camera: 1.5 radii above the sphere, looking at the centre.
        let cam = v.camera(&cameras, 20, 0.0, &params, &height, &ground);
        assert!((cam.position.0.length() - 2.5 * R).abs() < 1e-3);
        assert!(!v.is_free(), "the overview does not change the camera mode");
        // G again: back to the flight.
        v.command(Command::ToggleGlobe, 30, &c);
        assert!(v.globe.is_none());
        // A click on a marker starts the flight; when it ends the free camera takes over over the marker.
        v.command(Command::ToggleGlobe, 40, &c);
        let marker = Marker { id: 9, dir: v.globe.unwrap().view.center, radius_m: 2_000.0 };
        let viewport = Viewport { width_px: 1600.0, height_px: 900.0 };
        let cam = v.camera(&cameras, 50, 0.0, &params, &height, &ground);
        let at = crate::globe::visible_markers(&cam, &[marker], R, viewport)[0].pixel;
        let limits = PickLimits::new(R, 9_000.0);
        let flat = |_: planet_core::Vec3| 0.0;
        let click = v.globe.as_mut().unwrap().click(at, &cam, viewport, &[marker], &limits, &flat);
        assert_eq!(click, Some(Click::Marker(9)));
        let mut now = 60;
        let mut landed = None;
        for _ in 0..1_000 {
            let cam = v.camera(&cameras, now, 0.05, &params, &height, &ground);
            now += 50;
            if v.globe.is_none() {
                landed = Some(cam);
                break;
            }
        }
        let cam = landed.expect("the flight ends");
        assert!(v.is_free(), "the free camera took over");
        assert!(v.clock.is_paused(), "the flight clock waits while the free camera flies");
        assert!((cam.position.0.length() - (R + 8_000.0)).abs() < 1.0, "{}", cam.position.0.length() - R);
        assert!(cam.position.0.normalized().dot(marker.dir) > 1.0 - 1e-9, "over the marker");
        // G in the free camera shows the overview over the free camera's point and leaves the free mode alone.
        v.command(Command::ToggleGlobe, 90_000, &c);
        assert!(v.globe.is_some() && v.is_free());
        assert!(KEYS_HELP.contains("globe overview"), "{KEYS_HELP}");
    }

    // spec: VIEW-034
    #[test]
    fn the_title_tells_the_held_keys_the_pointer_and_the_overview() {
        assert_eq!(input_text(&Held::default(), false, false), "keys - | pointer free");
        let held = Held { forward: true, left: true, boost: true, look_up: true, ..Held::default() };
        assert_eq!(input_text(&held, true, true), "keys WA^+ | pointer captured | overview");
    }

    // spec: VIEW-034
    #[test]
    fn the_movement_keys_move_the_free_camera_after_the_overview_was_shown_and_left_or_flown_into() {
        use crate::globe::{Marker, Viewport};
        use crate::pick::PickLimits;
        const R: f64 = 6_371_000.0;
        let c = camera_for_tests();
        let cameras = [c];
        let params = ControllerParams::for_radius(R);
        let (height, ground) = (|p: planet_core::PlanetFixed| p.0.length() - R, |_: planet_core::PlanetFixed| R);
        let mut v = ViewerState::new(0, 3, 0, true).with_lit(true).with_radius_m(R);
        let step = |v: &mut ViewerState, now: u64| v.camera(&cameras, now, 0.1, &params, &height, &ground);
        let moved = |v: &mut ViewerState, now: u64| {
            v.held.forward = true;
            let before = step(v, now).position.0;
            let after = step(v, now + 100).position.0;
            v.held.forward = false;
            (after - before).length()
        };
        v.command(Command::ToggleCamera, 5, &c);
        assert!(moved(&mut v, 10) > 0.0, "W moves the free camera");
        // G and G again: the free camera is as it was and W moves it.
        v.command(Command::ToggleGlobe, 200, &c);
        v.command(Command::ToggleGlobe, 210, &c);
        assert!(v.globe.is_none() && v.is_free());
        assert!(moved(&mut v, 300) > 0.0, "W moves the free camera after G and G");
        // A key held through the overview is still held after it.
        v.held.forward = true;
        v.command(Command::ToggleGlobe, 400, &c);
        step(&mut v, 410);
        v.command(Command::ToggleGlobe, 420, &c);
        let (a, b) = (step(&mut v, 430).position.0, step(&mut v, 530).position.0);
        assert!((b - a).length() > 0.0, "a key held across the overview keeps moving the camera");
        v.held.forward = false;
        // The overview, a click flight on a marker, and the landing: W moves the camera that landed.
        v.command(Command::ToggleGlobe, 600, &c);
        let marker = Marker { id: 9, dir: v.globe.unwrap().view.center, radius_m: 2_000.0 };
        let viewport = Viewport { width_px: 1600.0, height_px: 900.0 };
        let cam = step(&mut v, 610);
        let at = crate::globe::visible_markers(&cam, &[marker], R, viewport)[0].pixel;
        let flat = |_: planet_core::Vec3| 0.0;
        v.globe.as_mut().unwrap().click(at, &cam, viewport, &[marker], &PickLimits::new(R, 9_000.0), &flat).unwrap();
        let mut now = 700;
        while v.globe.is_some() && now < 100_000 {
            v.camera(&cameras, now, 0.05, &params, &height, &ground);
            now += 50;
        }
        assert!(v.globe.is_none() && v.is_free(), "the flight landed");
        assert!(moved(&mut v, now + 100) > 0.0, "W moves the camera that landed");
    }

    // spec: VIEW-013
    #[test]
    fn g_during_a_click_flight_abandons_it_and_leaves_the_camera_mode_as_it_was() {
        use crate::globe::{Marker, Viewport};
        use crate::pick::PickLimits;
        const R: f64 = 6_371_000.0;
        let c = camera_for_tests();
        let cameras = [c];
        let params = ControllerParams::for_radius(R);
        let (height, ground) = (|_: planet_core::PlanetFixed| 1.0e6, |_: planet_core::PlanetFixed| R);
        let mut v = ViewerState::new(0, 3, 0, true).with_lit(true).with_radius_m(R);
        v.command(Command::ToggleGlobe, 10, &c);
        let marker = Marker { id: 1, dir: v.globe.unwrap().view.center, radius_m: 2_000.0 };
        let viewport = Viewport { width_px: 1600.0, height_px: 900.0 };
        let cam = v.camera(&cameras, 20, 0.0, &params, &height, &ground);
        let at = crate::globe::visible_markers(&cam, &[marker], R, viewport)[0].pixel;
        let flat = |_: planet_core::Vec3| 0.0;
        v.globe.as_mut().unwrap().click(at, &cam, viewport, &[marker], &PickLimits::new(R, 9_000.0), &flat).unwrap();
        v.camera(&cameras, 30, 0.2, &params, &height, &ground);
        assert!(v.globe.unwrap().flying());
        // G now: the overview is gone, the flight is dropped, the camera mode is the flight's again, the clock was never paused.
        v.command(Command::ToggleGlobe, 40, &c);
        assert!(v.globe.is_none() && !v.is_free() && !v.clock.is_paused());
        v.camera(&cameras, 50, 0.2, &params, &height, &ground);
        assert!(v.globe.is_none() && !v.is_free(), "no landing happens after the abandoned flight");
    }

    // spec: VIEW-014
    #[test]
    fn a_refused_coarse_plan_falls_back_to_the_base_parameters_once_and_other_failures_are_not_hidden() {
        let (mut coarse, mut refusals) = (true, 0u64);
        let mut calls = Vec::new();
        // The coarse attempt fails, the base one works: the result is the base one, the switch is off, the refusal counted.
        let r: Result<&str, String> = with_coarse_fallback(&mut coarse, &mut refusals, |c| {
            calls.push(c);
            if c {
                Err("morph interval is empty".to_string())
            } else {
                Ok("base plan")
            }
        });
        assert_eq!(r, Ok("base plan"));
        assert_eq!((coarse, refusals), (false, 1));
        assert_eq!(calls, [true, false]);
        // Coarse off: an error is returned at once, with no second attempt and no refusal.
        calls.clear();
        let r: Result<(), String> = with_coarse_fallback(&mut coarse, &mut refusals, |c| {
            calls.push(c);
            Err("a real failure".to_string())
        });
        assert_eq!(r, Err("a real failure".to_string()));
        assert_eq!((coarse, refusals, calls.as_slice()), (false, 1, &[false][..]));
        // Coarse on and both attempts fail: the second error comes back, the refusal is counted once.
        let (mut coarse, mut refusals) = (true, 0u64);
        let mut n = 0;
        let r: Result<(), String> = with_coarse_fallback(&mut coarse, &mut refusals, |_| {
            n += 1;
            Err(format!("failure {n}"))
        });
        assert_eq!(r, Err("failure 2".to_string()));
        assert_eq!((coarse, refusals), (false, 1));
        // Coarse on and the attempt works: nothing changes.
        let (mut coarse, mut refusals) = (true, 0u64);
        let r: Result<u32, String> = with_coarse_fallback(&mut coarse, &mut refusals, |_| Ok(7));
        assert_eq!((r, coarse, refusals), (Ok(7), true, 0));
    }

    // spec: VIEW-014
    #[test]
    fn the_coarse_lod_multiplies_only_the_error_bound_and_the_corner_label_says_what_is_active() {
        let base = LodParams::earth_1080p();
        let same = effective_lod(&base, false);
        assert_eq!(same, base, "off: the parameters are the base ones");
        assert_eq!(crate::lod::COARSE_LOD_FACTOR, 4.0);
        // The production 1080p parameters (error bound 6 px, 32 cells) cannot take four times the bound (viewport / tau would be 45,
        // under 64): the factor is the largest the morph intervals allow (and the plan accepts at the large levels, about 1.7), between 1.5 and 4, and the result is valid.
        let coarse = effective_lod(&base, true);
        assert!(coarse.tau_px > 1.5 * base.tau_px && coarse.tau_px < 4.0 * base.tau_px, "{}", coarse.tau_px);
        assert_eq!(LodParams { tau_px: base.tau_px, ..coarse.clone() }, base, "nothing else changes");
        coarse.validate().expect("the coarse parameters are valid at 1080p");
        // It is the largest the plan accepts wherever it accepts the base ones, with the margin kept: a little more is invalid by the node
        // radius alone or leaves a level the base meets (with the neighbourhood's height extent added) over the limit.
        assert!((base.coarse_factor() - 1.737).abs() < 0.02, "the production 1080p coarse factor is about 1.7: {}", base.coarse_factor());
        let bigger = LodParams { tau_px: coarse.tau_px * 1.05, ..base.clone() };
        let too_much = bigger.validate().is_err()
            || base.worst_case_levels().into_iter().any(|l| {
                let (bs, be) = base.morph_interval_worst(l);
                let (cs, ce) = bigger.morph_interval_worst(l);
                be - bs >= 0.05 * be && ce - cs < 0.05 * ce
            });
        assert!(too_much, "it is the largest that keeps the plan from refusing it");
        assert!((base.coarse_factor() - coarse.tau_px / base.tau_px).abs() < 1e-12);
        // Parameters with room (2.5 px, the golden tolerance) get exactly four times.
        let fine = LodParams { tau_px: 2.5, ..base.clone() };
        assert_eq!(effective_lod(&fine, true).tau_px, 10.0);
        assert_eq!(fine.coarse_factor(), 4.0);
        assert_eq!(overlay_status(Overlay::None, 1.0), "Overlay: off, LOD x1");
        assert_eq!(overlay_status(Overlay::Tiles, 1.0), "Overlay: Tiles, LOD x1");
        assert_eq!(overlay_status(Overlay::Mesh, 4.0), "Overlay: Mesh, LOD x4");
        assert_eq!(overlay_status(Overlay::Tiles, 2.63), "Overlay: Tiles, LOD x2.6");
    }

    fn camera_for_tests() -> Camera {
        Camera::looking_at_centre(
            planet_core::PlanetFixed(planet_core::Vec3::new(0.0, 0.0, 2.0e7)),
            planet_core::Vec3::new(0.0, 1.0, 0.0),
            1.0,
            1.6,
            1.0,
        )
    }

    // spec: VIEW-005
    #[test]
    fn the_smoke_verdict_needs_lit_frames_with_the_atmosphere_a_presented_frame_and_both_paths() {
        smoke_verdict(&good_counts(), true).unwrap();
        let bad = |f: &dyn Fn(&mut SmokeCounts), want: bool, needle: &str| {
            let mut c = good_counts();
            f(&mut c);
            let e = smoke_verdict(&c, want).unwrap_err();
            assert!(e.contains(needle), "{e}");
        };
        bad(&|c| c.lit_frames = SMOKE_MIN_LIT_FRAMES - 1, true, "lit frames drawn");
        bad(&|c| c.lit_presented = 0, true, "lit frames presented");
        bad(&|c| c.atmosphere_frames = 39, true, "with the atmosphere");
        bad(&|c| c.debug_presented = 0, true, "debug-path");
        bad(&|c| c.debug_frames = 0, true, "debug-path");
        bad(&|c| c.exposure_readings = 0, true, "exposure readings");
        bad(&|c| c.renderer_ran_lit = false, true, "renderer reports");
        bad(&|c| c.renderer_ran_atmosphere = false, true, "renderer reports");
        // Without the atmosphere asked for, none may be drawn and the renderer need not report it.
        let mut c = good_counts();
        c.atmosphere_frames = 0;
        c.renderer_ran_atmosphere = false;
        smoke_verdict(&c, false).unwrap();
        assert!(smoke_verdict(&good_counts(), false).is_err());
    }

    // spec: VIEW-044
    #[test]
    fn the_states_are_counted_titled_and_listed_in_the_legend_with_the_colours_of_the_shader() {
        use crate::terrain::{TerrainDraw, TileState};
        let node = |state: TileState| TerrainDraw {
            tile: planet_core::TileId::new(planet_core::Face(0), 3, 1, 1).unwrap(),
            level: 3,
            origin: PlanetFixed(Vec3::new(1.0, 0.0, 0.0)),
            camera_relative_origin: [0.0; 3],
            morph: 0.0,
            morph_start_m: 0.0,
            morph_end_m: 0.0,
            color: [0; 4],
            mesh_key: 0,
            preview: false,
            transient_only: false,
            capped: false,
            stitch: 0,
            fine: 0,
            quadrants: 0b1111,
            state,
        };
        let nodes: Vec<_> =
            [TileState::Exact, TileState::Exact, TileState::Stale, TileState::StandIn, TileState::Previewed, TileState::Exact]
                .map(node)
                .to_vec();
        let c = state_counts(&nodes);
        assert_eq!(c, [3, 1, 1, 1, 0]);
        assert_eq!(state_title(c), "tiles 3 exact, 1 stale, 1 stand-in, 1 previewed");
        assert_eq!(state_title([1, 0, 0, 0, 2]), "tiles 1 exact, 0 stale, 0 stand-in, 0 previewed, 2 capped");
        let legend = state_legend();
        assert_eq!(legend.len(), TileState::ALL.len());
        for (i, (rgb, words)) in legend.iter().enumerate() {
            assert_eq!(*rgb, STATE_COLOURS[i]);
            assert!(words.starts_with(TileState::ALL[i].name()), "{words}");
        }
        assert!(legend[4].1.contains("reserved"), "the capped colour is reserved for ADR 0074");
        // The codes are the node record's and the shader's (state 0 exact ... 4 capped).
        assert_eq!(TileState::ALL.map(TileState::code), [0, 1, 2, 3, 4]);
    }

    // spec: VIEW-042
    #[test]
    fn the_editing_sun_is_fixed_in_camera_space_over_the_right_shoulder_35_degrees_up_and_follows_the_view() {
        let r = 6_371_000.0;
        let place = PlanetFixed(Vec3::new(0.3, 0.5, 0.8).normalized() * (r + 1_500.0));
        let up = place.0.normalized();
        let cam = |hint: Vec3, pitch: f64| crate::camera::Camera::at_surface_point(place, hint, pitch, 1.0, 16.0 / 9.0, 0.1);
        let sun = |c: &crate::camera::Camera| {
            let s = editing_sun_dir(c);
            Vec3::new(f64::from(s[0]), f64::from(s[1]), f64::from(s[2]))
        };
        let a = cam(Vec3::new(0.0, 0.0, 1.0), -0.3);
        let s = sun(&a);
        assert!((s.length() - 1.0).abs() < 1e-6, "a unit vector: {}", s.length());
        assert!((s.dot(up) - libm::sin(35f64.to_radians())).abs() < 1e-6, "35 degrees above the horizon of the viewer's place");
        let heading = (a.forward - up * a.forward.dot(up)).normalized();
        let right = heading.cross(up).normalized();
        assert!(s.dot(heading) < -0.5, "behind the viewer: the view is lit from the front side of what it sees");
        assert!(s.dot(right) > 0.2, "over the right shoulder, so slopes have a shade");
        assert!((s.dot(heading) / s.dot(right)) < 0.0, "behind and to the right");
        // The view turned a quarter circle turns the sun with it: the same angles to the new heading.
        let b = cam(right, -0.3);
        let (hb, rb) = {
            let h = (b.forward - up * b.forward.dot(up)).normalized();
            (h, h.cross(up).normalized())
        };
        let sb = sun(&b);
        assert!(
            (sb.dot(hb) - s.dot(heading)).abs() < 1e-6 && (sb.dot(rb) - s.dot(right)).abs() < 1e-6 && (sb.dot(up) - s.dot(up)).abs() < 1e-6
        );
        // Looking straight down or up still gives a sun above the horizon, from the camera's up; the same camera gives the same sun.
        for pitch in [-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2] {
            let c = cam(Vec3::new(0.0, 0.0, 1.0), pitch);
            let s = sun(&c);
            assert!(s.dot(up) > 0.5 && (s.length() - 1.0).abs() < 1e-6, "pitch {pitch}: {s:?}");
            assert_eq!(editing_sun_dir(&c), editing_sun_dir(&c));
        }
        // Off at the start; the command toggles it and changes nothing else.
        let mut v = ViewerState::new(0, 3, 0, true);
        assert!(!v.editing_light);
        let c = a;
        v.command(Command::ToggleEditingLight, 10, &c);
        assert!(v.editing_light);
        v.command(Command::ToggleEditingLight, 20, &c);
        assert!(!v.editing_light);
    }
}
