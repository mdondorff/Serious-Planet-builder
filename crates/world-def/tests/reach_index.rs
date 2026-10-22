//! Tier A: the candidate index of the footprint query (WDEF-014, SPIKE-29): a query gives the set the scan of every item gives, whatever the
//! caps, the reaches, the layers and the edits, and it looks at the items near its cap, not at all of them.

use planet_core::{TileId, Vec3};
use planet_world_def::cells::cell_of;
use planet_world_def::footprint::{self, Cap};
use planet_world_def::{BodyId, ItemSpec, WorldDefinition};

const R: f64 = 6_371_000.0;

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
}

fn unit(lat_deg: f64, lon_deg: f64) -> Vec3 {
    let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
    Vec3::new(libm::cos(lat) * libm::cos(lon), libm::cos(lat) * libm::sin(lon), libm::sin(lat))
}

fn spec(id: u128, layer: &str, dir: Vec3, reach_m: f64, priority: i32) -> ItemSpec {
    ItemSpec { id, layer: layer.into(), priority, dir, content_hash: id * 7, reach_m, max_delta_m: 1.0, stamp: None }
}

/// The items a scan of every item accepts for `cap` in `layer`, in application order.
fn scan(items: &[ItemSpec], layer: &str, cap: Cap, radius_m: f64) -> Vec<u128> {
    let mut found: Vec<(i32, u128)> = items
        .iter()
        .filter(|i| i.layer == layer && footprint::touches(cap, i.dir.normalized(), i.reach_m, radius_m))
        .map(|i| (i.priority, i.id))
        .collect();
    found.sort();
    found.into_iter().map(|(_, id)| id).collect()
}

fn ids(found: &[footprint::FootprintItem]) -> Vec<u128> {
    found.iter().map(|i| i.id).collect()
}

/// Places that stress the index: across the face seams and corners, at the poles, and anywhere.
fn random_dir(rng: &mut Rng) -> Vec3 {
    match rng.next_u64() % 4 {
        0 => unit(rng.range(-90.0, 90.0), rng.range(-180.0, 180.0)),
        1 => unit(35.264 + rng.range(-0.5, 0.5), 45.0 + rng.range(-0.5, 0.5)), // a cube corner
        2 => unit(rng.range(-0.5, 0.5), 45.0 + rng.range(-0.5, 0.5)),          // a face edge
        _ => unit(89.9 + rng.range(0.0, 0.1), rng.range(-180.0, 180.0)),       // a pole
    }
}

// spec: WDEF-014
#[test]
fn the_indexed_query_gives_the_set_the_scan_of_every_item_gives_after_every_kind_of_edit() {
    let mut rng = Rng(5);
    let mut def = WorldDefinition::single_planet(R, 3);
    let mut items: Vec<ItemSpec> = Vec::new();
    let mut next_id = 1u128;
    let mut add = |def: &mut WorldDefinition, items: &mut Vec<ItemSpec>, rng: &mut Rng, near: Option<Vec3>| {
        let dir = near.unwrap_or_else(|| random_dir(rng));
        // Reaches of every class, and the ends of the range.
        let reach_m = match rng.next_u64() % 8 {
            0 => 0.0,
            1 => 40_000.0,
            _ => 40.0 * libm::pow(1000.0, rng.f()),
        };
        let layer = if rng.next_u64() % 3 == 0 { "other" } else { "stamps" };
        let it = spec(next_id, layer, dir, reach_m, (rng.next_u64() % 4) as i32);
        next_id += 1;
        def.put_item_spec(BodyId::PLANET, it.clone()).unwrap();
        items.push(it);
    };
    for _ in 0..600 {
        add(&mut def, &mut items, &mut rng, None);
    }
    // A cluster, so that cells hold many.
    let centre = unit(20.0, 30.0);
    for _ in 0..300 {
        let d = (centre + Vec3::new(rng.range(-0.002, 0.002), rng.range(-0.002, 0.002), rng.range(-0.002, 0.002))).normalized();
        add(&mut def, &mut items, &mut rng, Some(d));
    }
    let mut check = |def: &WorldDefinition, items: &[ItemSpec], rng: &mut Rng, what: &str| {
        for q in 0..40 {
            let dir = if q % 4 == 0 { centre } else { random_dir(rng) };
            let angle = match q % 5 {
                0 => 0.0,
                1 => rng.range(0.0, 2.0e-5),
                2 => rng.range(0.0, 2.0e-3),
                3 => rng.range(0.0, 0.2),
                _ => core::f64::consts::PI,
            };
            let cap = Cap { centre: dir, angle };
            for layer in ["stamps", "other"] {
                let got = def.footprint_items_cap(BodyId::PLANET, layer, cap).unwrap();
                assert_eq!(ids(&got), scan(items, layer, cap, R), "{what}: a cap of {angle} rad at {dir:?} in {layer}");
            }
        }
        // The tiles of the footprint query, at the cell level and finer and coarser.
        for q in 0..30 {
            let dir = if q % 3 == 0 { centre } else { random_dir(rng) };
            let tile = TileId::from_direction(&planet_core::TangentWarp, dir, [4u8, 8, 8, 12, 16, 20][q % 6]);
            let halo = [0.0, 0.5, 1.0][q % 3];
            let cap = footprint::tile_cap(&planet_core::TangentWarp, tile, halo);
            for layer in ["stamps", "other"] {
                let got = def.footprint_items(BodyId::PLANET, layer, tile, halo).unwrap();
                assert_eq!(ids(&got), scan(items, layer, cap, R), "{what}: tile {tile:?} halo {halo} in {layer}");
            }
        }
    };
    check(&def, &items, &mut rng, "after the puts");
    // Replacing an item (same id, another place and reach), removing some, and adding more.
    for k in 0..60 {
        let id = items[(rng.next_u64() as usize) % items.len()].id;
        let dir = random_dir(&mut rng);
        let reach_m = 40.0 * libm::pow(1000.0, rng.f());
        let replaced = spec(id, "stamps", dir, reach_m, (k % 3) as i32);
        def.put_item_spec(BodyId::PLANET, replaced.clone()).unwrap();
        let at = items.iter().position(|i| i.id == id).unwrap();
        items[at] = replaced;
    }
    check(&def, &items, &mut rng, "after replacing");
    for _ in 0..100 {
        let at = (rng.next_u64() as usize) % items.len();
        assert!(def.remove_item(BodyId::PLANET, items.remove(at).id));
    }
    check(&def, &items, &mut rng, "after removing");
    // A change of the radius re-files everything (the cell levels change with it).
    def.set_body_radius(BodyId::PLANET, 3.0e6);
    let small: Vec<ItemSpec> = items.iter().filter(|i| def.item_anchor(BodyId::PLANET, i.id).is_some()).cloned().collect();
    assert_eq!(small.len(), items.len(), "the items stay");
    for q in 0..40 {
        let cap = Cap { centre: random_dir(&mut rng), angle: [0.0, 1e-4, 1e-2, 0.5][q % 4] };
        let got = def.footprint_items_cap(BodyId::PLANET, "stamps", cap).unwrap();
        assert_eq!(ids(&got), scan(&small, "stamps", cap, 3.0e6), "after a smaller radius: {cap:?}");
    }
}

