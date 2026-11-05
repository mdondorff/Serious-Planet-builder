//! The baked channels of the erosion layer read as per-point values (STRM-072, GEN-110, GEN-111; I8a slice 2b): the drainage area and the deposited depth at a direction. The source is a trait so that the
//! GPU-free generators crate can read them while the cache crate implements it over the store (CON-02, as [`LayerSource`](crate::erosion_layer::LayerSource) does for the layer's change).
//!
//! Presentation reads these for the ground materials through the tile's driver block (ground-channels-2b); a CPU consumer of the same values at a point (M9 I7) reads the same source.

use planet_core::Vec3;

/// The channels at a direction. Every read is bilinear in the channel's own grid and returns `Ok(None)` where the store holds no channel for the ground there; an error names what could not be read.
pub trait ChannelSource: Send + Sync {
    /// An identity of the channels (the layer they were made beside, their versions and scales): equal sources give equal numbers, so a field that reads them keys its tiles by it.
    fn identity(&self) -> u64;

    /// The drainage area at `d` as log2 of cells of pass 1's grid.
    fn area_log2_cells(&self, d: Vec3) -> Result<Option<f32>, String>;

    /// The deposited depth at `d` in metres (codes of 1 m, interpolated bilinearly in metres).
    fn deposited_depth_m(&self, d: Vec3) -> Result<Option<f32>, String>;
}
