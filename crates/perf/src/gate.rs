//! Readiness gate and validity rules of ADR 0020.
//!
//! Every rule is a pure function over measured facts, so each refusal has a tier-A test. The platform queries at the
//! bottom are thin: they only turn a command's output into the plain structs the rules take.

use std::process::Command;

use crate::{check_reference_adapter, is_performance_scheme, AdapterFacts};

/// Names of processes that distort a CPU-plan-time measurement when they run beside the harness (lower case, no `.exe`).
pub const BUSY_PROCESSES: [&str; 6] = ["cargo", "rustc", "cargo-nextest", "nextest", "planet", "clippy-driver"];
/// Mean CPU load over the sampling window must stay below this (percent).
pub const MAX_CPU_LOAD_PCT: f64 = 15.0;
/// Mean GPU utilisation over the sampling window must stay below this (percent).
pub const MAX_GPU_UTIL_PCT: f64 = 10.0;
/// GPU temperature must stay below this (degrees C).
pub const MAX_GPU_TEMP_C: f64 = 70.0;
/// Length of the CPU and GPU load sampling window (seconds).
pub const LOAD_WINDOW_S: u32 = 10;
/// Largest accepted run-to-run noise `(max - min) / median`.
pub const MAX_NOISE: f64 = 0.10;

/// One entry of the process table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    /// Creation time as a sortable number (`yyyyMMddHHmmssfff`). A recorded parent that is younger than its child is a
    /// reused pid, not the parent.
    pub created: u64,
    pub name: String,
}

/// Everything the readiness gate looks at. `None` means the query failed, which is a refusal, never a pass.
#[derive(Clone, Debug)]
pub struct MachineFacts {
    pub adapter: AdapterFacts,
    pub ac_power: Option<bool>,
    pub power_scheme: Option<String>,
    pub processes: Option<Vec<ProcInfo>>,
    pub own_pid: u32,
    pub cpu_load_pct: Option<f64>,
    pub gpu_util_pct: Option<f64>,
    pub gpu_temp_c: Option<f64>,
}

/// A failed gate condition with its measured value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateFailure {
    pub condition: &'static str,
    pub measured: String,
}

impl std::fmt::Display for GateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.condition, self.measured)
    }
}

/// Names of busy processes that are not the harness itself, its ancestors (`cargo run`, `cargo xtask`) or its descendants.
pub fn offending_processes(procs: &[ProcInfo], own_pid: u32) -> Vec<String> {
    let mut exempt = std::collections::BTreeSet::new();
    // Ancestors: walk the parent chain (bounded, the table may contain cycles from reused pids).
    let mut pid = own_pid;
    for _ in 0..64 {
        if !exempt.insert(pid) {
            break;
        }
        match procs.iter().find(|p| p.pid == pid) {
            Some(p) if p.ppid != 0 => match procs.iter().find(|q| q.pid == p.ppid) {
                Some(parent) if parent.created <= p.created => pid = p.ppid,
                // The parent is gone and its pid was reused (or is unknown): the chain ends here.
                _ => break,
            },
            _ => break,
        }
    }
    // Descendants of the harness itself (not of its ancestors: a sibling build under the same shell is another session).
    let mut tree = std::collections::BTreeSet::from([own_pid]);
    loop {
        let before = tree.len();
        for p in procs {
            // A child counts only if its parent existed when it started (otherwise the parent pid was reused).
            let parent_older = procs.iter().any(|q| q.pid == p.ppid && q.created <= p.created);
            if tree.contains(&p.ppid) && parent_older {
                tree.insert(p.pid);
            }
        }
        if tree.len() == before {
            break;
        }
    }
    exempt.extend(tree);
    let mut out: Vec<String> = procs
        .iter()
        .filter(|p| !exempt.contains(&p.pid))
        .filter(|p| {
            let n = p.name.to_lowercase();
            let n = n.strip_suffix(".exe").unwrap_or(&n);
            BUSY_PROCESSES.contains(&n)
        })
        .map(|p| format!("{} (pid {})", p.name, p.pid))
        .collect();
    out.sort();
    out
}

