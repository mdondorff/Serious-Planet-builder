//! The atmosphere's passes (ADR 0040, ADR 0042 point 2, M5 item 5): the static tables uploaded when the body's tables change, the
//! sky-view table and the aerial-perspective volume (compute, once per frame), and the composite that applies them over the scene
//! target (a fullscreen pass: the sky for pixels without geometry, the aerial perspective for the rest; from outside the atmosphere
//! the per-pixel orbit march). The renderer executes the plan and decides nothing (CON-03): the tables, the frame geometry and the
//! parameters come from `planet_frame::look::atmosphere` (f64 on the CPU, rounded once); the WGSL here mirrors that module's
//! reference line by line in f32.
//!
//! Class and bounds (ADR 0035, ADR 0043; ATMO-001, ATMO-010): presentation data. The sky-view table, the aerial-perspective volume,
//! the orbit march and the terrain's atmosphere lighting match the CPU reference, evaluated on the same f32-rounded inputs with the
//! same mapping, step counts and placement, within `1 % * |ref| + 1e-4` per channel (tier B), including a camera at 1 m altitude
//! walking in millimetre steps (temporal stability) and the planet's limb seen from 20,000 km. The planet's radius reaches these
//! shaders only as a value in the uniform block (decided in f64 on the CPU) and every ray quantity is formed from the
//! altitude-relative primitives (CON-11 v1.4, ADR 0040 point 5); no shader forms `r^2 - R^2`, `r - R` or a ray-sphere root as a
//! difference of planet-sized numbers.

use crate::GpuContext;
use planet_frame::look::atmosphere::{
    Atmosphere, AtmosphereView, FrameGeometry, StaticLuts, AP_D, AP_H, AP_W, MS_H, MS_W, SKY_H, SKY_W, TRANSMITTANCE_H, TRANSMITTANCE_W,
};

/// Format of the static tables (optical depth and the multiple-scattering factor): exact enough for the 1 % bound and read with
/// `textureLoad` and a manual lerp, which needs no filtering.
const LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// Format of the per-frame products.
const PRODUCT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Bytes of the uniform block: 16 `vec4<f32>` (6 of the body, 10 of the frame).
pub const UNIFORM_BYTES: u64 = 256;

/// The shared declarations and the table lookups the terrain shader also needs. The consumer declares `A: AtmoBlock`, `tr_lut` and
/// `ms_lut` before this text.
const DECLS: &str = r#"
const ATM_PI: f32 = 3.14159265358979;
const TR_W: i32 = 256;
const TR_H: i32 = 64;
const MS_W: i32 = 32;
const MS_H: i32 = 32;
const SKY_W: i32 = 192;
const SKY_H: i32 = 108;
const AP_W: i32 = 32;
const AP_H: i32 = 32;
const AP_D: i32 = 32;
const SKY_STEPS: i32 = 32;
const ORBIT_STEPS: i32 = 40;
const AP_MAX_STEPS: i32 = 64;

// Atmosphere::static_words (6 vec4) then FrameGeometry::words (10 vec4).
struct AtmoBlock {
    rayleigh: vec4<f32>,   // xyz Rayleigh scattering per metre, w scale height
    mie_sc: vec4<f32>,     // xyz Mie scattering, w scale height
    mie_abs: vec4<f32>,    // xyz Mie absorption, w asymmetry g
    ozone: vec4<f32>,      // xyz ozone absorption, w peak altitude
    ground: vec4<f32>,     // xyz ground albedo, w ozone half width
    body: vec4<f32>,       // x body radius, y top height, z horizon distance at the top, w sun disc size
    cam: vec4<f32>,        // x h, y r, z c, w c_top
    cam2: vec4<f32>,       // x sqrt(c), y horizon cosine, z horizon angle, w angle from there to the nadir
    up: vec4<f32>,         // xyz up at the camera, w cosine of the sun to it
    sun: vec4<f32>,        // xyz sun direction, w sine of its angle to the up
    e1: vec4<f32>,         // xyz horizontal axis towards the sun
    e2: vec4<f32>,         // xyz horizontal axis across
    right: vec4<f32>,      // xyz camera right times tan(fov_x / 2)
    upv: vec4<f32>,        // xyz camera up times tan(fov_y / 2)
    fwd: vec4<f32>,        // xyz camera forward, w near plane
    misc: vec4<f32>,       // x pre-exposure factor, y inside the atmosphere, z aerial-perspective range, w debug view (bits)
};

