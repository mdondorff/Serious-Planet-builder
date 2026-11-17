//! `cargo xtask river-gate <store dir | window> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--no-key-check]` (BUILD-055): the river gate's numbers on a region. Runs the `river_gate` example of
//! `planet-cache` in release under the heavy lock: the first argument is a commit's store (counted over its tiles, on the window `--window`, default R1h) or a window, measured on the start
//! surface. A window is `face,i0,i1,j0,j1,seed` in regional cells or one of the canonical names of [`WINDOWS`] (lead, 2026-11-14). The report is printed, never judged: the exit status is 0 when the run ends.

use crate::util::{run, Res};
use std::path::Path;

/// The options the example takes after the target, each with a value.
const OPTIONS: [&str; 6] = ["--window", "--margin", "--min-cells", "--d", "--threads", "--network-km2"];

/// The canonical region windows (lead, 2026-11-14): `face,i0,i1,j0,j1,seed`, 620-cell cores (with the gate's 200-cell margin SPIKE-111's 1,020-cell windows). R1h is the E1 region stores'
/// window (r3a-e1, r3a-p2, r3a-p2e). M1 (mountain belt) and F1 (low-gradient river landscape) are SPIKE-119's (spikes, 2026-11-15).
pub const WINDOWS: [(&str, &str); 5] = [
    ("r1h", "1,8056,8675,760,1379,14"),
    ("r3a", "4,368,987,4720,5339,15"),
    ("r3b", "2,9968,10587,8304,8923,15"),
    ("m1", "4,6600,7219,10200,10819,15"),
    ("f1", "4,4800,5419,3300,3919,15"),
];

/// A window argument: a canonical name (any case) becomes its numbers; anything else is passed as written.
pub fn window_of(arg: &str) -> String {
    WINDOWS.iter().find(|(n, _)| n.eq_ignore_ascii_case(arg)).map_or_else(|| arg.to_string(), |(_, w)| w.to_string())
}

/// The example's arguments for the xtask's: the target becomes `--window` or `--store`; an unknown option, a missing value or a second window is refused.
pub fn example_args(args: &[String]) -> Result<Vec<String>, String> {
    let usage = "usage: cargo xtask river-gate <store dir | face,i0,i1,j0,j1,seed | r1h | r3a | r3b | m1 | f1 | grid=file,w,h,sea_level_m> [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--network-km2 A,B] [--no-key-check]";
    let Some(target) = args.first().filter(|a| !a.starts_with("--")) else { return Err(usage.to_string()) };
    let mut out = Vec::new();
    let window = window_of(target);
    let grid = target.strip_prefix("grid=");
    // A raw grid (BUILD-059's GLO-30 input) is a window of its own: like a window target it takes no --window and no --no-key-check.
    let window_target = grid.is_some() || (window.split(',').count() == 6 && window.split(',').all(|x| x.parse::<usize>().is_ok()));
    if let Some(g) = grid {
        out.extend(["--grid".to_string(), g.to_string()]);
    } else if window_target {
        out.extend(["--window".to_string(), window]);
    } else {
        out.extend(["--store".to_string(), target.clone()]);
    }
    let mut k = 1;
    while k < args.len() {
        let name = args[k].as_str();
        if name == "--no-key-check" {
            if window_target {
                return Err(format!("--no-key-check reads a store: the target is a window; {usage}"));
            }
            out.push(name.to_string());
            k += 1;
            continue;
        }
        if !OPTIONS.contains(&name) {
            return Err(format!("unknown river-gate option '{name}' (known: {}); {usage}", OPTIONS.join(", ")));
        }
        let Some(v) = args.get(k + 1).filter(|v| !v.starts_with("--")) else { return Err(format!("{name} takes a value; {usage}")) };
        if name == "--window" && window_target {
            return Err(format!("the target is a window already: --window goes with a store; {usage}"));
        }
        out.extend([name.to_string(), if name == "--window" { window_of(v) } else { v.clone() }]);
        k += 2;
    }
    Ok(out)
}

pub fn run_gate(root: &Path, args: &[String]) -> Res {
    let tail = example_args(args)?;
    let mut cmd: Vec<&str> = vec!["run", "--release", "-q", "-p", "planet-cache", "--example", "river_gate", "--"];
    cmd.extend(tail.iter().map(String::as_str));
    run(root, "cargo", &cmd, &[])
}

/// One row of `--table`: its label and the gate's arguments for it.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub label: String,
    pub args: Vec<String>,
}

