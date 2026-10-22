//! `planet editor`: the viewer window (VIEW-001 to VIEW-006, ADR 0042 point 4). A thin `winit` loop around decisions that live in
//! GPU-free crates: the adapter gate (`planet_frame::adapter`), the flight clock, the key commands, the free camera and the title
//! (`planet_frame::viewer`), the pipelined loop (`pipelined::Rig`) and the plan. `render` owns the surface (ADR 0015); nothing
//! here names a graphics API type.
//!
//! The window runs the scripted orbit-to-1 m flight (the `dive` script of ADR 0032) on the injected clock, on today's debug path.
//! `--smoke` is the test mode: it opens the window, drives the keys through the same command path for a few dozen frames and
//! closes it. Where there is no display it stops with an error that starts with [`NO_DISPLAY`], which a test reports as a skip.

use crate::editwin::{EditKey, EditRuntime, Pointer, EDIT_KEYS_HELP, SMOKE_EDIT_FRAMES};
use crate::pipelined::{RendererSink, Rig};
use crate::worldfield::WorldTerrain;
use crate::{err, Options};
use planet_core::TileId;
use planet_editor::layout::Layout;
use planet_frame::globe::{marker_at, visible_markers, wheel_zoom_factor, ClickTracker, Marker, ScreenMarker, Viewport, HIT_RADIUS_PX};
use planet_frame::look::file::LookParams;
use planet_frame::mouselook::{Grab, GrabChange, GRAB_ORDER};
use planet_frame::pathlog::PathLog;
use planet_frame::pick::PickLimits;
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
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

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

/// Whether a key press is the camera's and the commands': it is when the world gets it (no text field has the focus), and always while the
/// pointer is captured (looking around: the cursor is hidden and cannot have clicked into a field, so a focus left over from before the
/// capture must not swallow the movement keys).
pub fn key_goes_to_world(world: bool, captured: bool) -> bool {
    world || captured
}

/// Whether a key event is the user's: a release always is (it lets go of a held key), a press only when winit did not make it up on a
/// focus gain (VIEW-022).
pub fn key_press_counts(down: bool, synthetic: bool) -> bool {
    !(down && synthetic)
}

/// The command a key press asks for, if any.
pub fn key_command(code: KeyCode) -> Option<Command> {
    Some(match code {
        KeyCode::KeyP | KeyCode::Space => Command::TogglePause,
        KeyCode::KeyR => Command::Restart,
        KeyCode::KeyV => Command::NextView,
        KeyCode::KeyT => Command::ToggleVsync,
        KeyCode::KeyC => Command::ToggleCamera,
        KeyCode::KeyL => Command::ToggleLit,
        KeyCode::KeyG => Command::ToggleGlobe,
        KeyCode::F3 => Command::CycleOverlay,
        KeyCode::F4 => Command::ToggleCoarseLod,
        KeyCode::F6 => Command::CycleGroundDetail,
        KeyCode::F7 => Command::ToggleScaleRefs,
        KeyCode::KeyH => Command::ToggleEditingLight,
        KeyCode::F9 => Command::SaveRepro,
        KeyCode::Escape => Command::Quit,
        _ => return None,
    })
}

/// What a mouse button does in the editor mode (VIEW-022).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMouse {
    /// The left button went down in the world: press.
    Press,
    /// The left button went up, wherever: release.
    Release,
    /// The right button went down: look around while it is held.
    StartLook,
    /// The right button went up: stop looking.
    EndLook,
}

/// The decision for a mouse button in the editor mode. `world` says the event is the world's (not a panel's), `captured` that the pointer is
/// captured (looking around), `holding` that the left button holds an edit.
pub fn edit_mouse_action(button: MouseButton, state: ElementState, world: bool, captured: bool, holding: bool) -> Option<EditMouse> {
    match (button, state) {
        // A press while the pointer is captured is not at the cursor the window last saw: it does nothing.
        (MouseButton::Left, ElementState::Pressed) if world && !captured => Some(EditMouse::Press),
        (MouseButton::Left, ElementState::Released) => Some(EditMouse::Release),
        // Looking around waits for the left button: a handle held is not let go of by a second button.
        (MouseButton::Right, ElementState::Pressed) if world && !holding => Some(EditMouse::StartLook),
        (MouseButton::Right, ElementState::Released) => Some(EditMouse::EndLook),
        _ => None,
    }
}

/// What the editor mode does to the camera and the overview this frame (VIEW-022), as (end the overview, take the free camera): turning
/// the mode on ends the overview once (`end_overview` is that one request, taken by the caller), and while the mode is on and no overview
/// is shown the camera is the free one. The overview shown later with G stays.
pub fn mode_camera_steps(end_overview: bool, edit_on: bool, globe: bool, free: bool) -> (bool, bool) {
    let end_globe = end_overview && globe;
    let globe_after = globe && !end_globe;
    (end_globe, edit_on && !globe_after && !free)
}

/// The viewer commands the editor mode does not take: it edits from the free camera (VIEW-022); the overview (G) is shown as before.
pub fn editor_mode_blocks(cmd: Command) -> bool {
    matches!(cmd, Command::ToggleCamera)
}

