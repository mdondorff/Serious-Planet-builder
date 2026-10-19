//! The water pass (ADR 0041 points 4 to 6, ADR 0042 point 2 pass 4, ADR 0053): the terrain's own grid drawn again at the water level,
//! with Gerstner waves, the shoreline from the vertical depth `W - h_morph` and the lit surface (absorption, Fresnel, sun glint,
//! emission). The renderer executes the plan's `WaterPlan` and decides nothing (CON-03): which nodes carry water, their level and liquid,
//! the reduced wave phases and the look all arrive in the plan; the f64 reference of every function below is
//! `planet_frame::look::water`.
//!
//! Class and bound (ADR 0035, ADR 0043): presentation data. Wave displacement and slope match the reference within
//! `1 mm + 1e-3 x (sum of the amplitudes)`; the surface's radiance within `max(1 % |v|, 1e-4)` per channel (ATMO-014). Geometry stays
//! tile-local (CON-11): a water vertex is the terrain vertex's tile-local position plus `up x (W - h_morph)` plus the wave
//! displacement, `up` formed from the node's tile-local numbers; no planet-sized value reaches the shader, and the waves' phases are
//! reduced on the CPU in f64 (the node record holds `(k . O + phi - omega t) mod 2 pi`, the shader adds `k . local`).
//!
//! Bindings. Group 0 is the terrain's (globals, node records) plus binding 2, the water node records, and binding 3, the water globals
//! (look values, the debug view and the liquids). Group 3 is the water pass's own: binding 4 is the copy of the scene target the
//! terrain pass wrote (the water reads the seabed through itself, so it needs a copy: a pass cannot sample the target it draws into);
//! with an atmosphere, bindings 0 to 3 are the terrain's atmosphere group (the uniform block, the two static tables) and the sky-view
//! table, which the surface reflects.

use planet_frame::look::water::{WaterClass, GLOBALS_BYTES, LIQUID_VEC4S, MAX_LIQUIDS, NODE_RECORD_BYTES};
use planet_frame::water::WaterPlan;

/// The pure maths of the water surface: waves and shading, free of the terrain's declarations (the probes compile it alone).
pub const WATER_MATH: &str = r#"
const W_PI: f32 = 3.14159265358979;
const W_TWO_PI: f32 = 6.28318530717959;

struct WLiquid {
    // rgb: the colour in deep water; rgb: the extinction per metre; rgb: the emitted radiance; then the four waves: xyz the wave vector
    // in rad/m (planet-fixed axes), w the amplitude in m.
    colour: vec4<f32>,
    sigma: vec4<f32>,
    emission: vec4<f32>,
    wave: array<vec4<f32>, 4>,
};

struct WaveOut {
    disp: vec3<f32>,
    slope: vec3<f32>,
};

fn w_smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// The argument of wave `kw` at a tile-local point, brought to [-pi, pi]: the tile's reduced phase plus k . local (a small number).
fn water_wave_arg(kw: vec4<f32>, local: vec3<f32>, phase: f32) -> f32 {
    let a = dot(kw.xyz, local) + phase;
    return a - W_TWO_PI * floor(a / W_TWO_PI + 0.5);
}

