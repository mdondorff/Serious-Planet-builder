//! The upload budget's rules as pure functions (STRM-006 upload cap, ADR 0033 point 9): order, the prefix that fits, the
//! one-tile rule, and the fill times at the provisional 1 MiB cap. The loop-level tests are in `managed.rs`.

use planet_core::TileId;
use planet_frame::upload::{sort_waiting, upload_count, UploadBudget, UploadClass, Waiting, DEFAULT_TILE_BYTES_PER_FRAME};
use planet_generators::compact::slot_bytes;

fn tile(level: u8, n: u64) -> TileId {
    let side = 1u64 << level;
    TileId::new(planet_core::Face(0), level, (n % side) as u32, (n / side) as u32).unwrap()
}

fn w(tile: TileId, class: UploadClass, priority: f64, bytes: u64) -> Waiting {
    Waiting { tile, class, priority, bytes }
}

// spec: STRM-006
#[test]
fn the_order_is_class_then_coarse_first_then_plan_order_and_does_not_depend_on_the_input_order() {
    let mut list = vec![
        w(tile(3, 1), UploadClass::Prefetch, 0.0, 10),
        w(tile(5, 1), UploadClass::Draw, 1.0, 10),
        w(tile(2, 2), UploadClass::Residency, 5.0, 10),
        w(tile(5, 2), UploadClass::Draw, 0.0, 10),
        w(tile(1, 0), UploadClass::Other, 0.0, 10),
        w(tile(4, 3), UploadClass::Draw, 9.0, 10),
        w(tile(2, 1), UploadClass::Residency, 1.0, 10),
    ];
    let mut sorted = list.clone();
    sort_waiting(&mut sorted);
    let key: Vec<_> = sorted.iter().map(|x| (x.class, x.tile.level())).collect();
    assert_eq!(
        key,
        vec![
            (UploadClass::Draw, 4),
            (UploadClass::Draw, 5),
            (UploadClass::Draw, 5),
            (UploadClass::Residency, 2),
            (UploadClass::Residency, 2),
            (UploadClass::Prefetch, 3),
            (UploadClass::Other, 1),
        ]
    );
    assert!(sorted[1].priority < sorted[2].priority, "within a level the plan order");
    for rot in 0..list.len() {
        list.rotate_left(rot);
        let mut again = list.clone();
        sort_waiting(&mut again);
        let ids = |v: &[Waiting]| v.iter().map(|x| x.tile).collect::<Vec<_>>();
        assert_eq!(ids(&again), ids(&sorted), "rotation {rot}");
    }
    list.reverse();
    sort_waiting(&mut list);
    assert_eq!(list.iter().map(|x| x.tile).collect::<Vec<_>>(), sorted.iter().map(|x| x.tile).collect::<Vec<_>>());
}

// spec: STRM-006
#[test]
fn the_prefix_that_fits_is_written_and_a_tile_that_does_not_fit_stops_the_frame() {
    let t = tile(2, 0);
    let list = |sizes: &[u64]| sizes.iter().map(|&b| w(t, UploadClass::Draw, 0.0, b)).collect::<Vec<_>>();
    assert_eq!(upload_count(&list(&[]), 100), 0);
    assert_eq!(upload_count(&list(&[40, 40, 40]), 100), 2, "two fit, the third waits");
    assert_eq!(upload_count(&list(&[40, 40, 20, 40]), 100), 3, "exactly the cap fits");
    // A big tile in front is not overtaken by a small one behind it, and a small one in front does not let a big one past.
    assert_eq!(upload_count(&list(&[60, 80, 10]), 100), 1, "the 10 B tile does not jump the 80 B tile");
    assert_eq!(upload_count(&list(&[10, 80, 20]), 100), 2);
    // The one-tile rule: a tile over the cap is written alone, so it cannot starve.
    assert_eq!(upload_count(&list(&[500, 10]), 100), 1);
    assert_eq!(upload_count(&list(&[500]), 0), 1);
    assert_eq!(upload_count(&list(&[1, 1]), 0), 1, "a zero allowance still writes one tile");
    assert_eq!(upload_count(&list(&[u64::MAX, 1]), 100), 1, "no overflow");
}

// spec: STRM-006
#[test]
fn the_budget_is_the_provisional_1_mib_of_tile_data_and_node_records_are_inside_it_only_when_asked() {
    let b = UploadBudget::default();
    assert_eq!(b.tile_bytes_per_frame, 1 << 20);
    assert_eq!(DEFAULT_TILE_BYTES_PER_FRAME, 1 << 20);
    assert!(!b.node_records_in_cap, "ADR 0033 point 9: the cap is on tile data");
    assert_eq!(b.allowance(614_164), 1 << 20, "ground-1m's node records are their own line");
    let inside = UploadBudget { node_records_in_cap: true, ..b };
    assert_eq!(inside.allowance(614_164), (1 << 20) - 614_164);
    assert_eq!(inside.allowance(u64::MAX), 0, "never negative; the one-tile rule still writes a tile");
    assert_eq!(UploadBudget::unlimited().allowance(1 << 40), u64::MAX);
}

// spec: STRM-006
#[test]
fn a_full_ground_1m_fill_takes_the_frames_the_cap_allows_in_both_formats() {
    // ADR 0033 point 9: a full ground-1m fill is 8,763 compact slots (80 MB at 9,168 B). Whole tiles only: 114 per frame at 9,168 B (GEN-025).
    let slots = 8_763u64;
    let compact = slot_bytes(32);
    assert_eq!(compact, 9_168);
    let frames = |tile_bytes: u64| {
        let mut list: Vec<Waiting> = (0..slots).map(|i| w(tile(7, i), UploadClass::Draw, i as f64, tile_bytes)).collect();
        sort_waiting(&mut list);
        let (mut left, mut n) = (list.as_slice(), 0);
        while !left.is_empty() {
            left = &left[upload_count(left, DEFAULT_TILE_BYTES_PER_FRAME)..];
            n += 1;
        }
        n
    };
    let per_frame = |b: u64| (DEFAULT_TILE_BYTES_PER_FRAME / b).max(1);
    assert_eq!(per_frame(compact), 114);
    assert_eq!(frames(compact), slots.div_ceil(114) as usize);
    assert_eq!(frames(compact), 77, "80 MB / 1 MiB is 76.6 frames; whole tiles of 9,168 B make it 77 (STRM-006)");
    let vertex = 58_608;
    assert_eq!(per_frame(vertex), 17);
    assert_eq!(frames(vertex), 516, "the vertex format needs almost seven times the frames");
}