fn pct(v: Option<f64>) -> String {
    v.map_or("unavailable".to_string(), |v| format!("{v:.1}"))
}

/// The readiness gate: every failed condition with its measured value; empty means the machine is ready.
pub fn readiness(f: &MachineFacts) -> Vec<GateFailure> {
    let mut out = Vec::new();
    let mut fail = |condition: &'static str, measured: String| out.push(GateFailure { condition, measured });
    match f.ac_power {
        Some(true) => {}
        Some(false) => fail("AC power connected", "running on battery".into()),
        None => fail("AC power connected", "could not be read".into()),
    }
    match f.power_scheme.as_deref() {
        Some(s) if is_performance_scheme(s) => {}
        Some(s) => fail("performance power scheme", format!("'{}'", s.trim())),
        None => fail("performance power scheme", "could not be read".into()),
    }
    if let Err(r) = check_reference_adapter(&f.adapter) {
        fail("reference adapter", r.to_string());
    }
    match &f.processes {
        Some(procs) => {
            let bad = offending_processes(procs, f.own_pid);
            if !bad.is_empty() {
                fail("quiet machine: no build or test processes", bad.join(", "));
            }
        }
        None => fail("quiet machine: no build or test processes", "the process list could not be read".into()),
    }
    match f.cpu_load_pct {
        Some(v) if v < MAX_CPU_LOAD_PCT => {}
        v => fail("CPU load", format!("{} % mean over {LOAD_WINDOW_S} s (limit {MAX_CPU_LOAD_PCT} %)", pct(v))),
    }
    match f.gpu_util_pct {
        Some(v) if v < MAX_GPU_UTIL_PCT => {}
        v => fail("GPU utilisation", format!("{} % mean over {LOAD_WINDOW_S} s (limit {MAX_GPU_UTIL_PCT} %)", pct(v))),
    }
    match f.gpu_temp_c {
        Some(v) if v < MAX_GPU_TEMP_C => {}
        v => fail("GPU temperature", format!("{} C (limit {MAX_GPU_TEMP_C} C)", pct(v))),
    }
    out
}

/// The software power cap bit of `clocks_throttle_reasons.active`: the laptop GPU's normal governor at full load. Recorded and
/// printed, never invalidating (ADR 0036 point 1, owner 2026-10-15).
pub const THROTTLE_SW_POWER_CAP: u64 = 0x4;

fn throttle_mask(reasons: &str) -> Option<u64> {
    let t = reasons.trim();
    let digits = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    u64::from_str_radix(digits, 16).ok()
}

/// Whether an `nvidia-smi` `clocks_throttle_reasons.active` value (hex bit mask) reports throttling that changes the
/// measurement and so invalidates the session: hardware slowdown (0x8), software thermal (0x20), hardware thermal (0x40),
/// power brake (0x80). The software power cap (0x4) is normal at full load and does not count (ADR 0036 point 1; see
/// [`has_power_cap`]); idle (0x1), applications clocks (0x2) and sync boost (0x10) do not count either. A value that cannot
/// be parsed is an unknown reason and counts as throttling.
pub fn is_throttling(reasons: &str) -> bool {
    throttle_mask(reasons).is_none_or(|mask| mask & (0x8 | 0x20 | 0x40 | 0x80) != 0)
}

/// Whether the software power cap (0x4) is active in the value; recorded per run and printed, not judged.
pub fn has_power_cap(reasons: &str) -> bool {
    throttle_mask(reasons).is_some_and(|mask| mask & THROTTLE_SW_POWER_CAP != 0)
}

/// Outcome of the after-the-run validity rules.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Validity {
    pub reasons: Vec<String>,
}

impl Validity {
    pub fn is_valid(&self) -> bool {
        self.reasons.is_empty()
    }
}

/// Whether the run-to-run noise of one workload's run medians needs a retry (above [`MAX_NOISE`], or not computable).
pub fn noise_needs_retry(run_medians: &[f64]) -> bool {
    crate::run_to_run_noise(run_medians).is_none_or(|n| n > MAX_NOISE)
}