// Gerstner displacement (planet-fixed axes, metres) and slope (the gradient along the tangent plane) of a liquid at a tile-local point.
// A wave's weight is the product of its tangent share (a wave that points out of the surface has none), its resolution (it fades out
// at the vertex's own spacing, from the plan and the morph) and the shore (the amplitude falls to zero with the depth), each continuous.
fn water_waves(liq: WLiquid, local: vec3<f32>, up: vec3<f32>, phases: vec4<f32>, spacing: f32, depth: f32, choppy: f32, footprint: f32) -> WaveOut {
    var out: WaveOut;
    out.disp = vec3<f32>(0.0);
    out.slope = vec3<f32>(0.0);
    var total = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        total = total + liq.wave[i].w;
    }
    if (total <= 0.0) {
        return out;
    }
    let shore = w_smooth(0.0, 4.0 * total, depth);
    for (var i = 0; i < 4; i = i + 1) {
        let kw = liq.wave[i];
        if (kw.w <= 0.0) {
            continue;
        }
        let klen = length(kw.xyz);
        let khat = kw.xyz / klen;
        let kn = dot(khat, up);
        let tang = khat - up * kn;
        let s = length(tang);
        let w_tan = w_smooth(0.25, 0.6, s);
        let w_res = w_smooth(4.0, 8.0, (W_TWO_PI / klen) / spacing);
        // A wave under about two pixels per wavelength fades out of the shading (footprint 0: the vertex stage, no fade).
        var w_px = 1.0;
        if (footprint > 0.0) {
            w_px = w_smooth(2.0, 4.0, (W_TWO_PI / klen) / footprint);
        }
        let w = w_tan * w_res * w_px * shore;
        if (w <= 0.0) {
            continue;
        }
        let dir = tang / max(s, 1.0e-6);
        let arg = water_wave_arg(kw, local, phases[i]);
        let aw = kw.w * w;
        let sn = sin(arg);
        out.disp = out.disp + up * (aw * cos(arg)) - dir * (choppy * aw * sn);
        out.slope = out.slope - dir * (aw * klen * s * sn);
    }
    return out;
}

// The radiance leaving the surface towards the viewer, before the pre-exposure: the seabed seen through the extinction and the
// liquid body's own colour lit by the same light, the Fresnel-weighted sky and the GGX sun glint, the emission, all faded in from the
// shore (`la.x`), then the foam band (`la.y` wide, `la.z` dense). `la` = (shore fade, foam width, foam strength, choppy);
// `lb` = (Fresnel F0, glint roughness, the filter width of the shore bands in metres of depth, 0).  The filter (ADR 0041 point 6)
// stretches the bands' edges by that width so a band narrower than a pixel fades instead of coming and going.
fn water_surface(liq: WLiquid, la: vec4<f32>, lb: vec4<f32>, n: vec3<f32>, v: vec3<f32>, up: vec3<f32>, sun: vec3<f32>, depth: f32,
                 foam_mod: f32, seabed: vec3<f32>, diffuse: vec3<f32>, sun_t: vec3<f32>, sky: vec3<f32>) -> vec3<f32> {
    let mu_v = clamp(dot(n, v), 0.0, 1.0);
    let path_mu = max(dot(up, v), 0.1);
    let p = depth / path_mu;
    let t = exp(-liq.sigma.xyz * p);
    let t_mean = (t.x + t.y + t.z) / 3.0;
    let om = 1.0 - mu_v;
    let o2 = om * om;
    let fresnel = lb.x + (1.0 - lb.x) * (o2 * o2 * om);
    let h = normalize(sun + v);
    let nh = max(dot(n, h), 0.0);
    let nl = max(dot(n, sun), 0.0);
    let al = lb.y * lb.y;
    let nxh = cross(n, h);
    let denom = dot(nxh, nxh) + al * al * nh * nh;
    let d = al * al / (W_PI * denom * denom);
    let spec = fresnel * d / (4.0 * max(mu_v, 0.05)) * min(nl * 20.0, 1.0);
    let cover = w_smooth(0.0, la.x + 2.0 * lb.z, depth);
    let f_in = w_smooth(0.0, 0.15 * la.y + 2.0 * lb.z, depth);
    let f_out = 1.0 - w_smooth(0.15 * la.y - lb.z, la.y + lb.z, depth);
    let foam = la.z * f_in * f_out * (0.6 + 0.4 * foam_mod);
    let body = liq.colour.xyz * diffuse;
    let refracted = seabed * t + body * (vec3<f32>(1.0) - t);
    let water = refracted * (1.0 - fresnel) + fresnel * sky + spec * sun_t + liq.emission.xyz * (1.0 - t_mean);
    let surf = seabed * (1.0 - cover) + water * cover;
    return surf * (1.0 - foam) + (0.9 * diffuse) * foam;
}
"#;

