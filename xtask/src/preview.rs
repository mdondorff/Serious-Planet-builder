//! `cargo xtask preview world --seed N [--out DIR]` (GEN-103): the whole-planet preview of the generator's default class source. The work is the
//! `preview_world` example of planet-generators (CPU only, no store); this file parses the options and runs it in release.

use crate::util::{run, Res};
use std::path::Path;

const KNOWN: &str = "--seed N, --out DIR";

#[derive(Debug, PartialEq)]
pub struct Opts {
    pub seed: u64,
    pub out: Option<String>,
}

pub fn parse_args(args: &[String]) -> Res<Opts> {
    match args.first().map(String::as_str) {
        Some("world") => {}
        Some(other) => return Err(format!("unknown preview '{other}' (known: world)")),
        None => return Err("usage: cargo xtask preview world --seed N [--out DIR]".into()),
    }
    let mut o = Opts { seed: 1, out: None };
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--seed" => o.seed = value()?.parse().map_err(|_| "--seed needs a non-negative integer".to_string())?,
            "--out" => o.out = Some(value()?),
            other => return Err(format!("unknown preview option '{other}' (known: {KNOWN})")),
        }
    }
    Ok(o)
}

/// The `cargo` arguments that run the example.
pub fn cargo_args(o: &Opts) -> Vec<String> {
    let mut v: Vec<String> = ["run", "--release", "-q", "-p", "planet-generators", "--example", "preview_world", "--", "--seed"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    v.push(o.seed.to_string());
    if let Some(out) = &o.out {
        v.push("--out".into());
        v.push(out.clone());
    }
    v
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let o = parse_args(args)?;
    let a = cargo_args(&o);
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    run(root, "cargo", &refs, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // spec: GEN-103
    #[test]
    fn the_seed_and_the_out_directory_are_passed_to_the_example() {
        let o = parse_args(&a(&["world", "--seed", "7", "--out", "D"])).unwrap();
        assert_eq!(o, Opts { seed: 7, out: Some("D".into()) });
        let c = cargo_args(&o);
        assert!(c.windows(2).any(|w| w == ["--seed", "7"]) && c.windows(2).any(|w| w == ["--out", "D"]), "{c:?}");
        assert_eq!(parse_args(&a(&["world"])).unwrap(), Opts { seed: 1, out: None });
    }

    // spec: GEN-103
    #[test]
    fn an_unknown_preview_or_option_is_refused_naming_what_is_known() {
        assert!(parse_args(&a(&[])).unwrap_err().contains("usage"));
        assert!(parse_args(&a(&["moon"])).unwrap_err().contains("world"));
        let e = parse_args(&a(&["world", "--fast"])).unwrap_err();
        assert!(e.contains("'--fast'") && e.contains("--seed N") && e.contains("--out DIR"), "{e}");
        assert!(parse_args(&a(&["world", "--seed"])).unwrap_err().contains("needs a value"));
        assert!(parse_args(&a(&["world", "--seed", "x"])).unwrap_err().contains("integer"));
    }
}
