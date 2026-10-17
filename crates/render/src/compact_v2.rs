//! SPIKE-12 ONLY (never merged): the compact vertex stage generated for the prototype layouts of the vertex-retirement spike.
//!
//! One generator, five switches (`planet_generators::compact::spike_flags`): a flat record array read in place instead of copied
//! (`SPIKE_V2_SHADER`), a 5 x 5 base surface (`SPIKE_BASE5`, 16 more control points in the record), a wide texel (`SPIKE_WIDE`:
//! 32-bit height, two 15-bit normal components, the climate read from the record), and both texel decoders chosen per slot by a
//! record flag (`SPIKE_BOTH`). Blocks are storage buffers read by index (ADR 0054), so the texel is two `u32` words.

use planet_generators::compact::{
    record_climate_offset, record_words, spike_flags, REC_HEADER_WORDS, SPIKE_BASE5, SPIKE_BOTH, SPIKE_V2_SHADER, SPIKE_WIDE,
};

/// Whether the flat-record generator is in use.
pub fn v2_active() -> bool {
    spike_flags() & (SPIKE_BASE5 | SPIKE_WIDE | SPIKE_BOTH | SPIKE_V2_SHADER) != 0
}

/// Name of the lattice node `(a, b)` of the base surface (5 x 5: the 3 x 3 control points keep their names).
fn node_name(a: usize, b: usize, n: usize) -> String {
    if n == 3 {
        return format!("c{}", 3 * b + a);
    }
    if a % 2 == 0 && b % 2 == 0 {
        return format!("c{}", 3 * (b / 2) + a / 2);
    }
    let before: usize = (0..b).map(|rb| if rb % 2 == 0 { 2 } else { 5 }).sum();
    format!("e{}", before + if b % 2 == 0 { (a - 1) / 2 } else { a })
}

/// Lagrange basis and derivative as WGSL for `n` equidistant nodes on [0, 1]: `basis{n}` and `dbasis{n}` return `array<f32, n>`.
fn basis_wgsl(n: usize) -> String {
    let t: Vec<f64> = (0..n).map(|k| k as f64 / (n - 1) as f64).collect();
    let mut out = String::new();
    for (name, deriv) in [("basis", false), ("dbasis", true)] {
        out += &format!("fn {name}{n}(t: f32) -> array<f32, {n}> {{\n    return array<f32, {n}>(\n");
        for k in 0..n {
            let denom: f64 = (0..n).filter(|&m| m != k).map(|m| t[k] - t[m]).product();
            let expr = if !deriv {
                (0..n).filter(|&m| m != k).map(|m| format!("(t - {:?})", t[m] as f32)).collect::<Vec<_>>().join(" * ")
            } else {
                let terms: Vec<String> = (0..n)
                    .filter(|&j| j != k)
                    .map(|j| {
                        let f: Vec<String> = (0..n).filter(|&m| m != k && m != j).map(|m| format!("(t - {:?})", t[m] as f32)).collect();
                        if f.is_empty() {
                            "1.0".to_string()
                        } else {
                            f.join(" * ")
                        }
                    })
                    .collect();
                terms.join(" + ")
            };
            out += &format!("        ({expr}) / {:?}{}\n", denom as f32, if k + 1 < n { "," } else { "" });
        }
        out += "    );\n}\n";
    }
    out
}

