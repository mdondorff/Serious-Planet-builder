//! Which edits the camera shows (VIEW-015): the footprint find over the visible tiles, then the screen size of the reach. The same set
//! feeds the markers, the outlines and the list of nearby edits; the selected edit is always in it.

use planet_core::{TileId, Vec3};
use planet_frame::globe::Viewport;
use planet_frame::Camera;
use planet_world_def::{BodyId, ItemId, MissingNode, WorldDefinition};
use std::collections::BTreeSet;

/// The least size on the screen, in pixels, that the reach of an edit covers for the edit to be shown (the radius of its reach on the
/// screen). A spec value (VIEW-015).
pub const MIN_REACH_PX: f64 = 8.0;

/// What the camera sees of the planet.
#[derive(Clone, Copy, Debug)]
pub struct ViewState {
    pub camera: Camera,
    pub viewport: Viewport,
    /// The radius of the body, metres.
    pub radius_m: f64,
}

/// The radius in pixels of a reach of `reach_m` metres around the unit direction `dir`, seen from the camera: the reach over the distance
/// to the point on the sphere, in the units of the vertical field of view. A camera at the point (or in the reach) gives infinity.
pub fn reach_px(view: &ViewState, dir: Vec3, reach_m: f64) -> f64 {
    let d = (dir * view.radius_m - view.camera.position.0).length();
    if d.is_nan() || d <= 0.0 {
        return f64::INFINITY;
    }
    reach_m / d * (0.5 * view.viewport.height_px) / libm::tan(0.5 * view.camera.fov_y_rad)
}

/// The edits of `body` the camera shows: every item of `layers` whose reach meets one of the visible `tiles` (the footprint find, no scan
/// of all items) and whose reach covers at least `min_px` on the screen, plus `selected` whatever its size or place. Ascending by id.
pub fn relevant_items(
    def: &WorldDefinition,
    body: BodyId,
    layers: &[&str],
    tiles: &[TileId],
    view: &ViewState,
    min_px: f64,
    selected: Option<ItemId>,
) -> Result<Vec<ItemId>, MissingNode> {
    let mut found = BTreeSet::new();
    for layer in layers {
        for &tile in tiles {
            for item in def.footprint_items(body, layer, tile, 0.0)? {
                found.insert(item.id);
            }
        }
    }
    let mut shown: BTreeSet<ItemId> = found
        .into_iter()
        .filter(|id| def.item_anchor(body, *id).is_some_and(|(dir, reach)| reach_px(view, dir, reach) >= min_px))
        .collect();
    if let Some(id) = selected.filter(|id| def.item_anchor(body, *id).is_some()) {
        shown.insert(id);
    }
    Ok(shown.into_iter().collect())
}
