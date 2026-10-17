//! `planet editor`: the viewer window (VIEW-001 to VIEW-006, ADR 0042 point 4). A thin `winit` loop around decisions that live in
//! GPU-free crates: the adapter gate (`planet_frame::adapter`), the flight clock, the key commands, the free camera and the title
//! (`planet_frame::viewer`), the pipelined loop (`pipelined::Rig`) and the plan. `render` owns the surface (ADR 0015); nothing
//! here names a graphics API type.
//!
//! The window runs the scripted orbit-to-1 m flight (the `dive` script of ADR 0032) on the injected clock, on today's debug path.
//! `--smoke` is the test mode: it opens the window, drives the keys through the same command path for a few dozen frames and
//! closes it. Where there is no display it stops with an error that starts with [`NO_DISPLAY`], which a test reports as a skip.

use crate::pipelined::{RendererSink, Rig};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_frame::look::file::LookParams;
use planet_frame::scenes::{build_script, DrawnSet, StreamScene};
use planet_frame::viewer::{
    lit_warm_plan, mode_label, smoke_verdict, title, view_coverage, viewer_lit_plan, Command, ExposureReading, FrameStats, Held, Readiness,
    SmokeCounts, TitleInfo, ViewCoverage, ViewerState, KEYS_HELP,
};
use planet_frame::water::{empty_water_plan, plan_water, WaterRequest, WaterTileCache, WaterView};
use planet_frame::{Camera, ControllerParams, LodParams, TerrainRequest, TerrainView};
use planet_generators::height::HeightField;
use planet_render::{AdapterPolicy, GpuContext, Presented, WindowSurface};
use planet_streaming::{Clock, SystemClock};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// The start of the error of a run that found no display to open a window on: a test reports it as a skip with this reason.
pub const NO_DISPLAY: &str = "no display: ";

/// Frames the smoke run drives before it closes the window.
const SMOKE_FRAMES: u64 = 48;
/// Frames after which a smoke run that has presented nothing gives up (a window that is never shown is not a pass).
const SMOKE_GIVE_UP: u64 = 1500;
/// Seconds between the title updates, and between the GPU-time samples.
const TITLE_PERIOD_MS: u64 = 500;
/// The smoke run reports sooner, so that its short run shows a title.
const SMOKE_TITLE_PERIOD_MS: u64 = 100;
const GPU_SAMPLE_PERIOD: Duration = Duration::from_secs(1);
/// The pre-fill levels the readiness report counts (ADR 0045 point 3 default).
const READINESS_PREFILL_LEVELS: u8 = 4;

/// The command a key press asks for, if any.
pub fn key_command(code: KeyCode) -> Option<Command> {
    Some(match code {
        KeyCode::KeyP | KeyCode::Space => Command::TogglePause,
        KeyCode::KeyR => Command::Restart,
        KeyCode::KeyV => Command::NextView,
        KeyCode::KeyT => Command::ToggleVsync,
        KeyCode::KeyC => Command::ToggleCamera,
        KeyCode::KeyL => Command::ToggleLit,
        KeyCode::Escape => Command::Quit,
        _ => return None,
    })
}

/// Set or clear the held key `code` stands for; `false` when the key is not a held key.
pub fn set_held(held: &mut Held, code: KeyCode, down: bool) -> bool {
    let slot = match code {
        KeyCode::KeyW => &mut held.forward,
        KeyCode::KeyS => &mut held.back,
        KeyCode::KeyA => &mut held.left,
        KeyCode::KeyD => &mut held.right,
        KeyCode::KeyQ => &mut held.down,
        KeyCode::KeyE => &mut held.up,
        KeyCode::ArrowLeft => &mut held.look_left,
        KeyCode::ArrowRight => &mut held.look_right,
        KeyCode::ArrowUp => &mut held.look_up,
        KeyCode::ArrowDown => &mut held.look_down,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => &mut held.boost,
        _ => return false,
    };
    *slot = down;
    true
}