/// Declarations and the stages common to both tile formats and both lightings. They follow the terrain shader's own (`g`, `nodes`,
/// `VsOut`, `up_at`).
const WATER_GLUE: &str = r#"
struct WaterNodeRec {
    // x: the water level W, y: the liquid slot (as a float), z: the tile's sample spacing in metres, w: reserved.
    a: vec4<f32>,
    phases: vec4<f32>,
};

struct WaterGlobals {
    // x: shore fade, y: foam width, z: foam strength, w: choppy.
    a: vec4<f32>,
    // x: Fresnel F0, y: glint roughness, z: the shore filter strength (a multiple of fwidth of the depth), w: reserved.
    b: vec4<f32>,
    // x: debug view (0 lit, 1 class, 2 waves, 3 normals), y: liquid count.
    c: vec4<u32>,
    liquids: array<WLiquid, 4>,
};

@group(0) @binding(2) var<storage, read> water_nodes: array<WaterNodeRec>;
@group(0) @binding(3) var<uniform> WG: WaterGlobals;

struct WOut {
    @builtin(position) pos: vec4<f32>,
    // The undisplaced surface point relative to the tile origin: small, tile-local.
    @location(0) local: vec3<f32>,
    // The vertical depth W - h_morph at the point (linear in the triangle, so the shoreline is where it crosses zero).
    @location(1) depth: f32,
    @location(2) @interpolate(flat) node_index: u32,
    // The vertex's effective grid spacing (tile spacing times one plus the morph), interpolated: the waves' resolution weight.
    @location(3) spacing: f32,
};

// A water vertex from the terrain vertex: the same grid, LOD and morph; the height is the terrain's morphed height, so water and
// terrain agree across LOD. Position = terrain-local position + up x (W - h_morph) + the wave displacement; all tile-local (CON-11).
fn water_vertex(t: VsOut) -> WOut {
    let node = nodes[t.node_index];
    let rec = water_nodes[t.node_index];
    let liq = WG.liquids[u32(rec.a.y)];
    let depth = rec.a.x - t.height;
    let up = up_at(node.up, t.local);
    let base = t.local + up * depth;
    let spacing = rec.a.z * (1.0 + t.morph);
    let w = water_waves(liq, base, up, rec.phases, spacing, depth, WG.a.w, 0.0);
    var out: WOut;
    out.pos = g.view_proj * vec4<f32>(node.origin_rel.xyz + base + w.disp, 1.0);
    out.local = base;
    out.depth = depth;
    out.spacing = spacing;
    out.node_index = t.node_index;
    return out;
}

struct WLight {
    diffuse: vec3<f32>,
    sun_t: vec3<f32>,
    sky: vec3<f32>,
};

