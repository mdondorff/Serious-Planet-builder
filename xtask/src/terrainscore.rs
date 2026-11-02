//! `cargo xtask terrain-score` (BUILD-029): the terrain scorecard (TEST-079) in release under the heavy lock, CPU only. The work is the `terrain_score`
//! example of the app; this file parses the options, checks the inputs by name and runs it.

use crate::util::{run, Res};
use std::path::Path;

/// The production store the editor opens (VIEW-052).
pub const DEFAULT_STORE: &str = "C:/DEV/SeriousPlanetData/global16384";
/// The GLO-30 sample tiles (outside the repository, licence notes beside them).
pub const DEFAULT_DEM: &str = "C:/DEV/SeriousPlanetData/dem/glo30-samples";
pub const DEFAULT_SITES: &str = "crates/app/data/terrain-score-sites.toml";
pub const OUT_BASE: &str = "target/review/terrain-score";

const KNOWN: &str = "--store DIR, --dem DIR, --sites FILE, --out DIR, --maps";

#[derive(Debug, PartialEq)]
pub struct Opts {
    pub store: String,
    pub dem: String,
    pub sites: String,
    pub out: String,
    pub maps: bool,
}

/// Parses the options; `date` names the default output directory.
pub fn parse_args(args: &[String], date: &str) -> Res<Opts> {
    let mut o = Opts {
        store: DEFAULT_STORE.into(),
        dem: DEFAULT_DEM.into(),
        sites: DEFAULT_SITES.into(),
        out: format!("{OUT_BASE}/{date}"),
        maps: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--store" => o.store = value()?,
            "--dem" => o.dem = value()?,
            "--sites" => o.sites = value()?,
            "--out" => o.out = value()?,
            "--maps" => o.maps = true,
            other => return Err(format!("unknown terrain-score option '{other}' (known: {KNOWN})")),
        }
    }
    Ok(o)
}

/// The `cargo` arguments that run the example with every option explicit.
pub fn cargo_args(o: &Opts) -> Vec<String> {
    let mut v: Vec<String> = [
        "run",
        "--release",
        "-p",
        "planet",
        "--example",
        "terrain_score",
        "--",
        "--store",
        &o.store,
        "--dem",
        &o.dem,
        "--sites",
        &o.sites,
        "--out",
        &o.out,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if o.maps {
        v.push("--maps".into());
    }
    v
}

/// Refuses a missing store, tile directory or sites file by name, and says which option changes it.
pub fn check_inputs(root: &Path, o: &Opts) -> Res {
    let at = |p: &str| if Path::new(p).is_absolute() { Path::new(p).to_path_buf() } else { root.join(p) };
    if !at(&o.store).is_dir() {
        return Err(format!("the store {} does not exist; point --store DIR at a baked store (cargo xtask store builds one)", o.store));
    }
    if !at(&o.dem).is_dir() {
        return Err(format!("the tile directory {} does not exist; point --dem DIR at the folder of GLO-30 tiles", o.dem));
    }
    if !at(&o.sites).is_file() {
        return Err(format!("the sites file {} does not exist; point --sites FILE at a sites file", o.sites));
    }
    Ok(())
}

/// `terrain-score [--store DIR] [--dem DIR] [--sites FILE] [--out DIR] [--maps]`; the caller holds the heavy lock.
pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let date = crate::accept::utc_date(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
    let o = parse_args(args, &date)?;
    check_inputs(root, &o)?;
    let started = std::time::Instant::now();
    let a = cargo_args(&o);
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])?;
    println!("terrain-score: report {}/report.md, {:.1} s including the build", o.out, started.elapsed().as_secs_f64());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // spec: BUILD-029
    #[test]
    fn no_options_run_the_example_with_the_defaults() {
        let o = parse_args(&[], "2026-10-31").unwrap();
        assert_eq!((o.store.as_str(), o.dem.as_str(), o.sites.as_str(), o.maps), (DEFAULT_STORE, DEFAULT_DEM, DEFAULT_SITES, false));
        assert_eq!(o.out, "target/review/terrain-score/2026-10-31");
        let c = cargo_args(&o);
        assert_eq!(&c[..7], &a(&["run", "--release", "-p", "planet", "--example", "terrain_score", "--"])[..]);
        assert!(c.contains(&DEFAULT_STORE.to_string()) && !c.contains(&"--maps".to_string()));
    }

    // spec: BUILD-029
    #[test]
    fn every_option_is_passed_through() {
        let o = parse_args(&a(&["--store", "S", "--dem", "D", "--sites", "F.toml", "--out", "O", "--maps"]), "2026-10-31").unwrap();
        assert_eq!(o, Opts { store: "S".into(), dem: "D".into(), sites: "F.toml".into(), out: "O".into(), maps: true });
        let c = cargo_args(&o);
        for pair in [["--store", "S"], ["--dem", "D"], ["--sites", "F.toml"], ["--out", "O"]] {
            assert!(c.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]), "{pair:?} in {c:?}");
        }
        assert_eq!(c.last().unwrap(), "--maps");
    }

    // spec: BUILD-029
    #[test]
    fn an_unknown_option_is_refused_naming_the_known_ones_and_a_missing_value_too() {
        let e = parse_args(&a(&["--fast"]), "d").unwrap_err();
        assert!(e.contains("'--fast'") && KNOWN.split(", ").all(|k| e.contains(k)), "{e}");
        assert!(parse_args(&a(&["--dem"]), "d").unwrap_err().contains("--dem needs a value"));
    }

    // spec: BUILD-029
    #[test]
    fn a_missing_directory_is_named_with_its_option() {
        let root = std::env::temp_dir().join(format!("xtask-terrain-score-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("store")).unwrap();
        std::fs::write(root.join("sites.toml"), "").unwrap();
        let mut o = parse_args(&a(&["--store", "store", "--dem", "tiles", "--sites", "sites.toml"]), "d").unwrap();
        let e = check_inputs(&root, &o).unwrap_err();
        assert!(e.contains("tiles") && e.contains("--dem"), "{e}");
        o.store = "nostore".into();
        let e = check_inputs(&root, &o).unwrap_err();
        assert!(e.contains("nostore") && e.contains("--store"), "{e}");
        std::fs::create_dir_all(root.join("tiles")).unwrap();
        o.store = "store".into();
        assert!(check_inputs(&root, &o).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }
}
