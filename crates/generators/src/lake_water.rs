//! The lakes as a water producer (REND-058, item 15 PR 4a): a direction answers with its lake when the commit's lake mask has the local cell under it wet. The mask and the lake levels come
//! through [`LakeMaskSource`] (the cache implements it over a commit: the L tiles' water sections and the basin tables of the same commit, CON-22); this module only maps the direction to the
//! cell and names the body. Listed after the sea producer in a [`crate::water::WaterField`], so the sea wins where both answer.

use planet_core::cube::{direction_to_face, FaceMapping};
use planet_core::Face;
use planet_world_def::LiquidKindId;

use crate::global::ReadError;
use crate::water::{WaterBodyId, WaterProducer, WaterQuery, WaterSample};

/// The body kinds packed into a [`WaterBodyId`] (REND-058): the kind in bits 60..62, the payload below. The sea is [`WaterBodyId::SEA`] (kind 0, payload 1); a lake is kind 1 with
/// its face in bits 32..40 and its index in that face's basin table in bits 0..32. Bit 63 stays reserved for the bake's component labels ([`WaterBodyId::LABEL_BIT`]).
pub const BODY_KIND_SHIFT: u32 = 60;
pub const BODY_KIND_LAKE: u64 = 1;

/// The body of the lake at `index` of the basin table of `face`.
pub fn lake_body(face: Face, index: u32) -> WaterBodyId {
    WaterBodyId((BODY_KIND_LAKE << BODY_KIND_SHIFT) | (u64::from(face.0) << 32) | u64::from(index))
}

/// The face and table index of a lake body, `None` for any other body.
pub fn lake_of_body(body: WaterBodyId) -> Option<(Face, u32)> {
    (body.0 >> BODY_KIND_SHIFT == BODY_KIND_LAKE).then_some((Face(((body.0 >> 32) & 0xff) as u8), (body.0 & 0xffff_ffff) as u32))
}

/// A lake under one local cell of the mask grid: the face whose basin table lists it, its index there and its level in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LakeCell {
    pub face: Face,
    pub index: u32,
    pub level_m: f64,
}

/// The lake mask of a commit, read by local cell.
pub trait LakeMaskSource: Sync {
    /// Local cells a face edge of the mask grid.
    fn cells(&self) -> usize;
    /// The lake whose water the mask has at local cell `(i, j)` of `face`, if any (a cell outside the face or of a tile the commit does not hold is dry).
    fn lake_at(&self, face: Face, i: i64, j: i64) -> Result<Option<LakeCell>, ReadError>;
}

/// The lakes of a commit as water: W is the lake's level where the mask is wet.
pub struct LakeProducer<'a> {
    map: &'a (dyn FaceMapping + Sync),
    mask: &'a dyn LakeMaskSource,
    liquid: LiquidKindId,
}

impl<'a> LakeProducer<'a> {
    /// The liquid is the sea's for now (architect, 2026-11-11: a fresh or saline liquid is a schema change of its own).
    pub fn new(map: &'a (dyn FaceMapping + Sync), mask: &'a dyn LakeMaskSource, liquid: LiquidKindId) -> LakeProducer<'a> {
        LakeProducer { map, mask, liquid }
    }

    /// The local cell of the mask grid under a unit direction.
    pub fn cell_of(&self, dir: planet_core::Vec3) -> (Face, i64, i64) {
        let (face, u, v) = direction_to_face(self.map, dir);
        let n = self.mask.cells() as f64;
        let to = |x: f64| ((x + 1.0) * 0.5 * n).floor().clamp(0.0, n - 1.0) as i64;
        (face, to(u), to(v))
    }
}

impl WaterProducer for LakeProducer<'_> {
    fn water_at(&self, q: &WaterQuery) -> Result<Option<WaterSample>, ReadError> {
        let (face, i, j) = self.cell_of(q.dir);
        Ok(self.mask.lake_at(face, i, j)?.map(|l| WaterSample { level_m: l.level_m, body: lake_body(l.face, l.index), kind: self.liquid }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REND-058
    #[test]
    fn a_lake_body_never_equals_the_sea_or_a_bake_label_and_round_trips() {
        for f in 0..6u8 {
            for index in [0u32, 1, 7, u32::MAX] {
                let b = lake_body(Face(f), index);
                assert_ne!(b, WaterBodyId::SEA);
                assert_eq!(b.0 & WaterBodyId::LABEL_BIT, 0, "bit 63 stays the bake labels'");
                assert_eq!(lake_of_body(b), Some((Face(f), index)));
            }
        }
        assert_eq!(lake_of_body(WaterBodyId::SEA), None);
        assert_eq!(lake_of_body(WaterBodyId(WaterBodyId::LABEL_BIT | 5)), None);
        assert_ne!(lake_body(Face(0), 1), lake_body(Face(1), 1), "the face is part of the body");
    }
}
