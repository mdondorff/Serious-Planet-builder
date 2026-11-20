//! Code-generated descriptor of the real terrain and hydrology pipeline (`BUILD-063`, `TEST-142`).
//!
//! Pure, read-only introspection metadata (`CON-02`, `CON-06`, `CON-07`): references live version constants directly from
//! `planet-world-def` and `planet-generators`, binds bake stages to [`crate::bake::probe::Stage`] hooks (`GEN-161`),
//! separates M9 layer passes from the K5-K7 regional/local chain, and validates [`HeightField::upstream`] against
//! [`field_chain_erosion`](crate::chain::field_chain_erosion).

use crate::arid::ARID_RULES_VERSION;
use crate::bake::lakes::LAKES_VERSION;
use crate::bake::probe::Stage;
use crate::bake::tile::TileGrid;
use crate::bake::{BAKE_KERNEL_VERSION, PASS2_KERNEL_VERSION};
use crate::channels::ChannelSource;
use crate::classes::{CLASS_LIST_VERSION, CLASS_SOURCE_VERSION};
use crate::climate::{CLIMATE_VERSION, HEIGHT_CLIMATE_VERSION};
use crate::coarsehorizon::COARSE_HORIZON_VERSION;
use crate::coupled::{COUPLED_VERSION, LENS_VERSION};
use crate::drivers::{CHANNEL_BLOCK_VERSION, DRIVER_MAP_VERSION};
use crate::erosion::{
    CREASE_BAND_VERSION, EROSION_C1_LAYER_IMPLEMENTATION_ID, EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID, EROSION_C1_VERSION,
    EROSION_LAYER_IMPLEMENTATION_ID, EROSION_LAYER_ONLY_IMPLEMENTATION_ID, EROSION_VERSION, PINNED_DETAIL_VERSION,
};
use crate::erosion_layer::{ErosionLayer, LayerSource, LayerTile, LAYER_READ_VERSION};
use crate::global::{GLOBAL_VERSION, READER_C1_VERSION, READER_VERSION};
use crate::height::{turned_noise, HeightField, GENERATOR_VERSION, KEY_FORMAT_VERSION};
use crate::height_fields::{known_ids, resolve, ErosionSwitch, HeightFieldKind, DEFAULT_HEIGHT_FIELD};
use crate::ice::ICE_VERSION;
use crate::lithology::LITHOLOGY_VERSION;
use crate::mesh::MESH_VERSION;
use crate::orography::OROGRAPHY_VERSION;
use crate::preview::PREVIEW_VERSION;
use crate::relief_channels::{ReliefChannels, RELIEF_CHANNELS_VERSION};
use crate::relief_lattice::ReliefLattice;
use crate::runoff::RUNOFF_VERSION;
use crate::stage::STAGE_KEY_VERSION;
use crate::stamp::{STAMP_STAGE_ID, STAMP_STAGE_VERSION};
use crate::terrain::TERRAIN_VERSION;
use crate::uplift::UPLIFT_VERSION;
use crate::water::WATER_VERSION;
use planet_core::{Face, Vec3};
use planet_world_def::item::{ITEM_SCHEMA_VERSION, ITEM_SCHEMA_VERSION_POINTS, ITEM_SCHEMA_VERSION_STROKES};
use planet_world_def::toml_io::SCHEMA_VERSION;
use planet_world_def::{BodyId, GenContext, WorldDefinition, CHAIN_ORDER_ERA, LAYER_STACK};
use std::fmt::Write as _;
use std::sync::Arc;

/// Schema version of the serialized pipeline JSON document itself.
pub const SCHEMA_FORMAT_VERSION: u32 = 2;

/// Re-run and spatial invalidation scope of a pipeline stage (`BUILD-063`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageScope {
    Global,
    Region,
    Tile,
}

impl StageScope {
    pub fn as_str(self) -> &'static str {
        match self {
            StageScope::Global => "global",
            StageScope::Region => "region",
            StageScope::Tile => "tile",
        }
    }
}

/// CPU-only preview command and exact file artifacts emitted for a stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewSpec {
    pub command_template: &'static str,
    pub files: &'static [&'static str],
    pub optional_files: &'static [&'static str],
}

/// Binding between a catalog stage and [`crate::bake::probe::Stage`] outputs in `docs/stages/manifest.txt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeHook {
    pub stage: Stage,
    pub fields: &'static [&'static str],
}

/// One live `*_VERSION` or behavioral default constant pinned from the Rust source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionPin {
    pub key: &'static str,
    pub value: u32,
    pub const_path: &'static str,
    pub spec_ids: &'static [&'static str],
}

/// Curated node of the terrain and hydrology authoring pipeline (`K1`..`K11`, `M9-P1`/`M9-P2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageDef {
    pub id: &'static str,
    pub title: &'static str,
    pub k_label: &'static str,
    pub scope: StageScope,
    pub summary: &'static str,
    pub reads: &'static [&'static str],
    pub outputs: &'static [&'static str],
    pub version_pins: &'static [&'static str],
    pub probe_hooks: &'static [ProbeHook],
    pub active_when: &'static str,
    pub edit_preview_level: Option<&'static str>,
    pub spec_ids: &'static [&'static str],
    pub preview: Option<PreviewSpec>,
}

/// Active erosion layer generation for a resolved runtime configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErosionBakeMode {
    None,
    M9Layer,
    K5ToK7Commit,
}

impl ErosionBakeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErosionBakeMode::None => "none",
            ErosionBakeMode::M9Layer => "m9-layer",
            ErosionBakeMode::K5ToK7Commit => "k5-k7-commit",
        }
    }
}

/// Runtime switches describing one concrete assembled [`HeightField`] stack.
#[derive(Clone, Debug, PartialEq)]
pub struct PipelineSwitches {
    pub height_field: HeightFieldKind,
    pub erosion_mode: ErosionBakeMode,
    pub erosion_layer: bool,
    pub relief_lattice: bool,
    pub channels: bool,
    pub stamp_layers: Vec<(String, u32)>,
    pub bound_water_level_m: Option<f64>,
}

/// One link along [`HeightField::upstream`], ordered from outermost wrapper to base field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainLink {
    pub implementation_id: String,
    pub version: u32,
    pub stage_layer: Option<(String, u32)>,
    pub has_source_key: bool,
}

/// Predicted or observed [`HeightField`] chain and runtime sidecar metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedChain {
    pub links_top_down: Vec<ChainLink>,
    pub water_cull_level_m: Option<f64>,
    pub has_driver_channels: bool,
    pub turned_noise: bool,
}

/// Maps every [`Stage`] variant in `crate::bake::probe` exhaustively to its catalog stage ID (`BUILD-063`).
pub fn stage_id_for_probe(stage: Stage) -> &'static str {
    match stage {
        Stage::Landmass => "k5-landmass-and-window-cut",
        Stage::Round(_) => "k5-regional-rounds",
        Stage::H1 => "k6-h1-hydrology-and-lakes",
        Stage::Thermal | Stage::Deposition | Stage::PitSpreading => "k6-regional-sediment",
        Stage::K7 { .. } => "k7-local-refinement",
    }
}

