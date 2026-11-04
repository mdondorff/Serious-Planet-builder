//! Tier A, STRM-074 (m9-bake): the erosion layer's manifest. It round-trips exactly (the f64s by their bits), a run merges into the manifest in the directory with a range that only widens, a run of
//! another shape, version or source is refused naming both texts, it is written atomically (no temporary file is left), and a layer of another version is named.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use planet_cache::global_store::GlobalWorld;
use planet_cache::layer_manifest::{LayerManifest, ManifestSource, MANIFEST_FILE};
use planet_cache::layer_tiles::BakeShape;
use planet_generators::bake::inputs::BakeInputs;
use planet_world_def::WorldDefinition;

const R: f64 = 6_371_000.0;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p =
            std::env::temp_dir().join(format!("planet-layer-manifest-{}-{}-{name}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&p);
        Dir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn legacy() -> GlobalWorld {
    GlobalWorld::from_definition(&WorldDefinition::single_planet(R, 14)).unwrap()
}

fn shape() -> BakeShape {
    BakeShape { seed: 14, radius_m: R, face_res: 12_288, planet_res: 98_304, tile: None, halo: None }
}

fn manifest() -> LayerManifest {
    LayerManifest::new(&legacy(), &shape()).unwrap()
}

// spec: STRM-074
#[test]
fn a_manifest_resolves_the_shape_records_this_builds_versions_and_round_trips_exactly() {
    let mut m = manifest();
    assert_eq!((m.shape.tile, m.shape.halo), (Some(3072), Some(128)), "the tile and the halo are resolved as bake_specs does");
    assert_eq!(m.source, ManifestSource::Legacy);
    assert_eq!(m.pyramid_block_cells, planet_generators::erosion_layer::BLOCK_CELLS);
    assert_eq!(m.change_range, None);
    m.check_current().unwrap();
    // Without a range the text has none and the reader's range is the representable one.
    assert_eq!(LayerManifest::parse(&m.to_text()).unwrap(), m);
    assert_eq!(m.range(), planet_cache::layer_tiles::default_change_range());
    // A range that is not a short decimal survives exactly, and so does a class source.
    m.widen(-1_752.123_456_789_012_3, 4_321.000_000_000_1);
    let class = LayerManifest::new(&legacy().with_class_source(Arc::new(BakeInputs::new(14, 2))), &shape()).unwrap();
    assert_eq!(class.source, ManifestSource::Class(BakeInputs::id()));
    for x in [&m, &class] {
        let back = LayerManifest::parse(&x.to_text()).unwrap();
        assert_eq!(&back, x);
        assert_eq!(back.shape.radius_m.to_bits(), R.to_bits());
    }
    assert_eq!(LayerManifest::parse(&m.to_text()).unwrap().range(), (-1_752.123_456_789_012_3, 4_321.000_000_000_1));
}

// spec: STRM-074
#[test]
fn a_run_merges_into_the_directory_with_a_range_that_only_widens_and_leaves_no_temporary_file() {
    let d = Dir::new("merge");
    assert_eq!(LayerManifest::read(d.path()).unwrap(), None);
    let mut a = manifest();
    a.widen(-100.0, 50.0);
    a.merge_into(d.path()).unwrap();
    let mut b = manifest();
    b.widen(-20.0, 400.0);
    let merged = b.merge_into(d.path()).unwrap();
    assert_eq!(merged.range(), (-100.0, 400.0), "the range holds both runs");
    let mut c = manifest();
    c.widen(-30.0, 40.0);
    assert_eq!(c.merge_into(d.path()).unwrap().range(), (-100.0, 400.0), "a narrower run does not narrow it");
    assert_eq!(LayerManifest::read(d.path()).unwrap().unwrap(), merged);
    let names: Vec<String> = fs::read_dir(d.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, [MANIFEST_FILE], "the write is a rename: no temporary file remains: {names:?}");
}

// spec: STRM-074
#[test]
fn a_run_of_another_shape_version_or_source_is_refused_naming_both_and_a_layer_of_another_version_is_named() {
    let d = Dir::new("refuse");
    manifest().merge_into(d.path()).unwrap();
    let other_shape = LayerManifest::new(&legacy(), &BakeShape { planet_res: 49_152, tile: Some(1536), ..shape() }).unwrap();
    let e = other_shape.merge_into(d.path()).unwrap_err();
    assert!(e.contains("another layer's run is not merged") && e.contains("planet_res 98304") && e.contains("planet_res 49152"), "{e}");
    let other_version = LayerManifest { pass2_kernel_version: manifest().pass2_kernel_version + 1, ..manifest() };
    assert!(other_version.merge_into(d.path()).unwrap_err().contains("pass2_kernel_version"));
    let class = LayerManifest::new(&legacy().with_class_source(Arc::new(BakeInputs::new(14, 2))), &shape()).unwrap();
    let e = class.merge_into(d.path()).unwrap_err();
    assert!(e.contains("source legacy") && e.contains("source class"), "{e}");
    // Nothing was written by a refusal.
    assert_eq!(LayerManifest::read(d.path()).unwrap().unwrap(), manifest());
    // A layer made with another version is named, version by version.
    let old = LayerManifest { bake_kernel_version: 1, channels_version: 99, ..manifest() };
    let e = old.check_current().unwrap_err();
    assert!(e.contains("bake kernel version 1") && e.contains("channels version 99"), "{e}");
    // A layer of another quantisation step or pyramid block is named too.
    let other = LayerManifest { quant_step_m: 0.5, pyramid_block_cells: 64, ..manifest() };
    let e = other.check_current().unwrap_err();
    assert!(e.contains("quantisation step 0.5 m") && e.contains("pyramid block of 64 cells"), "{e}");
}

// spec: STRM-074
#[test]
fn a_damaged_manifest_is_an_error_naming_the_file_and_the_line() {
    let d = Dir::new("damaged");
    fs::create_dir_all(d.path()).unwrap();
    let path = d.path().join(MANIFEST_FILE);
    for (text, expect) in [
        ("layer-manifest 2\n", "this build reads layer-manifest 1"),
        ("layer-manifest 1\nseed 14\nseed 14\n", "'seed' appears twice"),
        ("layer-manifest 1\nseed 14\n", "the manifest has no '"),
        ("garbage", "layer-manifest"),
    ] {
        fs::write(&path, text).unwrap();
        let e = LayerManifest::read(d.path()).unwrap_err();
        assert!(e.contains(MANIFEST_FILE) && e.contains(expect), "{text:?}: {e}");
    }
    let mut good = manifest().to_text();
    good.push_str("change_min_m -1 0000000000000000\n");
    fs::write(&path, good).unwrap();
    assert!(LayerManifest::read(d.path()).unwrap_err().contains("one end of the change range"));
}
