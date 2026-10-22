//! Which edits the camera shows (VIEW-015): the footprint find over the visible tiles, then the screen size of the reach. The same set
//! feeds the markers, the outlines and the list of nearby edits; the selected edit is always in it.

use planet_core::{TileId, Vec3};
use planet_frame::globe::Viewport;
use planet_frame::Camera;
use planet_world_def::{BodyId, ItemId, MissingNode, WorldDefinition};
use std::collections::{BTreeMap, BTreeSet};

/// The least size on the screen, in pixels, that the reach of an edit covers for the edit to be shown (the radius of its reach on the
/// screen). A spec value (VIEW-015).
pub const MIN_REACH_PX: f64 = 8.0;

/// The most edits the camera shows at once (VIEW-051): the ones whose reach covers the most pixels. The outlines, markers and the nearby list are
/// drawn for these, so a frame costs the same however many edits the world holds (SPIKE-29: 15,000 edits in view took 100 s a frame).
pub const MAX_SHOWN: usize = 400;

/// How far the ground may lie above or below the sphere an edit's anchor is projected on when the screen test leaves it out, metres: the
/// ground of the bodies this editor edits lies within this of the sphere (an edit nearer than this to the camera is never left out for its place).
pub const HEIGHT_MARGIN_M: f64 = 4_000.0;

/// What the camera sees of the planet.
#[derive(Clone, Copy, Debug)]
pub struct ViewState {
    pub camera: Camera,
    pub viewport: Viewport,
    /// The radius of the body, metres.
    pub radius_m: f64,
}

/// The radius in pixels of a reach of `reach_m` metres around the unit direction `dir`, seen from the camera: the reach over the distance
/// to the point on the sphere, in the units of the vertical field of view. A camera at the point gives infinity.
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
    let mut shown: BTreeSet<ItemId> = footprint_candidates(def, body, layers, tiles)?
        .into_iter()
        .filter(|(_, (dir, reach))| reach_px(view, *dir, *reach) >= min_px)
        .map(|(id, _)| id)
        .collect();
    if let Some(id) = selected.filter(|id| def.item_anchor(body, *id).is_some()) {
        shown.insert(id);
    }
    Ok(shown.into_iter().collect())
}

/// The items whose footprint meets one of `tiles` in one of `layers`, by id, with the unit direction and the reach of each (read from the
/// footprint find, so no lookup per item).
pub fn footprint_candidates(
    def: &WorldDefinition,
    body: BodyId,
    layers: &[&str],
    tiles: &[TileId],
) -> Result<BTreeMap<ItemId, (Vec3, f64)>, MissingNode> {
    let mut found = BTreeMap::new();
    for layer in layers {
        for &tile in tiles {
            for item in def.footprint_items(body, layer, tile, 0.0)? {
                let dir = Vec3::new(f64::from_bits(item.dir[0]), f64::from_bits(item.dir[1]), f64::from_bits(item.dir[2]));
                found.insert(item.id, (dir, f64::from_bits(item.reach_bits)));
            }
        }
    }
    Ok(found)
}

/// Whether the reach disc of an edit at `dir` (the anchor's point on the sphere), widened by `margin_m` for the ground's height, can be seen:
/// the cone of the disc meets the cone of the camera's view (its half diagonal), or the camera is inside the widened disc. It does not test
/// the horizon (the planet hides what lies behind it; the footprint find already keeps the far side out).
pub fn in_view(view: &ViewState, dir: Vec3, reach_m: f64, margin_m: f64) -> bool {
    let to = dir * view.radius_m - view.camera.position.0;
    let d = to.length();
    let rho = reach_m + margin_m;
    if !(d.is_finite() && d > rho) {
        return true;
    }
    let aspect = view.viewport.width_px / view.viewport.height_px;
    let half_diagonal = libm::atan(libm::tan(0.5 * view.camera.fov_y_rad) * libm::sqrt(1.0 + aspect * aspect));
    let forward = view.camera.forward.normalized();
    let angle = libm::atan2(forward.cross(to).length(), forward.dot(to));
    angle - libm::asin(rho / d) <= half_diagonal
}

/// [`relevant_items`] for what the camera shows (VIEW-051): the items whose reach can be seen ([`in_view`]) and covers at least `min_px`, at most
/// `max_shown` of them, those with the largest reach on the screen (ties by the lower id), and `selected` besides. Ascending by id.
pub fn shown_items(
    def: &WorldDefinition,
    body: BodyId,
    layers: &[&str],
    tiles: &[TileId],
    view: &ViewState,
    min_px: f64,
    max_shown: usize,
    selected: Option<ItemId>,
) -> Result<Vec<ItemId>, MissingNode> {
    let candidates = footprint_candidates(def, body, layers, tiles)?;
    Ok(shown_from(&candidates, def, body, view, min_px, max_shown, selected))
}

/// [`shown_items`] from the footprint candidates already found: the find depends on the items and the tiles only, so a frame that changes neither
/// reuses it and only the screen tests, which are the camera's, are made again.
pub fn shown_from(
    candidates: &BTreeMap<ItemId, (Vec3, f64)>,
    def: &WorldDefinition,
    body: BodyId,
    view: &ViewState,
    min_px: f64,
    max_shown: usize,
    selected: Option<ItemId>,
) -> Vec<ItemId> {
    let mut sized: Vec<(f64, ItemId)> = candidates
        .iter()
        .filter(|(_, (dir, reach))| in_view(view, *dir, *reach, HEIGHT_MARGIN_M))
        .map(|(id, (dir, reach))| (reach_px(view, *dir, *reach), *id))
        .filter(|(px, _)| *px >= min_px)
        .collect();
    if sized.len() > max_shown {
        sized.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        sized.truncate(max_shown);
    }
    let mut shown: BTreeSet<ItemId> = sized.into_iter().map(|(_, id)| id).collect();
    if let Some(id) = selected.filter(|id| def.item_anchor(body, *id).is_some()) {
        shown.insert(id);
    }
    shown.into_iter().collect()
}
