//! The hardware half of `cargo xtask accept M8` (TEST-060): reads the logs the M8 perf scenes write and judges criteria 4 to 7 against the
//! spec's budgets, or says what is missing. Pure over the file contents, like `accept_m5`; the xtask reads the files and prints the table.
//!
//! The rows: criterion 4 (`coupled-make.json`: a fresh make of the coupled run from the 16384 bake in at most 600 s, GEN-042), criterion 5
//! (`shadow-cost.json`: shadows and material map together at most 1.5 ms of terrain-pass GPU at every pose, from the compiled-out draws,
//! REND-044), criterion 6 (`edit-latency.md`: no `OVER` and no `over` row of the M6 table, the drawn tiles within budget; the horizon and chunk tables are not judged, owner decision 2026-10-29, TEST-042, TEST-056 and TEST-065) and
//! criterion 7 (`envelope.json`: the highest steady speed at 300 m is at least 200 m/s; the curve of the other altitudes is reported).
//! A smoke log, a log without the production store or a log of another scene is refused as a source of a verdict.

use crate::compare::{parse, Json};

/// GEN-042: the coupled run's budget in seconds.
pub const COUPLED_BUDGET_S: f64 = 600.0;
/// REND-044: the shadows and the material map together, in milliseconds of terrain-pass GPU time.
pub const SHADOW_BUDGET_MS: f64 = 1.5;
/// M8 criterion 7: the steady-flight floor at 300 m, metres per second.
pub const ENVELOPE_FLOOR_MPS: f64 = 200.0;
/// The altitude of the floor.
pub const ENVELOPE_FLOOR_ALTITUDE_M: f64 = 300.0;
/// The production bake's cells per face edge.
pub const PRODUCTION_BAKE_CELLS: f64 = 16384.0;

/// The files of a window, as text (a missing file is `None`).
#[derive(Clone, Debug, Default)]
pub struct M8Files {
    /// `coupled-make.json`.
    pub coupled: Option<Vec<u8>>,
    /// `shadow-cost.json`.
    pub shadow: Option<Vec<u8>>,
    /// `edit-latency.md`.
    pub edit: Option<String>,
    /// `envelope.json`.
    pub envelope: Option<Vec<u8>>,
}

/// What a criterion's log says.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Pass(String),
    Fail(String),
    /// The log is missing or cannot be trusted (smoke, another store, another scene) or lacks what the row needs: not a pass, not a failure.
    Missing(String),
}

/// One criterion's row.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub criterion: u8,
    pub what: &'static str,
    pub verdict: Verdict,
}

/// Overall: any failure fails; else anything missing is incomplete; else pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Incomplete,
}

impl Outcome {
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Pass => "PASS",
            Outcome::Fail => "FAIL",
            Outcome::Incomplete => "INCOMPLETE",
        }
    }
}

fn read(bytes: &Option<Vec<u8>>, file: &str, scene: &str) -> Result<Json, String> {
    let bytes = bytes.as_ref().ok_or_else(|| format!("{file}: no log"))?;
    let j = parse(bytes).map_err(|e| format!("{file}: {e}"))?;
    if j.get("scene").and_then(|s| if let Json::Str(s) = s { Some(s.as_str()) } else { None }) != Some(scene) {
        return Err(format!("{file}: not the log of the scene {scene}"));
    }
    if matches!(j.get("smoke"), Some(Json::Bool(true))) {
        return Err(format!("{file}: a smoke run, not a measurement"));
    }
    if let Some(Json::Arr(o)) = j.get("owner_override") {
        if !o.is_empty() {
            return Err(format!("{file}: run with an owner override of the gate ({} item(s)): not a valid window", o.len()));
        }
    }
    Ok(j)
}

fn num(j: &Json, key: &str) -> Option<f64> {
    j.get(key).and_then(Json::num)
}

