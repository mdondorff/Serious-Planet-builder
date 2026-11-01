//! `render`: executes a FramePlan and makes no decisions (CON-03). It holds the GPU context and the passes
//! that draw the plan: compact terrain, atmosphere, lit post, water, the window and the egui layer, with the render graph and the GPU timing.

use std::fmt;
use std::sync::{Arc, Mutex};

#[macro_use]
mod horizon_wgsl;
pub mod atmosphere;
pub mod burn;
pub mod compact;
pub mod exec;
pub mod gate;
pub mod gpu_timing;
pub mod graph;
pub mod horizon;
pub use horizon::{probe_horizon, HorizonAnswer, HorizonQuery};
pub mod lit;
#[cfg(feature = "reference-renderer")]
pub mod reference;
pub(crate) mod refs;
pub mod terrain;
pub mod ui;
pub mod water;
pub mod water_gpu;
pub mod window;
pub use atmosphere::{atmosphere_shaders, AtmospherePath, ProbeKind};
pub use burn::{BurnCalibration, BurnPass};
pub use compact::compact_terrain_shader;
pub use exec::{execute, reference_frame, ExecError, ExecStats, TileResource, EXEC_SHADERS};
pub use gate::requirements as adapter_requirements;
pub use gpu_timing::{GpuPass, PassTimes};
pub use lit::{lit_post_shaders, ExposureState, LitPath};
pub use terrain::{
    execute_terrain_tiles, execute_terrain_tiles_preview, execute_terrain_tiles_preview_split, DrawMode, FirstUse, FrameTiming, InitCounts,
    MeasureVariant, TerrainRenderer, TerrainStats, UploadCounters, UploadMode, UploadTimes, RING_SETS, WARM_NODES,
};
pub use ui::{UiFrame, UiPainter};
pub use water::water_shaders;
pub use water_gpu::{probe_surface, probe_waves, SurfaceProbe, WaveProbe};
pub use window::{Presented, WindowHandles, WindowSurface, WINDOW_SHADERS};

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
    // wgpu stays behind render's API (ADR 0004): other crates see names and verdicts, never wgpu types.
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) info: wgpu::AdapterInfo,
    /// Whether the adapter can run indirect draws that carry a first instance (REND-015): the `INDIRECT_FIRST_INSTANCE`
    /// feature and the downlevel `INDIRECT_EXECUTION` flag (`multi_draw_indexed_indirect` itself is core in wgpu 30 and needs
    /// only the latter).
    pub(crate) indirect: bool,
    /// The draw mode new terrain renderers start in: 0 = the default (indirect where supported), 1 = per-draw, 2 = indirect.
    pub(crate) draw_preference: std::sync::atomic::AtomicU8,
    /// How terrain renderers write tiles: 0 = the default (batched), 1 = per slot, 2 = batched.
    pub(crate) upload_preference: std::sync::atomic::AtomicU8,
    /// What the device reported about itself (REND-029).
    diagnostics: Arc<Diagnostics>,
}

impl GpuContext {
    /// Create a headless context. The adapter name is logged by the caller (hybrid-graphics rule, report §8).
    pub fn new(policy: AdapterPolicy) -> Result<Self, GpuError> {
        pollster::block_on(Self::new_async(policy))
    }