fn vertex_source(cells: u32) -> String {
    let flags = spike_flags();
    let (base5, wide, both) = (flags & SPIKE_BASE5 != 0, flags & SPIKE_WIDE != 0, flags & SPIKE_BOTH != 0);
    let n = if base5 { 5 } else { 3 };
    // Record loads.
    let mut vars = String::new();
    let mut load = String::new();
    for k in 0..9 {
        vars += &format!("var<private> c{k}: vec3<f32>;\n");
        load += &format!("    c{k} = vec3<f32>(recf({}u), recf({}u), recf({}u));\n", 3 * k, 3 * k + 1, 3 * k + 2);
    }
    if base5 {
        for k in 0..16 {
            vars += &format!("var<private> e{k}: vec3<f32>;\n");
            let w = REC_HEADER_WORDS + 3 * k;
            load += &format!("    e{k} = vec3<f32>(recf({}u), recf({}u), recf({}u));\n", w, w + 1, w + 2);
        }
    }
    // Rows of the base surface and its evaluation.
    let mut base = basis_wgsl(n);
    for b in 0..n {
        let terms: Vec<String> = (0..n).map(|a| format!("{} * w[{a}]", node_name(a, b, n))).collect();
        base += &format!("fn row{b}(w: array<f32, {n}>) -> vec3<f32> {{ return {}; }}\n", terms.join(" + "));
    }
    let blend = |basis: &str, other: &str| -> String { (0..n).map(|b| format!("row{b}({basis}) * {other}[{b}]")).collect::<Vec<_>>().join(" + ") };
    base += &format!(
        "fn base_point(u: f32, v: f32) -> vec3<f32> {{\n    let bu = basis{n}(u);\n    let bv = basis{n}(v);\n    return {};\n}}\n",
        blend("bu", "bv")
    );
    base += &format!(
        "fn base_normal(u: f32, v: f32) -> vec3<f32> {{\n    let bu = basis{n}(u);\n    let bv = basis{n}(v);\n    let du = dbasis{n}(u);\n    let dv = dbasis{n}(v);\n    let pv = {};\n    let pu = {};\n    return normalize(cross(pu, pv)) * select(1.0, -1.0, flip);\n}}\n",
        blend("bu", "dv"),
        blend("du", "bv")
    );
    // Texel decoders.
    let narrow_h = "hr_min + f32(t.x & 0xffffu) * hr_step";
    let wide_h = "hr_min + f32(t.x) * hr_step";
    let narrow_n = "oct_decode(unpack2x16snorm((t.x >> 16u) | (t.y << 16u)))";
    let wide_n = "oct_decode(vec2<f32>(f32(extractBits(i32(t.y), 0u, 15u)), f32(extractBits(i32(t.y), 15u, 15u))) / 16383.0)";
    let (rel_height, normal_of) = if both {
        (
            format!("    if (wide_slot) {{ return {wide_h}; }}\n    return {narrow_h};\n"),
            format!("    if (wide_slot) {{ return {wide_n}; }}\n    return {narrow_n};\n"),
        )
    } else if wide {
        (format!("    return {wide_h};\n"), format!("    return {wide_n};\n"))
    } else {
        (format!("    return {narrow_h};\n"), format!("    return {narrow_n};\n"))
    };
    // The climate: the spare channel of the texels (narrow) or the record's block (wide).
    let clim_narrow = "unpack2x16float(climate_bits(base, idx) | (climate_bits(base, 81 + idx) << 16u))";
    let clim_wide = "unpack2x16float(tile_records[slot * REC_WORDS + CLIM_OFF + u32(idx)])";
    let climate_texel = if both {
        format!(
            "    if ((tile_records[slot * REC_WORDS + 31u] & 2u) != 0u) {{ return {clim_wide}; }}\n    return {clim_narrow};\n"
        )
    } else if wide {
        format!("    return {clim_wide};\n")
    } else {
        format!("    return {clim_narrow};\n")
    };
    VERTEX
        .replace("{CELLS}", &cells.to_string())
        .replace("{REC_WORDS}", &record_words().to_string())
        .replace("{CLIM_OFF}", &record_climate_offset().to_string())
        .replace("/*VARS*/", &vars)
        .replace("/*LOAD*/", &load)
        .replace("/*BASE*/", &base)
        .replace("/*REL_HEIGHT*/", &rel_height)
        .replace("/*NORMAL*/", &normal_of)
        .replace("/*CLIMATE_TEXEL*/", &climate_texel)
}

/// The full compact shader of the prototype: the product's header and fragment stage around [`vertex_source`].
pub fn shader(cells: u32, head: &str, tail: &str) -> String {
    format!("{head}{}{tail}", vertex_source(cells))
}

const VERTEX: &str = r#"
// SPIKE-12 compact vertex stage: flat record array, blocks as storage buffers of two-word texels.
@group(1) @binding(0) var<storage, read> tile_records: array<u32>;
@group(1) @binding(1) var<storage, read> tile_refs: array<vec4<u32>>;
@group(2) @binding(0) var<storage, read> atlas: array<vec2<u32>>;

const CELLS: i32 = {CELLS};
const REC_WORDS: u32 = {REC_WORDS}u;
const CLIM_OFF: u32 = {CLIM_OFF}u;

/*VARS*/
var<private> hr_min: f32;
var<private> hr_step: f32;
var<private> h_origin: f32;
var<private> skirt_depth: f32;
var<private> flip: bool;
var<private> wide_slot: bool;
var<private> origin: i32;
var<private> rb: u32;

fn recf(k: u32) -> f32 {
    return bitcast<f32>(tile_records[rb + k]);
}

fn load_tile(slot: u32, o: vec2<u32>) {
    rb = slot * REC_WORDS;
/*LOAD*/
    hr_min = recf(27u);
    hr_step = recf(28u);
    h_origin = recf(29u);
    skirt_depth = recf(30u);
    let fl = tile_records[rb + 31u];
    flip = (fl & 1u) != 0u;
    wide_slot = (fl & 2u) != 0u;
    origin = i32(o.x);
}

