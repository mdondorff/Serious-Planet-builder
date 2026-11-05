//! The arid class rules: default weights for the badlands and sand-sea classes (GEN-128; ADR 0104 point 4; ADR 0089).
//!
//! A pure function of five inputs at a point (the aridity of the orographic climate, the lithology hardness, the base elevation, the plate-history terms and, through them, the sand supply), in `libm`-free
//! arithmetic and `smoothstep` ramps (CON-14). The rules do not touch the class lattice: [`AridRules::blend`] mixes their weights over the base class weights at read time
//! (`BakeInputs::weights_at`), so the climate, which reads only the base elevation `e0`, never sees a class weight. Every ramp is smooth, so no weight steps between lattice cells (GEN-097's limit on the
//! step of `k`), and the badlands effect is left to the soft rock's erodibility K (GEN-116) and the hill band, not to a relief-amplitude step at a class edge.
//!
//! **Earth is the default preset, not the model** (owner, 2026-11-05): every threshold is a field of [`AridParams`], whose `Default` is the Earth preset; the kernels read the fields, never a literal.

use crate::classes::CLASS_COUNT;
use crate::lithology::Provinces;
use planet_core::smoothstep;

/// Bump whenever the rules' output changes (the formulas or the Earth preset's values); a consumer folds it into its keys.
pub const ARID_RULES_VERSION: u32 = 1;

/// The index of the badlands class and of the sand-sea class in the class weights.
pub const BADLANDS: usize = 6;
pub const SAND_SEA: usize = 7;

/// The thresholds of the rules: ramps `(from, to)` that rise (or fall) smoothly between two values. `Default` is the Earth preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AridParams {
    /// The hardness `H` below which rock is soft: the weight is 0 at `soft.0` and 1 at `soft.1` (falling hardness).
    pub soft: (f64, f64),
    /// The semi-arid band of the aridity `P / PET`: rises over `semi_from` and falls over `semi_to`.
    pub semi_from: (f64, f64),
    pub semi_to: (f64, f64),
    /// The heights (m above the sea) of the badlands: rising over `moderate_from`, falling over `moderate_to`.
    pub moderate_from: (f64, f64),
    pub moderate_to: (f64, f64),
    /// The aridity below which sand can gather: 1 at `arid.0`, 0 at `arid.1`.
    pub arid: (f64, f64),
    /// The height (m) below which dunes can lie: 1 at `low.0`, 0 at `low.1`.
    pub low: (f64, f64),
    /// The weights of the sediment sources in the sand supply: the basin, the old margin, the rift.
    pub supply: (f64, f64, f64),
    /// The most weight one rule gives, and the most both give together (a neighbouring class keeps the rest).
    pub cap: f64,
    pub cap_both: f64,
}

impl Default for AridParams {
    /// The Earth preset (SPIKE-77's start values; the owner's preview round tunes them).
    fn default() -> AridParams {
        AridParams {
            soft: (0.55, 0.35),
            semi_from: (0.08, 0.25),
            semi_to: (0.40, 0.65),
            moderate_from: (100.0, 400.0),
            moderate_to: (1800.0, 2800.0),
            arid: (0.05, 0.22),
            low: (300.0, 900.0),
            supply: (1.0, 0.5, 0.4),
            cap: 0.8,
            cap_both: 0.85,
        }
    }
}

/// What the rules read at a point.
#[derive(Clone, Copy, Debug)]
pub struct AridInputs {
    /// The aridity `P / PET` (GEN-117).
    pub aridity: f64,
    /// The lithology hardness in `[0, 1]` (GEN-116).
    pub hardness: f64,
    /// The base elevation in metres (before the class relief driver).
    pub e0: f64,
    /// The province terms (GEN-116): the broken relief and the sand supply come from them.
    pub provinces: Provinces,
}

/// The arid rules of one parameter set.
#[derive(Clone, Copy, Debug)]
pub struct AridRules {
    pub params: AridParams,
}

impl AridRules {
    pub fn new(params: AridParams) -> AridRules {
        AridRules { params }
    }

    /// The Earth preset.
    pub fn earth() -> AridRules {
        AridRules::new(AridParams::default())
    }

    /// The weights `(badlands, sand sea)` at a point: each in `[0, cap]`, together at most `cap_both`.
    pub fn weights(&self, i: &AridInputs) -> (f64, f64) {
        let p = &self.params;
        let ramp = |(a, b): (f64, f64), x: f64| smoothstep(a, b, x);
        let soft = ramp(p.soft, i.hardness);
        let semi = ramp(p.semi_from, i.aridity) * (1.0 - ramp(p.semi_to, i.aridity));
        let moderate = ramp(p.moderate_from, i.e0) * (1.0 - ramp(p.moderate_to, i.e0));
        // The relief the water can cut: a foreland, an old margin or the flank of a belt.
        let broken = i.provinces.foreland.max(i.provinces.old_margin).max(0.6 * i.provinces.core).clamp(0.0, 1.0);
        let badlands = (soft * semi * moderate * broken).min(p.cap);
        // Falling ramps: 1 at the first value, 0 at the second (`smoothstep` with its edges swapped).
        let arid = ramp((p.arid.1, p.arid.0), i.aridity);
        let low = ramp((p.low.1, p.low.0), i.e0);
        let supply =
            (p.supply.0 * i.provinces.basin).max(p.supply.1 * i.provinces.old_margin).max(p.supply.2 * i.provinces.rift).clamp(0.0, 1.0);
        let sand = (arid * low * supply).min(p.cap);
        let total = badlands + sand;
        if total > p.cap_both {
            let k = p.cap_both / total;
            (badlands * k, sand * k)
        } else {
            (badlands, sand)
        }
    }

    /// The class weights `base` with the two rules blended over them: `(1 - w_bad - w_sand) base + w_bad e_badlands + w_sand e_sand_sea`, summing to what `base` summed to (1). The base weights'
    /// own badlands and sand-sea shares (0 in the default source) are replaced by the rules'.
    pub fn blend(&self, base: &[f64; CLASS_COUNT], i: &AridInputs) -> [f64; CLASS_COUNT] {
        let (bad, sand) = self.weights(i);
        let rest = 1.0 - bad - sand;
        let mut w = [0.0; CLASS_COUNT];
        let kept: f64 = base.iter().enumerate().filter(|(c, _)| *c != BADLANDS && *c != SAND_SEA).map(|(_, v)| *v).sum();
        for c in 0..CLASS_COUNT {
            if c != BADLANDS && c != SAND_SEA {
                w[c] = if kept > 0.0 { rest * base[c] / kept } else { 0.0 };
            }
        }
        w[BADLANDS] = bad;
        w[SAND_SEA] = sand;
        w
    }
}
