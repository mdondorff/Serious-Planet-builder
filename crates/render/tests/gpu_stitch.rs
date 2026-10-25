//! ADR 0074 (change 2): the edge masks of a node (`stitch`: its edge faces a capped leaf one level coarser, `fine`: a capped leaf's edge faces
//! finer ones) reach the vertex stage of the compact renderer: with a mask the vertices of that edge are held to the coarse (stitch) or the
//! own (fine) grid, so the picture changes along the border and nowhere else; a node without a mask is drawn as before. On every adapter.

use planet_core::cube::TangentWarp;
use planet_core::tile::Side;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_frame::camera::Camera;
use planet_frame::pool::compact_format_pool_with;
use planet_frame::terrain::side_bit;
use planet_frame::{TerrainDraw, TerrainPlan, TerrainView};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::HeightField;
use planet_generators::stress::Steep;
use planet_render::{AdapterPolicy, GpuContext, TerrainRenderer};

const R: f64 = 6_371_000.0;
const SIZE: (u32, u32) = (320, 240);
const CELLS: u32 = 32;

fn draw(tile: TileId, data: &planet_generators::compact::CompactTile, camera: &Camera, stitch: u8, fine: u8) -> TerrainDraw {
    let rel = data.origin.0 - camera.position.0;
    TerrainDraw {
        tile,
        level: tile.level(),
        origin: data.origin,
        camera_relative_origin: [rel.x as f32, rel.y as f32, rel.z as f32],
        morph: 0.0,
        // The leaves do not morph by distance here: the edge masks are all that moves a vertex.
        morph_start_m: 1.0e30,
        morph_end_m: 1.0e30,
        color: [200, 160, 60, 255],
        mesh_key: data.key,
        preview: false,
        transient_only: false,
        capped: fine != 0,
        stitch,
        fine,
        quadrants: 0b1111,
        state: planet_frame::TileState::default(),
        chunk_weight: 1.0,
    }
}

fn render(ctx: &GpuContext, tiles: &[&planet_generators::compact::CompactTile], plan: &TerrainPlan) -> Vec<u8> {
    let block_slots = 8 * 8;
    let pool = compact_format_pool_with(
        (128 << 20) + 40 * u64::from(block_slots) * planet_generators::compact::slot_bytes(CELLS),
        CELLS,
        block_slots,
        1,
    )
    .unwrap();
    let mut r = TerrainRenderer::with_pools(ctx, SIZE, CELLS, pool);
    for t in tiles {
        r.upload_tile(ctx, t).unwrap();
    }
    r.render(ctx, plan, false).unwrap();
    r.read_color(ctx).rgba
}

