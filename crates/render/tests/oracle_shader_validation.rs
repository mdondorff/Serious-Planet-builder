//! Tier B: every shipped WGSL module passes the backend's validation on the software adapter.

use planet_render::{compact_terrain_shader, lit_post_shaders, AdapterPolicy, GpuContext, EXEC_SHADERS, SHADERS, WINDOW_SHADERS};

// spec: REND-002, ATMO-011, ATMO-016
#[test]
fn every_shader_module_validates() {
    // PLANET_ADAPTER (software by default; `cargo xtask test gpu --real` runs the oracles on the real GPU too, TEST-013).
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    assert!(!SHADERS.is_empty());
    for (name, source) in SHADERS.iter().chain(EXEC_SHADERS).chain(WINDOW_SHADERS) {
        if let Err(e) = ctx.validate_wgsl(name, source) {
            panic!("{e}");
        }
    }
    // The lit path's post chain: the exposure histogram and adaptation (compute) and the tone map (REND-022, REND-023).
    for (name, source) in lit_post_shaders() {
        if let Err(e) = ctx.validate_wgsl(name, &source) {
            panic!("{e}");
        }
    }
    // The atmosphere's passes and the terrain's atmosphere lighting (ATMO-011).
    for (name, source) in planet_render::atmosphere_shaders() {
        if let Err(e) = ctx.validate_wgsl(name, &source) {
            panic!("{e}");
        }
    }
    // The water pass: the pure maths alone and the two tile formats with and without the atmosphere's lighting (ATMO-014, ATMO-016).
    for (name, source) in planet_render::water_shaders() {
        if let Err(e) = ctx.validate_wgsl(name, &source) {
            panic!("{e}");
        }
    }
    // The test-only reference renderer's module (REND-031): the product header and fragment stage around its own vertex stage.
    if let Err(e) = ctx.validate_wgsl("terrain-reference", &planet_render::reference::reference_shader()) {
        panic!("{e}");
    }
    // The compact tile shader (REND-016) is built from the product shader at run time, for the tile size of the renderer.
    for cells in [4, 16, 32, 64] {
        if let Err(e) = ctx.validate_wgsl("terrain-compact", &compact_terrain_shader(cells)) {
            panic!("{e}");
        }
    }
    // The clipmap gather (REND-047) is built at run time like the compact shader, for the tile size of the renderer.
    for cells in [4, 16, 32, 64] {
        if let Err(e) = ctx.validate_wgsl("clipmap-gather", &planet_render::shadow::gather_shader(cells)) {
            panic!("{e}");
        }
    }
    for cells in [4, 16, 32, 64] {
        if let Err(e) = ctx.validate_wgsl("coarse-cube", &planet_render::shadow::cube_shader(cells)) {
            panic!("{e}");
        }
    }
    if let Err(e) = ctx.validate_wgsl("coarse-cube-mip", &planet_render::shadow::cube_mip_shader()) {
        panic!("{e}");
    }
    if let Err(e) = ctx.validate_wgsl("clipmap-mip", &planet_render::shadow::mip_shader()) {
        panic!("{e}");
    }
}
