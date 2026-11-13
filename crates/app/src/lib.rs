//! The single binary's run modes (CON-01): `editor` (interactive viewer), `generate` (headless CLI)
//! and `test-render` (headless harness). It wires the other crates together: the window and the editor mode (`window`, `editwin`, `panels`), the world's field chain (`worldfield`), the stores and bakes, and the perf and stress scenes.

pub mod cameraground;
pub mod coupledworld;
pub mod crash;
pub mod editperf;
pub mod editscale;
pub mod editwin;
pub mod envelopeperf;
pub mod fixturebase;
pub mod gate;
pub mod globalperf;
pub mod horizonstep;
pub mod horizonworld;
/// The default cells a face edge of the level-1 preview's H0 grid (`--preview-l1`): about a second a preview.
pub const PREVIEW_GRID_DEFAULT: usize = 768;

pub mod hoverpick;
pub mod litperf;
pub mod lookload;
pub mod manifest;
pub mod panels;
pub mod pathreplay;
pub mod pipelined;
mod plancheck;
pub mod prefill;
pub mod preflightperf;
pub mod previewjob;
pub mod reproworld;
pub mod reserveload;
pub mod riverperf;
pub mod rivers;
pub mod shadowrig;
pub mod sheet;
pub mod skeleton;
pub mod stall;
pub mod stills;
pub mod streamperf;
pub mod sweep;
pub mod terrainscore;
pub mod ui;
pub mod window;
pub mod worldfield;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use planet_perf::{AdapterFacts, DeviceKind, Protocol, RunReport};
use planet_render::{AdapterPolicy, GpuContext};

