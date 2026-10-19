//! The footprint query and item reach (WDEF-007, ADR 0066 point 2).

use planet_core::cube::face_to_direction;
use planet_core::{TangentWarp, TileId, Vec3};
use planet_world_def::footprint::{angle_between, tile_cap, touches};
use planet_world_def::{BodyId, GenContext, ItemError, ItemSpec, ReadRecord, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

/// A small deterministic generator (SplitMix64); the tests need spread, not quality.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn dir(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(2.0 * self.unit() - 1.0, 2.0 * self.unit() - 1.0, 2.0 * self.unit() - 1.0);
            if v.length() > 0.1 && v.length() < 1.0 {
                return v.normalized();
            }
        }
    }
}

fn spec(id: u64, dir: Vec3, reach_m: f64) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir, content_hash: id * 7 + 1, reach_m, max_delta_m: 10.0 }
}

/// The footprint of `tile` and its halo as sample points on the tile grid, ignoring the cap: independent of `tile_cap`.
fn halo_samples(tile: TileId, halo: f64) -> Vec<Vec3> {
    let (s0, s1, t0, t1) = tile.bounds();
    let (hs, ht) = (halo * (s1 - s0), halo * (t1 - t0));
    let n = 8;
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            let s = s0 - hs + (s1 - s0 + 2.0 * hs) * f64::from(i) / f64::from(n);
            let t = t0 - ht + (t1 - t0 + 2.0 * ht) * f64::from(j) / f64::from(n);
            if s.abs() <= 1.0 && t.abs() <= 1.0 {
                out.push(face_to_direction(&TangentWarp, tile.face(), s, t));
            }
        }
    }
    out
}

// spec: WDEF-007
#[test]
fn the_footprint_query_returns_every_item_whose_reach_reaches_a_point_of_the_tile_and_its_halo() {
    // No false negative: any item with a sample point of the tile and its halo within its reach is returned. Tiles at the
    // coarse levels (the whole-body scan), at the cell level and finer, around face corners and edges.
    let mut rng = Rng(5);
    let mut tiles = Vec::new();
    for level in [0u8, 2, 4, 6, 8, 10, 12, 15] {
        for _ in 0..6 {
            tiles.push(TileId::from_direction(&TangentWarp, rng.dir(), level));
        }
    }
    // Face corners and edges, where the warp and the cell neighbours are worst.
    for dir in [Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, -1.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 1.0)] {
        for level in [6u8, 8, 9, 12] {
            tiles.push(TileId::from_direction(&TangentWarp, dir.normalized(), level));
        }
    }
    // Items around every tile, near enough that many reach it: up to 60 km from its centre, with reaches up to the limit.
    let mut d = WorldDefinition::single_planet(R, 7);
    let mut id = 0u64;
    for tile in &tiles {
        let c = tile.center_direction(&TangentWarp);
        for _ in 0..12 {
            id += 1;
            let offset = rng.dir() * (60_000.0 * rng.unit() / R);
            let at = (c + offset - c * c.dot(offset)).normalized();
            d.put_item_spec(BodyId::PLANET, spec(id, at, rng.unit() * 40_000.0)).unwrap();
        }
    }
    let mut checked = 0;
    let mut found = 0;
    for tile in tiles {
        for halo in [0.0, 0.25, 1.0] {
            let got: Vec<u64> = d.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, halo).unwrap().iter().map(|i| i.id).collect();
            let samples = halo_samples(tile, halo);
            for id in 1..=id {
                let (anchor, reach) = item_anchor_and_reach(&d, id);
                let reached = samples.iter().any(|p| R * angle_between(*p, anchor) <= reach);
                checked += 1;
                if reached {
                    found += 1;
                    assert!(got.contains(&id), "item {id} reaches tile {tile:?} (halo {halo}) but the query missed it");
                }
            }
        }
    }
    assert!(found > 20, "the setup must exercise the positive case, {found} of {checked} reached");
}

fn item_anchor_and_reach(d: &WorldDefinition, id: u64) -> (Vec3, f64) {
    d.item_anchor(BodyId::PLANET, id).expect("the item exists")
}

// spec: WDEF-007
#[test]
fn the_result_is_the_same_by_the_cell_index_and_by_a_scan_of_every_item() {
    // Items around every tile (up to 250 km out, reaches up to the limit), tiles from the coarse levels, through the cell level
    // (8), to fine ones, in the middle of faces and at corners and edges; every halo.
    let mut rng = Rng(9);
    let mut tiles = Vec::new();
    for level in 3u8..=14 {
        for _ in 0..3 {
            tiles.push(TileId::from_direction(&TangentWarp, rng.dir(), level));
        }
        for corner in [Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)] {
            tiles.push(TileId::from_direction(&TangentWarp, corner.normalized(), level));
        }
    }
    let mut d = WorldDefinition::single_planet(R, 7);
    let mut count = 0u64;
    for tile in &tiles {
        let c = tile.center_direction(&TangentWarp);
        for _ in 0..10 {
            count += 1;
            let offset = rng.dir() * (250_000.0 * rng.unit() / R);
            let at = (c + offset - c * c.dot(offset)).normalized();
            d.put_item_spec(BodyId::PLANET, spec(count, at, rng.unit() * 40_000.0)).unwrap();
        }
    }
    let mut positives = 0;
    for tile in tiles {
        for halo in [0.0, 0.25, 1.0] {
            let cap = tile_cap(&TangentWarp, tile, halo);
            let want: Vec<u64> = (1..=count)
                .filter(|id| {
                    let (anchor, reach) = item_anchor_and_reach(&d, *id);
                    touches(cap, anchor, reach, R)
                })
                .collect();
            let mut got: Vec<u64> =
                d.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, halo).unwrap().iter().map(|i| i.id).collect();
            got.sort_unstable();
            positives += want.len();
            assert_eq!(got, want, "tile {tile:?}, halo {halo}: the index and the scan of every item must give one set");
        }
    }
    assert!(positives > 500, "the setup must return items, {positives}");
}

