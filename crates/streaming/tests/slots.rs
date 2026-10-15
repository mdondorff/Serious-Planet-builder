//! STRM-006, STRM-007: pooled fixed-size slots on fakes. The slot accounting (allocated plus retiring plus free equal
//! capacity), reuse only after the frames in flight retire, the share as a ceiling counted in slots, and the streamer's
//! eviction order under a hard slot capacity. No device.

use planet_core::{Face, TileId};
use planet_streaming::budget::{FIXED_RESERVE_BYTES, GIB, VERTEX_POOL_SLOTS};
use planet_streaming::{
    FakeClock, ManualSpawner, PoolConfig, PoolKind, SlotPool, SlotPools, Spawner, StreamConfig, Streamer, Tier, TileSource, Wants,
};
use std::sync::Arc;

fn id(f: u8, l: u8, x: u32, y: u32) -> TileId {
    TileId::new(Face(f), l, x, y).unwrap()
}

/// A small deterministic generator for the churn tests (not `libm`, not a platform generator).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

// spec: STRM-006
#[test]
fn allocated_plus_retiring_plus_free_equal_capacity_under_churn() {
    let mut pool = SlotPool::new(40, 3);
    let (mut held, mut rng) = (Vec::new(), Lcg(7));
    for frame in 0..4000u64 {
        for _ in 0..(rng.next() % 6) {
            match rng.next() % 3 {
                0 | 1 => {
                    if let Some(s) = pool.acquire(frame) {
                        assert!(!held.contains(&s), "slot {s} handed out twice");
                        held.push(s);
                    }
                }
                _ => {
                    if !held.is_empty() {
                        let s = held.swap_remove((rng.next() as usize) % held.len());
                        assert!(pool.release(s, frame));
                        assert!(!pool.release(s, frame), "a double release is refused");
                    }
                }
            }
            pool.check().unwrap();
            assert_eq!(pool.in_use() + pool.retiring() + pool.free(), pool.capacity());
            assert_eq!(pool.in_use() as usize, held.len());
        }
    }
}

// spec: STRM-006, STRM-007
#[test]
fn a_released_slot_is_reused_only_after_the_frames_in_flight_retire() {
    let mut pool = SlotPool::new(2, 2);
    let (a, b) = (pool.acquire(10).unwrap(), pool.acquire(10).unwrap());
    assert_eq!((a, b), (0, 1), "lowest slot first");
    assert_eq!(pool.acquire(10), None, "full");
    pool.release(a, 10);
    assert_eq!(pool.acquire(10), None, "released this frame: a frame in flight may still read it");
    assert_eq!(pool.acquire(11), None, "one frame later: still in flight");
    assert_eq!(pool.free_at(12), 1);
    assert_eq!(pool.acquire(12), Some(a), "two frames in flight have retired");
    // Lowest index first among several returned slots.
    pool.release(b, 12);
    pool.release(a, 12);
    assert_eq!(pool.acquire(14), Some(a));
    pool.check().unwrap();
}

// spec: STRM-006
#[test]
fn the_terrain_share_bounds_the_allocated_bytes_at_both_tiers() {
    // The vertex format at cells 32: 1,221 vertices of 44 bytes; blocks of 256 slots; node records 512 B x 32,768.
    let slot = 1_221 * 44;
    for (tier, share) in [(Tier::Mid, 2 * GIB), (Tier::High, 3 * GIB)] {
        assert_eq!(tier.terrain_share_bytes(), share);
        let c = PoolConfig::fit_share(share, slot, 256, slot, 25_000, 512 * 32_768, 2).unwrap();
        assert!(c.fixed_bytes() <= FIXED_RESERVE_BYTES, "{tier:?}: fixed items {} B", c.fixed_bytes());
        assert!(c.allocated_bytes() <= share, "{tier:?}: {} B allocated against a share of {share} B", c.allocated_bytes());
        // One more block would breach the share: the ceiling is tight to a block.
        assert!(share - FIXED_RESERVE_BYTES - u64::from(c.main_blocks) * c.block_bytes() < c.block_bytes(), "one more block would fit");
        assert!(c.allocated_bytes() < tier.vram_cap_bytes());
        assert_eq!(c.main_slots(), c.main_blocks * 256);
        // Nothing allocated until used; each block adds exactly its bytes.
        assert_eq!(c.allocated_bytes_with(0, false), c.index_bytes + c.node_record_bytes);
        assert_eq!(c.allocated_bytes_with(3, false) - c.allocated_bytes_with(2, false), c.block_bytes());
        assert_eq!(c.blocks_for(0), 0);
        assert_eq!(c.blocks_for(1), 1);
        assert_eq!(c.blocks_for(256), 1);
        assert_eq!(c.blocks_for(257), 2);
    }
    let mid = PoolConfig::fit_share(Tier::Mid.terrain_share_bytes(), slot, 256, slot, 25_000, 512 * 32_768, 2).unwrap();
    let high = PoolConfig::fit_share(Tier::High.terrain_share_bytes(), slot, 256, slot, 25_000, 512 * 32_768, 2).unwrap();
    assert!(high.main_slots() > mid.main_slots());
    assert_eq!(mid.vertex_slots, VERTEX_POOL_SLOTS);
}

