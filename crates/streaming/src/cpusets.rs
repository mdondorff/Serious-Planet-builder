//! SPIKE-90 (spike branch only, never merged): the dual pool's core classes. With `PLANET_DUAL_POOL` set, the critical workers are kept on the
//! performance cores and the background workers on the efficiency cores through Windows CPU sets (`SetThreadSelectedCpuSets`, a soft affinity the
//! scheduler honours). A machine without two efficiency classes, and Linux, keep one pool: `pin_current` reports `false` and changes nothing.
//! Placement changes when a thread runs, never what it computes (CON-14).

/// Where a worker should run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreClass {
    /// The cores of the highest efficiency class (the P-cores of a hybrid CPU).
    Performance,
    /// Every core below the highest class (the E-cores).
    Efficiency,
}

/// Whether the dual pool is asked for (`PLANET_DUAL_POOL` set to anything but `0`).
pub fn dual_pool_enabled() -> bool {
    std::env::var("PLANET_DUAL_POOL").is_ok_and(|v| v != "0")
}

/// The logical processor indices of each class: `(performance, efficiency)`; both empty where the platform cannot say or the CPU has one class.
pub fn classes() -> (Vec<u32>, Vec<u32>) {
    let sets = imp::cpu_sets();
    let Some(top) = sets.iter().map(|s| s.efficiency_class).max() else { return (Vec::new(), Vec::new()) };
    if sets.iter().all(|s| s.efficiency_class == top) {
        return (Vec::new(), Vec::new());
    }
    let p = sets.iter().filter(|s| s.efficiency_class == top).map(|s| s.logical).collect();
    let e = sets.iter().filter(|s| s.efficiency_class < top).map(|s| s.logical).collect();
    (p, e)
}

/// Keep the calling thread on the cores of `class`; `false` (nothing changed) on a CPU with one class or a platform without CPU sets.
pub fn pin_current(class: CoreClass) -> bool {
    let sets = imp::cpu_sets();
    let Some(top) = sets.iter().map(|s| s.efficiency_class).max() else { return false };
    if sets.iter().all(|s| s.efficiency_class == top) {
        return false;
    }
    let ids: Vec<u32> = sets
        .iter()
        .filter(|s| match class {
            CoreClass::Performance => s.efficiency_class == top,
            CoreClass::Efficiency => s.efficiency_class < top,
        })
        .map(|s| s.id)
        .collect();
    imp::select(&ids)
}

#[derive(Clone, Copy, Debug)]
struct CpuSet {
    id: u32,
    logical: u32,
    efficiency_class: u8,
}

#[cfg(windows)]
mod imp {
    use super::CpuSet;
    use windows::Win32::System::SystemInformation::{CpuSetInformation, GetSystemCpuSetInformation, SYSTEM_CPU_SET_INFORMATION};
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadSelectedCpuSets};

    pub fn cpu_sets() -> Vec<CpuSet> {
        let mut len = 0u32;
        // SAFETY: a size query with no buffer; the call writes only `len`.
        let _ = unsafe { GetSystemCpuSetInformation(None, 0, &mut len, None, None) };
        if len == 0 {
            return Vec::new();
        }
        // u64 words keep the records aligned for the struct reads below.
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: the buffer holds `len` bytes, the size the first call asked for.
        let ok = unsafe { GetSystemCpuSetInformation(Some(buf.as_mut_ptr().cast()), len, &mut len, None, None) };
        if !ok.as_bool() {
            return Vec::new();
        }
        let bytes = buf.as_ptr().cast::<u8>();
        let mut out = Vec::new();
        let mut at = 0usize;
        while at + std::mem::size_of::<u32>() * 2 <= len as usize {
            // SAFETY: `at` is the start of a record inside the filled bytes; each record begins with its own size, and the CpuSet
            // member is read only for records of that type, which are at least the struct's size.
            let rec = unsafe { &*bytes.add(at).cast::<SYSTEM_CPU_SET_INFORMATION>() };
            if rec.Size == 0 {
                break;
            }
            if rec.Type == CpuSetInformation {
                // SAFETY: the record's type says the union holds a CpuSet.
                let c = unsafe { rec.Anonymous.CpuSet };
                out.push(CpuSet {
                    id: c.Id,
                    logical: u32::from(c.Group) * 64 + u32::from(c.LogicalProcessorIndex),
                    efficiency_class: c.EfficiencyClass,
                });
            }
            at += rec.Size as usize;
        }
        out
    }

    pub fn select(ids: &[u32]) -> bool {
        if ids.is_empty() {
            return false;
        }
        // SAFETY: the pseudo handle of the calling thread is always valid; the ids come from the system's own CPU set list.
        unsafe { SetThreadSelectedCpuSets(GetCurrentThread(), ids).as_bool() }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::CpuSet;

    pub fn cpu_sets() -> Vec<CpuSet> {
        Vec::new()
    }

    pub fn select(_: &[u32]) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A probe of the machine, not a test of a rule (spike).
    #[test]
    fn the_classes_partition_the_machine_or_are_both_empty() {
        let (p, e) = classes();
        eprintln!("performance {p:?}, efficiency {e:?}");
        assert_eq!(p.is_empty(), e.is_empty(), "either a hybrid CPU with both classes or neither");
        assert!(p.iter().all(|x| !e.contains(x)));
    }
}
