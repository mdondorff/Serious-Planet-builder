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
            dirty_since: 0,
        };
        let params = ResolveParams {
            camera,
            previous: self.previous.unwrap_or(camera),
            history_weight: MAX_HISTORY_WEIGHT,
            cut,
            frame,
            spread: spread_for_sun(f.view.sun[2]),
        };
        self.resolve.resolve_view(ctx, &self.march, &self.clip, &self.cube, &march, depth_view, size, &params);
        self.previous = Some(camera);
        let mut out = skipped;
        out.extend(self.pending_floor.iter().copied());
        out
    }

    /// The resolved visibility of the last frame, `width * height` values in [0, 1], row-major (tests and the shadow view).
    pub fn read_visibility(&self, ctx: &GpuContext) -> Vec<f32> {
        self.resolve.read_shadow(ctx)
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
