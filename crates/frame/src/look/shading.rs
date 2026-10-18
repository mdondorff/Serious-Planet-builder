//! Base shading of the lit terrain (ADR 0039 points 1, 3 and 4): the albedo of the surface from the climate field, the slope and the
//! height above the water level, Lambert diffuse under the plan's sun, the sun's direction from a time of day, and the debug views
//! (albedo only, lighting only, the climate channels). The f64 reference of every function the terrain shader evaluates lives here
//! (CON-02, CON-03): the renderer executes, the tests compare.
//!
//! Class (ADR 0035): **presentation data**. The climate field it reads is a field (GEN-021) but nothing here is stored, keyed or read
//! back. Declared bound (REND-026): the GPU albedo and radiance match this module's f64 evaluation of the same inputs within
//! `max(2e-4 |v|, 2e-5)` per channel before the f16 store, and an input that is a planet-sized value is formed from tile-local
//! quantities only (the up vector is `normalize(up0 + local / R)`, [`up_at`]), so the result does not depend on the camera.
//!
//! Colours are linear albedos from published measurement ranges for natural surfaces (Oke, *Boundary Layer Climates*, 2nd ed. 1987,
//! tables of surface albedo; the NASA Earth Observatory albedo summaries): fresh snow 0.80 to 0.90, desert sand 0.30 to 0.45, bare
//! rock 0.15 to 0.30, grassland 0.15 to 0.25, coniferous forest 0.08 to 0.15, tundra 0.15 to 0.25. The thresholds (where the
//! vegetation begins, where snow lies) are artistic parameters of the look, reviewed at the M5 gate (ADR 0037); they are data.

/// The scale a colour channel or threshold may take: albedos are in `[0, 1]`.
pub const ALBEDO_LIMITS: (f64, f64) = (0.0, 1.0);

/// Number of `vec4<f32>` words the albedo table occupies in the globals.
pub const ALBEDO_VEC4S: usize = 10;

/// The albedo table of a body (look data, ADR 0039 point 3, ADR 0042 point 6). Colours are linear RGB albedos; the thresholds
/// are the places where the mixes between them begin and end (`lo < hi` for each pair, so every smoothstep has an interval).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlbedoParams {
    pub rock: [f64; 3],
    pub snow: [f64; 3],
    pub grass: [f64; 3],
    pub forest: [f64; 3],
    pub desert: [f64; 3],
    pub tundra: [f64; 3],
    pub beach: [f64; 3],
    pub seabed: [f64; 3],
    /// Precipitation (mm per year) below which the ground is desert and above which it is vegetated (`dry < wet`).
    pub dry_mm: f64,
    pub wet_mm: f64,
    /// Precipitation between which grass turns to forest.
    pub forest_mm_lo: f64,
    pub forest_mm_hi: f64,
    /// Temperature (degrees C) between which tundra turns to vegetation.
    pub tundra_c_lo: f64,
    pub tundra_c_hi: f64,
    /// `cos` of the slope angle (the angle between the surface normal and up) at and below which the ground is bare rock, and
    /// above which it carries soil and vegetation (`rock_cos_full < rock_cos_none`).
    pub rock_cos_full: f64,
    pub rock_cos_none: f64,
    /// Temperature at and below which snow lies fully, and at and above which none lies (`snow_c_full < snow_c_none`).
    pub snow_c_full: f64,
    pub snow_c_none: f64,
    /// `cos` of the slope below which snow does not stay and above which it lies fully (`snow_cos_none < snow_cos_full`).
    pub snow_cos_none: f64,
    pub snow_cos_full: f64,
    /// Height above the water level up to which the shore is a beach, and the further height over which it fades to the ground.
    pub beach_height_m: f64,
    pub beach_fade_m: f64,
    /// Width of the transition from the seabed to the shore at the water level.
    pub shore_blend_m: f64,
}

