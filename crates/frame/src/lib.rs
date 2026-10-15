//! `frame`: GPU-free construction of the `FramePlan` (CON-03, ADR 0009). Everything that decides *what* is
//! drawn lives here; `render` only executes the plan. M1 plans a row of tiles shown through a debug view.

pub mod camera;
pub mod controller;
pub mod instances;
pub mod lod;
pub mod plan;
pub mod pool;
pub mod repro;
pub mod residency;
pub mod terrain;
pub mod terrain_text;

pub use camera::{Camera, Frustum};
pub use controller::{CameraController, CameraInput, CameraMode, ControllerParams};
pub use instances::{camera_relative_positions, cull_instances, indirect_args, IndirectArgs, Instance};
pub use lod::{
    balance_leaves, select_nodes, select_nodes_cached, select_nodes_field, select_nodes_field_cached, select_nodes_in_view,
    select_nodes_in_view_cached, select_nodes_in_view_field, LodParams, NodeSelection, SelectedNode, SelectionCache,
};
pub use plan::{plan_tiles, DebugView, FramePlan, PlanError, PlanRequest, TileDraw};
pub use repro::ReproBundle;
pub use residency::{residency_set, residency_set_field, ResidencyEntry, ResidencySet};
pub use terrain::{
    flight_path, flight_path_field, level_color, plan_terrain, plan_terrain_cached, plan_terrain_field, scripted_views,
    scripted_views_field, CountBudget, PlanCounts, TerrainDraw, TerrainPlan, TerrainRequest, TerrainView,
};
pub use terrain_text::{BundleWorld, TerrainBundle};
