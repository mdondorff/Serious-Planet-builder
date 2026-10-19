//! Region summaries (WDEF-008, ADR 0066 point 3): the Merkle quadtree and the node sets coarse tiles read.

use std::collections::BTreeSet;

use planet_core::{TangentWarp, TileId, Vec3};
use planet_world_def::cells::{cell_level, cell_of, cells_covering, with_margin};
use planet_world_def::summary::{cell_hash128, cells_under, empty_hash128, margin_rings, node_hash128};
use planet_world_def::{BodyId, GenContext, ItemSpec, ReadRecord, WorldDefinition, DEFAULT_ITEM_LAYER};

const R: f64 = 6_371_000.0;

fn spec(id: u64, dir: Vec3, reach_m: f64) -> ItemSpec {
    ItemSpec { id, layer: DEFAULT_ITEM_LAYER.into(), priority: 0, dir, content_hash: id * 7 + 1, reach_m, max_delta_m: 10.0 }
}

fn dir(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z).normalized()
}

/// A small deterministic generator (SplitMix64).
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

fn coarse_tiles() -> Vec<TileId> {
    let mut tiles = Vec::new();
    for d in [dir(1.0, 0.2, 0.1), dir(1.0, 0.99, 0.99), dir(1.0, 0.0, 0.0), dir(-0.3, 1.0, 0.4), dir(0.2, -0.1, -1.0)] {
        for level in [3u8, 4, 5] {
            tiles.push(TileId::from_direction(&TangentWarp, d, level));
        }
    }
    tiles
}

// spec: WDEF-008
#[test]
fn the_nodes_of_a_coarse_tiles_read_cover_exactly_its_cells_and_the_margin_ring_with_no_overlap_and_no_mergeable_siblings() {
    let d = WorldDefinition::single_planet(R, 7);
    let level = cell_level(R);
    assert_eq!(level, 8);
    for tile in coarse_tiles() {
        for halo in [0.0, 0.25] {
            let nodes = d.summary_cover(BodyId::PLANET, tile, halo).unwrap().expect("a coarse tile");
            let rings = margin_rings(tile, halo, R, level);
            let covered: Vec<TileId> = nodes.iter().flat_map(|n| cells_under(*n, level)).collect();
            let as_set: BTreeSet<u64> = covered.iter().map(|c| c.raw()).collect();
            assert_eq!(as_set.len(), covered.len(), "tile {tile:?}: the nodes overlap");
            match rings {
                Some(rings) => {
                    let own = cells_covering(tile, level, 1 << 20).unwrap();
                    let want: BTreeSet<u64> = with_margin(&own, rings).iter().map(|c| c.raw()).collect();
                    assert_eq!(
                        as_set, want,
                        "tile {tile:?}, halo {halo}: the nodes cover the tile's cells and {rings} rings, no more, no less"
                    );
                }
                None => assert_eq!(nodes.len(), 6, "tile {tile:?}: a tile that needs too many rings reads the six face roots"),
            }
            // Smallest: no node has its three siblings and its parent absent.
            let present: BTreeSet<u64> = nodes.iter().map(|n| n.raw()).collect();
            for n in &nodes {
                if let Some(p) = n.parent() {
                    let all = p.children().unwrap().iter().all(|c| present.contains(&c.raw()));
                    assert!(!all, "tile {tile:?}: the four children of {p:?} are all in the set and should have been merged");
                }
            }
        }
    }
}

// spec: WDEF-008
#[test]
fn an_edit_inside_the_covered_cells_changes_a_coarse_tiles_read_and_one_far_beyond_the_margin_changes_none() {
    let at = dir(1.0, 0.2, 0.1);
    let tile = TileId::from_direction(&TangentWarp, at, 4);
    let reads = |d: &WorldDefinition| {
        let ctx = GenContext::new(d);
        // The halo of a tile that samples its border and one sample past it (32 cells).
        ctx.query_footprint(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, 1.0 / 32.0).unwrap();
        ctx.read_records()
    };
    let plain = WorldDefinition::single_planet(R, 7);
    assert!(
        reads(&plain).iter().all(|r| !matches!(r, ReadRecord::Footprint { .. })),
        "a coarse tile records summary nodes, not the footprint"
    );
    assert!(reads(&plain).iter().any(|r| matches!(r, ReadRecord::SummaryNode { .. })));
    let before = reads(&plain);
    // Inside the tile.
    let mut inside = plain.clone();
    inside.put_item_spec(BodyId::PLANET, spec(1, at, 100.0)).unwrap();
    assert_ne!(reads(&inside), before, "an item under the tile re-keys it");
    // Moved within its cell, edited, removed: each changes the read; removing restores it.
    let mut moved = inside.clone();
    moved.put_item_spec(BodyId::PLANET, spec(1, dir(1.0, 0.2000001, 0.1), 100.0)).unwrap();
    assert_ne!(reads(&moved), reads(&inside));
    inside.remove_item(BodyId::PLANET, 1);
    assert_eq!(reads(&inside), before, "removing the item restores the read exactly");
    // The other side of the planet changes nothing.
    let mut far = plain.clone();
    far.put_item_spec(BodyId::PLANET, spec(2, dir(-1.0, -0.3, -0.2), 100.0)).unwrap();
    assert_eq!(reads(&far), before, "an item far beyond the margin changes no read of the tile");
}

