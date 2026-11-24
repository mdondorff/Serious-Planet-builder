//! lead #183 (spike only): a crop store (BUILD-067) cut from a full chain directory, for runs dumped before `--probe-crops` existed or whose crops were chosen from the run itself
//! (SPIKE-143's rule reads the final surface). Usage: `crops_from_chain CHAIN_DIR CROPS_JSON OUT_DIR`.

use std::path::PathBuf;

use planet_cache::look_sheet::{parse_crops_json, CropWriter};
use planet_cache::stage_sheet::Manifest;

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [dir, crops, out] = &a[..] else { return Err("usage: crops_from_chain CHAIN_DIR CROPS_JSON OUT_DIR".into()) };
    let dir = PathBuf::from(dir);
    let text = std::fs::read_to_string(dir.join("chain.txt")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let m = Manifest::parse(&text)?;
    let cs = parse_crops_json(&std::fs::read_to_string(crops).map_err(|e| format!("{crops}: {e}"))?)?;
    let w = CropWriter::new(&PathBuf::from(out), m.w, m.h, m.coarse, &cs)?;
    let mut n = 0;
    for sub in ["", "stages"] {
        let d = if sub.is_empty() { dir.clone() } else { dir.join(sub) };
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut names: Vec<String> =
            rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        for name in names.into_iter().filter(|f| [".f32", ".u8", ".u16"].iter().any(|x| f.ends_with(x))) {
            let b = std::fs::read(d.join(&name)).map_err(|e| format!("{name}: {e}"))?;
            let rel = if sub.is_empty() { name } else { format!("{sub}/{name}") };
            w.field(&rel, &b)?;
            n += 1;
        }
    }
    w.manifest(&text)?;
    println!("crops_from_chain: {n} fields of {} into {} crops at {out}", dir.display(), cs.len());
    Ok(())
}