fn criterion4(files: &M8Files) -> Verdict {
    let j = match read(&files.coupled, "coupled-make.json", "coupled-make") {
        Ok(j) => j,
        Err(e) => return Verdict::Missing(e),
    };
    let (Some(secs), Some(bake)) = (num(&j, "seconds"), num(&j, "bake_res")) else {
        return Verdict::Missing("coupled-make.json: no `seconds` or `bake_res`".into());
    };
    if bake != PRODUCTION_BAKE_CELLS {
        return Verdict::Missing(format!("coupled-make.json: bake of {bake} cells, not the production {PRODUCTION_BAKE_CELLS}"));
    }
    if secs <= COUPLED_BUDGET_S {
        Verdict::Pass(format!("{secs:.1} s against {COUPLED_BUDGET_S} s"))
    } else {
        Verdict::Fail(format!("{secs:.1} s over {COUPLED_BUDGET_S} s"))
    }
}

fn criterion5(files: &M8Files) -> Verdict {
    let j = match read(&files.shadow, "shadow-cost.json", "shadow-cost") {
        Ok(j) => j,
        Err(e) => return Verdict::Missing(e),
    };
    let Some(Json::Arr(poses)) = j.get("poses") else { return Verdict::Missing("shadow-cost.json: no poses".into()) };
    if poses.is_empty() {
        return Verdict::Missing("shadow-cost.json: no poses".into());
    }
    let (mut worst, mut worst_pose, mut runtime_off) = (0.0f64, String::new(), Vec::new());
    for p in poses {
        let name = if let Some(Json::Str(s)) = p.get("pose") { s.clone() } else { "?".into() };
        let Some(together) = num(p, "together_ms") else {
            return Verdict::Missing(format!("shadow-cost.json: pose {name} has no together_ms"));
        };
        if !matches!(p.get("compiled_out_ms"), Some(Json::Arr(a)) if !a.is_empty()) {
            runtime_off.push(name.clone());
        }
        if together > worst {
            (worst, worst_pose) = (together, name);
        }
    }
    if worst > SHADOW_BUDGET_MS {
        return Verdict::Fail(format!("{worst_pose}: {worst:.2} ms over {SHADOW_BUDGET_MS} ms"));
    }
    // REND-044: a pose within 20 % of the budget by the runtime-off draws is not called met before the compiled-out draws are measured.
    if !runtime_off.is_empty() && runtime_off.len() == poses.len() && worst > 0.8 * SHADOW_BUDGET_MS {
        return Verdict::Missing(format!(
            "worst {worst:.2} ms is by the runtime-off draws and within 20 % of the budget: measure the compiled-out draws"
        ));
    }
    Verdict::Pass(format!("worst pose {worst_pose} {worst:.2} ms against {SHADOW_BUDGET_MS} ms over {} poses", poses.len()))
}

/// One judged table of the edit scene: its rows and the rows over.
#[derive(Default)]
struct EditTable {
    rows: usize,
    bad: Vec<String>,
}

/// The M6 table of the edit scene (the column headed exactly `verdict`; cell `OVER`, `OVER (no preview)`, `no result` or `over` fails; `ok`, `ok (no preview)`,
/// `recorded` and `-` do not). Other verdict columns are not judged: `verdict (10 s, drawn tiles)` (the eroded chunk result, M8 amendment 2026-10-28) and
/// `verdict (10 s, horizon maps)` (logs before the owner decision of 2026-10-29; later logs have no verdict on the horizon table).
fn edit_table(text: &str) -> Option<EditTable> {
    let mut m6: Option<EditTable> = None;
    let mut col: Option<usize> = None;
    for line in text.lines() {
        if !line.starts_with('|') {
            col = None;
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if let Some(c) = cells.iter().position(|c| c.starts_with("verdict")) {
            col = (cells[c] == "verdict").then_some(c);
            if col.is_some() {
                m6.get_or_insert_with(EditTable::default);
            }
            continue;
        }
        let Some(c) = col else { continue };
        if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) {
            continue;
        }
        if let (Some(t), Some(v)) = (m6.as_mut(), cells.get(c)) {
            t.rows += 1;
            if v.starts_with("OVER") || *v == "over" || *v == "no result" {
                t.bad.push(format!("{} {} ({v})", cells.first().copied().unwrap_or("?"), cells.get(1).copied().unwrap_or("?")));
            }
        }
    }
    m6.filter(|t| t.rows > 0)
}