/// Live version and toggle constants pinned straight from `planet-world-def` and `planet-generators`.
pub fn version_pins() -> Vec<VersionPin> {
    vec![
        VersionPin {
            key: "world_def_schema",
            value: SCHEMA_VERSION,
            const_path: "planet_world_def::toml_io::SCHEMA_VERSION",
            spec_ids: &["WDEF-002"],
        },
        VersionPin {
            key: "world_def_item_schema",
            value: ITEM_SCHEMA_VERSION,
            const_path: "planet_world_def::item::ITEM_SCHEMA_VERSION",
            spec_ids: &["AUTH-001"],
        },
        VersionPin {
            key: "world_def_item_points",
            value: ITEM_SCHEMA_VERSION_POINTS,
            const_path: "planet_world_def::item::ITEM_SCHEMA_VERSION_POINTS",
            spec_ids: &["AUTH-005"],
        },
        VersionPin {
            key: "world_def_item_strokes",
            value: ITEM_SCHEMA_VERSION_STROKES,
            const_path: "planet_world_def::item::ITEM_SCHEMA_VERSION_STROKES",
            spec_ids: &["AUTH-005"],
        },
        VersionPin {
            key: "world_def_chain_order_era",
            value: CHAIN_ORDER_ERA as u32,
            const_path: "planet_world_def::CHAIN_ORDER_ERA",
            spec_ids: &["GEN-039", "TEST-075"],
        },
        VersionPin {
            key: "class_list",
            value: CLASS_LIST_VERSION,
            const_path: "planet_generators::classes::CLASS_LIST_VERSION",
            spec_ids: &["GEN-094", "GEN-095"],
        },
        VersionPin {
            key: "class_source",
            value: CLASS_SOURCE_VERSION,
            const_path: "planet_generators::classes::CLASS_SOURCE_VERSION",
            spec_ids: &["GEN-094", "GEN-140"],
        },
        VersionPin {
            key: "lithology",
            value: LITHOLOGY_VERSION,
            const_path: "planet_generators::lithology::LITHOLOGY_VERSION",
            spec_ids: &["GEN-116"],
        },
        VersionPin {
            key: "uplift",
            value: UPLIFT_VERSION,
            const_path: "planet_generators::uplift::UPLIFT_VERSION",
            spec_ids: &["GEN-123"],
        },
        VersionPin {
            key: "orography",
            value: OROGRAPHY_VERSION,
            const_path: "planet_generators::orography::OROGRAPHY_VERSION",
            spec_ids: &["GEN-117"],
        },
        VersionPin {
            key: "arid_rules",
            value: ARID_RULES_VERSION,
            const_path: "planet_generators::arid::ARID_RULES_VERSION",
            spec_ids: &["GEN-128"],
        },
        VersionPin { key: "ice_history", value: ICE_VERSION, const_path: "planet_generators::ice::ICE_VERSION", spec_ids: &["GEN-134"] },
        VersionPin {
            key: "global_bake",
            value: GLOBAL_VERSION,
            const_path: "planet_generators::global::GLOBAL_VERSION",
            spec_ids: &["GEN-013", "GEN-136"],
        },
        VersionPin {
            key: "reader_bilinear",
            value: READER_VERSION,
            const_path: "planet_generators::global::READER_VERSION",
            spec_ids: &["GEN-013"],
        },
        VersionPin {
            key: "reader_c1",
            value: READER_C1_VERSION,
            const_path: "planet_generators::global::READER_C1_VERSION",
            spec_ids: &["GEN-013", "GEN-148"],
        },
        VersionPin {
            key: "climate",
            value: CLIMATE_VERSION,
            const_path: "planet_generators::climate::CLIMATE_VERSION",
            spec_ids: &["GEN-021"],
        },
        VersionPin {
            key: "height_climate",
            value: HEIGHT_CLIMATE_VERSION,
            const_path: "planet_generators::climate::HEIGHT_CLIMATE_VERSION",
            spec_ids: &["GEN-021"],
        },
        VersionPin {
            key: "relief_channels",
            value: RELIEF_CHANNELS_VERSION,
            const_path: "planet_generators::relief_channels::RELIEF_CHANNELS_VERSION",
            spec_ids: &["GEN-099"],
        },
        VersionPin {
            key: "coupled_h0",
            value: COUPLED_VERSION,
            const_path: "planet_generators::coupled::COUPLED_VERSION",
            spec_ids: &["GEN-040"],
        },
        VersionPin {
            key: "coupled_lens",
            value: LENS_VERSION,
            const_path: "planet_generators::coupled::LENS_VERSION",
            spec_ids: &["GEN-043"],
        },
        VersionPin {
            key: "coarse_horizon",
            value: COARSE_HORIZON_VERSION,
            const_path: "planet_generators::coarsehorizon::COARSE_HORIZON_VERSION",
            spec_ids: &["STRM-045"],
        },
        VersionPin {
            key: "runoff",
            value: RUNOFF_VERSION,
            const_path: "planet_generators::runoff::RUNOFF_VERSION",
            spec_ids: &["GEN-117"],
        },
        VersionPin {
            key: "bake_kernel",
            value: BAKE_KERNEL_VERSION,
            const_path: "planet_generators::bake::BAKE_KERNEL_VERSION",
            spec_ids: &["GEN-100", "GEN-173"],
        },
        VersionPin {
            key: "pass2_kernel",
            value: PASS2_KERNEL_VERSION,
            const_path: "planet_generators::bake::PASS2_KERNEL_VERSION",
            spec_ids: &["GEN-101", "GEN-176"],
        },
        VersionPin {
            key: "lakes",
            value: LAKES_VERSION,
            const_path: "planet_generators::bake::lakes::LAKES_VERSION",
            spec_ids: &["GEN-163"],
        },
        VersionPin {
            key: "erosion_layer_read",
            value: LAYER_READ_VERSION,
            const_path: "planet_generators::erosion_layer::LAYER_READ_VERSION",
            spec_ids: &["GEN-112", "GEN-188"],
        },
        VersionPin {
            key: "driver_map",
            value: DRIVER_MAP_VERSION,
            const_path: "planet_generators::drivers::DRIVER_MAP_VERSION",
            spec_ids: &["GEN-119"],
        },
        VersionPin {
            key: "channel_block",
            value: CHANNEL_BLOCK_VERSION,
            const_path: "planet_generators::drivers::CHANNEL_BLOCK_VERSION",
            spec_ids: &["GEN-126"],
        },
        VersionPin {
            key: "erosion_field_v1",
            value: EROSION_VERSION,
            const_path: "planet_generators::erosion::EROSION_VERSION",
            spec_ids: &["GEN-017", "GEN-140"],
        },
        VersionPin {
            key: "erosion_field_c1_v2",
            value: EROSION_C1_VERSION,
            const_path: "planet_generators::erosion::EROSION_C1_VERSION",
            spec_ids: &["GEN-017", "GEN-140", "GEN-148"],
        },
        VersionPin {
            key: "crease_band",
            value: CREASE_BAND_VERSION,
            const_path: "planet_generators::erosion::CREASE_BAND_VERSION",
            spec_ids: &["GEN-109"],
        },
        VersionPin {
            key: "pinned_detail",
            value: PINNED_DETAIL_VERSION,
            const_path: "planet_generators::erosion::PINNED_DETAIL_VERSION",
            spec_ids: &["GEN-187"],
        },
        VersionPin {
            key: "turned_noise_default",
            value: u32::from(turned_noise()),
            const_path: "planet_generators::height::turned_noise()",
            spec_ids: &["GEN-140"],
        },
        VersionPin {
            key: "terrain_field",
            value: TERRAIN_VERSION,
            const_path: "planet_generators::terrain::TERRAIN_VERSION",
            spec_ids: &["GEN-016"],
        },
        VersionPin {
            key: "reference_generator",
            value: GENERATOR_VERSION,
            const_path: "planet_generators::height::GENERATOR_VERSION",
            spec_ids: &["GEN-001"],
        },
        VersionPin {
            key: "key_format",
            value: KEY_FORMAT_VERSION,
            const_path: "planet_generators::height::KEY_FORMAT_VERSION",
            spec_ids: &["STRM-003"],
        },
        VersionPin {
            key: "stamp_stage",
            value: STAMP_STAGE_VERSION,
            const_path: "planet_generators::stamp::STAMP_STAGE_VERSION",
            spec_ids: &["GEN-026"],
        },
        VersionPin {
            key: "water_surface",
            value: WATER_VERSION,
            const_path: "planet_generators::water::WATER_VERSION",
            spec_ids: &["GEN-023"],
        },
        VersionPin { key: "mesh", value: MESH_VERSION, const_path: "planet_generators::mesh::MESH_VERSION", spec_ids: &["STRM-003"] },
        VersionPin {
            key: "stage_key",
            value: STAGE_KEY_VERSION,
            const_path: "planet_generators::stage::STAGE_KEY_VERSION",
            spec_ids: &["STRM-011"],
        },
        VersionPin {
            key: "preview_world",
            value: PREVIEW_VERSION,
            const_path: "planet_generators::preview::PREVIEW_VERSION",
            spec_ids: &["GEN-103"],
        },
        VersionPin {
            key: "bake_preview",
            value: crate::bake::preview::PREVIEW_VERSION as u32,
            const_path: "planet_generators::bake::preview::PREVIEW_VERSION",
            spec_ids: &["GEN-103"],
        },
    ]
}

