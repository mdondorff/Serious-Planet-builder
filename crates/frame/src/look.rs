//! `frame::look`: the GPU-free maths of the lit path's presentation data (ADR 0038, ADR 0042 point 7): exposure units, the
//! luminance histogram and its percentile band, the adaptation step, the ACES filmic fit and the sRGB encode, the pre-exposure
//! factor, and the lit terrain's base radiance. Everything the shaders compute with these formulas has its f64 reference here
//! (CON-02, CON-03): the renderer executes, the tests compare. Class: presentation data (ADR 0035); the bounds are stated per
//! function and in `openspec/specs/renderer` (REND-019 to REND-022).
//!
//! Units. Scene radiance is in units of the top-of-atmosphere solar illuminance (= 1, ADR 0038 point 1): a white Lambertian
//! surface facing the sun has `1 / pi`. One unit is [`SOLAR_ILLUMINANCE_LUX`] lux, so `luminance_cd = unit * LUMINANCE_PER_UNIT`
//! for a diffuse surface when the unit radiance is read as luminance per steradian. Exposure follows the saturation-based
//! convention: `exposure = 1 / (1.2 * 2^EV100)` applied to luminance in cd/m2, and the EV100 that suits a luminance `L` is
//! `log2(L * 100 / 12.5) = log2(8 L)`, the same function the histogram bins by.

pub mod atmosphere;
pub mod file;
pub mod materials;
pub mod shading;
pub mod water;

use atmosphere::AtmospherePlan;
use planet_core::hash::hash_u64s;
use shading::{AlbedoParams, ShadeView};

/// The Earth default of the scene-unit scale: illuminance of the sun above the atmosphere in lux (1361 W/m2 at 93 lm/W), the value
/// of "1" in scene units. The value in use comes from the look data ([`SceneUnits`], `file`); this constant is only the default.
pub const SOLAR_ILLUMINANCE_LUX: f64 = 126_600.0;
/// Luminance in cd/m2 of one scene unit of (diffuse) radiance at the Earth default.
pub const LUMINANCE_PER_UNIT: f64 = SOLAR_ILLUMINANCE_LUX;
/// Every write into the scene target is clamped to this, so the sun disk cannot make `inf` or `NaN` in f16 (ADR 0038 point 1).
pub const HDR_CLAMP: f32 = 60_000.0;
/// Bins of the luminance histogram.
pub const HIST_BINS: usize = 64;
/// The Earth default of the histogram axis: EV -10 to +20 of pixel luminance (`log2(8 L)`), 30 stops in 64 bins.
pub const HIST_EV_MIN: f64 = -10.0;
pub const HIST_EV_MAX: f64 = 20.0;
/// The Earth default of the band: the mean is taken over the pixels between these fractions of the counted pixels (ADR 0038 point 2).
pub const BAND_LOW: f64 = 0.4;
pub const BAND_HIGH: f64 = 0.9;
/// Luminance weights of linear Rec. 709 / sRGB primaries.
pub const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// The scene-unit scale (look data, ADR 0048 point 2): one unit of scene radiance is the sun's illuminance above the atmosphere,
/// `solar_illuminance_lux` lux, so a unit of diffuse radiance is that many cd/m2. A body far from the sun has a smaller value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneUnits {
    pub solar_illuminance_lux: f64,
}

impl SceneUnits {
    /// The Earth default (1361 W/m2 at 93 lm/W).
    pub const EARTH: SceneUnits = SceneUnits { solar_illuminance_lux: SOLAR_ILLUMINANCE_LUX };

    /// Luminance in cd/m2 of one scene unit of (diffuse) radiance.
    pub fn luminance_per_unit(&self) -> f64 {
        self.solar_illuminance_lux
    }
}

impl Default for SceneUnits {
    fn default() -> Self {
        Self::EARTH
    }
}

/// The exposure histogram's axis and the percentile band of the mean (look data, ADR 0038 point 2; the bin count 64 is part of
/// the shaders' structure and not data). Bin `i` covers `ev_min + i w` to `ev_min + (i + 1) w` with `w = (ev_max - ev_min) / 64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistogramParams {
    pub ev_min: f64,
    pub ev_max: f64,
    /// The mean is taken over the pixels between these fractions of the counted pixels.
    pub band_low: f64,
    pub band_high: f64,
}

impl HistogramParams {
    /// The Earth default: EV -10 to +20, the band between 40 % and 90 %.
    pub const EARTH: HistogramParams =
        HistogramParams { ev_min: HIST_EV_MIN, ev_max: HIST_EV_MAX, band_low: BAND_LOW, band_high: BAND_HIGH };

