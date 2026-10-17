//! The adapter gate on the wgpu side (VIEW-001): enumerate the adapters, describe each one as plain data and let the GPU-free
//! selection in `planet_frame::adapter` choose, before any device is created. The renderer's requirements are stated here once:
//! the device is requested with the same limits the gate checks.

use crate::GpuError;
use planet_frame::adapter::{select, AdapterDesc, DeviceClass, Requirements, SelectionMode};

/// The limits the renderer needs, by name: the pools, the compact blocks (storage-buffer bindings of up to 128 MiB, ADR 0054) and the bind groups of the terrain path (wgpu's default limits,
/// which is what the device asks for). An adapter below any of them is refused by name.
fn limit_table(l: &wgpu::Limits) -> [(&'static str, u64); 6] {
    [
        ("max_texture_dimension_2d", u64::from(l.max_texture_dimension_2d)),
        ("max_buffer_size", l.max_buffer_size),
        ("max_storage_buffer_binding_size", l.max_storage_buffer_binding_size),
        ("max_uniform_buffer_binding_size", l.max_uniform_buffer_binding_size),
        ("max_bind_groups", u64::from(l.max_bind_groups)),
        ("max_storage_buffers_per_shader_stage", u64::from(l.max_storage_buffers_per_shader_stage)),
    ]
}

/// The features every adapter must have: none today. The two the renderer uses (`TIMESTAMP_QUERY`, `INDIRECT_FIRST_INSTANCE`)
/// have fallbacks (no GPU time; the per-draw loop), so they are asked for when present and are not required.
const REQUIRED_FEATURES: [(&str, wgpu::Features); 0] = [];

/// What the renderer requires of an adapter.
pub fn requirements() -> Requirements {
    Requirements {
        features: REQUIRED_FEATURES.iter().map(|(n, _)| (*n).to_string()).collect(),
        limits: limit_table(&wgpu::Limits::default()).iter().map(|(n, v)| ((*n).to_string(), *v)).collect(),
    }
}

fn class_of(t: wgpu::DeviceType) -> DeviceClass {
    match t {
        wgpu::DeviceType::DiscreteGpu => DeviceClass::Discrete,
        wgpu::DeviceType::IntegratedGpu => DeviceClass::Integrated,
        wgpu::DeviceType::VirtualGpu => DeviceClass::Virtual,
        wgpu::DeviceType::Cpu => DeviceClass::Software,
        wgpu::DeviceType::Other => DeviceClass::Other,
    }
}

/// An adapter as the gate sees it; `can_present` is `None` without a surface.
pub(crate) fn describe(adapter: &wgpu::Adapter, can_present: Option<bool>) -> AdapterDesc {
    let info = adapter.get_info();
    let features = adapter.features();
    let mut names: std::collections::BTreeSet<String> =
        REQUIRED_FEATURES.iter().filter(|(_, f)| features.contains(*f)).map(|(n, _)| (*n).to_string()).collect();
    // The optional features are listed too, so a log of the description shows what the adapter offers.
    for (name, f) in
        [("TIMESTAMP_QUERY", wgpu::Features::TIMESTAMP_QUERY), ("INDIRECT_FIRST_INSTANCE", wgpu::Features::INDIRECT_FIRST_INSTANCE)]
    {
        if features.contains(f) {
            names.insert(name.to_string());
        }
    }
    AdapterDesc {
        name: info.name,
        backend: format!("{:?}", info.backend),
        class: class_of(info.device_type),
        features: names,
        limits: limit_table(&adapter.limits()).iter().map(|(n, v)| ((*n).to_string(), *v)).collect(),
        can_present,
    }
}

/// Enumerate, describe and select. Returns the chosen adapter and its description; a refusal becomes a `GpuError::NoAdapter`
/// whose message names the reason for every adapter turned down.
pub(crate) async fn pick(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
    mode: SelectionMode,
) -> Result<(wgpu::Adapter, AdapterDesc), GpuError> {
    let adapters = instance.enumerate_adapters(wgpu::Backends::all()).await;
    let descs: Vec<AdapterDesc> = adapters.iter().map(|a| describe(a, surface.map(|s| a.is_surface_supported(s)))).collect();
    let i = select(&descs, &requirements(), mode).map_err(|r| GpuError::NoAdapter(r.to_string()))?;
    Ok((adapters.into_iter().nth(i).expect("selected index"), descs.into_iter().nth(i).expect("selected index")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: VIEW-001
    #[test]
    fn the_requirements_are_the_limits_the_device_is_requested_with() {
        let r = requirements();
        let defaults = wgpu::Limits::default();
        for (name, min) in &r.limits {
            let v = limit_table(&defaults).iter().find(|(n, _)| n == name).expect("named").1;
            assert_eq!(*min, v, "{name}");
        }
        assert!(r.limits.iter().any(|(n, _)| n == "max_texture_dimension_2d"));
        assert!(r.features.is_empty(), "no feature is required today: TIMESTAMP_QUERY and INDIRECT_FIRST_INSTANCE have fallbacks");
    }

    // spec: VIEW-001, REND-016
    #[test]
    fn the_gate_requires_the_binding_size_of_a_compact_block_and_the_storage_buffers_of_a_stage_by_name() {
        let r = requirements();
        let min = |name: &str| r.limits.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("{name} is required")).1;
        // A block of the compact pool is bound whole (ADR 0054): the adapter must offer a binding of that size.
        assert!(min("max_storage_buffer_binding_size") >= planet_frame::pool::COMPACT_BLOCK_BYTES);
        assert!(min("max_buffer_size") >= planet_frame::pool::COMPACT_BLOCK_BYTES);
        // The compact vertex stage reads four storage buffers (nodes, records, tile references, one block); the water and lit
        // fragment stages fewer.
        assert!(min("max_storage_buffers_per_shader_stage") >= 4);
        assert!(min("max_bind_groups") >= 4, "frame, records and references, block, atmosphere");
    }
}