// spec: WDEF-008
#[test]
fn an_edit_rewrites_one_node_per_level_and_every_node_equals_a_rebuild_from_the_items() {
    let level = usize::from(cell_level(R));
    let mut d = WorldDefinition::single_planet(R, 7);
    let n0 = d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER);
    assert!(n0.is_empty(), "no items, no stored nodes");
    d.put_item_spec(BodyId::PLANET, spec(1, dir(1.0, 0.2, 0.1), 100.0)).unwrap();
    let n1 = d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER);
    assert_eq!(n1.len(), level + 1, "the cell and one node per level above it, up to the face root");
    let changed = |a: &std::collections::BTreeMap<(u8, u64), [u8; 16]>, b: &std::collections::BTreeMap<(u8, u64), [u8; 16]>| {
        a.iter().filter(|(k, v)| b.get(k) != Some(v)).count() + b.keys().filter(|k| !a.contains_key(k)).count()
    };
    // A second item in the same cell and one in another face each rewrite exactly one path.
    d.put_item_spec(BodyId::PLANET, spec(2, dir(1.0, 0.2, 0.1), 50.0)).unwrap();
    let n2 = d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER);
    assert_eq!(changed(&n1, &n2), level + 1, "the same cell: the leaf and its ancestors change, nothing else");
    d.put_item_spec(BodyId::PLANET, spec(3, dir(-1.0, 0.3, 0.2), 50.0)).unwrap();
    let n3 = d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER);
    assert_eq!(changed(&n2, &n3), level + 1, "another face: a whole new path of one node per level");
    d.remove_item(BodyId::PLANET, 3);
    d.remove_item(BodyId::PLANET, 2);
    assert_eq!(d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER), n1, "removing restores the earlier tree");
    d.remove_item(BodyId::PLANET, 1);
    assert!(d.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER).is_empty(), "an empty tree stores nothing");

    // Any sequence of puts, moves and removes ends in the tree a rebuild from the final items gives.
    let mut rng = Rng(21);
    let mut live: std::collections::BTreeMap<u64, (Vec3, f64)> = Default::default();
    let mut w = WorldDefinition::single_planet(R, 7);
    for step in 0..400u64 {
        let id = rng.next() % 40;
        if rng.next().is_multiple_of(4) {
            w.remove_item(BodyId::PLANET, id);
            live.remove(&id);
        } else {
            let (at, reach) = (rng.dir(), rng.unit() * 40_000.0);
            w.put_item_spec(BodyId::PLANET, spec(id, at, reach)).unwrap();
            live.insert(id, (at, reach));
        }
        if step % 50 == 49 {
            // Setting the radius again rebuilds the cell index and the tree from the items alone.
            let mut fresh = w.clone();
            fresh.set_body_radius(BodyId::PLANET, R);
            assert_eq!(
                w.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER),
                fresh.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER),
                "step {step}"
            );
        }
    }
    // A radius edit across a cell-level boundary rebuilds the tree for the new level; restoring the radius restores it.
    let before = w.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER);
    w.set_body_radius(BodyId::PLANET, R * 3.0);
    assert_ne!(cell_level(R * 3.0), cell_level(R));
    let big = w.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER);
    assert!(big.keys().all(|(l, _)| *l <= cell_level(R * 3.0)));
    w.set_body_radius(BodyId::PLANET, R);
    assert_eq!(w.summary_nodes(BodyId::PLANET, DEFAULT_ITEM_LAYER), before);
}

// spec: WDEF-008
#[test]
fn an_empty_subtree_hashes_differently_at_every_level_and_the_hashes_are_pinned() {
    let all: BTreeSet<[u8; 16]> = (0..=8u8).map(|l| empty_hash128(l, 8)).collect();
    assert_eq!(all.len(), 9, "the level is inside an empty subtree's hash");
    assert_eq!(empty_hash128(8, 8), cell_hash128(&[]), "an empty cell is the cell hash of no rows");
    assert_eq!(empty_hash128(7, 8), node_hash128(7, &[cell_hash128(&[]); 4]));
    assert_ne!(empty_hash128(0, 8), empty_hash128(0, 9), "the cell level is inside it too");
    let hex = |b: [u8; 16]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(hex(empty_hash128(8, 8)), KNOWN_EMPTY_CELL);
    assert_eq!(hex(empty_hash128(0, 8)), KNOWN_EMPTY_ROOT);
}

// spec: WDEF-008
#[test]
fn the_items_a_coarse_tile_receives_have_their_anchors_in_the_cells_its_nodes_cover() {
    let mut rng = Rng(77);
    let mut d = WorldDefinition::single_planet(R, 7);
    let tiles = coarse_tiles();
    let mut id = 0;
    for tile in &tiles {
        let c = tile.center_direction(&TangentWarp);
        for _ in 0..15 {
            id += 1;
            let offset = rng.dir() * (900_000.0 * rng.unit() / R);
            let at = (c + offset - c * c.dot(offset)).normalized();
            d.put_item_spec(BodyId::PLANET, spec(id, at, rng.unit() * 40_000.0)).unwrap();
        }
    }
    let level = cell_level(R);
    let mut returned = 0;
    for tile in tiles {
        for halo in [0.0, 0.25] {
            let covered: BTreeSet<u64> = d
                .summary_cover(BodyId::PLANET, tile, halo)
                .unwrap()
                .unwrap()
                .iter()
                .flat_map(|n| cells_under(*n, level))
                .map(|c| c.raw())
                .collect();
            for item in d.footprint_items(BodyId::PLANET, DEFAULT_ITEM_LAYER, tile, halo).unwrap() {
                let (anchor, _) = d.item_anchor(BodyId::PLANET, item.id).unwrap();
                returned += 1;
                assert!(
                    covered.contains(&cell_of(R, anchor).raw()),
                    "tile {tile:?}, halo {halo}: item {} is returned but its cell is not read",
                    item.id
                );
            }
        }
    }
    assert!(returned > 100, "the setup must return items, {returned}");
}

const KNOWN_EMPTY_CELL: &str = "bc60602dff5649e21a97e05feaf28b78";
const KNOWN_EMPTY_ROOT: &str = "552b79f2eedcdad6d9225b52ed58b97d";