const HOOKS_LANDMASS: [ProbeHook; 1] = [ProbeHook { stage: Stage::Landmass, fields: &["z"] }];
const HOOKS_ROUND: [ProbeHook; 1] = [ProbeHook { stage: Stage::Round(0), fields: &["z", "area (log10 m2)"] }];
const HOOKS_H1: [ProbeHook; 1] = [ProbeHook { stage: Stage::H1, fields: &["q (log10 m2)", "channel"] }];
const HOOKS_SEDIMENT: [ProbeHook; 3] = [
    ProbeHook { stage: Stage::Thermal, fields: &["z", "delta"] },
    ProbeHook { stage: Stage::Deposition, fields: &["z", "delta"] },
    ProbeHook { stage: Stage::PitSpreading, fields: &["z", "delta"] },
];
const HOOKS_K7: [ProbeHook; 1] = [ProbeHook { stage: Stage::K7 { face: 0, ti: 0, tj: 0 }, fields: &["z"] }];

/// Ordered stages and products of the real terrain and hydrology pipeline (`K1`..`K11`, `M9-P1`/`M9-P2`).
pub fn stages() -> Vec<StageDef> {
    vec![
        StageDef {
            id: "world-definition",
            title: "World Definition & Authored Layers",
            k_label: "K1",
            scope: StageScope::Global,
            summary: "Seed, body radius, sea params, `world.height_field`, and spatial-indexed authored items ordered by ADR 0007 layer stack.",
            reads: &[],
            outputs: &["WorldDefinition node hashes", "GenContext read records", "per-layer ItemSpec footprint index"],
            version_pins: &["world_def_schema", "world_def_item_schema", "world_def_chain_order_era"],
            probe_hooks: &[],
            active_when: "always",
            edit_preview_level: Some("L0"),
            spec_ids: &["WDEF-002", "WDEF-004", "AUTH-001"],
            preview: None,
        },
        StageDef {
            id: "continents-and-plates",
            title: "Continents, Plate Mask & Relief Classes",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "Default `ClassSource` continents, active/passive plate margins, base elevation `e0`, 8-class weights, and untapered relief driver `k`.",
            reads: &["world-definition"],
            outputs: &["ClassSource (e0, land_depth, plate terms)", "ClassLattice (256 cells/face, weights, k)"],
            version_pins: &["class_list", "class_source", "preview_world"],
            probe_hooks: &[],
            active_when: "height_field != cpu-ref-height-v2 && global bake source == class",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-094", "GEN-095", "GEN-103", "GEN-104"],
            preview: Some(PreviewSpec {
                command_template: "cargo xtask preview world --seed {seed} --out {out_dir}",
                files: &[
                    "land-sea.png",
                    "elevation.png",
                    "elevation-today.png",
                    "elevation-compare.png",
                    "plate-mask.png",
                    "plateau-backarc.png",
                    "classes-argmax.png",
                    "classes-blend.png",
                    "k.png",
                    "report.txt",
                ],
                optional_files: &[],
            }),
        },
        StageDef {
            id: "shelf-bathymetry",
            title: "Continental Shelf & Ocean Bathymetry",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "Coast-distance lattice and smooth passive/active margin shelf-slope-abyssal profile merged into `ClassSource` sea floor.",
            reads: &["world-definition", "continents-and-plates"],
            outputs: &["CoastLattice distance-to-coast", "ShelfProfile ocean-floor elevation"],
            version_pins: &["class_source", "global_bake"],
            probe_hooks: &[],
            active_when: "height_field != cpu-ref-height-v2 && global bake source == class",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-118", "GEN-136"],
            preview: Some(PreviewSpec {
                command_template: "cargo run --release -p planet -- generate --shelf-preview {out_dir} --seed {seed}",
                files: &["bathymetry.png", "shelf.txt"],
                optional_files: &[
                    "region_passive_2km.png",
                    "region_passive_10km.png",
                    "region_active_2km.png",
                    "region_active_10km.png",
                ],
            }),
        },
        StageDef {
            id: "lithology",
            title: "Analytic Lithology & Rock Strata",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "Unstored `H(d)` hardness, tilted/folded stratigraphic bands `K(d, z)`, caprock flags, and soluble carbonate gates for the erosion carve.",
            reads: &["world-definition", "continents-and-plates"],
            outputs: &["ProvinceLattice", "Column::at(z) erodibility K, caprock, soluble"],
            version_pins: &["lithology"],
            probe_hooks: &[],
            active_when: "regional/global erosion bake",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-116", "GEN-122"],
            preview: Some(PreviewSpec {
                command_template: "cargo run --release -p planet -- generate --lithology-preview {out_dir} --seed {seed}",
                files: &["hardness.png", "strata_surface.png", "strata_200m_below.png", "soluble.png", "section.png", "lithology.txt"],
                optional_files: &[],
            }),
        },
        StageDef {
            id: "tectonic-uplift",
            title: "Analytic Tectonic Uplift",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "Unstored `U(d)` uplift field driven by craton, belt/arc coast, back-arc, and rift province terms.",
            reads: &["world-definition", "lithology"],
            outputs: &["Uplift::at(provinces, d)"],
            version_pins: &["uplift"],
            probe_hooks: &[],
            active_when: "regional/global erosion bake",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-123", "GEN-125"],
            preview: Some(PreviewSpec {
                command_template: "cargo run --release -p planet -- generate --uplift-preview {out_dir} --seed {seed}",
                files: &["uplift.png", "steepness.png", "section.png", "uplift.txt"],
                optional_files: &[],
            }),
        },
        StageDef {
            id: "orographic-climate",
            title: "Two-Season Orographic Climate",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "Jacobi vapour/cloud advection over `e0` for northern summer and winter winds giving precipitation, temperature, aridity, and runoff.",
            reads: &["world-definition", "continents-and-plates", "lithology"],
            outputs: &["Orography 512x512 lattice (precipitation, temperature, aridity, seasonality)"],
            version_pins: &["orography", "runoff"],
            probe_hooks: &[],
            active_when: "height_field != cpu-ref-height-v2 && global bake source == class",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-117", "GEN-129", "GEN-130"],
            preview: Some(PreviewSpec {
                command_template: "cargo run --release -p planet -- generate --climate-preview {out_dir} --seed {seed}",
                files: &[
                    "precipitation_annual_wind.png",
                    "precipitation_two_seasons.png",
                    "precipitation_difference.png",
                    "aridity.png",
                    "seasonality.png",
                    "wind_northern_summer.png",
                    "wind_northern_winter.png",
                    "regions.png",
                    "orography.txt",
                ],
                optional_files: &[],
            }),
        },
        StageDef {
            id: "arid-class-rules",
            title: "Arid Class Rules (Badlands & Sand Sea)",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "Read-time class weight remix for badlands and sand seas from aridity, lithology hardness, base elevation, and sediment supply.",
            reads: &["continents-and-plates", "lithology", "orographic-climate"],
            outputs: &["AridRules blended class weights", "tapered/blended k"],
            version_pins: &["arid_rules", "relief_channels"],
            probe_hooks: &[],
            active_when: "height_field != cpu-ref-height-v2 && global bake source == class",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-128"],
            preview: Some(PreviewSpec {
                command_template: "cargo run --release -p planet -- generate --arid-preview {out_dir} --seed {seed}",
                files: &["classes_after.png", "classes_before.png", "badlands_weight.png", "sand_sea_weight.png", "arid.txt"],
                optional_files: &[],
            }),
        },
        StageDef {
            id: "ice-history",
            title: "Glacial Maximum Ice History",
            k_label: "K2",
            scope: StageScope::Global,
            summary: "LGM equilibrium line altitude (ELA) and glaciated cover mask derived analytically from the two-season climate lattice.",
            reads: &["orographic-climate"],
            outputs: &["IceHistory (ela_m, cover)"],
            version_pins: &["ice_history"],
            probe_hooks: &[],
            active_when: "regional/global ice and fjord stages",
            edit_preview_level: Some("L0"),
            spec_ids: &["GEN-134"],
            preview: Some(PreviewSpec {
                command_template: "cargo run --release -p planet -- generate --ice-preview {out_dir} --seed {seed}",
                files: &["ice_extent.png", "ela.png", "ice.txt"],
                optional_files: &[],
            }),
        },
        StageDef {
            id: "global-bake-lattice",
            title: "Global-Coarse Bake Lattice & Channels",
            k_label: "K3",
            scope: StageScope::Global,
            summary: "Block-stored cube-sphere lattice (production 16,384 cells/face): `elevation_m`, `continent`, climate, ocean mask, water summary, relief `k`/`m`.",
            reads: &["world-definition", "continents-and-plates", "shelf-bathymetry", "orographic-climate", "arid-class-rules"],
            outputs: &["GlobalGrid (cpu-global-coarse-v1)", "OceanLattice", "WaterSummary", "ReliefChannels (k, m)"],
            version_pins: &["global_bake", "reader_bilinear", "reader_c1", "climate", "relief_channels", "stage_key"],
            probe_hooks: &[],
            active_when: "height_field != cpu-ref-height-v2",
            edit_preview_level: Some("L1"),
            spec_ids: &["GEN-013", "GEN-099", "STRM-012"],
            preview: None,
        },
        StageDef {
            id: "coupled-h0",
            title: "Coupled H0 Drainage & Lake Graph",
            k_label: "K4",
            scope: StageScope::Global,
            summary: "Single-thread global D8 priority flood, drainage area accumulation, implicit stream-power step, lake basins, and coarse horizon.",
            reads: &["global-bake-lattice", "lithology", "tectonic-uplift"],
            outputs: &["CoupledMaps (H0 drainage, lakes)", "CoarseHorizon", "CoupledLens debug climate view"],
            version_pins: &["coupled_h0", "coupled_lens", "coarse_horizon"],
            probe_hooks: &[],
            active_when: "coupled run or coarse horizon enabled on stored bake",
            edit_preview_level: Some("L1"),
            spec_ids: &["GEN-040", "GEN-043", "GEN-164", "STRM-045", "STRM-051"],
            preview: None,
        },
        StageDef {
            id: "m9-pass1-face-carve",
            title: "M9 Layer Pass 1: Face Routing & Carve",
            k_label: "M9-P1",
            scope: StageScope::Region,
            summary: "Per-cube-face stream-power carve (`bake_pass1`, 12,288 cells/face default) emitting regional delta and MFD drainage area (`--erosion on`).",
            reads: &["global-bake-lattice", "lithology", "tectonic-uplift", "orographic-climate"],
            outputs: &["FaceChange delta", "Pass-1 drainage area"],
            version_pins: &["bake_kernel", "bake_preview"],
            probe_hooks: &[],
            active_when: "erosion_mode == m9-layer (--erosion on)",
            edit_preview_level: Some("L2"),
            spec_ids: &["GEN-100", "GEN-110", "GEN-120", "GEN-127", "GEN-129"],
            preview: None,
        },
        StageDef {
            id: "m9-pass2-tile-layer",
            title: "M9 Layer Pass 2: Tile Carve & Channels",
            k_label: "M9-P2",
            scope: StageScope::Tile,
            summary: "Local tile carve (`bake_layer`, 98,304 cells/face default) with thermal diffusion, deposition sweep, pit spreading, kind-13 deltas, and driver channels.",
            reads: &["m9-pass1-face-carve", "global-bake-lattice"],
            outputs: &["ErosionLayer kind-13 delta tiles", "ChannelSource (soil_m, area_log2_cells, fill_m, erosion_m)"],
            version_pins: &["pass2_kernel", "erosion_layer_read", "driver_map", "channel_block"],
            probe_hooks: &[],
            active_when: "erosion_mode == m9-layer (--erosion on)",
            edit_preview_level: Some("L2"),
            spec_ids: &["GEN-099", "GEN-101", "GEN-111", "GEN-112", "GEN-121", "STRM-072", "STRM-076"],
            preview: None,
        },
        StageDef {
            id: "k5-landmass-and-window-cut",
            title: "K5 Landmass Extraction & Window Cut Boundary",
            k_label: "K5",
            scope: StageScope::Region,
            summary: "Extracts the regional landmass grid on B and classifies border outlets as `CarveParams::window_cut` exits so breaches get no false floor (`GEN-172`).",
            reads: &["global-bake-lattice"],
            outputs: &["Compact landmass Grid z0", "window_cut / exit_window_cut flags", "stage probe: landmass/z"],
            version_pins: &["bake_kernel"],
            probe_hooks: &HOOKS_LANDMASS,
            active_when: "erosion_mode == k5-k7-commit (--commit DIR)",
            edit_preview_level: Some("L2"),
            spec_ids: &["GEN-161", "GEN-162", "GEN-172"],
            preview: None,
        },
        StageDef {
            id: "k5-regional-rounds",
            title: "K5 Regional Catchment Rounds",
            k_label: "K5",
            scope: StageScope::Region,
            summary: "Catchment-parallel round solver (`round::solve_rounds`) with graded flats (`GEN-173`), f32/f64 precision (`GEN-175`), and memo cache (`GEN-153`).",
            reads: &["k5-landmass-and-window-cut", "lithology", "tectonic-uplift", "orographic-climate"],
            outputs: &["Regional f32 surface S0", "stage probe: round/{z, area (log10 m2)}"],
            version_pins: &["bake_kernel", "bake_preview"],
            probe_hooks: &HOOKS_ROUND,
            active_when: "erosion_mode == k5-k7-commit (--commit DIR)",
            edit_preview_level: Some("L2"),
            spec_ids: &["GEN-153", "GEN-166", "GEN-173", "GEN-178", "BUILD-046", "BUILD-047"],
            preview: None,
        },
        StageDef {
            id: "k6-h1-hydrology-and-lakes",
            title: "K6 H1 Channel Graph, Basins & Lakes",
            k_label: "K6",
            scope: StageScope::Region,
            summary: "Computes canonical D8/MFD H1 hydrology, channel segments with routed discharge `Q_w`, kept lake basins (`lakes_from_hydro`), and cut river pieces (`GEN-163`).",
            reads: &["k5-regional-rounds", "orographic-climate"],
            outputs: &["Hydro channel graph (H1)", "BasinTable / Lakes (GEN-163)", "stage probe: h1/{q, channel}"],
            version_pins: &["lakes", "runoff"],
            probe_hooks: &HOOKS_H1,
            active_when: "erosion_mode == k5-k7-commit (--commit DIR)",
            edit_preview_level: Some("L2"),
            spec_ids: &["GEN-145", "GEN-163", "GEN-170", "GEN-189", "STRM-080", "STRM-084"],
            preview: None,
        },
        StageDef {
            id: "k6-regional-sediment",
            title: "K6 Regional Sediment Pass (Thermal, Deposition, Pits)",
            k_label: "K6",
            scope: StageScope::Region,
            summary: "Applies cross-divide thermal diffusion, fluvial deposition sweeps, and one-cell pit spreading (`bake::sediment::sediment_pass`) after the regional rounds.",
            reads: &["k5-regional-rounds", "k6-h1-hydrology-and-lakes"],
            outputs: &["Settled regional height delta", "stage probes: thermal/{z,delta}, deposition/{z,delta}, pit-spreading/{z,delta}"],
            version_pins: &["pass2_kernel"],
            probe_hooks: &HOOKS_SEDIMENT,
            active_when: "erosion_mode == k5-k7-commit (--commit DIR)",
            edit_preview_level: Some("L2"),
            spec_ids: &["GEN-155", "GEN-178"],
            preview: None,
        },
        StageDef {
            id: "k7-local-refinement",
            title: "K7 Local Tile Refinement & Commit Writer",
            k_label: "K7",
            scope: StageScope::Tile,
            summary: "Starts each L tile from `B + C1(dR)`, burns width-law K6 channels (`w = a Q_w^b`), breaches, carves, runs K7 sediment, and writes `L` + basin tables (`GEN-177`).",
            reads: &["global-bake-lattice", "k5-regional-rounds", "k6-h1-hydrology-and-lakes", "k6-regional-sediment"],
            outputs: &["Kind-13 L delta tiles", "Commit tables & channel graph", "docs/stages/manifest.txt", "stage probe: k7/z"],
            version_pins: &["pass2_kernel", "lakes", "erosion_layer_read", "pinned_detail"],
            probe_hooks: &HOOKS_K7,
            active_when: "erosion_mode == k5-k7-commit (--commit DIR)",
            edit_preview_level: Some("L2"),
            spec_ids: &[
                "GEN-148",
                "GEN-154",
                "GEN-156",
                "GEN-165",
                "GEN-167",
                "GEN-168",
                "GEN-169",
                "GEN-176",
                "GEN-177",
                "STRM-081",
                "STRM-082",
                "BUILD-047",
                "BUILD-060",
                "BUILD-061",
            ],
            preview: Some(PreviewSpec {
                command_template: "cargo xtask stages",
                files: &["docs/stages/manifest.txt"],
                optional_files: &["target/review/stages/pipeline.json", "target/review/stages/ci/r1h.png", "target/review/stages/ci/m1.png"],
            }),
        },
        StageDef {
            id: "runtime-base-field",
            title: "Runtime Base HeightField Selection",
            k_label: "K8",
            scope: StageScope::Tile,
            summary: "Composes `B + L + C` from `world.height_field` plus M9 `--erosion` or K5-K7 `--commit` into `ErosionField`, `TerrainField`, or `Reference`.",
            reads: &["world-definition", "global-bake-lattice", "m9-pass2-tile-layer", "k7-local-refinement"],
            outputs: &["Base HeightField (height, height_bounds, climate, optional driver_channels)"],
            version_pins: &[
                "erosion_field_v1",
                "erosion_field_c1_v2",
                "crease_band",
                "pinned_detail",
                "turned_noise_default",
                "terrain_field",
                "reference_generator",
            ],
            probe_hooks: &[],
            active_when: "always",
            edit_preview_level: Some("L3"),
            spec_ids: &["GEN-016", "GEN-017", "GEN-018", "GEN-109", "GEN-112", "GEN-113", "GEN-140", "GEN-148", "GEN-187", "GEN-188", "STRM-083", "VIEW-069"],
            preview: None,
        },
        StageDef {
            id: "authored-stamp-stages",
            title: "Authored Stamp Stage Stack",
            k_label: "K8",
            scope: StageScope::Tile,
            summary: "Stacks one `StampStage` per populated layer in ADR 0007 order over the base field (`+ A`), passing untouched tiles through (`STRM-029`).",
            reads: &["world-definition", "runtime-base-field"],
            outputs: &["Chained HeightField (upstream() -> below, stage_params() -> (layer, stack_index))"],
            version_pins: &["stamp_stage", "world_def_chain_order_era"],
            probe_hooks: &[],
            active_when: "def.stamp_layers(PLANET) is non-empty",
            edit_preview_level: Some("L3"),
            spec_ids: &["GEN-026", "GEN-027", "GEN-028", "GEN-030", "STRM-027", "STRM-029"],
            preview: None,
        },
        StageDef {
            id: "water-cull-and-tile-plan",
            title: "Water Bound, Drivers, Rivers & Tile Plan",
            k_label: "K11",
            scope: StageScope::Tile,
            summary: "Optional `WaterBounded` frustum cull wrapper, `WaterField` (sea + commit lakes), river plans, driver/channel block encoding, and compact tile meshes.",
            reads: &["authored-stamp-stages", "global-bake-lattice", "k6-h1-hydrology-and-lakes", "m9-pass2-tile-layer"],
            outputs: &["WaterBounded::water_cull_level", "WaterField", "CompactTile (heights, normals, driver, channel, mesh)"],
            version_pins: &["water_surface", "driver_map", "channel_block", "mesh", "key_format"],
            probe_hooks: &[],
            active_when: "always (water_cull_level active when bound_water == true)",
            edit_preview_level: Some("L3"),
            spec_ids: &["GEN-023", "GEN-025", "GEN-074", "GEN-119", "GEN-163", "STRM-003", "STRM-085", "STRM-086"],
            preview: None,
        },
    ]
}

