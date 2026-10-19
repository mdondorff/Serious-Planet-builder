//! The window surface (ADR 0015, VIEW-005): `render` owns the wgpu surface and presents the terrain renderer's colour target to
//! it. The caller hands over a window through `raw-window-handle` (no wgpu type in the call), resizes it, toggles vsync and calls
//! [`WindowSurface::present`] once per frame. The frame is drawn at the renderer's own resolution and scaled into the window
//! keeping its aspect (bars on the sides), so a resize never rebuilds the terrain renderer.
//!
//! Nothing here assumes an adapter or a present mode beyond what the gate allows: the vsync-off request falls back from
//! Immediate to Mailbox to Fifo (Fifo is always there, and the only one over some remote desktops), and the mode in use is
//! reported by name.

use crate::{gate, AdapterPolicy, GpuContext, GpuError, TerrainRenderer};
use planet_frame::adapter::SelectionMode;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::fmt::Debug;
use std::sync::Arc;

/// A window the surface can be made for: any `raw-window-handle` window that is shareable (a `winit` window in an `Arc`).
pub trait WindowHandles: HasWindowHandle + HasDisplayHandle + Debug + Send + Sync + 'static {}
impl<T: HasWindowHandle + HasDisplayHandle + Debug + Send + Sync + 'static> WindowHandles for T {}

/// What a call to [`WindowSurface::present`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presented {
    Yes,
    /// Nothing was shown this time and the next call may succeed: the window is minimized or hidden, the surface was outdated and
    /// has been reconfigured, or the acquisition timed out.
    Skipped(&'static str),
}

const BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    // One triangle covering the viewport.
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: V;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs(v: V) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(src, smp, v.uv).rgb, 1.0);
}
"#;

/// The shaders of this module, so one test validates them with the others.
pub const WINDOW_SHADERS: &[(&str, &str)] = &[("window-blit", BLIT_WGSL)];

pub(crate) struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    format: wgpu::TextureFormat,
}

impl Blit {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("window-blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("window-blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("window-blit-pl"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("window-blit"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("window-blit-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { pipeline, layout, sampler, format }
    }

    /// Draw `src` (of `src_size` pixels) into `target` (of `target_size`), scaled to fit keeping its aspect, bars black.
    pub(crate) fn draw(
        &self,
        ctx: &GpuContext,
        src: &wgpu::TextureView,
        src_size: (u32, u32),
        target: &wgpu::TextureView,
        target_size: (u32, u32),
    ) {
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("window-blit-bind"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let (vx, vy, vw, vh) = fit(src_size, target_size);
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("window-blit") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        ctx.queue.submit([encoder.finish()]);
    }
}

/// The viewport `(x, y, w, h)` that fits a frame of `src` pixels into a target of `dst` pixels keeping its aspect, centred.
pub fn fit(src: (u32, u32), dst: (u32, u32)) -> (f32, f32, f32, f32) {
    let (fw, fh) = (src.0.max(1) as f32, src.1.max(1) as f32);
    let (ww, wh) = (dst.0 as f32, dst.1 as f32);
    let scale = (ww / fw).min(wh / fh);
    let (vw, vh) = ((fw * scale).floor().max(1.0), (fh * scale).floor().max(1.0));
    (((ww - vw) / 2.0).floor(), ((wh - vh) / 2.0).floor(), vw, vh)
}

/// The blit on an offscreen target, for the tier-C test of VIEW-005: `src` scaled into a `target`-sized RGBA8 frame.
pub fn blit_to_image(ctx: &GpuContext, src: &crate::RgbaFrame, target: (u32, u32)) -> crate::RgbaFrame {
    let make = |w: u32, h: u32, usage| {
        ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blit-test"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    };
    let source = make(src.width, src.height, wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
    ctx.queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &source, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &src.rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(src.width * 4), rows_per_image: Some(src.height) },
        wgpu::Extent3d { width: src.width, height: src.height, depth_or_array_layers: 1 },
    );
    let out = make(target.0, target.1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
    let blit = Blit::new(&ctx.device, wgpu::TextureFormat::Rgba8Unorm);
    blit.draw(ctx, &source.create_view(&Default::default()), (src.width, src.height), &out.create_view(&Default::default()), target);
    crate::read_rgba8(ctx, &out, target.0, target.1)
}

/// The presentable surface of a window and what it takes to show the renderer's frame on it.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    modes: Vec<wgpu::PresentMode>,
    vsync: bool,
    blit: Blit,
}

impl GpuContext {
    /// The context and surface for `window` of `size` pixels. The adapter is chosen by the gate before any device exists
    /// (VIEW-001): with [`AdapterPolicy::HighPerformance`] a discrete GPU that can present to this window, with
    /// [`AdapterPolicy::Software`] (the test and CI mode only) a software adapter. A refusal is an error naming the reason for
    /// every adapter turned down.
    pub fn for_window<W: WindowHandles>(
        window: Arc<W>,
        size: (u32, u32),
        policy: AdapterPolicy,
        vsync: bool,
    ) -> Result<(GpuContext, WindowSurface), GpuError> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(Box::new(Arc::clone(&window))));
            let surface = instance.create_surface(window).map_err(|e| GpuError::Device(format!("creating the window surface: {e}")))?;
            let mode = match policy {
                AdapterPolicy::HighPerformance => SelectionMode::Interactive,
                AdapterPolicy::Software => SelectionMode::TestSoftware,
            };
            let (adapter, _) = gate::pick(&instance, Some(&surface), mode).await?;
            let caps = surface.get_capabilities(&adapter);
            let mut config = surface
                .get_default_config(&adapter, size.0.max(1), size.1.max(1))
                .ok_or_else(|| GpuError::Device("the surface reports no usable configuration for this adapter".into()))?;
            // The renderer's colour is display-encoded already: a non-sRGB surface format shows it as it is.
            if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
                config.format = *f;
            }
            config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
            let ctx = GpuContext::from_adapter(adapter).await?;
            let blit = Blit::new(&ctx.device, config.format);
            let mut s = WindowSurface { surface, config, modes: caps.present_modes, vsync, blit };
            s.apply_mode();
            s.surface.configure(&ctx.device, &s.config);
            Ok((ctx, s))
        })
    }
}

