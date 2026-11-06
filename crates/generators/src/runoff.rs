//! The runoff the carves read (GEN-129; ADR 0104 point 3 as amended, ADR 0108): runoff as precipitation minus Budyko's evapotranspiration, the weight that turns the carve's drainage area into a
//! discharge, and the vegetation factor of the erodibility. Pure functions of `P` and `T` with `libm`; every Earth-specific value is a named constant (the Earth preset).

use crate::orography::aridity_index;

/// Bump whenever a number of this module or a formula changes the weights or the factor; folded into the bake's identity.
pub const RUNOFF_VERSION: u32 = 1;
/// Choudhury's (1999) exponent of Budyko's curve in `E = P PET / (P^n + PET^n)^(1/n)`.
pub const BUDYKO_N: f64 = 2.6;
/// The reference runoff in mm a year: Earth's mean continental runoff (about 40,000 km3 a year over 1.5e8 km2), the runoff of weight 1.
pub const RUNOFF_REF_MM: f64 = 300.0;
/// The bounds of a weight: a desert cell keeps a little area of its own, a rain forest at most eight times the reference.
pub const WEIGHT_MIN: f64 = 0.02;
pub const WEIGHT_MAX: f64 = 8.0;
/// The annual precipitation in mm at which the vegetation factor peaks (the peak of sediment yield, Langbein and Schumm 1958).
pub const VEG_PEAK_MM: f64 = 300.0;
/// The factor of bare desert and dense cover, and at the peak.
pub const VEG_FLOOR: f64 = 0.5;
pub const VEG_PEAK: f64 = 1.5;

/// The potential evapotranspiration in mm a year at `t_c` degrees C (the orographic climate's law).
pub fn pet_mm(t_c: f64) -> f64 {
    1.0 / aridity_index(1.0, t_c)
}

/// The runoff in mm a year for the annual precipitation `p_mm` and temperature `t_c`: `P - E` with Budyko's evaporation in Choudhury's form; 0 for no rain, at most `P`.
pub fn runoff_mm(p_mm: f64, t_c: f64) -> f64 {
    if p_mm <= 0.0 {
        return 0.0;
    }
    let pet = pet_mm(t_c);
    let e = p_mm * pet / libm::pow(libm::pow(p_mm, BUDYKO_N) + libm::pow(pet, BUDYKO_N), 1.0 / BUDYKO_N);
    (p_mm - e).clamp(0.0, p_mm)
}

/// The weight of a cell with runoff `runoff_mm`: its runoff over the reference, within the bounds.
pub fn discharge_weight(runoff_mm: f64) -> f64 {
    (runoff_mm / RUNOFF_REF_MM).clamp(WEIGHT_MIN, WEIGHT_MAX)
}

/// The vegetation factor of the erodibility at the annual precipitation `p_mm`: `VEG_PEAK` at `VEG_PEAK_MM`, `VEG_FLOOR` for none and for much more.
pub fn vegetation_factor(p_mm: f64) -> f64 {
    let u = (p_mm / VEG_PEAK_MM).max(0.0);
    VEG_FLOOR + (VEG_PEAK - VEG_FLOOR) * u * libm::exp(1.0 - u)
}