/// Validity of a finished session: no throttling seen during measured runs, and the noise of each workload (after its
/// one retry) within [`MAX_NOISE`]. `throttled` lists the runs that reported throttling, `noisy` the workloads whose
/// noise stayed above the limit with the value.
pub fn validity(throttled: &[String], noisy: &[(String, f64)]) -> Validity {
    let mut reasons = Vec::new();
    for t in throttled {
        reasons.push(format!("GPU throttling reported during '{t}'"));
    }
    for (name, n) in noisy {
        reasons.push(format!("run-to-run noise {n:.3} on '{name}' is above {MAX_NOISE} after one retry"));
    }
    Validity { reasons }
}

// ---- platform queries (thin; the rules above take their output) ----

/// Parse the CSV of `pid,ppid,created,name` lines produced by [`query_processes`].
pub fn parse_process_csv(csv: &str) -> Vec<ProcInfo> {
    csv.lines()
        .filter_map(|l| {
            let mut it = l.trim().splitn(4, ',');
            let pid = it.next()?.trim().parse().ok()?;
            let ppid = it.next()?.trim().parse().ok()?;
            let created = it.next()?.trim().parse().ok()?;
            let name = it.next()?.trim().to_string();
            (!name.is_empty()).then_some(ProcInfo { pid, ppid, created, name })
        })
        .collect()
}

