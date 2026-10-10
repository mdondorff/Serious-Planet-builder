//! Performance-harness protocol (CON-21, report §8): adapter check, logged clocks and power,
//! warm-up, medians and p99, JSON run logs. GPU-free: adapter facts arrive as plain structs and the
//! frame loop itself is supplied by the renderer once it exists (M2).

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
}

impl Default for Protocol {
    fn default() -> Self {
        Self { warmup_frames: 120, measured_frames: 600, runs: 5, vram_caps_gb: vec![8, 14], internal_resolution: (2560, 1440) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunReport {
    pub path_name: String,
    pub vram_cap_gb: u32,
    pub stats: Stats,
    pub clocks_before: Option<ClockSample>,
    pub clocks_after: Option<ClockSample>,
}

fn json_str(s: &str) -> String {
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

fn json_f(v: f64) -> String {
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

/// Serialise a whole harness session to JSON (hand-written to keep this crate dependency-free).
pub fn report_json(
    adapter: &AdapterFacts,
    power_scheme: Option<&str>,
    owner_confirmed: bool,
    protocol: &Protocol,
    runs: &[RunReport],
) -> String {
    let run_json: Vec<String> = runs
        .iter()
        .map(|r| {
            format!(
                "{{\"path\":{},\"vram_cap_gb\":{},\"samples\":{},\"median_ms\":{},\"p99_ms\":{},\"min_ms\":{},\"max_ms\":{},\"mean_ms\":{},\"clocks_before\":{},\"clocks_after\":{}}}",
                json_str(&r.path_name),
                r.vram_cap_gb,
                r.stats.samples,
                json_f(r.stats.median),
                json_f(r.stats.p99),
                json_f(r.stats.min),
                json_f(r.stats.max),
                json_f(r.stats.mean),
                clock_json(&r.clocks_before),
                clock_json(&r.clocks_after)
            )
        })
        .collect();
    format!(
        "{{\"adapter\":{{\"name\":{},\"backend\":{}}},\"power_scheme\":{},\"owner_confirmed_machine_ready\":{},\"protocol\":{{\"warmup_frames\":{},\"measured_frames\":{},\"runs\":{},\"vram_caps_gb\":{:?},\"internal_resolution\":[{},{}]}},\"runs\":[{}]}}\n",
        json_str(&adapter.name),
        json_str(&adapter.backend),
        power_scheme.map_or("null".to_string(), json_str),
        owner_confirmed,
        protocol.warmup_frames,
        protocol.measured_frames,
        protocol.runs,
        protocol.vram_caps_gb,
        protocol.internal_resolution.0,
        protocol.internal_resolution.1,
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
        let run = RunReport { path_name: "orbit".into(), vram_cap_gb: 8, stats, clocks_before: Some(clock), clocks_after: None };
        let j = report_json(&a, Some("Power Scheme GUID: x (High performance)"), true, &Protocol::default(), &[run]);
        for needle in [
            "\"owner_confirmed_machine_ready\":true",
            "\"median_ms\":16.5",
            "\"clocks_after\":null",
            "\"power_w\":3",
            "\"vram_caps_gb\":[8, 14]",
        ] {
            assert!(j.contains(needle), "missing {needle} in {j}");
        }
    }
}
