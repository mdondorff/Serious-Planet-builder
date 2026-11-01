//! The shadow pass of a frame (REND-047 to REND-050, ADR 0085): the clipmap gather over the frame's plan, the coarse cube when a floor tile
//! changed, then the receivers, the march, the upsample and the temporal pass over the lit pass's depth. The renderer decides nothing
//! (CON-03): the plan, the preview, the camera and the sun arrive as a [`ShadowFrame`] set by the app, as the horizon data did. The
//! resolved visibility is kept in a target of its own; nothing reads it for shading yet (REND-051 follows).

use planet_core::TileId;
use planet_frame::clipmap::{ClipmapPlan, TexelRect};
use planet_frame::shadowview::ShadowView;

use crate::march::{MarchParams, ShadowMarchGpu, MAX_MARCH_M, PENUMBRA_SEARCH_M, SUN_DISC_TAN};
use crate::resolve::{spread_for_sun, ResolveCamera, ResolveParams, ShadowResolveGpu, MAX_HISTORY_WEIGHT};
use crate::shadow::{ClipmapGpu, ClipmapPreview, CoarseCubeGpu, COARSE_CUBE_TEXELS};
use crate::terrain::TerrainRenderer;
use crate::GpuContext;

const SHADOW_VIEW_SHADER: &str = r#"
@group(0) @binding(0) var shadow: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_depth_2d;
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    if (textureLoad(depth, p, 0) <= 0.0) {
        discard;
    }
    let v = textureLoad(shadow, p, 0).x;
    return vec4<f32>(v, v, v, 1.0);
}
"#;

/// What the app decides about the shadow of one frame.
#[derive(Clone, Debug)]
pub struct ShadowFrame {
    /// The clipmap plan: tile blocks, dirty rectangles, the anchor and the generation.
    pub plan: ClipmapPlan,
    /// The frame's preview stamps and flagged tiles (REND-033, REND-045).
    pub preview: ClipmapPreview,
    /// The camera and the sun in the anchor's tangent frame.
    pub view: ShadowView,
    /// The floor tiles (levels 3 and 4) that landed or changed since the last frame: the coarse cube is rewritten from them. On the first
    /// frame, and after the cube was lost, all of them.
    pub floor_tiles: Vec<TileId>,
    /// A camera cut or teleport: history is void and every ray is marched.
    pub cut: bool,
    /// The highest terrain above the sphere this frame, metres, preview lifts included (`planet_frame::shadowview::ceiling_m`): a ray above
    /// it ends its march. Infinity: unknown, no early exit.
    pub ceiling_m: f64,
}

/// The GPU side of the shadow pass.
pub struct ShadowPath {
    clip: ClipmapGpu,
    cube: CoarseCubeGpu,
    march: ShadowMarchGpu,
    resolve: ShadowResolveGpu,
    cube_texel_m: f32,
    frame: u32,
    generation: Option<u64>,
    previous: Option<ResolveCamera>,
    /// Floor tiles the cube found not resident: asked for again next frame.
    pending_floor: Vec<TileId>,
    grey: Option<wgpu::RenderPipeline>,
}

fn camera_of(v: &ShadowView) -> ResolveCamera {
    ResolveCamera { position: v.position, forward: v.forward, right: v.right, up: v.up, tan_half_fov: v.tan_half_fov, near_m: v.near_m }
}

fn window(r: &TexelRect) -> [i32; 4] {
    [r.x0 as i32, r.y0 as i32, r.x1 as i32, r.y1 as i32]
}

impl ShadowPath {
    /// `texels` and `levels` as the plan's `ClipmapParams`, `cells` the tiles' cell count, `radius_m` the body's (the coarse cube's texel
    /// size is computed here, on the CPU).
    pub fn new(ctx: &GpuContext, texels: u32, levels: usize, cells: u32, radius_m: f64) -> ShadowPath {
        let clip = ClipmapGpu::new(ctx, texels, levels, cells);
        ShadowPath {
            resolve: ShadowResolveGpu::new(ctx, &clip),
            cube: CoarseCubeGpu::new(ctx, COARSE_CUBE_TEXELS, cells),
            march: ShadowMarchGpu::new(ctx),
            clip,
            cube_texel_m: (radius_m * core::f64::consts::FRAC_PI_2 / f64::from(COARSE_CUBE_TEXELS)) as f32,
            frame: 0,
            generation: None,
            previous: None,
            pending_floor: Vec::new(),
            grey: None,
        }
    }

