//! `cargo xtask stages --chain DIR [--step S] [--view X,Y,SX,SY] [--out FILE]` (BUILD-060): the chain sheet of a chain directory (`planet_cache::stage_sheet`).
//! The default is the whole window at 12 L cells per pixel, written to `target/review/stages/chain/<label>.png`.

use std::path::{Path, PathBuf};

use planet_cache::stage_sheet::{default_out, render, Manifest, View};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().ok_or("usage: stage_sheet DIR [--step S] [--view X,Y,SX,SY] [--out FILE]")?);
    let value = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(String::as_str);
    let step: usize = value("--step").map_or(Ok(12), str::parse).map_err(|_| "--step: a positive whole number")?;
    let view = match value("--view") {
        Some(s) => {
            let p: Vec<usize> = s.split(',').map(str::parse).collect::<Result<_, _>>().map_err(|_| "--view X,Y,SX,SY")?;
            let [x, y, size_x, size_y] = p[..] else { return Err("--view X,Y,SX,SY".into()) };
            Some(View { x, y, size_x, size_y, step: step.max(1) })
        }
        None => None,
    };
    let out = match value("--out") {
        Some(o) => PathBuf::from(o),
        None => {
            let mp = dir.join("chain.txt");
            let m = Manifest::parse(&std::fs::read_to_string(&mp).map_err(|e| format!("{}: {e}", mp.display()))?)?;
            default_out(Path::new("."), &m)
        }
    };
    let (w, h) = render(&dir, view, step, &out)?;
    println!("stages --chain: {} ({w} x {h} px)", out.display());
    Ok(())
}