pub const USAGE: &str = "usage: planet <mode> [options]
modes:
  editor        the viewer window (VIEW-001..VIEW-007): the scripted orbit-to-1 m flight on the pipelined loop; keys P/Space pause, R restart, L lit (with the atmosphere) or debug path, V debug view, T vsync, C flight or free camera (WASD move, Q/E down/up, arrows look, Shift boost), Esc quit; the window opens on the lit path with the atmosphere, the look file drawn (sun, adaptive exposure); the title shows frame rate, CPU and GPU time, the path (or the debug view), on the lit path the exposure, and the readiness report; runs on a discrete GPU only (an integrated GPU, a software adapter or an adapter missing a feature or limit is refused with the reason); [--world F --global-res N | --store DIR] [--look F] [--size WxH] [--view NAME] [--cells N --tau PX --finest-level N] [--no-vsync] [--erosion on|off] (draw the erosion layer of a bake over the stored world, off by default, VIEW-062: --store holds the world, the layer is read from --erosion-store DIR, the bake's output folder, or the same store; the layer's manifest gives the bake's shape, versions, field source and change range, and --erosion-face-res N and --erosion-res N replace its pass-1 and pass-2 cells (12288 and 98304 for a layer without a manifest); the run refuses a store that holds the legacy generator's bake unless --erosion-unchecked, and a manifest of another version or field source; the title and the log say what is on and name the field source, VIEW-065) [--channels on|off] (fill the tiles' driver block, the soil depth and the moisture of the ground materials, from the layer's baked channels, the deposited depth and the drainage area; off by default, needs --erosion on, and the run refuses a layer that holds no channels, VIEW-066) [--class-source | --legacy-source] (which bake to open when the store holds both, STRM-073; the store's recorded source otherwise) [--no-shadow-march] (turn the clipmap shadow off: the march shades the terrain and the water by default, REND-051; J shows its visibility; --shadow-march is accepted and does nothing) [--no-ui] (the editor UI, an egui layer drawn after the tone map, VIEW-011) [--edit] (start in the editor mode, VIEW-022; with --site it turns on where the scripted descent over the site ends: Tab toggles it; the panels, stamps by 2-4 and a click, handles on the planet, Ctrl+Z / Ctrl+Y) [--items DIR] (the edits as one TOML file each in DIR, loaded at the start) [--editor-layout F] (the panel layout, VIEW-018) [--marker LAT,LON[,RADIUS_M]]... (item markers of the globe overview, G, VIEW-013) [--site LAT,LON] (the scripted flight descends over this point; without it the world definition's `[world] site = [lat, lon]`, else 20,30 where that is land, else the first shore of the walk from there, GEN-108); --smoke opens the window, drives the keys for a few dozen frames and closes it (test mode: PLANET_ADAPTER or --adapter, software by default; exits with `no display` where there is none); --no-window starts and exits without a window; --offscreen runs the skeleton without a window
  generate      headless generator CLI: --tile F,L,X,Y [--seed N --res N --out height.png --face-net net.png]; --manifest <file> writes the determinism manifest (GEN-003); --cache-dir <dir> --region F,L [--res N] writes a tile face region into a tile store; --verify-cache <dir> [--sample N] regenerates stored tiles and compares (both take --world <definition.toml> or --seed N; --verify-cache fails when nothing could be compared, --allow-stale accepts that); --global-dir <dir> [--global-res N, default 256] bakes the global-coarse field (continents, elevation, climate) of N cells per face edge into a tile store as blocks in the canonical layout (blocks of min(N, 1024) cells) together with its ocean-connectivity channel (one bit per vertex; the area threshold is the definition's `body.0.sea.min_area_fraction`) and prints the time per sample and the channel's cost; --class-source (with --global-dir or --verify-global) samples the class source's surface instead of the legacy generator's and keys the store by it (GEN-115, STRM-073); --verify-global <dir> [--sample N] regenerates stored blocks and compares; --global-products <dir> [--global-res N] writes cube nets and maps of an in-memory bake (these take --world or --seed and --threads N); --coupled-run <dir> --global-res N (required) makes the coupled run of the bake stored in <dir> (or finds it there) and stores it, with a progress line at most every 10 s during the reduction of the bake and the wall time (STRM-051); --coarse-horizon <dir> --global-res N (required) [--threads N] makes the coarse horizon (far-field shadows, STRM-045) from the coupled run stored beside the bake in <dir> and stores it there, with a progress line and the wall time (the coupled run is made and stored by the viewer or `test-render --scene coupled`); --plan-check <dir> --global-res N (required) plans the standard flight at cells 32 and viewport / tau 240 over the stored bake with the terrain and the eroded field and fails if a view is rejected (M3 acceptance); --prefill-dir <dir> --global-res N [--prefill-levels L] [--floor-level F] writes the coarse tiles of levels F..=L (F the render floor, default 3; L default 4; an L below F writes nothing and says so; the tiles of cells 32, finest level 22, compact form, as the streamer generates them) of the stored bake into its tile pack, skipping what is there; --global-dir pre-fills the same way after the bake (--prefill-levels 0 disables; --floor-level takes both); --smoke starts and exits
  test-render   headless render harness
    --scene hello-triangle|tiles|terrain   render a scene (default hello-triangle)
    --script N|name          scripted terrain camera 0..9 or its name (default 3 = aerial-20km)
    --cells N --tau PX --frames N   terrain perf workload parameters (defaults 32, 6, 300)
    --floor-level N          the render floor (LOD-011, default 3): levels below it are not drawn, its 384 tiles at level 3 are the roots; test-render's terrain, streaming and lit scenes and generate's pre-fill take it, the viewer takes no flag; above --finest-level it is refused (the effective floor is the lower of the two)
    --finest-level N         the LOD cap, the finest geometry level of the single views, --stream, --pipelined, --flight and --perf (default 27 - ceil(log2(cells)): 22 at cells 32, 21 at 64, equal vertex spacing of about 7.5 cm; 26 reproduces the SPIKE-07 reference)
    --perf-smoke             run the terrain perf path briefly on any adapter (labelled, never a measurement)
    --flight N --out-dir DIR [--size WxH]   render the scripted orbit-to-1 m flight (TEST-013) as DIR/frame_0000.png... plus DIR/flight.txt and DIR/contact_sheet.png
                             with the production LOD defaults (override with --cells, --tau; --view applies; default size 1280x720,
                             small sizes can fail LOD validation) for the real-GPU jitter review
    --stream                 generate terrain tiles asynchronously through the streaming path (same image as the default)
    --pipelined [--then SCRIPT]   draw the terrain view through the pipelined loop (plans from a worker, a cover plan until the first one arrives,
                             the GPU never waited for; STRM-016): from a cold start, then (with --then) after a jump to the other camera; the
                             image is the one frame the loop settles on, equal to the default path's; --out-dir DIR also writes pipelined.txt
                             (per-frame kind, counts and the time columns plan_ms scheduler_ms upload_ms record_ms gpu_ms wall_ms)
    --lockstep               with --pipelined: wait for the plan worker and the generation after every iteration, so the frames and the tiles each
                             uploads do not depend on thread timing (a test mode; slower than the free-running loop)
    --view face|tile-id|level|morph|height|normals|depth|relief|morph-height   debug view (tiles scene: face, tile-id, height; a flight defaults to relief)
    --tile FACE,LEVEL,X,Y    tile to draw (repeatable; default: a tile and its east neighbour)
    --seed N --res N         generator seed and tile resolution (cells, power of two)
    --look <look.toml>       the presentation parameters per body (LOOK-001..005; exposure, scene units, histogram, tone curve; the file is outside the world definition and every cache key, so it never changes a tile): default the `<world stem>.look.toml` beside --world when it exists, else the Earth defaults; a bad file is refused naming the key and the value
    --world <definition.toml> [--global-res N]   terrain scene (a view, --stream, --bundle, --flight, --perf, --perf-smoke): the world's height field (world.height_field, default the eroded field) over an in-memory bake of N cells per face edge (default 256, at most 1024); without it a view, stream or bundle is the seed-only reference field and --perf is the default world of --seed; a bundle records the world, --repro reads it back
    --bundle <file>          write a repro bundle of the render
    --repro <file> [--store DIR]   replay a repro bundle (see cargo xtask repro); a bundle of a bake beyond the in-memory limit replays from its stored bake (read-only)
    --out <file.png>         write the image
    --adapter software|hardware   adapter policy (default: PLANET_ADAPTER or software)
    --draw per-draw|indirect   how the terrain renderer submits draws (REND-015): per-draw is the reference loop, indirect one multi-draw per buffer run from a CPU-built argument buffer (default indirect where the adapter supports it, else per-draw)
    --upload per-slot|batched   how a frame's tile writes reach the queue (REND-016): batched (default) writes each run of adjacent slots with one call, per-slot is the reference (one or two calls per tile); the pool contents are identical
    --gen-workers N          threads of the tile generation pool and so jobs running at once in the pipelined loop and the streaming perf scenes (STRM-020; default: the terrain share, BUDG-003; the rig keeps the pool inside that share; the plan worker's threads are separate)
    --gen-reserve N          size the generation pool as the hardware threads less N instead of the terrain share (STRM-020; ignored when --gen-workers is given)
    --gen-ahead N            jobs queued behind the running ones so a worker that finishes between two frames takes the next at once (STRM-020; default: the worker count; 0: refill at frame boundaries only)
    --gen-priority below-normal|normal   OS priority of the generation workers (STRM-020, ADR 0046; default below-normal: the frame thread wins a core both want; no result depends on it)
    --lit                    the streaming perf scenes on the lit path with the atmosphere and water (TEST-025): same scripts, same verdicts, logs named <scene>-lit
    --views a,b,c            only these views of the lit perf scenes (ground-1m, aerial-20km, coast-2km, coast-3m, orbit, coast-glide, coast-descent); a partial log is not a window
    --scene capture-stills   one PNG per pose of --poses FILE (TEST-048) in --out-dir, over the base terrain of a world (a headless scene; no viewer starts it); needs --store DIR or --allow-memory-bake, takes --size WxH, --views (pose names) and --note TEXT (a known defect and its cause, drawn above every still's pose label, TEST-128); never a measurement
    --scene capture-flights  the two coastal flights as 1440p PNG sequences (shore_filter at the look default, 0, and then 1) in --out-dir, never a measurement (TEST-027; `cargo xtask capture flights` also makes the videos); needs --store DIR or --allow-memory-bake, takes --size WxH, --flight-frames N
    --flight-frames N        frames of each coastal flight of the lit perf scenes (default 240; --scene lit, TEST-025)
    --perf                   performance-harness skeleton (needs --adapter hardware; the readiness gate decides, ADR 0020)
    --calibrate              measure the CPU calibration in a smoke run too (TEST-039; a measured run always does): proves the logging on any adapter, the log is still no measurement
    --machine-ready          owner override of a failed readiness gate (ADR 0020); recorded in the log
    --reserve                run the streaming scenes with the synthetic loads of the reserved shares on (BUDG-011, BUDG-012) and print the reserve report
    --loaded                 loaded mode (ADR 0087, BUILD-030): the load of the machine is recorded, not refused; the hard conditions still refuse
    --smoke                  start and exit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Generate,
    TestRender,
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub smoke: bool,
    pub scene: String,
    pub out: Option<PathBuf>,
    pub adapter: Option<AdapterPolicy>,
    /// `--draw per-draw|indirect`: how the terrain renderer submits draws (REND-015); default indirect where supported.
    pub draw: Option<planet_render::DrawMode>,
    /// `--upload per-slot|batched`: how a frame's tile writes reach the queue (REND-016); default batched, per-slot the reference.
    pub upload: Option<planet_render::UploadMode>,
    /// `--gen-workers N` and `--gen-ahead N`: the tile generation pool (STRM-020); default the terrain share (BUDG-003) and as many queued.
    pub gen_workers: Option<usize>,
    pub gen_ahead: Option<usize>,
    /// `--gen-reserve N`: size the pool as the machine's threads less N instead of the terrain share (none when absent).
    pub gen_reserve: Option<usize>,
    /// `--gen-priority below-normal|normal`: the priority of the generation workers (STRM-020, ADR 0046); below-normal when absent.
    pub gen_priority: Option<planet_streaming::Priority>,
    pub perf: bool,
    pub machine_ready: bool,
    /// Loaded mode (BUILD-030): the readiness gate records the load of the machine instead of refusing on it.
    pub loaded: bool,
    /// The streaming scenes run beside the synthetic loads of the reserved shares (BUDG-012).
    pub reserve: bool,
    pub view: String,
    pub tiles: Vec<String>,
    pub seed: u64,
    pub res: u32,
    pub bundle_out: Option<PathBuf>,
    pub repro: Option<PathBuf>,
    /// `--replay-path F`: with `--repro`, drive the bundle's terrain through the camera path file F (VIEW-026); `--replay-every N` renders every Nth pose.
    pub replay_path: Option<PathBuf>,
    pub replay_every: usize,
    pub face_net: Option<PathBuf>,
    pub manifest: Option<PathBuf>,
    /// A definition file (TOML, WDEF-002) instead of the default single planet with `--seed`.
    pub world: Option<PathBuf>,
    /// `--look F`: the look file (LOOK-001); without it the file beside `--world`, else the Earth defaults (LOOK-005).
    pub look: Option<PathBuf>,
    /// `generate`: write the tiles of `--region` into the tile store in this directory (ADR 0019).
    pub cache_dir: Option<PathBuf>,
    /// `generate --cache-dir`: all tiles of one face at one level, `FACE,LEVEL`.
    pub region: Option<(u8, u8)>,
    /// `generate`: regenerate a sample of the tile store in this directory and compare (STRM-004).
    pub verify_cache: Option<PathBuf>,
    /// `generate`: compact the store in this directory (STRM-013), dropping records the definition no longer produces.
    pub compact_cache: Option<PathBuf>,
    /// `--compact-cache`: keep at most this many bytes of records (the oldest-written packs are evicted first, ADR 0029).
    pub cap_bytes: Option<u64>,
    /// `--verify-cache`: how many evenly spaced entries to check; 0 is all.
    pub sample: usize,
    /// `--verify-cache`: entries that are all stale are expected (the definition changed on purpose), not a failure.
    pub allow_stale: bool,
    /// `generate --global-dir` and `--verify-global`: sample the class source's surface (GEN-115) instead of the legacy generator's; the store's keys differ (STRM-073).
    pub class_source: bool,
    /// `--legacy-source`: the store's legacy bake when it holds both (STRM-073).
    pub legacy_source: bool,
    /// `generate`: bake the global-coarse field into a tile store in this directory as blocks (STRM-012).
    pub global_dir: Option<PathBuf>,
    /// `generate`: regenerate a sample of the global bake stored in this directory and compare.
    pub verify_global: Option<PathBuf>,
    /// `generate`: plan the standard flight at production parameters over the stored global bake in this directory.
    pub plan_check: Option<PathBuf>,
    /// `generate --coarse-horizon DIR`: make the coarse horizon of the store `DIR`'s stored coupled run (STRM-045).
    pub coarse_horizon: Option<PathBuf>,
    /// `--radii-m`: the reaches of the hills the `edit-latency` scene puts, in metres separated by commas.
    pub radii_m: Option<String>,
    /// `--altitudes-m`: cameras at these altitudes (metres separated by commas) in the `edit-latency` and envelope scenes; none: the scene's own.
    pub altitudes_m: Option<String>,
    /// `--horizon-threads N`: the scoped threads one horizon bake uses in the `edit-latency` scene (default: the cores, at most 4).
    pub horizon_threads: Option<usize>,
    /// `--repeats N`: the runs of every case of the `edit-latency` scene (default 3).
    pub repeats: Option<usize>,
    /// `--no-horizon-jobs`: the `edit-latency` scene without the horizon jobs (the edit's tile refresh alone).
    pub no_horizon_jobs: bool,
    /// `perf coupled --store DIR --remake-into OUT`: time a fresh make of the coupled run from the stored bake, written to `OUT` under `target/`, never into the store (GEN-042).
    pub remake_into: Option<PathBuf>,
    /// `generate --coupled-run DIR`: make (or find) the coupled run of the bake stored in `DIR` (STRM-051).
    pub coupled_run: Option<PathBuf>,
    /// `generate`: write the coarse tiles (levels `F..=--prefill-levels`, F the render floor) into the pack of the stored bake in this directory (STRM-021).
    pub prefill_dir: Option<PathBuf>,
    /// `--global-dir` and `--prefill-dir`: how many levels of coarse tiles the pre-fill writes, 0 for none (default 4, ADR 0045 point 3).
    pub prefill_levels: Option<u8>,
    /// `generate`: write the data products (cube nets and maps) of an in-memory bake into this directory.
    pub global_products: Option<PathBuf>,
    /// `generate --lithology-preview DIR` (GEN-122): the planet preview maps and cross-section of the lithology field (ADR 0102) into DIR.
    pub lithology_preview: Option<PathBuf>,
    /// `generate --uplift-preview DIR` (GEN-125): the planet preview maps and section of the uplift field (GEN-123) into DIR.
    pub uplift_preview: Option<PathBuf>,
    /// `generate --climate-preview DIR` (GEN-130): the planet preview maps and numbers of the orographic climate (GEN-117) into DIR; `--fine-lattice N` adds the windward and lee ratios on a lattice of N cells a face edge.
    pub climate_preview: Option<PathBuf>,
    /// `generate --arid-preview DIR` (GEN-128): the class map with the arid rules, their weights, the shares against Earth's and the scorecard sites' classes into DIR.
    pub arid_preview: Option<PathBuf>,
    /// `generate --shelf-preview DIR` (GEN-118): the bathymetry map, a passive and an active coast region and the shelf numbers against Earth's into DIR.
    pub shelf_preview: Option<PathBuf>,
    /// `generate --ice-preview DIR` (GEN-134): the ice-extent and equilibrium-line maps, the land share under ice against the glacial maximum of Earth and the glaciated scorecard sites into DIR.
    pub ice_preview: Option<PathBuf>,
    pub fine_lattice: Option<usize>,
    /// `--km N`: kilometres a pixel of the lithology and uplift previews' maps (16 by default; 8 to 500).
    pub preview_km: f64,
    /// `--global-dir` and `--global-products`: cells per face edge of the bake (a power of two; blocks are stored in the canonical layout, ADR 0027).
    pub global_res: u32,
    /// `--global-dir`: worker threads (0 = all cores).
    pub threads: usize,
    pub offscreen: bool,
    pub script: String,
    pub stream: bool,
    /// `--pipelined`: the terrain view through the pipelined loop (STRM-016); `--then SCRIPT` jumps to another camera afterwards.
    pub pipelined: bool,
    pub then: Option<String>,
    /// `--lockstep` (with `--pipelined`): after every iteration wait until the plan worker and the generation are idle, so the
    /// frames drawn and the tiles each one uploads are a function of the script alone, not of thread timing (TEST-012).
    pub lockstep: bool,
    pub cells: u32,
    /// `--finest-level N`: the LOD cap (`LodParams::max_level`); none = the production default for the tile size (LOD-010).
    pub finest_level: Option<u8>,
    /// `--floor-level N`: the render floor (`LodParams::min_level`, LOD-011); none = the production default
    /// ([`planet_frame::DEFAULT_RENDER_FLOOR`], 3). Taken by `test-render` and `generate` (the pre-fill), never by the viewer.
    pub floor_level: Option<u8>,
    pub tau: f64,
    pub frames: usize,
    pub perf_smoke: bool,
    /// `--settle-ms N`: the refresh settle window of the `edit-latency` scene in milliseconds (ADR 0072, STRM-031); the default is the editor's.
    pub settle_ms: u64,
    /// `--share-bound F`: the coarse share of the screen over a window the `envelope` scene accepts (TEST-047); 1 % by default.
    pub share_bound: f64,
    /// `--steady-window-s S`: the measured window of the `envelope` scene's steady-flight trial after its run-in (TEST-057); 60 s by default, 12 s is the quick single-edge trial.
    pub steady_window_s: f64,
    /// `--only-steady`: the `envelope` scene runs only the steady-flight curve (no cold curve, no horizon-off comparison), over the `--altitudes-m` it is given (TEST-057).
    pub only_steady: bool,
    /// `--places K`: the start places the steady-flight search runs at per altitude, each with one place for all its trials (default 2; the log gives each and the lowest speed is the figure).
    pub places: usize,
    /// `--lit`: the streaming perf scenes draw the lit path with the atmosphere and the water (TEST-025).
    pub lit: bool,
    /// `--flight-frames N`: frames of each coastal flight of the lit perf scenes.
    pub flight_frames: usize,
    /// `--views a,b,c`: only these views of the lit perf scenes.
    pub views: Option<Vec<String>>,
    /// `--poses FILE`: the poses of the scene `capture-stills` (TEST-048).
    pub poses: Option<PathBuf>,
    /// `--note TEXT`: a known defect and its cause, drawn above the pose label of every still of `capture-stills` (TEST-128).
    pub note: Option<String>,
    /// `--commit DIR`: the commit of the new chain in DIR over the production base computed in memory, for the viewer and `capture-stills` (VIEW-069).
    pub commit: Option<PathBuf>,
    /// `--no-ui`: the editor window without the UI (VIEW-011).
    pub no_ui: bool,
    /// `editor --edit`: start in the editor mode (VIEW-022); Tab toggles it in the window.
    pub edit: bool,
    /// `editor --items DIR`: the directory the edits are written to and loaded from (one TOML file per edit, AUTH-001); without it they live in memory.
    pub items: Option<PathBuf>,
    /// `editor --editor-layout F`: the panel layout file (VIEW-018).
    pub editor_layout: Option<PathBuf>,
    /// `--marker LAT,LON[,RADIUS_M]` (repeatable): an item marker for the globe overview (VIEW-013).
    pub markers: Vec<String>,
    /// `--site LAT,LON`: where the scripted flight of the viewer descends, in degrees.
    pub site: Option<(f64, f64)>,
    /// `--calibrate`: measure the CPU calibration in a smoke run too (TEST-039).
    pub calibrate: bool,
    /// `--store DIR`: the stored bake the streaming perf scenes read (the production source, GEN-014); `--allow-memory-bake` runs them over the in-memory bake of `--global-res` cells instead, labelled and not valid for streaming budgets.
    pub store: Option<PathBuf>,
    pub allow_memory_bake: bool,
    pub flight: Option<usize>,
    pub out_dir: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    /// `editor --no-vsync`: start with vsync off (the `T` key toggles it).
    pub no_vsync: bool,
    /// The clipmap shadow pass (ADR 0085, REND-050) runs with the lit frames and shades the terrain and the water (REND-051); on by default,
    /// `editor --no-shadow-march` turns it off (`--shadow-march` is accepted and sets the default). J's shadow view shows its visibility.
    pub shadow_march: bool,
    /// `editor --erosion on|off` (VIEW-062): draw the erosion layer of a bake over the stored world (GEN-112); off by default.
    pub erosion: bool,
    /// `editor --channels on|off` (VIEW-066, GEN-119): fill the tiles' driver block from the layer's baked channels; off by default, needs `--erosion on`.
    pub channels: bool,
    /// `editor --preview-l1 [--preview-grid N]` (VIEW-068): the level-1 preview of the edit being made, drawn over the planet as a debug view (CON-18); the cells a face edge of its H0 grid. Off by default.
    pub preview_l1: Option<usize>,
    /// `editor --erosion-store DIR`: the folder of the layer (`bake_layer`'s output folder or its `tiles/`); the world's `--store` when not given.
    pub erosion_store: Option<PathBuf>,
    /// `editor --erosion-face-res N` and `--erosion-res N`: the bake's `--face-res` and `--planet-res` (the layer's keys follow them; GEN-100's 12,288 and 98,304 by default).
    pub erosion_face_res: Option<usize>,
    pub erosion_res: Option<usize>,
    /// `editor --erosion-unchecked`: draw the layer although the store does not say it was baked from the class source (a test of the plumbing, not a look).
    pub erosion_unchecked: bool,
    /// `editor --no-window`: do not open the window (the start check of CON-01 on a machine without a display).
    pub no_window: bool,
    /// Every option given on the command line, so modes can reject options they would otherwise ignore.
    pub given: std::collections::BTreeSet<String>,
}

impl Options {
    /// Which field source of a store to read when it holds both (`--class-source` or `--legacy-source`, STRM-073), or none to let the store decide.
    pub fn field_source_choice(&self) -> Result<Option<planet_cache::global_store::FieldSource>, String> {
        match (self.class_source, self.legacy_source) {
            (true, true) => Err("--class-source and --legacy-source exclude each other".into()),
            (true, false) => Ok(Some(planet_cache::global_store::FieldSource::Class)),
            (false, true) => Ok(Some(planet_cache::global_store::FieldSource::Legacy)),
            (false, false) => Ok(None),
        }
    }

    /// The tile generation pool of the pipelined loop and the streaming scenes (STRM-020): `--gen-workers` threads, else the machine's
    /// hardware threads less `--gen-reserve` when given, else the terrain share (BUDG-003); with `--gen-ahead` jobs queued behind them
    /// (default as many).
    pub fn gen_workers(&self) -> planet_streaming::GenWorkers {
        let g = match (self.gen_workers, self.gen_reserve) {
            (Some(n), _) => planet_streaming::GenWorkers::new(n),
            (None, Some(reserve)) => planet_streaming::GenWorkers::machine_reserving(reserve),
            (None, None) => planet_streaming::GenWorkers::machine(),
        };
        let g = self.gen_priority.map_or(g, |p| g.with_priority(p));
        self.gen_ahead.map_or(g, |a| g.with_ahead(a))
    }

    /// The reserve behind the pool size, for the perf log: `None` when `--gen-workers` fixed the size or the terrain share set it
    /// (no `--gen-reserve`), so the reserve decided nothing.
    pub fn gen_reserve_used(&self) -> Option<usize> {
        if self.gen_workers.is_some() {
            None
        } else {
            self.gen_reserve
        }
    }

    /// The finest geometry level of a run with tiles of `cells` cells: `--finest-level`, else the production default
    /// (`planet_frame::production_finest_level`: 22 at cells 32, 21 at 64). The one place the apps take the LOD cap from (LOD-010).
    pub fn finest_level_for(&self, cells: u32) -> u8 {
        self.finest_level.unwrap_or_else(|| planet_frame::production_finest_level(cells))
    }

    /// The render floor asked for (`LodParams::min_level`): `--floor-level`, else the production default. The effective floor of a
    /// run is the lower of this and the finest level ([`Options::floor_for`], LOD-011).
    pub fn min_level(&self) -> u8 {
        self.floor_level.unwrap_or(planet_frame::DEFAULT_RENDER_FLOOR)
    }

    /// The effective render floor of a run with tiles of `cells` cells: `min(min_level, finest level)` (LOD-011), the level whose
    /// tiles are the roots and the first level the pre-fill writes.
    pub fn floor_for(&self, cells: u32) -> u8 {
        planet_frame::effective_floor(self.min_level(), self.finest_level_for(cells))
    }

    /// An explicit `--floor-level` above the finest level is refused, naming both (LOD-011): checked before any work.
    pub fn check_floor(&self) -> Result<(), String> {
        match self.floor_level {
            Some(f) => planet_frame::check_explicit_floor(f, self.finest_level_for(self.cells)),
            None => Ok(()),
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut it = args.iter();
    let mode = match it.next().map(String::as_str) {
        Some("editor") | None => Mode::Editor,
        Some("generate") => Mode::Generate,
        Some("test-render") => Mode::TestRender,
        Some(other) => return Err(format!("unknown mode '{other}'\n{USAGE}")),
    };
    let mut o = Options {
        mode,
        smoke: false,
        scene: "hello-triangle".into(),
        out: None,
        adapter: None,
        draw: None,
        upload: None,
        gen_workers: None,
        gen_priority: None,
        gen_ahead: None,
        gen_reserve: None,
        perf: false,
        machine_ready: false,
        loaded: false,
        reserve: false,
        view: "face".into(),
        tiles: vec![],
        seed: 1,
        res: 16,
        bundle_out: None,
        repro: None,
        replay_path: None,
        replay_every: 0,
        face_net: None,
        manifest: None,
        world: None,
        look: None,
        cache_dir: None,
        region: None,
        verify_cache: None,
        compact_cache: None,
        cap_bytes: None,
        sample: 0,
        allow_stale: false,
        class_source: false,
        legacy_source: false,
        global_dir: None,
        verify_global: None,
        plan_check: None,
        coarse_horizon: None,
        radii_m: None,
        altitudes_m: None,
        horizon_threads: None,
        repeats: None,
        no_horizon_jobs: false,
        coupled_run: None,
        remake_into: None,
        prefill_dir: None,
        prefill_levels: None,
        global_products: None,
        lithology_preview: None,
        uplift_preview: None,
        climate_preview: None,
        arid_preview: None,
        shelf_preview: None,
        ice_preview: None,
        fine_lattice: None,
        preview_km: 16.0,
        global_res: 256,
        threads: 0,
        offscreen: false,
        script: "3".into(),
        stream: false,
        pipelined: false,
        lockstep: false,
        then: None,
        cells: 32,
        finest_level: None,
        floor_level: None,
        tau: 6.0,
        frames: 300,
        perf_smoke: false,
        settle_ms: editwin::REFRESH_SETTLE_MS,
        share_bound: envelopeperf::DEFAULT_BOUND,
        steady_window_s: envelopeperf::STEADY_WINDOW_S,
        only_steady: false,
        places: envelopeperf::STEADY_PLACES,
        lit: false,
        flight_frames: 240,
        views: None,
        poses: None,
        note: None,
        commit: None,
        no_ui: false,
        edit: false,
        items: None,
        editor_layout: None,
        markers: Vec::new(),
        site: None,
        calibrate: false,
        store: None,
        allow_memory_bake: false,
        flight: None,
        out_dir: None,
        size: None,
        no_vsync: false,
        shadow_march: true,
        erosion: false,
        erosion_store: None,
        erosion_face_res: None,
        erosion_res: None,
        erosion_unchecked: false,
        channels: false,
        preview_l1: None,
        no_window: false,
        given: Default::default(),
    };
    while let Some(a) = it.next() {
        o.given.insert(a.clone());
        match a.as_str() {
            "--smoke" => o.smoke = true,
            "--no-vsync" => o.no_vsync = true,
            "--shadow-march" => o.shadow_march = true,
            "--no-shadow-march" => o.shadow_march = false,
            "--erosion" => {
                o.erosion = match it.next().map(String::as_str) {
                    Some("on") => true,
                    Some("off") => false,
                    other => return Err(format!("--erosion needs on or off, got {}", other.unwrap_or("nothing"))),
                }
            }
            "--erosion-store" => {
                o.erosion_store = Some(PathBuf::from(it.next().ok_or("--erosion-store needs the folder of the erosion layer")?))
            }
            "--erosion-face-res" => {
                o.erosion_face_res = Some(
                    it.next().and_then(|v| v.parse().ok()).ok_or("--erosion-face-res needs a number of cells (the bake's --face-res)")?,
                )
            }
            "--erosion-res" => {
                o.erosion_res =
                    Some(it.next().and_then(|v| v.parse().ok()).ok_or("--erosion-res needs a number of cells (the bake's --planet-res)")?)
            }
            "--erosion-unchecked" => o.erosion_unchecked = true,
            "--channels" => {
                o.channels = match it.next().map(String::as_str) {
                    Some("on") => true,
                    Some("off") => false,
                    other => return Err(format!("--channels needs on or off, got {}", other.unwrap_or("nothing"))),
                }
            }
            "--preview-l1" => o.preview_l1 = Some(o.preview_l1.unwrap_or(PREVIEW_GRID_DEFAULT)),
            "--preview-grid" => {
                let n: usize = it.next().and_then(|v| v.parse().ok()).ok_or("--preview-grid needs a number of cells a face edge")?;
                if !(8..=3072).contains(&n) {
                    return Err(format!("--preview-grid is 8 to 3072 cells a face edge, got {n}"));
                }
                o.preview_l1 = Some(n);
            }
            "--no-window" => o.no_window = true,
            "--perf" => o.perf = true,
            "--offscreen" => o.offscreen = true,
            "--stream" => o.stream = true,
            "--pipelined" => o.pipelined = true,
            "--lockstep" => o.lockstep = true,
            "--then" => o.then = Some(it.next().ok_or("--then needs a script index or name")?.clone()),
            "--perf-smoke" => o.perf_smoke = true,
            "--settle-ms" => {
                o.settle_ms = it.next().ok_or("--settle-ms needs a value")?.parse().map_err(|e| format!("--settle-ms: {e}"))?
            }
            "--places" => {
                o.places = it.next().ok_or("--places needs a value")?.parse().map_err(|e| format!("--places: {e}"))?;
                if o.places == 0 {
                    return Err("--places must be at least 1".into());
                }
            }
            "--only-steady" => o.only_steady = true,
            "--steady-window-s" => {
                o.steady_window_s =
                    it.next().ok_or("--steady-window-s needs a value")?.parse().map_err(|e| format!("--steady-window-s: {e}"))?
            }
            "--share-bound" => {
                o.share_bound = it.next().ok_or("--share-bound needs a value")?.parse().map_err(|e| format!("--share-bound: {e}"))?
            }
            "--lit" => o.lit = true,
            "--no-ui" => o.no_ui = true,
            "--edit" => o.edit = true,
            "--items" => o.items = Some(PathBuf::from(it.next().ok_or("--items needs a directory")?)),
            "--editor-layout" => o.editor_layout = Some(PathBuf::from(it.next().ok_or("--editor-layout needs a file")?)),
            "--site" => {
                let v = it.next().ok_or("--site needs LAT,LON")?;
                let bad = || format!("--site {v}: expected LAT,LON in degrees");
                let (la, lo) = v.split_once(',').ok_or_else(bad)?;
                let (la, lo) = (la.trim().parse::<f64>().map_err(|_| bad())?, lo.trim().parse::<f64>().map_err(|_| bad())?);
                if !(la.abs() <= 90.0 && lo.abs() <= 360.0) {
                    return Err(bad());
                }
                o.site = Some((la, lo));
            }
            "--marker" => o.markers.push(it.next().ok_or("--marker needs LAT,LON[,RADIUS_M]")?.clone()),
            "--calibrate" => o.calibrate = true,
            "--poses" => o.poses = Some(PathBuf::from(it.next().ok_or("--poses needs a poses file")?)),
            "--note" => o.note = Some(it.next().ok_or("--note needs a text")?.clone()),
            "--commit" => o.commit = Some(PathBuf::from(it.next().ok_or("--commit needs the folder of a commit")?)),
            "--views" => {
                o.views = Some(it.next().ok_or("--views needs a comma-separated list")?.split(',').map(|v| v.trim().to_string()).collect())
            }
            "--flight-frames" => {
                o.flight_frames = it.next().ok_or("--flight-frames needs a value")?.parse().map_err(|e| format!("--flight-frames: {e}"))?;
                if o.flight_frames < 2 {
                    return Err("--flight-frames must be at least 2".into());
                }
            }
            "--store" => o.store = Some(PathBuf::from(it.next().ok_or("--store needs the directory of a stored bake")?)),
            "--allow-memory-bake" => o.allow_memory_bake = true,
            "--cells" => o.cells = it.next().ok_or("--cells needs a value")?.parse().map_err(|e| format!("--cells: {e}"))?,
            "--tau" => o.tau = it.next().ok_or("--tau needs a value")?.parse().map_err(|e| format!("--tau: {e}"))?,
            "--finest-level" => {
                let level: u8 = it.next().ok_or("--finest-level needs a level")?.parse().map_err(|e| format!("--finest-level: {e}"))?;
                if level > planet_core::tile::MAX_LEVEL {
                    return Err(format!("--finest-level must be at most {}, got {level}", planet_core::tile::MAX_LEVEL));
                }
                o.finest_level = Some(level);
            }
            "--floor-level" => {
                let level: u8 = it.next().ok_or("--floor-level needs a level")?.parse().map_err(|e| format!("--floor-level: {e}"))?;
                if level > planet_core::tile::MAX_LEVEL {
                    return Err(format!("--floor-level must be at most {}, got {level}", planet_core::tile::MAX_LEVEL));
                }
                o.floor_level = Some(level);
            }
            "--frames" => o.frames = it.next().ok_or("--frames needs a value")?.parse().map_err(|e| format!("--frames: {e}"))?,
            "--flight" => o.flight = Some(it.next().ok_or("--flight needs a frame count")?.parse().map_err(|e| format!("--flight: {e}"))?),
            "--out-dir" => o.out_dir = Some(PathBuf::from(it.next().ok_or("--out-dir needs a path")?)),
            "--size" => o.size = Some(parse_size(it.next().ok_or("--size needs WxH")?)?),
            "--script" => o.script = it.next().ok_or("--script needs an index or name")?.clone(),
            "--view" => o.view = it.next().ok_or("--view needs a value")?.clone(),
            "--tile" => o.tiles.push(it.next().ok_or("--tile needs FACE,LEVEL,X,Y")?.clone()),
            "--seed" => o.seed = it.next().ok_or("--seed needs a value")?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--res" => {
                o.res = it.next().ok_or("--res needs a value")?.parse().map_err(|e| format!("--res: {e}"))?;
                if !o.res.is_power_of_two() || o.res > planet_render::exec::MAX_TILE_RES {
                    return Err(format!("--res must be a power of two up to {}, got {}", planet_render::exec::MAX_TILE_RES, o.res));
                }
            }
            "--bundle" => o.bundle_out = Some(PathBuf::from(it.next().ok_or("--bundle needs a path")?)),
            "--repro" => o.repro = Some(PathBuf::from(it.next().ok_or("--repro needs a path")?)),
            "--replay-path" => o.replay_path = Some(PathBuf::from(it.next().ok_or("--replay-path needs a camera path file")?)),
            "--replay-every" => {
                o.replay_every = it.next().ok_or("--replay-every needs a count")?.parse().map_err(|e| format!("--replay-every: {e}"))?
            }
            "--world" => o.world = Some(PathBuf::from(it.next().ok_or("--world needs a path to a definition file")?)),
            "--look" => o.look = Some(PathBuf::from(it.next().ok_or("--look needs a path to a look file")?)),
            "--cache-dir" => o.cache_dir = Some(PathBuf::from(it.next().ok_or("--cache-dir needs a directory")?)),
            "--verify-cache" => o.verify_cache = Some(PathBuf::from(it.next().ok_or("--verify-cache needs a directory")?)),
            "--compact-cache" => o.compact_cache = Some(PathBuf::from(it.next().ok_or("--compact-cache needs a directory")?)),
            "--cap-bytes" => {
                o.cap_bytes = Some(it.next().ok_or("--cap-bytes needs a byte count")?.parse().map_err(|e| format!("--cap-bytes: {e}"))?)
            }
            "--allow-stale" => o.allow_stale = true,
            "--class-source" => o.class_source = true,
            "--legacy-source" => o.legacy_source = true,
            "--global-dir" => o.global_dir = Some(PathBuf::from(it.next().ok_or("--global-dir needs a directory")?)),
            "--repeats" => o.repeats = Some(it.next().ok_or("--repeats needs a number")?.parse().map_err(|e| format!("--repeats: {e}"))?),
            "--horizon-threads" => {
                o.horizon_threads =
                    Some(it.next().ok_or("--horizon-threads needs a number")?.parse().map_err(|e| format!("--horizon-threads: {e}"))?)
            }
            "--no-horizon-jobs" => o.no_horizon_jobs = true,
            "--altitudes-m" => o.altitudes_m = Some(it.next().ok_or("--altitudes-m needs metres separated by commas")?.clone()),
            "--radii-m" => o.radii_m = Some(it.next().ok_or("--radii-m needs metres separated by commas")?.clone()),
            "--coarse-horizon" => o.coarse_horizon = Some(PathBuf::from(it.next().ok_or("--coarse-horizon needs a store directory")?)),
            "--remake-into" => {
                o.remake_into = Some(PathBuf::from(it.next().ok_or("--remake-into needs an output directory under target/")?))
            }
            "--coupled-run" => o.coupled_run = Some(PathBuf::from(it.next().ok_or("--coupled-run needs a store directory")?)),
            "--plan-check" => o.plan_check = Some(PathBuf::from(it.next().ok_or("--plan-check needs a directory")?)),
            "--prefill-dir" => o.prefill_dir = Some(PathBuf::from(it.next().ok_or("--prefill-dir needs a directory")?)),
            "--prefill-levels" => {
                let v: u8 = it.next().ok_or("--prefill-levels needs a count")?.parse().map_err(|e| format!("--prefill-levels: {e}"))?;
                if v > planet_cache::meshpack::MAX_PREFILL_LEVELS {
                    return Err(format!("--prefill-levels {v}: at most {} (0 disables)", planet_cache::meshpack::MAX_PREFILL_LEVELS));
                }
                o.prefill_levels = Some(v);
            }
            "--verify-global" => o.verify_global = Some(PathBuf::from(it.next().ok_or("--verify-global needs a directory")?)),
            "--global-products" => o.global_products = Some(PathBuf::from(it.next().ok_or("--global-products needs a directory")?)),
            "--lithology-preview" => o.lithology_preview = Some(PathBuf::from(it.next().ok_or("--lithology-preview needs a directory")?)),
            "--uplift-preview" => o.uplift_preview = Some(PathBuf::from(it.next().ok_or("--uplift-preview needs a directory")?)),
            "--climate-preview" => o.climate_preview = Some(PathBuf::from(it.next().ok_or("--climate-preview needs a directory")?)),
            "--arid-preview" => o.arid_preview = Some(PathBuf::from(it.next().ok_or("--arid-preview needs a directory")?)),
            "--shelf-preview" => o.shelf_preview = Some(PathBuf::from(it.next().ok_or("--shelf-preview needs a directory")?)),
            "--ice-preview" => o.ice_preview = Some(PathBuf::from(it.next().ok_or("--ice-preview needs a directory")?)),
            "--fine-lattice" => {
                let n: usize =
                    it.next().ok_or("--fine-lattice needs a number of cells")?.parse().map_err(|e| format!("--fine-lattice: {e}"))?;
                if !(64..=1024).contains(&n) {
                    return Err(format!("--fine-lattice {n} must be from 64 to 1024 cells a face edge"));
                }
                o.fine_lattice = Some(n);
            }
            "--km" => {
                o.preview_km = it.next().ok_or("--km needs a number of kilometres")?.parse().map_err(|e| format!("--km: {e}"))?;
                if !(o.preview_km.is_finite() && (8.0..=500.0).contains(&o.preview_km)) {
                    return Err(format!("--km {} must be from 8 to 500 (finer maps need tens of gigabytes)", o.preview_km));
                }
            }
            "--global-res" => {
                o.global_res = it.next().ok_or("--global-res needs a value")?.parse().map_err(|e| format!("--global-res: {e}"))?
            }
            "--threads" => o.threads = it.next().ok_or("--threads needs a count")?.parse().map_err(|e| format!("--threads: {e}"))?,
            "--sample" => o.sample = it.next().ok_or("--sample needs a count")?.parse().map_err(|e| format!("--sample: {e}"))?,
            "--region" => {
                let v = it.next().ok_or("--region needs FACE,LEVEL")?;
                let bad = || format!("--region must be FACE,LEVEL with face 0..=5 and level 0..={MAX_REGION_LEVEL}, got '{v}'");
                let (f, l) = v.split_once(',').ok_or_else(bad)?;
                let (f, l): (u8, u8) = (f.parse().map_err(|_| bad())?, l.parse().map_err(|_| bad())?);
                if f > 5 || l > MAX_REGION_LEVEL {
                    return Err(bad());
                }
                o.region = Some((f, l));
            }
            "--manifest" => o.manifest = Some(PathBuf::from(it.next().ok_or("--manifest needs a path")?)),
            "--face-net" => o.face_net = Some(PathBuf::from(it.next().ok_or("--face-net needs a path")?)),
            "--machine-ready" => o.machine_ready = true,
            "--loaded" => o.loaded = true,
            "--reserve" => o.reserve = true,
            "--scene" => o.scene = it.next().ok_or("--scene needs a value")?.clone(),
            "--out" => o.out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            "--upload" => {
                let v = it.next().ok_or("--upload needs per-slot or batched")?;
                o.upload =
                    Some(planet_render::UploadMode::parse(v).ok_or_else(|| format!("--upload must be per-slot or batched, got '{v}'"))?)
            }
            "--gen-workers" => {
                let v = it.next().ok_or("--gen-workers needs a thread count")?;
                o.gen_workers = Some(
                    v.parse::<usize>()
                        .ok()
                        .filter(|n| *n >= 1)
                        .ok_or_else(|| format!("--gen-workers must be a positive integer, got '{v}'"))?,
                )
            }
            "--gen-reserve" => {
                let v = it.next().ok_or("--gen-reserve needs a count")?;
                o.gen_reserve = Some(v.parse::<usize>().map_err(|_| format!("--gen-reserve must be a non-negative integer, got '{v}'"))?)
            }
            "--gen-priority" => {
                let v = it.next().ok_or("--gen-priority needs a value")?;
                o.gen_priority = Some(
                    planet_streaming::Priority::parse(v)
                        .ok_or_else(|| format!("--gen-priority must be below-normal or normal, got '{v}'"))?,
                )
            }
            "--gen-ahead" => {
                let v = it.next().ok_or("--gen-ahead needs a count")?;
                o.gen_ahead = Some(v.parse::<usize>().map_err(|_| format!("--gen-ahead must be a non-negative integer, got '{v}'"))?)
            }
            "--draw" => o.draw = Some(planet_render::DrawMode::parse(it.next().ok_or("--draw needs per-draw or indirect")?)?),
            "--adapter" => {
                o.adapter = Some(match it.next().ok_or("--adapter needs a value")?.as_str() {
                    "software" => AdapterPolicy::Software,
                    "hardware" => AdapterPolicy::HighPerformance,
                    other => return Err(format!("--adapter must be software or hardware, got '{other}'")),
                })
            }
            other => return Err(format!("unknown option '{other}'\n{USAGE}")),
        }
    }
    if o.loaded && o.machine_ready {
        return Err("--loaded and --machine-ready exclude each other: loaded mode records the load, the owner's override ignores a failed gate (BUILD-030)".into());
    }
    Ok(o)
}

use planet_cache::tiles::MAX_REGION_LEVEL;

/// Options that only the tile store commands of `generate` understand.
pub(crate) const STORE_OPTIONS: [&str; 29] = [
    "--repeats",
    "--horizon-threads",
    "--altitudes-m",
    "--radii-m",
    "--coarse-horizon",
    "--prefill-dir",
    "--prefill-levels",
    "--plan-check",
    "--compact-cache",
    "--cap-bytes",
    "--world",
    "--cache-dir",
    "--verify-cache",
    "--region",
    "--sample",
    "--allow-stale",
    "--global-dir",
    "--verify-global",
    "--global-products",
    "--lithology-preview",
    "--uplift-preview",
    "--climate-preview",
    "--arid-preview",
    "--shelf-preview",
    "--ice-preview",
    "--fine-lattice",
    "--km",
    "--global-res",
    "--threads",
];

/// Options a `--flight` run would ignore, and the flight-only options other runs would ignore (TEST-013).
const FLIGHT_EXCLUDES: [&str; 13] = [
    "--perf",
    "--perf-smoke",
    "--repro",
    "--out",
    "--bundle",
    "--stream",
    "--pipelined",
    "--tile",
    "--script",
    "--frames",
    "--res",
    "--manifest",
    "--face-net",
];
const FLIGHT_ONLY: [&str; 2] = ["--size", "--out-dir"];

/// `--commit DIR` (VIEW-069): the commit opens over the production base computed in memory, so the options of a stored world or another base are refused beside it, and it applies to the viewer and
/// `capture-stills` only.
pub fn check_commit_options(o: &Options) -> Result<(), String> {
    if !o.given.contains("--commit") {
        return Ok(());
    }
    let clash: Vec<&str> =
        ["--store", "--erosion", "--erosion-store", "--global-res", "--allow-memory-bake", "--class-source", "--legacy-source"]
            .into_iter()
            .filter(|f| o.given.contains(*f))
            .collect();
    if !clash.is_empty() {
        return Err(format!(
            "--commit opens a commit over the production base computed in memory: it excludes {} (a stored world, the old layer or another in-memory bake)",
            clash.join(", ")
        ));
    }
    if !matches!(o.mode, Mode::Editor) && !stills::is_stills_scene(&o.scene) {
        return Err("--commit applies to the editor (viewer) and to the scene `capture-stills`".into());
    }
    Ok(())
}

/// Reject option combinations that would be silently ignored, before any GPU work.
fn check_flight_options(o: &Options) -> Result<(), String> {
    if o.flight.is_some() {
        if o.scene != "terrain" {
            return Err("--flight is only available for --scene terrain".into());
        }
        let clash: Vec<&str> = FLIGHT_EXCLUDES.iter().copied().filter(|f| o.given.contains(*f)).collect();
        if !clash.is_empty() {
            return Err(format!("--flight writes a frame sequence and excludes {}", clash.join(", ")));
        }
        if o.out_dir.is_none() {
            return Err("--flight needs --out-dir".into());
        }
    } else if let Some(f) = FLIGHT_ONLY.iter().find(|f| {
        o.given.contains(**f)
            && !((o.pipelined || o.replay_path.is_some()) && **f == "--out-dir")
            && !litperf::is_capture_scene(&o.scene)
            && !stills::is_cost_scene(&o.scene)
            && !stills::is_layer1_scene(&o.scene)
    }) {
        return Err(format!("{f} only applies to --flight"));
    }
    if o.replay_path.is_some() {
        if o.repro.is_none() {
            return Err("--replay-path needs --repro BUNDLE (the terrain bundle F9 wrote beside the path file)".into());
        }
        if o.out_dir.is_none() {
            return Err("--replay-path needs --out-dir".into());
        }
    } else if o.given.contains("--replay-every") {
        return Err("--replay-every only applies to --replay-path".into());
    }
    if o.lockstep && !o.pipelined {
        return Err("--lockstep only applies to --pipelined".into());
    }
    if o.then.is_some() && !o.pipelined {
        return Err("--then only applies to --pipelined".into());
    }
    if o.pipelined {
        if o.scene != "terrain" {
            return Err("--pipelined is only available for --scene terrain".into());
        }
        const CLASH: [&str; 8] = ["--stream", "--bundle", "--perf", "--perf-smoke", "--repro", "--flight", "--size", "--tile"];
        let clash: Vec<&str> = CLASH.iter().copied().filter(|f| o.given.contains(*f)).collect();
        if !clash.is_empty() {
            return Err(format!("--pipelined draws one settled view and excludes {}", clash.join(", ")));
        }
    }
    Ok(())
}

/// `--floor-level` (LOD-011): `test-render` and `generate` take it, the viewer takes no flag; it applies to the terrain scenes, the
/// streaming and lit scenes and the pre-fill commands only (anything else would ignore it), not to a replayed bundle (the bundle
/// carries its own `min_level`); and an explicit floor above the finest level is refused, naming both, before any work.
fn check_floor_option(o: &Options) -> Result<(), String> {
    if !o.given.contains("--floor-level") {
        return Ok(());
    }
    match o.mode {
        Mode::Editor => return Err("--floor-level applies to `test-render` and `generate`: the viewer takes no flag (LOD-011)".into()),
        Mode::Generate if o.prefill_dir.is_none() && o.global_dir.is_none() => {
            return Err("--floor-level only applies to the pre-fill of `generate` (--prefill-dir, --global-dir)".into())
        }
        Mode::TestRender => {
            if o.repro.is_some() {
                return Err("--floor-level does not apply to --repro: a bundle carries its own render floor (`min_level`)".into());
            }
            let takes = o.scene == "terrain"
                || streamperf::is_stream_scene(&o.scene)
                || litperf::is_lit_scene(&o.scene)
                || litperf::is_capture_scene(&o.scene);
            if !takes {
                return Err(format!("--floor-level does not apply to --scene {}: it applies to the terrain and streaming scenes", o.scene));
            }
        }
        Mode::Generate => {}
    }
    o.check_floor()
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let bad = || format!("--size must be WxH with both sides 16..=8192, got '{s}'");
    let (w, h) = s.split_once('x').ok_or_else(bad)?;
    let (w, h): (u32, u32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if !(16..=8192).contains(&w) || !(16..=8192).contains(&h) {
        return Err(bad());
    }
    Ok((w, h))
}

pub fn run(args: &[String], out: &mut dyn Write) -> Result<(), String> {
    let o = parse_args(args)?;
    check_floor_option(&o)?;
    if !matches!(o.mode, Mode::Generate) {
        // The global-bake perf scene takes the definition, grid size and thread count of the global commands.
        let bake_scene = matches!(o.mode, Mode::TestRender) && o.perf && matches!(o.scene.as_str(), "global-bake" | "coupled");
        let stream_scene = matches!(o.mode, Mode::TestRender)
            && (o.perf || o.perf_smoke || litperf::is_capture_scene(&o.scene))
            && (streamperf::is_stream_scene(&o.scene) || litperf::is_lit_scene(&o.scene) || litperf::is_capture_scene(&o.scene));
        let terrain_scene = matches!(o.mode, Mode::TestRender) && o.scene == "terrain";
        let window = matches!(o.mode, Mode::Editor) && !o.offscreen && !o.no_window;
        let allowed: &[&str] = if bake_scene {
            &["--world", "--global-res", "--threads"]
        } else if stream_scene {
            // `--prefill-levels`: the levels of the tile pack the gated dives require pre-filled at their start (default 4).
            // `--altitudes-m`: the altitudes of the envelope scene (`--only-steady`, TEST-057).
            // `--radii-m`: with `--altitudes-m`, the hill stamps of the edit-latency scene to run (a diagnosis of one case).
            &["--world", "--global-res", "--prefill-levels", "--horizon-threads", "--repeats", "--altitudes-m", "--radii-m", "--places"]
        } else if window {
            &["--world", "--global-res", "--prefill-levels"]
        } else if terrain_scene {
            &["--world", "--global-res"]
        } else {
            &[]
        };
        if let Some(f) = STORE_OPTIONS.iter().find(|f| o.given.contains(**f) && !allowed.contains(f)) {
            return Err(format!("{f} only applies to `generate` (the tile store commands)"));
        }
    }
    // The erosion layer is drawn by the viewer over a stored bake (VIEW-062); no other mode reads it, and its options mean nothing without `--erosion on`.
    const EROSION_OPTIONS: [&str; 4] = ["--erosion-store", "--erosion-face-res", "--erosion-res", "--erosion-unchecked"];
    if o.preview_l1.is_some() && !matches!(o.mode, Mode::Editor) {
        return Err("--preview-l1 and --preview-grid only apply to `editor` (the level-1 preview is drawn over the editor's planet)".into());
    }
    let capture_stills = matches!(o.mode, Mode::TestRender) && o.scene == "capture-stills";
    if o.given.contains("--erosion") && o.erosion && !matches!(o.mode, Mode::Editor) && !capture_stills {
        return Err(
            "--erosion on only applies to `editor` and `test-render --scene capture-stills` (they draw the layer over a stored bake)"
                .into(),
        );
    }
    if !o.erosion {
        if let Some(f) = EROSION_OPTIONS.iter().find(|f| o.given.contains(**f)) {
            return Err(format!("{f} only applies with --erosion on"));
        }
        if o.channels {
            return Err("--channels on only applies with --erosion on (the channels are read from the erosion layer's store)".into());
        }
    }
    if matches!(o.mode, Mode::Generate) && o.given.contains("--look") {
        return Err(
            "--look only applies to `editor` and `test-render`: the look is presentation data and never changes a generated tile".into()
        );
    }
    // A replayed bundle is self-contained (CON-19): it carries its plans' look, so no sibling file is looked for beside it.
    if !matches!(o.mode, Mode::Generate) && !(o.repro.is_some() && o.look.is_none()) {
        // The look file is loaded (and so validated) before any world is built or device opened: a bad file stops the run with
        // the key and the value (LOOK-005).
        if let Some(line) = lookload::load(&o)?.describe() {
            writeln!(out, "{line}").map_err(err)?;
        }
    }
    let smoke = if o.smoke { " [smoke ok]" } else { "" };
    match o.mode {
        Mode::Editor if o.offscreen => skeleton::render_offscreen(&o, out),
        Mode::Editor if o.no_window => writeln!(out, "planet editor: window not opened (--no-window){smoke}").map_err(err),
        Mode::Editor => window::run_window(&o, out),
        Mode::Generate if !o.smoke => skeleton::generate(&o, out),
        Mode::Generate => writeln!(out, "planet generate: ready{smoke}").map_err(err),
        Mode::TestRender => test_render(&o, out),
    }
}

pub(crate) fn err(e: std::io::Error) -> String {
    e.to_string()
}

/// ADR 0020: measure the machine and refuse (listing every failed condition with its measured value) unless it is ready.
/// `--machine-ready` is the owner's explicit override: the failed conditions are printed and recorded in the log.
/// The adapter itself is checked by the caller before this (it is never overridable and costs no sampling time).
pub(crate) fn readiness_gate(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<Vec<String>, String> {
    writeln!(out, "readiness gate: sampling the machine for {} s ...", planet_perf::gate::LOAD_WINDOW_S).map_err(err)?;
    let mode = if o.loaded { planet_perf::gate::Mode::Loaded } else { planet_perf::gate::Mode::Quiet };
    let gate = planet_perf::gate::readiness_mode(&planet_perf::gate::measure_machine(facts(ctx)), mode);
    // Loaded mode: the load is recorded with the session and printed; the hard conditions below still refuse.
    if let Some(load) = &gate.load {
        writeln!(out, "readiness gate (loaded mode, load recorded not judged): {}", load.text()).map_err(err)?;
    }
    planet_perf::gate::record_load(gate.load.clone());
    let failures = gate.failures;
    if failures.is_empty() {
        writeln!(out, "readiness gate: ok").map_err(err)?;
        return Ok(Vec::new());
    }
    let lines: Vec<String> = failures.iter().map(ToString::to_string).collect();
    if o.machine_ready {
        for l in &lines {
            writeln!(out, "readiness gate FAILED (owner override --machine-ready, recorded in the log): {l}").map_err(err)?;
        }
        return Ok(lines);
    }
    Err(format!(
        "refusing to measure: the machine is not ready (ADR 0020):\n  {}\nFix these and retry; only the owner overrides with --machine-ready.",
        lines.join("\n  ")
    ))
}

/// Whether a run reported throttling in the clock samples taken at its start or end (`nvidia-smi`, the boundary
/// approximation of "during measured frames"). A missing sample cannot show the absence of throttling, so it counts.
pub(crate) fn run_throttled(r: &planet_perf::RunReport) -> bool {
    [&r.clocks_before, &r.clocks_after].iter().any(|c| c.as_ref().is_none_or(|c| planet_perf::gate::is_throttling(&c.throttle_reasons)))
}

/// The session verdict, printed once before any budget verdict so a bad session never shows "ok" first (ADR 0020). One definition
/// of what voids frame verdicts and what only GPU verdicts (`planet_perf::validity`); the lines come from it.
pub(crate) fn session_verdict_line(out: &mut dyn Write, verdict: &planet_perf::validity::SessionVerdict) -> Result<(), String> {
    session_verdict_line_for(out, verdict, true)
}

/// [`session_verdict_line`] for a session that takes no GPU rows (the streaming scenes) when `gpu_rows` is false.
pub(crate) fn session_verdict_line_for(
    out: &mut dyn Write,
    verdict: &planet_perf::validity::SessionVerdict,
    gpu_rows: bool,
) -> Result<(), String> {
    for l in verdict.lines_for(gpu_rows) {
        writeln!(out, "{l}").map_err(err)?;
    }
    Ok(())
}

pub(crate) fn facts(ctx: &GpuContext) -> AdapterFacts {
    AdapterFacts {
        name: ctx.adapter_name().to_string(),
        backend: ctx.backend_name(),
        kind: match ctx.kind() {
            planet_render::AdapterKind::Discrete => DeviceKind::Discrete,
            planet_render::AdapterKind::Integrated => DeviceKind::Integrated,
            planet_render::AdapterKind::Software => DeviceKind::Cpu,
            _ => DeviceKind::Other,
        },
    }
}

fn test_render(o: &Options, out: &mut dyn Write) -> Result<(), String> {
    if o.smoke {
        return writeln!(out, "planet test-render [smoke ok]").map_err(err);
    }
    // The edit-scale scene is CPU only: it takes no GPU context and no perf flags (SPIKE-29).
    if editscale::is_scale_scene(&o.scene) {
        return editscale::run(o, out);
    }
    check_flight_options(o)?;
    worldfield::check_options(o)?;
    let policy = o.adapter.unwrap_or_else(AdapterPolicy::from_env);
    let ctx = GpuContext::new(policy).map_err(|e| e.to_string())?;
    // Hybrid-graphics rule (report §8): always log which adapter we got.
    writeln!(out, "{}", ctx.adapter_log_line()).map_err(err)?;
    if let Some(mode) = o.draw {
        ctx.prefer_draw_mode(mode);
        writeln!(out, "draw: {} requested, {} in effect", mode.name(), ctx.default_draw_mode().name()).map_err(err)?;
    }
    if let Some(mode) = o.upload {
        ctx.prefer_upload_mode(mode);
    }
    writeln!(out, "tile format: compact").map_err(err)?;
    if o.cells < planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS {
        return Err(format!(
            "--cells {}: the compact tile format needs at least {} cells per tile edge (the climate of a tile rides in the spare channel of its first 162 texels, GEN-021): use --cells {} or more",
            o.cells,
            planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS,
            planet_generators::climate::CLIMATE_MIN_COMPACT_CELLS
        ));
    }
    if o.perf && o.perf_smoke {
        return Err("--perf and --perf-smoke exclude each other: a smoke run is never a measurement".into());
    }
    if o.perf_smoke
        && o.scene != "terrain"
        && !streamperf::is_stream_scene(&o.scene)
        && !litperf::is_lit_scene(&o.scene)
        && !preflightperf::is_preflight_scene(&o.scene)
    {
        return Err("--perf-smoke is only available for --scene terrain, a streaming scene (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, streaming), the lit scene (lit) or preflight".into());
    }
    if o.lit && !streamperf::is_stream_scene(&o.scene) {
        return Err("--lit applies to the streaming perf scenes (dive, dive-fast, teleport-1m, teleport-2km, look-1m, look-2km, dive-cold, streaming); the scene `lit` is always lit".into());
    }
    if o.given.contains("--views") && !litperf::is_lit_scene(&o.scene) && !litperf::is_capture_scene(&o.scene)
        || o.given.contains("--flight-frames") && !litperf::is_lit_scene(&o.scene) && !litperf::is_capture_scene(&o.scene)
    {
        return Err("--views applies to the scenes `lit` and `capture-flights`, --flight-frames to those".into());
    }
    if o.given.contains("--poses")
        && !stills::is_stills_scene(&o.scene)
        && !stills::is_cost_scene(&o.scene)
        && !stills::is_layer1_scene(&o.scene)
    {
        return Err("--poses applies to the scene `capture-stills` (and to `shadow-cost` and `shadow-layer1`)".into());
    }
    if o.given.contains("--note") && !stills::is_stills_scene(&o.scene) {
        return Err("--note applies to the scene `capture-stills`".into());
    }
    check_commit_options(o)?;
    if stills::is_stills_scene(&o.scene) && o.given.contains("--flight-frames") {
        return Err("--flight-frames does not apply to `capture-stills` (one frame per pose)".into());
    }
    streamperf::check_options(o)?;
    if litperf::is_capture_scene(&o.scene) {
        return if stills::is_stills_scene(&o.scene) {
            stills::capture_stills(o, &ctx, out)
        } else if o.scene == "river-frames" {
            litperf::capture_river_frames(o, &ctx, out)
        } else {
            litperf::capture_flights(o, &ctx, out)
        };
    }
    if o.flight.is_some() {
        return skeleton::render_flight(&ctx, o, out);
    }
    if o.perf || o.perf_smoke {
        return match o.scene.as_str() {
            "terrain" => skeleton::perf_terrain(o, &ctx, out),
            "global-bake" => globalperf::perf_global_bake(o, &ctx, out),
            "coupled" => globalperf::perf_coupled(o, &ctx, out),
            s if envelopeperf::is_envelope_scene(s) => envelopeperf::perf_envelope(o, &ctx, out),
            s if preflightperf::is_preflight_scene(s) => preflightperf::perf_preflight(o, &ctx, out),
            s if editperf::is_edit_scene(s) => editperf::perf_edit_latency(o, &ctx, out),
            s if stills::is_cost_scene(s) => stills::perf_shadow_cost(o, &ctx, out),
            s if stills::is_layer1_scene(s) => stills::perf_shadow_layer1(o, &ctx, out),
            s if streamperf::is_stream_scene(s) => streamperf::perf_streaming(o, &ctx, out),
            s if litperf::is_lit_scene(s) => litperf::perf_lit(o, &ctx, out),
            _ => perf_skeleton(o, &ctx, out),
        };
    }
    if let Some(bundle) = &o.repro {
        if o.replay_path.is_some() {
            return pathreplay::replay_path(&ctx, o, bundle, out);
        }
        return skeleton::replay(&ctx, bundle, o.out.as_deref(), o.store.as_deref(), out);
    }
    match o.scene.as_str() {
        "tiles" => skeleton::render_tiles(&ctx, o, out),
        "terrain" => skeleton::render_terrain(&ctx, o, out),
        "hello-triangle" => {
            let frame = planet_render::hello_triangle(&ctx, 256);
            if let Some(path) = &o.out {
                frame.write_png(path).map_err(|e| format!("writing {}: {e}", path.display()))?;
                writeln!(out, "wrote {}", path.display()).map_err(err)?;
            }
            Ok(())
        }
        other => Err(format!("unknown scene '{other}' (known: hello-triangle, tiles, terrain)")),
    }
}

/// Performance-harness skeleton: adapter check, power/clock logging, warm-up, medians, JSON log.
/// It times the hello-triangle round trip as a stand-in for a frame (the terrain scenes time real frames).
fn perf_skeleton(o: &Options, ctx: &GpuContext, out: &mut dyn Write) -> Result<(), String> {
    planet_perf::check_reference_adapter(&facts(ctx)).map_err(|r| r.to_string())?;
    let owner_override = readiness_gate(o, ctx, out)?;
    let protocol = Protocol { warmup_frames: 10, measured_frames: 50, runs: 3, ..Protocol::default() };
    let mut runs = Vec::new();
    for _ in 0..protocol.runs {
        let before = planet_perf::query_nvidia_smi();
        let mut samples = Vec::new();
        for _ in 0..(protocol.warmup_frames + protocol.measured_frames) {
            let t = Instant::now();
            planet_render::hello_triangle(ctx, 256);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let stats = planet_perf::summarize(&samples, protocol.warmup_frames).ok_or("no valid samples")?;
        runs.push(RunReport {
            path_name: "skeleton-hello-triangle".into(),
            vram_cap_gb: 8,
            stats,
            clocks_before: before,
            clocks_after: planet_perf::query_nvidia_smi(),
            metrics: vec![],
            counts: vec![],
        });
    }
    let medians: Vec<f64> = runs.iter().map(|r| r.stats.median).collect();
    let throttled: Vec<String> = runs.iter().filter(|r| run_throttled(r)).map(|r| r.path_name.clone()).collect();
    // The skeleton is a stand-in workload: its noise is judged on its three runs without a retry.
    let noisy: Vec<(String, f64)> = if planet_perf::gate::noise_needs_retry(&medians) {
        vec![("skeleton-hello-triangle".into(), planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN))]
    } else {
        Vec::new()
    };
    let validity = planet_perf::gate::validity(&throttled, &noisy);
    let scheme = planet_perf::query_power_scheme();
    let verdict = planet_perf::validity::SessionVerdict::assess(&facts(ctx), scheme.as_deref(), &runs, false, &validity, &owner_override);
    session_verdict_line(out, &verdict)?;
    writeln!(out, "run-to-run noise (max-min)/median: {:.3}", planet_perf::run_to_run_noise(&medians).unwrap_or(f64::NAN)).map_err(err)?;
    let json =
        planet_perf::report_json(&facts(ctx), planet_perf::query_power_scheme().as_deref(), &owner_override, &validity, &protocol, &runs);
    let path = o.out.clone().unwrap_or_else(|| Path::new("perf").join("logs").join("skeleton.json"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(&path, json).map_err(err)?;
    writeln!(out, "wrote {}", path.display()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_str(args: &[&str]) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut buf = Vec::new();
        run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
    }

    // spec: LOD-010
    #[test]
    fn the_finest_level_defaults_to_the_production_cap_for_the_tile_size_and_is_overridable() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        let o = args(&["test-render", "--scene", "terrain"]);
        assert_eq!((o.finest_level, o.finest_level_for(32), o.finest_level_for(64), o.finest_level_for(16)), (None, 22, 21, 23));
        let o = args(&["test-render", "--finest-level", "26", "--cells", "64"]);
        assert_eq!((o.finest_level, o.finest_level_for(64)), (Some(26), 26), "the explicit cap wins whatever the tile size");
        assert!(parse_args(&["test-render".into(), "--finest-level".into(), "29".into()]).unwrap_err().contains("at most 28"));
        assert!(parse_args(&["test-render".into(), "--finest-level".into()]).unwrap_err().contains("needs a level"));
    }

    // spec: VIEW-022
    #[test]
    fn the_editor_mode_options_are_parsed() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        let o = args(&["editor"]);
        assert!(!o.edit && o.items.is_none() && o.editor_layout.is_none(), "off by default, edits in memory");
        let o = args(&["editor", "--edit", "--items", "edits", "--editor-layout", "layout.toml"]);
        assert!(o.edit);
        assert_eq!(o.items, Some(PathBuf::from("edits")));
        assert_eq!(o.editor_layout, Some(PathBuf::from("layout.toml")));
        for flag in ["--items", "--editor-layout"] {
            assert!(parse_args(&["editor".to_string(), flag.to_string()]).is_err(), "{flag} needs a value");
        }
        assert!(USAGE.contains("--edit") && USAGE.contains("--items") && USAGE.contains("--editor-layout"));
        // The editor mode needs the UI: the run says so before it opens anything.
        let o = args(&["editor", "--edit", "--no-ui"]);
        let e = crate::window::run_window(&o, &mut std::io::sink()).unwrap_err();
        assert!(e.contains("--edit needs the editor UI"), "{e}");
    }

    // spec: VIEW-011
    #[test]
    fn the_editor_window_draws_its_ui_unless_no_ui_is_given() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        assert!(!args(&["editor"]).no_ui, "the UI is on by default");
        assert!(args(&["editor", "--no-ui"]).no_ui);
        // VIEW-013: markers of the globe overview, repeatable.
        assert!(args(&["editor"]).markers.is_empty());
        assert_eq!(args(&["editor", "--marker", "1,2", "--marker", "3,4,500"]).markers, ["1,2", "3,4,500"]);
        assert!(parse_args(&["editor".to_string(), "--marker".to_string()]).is_err(), "--marker needs a value");
        assert!(USAGE.contains("--marker"), "the usage names the marker option");
        assert_eq!(args(&["editor", "--site", "45,-100"]).site, Some((45.0, -100.0)));
        assert!(parse_args(&["editor".to_string(), "--site".to_string(), "95,0".to_string()]).is_err(), "latitude out of range");
        assert!(USAGE.contains("--no-ui"), "the usage names the option");
    }

    // spec: VIEW-058
    #[test]
    fn the_flight_site_is_given_by_site_and_refused_out_of_range() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(args(&["editor"]).unwrap().site, None, "the default site is the scene's");
        assert_eq!(args(&["editor", "--site", "45,-100"]).unwrap().site, Some((45.0, -100.0)));
        for bad in ["95,0", "0,400", "x,1", "1"] {
            assert!(args(&["editor", "--site", bad]).is_err(), "--site {bad} is refused");
        }
        assert!(args(&["editor", "--site"]).is_err(), "--site needs a value");
        assert!(USAGE.contains("--site"), "the usage names the option");
    }

    // spec: TEST-041
    #[test]
    fn the_edit_latency_scene_takes_a_number_of_repeats() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let o = args(&["test-render", "--perf", "--scene", "edit-latency", "--repeats", "5"]).unwrap();
        assert_eq!(o.repeats, Some(5));
        assert_eq!(args(&["test-render", "--perf", "--scene", "edit-latency"]).unwrap().repeats, None);
        assert!(args(&["test-render", "--perf", "--scene", "edit-latency", "--repeats", "x"]).is_err(), "--repeats needs a number");
    }

    // spec: STRM-020, TEST-012
    #[test]
    fn the_generation_pool_defaults_to_the_machine_less_the_reserve_and_the_options_override_it() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        let o = args(&["test-render", "--scene", "terrain"]);
        let g = o.gen_workers();
        assert_eq!(g.workers, planet_streaming::shares::Shares::machine().terrain, "the default is the terrain share (BUDG-003)");
        assert_eq!((g.ahead, o.gen_reserve_used()), (g.workers, None));
        let o = args(&["test-render", "--gen-reserve", "5"]);
        assert_eq!(o.gen_workers().workers, planet_streaming::GenWorkers::workers_for(cores, 5));
        assert_eq!(o.gen_reserve_used(), Some(5));
        // An explicit size wins and the reserve decided nothing, so the log records none.
        let o = args(&["test-render", "--gen-workers", "7", "--gen-reserve", "5", "--gen-ahead", "3"]);
        assert_eq!((o.gen_workers().workers, o.gen_workers().ahead, o.gen_reserve_used()), (7, 3, None));
        assert_eq!(args(&["test-render", "--gen-reserve", "0"]).gen_workers().workers, cores);
        assert!(parse_args(&["test-render".into(), "--gen-reserve".into(), "x".into()]).unwrap_err().contains("non-negative"));
        assert!(parse_args(&["test-render".into(), "--gen-reserve".into()]).unwrap_err().contains("needs a count"));
    }

    // spec: STRM-016, TEST-012
    #[test]
    fn lockstep_is_a_pipelined_only_test_mode() {
        let parse = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(!parse(&["test-render", "--scene", "terrain", "--pipelined"]).unwrap().lockstep);
        assert!(parse(&["test-render", "--scene", "terrain", "--pipelined", "--lockstep"]).unwrap().lockstep);
        let o = parse(&["test-render", "--scene", "terrain", "--lockstep"]).unwrap();
        assert!(check_flight_options(&o).unwrap_err().contains("--lockstep only applies to --pipelined"));
    }

    // spec: STRM-020, TEST-012
    #[test]
    fn the_generation_workers_run_below_normal_priority_unless_the_option_says_normal() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let low = planet_streaming::Priority::BelowNormal;
        assert_eq!(args(&["test-render"]).unwrap().gen_workers().priority, low);
        assert_eq!(args(&["test-render", "--gen-priority", "below-normal"]).unwrap().gen_workers().priority, low);
        let o = args(&["test-render", "--gen-priority", "normal", "--gen-workers", "3"]).unwrap();
        assert_eq!((o.gen_workers().priority, o.gen_workers().workers), (planet_streaming::Priority::Normal, 3));
        assert!(args(&["test-render", "--gen-priority", "high"]).unwrap_err().contains("below-normal or normal"));
        assert!(args(&["test-render", "--gen-priority"]).unwrap_err().contains("needs a value"));
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "planet-app-tests-{}-{}-{name}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn s(&self, leaf: &str) -> String {
            self.0.join(leaf).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // spec: STRM-004
    #[test]
    fn generate_writes_a_region_into_a_store_and_cache_verify_checks_it() {
        let d = TempDir::new("store");
        let store = d.s("store");
        let out = run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        assert!(out.contains("4 tiles") && out.contains("4 generated, 0 already stored"), "{out}");
        let again = run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        assert!(again.contains("0 generated, 4 already stored"), "{again}");
        let ok = run_str(&["generate", "--verify-cache", &store, "--seed", "3"]).unwrap();
        assert!(ok.contains("checked 4 of 4 entries: 4 match, 0 stale"), "{ok}");
        let sampled = run_str(&["generate", "--verify-cache", &store, "--seed", "3", "--sample", "2"]).unwrap();
        assert!(sampled.contains("checked 2 of 4 entries: 2 match"), "{sampled}");
        // Another seed: every stored tile is stale, nothing could be compared: that is not a pass.
        let e = run_str(&["generate", "--verify-cache", &store, "--seed", "4"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("--allow-stale") && e.contains("4 entries"), "{e}");
        // Unless stale entries are expected (the definition changed on purpose).
        let stale = run_str(&["generate", "--verify-cache", &store, "--seed", "4", "--allow-stale"]).unwrap();
        assert!(stale.contains("4 stale without a known reason") && stale.contains("0 differ"), "{stale}");
    }

    // spec: STRM-013
    #[test]
    fn generate_compacts_a_store_dropping_dead_records_and_honouring_the_cap() {
        use planet_cache::pack::{PackWriter, Record, TileStore};
        let d = TempDir::new("compact");
        let store = d.s("store");
        run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        // A record of an older generator version, as left behind by a version bump.
        let dir = std::path::Path::new(&store);
        let params = planet_cache::tiles::World::from_definition(&planet_world_def::WorldDefinition::single_planet(6_371_000.0, 3))
            .unwrap()
            .params();
        let read = TileStore::open(dir, params).unwrap();
        let (kind, key) = read.all_entries()[0];
        let tile = planet_core::TileId::new(planet_core::Face(0), 1, 0, 0).unwrap();
        let live = read.get(kind, tile, key).unwrap().expect("the first tile of the region");
        drop(read);
        let mut w = PackWriter::open(dir, params, planet_cache::pack::DEFAULT_MAX_PACK_BYTES).unwrap();
        w.append(&Record { key: planet_cache::key::CacheKey([9; 16]), generator_version: live.generator_version + 1000, ..live.clone() })
            .unwrap();
        // A record of a kind this build does not know is kept.
        w.append(&Record { key: planet_cache::key::CacheKey([8; 16]), kind: 9, generator_version: 0, ..live.clone() }).unwrap();
        w.finish().unwrap();
        let out = run_str(&["generate", "--compact-cache", &store, "--seed", "3"]).unwrap();
        assert!(out.contains("kept 5 records") && out.contains("dropped 1 dead") && out.contains("seed 3"), "{out}");
        let again = run_str(&["generate", "--compact-cache", &store, "--seed", "3"]).unwrap();
        assert!(again.contains("kept 5 records") && again.contains("dropped 0 dead"), "{again}");
        assert!(run_str(&["generate", "--verify-cache", &store, "--seed", "3"]).unwrap().contains("4 match"));
        let capped = run_str(&["generate", "--compact-cache", &store, "--seed", "3", "--cap-bytes", "1"]).unwrap();
        assert!(capped.contains("kept 0 records") && capped.contains("5 evicted"), "{capped}");
        assert!(run_str(&["generate", "--compact-cache", &store]).unwrap_err().contains("give --world"));
        let clash = run_str(&["generate", "--compact-cache", &store, "--region", "0,1"]).unwrap_err();
        assert!(clash.contains("--compact-cache takes only"), "{clash}");
        let both = run_str(&["generate", "--compact-cache", &store, "--seed", "3", "--plan-check", &store]).unwrap_err();
        assert!(both.contains("--compact-cache takes only"), "{both}");
        let lone = run_str(&["generate", "--cap-bytes", "5", "--seed", "3"]).unwrap_err();
        assert!(lone.contains("belongs to --compact-cache"), "{lone}");
        assert!(run_str(&["generate", "--compact-cache", &d.s("missing")]).unwrap_err().contains("no such cache directory"));
    }

    // spec: STRM-073
    #[test]
    fn generate_bakes_the_class_source_with_class_source_and_a_verify_without_it_compares_nothing() {
        let d = TempDir::new("class-global");
        let store = d.s("store");
        let args = |extra: &[&'static str]| {
            let mut v = vec![
                "generate",
                "--global-dir",
                store.as_str(),
                "--global-res",
                "8",
                "--seed",
                "3",
                "--threads",
                "2",
                "--prefill-levels",
                "0",
            ];
            v.extend_from_slice(extra);
            v
        };
        let built = run_str(&args(&["--class-source"])).unwrap();
        assert!(built.contains("6 blocks written, 0 already stored"), "{built}");
        let again = run_str(&args(&["--class-source"])).unwrap();
        assert!(again.contains("0 blocks written, 6 already stored"), "{again}");
        let ok = run_str(&["generate", "--verify-global", &store, "--seed", "3", "--class-source"]).unwrap();
        assert!(ok.contains("checked 19 of 19 entries: 19 match") && ok.contains("0 differ"), "{ok}");
        // The legacy world asks for other keys: the entries are stale and nothing is compared.
        let e = run_str(&["generate", "--verify-global", &store, "--seed", "3"]).unwrap_err();
        assert!(e.contains("compared nothing"), "{e}");
        // The pre-fill after the bake follows the store's recorded source (one level: the class store gets its coarse tiles through the class world).
        let pre = run_str(&[
            "generate",
            "--prefill-dir",
            &store,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--class-source",
            "--prefill-levels",
            "3",
            "--floor-level",
            "3",
        ])
        .unwrap();
        assert!(pre.contains("pre-fill"), "{pre}");
        // The same directory holds the legacy bake beside it without either reading the other's blocks.
        let legacy = run_str(&args(&[])).unwrap();
        assert!(legacy.contains("6 blocks written, 0 already stored"), "{legacy}");
    }

    // spec: STRM-012
    #[test]
    fn generate_bakes_the_global_field_into_blocks_and_verify_global_checks_them() {
        let d = TempDir::new("global");
        let store = d.s("store");
        let built =
            run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--threads", "2", "--prefill-levels", "0"])
                .unwrap();
        assert!(
            built.contains("8 cells per face edge in blocks of 8 (level 0)") && built.contains("6 blocks written, 0 already stored"),
            "{built}"
        );
        let again = run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        assert!(again.contains("0 blocks written, 6 already stored"), "{again}");
        // spec: STRM-023
        assert!(built.contains("global ocean channel: 6 blocks written, 0 already stored; "), "{built}");
        assert!(again.contains("global ocean channel: 6 blocks already stored under their current key, nothing recomputed"), "{again}");
        // spec: STRM-025
        assert!(built.contains("global water summary: written, "), "{built}");
        assert!(again.contains("global water summary: already stored under its current key, nothing recomputed"), "{again}");
        let ok = run_str(&["generate", "--verify-global", &store, "--seed", "3"]).unwrap();
        assert!(ok.contains("checked 19 of 19 entries: 19 match"), "6 blocks, 6 ocean blocks, 6 floor blocks and the water summary: {ok}");
        let sampled = run_str(&["generate", "--verify-global", &store, "--seed", "3", "--sample", "5"]).unwrap();
        assert!(sampled.contains("checked 5 of 19 entries: 5 match"), "{sampled}");
        // Another seed: all stale, nothing compared.
        let e = run_str(&["generate", "--verify-global", &store, "--seed", "4"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("--allow-stale"), "{e}");
        assert!(run_str(&["generate", "--verify-global", &store, "--seed", "4", "--allow-stale"]).is_ok());
        // A missing directory is refused, not an empty store.
        let missing = d.s("nowhere");
        assert!(run_str(&["generate", "--verify-global", &missing]).unwrap_err().contains("no such cache directory"));
        // Bad blocks are refused naming the limit.
        let bad = run_str(&["generate", "--global-dir", &d.s("bad"), "--global-res", "3"]).unwrap_err();
        assert!(!std::path::Path::new(&d.s("bad")).exists(), "a refused bake creates nothing");
        assert!(bad.contains("power-of-two"), "{bad}");
        let huge = run_str(&["generate", "--global-dir", &d.s("huge"), "--global-res", "8388608"]).unwrap_err();
        assert!(huge.contains("65536") && !huge.contains("level"), "a resolution error naming the finest grid: {huge}");
    }

    // spec: STRM-021
    #[test]
    fn generate_prefills_the_coarse_tiles_after_the_bake_or_into_an_existing_bake_and_a_second_run_writes_nothing() {
        let d = TempDir::new("prefill");
        let (a, b) = (d.s("a"), d.s("b"));
        // After the bake: level 3 is the 384 roots; the bake's own blocks are still there.
        let built = run_str(&[
            "generate",
            "--global-dir",
            &a,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--threads",
            "2",
            "--prefill-levels",
            "3",
        ])
        .unwrap();
        assert!(
            built.contains("6 blocks written")
                && built.contains("levels 3..=3")
                && built.contains("384 tiles, 384 written, 0 already stored"),
            "{built}"
        );
        let again = run_str(&[
            "generate",
            "--global-dir",
            &a,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--prefill-levels",
            "3",
        ])
        .unwrap();
        assert!(
            again.contains("0 blocks written, 6 already stored") && again.contains("384 tiles, 0 written, 384 already stored"),
            "{again}"
        );
        // The bake still verifies; the tiles are another kind to it.
        let ok = run_str(&["generate", "--verify-global", &a, "--seed", "3"]).unwrap();
        assert!(
            ok.contains("19 match") && ok.contains("384 of another kind"),
            "6 blocks, 6 ocean blocks, 6 floor blocks and the summary match: {ok}"
        );
        // Into an existing bake, a deeper pre-fill adds the new level only; levels 0 disables.
        run_str(&[
            "generate",
            "--global-dir",
            &b,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--prefill-levels",
            "0",
        ])
        .unwrap();
        let off = run_str(&[
            "generate",
            "--prefill-dir",
            &b,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--prefill-levels",
            "0",
        ])
        .unwrap();
        assert!(off.contains("disabled"), "{off}");
        let one = run_str(&[
            "generate",
            "--prefill-dir",
            &b,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--prefill-levels",
            "3",
            "--threads",
            "3",
        ])
        .unwrap();
        assert!(one.contains("384 tiles, 384 written, 0 already stored") && one.contains("on 3 threads"), "{one}");
        let two = run_str(&[
            "generate",
            "--prefill-dir",
            &b,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--prefill-levels",
            "4",
        ])
        .unwrap();
        assert!(two.contains("1920 tiles, 1536 written, 384 already stored"), "{two}");
        let none = run_str(&[
            "generate",
            "--prefill-dir",
            &b,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--floor-level",
            "3",
            "--cells",
            "16",
            "--prefill-levels",
            "4",
        ])
        .unwrap();
        assert!(none.contains("1920 tiles, 0 written, 1920 already stored") && none.contains("packs grew by 0 bytes"), "{none}");
        // Refusals: the bake's size is required, another store and combinations of commands are refused, the depth is bounded.
        for (args, needle) in [
            (vec!["generate", "--prefill-dir", b.as_str(), "--seed", "3"], "needs --global-res"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--global-dir", b.as_str()], "separate commands"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--verify-global", b.as_str()], "excludes --verify-global"),
            (vec!["generate", "--prefill-levels", "2", "--seed", "3"], "applies to --global-dir"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--prefill-levels", "9"], "at most 8"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "8", "--prefill-levels", "22"], "at most 8"),
            (vec!["generate", "--prefill-dir", d.s("nowhere").as_str(), "--global-res", "8"], "no such store directory"),
            (vec!["generate", "--prefill-dir", b.as_str(), "--global-res", "16", "--seed", "3"], "holds no bake of 16 cells"),
            (
                vec![
                    "generate",
                    "--prefill-dir",
                    b.as_str(),
                    "--global-res",
                    "8",
                    "--seed",
                    "3",
                    "--finest-level",
                    "2",
                    "--prefill-levels",
                    "2",
                ],
                "finest level 2",
            ),
            (vec!["test-render", "--prefill-dir", b.as_str()], "only applies to `generate`"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
    }

    // spec: STRM-021, LOD-011
    #[test]
    fn generate_prefills_from_the_render_floor_says_so_when_the_depth_is_below_it_and_refuses_a_floor_above_the_finest_level() {
        let d = TempDir::new("prefill-floor");
        let (a, b) = (d.s("a"), d.s("b"));
        let common = ["--global-res", "8", "--seed", "3", "--cells", "16", "--threads", "4"];
        let gen = |extra: &[&str]| {
            let mut args = vec!["generate"];
            args.extend(common);
            args.extend(extra);
            run_str(&args)
        };
        // After the bake, at the default floor 3: levels 3 are the 384 roots; a deeper run adds level 4 only; a third writes nothing.
        let built = gen(&["--global-dir", &a, "--prefill-levels", "3"]).unwrap();
        assert!(built.contains("levels 3..=3") && built.contains("384 tiles, 384 written, 0 already stored"), "{built}");
        let four = gen(&["--prefill-dir", &a, "--prefill-levels", "4"]).unwrap();
        assert!(four.contains("levels 3..=4") && four.contains("1920 tiles, 1536 written, 384 already stored"), "{four}");
        let again = gen(&["--prefill-dir", &a, "--prefill-levels", "4"]).unwrap();
        assert!(again.contains("1920 tiles, 0 written, 1920 already stored") && again.contains("packs grew by 0 bytes"), "{again}");
        // `--prefill-levels 0` disables; a depth below the floor writes nothing and says so; a floor below level 3 is refused (the compact format starts there).
        gen(&["--global-dir", &b, "--prefill-levels", "0"]).unwrap();
        let off = gen(&["--prefill-dir", &b, "--prefill-levels", "0"]).unwrap();
        assert!(off.contains("disabled"), "{off}");
        let below = gen(&["--prefill-dir", &b, "--floor-level", "3", "--prefill-levels", "1"]).unwrap();
        assert!(
            below.contains("nothing to write") && below.contains("--prefill-levels 1") && below.contains("below the render floor 3"),
            "{below}"
        );
        let zero = gen(&["--prefill-dir", &b, "--floor-level", "0", "--prefill-levels", "1"]).unwrap_err();
        assert!(zero.contains("below level 3") && zero.contains("compact format"), "a floor below level 3 draws nothing: {zero}");
        // Refusals: a floor above the finest level names both values; the viewer takes no flag; other generate commands would ignore it.
        let e = gen(&["--prefill-dir", &b, "--floor-level", "5", "--finest-level", "4", "--prefill-levels", "2"]).unwrap_err();
        assert!(e.contains("--floor-level 5") && e.contains("finest level 4"), "{e}");
        let e = run_str(&["test-render", "--scene", "terrain", "--repro", "bundle.txt", "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("does not apply to --repro"), "{e}");
        let e = run_str(&["editor", "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("the viewer takes no flag"), "{e}");
        let e = gen(&["--verify-global", &a, "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("--floor-level only applies to the pre-fill"), "{e}");
        let e = run_str(&["test-render", "--scene", "hello-triangle", "--floor-level", "3"]).unwrap_err();
        assert!(e.contains("does not apply to --scene hello-triangle"), "{e}");
        // The effective floor is the lower of the floor and the finest level: a finest level of 2 needs no flag.
        let o = parse_args(&["test-render".to_string(), "--finest-level".to_string(), "2".to_string()]).unwrap();
        assert_eq!((o.floor_for(32), o.min_level()), (2, 3));
        assert_eq!(parse_args(&["test-render".to_string()]).unwrap().floor_for(32), 3);
    }

    // spec: GEN-019
    #[test]
    fn the_compact_format_with_fewer_than_16_cells_is_refused_naming_the_minimum() {
        let e = run_str(&["test-render", "--scene", "terrain", "--cells", "8"]).unwrap_err();
        assert!(e.contains("--cells 8") && e.contains("at least 16"), "{e}");
    }

    // spec: GEN-122
    #[test]
    fn generate_writes_the_lithology_preview_maps_a_cross_section_and_the_numbers_and_refuses_what_it_excludes() {
        let d = TempDir::new("lithology-preview");
        let dir = d.s("out");
        let out = run_str(&["generate", "--lithology-preview", &dir, "--seed", "3", "--km", "400"]).unwrap();
        assert!(out.contains("lithology field, an input of the carve, not terrain") && out.contains("400 km a pixel"), "{out}");
        for f in ["hardness.png", "strata_surface.png", "strata_200m_below.png", "soluble.png", "section.png", "lithology.txt"] {
            assert!(std::path::Path::new(&dir).join(f).is_file(), "{f}");
        }
        let txt = std::fs::read_to_string(std::path::Path::new(&dir).join("lithology.txt")).unwrap();
        assert!(txt.contains("lithology field (input), not terrain") && txt.contains("band thickness 30 to 170 m"), "{txt}");
        // The preview excludes the other generate commands and a definition named twice, and `--km` belongs to it alone.
        let e = run_str(&["generate", "--lithology-preview", &dir, "--global-dir", &dir]).unwrap_err();
        assert!(e.contains("--lithology-preview excludes") && e.contains("--global-dir"), "{e}");
        let e = run_str(&["generate", "--lithology-preview", &dir, "--seed", "3", "--world", "w.toml"]).unwrap_err();
        assert!(e.contains("--world and --seed"), "{e}");
        let e = run_str(&["generate", "--km", "16", "--seed", "3"]).unwrap_err();
        assert!(
            e.contains("--km applies to --lithology-preview, --uplift-preview, --climate-preview, --arid-preview, --shelf-preview or --ice-preview"),
            "{e}"
        );
        let e = run_str(&["generate", "--lithology-preview", &dir, "--km", "0.5"]).unwrap_err();
        assert!(e.contains("--km 0.5 must be from 8 to 500"), "{e}");
        for bad in ["1", "7.9", "501", "NaN"] {
            let e = run_str(&["generate", "--lithology-preview", &dir, "--km", bad]).unwrap_err();
            assert!(e.contains("--km") && e.contains("must be from 8 to 500"), "{bad}: {e}");
        }
    }

    // spec: GEN-125
    #[test]
    fn generate_writes_the_uplift_preview_maps_a_section_and_the_numbers_and_refuses_what_it_excludes() {
        let d = TempDir::new("uplift-preview");
        let dir = d.s("out");
        let out = run_str(&["generate", "--uplift-preview", &dir, "--seed", "3", "--km", "400"]).unwrap();
        assert!(out.contains("uplift field, an input of the carve, not terrain") && out.contains("400 km a pixel"), "{out}");
        for f in ["uplift.png", "steepness.png", "section.png", "uplift.txt"] {
            assert!(std::path::Path::new(&dir).join(f).is_file(), "{f}");
        }
        let txt = std::fs::read_to_string(std::path::Path::new(&dir).join("uplift.txt")).unwrap();
        assert!(txt.contains("uplift field (input), not terrain") && txt.contains("belt or arc coast"), "{txt}");
        let e = run_str(&["generate", "--uplift-preview", &dir, "--lithology-preview", &dir]).unwrap_err();
        assert!(e.contains("excludes") && e.contains("--lithology-preview") && e.contains("--uplift-preview"), "{e}");
        let e = run_str(&["generate", "--uplift-preview", &dir, "--global-dir", &dir]).unwrap_err();
        assert!(e.contains("--uplift-preview excludes") && e.contains("--global-dir"), "{e}");
        let e = run_str(&["generate", "--uplift-preview", &dir, "--seed", "3", "--world", "w.toml"]).unwrap_err();
        assert!(e.contains("--world and --seed"), "{e}");
        let e = run_str(&["generate", "--uplift-preview", &dir, "--km", "7.9"]).unwrap_err();
        assert!(e.contains("--km") && e.contains("must be from 8 to 500"), "{e}");
    }

    // spec: GEN-130
    #[test]
    fn generate_writes_the_climate_preview_maps_the_regions_and_the_numbers_and_refuses_what_it_excludes() {
        let d = TempDir::new("climate-preview");
        let dir = d.s("out");
        let out = run_str(&["generate", "--climate-preview", &dir, "--seed", "3", "--km", "400"]).unwrap();
        assert!(out.contains("orographic climate, an input of the bake, not terrain") && out.contains("400 km a pixel"), "{out}");
        for f in [
            "precipitation_annual_wind.png",
            "precipitation_two_seasons.png",
            "precipitation_difference.png",
            "aridity.png",
            "seasonality.png",
            "wind_northern_summer.png",
            "wind_northern_winter.png",
            "regions.png",
            "orography.txt",
        ] {
            assert!(std::path::Path::new(&dir).join(f).is_file(), "{f}");
        }
        let txt = std::fs::read_to_string(std::path::Path::new(&dir).join("orography.txt")).unwrap();
        assert!(
            txt.contains("orographic climate (input), not terrain")
                && txt.contains("windward over lee ratio")
                && txt.contains("by latitude band"),
            "{txt}"
        );
        let e = run_str(&["generate", "--climate-preview", &dir, "--uplift-preview", &dir]).unwrap_err();
        assert!(e.contains("excludes") && e.contains("--uplift-preview"), "{e}");
        let e = run_str(&["generate", "--climate-preview", &dir, "--global-dir", &dir]).unwrap_err();
        assert!(e.contains("--climate-preview excludes") && e.contains("--global-dir"), "{e}");
        let e = run_str(&["generate", "--climate-preview", &dir, "--seed", "3", "--world", "w.toml"]).unwrap_err();
        assert!(e.contains("--world and --seed"), "{e}");
        let e = run_str(&["generate", "--climate-preview", &dir, "--fine-lattice", "63"]).unwrap_err();
        assert!(e.contains("--fine-lattice 63 must be from 64 to 1024"), "{e}");
        let e = run_str(&["generate", "--fine-lattice", "512", "--seed", "3"]).unwrap_err();
        assert!(e.contains("--fine-lattice applies to --climate-preview"), "{e}");
    }

    // spec: GEN-128
    #[test]
    fn generate_writes_the_arid_preview_maps_the_shares_and_the_sites_and_refuses_what_it_excludes() {
        let d = TempDir::new("arid-preview");
        let dir = d.s("out");
        let out = run_str(&["generate", "--arid-preview", &dir, "--seed", "3", "--km", "500"]).unwrap();
        assert!(out.contains("class weights, an input of the bake, not terrain") && out.contains("500 km a pixel"), "{out}");
        for f in ["classes_after.png", "classes_before.png", "badlands_weight.png", "sand_sea_weight.png", "arid.txt"] {
            assert!(std::path::Path::new(&dir).join(f).is_file(), "{f}");
        }
        let txt = std::fs::read_to_string(std::path::Path::new(&dir).join("arid.txt")).unwrap();
        assert!(
            txt.contains("arid class rules (input), not terrain")
                && txt.contains("SPIKE-77")
                && txt.contains("Erg Chebbi desert")
                && txt.contains("step of the blended k"),
            "{txt}"
        );
        let e = run_str(&["generate", "--arid-preview", &dir, "--climate-preview", &dir]).unwrap_err();
        assert!(e.contains("excludes") && e.contains("--arid-preview") && e.contains("--climate-preview"), "{e}");
        let e = run_str(&["generate", "--arid-preview", &dir, "--global-dir", &dir]).unwrap_err();
        assert!(e.contains("--arid-preview excludes") && e.contains("--global-dir"), "{e}");
        let e = run_str(&["generate", "--arid-preview", &dir, "--seed", "3", "--world", "w.toml"]).unwrap_err();
        assert!(e.contains("--world and --seed"), "{e}");
    }

    // spec: GEN-118
    #[test]
    fn generate_writes_the_shelf_preview_maps_and_numbers_and_refuses_what_it_excludes() {
        let d = TempDir::new("shelf-preview");
        let dir = d.s("out");
        let out = run_str(&["generate", "--shelf-preview", &dir, "--seed", "3", "--km", "500"]).unwrap();
        assert!(out.contains("an input of the bake, not terrain") && out.contains("500 km a pixel"), "{out}");
        assert!(std::path::Path::new(&dir).join("bathymetry.png").is_file());
        let txt = std::fs::read_to_string(std::path::Path::new(&dir).join("shelf.txt")).unwrap();
        assert!(
            txt.contains("shelf profile (input), not terrain") && txt.contains("shelf share of the ocean") && txt.contains("passive"),
            "{txt}"
        );
        let e = run_str(&["generate", "--shelf-preview", &dir, "--arid-preview", &dir]).unwrap_err();
        assert!(e.contains("excludes") && e.contains("--shelf-preview") && e.contains("--arid-preview"), "{e}");
        let e = run_str(&["generate", "--shelf-preview", &dir, "--global-dir", &dir]).unwrap_err();
        assert!(e.contains("--shelf-preview excludes") && e.contains("--global-dir"), "{e}");
        let e = run_str(&["generate", "--shelf-preview", &dir, "--seed", "3", "--world", "w.toml"]).unwrap_err();
        assert!(e.contains("--world and --seed"), "{e}");
    }

    // spec: GEN-134
    #[test]
    fn generate_writes_the_ice_preview_maps_and_numbers_and_refuses_what_it_excludes() {
        let d = TempDir::new("ice-preview");
        let dir = d.s("out");
        let out = run_str(&["generate", "--ice-preview", &dir, "--seed", "3", "--km", "500"]).unwrap();
        assert!(out.contains("the ice history, an input of the bake, not terrain") && out.contains("500 km a pixel"), "{out}");
        for f in ["ice_extent.png", "ela.png", "ice.txt"] {
            assert!(std::path::Path::new(&dir).join(f).is_file(), "{f}");
        }
        let txt = std::fs::read_to_string(std::path::Path::new(&dir).join("ice.txt")).unwrap();
        assert!(
            txt.contains("ice history (input), not terrain") && txt.contains("land under ice") && txt.contains("Scottish Highlands"),
            "{txt}"
        );
        let e = run_str(&["generate", "--ice-preview", &dir, "--shelf-preview", &dir]).unwrap_err();
        assert!(e.contains("excludes") && e.contains("--ice-preview") && e.contains("--shelf-preview"), "{e}");
        let e = run_str(&["generate", "--ice-preview", &dir, "--global-dir", &dir]).unwrap_err();
        assert!(e.contains("--ice-preview excludes") && e.contains("--global-dir"), "{e}");
        let e = run_str(&["generate", "--ice-preview", &dir, "--seed", "3", "--world", "w.toml"]).unwrap_err();
        assert!(e.contains("--world and --seed"), "{e}");
    }

    // spec: VIEW-068
    #[test]
    fn the_preview_flag_is_an_editor_option_with_a_bounded_grid_and_off_by_default() {
        let parse = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(parse(&["editor"]).unwrap().preview_l1, None, "off by default: nothing runs");
        assert_eq!(parse(&["editor", "--preview-l1"]).unwrap().preview_l1, Some(PREVIEW_GRID_DEFAULT));
        assert_eq!(parse(&["editor", "--preview-l1", "--preview-grid", "384"]).unwrap().preview_l1, Some(384));
        assert_eq!(parse(&["editor", "--preview-grid", "96"]).unwrap().preview_l1, Some(96));
        assert!(parse(&["editor", "--preview-grid", "4"]).unwrap_err().contains("8 to 3072"));
        assert!(parse(&["editor", "--preview-grid", "9999"]).unwrap_err().contains("8 to 3072"));
        assert!(parse(&["editor", "--preview-grid"]).unwrap_err().contains("--preview-grid needs"));
        assert!(run_str(&["test-render", "--preview-l1"]).unwrap_err().contains("only apply to `editor`"));
    }

    // spec: VIEW-066
    // spec: VIEW-062
    #[test]
    fn the_erosion_options_are_viewer_options_that_need_on_and_default_to_the_production_bake() {
        let parse = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let o = parse(&["editor"]).unwrap();
        assert!(!o.erosion, "off by default");
        assert_eq!((o.erosion_face_res, o.erosion_res, o.erosion_store.is_none(), o.erosion_unchecked), (None, None, true, false));
        let o = parse(&[
            "editor",
            "--erosion",
            "on",
            "--erosion-store",
            "d",
            "--erosion-face-res",
            "4096",
            "--erosion-res",
            "8192",
            "--erosion-unchecked",
        ])
        .unwrap();
        assert!(o.erosion && o.erosion_store.as_deref() == Some(std::path::Path::new("d")) && o.erosion_unchecked);
        assert_eq!((o.erosion_face_res, o.erosion_res), (Some(4096), Some(8192)));
        assert!(!parse(&["editor", "--erosion", "off"]).unwrap().erosion);
        for bad in [
            &["editor", "--erosion", "maybe"][..],
            &["editor", "--erosion"],
            &["editor", "--erosion-res", "x"],
            &["editor", "--erosion-store"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        assert!(
            USAGE.contains("--erosion on|off") && USAGE.contains("--erosion-store") && USAGE.contains("--erosion-unchecked"),
            "the usage names the options"
        );
        // The options mean nothing without --erosion on, and only the viewer draws the layer.
        let e = run_str(&["editor", "--no-window", "--erosion-store", "d"]).unwrap_err();
        assert!(e.contains("--erosion-store only applies with --erosion on"), "{e}");
        // The channels are read from the erosion layer's store: `--channels on` needs `--erosion on`, takes on or off, and is off by default (VIEW-066).
        let e = run_str(&["editor", "--no-window", "--channels", "on"]).unwrap_err();
        assert!(e.contains("--channels on only applies with --erosion on"), "{e}");
        let e = run_str(&["editor", "--no-window", "--erosion", "on", "--channels", "maybe"]).unwrap_err();
        assert!(e.contains("--channels needs on or off"), "{e}");
        assert!(USAGE.contains("--channels on|off"));
        let e = run_str(&["test-render", "--scene", "hello-triangle", "--erosion", "on"]).unwrap_err();
        assert!(e.contains("only applies to `editor`"), "{e}");
        // The stills of the product path draw the layer too (VIEW-062): the option is accepted there and the run goes on to what it needs (the poses file).
        let e = run_str(&["test-render", "--scene", "capture-stills", "--erosion", "on"]).unwrap_err();
        assert!(!e.contains("only applies to"), "{e}");
        // ... and the channels of the layer with it (VIEW-066).
        let e = run_str(&["test-render", "--scene", "capture-stills", "--erosion", "on", "--channels", "on"]).unwrap_err();
        assert!(!e.contains("only applies to") && !e.contains("--channels"), "{e}");
    }

    // spec: VIEW-063
    // spec: VIEW-066
    // spec: VIEW-064
    #[test]
    fn the_erosion_layer_is_drawn_over_a_stored_world_under_the_keys_of_its_bake_and_refuses_anything_else() {
        use planet_cache::global_store::GlobalWorld;
        use planet_cache::layer_store::{load_face_change, run_pass1_rock};
        use planet_cache::layer_tiles::{bake_specs, face_change_reader, run_pass2, BakeShape, Pass2Inputs};
        use planet_cache::pack::TileStore;
        use planet_generators::erosion::{EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID, EROSION_LAYER_ONLY_IMPLEMENTATION_ID};
        let d = TempDir::new("erosion-on");
        let (store, bake_dir) = (d.s("store"), d.0.join("bake"));
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        let def = || planet_world_def::WorldDefinition::single_planet(skeleton::RADIUS_M, 3);
        // A tiny bake of the layer (pass 1 and pass 2 at 24 cells a face edge, one tile a face) in the folder layout of `bake_layer`.
        let shape = BakeShape { seed: 3, radius_m: skeleton::RADIUS_M, face_res: 24, planet_res: 24, tile: None, halo: None };
        let world = GlobalWorld::from_definition(&def()).unwrap();
        let (layer_spec, tile_spec) = bake_specs(&world, &shape).unwrap();
        let inputs = planet_cache::global_store::class_inputs(&def(), 3, 8);
        let (base, factor) = (|v: planet_core::Vec3| inputs.base(v), |v: planet_core::Vec3| inputs.carve_factor(v));
        inputs.lithology(); // built here, on their threads, not inside the carve's parallel pass
        inputs.orography();
        let (rock_k, rock_u, rock_r) = (
            |v: planet_core::Vec3, z: f64| inputs.climate_k(v, z),
            |v: planet_core::Vec3| inputs.uplift_at(v),
            |v: planet_core::Vec3| inputs.runoff_weight(v),
        );
        let rock =
            planet_generators::bake::face::RockFields { k: Some(&rock_k), uplift: Some(&rock_u), runoff: Some(&rock_r), vegetation: true };
        let faces: Vec<planet_core::Face> = (0..6).map(planet_core::Face).collect();
        run_pass1_rock(&bake_dir.join("layer"), &world, 24, &layer_spec, &base, Some(&factor), &rock, &faces, 2).unwrap();
        let pass1 = TileStore::open(&bake_dir.join("layer"), world.params()).unwrap();
        let six: Vec<_> =
            (0..6).map(|f| load_face_change(&pass1, &world, 24, &layer_spec, planet_core::Face(f)).unwrap().expect("a face")).collect();
        let six: [planet_generators::bake::face::FaceChange; 6] = six.try_into().expect("six faces");
        let reader = face_change_reader(&six);
        run_pass2(
            &bake_dir.join("tiles"),
            &world,
            24,
            &tile_spec,
            &Pass2Inputs { base: &base, face_change: &reader, factor: Some(&factor), scratch: None, rock },
            &tile_spec.grid.all_tiles(),
            2,
            &mut |_| {},
        )
        .unwrap();
        let options = |dir: Option<PathBuf>, unchecked: bool, face_res: usize| worldfield::ErosionOptions {
            layer_dir: dir,
            face_res: Some(face_res),
            planet_res: Some(24),
            unchecked,
            channels: false,
        };
        let open = || worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap();
        // Refusals: no marker, no stored bake, a layer made under other keys, a folder with no layer.
        let e = open().with_erosion(&options(Some(bake_dir.clone()), false, 24)).err().unwrap();
        assert!(e.contains("holds the legacy generator's bake") && e.contains("--erosion-unchecked"), "{e}");
        let e = worldfield::WorldTerrain::open(def(), 8).unwrap().with_erosion(&options(Some(bake_dir.clone()), true, 24)).err().unwrap();
        assert!(e.contains("needs --store"), "{e}");
        let e = open().with_erosion(&options(Some(bake_dir.clone()), true, 48)).err().unwrap();
        assert!(
            e.contains("another bake") && e.contains("refused, not mixed"),
            "a layer of another pass 1 is refused naming both keys, not mixed: {e}"
        );
        let e = open().with_erosion(&options(Some(d.0.join("nowhere")), true, 24)).err().unwrap();
        assert!(e.contains("no such folder"), "{e}");
        // On: the bake's output folder (its tiles/ is found), the field's id and the log say it, and the heights differ from the plain world's over land.
        let plain = open();
        let on = open().with_erosion(&options(Some(bake_dir.clone()), true, 24)).unwrap();
        assert!(on.erosion_note().contains("erosion layer") && on.describe().contains("erosion layer"), "{}", on.describe());
        // A layer without a manifest takes the options, or the production shape without them (which a tiny layer was not made for: refused by its keys).
        assert!(on.erosion_note().contains("no manifest: the options and the production shape"), "{}", on.erosion_note());
        let bare = worldfield::ErosionOptions { face_res: None, planet_res: None, ..options(Some(bake_dir.clone()), true, 24) };
        let e = open().with_erosion(&bare).err().unwrap();
        assert!(e.contains("holds no layer tile") && e.contains("98304-cell"), "the production shape is not the tiny layer's: {e}");
        // The default world's field is the v1 (bilinear) one: with the layer it is upgraded to the C1 base the layer's ground is read with.
        assert_eq!(plain.id(), planet_generators::erosion::EROSION_IMPLEMENTATION_ID);
        assert_eq!(on.id(), EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID);
        assert_ne!(on.id(), plain.id());
        assert_ne!(on.id(), EROSION_LAYER_ONLY_IMPLEMENTATION_ID);
        // A field that is not an eroded one has nothing to add the layer to.
        let mut terrain_field = def();
        terrain_field.set_height_field("cpu-terrain-v1");
        let e = worldfield::WorldTerrain::open_store(terrain_field, std::path::Path::new(&store), 8)
            .unwrap()
            .with_erosion(&options(Some(bake_dir.clone()), true, 24))
            .err()
            .unwrap();
        assert!(e.contains("applies to the eroded fields"), "{e}");
        // A definition whose relief scale or sea differs from what the layer was carved on is refused.
        let mut scaled = def();
        scaled.set(&planet_world_def::body_relief_scale_path(planet_world_def::BodyId::PLANET), planet_world_def::NodeValue::F64(1.5));
        let e = worldfield::WorldTerrain::open_store(scaled, std::path::Path::new(&store), 8)
            .unwrap()
            .with_erosion(&options(Some(bake_dir.clone()), true, 24))
            .err()
            .unwrap();
        assert!(e.contains("relief scale 1") && e.contains("1.5"), "{e}");
        let mut high_sea = def();
        high_sea.set(&planet_world_def::body_sea_level_path(planet_world_def::BodyId::PLANET), planet_world_def::NodeValue::F64(100.0));
        let e = worldfield::WorldTerrain::open_store(high_sea, std::path::Path::new(&store), 8)
            .unwrap()
            .with_erosion(&options(Some(bake_dir.clone()), true, 24))
            .err()
            .unwrap();
        assert!(e.contains("sea at 0 m") && e.contains("100"), "{e}");
        let (a, b) = (plain.field().unwrap(), on.field().unwrap());
        assert_eq!(b.implementation_id(), on.id());
        let mut rng = planet_core::hash::SplitMix64(5);
        let (mut land, mut moved) = (0, 0);
        for _ in 0..400 {
            let v = planet_core::Vec3::new(rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0, rng.next_f64() * 2.0 - 1.0).normalized();
            if a.height(v) > 20.0 {
                land += 1;
                moved += usize::from((a.height(v) - b.height(v)).abs() > 1e-6);
            }
        }
        assert!(land > 30 && moved > 0, "the layer or the band changes land heights ({moved} of {land})");
        // The baked channels (VIEW-066, GEN-119): refused while the layer's folder holds no drainage area, read once the areas are copied beside the tiles; the field's version word (so its mesh keys) differs,
        // its heights do not, and it hands the tile builder the channels at a direction.
        let with = |dir: &PathBuf| worldfield::ErosionOptions { channels: true, ..options(Some(dir.clone()), true, 24) };
        let e = open().with_erosion(&with(&bake_dir)).err().unwrap();
        assert!(e.contains("--channels on") && e.contains("holds no drainage area"), "{e}");
        planet_cache::layer_channels::copy_areas(&bake_dir.join("layer"), &bake_dir.join("tiles"), &world, 24, &layer_spec, &faces)
            .unwrap();
        let chan = open().with_erosion(&with(&bake_dir)).unwrap();
        assert!(chan.erosion_note().contains("the driver block from the layer's channels"), "{}", chan.erosion_note());
        assert!(!on.erosion_note().contains("driver block"), "off by default");
        let c = chan.field().unwrap();
        assert_ne!(c.climate_version_word(), b.climate_version_word(), "the channels re-key the tiles");
        assert_eq!(b.driver_channels(&[planet_core::Vec3::new(0.3, 0.5, 0.8).normalized()]), None);
        let got = c.driver_channels(&[planet_core::Vec3::new(0.3, 0.5, 0.8).normalized()]).expect("channels");
        assert!(got[0].area_log2_cells.is_some(), "the area is read from the layer's folder");
        let v = planet_core::Vec3::new(0.2, -0.4, 0.7).normalized();
        assert_eq!(b.height(v).to_bits(), c.height(v).to_bits(), "the heights are the layer's, channels or not");
    }

    // spec: STRM-083, STRM-086
    #[test]
    fn the_erosion_layer_of_a_commit_of_the_new_chain_is_read_through_its_commit_and_refuses_the_old_channels() {
        use planet_cache::global_store::GlobalWorld;
        use planet_cache::l_writer::{bake_l, ChannelFeed, LBake};
        use planet_cache::layer_l::LTileSpec;
        use planet_generators::bake::k7::{Channel, K7Params};
        use planet_generators::bake::lakes::Lakes;
        use planet_generators::bake::tile::TileGrid;
        struct NoChannels;
        impl ChannelFeed for NoChannels {
            fn q_is_discharge(&self) -> bool {
                true
            }
            fn channels_near(&self, _: planet_core::Face, _: i64, _: i64, _: i64) -> Vec<Channel> {
                Vec::new()
            }
        }
        let d = TempDir::new("erosion-commit");
        let (store, dir) = (d.s("store"), d.0.join("lcommit"));
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        let def = || planet_world_def::WorldDefinition::single_planet(skeleton::RADIUS_M, 3);
        let world = GlobalWorld::from_definition(&def()).unwrap();
        let inputs = planet_cache::global_store::class_inputs(&def(), 3, 8);
        let base = |v: planet_core::Vec3| inputs.base(v);
        let zero = |_: planet_core::Vec3| 0.0;
        let spec = LTileSpec {
            grid: TileGrid { cells: 64, tile: 64, halo: 8, radius_m: skeleton::RADIUS_M },
            k7: K7Params::standard(3, 0.0),
            base_id: 1,
            inputs_id: 2,
        };
        let lakes = Lakes { lakes: Vec::new(), pieces: Vec::new(), dropped_cells: 0, water: Vec::new(), ring: Vec::new() };
        let b = LBake {
            world: &world,
            res: 64,
            spec,
            basin: planet_cache::basin_table::BasinSpec {
                hydro: planet_generators::bake::hydro::HydroParams::default(),
                tile_cells: 64,
                tiles_per_edge: 1,
                base_id: 1,
                inputs_id: 2,
            },
            base: &base,
            regional: &zero,
            feed: &NoChannels,
            lakes: &lakes,
            h1_cells: 64,
            h1_cell: &|_, _, _| None,
            face_of: &|_| planet_core::Face(0),
            threads: 2,
            graph: None,
        };
        let tiles: Vec<_> = spec.grid.all_tiles();
        let commit = bake_l(&dir, &b, &tiles).unwrap();
        let options = |channels: bool| worldfield::ErosionOptions {
            layer_dir: Some(dir.clone()),
            face_res: None,
            planet_res: None,
            unchecked: true,
            channels,
        };
        let open = || worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap();
        let e = open().with_erosion(&options(true)).err().unwrap();
        assert!(e.contains("holds a commit of the new chain") && e.contains("--channels off"), "{e}");
        let on = open().with_erosion(&options(false)).unwrap();
        assert!(on.erosion_note().contains(&format!("commit {:016x}", commit.id)), "{}", on.erosion_note());
        // A commit with no channel graph has no river source, and the note says so (STRM-086).
        assert!(on.rivers().is_none() && on.erosion_note().contains("no channel graph"), "{}", on.erosion_note());
        assert!(on.describe().contains("erosion layer"), "{}", on.describe());
    }

    // spec: VIEW-069
    #[test]
    fn a_commit_opened_in_memory_refuses_a_stored_world_another_base_and_other_scenes() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap_or_else(|e| panic!("{a:?}: {e}"));
        for extra in
            [&["--store", "x"][..], &["--erosion", "on"], &["--global-res", "64"], &["--allow-memory-bake"], &["--erosion-store", "y"]]
        {
            let mut a = vec!["editor", "--commit", "c"];
            a.extend_from_slice(extra);
            let e = check_commit_options(&args(&a)).unwrap_err();
            assert!(e.contains("excludes") && e.contains(extra[0]), "{extra:?}: {e}");
        }
        assert!(check_commit_options(&args(&["editor", "--commit", "c"])).is_ok());
        assert!(check_commit_options(&args(&["test-render", "--scene", "capture-stills", "--commit", "c"])).is_ok());
        let e = check_commit_options(&args(&["test-render", "--scene", "terrain", "--commit", "c"])).unwrap_err();
        assert!(e.contains("editor") && e.contains("capture-stills"), "{e}");
        assert!(check_commit_options(&args(&["editor"])).is_ok(), "no --commit, nothing to check");
    }

    // spec: VIEW-069
    #[test]
    fn a_commit_baked_for_another_seed_is_refused_before_any_base_is_computed() {
        let dir = std::env::temp_dir().join(format!("planet-view069-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let world =
            planet_cache::global_store::GlobalWorld::from_definition(&planet_world_def::WorldDefinition::single_planet(6_371_000.0, 3))
                .unwrap()
                .with_block_limit(4);
        let spec = planet_cache::region_bake::RegionSpec {
            seed: 3,
            face: planet_core::Face(2),
            radius_m: 6_371_000.0,
            regional: 64,
            region: (16, 39, 16, 39),
            margin: 6,
            tile: 64,
            halo: 24,
            rounds: 2,
            threads: 2,
            res: 64,
            base_id: 7,
            inputs_id: 11,
            height_bounds: (-11_000.0, 9_000.0),
            keep_channels: false,
        };
        let base = |d: planet_core::Vec3| 500.0 - 2400.0 * (d.x - 0.1) + 120.0 * libm::sin(40.0 * d.z);
        let climate = |_: planet_core::Vec3| (1.0e-8, 0.0, 1.0);
        planet_cache::region_bake::bake_region(&world, &spec, &base, &climate, &dir).expect("the fixture commit");
        let started = std::time::Instant::now();
        let e = worldfield::WorldTerrain::open_commit_in_memory(planet_world_def::WorldDefinition::single_planet(6_371_000.0, 4), &dir, 2)
            .err()
            .expect("another seed is refused");
        assert!(e.contains("baked for seed 3") && e.contains("4"), "{e}");
        assert!(started.elapsed().as_secs_f64() < 5.0, "refused before the in-memory bake");
        let missing = worldfield::WorldTerrain::open_commit_in_memory(
            planet_world_def::WorldDefinition::single_planet(6_371_000.0, 3),
            &dir.join("none"),
            2,
        )
        .err()
        .expect("a folder without a commit is refused");
        assert!(missing.contains("holds no commit"), "{missing}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: VIEW-065
    #[test]
    fn the_viewer_opens_the_world_a_store_records_and_the_layer_by_its_manifest() {
        use planet_cache::global_store::{FieldSource, GlobalWorld};
        use planet_cache::layer_manifest::{LayerManifest, ManifestSource};
        use planet_cache::layer_store::{load_face_change, run_pass1_rock};
        use planet_cache::layer_tiles::{bake_specs, face_change_reader, run_pass2, BakeShape, Pass2Inputs};
        use planet_cache::pack::TileStore;
        let d = TempDir::new("class-store");
        let (store, bake_dir) = (d.s("store"), d.0.join("bake"));
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--class-source", "--prefill-levels", "0"])
            .unwrap();
        let def = || planet_world_def::WorldDefinition::single_planet(skeleton::RADIUS_M, 3);
        let inputs = planet_cache::global_store::class_inputs(&def(), 3, 8);
        let world = GlobalWorld::from_definition(&def()).unwrap().with_class_source(inputs.clone());
        // The viewer's terrain over the store is the class world, named so (the title and the log).
        let class = worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap();
        assert_eq!(class.field_source_name(), "class source");
        assert!(class.describe().contains("class source"), "{}", class.describe());
        // A tiny layer carved on the class source, with its manifest (the range of what it holds).
        let shape = BakeShape { seed: 3, radius_m: skeleton::RADIUS_M, face_res: 24, planet_res: 24, tile: None, halo: None };
        let (layer_spec, tile_spec) = bake_specs(&world, &shape).unwrap();
        let (base, factor) = (|v: planet_core::Vec3| inputs.base(v), |v: planet_core::Vec3| inputs.carve_factor(v));
        inputs.lithology(); // built here, on its threads, not inside the carve's parallel pass
        inputs.orography();
        let (rock_k, rock_u, rock_r) = (
            |v: planet_core::Vec3, z: f64| inputs.climate_k(v, z),
            |v: planet_core::Vec3| inputs.uplift_at(v),
            |v: planet_core::Vec3| inputs.runoff_weight(v),
        );
        let rock =
            planet_generators::bake::face::RockFields { k: Some(&rock_k), uplift: Some(&rock_u), runoff: Some(&rock_r), vegetation: true };
        let faces: Vec<planet_core::Face> = (0..6).map(planet_core::Face).collect();
        run_pass1_rock(&bake_dir.join("layer"), &world, 24, &layer_spec, &base, Some(&factor), &rock, &faces, 2).unwrap();
        let pass1 = TileStore::open(&bake_dir.join("layer"), world.params()).unwrap();
        let six: Vec<_> =
            (0..6).map(|f| load_face_change(&pass1, &world, 24, &layer_spec, planet_core::Face(f)).unwrap().expect("a face")).collect();
        let six: [planet_generators::bake::face::FaceChange; 6] = six.try_into().expect("six faces");
        let reader = face_change_reader(&six);
        let mut manifest = LayerManifest::new(&world, &shape).unwrap();
        run_pass2(
            &bake_dir.join("tiles"),
            &world,
            24,
            &tile_spec,
            &Pass2Inputs { base: &base, face_change: &reader, factor: Some(&factor), scratch: None, rock },
            &tile_spec.grid.all_tiles(),
            2,
            &mut |r| {
                let lo = r.core.iter().fold(0.0f32, |m, v| m.min(*v));
                let hi = r.core.iter().fold(0.0f32, |m, v| m.max(*v));
                manifest.widen(f64::from(lo), f64::from(hi));
            },
        )
        .unwrap();
        manifest.write(&bake_dir.join("tiles")).unwrap();
        let options = |dir: &std::path::Path, unchecked: bool| worldfield::ErosionOptions {
            layer_dir: Some(dir.to_path_buf()),
            face_res: None,
            planet_res: None,
            unchecked,
            channels: false,
        };
        let open = || worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap();
        // On a class store with the layer's manifest: no marker flag, no shape options; the manifest gives the shape and the range, and the log says so.
        let on = open().with_erosion(&options(&bake_dir, false)).unwrap();
        assert!(on.erosion_note().contains("its manifest") && on.erosion_note().contains("pass 1 at 24 cells"), "{}", on.erosion_note());
        assert!(on.describe().contains("class source") && on.describe().contains("erosion layer"), "{}", on.describe());
        // The overrides replace the manifest's cells: a pass 1 the layer was not made for is refused by its keys.
        let over = worldfield::ErosionOptions { face_res: Some(48), ..options(&bake_dir, false) };
        let e = open().with_erosion(&over).err().unwrap();
        assert!(e.contains("another bake") && e.contains("refused, not mixed"), "{e}");
        // A manifest of another version or of another field source is refused, naming it.
        let mut old = manifest.clone();
        old.pass2_kernel_version += 1;
        old.write(&bake_dir.join("tiles")).unwrap();
        let e = open().with_erosion(&options(&bake_dir, false)).err().unwrap();
        assert!(e.contains("another version") && e.contains("pass-2 kernel"), "{e}");
        let mut legacy = manifest.clone();
        legacy.source = ManifestSource::Legacy;
        legacy.write(&bake_dir.join("tiles")).unwrap();
        let e = open().with_erosion(&options(&bake_dir, false)).err().unwrap();
        assert!(e.contains("legacy generator") && e.contains("class source (identity"), "{e}");
        // Another seed in the manifest is refused naming both.
        let mut other_seed = manifest.clone();
        other_seed.shape.seed = 4;
        other_seed.write(&bake_dir.join("tiles")).unwrap();
        let e = open().with_erosion(&options(&bake_dir, false)).err().unwrap();
        assert!(e.contains("seed 4") && e.contains("seed 3"), "{e}");
        manifest.write(&bake_dir.join("tiles")).unwrap();
        // The manifest's change range is the layer's bound (not the representable one), and the replaced cells show in the log line.
        let (opened, _, shape_used) = planet_cache::layer_tiles::StoreLayer::open_recorded(
            &bake_dir,
            std::sync::Arc::new(world.clone()),
            &BakeShape::production(3, skeleton::RADIUS_M),
            planet_cache::layer_tiles::ShapeOverride::default(),
            4,
        )
        .unwrap();
        assert_eq!(planet_generators::erosion_layer::LayerSource::global_range(&opened), manifest.range());
        assert!(manifest.range().1 < planet_cache::layer_tiles::default_change_range().1, "a tighter bound than the representable range");
        assert_eq!((shape_used.face_res, shape_used.planet_res), (24, 24), "the shape is the manifest's, not the production one");
        // The title names the world's source and whether the layer is on (VIEW-065).
        assert_eq!(window::title_world_label(None, 8, "legacy source", false), "preview bake 8 cells, legacy source");
        assert_eq!(
            window::title_world_label(Some(std::path::Path::new("s")), 8, "class source", true),
            "store s, class source + erosion layer"
        );
        // A store that holds both bakes refuses without a choice and opens with it (STRM-073).
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        let e = worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).err().unwrap();
        assert!(e.contains("legacy bake and a class-source bake") && e.contains("--class-source"), "{e}");
        let chosen = |c| worldfield::WorldTerrain::open_store_as(def(), std::path::Path::new(&store), 8, Some(c)).unwrap();
        assert_eq!(chosen(FieldSource::Class).field_source_name(), "class source");
        assert_eq!(chosen(FieldSource::Legacy).field_source_name(), "legacy source");
        // A legacy world with `--erosion-unchecked` still never draws a layer whose manifest says class source.
        let e = chosen(FieldSource::Legacy).with_erosion(&options(&bake_dir, true)).err().unwrap();
        assert!(e.contains("class source (identity") && e.contains("legacy generator"), "{e}");
        // The viewer's options carry the choice.
        let o = parse_args(&["editor".to_string(), "--legacy-source".to_string()]).unwrap();
        assert_eq!(o.field_source_choice().unwrap(), Some(FieldSource::Legacy));
    }

    // spec: VIEW-065
    #[test]
    fn plan_check_and_the_coarse_horizon_open_the_world_the_store_records() {
        let d = TempDir::new("class-store-readers");
        let store = d.s("store");
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--class-source", "--prefill-levels", "0"])
            .unwrap();
        // The plan check opens the class world (it found no bake there when it built the legacy world by hand).
        let ok = run_str(&["generate", "--plan-check", &store, "--global-res", "8", "--seed", "3"]).unwrap();
        assert!(ok.contains("plan-check"), "{ok}");
        // The coarse horizon opens it too: what it finds missing is the coupled run, not the bake.
        let e = run_str(&["generate", "--coarse-horizon", &store, "--global-res", "8", "--seed", "3"]).unwrap_err();
        assert!(e.contains("coupled run") && !e.contains("holds no bake"), "{e}");
        // A store holding both bakes asks which, for both commands, and takes the choice.
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        for command in ["--plan-check", "--coarse-horizon"] {
            let e = run_str(&["generate", command, &store, "--global-res", "8", "--seed", "3"]).unwrap_err();
            assert!(e.contains("legacy bake and a class-source bake") && e.contains("--class-source"), "{command}: {e}");
        }
        assert!(run_str(&["generate", "--plan-check", &store, "--global-res", "8", "--seed", "3", "--legacy-source"]).is_ok());
    }

    // spec: VIEW-061
    #[test]
    fn the_camera_phase_decodes_no_block_on_the_main_thread_and_the_ground_arrives_pinned_and_exact() {
        // A cold store: the camera's first ground read decodes a block. The frame polls the ground thread inside its camera phase; the decode
        // happens on that thread, the phase records none, and the answer equals the field's own height bit for bit with its blocks pinned.
        let d = TempDir::new("camera-ground");
        let store = d.s("store");
        run_str(&["generate", "--global-dir", &store, "--global-res", "8", "--seed", "3", "--prefill-levels", "0"]).unwrap();
        let def = planet_world_def::WorldDefinition::single_planet(skeleton::RADIUS_M, 3);
        let world = std::sync::Arc::new(worldfield::WorldTerrain::open_store(def, std::path::Path::new(&store), 8).unwrap());
        assert!(world.pinned_blocks().is_empty(), "nothing pinned before the first ground read");
        let w = std::sync::Arc::clone(&world);
        let mut ground = cameraground::CameraGround::new(std::sync::Arc::new(move |d| w.ground_pinned(d)));
        let nadir = planet_core::Vec3::new(0.3, -0.6, 0.74).normalized();
        let mut probe = stall::StallProbe::default();
        let start = std::time::Instant::now();
        let answer = loop {
            probe.begin();
            let a = ground.poll(nadir);
            probe.mark("camera, ground heights and the globe overview");
            assert!(probe.main_decodes().is_empty(), "the camera phase decoded a block on the main thread: {:?}", probe.main_decodes());
            if let Some(a) = a {
                break a;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(20), "the ground never arrived");
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(answer.0, nadir);
        let exact = world.field().unwrap().height(nadir);
        assert_eq!(answer.1.to_bits(), exact.to_bits(), "the ground thread reads the same field: {} against {exact}", answer.1);
        assert!(!world.pinned_blocks().is_empty(), "the blocks under the camera are pinned");
    }

    // spec: STRM-021, STRM-022
    #[test]
    fn a_pre_filled_tile_read_by_the_streamer_equals_the_generated_one_bit_for_bit_and_a_cold_reader_ignores_the_pack() {
        use planet_streaming::TileSource;
        let d = TempDir::new("prefill-read");
        let store = d.s("store");
        run_str(&[
            "generate",
            "--global-dir",
            &store,
            "--global-res",
            "8",
            "--seed",
            "3",
            "--prefill-levels",
            "4",
            "--floor-level",
            "3",
            "--threads",
            "4",
            "--cells",
            "16",
        ])
        .unwrap();
        let def = || planet_world_def::WorldDefinition::single_planet(skeleton::RADIUS_M, 3);
        let world = std::sync::Arc::new(worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap());
        // Levels 3..=4 (the render floor to the pre-fill depth) are all present for the run's cells and finest level.
        assert_eq!(world.prefill_present(3, 4, 16, 22).unwrap(), (1920, 1920));
        assert_eq!(world.prefill_present(3, 3, 16, 22).unwrap(), (384, 384));
        assert_eq!(world.prefill_present(3, 5, 16, 22).unwrap(), (1920, 1920 + 6 * 1024), "level 5 was not pre-filled");
        assert_eq!(world.prefill_present(3, 4, 32, 22).unwrap(), (0, 1920), "other cells");
        let src = |w: &std::sync::Arc<worldfield::WorldTerrain>| skeleton::MeshSource {
            world: std::sync::Arc::clone(w),
            max_level: 22,
            cells: 16,
        };
        let cold = std::sync::Arc::new(worldfield::WorldTerrain::open_store(def(), std::path::Path::new(&store), 8).unwrap().cold());
        let field = world.field().unwrap();
        let mut sampled = 0;
        for tile in planet_cache::meshpack::prefill_tiles_from(3, 4)
            .into_iter()
            .step_by(11)
            .chain(planet_cache::meshpack::prefill_tiles_from(3, 4).into_iter().take(8))
        {
            let on_demand = skeleton::tile_for(field.as_ref(), tile, 16, world.radius_m());
            let before = world.prefill_hits();
            assert_eq!(src(&world).load(tile).unwrap(), on_demand, "level {} tile {:#x} from the pack", tile.level(), tile.raw());
            assert_eq!(world.prefill_hits(), before + 1, "served from the pack");
            let staged = src(&world).load_staged(tile, &planet_streaming::CancelToken::never()).unwrap();
            assert_eq!(staged, Some(on_demand.clone()));
            assert_eq!(src(&cold).load(tile).unwrap(), on_demand, "the cold reader generates the same tile");
            sampled += 1;
        }
        assert_eq!(cold.prefill_hits(), 0, "the cold reader never reads the pack");
        // The recorded-only counters (STRM-022): each sampled tile was looked up twice (load and load_staged) and found; the cold
        // reader counted nothing; the pack saw those lookups (and the blocks the on-demand tiles read).
        let c = world.counters();
        assert_eq!((c.prefill_lookups, c.prefill_found, c.prefill_skipped), (2 * sampled, 2 * sampled, 0), "{c:?}");
        assert_eq!(cold.counters().prefill_lookups, 0);
        assert!(c.pack.gets >= c.prefill_lookups && c.pack.found >= c.prefill_found, "{c:?}");
        assert!(c.field_builds >= 2 * sampled, "every load builds its field: {c:?}");
        assert!(!cold.reads_prefilled() && world.reads_prefilled() && sampled > 50);
        // A tile deeper than the pre-fill is generated, not found.
        let deep = planet_core::TileId::new(planet_core::Face(0), 5, 1, 1).unwrap();
        let before = world.prefill_hits();
        src(&world).load(deep).unwrap();
        assert_eq!(world.prefill_hits(), before);
        // A tile deeper than the pre-fill but within the readable levels is looked up and missed; one beyond them is skipped at once.
        let after = world.counters().since(&c);
        assert_eq!((after.prefill_lookups, after.prefill_found, after.prefill_skipped), (1, 0, 0), "{after:?}");
        let beyond = planet_core::TileId::new(planet_core::Face(0), 9, 1, 1).unwrap();
        src(&world).load(beyond).unwrap();
        let after = world.counters().since(&c);
        assert_eq!((after.prefill_lookups, after.prefill_skipped), (1, 1), "{after:?}");
    }

    // spec: STRM-012
    #[test]
    fn the_global_commands_refuse_options_they_would_ignore_and_combinations_of_commands() {
        let d = TempDir::new("global-refuse");
        let s = d.s("store");
        for (args, needle) in [
            (vec!["generate", "--global-dir", s.as_str(), "--verify-global", s.as_str()], "separate commands"),
            (vec!["generate", "--global-dir", s.as_str(), "--cache-dir", s.as_str()], "exclude"),
            (vec!["generate", "--global-dir", s.as_str(), "--sample", "2"], "only apply to --verify-global"),
            (vec!["generate", "--verify-global", s.as_str(), "--global-res", "64"], "excludes --global-res"),
            (vec!["generate", "--global-dir", s.as_str(), "--global-products", s.as_str()], "separate commands"),
            (vec!["generate", "--cache-dir", s.as_str(), "--region", "0,1", "--global-res", "64"], "exclude"),
            (vec!["generate", "--tile", "0,1,0,0", "--global-res", "64"], "belongs to the tile store"),
            (vec!["generate", "--cache-dir", s.as_str(), "--region", "0,1", "--threads", "2"], "exclude --threads"),
            (vec!["generate", "--verify-global", s.as_str(), "--threads", "2"], "excludes --threads"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
        let e = run_str(&["test-render", "--global-dir", s.as_str()]).unwrap_err();
        assert!(e.contains("only applies to `generate`"), "{e}");
    }

    // spec: STRM-012
    #[test]
    fn global_products_writes_cube_nets_and_maps_for_review() {
        let d = TempDir::new("global-products");
        let dir = d.s("img");
        let out = run_str(&["generate", "--global-products", &dir, "--global-res", "16", "--seed", "3", "--threads", "2"]).unwrap();
        assert!(out.contains("4 cube nets and 4 maps"), "{out}");
        for ch in ["continent", "elevation", "temperature", "precipitation"] {
            assert!(std::path::Path::new(&dir).join(format!("{ch}-net.png")).is_file(), "{ch} net");
            assert!(std::path::Path::new(&dir).join(format!("{ch}-map.png")).is_file(), "{ch} map");
        }
    }

    // spec: STRM-004
    #[test]
    fn cache_verify_of_an_existing_empty_directory_compared_nothing_and_fails_even_with_allow_stale() {
        let d = TempDir::new("empty-store");
        let store = d.s("store");
        std::fs::create_dir_all(&store).unwrap();
        let e = run_str(&["generate", "--verify-cache", &store, "--seed", "3"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("0 entries"), "{e}");
        let e = run_str(&["generate", "--verify-cache", &store, "--seed", "3", "--allow-stale"]).unwrap_err();
        assert!(e.contains("compared nothing") && e.contains("does not help"), "{e}");
    }

    // spec: STRM-004
    #[test]
    fn cache_verify_fails_naming_a_tile_made_with_an_input_missing_from_its_key() {
        let d = TempDir::new("bad-store");
        let store = d.s("store");
        run_str(&["generate", "--cache-dir", &store, "--region", "0,1", "--res", "8", "--seed", "3"]).unwrap();
        // Append a tile whose key is the seed-3 world's but whose payload was made with seed 5.
        let world =
            planet_cache::tiles::World::from_definition(&planet_world_def::WorldDefinition::single_planet(RADIUS_M_TEST, 3)).unwrap();
        let t = planet_core::TileId::new(planet_core::Face(1), 2, 1, 1).unwrap();
        let wrong = planet_generators::generate_tile(5, &planet_core::cube::TangentWarp, t, 8);
        let mut bad = world.tile_record(t, 8).unwrap();
        bad.payload = planet_cache::codec::encode_height_tile(&wrong).unwrap();
        let mut w =
            planet_cache::pack::PackWriter::open(std::path::Path::new(&store), world.params(), planet_cache::pack::DEFAULT_MAX_PACK_BYTES)
                .unwrap();
        w.append(&bad).unwrap();
        w.finish().unwrap();
        let mut buf = Vec::new();
        let args: Vec<String> = ["generate", "--verify-cache", &store, "--seed", "3"].iter().map(|s| s.to_string()).collect();
        let e = run(&args, &mut buf).unwrap_err();
        let printed = String::from_utf8(buf).unwrap();
        assert!(e.contains("1 differing"), "{e}");
        assert!(
            printed.contains("MISMATCH tile")
                && printed.contains("face 1 level 2 x 1 y 1")
                && printed.contains("at offset")
                && printed.contains("missing from its key"),
            "{printed}"
        );
        assert!(printed.contains("4 match") && printed.contains("1 differ"), "{printed}");
    }

    const RADIUS_M_TEST: f64 = 6_371_000.0;

    // spec: STRM-004
    #[test]
    fn generate_reads_the_definition_from_a_toml_file_and_refuses_inconsistent_options() {
        let d = TempDir::new("world");
        let world = d.s("world.toml");
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 3\n[body.0]\nradius_m = 6371000.0\n").unwrap();
        let store = d.s("store");
        // The file's world is the default single planet with seed 3: same keys, so the seed-3 store verifies against it.
        run_str(&["generate", "--cache-dir", &store, "--region", "2,1", "--res", "4", "--seed", "3"]).unwrap();
        let ok = run_str(&["generate", "--verify-cache", &store, "--world", &world]).unwrap();
        assert!(ok.contains("4 match"), "{ok}");
        std::fs::write(&world, "[world]\nseed = 3\n").unwrap();
        assert!(run_str(&["generate", "--verify-cache", &store, "--world", &world]).unwrap_err().contains("schema_version"));
        assert!(run_str(&["generate", "--verify-cache", &store, "--world", &d.s("missing.toml")]).unwrap_err().contains("reading"));
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 3\n").unwrap();
        assert!(run_str(&["generate", "--verify-cache", &store, "--world", &world]).unwrap_err().contains("body.0.radius_m"));

        // The store options belong to `generate`'s store commands and nowhere else, and nothing is silently ignored.
        for (args, needle) in [
            (vec!["generate", "--verify-cache", store.as_str(), "--world", world.as_str(), "--seed", "3"], "give one"),
            (vec!["generate", "--verify-cache", store.as_str(), "--region", "0,1"], "only reads"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--sample", "0"], "only apply to --verify-cache"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--allow-stale"], "only apply to --verify-cache"),
            (vec!["generate", "--manifest", "m.txt", "--world", world.as_str()], "--world"),
            (vec!["generate", "--manifest", "m.txt", "--cache-dir", store.as_str()], "--cache-dir"),
            (vec!["editor", "--no-window", "--world", world.as_str()], "only applies to `generate`"),
            (vec!["editor", "--smoke", "--sample", "3"], "only applies to `generate`"),
            (vec!["test-render", "--cache-dir", store.as_str()], "only applies to `generate`"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,7"], "--region must be"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
        let missing = d.s("no-such-store");
        let e = run_str(&["generate", "--verify-cache", &missing, "--seed", "3"]).unwrap_err();
        assert!(e.contains("no such cache directory"), "{e}");
        let too_big = run_str(&["generate", "--cache-dir", &d.s("big"), "--region", "0,6", "--res", "2048"]).unwrap_err();
        assert!(too_big.contains("use a lower level or resolution"), "{too_big}");
        let no_region = run_str(&["generate", "--cache-dir", &store]).unwrap_err();
        assert!(no_region.contains("--region"), "{no_region}");
        for (args, needle) in [
            (vec!["generate", "--region", "0,1"], "belongs to the tile store commands"),
            (vec!["generate", "--sample", "3"], "belongs to the tile store commands"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--sample", "3"], "only apply to --verify-cache"),
            (vec!["generate", "--verify-cache", store.as_str(), "--cache-dir", store.as_str()], "only reads"),
            (vec!["generate", "--verify-cache", store.as_str(), "--res", "8"], "only reads"),
            (vec!["generate", "--cache-dir", store.as_str(), "--region", "0,1", "--tile", "0,0,0,0"], "exclude"),
            (vec!["generate", "--region", "6,1", "--cache-dir", store.as_str()], "--region must be"),
            (vec!["generate", "--region", "0,7", "--cache-dir", store.as_str()], "--region must be"),
            (vec!["generate", "--region", "x", "--cache-dir", store.as_str()], "--region must be"),
        ] {
            let e = run_str(&args).unwrap_err();
            assert!(e.contains(needle), "{args:?}: {e}");
        }
    }

    // spec: BUILD-004
    #[test]
    fn all_three_run_modes_start() {
        // The editor's `--smoke` opens the window (tier C, `tests/gpu_window.rs`, VIEW-005); `--no-window` is the start check that
        // needs no display.
        for (mode, args) in [
            ("editor", vec!["editor", "--smoke", "--no-window"]),
            ("generate", vec!["generate", "--smoke"]),
            ("test-render", vec!["test-render", "--smoke"]),
        ] {
            let out = run_str(&args).unwrap_or_else(|e| panic!("{mode}: {e}"));
            assert!(out.contains("smoke ok"), "{mode}: {out}");
        }
    }

    // spec: VIEW-001
    #[test]
    fn the_window_refuses_a_software_adapter_outside_test_mode_before_anything_starts() {
        // No window, no display and no GPU is touched: the policy is checked first.
        let e = run_str(&["editor", "--adapter", "software"]).unwrap_err();
        assert!(e.contains("test and CI mode") && e.contains("discrete GPU"), "{e}");
        // The smoke is the test mode, and the viewer's options parse.
        let o = parse_args(&[
            "editor".into(),
            "--smoke".into(),
            "--no-vsync".into(),
            "--size".into(),
            "800x450".into(),
            "--view".into(),
            "level".into(),
        ])
        .unwrap();
        assert!(o.smoke && o.no_vsync && o.size == Some((800, 450)) && o.view == "level");
    }

    // spec: BUILD-004
    #[test]
    fn bad_arguments_are_actionable() {
        assert!(run_str(&["frobnicate"]).unwrap_err().contains("usage:"));
        assert!(run_str(&["test-render", "--adapter", "gpu"]).unwrap_err().contains("software or hardware"));
        assert!(run_str(&["test-render", "--scene"]).unwrap_err().contains("needs a value"));
    }

    // spec: VIEW-026
    #[test]
    fn a_path_replay_without_what_it_needs_is_refused_before_any_gpu_work() {
        let run = |a: &[&str]| {
            let mut v = vec!["test-render", "--scene", "terrain"];
            v.extend_from_slice(a);
            run_str(&v).unwrap_err()
        };
        assert!(run(&["--replay-path", "p.txt", "--out-dir", "d"]).contains("--replay-path needs --repro"));
        assert!(run(&["--replay-path", "p.txt", "--repro", "b.bundle"]).contains("--replay-path needs --out-dir"));
        assert!(run(&["--replay-every", "5"]).contains("--replay-every only applies to --replay-path"));
        assert!(run(&["--replay-path", "p.txt", "--repro", "b.bundle", "--out-dir", "d", "--replay-every", "x"]).contains("--replay-every"));
    }

    // spec: TEST-013
    #[test]
    fn flight_options_that_would_be_ignored_are_rejected_before_any_gpu_work() {
        let d = std::env::temp_dir().join(format!("planet-flight-args-{}", std::process::id()));
        let p = d.to_str().unwrap();
        let flight = |extra: &[&str]| {
            let mut a = vec!["test-render", "--scene", "terrain", "--flight", "12", "--out-dir", p];
            a.extend_from_slice(extra);
            run_str(&a).unwrap_err()
        };
        assert!(flight(&["--perf-smoke"]).contains("excludes --perf-smoke"));
        assert!(flight(&["--perf", "--machine-ready"]).contains("excludes --perf"));
        assert!(flight(&["--repro", "x.repro"]).contains("excludes --repro"));
        assert!(flight(&["--script", "6", "--tile", "0,0,0,0"]).contains("--tile, --script"));
        let e = run_str(&["test-render", "--scene", "tiles", "--flight", "12", "--out-dir", p]).unwrap_err();
        assert!(e.contains("only available for --scene terrain"), "{e}");
        assert!(run_str(&["test-render", "--scene", "terrain", "--flight", "12"]).unwrap_err().contains("needs --out-dir"));
        assert!(run_str(&["test-render", "--scene", "terrain", "--size", "160x120"])
            .unwrap_err()
            .contains("--size only applies to --flight"));
        assert!(run_str(&["test-render", "--out-dir", p]).unwrap_err().contains("--out-dir only applies to --flight"));
        assert!(run_str(&["test-render", "--size", "12x12"]).unwrap_err().contains("--size must be WxH"));
        assert!(!d.exists(), "no rejected run may create the output directory");
    }

    // spec: BUDG-012
    #[test]
    fn the_reserve_option_is_parsed() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(args(&["test-render", "--perf", "--reserve"]).unwrap().reserve);
        assert!(!args(&["test-render", "--perf"]).unwrap().reserve);
    }

    // spec: BUILD-030
    #[test]
    fn loaded_mode_is_an_option_and_excludes_the_owners_override() {
        let args = |a: &[&str]| parse_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(args(&["test-render", "--perf", "--loaded"]).unwrap().loaded);
        assert!(!args(&["test-render", "--perf"]).unwrap().loaded);
        let e = args(&["test-render", "--perf", "--loaded", "--machine-ready"]).unwrap_err();
        assert!(e.contains("--loaded and --machine-ready exclude each other"), "{e}");
    }

    // spec: GEN-003
    #[test]
    fn generate_manifest_writes_the_determinism_manifest() {
        let path = std::env::temp_dir().join(format!("planet-manifest-{}", std::process::id())).join("m.txt");
        let out = run_str(&["generate", "--manifest", path.to_str().unwrap()]).unwrap();
        assert!(out.contains("wrote"), "{out}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), crate::manifest::full_manifest());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert!(run_str(&["generate", "--manifest", "x.txt", "--tile", "0,0,0,0"]).unwrap_err().contains("excludes --tile"));
        assert!(run_str(&["generate", "--manifest"]).unwrap_err().contains("needs a path"));
    }
}
