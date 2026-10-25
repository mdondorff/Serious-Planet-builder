//! The WGSL of the horizon shadows (REND-038), as macros that expand to string literals so `concat!` can put them into the terrain shader's
//! head and tail and into the compute probe of the tests: the same text runs in both.

/// The bindings (group 0, 4 to 7): per node two `vec4` [slot (-1: none), sun azimuth, sun elevation, fade] and [u0, v0, du, dv] (indexed by the
/// plan's node index like `nodes`); the chunk horizon maps (four layers a slot, four azimuths a texel); the coarse map (six faces, four layers
/// each); a linear clamp sampler.
macro_rules! horizon_decls {
    () => {
        r#"
@group(0) @binding(4) var<storage, read> horizon_nodes: array<vec4<f32>>;
@group(0) @binding(5) var horizon_chunks: texture_2d_array<f32>;
@group(0) @binding(6) var horizon_coarse: texture_2d_array<f32>;
@group(0) @binding(7) var horizon_sampler: sampler;
"#
    };
}

/// The functions: the horizon at a point and the shadow factor (GEN-046's `horizon_lookup` is the CPU reference). The sun and `up` are unit
/// vectors in the planet-fixed axes, `up` formed from the node's tile-local numbers (CON-11): no planet-radius value is used. The azimuths are 16
/// evenly spaced (one texel holds four, so four layers), the byte `q` stands for `q / 255` of 90 degrees (`planet_generators::horizon::dequantize`).
macro_rules! horizon_fns {
    () => {
        r#"
const HORIZON_AZIMUTHS: u32 = 16u;
const HORIZON_INV_TAU: f32 = 0.15915494309189535;
const HORIZON_HALF_PI: f32 = 1.5707963267948966;
const HORIZON_QUARTER_PI: f32 = 0.7853981633974483;
// The soft edge in radians of sun elevation over the horizon (GEN-046): fully in shadow 0.35 degrees below, fully lit 0.9 degrees above.
const HORIZON_SOFT_BELOW: f32 = 0.006108652381980153;
const HORIZON_SOFT_ABOVE: f32 = 0.015707963267948967;

// The horizon elevation in radians at azimuth `az` (radians, any real number) read from the four layers of `maps` starting at `layer0`, at the
// texel position `uv` in [0, 1]^2: linear between the two nearest azimuths, wrapping from the last to the first.
fn horizon_elevation_of(maps: texture_2d_array<f32>, uv: vec2<f32>, layer0: u32, az: f32) -> f32 {
    let x = fract(az * HORIZON_INV_TAU) * f32(HORIZON_AZIMUTHS);
    let k0 = min(u32(floor(x)), HORIZON_AZIMUTHS - 1u);
    let f = x - f32(k0);
    let k1 = (k0 + 1u) % HORIZON_AZIMUTHS;
    let a = textureSampleLevel(maps, horizon_sampler, uv, layer0 + k0 / 4u, 0.0);
    let b = textureSampleLevel(maps, horizon_sampler, uv, layer0 + k1 / 4u, 0.0);
    return mix(a[k0 % 4u], b[k1 % 4u], f) * HORIZON_HALF_PI;
}

// The cube face of a direction and its plane coordinates, the way `planet_core::cube::direction_to_face` does it (ties to X, then Y, then Z; the
// tangent warp). Returns (face, s, t) with s and t in [-1, 1].
fn horizon_face_of(d: vec3<f32>) -> vec3<f32> {
    let a = abs(d);
    var face = 4u;
    var w = d.z;
    var u = d.x;
    var v = d.y;
    if (a.x >= a.y && a.x >= a.z) {
        if (d.x >= 0.0) { face = 0u; w = d.x; u = d.y; v = d.z; } else { face = 1u; w = -d.x; u = -d.y; v = d.z; }
    } else if (a.y >= a.z) {
        if (d.y >= 0.0) { face = 2u; w = d.y; u = -d.x; v = d.z; } else { face = 3u; w = -d.y; u = d.x; v = d.z; }
    } else if (d.z >= 0.0) {
        face = 4u; w = d.z; u = d.x; v = d.y;
    } else {
        face = 5u; w = -d.z; u = d.x; v = -d.y;
    }
    return vec3<f32>(f32(face), atan(u / w) / HORIZON_QUARTER_PI, atan(v / w) / HORIZON_QUARTER_PI);
}

// The coarse map's horizon at the sun's azimuth in the geographic frame (north toward +z); 0 where there is no coarse map (a 1 x 1 dummy).
fn horizon_coarse_elevation(up: vec3<f32>, sun: vec3<f32>) -> f32 {
    if (textureDimensions(horizon_coarse).x <= 1u) {
        return 0.0;
    }
    var east = cross(vec3<f32>(0.0, 0.0, 1.0), up);
    if (length(east) < 1e-9) {
        east = cross(vec3<f32>(0.0, 1.0, 0.0), up);
    }
    east = normalize(east);
    let north = normalize(cross(up, east));
    let pe = dot(sun, east);
    let pn = dot(sun, north);
    // A sun straight overhead has no horizontal direction (atan2(0, 0) is undefined in WGSL); the CPU reference gives 0.
    let az = select(0.0, atan2(pe, pn), abs(pe) + abs(pn) > 0.0);
    let f = horizon_face_of(up);
    let uv = vec2<f32>(0.5 * (f.y + 1.0), 0.5 * (f.z + 1.0));
    return horizon_elevation_of(horizon_coarse, uv, u32(f.x) * 4u, az);
}

// The horizon of a fragment: x = the larger of the chunk's (times the node's fade) and the coarse map's elevation, y = 1 when some map is there,
// else 0 (the horizon is then the sky's own and the shadow term the identity). `tile_uv` is the fragment's tile coordinates.
fn horizon_at(node_index: u32, tile_uv: vec2<f32>, up: vec3<f32>, sun: vec3<f32>) -> vec2<f32> {
    let e0 = horizon_nodes[2u * node_index];
    let e1 = horizon_nodes[2u * node_index + 1u];
    var h = horizon_coarse_elevation(up, sun);
    var present = select(0.0, 1.0, textureDimensions(horizon_coarse).x > 1u);
    if (e0.x >= 0.0) {
        let uv = e1.xy + e1.zw * clamp(tile_uv, vec2<f32>(0.0), vec2<f32>(1.0));
        h = max(h, horizon_elevation_of(horizon_chunks, uv, u32(e0.x) * 4u, e0.y) * e0.w);
        present = 1.0;
    }
    return vec2<f32>(h, present);
}

// The lit factor of the sun over a horizon `h` (GEN-046): 0 in shadow, 1 lit, a smoothstep between.
fn horizon_lit(sun: vec3<f32>, up: vec3<f32>, h: f32) -> f32 {
    let el = asin(clamp(dot(sun, up), -1.0, 1.0));
    let x = clamp((el - h + HORIZON_SOFT_BELOW) / (HORIZON_SOFT_BELOW + HORIZON_SOFT_ABOVE), 0.0, 1.0);
    return x * x * (3.0 - 2.0 * x);
}

// The factor the direct sun term of a lit fragment is multiplied by (REND-038): 1.0 for a node with no chunk map when there is no coarse map.
fn shadow_factor(node_index: u32, tile_uv: vec2<f32>, up: vec3<f32>, sun: vec3<f32>) -> f32 {
    let h = horizon_at(node_index, tile_uv, up, sun);
    if (h.y < 0.5) {
        return 1.0;
    }
    return horizon_lit(sun, up, h.x);
}
"#
    };
}