impl AlbedoParams {
    /// The Earth default: the measured ranges above.
    pub const EARTH: AlbedoParams = AlbedoParams {
        rock: [0.30, 0.27, 0.24],
        snow: [0.85, 0.87, 0.90],
        grass: [0.14, 0.19, 0.06],
        forest: [0.045, 0.075, 0.03],
        desert: [0.42, 0.33, 0.22],
        tundra: [0.20, 0.19, 0.15],
        beach: [0.52, 0.46, 0.34],
        seabed: [0.06, 0.08, 0.09],
        dry_mm: 250.0,
        wet_mm: 900.0,
        forest_mm_lo: 1000.0,
        forest_mm_hi: 2200.0,
        tundra_c_lo: -3.0,
        tundra_c_hi: 5.0,
        rock_cos_full: 0.62,
        rock_cos_none: 0.86,
        snow_c_full: -8.0,
        snow_c_none: -1.0,
        snow_cos_none: 0.55,
        snow_cos_full: 0.80,
        beach_height_m: 3.0,
        beach_fade_m: 5.0,
        shore_blend_m: 1.0,
    };

    /// The table as the globals hold it (`ALBEDO_VEC4S` vec4 of f32, each value rounded once): eight colours with one threshold in
    /// the fourth lane each (`dry`, `wet`, `forest_lo`, `forest_hi`, `tundra_lo`, `tundra_hi`, `rock_full`, `rock_none`), then
    /// `(snow_c_full, snow_c_none, snow_cos_none, snow_cos_full)` and `(beach_height, beach_fade, shore_blend, 0)`.
    pub fn to_words(&self) -> [f32; ALBEDO_VEC4S * 4] {
        let c = [self.rock, self.snow, self.grass, self.forest, self.desert, self.tundra, self.beach, self.seabed];
        let s = [
            self.dry_mm,
            self.wet_mm,
            self.forest_mm_lo,
            self.forest_mm_hi,
            self.tundra_c_lo,
            self.tundra_c_hi,
            self.rock_cos_full,
            self.rock_cos_none,
        ];
        let mut w = [0f32; ALBEDO_VEC4S * 4];
        for k in 0..8 {
            w[4 * k..4 * k + 3].copy_from_slice(&c[k].map(|v| v as f32));
            w[4 * k + 3] = s[k] as f32;
        }
        w[32..36].copy_from_slice(&[self.snow_c_full, self.snow_c_none, self.snow_cos_none, self.snow_cos_full].map(|v| v as f32));
        w[36..40].copy_from_slice(&[self.beach_height_m, self.beach_fade_m, self.shore_blend_m, 0.0].map(|v| v as f32));
        w
    }

    /// The same values rounded once to `f32` and read back as `f64`: what the shader sees. The reference evaluated on this is the
    /// shader's function up to arithmetic rounding.
    pub fn as_f32(&self) -> AlbedoParams {
        let w = self.to_words();
        let c = |k: usize| [f64::from(w[4 * k]), f64::from(w[4 * k + 1]), f64::from(w[4 * k + 2])];
        let s = |k: usize| f64::from(w[4 * k + 3]);
        AlbedoParams {
            rock: c(0),
            snow: c(1),
            grass: c(2),
            forest: c(3),
            desert: c(4),
            tundra: c(5),
            beach: c(6),
            seabed: c(7),
            dry_mm: s(0),
            wet_mm: s(1),
            forest_mm_lo: s(2),
            forest_mm_hi: s(3),
            tundra_c_lo: s(4),
            tundra_c_hi: s(5),
            rock_cos_full: s(6),
            rock_cos_none: s(7),
            snow_c_full: f64::from(w[32]),
            snow_c_none: f64::from(w[33]),
            snow_cos_none: f64::from(w[34]),
            snow_cos_full: f64::from(w[35]),
            beach_height_m: f64::from(w[36]),
            beach_fade_m: f64::from(w[37]),
            shore_blend_m: f64::from(w[38]),
        }
    }

    /// Bit patterns of every value (the plan text form): 8 x 3 colours, then the 15 thresholds in field order.
    pub fn bits(&self) -> [u64; 39] {
        let mut out = [0u64; 39];
        let mut k = 0;
        for c in [self.rock, self.snow, self.grass, self.forest, self.desert, self.tundra, self.beach, self.seabed] {
            for v in c {
                out[k] = v.to_bits();
                k += 1;
            }
        }
        for v in self.scalars() {
            out[k] = v.to_bits();
            k += 1;
        }
        out
    }

