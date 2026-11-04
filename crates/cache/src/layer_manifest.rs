//! The erosion layer's manifest (STRM-074; m9-bake): what a reader needs to open the layer from its store alone: the shape of the bake (so the keys of every record can be rebuilt with
//! [`bake_specs`]), the versions of the kernels and records it was made with, the field source it was carved on, the global range of the change (the bound of the min/max pyramid) and the
//! pyramid's block size.
//!
//! One plain text file, [`MANIFEST_FILE`], in the layer's store directory (the bake's `tiles/`): a version line, then `key value` lines; an `f64` is written as its decimal value and its bits in
//! hex, and read back from the bits, so it round-trips exactly. It is written atomically (a temporary file in the directory, then a rename) so that a killed bake or two runs never leave a half
//! file. A run adds to the manifest that is there: the shape, the versions and the source must be equal (else an error naming both, never a mix) and the range only ever widens, so after a
//! planet run it is exactly the range of the layer.

use std::path::Path;

use planet_generators::bake::tile::QUANT_STEP_M;

use crate::global_store::GlobalWorld;
use crate::layer_channels::CHANNELS_VERSION;
use crate::layer_store::LAYER_VERSION;
use crate::layer_tiles::{BakeShape, TILE_LAYER_VERSION};

/// The file name of the manifest.
pub const MANIFEST_FILE: &str = "layer-manifest.txt";
/// Version of the manifest's layout.
pub const MANIFEST_VERSION: u32 = 1;

/// The field source the layer was carved on (STRM-073).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestSource {
    /// The class source, with its identity (`BakeInputs::id()`).
    Class(u64),
    /// The legacy generator.
    Legacy,
}

/// The manifest of a layer.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerManifest {
    /// The bake shape with the tile and the halo resolved (both `Some`).
    pub shape: BakeShape,
    pub bake_kernel_version: u32,
    pub pass2_kernel_version: u32,
    pub tile_layer_version: u32,
    pub layer_version: u32,
    pub channels_version: u32,
    /// The quantisation step of the change in metres.
    pub quant_step_m: f64,
    pub source: ManifestSource,
    /// Cells a side of a block of the pyramid's level 0 (`erosion_layer::BLOCK_CELLS`).
    pub pyramid_block_cells: usize,
    /// The range of the change over the tiles recorded so far, or none when no tile is.
    pub change_range: Option<(f64, f64)>,
}

fn pair(v: f64) -> String {
    format!("{v} {:016x}", v.to_bits())
}

fn unpair(s: &str) -> Result<f64, String> {
    let bits = s.split_whitespace().nth(1).ok_or_else(|| format!("'{s}' holds no bits"))?;
    u64::from_str_radix(bits, 16).map(f64::from_bits).map_err(|e| format!("'{bits}': {e}"))
}

impl LayerManifest {
    /// The manifest of a bake of `shape` beside `world`, with this build's versions, the world's field source and no range yet. The tile and halo of `shape` are resolved as `bake_specs` does.
    pub fn new(world: &GlobalWorld, shape: &BakeShape) -> Result<LayerManifest, String> {
        let (_, spec) = crate::layer_tiles::bake_specs(world, shape)?;
        let source = match world.class_source() {
            Some(_) => ManifestSource::Class(planet_generators::bake::inputs::BakeInputs::id()),
            None => ManifestSource::Legacy,
        };
        Ok(LayerManifest {
            shape: BakeShape { tile: Some(spec.grid.tile), halo: Some(spec.grid.halo), ..*shape },
            bake_kernel_version: planet_generators::bake::BAKE_KERNEL_VERSION,
            pass2_kernel_version: planet_generators::bake::PASS2_KERNEL_VERSION,
            tile_layer_version: TILE_LAYER_VERSION,
            layer_version: LAYER_VERSION,
            channels_version: CHANNELS_VERSION,
            quant_step_m: f64::from(QUANT_STEP_M),
            source,
            pyramid_block_cells: planet_generators::erosion_layer::BLOCK_CELLS,
            change_range: None,
        })
    }