    /// Width of one histogram bin in stops.
    pub fn bin_width(&self) -> f64 {
        (self.ev_max - self.ev_min) / HIST_BINS as f64
    }

    /// Centre of bin `i` in EV.
    pub fn bin_center_ev(&self, i: usize) -> f64 {
        self.ev_min + (i as f64 + 0.5) * self.bin_width()
    }

    /// The bin of a pixel EV, or `None` for a pixel that is not counted (no light: NaN or minus infinity; plus infinity is the last
    /// bin). EVs outside the range land in the first or last bin.
    pub fn bin_of_ev(&self, ev: f64) -> Option<usize> {
        if ev.is_nan() || ev == f64::NEG_INFINITY {
            return None;
        }
        let b = libm::floor((ev - self.ev_min) / self.bin_width());
        Some(b.clamp(0.0, (HIST_BINS - 1) as f64) as usize)
    }

    /// The mean EV of the pixels between the band's fractions of the counted ones (bins weighted by how much of their count lies in
    /// the band, bin centres as values); `None` when nothing was counted.
    pub fn band_mean_ev(&self, hist: &[u32; HIST_BINS]) -> Option<f64> {
        let total: u64 = hist.iter().map(|&c| u64::from(c)).sum();
        if total == 0 {
            return None;
        }
        let (lo, hi) = (self.band_low * total as f64, self.band_high * total as f64);
        let (mut below, mut weight, mut sum) = (0.0f64, 0.0f64, 0.0f64);
        for (i, &c) in hist.iter().enumerate() {
            let (a, b) = (below, below + f64::from(c));
            let overlap = (b.min(hi) - a.max(lo)).max(0.0);
            weight += overlap;
            sum += overlap * self.bin_center_ev(i);
            below = b;
        }
        (weight > 0.0).then(|| sum / weight)
    }

    /// The histogram of a scene target image under these bins, the CPU reference of the GPU pass (see [`histogram_of`]).
    pub fn histogram_of(&self, pixels: &[[f64; 3]], pre_ev100: f64) -> [u32; HIST_BINS] {
        let mut h = [0u32; HIST_BINS];
        for p in pixels {
            let y = luminance(*p);
            if y > 0.0 {
                if let Some(b) = self.bin_of_ev(luminance_ev(y / exposure_scale(pre_ev100))) {
                    h[b] += 1;
                }
            }
        }
        h
    }

    /// The EV100 the exposure moves towards: the band mean clamped to the range, or `current` when nothing was counted.
    pub fn target_ev(&self, hist: &[u32; HIST_BINS], params: &ExposureParams, current: f64) -> f64 {
        match self.band_mean_ev(hist) {
            Some(m) => m.clamp(f64::from(params.min_ev100), f64::from(params.max_ev100)),
            None => current,
        }
    }
}

impl Default for HistogramParams {
    fn default() -> Self {
        Self::EARTH
    }
}

/// Exposure range and adaptation speeds (look data, ADR 0042 point 6; the defaults of ADR 0038 point 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExposureParams {
    pub min_ev100: f32,
    pub max_ev100: f32,
    /// Stops per second when the scene gets brighter (EV100 rises; light adaptation is fast).
    pub speed_up: f32,
    /// Stops per second when the scene gets darker (EV100 falls; dark adaptation is slow).
    pub speed_down: f32,
}

impl Default for ExposureParams {
    fn default() -> Self {
        Self { min_ev100: -2.0, max_ev100: 16.0, speed_up: 3.0, speed_down: 1.0 }
    }
}

/// How the exposure is set for a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExposureMode {
    /// A fixed EV100: goldens, perf scenes and repro bundles (CON-19). No histogram, no history.
    Fixed { ev100: f32 },
    /// Automatic exposure from the luminance histogram. `pre_ev100` is the CPU-decided pre-exposure: the EV100 the scene
    /// target is written at (so f16 holds the range), chosen by the view, not read back from the GPU.
    Adaptive { params: ExposureParams, pre_ev100: f32 },
}

impl ExposureMode {
    /// The EV100 the scene target is written at.
    pub fn pre_ev100(&self) -> f32 {
        match *self {
            ExposureMode::Fixed { ev100 } => ev100,
            ExposureMode::Adaptive { pre_ev100, .. } => pre_ev100,
        }
    }

    pub fn is_adaptive(&self) -> bool {
        matches!(self, ExposureMode::Adaptive { .. })
    }
}