fn atm_smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Manual bilinear interpolation of a table whose texel i sits at i / (N - 1).
fn atm_bilerp(t: texture_2d<f32>, w: i32, h: i32, p: vec2<f32>) -> vec3<f32> {
    let q = clamp(p, vec2<f32>(0.0), vec2<f32>(f32(w - 1), f32(h - 1)));
    let x0 = min(i32(floor(q.x)), w - 2);
    let y0 = min(i32(floor(q.y)), h - 2);
    let f = q - vec2<f32>(f32(x0), f32(y0));
    let a = textureLoad(t, vec2<i32>(x0, y0), 0).xyz;
    let b = textureLoad(t, vec2<i32>(x0 + 1, y0), 0).xyz;
    let c = textureLoad(t, vec2<i32>(x0, y0 + 1), 0).xyz;
    let d = textureLoad(t, vec2<i32>(x0 + 1, y0 + 1), 0).xyz;
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// Distance from a point inside the atmosphere to its top along a ray with r mu = rmu (the altitude-relative form).
fn atm_dist_to_top(rmu: f32, c_top: f32) -> f32 {
    let sq = sqrt(max(rmu * rmu - c_top, 0.0));
    if (rmu >= 0.0) {
        let den = rmu + sq;
        return select(0.0, -c_top / den, den > 0.0);
    }
    return -rmu + sq;
}

fn atm_planet_shadow(h0: f32, mu_sun: f32) -> f32 {
    let h = clamp(h0, 0.0, A.body.y);
    let c = h * (2.0 * A.body.x + h);
    let mu_h = -sqrt(c) / (A.body.x + h);
    // The penumbra: the sun's angular radius in cosine terms at the horizon of a point at radius r is w R / r.
    let w = A.body.w * A.body.x / (A.body.x + h);
    return atm_smooth(-w, w, mu_sun - mu_h);
}

fn atm_tr_coords(h0: f32, mu0: f32) -> vec2<f32> {
    let top = A.body.y;
    let h = clamp(h0, 0.0, top);
    let r = A.body.x + h;
    let rho = sqrt(h * (2.0 * A.body.x + h));
    let c_top = (h - top) * (2.0 * A.body.x + h + top);
    let d = atm_dist_to_top(r * clamp(mu0, -1.0, 1.0), c_top);
    let d_min = top - h;
    let d_max = rho + A.body.z;
    return vec2<f32>(clamp((d - d_min) / (d_max - d_min), 0.0, 1.0), clamp(rho / A.body.z, 0.0, 1.0));
}

fn atm_optical_depth(h: f32, mu: f32) -> vec3<f32> {
    let x = atm_tr_coords(h, mu);
    return atm_bilerp(tr_lut, TR_W, TR_H, vec2<f32>(x.x * f32(TR_W - 1), x.y * f32(TR_H - 1)));
}

fn atm_transmittance(h: f32, mu: f32) -> vec3<f32> {
    return exp(-atm_optical_depth(h, mu));
}

fn atm_multiscatter(h: f32, mu_sun: f32) -> vec3<f32> {
    let x = (clamp(mu_sun, -1.0, 1.0) + 1.0) * 0.5;
    let y = clamp(h / A.body.y, 0.0, 1.0);
    return atm_bilerp(ms_lut, MS_W, MS_H, vec2<f32>(x * f32(MS_W - 1), y * f32(MS_H - 1)));
}
"#;

/// The ray march and the per-frame products, on top of [`DECLS`].
const MARCH: &str = r#"
// Distance to the ground along a downward ray, or -1 when it passes above it.
fn atm_dist_to_ground(rmu: f32, c: f32) -> f32 {
    if (rmu >= 0.0) { return -1.0; }
    let d = rmu * rmu - c;
    if (d < 0.0) { return -1.0; }
    return c / (-rmu + sqrt(d));
}

// The start of a ray: altitude, r, c = r^2 - R^2, r mu and r mu_sun. A sample at s has q = s (s + 2 r mu), r_s^2 = r^2 + q and
// h_s = (c + q) / (r_s + R).
struct AtmoOrigin { h: f32, r: f32, c: f32, rmu: f32, r_musun: f32 };

fn atm_sample_at(o: AtmoOrigin, s: f32) -> vec2<f32> {
    let q = s * (s + 2.0 * o.rmu);
    let r_s = sqrt(max(o.r * o.r + q, 0.0));
    return vec2<f32>(max((o.c + q) / (r_s + A.body.x), 0.0), r_s);
}

struct AtmoMedium { rayleigh: vec3<f32>, mie: vec3<f32>, scattering: vec3<f32>, extinction: vec3<f32> };

fn atm_medium_at(h0: f32) -> AtmoMedium {
    let h = max(h0, 0.0);
    let dm = exp(-h / A.mie_sc.w);
    let dz = max(1.0 - abs(h - A.ozone.w) / A.ground.w, 0.0);
    var m: AtmoMedium;
    m.rayleigh = A.rayleigh.xyz * exp(-h / A.rayleigh.w);
    m.mie = A.mie_sc.xyz * dm;
    m.scattering = m.rayleigh + m.mie;
    m.extinction = m.scattering + A.mie_abs.xyz * dm + A.ozone.xyz * dz;
    return m;
}

fn atm_rayleigh_phase(c: f32) -> f32 { return 3.0 / (16.0 * ATM_PI) * (1.0 + c * c); }

fn atm_cs_phase(g: f32, c: f32) -> f32 {
    let k = 3.0 / (8.0 * ATM_PI) * (1.0 - g * g) / (2.0 + g * g);
    let d = 1.0 + g * g - 2.0 * g * c;
    return k * (1.0 + c * c) / (d * sqrt(d));
}

// The start of step i of n: lay 0 uniform, 1 quadratic.
fn atm_step_at(lay: i32, s_end: f32, i: i32, n: i32) -> f32 {
    let x = f32(i) / f32(n);
    return select(s_end * f32(i) / f32(n), s_end * x * x, lay == 1);
}

struct AtmoMarch { radiance: vec3<f32>, transmittance: vec3<f32> };

fn atm_march(o: AtmoOrigin, cos_vs: f32, s_end: f32, lay: i32, n: i32) -> AtmoMarch {
    var radiance = vec3<f32>(0.0);
    var thr = vec3<f32>(1.0);
    let phase_r = atm_rayleigh_phase(cos_vs);
    let phase_m = atm_cs_phase(A.mie_abs.w, cos_vs);
    for (var i = 0; i < n; i = i + 1) {
        let a = atm_step_at(lay, s_end, i, n);
        let b = atm_step_at(lay, s_end, i + 1, n);
        let ds = b - a;
        if (ds <= 0.0) { continue; }
        let s = 0.5 * (a + b);
        let hr = atm_sample_at(o, s);
        let m = atm_medium_at(hr.x);
        let h_c = min(hr.x, A.body.y);
        let mu_sun_s = select(1.0, clamp((o.r_musun + s * cos_vs) / hr.y, -1.0, 1.0), hr.y > 0.0);
        let t_sun = atm_transmittance(h_c, mu_sun_s);
        let shadow = atm_planet_shadow(h_c, mu_sun_s);
        let ms = atm_multiscatter(h_c, mu_sun_s);
        let single = (m.rayleigh * phase_r + m.mie * phase_m) * t_sun * shadow;
        let s_in = single + ms * m.scattering;
        let tr = exp(-m.extinction * ds);
        // (1 - exp(-x)) / x without cancellation for the thin steps of a thin medium.
        let x = m.extinction * ds;
        let phi = select((vec3<f32>(1.0) - tr) / x, vec3<f32>(1.0) - x * 0.5 + x * x * (1.0 / 6.0), x < vec3<f32>(1.0e-3));
        let s_int = s_in * ds * phi;
        radiance = radiance + thr * s_int;
        thr = thr * tr;
    }
    var out: AtmoMarch;
    out.radiance = radiance;
    out.transmittance = thr;
    return out;
}

fn atm_ground_radiance(mu_sun_g: f32) -> vec3<f32> {
    let t = atm_transmittance(0.0, mu_sun_g);
    let shadow = atm_planet_shadow(0.0, mu_sun_g);
    let ms = atm_multiscatter(0.0, mu_sun_g);
    let direct = shadow * max(mu_sun_g, 0.0) / ATM_PI;
    return A.ground.xyz * (t * direct + ms);
}

fn atm_cam_origin(mu: f32) -> AtmoOrigin {
    var o: AtmoOrigin;
    o.h = A.cam.x;
    o.r = A.cam.y;
    o.c = A.cam.z;
    o.rmu = A.cam.y * mu;
    o.r_musun = A.cam.y * A.up.w;
    return o;
}

// The ray end for a camera inside the atmosphere: x the distance, y 1 when it reaches the ground.
fn atm_inside_end(mu: f32) -> vec2<f32> {
    let rmu = A.cam.y * mu;
    if (mu < A.cam2.y) {
        let t = atm_dist_to_ground(rmu, A.cam.z);
        if (t >= 0.0) { return vec2<f32>(t, 1.0); }
    }
    return vec2<f32>(atm_dist_to_top(rmu, A.cam.w), 0.0);
}

// Texel (ix, iy) of the sky-view table as (mu, cos(view, sun), reaches the ground).
fn atm_sky_texel_dir(ix: i32, iy: i32) -> vec3<f32> {
    let u = f32(ix) / f32(SKY_W - 1);
    let v = f32(iy) / f32(SKY_H - 1);
    let phi = ATM_PI * u * u;
    var theta: f32;
    var ground = 0.0;
    if (v < 0.5) {
        let q = 1.0 - 2.0 * v;
        theta = (1.0 - q * q) * A.cam2.z;
    } else {
        let q = 2.0 * v - 1.0;
        theta = A.cam2.z + A.cam2.w * q * q;
        ground = 1.0;
    }
    let mu = clamp(cos(theta), -1.0, 1.0);
    let sin_t = abs(sin(theta));
    let cos_vs = clamp(sin_t * cos(phi) * A.sun.w + mu * A.up.w, -1.0, 1.0);
    return vec3<f32>(mu, cos_vs, ground);
}

fn atm_sky_texel(ix: i32, iy: i32) -> vec3<f32> {
    let d = atm_sky_texel_dir(ix, iy);
    let o = atm_cam_origin(d.x);
    var s_end = atm_dist_to_top(o.rmu, A.cam.w);
    var hits = false;
    if (d.z > 0.5) {
        let g = atm_dist_to_ground(o.rmu, A.cam.z);
        if (g >= 0.0) { s_end = g; hits = true; }
    }
    let m = atm_march(o, d.y, s_end, 1, SKY_STEPS);
    var l = m.radiance;
    if (hits) {
        let r_g = atm_sample_at(o, s_end).y;
        let mu_g = clamp((o.r_musun + s_end * d.y) / max(r_g, 1.0), -1.0, 1.0);
        l = l + m.transmittance * atm_ground_radiance(mu_g);
    }
    return l;
}

// The sky-view coordinates (u, v) of a unit view direction in planet-fixed axes.
fn atm_sky_coords(dir: vec3<f32>) -> vec2<f32> {
    let mu = clamp(dot(dir, A.up.xyz), -1.0, 1.0);
    let py = abs(dot(dir, A.e2.xyz));
    let px = dot(dir, A.e1.xyz);
    // atan2(0, 0) is undefined: a direction along the up has no azimuth.
    let phi = select(atan2(py, px), 0.0, py + abs(px) < 1.0e-20);
    let u = sqrt(phi / ATM_PI);
    let theta = acos(mu);
    var v: f32;
    if (mu > A.cam2.y) {
        v = 0.5 * (1.0 - sqrt(max(1.0 - theta / A.cam2.z, 0.0)));
    } else {
        v = 0.5 + 0.5 * sqrt(clamp((theta - A.cam2.z) / A.cam2.w, 0.0, 1.0));
    }
    return vec2<f32>(u, v);
}

fn atm_ap_slice_distance(k: i32) -> f32 {
    let w = f32(k + 1) / f32(AP_D);
    return A.misc.z * w * w;
}

fn atm_froxel_ray(ix: i32, iy: i32) -> vec3<f32> {
    let a = 2.0 * (f32(ix) + 0.5) / f32(AP_W) - 1.0;
    let b = 1.0 - 2.0 * (f32(iy) + 0.5) / f32(AP_H);
    return A.fwd.xyz + A.right.xyz * a + A.upv.xyz * b;
}

fn atm_ap_texel(ix: i32, iy: i32, iz: i32) -> vec4<f32> {
    let dir = normalize(atm_froxel_ray(ix, iy));
    let mu = clamp(dot(dir, A.up.xyz), -1.0, 1.0);
    let cos_vs = clamp(dot(dir, A.sun.xyz), -1.0, 1.0);
    let s_end = min(atm_ap_slice_distance(iz), atm_dist_to_top(A.cam.y * mu, A.cam.w));
    let steps = clamp(2 * (iz + 1), 1, AP_MAX_STEPS);
    let m = atm_march(atm_cam_origin(mu), cos_vs, s_end, 0, steps);
    return vec4<f32>(m.radiance, (m.transmittance.x + m.transmittance.y + m.transmittance.z) / 3.0);
}

// A ray from a camera outside the atmosphere: radiance and transmittance up to the geometry (t_geo >= 0), the ground or the exit;
// flags.x: reaches the ground, flags.y: crosses the atmosphere. The march starts at the entry point (altitude H, r mu = -sqrt(D_top)).
struct AtmoOrbit { radiance: vec3<f32>, transmittance: vec3<f32>, flags: vec2<f32>, ground_mu: f32 };

fn atm_orbit_ray(dir: vec3<f32>, t_geo: f32) -> AtmoOrbit {
    return atm_orbit_core(clamp(dot(dir, A.up.xyz), -1.0, 1.0), clamp(dot(dir, A.sun.xyz), -1.0, 1.0), t_geo);
}

// The march of a ray with the given cosines to the up and to the sun (the arithmetic the bound of ATMO-001 measures; the cosines
// themselves are rounded dot products of the pixel's direction).
fn atm_orbit_core(mu: f32, cos_vs: f32, t_geo: f32) -> AtmoOrbit {
    var out: AtmoOrbit;
    out.radiance = vec3<f32>(0.0);
    out.transmittance = vec3<f32>(1.0);
    out.flags = vec2<f32>(0.0);
    out.ground_mu = 1.0;
    // r mu as a high and a low f32 (the product's rounding error is exact with fma, and r itself is high + low): near the limb the
    // discriminants (r mu - q)(r mu + q) and (r mu - rho)(r mu + rho) keep their digits where r mu * r mu - c would lose them.
    let rmu = A.cam.y * mu;
    let rmu_lo = fma(A.cam.y, mu, -rmu) + A.e1.w * mu;
    if (mu >= 0.0) { return out; }
    let q = A.right.w;
    let d_top = (rmu - q) * ((rmu + q) + (rmu_lo + A.upv.w));
    if (d_top < 0.0) { return out; }
    let sq = sqrt(d_top);
    let t0 = A.cam.w / (-rmu + sq);
    var s_end = 2.0 * sq;
    var ground = false;
    let rho = A.cam2.x;
    let d_ground = (rmu - rho) * ((rmu + rho) + (rmu_lo + A.e2.w));
    if (d_ground >= 0.0) { s_end = A.cam.z / (-rmu + sqrt(d_ground)) - t0; ground = true; }
    if (t_geo >= 0.0 && t_geo - t0 < s_end) { s_end = t_geo - t0; ground = false; }
    out.flags = vec2<f32>(select(0.0, 1.0, ground), 1.0);
    if (s_end <= 0.0) { out.flags.x = 0.0; return out; }
    var o: AtmoOrigin;
    o.h = A.body.y;
    o.r = A.body.x + A.body.y;
    o.c = A.body.z * A.body.z;
    o.rmu = -sq;
    o.r_musun = A.cam.y * A.up.w + t0 * cos_vs;
    let m = atm_march(o, cos_vs, s_end, 0, ORBIT_STEPS);
    out.radiance = m.radiance;
    out.transmittance = m.transmittance;
    out.ground_mu = clamp((o.r_musun + s_end * cos_vs) / A.body.x, -1.0, 1.0);
    return out;
}

// Radiance of the sun's disc (illuminance 1 over its solid angle) with a ramp of 6 % of its size across the rim.
fn atm_sun_disc(dir: vec3<f32>) -> f32 {
    let w = A.body.w;
    // The angle from the chord between the view and the sun: 1 - cos is not resolved in f32 at 0.005 rad.
    // asin is accurate to only a few thousand ulp on some adapters: for the chords that matter (< 0.1) use its series.
    let chord = length(dir - A.sun.xyz);
    let angle = select(2.0 * asin(min(0.5 * chord, 1.0)), chord * (1.0 + chord * chord / 24.0), chord < 0.1);
    let coverage = clamp((w - angle) / (0.06 * w) + 0.5, 0.0, 1.0);
    return coverage / (ATM_PI * w * w);
}

"#;

const SKY_VIEW: &str = r#"
@group(0) @binding(0) var<uniform> A: AtmoBlock;
@group(0) @binding(1) var tr_lut: texture_2d<f32>;
@group(0) @binding(2) var ms_lut: texture_2d<f32>;
@group(0) @binding(3) var out_tex: texture_storage_2d<rgba16float, write>;
"#;

const SKY_VIEW_MAIN: &str = r#"
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (i32(id.x) >= SKY_W || i32(id.y) >= SKY_H) { return; }
    let l = atm_sky_texel(i32(id.x), i32(id.y));
    textureStore(out_tex, vec2<i32>(id.xy), vec4<f32>(l, 1.0));
}
"#;