/// Computes the ADR 0007 stack index matching [`crate::chain::with_stamps`].
fn stack_indices_for_layers(layers: &[String]) -> Vec<(String, u32)> {
    let unknown: Vec<&String> = layers.iter().filter(|l| !LAYER_STACK.contains(&l.as_str())).collect();
    layers
        .iter()
        .map(|layer| {
            let idx = LAYER_STACK.iter().position(|s| *s == layer).unwrap_or_else(|| {
                LAYER_STACK.len() + unknown.iter().position(|u| u.as_str() == layer).expect("layer is in the unknown slice")
            });
            (layer.clone(), idx as u32)
        })
        .collect()
}

/// Derives [`PipelineSwitches`] from a [`WorldDefinition`], water-bound flag, and optional M9 [`ErosionSwitch`].
pub fn switches_for_definition(
    def: &WorldDefinition,
    bound_water: bool,
    erosion: Option<&ErosionSwitch>,
) -> Result<PipelineSwitches, String> {
    let height_field = resolve(def).map_err(|e| e.to_string())?;
    let stamp_layers = stack_indices_for_layers(&def.stamp_layers(BodyId::PLANET));
    let bound_water_level_m =
        if bound_water { Some(def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m) } else { None };
    Ok(PipelineSwitches {
        height_field,
        erosion_mode: if erosion.is_some() { ErosionBakeMode::M9Layer } else { ErosionBakeMode::None },
        erosion_layer: erosion.is_some(),
        relief_lattice: erosion.is_some_and(|e| e.relief.is_some()),
        channels: erosion.is_some_and(|e| e.channels.is_some()),
        stamp_layers,
        bound_water_level_m,
    })
}

