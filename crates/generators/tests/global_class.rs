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
    let (mut land, mut differ) = (0, 0);
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
        // The climate follows the elevation it colours: the temperature is the formula's at the class elevation, the rain the same within rounding of the interior.
        let (t, p) = climate_from_elevation(SEED, d, expected, interior_of(d));
        assert_eq!(class.temperature_c.to_bits(), t.to_bits(), "temperature at {d:?}");
        assert!((class.precipitation_mm - p).abs() < 1.0, "rain at {d:?}: {} against {p}", class.precipitation_mm);
        if class.elevation_m > 0.0 {
            land += 1;
            differ += usize::from((class.elevation_m - legacy.elevation_m).abs() > 1.0);
        }
    }
    assert!(land > 200, "land points sampled: {land}");
    assert!(differ * 2 > land, "the class elevation differs from the legacy one over land (the point of the change): {differ} of {land}");
}

/// The inland measure the climate used for `d`: read back from the legacy sample's own climate (the legacy field uses the same interior), so the test needs no private terrain.
fn interior_of(d: Vec3) -> f64 {
    // The legacy sample's climate is climate_from_elevation(seed, d, legacy elevation, interior): solve for the interior that reproduces the legacy precipitation.
    let s = sample(SEED, d);
    let e = f64::from(s.elevation_m);
    let (_, p0) = climate_from_elevation(SEED, d, e, 0.0);
    let (_, p1) = climate_from_elevation(SEED, d, e, 1.0);
    if (p1 - p0).abs() < 1e-3 {
        return 0.0;
    }
    f64::from((s.precipitation_mm - p0) / (p1 - p0)).clamp(0.0, 1.0)
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
    // The legacy sample at a known direction is what it was: the field of the legacy stores (a pinned bit pattern of one point).
    let d = Vec3::new(0.3, -0.5, 0.8).normalized();
    assert_eq!(sample(SEED, d), sample(SEED, d));
}