const AERIAL: &str = r#"
@group(0) @binding(0) var<uniform> A: AtmoBlock;
@group(0) @binding(1) var tr_lut: texture_2d<f32>;
@group(0) @binding(2) var ms_lut: texture_2d<f32>;
@group(0) @binding(3) var out_tex: texture_storage_3d<rgba16float, write>;
"#;

const AERIAL_MAIN: &str = r#"
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (i32(id.x) >= AP_W || i32(id.y) >= AP_H || i32(id.z) >= AP_D) { return; }
    textureStore(out_tex, vec3<i32>(id), atm_ap_texel(i32(id.x), i32(id.y), i32(id.z)));
}
"#;

const COMPOSITE: &str = r#"
@group(0) @binding(0) var<uniform> A: AtmoBlock;
@group(0) @binding(1) var tr_lut: texture_2d<f32>;
@group(0) @binding(2) var ms_lut: texture_2d<f32>;
@group(0) @binding(3) var sky_lut: texture_2d<f32>;
@group(0) @binding(4) var ap_vol: texture_3d<f32>;
@group(0) @binding(5) var scene: texture_2d<f32>;
@group(0) @binding(6) var scene_depth: texture_2d<f32>;
"#;

const COMPOSITE_MAIN: &str = r#"
struct V { @builtin(position) pos: vec4<f32> };

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    let q = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: V;
    out.pos = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn atm_sky_lookup(dir: vec3<f32>) -> vec3<f32> {
    let uv = atm_sky_coords(dir);
    return atm_bilerp(sky_lut, SKY_W, SKY_H, vec2<f32>(uv.x * f32(SKY_W - 1), uv.y * f32(SKY_H - 1)));
}

