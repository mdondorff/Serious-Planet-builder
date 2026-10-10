//! CPU reference for instance culling and indirect-draw arguments (report §8, §16). Instances (vegetation, buildings)
//! are numerous enough that culling will move to the GPU; terrain node selection stays on the CPU. This module is the
//! reference such a GPU pass must match (REND-008). It is not yet part of the FramePlan.

use crate::camera::Frustum;
use planet_core::{PlanetFixed, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub position: PlanetFixed,
    /// Bounding-sphere radius, metres.
    pub radius_m: f32,
}

/// Arguments of one indexed indirect draw (the layout of `DrawIndexedIndirect`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndirectArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

/// Indices of the instances whose bounding sphere touches the frustum, in ascending order.
pub fn cull_instances(frustum: &Frustum, instances: &[Instance]) -> Vec<u32> {
    instances
        .iter()
        .enumerate()
        .filter(|(_, i)| !frustum.culls_sphere(i.position.0, f64::from(i.radius_m)))
        .map(|(k, _)| k as u32)
        .collect()
}

/// Indirect arguments for one batch of `visible` instances drawing a mesh of `index_count` indices.
pub fn indirect_args(index_count: u32, visible: &[u32]) -> IndirectArgs {
    IndirectArgs { index_count, instance_count: visible.len() as u32, first_index: 0, base_vertex: 0, first_instance: 0 }
}

/// Camera-relative f32 positions of the visible instances: what the instance buffer would hold (CON-11).
pub fn camera_relative_positions(camera: PlanetFixed, instances: &[Instance], visible: &[u32]) -> Vec<[f32; 3]> {
    visible
        .iter()
        .map(|&k| {
            let d: Vec3 = instances[k as usize].position.0 - camera.0;
            [d.x as f32, d.y as f32, d.z as f32]
        })
        .collect()
}
