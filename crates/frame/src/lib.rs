//! `frame`: GPU-free construction of the `FramePlan` (CON-03, ADR 0009). Everything that decides *what* is
//! drawn lives here; `render` only executes the plan. M1 plans a row of tiles shown through a debug view.

pub mod camera;
pub mod lod;
pub mod plan;
pub mod repro;
pub mod terrain;

pub use camera::{Camera, Frustum};
pub use lod::{balance_leaves, select_nodes, select_nodes_in_view, LodParams, NodeSelection, SelectedNode};
pub use plan::{plan_tiles, DebugView, FramePlan, PlanError, PlanRequest, TileDraw};
pub use repro::ReproBundle;
pub use terrain::{level_color, plan_terrain, scripted_views, TerrainDraw, TerrainPlan, TerrainRequest, TerrainView};
