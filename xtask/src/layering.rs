//! CON-02: crate dependencies point downward only, and the GPU-free crates have no graphics crate in
//! their dependency tree.

use crate::util::{capture, Res};
use std::path::Path;

/// `core → world-def → generators → cache → streaming → frame → render → editor → app`
pub const CHAIN: [&str; 9] = ["core", "world-def", "generators", "cache", "streaming", "frame", "render", "editor", "app"];
/// The first six compile without any graphics crate.
pub const GPU_FREE: usize = 6;
const GRAPHICS_CRATES: [&str; 8] = ["wgpu", "wgpu-core", "wgpu-hal", "naga", "winit", "ash", "egui", "eframe"];

/// Graphics-API crates only `render` may depend on (ADR 0004: render's public API is the seam to wgpu).
const RENDER_ONLY: [&str; 5] = ["wgpu", "wgpu-core", "wgpu-hal", "naga", "ash"];

fn package_name(dir: &str) -> String {
    if dir == "app" {
        "planet".to_string()
    } else {
        format!("planet-{dir}")
    }
}

/// Errors for one crate manifest: any `[dependencies]` entry on a crate at the same or a higher layer.
pub fn check_manifest(dir: &str, manifest: &str) -> Vec<String> {
    let me = CHAIN.iter().position(|c| *c == dir).expect("crate in chain");
    let mut errors = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_deps = t == "[dependencies]" || t == "[build-dependencies]" || (t.starts_with("[target.") && t.ends_with(".dependencies]"));
            continue;
        }
        if !in_deps {
            continue;
        }
        if t.starts_with("planet-testkit") {
            errors.push(format!(
                "crate '{dir}' lists planet-testkit outside [dev-dependencies]; the test kit is test support only (ADR 0008)"
            ));
        }
        let dep = t.split(|c: char| c == '.' || c == '=' || c.is_whitespace()).next().unwrap_or("");
        if let Some(i) = CHAIN.iter().position(|c| package_name(c) == dep) {
            if i >= me {
                errors.push(format!(
                    "crate '{dir}' depends on '{}', which is not below it (chain: {}); dependencies point downward only (CON-02)",
                    CHAIN[i],
                    CHAIN.join(" -> ")
                ));
            }
        }
    }
    errors
}

/// Errors when a manifest other than `render`'s names a graphics-API crate in any dependency section (dev and
/// build dependencies included: tests of other crates must not reach wgpu either).
pub fn check_graphics_isolation(dir: &str, manifest: &str) -> Vec<String> {
    if dir == "render" {
        return Vec::new();
    }
    let mut errors = Vec::new();
    let mut flag = |dep: &str| {
        errors.push(format!("'{dir}' depends on '{dep}': only render may depend on the graphics API; use render's public API (ADR 0004)"))
    };
    let unquote = |t: &str| t.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
    let mut in_deps = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            let header = t.trim_start_matches('[').trim_end_matches(']');
            // `[dependencies]`, `[dev-dependencies]`, `[target.'cfg'.dependencies]` or the table form
            // `[dependencies.wgpu]` / `[target.'cfg'.dev-dependencies.wgpu]`.
            let (head, tail) = match header.rfind("dependencies.") {
                Some(i) if header[..i + 12].ends_with("dependencies") => (&header[..i + 12], Some(unquote(&header[i + 13..]))),
                _ => (header, None),
            };
            in_deps = head.ends_with("dependencies");
            if let Some(name) = tail.filter(|n| in_deps && RENDER_ONLY.contains(&n.as_str())) {
                flag(&name);
            }
            continue;
        }
        if !in_deps {
            continue;
        }
        let key = unquote(t.split(|c: char| c == '.' || c == '=' || c.is_whitespace()).next().unwrap_or(""));
        if RENDER_ONLY.contains(&key.as_str()) {
            flag(&key);
        }
        // A renamed dependency: `gfx = { package = "wgpu", ... }` or a `package = "wgpu"` line in a table.
        if let Some(i) = t.find("package") {
            let rest = t[i + 7..].trim_start();
            if let Some(v) = rest.strip_prefix('=') {
                let name = unquote(v.trim_start().split([',', '}']).next().unwrap_or(""));
                if RENDER_ONLY.contains(&name.as_str()) {
                    flag(&name);
                }
            }
        }
    }
    errors
}

/// The `render` feature that builds the test-only reference renderer (REND-031).
const REFERENCE_FEATURE: &str = "reference-renderer";

