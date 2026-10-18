//! SPIKE-14 ONLY (never merged): the egui painter inside `render` (ADR 0015). The editor produces GPU-free egui output (a
//! [`UiFrame`]); render paints it after the tone mapping, into the LDR frame that goes to the window, and the HDR targets stay
//! untouched. No wgpu type crosses the API.

use crate::window::Blit;
use crate::{GpuContext, RgbaFrame};

/// What egui produced for one frame: the tessellated shapes and the texture changes (GPU-free data).
pub struct UiFrame {
    pub primitives: Vec<egui::ClippedPrimitive>,
    pub textures_delta: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

/// Paints [`UiFrame`]s with `egui-wgpu` into a target of one colour format.
pub struct UiPainter {
    renderer: egui_wgpu::Renderer,
}

impl UiPainter {
    pub(crate) fn for_format(ctx: &GpuContext, format: wgpu::TextureFormat) -> UiPainter {
        UiPainter { renderer: egui_wgpu::Renderer::new(&ctx.device, format, egui_wgpu::RendererOptions::default()) }
    }

    /// A painter for the offscreen compose path (`Rgba8Unorm`, the format of the headless window blit).
    pub fn offscreen(ctx: &GpuContext) -> UiPainter {
        Self::for_format(ctx, wgpu::TextureFormat::Rgba8Unorm)
    }

    /// Paint `ui` over what `target` holds (the load is `Load`: the scene stays, the UI blends over it).
    pub(crate) fn paint(&mut self, ctx: &GpuContext, target: &wgpu::TextureView, size_px: (u32, u32), ui: &UiFrame) {
        let device = &ctx.device;
        for (id, deltas) in &ui.textures_delta.set {
            for delta in deltas.iter() {
                self.renderer.update_texture(device, &ctx.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [size_px.0, size_px.1], pixels_per_point: ui.pixels_per_point };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ui") });
        let extra = self.renderer.update_buffers(device, &ctx.queue, &mut encoder, &ui.primitives, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &ui.primitives, &screen);
        }
        ctx.queue.submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
        for id in &ui.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    /// The window path without a window: `src` fitted into a `target`-sized frame (the blit of the window), then `ui` over it; read
    /// back. `ui` may be `None` (the blit alone). Used by tests, images and the cost measurement.
    pub fn compose(&mut self, ctx: &GpuContext, src: &RgbaFrame, target: (u32, u32), ui: Option<&UiFrame>) -> RgbaFrame {
        let make = |w: u32, h: u32, usage: wgpu::TextureUsages| {
            ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("ui-compose"),
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
        let out_view = out.create_view(&Default::default());
        blit.draw(ctx, &source.create_view(&Default::default()), (src.width, src.height), &out_view, target);
        if let Some(ui) = ui {
            self.paint(ctx, &out_view, target, ui);
        }
        crate::read_rgba8(ctx, &out, target.0, target.1)
    }

    /// The cost of the UI pass alone: paint `ui` `frames` times into an offscreen target of `target` pixels and wait for the GPU after
    /// each; returns the wall time of each paint (record + submit + GPU finish) in milliseconds.
    pub fn time_paint(&mut self, ctx: &GpuContext, target: (u32, u32), ui: &UiFrame, frames: usize) -> Vec<f64> {
        let tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui-time"),
            size: wgpu::Extent3d { width: target.0, height: target.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let mut out = Vec::with_capacity(frames);
        let steady = UiFrame { primitives: ui.primitives.clone(), textures_delta: Default::default(), pixels_per_point: ui.pixels_per_point };
        for i in 0..frames {
            let t = std::time::Instant::now();
            self.paint(ctx, &view, target, if i == 0 { ui } else { &steady });
            ctx.wait_idle();
            out.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        out
    }
}
