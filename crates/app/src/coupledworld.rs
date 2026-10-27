//! The global coupled run (GEN-040) in a world's sources (M8-1 part c): the run's input read from the world's bake, the stored channel loaded when
//! it is there (STRM-043, keyed by the run's parameters and a hash of its inputs) and the run made and stored when it is not, and the slot the
//! viewer asks for it from a thread (the run takes minutes at 2048 cells; no frame waits for it, CON-22). GPU-free.

use std::path::Path;
use std::sync::{Arc, Mutex};

use planet_cache::global_store::{load_coupled, store_coupled, CoupledSpec, GlobalWorld};
use planet_cache::pack::{TileStore, DEFAULT_MAX_PACK_BYTES};
use planet_core::{Face, TileId};
use planet_generators::coupled::{run_with, stationary_uplift, CoupledInput, CoupledMaps, CoupledParams, Lattice};
use planet_generators::coupledinput::{declared_inputs_hash, reduce_bake_with};
use planet_generators::global::LatticeSource;
use planet_world_def::{BodyId, GenContext, WorldDefinition};

/// Steps of the run a world version takes (the middle of the 70 to 100 of the plan).
pub const COUPLED_STEPS: u32 = 80;

/// Cells per face edge of the run on a bake that has at least that many (see [`run_cells`]).
pub const COUPLED_CELLS: u32 = 2048;

/// The factor of the stationary uplift (GEN-063): 1 makes the bake a steady state of the stream power law, so the run keeps its relief; lower
/// values let it keep eroding. A declared parameter of the run, part of its key.
pub const UPLIFT_RHO: f64 = 1.0;

/// Whether a world's coupled run erodes (GEN-065): off for M8, so the run keeps the relief of its input (the stream-power steps did not hold the
/// input at their steady state on the real bake: ADR 0080 point 4) and gives the river network and the areas; the stream power code stays for
/// M9's calibration. Part of the key of the stored run.
pub const COUPLED_ERODE: bool = false;

/// The parameters of a world's run.
pub fn run_params() -> CoupledParams {
    CoupledParams { erode: COUPLED_ERODE, ..CoupledParams::default() }
}

/// The erodibility of the stream power law: a placeholder until the lithology driver exists (ADR 0076). An input, so the hash in the key
/// covers it.
pub const ERODIBILITY: f32 = 2.0e-5;

/// Where the maps of a run came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Read back from the stored channel.
    Stored,
    /// Made by the run (and written to the store when there is one).
    Ran,
}

/// A run ready for the viewer.
pub struct CoupledRun {
    pub maps: Arc<CoupledMaps>,
    pub origin: Origin,
    /// What went wrong writing the result to the store; the run itself is good.
    pub store_error: Option<String>,
}

/// Cells per face edge of the run's lattice on a bake of `bake_res` cells: the run is capped at 2048 (its cost grows with the cells), and a
/// smaller bake is run at its own resolution. Both are powers of two, so the result divides the bake (GEN-056).
pub fn run_cells(bake_res: u32) -> u32 {
    bake_res.min(COUPLED_CELLS)
}

/// Threads the reduction of a bake onto the run's lattice uses; the result does not depend on it (CON-14).
fn reduce_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get()).min(8)
}

