//! The editor's region boxes (VIEW-077): the boxes of a world's `regions.toml` (WDEF-019) placed, moved, resized and deleted in `planet editor`, snapped to whole L tiles
//! and kept valid by `check_box`, the rule `bake-region` bakes by (BUILD-078). GPU-free; the window draws the outlines this module gives.

use planet_cache::regions_file::{
    self as rf, check_box, commit_dir, region_state, RegionEntry, RegionLayout, RegionState, RegionsFile, MAX_TILES, MIN_TILES,
};
use planet_core::cube::{direction_to_face, face_to_direction};
use planet_core::{Face, TangentWarp, Vec3};
use planet_world_def::{BodyId, GenContext, WorldDefinition};
use std::path::{Path, PathBuf};

/// Points a tile along a drawn edge (the edges are curves on the sphere).
pub const SAMPLES_PER_TILE: usize = 4;

/// What a press on the ground does while placing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placing {
    /// Puts a new box under this name (empty: `region-<n>`).
    New(String),
    /// Re-places the named box.
    Move(String),
}

/// How the editor names the world in the command that opens a box: the options it was started with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldArgs {
    pub world: Option<PathBuf>,
    pub seed: u64,
    pub items: Option<PathBuf>,
}

/// One row of the Regions section.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionRow {
    pub name: String,
    pub face: u8,
    /// `i0..=i1 x j0..=j1`.
    pub tiles: String,
    pub width: u32,
    pub height: u32,
    pub state: RegionState,
    pub selected: bool,
}

/// The region boxes of one regions file and the placement in progress.
pub struct RegionEditor {
    layout: RegionLayout,
    path: Option<PathBuf>,
    file: RegionsFile,
    world: WorldArgs,
    pub selected: Option<String>,
    pub placing: Option<Placing>,
    /// The side of a new box in tiles: `(width along i, height along j)`.
    pub size: (u32, u32),
    /// `(definition hash, inputs id)`: the id is computed once per definition.
    inputs: Option<(u64, u64)>,
}

/// The L tile of `layout` under a direction: `(face, i, j)`, `i` along the face's first coordinate (the axis of the bake's lattice).
pub fn tile_of(layout: &RegionLayout, dir: Vec3) -> (u8, u32, u32) {
    let (face, s, t) = direction_to_face(&TangentWarp, dir);
    let n = layout.tiles_per_edge();
    let index = |x: f64| -> u32 {
        let k = libm::floor((x + 1.0) * 0.5 * f64::from(n));
        if k.is_nan() || k < 0.0 {
            0
        } else {
            (k as u32).min(n - 1)
        }
    };
    (face.0, index(s), index(t))
}

/// A run of `side` tiles that holds tile `c` (centred, the extra tile of an even side after it) clamped into `lo..=hi`: `(first, last)`.
fn run(c: u32, side: u32, lo: u32, hi: u32) -> (u32, u32) {
    let first = c.saturating_sub((side - 1) / 2);
    let top = (hi + 1).saturating_sub(side).max(lo);
    let first = first.clamp(lo, top);
    (first, first + side - 1)
}

/// The box of `w x h` tiles under the tile `(ci, cj)` of `face`, inside the allowed tiles of `layout`.
pub fn snapped(layout: &RegionLayout, face: u8, ci: u32, cj: u32, w: u32, h: u32) -> RegionEntry {
    let (lo, hi) = layout.allowed_tiles();
    let (i0, i1) = run(ci, w, lo, hi);
    let (j0, j1) = run(cj, h, lo, hi);
    RegionEntry { name: String::new(), face, tiles_per_edge: layout.tiles_per_edge(), i0, j0, i1, j1 }
}

/// The face direction of the face coordinate of tile boundary `k` (0 to `n`).
fn edge(k: u32, n: u32) -> f64 {
    2.0 * f64::from(k) / f64::from(n) - 1.0
}