/// Errors when a manifest enables the reference renderer outside `[dev-dependencies]`: any line of a `[dependencies]`,
/// `[build-dependencies]` or `[target.*.dependencies]` section (also the table form) that names the feature, or a `[features]` entry
/// other than render's own definition of it (a `default` list or a forwarding feature). Comments are ignored.
pub fn check_reference_feature(dir: &str, manifest: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let mut section = String::new();
    for (n, line) in manifest.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            section = t.trim_start_matches('[').trim_end_matches(']').to_string();
            continue;
        }
        if t.starts_with('#') || !t.contains(REFERENCE_FEATURE) {
            continue;
        }
        let key = t.split(|c: char| c == '=' || c.is_whitespace()).next().unwrap_or("").trim_matches('"');
        let product_dependency = section.contains("dependencies") && !section.contains("dev-dependencies");
        // In `[features]`: the `default` list, or another crate's feature forwarding to it (`planet-render/reference-renderer`). A feature of
        // render itself that turns it on (`spike07 = ["reference-renderer"]`) is off by default and fine.
        let feature_entry = section == "features" && key != REFERENCE_FEATURE && (key == "default" || t.contains("/reference-renderer"));
        if product_dependency || feature_entry {
            errors.push(format!(
                "crates/{dir}/Cargo.toml:{}: '{dir}' enables the test-only feature '{REFERENCE_FEATURE}' of render outside [dev-dependencies] (REND-031, BUILD-001)",
                n + 1
            ));
        }
    }
    errors
}

/// Public items of render's sources whose signature or field type names a wgpu type: render's public API must not
/// expose the graphics API (ADR 0004). `(line, text)`; `pub(crate)` and friends are fine.
pub fn wgpu_in_public_api(source: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut hits = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if !t.starts_with("pub ") {
            continue;
        }
        // The item head: up to the first `{` or `;`; a field (`pub x: T,`) ends with its line.
        let field = !t.contains("fn ") && !t.contains('(');
        let mut head = String::new();
        for l2 in &lines[i..] {
            head.push_str(l2.trim());
            head.push(' ');
            let end = l2.trim_end();
            if field || end.ends_with('{') || end.ends_with(';') || end.contains('{') {
                break;
            }
        }
        let head = head.split('{').next().unwrap_or("").split(" where ").next().unwrap_or("");
        if head.contains("wgpu::") && !head.trim_start().starts_with("pub use") {
            hits.push((i + 1, t.to_string()));
        }
    }
    hits
}

/// Errors for the output of `cargo tree --prefix none`: any graphics crate in a GPU-free crate.
pub fn check_tree(dir: &str, tree: &str) -> Vec<String> {
    tree.lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|name| GRAPHICS_CRATES.contains(name))
        .map(|name| format!("GPU-free crate '{dir}' has graphics crate '{name}' in its dependency tree (CON-02)"))
        .collect()
}