/// What the plan says about the lit path of one frame (ADR 0042 point 3). The renderer decides nothing: the sun, the exposure
/// mode, the time step and the reset all arrive here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LitPlan {
    pub exposure: ExposureMode,
    /// Seconds since the previous frame, from the injected clock (CON-04): the only time the exposure adaptation sees.
    pub dt_s: f32,
    /// Unit vector towards the sun in the planet-fixed axes the vertex normals use. Until the sun is a field of the plan
    /// (ADR 0039) the views set it.
    pub sun_dir: [f32; 3],
    /// Snap the adapted exposure to its target now (a cut, a first frame).
    pub reset_exposure: bool,
    /// Tone-map output: normal, or the false-colour luminance debug view (CON-18).
    pub tonemap: ToneMapMode,
    /// The scene-unit scale (look data): the pre-exposure factor is computed from it on the CPU.
    pub units: SceneUnits,
    /// The histogram axis and band the exposure passes use (look data).
    pub histogram: HistogramParams,
    /// The albedo table of the base shading (look data, ADR 0039 point 3).
    pub albedo: AlbedoParams,
    /// The response curves of the ground materials (M7 G2): which material the ground detail shows where (look data, [`materials`]).
    pub materials: materials::MaterialParams,
    /// The strength of the procedural ground detail (M7 G2a, LOOK-009): 0 off (the default, the picture is what it was), 1 full.
    pub ground_detail: f64,
    /// The specular term of the ground that goes with the detail (M7 G2b, LOOK-010): roughness per material and modifier, and the base reflectance.
    pub gloss: materials::GlossParams,
    /// The water level W in metres above the reference surface (ADR 0041; until the water-level field exists the sea's constant
    /// level, 0): the beach band and the seabed are measured from it, never from a hard-coded sea level.
    pub water_level_m: f32,
    /// Which image the terrain shows (debug views, CON-18).
    pub view: ShadeView,
    /// The atmosphere of this frame (ADR 0040): `None` draws none, and the plan's text and hash are what they were before the
    /// atmosphere existed. The body's radius comes from the world definition, the coefficients from the look.
    pub atmosphere: Option<AtmospherePlan>,
    /// The debug overlay of this frame (editor mode only): `None` draws none, and the plan's text and hash are what they were before.
    pub overlay: Overlay,
}

/// A debug overlay drawn over the lit terrain (CON-18): chosen as data in the plan, never as a renderer branch (CON-03). The bigger
/// overlays of the editor (maps, authored regions, surface type) plug into this enum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overlay {
    /// Nothing is drawn over the terrain: the plan's text and hash are what they were before overlays existed.
    #[default]
    None,
    /// The tile outlines and a tint of each tile's level colour (colour per level of detail, dimmed with the morph factor).
    Tiles,
    /// The tile outlines and the tint, and every grid edge and the diagonal of every cell as a fixed line of about a pixel (no fade-out,
    /// however fine the mesh is).
    Mesh,
    /// The surface-type tint of the regions (VIEW-030): the lit colour mixed with the tint of the region over each fragment. The polygons
    /// come from the app per frame ([`RegionList`](crate::regions::RegionList)), not from the lit plan.
    Regions,
    /// The state of each drawn tile against the current definition (VIEW-044): exact, stale, a stand-in, displaced by a preview, capped.
    /// The state of a node comes in the node record ([`TileState`](crate::terrain::TileState)), not from the lit plan.
    Stale,
}