    async fn new_async(policy: AdapterPolicy) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = match policy {
            // The test adapters: WARP or lavapipe, the fallback adapter of the platform (test and CI mode, VIEW-001).
            AdapterPolicy::Software => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    force_fallback_adapter: true,
                    compatible_surface: None,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|e| GpuError::NoAdapter(format!("{e} (policy {policy:?})")))?,
            // The discrete GPU, chosen by the adapter gate before any device exists (VIEW-001).
            AdapterPolicy::HighPerformance => gate::pick(&instance, None, planet_frame::adapter::SelectionMode::Interactive).await?.0,
        };
        Self::from_adapter(adapter).await
    }

    /// The context on `adapter`, which the caller has already accepted.
    pub(crate) async fn from_adapter(adapter: wgpu::Adapter) -> Result<Self, GpuError> {
        let info = adapter.get_info();
        let indirect = adapter.features().contains(wgpu::Features::INDIRECT_FIRST_INSTANCE)
            && adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("planet"),
                // Timestamp queries time the GPU in the performance harness; request them only when the adapter has them.
                required_features: adapter.features() & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::INDIRECT_FIRST_INSTANCE),
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::Device(e.to_string()))?;
        let diagnostics = Arc::new(Diagnostics::default());
        install_diagnostics(&device, &diagnostics);
        Ok(Self { device, queue, info, indirect, draw_preference: Default::default(), upload_preference: Default::default(), diagnostics })
    }

    /// The first wgpu error the device reported outside any error scope, with its description (REND-029).
    pub fn first_error(&self) -> Option<String> {
        self.diagnostics.first_error.lock().ok()?.clone()
    }

    /// A buffer with no usage flags, which wgpu refuses: the test of the error handler (REND-029).
    #[doc(hidden)]
    pub fn test_create_invalid_buffer(&self, label: &str) {
        let _ = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: 16,
            usage: wgpu::BufferUsages::empty(),
            mapped_at_creation: false,
        });
    }

    /// Why the device was lost (a driver reset, a GPU timeout), if it was; a deliberate destroy is not a loss (REND-029).
    pub fn device_lost(&self) -> Option<String> {
        self.diagnostics.lost.lock().ok()?.clone()
    }

    /// Adapter name as the driver reports it.
    pub fn adapter_name(&self) -> &str {
        &self.info.name
    }

    /// Whether indirect multi-draw with a first instance is available (REND-015); the terrain renderer falls back to the
    /// per-draw loop where it is not.
    pub fn indirect_draw_supported(&self) -> bool {
        self.indirect
    }

    /// Choose the draw mode terrain renderers created from now on start in (`--draw`); without a preference they start in
    /// indirect mode where the adapter supports it. A renderer can still be switched with `TerrainRenderer::set_draw_mode`.
    pub fn prefer_draw_mode(&self, mode: DrawMode) {
        let v = match mode {
            DrawMode::PerDraw => 1,
            DrawMode::Indirect => 2,
        };
        self.draw_preference.store(v, std::sync::atomic::Ordering::SeqCst);
    }

    /// The mode a new terrain renderer starts in: the preference, else indirect where supported; indirect needs the feature.
    pub fn default_draw_mode(&self) -> DrawMode {
        match self.draw_preference.load(std::sync::atomic::Ordering::SeqCst) {
            1 => DrawMode::PerDraw,
            2 if self.indirect => DrawMode::Indirect,
            2 => DrawMode::PerDraw,
            _ if self.indirect => DrawMode::Indirect,
            _ => DrawMode::PerDraw,
        }
    }

    /// Choose how terrain renderers created from now on write tiles (`--upload`): batched (default, REND-016) or per slot (the
    /// reference; identical pool contents).
    pub fn prefer_upload_mode(&self, mode: UploadMode) {
        let v = match mode {
            UploadMode::PerSlot => 1,
            UploadMode::Batched => 2,
        };
        self.upload_preference.store(v, std::sync::atomic::Ordering::SeqCst);
    }

    /// The upload mode a new terrain renderer starts in.
    pub fn default_upload_mode(&self) -> UploadMode {
        match self.upload_preference.load(std::sync::atomic::Ordering::SeqCst) {
            1 => UploadMode::PerSlot,
            _ => UploadMode::Batched,
        }
    }

    /// Graphics backend, e.g. `Vulkan` or `Dx12`.
    pub fn backend_name(&self) -> String {
        format!("{:?}", self.info.backend)
    }

    /// The first output line of every run (hybrid-graphics rule, report §8): backend, name, type and the golden key.
    pub fn adapter_log_line(&self) -> String {
        format!("adapter: {:?} {} ({:?}), key {}", self.info.backend, self.info.name, self.info.device_type, self.adapter_key())
    }

    /// Validate one WGSL module on this device; `Err` carries the backend's message.
    pub fn validate_wgsl(&self, name: &str, source: &str) -> Result<(), String> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(name), source: wgpu::ShaderSource::Wgsl(source.into()) });
        match pollster::block_on(scope.pop()) {
            Some(err) => Err(format!("shader '{name}' failed validation: {err}")),
            None => Ok(()),
        }
    }

    /// Adapter family as a directory-safe key, e.g. `dx12-microsoft-basic-render-driver` or `vulkan-llvmpipe`:
    /// parenthesised details (LLVM version, bit width) are dropped so driver updates do not orphan the goldens
    /// (same rule as `planet_testkit::adapter_key`).
    pub fn adapter_key(&self) -> String {
        family_key(&format!("{:?}", self.info.backend), &self.info.name)
    }

    /// Everything that identifies the driver: name, backend, device type, driver and driver info. Recorded beside the
    /// goldens and printed on every mismatch.
    pub fn driver_string(&self) -> String {
        format!(
            "{} | {:?} | {:?} | driver '{}' '{}'",
            self.info.name, self.info.backend, self.info.device_type, self.info.driver, self.info.driver_info
        )
    }
}