/// Derives [`PipelineSwitches`] for opening a K5-K7 regional commit (`--commit DIR`, C1 base + layer without relief channels).
pub fn switches_for_commit(def: &WorldDefinition, bound_water: bool) -> Result<PipelineSwitches, String> {
    let stamp_layers = stack_indices_for_layers(&def.stamp_layers(BodyId::PLANET));
    let bound_water_level_m =
        if bound_water { Some(def.sea(BodyId::PLANET).map_err(|e| format!("the sea of the definition: {e}"))?.level_m) } else { None };
    Ok(PipelineSwitches {
        height_field: HeightFieldKind::ErodedC1,
        erosion_mode: ErosionBakeMode::K5ToK7Commit,
        erosion_layer: true,
        relief_lattice: false,
        channels: false,
        stamp_layers,
        bound_water_level_m,
    })
}

fn base_chain_link(switches: &PipelineSwitches) -> Result<ChainLink, String> {
    let implementation_id = if switches.erosion_layer {
        match (switches.height_field, switches.relief_lattice) {
            (HeightFieldKind::Eroded, true) => EROSION_LAYER_IMPLEMENTATION_ID,
            (HeightFieldKind::ErodedC1, true) => EROSION_C1_LAYER_IMPLEMENTATION_ID,
            (HeightFieldKind::Eroded, false) => EROSION_LAYER_ONLY_IMPLEMENTATION_ID,
            (HeightFieldKind::ErodedC1, false) => EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID,
            (other, _) => return Err(format!("--erosion applies to the eroded fields, the world's is {}", other.id())),
        }
    } else {
        switches.height_field.id()
    };
    let version = match switches.height_field {
        HeightFieldKind::Eroded => EROSION_VERSION,
        HeightFieldKind::ErodedC1 => EROSION_C1_VERSION,
        HeightFieldKind::Terrain => TERRAIN_VERSION,
        HeightFieldKind::Reference => GENERATOR_VERSION,
    };
    Ok(ChainLink {
        implementation_id: implementation_id.to_string(),
        version,
        stage_layer: None,
        has_source_key: switches.height_field.needs_bake(),
    })
}