impl Overlay {
    pub fn name(self) -> &'static str {
        match self {
            Overlay::None => "none",
            Overlay::Tiles => "tiles",
            Overlay::Mesh => "mesh",
            Overlay::Regions => "regions",
            Overlay::Stale => "stale",
        }
    }

    pub fn parse(s: &str) -> Option<Overlay> {
        Overlay::ALL.into_iter().find(|o| o.name() == s)
    }

    /// Every overlay, in the order F3 cycles through them.
    pub const ALL: [Overlay; 5] = [Overlay::None, Overlay::Tiles, Overlay::Mesh, Overlay::Regions, Overlay::Stale];

    /// The next one in the cycle (off, tiles, mesh, regions, stale, off).
    pub fn next(self) -> Overlay {
        match self {
            Overlay::None => Overlay::Tiles,
            Overlay::Tiles => Overlay::Mesh,
            Overlay::Mesh => Overlay::Regions,
            Overlay::Regions => Overlay::Stale,
            Overlay::Stale => Overlay::None,
        }
    }

    /// The code the lit shader reads (the globals' `extra.y`): 0 none, 1 tiles, 2 mesh, 3 regions, 4 stale.
    pub fn shader_code(self) -> f32 {
        match self {
            Overlay::None => 0.0,
            Overlay::Tiles => 1.0,
            Overlay::Mesh => 2.0,
            Overlay::Regions => 3.0,
            Overlay::Stale => 4.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneMapMode {
    /// ACES fit and sRGB encode.
    Aces,
    /// False-colour EV bands of the exposed luminance (debug view, ADR 0038 point 5).
    EvBands,
}

impl ToneMapMode {
    pub fn name(self) -> &'static str {
        match self {
            ToneMapMode::Aces => "aces",
            ToneMapMode::EvBands => "ev-bands",
        }
    }
}

/// The Earth default sun direction of the look (the relief view's sun, the sun of the first lit views): towards +x, +y and +z, normalised.
pub const DEFAULT_SUN: [f32; 3] = [0.501_405_9, 0.601_687_1, 0.621_743_3];

impl LitPlan {
    /// A fixed-exposure lit plan, the golden and perf form (CON-19), with the Earth look defaults.
    pub fn fixed(ev100: f32) -> Self {
        Self::fixed_with(&file::LookParams::default(), ev100)
    }

    /// An adaptive-exposure lit plan with the Earth look defaults (the default range and speeds).
    pub fn adaptive(pre_ev100: f32, dt_s: f32) -> Self {
        Self::adaptive_with(&file::LookParams::default(), pre_ev100, dt_s)
    }

    /// A fixed-exposure lit plan whose units and histogram come from a body's look parameters.
    pub fn fixed_with(look: &file::LookParams, ev100: f32) -> Self {
        Self {
            exposure: ExposureMode::Fixed { ev100 },
            dt_s: 0.0,
            sun_dir: look.sun.direction.map(|v| v as f32),
            reset_exposure: false,
            tonemap: look.tone_curve.mode(),
            units: look.units,
            histogram: look.histogram,
            albedo: look.albedo,
            materials: materials::MaterialParams::EARTH,
            ground_detail: 0.0,
            gloss: materials::GlossParams::EARTH,
            water_level_m: 0.0,
            view: ShadeView::Lit,
            atmosphere: None,
            overlay: Overlay::None,
        }
    }

    /// This plan with the atmosphere of a body: the look's coefficients on a body of `radius_m` (the world definition's radius).
    pub fn with_atmosphere(mut self, look: &file::LookParams, radius_m: f64) -> Result<LitPlan, String> {
        self.atmosphere = Some(AtmospherePlan::new(look.atmosphere, radius_m)?);
        Ok(self)
    }

    /// This plan shaded against the water level `level_m` (metres above the reference surface): the level the water-level field W
    /// gives for the view (ADR 0041 point 1, `planet_generators::water::WaterField::plan_water`; the sea's constant level in M5),
    /// which the beach band of the albedo is measured from (ADR 0039 point 3). The level reaches the shader as an `f32`
    /// (presentation data, ADR 0035: relative rounding 6e-8, 0.6 mm at 10 km); a level of 0, the Earth default, leaves the plan text
    /// and hash as they were.
    pub fn with_water_level(mut self, level_m: f64) -> LitPlan {
        // -0.0 is the default level too: it must not change the plan text.
        self.water_level_m = if level_m == 0.0 { 0.0 } else { level_m as f32 };
        self
    }

    /// This plan with the ground detail at `strength` (0 off, 1 full; LOOK-009).
    pub fn with_ground_detail(mut self, strength: f64) -> LitPlan {
        self.ground_detail = strength;
        self
    }

    /// This plan with a debug overlay drawn over the terrain.
    pub fn with_overlay(mut self, overlay: Overlay) -> LitPlan {
        self.overlay = overlay;
        self
    }

    /// An adaptive-exposure lit plan whose range, speeds, units and histogram come from a body's look parameters.
    pub fn adaptive_with(look: &file::LookParams, pre_ev100: f32, dt_s: f32) -> Self {
        Self { exposure: ExposureMode::Adaptive { params: look.exposure_params(), pre_ev100 }, dt_s, ..Self::fixed_with(look, pre_ev100) }
    }

    /// Stable text for plan snapshots and repro bundles (floats as bit patterns).
    pub fn to_text(&self) -> String {
        let h = |v: f32| format!("{:08x}", v.to_bits());
        let mode = match self.exposure {
            ExposureMode::Fixed { ev100 } => format!("fixed {}", h(ev100)),
            ExposureMode::Adaptive { params: p, pre_ev100 } => {
                format!("adaptive {} {} {} {} {}", h(pre_ev100), h(p.min_ev100), h(p.max_ev100), h(p.speed_up), h(p.speed_down))
            }
        };
        // The look data enter the text only when they differ from the Earth defaults, so a plan made with the defaults keeps the
        // text (and the hash) it had before the look file existed. The base shading's tokens (ADR 0039) follow the same rule: the
        // albedo table, the water level and the debug view are written only when they are not the default.
        let h64 = |v: f64| format!("{:016x}", v.to_bits());
        let mut look = String::new();
        if !(self.units == SceneUnits::EARTH && self.histogram == HistogramParams::EARTH) {
            let g = self.histogram;
            look.push_str(&format!(
                " units {} hist {} {} {} {}",
                h64(self.units.solar_illuminance_lux),
                h64(g.ev_min),
                h64(g.ev_max),
                h64(g.band_low),
                h64(g.band_high)
            ));
        }
        if self.albedo != AlbedoParams::EARTH {
            look.push_str(" albedo");
            for b in self.albedo.bits() {
                look.push_str(&format!(" {b:016x}"));
            }
        }
        if self.water_level_m.to_bits() != 0 {
            look.push_str(&format!(" water {}", h(self.water_level_m)));
        }
        if self.view != ShadeView::Lit {
            look.push_str(&format!(" view {}", self.view.name()));
        }
        if let Some(a) = &self.atmosphere {
            look.push_str(" atmosphere");
            for t in a.tokens() {
                look.push(' ');
                look.push_str(&t);
            }
        }
        if self.overlay != Overlay::None {
            look.push_str(&format!(" overlay {}", self.overlay.name()));
        }
        if self.materials != materials::MaterialParams::EARTH {
            look.push_str(" materials");
            for b in self.materials.bits() {
                look.push_str(&format!(" {b:016x}"));
            }
        }
        if self.ground_detail != 0.0 {
            look.push_str(&format!(" detail {:016x}", self.ground_detail.to_bits()));
        }
        if self.gloss != materials::GlossParams::EARTH {
            look.push_str(" gloss");
            for b in self.gloss.bits() {
                look.push_str(&format!(" {b:016x}"));
            }
        }
        format!(
            "lit {mode} dt {} sun {} {} {} reset {} tonemap {}{look}",
            h(self.dt_s),
            h(self.sun_dir[0]),
            h(self.sun_dir[1]),
            h(self.sun_dir[2]),
            u8::from(self.reset_exposure),
            self.tonemap.name()
        )
    }

    /// Parse the tokens after the leading `lit` of [`to_text`](LitPlan::to_text).
    pub fn from_tokens(t: &[&str]) -> Result<LitPlan, String> {
        let f = |s: &str| u32::from_str_radix(s, 16).map(f32::from_bits).map_err(|e| format!("bad f32 bits '{s}': {e}"));
        let (exposure, rest) = match t {
            ["fixed", ev, rest @ ..] => (ExposureMode::Fixed { ev100: f(ev)? }, rest),
            ["adaptive", pre, lo, hi, up, down, rest @ ..] => (
                ExposureMode::Adaptive {
                    params: ExposureParams { min_ev100: f(lo)?, max_ev100: f(hi)?, speed_up: f(up)?, speed_down: f(down)? },
                    pre_ev100: f(pre)?,
                },
                rest,
            ),
            _ => return Err("lit line needs 'fixed' or 'adaptive'".into()),
        };
        let f64_of = |s: &str| {
            if s.len() != 16 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(format!("bad f64 bits '{s}': 16 lower-case hex digits expected"));
            }
            u64::from_str_radix(s, 16).map(f64::from_bits).map_err(|e| format!("bad f64 bits '{s}': {e}"))
        };
        let usage = "lit line is not 'dt .. sun .. reset .. tonemap ..' (optionally followed by 'units .. hist ..', 'albedo ..', 'water ..', 'view ..', 'atmosphere ..', 'overlay ..', 'materials ..', 'detail ..' and 'gloss ..' in this order)";
        let ["dt", dt, "sun", sx, sy, sz, "reset", reset, "tonemap", tm, tail @ ..] = rest else {
            return Err(usage.into());
        };
        let mut plan = LitPlan {
            exposure,
            dt_s: f(dt)?,
            sun_dir: [f(sx)?, f(sy)?, f(sz)?],
            reset_exposure: *reset == "1",
            tonemap: match *tm {
                "aces" => ToneMapMode::Aces,
                "ev-bands" => ToneMapMode::EvBands,
                other => return Err(format!("unknown tone map '{other}'")),
            },
            units: SceneUnits::EARTH,
            histogram: HistogramParams::EARTH,
            albedo: AlbedoParams::EARTH,
            materials: materials::MaterialParams::EARTH,
            ground_detail: 0.0,
            gloss: materials::GlossParams::EARTH,
            water_level_m: 0.0,
            view: ShadeView::Lit,
            atmosphere: None,
            overlay: Overlay::None,
        };
        // The optional tokens, each at most once and in the order the text writes them.
        let mut rest = tail;
        let mut stage = 0;
        while let [key, more @ ..] = rest {
            let (rank, arity) = match *key {
                "units" => (1, 6),
                "albedo" => (2, 39),
                "water" => (3, 1),
                "view" => (4, 1),
                "atmosphere" => (5, 24),
                "overlay" => (6, 1),
                "materials" => (7, materials::MATERIALS * materials::INPUTS * 2),
                "detail" => (8, 1),
                "gloss" => (9, materials::GLOSS_VALUES),
                _ => return Err(usage.into()),
            };
            if rank <= stage || more.len() < arity {
                return Err(usage.into());
            }
            stage = rank;
            let (args, after) = more.split_at(arity);
            match rank {
                1 => {
                    let ["hist"] = [args[1]] else { return Err(usage.into()) };
                    plan.units = SceneUnits { solar_illuminance_lux: f64_of(args[0])? };
                    plan.histogram = HistogramParams {
                        ev_min: f64_of(args[2])?,
                        ev_max: f64_of(args[3])?,
                        band_low: f64_of(args[4])?,
                        band_high: f64_of(args[5])?,
                    };
                }
                2 => {
                    let mut bits = [0u64; 39];
                    for (b, a) in bits.iter_mut().zip(args) {
                        *b = f64_of(a)?.to_bits();
                    }
                    plan.albedo = AlbedoParams::from_bits(&bits);
                }
                3 => plan.water_level_m = f(args[0])?,
                4 => plan.view = ShadeView::parse(args[0]).ok_or_else(|| format!("unknown lit view '{}'", args[0]))?,
                5 => plan.atmosphere = Some(AtmospherePlan::from_tokens(args)?),
                7 => {
                    let mut bits = [0u64; materials::MATERIALS * materials::INPUTS * 2];
                    for (b, a) in bits.iter_mut().zip(args) {
                        *b = f64_of(a)?.to_bits();
                    }
                    plan.materials = materials::MaterialParams::from_bits(&bits);
                }
                8 => plan.ground_detail = f64_of(args[0])?,
                9 => {
                    let mut bits = [0u64; materials::GLOSS_VALUES];
                    for (b, a) in bits.iter_mut().zip(args) {
                        *b = f64_of(a)?.to_bits();
                    }
                    plan.gloss = materials::GlossParams::from_bits(&bits);
                }
                _ => plan.overlay = Overlay::parse(args[0]).ok_or_else(|| format!("unknown overlay '{}'", args[0]))?,
            }
            rest = after;
        }
        Ok(plan)
    }

    /// Refuse values a renderer cannot use: non-finite or negative time, non-finite EVs, a reversed or non-finite range, negative
    /// speeds, a sun that is not a unit vector. The message names the value (repro bundles can carry any text).
    pub fn validate(&self) -> Result<(), String> {
        let finite = |name: &str, v: f32| if v.is_finite() { Ok(()) } else { Err(format!("lit plan: {name} is {v}, not finite")) };
        finite("dt", self.dt_s)?;
        if self.dt_s < 0.0 {
            return Err(format!("lit plan: dt is {} s, negative", self.dt_s));
        }
        finite("pre-exposure EV100", self.exposure.pre_ev100())?;
        if let ExposureMode::Adaptive { params: p, .. } = self.exposure {
            for (n, v) in [("min EV100", p.min_ev100), ("max EV100", p.max_ev100), ("speed up", p.speed_up), ("speed down", p.speed_down)] {
                finite(n, v)?;
            }
            if p.min_ev100 > p.max_ev100 {
                return Err(format!("lit plan: EV100 range [{}, {}] is reversed", p.min_ev100, p.max_ev100));
            }
            if p.speed_up < 0.0 || p.speed_down < 0.0 {
                return Err(format!("lit plan: adaptation speeds {} and {} must not be negative", p.speed_up, p.speed_down));
            }
            file::validate_exposure(&file::ExposureLook {
                min_ev100: f64::from(p.min_ev100),
                max_ev100: f64::from(p.max_ev100),
                speed_up: f64::from(p.speed_up),
                speed_down: f64::from(p.speed_down),
            })
            .map_err(|e| format!("lit plan: {e}"))?;
        }
        self.albedo.validate().map_err(|(k, v, why)| format!("lit plan: `{k}` = {v}: {why}"))?;
        self.materials.validate().map_err(|(k, v, why)| format!("lit plan: `{k}` = {v}: {why}"))?;
        self.gloss.validate().map_err(|(k, v, why)| format!("lit plan: `{k}` = {v}: {why}"))?;
        if self.ground_detail != 0.0 && !(1.0e-6..=1.0).contains(&self.ground_detail) {
            return Err(format!("lit plan: `ground_detail` = {}: the strength must be 0 or a number from 1e-6 to 1", self.ground_detail));
        }
        if !self.water_level_m.is_finite() {
            return Err(format!("lit plan: water level is {}, not finite", self.water_level_m));
        }
        if let Some(a) = &self.atmosphere {
            atmosphere::Atmosphere::new(a.atmosphere.params, a.atmosphere.radius_m).map_err(|e| format!("lit plan: {e}"))?;
        }
        file::validate_units(&self.units).map_err(|e| format!("lit plan: {e}"))?;
        file::validate_histogram(&self.histogram).map_err(|e| format!("lit plan: {e}"))?;
        let factor = pre_exposure_factor_with(&self.units, f64::from(self.exposure.pre_ev100())) as f32;
        if !factor.is_finite() || factor < f32::MIN_POSITIVE {
            return Err(format!(
                "lit plan: the pre-exposure factor {factor} (EV100 {} with {} lux) does not fit an f32",
                self.exposure.pre_ev100(),
                self.units.solar_illuminance_lux
            ));
        }
        let n = self.sun_dir.map(f64::from);
        let len = libm::sqrt(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]);
        if !len.is_finite() || (len - 1.0).abs() > 1.0e-3 {
            return Err(format!("lit plan: sun direction {:?} has length {len}, not 1", self.sun_dir));
        }
        Ok(())
    }

    /// Hash of the text form (for the plan hash).
    pub fn hash(&self) -> u64 {
        let bytes: Vec<u64> = self.to_text().bytes().map(u64::from).collect();
        hash_u64s(0x006c_6974, &bytes)
    }
}

