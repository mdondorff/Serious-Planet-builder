//! Loading the look file for a run (LOOK-005, ADR 0042 point 6): `--look F`, else the file beside `--world` (`<stem>.look.toml`),
//! else the Earth defaults. The look is presentation data outside the world definition and every cache key, so a run loads it
//! after the options are checked and before any world is built or device opened, and a bad file stops the run with the path, the
//! key and the value. The viewer and the test harness take a body's parameters from [`LookSet::params`] when they build a plan
//! (`LitPlan::fixed_with` / `adaptive_with`); nothing generated depends on it.

use crate::Options;
use planet_frame::look::file::{parse_look, sibling_file_name, LookFile, LookParams};
use planet_world_def::BodyId;
use std::path::{Path, PathBuf};

/// Where a run's look came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LookSource {
    /// No file: every body is the Earth default.
    Defaults,
    /// `--look F`.
    Explicit(PathBuf),
    /// `<stem>.look.toml` beside the `--world` file.
    BesideWorld(PathBuf),
}

/// The loaded look of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct LookSet {
    file: LookFile,
    source: LookSource,
}

impl LookSet {
    /// The Earth defaults for every body.
    pub fn defaults() -> LookSet {
        LookSet { file: LookFile::empty(), source: LookSource::Defaults }
    }

    pub fn source(&self) -> &LookSource {
        &self.source
    }

    /// The look of a body: its entry in the file, else the Earth default.
    pub fn params(&self, body: BodyId) -> LookParams {
        self.file.body(body)
    }

    pub fn file(&self) -> &LookFile {
        &self.file
    }

    /// The line a run prints when it uses a file; `None` for the defaults (nothing is printed, so default runs print what they
    /// always did).
    pub fn describe(&self) -> Option<String> {
        let (how, path) = match &self.source {
            LookSource::Defaults => return None,
            LookSource::Explicit(p) => ("--look", p),
            LookSource::BesideWorld(p) => ("beside the world", p),
        };
        let bodies: Vec<String> = self.file.bodies().map(|(b, _)| b.0.to_string()).collect();
        let entries =
            if bodies.is_empty() { "no body entries, Earth defaults".to_string() } else { format!("bodies {}", bodies.join(", ")) };
        Some(format!("look: {} ({how}; schema {}; {entries})", path.display(), planet_frame::look::file::SCHEMA_VERSION))
    }
}

/// The look file that sits beside a definition file: `<dir>/<stem>.look.toml`.
pub fn beside_world(world: &Path) -> PathBuf {
    let stem = world.file_stem().map_or_else(|| "world".to_string(), |s| s.to_string_lossy().into_owned());
    world.with_file_name(sibling_file_name(&stem))
}

fn read(path: &Path) -> Result<LookFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading look file {}: {e}", path.display()))?;
    parse_look(&text).map_err(|e| format!("look file {}: {e}", path.display()))
}

