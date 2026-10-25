//! The horizon shadows' GPU resources (REND-038, M8-5 part b): the chunk-horizon texture array, the coarse map, the sampler and the per-node
//! table, all behind the terrain pass's bind group (bindings 4 to 7). The frame decides what they hold (`planet_frame::horizonview`, CON-03);
//! this only stores and binds it. Until the first [`HorizonGpu::set_capacity`] with a capacity both textures are 1 x 1 dummies, so the group is
//! always complete, and a table of slot `-1` everywhere is the identity of the shadow term.

use planet_frame::horizonview::{HorizonFrame, HORIZON_TEXELS, LAYERS_PER_SLOT, NODE_TABLE_FLOATS, SLOT_BYTES};

use crate::GpuContext;

/// Bytes of one node's table entry (two `vec4`).
pub(crate) const TABLE_ENTRY_BYTES: u64 = 4 * NODE_TABLE_FLOATS as u64;

/// The format of both maps (`pack_map`'s layout: four azimuths a texel).
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The bind group layout entries of bindings 4 to 7 (the table is a storage buffer; the maps and the sampler are read by the fragment stage).
pub(crate) fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 4] {
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    };
    [
        wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        texture(5),
        texture(6),
        wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

/// The table of `nodes` nodes with no map: slot `-1` in the first lane of each entry, the rest zero.
pub(crate) fn empty_table(nodes: usize) -> Vec<u8> {
    let mut out = vec![0u8; nodes * TABLE_ENTRY_BYTES as usize];
    for n in 0..nodes {
        let at = n * TABLE_ENTRY_BYTES as usize;
        out[at..at + 4].copy_from_slice(&(-1.0f32).to_le_bytes());
    }
    out
}

/// The table of `frame` for a ring set of `capacity` nodes: its entries, the rest slot `-1`.
pub(crate) fn table_bytes(frame: Option<&HorizonFrame>, capacity: usize) -> Vec<u8> {
    let mut out = empty_table(capacity);
    if let Some(f) = frame {
        for (i, v) in f.table().iter().take(capacity * NODE_TABLE_FLOATS).enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    out
}

pub(crate) struct HorizonGpu {
    chunk: wgpu::Texture,
    pub(crate) chunk_view: wgpu::TextureView,
    #[allow(dead_code)] // the coarse map's upload comes with the shader term (part c)
    coarse: wgpu::Texture,
    pub(crate) coarse_view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    /// Slots the chunk array holds (0: the dummy).
    capacity: u32,
}

fn make_array(device: &wgpu::Device, label: &str, size: u32, layers: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view =
        texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
    (texture, view)
}

impl HorizonGpu {
    pub(crate) fn new(device: &wgpu::Device) -> HorizonGpu {
        let (chunk, chunk_view) = make_array(device, "horizon-chunks-dummy", 1, 1);
        let (coarse, coarse_view) = make_array(device, "horizon-coarse-dummy", 1, 1);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("horizon-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        HorizonGpu { chunk, chunk_view, coarse, coarse_view, sampler, capacity: 0 }
    }

    /// Bytes of the chunk array (0 while it is the dummy).
    pub(crate) fn bytes(&self) -> u64 {
        u64::from(self.capacity) * SLOT_BYTES as u64
    }

    /// Make the chunk array hold `capacity` slots (a change of capacity makes a new, empty array). Returns true when the texture was replaced,
    /// so the bind groups have to be made again.
    pub(crate) fn set_capacity(&mut self, device: &wgpu::Device, capacity: u32) -> bool {
        if capacity == self.capacity {
            return false;
        }
        let (texture, view) = if capacity == 0 {
            make_array(device, "horizon-chunks-dummy", 1, 1)
        } else {
            make_array(device, "horizon-chunks", HORIZON_TEXELS as u32, capacity * LAYERS_PER_SLOT as u32)
        };
        self.chunk = texture;
        self.chunk_view = view;
        self.capacity = capacity;
        true
    }

    /// Write one slot's packed texels (`pack_map`: `SLOT_BYTES` bytes, `LAYERS_PER_SLOT` layers one after another).
    pub(crate) fn write_slot(&self, ctx: &GpuContext, slot: u32, packed: &[u8]) {
        assert!(slot < self.capacity, "slot {slot} of {} (set the capacity first)", self.capacity);
        assert_eq!(packed.len(), SLOT_BYTES, "a slot holds {SLOT_BYTES} B");
        let h = HORIZON_TEXELS as u32;
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.chunk,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: slot * LAYERS_PER_SLOT as u32 },
                aspect: wgpu::TextureAspect::All,
            },
            packed,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * h), rows_per_image: Some(h) },
            wgpu::Extent3d { width: h, height: h, depth_or_array_layers: LAYERS_PER_SLOT as u32 },
        );
    }

    /// Read the texels of a slot back (tests).
    pub(crate) fn read_slot(&self, ctx: &GpuContext, slot: u32) -> Vec<u8> {
        let h = HORIZON_TEXELS as u32;
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("horizon-readback"),
            size: SLOT_BYTES as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = ctx.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.chunk,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: slot * LAYERS_PER_SLOT as u32 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * h), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: h, height: h, depth_or_array_layers: LAYERS_PER_SLOT as u32 },
        );
        ctx.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("range").to_vec();
        buffer.unmap();
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REND-038
    #[test]
    fn an_empty_table_is_slot_minus_one_in_every_entry_and_zero_elsewhere() {
        let t = empty_table(3);
        assert_eq!(t.len(), 3 * TABLE_ENTRY_BYTES as usize);
        for n in 0..3 {
            let at = n * TABLE_ENTRY_BYTES as usize;
            assert_eq!(f32::from_le_bytes(t[at..at + 4].try_into().unwrap()), -1.0);
            assert!(t[at + 4..at + TABLE_ENTRY_BYTES as usize].iter().all(|&b| b == 0));
        }
    }

    // spec: REND-038
    #[test]
    fn a_frames_table_fills_the_entries_it_has_and_pads_the_rest_with_slot_minus_one() {
        use planet_frame::horizonview::{HorizonNode, SlotUpdate};
        let frame = HorizonFrame {
            capacity: 2,
            nodes: vec![HorizonNode { slot: 1, sun_azimuth: 0.5, sun_elevation: 0.25, fade: 1.0, rect: [0.0, 0.5, 0.5, 0.5] }],
            update: SlotUpdate::default(),
        };
        let t = table_bytes(Some(&frame), 2);
        let f = |i: usize| f32::from_le_bytes(t[4 * i..4 * i + 4].try_into().unwrap());
        assert_eq!([f(0), f(1), f(2), f(3)], [1.0, 0.5, 0.25, 1.0]);
        assert_eq!([f(4), f(5), f(6), f(7)], [0.0, 0.5, 0.5, 0.5]);
        assert_eq!(f(8), -1.0);
        assert_eq!(table_bytes(None, 2), empty_table(2));
    }
}
