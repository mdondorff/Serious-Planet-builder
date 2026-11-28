//! The regions file (WDEF-019): the named region boxes of a world, `<world-dir>/regions.toml`, outside the `WorldDefinition` like the site bookmarks. Each box is a rectangle of whole L tiles of one
//! face; [`bake_entry`] bakes it with the region bake (GEN-177) over the production base of the world's definition with its items (CON-05) into `<regions-parent>/regions/<name>/`, a commit the viewer
//! opens with `--commit` (VIEW-069). The state of a box (`unbaked`, `ready`, `stale`) is derived from that commit, never stored. [`check_box`] is the one geometry rule the editor (E2) and the bake share:
//! every box it accepts passes [`RegionSpec::check`].

use std::path::{Path, PathBuf};

use planet_core::{Face, Vec3};
use planet_generators::global::{read_elevation_c1, GlobalSample, LatticeSource, ReadError};
use planet_generators::runoff::{discharge_weight, pet_mm, runoff_mm};
use planet_world_def::{BodyId, GenContext, WorldDefinition};

use crate::global_store::{class_inputs, class_inputs_key, GlobalWorld};
use crate::l_writer::Commit;
use crate::region_bake::{bake_region, lattice_direction, RegionReport, RegionSpec, FACE_EDGE_CELLS, LOCAL_PER_REGIONAL};

/// The file's `format_version`.
pub const FORMAT_VERSION: u32 = 1;
/// The file name beside the world definition.
pub const FILE_NAME: &str = "regions.toml";
/// The folder beside the regions file that holds one commit folder per region.
pub const COMMITS_DIR: &str = "regions";
/// Regional cells added round a box for the solve and the hydrology in the production layout (the margin E1 and E2 share).
pub const SOLVE_MARGIN_CELLS: usize = 200;
/// The smallest and largest side of a box, in L tiles.
pub const MIN_TILES: u32 = 2;
pub const MAX_TILES: u32 = 4;
/// The longest region name.
pub const MAX_NAME: usize = 40;
/// The base's identity: the lattice tag of `bake_region` (the example's) with the lattice resolution.
const LATTICE_TAG: u64 = 0x6c61_7474_6963_6531;
/// Earth's height span (the base heights plus the profile's largest uplift, GEN-175), as the G-RT fixture bakes it.
pub const HEIGHT_BOUNDS: (f64, f64) = (-11_000.0, 9_000.0);
const SECONDS_PER_YEAR: f64 = 31_557_600.0;

/// The grid a region is baked on: the regional (H1) cells a face edge, the L tile and halo in local cells, the solve margin and rounds, and the base lattice's nodes a face edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionLayout {
    pub regional: usize,
    pub tile: usize,
    pub halo: usize,
    pub margin: usize,
    pub rounds: usize,
    pub res: u32,
}

impl RegionLayout {
    /// The production layout: 12,288 regional cells, 768-cell L tiles (128 a face edge), halo 64, a margin of [`SOLVE_MARGIN_CELLS`], 4 rounds, the 16,384-node lattice (the G-RT fixture's).
    pub const PRODUCTION: RegionLayout =
        RegionLayout { regional: 12_288, tile: 768, halo: 64, margin: SOLVE_MARGIN_CELLS, rounds: 4, res: 16_384 };

    /// L tiles a face edge.
    pub fn tiles_per_edge(&self) -> u32 {
        (self.regional * LOCAL_PER_REGIONAL / self.tile.max(1)) as u32
    }

    /// Regional cells an L tile side.
    pub fn regional_per_tile(&self) -> usize {
        self.tile / LOCAL_PER_REGIONAL
    }

