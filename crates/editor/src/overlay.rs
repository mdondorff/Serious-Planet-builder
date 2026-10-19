//! The outlines, labels and handles of the shown edits on the screen (VIEW-020): the camera's f64 projection of points on the surface,
//! as plain data the egui layer draws. Nothing here touches a window or a GPU.

use crate::relevance::ViewState;
use crate::session::Session;
use crate::shape::{self, Handle};
use planet_core::Vec3;
use planet_frame::globe::{faces_camera, project};
use planet_world_def::item_store::ItemStore;
use planet_world_def::ItemId;

/// The points of an outline: a closed ring of this many segments.
pub const OUTLINE_SEGMENTS: usize = 64;

/// One edit on the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayItem {
    pub id: ItemId,
    pub label: String,
    pub kind: String,
    pub selected: bool,
    pub hovered: bool,
    /// The ring of the outline in window pixels (origin top-left), `None` for a point behind the camera or outside the viewport; the
    /// ring is closed (the last point joins the first).
    pub outline: Vec<Option<[f64; 2]>>,
    /// The anchor in pixels.
    pub centre: Option<[f64; 2]>,
    /// The handles of the selected edit in pixels; empty for the others.
    pub handles: Vec<(Handle, Option<[f64; 2]>)>,
}

/// The shown edits on the screen, the selected one last (drawn on top). `ground_m` is the height of the ground above the sphere at a
/// direction: the outline and the handles sit on the ground at the edit.
pub fn build<S: ItemStore>(s: &Session<S>, shown: &[ItemId], view: &ViewState, ground_m: &dyn Fn(Vec3) -> f64) -> Vec<OverlayItem> {
    // A point on the screen only when it is on the side of the planet the camera sees (the projection alone also takes the far side).
    let see = |p: Vec3| -> Option<[f64; 2]> {
        faces_camera(view.camera.position.0, p.normalized(), view.radius_m).then(|| project(&view.camera, p, view.viewport)).flatten()
    };
    let mut out: Vec<OverlayItem> = shown
        .iter()
        .filter_map(|id| {
            let it = s.item(*id)?;
            let dir = it.direction();
            let ground = ground_m(dir);
            let r = view.radius_m + ground;
            let radius = shape::outline_radius_m(it);
            let outline = (0..OUTLINE_SEGMENTS)
                .map(|k| {
                    let heading = k as f64 * 360.0 / OUTLINE_SEGMENTS as f64;
                    see(shape::point_at(dir, heading, radius, view.radius_m) * r)
                })
                .collect();
            let selected = s.selected() == Some(*id);
            let handles = if selected { s.handles(ground).into_iter().map(|(h, p)| (h, see(p))).collect() } else { Vec::new() };
            Some(OverlayItem {
                id: *id,
                label: crate::lists::label_of(it),
                kind: it.kind.clone(),
                selected,
                hovered: s.hovered() == Some(*id),
                outline,
                centre: see(dir * r),
                handles,
            })
        })
        .collect();
    out.sort_by_key(|o| (o.selected, o.id));
    out
}

/// The handles of the selected edit that are on the screen, for the pointer.
pub fn handle_pixels(items: &[OverlayItem]) -> Vec<(Handle, [f64; 2])> {
    items.iter().filter(|o| o.selected).flat_map(|o| o.handles.iter().filter_map(|(h, p)| p.map(|p| (*h, p)))).collect()
}
