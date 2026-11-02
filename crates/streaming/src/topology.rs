//! The machine's CPU topology (BUDG-013, ADR 0092 point 4): logical threads on performance cores `p` and on efficiency cores `e`, from the
//! operating system with no new dependency. Windows reads the core records of `GetLogicalProcessorInformationEx`, Linux the hybrid sysfs
//! lists or the core capacities, and anything else (or a failed query) counts every thread of `available_parallelism` as performance.
//! `p` is never more than the threads the process may use. `PLANET_CPU_TOPOLOGY=P,E` overrides the detection. Every source is parsed by a
//! pure function, tested on recorded bytes and strings; the queries are thin.

/// The environment variable that overrides the detection (`P,E`, P at least 1).
pub const TOPOLOGY_ENV: &str = "PLANET_CPU_TOPOLOGY";

/// Logical threads on performance cores (`p`, SMT siblings counted) and on efficiency cores (`e`, reported only: the shares count `p`,
/// BUDG-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuTopology {
    pub p: usize,
    pub e: usize,
}

impl CpuTopology {
    /// `p` capped to the threads the process may use (`available`, which follows affinity and quotas), at least 1.
    pub fn capped(self, available: usize) -> CpuTopology {
        CpuTopology { p: self.p.min(available).max(1), e: self.e }
    }
}

/// Parse the override `P,E`: two counts, P at least 1.
pub fn parse_override(text: &str) -> Result<CpuTopology, String> {
    let bad = || format!("{TOPOLOGY_ENV} must be P,E with P at least 1 (got '{text}')");
    let (p, e) = text.trim().split_once(',').ok_or_else(bad)?;
    let p: usize = p.trim().parse().map_err(|_| bad())?;
    let e: usize = e.trim().parse().map_err(|_| bad())?;
    if p == 0 {
        return Err(bad());
    }
    Ok(CpuTopology { p, e })
}

/// One core of the Windows records: its efficiency class (higher is faster) and its logical threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreRecord {
    pub efficiency_class: u8,
    pub threads: usize,
}

/// The topology of cores by class: the highest class is performance, every lower one efficiency; one class (or none) is all performance.
pub fn from_core_records(cores: &[CoreRecord]) -> Option<CpuTopology> {
    let top = cores.iter().map(|c| c.efficiency_class).max()?;
    let (mut p, mut e) = (0, 0);
    for c in cores {
        if c.efficiency_class == top {
            p += c.threads;
        } else {
            e += c.threads;
        }
    }
    (p > 0).then_some(CpuTopology { p, e })
}

/// `RelationProcessorCore`, the relationship of the records read.
const RELATION_PROCESSOR_CORE: u32 = 0;

/// Parse the buffer `GetLogicalProcessorInformationEx(RelationProcessorCore)` fills: variable-sized records of `Relationship: u32`,
/// `Size: u32`, then `PROCESSOR_RELATIONSHIP` (`Flags: u8`, `EfficiencyClass: u8`, 20 reserved bytes, `GroupCount: u16`, and at offset 32
/// of the record `GroupCount` `GROUP_AFFINITY` entries of 16 bytes, a `u64` mask first). A core's threads are the set bits of its masks,
/// summed over its groups; records of another relationship are skipped. `None` for a buffer that does not parse.
pub fn parse_windows_records(buf: &[u8]) -> Option<Vec<CoreRecord>> {
    let u16_at = |b: &[u8], at: usize| b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]));
    let u32_at = |b: &[u8], at: usize| b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let u64_at = |b: &[u8], at: usize| b.get(at..at + 8).map(|s| u64::from_le_bytes(s.try_into().expect("8 bytes")));
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < buf.len() {
        let relationship = u32_at(buf, at)?;
        let size = u32_at(buf, at + 4)? as usize;
        if size < 8 || at + size > buf.len() {
            return None;
        }
        let rec = &buf[at..at + size];
        if relationship == RELATION_PROCESSOR_CORE {
            let efficiency_class = *rec.get(9)?;
            let groups = u16_at(rec, 30)? as usize;
            let mut threads = 0usize;
            for g in 0..groups {
                threads += u64_at(rec, 32 + 16 * g)?.count_ones() as usize;
            }
            out.push(CoreRecord { efficiency_class, threads });
        }
        at += size;
    }
    Some(out)
}

