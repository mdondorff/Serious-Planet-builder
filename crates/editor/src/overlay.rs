//! The outlines, labels and handles of the shown edits on the screen (VIEW-020): the camera's f64 projection of points on the surface,
//! as plain data the egui layer draws. Nothing here touches a window or a GPU.

use crate::polyline::{self, GeoPoint};
use crate::relevance::ViewState;
use crate::session::Session;
use crate::shape::{self, Handle};
use planet_core::Vec3;
use planet_frame::globe::{faces_camera, project};
use planet_world_def::item_store::ItemStore;
use planet_world_def::ItemId;

/// The points of an outline: a closed ring of this many segments.
pub const OUTLINE_SEGMENTS: usize = 64;
/// How many pieces each segment of a line or region is cut into for drawing, so that it follows the sphere.
pub const PATH_PIECES: usize = 8;

/// One edit on the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayItem {
    pub id: ItemId,
    pub label: String,
    pub kind: String,
    pub selected: bool,
    pub hovered: bool,
    /// The ring of the outline in window pixels (origin top-left), `None` for a point behind the camera or outside the viewport; the
    /// ring is closed (the last point joins the first) unless `closed` is false: a line is an open path.
    pub outline: Vec<Option<[f64; 2]>>,
    /// Whether the outline closes on itself (a ring, a region) or is an open path (a line).
    pub closed: bool,
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
            let (outline, closed) = if it.points.is_empty() {
                let ring = (0..OUTLINE_SEGMENTS)
                    .map(|k| {
                        let heading = k as f64 * 360.0 / OUTLINE_SEGMENTS as f64;
                        see(shape::point_at(dir, heading, radius, view.radius_m) * r)
                    })
                    .collect();
                (ring, true)
            } else {
                // A line or a region is its points: the selected one as the session holds them (a held point moves), the others as stored.
                let closed = it.kind == "region";
                let points: Vec<GeoPoint> = if s.selected() == Some(*id) {
                    s.drawn_points(*id)
                } else {
                    it.points.iter().map(|(la, lo)| GeoPoint::new(*la, *lo)).collect()
                };
                (path_pixels(&points, closed, view, ground_m, &see), closed)
            };
            let selected = s.selected() == Some(*id);
            let handles = if selected { s.handles(ground).into_iter().map(|(h, p)| (h, see(p))).collect() } else { Vec::new() };
            Some(OverlayItem {
                id: *id,
                label: crate::lists::label_of(it),
                kind: it.kind.clone(),
                selected,
                hovered: s.hovered() == Some(*id),
                outline,
                closed,
                centre: see(dir * r),
                handles,
            })
        })
        .collect();
    out.sort_by_key(|o| (o.selected, o.id));
    out
}

/// The pixels of a path through `points` on the ground (each sample at the ground height of its place).
fn path_pixels(
    points: &[GeoPoint],
    closed: bool,
    view: &ViewState,
    ground_m: &dyn Fn(Vec3) -> f64,
    see: &dyn Fn(Vec3) -> Option<[f64; 2]>,
) -> Vec<Option<[f64; 2]>> {
    polyline::sample_path(points, closed, PATH_PIECES).into_iter().map(|d| see(d * (view.radius_m + ground_m(d)))).collect()
}

/// The line or region being drawn, on the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DraftOverlay {
    /// The path through the points so far.
    pub path: Vec<Option<[f64; 2]>>,
    /// The points so far.
    pub points: Vec<Option<[f64; 2]>>,
    /// The cursor, where the next point would go: the path's rubber band ends there.
    pub rubber: Option<[f64; 2]>,
}

/// The draft of the line or region tool on the screen; empty when there is none. `cursor` is in the same pixels.
pub fn draft<S: ItemStore>(s: &Session<S>, view: &ViewState, ground_m: &dyn Fn(Vec3) -> f64, cursor: Option<[f64; 2]>) -> DraftOverlay {
    let points = s.draft();
    if points.is_empty() {
        return DraftOverlay::default();
    }
    let see = |p: Vec3| -> Option<[f64; 2]> {
        faces_camera(view.camera.position.0, p.normalized(), view.radius_m).then(|| project(&view.camera, p, view.viewport)).flatten()
    };
    DraftOverlay {
        path: path_pixels(points, false, view, ground_m, &see),
        points: points.iter().map(|p| see(p.dir() * (view.radius_m + ground_m(p.dir())))).collect(),
        rubber: cursor,
    }
}

/// The handles of the selected edit that are on the screen, for the pointer.
pub fn handle_pixels(items: &[OverlayItem]) -> Vec<(Handle, [f64; 2])> {
    items.iter().filter(|o| o.selected).flat_map(|o| o.handles.iter().filter_map(|(h, p)| p.map(|p| (*h, p)))).collect()
}