    /// The lowest and highest tile index a box may cover on either axis: the solve margin (and the face-edge band of `RegionSpec::check`) stays on the face.
    /// Whether boxes can be baked in this layout at all: whole regional cells a tile, a halo under half a tile, a lattice of at least one node a regional cell, and room for the C1
    /// stencil (two nodes) between the solved window and the face edge, so no read of the base falls on another face.
    pub fn check(&self) -> Result<(), String> {
        if self.tile < 2 * LOCAL_PER_REGIONAL
            || !self.tile.is_multiple_of(LOCAL_PER_REGIONAL)
            || !(self.regional * LOCAL_PER_REGIONAL).is_multiple_of(self.tile)
        {
            return Err(format!(
                "the layout's tile of {} local cells is not a whole number (2 or more) of regional cells dividing the face",
                self.tile
            ));
        }
        if 2 * self.halo >= self.tile {
            return Err(format!("the layout's halo of {} cells is half its tile of {} or more", self.halo, self.tile));
        }
        if (self.res as usize) < self.regional {
            return Err(format!("the layout's lattice of {} nodes is coarser than its {} regional cells", self.res, self.regional));
        }
        let (lo, _) = self.allowed_tiles();
        let first = lo as usize * self.regional_per_tile() - self.margin;
        if first * self.res as usize / self.regional < 2 {
            return Err(format!(
                "the layout's margin of {} regional cells leaves no room for the base's stencil at the face edge",
                self.margin
            ));
        }
        Ok(())
    }

    pub fn allowed_tiles(&self) -> (u32, u32) {
        let m = self.margin.max(FACE_EDGE_CELLS);
        let per = self.regional_per_tile().max(1);
        let lo = m.div_ceil(per);
        let hi = (self.regional.saturating_sub(m) / per).saturating_sub(1);
        (lo as u32, hi as u32)
    }
}

/// One `[[region]]`: a name and an inclusive rectangle of L tiles `i0..=i1`, `j0..=j1` of a face on a grid of `tiles_per_edge` tiles a face edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionEntry {
    pub name: String,
    pub face: u8,
    pub tiles_per_edge: u32,
    pub i0: u32,
    pub j0: u32,
    pub i1: u32,
    pub j1: u32,
}

/// Whether a box of L tiles can be baked in `layout`: the face, the tile grid of the layout, a side of [`MIN_TILES`] to [`MAX_TILES`], inside [`RegionLayout::allowed_tiles`], then
/// [`RegionSpec::check`]. Allocation-free on success; the editor calls it on every drag step.
#[allow(clippy::too_many_arguments)]
pub fn check_box(layout: &RegionLayout, face: u8, tiles_per_edge: u32, i0: u32, j0: u32, i1: u32, j1: u32) -> Result<(), String> {
    layout.check()?;
    if face > 5 {
        return Err(format!("face {face} does not exist (0 to 5)"));
    }
    if tiles_per_edge != layout.tiles_per_edge() {
        return Err(format!("tiles_per_edge is {tiles_per_edge}, the bake's L tiles are {} a face edge", layout.tiles_per_edge()));
    }
    if i0 > i1 || j0 > j1 {
        return Err(format!("the box ({i0}, {j0})..=({i1}, {j1}) is empty: i0 <= i1 and j0 <= j1"));
    }
    let (w, h) = ((i1 - i0).saturating_add(1), (j1 - j0).saturating_add(1));
    if !(MIN_TILES..=MAX_TILES).contains(&w) || !(MIN_TILES..=MAX_TILES).contains(&h) {
        return Err(format!("the box is {w} x {h} L tiles, a side is {MIN_TILES} to {MAX_TILES}"));
    }
    let (lo, hi) = layout.allowed_tiles();
    if i0 < lo || j0 < lo || i1 > hi || j1 > hi {
        return Err(format!(
            "the box ({i0}, {j0})..=({i1}, {j1}) leaves tiles {lo}..={hi}: the solve margin of {} regional cells must stay on the face (no region crosses a face edge)",
            layout.margin
        ));
    }
    spec_of(layout, Face(face), (i0, j0, i1, j1), 0, 1.0, 1, 0).check()
}