/// Criterion 6 (amended by the owner 2026-10-29): an edit shows within the M6 budgets, the drawn tiles decide. The horizon-map tables and the chunk table are
/// reported in `edit-latency.md` and not judged. No M6 table is MISSING.
fn criterion6(files: &M8Files) -> Verdict {
    let Some(text) = &files.edit else { return Verdict::Missing("edit-latency.md: no log".into()) };
    match edit_table(text) {
        None => Verdict::Missing("edit-latency.md: no verdict column found (the M6 table)".into()),
        Some(t) if !t.bad.is_empty() => Verdict::Fail(format!("{} of {} rows over: M6 budgets: {}", t.bad.len(), t.rows, t.bad.join("; "))),
        Some(t) => Verdict::Pass(format!("M6 budgets {} rows, none over; horizon maps and chunk table reported, not judged", t.rows)),
    }
}

fn criterion7(files: &M8Files) -> Verdict {
    let j = match read(&files.envelope, "envelope.json", "envelope") {
        Ok(j) => j,
        Err(e) => return Verdict::Missing(e),
    };
    // The criterion is measured on tiles and horizon maps without the chunk change (M8 amendment 2026-10-28, STRM-046): a log of a run with the chunk
    // stage, or from before the log recorded it, is not a measurement of it.
    match j.get("chunk_stage") {
        Some(Json::Bool(false)) => {}
        Some(Json::Bool(true)) => {
            return Verdict::Missing("envelope.json: the run had the chunk stage on (criterion 7 is measured with it off)".into())
        }
        _ => return Verdict::Missing("envelope.json: no chunk_stage field (a log from before it was recorded)".into()),
    }
    // A run over the production store with the horizon jobs running: the tiles and horizon maps the view wants are what is measured; an in-memory bake
    // (`--allow-memory-bake`) or a run without the jobs is not.
    if !matches!(j.get("horizon"), Some(Json::Bool(true))) {
        return Verdict::Missing("envelope.json: the run had no horizon jobs (criterion 7 is measured on tiles and horizon maps)".into());
    }
    match j.get("source") {
        Some(Json::Str(s)) if !s.contains("NOT the production source") => {}
        _ => return Verdict::Missing("envelope.json: not a run over the production store".into()),
    }
    let Some(Json::Arr(curve)) = j.get("steady_curve") else {
        return Verdict::Missing("envelope.json: no steady_curve (run `perf stream envelope --only-steady`)".into());
    };
    let at_floor: Vec<f64> = curve
        .iter()
        .filter(|c| num(c, "altitude_m") == Some(ENVELOPE_FLOOR_ALTITUDE_M))
        .filter_map(|c| num(c, "highest_speed_mps"))
        .collect();
    let report: Vec<String> =
        curve.iter().filter_map(|c| Some(format!("{} m {} m/s", num(c, "altitude_m")?, num(c, "highest_speed_mps")?))).collect();
    let Some(lowest) = at_floor.iter().copied().reduce(f64::min) else {
        return Verdict::Missing(format!("envelope.json: no steady curve at {ENVELOPE_FLOOR_ALTITUDE_M} m (curve: {})", report.join(", ")));
    };
    if lowest >= ENVELOPE_FLOOR_MPS {
        Verdict::Pass(format!(
            "{lowest} m/s at {ENVELOPE_FLOOR_ALTITUDE_M} m against {ENVELOPE_FLOOR_MPS} (steady curve: {})",
            report.join(", ")
        ))
    } else {
        Verdict::Fail(format!(
            "{lowest} m/s at {ENVELOPE_FLOOR_ALTITUDE_M} m under {ENVELOPE_FLOOR_MPS} (steady curve: {})",
            report.join(", ")
        ))
    }
}