/// The input of the run on the lattice `lat` from the bake `src` (a finer lattice whose cells nest in the run's): the bake's elevation reduced
/// by the box filter of GEN-056, the constant erodibility and the stationary uplift of those heights (`UPLIFT_RHO`, GEN-063). A read error
/// names the vertex.
pub fn bake_input(src: &dyn LatticeSource, lat: &Lattice, params: &CoupledParams) -> Result<CoupledInput, String> {
    let started = std::time::Instant::now();
    let last_say = std::sync::atomic::AtomicU64::new(0);
    // STRM-051: a line at most every 10 s with the share done and the projected total (the viewer's thread and the command both print it).
    let say = |done: usize, total: usize| {
        let secs = started.elapsed().as_secs();
        if done < total && (secs < last_say.load(std::sync::atomic::Ordering::Relaxed) + 10 || secs == 0) {
            return;
        }
        last_say.store(secs, std::sync::atomic::Ordering::Relaxed);
        let share = done as f64 / total as f64;
        eprintln!(
            "coupled run: reducing the bake, band {done} of {total} ({:.0} %), {secs} s so far, about {:.0} s in all",
            share * 100.0,
            secs as f64 / share
        );
    };
    let z = reduce_bake_with(src, lat.n() as u32, reduce_threads(), &say).map_err(|e| e.to_string())?;
    let erodibility = vec![ERODIBILITY; z.len()];
    // The uplift is read only by the stream-power steps; a run that only fills does not make it.
    let uplift = if params.erode { stationary_uplift(lat, &z, &erodibility, params, UPLIFT_RHO) } else { vec![0.0; z.len()] };
    Ok(CoupledInput { uplift, erodibility, z })
}

/// The maps read from the stored channel when the store in `dir` holds the run of `def` on `n` cells per face edge from the bake `src` (same
/// parameters, same bake); `None` when it does not. The run is not made and the bake is not read: this is the viewer's test for a world that
/// has a channel (STRM-046).
pub fn load_stored(
    def: &WorldDefinition,
    dir: Option<&Path>,
    src: &dyn LatticeSource,
    n: u32,
    steps: u32,
) -> Result<Option<CoupledRun>, String> {
    // Without a directory there is no channel.
    if dir.is_none() {
        return Ok(None);
    }
    Ok(prepare(def, src, n, steps)?.load(dir))
}

/// The world that keys the run of `def` on `n` cells per face edge from the bake `src` for `steps` steps (the stored channels beside it, such
/// as the coarse horizon, are keyed through it); the bake is not read.
pub fn keyed_world(def: &WorldDefinition, src: &dyn LatticeSource, n: u32, steps: u32) -> Result<GlobalWorld, String> {
    Ok(prepare(def, src, n, steps)?.world)
}

/// What a run of `def` needs: the lattice, the spec of the channel and the world that keys it. The bake is read only when the run is made.
struct Prepared {
    lat: Lattice,
    spec: CoupledSpec,
    world: GlobalWorld,
    n: u32,
    steps: u32,
    radius_m: f64,
    id: u64,
}

/// The key of the run: `n` and the bake's resolution (`src.res()`, never a number the caller states) are in the spec, the bake's stage key is
/// the source of every block's key, and the inputs enter by their declared identity (STRM-043), so the key is made without reading the bake.
/// `src` is the bake of `def` at its own resolution: the key names that bake, and the relief scale the source carries over it
/// (`LatticeSource::relief_scale`, WDEF-017) enters the inputs' identity, so a scaled source is never stored under the key of the unscaled one.
fn prepare(def: &WorldDefinition, src: &dyn LatticeSource, n: u32, steps: u32) -> Result<Prepared, String> {
    let bake_res = src.res();
    if n == 0 || bake_res < n || !bake_res.is_multiple_of(n) || !(bake_res / n).is_power_of_two() {
        return Err(format!(
            "a coupled run of {n} cells per face edge cannot start from a bake of {bake_res} cells: the run must divide the bake by a power of two"
        ));
    }
    let radius_m = GenContext::new(def).body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let lat = Lattice::new(n as usize, radius_m);
    let spec = CoupledSpec {
        n,
        steps,
        bake_res,
        params: run_params(),
        inputs: declared_inputs_hash(radius_m, UPLIFT_RHO, ERODIBILITY, src.relief_scale()),
    };
    let world = GlobalWorld::from_definition(def).map_err(|e| format!("the definition cannot be generated: {e}"))?.with_coupled(spec);
    let (level, cells) = world.layout(n).map_err(|e| format!("a coupled run of {n} cells: {e:?}"))?;
    let key = world.coupled_key(TileId::new(Face(0), level, 0, 0).expect("tile of the layout"), cells).0;
    let id = u64::from_le_bytes(key[..8].try_into().expect("8 bytes"));
    Ok(Prepared { lat, spec, world, n, steps, radius_m, id })
}