// spec: WDEF-007
#[test]
fn an_item_out_of_reach_of_a_tile_leaves_its_read_unchanged_and_one_in_reach_changes_it() {
    let tile_dir = Vec3::new(1.0, 0.2, 0.1).normalized();
    let tile = TileId::from_direction(&TangentWarp, tile_dir, 12);
    let reads = |d: &WorldDefinition| {
        let ctx = GenContext::new(d);
        ctx.query_footprint(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 0.25).unwrap();
        ctx.read_records()
    };
    let plain = WorldDefinition::single_planet(R, 7);
    // 30 km east of the tile centre with a reach of 100 m: far out of reach, yet in the margin ring of cells.
    let far = rotate_about_z(tile_dir, 30_000.0 / R);
    let mut out_of_reach = plain.clone();
    out_of_reach.put_item_spec(BodyId::PLANET, spec(1, far, 100.0)).unwrap();
    assert_eq!(reads(&plain), reads(&out_of_reach), "an item out of reach changes no read");
    // The same place with a reach that covers the distance reaches the tile.
    let mut in_reach = plain.clone();
    in_reach.put_item_spec(BodyId::PLANET, spec(1, far, 39_000.0)).unwrap();
    assert_ne!(reads(&plain), reads(&in_reach), "an item in reach changes the read");
    // Removing it restores the read exactly.
    in_reach.remove_item(BodyId::PLANET, 1);
    assert_eq!(reads(&plain), reads(&in_reach));
    // The record names the query.
    assert!(reads(&plain).iter().any(|r| matches!(r, ReadRecord::Footprint { .. })));
}

fn rotate_about_z(v: Vec3, angle: f64) -> Vec3 {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

// spec: WDEF-007
#[test]
fn a_reach_over_one_cell_edge_or_a_bad_number_is_refused_and_changes_nothing() {
    let mut d = WorldDefinition::single_planet(R, 7);
    d.put_item_spec(BodyId::PLANET, spec(1, Vec3::new(1.0, 0.0, 0.0), 100.0)).unwrap();
    let before = d.clone();
    let over = d.put_item_spec(BodyId::PLANET, spec(1, Vec3::new(0.0, 1.0, 0.0), 40_000.5));
    assert!(matches!(over, Err(ItemError::Reach { id: 1, limit_m, .. }) if limit_m == 40_000.0), "{over:?}");
    assert!(over.unwrap_err().to_string().contains("item 1"), "the refusal names the item");
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(d.put_item_spec(BodyId::PLANET, spec(1, Vec3::new(0.0, 1.0, 0.0), bad)), Err(ItemError::Reach { .. })),
            "reach {bad}"
        );
    }
    let mut s = spec(1, Vec3::new(0.0, 1.0, 0.0), 10.0);
    s.max_delta_m = -1.0;
    assert!(matches!(d.put_item_spec(BodyId::PLANET, s), Err(ItemError::MaxDelta { .. })));
    let mut s = spec(1, Vec3::new(0.0, 1.0, 0.0), 10.0);
    s.layer = String::new();
    assert!(matches!(d.put_item_spec(BodyId::PLANET, s), Err(ItemError::Layer { .. })));
    assert_eq!(d, before, "refused items change nothing, the item of the same id stays");
}

// spec: WDEF-007
#[test]
fn the_result_is_in_application_order_and_every_field_of_an_item_is_in_the_read() {
    let dir = Vec3::new(1.0, 0.2, 0.1).normalized();
    let tile = TileId::from_direction(&TangentWarp, dir, 12);
    let read = |d: &WorldDefinition| {
        let ctx = GenContext::new(d);
        ctx.query_footprint(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 0.0).unwrap();
        ctx.read_set_hash()
    };
    let item = |id: u64, priority: i32| ItemSpec { priority, ..spec(id, dir, 500.0) };
    let mut a = WorldDefinition::single_planet(R, 7);
    a.put_item_spec(BodyId::PLANET, item(1, 5)).unwrap();
    a.put_item_spec(BodyId::PLANET, item(2, 1)).unwrap();
    let mut b = WorldDefinition::single_planet(R, 7);
    b.put_item_spec(BodyId::PLANET, item(2, 1)).unwrap();
    b.put_item_spec(BodyId::PLANET, item(1, 5)).unwrap();
    assert_eq!(read(&a), read(&b), "insertion order does not matter");
    let order: Vec<u64> = a.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 0.0).unwrap().iter().map(|i| i.id).collect();
    assert_eq!(order, vec![2, 1], "lower priority first");
    // Each field changes the read.
    let base = read(&a);
    let mut changed = Vec::new();
    for edit in 0..5 {
        let mut c = a.clone();
        let mut s = item(1, 5);
        match edit {
            0 => s.reach_m = 501.0,
            1 => s.max_delta_m = 11.0,
            2 => s.priority = 6,
            3 => s.content_hash += 1,
            _ => s.dir = rotate_about_z(dir, 1e-9),
        }
        c.put_item_spec(BodyId::PLANET, s).unwrap();
        changed.push(read(&c) != base);
    }
    assert_eq!(changed, vec![true; 5], "reach, bound, priority, content and direction are each in the read");
    // Another layer is another query.
    let mut other = a.clone();
    other.put_item_spec(BodyId::PLANET, ItemSpec { layer: "other".into(), ..spec(3, dir, 500.0) }).unwrap();
    assert_eq!(read(&a), read(&other), "an item of another layer is not in this layer's read");
}
