//! `cargo xtask det-diff <a> <b>`: compare two determinism manifests (GEN-003, ADR 0010). Each line is
//! `<kind> <key> <value>`; the report names every differing entry, version lines first.

use std::path::Path;

use crate::util::Res;

pub fn run(root: &Path, args: &[String]) -> Res {
    let [a, b] = args else {
        return Err("usage: cargo xtask det-diff <manifest-a> <manifest-b>".into());
    };
    let read = |p: &str| std::fs::read_to_string(root.join(p)).map_err(|e| format!("reading {p}: {e}"));
    let (text, ok) = report(a, b, &read(a)?, &read(b)?);
    eprint!("{text}");
    if ok {
        Ok(())
    } else {
        Err("manifests differ (see above)".into())
    }
}

/// The full report for two manifests and whether they are equal. Lists every differing entry.
pub fn report(a: &str, b: &str, ta: &str, tb: &str) -> (String, bool) {
    let diffs = diff(ta, tb);
    if diffs.is_empty() {
        return (format!("det-diff: {} entries identical in {a} and {b}\n", ta.lines().count()), true);
    }
    let mut s = format!("det-diff: {} of {} entries differ ({a} vs {b}):\n", diffs.len(), ta.lines().count().max(tb.lines().count()));
    for d in &diffs {
        s.push_str(&format!("  {d}\n"));
    }
    if diffs.iter().any(|d| d.starts_with("version")) {
        s.push_str("det-diff: version lines differ, so the manifests come from different builds; this is not numeric drift\n");
    }
    (s, false)
}

/// Differences between two manifests, keyed by `<kind> <key>`, in the order of `a` and then entries only in `b`.
pub fn diff(a: &str, b: &str) -> Vec<String> {
    let parse = |t: &str| -> Vec<(String, String)> {
        t.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| match l.rsplit_once(' ') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (l.to_string(), String::new()),
            })
            .collect()
    };
    let (pa, pb) = (parse(a), parse(b));
    let mut out = Vec::new();
    for (name, p) in [("a", &pa), ("b", &pb)] {
        for (i, (k, _)) in p.iter().enumerate() {
            if p[..i].iter().any(|(kk, _)| kk == k) {
                out.push(format!("{k}: duplicate key in manifest {name}"));
            }
        }
    }
    let find = |p: &[(String, String)], k: &str| p.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone());
    for (k, va) in &pa {
        match find(&pb, k) {
            Some(vb) if &vb == va => {}
            Some(vb) => out.push(format!("{k}: {va} vs {vb}{}", height_delta(k, va, &vb))),
            None => out.push(format!("{k}: {va} vs (missing)")),
        }
    }
    for (k, vb) in &pb {
        if find(&pa, k).is_none() {
            out.push(format!("{k}: (missing) vs {vb}"));
        }
    }
    out.sort_by_key(|d| !d.starts_with("version"));
    out
}

/// For `terrain` and `erosion` entries (raw `f64` height bits): the heights, their difference in metres and the distance in
/// ulps, so a drift report says how large the drift is.
fn height_delta(key: &str, a: &str, b: &str) -> String {
    if !(key.starts_with("terrain ") || key.starts_with("erosion ")) {
        return String::new();
    }
    let (Ok(ba), Ok(bb)) = (u64::from_str_radix(a, 16), u64::from_str_radix(b, 16)) else {
        return String::new();
    };
    let (ha, hb) = (f64::from_bits(ba), f64::from_bits(bb));
    // Order-preserving map of the bits, so the distance counts representable steps across zero too.
    let ord = |x: u64| if x >> 63 == 1 { !x } else { x | 1 << 63 };
    let ulps = ord(ba).abs_diff(ord(bb));
    format!(" ({ha} m vs {hb} m, delta {:e} m, {ulps} ulp)", hb - ha)
}

#[cfg(test)]
mod tests {
    use super::diff;

    // spec: GEN-003
    #[test]
    fn identical_manifests_have_no_differences() {
        let m = "version generator 2\nheight 0/0/0/0 00ff\nmesh 0/0/0/0 0a0a\n";
        assert!(diff(m, m).is_empty());
    }

    // spec: GEN-003
    #[test]
    fn a_difference_names_the_tile_and_both_values() {
        let a = "version generator 2\nheight 0/0/0/0 00ff\nheight 3/28/1/2 1111\n";
        let b = "version generator 2\nheight 0/0/0/0 00ff\nheight 3/28/1/2 2222\n";
        assert_eq!(diff(a, b), vec!["height 3/28/1/2: 1111 vs 2222".to_string()]);
    }

    // spec: GEN-003
    #[test]
    fn missing_entries_and_version_lines_are_reported_first() {
        let a = "version generator 2\nheight 0/0/0/0 00ff\nmesh 0/0/0/0 0a0a\n";
        let b = "version generator 3\nheight 0/0/0/0 00ff\n";
        assert_eq!(diff(a, b), vec!["version generator: 2 vs 3".to_string(), "mesh 0/0/0/0: 0a0a vs (missing)".to_string()]);
    }

    // spec: GEN-003
    #[test]
    fn duplicate_keys_are_reported() {
        let a = "height 0/0/0/0 00ff\nheight 0/0/0/0 00ff\n";
        assert_eq!(diff(a, "height 0/0/0/0 00ff\n"), vec!["height 0/0/0/0: duplicate key in manifest a".to_string()]);
    }

    // spec: GEN-003
    #[test]
    fn report_lists_every_entry_and_names_different_builds() {
        let a: String = (0..80).map(|i| format!("height 0/7/{i}/0 {i:04x}\n")).collect();
        let b: String = (0..80).map(|i| format!("height 0/7/{i}/0 {:04x}\n", i + 1)).collect();
        let (text, ok) = super::report("w", "l", &a, &b);
        assert!(!ok);
        assert_eq!(text.lines().filter(|l| l.starts_with("  height")).count(), 80, "every differing entry is listed");
        assert!(!text.contains("different builds"));
        let (text, ok) = super::report("w", "l", "version mesh 2\n", "version mesh 3\n");
        assert!(!ok && text.contains("different builds"), "{text}");
        assert!(super::report("w", "l", &a, &a).1);
    }

    // spec: GEN-003
    #[test]
    fn a_height_difference_reports_the_heights_the_delta_and_the_ulps() {
        let one = 1.0f64.to_bits();
        let a = format!(
            "terrain 3/0.3/-0.7 {one:016x}
"
        );
        let b = format!(
            "terrain 3/0.3/-0.7 {:016x}
",
            one + 1
        );
        let d = diff(&a, &b);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("1 ulp") && d[0].contains("2.220446049250313e-16 m"), "{d:?}");
    }
}