/// `1 / (1.2 * 2^ev100)`: the factor from luminance in cd/m2 to display-referred linear (1.0 = the saturation luminance).
pub fn exposure_scale(ev100: f64) -> f64 {
    1.0 / (1.2 * libm::exp2(ev100))
}

/// The factor the scene target is written with: scene units to the pre-exposed value, `LUMINANCE_PER_UNIT * exposure_scale`.
/// Computed in f64 here and rounded once to f32 for the shader (CON-03: the CPU decides it).
pub fn pre_exposure_factor(pre_ev100: f64) -> f64 {
    pre_exposure_factor_with(&SceneUnits::EARTH, pre_ev100)
}

/// [`pre_exposure_factor`] under a body's scene-unit scale.
pub fn pre_exposure_factor_with(units: &SceneUnits, pre_ev100: f64) -> f64 {
    units.luminance_per_unit() * exposure_scale(pre_ev100)
}

/// The EV100 that suits a luminance in cd/m2 (and the axis of the histogram): `log2(8 L)`.
pub fn luminance_ev(luminance_cd: f64) -> f64 {
    libm::log2(8.0 * luminance_cd)
}

/// Luminance of a linear RGB triple.
pub fn luminance(rgb: [f64; 3]) -> f64 {
    LUMA[0] * rgb[0] + LUMA[1] * rgb[1] + LUMA[2] * rgb[2]
}