    /// Inverse of [`bits`](AlbedoParams::bits).
    pub fn from_bits(b: &[u64; 39]) -> AlbedoParams {
        let f = |k: usize| f64::from_bits(b[k]);
        let c = |i: usize| [f(3 * i), f(3 * i + 1), f(3 * i + 2)];
        AlbedoParams {
            rock: c(0),
            snow: c(1),
            grass: c(2),
            forest: c(3),
            desert: c(4),
            tundra: c(5),
            beach: c(6),
            seabed: c(7),
            dry_mm: f(24),
            wet_mm: f(25),
            forest_mm_lo: f(26),
            forest_mm_hi: f(27),
            tundra_c_lo: f(28),
            tundra_c_hi: f(29),
            rock_cos_full: f(30),
            rock_cos_none: f(31),
            snow_c_full: f(32),
            snow_c_none: f(33),
            snow_cos_none: f(34),
            snow_cos_full: f(35),
            beach_height_m: f(36),
            beach_fade_m: f(37),
            shore_blend_m: f(38),
        }
    }

    fn scalars(&self) -> [f64; 15] {
        [
            self.dry_mm,
            self.wet_mm,
            self.forest_mm_lo,
            self.forest_mm_hi,
            self.tundra_c_lo,
            self.tundra_c_hi,
            self.rock_cos_full,
            self.rock_cos_none,
            self.snow_c_full,
            self.snow_c_none,
            self.snow_cos_none,
            self.snow_cos_full,
            self.beach_height_m,
            self.beach_fade_m,
            self.shore_blend_m,
        ]
    }

    /// The reference albedo (linear RGB) of a surface point (ADR 0039 point 3): `temperature_c` and `precipitation_mm` from the
    /// climate field, `cos_slope` the cosine of the angle between the unit normal and up (clamped to `[0, 1]`), and
    /// `height_above_water_m` the height above the water level W (never a hard-coded sea level, ADR 0041). Ground from the climate
    /// (desert, grass, forest, tundra), bare rock on steep ground, snow by temperature and slope, a beach band just above the water,
    /// the seabed below it.
    pub fn albedo(&self, temperature_c: f64, precipitation_mm: f64, cos_slope: f64, height_above_water_m: f64) -> [f64; 3] {
        self.albedo_with_ramp(temperature_c, precipitation_mm, cos_slope, height_above_water_m, 0.0)
    }

    /// The width of the seabed-to-shore ramp at a node whose mesh cannot hold less than `ramp_min_m` of height (REND-030): the look's
    /// `shore_blend_m`, or `ramp_min_m` where that is larger. Monotone in `ramp_min_m`, equal to `shore_blend_m` where it is small.
    pub fn shore_ramp_m(&self, ramp_min_m: f64) -> f64 {
        self.shore_blend_m.max(if ramp_min_m.is_finite() { ramp_min_m } else { 0.0 })
    }

    /// [`albedo`](AlbedoParams::albedo) with the node's least ramp height (`TerrainDraw::shore_ramp_min_m`): the CPU reference of the
    /// shader's `albedo_of`. With `ramp_min_m` 0 it is `albedo`.
    pub fn albedo_with_ramp(
        &self,
        temperature_c: f64,
        precipitation_mm: f64,
        cos_slope: f64,
        height_above_water_m: f64,
        ramp_min_m: f64,
    ) -> [f64; 3] {
        let cos = cos_slope.clamp(0.0, 1.0);
        let wet = smoothstep(self.dry_mm, self.wet_mm, precipitation_mm);
        let forest = smoothstep(self.forest_mm_lo, self.forest_mm_hi, precipitation_mm);
        let vegetation = mix3(self.grass, self.forest, forest);
        let ground = mix3(self.desert, vegetation, wet);
        let warm = smoothstep(self.tundra_c_lo, self.tundra_c_hi, temperature_c);
        let ground = mix3(self.tundra, ground, warm);
        let rock_w = 1.0 - smoothstep(self.rock_cos_full, self.rock_cos_none, cos);
        let base = mix3(ground, self.rock, rock_w);
        let cold = 1.0 - smoothstep(self.snow_c_full, self.snow_c_none, temperature_c);
        let sticks = smoothstep(self.snow_cos_none, self.snow_cos_full, cos);
        let snow_w = cold * sticks;
        let land = mix3(base, self.snow, snow_w);
        let beach_w =
            (1.0 - smoothstep(self.beach_height_m, self.beach_height_m + self.beach_fade_m, height_above_water_m)) * (1.0 - snow_w);
        let shore = mix3(land, self.beach, beach_w);
        let land_w = smoothstep(-self.shore_ramp_m(ramp_min_m), 0.0, height_above_water_m);
        mix3(self.seabed, shore, land_w)
    }

