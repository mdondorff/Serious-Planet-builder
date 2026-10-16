//! The adapter gate (VIEW-001, owner 2026-10-16): which GPU the application may run on, decided on plain descriptions of the
//! adapters before any device exists (CON-03). `render` enumerates the adapters and describes each one as an [`AdapterDesc`];
//! this module picks one or refuses with a message that names the reason for every adapter it turned down.
//!
//! The rule: the interactive application runs on a discrete GPU that has every required feature and limit. An integrated GPU,
//! a software renderer, a GPU that cannot present to the window, and an adapter that lacks a feature or falls short of a limit
//! are refused. The software adapters (WARP, lavapipe) run only in the test and CI mode ([`SelectionMode::TestSoftware`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// What kind of device an adapter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    Discrete,
    Integrated,
    /// A virtual GPU (a hypervisor's device).
    Virtual,
    /// A software renderer: WARP on Windows, lavapipe or llvmpipe on Linux.
    Software,
    Other,
}

impl DeviceClass {
    fn describe(self) -> &'static str {
        match self {
            DeviceClass::Discrete => "a discrete GPU",
            DeviceClass::Integrated => "an integrated GPU",
            DeviceClass::Virtual => "a virtual GPU",
            DeviceClass::Software => "a software renderer",
            DeviceClass::Other => "a device of unknown type",
        }
    }
}

/// One adapter as the gate sees it: plain data, no graphics API type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterDesc {
    pub name: String,
    /// The graphics backend, e.g. `Vulkan` or `Dx12`.
    pub backend: String,
    pub class: DeviceClass,
    /// Names of the features the adapter supports.
    pub features: BTreeSet<String>,
    /// Limits by name, in the unit the limit is stated in.
    pub limits: BTreeMap<String, u64>,
    /// Whether the adapter can present to the window's surface; `None` for a headless selection.
    pub can_present: Option<bool>,
}

/// The features and limits every adapter must offer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requirements {
    pub features: Vec<String>,
    /// `(name, minimum)`: the adapter's limit must be at least the minimum.
    pub limits: Vec<(String, u64)>,
}

/// Which adapters a run may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    /// The application: a discrete GPU only.
    Interactive,
    /// The test and CI mode (`PLANET_ADAPTER=software`): a software adapter only, so that runs are reproducible.
    TestSoftware,
}

/// Why no adapter was chosen: one line per adapter considered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reasons: Vec<String>,
    mode: SelectionMode,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.mode {
            SelectionMode::Interactive => "no usable GPU: the application runs on a discrete GPU with every required feature and limit",
            SelectionMode::TestSoftware => "no usable software adapter: test mode runs on WARP or lavapipe",
        };
        write!(f, "{what}")?;
        if self.reasons.is_empty() {
            return write!(f, "; the graphics backends reported no adapter at all (check the graphics driver installation)");
        }
        for r in &self.reasons {
            write!(f, "\n  - {r}")?;
        }
        if self.mode == SelectionMode::Interactive && self.reasons.iter().any(|r| r.contains("integrated")) {
            write!(f, "\nOn a hybrid-graphics laptop set this program to High performance (Windows: Settings > System > Display > Graphics) or select the discrete GPU in the vendor's control panel.")?;
        }
        Ok(())
    }
}

impl std::error::Error for Refusal {}

/// Why `a` cannot be used in `mode`; `None` when it can.
pub fn reason_to_refuse(a: &AdapterDesc, req: &Requirements, mode: SelectionMode) -> Option<String> {
    let who = format!("'{}' ({})", a.name, a.backend);
    match mode {
        SelectionMode::Interactive => match a.class {
            DeviceClass::Discrete => {}
            DeviceClass::Software => {
                return Some(format!(
                    "{who} is a software renderer; software adapters run only in test and CI mode (PLANET_ADAPTER=software)"
                ))
            }
            other => {
                return Some(format!("{who} is {}, not a discrete GPU; the application runs on a discrete GPU only", other.describe()))
            }
        },
        SelectionMode::TestSoftware => {
            if a.class != DeviceClass::Software {
                return Some(format!("{who} is {}, not a software renderer (test mode is software only)", a.class.describe()));
            }
        }
    }
    if let Some(f) = req.features.iter().find(|f| !a.features.contains(*f)) {
        return Some(format!("{who} lacks the required feature {f}"));
    }
    for (name, min) in &req.limits {
        match a.limits.get(name) {
            Some(v) if v >= min => {}
            Some(v) => return Some(format!("{who} has the limit {name} = {v}, below the required {min}")),
            None => return Some(format!("{who} does not report the required limit {name}")),
        }
    }
    if a.can_present == Some(false) {
        return Some(format!("{who} cannot present to this window's surface"));
    }
    None
}

