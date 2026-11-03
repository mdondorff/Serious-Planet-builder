//! Tier A, GEN-100 (m9-bake PR 4): pass 1 on a cube face and the maps of the carved base. A synthetic base (mountains over a sea), a small resolution, no store and no bake:
//! the change is the carve's, the same bytes on any thread count, a face's border and the sea keep their heights, and the six faces make a planet map.

use planet_core::{Face, Vec3};
use planet_generators::bake::carve::CarveParams;
use planet_generators::bake::face::{pass1_face, FaceChange, FaceSpec};
use planet_generators::bake::map::{change_at, pass1_maps};

const R: f64 = 6_371_000.0;

/// Mountains of up to about 3,500 m on land, a sea of -2000 m, a function of the direction only.
fn base(d: Vec3) -> f64 {
    let ridge = 2200.0 * libm::sin(5.0 * d.x + 1.0) * libm::cos(4.0 * d.y) + 1400.0 * libm::sin(9.0 * d.z + 0.3);
    if ridge > 0.0 {
        ridge + 50.0
    } else {
        -2000.0 + 0.5 * ridge
    }
}

fn spec(face: u8, cells: usize) -> FaceSpec {
    FaceSpec { face: Face(face), cells, radius_m: R, sea_level: 0.0, seed: 17, halo: 0 }
}

fn bits(c: &FaceChange) -> Vec<u32> {
    c.change.iter().map(|v| v.to_bits()).collect()
}

// spec: GEN-100
#[test]
fn a_face_carve_is_the_same_bytes_on_1_and_8_threads_and_keeps_the_border_and_the_sea() {
    let (s, params) = (spec(0, 64), CarveParams::default());
    let (one, eight) = (pass1_face(&s, &base, None, &params, 1), pass1_face(&s, &base, None, &params, 8));
    assert_eq!(bits(&one), bits(&eight), "the face's change differs between 1 and 8 threads");
    assert_eq!((one.face, one.cells, one.change.len()), (Face(0), 64, 64 * 64));
    let uplift = (params.u_over_k * params.k_dt * params.steps as f64) as f32;
    let mut moved = 0;
    for j in 0..64 {
        for i in 0..64 {
            let c = one.change[j * 64 + i];
            let edge = i == 0 || j == 0 || i == 63 || j == 63;
            let sea = base(s.direction(i, j)) <= 0.0;
            if edge || sea {
                assert_eq!(c, 0.0, "cell ({i}, {j}) on the border or the sea changed by {c} m");
            } else {
                assert!(c <= uplift + 1e-3, "cell ({i}, {j}) rose by {c} m, more than the uplift {uplift}");
                moved += usize::from(c != 0.0);
            }
        }
    }
    assert!(moved > 500, "{moved} land cells changed");
}

// spec: GEN-100
#[test]
fn faces_draw_their_own_jitter_stream_and_the_carve_factor_changes_the_change() {
    let params = CarveParams::default();
    // The same base on a face and on the next: the streams differ, so the changes differ even over the same relief (a symmetric base would otherwise repeat).
    let flat_land = |_: Vec3| 300.0;
    let (a, b) = (pass1_face(&spec(0, 32), &flat_land, None, &params, 2), pass1_face(&spec(1, 32), &flat_land, None, &params, 2));
    assert_ne!(bits(&a), bits(&b), "two faces over the same base draw the same jitter");
    let half = |_: Vec3| 0.5;
    let weaker = pass1_face(&spec(0, 32), &flat_land, Some(&half), &params, 2);
    assert_ne!(bits(&a), bits(&weaker), "the carve factor changes nothing");
}

// spec: GEN-100
#[test]
fn the_six_faces_make_a_planet_map_with_its_base_carved_base_and_change() {
    let params = CarveParams::default();
    let faces: Vec<FaceChange> = (0..6).map(|f| pass1_face(&spec(f, 48), &base, None, &params, 4)).collect();
    let faces: [FaceChange; 6] = faces.try_into().expect("six faces");
    // The change at a cell centre is the cell's value; between centres it is bilinear.
    let s = spec(2, 48);
    let d = s.direction(10, 20);
    assert!((change_at(&faces, d) - faces[2].at(10, 20)).abs() < 1e-4);
    let maps = pass1_maps(256, 17, &base, &faces);
    assert_eq!(maps.iter().map(|m| m.0.as_str()).collect::<Vec<_>>(), ["base", "carved", "change"]);
    for (name, img) in &maps {
        assert_eq!(img.width, 256, "{name}");
        assert!(img.height > 128 && img.data.len() == (img.width * img.height * 4) as usize, "{name}: {} x {}", img.width, img.height);
    }
    // The carved map differs from the base map where the carve acts, and the change map is not blank.
    assert_ne!(maps[0].1.data, maps[1].1.data, "the carve changed nothing visible");
    if let Ok(dir) = std::env::var("PLANET_BAKE_MAP_DIR") {
        for (name, img) in &maps {
            img.write_png(&std::path::Path::new(&dir).join(format!("pass1-{name}.png"))).expect("a map written");
        }
    }
}