@fragment
fn fs_water(in: WOut) -> @location(0) vec4<f32> {
    // The derivative first, in uniform control flow (before the discard).
    let depth_width = WG.b.z * fwidth(in.depth);
    let footprint = max(length(dpdx(in.local)), length(dpdy(in.local)));
    if (in.depth <= 0.0) {
        discard;
    }
    let node = nodes[in.node_index];
    let rec = water_nodes[in.node_index];
    let liq = WG.liquids[u32(rec.a.y)];
    let up = up_at(node.up, in.local);
    let cam = node.origin_rel.xyz + in.local;
    let dist = length(cam);
    let w = water_waves(liq, in.local, up, rec.phases, in.spacing, in.depth, WG.a.w, footprint);
    let n = normalize(up - w.slope);
    let v = -cam / dist;
    let sun = vec3<f32>(bitcast<f32>(g.mode.y), bitcast<f32>(g.mode.z), bitcast<f32>(g.mode.w));
    let factor = g.range.z;
    let mode = WG.c.x;
    if (mode == 1u) {
        var cls = 5;
        if (in.depth < WG.a.x) {
            cls = 1;
        } else if (in.depth < WG.a.y) {
            cls = 2;
        } else if (in.depth < {CLASS_SHALLOW}) {
            cls = 3;
        } else if (in.depth < {CLASS_MID}) {
            cls = 4;
        }
        return vec4<f32>(WATER_CLASS_COLOURS[cls] * (0.31830988618 * factor), 1.0);
    }
    if (mode == 2u) {
        let total = liq.wave[0].w + liq.wave[1].w + liq.wave[2].w + liq.wave[3].w;
        let lvl = floor(clamp(0.5 + 0.5 * dot(w.disp, up) / max(total, 1.0e-6), 0.0, 0.999) * 8.0) / 8.0;
        return vec4<f32>(vec3<f32>(lvl * 0.8) * (0.31830988618 * factor), 1.0);
    }
    if (mode == 3u) {
        return vec4<f32>((n * 0.5 + vec3<f32>(0.5)) * (0.31830988618 * factor), 1.0);
    }
    let light = water_light(n, up, rec.a.x, sun, reflect(-v, n));
    let seabed = textureLoad(scene_copy, vec2<i32>(in.pos.xy), 0).rgb / factor;
    let foam_mod = 0.5 + 0.5 * cos(water_wave_arg(liq.wave[0], in.local, rec.phases.x));
    let rgb = water_surface(liq, WG.a, vec4<f32>(WG.b.x, WG.b.y, depth_width, 0.0), n, v, up, sun, in.depth, foam_mod, seabed, light.diffuse, light.sun_t, light.sky);
    return vec4<f32>(clamp(rgb * factor, vec3<f32>(0.0), vec3<f32>(60000.0)), 1.0);
}
"#;

/// The lighting of a frame without an atmosphere: the terrain's Lambert (no ambient, no sky), the sun at full strength.
const WATER_PLAIN_LIGHT: &str = r#"
@group(3) @binding(4) var scene_copy: texture_2d<f32>;

fn water_light(n: vec3<f32>, up: vec3<f32>, h_alt: f32, sun: vec3<f32>, refl: vec3<f32>) -> WLight {
    var l: WLight;
    l.diffuse = vec3<f32>(max(dot(n, sun), 0.0) * 0.31830988618);
    l.sun_t = vec3<f32>(1.0);
    l.sky = vec3<f32>(0.0);
    return l;
}
"#;

/// The lighting of a frame with an atmosphere: the terrain's own lighting function, the sun's transmitted and shadowed light for the
/// glint, and the sky reflected in the mirror direction from the sky-view table (from orbit, where the table is not defined, the
/// multiple-scattering ambient stands in).
const WATER_ATMOSPHERE_LIGHT: &str = r#"
@group(3) @binding(3) var sky_lut: texture_2d<f32>;
@group(3) @binding(4) var scene_copy: texture_2d<f32>;

fn water_light(n: vec3<f32>, up: vec3<f32>, h_alt: f32, sun: vec3<f32>, refl: vec3<f32>) -> WLight {
    var l: WLight;
    l.diffuse = atm_terrain_lighting(n, up, h_alt, sun);
    let mu_s = clamp(dot(up, sun), -1.0, 1.0);
    let h = clamp(h_alt, 0.0, A.body.y);
    l.sun_t = atm_transmittance(h, mu_s) * atm_planet_shadow(h, mu_s);
    // A mirror direction below the horizon (a steep wave seen at a grazing angle) is folded up.
    let rd = refl - 2.0 * min(dot(refl, up), 0.0) * up;
    if (A.misc.y > 0.5) {
        let uv = atm_sky_coords(rd);
        l.sky = atm_bilerp(sky_lut, SKY_W, SKY_H, vec2<f32>(uv.x * f32(SKY_W - 1), uv.y * f32(SKY_H - 1)));
    } else {
        l.sky = atm_multiscatter(h, mu_s) * 0.5;
    }
    return l;
}
"#;

const ENTRY_COMPACT: &str = r#"
@vertex
fn vs_water(@builtin(instance_index) inst: u32, @builtin(vertex_index) vi: u32) -> WOut {
    return water_vertex(terrain_vertex(inst, vi));
}
"#;