impl WindowSurface {
    /// The present mode for the vsync wish among those the surface offers.
    fn apply_mode(&mut self) {
        let has = |m: wgpu::PresentMode| self.modes.contains(&m);
        self.config.present_mode = if self.vsync {
            wgpu::PresentMode::Fifo
        } else if has(wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if has(wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
    }

    /// The present mode in use, by name (`Fifo`, `Immediate`, `Mailbox`).
    pub fn present_mode_name(&self) -> String {
        format!("{:?}", self.config.present_mode)
    }

    /// Whether the surface can do what the vsync wish asks: vsync on is always honoured, vsync off needs a mode that does not wait.
    pub fn vsync_honoured(&self) -> bool {
        self.vsync || self.config.present_mode != wgpu::PresentMode::Fifo
    }

    pub fn set_vsync(&mut self, ctx: &GpuContext, on: bool) {
        self.vsync = on;
        self.apply_mode();
        self.surface.configure(&ctx.device, &self.config);
    }

    /// The window changed size; a size of zero (minimized) is ignored until the window is shown again.
    pub fn resize(&mut self, ctx: &GpuContext, size: (u32, u32)) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        self.config.width = size.0;
        self.config.height = size.1;
        self.surface.configure(&ctx.device, &self.config);
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// A painter for this surface's format (VIEW-011).
    pub fn ui_painter(&self, ctx: &GpuContext) -> crate::ui::UiPainter {
        crate::ui::UiPainter::for_format(ctx, self.blit.format)
    }

    /// Show the renderer's colour target (the last frame it submitted) in the window.
    pub fn present(&mut self, ctx: &GpuContext, renderer: &TerrainRenderer) -> Result<Presented, GpuError> {
        self.present_with_ui(ctx, renderer, None)
    }

    /// [`WindowSurface::present`] with a UI pass after the blit: the scene is in the swapchain image, the UI is painted over it, then
    /// the frame is presented (VIEW-011, ADR 0015). Without a UI the frame is the blit alone, bit for bit as before.
    pub fn present_with_ui(
        &mut self,
        ctx: &GpuContext,
        renderer: &TerrainRenderer,
        ui: Option<(&mut crate::ui::UiPainter, &mut crate::ui::UiFrame)>,
    ) -> Result<Presented, GpuError> {
        use wgpu::CurrentSurfaceTexture as C;
        let frame = match self.surface.get_current_texture() {
            C::Success(f) => f,
            C::Suboptimal(f) => {
                // Shown this time, reconfigured for the next.
                self.surface.configure(&ctx.device, &self.config);
                f
            }
            C::Timeout => return Ok(Presented::Skipped("surface timeout")),
            C::Occluded => return Ok(Presented::Skipped("window occluded")),
            C::Outdated => {
                self.surface.configure(&ctx.device, &self.config);
                return Ok(Presented::Skipped("surface outdated, reconfigured"));
            }
            C::Lost => {
                // Reconfiguring the same surface recovers a lost swap chain on the backends that have one; a lost device is not.
                self.surface.configure(&ctx.device, &self.config);
                return Ok(Presented::Skipped("surface lost, reconfigured"));
            }
            C::Validation => return Err(GpuError::Device("the surface reported a validation error while acquiring a frame".into())),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(self.blit.format), ..Default::default() });
        self.blit.draw(ctx, renderer.color_view(), renderer.target_size(), &view, (self.config.width, self.config.height));
        if let Some((painter, frame_ui)) = ui {
            painter.paint(ctx, &view, (self.config.width, self.config.height), frame_ui);
        }
        ctx.queue.present(frame);
        Ok(Presented::Yes)
    }
}

#[cfg(test)]
mod tests {
    use super::fit;

    // spec: VIEW-006
    #[test]
    fn the_frame_fits_the_window_keeping_its_aspect() {
        // Same aspect: the whole window.
        assert_eq!(fit((1280, 720), (1920, 1080)), (0.0, 0.0, 1920.0, 1080.0));
        // A taller window: bars above and below.
        assert_eq!(fit((1280, 720), (1280, 1000)), (0.0, 140.0, 1280.0, 720.0));
        // A wider window: bars at the sides.
        assert_eq!(fit((1280, 720), (2000, 720)), (360.0, 0.0, 1280.0, 720.0));
        // A tiny window never gets an empty viewport.
        let (_, _, w, h) = fit((1280, 720), (1, 1));
        assert!(w >= 1.0 && h >= 1.0);
    }
}