/// Resolve and load the look of a run. An explicit `--look` that cannot be read is an error; a sibling file that does not exist is
/// simply absent; one that exists and is bad is an error.
pub fn load(o: &Options) -> Result<LookSet, String> {
    if let Some(p) = &o.look {
        return Ok(LookSet { file: read(p)?, source: LookSource::Explicit(p.clone()) });
    }
    if let Some(w) = &o.world {
        let p = beside_world(w);
        if p.is_file() {
            return Ok(LookSet { file: read(&p)?, source: LookSource::BesideWorld(p) });
        }
    }
    Ok(LookSet::defaults())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(args: &[&str]) -> Options {
        crate::parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("planet-lookload-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const LOOK: &str = "schema_version = 1\nkind = \"look\"\n[body.0.units]\nsolar_illuminance_lux = 50000.0\n";

    // spec: LOOK-005
    #[test]
    fn no_file_means_the_earth_defaults_and_prints_nothing() {
        let set = load(&options(&["editor", "--no-window"])).unwrap();
        assert_eq!(set, LookSet::defaults());
        assert_eq!(set.describe(), None);
        assert_eq!(set.params(BodyId::PLANET), LookParams::default());
        // A world file with no look file beside it: the same.
        let d = scratch("nofile");
        let world = d.join("w.toml");
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 1\n[body.0]\nradius_m = 6371000.0\n").unwrap();
        let set = load(&options(&["editor", "--no-window", "--world", world.to_str().unwrap()])).unwrap();
        assert_eq!(set.source(), &LookSource::Defaults);
    }

    // spec: LOOK-005
    #[test]
    fn an_explicit_file_and_the_file_beside_the_world_are_found_and_the_explicit_one_wins() {
        let d = scratch("found");
        let world = d.join("my.world.toml");
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 1\n[body.0]\nradius_m = 6371000.0\n").unwrap();
        assert_eq!(beside_world(&world), d.join("my.world.look.toml"));
        std::fs::write(beside_world(&world), LOOK).unwrap();
        let w = world.to_str().unwrap();
        let set = load(&options(&["editor", "--no-window", "--world", w])).unwrap();
        assert_eq!(set.source(), &LookSource::BesideWorld(beside_world(&world)));
        assert_eq!(set.params(BodyId(0)).units.solar_illuminance_lux, 50_000.0);
        assert_eq!(set.params(BodyId(1)), LookParams::default(), "a body without an entry is the Earth default");
        assert!(set.describe().unwrap().contains("beside the world") && set.describe().unwrap().contains("bodies 0"));
        // --look names another file and wins over the one beside the world.
        let other = d.join("other.toml");
        std::fs::write(&other, LOOK.replace("50000.0", "75000.0")).unwrap();
        let set = load(&options(&["test-render", "--world", w, "--look", other.to_str().unwrap()])).unwrap();
        assert_eq!(set.source(), &LookSource::Explicit(other.clone()));
        assert_eq!(set.params(BodyId(0)).units.solar_illuminance_lux, 75_000.0);
    }

    // spec: LOOK-005
    #[test]
    fn a_bad_or_missing_file_stops_the_run_naming_the_path_the_key_and_the_value() {
        let d = scratch("bad");
        let bad = d.join("bad.toml");
        std::fs::write(&bad, LOOK.replace("50000.0", "-3.0")).unwrap();
        let e = load(&options(&["editor", "--no-window", "--look", bad.to_str().unwrap()])).unwrap_err();
        assert!(e.contains("bad.toml") && e.contains("body.0.units.solar_illuminance_lux") && e.contains("-3.0"), "{e}");
        let future = d.join("future.toml");
        std::fs::write(&future, "schema_version = 9\nkind = \"look\"\n").unwrap();
        let e = load(&options(&["editor", "--no-window", "--look", future.to_str().unwrap()])).unwrap_err();
        assert!(e.contains("future.toml") && e.contains("newer") && e.contains("update the program"), "{e}");
        let missing = d.join("missing.toml");
        let e = load(&options(&["editor", "--no-window", "--look", missing.to_str().unwrap()])).unwrap_err();
        assert!(e.contains("reading look file") && e.contains("missing.toml"), "{e}");
        // A bad file beside the world is refused too (it is not silently ignored).
        let world = d.join("w.toml");
        std::fs::write(&world, "schema_version = 1\n[world]\nseed = 1\n[body.0]\nradius_m = 6371000.0\n").unwrap();
        std::fs::write(beside_world(&world), "kind = \"look\"\n").unwrap();
        let e = load(&options(&["editor", "--no-window", "--world", world.to_str().unwrap()])).unwrap_err();
        assert!(e.contains("w.look.toml") && e.contains("schema_version"), "{e}");
    }

    // spec: LOOK-005
    #[test]
    fn the_run_modes_load_it_before_anything_else_and_generate_refuses_it() {
        let d = scratch("modes");
        let bad = d.join("bad.toml");
        std::fs::write(&bad, "schema_version = 1\nkind = \"look\"\n[body.0.exposure]\nmin_ev100 = 30.0\nmax_ev100 = 20.0\n").unwrap();
        let b = bad.to_str().unwrap();
        for mode in [vec!["editor", "--smoke", "--no-window", "--look", b], vec!["test-render", "--smoke", "--look", b]] {
            let args: Vec<String> = mode.iter().map(|s| s.to_string()).collect();
            let e = crate::run(&args, &mut Vec::new()).unwrap_err();
            assert!(e.contains("body.0.exposure.min_ev100") && e.contains("30.0"), "{mode:?}: {e}");
        }
        let good = d.join("good.toml");
        std::fs::write(&good, LOOK).unwrap();
        let args: Vec<String> =
            ["editor", "--smoke", "--no-window", "--look", good.to_str().unwrap()].iter().map(|s| s.to_string()).collect();
        let mut out = Vec::new();
        crate::run(&args, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("look: ") && text.contains("good.toml") && text.contains("smoke ok"), "{text}");
        // Without the option the output has no look line at all.
        let mut out = Vec::new();
        crate::run(&["editor".into(), "--smoke".into(), "--no-window".into()], &mut out).unwrap();
        assert!(!String::from_utf8(out).unwrap().contains("look:"));
        let args: Vec<String> = ["generate", "--smoke", "--look", good.to_str().unwrap()].iter().map(|s| s.to_string()).collect();
        let e = crate::run(&args, &mut Vec::new()).unwrap_err();
        assert!(e.contains("--look only applies to `editor` and `test-render`"), "{e}");
    }
}