/// Criteria 4 to 7 from the logs.
pub fn evaluate(files: &M8Files) -> Vec<Row> {
    vec![
        Row { criterion: 4, what: "global coupled run at most 10 min", verdict: criterion4(files) },
        Row { criterion: 5, what: "shadows and material map at most 1.5 ms (1440p)", verdict: criterion5(files) },
        Row { criterion: 6, what: "an edit shows within the M6 budgets (drawn tiles)", verdict: criterion6(files) },
        Row { criterion: 7, what: "steady speed envelope, floor 200 m/s at 300 m", verdict: criterion7(files) },
    ]
}

pub fn outcome(rows: &[Row]) -> Outcome {
    if rows.iter().any(|r| matches!(r.verdict, Verdict::Fail(_))) {
        Outcome::Fail
    } else if rows.iter().any(|r| matches!(r.verdict, Verdict::Missing(_))) {
        Outcome::Incomplete
    } else {
        Outcome::Pass
    }
}

/// The rows as a Markdown table.
pub fn render(rows: &[Row]) -> String {
    let mut s = String::from("| # | Criterion | Result | Detail |\n|---|---|---|---|\n");
    for r in rows {
        let (word, detail) = match &r.verdict {
            Verdict::Pass(d) => ("PASS", d),
            Verdict::Fail(d) => ("FAIL", d),
            Verdict::Missing(d) => ("MISSING", d),
        };
        s += &format!("| {} | {} | {word} | {detail} |\n", r.criterion, r.what);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(s: &str) -> Option<Vec<u8>> {
        Some(s.as_bytes().to_vec())
    }

    fn good() -> M8Files {
        M8Files {
            coupled: bytes(r#"{"scene":"coupled-make","owner_override":[],"bake_res":16384,"res":2048,"steps":100,"seconds":19.661}"#),
            shadow: bytes(
                r#"{"scene":"shadow-cost","owner_override":[],"budget_ms":1.5,"poses":[{"pose":"300m-sunset","compiled_out_ms":[2.1,1.4,1.1],"together_ms":1.42},{"pose":"30m-sunset","compiled_out_ms":[1.7],"together_ms":1.0}]}"#,
            ),
            edit: Some(
                "| altitude | stamp radius | preview ms (budget 16) | drawn exact ms (budget) | verdict |\n|---|---|---|---|---|\n| 1 m | 50 m | 7.2 | 68.4 (1000) | ok |\n| 200 m | 5000 m ridge | - | 458 (recorded) | recorded |\n\n| altitude | stamp radius | drawn again ms | verdict (10 s, drawn tiles) | horizon |\n|---|---|---|---|---|\n| 1 m | 50 m | 16000 | OVER | 1 |\n\n| altitude | stamp radius | horizon maps ms | verdict (10 s, horizon maps) |\n|---|---|---|---|\n| 1 m | 50 m | 4000 | ok |\n"
                    .into(),
            ),
            envelope: bytes(
                r#"{"scene":"envelope","smoke":false,"chunk_stage":false,"horizon":true,"source":"store S","owner_override":[],"steady_curve":[{"altitude_m":300,"highest_speed_mps":480},{"altitude_m":100,"highest_speed_mps":340}]}"#,
            ),
        }
    }

    // spec: TEST-060
    #[test]
    fn logs_within_the_budgets_pass_each_row_and_the_outcome() {
        let rows = evaluate(&good());
        assert_eq!(rows.iter().map(|r| r.criterion).collect::<Vec<_>>(), [4, 5, 6, 7]);
        for r in &rows {
            assert!(matches!(r.verdict, Verdict::Pass(_)), "{r:?}");
        }
        assert_eq!(outcome(&rows), Outcome::Pass);
        assert!(render(&rows).contains("| 7 | steady speed envelope, floor 200 m/s at 300 m | PASS |"));
    }

    // spec: TEST-060
    #[test]
    fn a_number_over_its_budget_fails_its_row_and_names_it() {
        let mut f = good();
        f.coupled = bytes(r#"{"scene":"coupled-make","owner_override":[],"bake_res":16384,"seconds":601.0}"#);
        f.shadow = bytes(
            r#"{"scene":"shadow-cost","owner_override":[],"poses":[{"pose":"300m-sunset","compiled_out_ms":[1],"together_ms":1.61}]}"#,
        );
        f.edit =
            Some("| a | b | verdict |\n|---|---|---|\n| 1 m | 500 m hill | over |\n| 1 m | 50 m | OVER |\n| 2 m | 1 m | ok |\n".into());
        f.envelope = bytes(
            r#"{"scene":"envelope","owner_override":[],"chunk_stage":false,"horizon":true,"source":"store S","steady_curve":[{"altitude_m":300,"highest_speed_mps":150}]}"#,
        );
        let rows = evaluate(&f);
        for r in &rows {
            assert!(matches!(r.verdict, Verdict::Fail(_)), "{r:?}");
        }
        assert!(matches!(&rows[1].verdict, Verdict::Fail(m) if m.contains("300m-sunset") && m.contains("1.61")));
        assert!(matches!(&rows[2].verdict, Verdict::Fail(m) if m.contains("2 of 3 rows over")));
        assert_eq!(outcome(&rows), Outcome::Fail);
    }

    // spec: TEST-060
    #[test]
    fn criterion_6_judges_the_m6_table_only_and_reports_the_horizon_and_chunk_tables() {
        let mut f = good();
        // good() has a chunk table and a horizon table with OVER rows: neither is judged.
        assert!(matches!(&evaluate(&f)[2].verdict, Verdict::Pass(m) if m.contains("M6 budgets") && m.contains("not judged")));
        let m6 = "| altitude | stamp radius | verdict |
|---|---|---|
| 1 m | 50 m | ok |

";
        f.edit = Some(m6.into());
        assert!(matches!(&evaluate(&f)[2].verdict, Verdict::Pass(_)), "no horizon table is not missing");
        f.edit = Some(format!("{m6}| altitude | stamp radius | verdict (10 s, horizon maps) |
|---|---|---|
| 1 m | 50 m | OVER |
"));
        assert!(matches!(&evaluate(&f)[2].verdict, Verdict::Pass(_)), "an old log's over horizon row is information");
        f.edit = Some(format!("{m6}| altitude | stamp radius | map | edit to map ms |
|---|---|---|---|
| 1 m | 50 m | (0, 1, 1) | 21000.0 |
"));
        assert!(matches!(&evaluate(&f)[2].verdict, Verdict::Pass(_)));
        f.edit = Some("| altitude | stamp radius | verdict |
|---|---|---|
| 1 m | 50 m | OVER |
".into());
        assert!(matches!(&evaluate(&f)[2].verdict, Verdict::Fail(m) if m.contains("1 of 1 rows over")));
        f.edit = Some("| altitude | stamp radius | verdict (10 s, horizon maps) |
|---|---|---|
| 1 m | 50 m | ok |
".into());
        assert!(matches!(&evaluate(&f)[2].verdict, Verdict::Missing(m) if m.contains("verdict column")), "no M6 table is not a pass");
    }

    // spec: TEST-060
    #[test]
    fn criterion_7_is_missing_for_a_log_with_the_chunk_stage_on_or_without_the_field_that_says_so() {
        let mut f = good();
        let curve = r#""steady_curve":[{"altitude_m":300,"highest_speed_mps":480}]"#;
        f.envelope = bytes(&format!(r#"{{"scene":"envelope","owner_override":[],"chunk_stage":true,{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("chunk stage on")));
        f.envelope = bytes(&format!(r#"{{"scene":"envelope","owner_override":[],{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("no chunk_stage field")));
        f.envelope =
            bytes(&format!(r#"{{"scene":"envelope","owner_override":[],"chunk_stage":false,"horizon":true,"source":"store S",{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Pass(_)));
    }

    // spec: TEST-060
    #[test]
    fn criterion_7_is_missing_for_a_log_of_an_in_memory_bake_or_a_run_without_horizon_jobs() {
        let mut f = good();
        let curve = r#""steady_curve":[{"altitude_m":300,"highest_speed_mps":480}]"#;
        let head = r#""scene":"envelope","owner_override":[],"chunk_stage":false"#;
        f.envelope = bytes(&format!(r#"{{{head},"horizon":true,"source":"store S (NOT the production source)",{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("production store")));
        f.envelope = bytes(&format!(r#"{{{head},"horizon":false,"source":"store S",{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("no horizon jobs")));
        f.envelope = bytes(&format!(r#"{{{head},{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(_)), "a log without the fields is from before");
        f.envelope = bytes(&format!(r#"{{{head},"horizon":true,"source":"store S",{curve}}}"#));
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Pass(_)));
    }
    // spec: TEST-060
    #[test]
    fn a_missing_smoke_or_unfit_log_is_missing_not_a_pass_and_a_failure_still_wins() {
        let mut f = M8Files::default();
        assert!(evaluate(&f).iter().all(|r| matches!(r.verdict, Verdict::Missing(_))));
        assert_eq!(outcome(&evaluate(&f)), Outcome::Incomplete);
        f = good();
        f.coupled = bytes(r#"{"scene":"coupled-make","owner_override":[],"bake_res":256,"seconds":1.0}"#);
        f.shadow = bytes(r#"{"scene":"shadow-cost","owner_override":[],"poses":[{"pose":"p","together_ms":1.4}]}"#);
        f.envelope = bytes(r#"{"scene":"envelope","smoke":true,"steady_curve":[{"altitude_m":300,"highest_speed_mps":480}]}"#);
        f.edit = Some("no table here".into());
        let rows = evaluate(&f);
        assert!(matches!(&rows[0].verdict, Verdict::Missing(m) if m.contains("256")));
        assert!(
            matches!(&rows[1].verdict, Verdict::Missing(m) if m.contains("compiled-out")),
            "runtime-off within 20 % of the budget is not called met"
        );
        assert!(matches!(&rows[2].verdict, Verdict::Missing(m) if m.contains("verdict column")));
        assert!(matches!(&rows[3].verdict, Verdict::Missing(m) if m.contains("smoke")));
        assert_eq!(outcome(&rows), Outcome::Incomplete);
        f.envelope = bytes(
            r#"{"scene":"envelope","owner_override":[],"chunk_stage":false,"horizon":true,"source":"store S","steady_curve":[{"altitude_m":300,"highest_speed_mps":100}]}"#,
        );
        assert_eq!(outcome(&evaluate(&f)), Outcome::Fail);
        // The cold curve alone (no steady_curve) cannot judge the floor; another scene's log and an override are refused.
        f.envelope = bytes(r#"{"scene":"envelope","owner_override":[],"chunk_stage":false,"horizon":true,"source":"store S","curve":[]}"#);
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("steady_curve")));
        f.envelope = bytes(r#"{"scene":"lit","owner_override":[]}"#);
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("not the log")));
        f.envelope = bytes(r#"{"scene":"envelope","owner_override":["gate"],"steady_curve":[]}"#);
        assert!(matches!(&evaluate(&f)[3].verdict, Verdict::Missing(m) if m.contains("override")));
    }
}
