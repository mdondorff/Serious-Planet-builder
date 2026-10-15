//! The host's CPU clock and power plan for the run log (TEST-012, TEST-019). Recorded, never judged: a slow clock or an odd plan
//! invalidates nothing (the readiness gate of ADR 0020 refuses non-performance plans on its own). Parsing is pure and tested on
//! recorded strings; the queries are one short PowerShell or `powercfg` call outside any frame.

use std::process::Command;

/// The CPU's effective clock at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CpuClock {
    /// Base frequency of the package, MHz (`ProcessorFrequency` of the performance counter).
    pub base_mhz: f64,
    /// `% Processor Performance` of all logical processors: 100 is the base clock, above 100 is turbo.
    pub performance_pct: f64,
}

impl CpuClock {
    /// The effective clock: the base frequency times the performance percentage.
    pub fn effective_mhz(&self) -> f64 {
        self.base_mhz * self.performance_pct / 100.0
    }
}

/// The active power plan: its GUID and its (possibly localised) name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
}

impl std::fmt::Display for PowerPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.guid)
    }
}

fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5 && parts.iter().zip([8usize, 4, 4, 4, 12]).all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Parse `powercfg /getactivescheme` (`Power Scheme GUID: <guid>  (<name>)`, localised labels: the GUID is found by its shape and
/// the name is the last parenthesised text).
pub fn parse_power_plan(text: &str) -> Option<PowerPlan> {
    let line = text.lines().find(|l| l.split(|c: char| c.is_whitespace() || c == ':').any(is_guid))?;
    let guid = line.split(|c: char| c.is_whitespace() || c == ':').find(|w| is_guid(w))?.to_ascii_lowercase();
    let open = line.rfind('(')?;
    let close = line.rfind(')')?;
    (close > open).then(|| PowerPlan { guid, name: line[open + 1..close].trim().to_string() })
}

/// Parse the clock query's output: `<performance percent>,<base MHz>`. Anything else (an empty counter, a failed query) is `None`.
pub fn parse_cpu_clock(text: &str) -> Option<CpuClock> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let (pct, base) = line.split_once(',')?;
    let pct: f64 = pct.trim().parse().ok()?;
    let base: f64 = base.trim().parse().ok()?;
    (pct.is_finite() && base.is_finite() && pct > 0.0 && base > 0.0).then_some(CpuClock { base_mhz: base, performance_pct: pct })
}

/// The active power plan through `powercfg`. `None` off Windows or when the call fails.
pub fn query_power_plan() -> Option<PowerPlan> {
    let out = Command::new("powercfg").arg("/getactivescheme").output().ok()?;
    out.status.success().then(|| parse_power_plan(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// The CPU's effective clock now, from the `Processor Information` performance counter through WMI (the class name is the same in
/// every locale, unlike the counter path; about two seconds, so never on a frame path). `None` where it cannot be read.
pub fn query_cpu_clock() -> Option<CpuClock> {
    let script = "$c = Get-CimInstance Win32_PerfFormattedData_Counters_ProcessorInformation -Filter \"Name='_Total'\"; \
                  '{0},{1}' -f $c.PercentProcessorPerformance,$c.ProcessorFrequency";
    let out = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).output().ok()?;
    out.status.success().then(|| parse_cpu_clock(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// `<MHz> MHz` or `not measured`, for the console.
pub fn clock_text(c: Option<&CpuClock>) -> String {
    c.map_or("not measured".to_string(), |c| format!("{:.0} MHz", c.effective_mhz()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TEST-012
    #[test]
    fn the_power_plan_is_parsed_with_its_guid_and_name_in_any_locale() {
        let en = "Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (High performance)\r\n";
        let p = parse_power_plan(en).unwrap();
        assert_eq!((p.guid.as_str(), p.name.as_str()), ("8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", "High performance"));
        let de = "GUID des Energieschemas: 381B4222-F694-41F0-9685-FF5BB260DF2E  (Ausbalanciert)\n";
        let p = parse_power_plan(de).unwrap();
        assert_eq!((p.guid.as_str(), p.name.as_str()), ("381b4222-f694-41f0-9685-ff5bb260df2e", "Ausbalanciert"));
        assert_eq!(p.to_string(), "Ausbalanciert (381b4222-f694-41f0-9685-ff5bb260df2e)");
        assert!(parse_power_plan("").is_none() && parse_power_plan("not a plan (x)").is_none());
    }

    // spec: TEST-012
    #[test]
    fn the_cpu_clock_is_the_base_frequency_times_the_performance_percentage() {
        let c = parse_cpu_clock("147,2160\r\n").unwrap();
        assert_eq!((c.performance_pct, c.base_mhz), (147.0, 2160.0));
        assert!((c.effective_mhz() - 3175.2).abs() < 1e-9);
        assert_eq!(clock_text(Some(&c)), "3175 MHz");
        assert_eq!(clock_text(None), "not measured");
        for bad in ["", ",2400", "abc,def", "0,2400", "50"] {
            assert!(parse_cpu_clock(bad).is_none(), "{bad:?}");
        }
    }

    // spec: TEST-012
    #[test]
    #[ignore = "reads the live host (about two seconds of PowerShell); run by hand on Windows"]
    fn the_live_queries_read_this_host() {
        let plan = query_power_plan().expect("powercfg");
        let clock = query_cpu_clock().expect("processor performance counter");
        eprintln!("{plan}; {}", clock_text(Some(&clock)));
        assert!(clock.effective_mhz() > 100.0 && clock.effective_mhz() < 10_000.0);
    }
}
