//! The edit preview (REND-033): the stamps whose delta the vertex stage adds to the stale tiles until the exact ones arrive.
//!
//! A preview stamp is a stamp (GEN-026) with a sign: an item added by an edit is +1, an item removed is -1, a changed item is both its
//! old state (-1) and its new one (+1). The list is computed from the footprint items before and after an edit, GPU-free, and handed
//! to the plan pipeline; the renderer only evaluates it.

use planet_core::Vec3;
use planet_world_def::footprint::FootprintItem;
use planet_world_def::item_id::ItemId;
use planet_world_def::stamp::StampKind;

/// The most preview stamps a plan carries (the globals table of the renderer has this many entries).
pub const MAX_PREVIEW_STAMPS: usize = 8;

/// One signed stamp of the preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewStamp {
    /// +1 adds the stamp's delta, -1 takes it away.
    pub sign: i8,
    pub kind: StampKind,
    /// The stamp's first parameter: the hill's height, the crater's depth, the plateau's target (absolute).
    pub amount_m: f64,
    /// The unit direction of the anchor.
    pub dir: Vec3,
    pub reach_m: f64,
    /// The item's bound `M` (the plateau's clamp).
    pub max_delta_m: f64,
}

/// The list of a delta and how many entries it had to leave out.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PreviewList {
    /// In application order of the items they come from (priority, then id), a changed item's removal before its addition.
    pub stamps: Vec<PreviewStamp>,
    /// Entries beyond [`MAX_PREVIEW_STAMPS`] that were dropped (the farthest from the camera go first).
    pub dropped: usize,
}

fn entry(item: &FootprintItem, sign: i8) -> Option<(i32, ItemId, PreviewStamp)> {
    let stamp = item.stamp?;
    let dir = Vec3::new(f64::from_bits(item.dir[0]), f64::from_bits(item.dir[1]), f64::from_bits(item.dir[2]));
    Some((
        item.priority,
        item.id,
        PreviewStamp {
            sign,
            kind: stamp.kind,
            amount_m: stamp.params()[0],
            dir,
            reach_m: f64::from_bits(item.reach_bits),
            max_delta_m: f64::from_bits(item.max_delta_bits),
        },
    ))
}

/// The preview of an edit that took the footprint items `before` to `after` (any items: only stamps count): the removals and additions
/// that differ, the 8 nearest to `camera` (a planet-fixed position) when there are more, in application order.
pub fn preview_delta(before: &[FootprintItem], after: &[FootprintItem], camera: Vec3) -> PreviewList {
    let mut entries: Vec<(i32, ItemId, PreviewStamp)> = Vec::new();
    for old in before {
        if !after.iter().any(|new| new == old) {
            entries.extend(entry(old, -1));
        }
    }
    for new in after {
        if !before.iter().any(|old| old == new) {
            entries.extend(entry(new, 1));
        }
    }
    let dropped = entries.len().saturating_sub(MAX_PREVIEW_STAMPS);
    if dropped > 0 {
        // Nearest first: by the angle between the anchor and the camera direction (smaller angle = larger cosine), ties by id.
        let cam = camera.normalized();
        entries.sort_by(|a, b| b.2.dir.dot(cam).total_cmp(&a.2.dir.dot(cam)).then(a.1.cmp(&b.1)).then(a.2.sign.cmp(&b.2.sign)));
        entries.truncate(MAX_PREVIEW_STAMPS);
    }
    // Application order; a changed item's removal (-1) comes before its addition (+1).
    entries.sort_by_key(|a| (a.0, a.1, a.2.sign));
    PreviewList { stamps: entries.into_iter().map(|e| e.2).collect(), dropped }
}
