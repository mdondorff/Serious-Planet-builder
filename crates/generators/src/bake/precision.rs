//! The world's rounding precision (GEN-175, GEN-166's limit): a world datum and a per-world epsilon, fixed from the world's definition.
//!
//! The rounds store heights as f32 (GEN-166). Narrowing about the datum (the midpoint of the world's base height bounds) makes the f32 spacing the spacing at
//! `R = max |h - datum|`, so a 60 km span costs the spacing at 30 km, not at the top of the tallest peak. The flood's epsilon must exceed that spacing or the carved epsilon chains collapse
//! to equal heights: `epsilon_w = max(epsilon_profile, SPACING_FACTOR x spacing(R))`. Only when `epsilon_w` would exceed the named `epsilon_max` do the rounds run in f64.
//! The choice depends on the world's definition only (CON-30): never on a window, a tile or the heights found there.

use super::carve::CarveParams;

/// The largest `epsilon_w` the f32 rounds accept (a named parameter, metres). Above it the world's rounds run in f64. 0.01 m keeps f32 up to a half-span `R` of 131 km (a span of 262 km).
pub const EPSILON_MAX_M: f64 = 0.01;

/// The factor on the f32 spacing at `R` in `epsilon_w`. 1: an epsilon step of at least the spacing at the largest `|h - datum|` stays strictly descending after the narrowing, and Earth's span (R = 10 km, spacing 9.77e-4 m) keeps the profile epsilon of 1e-3 m; the architect's text says 2, which would raise Earth's epsilon to 1.95e-3 m (reported, awaiting the architect).
pub const SPACING_FACTOR: f64 = 1.0;

/// The spacing of the f32 numbers at `r` (metres): the gap from `|r|` to the next f32 above it. 0 for `r = 0`.
pub fn f32_spacing(r: f64) -> f64 {
    let a = (r.abs() as f32).max(0.0);
    if a == 0.0 {
        return 0.0;
    }
    f64::from(f32::from_bits(a.to_bits() + 1)) - f64::from(a)
}

/// The precision of one world's rounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPrecision {
    /// The midpoint of the world's base height bounds (metres).
    pub datum: f64,
    /// `R = max |h - datum|` over the world: half the span of the bounds.
    pub radius: f64,
    /// The flood epsilon of the world: `max(epsilon_profile, SPACING_FACTOR x spacing(R))` when the rounds are f32, `epsilon_profile` when f64.
    pub epsilon_w: f64,
    /// The rounds run in f64 (`SPACING_FACTOR x spacing(R)` exceeds `epsilon_max`).
    pub f64_rounds: bool,
}

impl WorldPrecision {
    /// The precision of a world whose base heights lie in `[lo, hi]` metres, for the profile's epsilon and the named `epsilon_max`.
    pub fn new(lo: f64, hi: f64, epsilon_profile: f64, epsilon_max: f64) -> WorldPrecision {
        assert!(lo.is_finite() && hi.is_finite() && lo <= hi, "the world's height bounds [{lo}, {hi}] are finite and ordered");
        assert!(epsilon_profile > 0.0 && epsilon_profile.is_finite(), "the profile epsilon is finite and positive");
        let datum = 0.5 * (lo + hi);
        let radius = 0.5 * (hi - lo);
        let want = epsilon_profile.max(SPACING_FACTOR * f32_spacing(radius));
        if want > epsilon_max {
            WorldPrecision { datum: 0.0, radius, epsilon_w: epsilon_profile, f64_rounds: true }
        } else {
            WorldPrecision { datum, radius, epsilon_w: want, f64_rounds: false }
        }
    }

    /// Set the carve's epsilon, datum and f32 flag from the world's precision.
    pub fn apply(&self, c: &mut CarveParams) {
        c.epsilon = self.epsilon_w;
        c.datum = self.datum;
        c.round_f32 = !self.f64_rounds;
    }

    /// The MARK line of the choice (recorded with the bake; the key carries the same values through the carve parameters).
    pub fn mark_line(&self) -> String {
        format!(
            "MARK world-precision rounds={} datum_m={} radius_m={} epsilon_w_m={}",
            if self.f64_rounds { "f64" } else { "f32" },
            self.datum,
            self.radius,
            self.epsilon_w
        )
    }
}
