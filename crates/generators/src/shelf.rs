//! The continental shelf of the default class source (GEN-118; ADR 0107, ADR 0108): the ocean floor falls from the coast to the shelf break over a width set by the margin type, then down the
//! continental slope to the abyssal depth. Land is untouched.
//!
//! The margin type is a smooth weight from the plate mask (active where a convergent margin lies near, passive elsewhere), never a switch. Every value is a named parameter with Earth's as the
//! default ([`ShelfProfile::earth`]); a tuning changes the preset's defaults, not this code.
//!
//! GPU-free, `libm`-free closed forms on `f64`, no thread-count dependence (CON-14).

use planet_core::smoothstep;

/// The parameters of the shelf profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShelfProfile {
    /// The depth of the shelf break in metres (positive down). Earth: about 130 m (the global median, Kennett 1982).
    pub break_depth_m: f64,
    /// The depth of the abyssal plain the slope ends at, in metres (positive down). Earth: the ocean basins' 4,500 m of the global field.
    pub abyssal_depth_m: f64,
    /// The shelf width at a passive margin in km. Earth: tens to a few hundred km (Atlantic margins; the global mean about 80 km, the passive median higher).
    pub passive_width_km: f64,
    /// The shelf width at an active margin in km. Earth: a few tens of km (Pacific rim trenches).
    pub active_width_km: f64,
    /// The horizontal extent of the continental slope from the shelf break to the abyssal depth, in km. Earth: 50 to 150 km (3 to 6 degrees).
    pub slope_width_km: f64,
    /// The distance to a plate margin, in km, within which a margin counts as active: full weight up to the first, none from the second.
    pub active_within_km: (f64, f64),
    /// The convergence rates (the mask's units) over which a margin goes from passive to active.
    pub active_convergence: (f64, f64),
    /// The cells per face edge of the lattice of the distance to the coast (a power of two): about 20 km a cell at 512. Calibration.
    pub coast_lattice_cells: usize,
    /// The sea depth (land depth) over which the profile fades in from the field's coast, so that it meets the land's coast ramp. Calibration.
    pub coast_ramp: f64,
    /// The water depth in metres a sea has beyond a thin coastal strip of `min_sea_ramp` of sea depth (a bay is not dry). Calibration.
    pub min_sea_m: f64,
    pub min_sea_ramp: f64,
}

impl ShelfProfile {
    /// The Earth preset.
    pub fn earth() -> ShelfProfile {
        ShelfProfile {
            break_depth_m: 130.0,
            abyssal_depth_m: 4500.0,
            passive_width_km: 80.0,
            active_width_km: 30.0,
            slope_width_km: 90.0,
            active_within_km: (150.0, 450.0),
            active_convergence: (0.05, 0.35),
            coast_lattice_cells: 512,
            coast_ramp: 0.01,
            min_sea_m: 5.0,
            min_sea_ramp: 0.01,
        }
    }

    /// # Panics
    /// When a depth, width or ramp length is not positive and finite, the break is not above the abyssal plain, or a ramp has equal or reversed edges (it would be 0/0).
    pub fn new(p: ShelfProfile) -> ShelfProfile {
        let positive = [
            p.break_depth_m,
            p.abyssal_depth_m,
            p.passive_width_km,
            p.active_width_km,
            p.slope_width_km,
            p.coast_ramp,
            p.min_sea_m,
            p.min_sea_ramp,
        ];
        assert!(positive.iter().all(|v| v.is_finite() && *v > 0.0), "the shelf's depths and widths are positive and finite: {p:?}");
        assert!(p.break_depth_m < p.abyssal_depth_m, "the shelf break lies above the abyssal plain: {p:?}");
        assert!(
            p.coast_lattice_cells.is_power_of_two() && p.coast_lattice_cells >= 4,
            "the coast lattice has a power of two cells per face edge: {p:?}"
        );
        let ramps = [p.active_within_km, p.active_convergence];
        assert!(
            ramps.iter().all(|r| r.0.is_finite() && r.1.is_finite() && r.0 < r.1),
            "every ramp of the shelf has two ordered edges: {p:?}"
        );
        p
    }

    /// The activity in `[0, 1]` of one plate margin `dist_km` away with the plates converging at `convergence` (the mask's units): 1 at a converging margin close by, 0 far from it or where
    /// the plates do not converge.
    pub fn margin_term(&self, dist_km: f64, convergence: f64) -> f64 {
        let (n0, n1) = self.active_within_km;
        let (c0, c1) = self.active_convergence;
        (1.0 - smoothstep(n0, n1, dist_km)) * smoothstep(c0, c1, convergence)
    }

    /// The shelf width in km for an active weight in `[0, 1]`: passive's at 0, active's at 1, a smooth blend between (no switch at a margin).
    pub fn width_km(&self, active: f64) -> f64 {
        let a = active.clamp(0.0, 1.0);
        self.passive_width_km + (self.active_width_km - self.passive_width_km) * a
    }

    /// The sea floor in metres (at most 0) at `x_km` from the coast, for a shelf of `width_km`: linear down the shelf to the break, then a smoothstep down the slope to the abyssal plain.
    /// Continuous, 0 at the coast, monotone non-increasing, and flat at the abyssal depth beyond the slope.
    pub fn floor_m(&self, x_km: f64, width_km: f64) -> f64 {
        let x = x_km.max(0.0);
        if x <= width_km {
            -self.break_depth_m * x / width_km
        } else {
            let t = smoothstep(0.0, 1.0, (x - width_km) / self.slope_width_km);
            -(self.break_depth_m + (self.abyssal_depth_m - self.break_depth_m) * t)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: GEN-118
    #[test]
    fn the_earth_preset_is_valid() {
        assert_eq!(ShelfProfile::new(ShelfProfile::earth()), ShelfProfile::earth());
    }
}