    /// Widens the range to hold `[lo, hi]`.
    pub fn widen(&mut self, lo: f64, hi: f64) {
        self.change_range = Some(match self.change_range {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }

    /// The text of the manifest.
    pub fn to_text(&self) -> String {
        let s = &self.shape;
        let mut t = format!("layer-manifest {MANIFEST_VERSION}\n");
        let mut line = |k: &str, v: String| t.push_str(&format!("{k} {v}\n"));
        line("seed", s.seed.to_string());
        line("radius_m", pair(s.radius_m));
        line("face_res", s.face_res.to_string());
        line("planet_res", s.planet_res.to_string());
        line("tile", s.tile.expect("a resolved tile").to_string());
        line("halo", s.halo.expect("a resolved halo").to_string());
        line("bake_kernel_version", self.bake_kernel_version.to_string());
        line("pass2_kernel_version", self.pass2_kernel_version.to_string());
        line("tile_layer_version", self.tile_layer_version.to_string());
        line("layer_version", self.layer_version.to_string());
        line("channels_version", self.channels_version.to_string());
        line("quant_step_m", pair(self.quant_step_m));
        line(
            "source",
            match self.source {
                ManifestSource::Class(id) => format!("class {id:016x}"),
                ManifestSource::Legacy => "legacy".to_string(),
            },
        );
        line("pyramid_block_cells", self.pyramid_block_cells.to_string());
        if let Some((lo, hi)) = self.change_range {
            line("change_min_m", pair(lo));
            line("change_max_m", pair(hi));
        }
        t
    }

    /// The manifest of `text`: an error naming the line when it is not one this build reads.
    pub fn parse(text: &str) -> Result<LayerManifest, String> {
        let mut lines = text.lines();
        let head = lines.next().ok_or("the manifest is empty")?;
        if head != format!("layer-manifest {MANIFEST_VERSION}") {
            return Err(format!("'{head}': this build reads layer-manifest {MANIFEST_VERSION}"));
        }
        let mut map = std::collections::BTreeMap::new();
        for l in lines.filter(|l| !l.trim().is_empty()) {
            let (k, v) = l.split_once(' ').ok_or_else(|| format!("'{l}' is not a `key value` line"))?;
            if map.insert(k, v).is_some() {
                return Err(format!("'{k}' appears twice"));
            }
        }
        let get = |k: &str| map.get(k).copied().ok_or_else(|| format!("the manifest has no '{k}'"));
        let num = |k: &str| -> Result<u64, String> { get(k)?.parse::<u64>().map_err(|e| format!("'{k}': {e}")) };
        let ver = |k: &str| -> Result<u32, String> { get(k)?.parse::<u32>().map_err(|e| format!("'{k}': {e}")) };
        let source = match get("source")? {
            "legacy" => ManifestSource::Legacy,
            s => match s.strip_prefix("class ") {
                Some(id) => ManifestSource::Class(u64::from_str_radix(id, 16).map_err(|e| format!("'source': {e}"))?),
                None => return Err(format!("'source {s}' is neither legacy nor class <id>")),
            },
        };
        let change_range = match (map.get("change_min_m"), map.get("change_max_m")) {
            (Some(lo), Some(hi)) => Some((unpair(lo)?, unpair(hi)?)),
            (None, None) => None,
            _ => return Err("the manifest holds one end of the change range, not both".into()),
        };
        Ok(LayerManifest {
            shape: BakeShape {
                seed: num("seed")?,
                radius_m: unpair(get("radius_m")?)?,
                face_res: num("face_res")? as usize,
                planet_res: num("planet_res")? as usize,
                tile: Some(num("tile")? as usize),
                halo: Some(num("halo")? as usize),
            },
            bake_kernel_version: ver("bake_kernel_version")?,
            pass2_kernel_version: ver("pass2_kernel_version")?,
            tile_layer_version: ver("tile_layer_version")?,
            layer_version: ver("layer_version")?,
            channels_version: ver("channels_version")?,
            quant_step_m: unpair(get("quant_step_m")?)?,
            source,
            pyramid_block_cells: num("pyramid_block_cells")? as usize,
            change_range,
        })
    }

    /// The manifest in `dir`: `Ok(None)` when there is none, an error naming the file when it cannot be read.
    pub fn read(dir: &Path) -> Result<Option<LayerManifest>, String> {
        let path = dir.join(MANIFEST_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => LayerManifest::parse(&text).map(Some).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes the manifest to `dir` atomically: a temporary file of the directory, then a rename over [`MANIFEST_FILE`].
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = dir.join(format!("{MANIFEST_FILE}.tmp-{}", std::process::id()));
        std::fs::write(&tmp, self.to_text()).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, dir.join(MANIFEST_FILE)).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", dir.join(MANIFEST_FILE).display())
        })
    }