/// The closed outline of the tile rectangle `i0..=i1 x j0..=j1` of `face`, `SAMPLES_PER_TILE` points a tile, counter-clockwise in `(s, t)`.
pub fn rectangle(face: u8, n: u32, (i0, j0, i1, j1): (u32, u32, u32, u32)) -> Vec<Vec3> {
    let (s0, s1, t0, t1) = (edge(i0, n), edge(i1 + 1, n), edge(j0, n), edge(j1 + 1, n));
    let per = |a: u32, b: u32| ((b + 1 - a) as usize) * SAMPLES_PER_TILE;
    let (ns, nt) = (per(i0, i1), per(j0, j1));
    let lerp = |a: f64, b: f64, k: usize, m: usize| a + (b - a) * k as f64 / m as f64;
    let at = |s: f64, t: f64| face_to_direction(&TangentWarp, Face(face), s, t);
    let mut out = Vec::with_capacity(2 * (ns + nt));
    out.extend((0..ns).map(|k| at(lerp(s0, s1, k, ns), t0)));
    out.extend((0..nt).map(|k| at(s1, lerp(t0, t1, k, nt))));
    out.extend((0..ns).map(|k| at(lerp(s1, s0, k, ns), t1)));
    out.extend((0..nt).map(|k| at(s0, lerp(t1, t0, k, nt))));
    out
}

/// The outline of a box.
pub fn outline(e: &RegionEntry) -> Vec<Vec3> {
    rectangle(e.face, e.tiles_per_edge, (e.i0, e.j0, e.i1, e.j1))
}

/// The face-edge margin of `face`: the boundary of the tiles a box may cover.
pub fn margin(layout: &RegionLayout, face: u8) -> Vec<Vec3> {
    let (lo, hi) = layout.allowed_tiles();
    rectangle(face, layout.tiles_per_edge(), (lo, lo, hi, hi))
}

/// The inputs id of a definition for its region commits, with the body seed it names (as `bake-region` derives it).
pub fn inputs_id(def: &WorldDefinition) -> Result<u64, String> {
    let seed = GenContext::new(def).body_seed(BodyId::PLANET).map_err(|e| format!("the definition has no body seed: {e}"))?;
    Ok(rf::region_inputs_id(def, seed))
}

impl RegionEditor {
    /// The editor over the production layout; `path` `None`: no regions file (a seed world without `--regions`).
    pub fn open(path: Option<PathBuf>, world: WorldArgs) -> Result<RegionEditor, String> {
        RegionEditor::with_layout(RegionLayout::PRODUCTION, path, world)
    }

    /// [`open`](RegionEditor::open) on another layout (tests).
    pub fn with_layout(layout: RegionLayout, path: Option<PathBuf>, world: WorldArgs) -> Result<RegionEditor, String> {
        let file = match &path {
            Some(p) => RegionsFile::read(p)?,
            None => RegionsFile::default(),
        };
        Ok(RegionEditor { layout, path, file, world, selected: None, placing: None, size: (MIN_TILES, MIN_TILES), inputs: None })
    }

