//! LOD-007, tier B: the vertex-per-plane sweeps at their full density. `frustum_cut.rs` (tier A) runs the same checks on a
//! slice (every eighth dropped tile of every camera; on the steep field only two of the four headings of each view); this
//! runs every second dropped tile of every camera, the density the tier-A tests had before `test-fast-regression`, because
//! the tile meshes are the most expensive work of `cargo xtask test fast`.

use planet_frame::{scripted_views, scripted_views_field};
use planet_generators::height::Reference;
use planet_generators::stress::Steep;

#[path = "support/frustum_cut.rs"]
mod support;
use support::{check_field, R};

// spec: LOD-007
#[test]
fn a_cut_tile_has_no_vertex_inside_the_frustum_at_every_second_dropped_tile() {
    check_field(&Reference(1), scripted_views(R, 16.0 / 9.0, 1), 8, 14, 6.0, 2, &|_| true);
}

// spec: LOD-007
#[test]
fn a_cut_tile_has_no_vertex_inside_the_frustum_on_the_steep_field_at_every_second_dropped_tile() {
    let field = Steep(1);
    let views =
        scripted_views_field(R, 16.0 / 9.0, &field).into_iter().filter(|(n, _)| ["ground-200m", "cube-corner-1km"].contains(n)).collect();
    check_field(&field, views, 8, 11, 2.0, 2, &|_| true);
}