impl Prepared {
    fn load(&self, dir: Option<&Path>) -> Option<CoupledRun> {
        let dir = dir?;
        // A fresh store each time: blocks written by an earlier run of this process are not in an index opened before them.
        let store = TileStore::open(dir, self.world.params()).ok()?;
        let stored = load_coupled(&store, &self.world).ok()?;
        Some(CoupledRun {
            maps: Arc::new(CoupledMaps::new(Lattice::new(self.n as usize, self.radius_m), stored.z, stored.area_m2, self.id)),
            origin: Origin::Stored,
            store_error: None,
        })
    }
}

/// The maps of the run of `def` on `n` cells per face edge, from the stored channel in `dir` when it holds this run (same parameters and
/// inputs), else from the run, which is then written to `dir`. `src` is the bake the run starts from, at any resolution `n` divides (GEN-056).
pub fn load_or_run(def: &WorldDefinition, dir: Option<&Path>, src: &dyn LatticeSource, n: u32, steps: u32) -> Result<CoupledRun, String> {
    let p = prepare(def, src, n, steps)?;
    if let Some(r) = p.load(dir) {
        return Ok(r);
    }
    make(p, src, dir)
}

/// Make the run (reduce the bake, run the steps) and write it to `dir` when given.
fn make(p: Prepared, src: &dyn LatticeSource, dir: Option<&Path>) -> Result<CoupledRun, String> {
    let started = std::time::Instant::now();
    let input = bake_input(src, &p.lat, &p.spec.params)?;
    eprintln!("coupled run: bake of {} cells reduced to {} in {:.1} s", src.res(), p.n, started.elapsed().as_secs_f64());
    let state = run_with(&p.lat, &input, &p.spec.params, p.steps as usize, |n, steps| {
        eprintln!("coupled run: step {n} of {steps}, {:.1} s", started.elapsed().as_secs_f64());
    });
    let store_error =
        dir.and_then(|d| store_coupled(d, &p.world, &state, DEFAULT_MAX_PACK_BYTES).err().map(|e| format!("{}: {e}", d.display())));
    Ok(CoupledRun { maps: Arc::new(CoupledMaps::new(p.lat, state.z, state.area_m2, p.id)), origin: Origin::Ran, store_error })
}

// spec: GEN-088
/// The directory a re-made run is written to: created if it is missing, never the store the bake is read from, and under a `target` directory
/// (a re-make is a measurement; the shared store stays as it is). Returns the canonical path.
pub fn remake_target(store: &Path, out: &Path) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out_c = std::fs::canonicalize(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let store_c = std::fs::canonicalize(store).map_err(|e| format!("{}: {e}", store.display()))?;
    if out_c.starts_with(&store_c) {
        return Err(format!(
            "the re-made run would be written into the store it reads ({}): name a directory under target/",
            store.display()
        ));
    }
    if !out_c.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target")) {
        return Err(format!("{}: a re-made run is written under a `target` directory, never beside the shared store", out_c.display()));
    }
    Ok(out_c)
}

// spec: GEN-042
/// Make the run of `def` from the bake `src` afresh, ignoring any stored channel, and write it to `out` (checked by [`remake_target`] against
/// `store`, the directory the bake is read from). Returns the run and the wall time of the make in seconds (GEN-088, criterion 4 of M8).
pub fn remake(
    def: &WorldDefinition,
    store: &Path,
    out: &Path,
    src: &dyn LatticeSource,
    n: u32,
    steps: u32,
) -> Result<(CoupledRun, f64), String> {
    let out = remake_target(store, out)?;
    let p = prepare(def, src, n, steps)?;
    let started = std::time::Instant::now();
    let run = make(p, src, Some(&out))?;
    if let Some(e) = &run.store_error {
        return Err(format!("the run was made but not stored: {e}"));
    }
    Ok((run, started.elapsed().as_secs_f64()))
}

