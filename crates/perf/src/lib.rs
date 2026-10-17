//! Performance-harness protocol (CON-21, report §8): adapter check, logged clocks and power,
//! warm-up, medians and p99, JSON run logs. GPU-free: adapter facts arrive as plain structs and the
//! frame loop itself is supplied by the renderer once it exists (M2).

pub mod accept;
pub mod accept_m5;
pub mod clock;
pub mod compare;
pub mod gate;
pub mod host;
pub mod lit;
pub mod load;
pub mod prefill;
pub mod session;
pub mod streaming;
pub mod validity;

use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Discrete,
    Integrated,
    Cpu,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterFacts {
    pub name: String,
    pub backend: String,
    pub kind: DeviceKind,
}

/// The only GPU on which time-based budgets are measured.
pub const REFERENCE_ADAPTER_NAME: &str = "3080 Ti";

#[derive(Debug, PartialEq, Eq)]
pub struct Refusal(pub String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Refuse to measure on anything but the discrete reference GPU (Optimus can hand out the iGPU).
pub fn check_reference_adapter(a: &AdapterFacts) -> Result<(), Refusal> {
    if a.kind != DeviceKind::Discrete {
        return Err(Refusal(format!(
            "adapter '{}' ({}) is {:?}, not a discrete GPU; set the app to High performance in Windows Graphics settings \
             or use the MUX discrete mode, then retry",
            a.name, a.backend, a.kind
        )));
    }
    if !a.name.contains(REFERENCE_ADAPTER_NAME) {
        return Err(Refusal(format!(
            "adapter '{}' is not the reference GPU (expected a name containing '{REFERENCE_ADAPTER_NAME}'); \
             time budgets are measured only on Rasierklinge",
            a.name
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub samples: usize,
    pub median: f64,
    pub p99: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

/// Nearest-rank percentile on an already sorted slice (`p` in 0..=100).
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

/// Summarise frame times (ms) after discarding the first `warmup` samples.
pub fn summarize(samples_ms: &[f64], warmup: usize) -> Option<Stats> {
    let measured = samples_ms.get(warmup..)?;
    if measured.is_empty() || measured.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let mut s = measured.to_vec();
    s.sort_by(f64::total_cmp);
    let median = if s.len() % 2 == 1 { s[s.len() / 2] } else { (s[s.len() / 2 - 1] + s[s.len() / 2]) / 2.0 };
    Some(Stats {
        samples: s.len(),
        median,
        p99: percentile_sorted(&s, 99.0),
        min: s[0],
        max: s[s.len() - 1],
        mean: s.iter().sum::<f64>() / s.len() as f64,
    })
}

/// Relative spread `(max - min) / median` of per-run medians; thresholds are set from this, not guessed.
pub fn run_to_run_noise(run_medians: &[f64]) -> Option<f64> {
    let stats = summarize(run_medians, 0)?;
    (stats.median > 0.0).then(|| (stats.max - stats.min) / stats.median)
}

/// One `nvidia-smi` clock/power/thermal reading.
#[derive(Clone, Debug, PartialEq)]
pub struct ClockSample {
    pub gpu_name: String,
    pub graphics_mhz: f64,
    pub memory_mhz: f64,
    pub power_w: f64,
    pub temperature_c: f64,
    pub pstate: String,
    pub throttle_reasons: String,
}

pub const NVIDIA_SMI_ARGS: [&str; 2] = [
    "--query-gpu=name,clocks.gr,clocks.mem,power.draw,temperature.gpu,pstate,clocks_throttle_reasons.active",
    "--format=csv,noheader,nounits",
];

/// Parse the first line of `nvidia-smi` CSV output produced with [`NVIDIA_SMI_ARGS`].
pub fn parse_nvidia_smi(csv: &str) -> Option<ClockSample> {
    let line = csv.lines().find(|l| !l.trim().is_empty())?;
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    if f.len() != 7 {
        return None;
    }
    Some(ClockSample {
        gpu_name: f[0].to_string(),
        graphics_mhz: f[1].parse().ok()?,
        memory_mhz: f[2].parse().ok()?,
        power_w: f[3].parse().ok()?,
        temperature_c: f[4].parse().ok()?,
        pstate: f[5].to_string(),
        throttle_reasons: f[6].to_string(),
    })
}

/// Query the GPU through the `nvidia-smi` CLI. `None` when the tool is missing or fails.
pub fn query_nvidia_smi() -> Option<ClockSample> {
    let out = Command::new("nvidia-smi").args(NVIDIA_SMI_ARGS).output().ok()?;
    out.status.success().then(|| parse_nvidia_smi(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// Active Windows power scheme as reported by `powercfg`, for the run log. `None` off Windows.
pub fn query_power_scheme() -> Option<String> {
    let out = Command::new("powercfg").arg("/getactivescheme").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Fixed protocol parameters. Changing them invalidates comparisons with earlier logs.
#[derive(Clone, Debug, PartialEq)]
pub struct Protocol {
    pub warmup_frames: usize,
    pub measured_frames: usize,
    pub runs: usize,
    /// Residency VRAM caps (GB) measured in turn: mid-range tier and high tier.
    pub vram_caps_gb: Vec<u32>,
    pub internal_resolution: (u32, u32),
    /// What the workload measured when that is a choice (the terrain scene: the height field and its bake, e.g.
    /// `cpu-erosion-v1 over a 256-cell bake`). A log is compared only with one that measured the same thing.
    pub field: Option<String>,
    /// Where the world's bake was read from when that is a choice (the streaming scenes: `store DIR, N cells` or
    /// `in-memory N cells (NOT the production source)`). A log is compared only with one that read the same source.
    pub source: Option<String>,
    /// The tile generation pool of a streaming scene (STRM-020): recorded so that logs taken with another pool are not mistaken for
    /// comparable ones (TEST-012). `None` for the scenes without one.
    pub gen: Option<GenSettings>,
    /// A smoke run: never valid for budgets.
    pub smoke: bool,
}

/// The tile generation settings a streaming session ran with (`protocol.gen`): the pool size, the jobs queued behind it and, when
/// the size came from the default rule, the cores that rule left free. Recorded only: a log without them reads and is accepted
/// as before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenSettings {
    pub workers: usize,
    pub ahead: usize,
    pub reserve: Option<usize>,
    /// The workers ran at below normal OS priority (ADR 0046); a log that records no priority ran at normal.
    pub below_normal: bool,
}

impl GenSettings {
    /// The settings as one line (`workers 18, ahead 18, reserve 2`), what the comparison prints and compares.
    pub fn describe(&self) -> String {
        let priority = if self.below_normal { ", below-normal priority" } else { "" };
        match self.reserve {
            Some(r) => format!("workers {}, ahead {}, reserve {r}{priority}", self.workers, self.ahead),
            None => format!("workers {}, ahead {}{priority}", self.workers, self.ahead),
        }
    }

    fn json(g: &Option<GenSettings>) -> String {
        match g {
            None => "null".into(),
            Some(g) => format!(
                "{{\"workers\":{},\"ahead\":{},\"reserve\":{},\"priority\":\"{}\"}}",
                g.workers,
                g.ahead,
                g.reserve.map_or("null".to_string(), |r| r.to_string()),
                if g.below_normal { "below-normal" } else { "normal" }
            ),
        }
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Self {
            warmup_frames: 120,
            measured_frames: 600,
            runs: 5,
            vram_caps_gb: vec![8, 14],
            internal_resolution: (2560, 1440),
            field: None,
            source: None,
            gen: None,
            smoke: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunReport {
    pub path_name: String,
    pub vram_cap_gb: u32,
    pub stats: Stats,
    pub clocks_before: Option<ClockSample>,
    pub clocks_after: Option<ClockSample>,
    /// Further named frame-time series of the run (milliseconds), for example `gpu_ms`, `cpu_ms`, `plan_ms`.
    pub metrics: Vec<(String, Stats)>,
    /// Deterministic counts of the run (draw calls, triangles, nodes, vertex bytes), CON-20.
    pub counts: Vec<(String, u64)>,
}

pub(crate) fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub(crate) fn json_f(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".to_string()
    }
}

fn clock_json(c: &Option<ClockSample>) -> String {
    match c {
        None => "null".to_string(),
        Some(c) => format!(
            "{{\"gpu\":{},\"graphics_mhz\":{},\"memory_mhz\":{},\"power_w\":{},\"temperature_c\":{},\"pstate\":{},\"throttle_reasons\":{}}}",
            json_str(&c.gpu_name),
            json_f(c.graphics_mhz),
            json_f(c.memory_mhz),
            json_f(c.power_w),
            json_f(c.temperature_c),
            json_str(&c.pstate),
            json_str(&c.throttle_reasons)
        ),
    }
}

/// Whether a Windows power scheme is a performance scheme. The scheme GUID is language- and encoding-independent
/// (powercfg prints localised names in the OEM code page, which arrive garbled), so it is checked first; names
/// (English, German, "Ultimate") are the fallback.
pub fn is_performance_scheme(scheme: &str) -> bool {
    let s = scheme.to_lowercase();
    // High performance, Ultimate performance.
    if ["8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", "e9a42b02-d5df-448d-aa00-03f14749eb61"].iter().any(|g| s.contains(g)) {
        return true;
    }
    ["high performance", "höchstleistung", "hochleistung", "ultimate", "maximale leistung"].iter().any(|k| s.contains(k))
}

/// Reasons a session cannot be used for time budgets (CON-21), every cause with its text. Which cause voids the frame verdicts and
/// which only the GPU verdicts is decided in [`validity`].
pub fn session_warnings(adapter: &AdapterFacts, power_scheme: Option<&str>, runs: &[RunReport], smoke: bool) -> Vec<String> {
    crate::validity::session_causes(adapter, power_scheme, runs, smoke).into_iter().map(|c| c.text).collect()
}

/// `(gpu_valid, gpu_clock_state)` JSON values of a view: the state is the clock state when valid, else the reason.
fn gpu_valid_json(v: &clock::GpuValidity) -> (String, String) {
    match v {
        clock::GpuValidity::Valid(s) => ("true".into(), json_str(&s.to_string())),
        clock::GpuValidity::Invalid(r) | clock::GpuValidity::Below(_, r) => ("false".into(), json_str(r)),
    }
}

fn warmup_json(view: &str, w: &clock::WarmUpOutcome) -> String {
    format!(
        "{{\"view\":{},\"reached\":{},\"chunks\":{},\"seconds\":{},\"last\":{}}}",
        json_str(view),
        w.reached,
        w.chunks,
        json_f(w.seconds),
        w.last.as_ref().map_or("null".to_string(), |p| json_str(&format!("{} {:.0} MHz", p.pstate, p.mhz)))
    )
}

/// Serialise a whole harness session to JSON (hand-written to keep this crate dependency-free).
pub fn report_json(
    adapter: &AdapterFacts,
    power_scheme: Option<&str>,
    owner_override: &[String],
    validity: &gate::Validity,
    protocol: &Protocol,
    runs: &[RunReport],
) -> String {
    report_json_with_warmups(adapter, power_scheme, owner_override, validity, protocol, runs, &[])
}

/// The clock samples of every run of the view `path`, before and after (ADR 0034 point 2).
pub fn view_clock_points(runs: &[RunReport], path: &str) -> Vec<Option<clock::ClockPoint>> {
    runs.iter()
        .filter(|r| r.path_name == path)
        .flat_map(|r| [&r.clocks_before, &r.clocks_after])
        .map(|c| c.as_ref().map(clock::ClockPoint::from))
        .collect()
}

/// The GPU validity of the view `path` over the runs of its final batch.
pub fn view_gpu_validity(runs: &[RunReport], path: &str) -> clock::GpuValidity {
    clock::gpu_validity(&view_clock_points(runs, path))
}

/// Whether the software power cap (throttle reason 0x4) was read in any clock sample of the view `path`. Recorded in the log
/// (`power_cap` per run) and printed; it does not invalidate the view or the session (ADR 0036 point 1).
pub fn view_power_cap(runs: &[RunReport], path: &str) -> bool {
    runs.iter()
        .filter(|r| r.path_name == path)
        .flat_map(|r| [&r.clocks_before, &r.clocks_after])
        .any(|c| c.as_ref().is_some_and(|c| gate::has_power_cap(&c.throttle_reasons)))
}

/// Wait, idle, for the GPU to cool to [`clock::COOL_START_C`] before a view's warm-up (ADR 0057): polls `nvidia-smi` every
/// [`clock::COOL_POLL_S`] seconds, at most [`clock::COOL_CAP_S`]. Blocks; never on a frame path, never in a smoke run.
pub fn cool_gpu() -> clock::CoolOutcome {
    let started = std::time::Instant::now();
    clock::cool_down(
        clock::COOL_START_C,
        clock::COOL_CAP_S,
        || query_nvidia_smi().map(|c| c.temperature_c),
        || std::thread::sleep(std::time::Duration::from_secs_f64(clock::COOL_POLL_S)),
        || started.elapsed().as_secs_f64(),
    )
}

/// The temperature and power the GPU read around the runs of the view `path`, for the line beside its GPU rows (ADR 0057):
/// `68-74 C, up to 95 W`. `None` without a sample. The clock is the reading that matters, the heat and the power are why.
pub fn view_thermal_text(runs: &[RunReport], path: &str) -> Option<String> {
    let samples: Vec<&ClockSample> =
        runs.iter().filter(|r| r.path_name == path).flat_map(|r| [&r.clocks_before, &r.clocks_after]).flatten().collect();
    if samples.is_empty() {
        return None;
    }
    let (lo, hi) = samples.iter().fold((f64::MAX, f64::MIN), |(l, h), c| (l.min(c.temperature_c), h.max(c.temperature_c)));
    let power = samples.iter().map(|c| c.power_w).fold(f64::MIN, f64::max);
    let temp = if (hi - lo).abs() < 0.5 { format!("{hi:.0} C") } else { format!("{lo:.0}-{hi:.0} C") };
    Some(format!("{temp}, up to {power:.0} W"))
}

/// [`report_json`] plus the warm-up outcome of each view (`warmups`: view name and what the warm-up did).
pub fn report_json_with_warmups(
    adapter: &AdapterFacts,
    power_scheme: Option<&str>,
    owner_override: &[String],
    validity: &gate::Validity,
    protocol: &Protocol,
    runs: &[RunReport],
    warmups: &[(String, clock::WarmUpOutcome)],
) -> String {
    report_json_full(adapter, power_scheme, owner_override, validity, protocol, runs, warmups, &[])
}

/// [`report_json_with_warmups`] plus the host load of each view or scene (`load` in the run's object, `load_notes` at the top;
/// TEST-023): `loads` are keyed by the run's path name, a run without an entry carries none. Recorded only: nothing here changes a
/// validity answer.
#[allow(clippy::too_many_arguments)]
pub fn report_json_full(
    adapter: &AdapterFacts,
    power_scheme: Option<&str>,
    owner_override: &[String],
    validity: &gate::Validity,
    protocol: &Protocol,
    runs: &[RunReport],
    warmups: &[(String, clock::WarmUpOutcome)],
    loads: &[(String, load::CaseLoad)],
) -> String {
    let load_of = |path: &str| loads.iter().find(|(p, _)| p == path).map(|(_, l)| l);
    let mut load_notes: Vec<String> = Vec::new();
    for (path, l) in loads {
        load_notes.extend(l.notes().into_iter().map(|n| format!("{path}: {n}")));
    }
    let verdict = crate::validity::SessionVerdict::assess(adapter, power_scheme, runs, protocol.smoke, validity, owner_override);
    let warnings = verdict.warnings();
    let run_json: Vec<String> = runs
        .iter()
        .map(|r| {
            let gv = gpu_valid_json(&view_gpu_validity(runs, &r.path_name));
            format!(
                "{{\"path\":{},\"vram_cap_gb\":{},\"samples\":{},\"median_ms\":{},\"p99_ms\":{},\"min_ms\":{},\"max_ms\":{},\"mean_ms\":{},\"clocks_before\":{},\"clocks_after\":{},\"gpu_valid\":{},\"gpu_clock_state\":{},\"power_cap\":{},\"metrics\":{{{}}},\"counts\":{{{}}}{}}}",
                json_str(&r.path_name),
                r.vram_cap_gb,
                r.stats.samples,
                json_f(r.stats.median),
                json_f(r.stats.p99),
                json_f(r.stats.min),
                json_f(r.stats.max),
                json_f(r.stats.mean),
                clock_json(&r.clocks_before),
                clock_json(&r.clocks_after),
                gv.0,
                gv.1,
                view_power_cap(runs, &r.path_name),
                r.metrics.iter().map(|(n, s)| format!("{}:{{\"median\":{},\"p99\":{},\"max\":{},\"mean\":{},\"samples\":{}}}", json_str(n), json_f(s.median), json_f(s.p99), json_f(s.max), json_f(s.mean), s.samples)).collect::<Vec<_>>().join(","),
                r.counts.iter().map(|(n, v)| format!("{}:{v}", json_str(n))).collect::<Vec<_>>().join(","),
                load_of(&r.path_name).map_or(String::new(), |l| format!(",\"load\":{}", load::load_json(l)))
            )
        })
        .collect();
    format!(
        "{{\"adapter\":{{\"name\":{},\"backend\":{}}},\"power_scheme\":{},\"owner_override\":[{}],\"valid_for_budgets\":{},\"frame_valid\":{},\"gpu_session_valid\":{},\"gpu_only_causes\":[{}],\"warnings\":[{}],\"load_notes\":[{}],\"warmups\":[{}],\"protocol\":{{\"warmup_frames\":{},\"measured_frames\":{},\"runs\":{},\"vram_caps_gb\":{:?},\"internal_resolution\":[{},{}],\"field\":{},\"source\":{},\"gen\":{}}},\"runs\":[{}]}}\n",
        json_str(&adapter.name),
        json_str(&adapter.backend),
        power_scheme.map_or("null".to_string(), json_str),
        owner_override.iter().map(|w| json_str(w)).collect::<Vec<_>>().join(","),
        warnings.is_empty(),
        verdict.frame_valid(),
        verdict.gpu_session_valid(),
        verdict.gpu_only().iter().map(|w| json_str(w)).collect::<Vec<_>>().join(","),
        warnings.iter().map(|w| json_str(w)).collect::<Vec<_>>().join(","),
        load_notes.iter().map(|w| json_str(w)).collect::<Vec<_>>().join(","),
        warmups.iter().map(|(v, w)| warmup_json(v, w)).collect::<Vec<_>>().join(","),
        protocol.warmup_frames,
        protocol.measured_frames,
        protocol.runs,
        protocol.vram_caps_gb,
        protocol.internal_resolution.0,
        protocol.internal_resolution.1,
        protocol.field.as_deref().map_or("null".to_string(), json_str),
        protocol.source.as_deref().map_or("null".to_string(), json_str),
        GenSettings::json(&protocol.gen),
        run_json.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(name: &str, kind: DeviceKind) -> AdapterFacts {
        AdapterFacts { name: name.to_string(), backend: "Vulkan".to_string(), kind }
    }

    // spec: TEST-005
    #[test]
    fn refuses_integrated_software_and_other_discrete_gpus() {
        assert!(check_reference_adapter(&gpu("NVIDIA GeForce RTX 3080 Ti Laptop GPU", DeviceKind::Discrete)).is_ok());
        let igpu = check_reference_adapter(&gpu("Intel(R) UHD Graphics", DeviceKind::Integrated)).unwrap_err();
        assert!(igpu.0.contains("not a discrete GPU") && igpu.0.contains("High performance"), "{igpu}");
        let cpu = check_reference_adapter(&gpu("llvmpipe", DeviceKind::Cpu)).unwrap_err();
        assert!(cpu.0.contains("Cpu"), "{cpu}");
        let other = check_reference_adapter(&gpu("NVIDIA GeForce RTX 4090", DeviceKind::Discrete)).unwrap_err();
        assert!(other.0.contains("not the reference GPU"), "{other}");
    }

    // spec: TEST-006
    #[test]
    fn summary_discards_warmup_and_uses_nearest_rank_p99() {
        let mut v = vec![100.0, 100.0]; // warm-up spikes
        v.extend((1..=100).map(f64::from));
        let s = summarize(&v, 2).unwrap();
        assert_eq!(s.samples, 100);
        assert_eq!(s.median, 50.5);
        assert_eq!(s.p99, 99.0);
        assert_eq!((s.min, s.max), (1.0, 100.0));
        assert!(summarize(&v, 1000).is_none(), "warm-up longer than the run is not a result");
        assert!(summarize(&[1.0, f64::NAN], 0).is_none(), "NaN samples are rejected, not averaged");
    }

    // spec: TEST-006
    #[test]
    fn noise_is_relative_spread_of_run_medians() {
        let n = run_to_run_noise(&[10.0, 10.5, 9.5, 10.0, 10.0]).unwrap();
        assert!((n - 0.1).abs() < 1e-12, "{n}");
    }

    // spec: TEST-006
    #[test]
    fn parses_nvidia_smi_csv() {
        let csv = "NVIDIA GeForce RTX 3080 Ti Laptop GPU, 1245, 6000, 87.52, 61, P0, 0x0000000000000004\n";
        let c = parse_nvidia_smi(csv).unwrap();
        assert_eq!(c.graphics_mhz, 1245.0);
        assert_eq!(c.power_w, 87.52);
        assert_eq!(c.throttle_reasons, "0x0000000000000004");
        assert!(parse_nvidia_smi("garbage").is_none());
    }

    // spec: TEST-006
    #[test]
    fn report_json_is_logged_with_clocks_and_confirmation() {
        let a = gpu("NVIDIA GeForce RTX 3080 Ti Laptop GPU", DeviceKind::Discrete);
        let stats = summarize(&[16.0, 16.5, 17.0], 0).unwrap();
        let clock = parse_nvidia_smi("X, 1, 2, 3, 4, P0, 0x0").unwrap();
        let run = RunReport {
            path_name: "orbit".into(),
            vram_cap_gb: 8,
            stats,
            clocks_before: Some(clock),
            clocks_after: None,
            metrics: vec![],
            counts: vec![],
        };
        let j =
            report_json(&a, Some("Power Scheme GUID: x (High performance)"), &[], &gate::Validity::default(), &Protocol::default(), &[run]);
        for needle in ["\"owner_override\":[]", "\"median_ms\":16.5", "\"clocks_after\":null", "\"power_w\":3", "\"vram_caps_gb\":[8, 14]"]
        {
            assert!(j.contains(needle), "missing {needle} in {j}");
        }
    }
}

#[cfg(test)]
mod warning_tests {
    use super::*;

    fn rtx() -> AdapterFacts {
        AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete }
    }

    fn run_with(pstate: &str) -> RunReport {
        let c = parse_nvidia_smi(&format!("X, 210, 405, 15, 50, {pstate}, 0x1")).unwrap();
        RunReport {
            path_name: "skeleton".into(),
            vram_cap_gb: 8,
            stats: summarize(&[1.0, 2.0], 0).unwrap(),
            clocks_before: Some(c),
            clocks_after: None,
            metrics: vec![],
            counts: vec![],
        }
    }

    // spec: TEST-006
    #[test]
    fn balanced_power_scheme_and_idle_gpu_make_a_session_invalid_for_budgets() {
        let balanced = "GUID des Energieschemas: 381b4222 (Ausbalanciert)";
        let w = session_warnings(&rtx(), Some(balanced), &[run_with("P8")], false);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[0].contains("Ausbalanciert") && w[1].contains("P8"), "{w:?}");
        let j = report_json(
            &AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete },
            Some(balanced),
            &[],
            &gate::Validity::default(),
            &Protocol::default(),
            &[run_with("P8")],
        );
        assert!(j.contains("\"valid_for_budgets\":false") && j.contains("\"warnings\":[\"power scheme"), "{j}");
    }

    // spec: TEST-006
    #[test]
    fn the_balanced_scheme_guid_is_not_a_performance_scheme() {
        assert!(!is_performance_scheme("GUID des Energieschemas: 381b4222-f694-41f0-9685-ff5bb260df2e  (Ausbalanciert)"));
    }

    // spec: TEST-006
    #[test]
    fn performance_schemes_in_two_languages_and_a_busy_gpu_are_valid() {
        for s in [
            "Power Scheme GUID: x  (High performance)",
            "GUID des Energieschemas: y  (Höchstleistung)",
            "Ultimate Performance",
            "GUID des Energieschemas: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (H\u{fffd}chstleistung)",
        ] {
            assert!(session_warnings(&rtx(), Some(s), &[run_with("P0")], false).is_empty(), "{s}");
        }
        assert!(session_warnings(&rtx(), None, &[], false).iter().any(|w| w.contains("could not be read")));
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let j = report_json(&a, Some("High performance"), &[], &gate::Validity::default(), &Protocol::default(), &[run_with("P0")]);
        assert!(j.contains("\"valid_for_budgets\":true"), "{j}");
        let over = ["AC power connected: running on battery".to_string()];
        let j = report_json(&a, Some("High performance"), &over, &gate::Validity::default(), &Protocol::default(), &[run_with("P0")]);
        assert!(
            j.contains("\"valid_for_budgets\":false") && j.contains("\"owner_override\":[\"AC power"),
            "an overridden gate is recorded and not valid: {j}"
        );
        let noisy = gate::validity(&[], &[("terrain/x".into(), 0.2)]);
        let j = report_json(&a, Some("High performance"), &[], &noisy, &Protocol::default(), &[run_with("P0")]);
        assert!(j.contains("\"valid_for_budgets\":false") && j.contains("noise 0.200"), "{j}");
    }
}

#[cfg(test)]
mod metric_tests {
    use super::*;

    // spec: TEST-006
    #[test]
    fn named_metrics_and_counts_are_logged() {
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let run = RunReport {
            path_name: "terrain/ground-1m".into(),
            vram_cap_gb: 8,
            stats: summarize(&[5.0, 6.0, 7.0], 0).unwrap(),
            clocks_before: None,
            clocks_after: None,
            metrics: vec![("gpu_ms".into(), summarize(&[3.0, 4.0, 5.0], 0).unwrap())],
            counts: vec![("draw_calls".into(), 772), ("triangles".into(), 790_528)],
        };
        let j = report_json(&a, Some("High performance"), &[], &gate::Validity::default(), &Protocol::default(), &[run]);
        for needle in ["\"metrics\":{\"gpu_ms\":{\"median\":4", "\"counts\":{\"draw_calls\":772,\"triangles\":790528}"] {
            assert!(j.contains(needle), "missing {needle} in {j}");
        }
    }
}

/// Terrain budget of milestone M2 (report §15) and the frame-time gate (CON-21).
pub const TERRAIN_GPU_BUDGET_MS: f64 = 8.0;
pub const FRAME_P99_BUDGET_MS: f64 = 20.0;
/// CPU time to plan one frame from scratch (selection and plan; M2 gate follow-up 3): half of a 60 Hz frame. The static
/// camera of the workload makes this a full recompute, the worst case.
pub const PLAN_CPU_BUDGET_MS: f64 = 8.0;

/// Median planning time must be at most [`PLAN_CPU_BUDGET_MS`] (inclusive).
pub fn plan_budget_ok(plan_median_ms: f64) -> bool {
    plan_median_ms <= PLAN_CPU_BUDGET_MS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetVerdict {
    /// `None` when the device has no GPU timestamps.
    pub gpu_ok: Option<bool>,
    pub frame_ok: bool,
}

/// Median GPU time must be at most 8 ms and p99 frame time at most 20 ms (both inclusive).
pub fn budget_verdict(gpu_median_ms: Option<f64>, frame_p99_ms: f64) -> BudgetVerdict {
    BudgetVerdict { gpu_ok: gpu_median_ms.map(|g| g <= TERRAIN_GPU_BUDGET_MS), frame_ok: frame_p99_ms <= FRAME_P99_BUDGET_MS }
}

/// The console line of one terrain view (ADR 0034 as amended by the owner, 2026-10-15). The frame verdict (p99 against the
/// budget) holds at any clock state and the state is printed beside it; the GPU median and its verdict need every clock sample
/// at or above the floor (ADR 0036), else the line says `NOT VALID for GPU budgets (<state>)` and shows the GPU median without a verdict. `smoke` is no measurement: no state.
pub fn terrain_view_line(
    name: &str,
    total: &Stats,
    gpu: Option<&Stats>,
    clock: &clock::GpuValidity,
    power_cap: bool,
    smoke: bool,
) -> String {
    let v = budget_verdict(gpu.map(|g| g.median), total.p99);
    let word = |ok: bool| if ok { "ok" } else { "OVER" };
    let frame =
        format!("frame median {:.2} ms, p99 {:.2} ms (budget {}) {}", total.median, total.p99, FRAME_P99_BUDGET_MS, word(v.frame_ok));
    let state = match clock {
        clock::GpuValidity::Valid(s) => s.to_string(),
        clock::GpuValidity::Invalid(r) | clock::GpuValidity::Below(_, r) => r.clone(),
    };
    let cap = if power_cap && !smoke { " [power cap 0x4 recorded, not invalidating]" } else { "" };
    let tag = if smoke { String::new() } else { format!(" [clock state {state}]{cap}") };
    let gpu_text = match (gpu, clock) {
        (None, _) => "no GPU timestamps (no GPU verdict)".to_string(),
        (Some(g), clock::GpuValidity::Invalid(r) | clock::GpuValidity::Below(_, r)) if !smoke => {
            format!("NOT VALID for GPU budgets ({r}): GPU median {:.2} ms (no verdict)", g.median)
        }
        (Some(g), _) => format!("GPU median {:.2} ms (budget {}) {}", g.median, TERRAIN_GPU_BUDGET_MS, word(v.gpu_ok == Some(true))),
    };
    format!("  {name}: {gpu_text}; {frame}{tag}")
}

#[cfg(test)]
mod view_line_tests {
    use super::*;
    use clock::{ClockState, GpuValidity};

    fn stats(median: f64, p99: f64) -> Stats {
        Stats { samples: 3, median, p99, min: median, max: p99, mean: median }
    }

    // spec: TEST-012
    #[test]
    fn a_view_below_the_floor_keeps_its_frame_verdict_and_loses_its_gpu_verdict() {
        let p2 = GpuValidity::Invalid("P2 1150 MHz, below the 1400 MHz floor".into());
        let line = terrain_view_line("ground-1m", &stats(7.0, 25.0), Some(&stats(4.2, 5.0)), &p2, false, false);
        assert!(
            line.contains("NOT VALID for GPU budgets (P2 1150 MHz, below the 1400 MHz floor): GPU median 4.20 ms (no verdict)"),
            "{line}"
        );
        assert!(line.contains("frame median 7.00 ms, p99 25.00 ms (budget 20) OVER"), "the frame verdict stays: {line}");
        assert!(line.ends_with("[clock state P2 1150 MHz, below the 1400 MHz floor]"), "{line}");
        let ok = terrain_view_line("orbit", &stats(1.0, 2.0), Some(&stats(9.0, 9.0)), &p2, false, false);
        assert!(ok.contains("frame median 1.00 ms, p99 2.00 ms (budget 20) ok") && !ok.contains("GPU median 9.00 ms (budget"), "{ok}");
    }

    // spec: TEST-012
    #[test]
    fn a_view_at_the_floor_has_both_verdicts_and_the_state_and_smoke_has_no_state() {
        let p0 = GpuValidity::Valid(ClockState { pstate: "P0".into(), mhz: 1470.0 });
        let line = terrain_view_line("ground-1m", &stats(7.0, 9.0), Some(&stats(9.0, 9.5)), &p0, false, false);
        assert!(line.contains("GPU median 9.00 ms (budget 8) OVER") && line.contains("p99 9.00 ms (budget 20) ok"), "{line}");
        assert!(line.ends_with("[clock state P0 1470 MHz]"), "{line}");
        let smoke = terrain_view_line(
            "ground-1m",
            &stats(7.0, 9.0),
            Some(&stats(1.0, 1.0)),
            &GpuValidity::Invalid("no clock sample".into()),
            true,
            true,
        );
        assert!(
            smoke.contains("GPU median 1.00 ms (budget 8) ok") && !smoke.contains("clock state") && !smoke.contains("NOT VALID"),
            "{smoke}"
        );
        let none = terrain_view_line("x", &stats(7.0, 9.0), None, &p0, false, false);
        assert!(none.contains("no GPU timestamps") && none.contains("frame median 7.00"), "{none}");
    }
}

#[cfg(test)]
mod power_cap_tests {
    use super::*;

    fn run(path: &str, smi: &str) -> RunReport {
        RunReport {
            path_name: path.into(),
            vram_cap_gb: 8,
            stats: summarize(&[5.0, 6.0, 7.0], 0).unwrap(),
            clocks_before: parse_nvidia_smi(smi),
            clocks_after: parse_nvidia_smi(smi),
            metrics: vec![],
            counts: vec![],
        }
    }

    // spec: TEST-012
    #[test]
    fn a_power_capped_view_is_recorded_in_the_log_and_the_session_stays_valid() {
        let a = AdapterFacts { name: "RTX 3080 Ti".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let runs = [
            run("terrain/ground-1m", "X, 1500, 810, 88, 54, P0, 0x0000000000000004"),
            run("terrain/orbit", "X, 1710, 810, 40, 50, P2, 0x0000000000000000"),
        ];
        assert!(view_power_cap(&runs, "terrain/ground-1m") && !view_power_cap(&runs, "terrain/orbit"));
        // The throttling rule the harness applies to the samples lets both runs through.
        assert!(runs.iter().all(|r| r.clocks_before.iter().all(|c| !gate::is_throttling(&c.throttle_reasons))));
        let j = report_json(&a, Some("High performance"), &[], &gate::validity(&[], &[]), &Protocol::default(), &runs);
        assert!(j.contains("\"valid_for_budgets\":true"), "{j}");
        assert!(j.contains("\"power_cap\":true") && j.contains("\"power_cap\":false"), "{j}");
        assert!(j.contains("\"throttle_reasons\":\"0x0000000000000004\""), "the raw reasons stay in the samples: {j}");
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    // spec: TEST-012
    #[test]
    fn budgets_are_inclusive_and_missing_timestamps_are_not_a_pass() {
        assert_eq!(budget_verdict(Some(8.0), 20.0), BudgetVerdict { gpu_ok: Some(true), frame_ok: true });
        assert_eq!(budget_verdict(Some(8.0001), 20.0).gpu_ok, Some(false));
        assert!(!budget_verdict(Some(1.0), 20.0001).frame_ok);
        assert!(plan_budget_ok(8.0) && !plan_budget_ok(8.0001) && !plan_budget_ok(f64::NAN), "NaN must not pass a budget");
        assert_eq!(budget_verdict(None, 5.0).gpu_ok, None, "no timestamps: no GPU verdict");
    }

    // spec: TEST-012
    #[test]
    fn smoke_and_software_adapters_are_never_valid_for_budgets() {
        let soft = AdapterFacts { name: "Microsoft Basic Render Driver".into(), backend: "Dx12".into(), kind: DeviceKind::Cpu };
        let w = session_warnings(&soft, Some("High performance"), &[], true);
        assert!(w.iter().any(|m| m.contains("smoke run")) && w.iter().any(|m| m.contains("not the reference adapter")), "{w:?}");
        let proto = Protocol { smoke: true, ..Protocol::default() };
        let rtx =
            AdapterFacts { name: "NVIDIA GeForce RTX 3080 Ti Laptop GPU".into(), backend: "Vulkan".into(), kind: DeviceKind::Discrete };
        let j = report_json(&rtx, Some("High performance"), &[], &gate::Validity::default(), &proto, &[]);
        assert!(j.contains("\"valid_for_budgets\":false") && j.contains("smoke run"), "{j}");
    }
}
