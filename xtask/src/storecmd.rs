//! `cargo xtask store`: the production store the editor opens by default (VIEW-052).

use crate::util::{run, Res};
use std::path::Path;

const POINTER: &str = "target/stores/default";

/// `store [--res N] [--link DIR]`: bake N cells per face edge (default 16384) into `target/stores/global<N>` and point the editor at it,
/// or with `--link` only point the editor at an existing store.
pub fn run_store(root: &Path, args: &[String]) -> Res {
    let (mut res, mut link) = (16384u32, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--res" => res = it.next().ok_or("--res needs a count")?.parse().map_err(|e| format!("--res: {e}"))?,
            "--link" => link = Some(it.next().ok_or("--link needs a directory")?.clone()),
            other => return Err(format!("unknown store option '{other}' (known: --res N, --link DIR)")),
        }
    }
    let dir = match link {
        Some(d) => {
            let p = std::fs::canonicalize(&d).map_err(|e| format!("--link {d}: {e}"))?;
            p.to_string_lossy().trim_start_matches("\\\\?\\").to_string()
        }
        None => {
            let dir = format!("target/stores/global{res}");
            let res_s = res.to_string();
            run(
                root,
                "cargo",
                &["run", "-q", "--release", "-p", "planet", "--", "generate", "--global-dir", &dir, "--global-res", &res_s],
                &[],
            )?;
            dir
        }
    };
    let pointer = root.join(POINTER);
    std::fs::create_dir_all(pointer.parent().expect("the pointer has a folder")).map_err(|e| e.to_string())?;
    std::fs::write(&pointer, format!("{dir}\n")).map_err(|e| format!("writing {}: {e}", pointer.display()))?;
    println!("the editor opens {dir} from the repo root (pointer file {POINTER})");
    Ok(())
}
