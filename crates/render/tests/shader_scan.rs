//! REND-005: geometry shaders see only tile-local and camera-relative quantities: no planet-radius constants or uniforms. The
//! atmosphere's shaders (presentation shading, CON-11 v1.4) may hold the radius in their uniform block but carry no planet-scale
//! literal or radius-like name, and no vertex stage may touch that block.

use planet_render::{compact_terrain_shader, lit_post_shaders, EXEC_SHADERS, SHADERS, TERRAIN_SHADERS};

/// Violations in one WGSL source: numeric literals of planet-radius magnitude (>= 1e6) and radius-like identifiers.
fn scan(source: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for tok in code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '+' || c == '-')) {
            let t = tok.trim_end_matches(['f', 'u', 'i']);
            if t.starts_with(|c: char| c.is_ascii_digit()) {
                if let Ok(v) = t.parse::<f64>() {
                    if v.abs() >= 1.0e6 {
                        hits.push(format!("line {}: literal {tok} has planet-scale magnitude", n + 1));
                    }
                }
            }
        }
        let lower = code.to_lowercase();
        for word in ["radius", "planet_r", "earth"] {
            if lower.contains(word) {
                hits.push(format!("line {}: identifier containing '{word}'", n + 1));
            }
        }
    }
    hits
}

// spec: REND-005
#[test]
fn the_scanner_flags_planet_scale_constants_and_names() {
    assert!(scan("let r = 6371000.0;").len() == 1);
    assert!(scan("let r = 6.371e6;").len() == 1);
    assert!(!scan("let x = planet_radius * 2.0;").is_empty());
    assert!(scan("let ok = 1e-6 + 0.5 + 255.0; // 6371000.0 in a comment").is_empty());
}

// spec: REND-005, ATMO-016
#[test]
fn shipped_shaders_contain_no_planet_scale_values() {
    let mut all = Vec::new();
    for (name, src) in SHADERS.iter().chain(EXEC_SHADERS).chain(TERRAIN_SHADERS) {
        for h in scan(src) {
            all.push(format!("{name}: {h}"));
        }
    }
    for (name, src) in lit_post_shaders() {
        for h in scan(&src) {
            all.push(format!("{name}: {h}"));
        }
    }
    // The atmosphere's shaders are presentation shading (CON-11 v1.4, ADR 0043): they hold the body's radius as a value of their
    // uniform block, decided in f64 on the CPU, so the radius-uniform rule does not apply; they carry no planet-scale literal and
    // no radius-like name either.
    for (name, src) in planet_render::atmosphere_shaders() {
        for h in scan(&src) {
            all.push(format!("{name}: {h}"));
        }
        // A vertex stage places geometry: it must not read the atmosphere's block (which holds the body's radius).
        for (at, _) in src.match_indices("@vertex") {
            let body = &src[at..];
            let end = body.find("\n}\n").unwrap_or(body.len());
            if body[..end].contains("A.") {
                all.push(format!("{name}: a vertex stage reads the atmosphere's uniform block"));
            }
        }
    }
    // The water pass places a surface on the terrain's grid (geometry stays tile-local, CON-11): no planet-scale literal or name in any
    // flavour, and the vertex stage reads neither the atmosphere's block nor anything but the node's tile-local numbers.
    for (name, src) in planet_render::water_shaders() {
        for h in scan(&src) {
            all.push(format!("{name}: {h}"));
        }
        for (at, _) in src.match_indices("@vertex") {
            let body = &src[at..];
            let end = body.find("\n}\n").unwrap_or(body.len());
            if body[..end].contains("A.") {
                all.push(format!("{name}: a vertex stage reads the atmosphere's uniform block"));
            }
        }
        // The functions the vertex stage calls: the entry is a one-line call, so their bodies are scanned too.
        for func in ["fn water_vertex(", "fn water_waves(", "fn terrain_vertex("] {
            for (at, _) in src.match_indices(func) {
                let body = &src[at..];
                let end = body.find("\n}\n").unwrap_or(body.len());
                if body[..end].contains("A.") {
                    all.push(format!("{name}: {func} reads the atmosphere's uniform block"));
                }
            }
        }
    }
    for cells in [4, 32, 64] {
        for h in scan(&compact_terrain_shader(cells)) {
            all.push(format!("terrain-compact ({cells} cells): {h}"));
        }
    }
    assert!(all.is_empty(), "CON-11: shaders must not compute planet-radius magnitudes:\n{}", all.join("\n"));
}

// spec: GEN-025, REND-016
#[test]
fn the_compact_shader_takes_the_slot_offsets_and_the_normal_scale_from_the_generators_constants() {
    use planet_generators::compact::{NORMAL_SCALE, SLOT_CLIMATE_OFFSET, SLOT_TEXEL_OFFSET};
    let src = compact_terrain_shader(32);
    assert!(!src.contains('{') || !src.contains("{TEXEL_WORD}"), "an unreplaced placeholder");
    for needle in [
        format!("const CLIMATE_WORD: u32 = {}u;", SLOT_CLIMATE_OFFSET / 4),
        format!("const TEXEL_WORD: u32 = {}u;", SLOT_TEXEL_OFFSET / 4),
        format!("const NORMAL_SCALE: f32 = {NORMAL_SCALE:.1};"),
    ] {
        assert!(src.contains(&needle), "the shader lacks `{needle}`");
    }
    assert_eq!((SLOT_CLIMATE_OFFSET % 4, SLOT_TEXEL_OFFSET % 4), (0, 0), "offsets are whole words");
}
