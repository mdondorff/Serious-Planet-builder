//! `cargo xtask transects --chain DIR [--face F] [--out FILE]` (BUILD-061): the cross-section sheet of a chain directory (`planet_cache::transect_sheet`).
//! The default output is `target/review/stages/transects/<label>.png` and its table `<label>.txt`.

use std::path::{Path, PathBuf};

use planet_cache::stage_sheet::Manifest;
use planet_cache::transect_sheet::{default_out, parse_dem, parse_transects, render, GLO30_CSV, TRANSECTS_CSV};

fn main() -> Result<(), String> {
    const USAGE: &str = "usage: transect_sheet DIR [--face F] [--out FILE]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().filter(|a| !a.starts_with("--")).ok_or(USAGE)?);
    let value = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(String::as_str);
    let face: Option<u8> = value("--face").map(str::parse).transpose().map_err(|_| "--face: 0 to 5")?;
    if face.is_some_and(|f| f > 5) {
        return Err("--face: 0 to 5".into());
    }
    let out = match value("--out") {
        Some(o) => PathBuf::from(o),
        None => {
            let mp = dir.join("chain.txt");
            let m = Manifest::parse(&std::fs::read_to_string(&mp).map_err(|e| format!("{}: {e}", mp.display()))?)?;
            default_out(Path::new("."), &m)
        }
    };
    let (n, w, h) = render(&dir, face, &out, &parse_transects(TRANSECTS_CSV)?, &parse_dem(GLO30_CSV)?)?;
    println!("transects --chain: {} ({n} transects, {w} x {h} px); table {}", out.display(), out.with_extension("txt").display());
    Ok(())
}