/// The number of CPUs in a sysfs CPU list (`0-11`, `0-3,8,10-11`); `None` when it does not parse. An empty list is 0.
pub fn parse_cpu_list(text: &str) -> Option<usize> {
    let mut n = 0usize;
    for part in text.trim().split(',').filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b): (usize, usize) = (a.parse().ok()?, b.parse().ok()?);
                if b < a {
                    return None;
                }
                n += b - a + 1;
            }
            None => {
                part.parse::<usize>().ok()?;
                n += 1;
            }
        }
    }
    Some(n)
}

/// The topology from the hybrid sysfs lists (`/sys/devices/cpu_core/cpus`, `/sys/devices/cpu_atom/cpus`): `None` without a core list.
pub fn from_sysfs_lists(core: Option<&str>, atom: Option<&str>) -> Option<CpuTopology> {
    let p = parse_cpu_list(core?)?;
    let e = match atom {
        Some(a) => parse_cpu_list(a)?,
        None => 0,
    };
    (p > 0).then_some(CpuTopology { p, e })
}

/// The topology from per-CPU capacities (`cpu_capacity`, 1024 the fastest): the highest value is performance, the rest efficiency.
pub fn from_capacities(capacities: &[u32]) -> Option<CpuTopology> {
    let top = *capacities.iter().max()?;
    let p = capacities.iter().filter(|&&c| c == top).count();
    Some(CpuTopology { p, e: capacities.len() - p })
}

/// The threads the process may use (`available_parallelism`, 1 if it cannot be read).
pub fn available_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// The machine's topology (BUDG-013): the override if set (a malformed one is an error), else the platform's source, else every
/// available thread as performance; `p` capped to the available threads.
pub fn detect() -> Result<CpuTopology, String> {
    let available = available_threads();
    if let Ok(text) = std::env::var(TOPOLOGY_ENV) {
        return parse_override(&text);
    }
    let found = platform_topology().unwrap_or(CpuTopology { p: available, e: 0 });
    Ok(found.capped(available))
}

#[cfg(windows)]
fn platform_topology() -> Option<CpuTopology> {
    use windows::Win32::System::SystemInformation::{GetLogicalProcessorInformationEx, RelationProcessorCore};
    let mut len = 0u32;
    // The first call fails with the needed length.
    let _ = unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut len) };
    if len == 0 {
        return None;
    }
    // u64 storage keeps the records' 8-byte alignment.
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, Some(buf.as_mut_ptr().cast()), &mut len) }.ok()?;
    let bytes: Vec<u8> = buf.iter().flat_map(|w| w.to_le_bytes()).take(len as usize).collect();
    from_core_records(&parse_windows_records(&bytes)?)
}