    /// Adds this run's manifest to the one in `dir` and writes the result: the shape, the versions and the source must be equal (an error naming both texts otherwise: another layer's run is
    /// never mixed) and the range only widens. Returns what was written.
    pub fn merge_into(&self, dir: &Path) -> Result<LayerManifest, String> {
        let mut merged = match LayerManifest::read(dir)? {
            None => self.clone(),
            Some(old) => {
                let strip = |m: &LayerManifest| LayerManifest { change_range: None, ..m.clone() };
                if strip(&old) != strip(self) {
                    return Err(format!(
                        "{}: the layer manifest there says\n{}this run says\n{}another layer's run is not merged (remove the directory or run with the same shape, versions and source)",
                        dir.display(),
                        strip(&old).to_text(),
                        strip(self).to_text()
                    ));
                }
                old
            }
        };
        if let Some((lo, hi)) = self.change_range {
            merged.widen(lo, hi);
        }
        merged.write(dir)?;
        Ok(merged)
    }

    /// Whether this build reads a layer made with these versions: an error naming each version that differs (a layer of another kernel or record layout is another layer, ADR 0006).
    pub fn check_current(&self) -> Result<(), String> {
        let step = f64::from(QUANT_STEP_M);
        let block = planet_generators::erosion_layer::BLOCK_CELLS;
        let current = [
            ("bake kernel", self.bake_kernel_version, planet_generators::bake::BAKE_KERNEL_VERSION),
            ("pass-2 kernel", self.pass2_kernel_version, planet_generators::bake::PASS2_KERNEL_VERSION),
            ("tile layer record", self.tile_layer_version, TILE_LAYER_VERSION),
            ("pass-1 layer record", self.layer_version, LAYER_VERSION),
            ("channels", self.channels_version, CHANNELS_VERSION),
        ];
        let mut bad: Vec<String> =
            current.iter().filter(|(_, m, c)| m != c).map(|(what, m, c)| format!("{what} version {m} (this build {c})")).collect();
        // The quantisation step and the pyramid's block size are not versions but decide what a stored count means and where a block's range lies: a layer of another step or block is another layer.
        if self.quant_step_m.to_bits() != step.to_bits() {
            bad.push(format!("quantisation step {} m (this build {step} m)", self.quant_step_m));
        }
        if self.pyramid_block_cells != block {
            bad.push(format!("pyramid block of {} cells (this build {block})", self.pyramid_block_cells));
        }
        if bad.is_empty() {
            Ok(())
        } else {
            Err(format!("the layer was made with another version: {}", bad.join(", ")))
        }
    }

    /// The change range of the layer, or the representable range of the counts when no tile is recorded ([`crate::layer_tiles::default_change_range`]).
    pub fn range(&self) -> (f64, f64) {
        self.change_range.unwrap_or_else(crate::layer_tiles::default_change_range)
    }
}
