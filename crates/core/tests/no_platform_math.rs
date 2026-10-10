//! COORD-009: code that produces tile data must not call platform transcendental functions.

use std::path::Path;

const PATH_BANNED: [&str; 16] =
    ["sin", "cos", "tan", "asin", "acos", "atan", "atan2", "exp", "exp2", "ln", "log", "log2", "log10", "powf", "hypot", "cbrt"];

const BANNED: [&str; 27] = [
    ".asinh(",
    ".acosh(",
    ".atanh(",
    ".sin(",
    ".cos(",
    ".tan(",
    ".asin(",
    ".acos(",
    ".atan(",
    ".atan2(",
    ".sinh(",
    ".cosh(",
    ".tanh(",
    ".exp(",
    ".exp2(",
    ".exp_m1(",
    ".ln(",
    ".ln_1p(",
    ".log(",
    ".log2(",
    ".log10(",
    ".powf(",
    ".cbrt(",
    ".hypot(",
    ".sin_cos(",
    ".mul_add(",
    ".powi(",
];

/// Offending `(line number, banned call)` pairs in `source`; comments are ignored.
fn scan(source: &str) -> Vec<(usize, &'static str)> {
    let mut hits = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for t in ["f64", "f32"] {
            for name in PATH_BANNED {
                if code.contains(&format!("{t}::{name}(")) {
                    hits.push((n + 1, "UFCS platform maths call"));
                }
            }
        }
        for b in BANNED {
            if code.contains(b) {
                hits.push((n + 1, b));
            }
        }
    }
    hits
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

// spec: COORD-009
#[test]
fn the_scanner_flags_banned_calls_and_ignores_comments() {
    let hits = scan("let a = x.sin();\nlet b = libm::sin(x); // x.cos() is fine in a comment\nlet c = y.powf(2.0);\n");
    assert_eq!(hits, [(1, ".sin("), (3, ".powf(")]);
    assert_eq!(
        scan(
            "let a = f64::sin(x);
let b = f32::powf(a, 2.0);"
        )
        .len(),
        2
    );
}

// spec: COORD-009
#[test]
fn generator_relevant_crates_use_libm_only() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offences = Vec::new();
    for name in ["core", "world-def", "generators", "cache", "streaming", "frame"] {
        assert!(crates.join(name).join("src").is_dir(), "crate directory {name} is missing: the scan would check nothing");
        let mut files = Vec::new();
        rust_files(&crates.join(name).join("src"), &mut files);
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            for (line, call) in scan(&text) {
                offences.push(format!("{}:{line}: {call}", f.display()));
            }
        }
    }
    assert!(offences.is_empty(), "use libm instead of platform maths (ADR 0010):\n{}", offences.join("\n"));
}