/// What the run needs besides the window.
struct Config {
    size: (u32, u32),
    lod: LodParams,
    seed: u64,
    smoke: bool,
    /// Frames a smoke run drives before it closes the window (`--frames`, default [`SMOKE_FRAMES`]).
    smoke_frames: u64,
    vsync: bool,
    policy: AdapterPolicy,
    gen: planet_streaming::GenWorkers,
    initial_view: usize,
    /// Pre-fill and bake progress of the world, fixed for the run.
    base_readiness: Readiness,
    radius_m: f64,
    /// The water level W the lit plan shades against: the sea of the definition (ADR 0041 point 1; the sea is M5's only producer).
    water_level_m: f64,
    draw: Option<planet_render::DrawMode>,
    upload: Option<planet_render::UploadMode>,
    tile_format: Option<planet_render::TileFormat>,
    /// The body's look (`--look`, the file beside `--world`, else the Earth defaults): the sun, units, exposure range and atmosphere of
    /// the lit path (VIEW-007).
    look: LookParams,
    /// Whether the lit path draws the atmosphere (the look's atmosphere fits the body); the lit path starts on, the debug path with L.
    atmosphere: bool,
    /// The liquid table of the world definition (WDEF-006): what the water tiles are made of.
    liquids: planet_world_def::LiquidTable,
}

/// Everything that exists once the window does.
struct Running {
    window: Arc<Window>,
    ctx: GpuContext,
    surface: WindowSurface,
    rig: Rig,
    viewer: ViewerState,
    stats: FrameStats,
    last_ms: Option<u64>,
    last_gpu_sample: Instant,
    gpu_ms: Option<f64>,
    report: Option<planet_frame::viewer::StatsReport>,
    readiness: Readiness,
    altitude_m: f64,
    frames: u64,
    presented: u64,
    skipped: Option<&'static str>,
    views_seen: usize,
    adapter_line: String,
    /// Whether the lit frames draw the atmosphere.
    atmosphere: bool,
    /// The last title set (the smoke run prints it).
    last_title: String,
    /// The exposure the last sampled lit frame was shown at (the title).
    exposure: Option<ExposureReading>,
    /// What the smoke run counts (VIEW-005); kept in every run, judged only in the smoke run.
    counts: SmokeCounts,
    /// The per-tile answers of the water selection query (ADR 0051 point 6, option 1), kept for the life of the run.
    water_cache: WaterTileCache,
}

struct App<'w> {
    cfg: &'w Config,
    world: &'w Arc<WorldTerrain>,
    field: &'w (dyn HeightField + 'w),
    cameras: &'w [Camera],
    clock: Arc<SystemClock>,
    params: ControllerParams,
    state: Option<Running>,
    error: Option<String>,
    summary: Option<String>,
}

