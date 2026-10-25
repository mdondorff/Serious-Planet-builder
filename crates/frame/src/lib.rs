//! `frame`: GPU-free construction of the `FramePlan` (CON-03, ADR 0009). Everything that decides *what* is
//! drawn lives here; `render` only executes the plan. It selects the tiles (height-aware LOD with morphing), plans the frame, runs the streaming loop (pool, residency, prefetch, upload) and holds the cameras, the picking, the look and the scenes.

pub mod adapter;
pub mod camera;
pub mod chunkshare;
pub mod controller;
pub mod draw;
pub mod freecam;
pub mod globe;
pub mod horizonview;
pub mod instances;
pub mod keytable;
pub mod litscenes;
pub mod lod;
pub mod look;
pub mod managed;
pub mod mouselook;
pub mod pathlog;
pub mod pick;
pub mod pipeline;
pub mod plan;
pub mod pool;
pub mod prefetch;
pub mod preview;
pub mod regions;
pub mod repro;
pub mod residency;
pub mod scalerefs;
pub mod scenes;
pub mod terrain;
pub mod terrain_text;
pub mod upload;
pub mod viewer;
pub mod water;

pub use camera::{Camera, Frustum};
pub use controller::{CameraController, CameraInput, CameraMode, ControllerParams};
pub use instances::{camera_relative_positions, cull_instances, indirect_args, IndirectArgs, Instance};
pub use lod::{
    balance_leaves, check_explicit_floor, child_index_toward, cover_selection, cover_selection_bounded, cover_selection_exact,
    cover_selection_masked, effective_floor, floor_tiles, plan_covers_view, production_finest_level, quadrant_bit, select_nodes,
    select_nodes_cached, select_nodes_field, select_nodes_field_cached, select_nodes_in_view, select_nodes_in_view_cached,
    select_nodes_in_view_field, LodParams, NodeSelection, SelectedNode, SelectionCache, TileBounds, ALL_QUADRANTS, DEFAULT_RENDER_FLOOR,
    PRODUCTION_FACE_CELLS_LOG2, REFERENCE_FINEST_LEVEL,
};
pub use plan::{plan_tiles, DebugView, FramePlan, PlanError, PlanRequest, TileDraw};
pub use repro::ReproBundle;
pub use residency::{residency_set, residency_set_field, ResidencyEntry, ResidencySet};
pub use terrain::{
    first_empty_morph_interval, flight_path, flight_path_field, level_color, plan_terrain, plan_terrain_cached, plan_terrain_field,
    plan_terrain_threaded, scripted_views, scripted_views_field, CountBudget, PlanCounts, TerrainDraw, TerrainPlan, TerrainRequest,
    TerrainView, TileState,
};
pub use terrain_text::{BundleWorld, TerrainBundle};