/// Predicts the exact [`ResolvedChain`] that [`crate::chain::field_chain_erosion`] builds for `switches`.
pub fn resolve_runtime_chain(switches: &PipelineSwitches) -> Result<ResolvedChain, String> {
    if switches.channels && !switches.erosion_layer {
        return Err("--channels requires --erosion".into());
    }
    let base = base_chain_link(switches)?;
    let mut links_top_down = Vec::with_capacity(switches.stamp_layers.len() + 1);
    for (layer, stack_index) in switches.stamp_layers.iter().rev() {
        links_top_down.push(ChainLink {
            implementation_id: STAMP_STAGE_ID.to_string(),
            version: STAMP_STAGE_VERSION,
            stage_layer: Some((layer.clone(), *stack_index)),
            has_source_key: false,
        });
    }
    links_top_down.push(base);
    Ok(ResolvedChain {
        links_top_down,
        water_cull_level_m: switches.bound_water_level_m,
        has_driver_channels: switches.erosion_layer && switches.channels,
        turned_noise: turned_noise(),
    })
}

/// Walks a live [`HeightField`] via [`HeightField::upstream`] and inspects runtime wrapper state.
pub fn walk_height_field(field: &dyn HeightField) -> ResolvedChain {
    let mut links_top_down = Vec::new();
    let mut cur = Some(field);
    while let Some(f) = cur {
        links_top_down.push(ChainLink {
            implementation_id: f.implementation_id().to_string(),
            version: f.version(),
            stage_layer: f.stage_params().map(|(l, idx)| (l.to_string(), idx)),
            has_source_key: f.source_key().is_some(),
        });
        cur = f.upstream();
    }
    ResolvedChain {
        links_top_down,
        water_cull_level_m: field.water_cull_level(),
        has_driver_channels: field.driver_channels(&[]).is_some(),
        turned_noise: turned_noise(),
    }
}

fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_str_array(out: &mut String, items: &[&str]) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        push_json_str(out, item);
    }
    out.push(']');
}