// spec: STRM-006
#[test]
fn a_share_that_cannot_hold_a_block_or_the_fixed_items_is_refused() {
    let slot = 1_221 * 44;
    assert!(PoolConfig::fit_share(GIB / 16, slot, 256, slot, 25_000, 512 * 32_768, 2).is_err(), "below the fixed reserve");
    assert!(PoolConfig::fit_share(FIXED_RESERVE_BYTES + slot, slot, 256, slot, 25_000, 512 * 32_768, 2).is_err(), "no whole block");
    // A big vertex slot first shrinks the small pool (never under 128 slots), then drops it; fixed items that stay over their
    // reserve (here the node records) are refused.
    let shrunk = PoolConfig::fit_share(2 * GIB, slot, 256, 600_000, 25_000, 512 * 32_768, 2).unwrap();
    assert_eq!(shrunk.vertex_slots, 128, "512 slots of 600 kB do not fit the reserve, 256 and 128 are tried");
    let dropped = PoolConfig::fit_share(2 * GIB, slot, 256, 1_000_000, 25_000, 512 * 32_768, 2).unwrap();
    assert_eq!(dropped.vertex_slots, 0, "128 slots of 1 MB do not fit either: one pool for every level");
    assert!(dropped.fixed_bytes() <= FIXED_RESERVE_BYTES);
    assert!(PoolConfig::fit_share(2 * GIB, slot, 256, slot, 25_000, 2 * FIXED_RESERVE_BYTES, 2).is_err(), "fixed items over their reserve");
}

#[derive(Default)]
struct Echo;

impl TileSource<u64> for Echo {
    fn load(&self, tile: TileId) -> Result<u64, String> {
        Ok(tile.raw())
    }
}

fn fixture(config: StreamConfig) -> (Streamer<u64>, Arc<ManualSpawner>) {
    let spawner = Arc::new(ManualSpawner::default());
    let mut s = Streamer::new(Arc::new(Echo), spawner.clone() as Arc<dyn Spawner>, Arc::new(FakeClock::default()), config);
    s.load_roots_blocking().unwrap();
    (s, spawner)
}

fn run_frames(s: &mut Streamer<u64>, sp: &ManualSpawner, w: &Wants, n: usize) {
    for _ in 0..n {
        s.begin_frame_wants(w);
        sp.run(64);
    }
    s.begin_frame_wants(w);
}

fn wants_of(draw: &[TileId], residency: &[(TileId, f64)]) -> Wants {
    Wants { draw: draw.iter().enumerate().map(|(i, t)| (*t, i as f64)).collect(), residency: residency.to_vec(), prefetch: vec![] }
}