// spec: LOD-008, REND-016
#[test]
fn the_edge_masks_hold_the_vertices_of_an_edge_to_the_coarse_or_the_own_grid_and_move_nothing_else() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let field = Steep(1);
    // A capped tile and, across its east edge, a finer leaf: a child of the east neighbour, the one that touches the edge.
    let at = Vec3::new(0.8, 0.45, 0.35).normalized();
    let capped = TileId::from_direction(&TangentWarp, at, 16);
    let east = capped.neighbor(&TangentWarp, Side::East);
    let finer = east
        .children()
        .unwrap()
        .into_iter()
        .find(|c| {
            (0..4).any(|s| c.neighbor(&TangentWarp, Side::ALL[s]).parent() == Some(capped))
                || c.neighbor(&TangentWarp, Side::West).parent() == Some(capped)
        })
        .expect("a child of the east neighbour touches the capped tile");
    let facing = Side::ALL.into_iter().find(|s| finer.neighbor(&TangentWarp, *s).parent() == Some(capped)).unwrap();
    let (cd, fd) = (compact_tile_field(&field, &TangentWarp, capped, CELLS, R), compact_tile_field(&field, &TangentWarp, finer, CELLS, R));
    // Seen obliquely from above the pair, so that the relief of the border shows in the picture.
    let centre = capped.center_direction(&TangentWarp);
    let aspect = f64::from(SIZE.0) / f64::from(SIZE.1);
    let camera =
        Camera::looking_at_centre(PlanetFixed(centre * (R + field.height(centre) + 220.0)), Vec3::new(0.0, 0.0, 1.0), 0.9, aspect, 1.0);
    let plan = |nodes: Vec<TerrainDraw>| TerrainPlan {
        size: SIZE,
        view: TerrainView::Relief,
        camera,
        view_projection: camera.view_projection(),
        clear_color: [10, 10, 30, 255],
        height_range: [-9_000.0, 9_000.0],
        cells: CELLS,
        shore_ramp_per_m: 0.0,
        nodes,
        lit: None,
        water: None,
    };
    let tiles = [&cd, &fd];
    let bare = render(&ctx, &tiles, &plan(vec![draw(capped, &cd, &camera, 0, 0), draw(finer, &fd, &camera, 0, 0)]));
    // The capped tile's edge facing the finer leaf is `fine`, the finer leaf's edge facing it `stitch`.
    let capped_facing = Side::ALL.into_iter().find(|s| capped.neighbor(&TangentWarp, *s) == finer.parent().unwrap()).unwrap();
    let masked = render(
        &ctx,
        &tiles,
        &plan(vec![draw(capped, &cd, &camera, 0, side_bit(capped_facing)), draw(finer, &fd, &camera, side_bit(facing), 0)]),
    );
    assert_ne!(bare, masked, "the finer leaf's border vertices moved to the coarse grid");
    // A mask on an edge that faces nothing draws the same as no mask only where its vertices are on that edge: a stitched edge and the same
    // edge twice are the same picture, and the finer leaf alone with the mask differs from it bare.
    let again = render(
        &ctx,
        &tiles,
        &plan(vec![draw(capped, &cd, &camera, 0, side_bit(capped_facing)), draw(finer, &fd, &camera, side_bit(facing), 0)]),
    );
    assert_eq!(masked, again, "the same plan draws the same picture");
    let finer_alone_bare = render(&ctx, &tiles, &plan(vec![draw(finer, &fd, &camera, 0, 0)]));
    let finer_alone_masked = render(&ctx, &tiles, &plan(vec![draw(finer, &fd, &camera, side_bit(facing), 0)]));
    assert_ne!(finer_alone_bare, finer_alone_masked, "the stitch alone moves the finer leaf's vertices");
    // The change is at the facing edge: the centre of the pixels that the stitch changes is nearer the middle of that edge of the finer leaf than
    // the middle of the opposite one.
    let project = |p: Vec3| -> (f64, f64) {
        let rel = p - camera.position.0;
        let m = camera.view_projection().map(f64::from);
        let clip = |row: usize| m[row] * rel.x + m[4 + row] * rel.y + m[8 + row] * rel.z + m[12 + row];
        let (x, y, w) = (clip(0), clip(1), clip(3));
        ((x / w * 0.5 + 0.5) * f64::from(SIZE.0), (1.0 - (y / w * 0.5 + 0.5)) * f64::from(SIZE.1))
    };
    let middle = |side: Side| {
        let (k, l) = match side {
            Side::East => (CELLS, CELLS / 2),
            Side::West => (0, CELLS / 2),
            Side::North => (CELLS / 2, CELLS),
            Side::South => (CELLS / 2, 0),
        };
        let q = finer.sample_direction(&TangentWarp, CELLS, k, l);
        project(q * (R + field.height(q)))
    };
    let opposite = match facing {
        Side::East => Side::West,
        Side::West => Side::East,
        Side::North => Side::South,
        Side::South => Side::North,
    };
    let changed: Vec<(f64, f64)> = finer_alone_bare
        .chunks(4)
        .zip(finer_alone_masked.chunks(4))
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| ((i as u32 % SIZE.0) as f64, (i as u32 / SIZE.0) as f64))
        .collect();
    let n = changed.len() as f64;
    let centre = (changed.iter().map(|p| p.0).sum::<f64>() / n, changed.iter().map(|p| p.1).sum::<f64>() / n);
    let dist = |q: (f64, f64)| libm::hypot(q.0 - centre.0, q.1 - centre.1);
    assert!(
        dist(middle(facing)) < dist(middle(opposite)),
        "the stitch changes the facing edge, not the far one: {centre:?} against {:?} and {:?}",
        middle(facing),
        middle(opposite)
    );
    // A fine edge holds the capped tile's edge to its own grid while the tile itself morphs to its parent's (every vertex fully morphed): the
    // picture differs from the same tile without the mask.
    let morphing = |mut d: TerrainDraw| {
        d.morph_start_m = 0.0;
        d.morph_end_m = 1.0e-3;
        d
    };
    let whole = render(&ctx, &tiles, &plan(vec![morphing(draw(capped, &cd, &camera, 0, 0))]));
    let held = render(&ctx, &tiles, &plan(vec![morphing(draw(capped, &cd, &camera, 0, side_bit(capped_facing)))]));
    assert_ne!(whole, held, "a fine edge is held to the tile's own grid while the tile morphs");
    // The picture changes only along the border: most of the pixels are the same.
    let same = bare.chunks(4).zip(masked.chunks(4)).filter(|(a, b)| a == b).count();
    let total = bare.len() / 4;
    assert!(same * 10 > total * 9, "only the border changes: {same} of {total} pixels are unchanged");
}
