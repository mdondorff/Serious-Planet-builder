//! Tier B: every shipped WGSL module passes the backend's validation on the software adapter.

use planet_render::{AdapterPolicy, GpuContext, EXEC_SHADERS, SHADERS, TERRAIN_SHADERS};

// spec: REND-002
#[test]
fn every_shader_module_validates() {
    // PLANET_ADAPTER (software by default; `cargo xtask test gpu --real` runs the oracles on the real GPU too, TEST-013).
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    assert!(!SHADERS.is_empty());
    for (name, source) in SHADERS.iter().chain(EXEC_SHADERS).chain(TERRAIN_SHADERS) {
        if let Err(e) = ctx.validate_wgsl(name, source) {
            panic!("{e}");
        }
    }
}
