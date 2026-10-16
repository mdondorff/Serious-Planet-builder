// SPIKE-09: a compact single-scattering ray march in two geometry forms (mode 0 naive f32, mode 1 altitude-relative primitives
// of ADR 0040 point 5). Throwaway.
const R: f32 = 6371000.0;
const H: f32 = 100000.0;
const RT: f32 = 6471000.0;
const PI: f32 = 3.14159265358979;
const N_VIEW: i32 = 24;
const N_SUN: i32 = 6;
const BETA_R: vec3<f32> = vec3<f32>(5.802e-6, 13.558e-6, 33.1e-6);
const BETA_M_S: f32 = 3.996e-6;
const BETA_M_E: f32 = 4.44e-6;
const SCALE_R: f32 = 8000.0;
const SCALE_M: f32 = 1200.0;
const G: f32 = 0.8;
const ALBEDO: f32 = 0.3;

struct Ray { h: f32, r: f32, c: f32, ctop: f32, mu: f32, ms: f32, cosg: f32, pad: f32 }
struct Params { mode: u32, count: u32, p0: u32, p1: u32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> rays: array<Ray>;
@group(0) @binding(2) var<storage, read_write> outp: array<vec4<f32>>;

struct Geo { r: f32, h: f32, c: f32 }

fn naive() -> bool { return params.mode == 0u; }

fn geo_at(b: Geo, mu: f32, s: f32) -> Geo {
    let r2 = sqrt(b.r * b.r + s * s + 2.0 * b.r * mu * s);
    if (naive()) {
        return Geo(r2, r2 - R, r2 * r2 - R * R);
    }
    let hs = (b.c + s * (s + 2.0 * b.r * mu)) / (r2 + R);
    return Geo(r2, hs, hs * (2.0 * R + hs));
}

// Distance to the ground along a downward ray, or -1.
fn ground_t(g: Geo, mu: f32) -> f32 {
    let rm = g.r * mu;
    if (mu >= 0.0) { return -1.0; }
    var d: f32;
    if (naive()) { d = rm * rm - (g.r * g.r - R * R); } else { d = rm * rm - g.c; }
    if (d < 0.0) { return -1.0; }
    if (naive()) { return -rm - sqrt(d); }
    return g.c / (-rm + sqrt(d));
}

fn ctop_of(g: Geo) -> f32 {
    if (naive()) { return g.r * g.r - RT * RT; }
    return (g.h - H) * (2.0 * R + g.h + H);
}

// Far root of the top of the atmosphere (inside the atmosphere, or the exit seen from outside); -1 if missed.
fn top_exit(g: Geo, mu: f32) -> f32 {
    let rm = g.r * mu;
    let ct = ctop_of(g);
    let d = rm * rm - ct;
    if (d < 0.0) { return -1.0; }
    if (!naive() && mu >= 0.0 && ct < 0.0) { return -ct / (rm + sqrt(d)); }
    return -rm + sqrt(d);
}

// Near root of the top seen from outside (mu < 0), or -1.
fn top_entry(g: Geo, mu: f32) -> f32 {
    let rm = g.r * mu;
    if (mu >= 0.0) { return -1.0; }
    let ct = ctop_of(g);
    let d = rm * rm - ct;
    if (d < 0.0) { return -1.0; }
    if (naive()) { return -rm - sqrt(d); }
    return ct / (-rm + sqrt(d));
}

fn dens(h: f32) -> vec2<f32> {
    let hh = max(h, 0.0);
    return vec2<f32>(exp(-hh / SCALE_R), exp(-hh / SCALE_M));
}

fn phase_r(c: f32) -> f32 { return 3.0 / (16.0 * PI) * (1.0 + c * c); }
fn phase_m(c: f32) -> f32 {
    let k = 3.0 / (8.0 * PI) * (1.0 - G * G) / (2.0 + G * G);
    return k * (1.0 + c * c) / pow(1.0 + G * G - 2.0 * G * c, 1.5);
}

fn sun_t(g: Geo, ms: f32) -> vec3<f32> {
    if (ms < 0.0 && ground_t(g, ms) >= 0.0) { return vec3<f32>(0.0); }
    let tt = top_exit(g, ms);
    if (tt <= 0.0) { return vec3<f32>(1.0); }
    let ds = tt / f32(N_SUN);
    var tau = vec3<f32>(0.0);
    for (var i = 0; i < N_SUN; i++) {
        let s = (f32(i) + 0.5) * ds;
        let gs = geo_at(g, ms, s);
        let d = dens(gs.h);
        tau += (BETA_R * d.x + vec3<f32>(BETA_M_E * d.y)) * ds;
    }
    return exp(-tau);
}

@compute @workgroup_size(64)
fn cs(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.count) { return; }
    let ray = rays[id.x];
    var g0: Geo;
    if (naive()) { g0 = Geo(ray.r, ray.r - R, ray.r * ray.r - R * R); } else { g0 = Geo(ray.r, ray.h, ray.c); }
    let mu = ray.mu;
    let outside = select(g0.h > H, ray.r > RT, naive());
    var t0 = 0.0;
    var hit = true;
    if (outside) {
        t0 = top_entry(g0, mu);
        if (t0 < 0.0) { hit = false; }
    }
    let tg = ground_t(g0, mu);
    var t1 = top_exit(g0, mu);
    if (tg >= 0.0) { t1 = tg; }
    var l = vec3<f32>(0.0);
    if (hit && t1 > t0) {
        let ds = (t1 - t0) / f32(N_VIEW);
        var tcur = vec3<f32>(1.0);
        let pr = phase_r(ray.cosg);
        let pm = phase_m(ray.cosg);
        for (var i = 0; i < N_VIEW; i++) {
            let s = t0 + (f32(i) + 0.5) * ds;
            let gs = geo_at(g0, mu, s);
            let ms_s = (ray.r * ray.ms + s * ray.cosg) / gs.r;
            let d = dens(gs.h);
            let ext = (BETA_R * d.x + vec3<f32>(BETA_M_E * d.y)) * ds;
            let tmid = tcur * exp(-0.5 * ext);
            let sc = BETA_R * d.x * pr + vec3<f32>(BETA_M_S * d.y * pm);
            l += tmid * sc * sun_t(gs, ms_s) * ds;
            tcur *= exp(-ext);
        }
        if (tg >= 0.0) {
            let gg = geo_at(g0, mu, tg);
            let ms_g = (ray.r * ray.ms + tg * ray.cosg) / gg.r;
            l += tcur * ALBEDO / PI * max(ms_g, 0.0) * sun_t(gg, ms_g);
        }
    }
    outp[id.x * 2u] = vec4<f32>(l, select(0.0, 1.0, tg >= 0.0));
    outp[id.x * 2u + 1u] = vec4<f32>(t1, tg, t0, 0.0);
}