fn powershell(script: &str) -> Option<String> {
    let out = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `Some(true)` on AC power, `Some(false)` on battery. `None` when it cannot be read (never a pass).
pub fn query_ac_power() -> Option<bool> {
    let s =
        powershell("Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SystemInformation]::PowerStatus.PowerLineStatus")?;
    match s.as_str() {
        "Online" => Some(true),
        "Offline" => Some(false),
        _ => None,
    }
}

/// The process table as `(pid, parent pid, creation time, name)`.
pub fn query_processes() -> Option<Vec<ProcInfo>> {
    let s = powershell("Get-CimInstance Win32_Process | ForEach-Object { \"$($_.ProcessId),$($_.ParentProcessId),$($_.CreationDate.ToString('yyyyMMddHHmmssfff')),$($_.Name)\" }")?;
    let v = parse_process_csv(&s);
    (!v.is_empty()).then_some(v)
}

/// Mean total CPU load over `seconds`, one sample per second. Uses the CIM class `Win32_PerfFormattedData_PerfOS_Processor`
/// (counter paths of `Get-Counter` are localised and fail on a German Windows, which is Rasierklinge's).
pub fn query_cpu_load(seconds: u32) -> Option<f64> {
    let script = format!(
        "1..{seconds} | ForEach-Object {{ (Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter \"Name='_Total'\").PercentProcessorTime; Start-Sleep -Seconds 1 }}"
    );
    let s = powershell(&script)?;
    let v: Vec<f64> = s.lines().filter_map(|l| l.trim().replace(',', ".").parse().ok()).collect();
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// Parse one `nvidia-smi --query-gpu=utilization.gpu,temperature.gpu --format=csv,noheader,nounits` line.
pub fn parse_gpu_util_temp(csv: &str) -> Option<(f64, f64)> {
    let line = csv.lines().find(|l| !l.trim().is_empty())?;
    let mut it = line.split(',').map(str::trim);
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// Mean GPU utilisation over `seconds` (one `nvidia-smi` sample per second) and the highest temperature seen.
pub fn query_gpu_load(seconds: u32) -> Option<(f64, f64)> {
    let mut utils = Vec::new();
    let mut temp = f64::NEG_INFINITY;
    for i in 0..seconds {
        let out = Command::new("nvidia-smi")
            .args(["--query-gpu=utilization.gpu,temperature.gpu", "--format=csv,noheader,nounits"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let (u, t) = parse_gpu_util_temp(&String::from_utf8_lossy(&out.stdout))?;
        utils.push(u);
        temp = temp.max(t);
        if i + 1 < seconds {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    (!utils.is_empty()).then(|| (utils.iter().sum::<f64>() / utils.len() as f64, temp))
}

/// Measure the machine: the 10 s load window runs the CPU and GPU samplers side by side.
pub fn measure_machine(adapter: AdapterFacts) -> MachineFacts {
    let gpu = std::thread::spawn(|| query_gpu_load(LOAD_WINDOW_S));
    let cpu_load_pct = query_cpu_load(LOAD_WINDOW_S);
    let gpu = gpu.join().ok().flatten();
    MachineFacts {
        adapter,
        ac_power: query_ac_power(),
        power_scheme: crate::query_power_scheme(),
        processes: query_processes(),
        own_pid: std::process::id(),
        cpu_load_pct,
        gpu_util_pct: gpu.map(|g| g.0),
        gpu_temp_c: gpu.map(|g| g.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceKind;

    fn adapter(kind: DeviceKind) -> AdapterFacts {
        AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind }
    }

    fn ready() -> MachineFacts {
        MachineFacts {
            adapter: adapter(DeviceKind::Discrete),
            ac_power: Some(true),
            power_scheme: Some("Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)".into()),
            processes: Some(vec![ProcInfo { pid: 10, ppid: 1, created: 10, name: "explorer.exe".into() }]),
            own_pid: 20,
            cpu_load_pct: Some(3.0),
            gpu_util_pct: Some(1.0),
            gpu_temp_c: Some(45.0),
        }
    }

    fn only(f: &MachineFacts) -> Vec<&'static str> {
        readiness(f).iter().map(|x| x.condition).collect()
    }

    // spec: TEST-012
    #[test]
    fn a_quiet_machine_on_ac_with_a_performance_scheme_passes() {
        assert_eq!(readiness(&ready()), vec![]);
    }

    // spec: TEST-012
    #[test]
    fn battery_power_is_refused_with_the_reason() {
        let mut f = ready();
        f.ac_power = Some(false);
        assert_eq!(readiness(&f), vec![GateFailure { condition: "AC power connected", measured: "running on battery".into() }]);
        f.ac_power = None;
        assert_eq!(only(&f), ["AC power connected"]);
    }

    // spec: TEST-012
    #[test]
    fn a_balanced_power_scheme_is_refused_and_named() {
        let mut f = ready();
        f.power_scheme = Some("Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)".into());
        let r = readiness(&f);
        assert_eq!(r.len(), 1);
        assert!(r[0].measured.contains("Balanced"), "{}", r[0]);
        f.power_scheme = None;
        assert_eq!(only(&f), ["performance power scheme"]);
    }

    // spec: TEST-012
    #[test]
    fn a_software_or_integrated_adapter_is_refused() {
        let mut f = ready();
        f.adapter = adapter(DeviceKind::Cpu);
        assert_eq!(only(&f), ["reference adapter"]);
    }

    // spec: TEST-012
    #[test]
    fn a_build_process_outside_the_harness_tree_is_refused_by_name_and_pid() {
        let mut f = ready();
        f.processes = Some(vec![
            ProcInfo { pid: 1, ppid: 0, created: 1, name: "System".into() },
            ProcInfo { pid: 5, ppid: 1, created: 5, name: "cargo.exe".into() },
            ProcInfo { pid: 6, ppid: 5, created: 6, name: "rustc.exe".into() },
            ProcInfo { pid: 7, ppid: 1, created: 7, name: "cargo-nextest.exe".into() },
            ProcInfo { pid: 8, ppid: 1, created: 8, name: "Clippy-Driver.exe".into() },
            ProcInfo { pid: 9, ppid: 1, created: 9, name: "planet.exe".into() },
        ]);
        let r = readiness(&f);
        assert_eq!(r.len(), 1);
        for want in
            ["cargo.exe (pid 5)", "rustc.exe (pid 6)", "cargo-nextest.exe (pid 7)", "Clippy-Driver.exe (pid 8)", "planet.exe (pid 9)"]
        {
            assert!(r[0].measured.contains(want), "{} lacks {want}", r[0]);
        }
        f.processes = None;
        assert_eq!(only(&f), ["quiet machine: no build or test processes"]);
    }

    // spec: TEST-012
    #[test]
    fn the_harness_own_ancestors_and_descendants_are_not_other_sessions() {
        // xtask (pid 3) -> cargo run (pid 4) -> planet (pid 20, the harness) -> a child (pid 21).
        let procs = vec![
            ProcInfo { pid: 2, ppid: 1, created: 2, name: "bash.exe".into() },
            ProcInfo { pid: 3, ppid: 2, created: 3, name: "cargo.exe".into() },
            ProcInfo { pid: 4, ppid: 3, created: 4, name: "cargo.exe".into() },
            ProcInfo { pid: 20, ppid: 4, created: 20, name: "planet.exe".into() },
            ProcInfo { pid: 21, ppid: 20, created: 21, name: "rustc.exe".into() },
            ProcInfo { pid: 30, ppid: 1, created: 30, name: "cargo.exe".into() },
        ];
        assert_eq!(offending_processes(&procs, 20), ["cargo.exe (pid 30)"]);
    }

    // spec: TEST-012
    #[test]
    fn a_cyclic_process_table_terminates() {
        let procs = vec![
            ProcInfo { pid: 1, ppid: 2, created: 1, name: "a".into() },
            ProcInfo { pid: 2, ppid: 1, created: 2, name: "cargo".into() },
        ];
        assert_eq!(offending_processes(&procs, 1), Vec::<String>::new());
    }

    // spec: TEST-012
    #[test]
    fn cpu_gpu_load_and_temperature_limits_refuse_at_and_above_the_limit() {
        let mut f = ready();
        f.cpu_load_pct = Some(MAX_CPU_LOAD_PCT);
        assert_eq!(only(&f), ["CPU load"]);
        f.cpu_load_pct = Some(MAX_CPU_LOAD_PCT - 0.1);
        assert_eq!(only(&f), Vec::<&str>::new());
        f.gpu_util_pct = Some(MAX_GPU_UTIL_PCT);
        assert_eq!(only(&f), ["GPU utilisation"]);
        f.gpu_util_pct = Some(MAX_GPU_UTIL_PCT - 0.1);
        f.gpu_temp_c = Some(MAX_GPU_TEMP_C);
        let r = readiness(&f);
        assert_eq!(r.iter().map(|x| x.condition).collect::<Vec<_>>(), ["GPU temperature"]);
        assert!(r[0].measured.contains("70.0 C"), "{}", r[0]);
        f.gpu_temp_c = None;
        f.cpu_load_pct = None;
        f.gpu_util_pct = None;
        assert_eq!(only(&f), ["CPU load", "GPU utilisation", "GPU temperature"]);
    }

    // spec: TEST-012
    #[test]
    fn every_failed_condition_is_reported_not_just_the_first() {
        let mut f = ready();
        f.ac_power = Some(false);
        f.cpu_load_pct = Some(80.0);
        f.gpu_temp_c = Some(85.0);
        assert_eq!(only(&f), ["AC power connected", "CPU load", "GPU temperature"]);
    }

    // spec: TEST-012
    #[test]
    fn throttle_masks_classify_as_the_gpu_reports_them() {
        for ok in ["0x0000000000000000", "0x0000000000000001", "0x0000000000000002", "0x0000000000000010", "0x0000000000000013"] {
            assert!(!is_throttling(ok), "{ok}");
        }
        for bad in ["0x0000000000000008", "0x0000000000000020", "0x0000000000000040", "0x0000000000000080", "0x0000000000000025"] {
            assert!(is_throttling(bad), "{bad}");
        }
        assert!(is_throttling("[Not Supported]"));
    }

    // spec: TEST-012
    #[test]
    fn the_software_power_cap_alone_is_recorded_and_does_not_invalidate() {
        // The reading of every heavy view on 2026-10-15: 0x4 at 86-90 W, 52-55 C.
        assert!(!is_throttling("0x0000000000000004") && has_power_cap("0x0000000000000004"));
        assert!(!is_throttling("0x4") && has_power_cap("0x4"));
        // With an idle or sync-boost bit beside it: still only the cap.
        assert!(!is_throttling("0x0000000000000005") && has_power_cap("0x0000000000000005"));
        // Thermal and hardware reasons beside the cap still invalidate, and the cap is still recorded.
        for bad in ["0x000000000000000c", "0x0000000000000024", "0x0000000000000044", "0x0000000000000084"] {
            assert!(is_throttling(bad) && has_power_cap(bad), "{bad}");
        }
        // No cap, no thermal reason; an unknown (unparseable) reading is not a cap but still invalidates.
        assert!(!has_power_cap("0x0000000000000000") && !has_power_cap("0x0000000000000020"));
        assert!(is_throttling("") && !has_power_cap("[Not Supported]"));
    }

    // spec: TEST-012
    #[test]
    fn noise_above_ten_percent_needs_a_retry_and_stays_invalid_after_it() {
        assert!(!noise_needs_retry(&[10.0, 10.5, 10.2]));
        assert!(noise_needs_retry(&[10.0, 11.5, 10.2]));
        assert!(noise_needs_retry(&[]));
        assert!(validity(&[], &[]).is_valid());
        let v = validity(&["terrain/ground-1m".into()], &[("terrain/aerial".into(), 0.18)]);
        assert_eq!(v.reasons.len(), 2);
        assert!(v.reasons[0].contains("throttling") && v.reasons[1].contains("0.180"), "{v:?}");
    }

    // spec: TEST-012
    #[test]
    fn platform_output_parsers_accept_the_documented_forms_and_reject_garbage() {
        let p = parse_process_csv("4,0,20261011120000001,System\r\n812,4,20261011120000002,cargo.exe\nbad line\n,,,x\n");
        assert_eq!(
            p,
            vec![
                ProcInfo { pid: 4, ppid: 0, created: 20261011120000001, name: "System".into() },
                ProcInfo { pid: 812, ppid: 4, created: 20261011120000002, name: "cargo.exe".into() }
            ]
        );
        assert_eq!(parse_gpu_util_temp("7, 52\n"), Some((7.0, 52.0)));
        assert_eq!(parse_gpu_util_temp("[N/A], 52"), None);
        assert_eq!(parse_gpu_util_temp(""), None);
    }

    // spec: TEST-012
    #[test]
    fn a_sibling_under_a_listed_ancestor_is_not_exempt() {
        // xtask (3) runs the harness (20); a second build (31) runs under the same shell (2).
        let procs = vec![
            ProcInfo { pid: 2, ppid: 1, created: 2, name: "bash.exe".into() },
            ProcInfo { pid: 3, ppid: 2, created: 3, name: "cargo.exe".into() },
            ProcInfo { pid: 20, ppid: 3, created: 20, name: "planet.exe".into() },
            ProcInfo { pid: 31, ppid: 2, created: 31, name: "cargo.exe".into() },
        ];
        assert_eq!(offending_processes(&procs, 20), ["cargo.exe (pid 31)"]);
    }

    // spec: TEST-012
    #[test]
    fn a_reused_parent_pid_does_not_make_a_stranger_an_ancestor_or_a_child() {
        // The harness (20) was started by a parent (pid 7) that has exited; pid 7 now belongs to another session's cargo,
        // created after the harness. It must not count as an ancestor.
        let procs = vec![
            ProcInfo { pid: 7, ppid: 1, created: 500, name: "cargo.exe".into() },
            ProcInfo { pid: 20, ppid: 7, created: 100, name: "planet.exe".into() },
        ];
        assert_eq!(offending_processes(&procs, 20), ["cargo.exe (pid 7)"]);
        // An orphan whose dead parent's pid the harness now holds is not the harness's child.
        let procs = vec![
            ProcInfo { pid: 20, ppid: 1, created: 500, name: "planet.exe".into() },
            ProcInfo { pid: 40, ppid: 20, created: 100, name: "rustc.exe".into() },
        ];
        assert_eq!(offending_processes(&procs, 20), ["rustc.exe (pid 40)"]);
    }
}