fn atm_ap_corner(x0: i32, y0: i32, z: i32, fx: f32, fy: f32) -> vec4<f32> {
    let a = textureLoad(ap_vol, vec3<i32>(x0, y0, z), 0);
    let b = textureLoad(ap_vol, vec3<i32>(x0 + 1, y0, z), 0);
    let c = textureLoad(ap_vol, vec3<i32>(x0, y0 + 1, z), 0);
    let d = textureLoad(ap_vol, vec3<i32>(x0 + 1, y0 + 1, z), 0);
    return mix(mix(a, b, fx), mix(c, d, fx), fy);
}

// Trilinear lookup of the aerial-perspective volume at screen position uv and distance t along the ray; the identity before slice 0.
fn atm_ap_lookup(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let px = clamp(uv.x * f32(AP_W) - 0.5, 0.0, f32(AP_W - 1));
    let py = clamp(uv.y * f32(AP_H) - 0.5, 0.0, f32(AP_H - 1));
    let w = sqrt(max(t / A.misc.z, 0.0));
    let pz = clamp(w * f32(AP_D) - 1.0, -1.0, f32(AP_D - 1));
    let x0 = min(i32(floor(px)), AP_W - 2);
    let y0 = min(i32(floor(py)), AP_H - 2);
    let fx = px - f32(x0);
    let fy = py - f32(y0);
    let z0 = i32(floor(pz));
    let z1 = min(z0 + 1, AP_D - 1);
    let fz = pz - f32(z0);
    var a = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    var b = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (z0 >= 0) { a = atm_ap_corner(x0, y0, z0, fx, fy); }
    if (z1 >= 0) { b = atm_ap_corner(x0, y0, z1, fx, fy); }
    return mix(a, b, fz);
}

// Debug views (CON-18): eight discrete levels of a value, so a view of a static table is identical on every adapter.
fn atm_levels(v: vec3<f32>) -> vec3<f32> {
    return floor(clamp(v, vec3<f32>(0.0), vec3<f32>(0.999))  * 8.0) / 8.0 * 0.8;
}

@fragment
fn fs(v: V) -> @location(0) vec4<f32> {
    let px = vec2<i32>(v.pos.xy);
    let size = vec2<f32>(textureDimensions(scene));
    let a = 2.0 * (f32(px.x) + 0.5) / size.x - 1.0;
    let b = 1.0 - 2.0 * (f32(px.y) + 0.5) / size.y;
    let ray = A.fwd.xyz + A.right.xyz * a + A.upv.xyz * b;
    let len = length(ray);
    let dir = ray / len;
    let z = textureLoad(scene_depth, px, 0).x;
    let geometry = z > 0.0;
    let t_geo = select(-1.0, A.fwd.w / max(z, 1.0e-30) * len, geometry);
    let color = textureLoad(scene, px, 0);
    let factor = A.misc.x;
    let uv = vec2<f32>((f32(px.x) + 0.5) / size.x, (f32(px.y) + 0.5) / size.y);
    let inside = A.misc.y > 0.5;
    let cos_vs = clamp(dot(dir, A.sun.xyz), -1.0, 1.0);
    let mu = clamp(dot(dir, A.up.xyz), -1.0, 1.0);
    let view = bitcast<u32>(A.misc.w);
    var rgb = color.xyz;
    var view_t = vec3<f32>(1.0);
    var ap = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (inside) {
        if (geometry) {
            ap = atm_ap_lookup(uv, t_geo);
            rgb = color.xyz * ap.w + ap.xyz * factor;
            view_t = vec3<f32>(ap.w);
        } else {
            var l = atm_sky_lookup(dir);
            if (mu > A.cam2.y) {
                let t = atm_transmittance(A.cam.x, mu);
                l = l + t * atm_sun_disc(dir);
                view_t = t;
            } else {
                view_t = vec3<f32>(0.0);
            }
            rgb = l * factor;
        }
    } else {
        let o = atm_orbit_ray(dir, t_geo);
        view_t = o.transmittance;
        if (geometry) {
            rgb = color.xyz * o.transmittance + o.radiance * factor;
        } else {
            var l = o.radiance;
            if (o.flags.x > 0.5) {
                l = l + o.transmittance * atm_ground_radiance(o.ground_mu);
                view_t = vec3<f32>(0.0);
            } else {
                l = l + o.transmittance * atm_sun_disc(dir);
            }
            rgb = l * factor;
        }
    }
    if (view == 1u) {
        let t = textureLoad(tr_lut, vec2<i32>(i32(uv.x * f32(TR_W)), i32(uv.y * f32(TR_H))), 0).xyz;
        return vec4<f32>(atm_levels(t / 8.0), color.w);
    }
    if (view == 2u) {
        let t = textureLoad(ms_lut, vec2<i32>(i32(uv.x * f32(MS_W)), i32(uv.y * f32(MS_H))), 0).xyz;
        return vec4<f32>(atm_levels(t / 0.08), color.w);
    }
    if (view == 3u) {
        let t = textureLoad(sky_lut, vec2<i32>(i32(uv.x * f32(SKY_W)), i32(uv.y * f32(SKY_H))), 0).xyz;
        return vec4<f32>(atm_levels(t / 0.2), color.w);
    }
    if (view == 4u) {
        let q = atm_ap_lookup(uv, select(A.misc.z * 0.25, t_geo, geometry));
        return vec4<f32>(atm_levels(q.xyz / 0.2), color.w);
    }
    if (view == 5u) {
        return vec4<f32>(atm_levels(view_t), color.w);
    }
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(60000.0)), color.w);
}
"#;

/// The atmosphere lighting the terrain shader adds when the frame has an atmosphere (group 3: the uniform block and the two static
/// tables). `lit` is the albedo times the lighting (REND-025's identity holds); the lighting is the sun's transmitted, shadowed
/// Lambert term plus the multiple-scattering ambient on the hemisphere. The same expression with the ground's albedo is
/// `Atmosphere::ground_radiance`. It follows the terrain shader's own declarations (`g`, `VsOut`, `up_at`, `tile_climate`,
/// `albedo_of`, `climate_colour`).
const TERRAIN: &str = r#"
@group(3) @binding(0) var<uniform> A: AtmoBlock;
@group(3) @binding(1) var tr_lut: texture_2d<f32>;
@group(3) @binding(2) var ms_lut: texture_2d<f32>;
"#;