// spec: STRM-006, STRM-007
#[test]
fn resident_plus_in_flight_never_exceeds_the_slots_and_no_slot_is_shared() {
    let config = StreamConfig { max_in_flight: 8, capacity: 40, retire_frames: 2, ..Default::default() };
    let (mut s, sp) = fixture(config);
    let mut rng = Lcg(99);
    for _ in 0..300 {
        // Interest jumps about: a random handful of level-3 tiles of face 0..2 as the draw set, R = the same plus neighbours.
        let draw: Vec<TileId> = (0..6).map(|_| id((rng.next() % 3) as u8, 3, (rng.next() % 8) as u32, (rng.next() % 8) as u32)).collect();
        let mut r: Vec<(TileId, f64)> = draw.iter().map(|t| (*t, 1.0)).collect();
        r.extend((0..4).map(|_| (id((rng.next() % 3) as u8, 3, (rng.next() % 8) as u32, (rng.next() % 8) as u32), 5.0)));
        s.begin_frame_wants(&wants_of(&draw, &r));
        sp.run((rng.next() % 5) as usize);
        let st = s.stats();
        assert!(st.resident + st.in_flight <= 40, "slots in use exceed the capacity: {st:?}");
        assert_eq!(st.main_in_use, st.resident + st.in_flight, "every resident or in-flight tile holds exactly one slot");
        assert_eq!(st.main_in_use + st.main_retiring + st.main_free, 40);
        s.pools().check().unwrap();
        let slots: Vec<_> =
            (0..4u8).flat_map(|f| (0..8).flat_map(move |x| (0..8).map(move |y| id(f % 6, 3, x, y)))).filter_map(|t| s.slot_of(t)).collect();
        let mut sorted = slots.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), slots.len(), "two resident tiles share a slot");
    }
}

// spec: STRM-007
#[test]
fn an_evicted_tiles_slot_is_not_given_to_another_tile_within_the_frames_in_flight() {
    // Capacity 8: six roots plus two. a and b fill it; interest moves to c; c needs a's slot.
    for retire in [0u64, 3] {
        let config = StreamConfig { max_in_flight: 4, capacity: 8, retire_frames: retire, ..Default::default() };
        let (mut s, sp) = fixture(config);
        let (a, b, c) = (id(0, 1, 0, 0), id(0, 1, 1, 0), id(0, 1, 0, 1));
        run_frames(&mut s, &sp, &wants_of(&[a, b], &[(a, 1.0), (b, 1.0)]), 4);
        assert!(s.is_resident(a) && s.is_resident(b));
        let slot_a = s.slot_of(a).unwrap();
        let w = wants_of(&[c], &[(c, 1.0)]);
        let (mut evicted_at, mut resident_at) = (None, None);
        for _ in 0..12 {
            s.begin_frame_wants(&w);
            sp.run(8);
            if evicted_at.is_none() && !s.is_resident(a) {
                evicted_at = Some(s.frame());
            }
            if resident_at.is_none() && s.slot_of(c).is_some() {
                resident_at = Some(s.frame());
            }
        }
        let (evicted_at, resident_at) = (evicted_at.expect("a was evicted"), resident_at.expect("c became resident"));
        // a (the oldest unused) was evicted for c; c's slot is a's slot only after the retire delay.
        assert_eq!(s.slot_of(c), Some(slot_a), "the freed slot is the one reused");
        assert!(resident_at >= evicted_at + retire, "retire {retire}: evicted at frame {evicted_at}, c resident at {resident_at}");
    }
}

// spec: STRM-007
#[test]
fn eviction_takes_tiles_outside_r_and_prefetch_before_r_tiles_and_never_the_draw_set() {
    let config = StreamConfig { max_in_flight: 8, capacity: 12, ..Default::default() };
    let (mut s, sp) = fixture(config);
    // Resident: roots (6) + d (draw, level 2 with ancestors) + r1, r2 (R only, r2 farther) + p (prefetch) + x (stale).
    let d = id(0, 2, 0, 0);
    let (r1, r2) = (id(1, 1, 0, 0), id(2, 1, 0, 0));
    let (p, x) = (id(3, 1, 0, 0), id(4, 1, 0, 0));
    let full = Wants { draw: vec![(d, 0.0)], residency: vec![(d, 1.0), (r1, 5.0), (r2, 9.0)], prefetch: vec![(p, 1.0)] };
    run_frames(&mut s, &sp, &Wants { draw: vec![], residency: vec![(x, 1.0)], prefetch: vec![] }, 3);
    run_frames(&mut s, &sp, &full, 6);
    for t in [d, r1, r2, p, x] {
        assert!(s.is_resident(t), "{t:?} resident before the squeeze");
    }
    assert_eq!(s.stats().resident, 6 + 1 + 5, "roots, the parent of d, d, r1, r2, p, x: {:?}", s.stats());
    // Now a new draw tile needs two slots (itself and a missing parent): the stale tile x goes first, then prefetch p.
    let n = id(5, 2, 0, 0);
    let wants = Wants { draw: vec![(n, 0.0), (d, 1.0)], residency: vec![(n, 1.0), (d, 1.0), (r1, 5.0), (r2, 9.0)], prefetch: vec![] };
    run_frames(&mut s, &sp, &wants, 6);
    assert!(s.is_resident(n) && s.is_resident(d), "the draw set is resident");
    assert!(!s.is_resident(x), "a tile outside R goes first");
    assert!(!s.is_resident(p), "a prefetch tile goes next");
    assert!(s.is_resident(r1) && s.is_resident(r2), "R tiles stay while tier 0 victims exist");
    // A second draw tile now needs a slot and only R tiles are left to evict: the farthest (r2) goes, r1 stays.
    let m = id(5, 2, 1, 0);
    let wants = Wants {
        draw: vec![(n, 0.0), (d, 1.0), (m, 2.0)],
        residency: vec![(n, 1.0), (d, 1.0), (m, 2.0), (r1, 5.0), (r2, 9.0)],
        prefetch: vec![],
    };
    run_frames(&mut s, &sp, &wants, 6);
    assert!(s.is_resident(m), "a draw tile displaces R tiles");
    assert!(!s.is_resident(r2) || !s.is_resident(r1), "an R tile was evicted for it");
    if !s.is_resident(r2) {
        assert!(s.is_resident(r1), "the farther R tile goes first");
    }
    assert!(s.is_resident(n) && s.is_resident(d), "the draw set is never evicted");
    for f in Face::ALL {
        assert!(s.is_resident(id(f.0, 0, 0, 0)), "roots are never evicted");
    }
    let st = s.stats();
    assert!(st.resident + st.in_flight <= 12);
}