#[cfg(target_os = "linux")]
fn platform_topology() -> Option<CpuTopology> {
    let read = |p: &str| std::fs::read_to_string(p).ok();
    if let Some(t) = from_sysfs_lists(read("/sys/devices/cpu_core/cpus").as_deref(), read("/sys/devices/cpu_atom/cpus").as_deref()) {
        return Some(t);
    }
    let mut caps = Vec::new();
    for cpu in 0..available_threads().max(1) * 4 {
        match read(&format!("/sys/devices/system/cpu/cpu{cpu}/cpu_capacity")) {
            Some(c) => caps.push(c.trim().parse().ok()?),
            None if cpu == 0 => return None,
            None => break,
        }
    }
    from_capacities(&caps)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_topology() -> Option<CpuTopology> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One core record as the API writes it: one affinity mask per processor group the core spans.
    fn record(class: u8, masks: &[u64]) -> Vec<u8> {
        let size = 32 + 16 * masks.len();
        let mut r = vec![0u8; size];
        r[0..4].copy_from_slice(&RELATION_PROCESSOR_CORE.to_le_bytes());
        r[4..8].copy_from_slice(&(size as u32).to_le_bytes());
        r[8] = u8::from(masks.iter().map(|m| m.count_ones()).sum::<u32>() > 1); // LTP_PC_SMT, not read
        r[9] = class;
        r[30..32].copy_from_slice(&(masks.len() as u16).to_le_bytes());
        for (g, m) in masks.iter().enumerate() {
            r[32 + 16 * g..40 + 16 * g].copy_from_slice(&m.to_le_bytes());
            r[40 + 16 * g..42 + 16 * g].copy_from_slice(&(g as u16).to_le_bytes());
        }
        r
    }

    // spec: BUDG-013
    #[test]
    fn windows_core_records_give_the_performance_and_efficiency_threads_in_one_or_two_groups() {
        // 6 P-cores with 2 threads each (class 1) and 8 E-cores with 1 (class 0), in one group.
        let mut one = Vec::new();
        for k in 0..6 {
            one.extend(record(1, &[0b11 << (2 * k)]));
        }
        for k in 0..8 {
            one.extend(record(0, &[1 << (12 + k)]));
        }
        let t = from_core_records(&parse_windows_records(&one).unwrap()).unwrap();
        assert_eq!(t, CpuTopology { p: 12, e: 8 });
        // The same machine split over two processor groups: a P-core whose two threads sit in different groups.
        let mut two = Vec::new();
        for _ in 0..5 {
            two.extend(record(1, &[0b11]));
        }
        two.extend(record(1, &[0b1, 0b1]));
        for _ in 0..8 {
            two.extend(record(0, &[0b1]));
        }
        assert_eq!(from_core_records(&parse_windows_records(&two).unwrap()).unwrap(), CpuTopology { p: 12, e: 8 });
        // 8 cores of 2 threads in a single class: all performance.
        let mut flat = Vec::new();
        for k in 0..8 {
            flat.extend(record(0, &[0b11 << (2 * k)]));
        }
        assert_eq!(from_core_records(&parse_windows_records(&flat).unwrap()).unwrap(), CpuTopology { p: 16, e: 0 });
        // A record of another relationship is skipped; a truncated buffer does not parse.
        let mut other = record(1, &[0b11]);
        other[0..4].copy_from_slice(&2u32.to_le_bytes());
        other.extend(record(1, &[0b1]));
        assert_eq!(parse_windows_records(&other).unwrap(), vec![CoreRecord { efficiency_class: 1, threads: 1 }]);
        assert_eq!(parse_windows_records(&one[..40]), None);
        assert_eq!(from_core_records(&[]), None);
    }

    // spec: BUDG-013
    #[test]
    fn linux_sysfs_lists_and_capacities_give_the_performance_and_efficiency_threads() {
        assert_eq!(from_sysfs_lists(Some("0-11\n"), Some("12-19\n")), Some(CpuTopology { p: 12, e: 8 }));
        assert_eq!(from_sysfs_lists(Some("0-3,8,10-11"), None), Some(CpuTopology { p: 7, e: 0 }));
        assert_eq!(from_sysfs_lists(None, Some("0-7")), None, "no core list: not a hybrid machine");
        assert_eq!(parse_cpu_list(""), Some(0));
        assert_eq!(parse_cpu_list("3-1"), None);
        assert_eq!(parse_cpu_list("a"), None);
        assert_eq!(from_capacities(&[1024, 1024, 1024, 1024, 512, 512, 512, 512]), Some(CpuTopology { p: 4, e: 4 }));
        assert_eq!(from_capacities(&[1024; 6]), Some(CpuTopology { p: 6, e: 0 }));
        assert_eq!(from_capacities(&[]), None);
    }

    // spec: BUDG-013
    #[test]
    fn the_performance_threads_are_capped_to_the_usable_threads_and_the_override_is_checked() {
        assert_eq!(CpuTopology { p: 12, e: 8 }.capped(8), CpuTopology { p: 8, e: 8 });
        assert_eq!(CpuTopology { p: 12, e: 8 }.capped(0), CpuTopology { p: 1, e: 8 });
        assert_eq!(parse_override("12,8"), Ok(CpuTopology { p: 12, e: 8 }));
        assert_eq!(parse_override(" 16 , 0 "), Ok(CpuTopology { p: 16, e: 0 }));
        for bad in ["0,4", "x", "12", "12,-1", ""] {
            let e = parse_override(bad).unwrap_err();
            assert!(e.contains(TOPOLOGY_ENV) && e.contains(&format!("'{bad}'")), "{bad}: {e}");
        }
        // The live detection never reports more performance threads than the process may use.
        if std::env::var(TOPOLOGY_ENV).is_err() {
            let t = detect().unwrap();
            assert!(t.p >= 1 && t.p <= available_threads(), "{t:?} on {} usable threads", available_threads());
            eprintln!("detected {t:?} on {} usable threads", available_threads());
        }
    }
}