/// The terrain's lighting function (the probe compiles it without the rest of the terrain shader).
const LIGHTING: &str = r#"
// The sun's light on a surface with the given normal at altitude `h_alt` above the reference surface (up and sun in planet-fixed
// axes): transmitted direct light on Lambert's cosine plus the multiple-scattering ambient on the sky hemisphere.
fn atm_terrain_lighting(normal: vec3<f32>, up: vec3<f32>, h_alt: f32, sun: vec3<f32>) -> vec3<f32> {
    let n = normalize(normal);
    let ndl = max(dot(n, sun), 0.0);
    let mu_s = clamp(dot(up, sun), -1.0, 1.0);
    let h = clamp(h_alt, 0.0, A.body.y);
    let t = atm_transmittance(h, mu_s);
    let shadow = atm_planet_shadow(h, mu_s);
    let ms = atm_multiscatter(h, mu_s);
    let hemi = 0.5 + 0.5 * clamp(dot(n, up), -1.0, 1.0);
    return t * (shadow * ndl * 0.31830988618) + ms * hemi;
}
"#;

const TERRAIN_MAIN: &str = r#"

fn atm_terrain_radiance(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, h_alt: f32, sun: vec3<f32>) -> vec3<f32> {
    let n = normalize(normal);
    let cs = clamp(dot(n, up), 0.0, 1.0);
    let inv_pi = 0.31830988618;
    if (view >= 3u) {
        return climate_colour(view, t, p) * inv_pi;
    }
    let light = atm_terrain_lighting(normal, up, h_alt, sun);
    if (view == 2u) {
        return light;
    }
    let a = albedo_of(tab, t, p, cs, h);
    if (view == 1u) {
        return a * inv_pi;
    }
    return a * light;
}

fn atm_shade_lit(view: u32, tab: array<vec4<f32>, 10>, normal: vec3<f32>, up: vec3<f32>, t: f32, p: f32, h: f32, h_alt: f32, sun: vec3<f32>, factor: f32) -> vec3<f32> {
    return clamp(atm_terrain_radiance(view, tab, normal, up, t, p, h, h_alt, sun) * factor, vec3<f32>(0.0), vec3<f32>(60000.0));
}

@fragment
fn fs_lit_atmo(in: VsOut) -> @location(0) vec4<f32> {
    let sun = vec3<f32>(bitcast<f32>(g.mode.y), bitcast<f32>(g.mode.z), bitcast<f32>(g.mode.w));
    let up = up_at(nodes[in.node_index].up, in.local);
    let climate = tile_climate(in);
    return vec4<f32>(atm_shade_lit(g.mode.x, g.albedo, in.normal, up, climate.x, climate.y, in.height - g.extra.x, in.height, sun, g.range.z), 1.0);
}
"#;

/// The text appended to the terrain shader (vertex or compact) for the frames that have an atmosphere.
pub fn terrain_atmosphere_source() -> String {
    format!("{TERRAIN}{DECLS}{LIGHTING}{TERRAIN_MAIN}")
}

/// What the water shader adds to the terrain's atmosphere declarations: the ray-march helpers (the sky-view coordinates) in the
/// order the lighting needs them.
pub(crate) fn water_atmosphere_source() -> String {
    format!("{TERRAIN}{DECLS}{MARCH}{LIGHTING}")
}

pub fn sky_view_shader() -> String {
    format!("{SKY_VIEW}{DECLS}{MARCH}{SKY_VIEW_MAIN}")
}

pub fn aerial_perspective_shader() -> String {
    format!("{AERIAL}{DECLS}{MARCH}{AERIAL_MAIN}")
}

pub fn composite_shader() -> String {
    format!("{COMPOSITE}{DECLS}{MARCH}{COMPOSITE_MAIN}")
}

/// The atmosphere shaders by name, for the validation and scan tests.
pub fn atmosphere_shaders() -> Vec<(&'static str, String)> {
    vec![
        ("atmosphere-sky-view", sky_view_shader()),
        ("atmosphere-aerial-perspective", aerial_perspective_shader()),
        ("atmosphere-composite", composite_shader()),
        ("atmosphere-terrain-vertex", format!("{}{}", crate::terrain::TERRAIN_SHADERS[0].1, terrain_atmosphere_source())),
        ("atmosphere-terrain-compact", format!("{}{}", crate::compact_terrain_shader(32), terrain_atmosphere_source())),
    ]
}