// spec: GEN-097
#[test]
fn the_carve_factor_is_one_but_half_in_a_sand_sea() {
    use planet_generators::classes::{blended_carve_factor, CLASS_COUNT};
    let mut w = [0.0; CLASS_COUNT];
    w[0] = 1.0;
    assert_eq!(blended_carve_factor(&w), 1.0);
    w = [0.0; CLASS_COUNT];
    w[CLASS_COUNT - 1] = 1.0;
    assert_eq!(blended_carve_factor(&w), 0.5);
    w[CLASS_COUNT - 1] = 0.5;
    w[3] = 0.5;
    assert_eq!(blended_carve_factor(&w), 0.75);
}

// spec: GEN-100
#[test]
fn the_change_is_the_same_bytes_on_1_and_8_threads_with_a_carve_factor() {
    let (s, params) = (spec(1, 48), CarveParams::default());
    let factor = |d: Vec3| 0.6 + 0.4 * libm::sin(3.0 * d.x + d.z);
    let (one, eight) = (pass1_face(&s, &base, Some(&factor), &params, 1), pass1_face(&s, &base, Some(&factor), &params, 8));
    assert_eq!(bits(&one), bits(&eight));
}

// spec: GEN-100
#[test]
fn a_halo_makes_the_border_an_ordinary_cell_and_the_change_continuous_across_an_edge() {
    use planet_core::cube::{face_to_direction, TangentWarp};
    let params = CarveParams::default();
    let with_halo = |face: u8| FaceSpec { halo: 6, ..spec(face, 48) };
    let faces: Vec<FaceChange> = (0..6).map(|f| pass1_face(&with_halo(f), &base, None, &params, 4)).collect();
    let faces: [FaceChange; 6] = faces.try_into().expect("six faces");
    assert_eq!((faces[0].cells, faces[0].halo, faces[0].change.len()), (48, 6, 60 * 60));
    // Without a halo the border is an outlet and keeps its height; with one, some border cells are carved.
    let carved_border = (0..48).filter(|&k| faces[0].at(0, k) != 0.0 || faces[0].at(k, 0) != 0.0).count();
    assert!(carved_border > 0, "no border cell was carved with a halo");
    // The cross-faded field is the same from both sides of an edge, to rounding.
    for t in [-0.7, -0.2, 0.3, 0.8] {
        let (a, b) = (face_to_direction(&TangentWarp, Face(0), 1.0 - 1e-9, t), face_to_direction(&TangentWarp, Face(0), 1.0 + 1e-9, t));
        let (ca, cb) = (change_at(&faces, a), change_at(&faces, b));
        assert!((ca - cb).abs() < 1e-3, "the change jumps by {} m across the edge at t = {t}", (ca - cb).abs());
    }
    // Deep inside a face the cross-fade does not act: the value is the face's own.
    let s = with_halo(2);
    assert!((change_at(&faces, s.direction(24, 24)) - faces[2].at(24, 24)).abs() < 1e-4);
}

