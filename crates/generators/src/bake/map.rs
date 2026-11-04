//! The preview maps of pass 1 (the owner's I5b look): the base and the carved base of the planet as equirectangular images, from the faces' changes, with no store and no viewer.

use super::face::FaceChange;
use crate::dump::Rgba;
use crate::preview::{change_map, elevation_map, Fields, Pixel};
use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
use planet_core::{Face, Vec3};

/// The change of one face read at a direction on the plane of that face (a direction of a neighbouring face reads at the plane's continuation, inside the halo; clamped to the
/// stored plane): the bilinear interpolant of the cells' values, the cell centres being the samples.
pub fn change_in_face(f: &FaceChange, d: Vec3) -> f64 {
    let w = d.dot(f.face.normal());
    assert!(w > 0.0 && w.is_finite(), "direction {d:?} is not in front of face {}", f.face.0);
    // The plane coordinate past the face (|u| > 1, the halo) continues the warp: `TangentWarp::plane_to_face` clamps there, so the inverse is taken here.
    let face_coordinate = |u: f64| libm::atan(u) / core::f64::consts::FRAC_PI_4;
    let (s, t) = (face_coordinate(d.dot(f.face.u_axis()) / w), face_coordinate(d.dot(f.face.v_axis()) / w));
    let (n, h, m) = (f.cells as f64, f.halo as f64, f.plane_cells());
    let top = (m - 1) as f64;
    let (x, y) = (((s + 1.0) * 0.5 * n - 0.5 + h).clamp(0.0, top), ((t + 1.0) * 0.5 * n - 0.5 + h).clamp(0.0, top));
    let (i, j) = ((x as usize).min(m - 2), (y as usize).min(m - 2));
    let (fx, fy) = (x - i as f64, y - j as f64);
    let at = |ii: usize, jj: usize| f64::from(f.change[jj * m + ii]);
    let (a, b) = (at(i, j) + (at(i + 1, j) - at(i, j)) * fx, at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fx);
    a + (b - a) * fy
}

/// The change of the six faces read at a direction. Within `halo` cells of a cube edge the face's change is cross-faded with the neighbouring face's, which carved the same ground
/// on its own lattice: the face's own weight is 1 and a neighbour's weight is `1 - smoothstep(distance to the shared edge / halo)` (so 0.5 and 0.5 at the edge), the sum is
/// normalised. At a corner the two neighbours across the two nearest edges both count (at the cube vertex the three faces weigh alike), so the field is continuous across the
/// edges and around the corners. Without a halo the face's own value is returned.
pub fn change_at(faces: &[FaceChange; 6], d: Vec3) -> f64 {
    let (face, s, t) = direction_to_face(&TangentWarp, d);
    let f = &faces[face.0 as usize];
    debug_assert_eq!(f.face, Face(face.0));
    let own = change_in_face(f, d);
    if f.halo == 0 {
        return own;
    }
    let (n, h) = (f.cells as f64, f.halo as f64);
    let (x, y) = ((s + 1.0) * 0.5 * n, (t + 1.0) * 0.5 * n);
    let beyond = 1.0 + 2.0 / n;
    let neighbour = |so: f64, to: f64| direction_to_face(&TangentWarp, face_to_direction(&TangentWarp, f.face, so, to)).0;
    let weight = |dist: f64| {
        let u = (dist / h).min(1.0);
        1.0 - u * u * (3.0 - 2.0 * u)
    };
    let (mut sum, mut total) = (own, 1.0);
    let (dx, dy) = (x.min(n - x), y.min(n - y));
    if dx < h {
        let nb = neighbour(if x < n - x { -beyond } else { beyond }, t);
        let w = weight(dx);
        sum += w * change_in_face(&faces[nb.0 as usize], d);
        total += w;
    }
    if dy < h {
        let nb = neighbour(s, if y < n - y { -beyond } else { beyond });
        let w = weight(dy);
        sum += w * change_in_face(&faces[nb.0 as usize], d);
        total += w;
    }
    sum / total
}

/// The size of the change's step across the cube edges against its step between neighbouring cells inside a face. For every cell along every edge of every face it takes the
/// difference of [`change_at`] between the border cell's centre and the centre of the cell just across the edge (one cell apart), and, as the reference, between two neighbouring
/// cells in the middle of the face. Returns `(seam, interior)`, each `(largest, 99th percentile)` in metres.
pub fn edge_steps(faces: &[FaceChange; 6]) -> ((f64, f64), (f64, f64)) {
    let (mut seam, mut inner): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for f in faces {
        let n = f.cells;
        let dir = |i: i64, j: i64| {
            let c = n as f64;
            face_to_direction(&TangentWarp, f.face, 2.0 * (i as f64 + 0.5) / c - 1.0, 2.0 * (j as f64 + 0.5) / c - 1.0)
        };
        let (n, mid) = (n as i64, n as i64 / 2);
        for k in 0..n {
            for (a, b) in [((0, k), (-1, k)), ((n - 1, k), (n, k)), ((k, 0), (k, -1)), ((k, n - 1), (k, n))] {
                seam.push((change_at(faces, dir(a.0, a.1)) - change_at(faces, dir(b.0, b.1))).abs());
            }
            inner.push((change_at(faces, dir(mid, k)) - change_at(faces, dir(mid + 1, k))).abs());
        }
    }
    let q = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        (v[v.len() - 1], v[(v.len() as f64 * 0.99) as usize])
    };
    (q(&mut seam), q(&mut inner))
}