/// Directory-safe family key of an adapter: parenthesised details are dropped (`planet_testkit::adapter_key` applies the
/// same rule; both are pinned by tests against the same table).
fn family_key(backend: &str, name: &str) -> String {
    let mut family = String::new();
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => family.push(c),
            _ => {}
        }
    }
    let mut key = String::new();
    for c in format!("{backend}-{family}").to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c);
        } else if !key.ends_with('-') {
            key.push('-');
        }
    }
    key.trim_matches('-').to_string()
}

#[cfg(test)]
mod key_tests {
    use super::family_key;

    // spec: TEST-002
    #[test]
    fn the_family_key_ignores_driver_and_compiler_versions() {
        assert_eq!(family_key("Dx12", "Microsoft Basic Render Driver"), "dx12-microsoft-basic-render-driver");
        assert_eq!(family_key("Vulkan", "llvmpipe (LLVM 15.0.7, 256 bits)"), "vulkan-llvmpipe");
        assert_eq!(family_key("Vulkan", "llvmpipe (LLVM 20.1.2, 256 bits)"), family_key("Vulkan", "llvmpipe (LLVM 21.0.0, 128 bits)"));
        assert_eq!(family_key("Vulkan", "NVIDIA GeForce RTX 3080 Ti Laptop GPU"), "vulkan-nvidia-geforce-rtx-3080-ti-laptop-gpu");
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
pub(crate) fn read_rgba8(ctx: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> RgbaFrame {
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
    /// Wait for every submitted work to finish (tools and ad hoc timing).
    pub fn wait_idle(&self) {
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll failed");
    }

    /// Whether GPU timestamp queries are available on this device.
    pub fn timestamps_supported(&self) -> bool {
        self.device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
    }
}

/// What a device reports about itself, kept for the context's readers (REND-029).
#[derive(Default)]
struct Diagnostics {
    first_error: Mutex<Option<String>>,
    lost: Mutex<Option<String>>,
}

/// The line for a lost device; `None` for a deliberate destroy, which is not a loss.
fn lost_line(reason: wgpu::DeviceLostReason, message: &str) -> Option<String> {
    (reason != wgpu::DeviceLostReason::Destroyed).then(|| format!("wgpu device lost ({reason:?}): {message}"))
}

/// Say what wgpu would otherwise hide. A lost device surfaces only through a callback and makes every later buffer invalid with no
/// error of its own; an uncaptured error is fatal by default and a later one is easier to see than the first. Both are printed to
/// stderr and kept; the uncaptured error still panics, with wgpu's own message (REND-029).
fn install_diagnostics(device: &wgpu::Device, diagnostics: &Arc<Diagnostics>) {
    let d = Arc::clone(diagnostics);
    device.set_device_lost_callback(move |reason, message| {
        if let Some(line) = lost_line(reason, &message) {
            eprintln!("{line}");
            if let Ok(mut lost) = d.lost.lock() {
                lost.get_or_insert(line);
            }
        }
    });
    let d = Arc::clone(diagnostics);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        let text = error.to_string();
        if let Ok(mut first) = d.first_error.lock() {
            if first.is_none() {
                eprintln!("first wgpu error: {text}");
                *first = Some(text.clone());
            }
        }
        panic!("wgpu error: {text}\n");
    }));
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;

    // spec: REND-029
    #[test]
    fn a_destroy_is_no_loss_and_anything_else_is() {
        assert_eq!(lost_line(wgpu::DeviceLostReason::Destroyed, "x"), None);
        assert!(lost_line(wgpu::DeviceLostReason::Unknown, "TDR").unwrap().contains("TDR"));
    }
}
