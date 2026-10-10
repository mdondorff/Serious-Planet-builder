//! `render`: executes a FramePlan and makes no decisions (CON-03). M0 holds only the headless GPU
//! context and the "hello triangle" used to prove the golden harness on software adapters.

use std::fmt;

pub use wgpu;

pub mod exec;
pub mod graph;
pub mod terrain;
pub use exec::{execute, reference_frame, ExecError, ExecStats, TileResource, EXEC_SHADERS};
pub use terrain::{execute_terrain, FrameTiming, TerrainRenderer, TerrainStats, TERRAIN_SHADERS};

/// Which adapter the process wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterPolicy {
    /// WARP on Windows, lavapipe/llvmpipe on Linux: what CI and `cargo xtask test` use.
    Software,
    /// The discrete GPU (performance runs, local real-GPU goldens).
    HighPerformance,
}

impl AdapterPolicy {
    /// Read `PLANET_ADAPTER=software|hardware`; default `software` so that tests are reproducible.
    pub fn from_env() -> Self {
        match std::env::var("PLANET_ADAPTER").as_deref() {
            Ok("hardware") | Ok("high-performance") => AdapterPolicy::HighPerformance,
            _ => AdapterPolicy::Software,
        }
    }
}

#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    Device(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::NoAdapter(m) => write!(f, "no suitable GPU adapter: {m}"),
            GpuError::Device(m) => write!(f, "device creation failed: {m}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Tightly packed RGBA8 pixels read back from the GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

impl GpuContext {
    /// Create a headless context. The adapter name is logged by the caller (hybrid-graphics rule, report §8).
    pub fn new(policy: AdapterPolicy) -> Result<Self, GpuError> {
        pollster::block_on(Self::new_async(policy))
    }

    async fn new_async(policy: AdapterPolicy) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: match policy {
                    AdapterPolicy::Software => wgpu::PowerPreference::None,
                    AdapterPolicy::HighPerformance => wgpu::PowerPreference::HighPerformance,
                },
                force_fallback_adapter: policy == AdapterPolicy::Software,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| GpuError::NoAdapter(format!("{e} (policy {policy:?})")))?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("planet"),
                // Timestamp queries time the GPU in the performance harness; request them only when the adapter has them.
                required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::Device(e.to_string()))?;
        Ok(Self { device, queue, info })
    }

    /// Backend + name as a directory-safe key, e.g. `dx12-microsoft-basic-render-driver`.
    pub fn adapter_key(&self) -> String {
        let backend = format!("{:?}", self.info.backend);
        let mut key = String::new();
        for c in format!("{backend}-{}", self.info.name).to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                key.push(c);
            } else if !key.ends_with('-') {
                key.push('-');
            }
        }
        key.trim_matches('-').to_string()
    }
}

/// Every WGSL module the renderer ships, by name, so one test can validate all of them.
pub const SHADERS: &[(&str, &str)] = &[("hello-triangle", TRIANGLE_WGSL)];

const TRIANGLE_WGSL: &str = r#"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(0.0, 0.75), vec2<f32>(-0.75, -0.75), vec2<f32>(0.75, -0.75));
    return vec4<f32>(p[i], 0.0, 1.0);
}

@fragment
fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 128.0 / 255.0, 0.0, 1.0);
}
"#;

/// Clear colour and triangle colour of [`hello_triangle`], as RGBA8.
pub const HELLO_CLEAR: [u8; 4] = [0, 0, 64, 255];
pub const HELLO_FILL: [u8; 4] = [255, 128, 0, 255];

/// Render one flat-coloured triangle into an offscreen `size`x`size` RGBA8 target and read it back.
pub fn hello_triangle(ctx: &GpuContext, size: u32) -> RgbaFrame {
    let device = &ctx.device;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hello-target"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("hello-triangle"),
        source: wgpu::ShaderSource::Wgsl(TRIANGLE_WGSL.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("hello-pipeline"),
        layout: None,
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("hello") });
    {
        let [r, g, b, a] = HELLO_CLEAR.map(|v| f64::from(v) / 255.0);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hello-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    ctx.queue.submit([encoder.finish()]);
    read_rgba8(ctx, &target, size, size)
}

/// Adapter class, so layers above `render` need no wgpu types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterKind {
    Discrete,
    Integrated,
    Software,
    Other,
}

impl GpuContext {
    pub fn kind(&self) -> AdapterKind {
        match self.info.device_type {
            wgpu::DeviceType::DiscreteGpu => AdapterKind::Discrete,
            wgpu::DeviceType::IntegratedGpu => AdapterKind::Integrated,
            wgpu::DeviceType::Cpu => AdapterKind::Software,
            _ => AdapterKind::Other,
        }
    }
}

impl RgbaFrame {
    pub fn write_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().map_err(std::io::Error::other)?.write_image_data(&self.rgba).map_err(std::io::Error::other)
    }
}

/// Copy an `Rgba8Unorm` texture (usage `COPY_SRC`) to the CPU as tightly packed pixels.
pub fn read_rgba8(ctx: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> RgbaFrame {
    let unpadded = width * 4;
    let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(height) },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    ctx.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map failed"));
    ctx.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    let data = slice.get_mapped_range().expect("readback range");
    let mut rgba = Vec::with_capacity((unpadded * height) as usize);
    for row in data.chunks_exact(padded as usize) {
        rgba.extend_from_slice(&row[..unpadded as usize]);
    }
    drop(data);
    readback.unmap();
    RgbaFrame { width, height, rgba }
}

impl GpuContext {
    /// Whether GPU timestamp queries are available on this device.
    pub fn timestamps_supported(&self) -> bool {
        self.device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
    }
}