// spec: STRM-051
/// `generate --coupled-run DIR --global-res N [--world F | --seed S]`: make the coupled run of the bake stored in `DIR` (or find it there) and
/// store it; prints the stages as the viewer's make does and the wall time.
pub fn generate_coupled_run(o: &crate::Options, out: &mut dyn std::io::Write) -> Result<(), String> {
    use crate::err;
    let dir = o.coupled_run.as_ref().expect("checked by the caller");
    if !o.given.contains("--global-res") {
        return Err(
            "--coupled-run needs --global-res N, the resolution the bake was made at (a store holds one bake per resolution)".into()
        );
    }
    if o.given.contains("--world") && o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if !dir.is_dir() {
        return Err(format!("{}: no such store directory (a bake made by `generate --global-dir`)", dir.display()));
    }
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
    };
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let store = TileStore::open(dir, world.params()).map_err(|e| e.to_string())?;
    let src = planet_cache::global_store::StoreSource::new(&store, &world, o.global_res).map_err(|e| e.to_string())?;
    let n = run_cells(o.global_res);
    writeln!(
        out,
        "coupled run: {n} cells per face edge from the bake of {} cells, {COUPLED_STEPS} steps, {} threads for the reduction",
        o.global_res,
        reduce_threads()
    )
    .map_err(err)?;
    let started = std::time::Instant::now();
    let run = load_or_run(&def, Some(dir), &src, n, COUPLED_STEPS)?;
    if let Some(e) = &run.store_error {
        return Err(format!("the run was made but not stored: {e}"));
    }
    let how = match run.origin {
        Origin::Stored => "found in the store",
        Origin::Ran => "made and stored",
    };
    writeln!(out, "coupled run: {how} in {} in {:.1} s", dir.display(), started.elapsed().as_secs_f64()).map_err(err)
}

// spec: GEN-042
/// The make of `perf coupled --store DIR --global-res N --remake-into OUT`: the coupled run of the bake stored in `DIR` made afresh and written
/// to `OUT`; the wall seconds of the make (reduction of the bake and the steps, as the viewer's thread makes it) and the cells per face edge.
pub fn timed_remake(o: &crate::Options, store: &Path, out: &Path) -> Result<(f64, u32), String> {
    if !o.given.contains("--global-res") {
        return Err("--remake-into needs --global-res N, the resolution the bake was made at".into());
    }
    let def = match &o.world {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
            planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => WorldDefinition::single_planet(crate::skeleton::RADIUS_M, o.seed),
    };
    let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
    let bake = TileStore::open(store, world.params()).map_err(|e| e.to_string())?;
    let src = planet_cache::global_store::StoreSource::new(&bake, &world, o.global_res).map_err(|e| e.to_string())?;
    let n = run_cells(o.global_res);
    let (_, secs) = remake(&def, store, out, &src, n, COUPLED_STEPS)?;
    Ok((secs, n))
}

/// Where a world's coupled run stands.
#[derive(Clone)]
pub enum Standing {
    /// Not asked for.
    Idle,
    /// The thread is loading or running it.
    Working,
    Ready(Arc<CoupledMaps>, Origin),
    Failed(String),
}

/// The slot a world keeps its coupled run in: asked for once, made on a thread, read by whoever draws.
pub struct CoupledSlot {
    inner: Arc<Mutex<Standing>>,
}

impl Default for CoupledSlot {
    fn default() -> Self {
        CoupledSlot { inner: Arc::new(Mutex::new(Standing::Idle)) }
    }
}

impl CoupledSlot {
    pub fn standing(&self) -> Standing {
        crate::stall::lock(&self.inner, "coupled run slot").clone()
    }