fn spec_of(
    layout: &RegionLayout,
    face: Face,
    b: (u32, u32, u32, u32),
    seed: u64,
    radius_m: f64,
    threads: usize,
    inputs_id: u64,
) -> RegionSpec {
    let per = layout.regional_per_tile();
    let (i0, j0, i1, j1) = (b.0 as usize, b.1 as usize, b.2 as usize, b.3 as usize);
    RegionSpec {
        seed,
        face,
        radius_m,
        regional: layout.regional,
        region: (i0 * per, (i1 + 1) * per - 1, j0 * per, (j1 + 1) * per - 1),
        margin: layout.margin,
        tile: layout.tile,
        halo: layout.halo,
        rounds: layout.rounds,
        threads,
        res: layout.res,
        base_id: LATTICE_TAG ^ u64::from(layout.res),
        inputs_id,
        height_bounds: HEIGHT_BOUNDS,
        keep_channels: false,
    }
}

impl RegionEntry {
    /// The name rules (1 to [`MAX_NAME`] characters of `a-z`, `0-9`, `-`, `_`: a folder name on every system), then [`check_box`].
    pub fn check(&self, layout: &RegionLayout) -> Result<(), String> {
        check_name(&self.name)?;
        check_box(layout, self.face, self.tiles_per_edge, self.i0, self.j0, self.i1, self.j1)
            .map_err(|e| format!("region '{}': {e}", self.name))
    }

    /// The region bake's spec of this box for a world of `seed` and `radius_m` whose inputs are `inputs_id` ([`region_inputs_id`]).
    pub fn region_spec(&self, layout: &RegionLayout, seed: u64, radius_m: f64, threads: usize, inputs_id: u64) -> RegionSpec {
        spec_of(layout, Face(self.face), (self.i0, self.j0, self.i1, self.j1), seed, radius_m, threads, inputs_id)
    }
}

fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > MAX_NAME
        || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err(format!("the region name '{name}' is not 1 to {MAX_NAME} characters of a-z, 0-9, '-' and '_'"));
    }
    Ok(())
}

/// The regions of a world, in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionsFile {
    pub regions: Vec<RegionEntry>,
}

impl RegionsFile {
    /// Parses the file: `format_version = 1` and `[[region]]` tables with exactly the keys `name`, `face`, `tiles_per_edge`, `i0`, `j0`, `i1`, `j1`. Names are unique. The geometry is checked by
    /// [`RegionEntry::check`] against a layout, not here.
    pub fn parse(text: &str) -> Result<RegionsFile, String> {
        let root: toml::Table = text.parse().map_err(|e: toml::de::Error| format!("the regions file: {e}"))?;
        for key in root.keys() {
            if key != "format_version" && key != "region" {
                return Err(format!("the regions file: unknown key '{key}' (format_version, region)"));
            }
        }
        match root.get("format_version") {
            Some(toml::Value::Integer(v)) if *v == i64::from(FORMAT_VERSION) => {}
            Some(toml::Value::Integer(v)) => {
                return Err(format!("the regions file has format_version {v}, this build reads {FORMAT_VERSION}"))
            }
            Some(other) => return Err(format!("the regions file: format_version is a {}, not an integer", other.type_str())),
            None => return Err("the regions file has no format_version".into()),
        }
        let tables = match root.get("region") {
            None => return Ok(RegionsFile::default()),
            Some(toml::Value::Array(a)) => a,
            Some(other) => return Err(format!("the regions file: region is a {}, not an array of tables ([[region]])", other.type_str())),
        };
        let mut regions: Vec<RegionEntry> = Vec::with_capacity(tables.len());
        for (k, v) in tables.iter().enumerate() {
            let t = v.as_table().ok_or_else(|| format!("region {}: not a table", k + 1))?;
            for key in t.keys() {
                if !["name", "face", "tiles_per_edge", "i0", "j0", "i1", "j1"].contains(&key.as_str()) {
                    return Err(format!("region {}: unknown key '{key}'", k + 1));
                }
            }
            let name = match t.get("name") {
                Some(toml::Value::String(s)) => s.clone(),
                _ => return Err(format!("region {}: name is missing or not a string", k + 1)),
            };
            let int = |key: &str| -> Result<u32, String> {
                match t.get(key) {
                    Some(toml::Value::Integer(v)) => {
                        u32::try_from(*v).map_err(|_| format!("region '{name}': {key} = {v} is not a whole number from 0"))
                    }
                    Some(other) => Err(format!("region '{name}': {key} is a {}, not an integer", other.type_str())),
                    None => Err(format!("region '{name}': {key} is missing")),
                }
            };
            let face = int("face")?;
            let e = RegionEntry {
                face: u8::try_from(face).map_err(|_| format!("region '{name}': face {face} does not exist"))?,
                tiles_per_edge: int("tiles_per_edge")?,
                i0: int("i0")?,
                j0: int("j0")?,
                i1: int("i1")?,
                j1: int("j1")?,
                name,
            };
            check_name(&e.name)?;
            if regions.iter().any(|r| r.name == e.name) {
                return Err(format!("the regions file names region '{}' twice", e.name));
            }
            regions.push(e);
        }
        Ok(RegionsFile { regions })
    }

