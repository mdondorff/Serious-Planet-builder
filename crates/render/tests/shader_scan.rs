//! REND-005: shaders see only tile-local and camera-relative quantities: no planet-radius constants or uniforms.

use planet_render::{EXEC_SHADERS, SHADERS, TERRAIN_SHADERS};

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

// spec: REND-005
#[test]
fn shipped_shaders_contain_no_planet_scale_values() {
    let mut all = Vec::new();
    for (name, src) in SHADERS.iter().chain(EXEC_SHADERS).chain(TERRAIN_SHADERS) {
        for h in scan(src) {
            all.push(format!("{name}: {h}"));
        }
    }
    assert!(all.is_empty(), "CON-11: shaders must not compute planet-radius magnitudes:\n{}", all.join("\n"));
}