pub fn run(root: &Path) -> Res {
    let mut errors = Vec::new();
    for (i, dir) in CHAIN.iter().enumerate() {
        let manifest = std::fs::read_to_string(root.join("crates").join(dir).join("Cargo.toml"))
            .map_err(|e| format!("crates/{dir}/Cargo.toml: {e}"))?;
        errors.extend(check_manifest(dir, &manifest));
        if i < GPU_FREE {
            let tree = capture(root, "cargo", &["tree", "-p", &package_name(dir), "-e", "normal", "--prefix", "none"])?;
            errors.extend(check_tree(dir, &tree));
        }
    }
    for e in std::fs::read_dir(root.join("crates/render/src")).map_err(|e| e.to_string())?.flatten() {
        if e.path().extension().is_some_and(|x| x == "rs") {
            let text = std::fs::read_to_string(e.path()).map_err(|e| e.to_string())?;
            for (line, item) in wgpu_in_public_api(&text) {
                errors.push(format!(
                    "{}:{line}: public item exposes a wgpu type: {item} (render's public API is the seam, ADR 0004)",
                    e.path().display()
                ));
            }
        }
    }
    // Every workspace member, chain or not (testkit, perf, xtask): only render may name a graphics-API crate.
    let mut members: Vec<(String, std::path::PathBuf)> = Vec::new();
    for e in std::fs::read_dir(root.join("crates")).map_err(|e| e.to_string())?.flatten() {
        members.push((e.file_name().to_string_lossy().into_owned(), e.path().join("Cargo.toml")));
    }
    members.push(("xtask".to_string(), root.join("xtask").join("Cargo.toml")));
    for (dir, path) in members {
        if let Ok(manifest) = std::fs::read_to_string(&path) {
            errors.extend(check_graphics_isolation(&dir, &manifest));
            errors.extend(check_reference_feature(&dir, &manifest));
        }
    }
    if errors.is_empty() {
        eprintln!("layering ok: {} crates, {} GPU-free", CHAIN.len(), GPU_FREE);
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-001
    #[test]
    fn wgpu_types_in_public_signatures_are_flagged() {
        let src = "pub fn a(x: &wgpu::Texture) -> u32 {
}
pub(crate) fn b(x: &wgpu::Texture) {}
fn c(x: wgpu::Device) {}
pub struct S {
    pub device: wgpu::Device,
    pub(crate) q: wgpu::Queue,
    pub n: u32,
}
pub fn d(
    ctx: &C,
    t: &wgpu::Texture,
) -> u32 {
}
pub fn e() { let _ = wgpu::Instance::new(); }
";
        let hits: Vec<usize> = wgpu_in_public_api(src).iter().map(|h| h.0).collect();
        assert_eq!(hits, [1, 6, 10], "{hits:?}");
    }

    // spec: BUILD-001
    #[test]
    fn the_reference_renderer_feature_is_enabled_by_dev_dependencies_only() {
        // render's own definition, its self dev-dependency and a comment are fine.
        let render = "# the reference-renderer feature
[dev-dependencies]
planet-render = { workspace = true, features = [\"reference-renderer\"] }
[features]
reference-renderer = []
spike07 = [\"reference-renderer\"]
";
        assert!(check_reference_feature("render", render).is_empty());
        for (what, manifest) in [
            ("inline", "[dependencies]\nplanet-render = { workspace = true, features = [\"reference-renderer\"] }\n"),
            ("build", "[build-dependencies]\nplanet-render = { workspace = true, features = [\"reference-renderer\"] }\n"),
            ("target", "[target.'cfg(windows)'.dependencies]\nplanet-render = { workspace = true, features = [\"reference-renderer\"] }\n"),
            ("table", "[dependencies.planet-render]\nworkspace = true\nfeatures = [\"reference-renderer\"]\n"),
            ("default", "[features]\ndefault = [\"planet-render/reference-renderer\"]\n"),
            ("forwarded", "[features]\ntests = [\"planet-render/reference-renderer\"]\n"),
        ] {
            let e = check_reference_feature("app", manifest);
            assert_eq!(e.len(), 1, "{what}: {e:?}");
            assert!(e[0].contains("outside [dev-dependencies]"), "{what}: {e:?}");
        }
        let e = check_reference_feature(
            "app",
            "[dev-dependencies]\nplanet-render = { workspace = true, features = [\"reference-renderer\"] }\n",
        );
        assert!(e.is_empty(), "{e:?}");
    }

    // spec: BUILD-001
    #[test]
    fn only_render_may_depend_on_the_graphics_api() {
        let wgpu = "[dependencies]
wgpu.workspace = true
";
        assert!(check_graphics_isolation("render", wgpu).is_empty());
        for dir in ["app", "frame", "testkit", "perf", "xtask"] {
            let e = check_graphics_isolation(dir, wgpu);
            assert_eq!(e.len(), 1, "{dir}: {e:?}");
            assert!(e[0].contains("only render"), "{e:?}");
        }
        // Dev- and build-dependencies count; unrelated names and other sections do not.
        assert_eq!(
            check_graphics_isolation(
                "app",
                "[dev-dependencies]
wgpu = { version = \"1\" }
"
            )
            .len(),
            1
        );
        assert_eq!(
            check_graphics_isolation(
                "app",
                "[target.'cfg(windows)'.dependencies]
naga = \"1\"
"
            )
            .len(),
            1
        );
        assert!(check_graphics_isolation(
            "app",
            "[dependencies]
planet-render.workspace = true
[package]
name = \"wgpu\"
"
        )
        .is_empty());
    }

    // spec: BUILD-001
    #[test]
    fn upward_and_sideways_dependencies_are_flagged() {
        let ok = "[dependencies]\nplanet-core.workspace = true\nplanet-world-def.workspace = true\n";
        assert!(check_manifest("generators", ok).is_empty());
        let up = "[dependencies]\nplanet-render.workspace = true\n";
        let e = check_manifest("generators", up);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("'render'") && e[0].contains("downward only"), "{e:?}");
        let side = "[dependencies]\nplanet-cache.workspace = true\n";
        assert_eq!(check_manifest("cache", side).len(), 1, "a crate must not depend on itself");
        let tk = "[dependencies]
planet-testkit.workspace = true
";
        assert_eq!(check_manifest("app", tk).len(), 1, "testkit is dev-dependency only");
        assert!(check_manifest(
            "app",
            "[dev-dependencies]
planet-testkit.workspace = true
"
        )
        .is_empty());
        // dev-dependencies (e.g. the test kit) are not part of the layering.
        assert!(check_manifest("core", "[dev-dependencies]\nplanet-render.workspace = true\n").is_empty());
    }

    // spec: BUILD-001
    #[test]
    fn graphics_crates_in_gpu_free_tree_are_flagged() {
        let tree = "planet-frame v0.0.0\nplanet-core v0.0.0\nlibm v0.2.16\n";
        assert!(check_tree("frame", tree).is_empty());
        let bad = "planet-frame v0.0.0\nwgpu v30.0.1\nnaga v30.0.1\n";
        assert_eq!(check_tree("frame", bad).len(), 2);
    }

    // spec: BUILD-001
    #[test]
    fn this_repository_satisfies_the_layering() {
        let root = crate::util::repo_root();
        run(&root).unwrap();
    }
}