    pub fn layout(&self) -> &RegionLayout {
        &self.layout
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn regions(&self) -> &[RegionEntry] {
        &self.file.regions
    }

    /// Where the boxes are saved, for the panel.
    pub fn where_label(&self) -> String {
        match &self.path {
            Some(p) => format!("saved in {}", p.display()),
            None => "no regions file: start with --world F (its regions.toml) or --regions FILE".to_string(),
        }
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.file.regions.iter().position(|r| r.name == name)
    }

    fn write(&self) -> Result<(), String> {
        match &self.path {
            Some(p) => self.file.write(p),
            None => Err("no regions file: start with --world F or --regions FILE".to_string()),
        }
    }

    /// Replaces the boxes with `next` when every box passes the rule and the file takes it; else nothing changes.
    fn commit(&mut self, next: RegionsFile) -> Result<(), String> {
        for e in &next.regions {
            e.check(&self.layout)?;
        }
        let names: std::collections::BTreeSet<&str> = next.regions.iter().map(|r| r.name.as_str()).collect();
        if names.len() != next.regions.len() {
            return Err("two boxes cannot have one name".to_string());
        }
        let before = std::mem::replace(&mut self.file, next);
        if let Err(e) = self.write() {
            self.file = before;
            return Err(e);
        }
        Ok(())
    }

    /// The first free `region-<n>`.
    fn free_name(&self) -> String {
        (1..).map(|n| format!("region-{n}")).find(|n| self.index(n).is_none()).unwrap_or_default()
    }

    /// The box placing would put at `dir`: the current side for a new box, the moved box's own side for a move.
    pub fn preview(&self, dir: Vec3) -> Option<RegionEntry> {
        let placing = self.placing.as_ref()?;
        let (w, h) = match placing {
            Placing::New(_) => self.size,
            Placing::Move(name) => {
                let e = &self.file.regions[self.index(name)?];
                (e.i1 - e.i0 + 1, e.j1 - e.j0 + 1)
            }
        };
        let (face, ci, cj) = tile_of(&self.layout, dir);
        Some(snapped(&self.layout, face, ci, cj, w, h))
    }

    /// Whether the placement box at `dir` would be accepted.
    pub fn preview_ok(&self, e: &RegionEntry) -> Result<(), String> {
        check_box(&self.layout, e.face, e.tiles_per_edge, e.i0, e.j0, e.i1, e.j1)
    }

    /// A press on the ground at `dir` while placing: puts or moves the box. Returns its name.
    pub fn press(&mut self, dir: Vec3) -> Result<String, String> {
        let placing = self.placing.clone().ok_or("not placing a region")?;
        let mut boxed = self.preview(dir).ok_or("the box to move is gone")?;
        let mut next = self.file.clone();
        let name = match &placing {
            Placing::New(typed) => {
                let name = if typed.trim().is_empty() { self.free_name() } else { typed.trim().to_string() };
                if self.index(&name).is_some() {
                    return Err(format!("a box is already called '{name}'"));
                }
                boxed.name = name.clone();
                next.regions.push(boxed);
                name
            }
            Placing::Move(name) => {
                let k = self.index(name).ok_or("the box to move is gone")?;
                boxed.name = name.clone();
                next.regions[k] = boxed;
                name.clone()
            }
        };
        self.commit(next)?;
        self.placing = None;
        self.selected = Some(name.clone());
        Ok(name)
    }

    /// Sets the side of the named box to `w x h` tiles, keeping its first tile and moving back into the allowed tiles where it would leave them.
    pub fn resize(&mut self, name: &str, w: u32, h: u32) -> Result<(), String> {
        if !(MIN_TILES..=MAX_TILES).contains(&w) || !(MIN_TILES..=MAX_TILES).contains(&h) {
            return Err(format!("a side is {MIN_TILES} to {MAX_TILES} L tiles"));
        }
        let k = self.index(name).ok_or_else(|| format!("no box '{name}'"))?;
        let (lo, hi) = self.layout.allowed_tiles();
        let mut next = self.file.clone();
        let e = &mut next.regions[k];
        let start = |first: u32, side: u32| first.min((hi + 1).saturating_sub(side)).max(lo);
        e.i0 = start(e.i0, w);
        e.j0 = start(e.j0, h);
        e.i1 = e.i0 + w - 1;
        e.j1 = e.j0 + h - 1;
        self.commit(next)
    }

    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let k = self.index(name).ok_or_else(|| format!("no box '{name}'"))?;
        let mut next = self.file.clone();
        next.regions.remove(k);
        self.commit(next)?;
        if self.selected.as_deref() == Some(name) {
            self.selected = None;
        }
        if self.placing == Some(Placing::Move(name.to_string())) {
            self.placing = None;
        }
        Ok(())
    }

    /// The inputs id of `def`, kept while its hash is the same.
    pub fn inputs_for(&mut self, def: &WorldDefinition) -> Option<u64> {
        let h = def.hash();
        if let Some((hash, id)) = self.inputs {
            if hash == h {
                return Some(id);
            }
        }
        let id = inputs_id(def).ok()?;
        self.inputs = Some((h, id));
        Some(id)
    }

    /// The box's derived state against inputs id `id`; without a file every box is unbaked.
    pub fn state(&self, e: &RegionEntry, id: u64) -> RegionState {
        match &self.path {
            Some(p) => region_state(e, &self.layout, p, id),
            None => RegionState::Unbaked,
        }
    }

    pub fn rows(&self, id: u64) -> Vec<RegionRow> {
        self.file
            .regions
            .iter()
            .map(|e| RegionRow {
                name: e.name.clone(),
                face: e.face,
                tiles: format!("{}..={} x {}..={}", e.i0, e.i1, e.j0, e.j1),
                width: e.i1 - e.i0 + 1,
                height: e.j1 - e.j0 + 1,
                state: self.state(e, id),
                selected: self.selected.as_deref() == Some(e.name.as_str()),
            })
            .collect()
    }