    /// Check every value against its range (colours and the unit-interval thresholds in `[0, 1]`, ordered pairs ordered, heights
    /// positive); the first violation names the key (relative to the body, `albedo.<key>`) and the value.
    pub fn validate(&self) -> Result<(), (String, String, String)> {
        let finite = |key: &str, v: f64| -> Result<(), (String, String, String)> {
            if v.is_finite() {
                Ok(())
            } else {
                Err((format!("albedo.{key}"), format!("{v:?}"), "must be a finite number".into()))
            }
        };
        for (name, c) in [
            ("rock", self.rock),
            ("snow", self.snow),
            ("grass", self.grass),
            ("forest", self.forest),
            ("desert", self.desert),
            ("tundra", self.tundra),
            ("beach", self.beach),
            ("seabed", self.seabed),
        ] {
            for v in c {
                finite(name, v)?;
                if !(ALBEDO_LIMITS.0..=ALBEDO_LIMITS.1).contains(&v) {
                    return Err((
                        format!("albedo.{name}"),
                        format!("{c:?}"),
                        "every channel of an albedo must be between 0.0 and 1.0".into(),
                    ));
                }
            }
        }
        for (key, v) in [
            ("dry_mm", self.dry_mm),
            ("wet_mm", self.wet_mm),
            ("forest_mm_lo", self.forest_mm_lo),
            ("forest_mm_hi", self.forest_mm_hi),
            ("tundra_c_lo", self.tundra_c_lo),
            ("tundra_c_hi", self.tundra_c_hi),
            ("rock_cos_full", self.rock_cos_full),
            ("rock_cos_none", self.rock_cos_none),
            ("snow_c_full", self.snow_c_full),
            ("snow_c_none", self.snow_c_none),
            ("snow_cos_none", self.snow_cos_none),
            ("snow_cos_full", self.snow_cos_full),
            ("beach_height_m", self.beach_height_m),
            ("beach_fade_m", self.beach_fade_m),
            ("shore_blend_m", self.shore_blend_m),
        ] {
            finite(key, v)?;
        }
        let order = |lo_key: &str, lo: f64, hi_key: &str, hi: f64| -> Result<(), (String, String, String)> {
            if lo < hi {
                Ok(())
            } else {
                Err((
                    format!("albedo.{lo_key}"),
                    format!("{lo:?}"),
                    format!("must be below {hi_key} ({hi:?}), so the transition has an interval"),
                ))
            }
        };
        order("dry_mm", self.dry_mm, "wet_mm", self.wet_mm)?;
        order("forest_mm_lo", self.forest_mm_lo, "forest_mm_hi", self.forest_mm_hi)?;
        order("tundra_c_lo", self.tundra_c_lo, "tundra_c_hi", self.tundra_c_hi)?;
        order("rock_cos_full", self.rock_cos_full, "rock_cos_none", self.rock_cos_none)?;
        order("snow_c_full", self.snow_c_full, "snow_c_none", self.snow_c_none)?;
        order("snow_cos_none", self.snow_cos_none, "snow_cos_full", self.snow_cos_full)?;
        let in_range = |key: &str, v: f64, lo: f64, hi: f64| -> Result<(), (String, String, String)> {
            if (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err((format!("albedo.{key}"), format!("{v:?}"), format!("must be between {lo:?} and {hi:?}")))
            }
        };
        in_range("dry_mm", self.dry_mm, 0.0, 20_000.0)?;
        in_range("wet_mm", self.wet_mm, 0.0, 20_000.0)?;
        in_range("forest_mm_lo", self.forest_mm_lo, 0.0, 20_000.0)?;
        in_range("forest_mm_hi", self.forest_mm_hi, 0.0, 20_000.0)?;
        in_range("tundra_c_lo", self.tundra_c_lo, -100.0, 100.0)?;
        in_range("tundra_c_hi", self.tundra_c_hi, -100.0, 100.0)?;
        in_range("snow_c_full", self.snow_c_full, -100.0, 100.0)?;
        in_range("snow_c_none", self.snow_c_none, -100.0, 100.0)?;
        in_range("rock_cos_full", self.rock_cos_full, 0.0, 1.0)?;
        in_range("rock_cos_none", self.rock_cos_none, 0.0, 1.0)?;
        in_range("snow_cos_none", self.snow_cos_none, 0.0, 1.0)?;
        in_range("snow_cos_full", self.snow_cos_full, 0.0, 1.0)?;
        in_range("beach_height_m", self.beach_height_m, 0.0, 1000.0)?;
        // A zero fade would make the beach edge a division by zero in the shader.
        in_range("beach_fade_m", self.beach_fade_m, 1.0e-3, 1000.0)?;
        // A zero width would divide by zero in the smoothstep of the shore; the blend needs an interval.
        in_range("shore_blend_m", self.shore_blend_m, 1.0e-3, 1000.0)?;
        // The shader sees the thresholds rounded to f32: a pair that is ordered in f64 but collapses when rounded would make an
        // undefined smoothstep, so the order is checked on the rounded values too.
        let f = self.as_f32();
        order("dry_mm", f.dry_mm, "wet_mm", f.wet_mm)?;
        order("forest_mm_lo", f.forest_mm_lo, "forest_mm_hi", f.forest_mm_hi)?;
        order("tundra_c_lo", f.tundra_c_lo, "tundra_c_hi", f.tundra_c_hi)?;
        order("rock_cos_full", f.rock_cos_full, "rock_cos_none", f.rock_cos_none)?;
        order("snow_c_full", f.snow_c_full, "snow_c_none", f.snow_c_none)?;
        order("snow_cos_none", f.snow_cos_none, "snow_cos_full", f.snow_cos_full)?;
        Ok(())
    }
}