// spec: GEN-100
#[test]
fn the_cross_fade_reads_the_neighbours_halo_and_is_continuous_around_a_corner() {
    use planet_core::cube::{direction_to_face, face_to_direction, TangentWarp};
    use planet_generators::bake::map::change_in_face;
    let (n, h) = (48usize, 6usize);
    let m = n + 2 * h;
    let step = 2.0 / n as f64;
    // The face across face 0's edge s = 1; its plane holds its plane index (so a read of its halo shows), the other faces a constant of their own.
    let nb = direction_to_face(&TangentWarp, face_to_direction(&TangentWarp, Face(0), 1.0 + step, 0.1)).0;
    let faces: Vec<FaceChange> = (0..6u8)
        .map(|f| FaceChange {
            face: Face(f),
            cells: n,
            halo: h,
            change: (0..m * m).map(|p| if Face(f) == nb { p as f32 } else { 3.0 * f32::from(f) }).collect(),
        })
        .collect();
    let faces: [FaceChange; 6] = faces.try_into().expect("six faces");
    // A direction on face 0 two and a half cells from the edge reads the neighbour's halo cells at that depth: its plane index there is not the edge line's.
    let a = face_to_direction(&TangentWarp, Face(0), 1.0 - 2.5 * step, 0.1);
    let deep = change_in_face(&faces[nb.0 as usize], a);
    let b = face_to_direction(&TangentWarp, Face(0), 1.0 - 0.5 * step, 0.1);
    let near = change_in_face(&faces[nb.0 as usize], b);
    assert!((deep - near).abs() > 1.0, "the neighbour's halo is not read: {near} at the edge, {deep} two cells in");
    // Around the vertex of face 0 with its two neighbours the field has no jump: points a cell either side of the diagonal, and at the vertex from each side, agree.
    let flat: Vec<FaceChange> =
        (0..6u8).map(|f| FaceChange { face: Face(f), cells: n, halo: h, change: vec![3.0 * f32::from(f); m * m] }).collect();
    let flat: [FaceChange; 6] = flat.try_into().expect("six faces");
    let vertex = |ds: f64, dt: f64| face_to_direction(&TangentWarp, Face(0), 1.0 - ds * step, 1.0 - dt * step);
    let (p, q) = (change_at(&flat, vertex(1.0, 1.1)), change_at(&flat, vertex(1.1, 1.0)));
    assert!((p - q).abs() < 0.5, "a jump of {} m across the corner diagonal", (p - q).abs());
    let (p, q, r) = (change_at(&flat, vertex(1e-6, 1e-6)), change_at(&flat, vertex(-1e-6, 1e-6)), change_at(&flat, vertex(1e-6, -1e-6)));
    assert!((p - q).abs() < 1e-3 && (p - r).abs() < 1e-3, "the vertex reads {p}, {q}, {r}");
}

// spec: GEN-100
#[test]
#[should_panic(expected = "under half the face")]
fn a_halo_of_half_the_face_or_more_is_refused() {
    pass1_face(&FaceSpec { halo: 3, ..spec(0, 6) }, &base, None, &CarveParams::default(), 1);
}

// spec: GEN-101
#[test]
fn the_pass_1_change_is_read_with_catmull_rom_exact_at_the_cells_and_smooth_across_them() {
    use planet_core::cube::{face_to_direction, TangentWarp};
    use planet_generators::bake::map::change_in_face;
    let s = FaceSpec { halo: 2, ..spec(0, 16) };
    let (m, h) = (s.cells + 2 * s.halo, s.halo as i64);
    let f = |i: i64, j: i64| 30.0 * libm::sin(0.45 * i as f64) + 20.0 * libm::cos(0.31 * j as f64) + 0.5 * (i * j) as f64;
    let change: Vec<f32> = (0..m * m).map(|p| f((p % m) as i64 - h, (p / m) as i64 - h) as f32).collect();
    let fc = FaceChange { face: s.face, cells: s.cells, halo: s.halo, change };
    // Exact at the cell centres.
    for (i, j) in [(3i64, 4i64), (8, 8), (11, 6), (0, 0), (15, 15)] {
        let v = change_in_face(&fc, s.halo_direction(i, j));
        assert!((v - f(i, j)).abs() < 1e-3 * (1.0 + f(i, j).abs()), "cell ({i}, {j}): {v} against {}", f(i, j));
    }
    // Continuous with its first derivative across a cell centre along each axis (a bilinear read has a kink there).
    let n = s.cells as f64;
    let at = |si: f64, ti: f64| {
        change_in_face(&fc, face_to_direction(&TangentWarp, s.face, 2.0 * (si + 0.5) / n - 1.0, 2.0 * (ti + 0.5) / n - 1.0))
    };
    let eps = 1e-3;
    for (ci, cj) in [(8.0, 8.0), (5.0, 10.0)] {
        let (left_i, right_i) = ((at(ci, cj) - at(ci - eps, cj)) / eps, (at(ci + eps, cj) - at(ci, cj)) / eps);
        let (left_j, right_j) = ((at(ci, cj) - at(ci, cj - eps)) / eps, (at(ci, cj + eps) - at(ci, cj)) / eps);
        let scale = |a: f64, b: f64| 1.0 + a.abs().max(b.abs());
        assert!((left_i - right_i).abs() < 0.03 * scale(left_i, right_i), "kink along i at ({ci}, {cj}): {left_i} against {right_i}");
        assert!((left_j - right_j).abs() < 0.03 * scale(left_j, right_j), "kink along j at ({ci}, {cj}): {left_j} against {right_j}");
    }
}
