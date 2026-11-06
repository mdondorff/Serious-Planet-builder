//! The ice history of a planet (GEN-134; ADR 0106 point 1, ADR 0108): the glacial-maximum equilibrium line and the ice extent above it, from the climate's sea-level temperature and rain.
//!
//! The equilibrium line altitude (ELA) is where the glacial-maximum annual temperature equals the temperature an equilibrium line needs, which falls with the rain it receives (a maritime
//! line sits near 0 degrees C, a dry continental one near -10). Ground above the ELA carries ice; where the sea-level temperature is already under the line's, the ELA lies under the ground
//! everywhere and the land is a continental sheet: the sheets come out of the same rule, not a second one. The field is analytic over the orographic climate's lattice (never stored).
//!
//! Every Earth-specific value is a field of [`IceParams`] with Earth's as the default ([`IceParams::earth`]). GPU-free, closed forms on `f64` (`libm` for the logarithm), no thread-count dependence (CON-14).

use crate::global::temperature_c;
use crate::orography::Orography;
use planet_core::{smoothstep, Vec3};

/// Bump whenever the ice field's output changes for the same climate; a consumer folds it into its keys.
pub const ICE_VERSION: u32 = 1;

/// The parameters of the ice history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IceParams {
    /// The cooling of the glacial maximum against the climate, in degrees C. Earth: about 6 (ADR 0106).
    pub lgm_cooling_c: f64,
    /// The annual temperature in degrees C an equilibrium line needs where the rain at it is `ela_dry_precip_mm`. Earth: about -10 (continental glaciers).
    pub ela_dry_temp_c: f64,
    /// The rain in mm a year the dry value belongs to. Earth: 300.
    pub ela_dry_precip_mm: f64,
    /// The rise in degrees C of the needed temperature for every tenfold of rain. Earth: 10 (maritime glaciers, 3,000 mm, sit near 0 degrees C).
    pub ela_temp_per_decade_c: f64,
    /// The range the needed temperature is held to, in degrees C. Earth: -12 to 2.
    pub ela_temp_range_c: (f64, f64),
    /// The height in metres above the ELA over which the ice cover goes from 0 to 1. Calibration.
    pub cover_ramp_m: f64,
    /// The height in metres above sea level over which land counts fully (no ice cover on the sea). Calibration.
    pub land_ramp_m: f64,
}

impl IceParams {
    /// The Earth preset.
    pub fn earth() -> IceParams {
        IceParams {
            lgm_cooling_c: 6.0,
            ela_dry_temp_c: -10.0,
            ela_dry_precip_mm: 300.0,
            ela_temp_per_decade_c: 10.0,
            ela_temp_range_c: (-12.0, 2.0),
            cover_ramp_m: 200.0,
            land_ramp_m: 50.0,
        }
    }

    /// # Panics
    /// When a value is not finite, the rain or a ramp is not positive, the cooling or the rise per decade is negative (the line would rise with the rain), or the temperature range is reversed.
    pub fn new(p: IceParams) -> IceParams {
        let finite = [
            p.lgm_cooling_c,
            p.ela_dry_temp_c,
            p.ela_dry_precip_mm,
            p.ela_temp_per_decade_c,
            p.ela_temp_range_c.0,
            p.ela_temp_range_c.1,
            p.cover_ramp_m,
            p.land_ramp_m,
        ];
        assert!(finite.iter().all(|v| v.is_finite()), "the ice parameters are finite: {p:?}");
        assert!(p.ela_dry_precip_mm > 0.0 && p.cover_ramp_m > 0.0 && p.land_ramp_m > 0.0, "the rain and the ramps are positive: {p:?}");
        assert!(
            p.lgm_cooling_c >= 0.0 && p.ela_temp_per_decade_c >= 0.0 && p.ela_temp_range_c.0 <= p.ela_temp_range_c.1,
            "a cooling, a rise of the needed temperature with the rain (the line would rise with it otherwise), and an ordered range: {p:?}"
        );
        p
    }

    /// The annual temperature in degrees C an equilibrium line needs where the rain at it is `precip_mm`: a line in `log10` of the rain, held to the range.
    pub fn ela_temp_c(&self, precip_mm: f64) -> f64 {
        let decades = libm::log10(precip_mm.max(1.0) / self.ela_dry_precip_mm);
        (self.ela_dry_temp_c + self.ela_temp_per_decade_c * decades).clamp(self.ela_temp_range_c.0, self.ela_temp_range_c.1)
    }

    /// The equilibrium line altitude in metres (negative: the line lies under sea level) for the climate's sea-level temperature `sea_level_c`, a lapse rate of `lapse_c_per_km` and the rain.
    pub fn ela_m(&self, sea_level_c: f64, lapse_c_per_km: f64, precip_mm: f64) -> f64 {
        1000.0 * (sea_level_c - self.lgm_cooling_c - self.ela_temp_c(precip_mm)) / lapse_c_per_km
    }

    /// The ice cover in `[0, 1]` of ground at `ground_m` under an ELA of `ela_m`: 0 below the line, 1 `cover_ramp_m` above it, and 0 at sea (a ramp over `land_ramp_m` above sea level).
    pub fn cover(&self, ground_m: f64, ela_m: f64) -> f64 {
        smoothstep(0.0, self.cover_ramp_m, ground_m - ela_m) * smoothstep(0.0, self.land_ramp_m, ground_m)
    }
}

/// The ice history at one direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IcePoint {
    pub ela_m: f64,
    pub ground_m: f64,
    pub cover: f64,
}

/// The ice history of a seed's climate.
#[derive(Clone, Copy, Debug)]
pub struct IceHistory {
    pub params: IceParams,
}

impl IceHistory {
    pub fn new(params: IceParams) -> IceHistory {
        IceHistory { params: IceParams::new(params) }
    }

    pub fn earth() -> IceHistory {
        IceHistory::new(IceParams::earth())
    }

    /// The ice history at `d` over `climate`: the ELA from the sea-level temperature and the rain, the ground at the climate's base elevation, and the cover.
    pub fn at(&self, climate: &Orography, d: Vec3) -> IcePoint {
        // The sea-level temperature is the function the climate itself was built with (`Orography::build` reads `temperature_c(seed, d, 0.0)` for its vapour column).
        let seed = climate.seed();
        let sea_level_c = temperature_c(seed, d, 0.0);
        let lapse = sea_level_c - temperature_c(seed, d, 1000.0);
        debug_assert!(lapse > 0.0, "the temperature falls with height: {lapse} C per km at {d:?}");
        let ela_m = self.params.ela_m(sea_level_c, lapse, climate.precipitation_mm(d));
        let ground_m = climate.base_elevation_m(d);
        IcePoint { ela_m, ground_m, cover: self.params.cover(ground_m, ela_m) }
    }
}