/// The maps of a pass 1 over the whole planet: `(base, carved base, change)` as images of `width x width / 2` pixels (elevation with hillshade; the change in a diverging tint),
/// from the base field and the six faces' changes. The base is read per pixel (not per cell), so the maps show what pass 1 adds to the field as the runtime reads it.
pub fn pass1_maps(width: usize, seed: u64, base: &(dyn Fn(Vec3) -> f64 + Sync), faces: &[FaceChange; 6]) -> [(String, Rgba); 3] {
    field_maps(width, seed, base, &|d| change_at(faces, d), "BAKE PASS 1", "CARVED BASE")
}

/// The maps of a change field over the whole planet: `(base, base plus change, change)` titled with `stage` and `name` (the second map's).
pub fn field_maps(
    width: usize,
    seed: u64,
    base: &(dyn Fn(Vec3) -> f64 + Sync),
    change: &dyn Fn(Vec3) -> f64,
    stage: &str,
    name: &str,
) -> [(String, Rgba); 3] {
    let height = width / 2;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let lat = core::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * core::f64::consts::PI;
        for x in 0..width {
            let lon = -core::f64::consts::PI + (x as f64 + 0.5) / width as f64 * 2.0 * core::f64::consts::PI;
            let d = Vec3::from_lat_lon(lat, lon);
            let (e, c) = (base(d), change(d));
            pixels.push(Pixel {
                land_depth: e as f32,
                today_m: e as f32,
                new_m: (e + c) as f32,
                k: 0.0,
                terms: [0.0; 6],
                margin_km: 0.0,
                weights: [0.0; crate::classes::CLASS_COUNT],
            });
        }
    }
    let fields = Fields { seed, width, height, pixels, largest_k_step: 0.0, k_edges_over: 0.0, k_step_p99: 0.0 };
    let base_map = elevation_map(&fields, &format!("{stage}: BASE ELEVATION"), |p| f64::from(p.today_m));
    let carved = elevation_map(&fields, &format!("{stage}: {name}"), |p| f64::from(p.new_m));
    let change = change_map(&fields, &format!("{stage}: CHANGE ({name} MINUS BASE)"), 400.0, |p| f64::from(p.new_m - p.today_m));
    [("base".into(), base_map), ("carved".into(), carved), ("change".into(), change)]
}

/// The change of the erosion layer at a direction: the stored tile's core cell that holds it (the nearest cell), or the pass-1 change where no tile was baked (the sea).
/// `tiles` maps `(face, ti, tj)` to a tile's core change, row-major `tile x tile`.
pub fn layer_change_at(
    g: &super::tile::TileGrid,
    tiles: &std::collections::HashMap<(u8, u32, u32), Vec<f32>>,
    faces: &[FaceChange; 6],
    d: Vec3,
) -> f64 {
    let (face, s, t) = direction_to_face(&TangentWarp, d);
    let n = g.cells as f64;
    let cell = |c: f64| ((c + 1.0) * 0.5 * n).floor().clamp(0.0, n - 1.0) as usize;
    let (i, j) = (cell(s), cell(t));
    match tiles.get(&(face.0, (i / g.tile) as u32, (j / g.tile) as u32)) {
        Some(core) => f64::from(core[(j % g.tile) * g.tile + i % g.tile]),
        None => change_at(faces, d),
    }
}

/// A tile as three panels side by side: the hillshade of the base, of the base with the layer's change, and the change (grey at 0, blue cut, red raised, over `scale_m`). `base` and
/// `change` are the tile's core, row-major `n x n`, `dx` metres a cell.
pub fn tile_image(n: usize, dx: f64, base: &[f64], change: &[f32], scale_m: f64) -> Rgba {
    assert!(base.len() == n * n && change.len() == n * n && n >= 3, "a tile image takes n x n cells");
    let shade = |h: &dyn Fn(usize, usize) -> f64, i: usize, j: usize| {
        let (ip, im, jp, jm) = ((i + 1).min(n - 1), i.saturating_sub(1), (j + 1).min(n - 1), j.saturating_sub(1));
        let (gx, gy) = ((h(ip, j) - h(im, j)) / (dx * (ip - im) as f64), (h(i, jp) - h(i, jm)) / (dx * (jp - jm) as f64));
        // Light from the north-west at 45 degrees.
        let lit = (0.7 * gx + 0.7 * gy + 0.7) / libm::sqrt(gx * gx + gy * gy + 1.0);
        (60.0 + 190.0 * lit.clamp(0.0, 1.0)) as u8
    };
    let b = |i: usize, j: usize| base[j * n + i];
    let a = |i: usize, j: usize| base[j * n + i] + f64::from(change[j * n + i]);
    let mut data = vec![255u8; 3 * n * n * 4];
    let width = 3 * n;
    for j in 0..n {
        for i in 0..n {
            let (s0, s1) = (shade(&b, i, j), shade(&a, i, j));
            let t = (f64::from(change[j * n + i]) / scale_m).clamp(-1.0, 1.0);
            let c = if t < 0.0 {
                [150.0 + (20.0 - 150.0) * -t, 150.0 + (60.0 - 150.0) * -t, 150.0 + (220.0 - 150.0) * -t]
            } else {
                [150.0 + (230.0 - 150.0) * t, 150.0 + (40.0 - 150.0) * t, 150.0 + (30.0 - 150.0) * t]
            };
            for (panel, px) in [[s0, s0, s0], [s1, s1, s1], [c[0] as u8, c[1] as u8, c[2] as u8]].into_iter().enumerate() {
                let o = (j * width + panel * n + i) * 4;
                data[o..o + 3].copy_from_slice(&px);
            }
        }
    }
    Rgba { width: width as u32, height: n as u32, data }
}