impl App<'_> {
    fn fail(&mut self, el: &ActiveEventLoop, e: String) {
        if self.error.is_none() {
            self.error = Some(e);
        }
        el.exit();
    }

    fn start(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let cfg = self.cfg;
        let attrs = Window::default_attributes().with_title("planet").with_inner_size(PhysicalSize::new(cfg.size.0, cfg.size.1));
        let window = Arc::new(el.create_window(attrs).map_err(|e| format!("{NO_DISPLAY}the window could not be created: {e}"))?);
        let inner = window.inner_size();
        // The gate runs here, before the device: an integrated or software adapter, or one missing a feature or limit, is a
        // message that names the reason (VIEW-001).
        let (ctx, surface) =
            GpuContext::for_window(Arc::clone(&window), (inner.width, inner.height), cfg.policy, cfg.vsync).map_err(|e| e.to_string())?;
        let adapter_line = ctx.adapter_log_line();
        if let Some(mode) = cfg.draw {
            ctx.prefer_draw_mode(mode);
        }
        if let Some(mode) = cfg.upload {
            ctx.prefer_upload_mode(mode);
        }
        if let Some(format) = cfg.tile_format {
            ctx.prefer_tile_format(format);
        }
        let first = self.request(self.cameras[0], TerrainView::ALL[cfg.initial_view]);
        let mut rig = Rig::new(&ctx, self.world, &first, cfg.gen)?;
        // `enable_lit` is not part of `warm_up`: the lit and atmosphere passes are enabled and used once per ring set here, before the
        // first measured frame, whether the window starts lit or on the debug path (REND-017, VIEW-007).
        let warm_lit = viewer_lit_plan(&cfg.look, cfg.radius_m, 0.0, true, cfg.atmosphere)?.with_water_level(cfg.water_level_m);
        rig.renderer
            .warm_up_lit(&ctx, &{
                let mut warm = lit_warm_plan(self.cameras[0], cfg.size, cfg.lod.cells, warm_lit);
                // A world with water warms the water pass too (no node: the pipelines and the copy target exist before frame 0).
                if self.world.has_water() {
                    warm.water = Some(empty_water_plan(&cfg.look.water));
                }
                warm
            })
            .map_err(|e| format!("warming the lit path: {e}"))?;
        let now = self.clock.now_ms();
        let viewer = ViewerState::new(now, TerrainView::ALL.len(), cfg.initial_view, cfg.vsync).with_lit(true);
        self.state = Some(Running {
            window,
            ctx,
            surface,
            rig,
            viewer,
            stats: FrameStats::new(now, if cfg.smoke { SMOKE_TITLE_PERIOD_MS } else { TITLE_PERIOD_MS }),
            last_ms: None,
            last_gpu_sample: Instant::now(),
            gpu_ms: None,
            report: None,
            readiness: cfg.base_readiness,
            altitude_m: 0.0,
            frames: 0,
            presented: 0,
            skipped: None,
            views_seen: 0,
            adapter_line,
            last_title: String::new(),
            exposure: None,
            atmosphere: cfg.atmosphere,
            counts: SmokeCounts::default(),
            water_cache: WaterTileCache::with_book(self.world.water_book()),
        });
        Ok(())
    }

    fn request(&self, camera: Camera, view: TerrainView) -> TerrainRequest {
        TerrainRequest {
            camera,
            size: self.cfg.size,
            view,
            lod: self.cfg.lod.clone(),
            face_mapping: planet_core::cube::FaceMapping::id(&planet_core::cube::TangentWarp).to_string(),
            seed: self.cfg.seed,
        }
    }

    /// One frame of the pipelined loop, drawn and presented. Returns `true` when the window should close.
    fn frame(&mut self) -> Result<bool, String> {
        let cfg = self.cfg;
        let began = Instant::now();
        let now_ms = self.clock.now_ms();
        let (field, radius) = (self.field, cfg.radius_m);
        // Take the state out so the helpers that borrow `self` stay usable; it is put back before returning.
        let mut st = self.state.take().ok_or("no window")?;
        let result = (|| {
            // Frame time on the injected clock; a stall (a dragged window, a breakpoint) does not fling the free camera.
            let dt = st.last_ms.map_or(0.0, |t| now_ms.saturating_sub(t) as f64 / 1000.0).min(0.25);
            st.last_ms = Some(now_ms);
            // The smoke run presses keys through the same command path as the keyboard does.
            let mut quit = false;
            if cfg.smoke {
                quit |= smoke_keys(&mut st, self.cameras, now_ms)?;
            }
            let ground = |p: planet_core::PlanetFixed| radius + field.height(p.0.normalized());
            let height = |p: planet_core::PlanetFixed| p.0.length() - ground(p);
            let camera = st.viewer.camera(self.cameras, now_ms, dt, &self.params, &height, &ground);
            st.altitude_m = height(camera.position);
            let view = TerrainView::ALL[st.viewer.view_index];
            let req = self.request(camera, view);
            let ctx = &st.ctx;
            let frame = st.rig.managed.begin_frame(&req, &mut RendererSink { ctx, renderer: &mut st.rig.renderer })?;
            st.rig.fi.retire_to(st.rig.renderer.retired_frames(ctx));
            while st.rig.fi.is_full() {
                st.rig.fi.note_wait();
                st.rig.fi.retire_to(st.rig.renderer.wait_oldest(ctx));
            }
            let draw = &frame.pipeline.draw;
            // The lit path puts the look's lit plan (adaptive exposure on this frame's dt, the sun and the atmosphere) on the plan
            // the pipelined loop decided; the debug path draws the plan as it is (VIEW-007).
            let lit_plan = if st.viewer.lit {
                let reset = st.viewer.take_exposure_reset();
                let lit = viewer_lit_plan(&cfg.look, cfg.radius_m, dt as f32, reset, cfg.atmosphere)?.with_water_level(cfg.water_level_m);
                let mut plan = draw.plan.clone();
                plan.lit = Some(lit);
                // The water of the frame (ADR 0041, ADR 0051 point 6): the nodes that carry water, decided by one query of the field W
                // per new tile; the renderer draws what the plan says.
                let time_s = now_ms as f64 / 1000.0;
                let (look, liquids, lod) = (&cfg.look.water, &cfg.liquids, &cfg.lod);
                let cache = &mut st.water_cache;
                let water = self.world.with_water(|field| {
                    let req = WaterRequest {
                        source: field,
                        map: &planet_core::cube::TangentWarp,
                        liquids,
                        look,
                        lod,
                        time_s,
                        view: WaterView::Lit,
                    };
                    plan_water(&req, cache, &plan)
                })?;
                if let Some(w) = water {
                    plan.water = Some(w.map_err(|e| e.to_string())?);
                }
                Some(plan)
            } else {
                None
            };
            let plan = lit_plan.as_ref().unwrap_or(&draw.plan);
            // One frame a second is timed on the GPU, which waits for it: the title's GPU time (meaningful with vsync off).
            let timed = ctx.timestamps_supported() && st.last_gpu_sample.elapsed() >= GPU_SAMPLE_PERIOD;
            let (timing, _) = if timed {
                st.last_gpu_sample = Instant::now();
                st.rig.renderer.render(ctx, plan, true)
            } else {
                st.rig.renderer.submit(ctx, plan)
            }
            .map_err(|e| format!("frame {}: {e}", st.frames))?;
            if timed {
                st.gpu_ms = timing.gpu_ms;
            }
            let drew_lit = lit_plan.is_some();
            if drew_lit {
                st.counts.lit_frames += 1;
                st.counts.atmosphere_frames += u64::from(plan.lit.is_some_and(|l| l.atmosphere.is_some()));
                st.counts.water_frames += u64::from(plan.water.as_ref().is_some_and(|w| !w.nodes.is_empty()));
                if cfg.smoke {
                    // The renderer's own account of the last frame's passes: the exposure passes ran, and the sky with the atmosphere.
                    let passes = st.rig.renderer.lit_path().map(|p| p.executed_passes()).unwrap_or_default();
                    st.counts.renderer_ran_lit |= passes.contains(&"exposure-histogram") && passes.contains(&"exposure-adapt");
                    st.counts.renderer_ran_atmosphere |= passes.contains(&"sky-view");
                    st.counts.renderer_ran_water |= passes.contains(&"water");
                }
            } else {
                st.counts.debug_frames += 1;
            }
            st.rig.fi.submit();
            let cpu_ms = began.elapsed().as_secs_f64() * 1000.0 - if timed { timing.total_ms - timing.cpu_ms } else { 0.0 };
            // The exposure the GPU holds is read back where the title wants it (a wait for the GPU, twice a second), and in a smoke
            // run every eighth frame so that a short run takes readings by frame count.
            let report = st.stats.frame(self.clock.now_ms(), cpu_ms);
            if drew_lit && (report.is_some() || (cfg.smoke && st.frames % 8 == 0)) {
                if let Some(path) = st.rig.renderer.lit_path() {
                    let e = path.read_exposure(ctx);
                    st.exposure = Some(ExposureReading { ev100: e.ev100, target_ev100: e.target_ev100 });
                    st.counts.exposure_readings += 1;
                }
            }
            if let Some(report) = report {
                st.report = Some(report);
                let wanted: Vec<_> = frame.pipeline.wanted.iter().map(|w| w.0).collect();
                st.readiness.view = view_coverage(&wanted, &DrawnSet::of_plan(&draw.plan));
                let text = title_text(&st, now_ms);
                st.window.set_title(&text);
                st.last_title = text;
            }
            match st.surface.present(ctx, &st.rig.renderer).map_err(|e| e.to_string())? {
                Presented::Yes => {
                    st.presented += 1;
                    if drew_lit {
                        st.counts.lit_presented += 1;
                    } else {
                        st.counts.debug_presented += 1;
                    }
                }
                Presented::Skipped(why) => st.skipped = Some(why),
            }
            st.frames += 1;
            if cfg.smoke {
                if st.frames >= cfg.smoke_frames.max(SMOKE_FRAMES) && st.presented > 0 {
                    // The verdict is on counts: lit frames with the atmosphere drawn and presented, the debug path too, an exposure
                    // reading, and the renderer's own account that the passes ran. No wall-clock (VIEW-005).
                    smoke_verdict(&st.counts, cfg.atmosphere)?;
                    quit = true;
                } else if st.presented == 0 && st.frames >= SMOKE_GIVE_UP {
                    return Err(format!(
                        "smoke: {} frames and nothing presented (last skip: {}); a window that is never shown is not a pass",
                        st.frames,
                        st.skipped.unwrap_or("none")
                    ));
                }
            }
            Ok(quit)
        })();
        self.state = Some(st);
        result
    }

    fn command(&mut self, cmd: Command) -> bool {
        let now = self.clock.now_ms();
        let Some(st) = self.state.as_mut() else { return false };
        let cam = planet_frame::viewer::camera_at(self.cameras, st.viewer.clock.time_s(now));
        let was_vsync = st.viewer.vsync;
        let quit = st.viewer.command(cmd, now, &cam);
        if st.viewer.vsync != was_vsync {
            st.surface.set_vsync(&st.ctx, st.viewer.vsync);
        }
        if cmd == Command::NextView {
            st.views_seen += 1;
        }
        quit
    }
}