/// The key of the editor mode a key press asks for, if any (VIEW-022).
///
/// Undo and redo are the keys *labelled* Z and Y, whatever the layout (`logical` is the character of the key; on a QWERTZ keyboard the
/// physical key `KeyZ` is labelled Y); without a character the physical keys of a QWERTY keyboard stand in. The rest are physical keys.
pub fn edit_key(code: KeyCode, logical: Option<&str>, ctrl: bool, shift: bool) -> Option<EditKey> {
    let (z, y) = match logical {
        Some(c) => (c.eq_ignore_ascii_case("z"), c.eq_ignore_ascii_case("y")),
        None => (code == KeyCode::KeyZ, code == KeyCode::KeyY),
    };
    if ctrl && z {
        return Some(if shift { EditKey::Redo } else { EditKey::Undo });
    }
    if ctrl && y {
        return Some(EditKey::Redo);
    }
    if logical == Some("?") {
        return Some(EditKey::Help);
    }
    Some(match code {
        KeyCode::Digit1 => EditKey::Tool(1),
        KeyCode::Digit2 => EditKey::Tool(2),
        KeyCode::Digit3 => EditKey::Tool(3),
        KeyCode::Digit4 => EditKey::Tool(4),
        KeyCode::Digit5 => EditKey::Tool(5),
        KeyCode::Digit6 => EditKey::Tool(6),
        KeyCode::Digit7 => EditKey::Tool(7),
        // The brushes: raise, lower, smooth, flatten (VIEW-035).
        KeyCode::Digit8 => EditKey::Tool(8),
        KeyCode::Digit9 => EditKey::Tool(9),
        KeyCode::Digit0 => EditKey::Tool(10),
        KeyCode::Minus => EditKey::Tool(11),
        KeyCode::Delete => EditKey::Delete,
        KeyCode::Backspace => EditKey::Back,
        KeyCode::Enter | KeyCode::NumpadEnter => EditKey::Finish,
        KeyCode::F1 => EditKey::Help,
        KeyCode::Escape => EditKey::Escape,
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
    /// Draw the editor UI (VIEW-011); `--no-ui` turns it off.
    ui: bool,
    /// Start in the editor mode (`--edit`, VIEW-022).
    edit: bool,
    /// The directory the edits are written to and loaded from (`--items`).
    items: Option<PathBuf>,
    /// The panel layout (`--editor-layout`, VIEW-018).
    layout: Layout,
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
    /// The body's look (`--look`, the file beside `--world`, else the Earth defaults): the sun, units, exposure range and atmosphere of
    /// the lit path (VIEW-007).
    look: LookParams,
    /// Whether the lit path draws the atmosphere (the look's atmosphere fits the body); the lit path starts on, the debug path with L.
    atmosphere: bool,
    /// The liquid table of the world definition (WDEF-006): what the water tiles are made of.
    liquids: planet_world_def::LiquidTable,
    /// The item markers of the globe overview (`--marker`, VIEW-013).
    markers: Vec<Marker>,
}

/// What the globe overview did in the run (the smoke summary reports it).
#[derive(Default)]
struct GlobeStats {
    frames: u64,
    markers_shown_max: usize,
    flights: u64,
    landings: u64,
}

/// Everything that exists once the window does.
struct Running {
    /// The editor UI (VIEW-011): egui in the frame after the tone map; `None` with `--no-ui`.
    ui: Option<crate::ui::EditorUi>,
    /// The editor mode (VIEW-022); present with the UI, on or off.
    edit: Option<EditRuntime>,
    /// Control and shift are down (the keys of the editor mode).
    ctrl: bool,
    shift: bool,
    /// The primary button is down in the world in the editor mode.
    edit_down: bool,
    /// The markers of the edits on the overview this frame, and their labels (VIEW-022).
    edit_markers: Vec<Marker>,
    marker_labels: HashMap<u64, String>,
    /// The tiles the last plan wanted: what the overview's markers are found over.
    last_wanted: Vec<TileId>,
    /// Turning the mode on ends the overview, once.
    end_overview: bool,
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
    /// The scale references in the world (VIEW-050) and whether they are dropped.
    scale_refs: Vec<planet_frame::scalerefs::RefBox>,
    scale_refs_on: bool,
    /// The item markers of the globe overview, the ones the overview shows this frame (window pixels) and the one under the cursor.
    markers: Vec<Marker>,
    screen_markers: Vec<ScreenMarker>,
    hover: Option<u64>,
    /// The cursor in render pixels, the camera of the last frame (what a click on the overview is traced through) and a press in progress.
    cursor: [f64; 2],
    /// What a click at the cursor would do in the overview (VIEW-045), kept from the last cursor move; `None` outside the overview.
    click_target: Option<planet_frame::globe::ClickTarget>,
    /// The cursor is the pointer (a click acts here), as last set on the window.
    pointer_cursor: bool,
    last_camera: Camera,
    /// The last minute of camera poses at 10 Hz and the files F9 has written (VIEW-024).
    path: PathLog,
    repro_files: u64,
    /// What the console and the crash directory get of the plan refusals (VIEW-040).
    refusals: planet_frame::pipeline::RefusalReporter,
    /// What the console gets of the tiles held back from refinement (ADR 0074, VIEW-040).
    capped_log: planet_frame::pipeline::CappedReporter,
    /// How many drawn nodes are in each tile state in the last frame drawn with the stale-tile overlay (VIEW-044), for the title.
    tile_states: [usize; 5],
    /// The notice the title shows after F9 and the clock time it is shown until.
    notice: Option<(String, u64)>,
    press: ClickTracker,
    globe_stats: GlobeStats,
    /// The item sweep (`PLANET_SWEEP`, `crate::sweep`): the camera goes to every item of `--items` and the run ends after the last stop.
    sweep: Option<crate::sweep::Sweep>,
}

struct App<'w> {
    cfg: &'w Config,
    world: &'w Arc<WorldTerrain>,
    /// The height field the window picks and reads the ground from; rebuilt when the edits change the definition.
    field: Box<dyn HeightField + 'w>,
    /// The plan parameters: the configured ones, with the height bound raised to the field's maximum when the stamps lift it (VIEW-022).
    lod: LodParams,
    cameras: &'w [Camera],
    clock: Arc<SystemClock>,
    params: ControllerParams,
    state: Option<Running>,
    error: Option<String>,
    summary: Option<String>,
}

/// How long the title keeps the notice of an F9 capture, milliseconds (VIEW-026).
const NOTICE_MS: u64 = 8_000;

/// Where F9 writes its files: `PLANET_REPRO_DIR`, else `target/review` (VIEW-024).
fn repro_dir() -> PathBuf {
    std::env::var_os("PLANET_REPRO_DIR").map_or_else(|| PathBuf::from("target/review"), PathBuf::from)
}

impl App<'_> {
    /// F9 (VIEW-024): `frame-<n>-path.txt` (the camera path of the last minute) and `frame-<n>.bundle` (the terrain repro bundle of this
    /// frame's request over the world as the window holds it, edits included) in [`repro_dir`]; `cargo xtask repro` replays the bundle.
    fn save_repro(&self, st: &Running, camera: Camera, view: TerrainView) -> Result<String, String> {
        let dir = repro_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        let n = st.frames;
        // The camera path of the last minute first: a bundle that cannot be written never loses it.
        let path_path = dir.join(format!("frame-{n}-path.txt"));
        std::fs::write(&path_path, st.path.to_text()).map_err(|e| format!("writing {}: {e}", path_path.display()))?;
        let bundle_path = dir.join(format!("frame-{n}.bundle"));
        let bundle =
            self.repro_bundle(st, camera, view).map_err(|e| format!("{e} (the camera path was written to {})", path_path.display()))?;
        std::fs::write(&bundle_path, bundle.to_text()).map_err(|e| format!("writing {}: {e}", bundle_path.display()))?;
        // The last frame the renderer submitted, as the final tone-mapped colour target: without the egui panels (they are painted on the
        // swapchain image only). A failed write is said in the notice, it does not lose the bundle.
        let png_path = dir.join(format!("frame-{n}.png"));
        let png = match st.rig.renderer.read_color(&st.ctx).write_png(&png_path) {
            Ok(()) => format!("; frame-{n}.png (the scene without the panels)"),
            Err(e) => format!("; the screenshot was not written: {e}"),
        };
        Ok(format!("{}{png}", st.path.saved_notice(n, &dir.display().to_string())))
    }

    /// The terrain repro bundle of this frame's request over the world as the window holds it: the definition text and, with edits, each
    /// item's file text (VIEW-024).
    fn repro_bundle(&self, st: &Running, camera: Camera, view: TerrainView) -> Result<planet_frame::TerrainBundle, String> {
        let request = self.request(camera, view, st.viewer.coarse_lod);
        let plan = planet_frame::plan_terrain_cached(&request, self.field.as_ref(), &mut planet_frame::SelectionCache::new())
            .map_err(|e| format!("planning the frame for the bundle: {e}"))?;
        let definition = self.world.definition();
        let items: Vec<_> = st.edit.as_ref().map(|e| e.session.editor().items().cloned().collect()).unwrap_or_default();
        let (toml, item_texts) = crate::reproworld::record_world(&definition, &items)?;
        Ok(planet_frame::TerrainBundle {
            generator_version: planet_generators::GENERATOR_VERSION,
            implementation_id: planet_generators::IMPLEMENTATION_ID.to_string(),
            mesh_version: planet_generators::mesh::MESH_VERSION,
            definition_hash: definition.hash(),
            adapter: st.ctx.adapter_key(),
            world: Some(planet_frame::BundleWorld {
                height_field: self.world.id().to_string(),
                global_res: self.world.global_res(),
                definition_toml: toml,
                items: item_texts,
            }),
            request,
            plan,
        })
    }

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
        let first = self.request(self.cameras[0], TerrainView::ALL[cfg.initial_view], false);
        // The 384 floor tiles load here, before frame 0 (STRM-001): the title shows how many are in.
        let mut rig = Rig::new_with_progress(&ctx, self.world, &first, cfg.gen, &mut |done, total| {
            window.set_title(&planet_frame::viewer::roots_title(done, total));
        })?;
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
        // The editor holds a re-keyed tile's refresh until its edits have been quiet for a moment (STRM-031, ADR 0072).
        if cfg.ui {
            rig.managed.set_refresh_settle(crate::editwin::REFRESH_SETTLE_MS);
        }
        let viewer = ViewerState::new(now, TerrainView::ALL.len(), cfg.initial_view, cfg.vsync).with_lit(true).with_radius_m(cfg.radius_m);
        let ui = cfg.ui.then(|| crate::ui::EditorUi::new(&window, surface.ui_painter(&ctx)));
        let edit = if cfg.ui {
            let mut e = EditRuntime::new(self.world.definition().as_ref().clone(), cfg.radius_m, cfg.items.as_ref(), cfg.layout.clone())?;
            // A smoke run turns the mode on itself, at the frame its script says, after the overview phases.
            e.on = cfg.edit && !cfg.smoke;
            // The brush guard limits the stack of strokes by what these parameters can draw (VIEW-043).
            e.session.set_drawable_slope(cfg.lod.drawable_slope());
            Some(e)
        } else {
            None
        };
        self.state = Some(Running {
            ui,
            edit,
            ctrl: false,
            shift: false,
            edit_down: false,
            edit_markers: Vec::new(),
            marker_labels: HashMap::new(),
            last_wanted: Vec::new(),
            end_overview: false,
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
            scale_refs: Vec::new(),
            scale_refs_on: false,
            markers: cfg.markers.clone(),
            screen_markers: Vec::new(),
            hover: None,
            cursor: [0.0, 0.0],
            click_target: None,
            pointer_cursor: false,
            last_camera: self.cameras[0],
            path: PathLog::new(),
            repro_files: 0,
            refusals: Default::default(),
            capped_log: Default::default(),
            tile_states: [0; 5],
            notice: None,
            press: ClickTracker::default(),
            globe_stats: GlobeStats::default(),
            sweep: crate::sweep::Sweep::from_env(cfg.items.as_deref())?,
        });
        Ok(())
    }

    /// A click on the globe overview at the cursor: a marker or the surface under it, and the flight there starts (VIEW-013).
    fn globe_click(&mut self) {
        let Some(st) = self.state.as_mut() else { return };
        let viewport = Viewport { width_px: f64::from(self.cfg.size.0), height_px: f64::from(self.cfg.size.1) };
        let limits = planet_frame::pick::PickLimits::new(self.cfg.radius_m, self.lod.max_height_m);
        let field = self.field.as_ref();
        let height = |d: planet_core::Vec3| field.height(d);
        if let Some(g) = st.viewer.globe.as_mut() {
            let all: Vec<Marker> = st.markers.iter().chain(st.edit_markers.iter()).copied().collect();
            // A click that cannot act says so and why, in the title's notice (VIEW-045).
            if let Some(why) = g.click_target(st.cursor, &st.last_camera, viewport, &all, &limits, &height).ignored() {
                st.notice = Some((why.to_string(), self.clock.now_ms() + NOTICE_MS));
            }
            if let Some(click) = g.click(st.cursor, &st.last_camera, viewport, &all, &limits, &height) {
                st.globe_stats.flights += 1;
                // A marker of an edit selects the edit as the flight to it starts.
                if let planet_frame::globe::Click::Marker(id) = click {
                    if let Some(e) = st.edit.as_mut() {
                        e.select_marker(id);
                    }
                }
            }
        }
    }

    /// What a click at the cursor would do in the overview, and the cursor to show for it (VIEW-045): a pointer where a click acts, the
    /// arrow elsewhere. Outside the overview the cursor is the arrow and there is no target.
    fn update_click_target(&mut self) {
        let Some(st) = self.state.as_mut() else { return };
        let viewport = Viewport { width_px: f64::from(self.cfg.size.0), height_px: f64::from(self.cfg.size.1) };
        let limits = planet_frame::pick::PickLimits::new(self.cfg.radius_m, self.lod.max_height_m);
        let field = self.field.as_ref();
        let height = |d: planet_core::Vec3| field.height(d);
        st.click_target = st.viewer.globe.as_ref().map(|g| {
            let all: Vec<Marker> = st.markers.iter().chain(st.edit_markers.iter()).copied().collect();
            g.click_target(st.cursor, &st.last_camera, viewport, &all, &limits, &height)
        });
        let pointer = st.click_target.is_some_and(planet_frame::globe::ClickTarget::acts);
        if pointer != st.pointer_cursor {
            st.pointer_cursor = pointer;
            st.window.set_cursor(if pointer { winit::window::CursorIcon::Pointer } else { winit::window::CursorIcon::Default });
        }
    }

    /// The request of a frame; `coarse` is the editor's coarse-LOD inspection view (the error bound times four, VIEW-014).
    fn request(&self, camera: Camera, view: TerrainView, coarse: bool) -> TerrainRequest {
        TerrainRequest {
            camera,
            size: self.cfg.size,
            view,
            lod: planet_frame::viewer::effective_lod(&self.lod, coarse),
            face_mapping: planet_core::cube::FaceMapping::id(&planet_core::cube::TangentWarp).to_string(),
            seed: self.cfg.seed,
        }
    }

    /// One frame of the pipelined loop, drawn and presented. Returns `true` when the window should close.
    fn frame(&mut self) -> Result<bool, String> {
        let cfg = self.cfg;
        let began = Instant::now();
        let now_ms = self.clock.now_ms();
        // A changed definition goes to the generator before the frame plans: the definition first, then the tiles it changed (STRM-028); the
        // field the window picks and reads the ground from is then built again from the new definition.
        let mut republished = false;
        if let Some(st) = self.state.as_mut() {
            if let Some(e) = st.edit.as_mut() {
                if let Some(def) = e.take_changed_definition() {
                    match self.world.set_definition(Arc::clone(&def)) {
                        Ok(()) => {
                            let resident: Vec<_> = st.rig.managed.resident().keys().copied().collect();
                            let tiles = e.refresh_tiles(&resident);
                            st.rig.managed.edit(&tiles);
                            // The edit in flight is drawn at once from the stale tiles, displaced by the change (VIEW-023).
                            e.preview_begin(&def);
                            let stamps = e.preview_stamps(st.last_camera.position.0);
                            st.rig.managed.set_preview(stamps);
                            republished = true;
                        }
                        Err(why) => {
                            e.stats.refused += 1;
                            e.session.set_status(format!("the terrain did not take the edit: {why}"));
                        }
                    }
                }
            }
        }
        // The brush stroke under the pointer is previewed as it is painted (VIEW-036); when it ends the put stroke's delta takes over.
        if let Some(st) = self.state.as_mut() {
            if let Some(stamps) = st.edit.as_mut().and_then(|e| e.transient_preview()) {
                st.rig.managed.set_transient_preview(stamps);
            }
        }
        if republished {
            let world = self.world;
            self.field = world.field()?;
            // The stamps lift the field: the plan's bound for the horizon, the frustum and the morph follows the field's maximum, and is never
            // below the configured one (a run with no edits plans as it did).
            self.lod.max_height_m = cfg.lod.max_height_m.max(self.field.max_abs_height_m());
        }
        let lod_max_height_m = self.lod.max_height_m;
        let (field, radius) = (self.field.as_ref(), cfg.radius_m);
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
            // The editor mode edits from the free camera: turning it on ended the overview once (G may show it again).
            {
                let flight = planet_frame::viewer::camera_at(self.cameras, st.viewer.clock.time_s(now_ms));
                let on = st.edit.as_ref().is_some_and(|e| e.on);
                let (end_globe, to_free) =
                    mode_camera_steps(std::mem::take(&mut st.end_overview), on, st.viewer.globe.is_some(), st.viewer.is_free());
                if end_globe {
                    st.viewer.command(Command::ToggleGlobe, now_ms, &flight);
                }
                if to_free {
                    st.viewer.command(Command::ToggleCamera, now_ms, &flight);
                }
            }
            if cfg.smoke && cfg.edit {
                let viewport = Viewport { width_px: f64::from(cfg.size.0), height_px: f64::from(cfg.size.1) };
                let limits = PickLimits::new(radius, lod_max_height_m);
                let camera = st.last_camera;
                let frames = st.frames;
                if let Some(e) = st.edit.as_mut() {
                    e.smoke_step(frames, &camera, viewport, &limits, field)?;
                }
            }
            // A command of this frame (C back to the flight, restart) may have released the mouse.
            apply_grab(&mut st);
            let ground = |p: planet_core::PlanetFixed| radius + field.height(p.0.normalized());
            let height = |p: planet_core::PlanetFixed| p.0.length() - ground(p);
            let had_globe = st.viewer.globe.is_some();
            let camera = st.viewer.camera(self.cameras, now_ms, dt, &self.params, &height, &ground);
            let camera = match st.sweep.as_ref() {
                Some(s) if !s.done() => s.camera(&|d| field.height(d), radius, f64::from(cfg.size.0) / f64::from(cfg.size.1)),
                _ => camera,
            };
            st.last_camera = camera;
            st.globe_stats.landings += u64::from(had_globe && st.viewer.globe.is_none() && st.viewer.is_free());
            // The globe overview: the markers it shows (render pixels to window pixels) and the one under the cursor.
            let viewport = Viewport { width_px: f64::from(cfg.size.0), height_px: f64::from(cfg.size.1) };
            if st.viewer.globe.is_some() {
                // The edits the camera shows are markers too (VIEW-022), found over the tiles the last plan wanted.
                if let Some(e) = st.edit.as_mut() {
                    st.edit_markers = e.globe_markers(&camera, viewport, &st.last_wanted);
                    st.marker_labels = e.marker_labels();
                }
                let all: Vec<Marker> = st.markers.iter().chain(st.edit_markers.iter()).copied().collect();
                let seen = visible_markers(&camera, &all, radius, viewport);
                st.hover = marker_at(&seen, st.cursor, HIT_RADIUS_PX);
                st.globe_stats.frames += 1;
                st.globe_stats.markers_shown_max = st.globe_stats.markers_shown_max.max(seen.len());
                let inner = st.window.inner_size();
                let (sx, sy) = (f64::from(inner.width) / viewport.width_px, f64::from(inner.height) / viewport.height_px);
                st.screen_markers = seen.iter().map(|m| ScreenMarker { id: m.id, pixel: [m.pixel[0] * sx, m.pixel[1] * sy] }).collect();
                if cfg.smoke {
                    smoke_globe(&mut st, &camera, viewport, radius, lod_max_height_m, field);
                }
            } else {
                st.screen_markers.clear();
                st.edit_markers.clear();
                st.marker_labels.clear();
                st.hover = None;
            }
            st.altitude_m = height(camera.position);
            st.path.record(now_ms, &camera, st.altitude_m, dt * 1000.0);
            let view = TerrainView::ALL[st.viewer.view_index];
            let ctx = &st.ctx;
            // The coarse view's error bound may not fit the terrain here (a steep neighbourhood leaves no morph room, the plan worker
            // says so): back to the base parameters for this frame and after (F4 asks again), never a dead window.
            let frame =
                planet_frame::viewer::with_coarse_fallback(&mut st.viewer.coarse_lod, &mut st.counts.coarse_lod_refusals, |coarse| {
                    let req = self.request(camera, view, coarse);
                    st.rig.managed.begin_frame(&req, &mut RendererSink { ctx, renderer: &mut st.rig.renderer })
                })?;
            st.counts.coarse_lod_frames += u64::from(st.viewer.coarse_lod);
            // A refusal of the plan for the base parameters degrades the frame (it draws the newest plan in hand); it is said once, never fatal.
            // One line, one report file and one title notice per cause (kind and level) in 5 s; the title counts all of them.
            let refusal = st.rig.managed.take_plan_refusal();
            if let Some(why) = refusal.clone() {
                if let Some(line) = st.refusals.observe(&why, now_ms) {
                    crate::crash::note(&format!("warning: {line}\n  the window keeps the last plan and goes on"));
                    crate::crash::set_path_text(st.path.to_text());
                    let report = crate::crash::snapshot("plan refusal (the window went on)", &line);
                    let text = report.map_or_else(|| why.clone(), |p| format!("plan refused, report {}", p.display()));
                    st.notice = Some((text, now_ms + NOTICE_MS));
                }
            }
            let capped = frame.pipeline.draw.plan.nodes.iter().filter(|n| n.capped).count();
            if let Some(line) = st.capped_log.observe(capped, now_ms) {
                crate::crash::note(&line);
            }
            if let Some(sw) = st.sweep.as_mut() {
                sw.frame(
                    dt * 1000.0,
                    frame.pipeline.delivered.map(|d| d.1),
                    frame.pipeline.residency_delivered.map(|d| d.1),
                    st.rig.managed.pipeline().refused_plans(),
                    refusal,
                );
                if sw.done() {
                    let (text, ok) = sw.finish()?;
                    println!("{text}");
                    if !ok {
                        return Err("the sweep had refused plans (the report above lists the stops)".into());
                    }
                    quit = true;
                }
            }
            // A lost GPU device cannot be drawn on again: report it and end the run with the reason.
            if let Some(lost) = st.ctx.device_lost() {
                crate::crash::set_path_text(st.path.to_text());
                crate::crash::report("GPU device lost", &lost, "wgpu device-lost callback", "");
                return Err(lost);
            }
            // The path the report carries, refreshed about once a second.
            if st.frames % 60 == 0 {
                crate::crash::set_path_text(st.path.to_text());
            }
            // F9: the camera path of the last minute and the repro bundle of this frame (VIEW-024).
            if st.viewer.take_repro_request() {
                match self.save_repro(&st, camera, view) {
                    Ok(text) => {
                        st.repro_files += 1;
                        eprintln!(
                            "{text}
{}",
                            st.path.summary()
                        );
                        st.window.set_title(&format!("planet | {text}"));
                        st.notice = Some((text, now_ms + NOTICE_MS));
                    }
                    Err(e) => {
                        eprintln!("F9: the repro files were not written: {e}");
                        let text = format!("F9: the repro files were not written: {e}");
                        st.window.set_title(&format!("planet | {text}"));
                        st.notice = Some((text, now_ms + NOTICE_MS));
                    }
                }
            }
            st.last_wanted = frame.pipeline.wanted.iter().map(|w| w.0).collect();
            // The preview ends when the exact tiles are in (VIEW-023).
            if let Some(e) = st.edit.as_mut() {
                e.stats.previewed_max = e.stats.previewed_max.max(frame.pipeline.draw.plan.nodes.iter().filter(|n| n.preview).count());
                if e.preview_active() && crate::editwin::exact_and_idle(&st.rig.managed, &frame) {
                    st.rig.managed.set_preview(Vec::new());
                    e.preview_end();
                }
            }
            // The editor mode: what the camera shows of the edits, their outlines and handles, the hover, the lists; the resident tiles the
            // edits of the last frame can touch (the footprint find).
            if let Some(e) = st.edit.as_mut().filter(|e| e.on) {
                let wanted = st.last_wanted.clone();
                let limits = PickLimits::new(radius, lod_max_height_m);
                let over_ui = st.ui.as_ref().is_some_and(|u| u.ctx.is_pointer_over_egui());
                // The tiles the plan holds back (ADR 0074): the edits over them are problems of the list (VIEW-047).
                e.set_capped_tiles(
                    frame.pipeline.draw.plan.nodes.iter().filter(|n| n.state == planet_frame::TileState::Capped).map(|n| n.tile).collect(),
                );
                e.update(&camera, viewport, &wanted, field, &limits, Pointer { cursor: st.cursor, over_ui });
            }
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
                let mut lit = viewer_lit_plan(&cfg.look, cfg.radius_m, dt as f32, reset, cfg.atmosphere)?
                    .with_water_level(cfg.water_level_m)
                    .with_overlay(st.viewer.overlay)
                    .with_ground_detail(st.viewer.ground_detail.strength());
                // The editing light (VIEW-042): one sun for the terrain shading, the atmosphere and the sky, fixed in camera space.
                if st.viewer.editing_light {
                    lit.sun_dir = planet_frame::viewer::editing_sun_dir(&draw.plan.camera);
                }
                st.counts.overlay_frames += u64::from(lit.overlay != planet_frame::look::Overlay::None);
                st.counts.mesh_overlay_frames += u64::from(lit.overlay == planet_frame::look::Overlay::Mesh);
                if lit.overlay == planet_frame::look::Overlay::Stale {
                    st.counts.stale_overlay_frames += 1;
                    st.tile_states = planet_frame::viewer::state_counts(&draw.plan.nodes);
                }
                let mut plan = draw.plan.clone();
                plan.lit = Some(lit);
                // The water of the frame (ADR 0041, ADR 0051 point 6): the nodes that carry water, decided by one query of the field W
                // per new tile; the renderer draws what the plan says.
                let time_s = now_ms as f64 / 1000.0;
                let coarse_lod = planet_frame::viewer::effective_lod(&cfg.lod, st.viewer.coarse_lod);
                let (look, liquids, lod) = (&cfg.look.water, &cfg.liquids, &coarse_lod);
                let cache = &mut st.water_cache;
                let water = self.world.with_water(|wf| {
                    let req = WaterRequest {
                        source: wf,
                        map: &planet_core::cube::TangentWarp,
                        liquids,
                        look,
                        lod,
                        time_s,
                        view: WaterView::Lit,
                        cut: Some(field),
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
            st.rig.renderer.set_preview_of(draw);
            // The scale references (VIEW-050): dropped on the ground in front of the camera when the key turns them on, taken away when it turns
            // them off; the renderer draws what it is given.
            if st.viewer.scale_refs != st.scale_refs_on {
                st.scale_refs_on = st.viewer.scale_refs;
                st.scale_refs = if st.scale_refs_on {
                    planet_frame::scalerefs::drop_references(plan.camera.position, plan.camera.forward, radius, &|d| field.height(d))
                } else {
                    Vec::new()
                };
            }
            st.rig.renderer.set_scale_refs(&st.scale_refs);
            // The regions overlay (VIEW-030): the region items of the editor's definition nearest the camera, decided here, drawn by the renderer.
            if plan.lit.is_some_and(|l| l.overlay == planet_frame::look::Overlay::Regions) {
                let list = st.edit.as_ref().map(|e| e.region_list(plan.camera.position.0)).unwrap_or_default();
                st.rig.renderer.set_regions(&list);
            }
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
                st.counts.editing_light_frames += u64::from(st.viewer.editing_light);
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
            let mut panel_events: Vec<crate::panels::Event> = Vec::new();
            let presented = if let Some(ui) = st.ui.as_mut() {
                let factor = if st.viewer.coarse_lod { cfg.lod.coarse_factor() } else { 1.0 };
                let status = planet_frame::viewer::overlay_status(st.viewer.overlay, factor);
                let inner = st.window.inner_size();
                let scale = [f64::from(inner.width) / viewport.width_px, f64::from(inner.height) / viewport.height_px];
                let layer = crate::ui::MarkerLayer { markers: &st.screen_markers, hover: st.hover, labels: &st.marker_labels };
                // The legend of the stale-tile overlay (VIEW-044) in the corner while it is on.
                ui.legend =
                    if st.viewer.overlay == planet_frame::look::Overlay::Stale { planet_frame::viewer::state_legend() } else { Vec::new() };
                let mut frame_ui = match st.edit.as_mut().filter(|e| e.on) {
                    Some(e) => {
                        let view_status = format!("{} · {status}", crate::editwin::mode_status(e));
                        let mut draw = |ui: &mut egui::Ui| panel_events = e.draw(ui, &view_status, scale);
                        ui.frame(&st.window, &status, &layer, Some(&mut draw))
                    }
                    None => ui.frame(&st.window, &status, &layer, None),
                };
                let shown = st.surface.present_with_ui(ctx, &st.rig.renderer, Some((ui.painter(), &mut frame_ui)));
                ui.reclaim(frame_ui);
                shown.map_err(|e| e.to_string())?
            } else {
                st.surface.present(ctx, &st.rig.renderer).map_err(|e| e.to_string())?
            };
            // What the panels asked for, and the camera flight to an edit.
            if let Some(e) = st.edit.as_mut() {
                e.apply(panel_events);
            }
            if let Some(id) = st.edit.as_mut().and_then(|e| e.take_fly()) {
                if let Some(pose) = st.edit.as_ref().and_then(|e| e.fly_pose(id, &|d| field.height(d))) {
                    let flight = planet_frame::viewer::camera_at(self.cameras, st.viewer.clock.time_s(now_ms));
                    if !st.viewer.is_free() {
                        st.viewer.command(Command::ToggleCamera, now_ms, &flight);
                    }
                    st.viewer.mode = planet_frame::viewer::CameraMode::Free(pose);
                    st.viewer.mouse.release();
                    apply_grab(&mut st);
                    if let Some(e) = st.edit.as_mut() {
                        e.stats.flights += 1;
                    }
                }
            }
            match presented {
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
                // The editing script starts once the camera rests, so the run waits for it (at most until the give-up frame).
                let editing_done = !cfg.edit || st.edit.as_ref().is_none_or(|e| e.smoke_finished()) || st.frames >= SMOKE_GIVE_UP;
                if st.frames >= cfg.smoke_frames.max(SMOKE_FRAMES) && st.presented > 0 && editing_done {
                    // The verdict is on counts: lit frames with the atmosphere drawn and presented, the debug path too, an exposure
                    // reading, and the renderer's own account that the passes ran. No wall-clock (VIEW-005).
                    smoke_verdict(&st.counts, cfg.atmosphere)?;
                    if cfg.edit {
                        if let Some(e) = st.edit.as_ref() {
                            e.smoke_verdict()?;
                        }
                    }
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

    /// The editor mode is on.
    fn edit_on(&self) -> bool {
        self.state.as_ref().and_then(|s| s.edit.as_ref()).is_some_and(|e| e.on)
    }

    /// A mouse button in the editor mode: the left one edits (a press in the world presses, a release always releases), the right one looks
    /// around while it is held.
    fn edit_mouse(&mut self, state: ElementState, button: MouseButton, world: bool) {
        let viewport = Viewport { width_px: f64::from(self.cfg.size.0), height_px: f64::from(self.cfg.size.1) };
        let limits = PickLimits::new(self.cfg.radius_m, self.lod.max_height_m);
        let field = self.field.as_ref();
        let Some(st) = self.state.as_mut() else { return };
        match edit_mouse_action(button, state, world, st.viewer.mouse.captured(), st.edit_down) {
            Some(EditMouse::Press) => {
                let camera = st.last_camera;
                if let Some(e) = st.edit.as_mut() {
                    let mods = crate::editwin::Mods { shift: st.shift, ctrl: st.ctrl };
                    e.press_with(&camera, viewport, st.cursor, &limits, field, mods);
                    st.edit_down = true;
                }
            }
            Some(EditMouse::Release) => {
                if st.edit_down {
                    if let Some(e) = st.edit.as_mut() {
                        e.release();
                    }
                }
                st.edit_down = false;
            }
            Some(EditMouse::StartLook) => {
                let free = st.viewer.is_free();
                st.viewer.mouse.click(free);
                apply_grab(st);
            }
            Some(EditMouse::EndLook) => {
                st.viewer.mouse.release();
                apply_grab(st);
            }
            None => {}
        }
    }

    fn command(&mut self, cmd: Command) -> bool {
        let now = self.clock.now_ms();
        let Some(st) = self.state.as_mut() else { return false };
        let cam = planet_frame::viewer::camera_at(self.cameras, st.viewer.clock.time_s(now));
        let was_vsync = st.viewer.vsync;
        if cmd == Command::ToggleGlobe {
            // A press in progress belongs to the overview that is going away, or to none.
            st.press.cancel();
        }
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

/// The markers of `--marker LAT,LON[,RADIUS_M]` (degrees; the footprint radius defaults to 1,000 m), numbered from 1 in the order given.
pub fn parse_markers(texts: &[String]) -> Result<Vec<Marker>, String> {
    texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let parts: Vec<&str> = text.split(',').map(str::trim).collect();
            let number = |s: &str| {
                s.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or_else(|| format!("--marker '{text}': '{s}' is not a number"))
            };
            let usage = || format!("--marker '{text}': expected LAT,LON[,RADIUS_M]");
            let (lat, lon, radius_m) = match parts.as_slice() {
                [lat, lon] => (number(lat)?, number(lon)?, 1_000.0),
                [lat, lon, r] => (number(lat)?, number(lon)?, number(r)?),
                _ => return Err(usage()),
            };
            if !(-90.0..=90.0).contains(&lat) {
                return Err(format!("--marker '{text}': latitude {lat} is outside -90..90"));
            }
            if radius_m <= 0.0 {
                return Err(format!("--marker '{text}': the radius {radius_m} m must be positive"));
            }
            let (la, lo) = (lat.to_radians(), lon.to_radians());
            let dir = planet_core::Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
            Ok(Marker { id: i as u64 + 1, dir, radius_m })
        })
        .collect()
}

/// The smoke run drives the globe overview as the mouse would: a drag and a wheel step, then a click on the marker it put under the camera.
fn smoke_globe(st: &mut Running, camera: &Camera, viewport: Viewport, radius_m: f64, max_height_m: f64, field: &dyn HeightField) {
    match st.frames {
        40 => {
            if let Some(g) = st.viewer.globe.as_mut() {
                g.drag(10.0, 5.0, camera.fov_y_rad, viewport.height_px);
                g.zoom(0.9);
            }
        }
        42 => {
            let limits = planet_frame::pick::PickLimits::new(radius_m, max_height_m);
            let seen = visible_markers(camera, &st.markers, radius_m, viewport);
            if let Some(m) = seen.iter().find(|m| m.id == 1000) {
                st.cursor = m.pixel;
                let height = |d: planet_core::Vec3| field.height(d);
                if let Some(g) = st.viewer.globe.as_mut() {
                    if g.click(m.pixel, camera, viewport, &st.markers, &limits, &height).is_some() {
                        st.globe_stats.flights += 1;
                    }
                }
            }
        }
        _ => {}
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
    let mut input = planet_frame::viewer::input_text(&st.viewer.held, st.viewer.mouse.captured(), st.viewer.globe.is_some());
    let refused = st.rig.managed.pipeline().refused_plans();
    if refused > 0 {
        input += &format!(" | plans refused x{refused}");
    }
    if st.viewer.editing_light {
        input += " | editing light";
    }
    input += &format!(" | detail {}", st.viewer.ground_detail.name());
    if st.viewer.scale_refs {
        input += " | scale references";
    }
    // The hover hint of the overview: only where a click acts (VIEW-045).
    if let Some(hint) = st.click_target.and_then(planet_frame::globe::ClickTarget::hint) {
        input += &format!(" | {hint}");
    }
    if st.viewer.overlay == planet_frame::look::Overlay::Stale {
        input += &format!(" | {}", planet_frame::viewer::state_title(st.tile_states));
    }
    match &st.notice {
        Some((text, until)) if now_ms < *until => format!("{} | {input} | {text}", title(&info)),
        _ => format!("{} | {input}", title(&info)),
    }
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
        // The overlay cycles off, tiles, mesh, regions, off; the coarse LOD is on from key 29 to key 34, over the mesh, regions and off frames.
        // The globe overview: G shows it over the point under the camera with a marker there (the frame loop drags, zooms and clicks it), G
        // again at the end of the run.
        38 => {
            press(st, Command::ToggleGlobe);
            if let Some(g) = st.viewer.globe {
                st.markers.push(Marker { id: 1000, dir: g.view.center, radius_m: 2_000.0 });
            }
        }
        26 => press(st, Command::CycleOverlay),
        28 => press(st, Command::CycleOverlay),
        29 => press(st, Command::ToggleCoarseLod),
        31 => press(st, Command::CycleOverlay),
        33 => press(st, Command::CycleOverlay),
        34 => press(st, Command::ToggleCoarseLod),
        // The fourth F3 press (key 33) showed the stale-tile overlay (VIEW-044); this one is back to off.
        35 => press(st, Command::CycleOverlay),
        36 => press(st, Command::Restart),
        // The editing light is on from key 41 to key 45, over lit frames of the restarted flight (VIEW-042).
        41 => press(st, Command::ToggleEditingLight),
        45 => press(st, Command::ToggleEditingLight),
        // F9 writes files: the run presses it only where a directory for them was named.
        40 if std::env::var_os("PLANET_REPRO_DIR").is_some() => press(st, Command::SaveRepro),
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

    /// The raw relative motion of the pointer (VIEW-010): counts, used only while the mouse is captured.
    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta: (dx, dy) }, Some(st)) = (event, self.state.as_mut()) {
            st.viewer.mouse.motion(dx, dy);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // The editor UI sees every window event first (VIEW-011); `world` says whether the camera and the commands may have it. While the
        // mouse is captured the pointer belongs to the camera: egui gets no pointer event then.
        let mut world = true;
        if let Some(st) = self.state.as_mut() {
            let pointer = matches!(
                event,
                WindowEvent::MouseInput { .. }
                    | WindowEvent::CursorMoved { .. }
                    | WindowEvent::CursorEntered { .. }
                    | WindowEvent::CursorLeft { .. }
                    | WindowEvent::MouseWheel { .. }
                    | WindowEvent::Touch(_)
            );
            if let Some(ui) = st.ui.as_mut().filter(|_| !(pointer && st.viewer.mouse.captured())) {
                world = ui.window_event(&st.window, &event);
            }
        }
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::ModifiersChanged(m) => {
                if let Some(st) = self.state.as_mut() {
                    st.ctrl = m.state().control_key();
                    st.shift = m.state().shift_key();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(st) = self.state.as_mut() {
                    st.surface.resize(&st.ctx, (size.width, size.height));
                }
            }
            WindowEvent::KeyboardInput { event, is_synthetic, .. } if !self.cfg.smoke => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let down = event.state == ElementState::Pressed;
                // A press winit makes up when the window gains the focus for a key that is still down (Alt+Tab back in holds Tab) is no
                // key press of the user: it toggles nothing and commands nothing (VIEW-022).
                if !key_press_counts(down, is_synthetic) {
                    return;
                }
                if let Some(st) = self.state.as_mut() {
                    // A key the UI took (a text field has the focus): the camera and the commands get no press. A release still lets go
                    // of a held key, so none stays down. Esc with the mouse captured is the camera's.
                    if !key_goes_to_world(world, st.viewer.mouse.captured()) && down {
                        return;
                    }
                    // Esc with the mouse captured releases it and does not quit (VIEW-010).
                    if down && !event.repeat && code == KeyCode::Escape && st.viewer.mouse.escape() {
                        apply_grab(st);
                        return;
                    }
                    // The editor mode (VIEW-022): Tab turns it on and off; while it is on its keys come first and Esc steps back in it.
                    if down && !event.repeat {
                        if code == KeyCode::Tab {
                            if let Some(e) = st.edit.as_mut() {
                                e.toggle();
                                st.edit_down = false;
                                st.end_overview = e.on;
                            }
                            return;
                        }
                        let logical = match &event.logical_key {
                            winit::keyboard::Key::Character(c) => Some(c.to_string()),
                            _ => None,
                        };
                        if let (Some(e), Some(k)) =
                            (st.edit.as_mut().filter(|e| e.on), edit_key(code, logical.as_deref(), st.ctrl, st.shift))
                        {
                            e.key(k);
                            return;
                        }
                    }
                    if set_held(&mut st.viewer.held, code, down) {
                        return;
                    }
                }
                if down && !event.repeat {
                    if let Some(cmd) = key_command(code) {
                        if self.edit_on() && editor_mode_blocks(cmd) {
                            return;
                        }
                        if self.command(cmd) {
                            el.exit();
                        }
                    }
                }
            }
            // The globe overview (VIEW-013): the cursor drags it while the left button is down, the wheel zooms, and a press and release that
            // stay within a few pixels are a click on a marker or the surface.
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(st) = self.state.as_mut() {
                    let inner = st.window.inner_size();
                    let (sx, sy) = (
                        f64::from(self.cfg.size.0) / f64::from(inner.width.max(1)),
                        f64::from(self.cfg.size.1) / f64::from(inner.height.max(1)),
                    );
                    let now = [position.x * sx, position.y * sy];
                    // A press only counts while the overview is shown; otherwise it is stale.
                    if st.viewer.globe.is_none() {
                        st.press.cancel();
                    }
                    let d = [now[0] - st.cursor[0], now[1] - st.cursor[1]];
                    if world && st.press.moved_by(libm::hypot(d[0], d[1])) {
                        if let Some(g) = st.viewer.globe.as_mut() {
                            g.drag(d[0], d[1], self.cameras[0].fov_y_rad, f64::from(self.cfg.size.1));
                        }
                    }
                    st.cursor = now;
                    if world && st.edit_down {
                        let viewport = Viewport { width_px: f64::from(self.cfg.size.0), height_px: f64::from(self.cfg.size.1) };
                        let limits = PickLimits::new(self.cfg.radius_m, self.lod.max_height_m);
                        let camera = st.last_camera;
                        if let Some(e) = st.edit.as_mut() {
                            e.moved(&camera, viewport, now, &limits, self.field.as_ref());
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. }
                if !self.cfg.smoke && self.state.as_ref().is_some_and(|s| s.viewer.globe.is_some()) =>
            {
                let mut clicked = false;
                if let Some(st) = self.state.as_mut() {
                    match (state, world) {
                        (ElementState::Pressed, true) => st.press.press(),
                        // A release ends the press wherever it lands (over a panel too); only one the overview owns is a click.
                        (ElementState::Pressed, false) => {
                            st.press.cancel();
                            st.notice = Some(("click ignored: the cursor is over a panel".to_string(), self.clock.now_ms() + NOTICE_MS));
                        }
                        (ElementState::Released, _) => clicked = st.press.release() && world,
                    }
                }
                if clicked {
                    self.globe_click();
                }
            }
            WindowEvent::MouseInput { state, button, .. }
                if !self.cfg.smoke && self.edit_on() && self.state.as_ref().is_some_and(|s| s.viewer.globe.is_none()) =>
            {
                self.edit_mouse(state, button, world);
            }
            WindowEvent::MouseWheel { delta, .. } if world => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                if let Some(st) = self.state.as_mut() {
                    // A brush tool takes the wheel for its radius (Shift: strength); without one the wheel zooms the overview as before (VIEW-041).
                    if st.viewer.globe.is_none() && st.edit.as_mut().is_some_and(|e| e.wheel(lines, st.shift)) {
                        return;
                    }
                    if let Some(g) = st.viewer.globe.as_mut() {
                        g.zoom(wheel_zoom_factor(lines));
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if !self.cfg.smoke && world && !self.edit_on() && self.state.as_ref().is_some_and(|s| s.viewer.globe.is_none()) =>
            {
                if let Some(st) = self.state.as_mut() {
                    let free = st.viewer.is_free();
                    st.viewer.mouse.click(free);
                    apply_grab(st);
                }
            }
            WindowEvent::Focused(false) => {
                if let Some(st) = self.state.as_mut() {
                    st.press.cancel();
                    st.viewer.focus_lost();
                    if let Some(e) = st.edit.as_mut() {
                        e.cancel_drag();
                    }
                    st.edit_down = false;
                    apply_grab(st);
                }
            }
            WindowEvent::RedrawRequested => {
                self.update_click_target();
                let drawn = self.frame();
                match drawn {
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
                }
            }
            _ => {}
        }
    }
}

/// Apply the grab change the mouse look asked for (VIEW-010): a capture grabs the pointer, `Locked` first and `Confined` when the platform
/// refuses it, and hides the cursor; a release shows it and lets it go. A capture neither mode allows is released again.
fn apply_grab(st: &mut Running) {
    let change = st.viewer.mouse.take_grab_change();
    // egui gets no pointer events while the pointer is captured: it is told the buttons are up when the capture starts and when it ends.
    if change.is_some() {
        if let Some(ui) = st.ui.as_mut() {
            ui.release_buttons();
        }
    }
    match change {
        Some(GrabChange::Capture) => {
            // A field that kept the focus would take the movement keys of the look that starts now.
            if let Some(ui) = st.ui.as_mut() {
                ui.release_keyboard_focus();
            }
            let mode = |g: &Grab| match g {
                Grab::Locked => CursorGrabMode::Locked,
                Grab::Confined => CursorGrabMode::Confined,
            };
            if GRAB_ORDER.iter().any(|g| st.window.set_cursor_grab(mode(g)).is_ok()) {
                st.window.set_cursor_visible(false);
            } else {
                st.viewer.mouse.release();
            }
        }
        Some(GrabChange::Release) => {
            let _ = st.window.set_cursor_grab(CursorGrabMode::None);
            st.window.set_cursor_visible(true);
        }
        None => {}
    }
}

fn smoke_summary(st: &Running) -> String {
    let ui = st.ui.as_ref().map_or(String::new(), |u| {
        format!(
            "
  ui: {} frames with the editor UI, {:.3} ms mean CPU per frame (egui run and tessellation), pointer events to the UI {} and to the world {}",
            u.frames,
            u.cpu_ms / u.frames.max(1) as f64,
            u.ui_pointer,
            u.world_pointer
        )
    });
    let edit = st.edit.as_ref().filter(|e| e.stats.frames > 0 || e.on).map_or(String::new(), |e| {
        format!(
            "
  edit: {} ({})",
            e.summary(),
            if e.on { "mode on" } else { "mode off" }
        )
    });
    // The F9 files of the run, when the script pressed it (`PLANET_REPRO_DIR` set).
    let repro = if st.repro_files > 0 {
        format!(
            "
  repro: {} F9 write(s) to {}, the camera path holds {} poses",
            st.repro_files,
            repro_dir().display(),
            st.path.len()
        )
    } else {
        String::new()
    };
    let globe = format!(
        "
  globe: {} frames of the overview, up to {} markers shown, {} flights started, {} landings",
        st.globe_stats.frames, st.globe_stats.markers_shown_max, st.globe_stats.flights, st.globe_stats.landings
    );
    format!(
        "{} frames drawn ({} lit, {} with the atmosphere, {} with water, {} debug, {} with an overlay ({} the mesh), {} coarse LOD, {} refused, {} stale-tile overlay, {} editing light), {} presented ({} lit, {} debug), {} exposure reading(s), {} view change(s), present mode {}, {}
  last title: {}",
        st.frames,
        st.counts.lit_frames,
        st.counts.atmosphere_frames,
        st.counts.water_frames,
        st.counts.debug_frames,
        st.counts.overlay_frames,
        st.counts.mesh_overlay_frames,
        st.counts.coarse_lod_frames,
        st.counts.coarse_lod_refusals,
        st.counts.stale_overlay_frames,
        st.counts.editing_light_frames,
        st.presented,
        st.counts.lit_presented,
        st.counts.debug_presented,
        st.counts.exposure_readings,
        st.views_seen.max(usize::from(st.viewer.view_index != 0)),
        st.surface.present_mode_name(),
        st.adapter_line,
        st.last_title
    ) + &ui
        + &edit
        + &globe
        + &repro
}

/// `planet editor`: open the window and run the flight until it is closed (or, for `--smoke`, for a few dozen frames).
pub fn run_window(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    crate::worldfield::check_options(o)?;
    if o.edit && o.no_ui {
        return Err("--edit needs the editor UI: drop --no-ui".into());
    }
    // The adapter policy: the application runs on a discrete GPU only; the software adapters are the test and CI mode.
    let policy = if o.smoke || std::env::var_os("PLANET_SWEEP").is_some() {
        o.adapter.unwrap_or_else(AdapterPolicy::from_env)
    } else {
        if o.adapter == Some(AdapterPolicy::Software) {
            return Err("--adapter software is the test and CI mode: the window runs on a discrete GPU (software adapters are used by `editor --smoke` and the `PLANET_SWEEP` run only)".into());
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
    let (prefill_done, prefill_total) = if o.store.is_some() {
        let levels = o.prefill_levels.unwrap_or(READINESS_PREFILL_LEVELS);
        world.prefill_present(o.floor_for(o.cells), levels, o.cells, o.finest_level_for(o.cells)).map(|(p, t)| (p as u64, t as u64))?
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
        ui: !o.no_ui,
        edit: o.edit,
        items: o.items.clone(),
        layout: match &o.editor_layout {
            Some(p) => Layout::from_toml(&std::fs::read_to_string(p).map_err(|e| format!("--editor-layout {}: {e}", p.display()))?)
                .map_err(|e| format!("--editor-layout {}: {e}", p.display()))?,
            None => Layout::default(),
        },
        smoke_frames: if o.given.contains("--frames") {
            o.frames as u64
        } else if o.smoke && o.edit {
            SMOKE_EDIT_FRAMES
        } else {
            SMOKE_FRAMES
        },
        vsync: !o.no_vsync,
        policy,
        gen: o.gen_workers(),
        initial_view,
        base_readiness: Readiness { bake_done, bake_total, prefill_done, prefill_total, view: ViewCoverage::default() },
        radius_m,
        water_level_m,
        draw: o.draw,
        upload: o.upload,
        look,
        atmosphere,
        liquids: world.definition().liquids().map_err(|e| format!("the liquid table of the definition: {e}"))?,
        markers: parse_markers(&o.markers)?,
    };
    writeln!(out, "planet editor: {} ({})", if o.smoke { "smoke test" } else { "viewer" }, world.describe()).map_err(err)?;
    writeln!(out, "keys: {KEYS_HELP}").map_err(err)?;
    if !o.no_ui {
        writeln!(out, "{EDIT_KEYS_HELP}").map_err(err)?;
    }
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
        field,
        lod: cfg.lod.clone(),
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

    // spec: VIEW-013
    #[test]
    fn markers_are_given_as_lat_lon_and_an_optional_radius_numbered_from_one() {
        let ms = parse_markers(&["48.1,11.6".to_string(), " 0 , -90 , 250.5 ".to_string(), "-33,151,10".to_string()]).unwrap();
        assert_eq!(ms.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(ms[0].radius_m, 1_000.0);
        assert_eq!(ms[1].radius_m, 250.5);
        for m in &ms {
            assert!((m.dir.length() - 1.0).abs() < 1e-12);
        }
        // 0 N, 90 W is the -y axis; 90 N would be +z.
        assert!((ms[1].dir.y + 1.0).abs() < 1e-12 && ms[1].dir.x.abs() < 1e-12);
        assert!((parse_markers(&["90,0".to_string()]).unwrap()[0].dir.z - 1.0).abs() < 1e-12);
        assert!(parse_markers(&[]).unwrap().is_empty());
        for bad in ["", "1", "1,2,3,4", "x,2", "91,0", "0,0,0", "0,0,-5", "nan,0", "0,inf"] {
            let e = parse_markers(&[bad.to_string()]).unwrap_err();
            assert!(e.contains("--marker"), "{bad:?}: {e}");
        }
    }

    /// Whether `help` names `label` as a word of its own (not as part of a longer word or number such as `F1` inside `F12`).
    fn names_key(help: &str, label: &str) -> bool {
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
        help.match_indices(label).any(|(i, _)| {
            let after = help[i + label.len()..].chars().next();
            let before = help[..i].chars().next_back();
            // A one-character label is a key where it stands apart: `8 raise`, `- flatten`, `Q/E`, not the 8 of `1280`.
            !(word(before) && label.chars().next().is_some_and(char::is_alphanumeric))
                && !(word(after) && label.chars().last().is_some_and(char::is_alphanumeric))
        })
    }

    // spec: VIEW-022
    #[test]
    fn the_help_text_of_the_editor_names_every_key_the_window_binds() {
        // Every key code the window can bind, with the label the help names it by; a key that binds and has no label here fails the test
        // (add it, and name it in `EDIT_KEYS_HELP`).
        let labels: &[(KeyCode, &str)] = &[
            (KeyCode::Digit1, "1"),
            (KeyCode::Digit2, "2"),
            (KeyCode::Digit3, "3"),
            (KeyCode::Digit4, "4"),
            (KeyCode::Digit5, "5"),
            (KeyCode::Digit6, "6"),
            (KeyCode::Digit7, "7"),
            (KeyCode::Digit8, "8"),
            (KeyCode::Digit9, "9"),
            (KeyCode::Digit0, "0"),
            (KeyCode::Minus, "-"),
            (KeyCode::Delete, "Delete"),
            (KeyCode::Backspace, "Backspace"),
            (KeyCode::Enter, "Enter"),
            (KeyCode::NumpadEnter, "Enter"),
            (KeyCode::F1, "F1"),
            (KeyCode::F3, "F3"),
            (KeyCode::F4, "F4"),
            (KeyCode::F6, "F6"),
            (KeyCode::F7, "F7"),
            (KeyCode::F9, "F9"),
            (KeyCode::Escape, "Esc"),
            (KeyCode::Tab, "Tab"),
            (KeyCode::KeyP, "P"),
            (KeyCode::Space, "Space"),
            (KeyCode::KeyR, "R"),
            (KeyCode::KeyV, "V"),
            (KeyCode::KeyT, "T"),
            (KeyCode::KeyC, "C"),
            (KeyCode::KeyL, "L"),
            (KeyCode::KeyG, "G"),
            (KeyCode::KeyH, "H"),
            (KeyCode::KeyW, "WASD"),
            (KeyCode::KeyA, "WASD"),
            (KeyCode::KeyS, "WASD"),
            (KeyCode::KeyD, "WASD"),
            (KeyCode::KeyQ, "WASD/QE"),
            (KeyCode::KeyE, "WASD/QE"),
            (KeyCode::ArrowLeft, "arrows"),
            (KeyCode::ArrowRight, "arrows"),
            (KeyCode::ArrowUp, "arrows"),
            (KeyCode::ArrowDown, "arrows"),
            (KeyCode::ShiftLeft, "Shift"),
            (KeyCode::ShiftRight, "Shift"),
        ];
        // The codes the window might bind: the labelled ones and every letter, digit and function key, so a new binding cannot hide.
        let others = [
            KeyCode::KeyB,
            KeyCode::KeyF,
            KeyCode::KeyI,
            KeyCode::KeyJ,
            KeyCode::KeyK,
            KeyCode::KeyM,
            KeyCode::KeyN,
            KeyCode::KeyO,
            KeyCode::KeyU,
            KeyCode::KeyX,
            KeyCode::KeyY,
            KeyCode::KeyZ,
            KeyCode::F2,
            KeyCode::F5,
            KeyCode::F8,
            KeyCode::F10,
            KeyCode::F11,
            KeyCode::F12,
            KeyCode::Equal,
            KeyCode::Slash,
            KeyCode::Insert,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
            KeyCode::Comma,
            KeyCode::Period,
        ];
        let bound = |code: KeyCode| {
            edit_key(code, None, false, false).is_some()
                || key_command(code).is_some()
                || set_held(&mut Held::default(), code, true)
                || code == KeyCode::Tab
        };
        for code in others {
            assert!(!bound(code), "{code:?} is bound but has no label in this test (add it, and name it in EDIT_KEYS_HELP)");
        }
        for (code, label) in labels {
            assert!(bound(*code), "{code:?} is labelled {label} but the window does not bind it");
            assert!(names_key(EDIT_KEYS_HELP, label), "EDIT_KEYS_HELP does not name {label} ({code:?}): {EDIT_KEYS_HELP}");
        }
        // The chords and the mouse, which have no key code of their own.
        for word in [
            "Ctrl+Z",
            "Ctrl+Y",
            "Ctrl+Shift+Z",
            "Shift+click",
            "Ctrl+click",
            "right mouse button",
            "?",
            "radius",
            "strength",
            "inspector",
            "flatten",
        ] {
            assert!(EDIT_KEYS_HELP.contains(word), "EDIT_KEYS_HELP does not name {word}");
        }
        assert!(!names_key("the 1280 pixels", "8") && names_key("8 raise", "8") && names_key("- flatten", "-"));
    }

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
            (KeyCode::F3, Command::CycleOverlay),
            (KeyCode::F4, Command::ToggleCoarseLod),
            (KeyCode::F6, Command::CycleGroundDetail),
            (KeyCode::F7, Command::ToggleScaleRefs),
            (KeyCode::KeyH, Command::ToggleEditingLight),
            (KeyCode::F9, Command::SaveRepro),
            (KeyCode::KeyG, Command::ToggleGlobe),
            (KeyCode::Escape, Command::Quit),
        ] {
            assert_eq!(key_command(code), Some(cmd), "{code:?}");
        }
        for code in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyX, KeyCode::Enter, KeyCode::F5] {
            assert_eq!(key_command(code), None, "{code:?} is not a command");
        }
        for ch in [
            "pause",
            "restart",
            "debug view",
            "vsync",
            "free camera",
            "WASD",
            "Esc",
            "lit",
            "atmosphere",
            "overlay",
            "coarse LOD",
            "ground detail",
            "globe overview",
        ] {
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

    // spec: VIEW-022
    #[test]
    fn the_keys_of_the_editor_mode_are_the_numbers_ctrl_z_and_y_delete_and_escape_by_the_letter_on_the_key() {
        use EditKey::*;
        let k = |code, logical: Option<&str>, ctrl, shift| edit_key(code, logical, ctrl, shift);
        assert_eq!(k(KeyCode::Digit1, None, false, false), Some(Tool(1)));
        assert_eq!(k(KeyCode::Digit4, None, false, false), Some(Tool(4)));
        assert_eq!(k(KeyCode::Delete, None, false, false), Some(Delete));
        // Backspace takes the last point of a line being drawn back and never deletes an edit; Delete does.
        assert_eq!(k(KeyCode::Backspace, None, false, false), Some(Back));
        assert_eq!(k(KeyCode::Escape, None, false, false), Some(Escape));
        // The line and region tools, finishing, and the key list (VIEW-031).
        assert_eq!(k(KeyCode::Digit5, None, false, false), Some(Tool(5)));
        assert_eq!(k(KeyCode::Digit7, None, false, false), Some(Tool(7)));
        // spec: VIEW-035
        assert_eq!(
            [KeyCode::Digit8, KeyCode::Digit9, KeyCode::Digit0, KeyCode::Minus].map(|c| k(c, None, false, false)),
            [Some(Tool(8)), Some(Tool(9)), Some(Tool(10)), Some(Tool(11))]
        );
        assert_eq!(k(KeyCode::Enter, None, false, false), Some(Finish));
        assert_eq!(k(KeyCode::NumpadEnter, None, false, false), Some(Finish));
        assert_eq!(k(KeyCode::F1, None, false, false), Some(Help));
        assert_eq!(k(KeyCode::Slash, Some("?"), false, true), Some(Help), "the key that types a question mark, on any layout");
        // QWERTY: the physical Z key is labelled z.
        assert_eq!(k(KeyCode::KeyZ, Some("z"), true, false), Some(Undo));
        assert_eq!(k(KeyCode::KeyZ, Some("Z"), true, true), Some(Redo));
        assert_eq!(k(KeyCode::KeyY, Some("y"), true, false), Some(Redo));
        // QWERTZ: the key labelled Z is the physical KeyY, and the key labelled Y is KeyZ.
        assert_eq!(k(KeyCode::KeyY, Some("z"), true, false), Some(Undo), "Ctrl+Z undoes on a German keyboard");
        assert_eq!(k(KeyCode::KeyZ, Some("y"), true, false), Some(Redo), "Ctrl+Y redoes on a German keyboard");
        // Without a character the QWERTY keys stand in; without Ctrl neither letter is a command.
        assert_eq!(k(KeyCode::KeyZ, None, true, false), Some(Undo));
        assert_eq!(k(KeyCode::KeyY, None, true, false), Some(Redo));
        assert_eq!(k(KeyCode::KeyZ, Some("z"), false, false), None);
        assert_eq!(k(KeyCode::KeyY, Some("y"), false, true), None);
        // The fly keys are not the editor's: W, S and the rest stay the camera's.
        for code in [
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::Space,
            KeyCode::KeyG,
            KeyCode::KeyC,
        ] {
            assert_eq!(k(code, None, false, false), None, "{code:?}");
        }
    }

    // spec: VIEW-022
    #[test]
    fn the_mouse_in_the_editor_mode_presses_in_the_world_releases_anywhere_and_looks_with_the_right_button() {
        use EditMouse::*;
        use ElementState::{Pressed, Released};
        let a = edit_mouse_action;
        // The left button: a press in the world edits, one over a panel or while the pointer is captured does nothing, any release releases.
        assert_eq!(a(MouseButton::Left, Pressed, true, false, false), Some(Press));
        assert_eq!(a(MouseButton::Left, Pressed, false, false, false), None, "over a panel");
        assert_eq!(a(MouseButton::Left, Pressed, true, true, false), None, "the pointer is captured: the cursor is stale");
        for (world, captured, holding) in [(true, false, true), (false, false, true), (true, true, false), (false, true, false)] {
            assert_eq!(a(MouseButton::Left, Released, world, captured, holding), Some(Release), "released wherever it lands");
        }
        // The right button: held, it looks around; it waits while the left button holds an edit; its release always ends the look.
        assert_eq!(a(MouseButton::Right, Pressed, true, false, false), Some(StartLook));
        assert_eq!(a(MouseButton::Right, Pressed, true, false, true), None, "a handle is held");
        assert_eq!(a(MouseButton::Right, Pressed, false, false, false), None, "over a panel");
        for holding in [false, true] {
            assert_eq!(a(MouseButton::Right, Released, false, true, holding), Some(EndLook));
        }
        // Other buttons are not the editor's.
        assert_eq!(a(MouseButton::Middle, Pressed, true, false, false), None);
        assert_eq!(a(MouseButton::Back, Released, true, false, false), None);
    }

    // spec: VIEW-022
    #[test]
    fn the_editor_mode_leaves_the_camera_and_the_overview_alone() {
        assert!(
            editor_mode_blocks(Command::ToggleCamera) && !editor_mode_blocks(Command::ToggleGlobe),
            "C is the editor mode's, G shows the overview"
        );
        for cmd in [
            Command::TogglePause,
            Command::Restart,
            Command::NextView,
            Command::ToggleLit,
            Command::ToggleVsync,
            Command::CycleOverlay,
            Command::ToggleCoarseLod,
            Command::CycleGroundDetail,
            Command::ToggleScaleRefs,
            Command::ToggleEditingLight,
            Command::SaveRepro,
        ] {
            assert!(!editor_mode_blocks(cmd), "{cmd:?}");
        }
        // The keys that reach them: C and G map to those commands, and Esc to quit (which the editor's Esc takes first).
        assert_eq!(key_command(KeyCode::KeyC), Some(Command::ToggleCamera));
        assert_eq!(key_command(KeyCode::KeyG), Some(Command::ToggleGlobe));
        // spec: VIEW-034
        // A key goes to the camera when no text field has the focus, and always while the pointer is captured for looking around.
        assert_eq!(
            [(true, false), (true, true), (false, true), (false, false)].map(|(world, captured)| key_goes_to_world(world, captured)),
            [true, true, true, false]
        );
        assert_eq!(edit_key(KeyCode::Escape, None, false, false), Some(EditKey::Escape));
    }

    // spec: VIEW-039
    #[test]
    fn a_press_winit_makes_up_on_a_focus_gain_is_no_key_press_but_a_release_always_counts() {
        assert!(key_press_counts(true, false), "the user's press");
        assert!(!key_press_counts(true, true), "Alt+Tab back in: Tab is still down, winit repeats its press");
        assert!(key_press_counts(false, true) && key_press_counts(false, false), "a release lets go of the key either way");
    }

    // spec: VIEW-022
    #[test]
    fn turning_the_mode_on_ends_the_overview_once_and_takes_the_free_camera_and_g_shows_the_overview_again() {
        // (end_overview, edit_on, globe, free) -> (end the overview, take the free camera)
        assert_eq!(mode_camera_steps(true, true, true, false), (true, true), "Tab with the overview up: it ends and the free camera comes");
        assert_eq!(mode_camera_steps(true, true, true, true), (true, false), "already free: only the overview ends");
        assert_eq!(mode_camera_steps(true, true, false, false), (false, true), "no overview to end");
        // Later frames: no request any more. The overview shown with G stays and the camera is its own.
        assert_eq!(mode_camera_steps(false, true, true, false), (false, false), "G in the mode: the overview stays");
        assert_eq!(mode_camera_steps(false, true, true, true), (false, false));
        assert_eq!(mode_camera_steps(false, true, false, false), (false, true), "the overview gone: the free camera again");
        assert_eq!(mode_camera_steps(false, true, false, true), (false, false));
        // The mode off leaves the camera alone.
        for globe in [false, true] {
            for free in [false, true] {
                assert_eq!(mode_camera_steps(false, false, globe, free), (false, false));
            }
        }
    }
}
