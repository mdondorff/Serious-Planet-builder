//! Tier A, STRM-083 (TEST-111): the layer source over a commit of the new chain, and the swap of the drawn commit (D17). A small face is baked with `l_writer` (the fixture of that test file); the source
//! returns the stored cores with no fill, reads a tile the commit does not name as no land, refuses a commit that names a record the store lacks, decodes a tile once, and the drawn commit changes only
//! when every part of the new one is ready.

use std::sync::Arc;

use crate::l_writer::{bake, base, basin, lakes, spec, tiles, world, zero, Dir, Feed, RES, T};
use planet_cache::l_layer::{l_change_range, CommitLayer, PairedLayer};
use planet_cache::l_writer::{read_l_tile, Commit};
use planet_cache::pack::TileStore;
use planet_core::Face;
use planet_generators::bake::k7::k7_tile;
use planet_generators::bake::tile::{TileId as PassTile, QUANT_STEP_M};
use planet_generators::erosion_layer::LayerSource;

fn tile(ti: u32, tj: u32) -> PassTile {
    PassTile { face: Face(2), ti, tj }
}

fn layer_of(dir: &Dir, w: &planet_cache::global_store::GlobalWorld, c: Commit, capacity: usize) -> CommitLayer {
    let store = Arc::new(TileStore::open(dir.path(), w.params()).unwrap());
    CommitLayer::with_commit(store, Arc::new(world()), c, capacity, l_change_range())
}

// spec: STRM-083, TEST-111
#[test]
fn t1_the_source_returns_the_stored_core_with_no_fill_and_t3_the_surface_is_b_plus_l_within_the_step() {
    let (w, d) = (world(), Dir::new("t1"));
    let c = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let layer = CommitLayer::open(d.path(), Arc::new(world()), 4).expect("opens through the commit file");
    assert_eq!(layer.commit(), &c);
    assert_eq!(layer.grid().tile, T);
    assert_eq!(layer.id(), c.id, "the layer's identity is the commit's");
    let store = TileStore::open(d.path(), w.params()).unwrap();
    let stored = read_l_tile(&store, &w, RES, &spec(7), &c, tile(3, 3)).unwrap().unwrap();
    let t = layer.tile(Face(2), 3, 3).unwrap().expect("a tile of the commit");
    for j in 0..T {
        for i in 0..T {
            assert_eq!(t.cell(i, j), stored.core[j * T + i], "cell ({i}, {j}) is the stored change, nothing added back");
        }
    }
    // T3: B + L from the source equals K7's own surface z within half a quantisation step at the cell centres of the core.
    let g = spec(7).grid;
    let r = k7_tile(&g, tile(3, 3), &base, &zero, &[], &spec(7).k7, 1);
    let (w_, h) = (g.window(), g.halo);
    for j in 0..T {
        for i in 0..T {
            let c = (j + h) * w_ + i + h;
            let surface = r.b[c] + f64::from(t.cell(i, j));
            assert!((surface - r.z[c]).abs() <= f64::from(QUANT_STEP_M) / 2.0 + 1e-6, "cell ({i}, {j}): {surface} against {}", r.z[c]);
        }
    }
}

// spec: STRM-083, TEST-111
#[test]
fn t2_a_tile_the_commit_does_not_name_is_no_land_and_one_it_names_but_the_store_lacks_is_an_error() {
    let (w, d) = (world(), Dir::new("t2"));
    let c = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let layer = layer_of(&d, &w, c.clone(), 4);
    assert!(layer.tile(Face(2), 6, 6).unwrap().is_none(), "outside the commit: no land");
    assert!(layer.tile(Face(4), 0, 0).unwrap().is_none());
    // A commit that names a tile under a key the store does not hold: the record is missing.
    let mut bad = c.clone();
    let key = bad.tiles.get_mut(&(2, 3, 3)).unwrap();
    key.0[0] ^= 0xff;
    let layer = layer_of(&d, &w, bad, 4);
    let e = layer.tile(Face(2), 3, 3).err().expect("refused");
    assert!(e.contains("names key") || e.contains("holds no record"), "{e}");
    // A folder with no commit is refused naming the file.
    let empty = Dir::new("t2-empty");
    std::fs::create_dir_all(empty.path()).unwrap();
    let e = CommitLayer::open(empty.path(), Arc::new(world()), 4).err().expect("refused");
    assert!(e.contains("layer-commit.txt"), "{e}");
}

