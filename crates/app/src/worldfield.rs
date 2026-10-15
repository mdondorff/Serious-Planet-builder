//! The terrain field of a run, chosen by the world (GEN-018): `test-render --scene terrain --world FILE` resolves the
//! definition's `world.height_field` (the eroded field by default, `cpu-terrain-v1` or the reference when named, an unknown id
//! refused with the known ids) and builds it over an in-memory bake of `--global-res` cells per face edge. A run that names no
//! world file is the seed-only single planet and names `cpu-ref-height-v2` explicitly: every golden, plan snapshot and repro
//! bundle was made with it, so they do not move.

use std::path::Path;

use planet_cache::global_store::GlobalWorld;
use planet_core::cube::TangentWarp;
use planet_core::Face;
use planet_generators::global::{GlobalCoarseGenerator, GlobalGrid, GlobalSample, GridSource, LatticeSource, ReadError};
use planet_generators::height::{HeightField, IMPLEMENTATION_ID as REFERENCE_ID};
use planet_generators::height_fields::{build_field, resolve, HeightFieldKind};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::Options;

/// The largest in-memory bake a run builds (cells per face edge): 1024 is about 100 MB.
pub const MAX_RUN_BAKE_RES: u32 = 1024;

/// The definition of a run that names no world file: the single planet of `seed` with the reference field named explicitly.
pub fn legacy_world(seed: u64, radius_m: f64) -> WorldDefinition {
    let mut def = WorldDefinition::single_planet(radius_m, seed);
    def.set_height_field(REFERENCE_ID);
    def
}

/// Load a definition file.
pub fn load_world(path: &Path) -> Result<WorldDefinition, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The reference field reads no bake; this stands in for one.
struct NoBake;

impl LatticeSource for NoBake {
    fn res(&self) -> u32 {
        1
    }

    fn at(&self, _: Face, _: u32, _: u32) -> Result<GlobalSample, ReadError> {
        Err(ReadError::Source("this height field reads no bake".into()))
    }
}

/// An in-memory bake the run owns.
struct OwnedBake(GlobalGrid);

impl LatticeSource for OwnedBake {
    fn res(&self) -> u32 {
        GridSource(&self.0).res()
    }

    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        GridSource(&self.0).at(face, i, j)
    }
}

/// A world's terrain: the definition, the field it selects and the bake the field reads.
pub struct WorldTerrain {
    def: WorldDefinition,
    kind: HeightFieldKind,
    src: Box<dyn LatticeSource + Send>,
    has_bake: bool,
    bake_key: [u8; 16],
    res: u32,
    radius_m: f64,
}

impl WorldTerrain {
    /// Resolve the height field of `def` (refusing an unknown id) and bake what it reads at `global_res` cells per face edge.
    pub fn open(def: WorldDefinition, global_res: u32) -> Result<WorldTerrain, String> {
        let kind = resolve(&def).map_err(|e| e.to_string())?;
        let radius_m =
            GenContext::new(&def).body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition cannot be generated: {e}"))?;
        let (src, bake_key): (Box<dyn LatticeSource + Send>, [u8; 16]) = if kind.needs_bake() {
            if global_res > MAX_RUN_BAKE_RES {
                return Err(format!("--global-res {global_res}: a run bakes in memory, at most {MAX_RUN_BAKE_RES} cells per face edge"));
            }
            let field = GlobalCoarseGenerator
                .field(&GenContext::new(&def), BodyId::PLANET)
                .map_err(|e| format!("the definition cannot be generated: {e}"))?;
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let grid = field.bake(&TangentWarp, global_res, threads).map_err(|e| format!("--global-res {global_res}: {e:?}"))?;
            let world = GlobalWorld::from_definition(&def).map_err(|e| format!("the definition cannot be generated: {e}"))?;
            (Box::new(OwnedBake(grid)), world.stage_key(global_res).0)
        } else {
            (Box::new(NoBake), [0; 16])
        };
        Ok(WorldTerrain { def, kind, src, has_bake: kind.needs_bake(), bake_key, res: global_res, radius_m })
    }