    /// The file text: `format_version`, then one `[[region]]` per region in order. `parse(to_toml())` gives the file back.
    pub fn to_toml(&self) -> String {
        let mut s = format!("format_version = {FORMAT_VERSION}\n");
        for r in &self.regions {
            s.push_str(&format!(
                "\n[[region]]\nname = \"{}\"\nface = {}\ntiles_per_edge = {}\ni0 = {}\nj0 = {}\ni1 = {}\nj1 = {}\n",
                r.name, r.face, r.tiles_per_edge, r.i0, r.j0, r.i1, r.j1
            ));
        }
        s
    }

    /// Reads the file; a missing file is an empty one.
    pub fn read(path: &Path) -> Result<RegionsFile, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => RegionsFile::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(RegionsFile::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes the file whole: a temporary file beside it, then a rename over it.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let tmp = path.with_extension(format!("toml.tmp-{}", std::process::id()));
        std::fs::write(&tmp, self.to_toml()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", path.display())
        })
    }
}

/// `<world-dir>/regions.toml`.
pub fn default_path(world_dir: &Path) -> PathBuf {
    world_dir.join(FILE_NAME)
}

/// The commit folder of a region: `<regions-parent>/regions/<name>/`.
pub fn commit_dir(regions_path: &Path, name: &str) -> PathBuf {
    regions_path.parent().unwrap_or(Path::new("")).join(COMMITS_DIR).join(name)
}

/// The identity of a world's inputs for its region commits: the class inputs' key (the seed, the base and climate versions, the definition's nodes) with the definition's hash, which holds its
/// items (CON-05), so an edit makes every commit of the world stale.
/// The code versions are in it too ([`REGION_BAKE_VERSION`], the C1 reader, the coarse generator, the L codec, the commit format): a commit of an older bake reads `stale`, not `ready`.
pub fn region_inputs_id(def: &WorldDefinition, seed: u64) -> u64 {
    let mut h = class_inputs_key(def, seed) ^ def.hash();
    for v in [
        u64::from(REGION_BAKE_VERSION),
        u64::from(planet_generators::global::READER_C1_VERSION),
        u64::from(planet_generators::global::GLOBAL_VERSION),
        u64::from(crate::layer_l::L_CODEC_VERSION),
        u64::from(crate::l_writer::COMMIT_VERSION),
    ] {
        h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The version of the region bake's stages that no key carries (the regional solve, H1's hydrology, the lakes, the channel graph, this file's lattice): raise it when one of them changes
/// its result, so every region commit reads `stale`.
pub const REGION_BAKE_VERSION: u32 = 1;

/// A region's state, derived from its commit folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionState {
    /// No commit file in the folder, or no folder.
    Unbaked,
    /// A commit of exactly this box, layout and inputs.
    Ready,
    /// A commit of another box, layout or inputs, or a commit file that cannot be read (a rebake replaces it).
    Stale,
}

impl RegionState {
    pub fn id(self) -> &'static str {
        match self {
            RegionState::Unbaked => "unbaked",
            RegionState::Ready => "ready",
            RegionState::Stale => "stale",
        }
    }
}

