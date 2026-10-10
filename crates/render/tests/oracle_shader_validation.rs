//! Tier B: every shipped WGSL module passes the backend's validation on the software adapter.

use planet_render::{wgpu, AdapterPolicy, GpuContext, EXEC_SHADERS, SHADERS, TERRAIN_SHADERS};

// spec: REND-002
#[test]
fn every_shader_module_validates() {
    let ctx = GpuContext::new(AdapterPolicy::Software).expect("software adapter");
    assert!(!SHADERS.is_empty());
    for (name, source) in SHADERS.iter().chain(EXEC_SHADERS).chain(TERRAIN_SHADERS) {
        let scope = ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(name), source: wgpu::ShaderSource::Wgsl((*source).into()) });
        if let Some(err) = pollster::block_on(scope.pop()) {
            panic!("shader '{name}' failed validation: {err}");
        }
    }
}
