//! `frame`: GPU-free construction of the `FramePlan` (CON-03, ADR 0009). Everything that decides *what* is
//! drawn lives here; `render` only executes the plan. M1 plans a row of tiles shown through a debug view.

pub mod lod;
pub mod plan;
pub mod repro;

pub use lod::{balance_leaves, select_nodes, LodParams, NodeSelection, SelectedNode};
pub use plan::{plan_tiles, DebugView, FramePlan, PlanError, PlanRequest, TileDraw};
pub use repro::ReproBundle;
