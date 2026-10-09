//! CON-02: crate dependencies point downward only, and the GPU-free crates have no graphics crate in
//! their dependency tree.

use crate::util::{capture, Res};
use std::path::Path;

/// `core → world-def → generators → cache → streaming → frame → render → editor → app`
pub const CHAIN: [&str; 9] = ["core", "world-def", "generators", "cache", "streaming", "frame", "render", "editor", "app"];
/// The first six compile without any graphics crate.
pub const GPU_FREE: usize = 6;
const GRAPHICS_CRATES: [&str; 8] = ["wgpu", "wgpu-core", "wgpu-hal", "naga", "winit", "ash", "egui", "eframe"];

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