fn texture_2d(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// The frame's atmosphere: its tables and products, the compute pipelines and the composite, created once.
pub struct AtmospherePath {
    size: (u32, u32),
    /// The body whose static tables are uploaded.
    tables: Option<Atmosphere>,
    uploads: u32,
    uniform: wgpu::Buffer,
    tr_lut: wgpu::Texture,
    ms_lut: wgpu::Texture,
    sky_view: wgpu::Texture,
    ap_volume: wgpu::Texture,
    scene: wgpu::Texture,
    scene_view: wgpu::TextureView,
    sky_pipeline: wgpu::ComputePipeline,
    sky_bind: wgpu::BindGroup,
    ap_pipeline: wgpu::ComputePipeline,
    ap_bind: wgpu::BindGroup,
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind: wgpu::BindGroup,
    /// The layout and bind group the terrain's atmosphere pipelines use (group 3).
    terrain_bgl: wgpu::BindGroupLayout,
    terrain_bind: wgpu::BindGroup,
    /// A synthetic depth target when the path was made without one (tests of the composite).
    synthetic_depth: Option<wgpu::Texture>,
    allocated: u64,
}

impl AtmospherePath {
    /// Create the tables, the products, the scene target and the pipelines for frames of `size`. `depth` is the depth view the terrain
    /// writes (the composite reads it); without one a synthetic `R32Float` depth is made that [`upload_depth`](Self::upload_depth) fills.
    /// A path of its own (tests and tools): a synthetic depth target and the scene format of the lit path.
    pub fn standalone(ctx: &GpuContext, size: (u32, u32)) -> Self {
        Self::new(ctx, size, None, crate::lit::HDR_FORMAT)
    }

    pub(crate) fn new(ctx: &GpuContext, size: (u32, u32), depth: Option<&wgpu::TextureView>, final_hdr: wgpu::TextureFormat) -> Self {
        let device = &ctx.device;
        let tex = wgpu::TextureUsages::TEXTURE_BINDING;
        let tr_lut = texture_2d(
            device,
            "atmosphere-transmittance-lut",
            (TRANSMITTANCE_W as u32, TRANSMITTANCE_H as u32),
            LUT_FORMAT,
            tex | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        );
        let ms_lut = texture_2d(
            device,
            "atmosphere-multiscatter-lut",
            (MS_W as u32, MS_H as u32),
            LUT_FORMAT,
            tex | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        );
        let product_usage = tex | wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC;
        let sky_view = texture_2d(device, "atmosphere-sky-view", (SKY_W as u32, SKY_H as u32), PRODUCT_FORMAT, product_usage);
        let ap_volume = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atmosphere-aerial-perspective"),
            size: wgpu::Extent3d { width: AP_W as u32, height: AP_H as u32, depth_or_array_layers: AP_D as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: PRODUCT_FORMAT,
            usage: product_usage,
            view_formats: &[],
        });
        let scene = texture_2d(
            device,
            "atmosphere-scene",
            size,
            final_hdr,
            wgpu::TextureUsages::RENDER_ATTACHMENT | tex | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
        );
        let scene_view = scene.create_view(&Default::default());
        let synthetic_depth = depth.is_none().then(|| {
            texture_2d(device, "atmosphere-synthetic-depth", size, wgpu::TextureFormat::R32Float, tex | wgpu::TextureUsages::COPY_DST)
        });
        let synthetic_view = synthetic_depth.as_ref().map(|t| t.create_view(&Default::default()));
        let depth_view = depth.or(synthetic_view.as_ref()).expect("a depth view");
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("atmosphere-uniform"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (tr_view, ms_view) = (tr_lut.create_view(&Default::default()), ms_lut.create_view(&Default::default()));
        let (sky_tex_view, ap_tex_view) = (sky_view.create_view(&Default::default()), ap_volume.create_view(&Default::default()));

        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let uniform_entry = |vis| {
            entry(
                0,
                vis,
                wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            )
        };
        let tex_entry = |binding, vis, dim| {
            entry(
                binding,
                vis,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: dim,
                    multisampled: false,
                },
            )
        };
        let storage_entry = |dim| {
            entry(
                3,
                wgpu::ShaderStages::COMPUTE,
                wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: PRODUCT_FORMAT,
                    view_dimension: dim,
                },
            )
        };
        let (c, f) = (wgpu::ShaderStages::COMPUTE, wgpu::ShaderStages::FRAGMENT);
        let d2 = wgpu::TextureViewDimension::D2;
        let module = |label, source: String| {
            device
                .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) })
        };
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            (bgl, pl)
        };
        let bind = |label, bgl: &wgpu::BindGroupLayout, resources: Vec<wgpu::BindingResource>| {
            let entries: Vec<wgpu::BindGroupEntry> =
                resources.into_iter().enumerate().map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource }).collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(label), layout: bgl, entries: &entries })
        };

        let (sky_bgl, sky_pl) =
            layout("atmosphere-sky-view", &[uniform_entry(c), tex_entry(1, c, d2), tex_entry(2, c, d2), storage_entry(d2)]);
        let sky_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("atmosphere-sky-view"),
            layout: Some(&sky_pl),
            module: &module("atmosphere-sky-view", sky_view_shader()),
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sky_bind = bind(
            "atmosphere-sky-view",
            &sky_bgl,
            vec![
                uniform.as_entire_binding(),
                wgpu::BindingResource::TextureView(&tr_view),
                wgpu::BindingResource::TextureView(&ms_view),
                wgpu::BindingResource::TextureView(&sky_tex_view),
            ],
        );
        let d3 = wgpu::TextureViewDimension::D3;
        let (ap_bgl, ap_pl) = layout("atmosphere-aerial", &[uniform_entry(c), tex_entry(1, c, d2), tex_entry(2, c, d2), storage_entry(d3)]);
        let ap_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("atmosphere-aerial-perspective"),
            layout: Some(&ap_pl),
            module: &module("atmosphere-aerial-perspective", aerial_perspective_shader()),
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let ap_bind = bind(
            "atmosphere-aerial",
            &ap_bgl,
            vec![
                uniform.as_entire_binding(),
                wgpu::BindingResource::TextureView(&tr_view),
                wgpu::BindingResource::TextureView(&ms_view),
                wgpu::BindingResource::TextureView(&ap_tex_view),
            ],
        );
        let (comp_bgl, comp_pl) = layout(
            "atmosphere-composite",
            &[
                uniform_entry(f),
                tex_entry(1, f, d2),
                tex_entry(2, f, d2),
                tex_entry(3, f, d2),
                tex_entry(4, f, d3),
                tex_entry(5, f, d2),
                tex_entry(6, f, d2),
            ],
        );
        let comp_module = module("atmosphere-composite", composite_shader());
        let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("atmosphere-composite"),
            layout: Some(&comp_pl),
            vertex: wgpu::VertexState {
                module: &comp_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &comp_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: final_hdr, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let composite_bind = bind(
            "atmosphere-composite",
            &comp_bgl,
            vec![
                uniform.as_entire_binding(),
                wgpu::BindingResource::TextureView(&tr_view),
                wgpu::BindingResource::TextureView(&ms_view),
                wgpu::BindingResource::TextureView(&sky_tex_view),
                wgpu::BindingResource::TextureView(&ap_tex_view),
                wgpu::BindingResource::TextureView(&scene_view),
                wgpu::BindingResource::TextureView(depth_view),
            ],
        );
        let terrain_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere-terrain"),
            entries: &[uniform_entry(f), tex_entry(1, f, d2), tex_entry(2, f, d2)],
        });
        let terrain_bind = bind(
            "atmosphere-terrain",
            &terrain_bgl,
            vec![uniform.as_entire_binding(), wgpu::BindingResource::TextureView(&tr_view), wgpu::BindingResource::TextureView(&ms_view)],
        );

        use planet_render_sizes as sizes;
        let pixels = u64::from(size.0) * u64::from(size.1);
        let mut allocated = pixels * crate::graph::HDR_BYTES_PER_PIXEL
            + sizes::TRANSMITTANCE_LUT_BYTES
            + sizes::MULTISCATTER_LUT_BYTES
            + sizes::SKY_VIEW_BYTES
            + sizes::AERIAL_PERSPECTIVE_BYTES;
        if depth.is_none() {
            allocated += pixels * 4;
        }
        Self {
            size,
            tables: None,
            uploads: 0,
            uniform,
            tr_lut,
            ms_lut,
            sky_view,
            ap_volume,
            scene,
            scene_view,
            sky_pipeline,
            sky_bind,
            ap_pipeline,
            ap_bind,
            composite_pipeline,
            composite_bind,
            terrain_bgl,
            terrain_bind,
            synthetic_depth,
            allocated,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Bytes this path allocated that the frame-target share counts: the scene target, the four tables and products (and a synthetic
    /// depth target in tests). Equals what the compiled graph counts for them.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated
    }

    /// The scene target the terrain draws into when the frame has an atmosphere.
    pub(crate) fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene_view
    }

    pub(crate) fn terrain_layout(&self) -> &wgpu::BindGroupLayout {
        &self.terrain_bgl
    }

    pub(crate) fn terrain_bind(&self) -> &wgpu::BindGroup {
        &self.terrain_bind
    }

    /// The water pass's group 3 with the atmosphere: the uniform block (0), the two static tables (1, 2), the sky-view table (3) and
    /// the copy of the scene target (4). Returns the layout and the bind group.
    pub(crate) fn water_group(&self, device: &wgpu::Device, scene_copy: &wgpu::TextureView) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water-atmosphere"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex(1),
                tex(2),
                tex(3),
                tex(4),
            ],
        });
        let (tr, ms, sky) = (
            self.tr_lut.create_view(&Default::default()),
            self.ms_lut.create_view(&Default::default()),
            self.sky_view.create_view(&Default::default()),
        );
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water-atmosphere"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tr) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&ms) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&sky) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(scene_copy) },
            ],
        });
        (layout, bind)
    }

    /// The scene target's texture (copy source of the water pass).
    pub(crate) fn scene_texture(&self) -> &wgpu::Texture {
        &self.scene
    }

    /// How often the static tables were computed and uploaded.
    pub fn table_uploads(&self) -> u32 {
        self.uploads
    }

    /// The body whose tables are uploaded.
    pub fn tables_for(&self) -> Option<&Atmosphere> {
        self.tables.as_ref()
    }

    /// Upload the static tables of `atm` if they are not the ones held (a new body or a look change): computed on the CPU in f64
    /// (ADR 0040 point 2), written once, never per frame. Returns the tables when it uploaded.
    pub fn ensure_tables(&mut self, ctx: &GpuContext, atm: &Atmosphere) -> Option<StaticLuts> {
        if self.tables.as_ref() == Some(atm) {
            return None;
        }
        let luts = StaticLuts::generate(atm);
        self.upload_tables(ctx, atm, &luts);
        Some(luts)
    }

    /// Upload given tables (tests that share one generation).
    pub fn upload_tables(&mut self, ctx: &GpuContext, atm: &Atmosphere, luts: &StaticLuts) {
        let write = |texture: &wgpu::Texture, size: (u32, u32), table: &[[f64; 3]]| {
            let bytes: Vec<u8> = StaticLuts::rgba_f32(table).iter().flat_map(|v| v.to_le_bytes()).collect();
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.0 * 16), rows_per_image: Some(size.1) },
                wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            );
        };
        write(&self.tr_lut, (TRANSMITTANCE_W as u32, TRANSMITTANCE_H as u32), &luts.optical_depth);
        write(&self.ms_lut, (MS_W as u32, MS_H as u32), &luts.multiscatter);
        self.tables = Some(*atm);
        self.uploads += 1;
    }

    /// Write the frame's uniform block: the body (static words) and the frame's geometry, the pre-exposure factor and the debug view.
    /// The queue orders the write before the submission that holds this frame's passes and after every earlier submission.
    pub fn write_frame(
        &self,
        ctx: &GpuContext,
        atm: &Atmosphere,
        geometry: &FrameGeometry,
        pre_exposure_factor: f32,
        view: AtmosphereView,
    ) {
        let mut words = [0f32; 64];
        words[..24].copy_from_slice(&atm.static_words());
        words[24..].copy_from_slice(&geometry.words(pre_exposure_factor, view.index()));
        let bytes: Vec<u8> = words.iter().flat_map(|v| v.to_le_bytes()).collect();
        ctx.queue.write_buffer(&self.uniform, 0, &bytes);
    }

    /// Record the sky-view table and the aerial-perspective volume (compute).
    pub(crate) fn record_sky_view(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sky-view"), timestamp_writes: None });
        pass.set_pipeline(&self.sky_pipeline);
        pass.set_bind_group(0, &self.sky_bind, &[]);
        pass.dispatch_workgroups((SKY_W as u32).div_ceil(8), (SKY_H as u32).div_ceil(8), 1);
    }

    pub(crate) fn record_aerial_perspective(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass =
            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("aerial-perspective"), timestamp_writes: None });
        pass.set_pipeline(&self.ap_pipeline);
        pass.set_bind_group(0, &self.ap_bind, &[]);
        pass.dispatch_workgroups((AP_W as u32).div_ceil(8), (AP_H as u32).div_ceil(8), AP_D as u32);
    }

    /// Record the composite into `target` (the final scene target of the lit path).
    pub(crate) fn record_composite(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sky-composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, &self.composite_bind, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Write the scene target (RGBA f32 per pixel, rounded to f16): the input of the composite's oracles.
    pub fn upload_scene(&self, ctx: &GpuContext, pixels: &[[f32; 4]]) {
        assert_eq!(pixels.len() as u64, u64::from(self.size.0) * u64::from(self.size.1), "one RGBA value per pixel");
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.iter().flat_map(|v| crate::lit::f32_to_f16_bits(*v).to_le_bytes())).collect();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.scene,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.size.0 * 8), rows_per_image: Some(self.size.1) },
            wgpu::Extent3d { width: self.size.0, height: self.size.1, depth_or_array_layers: 1 },
        );
    }

    /// Write the synthetic depth (reversed-Z, 0 = no geometry), for a path made without a depth view.
    pub fn upload_depth(&self, ctx: &GpuContext, depth: &[f32]) {
        let texture = self.synthetic_depth.as_ref().expect("a path made without a depth view");
        assert_eq!(depth.len() as u64, u64::from(self.size.0) * u64::from(self.size.1), "one depth value per pixel");
        let bytes: Vec<u8> = depth.iter().flat_map(|v| v.to_le_bytes()).collect();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.size.0 * 4), rows_per_image: Some(self.size.1) },
            wgpu::Extent3d { width: self.size.0, height: self.size.1, depth_or_array_layers: 1 },
        );
    }

    /// Run the sky-view and aerial-perspective passes alone (tests and tools).
    pub fn run_products(&self, ctx: &GpuContext) {
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("atmosphere-products") });
        self.record_sky_view(&mut encoder);
        self.record_aerial_perspective(&mut encoder);
        ctx.queue.submit([encoder.finish()]);
    }

    /// Run the composite alone into a scratch target and wait (ad hoc timing).
    pub fn composite_only(&self, ctx: &GpuContext) {
        let target = texture_2d(
            &ctx.device,
            "atmosphere-composite-scratch",
            self.size,
            crate::lit::HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let view = target.create_view(&Default::default());
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("atmosphere-composite") });
        self.record_composite(&mut encoder, &view);
        ctx.queue.submit([encoder.finish()]);
    }

    /// Run the composite alone into `target` (an `Rgba16Float` texture of this size created by the caller through
    /// [`composite_to_image`](Self::composite_to_image)).
    pub fn composite_to_image(&self, ctx: &GpuContext) -> Vec<[f32; 4]> {
        let target = texture_2d(
            &ctx.device,
            "atmosphere-composite-test",
            self.size,
            crate::lit::HDR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let view = target.create_view(&Default::default());
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("atmosphere-composite") });
        self.record_composite(&mut encoder, &view);
        ctx.queue.submit([encoder.finish()]);
        read_f16_texture(ctx, &target, (self.size.0, self.size.1, 1), 4)
    }

    /// The sky-view table as RGBA f32 (row-major, 192 x 108).
    pub fn read_sky_view(&self, ctx: &GpuContext) -> Vec<[f32; 4]> {
        read_f16_texture(ctx, &self.sky_view, (SKY_W as u32, SKY_H as u32, 1), 4)
    }

    /// The aerial-perspective volume as RGBA f32 (`z` slowest, then rows, 32 x 32 x 32).
    pub fn read_aerial_perspective(&self, ctx: &GpuContext) -> Vec<[f32; 4]> {
        read_f16_texture(ctx, &self.ap_volume, (AP_W as u32, AP_H as u32, AP_D as u32), 4)
    }

    /// The scene target as RGBA f32.
    pub fn read_scene(&self, ctx: &GpuContext) -> Vec<[f32; 4]> {
        read_f16_texture(ctx, &self.scene, (self.size.0, self.size.1, 1), 4)
    }
}