/// The class colours as a WGSL constant table (the debug view of the shoreline), from the reference's palette.
fn class_table() -> String {
    let c: Vec<String> =
        WaterClass::COLOURS.iter().map(|c| format!("vec3<f32>({:?}, {:?}, {:?})", c[0] as f32, c[1] as f32, c[2] as f32)).collect();
    format!("const WATER_CLASS_COLOURS = array<vec3<f32>, 6>(\n    {}\n);\n", c.join(",\n    "))
}

/// The water module for the compact format of tiles of `cells`; with `atmosphere` the lighting reads the atmosphere's tables.
pub fn water_compact_source(cells: u32, atmosphere: bool) -> String {
    assemble(crate::compact::compact_terrain_shader(cells), ENTRY_COMPACT, atmosphere)
}

fn assemble(terrain: String, entry: &str, atmosphere: bool) -> String {
    let lighting = if atmosphere { WATER_ATMOSPHERE_LIGHT } else { WATER_PLAIN_LIGHT };
    let atmo = if atmosphere { crate::atmosphere::water_atmosphere_source() } else { String::new() };
    let glue = WATER_GLUE
        .replace("{CLASS_SHALLOW}", &format!("{:?}", planet_frame::look::water::CLASS_SHALLOW_M as f32))
        .replace("{CLASS_MID}", &format!("{:?}", planet_frame::look::water::CLASS_MID_M as f32));
    format!("{terrain}{atmo}{WATER_MATH}{}{glue}{lighting}{entry}", class_table())
}

/// The water shaders by name, for the validation and scan tests.
pub fn water_shaders() -> Vec<(&'static str, String)> {
    vec![
        ("water-math", format!("{WATER_MATH}\n@compute @workgroup_size(1) fn noop() {{}}")),
        ("water-compact", water_compact_source(32, false)),
        ("water-compact-atmosphere", water_compact_source(32, true)),
    ]
}

/// Bytes of the water node records for `nodes` terrain nodes.
pub fn node_buffer_bytes(nodes: usize) -> u64 {
    NODE_RECORD_BYTES * nodes.max(1) as u64
}

/// The node records of a frame, dense over the terrain plan's `node_count` nodes (the water draws with the terrain's instance index);
/// nodes without water hold zeros and are never drawn.
pub fn node_records(plan: &WaterPlan, node_count: usize) -> Vec<u8> {
    let mut out = vec![0u8; node_buffer_bytes(node_count) as usize];
    for n in &plan.nodes {
        let o = n.node as usize * NODE_RECORD_BYTES as usize;
        let words = [n.level_m, f32::from(n.liquid), n.spacing_m, 0.0, n.phases[0], n.phases[1], n.phases[2], n.phases[3]];
        for (k, w) in words.iter().enumerate() {
            out[o + 4 * k..o + 4 * k + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    out
}

/// The water globals of a frame ([`GLOBALS_BYTES`]): the look, the debug view and the liquids, each value rounded once to f32.
pub fn globals_bytes(plan: &WaterPlan) -> Vec<u8> {
    let l = &plan.look;
    let mut out: Vec<u8> = Vec::with_capacity(GLOBALS_BYTES as usize);
    let f = |out: &mut Vec<u8>, v: &[f32]| out.extend(v.iter().flat_map(|x| x.to_le_bytes()));
    f(&mut out, &[l.shore_fade_m as f32, l.foam_width_m as f32, l.foam_strength as f32, l.choppy as f32]);
    f(&mut out, &[l.fresnel_f0 as f32, l.glint_roughness as f32, l.shore_filter as f32, 0.0]);
    out.extend([plan.view.index(), plan.liquids.len() as u32, 0, 0].iter().flat_map(|x| x.to_le_bytes()));
    for k in 0..MAX_LIQUIDS {
        match plan.liquids.get(k) {
            Some(q) => f(&mut out, &q.to_words()),
            None => f(&mut out, &[0.0; LIQUID_VEC4S * 4]),
        }
    }
    debug_assert_eq!(out.len() as u64, GLOBALS_BYTES);
    out
}
