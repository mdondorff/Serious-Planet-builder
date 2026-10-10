//! Tier B: every shipped WGSL module passes the backend's validation on the software adapter.

use planet_render::{AdapterPolicy, GpuContext, EXEC_SHADERS, SHADERS, TERRAIN_SHADERS};

// spec: REND-002
#[test]
fn every_shader_module_validates() {
    let ctx = GpuContext::new(AdapterPolicy::Software).expect("software adapter");
    assert!(!SHADERS.is_empty());
    for (name, source) in SHADERS.iter().chain(EXEC_SHADERS).chain(TERRAIN_SHADERS) {
        if let Err(e) = ctx.validate_wgsl(name, source) {
            panic!("{e}");
        }
    }
}