fn switch_matrix_rows() -> Vec<(&'static str, bool, bool, bool, &'static str, u32, bool)> {
    let mut rows = Vec::new();
    for kind in HeightFieldKind::ALL {
        let base_ver = match kind {
            HeightFieldKind::Eroded => EROSION_VERSION,
            HeightFieldKind::ErodedC1 => EROSION_C1_VERSION,
            HeightFieldKind::Terrain => TERRAIN_VERSION,
            HeightFieldKind::Reference => GENERATOR_VERSION,
        };
        rows.push((kind.id(), false, false, false, kind.id(), base_ver, kind.needs_bake()));
        if matches!(kind, HeightFieldKind::Eroded | HeightFieldKind::ErodedC1) {
            for (relief, channels) in [(false, false), (true, false), (false, true), (true, true)] {
                let impl_id = match (kind, relief) {
                    (HeightFieldKind::Eroded, false) => EROSION_LAYER_ONLY_IMPLEMENTATION_ID,
                    (HeightFieldKind::Eroded, true) => EROSION_LAYER_IMPLEMENTATION_ID,
                    (HeightFieldKind::ErodedC1, false) => EROSION_C1_LAYER_ONLY_IMPLEMENTATION_ID,
                    (HeightFieldKind::ErodedC1, true) => EROSION_C1_LAYER_IMPLEMENTATION_ID,
                    _ => unreachable!(),
                };
                rows.push((kind.id(), true, relief, channels, impl_id, base_ver, kind.needs_bake()));
            }
        }
    }
    rows
}

fn write_resolved_chain_json(out: &mut String, indent: &str, chain: &ResolvedChain) {
    out.push_str("{\n");
    let _ = writeln!(out, "{indent}  \"turned_noise\": {},", chain.turned_noise);
    let _ = writeln!(out, "{indent}  \"has_driver_channels\": {},", chain.has_driver_channels);
    match chain.water_cull_level_m {
        Some(lvl) => {
            let _ = writeln!(out, "{indent}  \"water_cull_level_m\": {lvl:?},");
        }
        None => {
            let _ = writeln!(out, "{indent}  \"water_cull_level_m\": null,");
        }
    }
    let _ = writeln!(out, "{indent}  \"links_top_down\": [");
    for (i, link) in chain.links_top_down.iter().enumerate() {
        let _ = writeln!(out, "{indent}    {{");
        let _ = write!(out, "{indent}      \"implementation_id\": ");
        push_json_str(out, &link.implementation_id);
        out.push_str(",\n");
        let _ = writeln!(out, "{indent}      \"version\": {},", link.version);
        let _ = writeln!(out, "{indent}      \"has_source_key\": {},", link.has_source_key);
        let _ = write!(out, "{indent}      \"stage_layer\": ");
        match &link.stage_layer {
            Some((name, idx)) => {
                out.push_str("{\"name\": ");
                push_json_str(out, name);
                let _ = write!(out, ", \"stack_index\": {idx}}}");
            }
            None => out.push_str("null"),
        }
        out.push('\n');
        let comma = if i + 1 < chain.links_top_down.len() { "," } else { "" };
        let _ = writeln!(out, "{indent}    }}{comma}");
    }
    let _ = writeln!(out, "{indent}  ]");
    let _ = write!(out, "{indent}}}");
}

fn write_catalog_fields(out: &mut String, indent: &str) {
    let _ = writeln!(out, "{indent}\"schema_format_version\": {SCHEMA_FORMAT_VERSION},");
    let _ = write!(out, "{indent}\"default_height_field\": ");
    push_json_str(out, DEFAULT_HEIGHT_FIELD.id());
    out.push_str(",\n");
    let _ = write!(out, "{indent}\"known_height_fields\": ");
    push_str_array(out, &known_ids());
    out.push_str(",\n");
    let _ = write!(out, "{indent}\"layer_stack\": ");
    push_str_array(out, &LAYER_STACK);
    out.push_str(",\n");
    let _ = writeln!(out, "{indent}\"turned_noise_default\": {},", turned_noise());

    let probe_kinds: Vec<&str> = Stage::ALL_KINDS.iter().map(|s| s.kind_id()).collect();
    let _ = write!(out, "{indent}\"probe_stage_kinds\": ");
    push_str_array(out, &probe_kinds);
    out.push_str(",\n");

    let pins = version_pins();
    let _ = writeln!(out, "{indent}\"version_pins\": [");
    for (i, p) in pins.iter().enumerate() {
        let _ = write!(out, "{indent}  {{\"key\": ");
        push_json_str(out, p.key);
        let _ = write!(out, ", \"value\": {}, \"const_path\": ", p.value);
        push_json_str(out, p.const_path);
        out.push_str(", \"spec_ids\": ");
        push_str_array(out, p.spec_ids);
        let comma = if i + 1 < pins.len() { "," } else { "" };
        let _ = writeln!(out, "}}{comma}");
    }
    let _ = writeln!(out, "{indent}],");

    let stage_list = stages();
    let _ = writeln!(out, "{indent}\"stages\": [");
    for (i, s) in stage_list.iter().enumerate() {
        let _ = writeln!(out, "{indent}  {{");
        let _ = write!(out, "{indent}    \"id\": ");
        push_json_str(out, s.id);
        let _ = write!(out, ",\n{indent}    \"title\": ");
        push_json_str(out, s.title);
        let _ = write!(out, ",\n{indent}    \"k_label\": ");
        push_json_str(out, s.k_label);
        let _ = write!(out, ",\n{indent}    \"scope\": ");
        push_json_str(out, s.scope.as_str());
        let _ = write!(out, ",\n{indent}    \"summary\": ");
        push_json_str(out, s.summary);
        let _ = write!(out, ",\n{indent}    \"reads\": ");
        push_str_array(out, s.reads);
        let _ = write!(out, ",\n{indent}    \"outputs\": ");
        push_str_array(out, s.outputs);
        let _ = write!(out, ",\n{indent}    \"version_pins\": ");
        push_str_array(out, s.version_pins);
        let _ = write!(out, ",\n{indent}    \"probe_hooks\": [");
        for (h_idx, hook) in s.probe_hooks.iter().enumerate() {
            if h_idx > 0 {
                out.push_str(", ");
            }
            out.push_str("{\"kind_id\": ");
            push_json_str(out, hook.stage.kind_id());
            out.push_str(", \"fields\": ");
            push_str_array(out, hook.fields);
            out.push('}');
        }
        let _ = write!(out, "],\n{indent}    \"active_when\": ");
        push_json_str(out, s.active_when);
        let _ = write!(out, ",\n{indent}    \"edit_preview_level\": ");
        match s.edit_preview_level {
            Some(lvl) => push_json_str(out, lvl),
            None => out.push_str("null"),
        }
        let _ = write!(out, ",\n{indent}    \"spec_ids\": ");
        push_str_array(out, s.spec_ids);
        let _ = write!(out, ",\n{indent}    \"preview\": ");
        match s.preview {
            Some(p) => {
                out.push_str("{\"command_template\": ");
                push_json_str(out, p.command_template);
                out.push_str(", \"files\": ");
                push_str_array(out, p.files);
                out.push_str(", \"optional_files\": ");
                push_str_array(out, p.optional_files);
                out.push('}');
            }
            None => out.push_str("null"),
        }
        out.push('\n');
        let comma = if i + 1 < stage_list.len() { "," } else { "" };
        let _ = writeln!(out, "{indent}  }}{comma}");
    }
    let _ = writeln!(out, "{indent}],");

    let matrix = switch_matrix_rows();
    let _ = writeln!(out, "{indent}\"switch_matrix\": [");
    for (i, (hf, er, rel, ch, impl_id, ver, needs_bake)) in matrix.iter().enumerate() {
        let _ = write!(out, "{indent}  {{\"height_field\": ");
        push_json_str(out, hf);
        let _ = write!(out, ", \"erosion_layer\": {er}, \"relief_lattice\": {rel}, \"channels\": {ch}, \"implementation_id\": ");
        push_json_str(out, impl_id);
        let comma = if i + 1 < matrix.len() { "," } else { "" };
        let _ = writeln!(out, ", \"version\": {ver}, \"needs_bake\": {needs_bake}}}{comma}");
    }
    let _ = writeln!(out, "{indent}]");
}