    /// The terrain of a run that views, streams or records: the world file's, or the seed-only world with the reference field
    /// (every golden, plan snapshot and old repro bundle was made with it).
    pub fn of_options(o: &Options, radius_m: f64) -> Result<WorldTerrain, String> {
        let def = match &o.world {
            Some(path) => load_world(path)?,
            None => legacy_world(o.seed, radius_m),
        };
        WorldTerrain::open(def, o.global_res)
    }

    /// The terrain of the perf scene: the world file's, or the default world of `--seed` (its field is the eroded one,
    /// GEN-018) over an in-memory bake of `--global-res` cells per face edge. The perf scene measures the world a user gets.
    pub fn of_perf_options(o: &Options, radius_m: f64) -> Result<WorldTerrain, String> {
        let def = match &o.world {
            Some(path) => load_world(path)?,
            None => WorldDefinition::single_planet(radius_m, o.seed),
        };
        WorldTerrain::open(def, o.global_res)
    }

    /// The definition this terrain was built from.
    pub fn definition(&self) -> &WorldDefinition {
        &self.def
    }

    /// Cells per face edge of the bake the field reads (the requested size even for a field that reads none).
    pub fn global_res(&self) -> u32 {
        self.res
    }

    /// The implementation id of the field.
    pub fn id(&self) -> &'static str {
        self.kind.id()
    }

    pub fn radius_m(&self) -> f64 {
        self.radius_m
    }

    /// A line for the run's output: which field, over which bake.
    pub fn describe(&self) -> String {
        if self.has_bake {
            format!("height field {} over a {}-cell bake", self.kind.id(), self.res)
        } else {
            format!("height field {}", self.kind.id())
        }
    }

    /// Build the field (a fresh context per field: its key covers what it read).
    pub fn field(&self) -> Result<Box<dyn HeightField + '_>, String> {
        let ctx = GenContext::new(&self.def);
        build_field(self.kind, &ctx, BodyId::PLANET, self.src.as_ref(), self.bake_key)
            .map_err(|e| format!("the definition cannot be generated: {e}"))
    }
}

/// Reject what a world-selected terrain run cannot do, before any GPU work (GEN-018). `--world` and `--global-res` belong to
/// the terrain scene: a single view (with `--stream` or `--bundle`), `--flight`, `--perf` and `--perf-smoke`; `--repro` takes
/// the world from the bundle. Without `--world` a view, stream, flight or bundle is the seed-only world (reference field, so
/// `--global-res` has nothing to bake), while the perf scene is the default world (eroded field). An unknown height-field id
/// is refused here with the known ids.
pub fn check_options(o: &Options) -> Result<(), String> {
    // The global-bake perf scene takes the --world and --global-res of the global commands (its own checks).
    if o.perf && o.scene == "global-bake" {
        return Ok(());
    }
    let perf_terrain = (o.perf || o.perf_smoke) && o.scene == "terrain";
    let given_res = o.given.contains("--global-res");
    if o.world.is_none() {
        return if given_res && !perf_terrain {
            Err(if o.repro.is_some() {
                "--global-res does not apply to --repro (the bundle records its bake size)".to_string()
            } else {
                "--global-res only applies with --world, or to --perf (the seed-only world's reference field reads no bake)".to_string()
            })
        } else {
            Ok(())
        };
    }
    if o.given.contains("--seed") {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if o.scene != "terrain" {
        return Err("--world applies to `test-render --scene terrain` (a view, --flight or --perf), not to other scenes".into());
    }
    if o.repro.is_some() {
        return Err(
            "--repro takes the world from the bundle (a bundle records its definition, height field and bake size): drop --world".into()
        );
    }
    let def = load_world(o.world.as_ref().expect("checked above"))?;
    resolve(&def).map_err(|e| e.to_string())?;
    Ok(())
}