mod planet_render_sizes {
    pub use crate::graph::{AERIAL_PERSPECTIVE_BYTES, MULTISCATTER_LUT_BYTES, SKY_VIEW_BYTES, TRANSMITTANCE_LUT_BYTES};
}

/// Read an `Rgba16Float` texture (2D or 3D) back as f32 RGBA.
fn read_f16_texture(ctx: &GpuContext, texture: &wgpu::Texture, extent: (u32, u32, u32), _channels: usize) -> Vec<[f32; 4]> {
    let (w, h, d) = extent;
    let unpadded = w * 8;
    let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("atmosphere-readback"),
        size: u64::from(padded) * u64::from(h) * u64::from(d),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("atmosphere-readback") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: d },
    );
    ctx.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
    ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    let data = slice.get_mapped_range().expect("readback range");
    let mut out = Vec::with_capacity((w * h * d) as usize);
    for row in data.chunks_exact(padded as usize) {
        for px in row[..unpadded as usize].as_chunks::<8>().0 {
            let c = |i: usize| crate::lit::f16_bits_to_f32(u16::from_le_bytes([px[2 * i], px[2 * i + 1]]));
            out.push([c(0), c(1), c(2), c(3)]);
        }
    }
    drop(data);
    readback.unmap();
    out
}

// ---------------------------------------------------------------------------------------------------------------------------
// Probes: the shader's own functions on given inputs (tier B oracles)
// ---------------------------------------------------------------------------------------------------------------------------

