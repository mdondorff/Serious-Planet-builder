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
    FaceSpec { face: Face(face), cells, radius_m: R, sea_level: 0.0, seed: 17 }
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
    assert!((change_at(&faces, d) - f64::from(faces[2].change[20 * 48 + 10])).abs() < 1e-4);
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