/// The rows of `river-gate --table <row>... [--windows W,W...] [options]` (BUILD-057): a row is a window (its start surface), `<store dir>@<window>` (the store on that window) or a store dir (on
/// `--window`, default R1h); `--windows` adds one start-surface row per window. The other options go to every row. A row's label is the store's directory name or "start", then the window.
pub fn table_rows(args: &[String]) -> Result<Vec<TableRow>, String> {
    let usage = "usage: cargo xtask river-gate --table <window | store dir@window | store dir | grid=file,w,h,sea_level_m>... [--windows W,W...] [--window W] [--margin M] [--min-cells N] [--d D] [--threads T] [--network-km2 A,B] [--no-key-check]";
    let (mut targets, mut common, mut windows) = (Vec::new(), Vec::new(), Vec::new());
    let mut k = 0;
    while k < args.len() {
        let a = &args[k];
        if a == "--windows" {
            let v = args.get(k + 1).filter(|v| !v.starts_with("--")).ok_or_else(|| format!("--windows takes a value; {usage}"))?;
            windows.extend(v.split(',').filter(|w| !w.is_empty()).map(str::to_string));
            k += 2;
        } else if a == "--no-key-check" {
            common.push(a.clone());
            k += 1;
        } else if a.starts_with("--") {
            if !OPTIONS.contains(&a.as_str()) {
                return Err(format!("unknown river-gate option '{a}' (known: --windows, {}, --no-key-check); {usage}", OPTIONS.join(", ")));
            }
            let v = args.get(k + 1).filter(|v| !v.starts_with("--")).ok_or_else(|| format!("{a} takes a value; {usage}"))?;
            common.extend([a.clone(), v.clone()]);
            k += 2;
        } else {
            targets.push(a.clone());
            k += 1;
        }
    }
    targets.extend(windows);
    if targets.is_empty() {
        return Err(format!("--table needs at least one row; {usage}"));
    }
    let is_window = |t: &str| {
        let w = window_of(t);
        w.split(',').count() == 6 && w.split(',').all(|x| x.parse::<usize>().is_ok())
    };
    let default_window = common.iter().position(|a| a == "--window").map(|i| common[i + 1].clone()).unwrap_or_else(|| "r1h".to_string());
    let base: Vec<String> = {
        // `--window` names a store row's window; a window row is its own window.
        let mut b = Vec::new();
        let mut i = 0;
        while i < common.len() {
            if common[i] == "--window" {
                i += 2;
            } else {
                b.push(common[i].clone());
                i += 1;
            }
        }
        b
    };
    targets
        .iter()
        .map(|t| {
            let (target, window) = match t.rsplit_once('@') {
                _ if t.starts_with("grid=") => (t.clone(), None),
                Some((dir, w)) if !dir.is_empty() && is_window(w) => (dir.to_string(), Some(w.to_string())),
                _ if is_window(t) => (t.clone(), None),
                _ => (t.clone(), Some(default_window.clone())),
            };
            let mut a = vec![target.clone()];
            let label = match &window {
                Some(w) => {
                    a.extend(["--window".to_string(), w.clone()]);
                    let name = Path::new(&target).file_name().map_or_else(|| target.clone(), |n| n.to_string_lossy().into_owned());
                    format!("{name} @ {w}")
                }
                None => match t.strip_prefix("grid=") {
                    // A grid row's label is its file's stem (a GLO-30 tile: glo-N47E011).
                    Some(g) => {
                        let file = g.split(',').next().unwrap_or(g);
                        format!(
                            "grid @ {}",
                            Path::new(file).file_stem().map_or_else(|| file.to_string(), |n| n.to_string_lossy().into_owned())
                        )
                    }
                    None => format!("start @ {t}"),
                },
            };
            let mut args = a;
            args.extend(base.iter().filter(|x| window.is_some() || x.as_str() != "--no-key-check").cloned());
            Ok(TableRow { label, args: example_args(&args)? })
        })
        .collect()
}

/// The `RIVER-GATE fields:` line of a gate run's output as (column, value).
pub fn parse_fields(out: &str) -> Result<Vec<(String, String)>, String> {
    let line = out.lines().find_map(|l| l.strip_prefix("RIVER-GATE fields: ")).ok_or("the gate printed no `RIVER-GATE fields:` line")?;
    line.split_whitespace()
        .map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).ok_or_else(|| format!("not name=value: '{kv}'")))
        .collect()
}