/// The state of `entry` in `layout` for a world whose inputs are `inputs_id` ([`region_inputs_id`]): the commit's key identity is the spec's `effective_inputs_id`, which holds the box, the layout and
/// the inputs (the seed and the radius are in the definition, so in `inputs_id`); the lattice resolution and the tile are compared too.
pub fn region_state(entry: &RegionEntry, layout: &RegionLayout, regions_path: &Path, inputs_id: u64) -> RegionState {
    let commit = match Commit::read(&commit_dir(regions_path, &entry.name)) {
        Ok(Some(c)) => c,
        Ok(None) => return RegionState::Unbaked,
        Err(_) => return RegionState::Stale,
    };
    let c = commit.spec;
    let want = entry.region_spec(layout, c.seed, c.radius_m, 1, inputs_id);
    if c.inputs_id == want.effective_inputs_id() && c.res == layout.res && c.tile as usize == layout.tile {
        RegionState::Ready
    } else {
        RegionState::Stale
    }
}

/// The definition of a world and its seed and radius: `def` with every item of `items` (a `LooseFileStore` root, the editor's `--items`) put into it.
pub fn world_with_items(mut def: WorldDefinition, items: Option<&Path>) -> Result<(WorldDefinition, u64, f64), String> {
    use planet_world_def::item_store::{ItemStore, LooseFileStore};
    if let Some(dir) = items {
        let store = LooseFileStore::new(dir);
        for item in store.load_all(BodyId::PLANET).map_err(|e| format!("the items in {}: {e}", dir.display()))? {
            item.put_into(&mut def).map_err(|e| format!("item {} of {}: {e:?}", item.id, dir.display()))?;
        }
    }
    let ctx = GenContext::new(&def);
    let seed = ctx.body_seed(BodyId::PLANET).map_err(|e| format!("the definition has no body seed: {e}"))?;
    let radius = ctx.body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition has no body radius: {e}"))?;
    Ok((def, seed, radius))
}

/// The lattice nodes of a rectangle of one face, sampled from the base (f32, as stored): what [`bake_entry`] reads C1 as the base B.
pub struct Lattice {
    res: u32,
    face: Face,
    i0: usize,
    j0: usize,
    w: usize,
    h: usize,
    z: Vec<f32>,
}

impl LatticeSource for Lattice {
    fn res(&self) -> u32 {
        self.res
    }
    fn at(&self, face: Face, i: u32, j: u32) -> Result<GlobalSample, ReadError> {
        let (i, j) = (i as usize, j as usize);
        if face != self.face || i < self.i0 || j < self.j0 || i >= self.i0 + self.w || j >= self.j0 + self.h {
            return Err(ReadError::Source(format!("node ({}, {i}, {j}) is outside the sampled rectangle", face.0)));
        }
        Ok(GlobalSample {
            continent: 0.0,
            elevation_m: self.z[(j - self.j0) * self.w + (i - self.i0)],
            temperature_c: 0.0,
            precipitation_mm: 0.0,
        })
    }
}

/// The base lattice of a region: `BakeInputs::base` of `def` at the nodes of `spec.res` over the solved window (the box and its margin) and three nodes round it (the C1 read's stencil,
/// `floor(p) - 1 ..= floor(p) + 2`). The node values do not depend on `threads` (each node is computed alone).
pub fn base_lattice(def: &WorldDefinition, spec: &RegionSpec, threads: usize) -> Lattice {
    let inputs = class_inputs(def, spec.seed, threads.max(1));
    let (face, res) = (spec.face, spec.res as usize);
    let (i0, i1, j0, j1) = spec.region;
    let (ri0, ri1, rj0, rj1) = (i0 - spec.margin, i1 + spec.margin, j0 - spec.margin, j1 + spec.margin);
    let to_node = |reg: usize| reg * res / spec.regional;
    const PAD: usize = 3;
    let (ni0, nj0) = (to_node(ri0).saturating_sub(PAD), to_node(rj0).saturating_sub(PAD));
    let (nw, nh) = ((to_node(ri1 + 1) + PAD).min(res) - ni0, (to_node(rj1 + 1) + PAD).min(res) - nj0);
    let mut nodes = vec![0f32; nw * nh];
    let rows = nh.div_ceil(threads.max(1));
    std::thread::scope(|s| {
        for (k, chunk) in nodes.chunks_mut(rows * nw).enumerate() {
            let inputs = &inputs;
            s.spawn(move || {
                for (c, v) in chunk.iter_mut().enumerate() {
                    let p = k * rows * nw + c;
                    *v = inputs.base(lattice_direction(face, res, ni0 + p % nw, nj0 + p / nw)) as f32;
                }
            });
        }
    });
    Lattice { res: spec.res, face, i0: ni0, j0: nj0, w: nw, h: nh, z: nodes }
}