/// Width of one histogram bin in stops.
pub fn bin_width() -> f64 {
    HistogramParams::EARTH.bin_width()
}

/// Centre of bin `i` in EV.
pub fn bin_center_ev(i: usize) -> f64 {
    HistogramParams::EARTH.bin_center_ev(i)
}

/// The bin of a pixel EV, or `None` for a pixel that is not counted (no light: NaN or minus infinity; plus infinity is the last bin). EVs outside the range land in the
/// first or last bin.
pub fn bin_of_ev(ev: f64) -> Option<usize> {
    HistogramParams::EARTH.bin_of_ev(ev)
}

/// The histogram of a scene target image, the CPU reference of the GPU pass. `pixels` are the values as written (pre-exposed,
/// linear RGB per pixel); `pre_ev100` undoes the pre-exposure so the bins are in scene EV. A pixel with no luminance is not
/// counted.
pub fn histogram_of(pixels: &[[f64; 3]], pre_ev100: f64) -> [u32; HIST_BINS] {
    // L_cd = y / exposure_scale(pre); EV = log2(8 L_cd).
    HistogramParams::EARTH.histogram_of(pixels, pre_ev100)
}

/// The mean EV of the pixels between the [`BAND_LOW`] and [`BAND_HIGH`] fractions of the counted ones (bins weighted by how much
/// of their count lies in the band, bin centres as values); `None` when nothing was counted.
pub fn band_mean_ev(hist: &[u32; HIST_BINS]) -> Option<f64> {
    HistogramParams::EARTH.band_mean_ev(hist)
}