// spec: STRM-006
#[test]
fn a_residency_request_never_evicts_an_r_tile() {
    // Capacity 8 = six roots + two R tiles. A third R tile has to wait: R does not evict R (no thrash for an undegraded
    // want set; the manager fits R to the capacity).
    let config = StreamConfig { max_in_flight: 4, capacity: 8, ..Default::default() };
    let (mut s, sp) = fixture(config);
    let (a, b, c) = (id(0, 1, 0, 0), id(0, 1, 1, 0), id(0, 1, 0, 1));
    let w = Wants { draw: vec![], residency: vec![(a, 1.0), (b, 2.0), (c, 3.0)], prefetch: vec![] };
    run_frames(&mut s, &sp, &w, 8);
    assert!(s.is_resident(a) && s.is_resident(b), "the nearer R tiles took the slots");
    assert!(!s.is_resident(c));
    assert_eq!(s.stats().evicted, 0);
    assert!(s.stats().blocked > 0, "the third request reports that it found no slot");
}

// spec: STRM-006
#[test]
fn the_vertex_pool_holds_levels_zero_to_two_and_does_not_count_against_the_main_capacity() {
    let config = StreamConfig { max_in_flight: 8, capacity: 4, vertex_pool_slots: 512, ..Default::default() };
    let (mut s, sp) = fixture(config);
    assert!(s.has_vertex_pool());
    let low: Vec<TileId> = (0..4).map(|x| id(0, 2, x, 0)).collect();
    let high: Vec<TileId> = (0..6).map(|x| id(0, 3, x, 0)).collect();
    let all: Vec<(TileId, f64)> = low.iter().chain(&high).map(|t| (*t, 1.0)).collect();
    run_frames(&mut s, &sp, &Wants { draw: vec![], residency: all, prefetch: vec![] }, 10);
    for t in &low {
        assert_eq!(s.slot_of(*t).map(|r| r.pool), Some(PoolKind::Vertex), "{t:?}");
    }
    let main_resident = high.iter().filter(|t| s.is_resident(**t)).count();
    assert!(main_resident <= 4, "the main pool holds {main_resident} of its 4 slots");
    assert_eq!(s.stats().main_in_use, main_resident + s.stats().in_flight);
    for f in Face::ALL {
        assert_eq!(s.slot_of(id(f.0, 0, 0, 0)).map(|r| r.pool), Some(PoolKind::Vertex), "roots live in the vertex pool");
    }
    // SlotPools alone: the pool kind follows the level.
    let pools = SlotPools::new(10, 512, 0);
    assert_eq!(pools.kind_of(id(0, 2, 0, 0)), PoolKind::Vertex);
    assert_eq!(pools.kind_of(id(0, 3, 0, 0)), PoolKind::Main);
    assert_eq!(SlotPools::new(10, 0, 0).kind_of(id(0, 0, 0, 0)), PoolKind::Main, "no small pool: one pool for every level");
}