    /// Start the load or run on a thread, once: a later call while it works or after it ended does nothing.
    pub fn request(&self, job: impl FnOnce() -> Result<CoupledRun, String> + Send + 'static) {
        {
            let mut s = crate::stall::lock(&self.inner, "coupled run slot");
            if !matches!(*s, Standing::Idle) {
                return;
            }
            *s = Standing::Working;
        }
        let inner = Arc::clone(&self.inner);
        std::thread::Builder::new()
            .name("coupled-run".into())
            .spawn(move || {
                let result = job();
                *inner.lock().expect("coupled slot") = match result {
                    Ok(r) => {
                        if let Some(e) = &r.store_error {
                            eprintln!("coupled run: not stored: {e}");
                        }
                        Standing::Ready(r.maps, r.origin)
                    }
                    Err(e) => Standing::Failed(e),
                };
            })
            .expect("spawn the coupled run's thread");
    }
}

/// The words the window title carries while a lens is on (VIEW-053): what is shown and where the run stands.
pub fn lens_status(lens: planet_generators::coupled::CoupledLens, standing: &Standing) -> String {
    use planet_generators::coupled::CoupledLens;
    let what = match lens {
        CoupledLens::Height => "coupled heights (temperature view)",
        CoupledLens::Area => "coupled drainage area (precipitation view)",
    };
    match standing {
        Standing::Idle | Standing::Working => format!("{what}: the run is working"),
        Standing::Ready(_, Origin::Stored) => format!("{what}, stored run"),
        Standing::Ready(_, Origin::Ran) => format!("{what}, run just made"),
        Standing::Failed(e) => format!("{what}: no run: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::cube::TangentWarp;
    use planet_generators::chain::OwnedBake;
    use planet_generators::coupled::{lens_climate, CoupledLens};
    use planet_generators::global::GlobalCoarseGenerator;
    use planet_world_def::WorldDefinition;

    const RES: u32 = 16;

    fn def() -> WorldDefinition {
        WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 11)
    }

    fn bake(def: &WorldDefinition) -> OwnedBake {
        let field = GlobalCoarseGenerator.field(&GenContext::new(def), BodyId::PLANET).expect("the field");
        OwnedBake(field.bake(&TangentWarp, RES, 2).expect("the bake"))
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("planet-coupled-world-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    // spec: STRM-044
    #[test]
    fn a_run_is_made_once_stored_and_then_loaded_bit_for_bit() {
        let (d, src) = (def(), bake(&def()));
        let dir = tmp("once");
        let first = load_or_run(&d, Some(&dir), &src, RES, 4).unwrap();
        assert_eq!((first.origin, first.store_error.as_deref()), (Origin::Ran, None), "nothing stored yet: it runs and writes");
        let second = load_or_run(&d, Some(&dir), &src, RES, 4).unwrap();
        assert_eq!(second.origin, Origin::Stored, "the same run is read, not made again");
        assert_eq!(second.maps.id(), first.maps.id());
        let probe = |m: &CoupledMaps| {
            [CoupledLens::Height, CoupledLens::Area]
                .map(|l| (0..6u8).map(|f| lens_climate(m, l, &TangentWarp, TileId::new(Face(f), 0, 0, 0).unwrap())).collect::<Vec<_>>())
        };
        let (a, b) = (probe(&first.maps), probe(&second.maps));
        assert_eq!(a, b, "the stored run reads back as the run made");
        // Another step count is another run: its key differs, so the stored one is not taken for it.
        let third = load_or_run(&d, Some(&dir), &src, RES, 5).unwrap();
        assert_eq!(third.origin, Origin::Ran);
        assert_ne!(third.maps.id(), first.maps.id());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: GEN-043
    // spec: STRM-044
    #[test]
    fn the_lens_shows_the_run_once_it_is_ready_and_leaves_the_heights_alone() {
        use crate::worldfield::WorldTerrain;
        let world = Arc::new(WorldTerrain::open(def(), RES).unwrap());
        let tile = TileId::new(Face(0), 1, 0, 0).unwrap();
        let dirs = [tile.sample_direction(&TangentWarp, 8, 3, 4)];
        let (want_climate, want_word, want_height) = {
            let plain = world.field().unwrap();
            (plain.climate(&TangentWarp, tile), plain.climate_version_word(), plain.heights_until(&dirs, &|| false)[0].to_bits())
        };
        world.set_coupled_lens(Some(CoupledLens::Area));
        // Until the run is ready the fields stay plain: no frame waits for it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            match world.coupled_standing() {
                Standing::Ready(..) => break,
                Standing::Failed(e) => panic!("{e}"),
                _ => {
                    assert!(std::time::Instant::now() < deadline, "the run did not end");
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
        let lensed = world.field().unwrap();
        assert_ne!(lensed.climate(&TangentWarp, tile), want_climate, "the climate views show the run");
        assert_ne!(lensed.climate_version_word(), want_word, "so the tiles are re-keyed and refreshed");
        assert_eq!(lensed.heights_until(&dirs, &|| false)[0].to_bits(), want_height, "the heights are the base's");
        drop(lensed);
        world.set_coupled_lens(None);
        let off = world.field().unwrap();
        assert_eq!(off.climate(&TangentWarp, tile), want_climate, "lens off: the plain field again");
        assert_eq!(off.climate_version_word(), want_word);
    }

    fn bake_of(def: &WorldDefinition, res: u32) -> OwnedBake {
        let field = GlobalCoarseGenerator.field(&GenContext::new(def), BodyId::PLANET).expect("the field");
        OwnedBake(field.bake(&TangentWarp, res, 2).expect("the bake"))
    }

    /// A bake that must not be read: only its resolution is known.
    struct Unread(u32);

    impl LatticeSource for Unread {
        fn res(&self) -> u32 {
            self.0
        }
        fn at(&self, _: Face, _: u32, _: u32) -> Result<planet_generators::global::GlobalSample, planet_generators::global::ReadError> {
            panic!("a stored run is found without reading the bake");
        }
    }

    // spec: GEN-056
    #[test]
    fn a_source_with_a_relief_scale_has_its_own_key_and_a_scale_of_one_keeps_the_key() {
        use planet_generators::global::ScaledLattice;
        let d = def();
        let plain = Unread(4 * RES);
        let id = |s: &dyn LatticeSource| prepare(&d, s, RES / 2, 3).unwrap().id;
        assert_eq!(id(&ScaledLattice::new(&plain, 1.0)), id(&plain), "a scale of 1 is the bake");
        let (two, three) = (id(&ScaledLattice::new(&plain, 2.0)), id(&ScaledLattice::new(&plain, 3.0)));
        assert!(two != id(&plain) && three != id(&plain) && two != three, "each scale has its own run");
        assert_eq!(id(&ScaledLattice::new(&ScaledLattice::new(&plain, 2.0), 1.5)), three, "scales multiply");
    }

    // spec: GEN-056
    #[test]
    fn the_run_covers_at_most_2048_cells_and_a_smaller_bake_is_run_at_its_own_resolution() {
        assert_eq!([16384, 8192, 4096, 2048, 1024, 16].map(run_cells), [2048, 2048, 2048, 2048, 1024, 16]);
        for bake in [16384u32, 4096, 1024] {
            assert_eq!(bake % run_cells(bake), 0, "the run divides the bake");
        }
    }

    // spec: GEN-056
    // spec: STRM-043
    #[test]
    fn a_run_from_a_finer_bake_is_stored_under_its_own_key_and_loaded_without_reading_the_bake() {
        let d = def();
        let (fine, coarse) = (bake_of(&d, 4 * RES), bake_of(&d, RES));
        let (n, dir) = (RES / 2, tmp("finer"));
        let made = load_or_run(&d, Some(&dir), &fine, n, 3).unwrap();
        assert_eq!((made.origin, made.store_error.as_deref()), (Origin::Ran, None));
        assert_eq!(made.maps.lattice().n(), n as usize, "the run is on its own lattice, not the bake's");
        // Found again without a single read of the bake.
        let again = load_or_run(&d, Some(&dir), &Unread(4 * RES), n, 3).unwrap();
        assert_eq!(again.origin, Origin::Stored);
        assert_eq!(again.maps.id(), made.maps.id());
        assert!(load_stored(&d, Some(&dir), &Unread(4 * RES), n, 3).unwrap().is_some());
        // The same run on another bake resolution, or another lattice on this bake, or another step count: not this key, nothing is taken for it.
        assert!(load_stored(&d, Some(&dir), &Unread(2 * RES), n, 3).unwrap().is_none(), "another bake resolution");
        assert!(load_stored(&d, Some(&dir), &Unread(4 * RES), RES, 3).unwrap().is_none(), "another lattice");
        assert!(load_stored(&d, Some(&dir), &Unread(4 * RES), n, 4).unwrap().is_none(), "another step count");
        let other_bake = load_or_run(&d, Some(&dir), &coarse, n, 3).unwrap();
        assert_eq!(other_bake.origin, Origin::Ran, "the stored run of the finer bake is not read for the coarser one");
        assert_ne!(other_bake.maps.id(), made.maps.id());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: STRM-043
    #[test]
    fn another_world_over_the_same_resolutions_has_another_key() {
        let (a, b) = (def(), WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M, 12));
        let (ka, kb) = (prepare(&a, &Unread(4 * RES), RES / 2, 3).unwrap(), prepare(&b, &Unread(4 * RES), RES / 2, 3).unwrap());
        assert_eq!(ka.id, prepare(&a, &Unread(4 * RES), RES / 2, 3).unwrap().id, "the key is a function of the run and the bake");
        assert_ne!(ka.id, kb.id, "another seed is another bake");
        let small = WorldDefinition::single_planet(planet_core::EARTH_RADIUS_M / 2.0, 11);
        assert_ne!(ka.id, prepare(&small, &Unread(4 * RES), RES / 2, 3).unwrap().id, "another radius is another run");
    }

    // spec: GEN-056
    #[test]
    fn a_run_that_does_not_divide_the_bake_is_refused_before_anything_is_read() {
        let e = load_or_run(&def(), None, &Unread(RES), 3, 2).err().expect("refused");
        assert!(e.contains("must divide the bake"), "{e}");
        assert!(load_or_run(&def(), None, &Unread(RES), 2 * RES, 2).is_err(), "a run finer than the bake");
    }

    // spec: STRM-044
    #[test]
    fn a_world_without_a_store_runs_in_memory() {
        let (d, src) = (def(), bake(&def()));
        let r = load_or_run(&d, None, &src, RES, 3).unwrap();
        assert_eq!((r.origin, r.store_error), (Origin::Ran, None));
    }

    // spec: STRM-044
    #[test]
    fn the_slot_works_once_and_reports_what_it_made() {
        let slot = CoupledSlot::default();
        assert!(matches!(slot.standing(), Standing::Idle));
        let (d, src) = (def(), bake(&def()));
        slot.request(move || load_or_run(&d, None, &src, RES, 2));
        // A second request while it works or after it ended starts nothing.
        slot.request(|| Err("must not run".into()));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            match slot.standing() {
                Standing::Ready(_, Origin::Ran) => break,
                Standing::Working => {
                    assert!(std::time::Instant::now() < deadline, "the run did not end");
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => panic!("unexpected standing: {}", matches!(other, Standing::Failed(_))),
            }
        }
        slot.request(|| Err("must not run".into()));
        assert!(matches!(slot.standing(), Standing::Ready(..)));
    }
}