// spec: STRM-083, TEST-111
#[test]
fn t4_a_tile_is_decoded_once_while_it_stays_in_the_cache_and_the_cache_is_bounded() {
    let (w, d) = (world(), Dir::new("t4"));
    let c = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let layer = layer_of(&d, &w, c, 2);
    let a1 = layer.tile(Face(2), 3, 3).unwrap().unwrap();
    let a2 = layer.tile(Face(2), 3, 3).unwrap().unwrap();
    assert!(Arc::ptr_eq(&a1, &a2));
    assert_eq!(layer.decodes(), 1, "one decode for two reads");
    layer.tile(Face(2), 2, 2).unwrap();
    layer.tile(Face(2), 4, 4).unwrap();
    assert_eq!(layer.decodes(), 3);
    // Capacity 2: the least recently used tile (3, 3) was evicted by the third tile; reading it again decodes again.
    layer.tile(Face(2), 3, 3).unwrap();
    assert_eq!(layer.decodes(), 4);
    // The whole-planet reader of the layer accepts it: a finite range that holds 0.
    let el = planet_generators::erosion_layer::ErosionLayer::new(Arc::new(layer_of(&d, &w, Commit::read(d.path()).unwrap().unwrap(), 4)))
        .unwrap();
    assert!(el.global_range().0 <= 0.0 && el.global_range().1 >= 0.0);
}

// spec: STRM-083, TEST-111
#[test]
fn d17_the_drawn_commit_changes_only_when_every_part_of_the_new_one_is_ready() {
    let (w, d) = (world(), Dir::new("d17"));
    // Commit A (base 7) is drawn. Commit B (base 8) is baked into the same store: its records are under other keys.
    let a = bake(&d, &w, spec(7), &Feed(true), &lakes(), 1).unwrap();
    let paired = PairedLayer::new(layer_of(&d, &w, a.clone(), 8));
    let b = bake(&d, &w, spec(8), &Feed(true), &lakes(), 1).unwrap();
    assert_ne!(a.id, b.id);
    let needed = tiles();
    // A frame in flight holds the drawn commit: it reads A's tiles, whatever happens next.
    let frame = paired.drawn();
    let before = frame.tile(Face(2), 3, 3).unwrap().unwrap();
    // B with a basin table not yet in the store (a key the store lacks): not ready, A stays drawn.
    let mut no_table = b.clone();
    no_table.tables[3].0[0] ^= 0xff;
    assert!(!paired.try_swap(layer_of(&d, &w, no_table, 8), &needed).unwrap(), "the lakes of the new commit are not ready");
    assert_eq!(paired.drawn().commit().id, a.id);
    // B with an L tile whose record is not in the store yet: not ready, A stays drawn.
    let mut no_tile = b.clone();
    no_tile.tiles.get_mut(&(2, 3, 3)).unwrap().0[0] ^= 0xff;
    assert!(!paired.try_swap(layer_of(&d, &w, no_tile, 8), &needed).unwrap(), "an L tile of the new commit is not ready");
    assert_eq!(paired.drawn().commit().id, a.id);
    assert_eq!(paired.drawn().id(), a.id, "a frame keeps reading A");
    // B complete: swapped, and the frame in flight still holds A's tile.
    assert!(paired.try_swap(layer_of(&d, &w, b.clone(), 8), &needed).unwrap());
    assert_eq!(paired.drawn().commit().id, b.id);
    let after = paired.drawn().tile(Face(2), 3, 3).unwrap().unwrap();
    assert!(Arc::ptr_eq(&before, &frame.tile(Face(2), 3, 3).unwrap().unwrap()), "the frame in flight still reads commit A");
    assert!(!Arc::ptr_eq(&before, &after), "the next frame reads commit B");
    assert_eq!(frame.commit().id, a.id);
    // The tiles of one commit come only from its keys: A's layer, read now with B's records in the store, still gives A's keys.
    let store = TileStore::open(d.path(), w.params()).unwrap();
    assert!(read_l_tile(&store, &w, RES, &spec(8), &a, tile(3, 3)).is_err(), "A's commit read with B's spec is refused, never mixed");
    let _ = basin();
}