/// The table of the rows' fields as (markdown, CSV): one row per run, the columns in the first run's order (every run prints the same columns); a value a row lacks is empty.
pub fn render_table(rows: &[(String, Vec<(String, String)>)]) -> (String, String) {
    let mut cols: Vec<String> = Vec::new();
    for (_, f) in rows {
        for (k, _) in f {
            if !cols.contains(k) {
                cols.push(k.clone());
            }
        }
    }
    let value = |f: &[(String, String)], c: &str| f.iter().find(|(k, _)| k == c).map(|(_, v)| v.clone()).unwrap_or_default();
    let mut md = format!("| row | {} |\n|---|{}\n", cols.join(" | "), "---|".repeat(cols.len()));
    let mut csv = format!("row,{}\n", cols.join(","));
    for (label, f) in rows {
        let vals: Vec<String> = cols.iter().map(|c| value(f, c)).collect();
        md.push_str(&format!("| {label} | {} |\n", vals.join(" | ")));
        csv.push_str(&format!("\"{}\",{}\n", label.replace('"', "\"\""), vals.join(",")));
    }
    (md, csv)
}

/// `river-gate --table`: runs the gate once per row (release, under the caller's heavy lock), prints each run's lines, and writes the table to `target/review/river-gate-table.md` and `.csv`.
pub fn run_table(root: &Path, args: &[String]) -> Res {
    let rows = table_rows(args)?;
    let mut table = Vec::new();
    for row in &rows {
        eprintln!("river-gate --table: {}", row.label);
        let mut cmd: Vec<&str> = vec!["run", "--release", "-q", "-p", "planet-cache", "--example", "river_gate", "--"];
        cmd.extend(row.args.iter().map(String::as_str));
        let out = crate::util::capture(root, "cargo", &cmd)?;
        print!("{out}");
        table.push((row.label.clone(), parse_fields(&out).map_err(|e| format!("{}: {e}", row.label))?));
    }
    let (md, csv) = render_table(&table);
    let dir = root.join("target").join("review");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, text) in [("river-gate-table.md", &md), ("river-gate-table.csv", &csv)] {
        std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))?;
    }
    println!("{md}");
    println!("RIVER-GATE table: {} rows, {} and .csv", table.len(), dir.join("river-gate-table.md").display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUILD-057
    #[test]
    fn table_rows_are_windows_stores_on_their_window_and_the_windows_option() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let rows = table_rows(&s(&["r1h", "C:/w/b-m1@m1", "C:/w/a", "--windows", "f1,r3a", "--d", "30", "--no-key-check"])).unwrap();
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["start @ r1h", "b-m1 @ m1", "a @ r1h", "start @ f1", "start @ r3a"]);
        assert_eq!(rows[0].args, s(&["--window", "1,8056,8675,760,1379,14", "--d", "30"]), "a window row drops --no-key-check");
        assert_eq!(rows[1].args, s(&["--store", "C:/w/b-m1", "--window", "4,6600,7219,10200,10819,15", "--d", "30", "--no-key-check"]));
        assert_eq!(rows[2].args[..4], s(&["--store", "C:/w/a", "--window", "1,8056,8675,760,1379,14"])[..]);
        // --window names the default window of a store row.
        let r = table_rows(&s(&["C:/w/a", "--window", "m1"])).unwrap();
        assert_eq!((r[0].label.as_str(), &r[0].args[2..4]), ("a @ m1", &s(&["--window", "4,6600,7219,10200,10819,15"])[..]));
        assert!(table_rows(&s(&["--d", "30"])).unwrap_err().contains("at least one row"));
        assert!(table_rows(&s(&["r1h", "--fast", "1"])).unwrap_err().contains("unknown river-gate option '--fast'"));
        assert!(table_rows(&s(&["r1h", "--windows"])).unwrap_err().contains("--windows takes a value"));
    }

    // spec: BUILD-057
    #[test]
    fn the_fields_line_parses_and_renders_one_row_per_run_with_empty_cells_for_missing_columns() {
        let out = "RIVER-GATE rivers: 3 river cells\nRIVER-GATE fields: river_cells=3 basin_pct=33.33\n";
        let f = parse_fields(out).unwrap();
        assert_eq!(f, vec![("river_cells".to_string(), "3".to_string()), ("basin_pct".to_string(), "33.33".to_string())]);
        assert!(parse_fields("RIVER-GATE rivers: 3").unwrap_err().contains("no `RIVER-GATE fields:` line"));
        assert!(parse_fields("RIVER-GATE fields: a=1 b").unwrap_err().contains("not name=value: 'b'"));
        let (md, csv) =
            render_table(&[("x @ r1h".to_string(), f), ("y @ m1".to_string(), vec![("river_cells".to_string(), "5".to_string())])]);
        assert_eq!(md, "| row | river_cells | basin_pct |\n|---|---|---|\n| x @ r1h | 3 | 33.33 |\n| y @ m1 | 5 |  |\n");
        assert_eq!(csv, "row,river_cells,basin_pct\n\"x @ r1h\",3,33.33\n\"y @ m1\",5,\n");
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    // spec: BUILD-055
    #[test]
    fn the_target_is_a_window_or_a_store() {
        // The canonical names (lead, 2026-11-14): R1h is the E1 region stores' window; R3a and R3b are SPIKE-111's face 4 and face 2 windows of seed 15; M1 and F1 are SPIKE-119's (spikes, 2026-11-15).
        assert_eq!(example_args(&s(&["r1h"])).unwrap(), s(&["--window", "1,8056,8675,760,1379,14"]));
        assert_eq!(example_args(&s(&["r3a"])).unwrap(), s(&["--window", "4,368,987,4720,5339,15"]));
        assert_eq!(example_args(&s(&["M1"])).unwrap(), s(&["--window", "4,6600,7219,10200,10819,15"]));
        assert_eq!(example_args(&s(&["f1"])).unwrap(), s(&["--window", "4,4800,5419,3300,3919,15"]));
        assert_eq!(example_args(&s(&["R3B"])).unwrap(), s(&["--window", "2,9968,10587,8304,8923,15"]));
        assert_eq!(
            example_args(&s(&["store", "--window", "r1h"])).unwrap(),
            s(&["--store", "store", "--window", "1,8056,8675,760,1379,14"])
        );
        assert_eq!(
            example_args(&s(&["1,8056,8675,760,1379,14", "--d", "30"])).unwrap(),
            s(&["--window", "1,8056,8675,760,1379,14", "--d", "30"])
        );
        assert_eq!(
            example_args(&s(&["C:/DEV/SeriousPlanetData/worlds/r3a-e1", "--min-cells", "30"])).unwrap(),
            s(&["--store", "C:/DEV/SeriousPlanetData/worlds/r3a-e1", "--min-cells", "30"])
        );
        assert_eq!(example_args(&s(&["store", "--window", "2,1,2,3,4,5"])).unwrap(), s(&["--store", "store", "--window", "2,1,2,3,4,5"]));
    }

    // spec: TEST-137, BUILD-059
    #[test]
    fn a_grid_target_becomes_the_grid_input_with_its_file_stem_as_the_row_label() {
        assert_eq!(
            example_args(&s(&["grid=C:/d/glo-N47E011.f64,1000,1093,0", "--network-km2", "1,75"])).unwrap(),
            s(&["--grid", "C:/d/glo-N47E011.f64,1000,1093,0", "--network-km2", "1,75"])
        );
        assert!(example_args(&s(&["grid=a.f64,1,2,0", "--window", "r1h"])).unwrap_err().contains("--window goes with a store"));
        assert!(example_args(&s(&["grid=a.f64,1,2,0", "--no-key-check"])).unwrap_err().contains("reads a store"));
        let rows = table_rows(&s(&["grid=C:/d/glo-N47E011.f64,1000,1093,0", "r1h", "--no-key-check", "--network-km2", "2,50"])).unwrap();
        assert_eq!(rows[0].label, "grid @ glo-N47E011");
        assert_eq!(rows[0].args, s(&["--grid", "C:/d/glo-N47E011.f64,1000,1093,0", "--network-km2", "2,50"]));
        assert_eq!(rows[1].label, "start @ r1h");
    }

    // spec: BUILD-055
    #[test]
    fn a_missing_target_an_unknown_option_a_missing_value_and_two_windows_are_refused() {
        assert!(example_args(&[]).is_err());
        assert!(example_args(&s(&["--d", "3"])).is_err());
        assert!(example_args(&s(&["r3a", "--fast"])).unwrap_err().contains("unknown river-gate option '--fast'"));
        assert!(example_args(&s(&["r3a", "--d"])).unwrap_err().contains("--d takes a value"));
        assert!(example_args(&s(&["r3a", "--window", "1,2,3,4,5,6"])).unwrap_err().contains("a window already"));
        assert!(example_args(&s(&["r3a", "--no-key-check"])).unwrap_err().contains("--no-key-check reads a store"));
        assert_eq!(example_args(&s(&["p2e", "--no-key-check", "--d", "5"])).unwrap(), s(&["--store", "p2e", "--no-key-check", "--d", "5"]));
    }
}
