//! `editor`: egui editor, commands, undo, gizmos, picking. Filled in from M6.
//!
//! `commands` and `ids` are the command layer over the world definition (AUTH-004): GPU-free, no window, no renderer.
//!
//! `relevance`, `lists`, `shape`, `params`, `layout` and `session` are the editor model (VIEW-015 to VIEW-018) the panels in `app` draw: what
//! the camera shows, the lists and the search, the handles and their drags, the inspector's values and the layout as data. Also GPU-free.

pub mod commands;
pub mod ids;
pub mod layout;
pub mod lists;
pub mod params;
pub mod relevance;
pub mod session;
pub mod shape;