/// Deterministic formatted JSON catalog generated in `target/review/stages/pipeline.json` during tier-A checks.
pub fn to_canonical_json() -> String {
    let mut out = String::from("{\n");
    write_catalog_fields(&mut out, "  ");
    out.push_str("}\n");
    out
}

/// Resolved pipeline JSON for one concrete [`WorldDefinition`] and walked runtime chain (`planet generate --pipeline-json`).
pub fn resolved_world_json(def: &WorldDefinition, switches: &PipelineSwitches, walked: &ResolvedChain) -> String {
    let ctx = GenContext::new(def);
    let seed = ctx.world_seed().unwrap_or(0);
    let radius_m = ctx.body_radius_m(BodyId::PLANET).unwrap_or(0.0);
    let sea_level_m = def.sea(BodyId::PLANET).map_or(0.0, |s| s.level_m);
    let active_stages = active_stage_ids(switches);

    let mut out = String::from("{\n");
    let _ = writeln!(out, "  \"schema_format_version\": {SCHEMA_FORMAT_VERSION},");
    let _ = writeln!(out, "  \"world\": {{");
    let _ = writeln!(out, "    \"seed\": {seed},");
    let _ = writeln!(out, "    \"base_node_hash\": \"{:016x}\",", def.base_hash());
    let _ = writeln!(out, "    \"definition_hash\": \"{:016x}\",", def.hash());
    let _ = writeln!(out, "    \"radius_m\": {radius_m:?},");
    let _ = writeln!(out, "    \"sea_level_m\": {sea_level_m:?},");
    out.push_str("    \"height_field\": ");
    push_json_str(&mut out, switches.height_field.id());
    out.push_str(",\n    \"needs_bake\": ");
    let _ = writeln!(out, "{},", switches.height_field.needs_bake());
    out.push_str("    \"erosion_mode\": ");
    push_json_str(&mut out, switches.erosion_mode.as_str());
    let _ = writeln!(out, ",\n    \"erosion_layer\": {},", switches.erosion_layer);
    let _ = writeln!(out, "    \"relief_lattice\": {},", switches.relief_lattice);
    let _ = writeln!(out, "    \"channels\": {},", switches.channels);
    let _ = write!(out, "    \"stamp_layers\": [");
    for (i, (layer, idx)) in switches.stamp_layers.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str("{\"name\": ");
        push_json_str(&mut out, layer);
        let _ = write!(out, ", \"stack_index\": {idx}}}");
    }
    out.push_str("]\n  },\n");
    out.push_str("  \"active_stages\": ");
    push_str_array(&mut out, &active_stages);
    out.push_str(",\n  \"walked_chain\": ");
    write_resolved_chain_json(&mut out, "  ", walked);
    out.push_str(",\n  \"catalog\": {\n");
    write_catalog_fields(&mut out, "    ");
    out.push_str("  }\n}\n");
    out
}

fn active_stage_ids(switches: &PipelineSwitches) -> Vec<&'static str> {
    let mut ids = vec!["world-definition"];
    if switches.height_field.needs_bake() {
        ids.extend([
            "continents-and-plates",
            "shelf-bathymetry",
            "lithology",
            "tectonic-uplift",
            "orographic-climate",
            "arid-class-rules",
            "ice-history",
            "global-bake-lattice",
            "coupled-h0",
        ]);
    }
    match switches.erosion_mode {
        ErosionBakeMode::None => {}
        ErosionBakeMode::M9Layer => {
            ids.extend(["m9-pass1-face-carve", "m9-pass2-tile-layer"]);
        }
        ErosionBakeMode::K5ToK7Commit => {
            ids.extend([
                "k5-landmass-and-window-cut",
                "k5-regional-rounds",
                "k6-h1-hydrology-and-lakes",
                "k6-regional-sediment",
                "k7-local-refinement",
            ]);
        }
    }
    ids.push("runtime-base-field");
    if !switches.stamp_layers.is_empty() {
        ids.push("authored-stamp-stages");
    }
    ids.push("water-cull-and-tile-plan");
    ids
}

struct MetadataLayerSource {
    grid: TileGrid,
}

impl LayerSource for MetadataLayerSource {
    fn grid(&self) -> &TileGrid {
        &self.grid
    }

    fn tile(&self, _face: Face, _ti: u32, _tj: u32) -> Result<Option<Arc<LayerTile>>, String> {
        Ok(None)
    }

    fn global_range(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn id(&self) -> u64 {
        0x7069_7065_6c61_7972
    }
}

struct MetadataChannelSource;

impl ChannelSource for MetadataChannelSource {
    fn identity(&self) -> u64 {
        0x7069_7065_6368_616e
    }

    fn area_log2_cells(&self, _d: Vec3) -> Result<Option<f32>, String> {
        Ok(None)
    }

    fn deposited_depth_m(&self, _d: Vec3) -> Result<Option<f32>, String> {
        Ok(None)
    }

    fn fill_depth_m(&self, _d: Vec3) -> Result<Option<f32>, String> {
        Ok(None)
    }

    fn erosion_depth_m(&self, _d: Vec3) -> Result<Option<f32>, String> {
        Ok(None)
    }
}

/// Zero-IO [`ErosionSwitch`] stub solely for assembling and inspecting the metadata chain without opening a disk store.
pub fn metadata_erosion_switch(radius_m: f64, sea_level_m: f64, relief: bool, channels: bool) -> Result<ErosionSwitch, String> {
    let grid = TileGrid { cells: 64, tile: 32, halo: 4, radius_m };
    let layer = Arc::new(ErosionLayer::new(Arc::new(MetadataLayerSource { grid }))?);
    let relief_lattice = if relief {
        let faces = vec![(vec![1.0f32; 33 * 33], vec![0.0f32; 33 * 33]); 6];
        let rc = ReliefChannels::from_planes(32, faces).ok_or("building neutral relief channels failed")?;
        Some(Arc::new(ReliefLattice::new(rc)?))
    } else {
        None
    };
    let channels_src: Option<Arc<dyn ChannelSource>> = channels.then(|| Arc::new(MetadataChannelSource) as Arc<dyn ChannelSource>);
    Ok(ErosionSwitch { relief: relief_lattice, layer, sea_level_m, channels: channels_src })
}