// spec: WDEF-014
#[test]
fn a_query_looks_at_the_items_near_its_cap_not_at_all_of_them() {
    // 10,000 items of 300 m reach on a grid 2 km apart over 200 km, and a few of large reach.
    let mut def = WorldDefinition::single_planet(R, 3);
    let centre = unit(20.0, 30.0);
    let east = Vec3::new(0.0, 0.0, 1.0).cross(centre).normalized();
    let north = centre.cross(east).normalized();
    let mut id = 1u128;
    for i in 0..100 {
        for j in 0..100 {
            let d = (centre * R + east * (f64::from(i) * 2_000.0 - 100_000.0) + north * (f64::from(j) * 2_000.0 - 100_000.0)).normalized();
            def.put_item_spec(BodyId::PLANET, spec(id, "stamps", d, 300.0, 0)).unwrap();
            id += 1;
        }
    }
    for k in 0..5 {
        let d = (centre * R + east * (f64::from(k) * 30_000.0)).normalized();
        def.put_item_spec(BodyId::PLANET, spec(id, "stamps", d, 30_000.0, 0)).unwrap();
        id += 1;
    }
    let total = 10_005;
    // A point query in the middle: the items of a few cells, a hundred or so of 10,000.
    let mut visited = 0usize;
    let cap = Cap { centre, angle: 0.0 };
    let candidates = def.candidate_ids(BodyId::PLANET, cap, Some(&mut visited));
    assert!(visited < 400, "{visited} entries looked at of {total}");
    assert!(candidates.len() <= visited);
    // A cap of the size of a tile of level 14 (about 600 m) the same; a cap of 20 km is wider and takes more, never more than all.
    for (angle_m, bound) in [(600.0, 500usize), (5_000.0, 1_500), (20_000.0, 10_005)] {
        let mut visited = 0usize;
        let _ = def.candidate_ids(BodyId::PLANET, Cap { centre, angle: angle_m / R }, Some(&mut visited));
        assert!(visited <= bound, "a cap of {angle_m} m looked at {visited} entries, bound {bound}");
    }
    // Over empty ground, far from every item: only the few of large reach are looked at (their class has few cells).
    let mut far = 0usize;
    let _ = def.candidate_ids(BodyId::PLANET, Cap { centre: unit(-40.0, 150.0), angle: 0.0 }, Some(&mut far));
    assert!(far <= 10, "{far} entries looked at over empty ground");
    assert_eq!(cell_of(R, centre).level(), planet_world_def::cells::cell_level(R));
}