    /// The command that opens the named box's commit in the editor (VIEW-069), the regions file named so the boxes come along.
    pub fn open_command(&self, name: &str) -> Result<String, String> {
        let path = self.path.as_ref().ok_or("no regions file")?;
        self.index(name).ok_or_else(|| format!("no box '{name}'"))?;
        let args = rf::BakeArgs {
            world: self.world.world.clone(),
            seed: self.world.world.is_none().then_some(self.world.seed),
            regions: path.clone(),
            items: self.world.items.clone(),
            name: None,
            force: false,
            threads: 0,
        };
        let open = args.open_command(&commit_dir(path, name));
        Ok(if self.world.world.is_none() { format!("{open} --regions {}", path.display()) } else { open })
    }
}

/// The regions file of the editor: `--regions FILE`, else `<world-dir>/regions.toml` of `--world F`, else none.
pub fn resolve_path(regions: Option<&PathBuf>, world: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = regions {
        return Some(p.clone());
    }
    let w = world?;
    let dir = w.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Some(rf::default_path(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("planet-regionedit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The centre direction of tile `(i, j)` of `face`.
    fn centre(face: u8, n: u32, i: u32, j: u32) -> Vec3 {
        let c = |k: u32| (edge(k, n) + edge(k + 1, n)) * 0.5;
        face_to_direction(&TangentWarp, Face(face), c(i), c(j))
    }

    // spec: VIEW-077
    // spec: TEST-166
    #[test]
    fn every_snapped_box_at_every_face_and_band_edge_passes_the_bake_rule() {
        let l = RegionLayout::PRODUCTION;
        let n = l.tiles_per_edge();
        let (lo, hi) = l.allowed_tiles();
        for face in 0..6u8 {
            for &(ci, cj) in &[(0, 0), (n - 1, n - 1), (0, n - 1), (lo, hi), (n / 2, n / 2), (hi, lo), (lo - 1, hi + 1)] {
                assert_eq!(tile_of(&l, centre(face, n, ci, cj)), (face, ci, cj), "the tile under a tile centre is that tile");
                for w in MIN_TILES..=MAX_TILES {
                    for h in MIN_TILES..=MAX_TILES {
                        let e = snapped(&l, face, ci, cj, w, h);
                        assert_eq!((e.i1 - e.i0 + 1, e.j1 - e.j0 + 1), (w, h));
                        assert!(e.i0 >= lo && e.j0 >= lo && e.i1 <= hi && e.j1 <= hi, "{e:?} leaves {lo}..={hi}");
                        check_box(&l, e.face, e.tiles_per_edge, e.i0, e.j0, e.i1, e.j1)
                            .unwrap_or_else(|why| panic!("face {face} at ({ci}, {cj}) {w}x{h}: {why}"));
                    }
                }
            }
        }
        // Inside the band a box holds the cursor's tile.
        let e = snapped(&l, 2, 60, 61, 3, 4);
        assert!((e.i0..=e.i1).contains(&60) && (e.j0..=e.j1).contains(&61), "{e:?}");
        assert_eq!(rectangle(2, n, (60, 60, 61, 61)).len(), 4 * 2 * SAMPLES_PER_TILE);
        assert_eq!(margin(&l, 0).len(), 4 * ((hi + 1 - lo) as usize) * SAMPLES_PER_TILE);
    }

    // spec: VIEW-077
    // spec: TEST-166
    #[test]
    fn boxes_are_placed_moved_resized_and_deleted_and_the_file_reread_says_so() {
        let dir = temp("edit");
        let path = dir.join("regions.toml");
        let l = RegionLayout::PRODUCTION;
        let n = l.tiles_per_edge();
        let (lo, hi) = l.allowed_tiles();
        let mut r = RegionEditor::open(Some(path.clone()), WorldArgs { seed: 14, ..WorldArgs::default() }).unwrap();
        assert!(r.regions().is_empty());
        assert!(r.press(centre(1, n, 0, 0)).is_err(), "not placing");

        // A 2x2 box with the cursor on the first tile of face 1 sits at the first allowed tile.
        r.placing = Some(Placing::New(String::new()));
        let p = r.preview(centre(1, n, 0, 0)).unwrap();
        assert!(r.preview_ok(&p).is_ok());
        assert_eq!(r.press(centre(1, n, 0, 0)).unwrap(), "region-1");
        assert_eq!(r.placing, None);
        assert_eq!(r.selected.as_deref(), Some("region-1"));
        let read = RegionsFile::read(&path).unwrap();
        assert_eq!(read.regions.len(), 1);
        let e = &read.regions[0];
        assert_eq!((e.face, e.i0, e.j0, e.i1, e.j1, e.tiles_per_edge), (1, lo, lo, lo + 1, lo + 1, n));

        // Widened to 4 and once more: the side stops at 4.
        r.resize("region-1", 4, 2).unwrap();
        assert!(r.resize("region-1", 5, 2).unwrap_err().contains("2 to 4"));
        let e = &RegionsFile::read(&path).unwrap().regions[0];
        assert_eq!((e.i0, e.i1, e.j0, e.j1), (lo, lo + 3, lo, lo + 1));

        // A move snaps to the new tile; near the far band edge a resize shifts back inside.
        r.placing = Some(Placing::Move("region-1".into()));
        r.press(centre(1, n, hi, hi)).unwrap();
        let e = &RegionsFile::read(&path).unwrap().regions[0];
        assert_eq!((e.i0, e.i1, e.j0, e.j1), (hi - 3, hi, hi - 1, hi));
        r.resize("region-1", 4, 4).unwrap();
        let e = &RegionsFile::read(&path).unwrap().regions[0];
        assert_eq!((e.i0, e.i1, e.j0, e.j1), (hi - 3, hi, hi - 3, hi));

        // A taken name is refused and the file is unchanged.
        r.placing = Some(Placing::New("region-1".into()));
        assert!(r.press(centre(3, n, 64, 64)).unwrap_err().contains("already"));
        assert_eq!(RegionsFile::read(&path).unwrap().regions.len(), 1);
        r.placing = Some(Placing::New("grt".into()));
        assert_eq!(r.press(centre(3, n, 64, 64)).unwrap(), "grt");
        r.placing = Some(Placing::New("Bad Name".into()));
        assert!(r.press(centre(3, n, 30, 30)).is_err(), "the bake's name rule");
        assert_eq!(RegionsFile::read(&path).unwrap().regions.len(), 2);

        // Reopened, the boxes come back; deleted, the file ends empty.
        let mut again = RegionEditor::open(Some(path.clone()), WorldArgs::default()).unwrap();
        assert_eq!(again.regions(), r.regions());
        again.delete("region-1").unwrap();
        again.delete("grt").unwrap();
        assert!(again.delete("grt").is_err());
        assert!(RegionsFile::read(&path).unwrap().regions.is_empty());

        // Without a file nothing is kept, and the panel says how to name one.
        let mut none = RegionEditor::open(None, WorldArgs::default()).unwrap();
        none.placing = Some(Placing::New(String::new()));
        assert!(none.press(centre(1, n, 60, 60)).unwrap_err().contains("--regions"));
        assert!(none.regions().is_empty());
        assert!(none.where_label().contains("--regions FILE"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: TEST-166
    #[test]
    fn the_regions_file_is_named_by_regions_or_beside_the_world() {
        let f = PathBuf::from("x/r.toml");
        assert_eq!(resolve_path(Some(&f), Some(&PathBuf::from("w/world.toml"))), Some(f));
        assert_eq!(resolve_path(None, Some(&PathBuf::from("w/world.toml"))), Some(PathBuf::from("w").join("regions.toml")));
        assert_eq!(resolve_path(None, Some(&PathBuf::from("world.toml"))), Some(PathBuf::from(".").join("regions.toml")));
        assert_eq!(resolve_path(None, None), None);
    }

    /// The crater item of TEST-165.
    fn crater(def: &WorldDefinition) -> planet_world_def::item::Item {
        planet_world_def::item::Item {
            id: 0x0001_8f00_0000_0000_0000_0000_0000_0001,
            body: BodyId::PLANET,
            surface: "sphere".into(),
            layer: "items".into(),
            priority: 3,
            kind: "crater".into(),
            anchor: planet_world_def::item::Anchor { latitude_deg: 10.0, longitude_deg: 20.0, height_m: 0.0, heading_deg: 0.0 },
            reach_m: 900.0,
            max_delta_m: 35.5,
            params: std::collections::BTreeMap::from([("depth_m".to_string(), 24.0), ("radius_m".to_string(), 310.0)]),
            base_hash: def.base_hash(),
            generator_version: 4,
            stage_version: 2,
            name: None,
            notes: None,
            points: Vec::new(),
            group: None,
            part: None,
            mask_layer: None,
            strokes: Vec::new(),
        }
    }

    // spec: TEST-166
    #[test]
    fn the_editors_inputs_id_for_a_seed_world_with_items_is_bake_regions() {
        let dir = temp("ids");
        let items = dir.join("items");
        // The editor's seed world (as the window builds it) with the item of the folder in it, against `bake-region --seed 14 --items DIR`.
        let mut def = WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 14);
        let item = crater(&def);
        let mut store = planet_world_def::item_store::LooseFileStore::new(&items);
        planet_world_def::item_store::ItemStore::put(&mut store, &item).unwrap();
        item.put_into(&mut def).unwrap();
        let argv: Vec<String> =
            ["--seed", "14", "--regions", "r.toml", "--items"].iter().map(|s| s.to_string()).chain([items.display().to_string()]).collect();
        let (bake_def, seed, _) = rf::BakeArgs::parse(&argv).unwrap().definition().unwrap();
        assert_eq!(seed, 14);
        assert_eq!(inputs_id(&def).unwrap(), rf::region_inputs_id(&bake_def, seed));
        // Without the item it is another id.
        assert_ne!(inputs_id(&WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 14)).unwrap(), inputs_id(&def).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // spec: VIEW-077
    // spec: TEST-166
    #[test]
    fn a_placed_box_reads_unbaked_then_ready_after_its_bake_then_stale_after_an_edit_and_prints_its_open_command() {
        // A small layout of the production shape (TEST-165): 16 L tiles a face edge.
        let layout = RegionLayout { regional: 128, tile: 64, halo: 24, margin: 6, rounds: 2, res: 1024 };
        let n = layout.tiles_per_edge();
        let (lo, hi) = layout.allowed_tiles();
        let def = WorldDefinition::single_planet(crate::skeleton::RADIUS_M, 14);
        // The first 2x2 box of face 1 whose four tile centres are land (as TEST-165).
        let inputs = planet_cache::global_store::class_inputs(&def, 14, 2);
        let land = |i: u32, j: u32| inputs.base(centre(1, n, i, j)) > 50.0;
        let (i0, j0) = (lo..hi)
            .flat_map(|j| (lo..hi).map(move |i| (i, j)))
            .find(|&(i, j)| land(i, j) && land(i + 1, j) && land(i, j + 1) && land(i + 1, j + 1))
            .expect("a land box on face 1 of seed 14");
        let dir = temp("state");
        let path = dir.join("regions.toml");
        let mut r = RegionEditor::with_layout(layout, Some(path.clone()), WorldArgs { seed: 14, ..WorldArgs::default() }).unwrap();
        r.placing = Some(Placing::New("land".into()));
        // The cursor on the first tile of the box: a 2x2 box centred there starts on it.
        r.press(centre(1, n, i0, j0)).unwrap();
        let e = r.regions()[0].clone();
        assert_eq!((e.i0, e.j0, e.i1, e.j1), (i0, j0, i0 + 1, j0 + 1));
        let id = r.inputs_for(&def).unwrap();
        assert_eq!(r.inputs_for(&def), Some(id), "kept while the definition is the same");
        assert_eq!(r.rows(id)[0].state, RegionState::Unbaked);
        rf::bake_entry(&def, &e, &layout, &commit_dir(&path, "land"), 2).expect("the box bakes");
        let row = &r.rows(id)[0];
        assert_eq!((row.state, row.width, row.height, row.face, row.selected), (RegionState::Ready, 2, 2, 1, true));
        let mut edited = def.clone();
        crater(&def).put_into(&mut edited).unwrap();
        let id2 = r.inputs_for(&edited).unwrap();
        assert_ne!(id2, id);
        assert_eq!(r.rows(id2)[0].state, RegionState::Stale, "an item makes every box stale");
        let cmd = r.open_command("land").unwrap();
        assert!(cmd.contains("editor --seed 14") && cmd.contains(&format!("--commit {}", commit_dir(&path, "land").display())), "{cmd}");
        assert!(cmd.ends_with(&format!("--regions {}", path.display())), "{cmd}");
        assert!(r.open_command("none").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
