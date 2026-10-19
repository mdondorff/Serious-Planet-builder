//! `editor`: egui editor, commands, undo, gizmos, picking. Filled in from M6.
//!
//! `commands` and `ids` are the command layer over the world definition (AUTH-004): GPU-free, no window, no renderer.

pub mod commands;
pub mod ids;
