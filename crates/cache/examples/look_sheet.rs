//! `cargo xtask look-sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]` (BUILD-067): the look sheets of a crop store (`planet_cache::look_sheet`). The default output is
//! `target/review/look/<RUN's directory name>/`, the default suns 315 and 135 degrees.

use std::path::{Path, PathBuf};

use planet_cache::look_sheet::{default_out, write_sheets, AZIMUTHS};

fn main() -> Result<(), String> {
    const USAGE: &str = "usage: look_sheet RUN [--against DEFAULTS] [--az A,B] [--out DIR]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run = PathBuf::from(args.first().filter(|a| !a.starts_with("--")).ok_or(USAGE)?);
    let value = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).map(String::as_str);
    let against = value("--against").map(PathBuf::from);
    let az: Vec<f64> = match value("--az") {
        Some(s) => s
            .split(',')
            .map(|a| a.trim().parse::<f64>().map_err(|_| format!("--az: `{a}` is not a number ({USAGE})")))
            .collect::<Result<_, _>>()?,
        None => AZIMUTHS.to_vec(),
    };
    let out = value("--out").map_or_else(|| default_out(Path::new("."), &run), PathBuf::from);
    let w = write_sheets(&run, against.as_deref(), &az, &out)?;
    for (p, (pw, ph), bytes) in &w.sheets {
        println!("look-sheet: {} ({pw} x {ph} px, {bytes} bytes)", p.display());
    }
    if let Some(n) = &w.numbers {
        println!("look-sheet: {}", n.display());
    }
    Ok(())
}