    /// Run the passes of one frame over `depth_view` (the lit pass's reversed-Z depth, `size` pixels). Returns the tiles that were not
    /// resident when the gather or the cube asked for them (the app passes them as refreshed once they are).
    pub(crate) fn run(
        &mut self,
        ctx: &GpuContext,
        terrain: &TerrainRenderer,
        f: &ShadowFrame,
        depth_view: &wgpu::TextureView,
        size: (u32, u32),
    ) -> Vec<TileId> {
        self.frame += 1;
        let frame = self.frame;
        let reanchored = self.generation != Some(f.plan.generation) || f.plan.reanchored;
        self.generation = Some(f.plan.generation);
        let cut = f.cut || reanchored || self.previous.is_none();
        let skipped = self.clip.gather(ctx, terrain, &f.plan, &f.preview);
        let generations = self.resolve.generations_mut();
        if cut {
            generations.mark_all(ctx, frame);
        } else {
            generations.mark_rects(ctx, frame, f.plan.levels.iter().enumerate().flat_map(|(k, l)| l.dirty.iter().map(move |r| (k, r))));
        }
        let mut floor: Vec<TileId> = std::mem::take(&mut self.pending_floor);
        floor.extend(f.floor_tiles.iter().copied());
        floor.sort();
        floor.dedup();
        if !floor.is_empty() {
            self.pending_floor = self.cube.rebuild(ctx, terrain, &floor);
        }
        let camera = camera_of(&f.view);
        let march = MarchParams {
            sun: f.view.sun,
            sun_tan: SUN_DISC_TAN,
            drop_coefficient: f.view.drop_coefficient,
            max_distance_m: MAX_MARCH_M,
            penumbra_search_m: PENUMBRA_SEARCH_M,
            levels: f.plan.levels.iter().map(|l| (window(&l.window), l.texel_m as f32)).collect(),
            use_cube: true,
            anchor: f.view.anchor,
            east: f.view.east,
            north: f.view.north,
            cube_texel_m: self.cube_texel_m,
            ceiling_m: f.ceiling_m as f32,
            dirty_since: 0,
        };
        let params = ResolveParams {
            camera,
            previous: self.previous.unwrap_or(camera),
            history_weight: MAX_HISTORY_WEIGHT,
            cut,
            frame,
            // Chosen from the sun's elevation at the anchor; receivers far off see it up to about 9 degrees away (a stated limit).
            spread: spread_for_sun(f.view.sun[2]),
        };
        self.resolve.resolve_view(ctx, &self.march, &self.clip, &self.cube, &march, depth_view, size, &params);
        self.previous = Some(camera);
        let mut out = skipped;
        out.extend(self.pending_floor.iter().copied());
        out
    }

    /// J's shadow view (VIEW-054): draw the resolved visibility as grey over `target` (`Rgba8Unorm`) where `depth_view` holds a surface; the
    /// sky keeps what it shows.
    pub(crate) fn draw_visibility(&mut self, ctx: &GpuContext, depth_view: &wgpu::TextureView, target: &wgpu::TextureView) {
        let Some(shadow) = self.resolve.shadow_texture_view() else { return };
        let device = &ctx.device;
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow-view"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline = self.grey.get_or_insert_with(|| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shadow-view"),
                source: wgpu::ShaderSource::Wgsl(SHADOW_VIEW_SHADER.into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow-view"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow-view"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-view"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shadow-view") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow-view"),
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
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        ctx.queue.submit([encoder.finish()]);
    }

    /// The resolved visibility of the last frame, `width * height` values in [0, 1], row-major (tests and the shadow view).
    pub fn read_visibility(&self, ctx: &GpuContext) -> Vec<f32> {
        self.resolve.read_shadow(ctx)
    }

    /// How many frames this pass has run.
    pub fn frames_run(&self) -> u32 {
        self.frame
    }

    /// How many half-resolution pixels of the last frame marched across a clipmap cell that changed this frame or the last (tests).
    pub fn changed_pixels(&self, ctx: &GpuContext) -> usize {
        self.resolve.read_half_changed(ctx).into_iter().filter(|c| *c).count()
    }

    /// The clipmap, for tests.
    pub fn clipmap(&self) -> &ClipmapGpu {
        &self.clip
    }
}
