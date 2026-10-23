//! `cargo xtask store`: the production store the editor opens by default (VIEW-052), and the stores of exaggerated worlds (WDEF-017).

use crate::util::{run, Res};
use std::path::Path;

const POINTER: &str = "target/stores/default";
/// The range of `body.0.relief_scale` (WDEF-017).
const RELIEF_RANGE: (f64, f64) = (1.0, 4.0);

/// `store [--res N] [--relief-scale S] [--link DIR]`: bake N cells per face edge (default 16384) into `target/stores/global<N>` and point
/// the editor at it, or with `--link` only point the editor at an existing store. With `--relief-scale S` (not 1) the world is an
/// exaggerated one: its definition is written beside the store as `world.toml`, the store is `target/stores/global<N>-relief<S>`, the
/// default pointer is left alone, and the command line that opens it is printed.
pub fn run_store(root: &Path, args: &[String]) -> Res {
    let (mut res, mut link, mut relief) = (16384u32, None, 1.0f64);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--res" => res = it.next().ok_or("--res needs a count")?.parse().map_err(|e| format!("--res: {e}"))?,
            "--link" => link = Some(it.next().ok_or("--link needs a directory")?.clone()),
            "--relief-scale" => {
                relief = it.next().ok_or("--relief-scale needs a factor")?.parse().map_err(|e| format!("--relief-scale: {e}"))?
            }
            other => return Err(format!("unknown store option '{other}' (known: --res N, --relief-scale S, --link DIR)")),
        }
    }
    if !(RELIEF_RANGE.0..=RELIEF_RANGE.1).contains(&relief) {
        return Err(format!("--relief-scale {relief}: a factor from {} to {}", RELIEF_RANGE.0, RELIEF_RANGE.1));
    }
    if link.is_some() && relief != 1.0 {
        return Err("--link points at a store of the default world: --relief-scale builds its own".into());
    }
    if let Some(d) = link {
        let p = std::fs::canonicalize(&d).map_err(|e| format!("--link {d}: {e}"))?;
        return point_at(root, p.to_string_lossy().trim_start_matches("\\\\?\\"));
    }
    let res_s = res.to_string();
    if relief == 1.0 {
        let dir = format!("target/stores/global{res}");
        run(
            root,
            "cargo",
            &["run", "-q", "--release", "-p", "planet", "--", "generate", "--global-dir", &dir, "--global-res", &res_s],
            &[],
        )?;
        return point_at(root, &dir);
    }
    let dir = format!("target/stores/global{res}-relief{relief}");
    std::fs::create_dir_all(root.join(&dir)).map_err(|e| format!("creating {dir}: {e}"))?;
    let world = format!("{dir}/world.toml");
    // The default world (seed 1, Earth's radius) with the scale: what the editor builds without --world, plus the one node.
    let text = format!("schema_version = 2\n\n[body.0]\nradius_m = 6371000.0\nrelief_scale = {relief:?}\n\n[world]\nseed = 1\n");
    std::fs::write(root.join(&world), text).map_err(|e| format!("writing {world}: {e}"))?;
    run(
        root,
        "cargo",
        &["run", "-q", "--release", "-p", "planet", "--", "generate", "--world", &world, "--global-dir", &dir, "--global-res", &res_s],
        &[],
    )?;
    println!("exaggerated world (relief scale {relief}) in {dir}; open it from the repo root with:");
    println!("cargo run --release -p planet -- editor --world {world} --store {dir} --global-res {res}");
    Ok(())
}

fn point_at(root: &Path, dir: &str) -> Res {
    let pointer = root.join(POINTER);
    std::fs::create_dir_all(pointer.parent().expect("the pointer has a folder")).map_err(|e| e.to_string())?;
    std::fs::write(&pointer, format!("{dir}\n")).map_err(|e| format!("writing {}: {e}", pointer.display()))?;
    println!("the editor opens {dir} from the repo root (pointer file {POINTER})");
    Ok(())
}
