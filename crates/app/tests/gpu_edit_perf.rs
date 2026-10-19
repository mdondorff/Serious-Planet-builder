//! Tier C, SPIKE-13 / M6 exit: the edit-latency scene (`test-render --perf --scene edit-latency`) runs end to end on the software adapter in
//! smoke mode: the stamp is put through `set_definition`, the preview is drawn, the tiles become exact, the edit is taken back, and the
//! report table is printed and written. The numbers are not a measurement (a smoke run never is); the scene's own logic is checked.

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut buf = Vec::new();
    planet::run(&args, &mut buf).map(|_| String::from_utf8(buf).unwrap())
}

// spec: TEST-019, TEST-041
#[test]
fn the_edit_latency_scene_runs_end_to_end_in_smoke_mode_and_prints_its_table() {
    let logs = std::env::temp_dir().join(format!("planet-edit-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&logs);
    std::fs::create_dir_all(&logs).unwrap();
    let out = run(&[
        "test-render",
        "--perf-smoke",
        "--scene",
        "edit-latency",
        "--adapter",
        "software",
        // The software rasteriser takes every core of a small runner; below-normal generation workers then never finish a job.
        "--gen-priority",
        "normal",
        "--global-res",
        "128",
        "--out",
        logs.to_str().unwrap(),
    ])
    .unwrap();
    assert!(out.contains("SMOKE RUN") && out.contains("not a measurement"), "{out}");
    assert!(out.contains("| altitude | stamp radius | preview ms (budget 16) | exact ms (budget)"), "{out}");
    // One altitude, one radius, one run: a row with a preview and an exact time.
    let row = out.lines().find(|l| l.starts_with("| 200 m | 50 m |")).unwrap_or_else(|| panic!("no row: {out}"));
    // The preview column may read `-` on a slow adapter: the refreshed tiles can arrive within one frame, so no frame draws a stale tile
    // (the scene says "no result" then). The exact column is always set (the scene errors when the edit never became exact), and the
    // preview itself is asserted by gpu_preview.
    let cells: Vec<&str> = row.split('|').map(str::trim).collect();
    assert!(cells[5] != "-", "the exact frame happened: {row}");
    let table = std::fs::read_to_string(logs.join("edit-latency.md")).unwrap();
    assert!(table.contains("| 200 m | 50 m |"), "{table}");
    // The stages table follows the budget table (TEST-041): the durations and the frame median are numbers.
    assert!(
        out.contains("| altitude | stamp radius | set_definition call | edit call | preview call | plan delivered (worker ms)"),
        "{out}"
    );
    let stages = out.lines().filter(|l| l.starts_with("| 200 m | 50 m |")).nth(1).unwrap_or_else(|| panic!("no stages row: {out}"));
    let cells: Vec<&str> = stages.split('|').map(str::trim).collect();
    for i in [3, 4, 5, 14, 15] {
        assert!(cells[i].parse::<f64>().is_ok(), "column {i} of the stages row is a number: {stages}");
    }
    assert!(table.contains("set_definition call"), "edit-latency.md holds both tables: {table}");
    // The refreshed set by level (TEST-041): at least one row of the case, named = drawn + resident only.
    assert!(
        out.contains("| altitude | stamp radius | level | spacing m | named | drawn | resident only | change below spacing |"),
        "{out}"
    );
    let after = &out[out.find("| change below spacing |").unwrap()..];
    let level_rows: Vec<Vec<&str>> =
        after.lines().filter(|l| l.starts_with("| 200 m | 50 m | ")).map(|l| l.split('|').map(str::trim).collect()).collect();
    assert!(!level_rows.is_empty(), "a row of the refreshed-set table: {out}");
    for c in &level_rows {
        let n = |i: usize| c[i].parse::<usize>().unwrap_or_else(|_| panic!("column {i} is a count: {c:?}"));
        assert_eq!(n(5), n(6) + n(7), "named = drawn + resident only: {c:?}");
    }
    assert!(table.contains("change below spacing"), "edit-latency.md holds the refreshed-set table: {table}");
}