/// Rank among the backends: Vulkan first (the backend the goldens of the discrete GPU are keyed to), then DX12, then the rest.
fn backend_rank(backend: &str) -> u8 {
    match backend {
        "Vulkan" => 0,
        "Dx12" => 1,
        _ => 2,
    }
}

/// Choose the adapter to run on: the index into `adapters` of the first acceptable one in a fixed order (backend rank, then
/// name, then position), so that the same machine always gets the same adapter. Refuses with every reason when none is
/// acceptable.
pub fn select(adapters: &[AdapterDesc], req: &Requirements, mode: SelectionMode) -> Result<usize, Refusal> {
    let mut order: Vec<usize> = (0..adapters.len()).collect();
    order.sort_by(|&i, &j| {
        let (a, b) = (&adapters[i], &adapters[j]);
        (backend_rank(&a.backend), &a.name, i).cmp(&(backend_rank(&b.backend), &b.name, j))
    });
    let mut reasons = Vec::new();
    for i in order {
        match reason_to_refuse(&adapters[i], req, mode) {
            None => return Ok(i),
            Some(r) => reasons.push(r),
        }
    }
    Err(Refusal { reasons, mode })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(name: &str, backend: &str, class: DeviceClass) -> AdapterDesc {
        AdapterDesc {
            name: name.into(),
            backend: backend.into(),
            class,
            features: ["TIMESTAMP_QUERY".to_string(), "INDIRECT_FIRST_INSTANCE".to_string()].into(),
            limits: [("max_buffer_size".to_string(), 1u64 << 32), ("max_texture_dimension_2d".to_string(), 16384)].into(),
            can_present: None,
        }
    }

    fn req() -> Requirements {
        Requirements {
            features: vec!["INDIRECT_FIRST_INSTANCE".into()],
            limits: vec![("max_buffer_size".into(), 1 << 28), ("max_texture_dimension_2d".into(), 8192)],
        }
    }

    // spec: VIEW-001
    #[test]
    fn a_discrete_gpu_is_accepted() {
        let a = [desc("NVIDIA GeForce RTX 3080 Ti Laptop GPU", "Vulkan", DeviceClass::Discrete)];
        assert_eq!(select(&a, &req(), SelectionMode::Interactive), Ok(0));
    }

    // spec: VIEW-001
    #[test]
    fn an_integrated_gpu_is_refused_with_the_reason_and_the_way_out() {
        let a = [desc("Intel(R) UHD Graphics", "Vulkan", DeviceClass::Integrated)];
        let e = select(&a, &req(), SelectionMode::Interactive).unwrap_err();
        let text = e.to_string();
        assert!(text.contains("'Intel(R) UHD Graphics' (Vulkan) is an integrated GPU, not a discrete GPU"), "{text}");
        assert!(text.contains("High performance"), "{text}");
    }

    // spec: VIEW-001
    #[test]
    fn a_missing_feature_or_limit_is_refused_by_name() {
        let mut no_feature = desc("Big GPU", "Vulkan", DeviceClass::Discrete);
        no_feature.features.remove("INDIRECT_FIRST_INSTANCE");
        let e = select(&[no_feature], &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("lacks the required feature INDIRECT_FIRST_INSTANCE"), "{e}");

        let mut small = desc("Small GPU", "Dx12", DeviceClass::Discrete);
        small.limits.insert("max_texture_dimension_2d".into(), 4096);
        let e = select(&[small], &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("limit max_texture_dimension_2d = 4096, below the required 8192"), "{e}");

        let mut unknown = desc("Odd GPU", "Dx12", DeviceClass::Discrete);
        unknown.limits.remove("max_buffer_size");
        let e = select(&[unknown], &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("does not report the required limit max_buffer_size"), "{e}");
    }

    // spec: VIEW-001
    #[test]
    fn a_gpu_that_cannot_present_is_refused() {
        let mut a = desc("GPU", "Vulkan", DeviceClass::Discrete);
        a.can_present = Some(false);
        let e = select(&[a], &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("cannot present to this window's surface"), "{e}");
    }

    // spec: VIEW-001
    #[test]
    fn a_software_adapter_runs_only_in_test_mode() {
        let warp = desc("Microsoft Basic Render Driver", "Dx12", DeviceClass::Software);
        let e = select(std::slice::from_ref(&warp), &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("is a software renderer; software adapters run only in test and CI mode"), "{e}");
        assert_eq!(select(std::slice::from_ref(&warp), &req(), SelectionMode::TestSoftware), Ok(0));
        // Test mode is software only: a discrete GPU is not taken for it.
        let gpu = desc("RTX", "Vulkan", DeviceClass::Discrete);
        let e = select(std::slice::from_ref(&gpu), &req(), SelectionMode::TestSoftware).unwrap_err().to_string();
        assert!(e.contains("not a software renderer"), "{e}");
        // Both present: each mode finds its own.
        let both = [gpu, warp];
        assert_eq!(select(&both, &req(), SelectionMode::Interactive), Ok(0));
        assert_eq!(select(&both, &req(), SelectionMode::TestSoftware), Ok(1));
    }

    // spec: VIEW-001
    #[test]
    fn several_adapters_pick_the_discrete_one_deterministically() {
        let igpu = desc("AMD Radeon(TM) Graphics", "Vulkan", DeviceClass::Integrated);
        let dx = desc("NVIDIA GeForce RTX 3080 Ti Laptop GPU", "Dx12", DeviceClass::Discrete);
        let vk = desc("NVIDIA GeForce RTX 3080 Ti Laptop GPU", "Vulkan", DeviceClass::Discrete);
        let second = desc("AMD Radeon RX 7900", "Vulkan", DeviceClass::Discrete);
        let all = vec![igpu, dx.clone(), vk.clone(), second.clone()];
        // The integrated GPU is skipped; among the discrete ones Vulkan wins, then the name.
        let picked = select(&all, &req(), SelectionMode::Interactive).unwrap();
        assert_eq!(all[picked], second);
        // The order the platform lists them in changes nothing.
        let mut shuffled = all.clone();
        shuffled.reverse();
        assert_eq!(shuffled[select(&shuffled, &req(), SelectionMode::Interactive).unwrap()], second);
        // The same name on two backends: Vulkan.
        let pair = [dx, vk.clone()];
        assert_eq!(pair[select(&pair, &req(), SelectionMode::Interactive).unwrap()], vk);
        // A discrete GPU that lacks a requirement is passed over for one that has it.
        let mut weak = desc("AMD Radeon RX 7900", "Vulkan", DeviceClass::Discrete);
        weak.features.clear();
        let list = [weak, vk.clone()];
        assert_eq!(list[select(&list, &req(), SelectionMode::Interactive).unwrap()], vk);
    }

    // spec: VIEW-001
    #[test]
    fn no_adapter_at_all_is_a_message_not_a_crash() {
        let e = select(&[], &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("reported no adapter at all"), "{e}");
    }

    // spec: VIEW-001
    #[test]
    fn every_adapter_turned_down_is_named() {
        let a = [desc("Intel(R) UHD Graphics", "Vulkan", DeviceClass::Integrated), desc("WARP", "Dx12", DeviceClass::Software)];
        let e = select(&a, &req(), SelectionMode::Interactive).unwrap_err().to_string();
        assert!(e.contains("Intel(R) UHD Graphics") && e.contains("WARP"), "{e}");
    }
}