const PROBE: &str = r#"
@group(0) @binding(0) var<uniform> A: AtmoBlock;
@group(0) @binding(1) var tr_lut: texture_2d<f32>;
@group(0) @binding(2) var ms_lut: texture_2d<f32>;
@group(0) @binding(8) var<storage, read> queries: array<vec4<f32>>;
@group(0) @binding(9) var<storage, read_write> results: array<vec4<f32>>;
"#;

const PROBE_MAIN: &str = r#"
@compute @workgroup_size(1)
fn probe_sky(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    results[id.x] = vec4<f32>(atm_sky_texel(i32(q.x), i32(q.y)), 1.0);
}

@compute @workgroup_size(1)
fn probe_ap(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    results[id.x] = atm_ap_texel(i32(q.x), i32(q.y), i32(q.z));
}

@compute @workgroup_size(1)
fn probe_orbit(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    let o = atm_orbit_ray(q.xyz, q.w);
    results[2u * id.x] = vec4<f32>(o.radiance, o.flags.x);
    results[2u * id.x + 1u] = vec4<f32>(o.transmittance, o.flags.y);
}

@compute @workgroup_size(1)
fn probe_orbit_mu(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    let o = atm_orbit_core(q.x, q.y, q.z);
    results[2u * id.x] = vec4<f32>(o.radiance, o.flags.x);
    results[2u * id.x + 1u] = vec4<f32>(o.transmittance, o.flags.y);
}

@compute @workgroup_size(1)
fn probe_sky_coords(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    results[id.x] = vec4<f32>(atm_sky_coords(q.xyz), 0.0, 0.0);
}

@compute @workgroup_size(1)
fn probe_transmittance(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    results[id.x] = vec4<f32>(atm_transmittance(q.x, q.y), atm_planet_shadow(q.x, q.y));
}

@compute @workgroup_size(1)
fn probe_lighting(@builtin(global_invocation_id) id: vec3<u32>) {
    let n = queries[3u * id.x];
    let u = queries[3u * id.x + 1u];
    let s = queries[3u * id.x + 2u];
    results[id.x] = vec4<f32>(atm_terrain_lighting(n.xyz, u.xyz, n.w, s.xyz), 1.0);
}

@compute @workgroup_size(1)
fn probe_disc(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    results[id.x] = vec4<f32>(atm_sun_disc(q.xyz) * (ATM_PI * A.body.w * A.body.w), 0.0, 0.0, 1.0);
}

@compute @workgroup_size(1)
fn probe_ground(@builtin(global_invocation_id) id: vec3<u32>) {
    let q = queries[id.x];
    results[id.x] = vec4<f32>(atm_ground_radiance(q.y), 1.0);
}
"#;

/// Which function of the atmosphere shader a probe runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeKind {
    /// `atm_sky_texel(ix, iy)` -> radiance.
    Sky,
    /// `atm_ap_texel(ix, iy, iz)` -> (radiance, mean transmittance).
    AerialPerspective,
    /// `atm_orbit_ray(dir, t_geo)` -> two vec4: (radiance, reaches the ground) and (transmittance, crosses).
    Orbit,
    /// `atm_orbit_core(mu, cos_vs, t_geo)`: the same from the cosines (query `(mu, cos_vs, t_geo, _)`).
    OrbitCosines,
    /// `atm_sky_coords(dir)` -> (u, v).
    SkyCoords,
    /// `atm_transmittance(h, mu)` and `atm_planet_shadow(h, mu)` -> (transmittance, shadow).
    Transmittance,
    /// `atm_ground_radiance(mu_sun)` -> radiance.
    Ground,
    /// `atm_sun_disc(dir)` times the disc's solid angle -> the pixel's coverage of the disc.
    SunDisc,
    /// `atm_terrain_lighting(normal, up, h, sun)`: three queries `(normal, h)`, `(up)`, `(sun)` per probe -> lighting.
    TerrainLighting,
}

impl ProbeKind {
    fn entry(self) -> &'static str {
        match self {
            ProbeKind::Sky => "probe_sky",
            ProbeKind::AerialPerspective => "probe_ap",
            ProbeKind::Orbit => "probe_orbit",
            ProbeKind::OrbitCosines => "probe_orbit_mu",
            ProbeKind::SkyCoords => "probe_sky_coords",
            ProbeKind::Transmittance => "probe_transmittance",
            ProbeKind::Ground => "probe_ground",
            ProbeKind::SunDisc => "probe_disc",
            ProbeKind::TerrainLighting => "probe_lighting",
        }
    }

    fn results_per_query(self) -> u64 {
        if matches!(self, ProbeKind::Orbit | ProbeKind::OrbitCosines) {
            2
        } else {
            1
        }
    }

    fn query_stride(self) -> usize {
        if self == ProbeKind::TerrainLighting {
            3
        } else {
            1
        }
    }
}

impl AtmospherePath {
    /// Run a probe of the shader's functions over `queries` (one `vec4` each) with the frame written by
    /// [`write_frame`](Self::write_frame) and the uploaded tables, and read back the results (two `vec4` per query for `Orbit`).
    pub fn probe(&self, ctx: &GpuContext, kind: ProbeKind, queries: &[[f32; 4]]) -> Vec<[f32; 4]> {
        use wgpu::util::DeviceExt;
        let device = &ctx.device;
        let bytes: Vec<u8> = queries.iter().flat_map(|q| q.iter().flat_map(|v| v.to_le_bytes())).collect();
        let query_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("atmosphere-probe-queries"),
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let count = queries.len() / kind.query_stride();
        let out_bytes = count as u64 * 16 * kind.results_per_query();
        let results = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("atmosphere-probe-results"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let source = format!("{PROBE}{DECLS}{MARCH}{LIGHTING}{PROBE_MAIN}");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("atmosphere-probe"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let storage = |binding, read_only| {
            entry(
                binding,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )
        };
        let texture = |binding| {
            entry(
                binding,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            )
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere-probe"),
            entries: &[
                entry(
                    0,
                    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                ),
                texture(1),
                texture(2),
                storage(8, true),
                storage(9, false),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("atmosphere-probe"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("atmosphere-probe"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some(kind.entry()),
            compilation_options: Default::default(),
            cache: None,
        });
        let (tr_view, ms_view) = (self.tr_lut.create_view(&Default::default()), self.ms_lut.create_view(&Default::default()));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere-probe"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tr_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&ms_view) },
                wgpu::BindGroupEntry { binding: 8, resource: query_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 9, resource: results.as_entire_binding() },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("atmosphere-probe") });
        {
            let mut pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("atmosphere-probe"), timestamp_writes: None });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(count as u32, 1, 1);
        }
        ctx.queue.submit([encoder.finish()]);
        let data = crate::lit::read_buffer_bytes(ctx, &results, out_bytes);
        data.as_chunks::<16>()
            .0
            .iter()
            .map(|c| {
                let f = |i: usize| f32::from_le_bytes([c[4 * i], c[4 * i + 1], c[4 * i + 2], c[4 * i + 3]]);
                [f(0), f(1), f(2), f(3)]
            })
            .collect()
    }
}