/// The EV100 the exposure moves towards: the band mean clamped to the range, or `current` when nothing was counted.
pub fn target_ev(hist: &[u32; HIST_BINS], params: &ExposureParams, current: f64) -> f64 {
    HistogramParams::EARTH.target_ev(hist, params, current)
}

/// One adaptation step in stops: `current` moves towards `target` by at most `speed_up * dt` up or `speed_down * dt` down;
/// with `reset` it snaps. Linear in EV, so a step does not overshoot and the result depends on `dt` only through the cap.
pub fn adapt_ev(current: f64, target: f64, dt_s: f64, params: &ExposureParams, reset: bool) -> f64 {
    if reset {
        return target;
    }
    let up = f64::from(params.speed_up) * dt_s.max(0.0);
    let down = f64::from(params.speed_down) * dt_s.max(0.0);
    current + (target - current).clamp(-down, up)
}

/// The ACES filmic fit (Narkowicz 2015) on exposed linear `x`, clamped to [0, 1].
pub fn aces(x: f64) -> f64 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// The sRGB transfer function (linear to encoded), both in [0, 1].
pub fn srgb_encode(l: f64) -> f64 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        12.92 * l
    } else {
        1.055 * libm::pow(l, 1.0 / 2.4) - 0.055
    }
}

/// The tone-mapped 8-bit colour of a scene-target value: `written` is the pre-exposed linear RGB, `pre_ev100` the EV100 it was
/// written at and `ev100` the exposure now; the exposed value is `written * 2^(pre - ev)`. The CPU reference of the tone-map pass
/// (bound: 1 / 255 per channel, REND-020).
pub fn tonemap_pixel(written: [f64; 3], pre_ev100: f64, ev100: f64) -> [u8; 3] {
    let k = libm::exp2(pre_ev100 - ev100);
    written.map(|v| (srgb_encode(aces(v * k)) * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8)
}

/// Stops per band of the false-colour view and the number of bands.
pub const EV_BAND_STOPS: f64 = 1.0;
pub const EV_BAND_COUNT: usize = 12;

/// Display colours (8-bit, written as they are) of the false-colour luminance view: band 0 (clipped or within one stop of
/// saturation) in red, then warm to cool towards the dark bands; the shader's table is generated from this one.
pub const EV_BAND_COLORS: [[u8; 3]; EV_BAND_COUNT] = [
    [255, 32, 32],
    [255, 128, 32],
    [255, 224, 32],
    [160, 224, 32],
    [32, 192, 64],
    [32, 192, 160],
    [32, 160, 224],
    [32, 96, 224],
    [64, 64, 192],
    [96, 32, 160],
    [64, 16, 96],
    [24, 8, 40],
];

/// The EV offset of the histogram shader: pixel EV = `log2(8 y) + offset` for a value `y` written at `pre_ev100`
/// (`log2(8 y / exposure_scale(pre))`).
pub fn histogram_ev_offset(pre_ev100: f64) -> f64 {
    libm::log2(1.2) + pre_ev100
}

/// The band of the false-colour luminance view: whole stops of the exposed luminance below the saturation value 1.0, band 0 being
/// `[0.5, 1]` and clipped values (above 1) in band 0 as well; `None` for black.
pub fn ev_band(exposed_luminance: f64) -> Option<usize> {
    if exposed_luminance <= 0.0 || !exposed_luminance.is_finite() {
        return None;
    }
    let stops_below = -libm::log2(exposed_luminance) / EV_BAND_STOPS;
    Some((libm::floor(stops_below).max(0.0) as usize).min(EV_BAND_COUNT - 1))
}

/// What the scene target holds for a radiance: pre-exposed and clamped to [`HDR_CLAMP`] (the CPU reference of every lit write).
pub fn written_value(radiance: [f64; 3], pre_ev100: f64) -> [f64; 3] {
    written_value_with(&SceneUnits::EARTH, radiance, pre_ev100)
}

/// [`written_value`] under a body's scene-unit scale.
pub fn written_value_with(units: &SceneUnits, radiance: [f64; 3], pre_ev100: f64) -> [f64; 3] {
    let f = pre_exposure_factor_with(units, pre_ev100);
    radiance.map(|v| (v * f).clamp(0.0, f64::from(HDR_CLAMP)))
}
