//! The world's rounding precision (GEN-175, GEN-166's limit): a per-world flood epsilon, fixed from the world's definition.
//!
//! The rounds store heights as f32 as they are (metres above the sea level, which is 0 in the carve's frame), so the f32 spacing is finest at the coasts, outlets, lakes and the K7 floor, where
//! the physics lives, and coarsest at the summits. The flood's epsilon must exceed the spacing at the largest `R = max |h|` over the world, or carved epsilon chains collapse to equal heights
//! near the summits: `epsilon_w = max(epsilon_profile, SPACING_FACTOR x spacing(R) x (1 + TIE_MARGIN))`. Only when `epsilon_w` would exceed the named `epsilon_max` do the rounds run in f64.
//! The choice depends on the world's definition only (CON-30): never on a window, a tile or the heights found there. The bounds passed in must hold every height a round can produce
//! (the base heights plus the profile's largest uplift).

use super::carve::CarveParams;

/// The largest `epsilon_w` the f32 rounds accept (a named parameter, metres). Above it the world's rounds run in f64. 0.01 m keeps f32 up to `R` = 131 km.
pub const EPSILON_MAX_M: f64 = 0.01;

/// The factor on the f32 spacing at `R` in `epsilon_w`. 1, and strictly above: an epsilon step above the spacing at the largest `|h|` stays strictly descending after the narrowing even when a
/// height lands on a rounding tie (a step of exactly the spacing would not), and Earth's span (R = 11 km, spacing 9.77e-4 m up to 16,384 m) keeps the profile epsilon of 1e-3 m.
pub const SPACING_FACTOR: f64 = 1.0;

/// The relative margin above the spacing (2^-20, far above the f64 rounding of a height of f32 size): an epsilon of exactly the spacing collapses two heights on a rounding tie.
pub const TIE_MARGIN: f64 = 1.0 / 1_048_576.0;

/// The spacing of the f32 numbers at `r` (metres): the gap from `|r|` to the next f32 above it. 0 for `r = 0`; infinite for a value that does not fit an f32 (or NaN).
pub fn f32_spacing(r: f64) -> f64 {
    let a = r.abs() as f32;
    if a == 0.0 {
        return 0.0;
    }
    if !a.is_finite() {
        return f64::INFINITY;
    }
    f64::from(f32::from_bits(a.to_bits() + 1)) - f64::from(a)
}

/// The precision of one world's rounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPrecision {
    /// `R = max |h|` over the world's height bounds (metres).
    pub radius: f64,
    /// The flood epsilon of the world: `max(epsilon_profile, SPACING_FACTOR x spacing(R) x (1 + TIE_MARGIN))` when the rounds are f32, `epsilon_profile` when f64.
    pub epsilon_w: f64,
    /// The rounds run in f64 (the epsilon the f32 rounds need exceeds `epsilon_max`, or does not exist).
    pub f64_rounds: bool,
}

impl WorldPrecision {
    /// The precision of a world whose heights lie in `[lo, hi]` metres, for the profile's epsilon and the named `epsilon_max`.
    pub fn new(lo: f64, hi: f64, epsilon_profile: f64, epsilon_max: f64) -> WorldPrecision {
        assert!(lo.is_finite() && hi.is_finite() && lo <= hi, "the world's height bounds [{lo}, {hi}] are finite and ordered");
        assert!(epsilon_profile > 0.0 && epsilon_profile.is_finite(), "the profile epsilon is finite and positive");
        assert!(
            epsilon_max.is_finite() && epsilon_max >= epsilon_profile,
            "epsilon_max {epsilon_max} is finite and at least the profile epsilon"
        );
        let radius = lo.abs().max(hi.abs());
        let spacing = SPACING_FACTOR * f32_spacing(radius);
        let need = spacing * (1.0 + TIE_MARGIN);
        let want = epsilon_profile.max(need);
        if want > epsilon_max {
            WorldPrecision { radius, epsilon_w: epsilon_profile, f64_rounds: true }
        } else {
            WorldPrecision { radius, epsilon_w: want, f64_rounds: false }
        }
    }

    /// Set the carve's epsilon and f32 flag from the world's precision.
    pub fn apply(&self, c: &mut CarveParams) {
        c.epsilon = self.epsilon_w;
        c.round_f32 = !self.f64_rounds;
    }

    /// The MARK line of the choice (recorded with the bake; the key carries the choice through the carve parameters: the epsilon and the f32 byte).
    pub fn mark_line(&self) -> String {
        format!(
            "MARK world-precision rounds={} radius_m={} epsilon_w_m={}",
            if self.f64_rounds { "f64" } else { "f32" },
            self.radius,
            self.epsilon_w
        )
    }
}
