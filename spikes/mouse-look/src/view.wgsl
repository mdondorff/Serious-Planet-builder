// SPIKE-11 viewer: a small checkered planet (radius in u.params.z) seen from a free camera. Throwaway.
struct U {
    pos: vec4<f32>,      // camera position relative to the planet centre
    right: vec4<f32>,
    up: vec4<f32>,
    fwd: vec4<f32>,
    params: vec4<f32>,   // tan(fovy/2), aspect, radius, time
    size: vec4<f32>,     // width, height
};
@group(0) @binding(0) var<uniform> u: U;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

const PI: f32 = 3.14159265;

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = vec2<f32>((frag.x / u.size.x) * 2.0 - 1.0, 1.0 - (frag.y / u.size.y) * 2.0);
    let d = normalize(u.fwd.xyz + u.right.xyz * (uv.x * u.params.y * u.params.x) + u.up.xyz * (uv.y * u.params.x));
    let o = u.pos.xyz;
    let r = u.params.z;
    let local_up = normalize(o);
    var col: vec3<f32>;
    let b = dot(o, d);
    let c = dot(o, o) - r * r;
    let disc = b * b - c;
    var hit = false;
    var t = 0.0;
    if (disc > 0.0) {
        t = -b - sqrt(disc);
        hit = t > 0.0;
    }
    if (hit) {
        let p = o + d * t;
        let n = p / r;
        let lat = asin(clamp(n.z, -1.0, 1.0));
        let lon = atan2(n.y, n.x);
        let coarse = PI / 18.0;
        let fine = PI / 720.0;
        let a = i32(floor(lat / coarse)) + i32(floor(lon / coarse));
        var base = select(vec3<f32>(0.20, 0.45, 0.20), vec3<f32>(0.30, 0.60, 0.28), (a & 1) == 0);
        let f = i32(floor(lat / fine)) + i32(floor(lon / fine));
        if (t < 300.0) {
            base = base * select(0.82, 1.0, (f & 1) == 0);
        }
        if (lat > 1.40) { base = vec3<f32>(0.95, 0.95, 1.0); }
        if (lat < -1.40) { base = vec3<f32>(0.85, 0.2, 0.2); }
        if (abs(lon) < 0.004 && lat > 0.0) { base = vec3<f32>(1.0, 0.55, 0.1); }
        col = base;
    } else {
        let s = clamp(dot(d, local_up), -0.2, 1.0);
        col = mix(vec3<f32>(0.80, 0.85, 0.92), vec3<f32>(0.12, 0.32, 0.75), pow(max(s, 0.0), 0.5));
    }
    let dx = abs(frag.x - u.size.x * 0.5);
    let dy = abs(frag.y - u.size.y * 0.5);
    if ((dx < 1.0 && dy < 9.0) || (dy < 1.0 && dx < 9.0)) {
        col = vec3<f32>(1.0) - col;
    }
    return vec4<f32>(col, 1.0);
}