/// Bakes one region of `def` (its items already in it, [`world_with_items`]) into `dir`: the production base (`BakeInputs::base`) sampled on the layout's lattice over the box and its margin and read
/// C1, the climate of the orography as the build gives it to H1, then [`bake_region`]. The commit opens over the in-memory production base of the same definition (VIEW-069, VIEW-072).
/// The base does not read items (the class inputs are the seed's): the items enter the commit's identity only, so any item change makes every commit of the world stale (WDEF-019).
pub fn bake_entry(
    def: &WorldDefinition,
    entry: &RegionEntry,
    layout: &RegionLayout,
    dir: &Path,
    threads: usize,
) -> Result<(Commit, RegionReport), String> {
    entry.check(layout)?;
    let ctx = GenContext::new(def);
    let seed = ctx.body_seed(BodyId::PLANET).map_err(|e| format!("the definition has no body seed: {e}"))?;
    let radius_m = ctx.body_radius_m(BodyId::PLANET).map_err(|e| format!("the definition has no body radius: {e}"))?;
    let threads = threads.max(1);
    let spec = entry.region_spec(layout, seed, radius_m, threads, region_inputs_id(def, seed));
    spec.check()?;
    let world = GlobalWorld::from_definition(def).map_err(|e| format!("the world of the definition: {e}"))?;
    let inputs = class_inputs(def, seed, threads);
    let lattice = base_lattice(def, &spec, threads);
    // Every read of the bake lies in the solved window (the solve's cell centres, K7's tile and halo, the land test's cores); a read outside the rectangle is a bug of this function.
    let base = |d: Vec3| -> f64 {
        read_elevation_c1(&lattice, &planet_core::TangentWarp, d)
            .map(f64::from)
            .unwrap_or_else(|e| panic!("the region's base lattice: {e}"))
    };
    // The climate as the build gives it to H1: the runoff in m/s, the open-water evaporation, the discharge weight.
    let orography = inputs.orography();
    let climate = |d: Vec3| {
        let (pm, tc) = (orography.precipitation_mm(d), orography.temperature_c(d));
        let ro = runoff_mm(pm, tc);
        (ro / 1000.0 / SECONDS_PER_YEAR, (pet_mm(tc) - (pm - ro)).max(0.0) / 1000.0 / SECONDS_PER_YEAR, discharge_weight(ro))
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    bake_region(&world, &spec, &base, &climate, dir)
}

/// The radius of a `--seed` world: the viewer's single planet (`skeleton::RADIUS_M`, Earth's), so `--commit` with the same `--seed` opens over the same definition.
pub const SEED_WORLD_RADIUS_M: f64 = planet_core::EARTH_RADIUS_M;

/// The options of `cargo xtask bake-region` (BUILD-078).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakeArgs {
    /// `--world F`: the definition file; the regions file defaults to `regions.toml` beside it.
    pub world: Option<PathBuf>,
    /// `--seed N`: the viewer's single planet of that seed; needs `--regions`.
    pub seed: Option<u64>,
    pub regions: PathBuf,
    /// `--items DIR`: the editor's item files, put into the definition (CON-05).
    pub items: Option<PathBuf>,
    /// `--name N`: bake only this region (by default every region that is not `ready`).
    pub name: Option<String>,
    /// `--force`: bake ready regions too.
    pub force: bool,
    /// `--threads T` (0: the topology's bake count).
    pub threads: usize,
}

/// The usage line of `bake-region`.
pub const BAKE_USAGE: &str =
    "usage: cargo xtask bake-region (--world F [--regions FILE] | --seed N --regions FILE) [--items DIR] [--name NAME] [--force] [--threads T]";