/*BASE*/

fn texel(i: i32, j: i32) -> vec2<u32> {
    return atlas[u32(origin + j * (CELLS + 1) + i)];
}

fn rel_height(t: vec2<u32>) -> f32 {
/*REL_HEIGHT*/
}

fn place(i: i32, j: i32, hr: f32) -> vec4<f32> {
    let u = f32(i) / f32(CELLS);
    let v = f32(j) / f32(CELLS);
    return vec4<f32>(base_point(u, v) + base_normal(u, v) * hr, hr);
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        let s = select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0));
        n = vec3<f32>((1.0 - abs(n.y)) * s.x, (1.0 - abs(n.x)) * s.y, n.z);
    }
    return normalize(n);
}

fn normal_of(t: vec2<u32>) -> vec3<f32> {
/*NORMAL*/
}

fn terrain_vertex(inst: u32, vi: u32) -> VsOut {
    let node = nodes[inst];
    let tref = tile_refs[inst];
    load_tile(tref.z, tref.xy);
    let w = CELLS + 1;
    let idx = i32(vi);
    var i = 0;
    var j = 0;
    var skirt = false;
    if (idx < w * w) {
        i = idx % w;
        j = idx / w;
    } else {
        let r = idx - w * w;
        let k = r / w;
        let t = r % w;
        skirt = true;
        if (k == 0) {
            i = t;
        } else if (k == 1) {
            i = t;
            j = CELLS;
        } else if (k == 2) {
            j = t;
        } else {
            i = CELLS;
            j = t;
        }
    }
    let own_t = texel(i, j);
    let own = place(i, j, rel_height(own_t));
    var coarse = own;
    let odd_i = (i & 1) == 1;
    let odd_j = (j & 1) == 1;
    if (odd_i && !odd_j) {
        coarse = 0.5 * (place(i - 1, j, rel_height(texel(i - 1, j))) + place(i + 1, j, rel_height(texel(i + 1, j))));
    } else if (!odd_i && odd_j) {
        coarse = 0.5 * (place(i, j - 1, rel_height(texel(i, j - 1))) + place(i, j + 1, rel_height(texel(i, j + 1))));
    } else if (odd_i && odd_j) {
        coarse = 0.5 * (place(i - 1, j - 1, rel_height(texel(i - 1, j - 1))) + place(i + 1, j + 1, rel_height(texel(i + 1, j + 1))));
    }
    var pos = own.xyz;
    var cpos = coarse.xyz;
    if (skirt) {
        let d = base_normal(f32(i) / f32(CELLS), f32(j) / f32(CELLS)) * skirt_depth;
        pos = pos - d;
        cpos = cpos - d;
    }
    let m = clamp((length(node.origin_rel.xyz + pos) - node.morph.x) / max(node.morph.y - node.morph.x, 1e-6), 0.0, 1.0);
    let local = mix(pos, cpos, m);
    let p = node.origin_rel.xyz + local;
    let h = h_origin + own.w;
    let ch = h_origin + coarse.w;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(p, 1.0);
    out.normal = normal_of(own_t);
    out.height = mix(h, ch, m);
    out.height_fine = h;
    out.morph = m;
    out.local = local;
    out.node_index = inst;
    out.aux = vec2<f32>(f32(i), f32(j)) / f32(CELLS);
    return out;
}

@vertex
fn vs(@builtin(instance_index) inst: u32, @builtin(vertex_index) vi: u32) -> VsOut {
    return terrain_vertex(inst, vi);
}

fn climate_bits(base: u32, idx: i32) -> u32 {
    return atlas[base + u32(idx)].y >> 16u;
}

fn climate_texel(slot: u32, base: u32, idx: i32) -> vec2<f32> {
/*CLIMATE_TEXEL*/
}

fn tile_climate(in: VsOut) -> vec2<f32> {
    let tref = tile_refs[in.node_index];
    let base = tref.x;
    let slot = tref.z;
    let x = clamp(in.aux.x, 0.0, 1.0) * 8.0;
    let y = clamp(in.aux.y, 0.0, 1.0) * 8.0;
    let a = min(i32(x), 7);
    let b = min(i32(y), 7);
    let wx = x - f32(a);
    let wy = y - f32(b);
    let i00 = b * 9 + a;
    let c00 = climate_texel(slot, base, i00);
    let c10 = climate_texel(slot, base, i00 + 1);
    let c01 = climate_texel(slot, base, i00 + 9);
    let c11 = climate_texel(slot, base, i00 + 10);
    return mix(mix(c00, c10, wx), mix(c01, c11, wx), wy);
}

"#;