impl Default for AlbedoParams {
    fn default() -> Self {
        Self::EARTH
    }
}

/// `smoothstep(e0, e1, x)` as WGSL defines it for `e0 < e1` (the validated tables always have an interval).
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t, a[2] * (1.0 - t) + b[2] * t]
}

/// The direction of up at a point `local` metres from a tile origin whose unit up vector is `up0` and whose distance from the planet
/// centre is `1 / inv_r`: `normalize(up0 + local * inv_r)`. Exactly the radial direction of the point, and formed from the tile-local
/// `local` and the small number `inv_r` only, never a planet-sized value (CON-11): the shader's slope input.
pub fn up_at(up0: [f64; 3], inv_r: f64, local: [f64; 3]) -> [f64; 3] {
    let v = [up0[0] + local[0] * inv_r, up0[1] + local[1] * inv_r, up0[2] + local[2] * inv_r];
    let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    v.map(|c| c / l)
}

/// Which image the lit terrain shows (debug views, CON-18): the lit surface, the albedo alone, the lighting alone (a white surface),
/// or a climate channel in false colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShadeView {
    #[default]
    Lit,
    Albedo,
    Lighting,
    Temperature,
    Precipitation,
}

impl ShadeView {
    pub const ALL: [ShadeView; 5] =
        [ShadeView::Lit, ShadeView::Albedo, ShadeView::Lighting, ShadeView::Temperature, ShadeView::Precipitation];

    pub fn name(self) -> &'static str {
        match self {
            ShadeView::Lit => "lit",
            ShadeView::Albedo => "albedo",
            ShadeView::Lighting => "lighting",
            ShadeView::Temperature => "temperature",
            ShadeView::Precipitation => "precipitation",
        }
    }

    pub fn parse(s: &str) -> Option<ShadeView> {
        ShadeView::ALL.into_iter().find(|v| v.name() == s)
    }

    /// The index the shader receives.
    pub fn index(self) -> u32 {
        ShadeView::ALL.iter().position(|v| *v == self).expect("known view") as u32
    }
}

/// Temperature range of the false-colour view (degrees C) and precipitation range (mm per year).
pub const TEMPERATURE_VIEW_RANGE_C: (f64, f64) = (-30.0, 40.0);
pub const PRECIPITATION_VIEW_RANGE_MM: (f64, f64) = (0.0, 3000.0);