impl BakeArgs {
    /// Parses the options; `--world` and `--seed` exclude each other, `--seed` needs `--regions` (as the editor's `--bookmarks`).
    pub fn parse(args: &[String]) -> Result<BakeArgs, String> {
        let (mut world, mut seed, mut regions, mut items, mut name, mut force, mut threads) = (None, None, None, None, None, false, 0usize);
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let mut value = |what: &str| it.next().cloned().ok_or_else(|| format!("{a} needs {what}\n{BAKE_USAGE}"));
            match a.as_str() {
                "--world" => world = Some(PathBuf::from(value("a definition file")?)),
                "--seed" => seed = Some(value("a number")?.parse::<u64>().map_err(|e| format!("--seed: {e}"))?),
                "--regions" => regions = Some(PathBuf::from(value("a file")?)),
                "--items" => items = Some(PathBuf::from(value("a directory")?)),
                "--name" => name = Some(value("a region name")?),
                "--threads" => threads = value("a number")?.parse::<usize>().map_err(|e| format!("--threads: {e}"))?,
                "--force" => force = true,
                other => return Err(format!("unknown option '{other}'\n{BAKE_USAGE}")),
            }
        }
        let regions = match (&world, seed, regions) {
            (Some(_), Some(_), _) => return Err(format!("--world and --seed exclude each other\n{BAKE_USAGE}")),
            (Some(w), None, r) => r.unwrap_or_else(|| default_path(w.parent().unwrap_or(Path::new("")))),
            (None, Some(_), Some(r)) => r,
            (None, Some(_), None) => return Err(format!("--seed needs --regions FILE (a seed has no world folder)\n{BAKE_USAGE}")),
            (None, None, _) => return Err(format!("give --world F or --seed N\n{BAKE_USAGE}")),
        };
        Ok(BakeArgs { world, seed, regions, items, name, force, threads })
    }

    /// The definition the options name, with its items, its seed and radius.
    pub fn definition(&self) -> Result<(WorldDefinition, u64, f64), String> {
        let def = match (&self.world, self.seed) {
            (Some(p), _) => {
                let text = std::fs::read_to_string(p).map_err(|e| format!("reading {}: {e}", p.display()))?;
                planet_world_def::toml_io::parse_definition(&text).map_err(|e| format!("{}: {e}", p.display()))?
            }
            (None, Some(s)) => WorldDefinition::single_planet(SEED_WORLD_RADIUS_M, s),
            (None, None) => return Err(BAKE_USAGE.into()),
        };
        world_with_items(def, self.items.as_deref())
    }

    /// The command that opens a region's commit in the editor (VIEW-069).
    pub fn open_command(&self, commit: &Path) -> String {
        let world = match (&self.world, self.seed) {
            (Some(p), _) => format!("--world {}", p.display()),
            (_, s) => format!("--seed {}", s.unwrap_or(0)),
        };
        let items = self.items.as_ref().map_or(String::new(), |d| format!(" --items {}", d.display()));
        format!("cargo run --release -p planet -- editor {world}{items} --commit {}", commit.display())
    }
}

/// What `bake-region` does with each region of the file, in order: `(entry, state before, bake it)`. An unknown `--name`, an empty file and a box that fails [`RegionEntry::check`] are errors
/// before any bake.
pub fn bake_plan(
    file: &RegionsFile,
    args: &BakeArgs,
    layout: &RegionLayout,
    inputs_id: u64,
) -> Result<Vec<(RegionEntry, RegionState, bool)>, String> {
    if file.regions.is_empty() {
        return Err(format!("{} holds no region", args.regions.display()));
    }
    if let Some(n) = &args.name {
        if !file.regions.iter().any(|r| &r.name == n) {
            return Err(format!("{} has no region '{n}'", args.regions.display()));
        }
    }
    // Every box of the file is checked, also with `--name`: a file with a box the editor could not have made is refused whole.
    for r in &file.regions {
        r.check(layout)?;
    }
    file.regions
        .iter()
        .filter(|r| args.name.as_ref().is_none_or(|n| &r.name == n))
        .map(|r| {
            let state = region_state(r, layout, &args.regions, inputs_id);
            Ok((r.clone(), state, args.force || state != RegionState::Ready))
        })
        .collect()
}
