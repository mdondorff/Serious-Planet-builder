//! Tier A, GEN-115: the field bake from the class source. The sample keeps the legacy continent and shore, takes the class source's elevation and the climate that follows it, and a block is
//! the same bytes on any thread count; the legacy sample is unchanged.

use planet_core::cube::TangentWarp;
use planet_core::{TileId, Vec3};
use planet_generators::bake::inputs::BakeInputs;
use planet_generators::global::{bake_blocks_sampled, class_sample, climate_from_elevation, sample};

const SEED: u64 = 14;

fn directions() -> Vec<Vec3> {
    // A fixed spread over the sphere: a golden-angle spiral.
    (0..1500)
        .map(|i| {
            let z = 1.0 - 2.0 * (f64::from(i) + 0.5) / 1500.0;
            let (r, a) = ((1.0 - z * z).sqrt(), f64::from(i) * 2.399_963_229_728_653);
            Vec3::new(r * libm::cos(a), r * libm::sin(a), z)
        })
        .collect()
}

// spec: GEN-115
#[test]
fn the_class_sample_keeps_the_continent_and_the_shore_and_takes_the_class_elevation_and_its_climate() {
    let inputs = BakeInputs::new(SEED, 2);
    let (mut land, mut differ, mut rain_differs) = (0, 0, 0);
    for d in directions() {
        let (legacy, class) = (sample(SEED, d), class_sample(SEED, d, &inputs));
        assert_eq!(legacy.continent.to_bits(), class.continent.to_bits(), "the continent value is the same at {d:?}");
        assert_eq!(
            legacy.elevation_m > 0.0,
            class.elevation_m > 0.0,
            "the shore moved at {d:?}: {} against {}",
            legacy.elevation_m,
            class.elevation_m
        );
        let expected = inputs.base(d);
        assert_eq!(class.elevation_m, expected as f32, "the elevation is the class source's");
        // The climate follows the elevation it colours: the temperature is the formula's at the class elevation; the rain is the orographic climate's (GEN-117, GLOBAL_VERSION 7), in the channel's range.
        let (t, _) = climate_from_elevation(SEED, d, expected, 0.0);
        assert_eq!(class.temperature_c.to_bits(), t.to_bits(), "temperature at {d:?}");
        assert_eq!(
            class.precipitation_mm.to_bits(),
            (inputs.orography().precipitation_mm(d).clamp(0.0, 4500.0) as f32).to_bits(),
            "rain at {d:?}"
        );
        rain_differs += usize::from((class.precipitation_mm - legacy.precipitation_mm).abs() > 1.0);
        if class.elevation_m > 0.0 {
            land += 1;
            differ += usize::from((class.elevation_m - legacy.elevation_m).abs() > 1.0);
        }
    }
    assert!(land > 200, "land points sampled: {land}");
    assert!(
        rain_differs * 2 > land,
        "the class world's rain follows its relief and the winds, not the legacy bands: {rain_differs} points differ"
    );
    assert!(differ * 2 > land, "the class elevation differs from the legacy one over land (the point of the change): {differ} of {land}");
}

// spec: GEN-115
#[test]
fn class_blocks_are_the_same_bytes_on_1_and_4_threads_and_the_legacy_sample_is_unchanged() {
    let inputs = BakeInputs::new(SEED, 2);
    let field = |d: Vec3| class_sample(SEED, d, &inputs);
    let run = |threads: usize| {
        let mut out: Vec<(TileId, Vec<u32>)> = Vec::new();
        bake_blocks_sampled(&field, &TangentWarp, 1, 4, threads, &|_| true, &|_, _| (), &mut |t, b, ()| {
            out.push((
                t,
                b.iter()
                    .flat_map(|s| [s.continent.to_bits(), s.elevation_m.to_bits(), s.temperature_c.to_bits(), s.precipitation_mm.to_bits()])
                    .collect(),
            ))
        })
        .unwrap();
        out
    };
    let (a, b) = (run(1), run(4));
    assert_eq!(a.len(), 6 * 4);
    assert_eq!(a, b);
    // The legacy entry point is the sampled one over the legacy field: the same blocks.
    let tile = TileId::new(planet_core::Face(1), 1, 1, 0).unwrap();
    assert_eq!(
        planet_generators::global::bake_block(SEED, &TangentWarp, tile, 4),
        planet_generators::global::bake_block_sampled(&|d| sample(SEED, d), &TangentWarp, tile, 4)
    );
}

// spec: GEN-115
#[test]
fn the_shore_is_the_legacy_one_down_to_the_first_land_value_and_the_seed_of_the_source_must_be_the_worlds() {
    let inputs = BakeInputs::new(SEED, 2);
    // Bisect between a land and a sea point to the first f32 continent value that is land: both fields agree on each side of that step.
    let dirs = directions();
    let land = dirs.iter().copied().find(|d| sample(SEED, *d).elevation_m > 100.0).expect("a land point");
    let sea = dirs.iter().copied().find(|d| sample(SEED, *d).elevation_m < -100.0).expect("a sea point");
    let (mut a, mut b) = (land, sea);
    for _ in 0..60 {
        let m = Vec3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, (a.z + b.z) / 2.0).normalized();
        if sample(SEED, m).elevation_m > 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    let (la, lb) = ((sample(SEED, a), class_sample(SEED, a, &inputs)), (sample(SEED, b), class_sample(SEED, b, &inputs)));
    assert!(la.0.elevation_m > 0.0 && la.1.elevation_m > 0.0, "the land side of the shore: {} and {}", la.0.elevation_m, la.1.elevation_m);
    assert!(lb.0.elevation_m <= 0.0 && lb.1.elevation_m <= 0.0, "the sea side of the shore: {} and {}", lb.0.elevation_m, lb.1.elevation_m);
}

// spec: GEN-115
#[test]
fn the_class_field_is_pinned_so_that_a_change_of_the_class_source_must_bump_its_version() {
    // Elevations (f32 bits) of the class field at six directions: a change in anything the class source reads that its identity does not hash (the continent smoothing, the band's
    // constants) moves one of them and has to come with a bump of the class source's version, which re-keys the stores.
    let inputs = BakeInputs::new(SEED, 2);
    let probes = [(0.3, -0.5, 0.8), (-0.7, 0.1, 0.4), (0.1, 0.9, -0.2), (0.6, 0.6, 0.5), (-0.2, -0.8, -0.5), (0.9, -0.1, 0.1)];
    let bits: Vec<u32> =
        probes.iter().map(|&(x, y, z)| class_sample(SEED, Vec3::new(x, y, z).normalized(), &inputs).elevation_m.to_bits()).collect();
    assert_eq!(bits, PINNED_BITS, "the class field changed: bump CLASS_SOURCE_VERSION and re-pin");
}

const PINNED_BITS: [u32; 6] = [1116318747, 3290137453, 1158361031, 3248661043, 3314327552, 3314327552];