/// The false colour of a climate channel value: blue to red over the temperature range, white to dark blue over the precipitation
/// range; the view writes this as if it were the radiance of a surface facing the sun (`/ pi`).
pub fn climate_view_colour(view: ShadeView, temperature_c: f64, precipitation_mm: f64) -> [f64; 3] {
    match view {
        ShadeView::Temperature => {
            let t =
                ((temperature_c - TEMPERATURE_VIEW_RANGE_C.0) / (TEMPERATURE_VIEW_RANGE_C.1 - TEMPERATURE_VIEW_RANGE_C.0)).clamp(0.0, 1.0);
            mix3(mix3([0.05, 0.15, 0.85], [0.95, 0.95, 0.85], smoothstep(0.0, 0.5, t)), [0.90, 0.10, 0.05], smoothstep(0.5, 1.0, t))
        }
        ShadeView::Precipitation => {
            let t = ((precipitation_mm - PRECIPITATION_VIEW_RANGE_MM.0) / (PRECIPITATION_VIEW_RANGE_MM.1 - PRECIPITATION_VIEW_RANGE_MM.0))
                .clamp(0.0, 1.0);
            mix3([0.95, 0.92, 0.80], [0.05, 0.15, 0.55], t)
        }
        _ => [0.0; 3],
    }
}

/// The radiance (scene units, before the pre-exposure) of the lit terrain at a surface point, in the view `view`: the CPU reference of
/// the terrain shader's `fs_lit` (ADR 0039 point 4). Lambert diffuse `albedo * max(n . sun, 0) / pi` for the sun's illuminance of one
/// unit; no ambient, no shadows, no specular (the sky's light and the atmosphere's transmittance come with ADR 0040).
#[allow(clippy::too_many_arguments)]
pub fn terrain_radiance(
    params: &AlbedoParams,
    view: ShadeView,
    normal: [f64; 3],
    up: [f64; 3],
    temperature_c: f64,
    precipitation_mm: f64,
    height_above_water_m: f64,
    sun: [f64; 3],
) -> [f64; 3] {
    terrain_radiance_with_ramp(params, view, normal, up, temperature_c, precipitation_mm, height_above_water_m, 0.0, sun)
}

/// [`terrain_radiance`] at a node with the least ramp height `ramp_min_m` (REND-030).
#[allow(clippy::too_many_arguments)]
pub fn terrain_radiance_with_ramp(
    params: &AlbedoParams,
    view: ShadeView,
    normal: [f64; 3],
    up: [f64; 3],
    temperature_c: f64,
    precipitation_mm: f64,
    height_above_water_m: f64,
    ramp_min_m: f64,
    sun: [f64; 3],
) -> [f64; 3] {
    let n = normalize(normal);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cos_slope = dot(n, up).clamp(0.0, 1.0);
    let ndl = dot(n, sun).max(0.0);
    let pi = core::f64::consts::PI;
    match view {
        ShadeView::Lit => {
            let a = params.albedo_with_ramp(temperature_c, precipitation_mm, cos_slope, height_above_water_m, ramp_min_m);
            a.map(|c| c * ndl / pi)
        }
        ShadeView::Albedo => {
            params.albedo_with_ramp(temperature_c, precipitation_mm, cos_slope, height_above_water_m, ramp_min_m).map(|c| c / pi)
        }
        ShadeView::Lighting => [ndl / pi; 3],
        ShadeView::Temperature | ShadeView::Precipitation => climate_view_colour(view, temperature_c, precipitation_mm).map(|c| c / pi),
    }
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let l = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    v.map(|c| c / l)
}

/// The unit direction towards the sun in the planet-fixed axes for a time of day and a declination (ADR 0039 point 1: the plan's
/// sun comes from a time-of-day value the view sets). The axes are COORD's: +z is the north pole, +x is latitude 0 and longitude 0,
/// +y is longitude 90 degrees east. The sub-solar point is at latitude `declination` and at longitude 0 at 12:00, moving westward by
/// 15 degrees per hour: `(cos d cos l, cos d sin l, sin d)` with `l = -15 degrees (hours - 12)`. Computed in f64 with `libm` and
/// rounded once to f32 (the view decides, the renderer executes).
pub fn sun_direction(time_of_day_hours: f64, declination_deg: f64) -> [f32; 3] {
    let rad = core::f64::consts::PI / 180.0;
    let lon = -15.0 * (time_of_day_hours - 12.0) * rad;
    let dec = declination_deg * rad;
    let c = libm::cos(dec);
    [(c * libm::cos(lon)) as f32, (c * libm::sin(lon)) as f32, libm::sin(dec) as f32]
}