/// The text of the window title now.
fn title_text(st: &Running, now_ms: u64) -> String {
    let Some(r) = st.report else { return String::new() };
    let info = TitleInfo {
        fps: r.fps,
        cpu_ms: r.cpu_ms,
        gpu_ms: st.gpu_ms,
        vsync: st.viewer.vsync,
        present_mode: st.surface.present_mode_name(),
        view: mode_label(st.viewer.lit, st.atmosphere, TerrainView::ALL[st.viewer.view_index].name()),
        free: st.viewer.is_free(),
        paused: st.viewer.clock.is_paused(),
        flight_s: st.viewer.clock.time_s(now_ms),
        altitude_m: st.altitude_m,
        exposure: st.exposure.filter(|_| st.viewer.lit),
        readiness: st.readiness,
    };
    title(&info)
}

/// The keys the smoke run presses, by frame: the view cycle, vsync off and on, pause and resume, the free camera with W held,
/// back to the flight, restart. They go through [`ViewerState::command`] as the keyboard's do.
fn smoke_keys(st: &mut Running, cameras: &[Camera], now_ms: u64) -> Result<bool, String> {
    let cam = planet_frame::viewer::camera_at(cameras, st.viewer.clock.time_s(now_ms));
    let press = |st: &mut Running, c: Command| {
        let was = st.viewer.vsync;
        st.viewer.command(c, now_ms, &cam);
        if st.viewer.vsync != was {
            st.surface.set_vsync(&st.ctx, st.viewer.vsync);
        }
    };
    match st.frames {
        // The window starts lit: V does nothing there, L goes to the debug path, V cycles a debug view, L comes back.
        4 => press(st, Command::NextView),
        6 => press(st, Command::ToggleLit),
        8 => press(st, Command::NextView),
        10 => press(st, Command::ToggleLit),
        12 => press(st, Command::ToggleVsync),
        14 => press(st, Command::ToggleVsync),
        16 => press(st, Command::TogglePause),
        20 => press(st, Command::TogglePause),
        24 => {
            press(st, Command::ToggleCamera);
            st.viewer.held.forward = true;
        }
        32 => {
            st.viewer.held = Held::default();
            press(st, Command::ToggleCamera);
        }
        36 => press(st, Command::Restart),
        _ => {}
    }
    Ok(false)
}

impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.state.is_none() {
            if let Err(e) = self.start(el) {
                self.fail(el, e);
            } else if let Some(st) = &self.state {
                st.window.request_redraw();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(st) = self.state.as_mut() {
                    st.surface.resize(&st.ctx, (size.width, size.height));
                }
            }
            WindowEvent::KeyboardInput { event, .. } if !self.cfg.smoke => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let down = event.state == ElementState::Pressed;
                if let Some(st) = self.state.as_mut() {
                    if set_held(&mut st.viewer.held, code, down) {
                        return;
                    }
                }
                if down && !event.repeat {
                    if let Some(cmd) = key_command(code) {
                        if self.command(cmd) {
                            el.exit();
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => match self.frame() {
                Ok(true) => {
                    self.summary = self.state.as_ref().map(smoke_summary);
                    el.exit();
                }
                Ok(false) => {
                    if let Some(st) = &self.state {
                        st.window.request_redraw();
                    }
                }
                Err(e) => self.fail(el, e),
            },
            _ => {}
        }
    }
}

fn smoke_summary(st: &Running) -> String {
    format!(
        "{} frames drawn ({} lit, {} with the atmosphere, {} with water, {} debug), {} presented ({} lit, {} debug), {} exposure reading(s), {} view change(s), present mode {}, {}
  last title: {}",
        st.frames,
        st.counts.lit_frames,
        st.counts.atmosphere_frames,
        st.counts.water_frames,
        st.counts.debug_frames,
        st.presented,
        st.counts.lit_presented,
        st.counts.debug_presented,
        st.counts.exposure_readings,
        st.views_seen.max(usize::from(st.viewer.view_index != 0)),
        st.surface.present_mode_name(),
        st.adapter_line,
        st.last_title
    )
}

/// `planet editor`: open the window and run the flight until it is closed (or, for `--smoke`, for a few dozen frames).
pub fn run_window(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    crate::worldfield::check_options(o)?;
    // The adapter policy: the application runs on a discrete GPU only; the software adapters are the test and CI mode.
    let policy = if o.smoke {
        o.adapter.unwrap_or_else(AdapterPolicy::from_env)
    } else {
        if o.adapter == Some(AdapterPolicy::Software) {
            return Err("--adapter software is the test and CI mode: the window runs on a discrete GPU (software adapters are used by `editor --smoke` only)".into());
        }
        AdapterPolicy::HighPerformance
    };
    // The display first: a machine without one is told so before any world is baked.
    let event_loop = EventLoop::new().map_err(|e| format!("{NO_DISPLAY}the event loop could not start: {e}"))?;

    let world = Arc::new(
        match &o.store {
            Some(dir) => {
                let res = if o.given.contains("--global-res") { o.global_res } else { crate::streamperf::PRODUCTION_BAKE_RES };
                let def = match &o.world {
                    Some(p) => crate::worldfield::load_world(p)?,
                    None => planet_world_def::WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
                };
                WorldTerrain::open_store(def, dir, res)?
            }
            None => WorldTerrain::of_perf_options(o, crate::skeleton::RADIUS_M)?,
        }
        // The viewer's plans keep the tiles whose water the view sees even where their terrain is culled (ADR 0053).
        .bounding_water(),
    );
    let field = world.field()?;
    let radius_m = world.radius_m();
    let size = o.size.unwrap_or((1280, 720));
    // The same viewport-to-tolerance ratio as the 1440p runs (the plan's steepness check depends on it).
    let tau = o.tau * f64::from(size.1) / 1440.0;
    let lod = LodParams {
        cells: o.cells,
        viewport_h_px: f64::from(size.1),
        tau_px: tau,
        max_level: o.finest_level_for(o.cells),
        radius_m,
        max_height_m: LodParams::earth_1080p().max_height_m.max(field.max_abs_height_m()),
        ..LodParams::earth_1080p()
    };
    lod.validate().map_err(|e| format!("--cells {} --tau {}: {e}", o.cells, tau))?;
    let script = build_script(StreamScene::Dive, radius_m, f64::from(size.0) / f64::from(size.1), field.as_ref(), field.seed());
    let views: Vec<&str> = TerrainView::ALL.iter().map(|v| v.name()).collect();
    // A flight shows the relief view unless `--view` says otherwise (the flat-colour views are one colour at 1 m).
    let view_name = if o.given.contains("--view") { o.view.as_str() } else { "relief" };
    let initial_view = views
        .iter()
        .position(|v| *v == view_name)
        .ok_or_else(|| format!("--view must be one of {}, got '{view_name}'", views.join(", ")))?;
    let (bake_done, bake_total) = world.bake_blocks();
    // The software adapter runs the smoke in the vertex format (the reference format): the compact format with the atmosphere removes the
    // WARP device when a buffer is read back after a frame (docs/learnings.md, 2026-10-17); the compact format keeps the debug path's
    // coverage and the discrete GPU's smoke.
    let software_smoke = o.smoke && policy == AdapterPolicy::Software;
    let tile_format_pref = o.tile_format.or(software_smoke.then_some(planet_render::TileFormat::Vertex));
    let tile_format = tile_format_pref.unwrap_or(planet_render::TileFormat::Compact);
    let (prefill_done, prefill_total) = if o.store.is_some() {
        let levels = o.prefill_levels.unwrap_or(READINESS_PREFILL_LEVELS);
        world.prefill_present(levels, o.cells, o.finest_level_for(o.cells), tile_format).map(|(p, t)| (p as u64, t as u64))?
    } else {
        (0, 0)
    };
    // The lit path draws the body's look (VIEW-007); a look whose atmosphere does not fit the body (a radius too small for it) leaves the
    // lit path without the atmosphere, and the run says so.
    let look = crate::lookload::load(o)?.params(planet_world_def::BodyId::PLANET);
    let atmosphere_note = viewer_lit_plan(&look, radius_m, 0.0, true, true).err();
    let atmosphere = atmosphere_note.is_none();
    let water_level_m =
        world.definition().sea(planet_world_def::BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m;
    let cfg = Config {
        size,
        lod,
        seed: field.seed(),
        smoke: o.smoke,
        smoke_frames: if o.given.contains("--frames") { o.frames as u64 } else { SMOKE_FRAMES },
        vsync: !o.no_vsync,
        policy,
        gen: o.gen_workers(),
        initial_view,
        base_readiness: Readiness { bake_done, bake_total, prefill_done, prefill_total, view: ViewCoverage::default() },
        radius_m,
        water_level_m,
        draw: o.draw,
        upload: o.upload,
        tile_format: tile_format_pref,
        look,
        atmosphere,
        liquids: world.definition().liquids().map_err(|e| format!("the liquid table of the definition: {e}"))?,
    };
    writeln!(out, "planet editor: {} ({})", if o.smoke { "smoke test" } else { "viewer" }, world.describe()).map_err(err)?;
    writeln!(out, "keys: {KEYS_HELP}").map_err(err)?;
    writeln!(
        out,
        "view: the lit path with {} (the look's sun, adaptive exposure); L switches to the debug path",
        if atmosphere { "the atmosphere" } else { "no atmosphere" }
    )
    .map_err(err)?;
    if let Some(why) = atmosphere_note {
        writeln!(out, "atmosphere off: {why}").map_err(err)?;
    }
    let mut app = App {
        cfg: &cfg,
        world: &world,
        field: field.as_ref(),
        cameras: &script.cameras,
        clock: Arc::new(SystemClock::new()),
        params: ControllerParams::for_radius(radius_m),
        state: None,
        error: None,
        summary: None,
    };
    event_loop.run_app(&mut app).map_err(|e| format!("{NO_DISPLAY}the event loop failed: {e}"))?;
    if let Some(e) = app.error {
        return Err(e);
    }
    if let Some(st) = &app.state {
        writeln!(out, "{}", st.adapter_line).map_err(err)?;
    }
    match app.summary {
        Some(s) => writeln!(out, "planet editor [smoke ok]: {s}").map_err(err),
        None => writeln!(out, "planet editor: window closed").map_err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: VIEW-003
    #[test]
    fn every_documented_key_is_bound_and_nothing_else_is() {
        // The help text names what the table binds.
        for (code, cmd) in [
            (KeyCode::KeyP, Command::TogglePause),
            (KeyCode::Space, Command::TogglePause),
            (KeyCode::KeyR, Command::Restart),
            (KeyCode::KeyV, Command::NextView),
            (KeyCode::KeyT, Command::ToggleVsync),
            (KeyCode::KeyC, Command::ToggleCamera),
            (KeyCode::KeyL, Command::ToggleLit),
            (KeyCode::Escape, Command::Quit),
        ] {
            assert_eq!(key_command(code), Some(cmd), "{code:?}");
        }
        for code in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyX, KeyCode::Enter, KeyCode::F5] {
            assert_eq!(key_command(code), None, "{code:?} is not a command");
        }
        for ch in ["pause", "restart", "debug view", "vsync", "free camera", "WASD", "Esc", "lit", "atmosphere"] {
            assert!(KEYS_HELP.to_lowercase().contains(&ch.to_lowercase()), "the help names {ch}: {KEYS_HELP}");
        }
    }

    // spec: VIEW-004
    #[test]
    fn the_held_keys_are_wasd_qe_arrows_and_shift() {
        let mut h = Held::default();
        for code in [
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ShiftLeft,
        ] {
            assert!(set_held(&mut h, code, true), "{code:?}");
        }
        assert_eq!(
            h,
            Held {
                forward: true,
                back: true,
                left: true,
                right: true,
                down: true,
                up: true,
                look_left: true,
                look_right: true,
                look_up: true,
                look_down: true,
                boost: true
            }
        );
        assert!(set_held(&mut h, KeyCode::KeyW, false));
        assert!(!h.forward);
        assert!(!set_held(&mut h, KeyCode::KeyP, true), "P is a command, not a held key");
        assert!(!set_held(&mut h, KeyCode::Space, true));
    }
}
