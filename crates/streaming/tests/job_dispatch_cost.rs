//! The dispatch cost of the job system against the thread pool it replaces for the tile pool (BUDG-004), next to the cost of a tile job. A diagnostic, not a
//! gate (`--run-ignored only --no-capture`): it prints the median time to dispatch and finish one empty job on a `ThreadPool` and on the `terrain` share
//! of `GenWorkers::pool`, and the median time of one tile job (a compact tile of 32 cells at level 12 of the reference field), so that the ratio shows
//! whether the share's lock matters.

use planet_core::cube::TangentWarp;
use planet_core::{Face, TileId};
use planet_generators::compact::compact_tile_field;
use planet_generators::height::Reference;
use planet_streaming::{GenWorkers, Spawner, ThreadPool};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

const JOBS: usize = 4000;
const REPS: usize = 101;

/// Seconds per job to dispatch `JOBS` empty jobs on `pool` and wait until all have run, median of `REPS` rounds.
fn per_job_us(pool: &dyn Spawner) -> f64 {
    let mut rounds: Vec<f64> = (0..REPS)
        .map(|_| {
            let done = Arc::new((AtomicUsize::new(0), Mutex::new(()), Condvar::new()));
            let started = Instant::now();
            for _ in 0..JOBS {
                let done = Arc::clone(&done);
                pool.spawn(Box::new(move || {
                    if done.0.fetch_add(1, Ordering::SeqCst) + 1 == JOBS {
                        let _g = done.1.lock().unwrap();
                        done.2.notify_all();
                    }
                }));
            }
            let mut g = done.1.lock().unwrap();
            while done.0.load(Ordering::SeqCst) < JOBS {
                g = done.2.wait(g).unwrap();
            }
            started.elapsed().as_secs_f64() * 1e6 / JOBS as f64
        })
        .collect();
    rounds.sort_by(f64::total_cmp);
    rounds[REPS / 2]
}

// spec: BUDG-004
#[test]
#[ignore = "a diagnostic: prints the dispatch cost per job on the thread pool and on the terrain share"]
fn the_share_lock_is_noise_next_to_a_tile_job() {
    let workers = GenWorkers::machine().workers;
    let old = ThreadPool::with_priority(workers, GenWorkers::machine().priority);
    let old_us = per_job_us(&old);
    drop(old);
    let share = GenWorkers::machine().pool();
    let new_us = per_job_us(share.as_ref());
    let tile = TileId::new(Face(0), 12, 100, 100).expect("a tile");
    let mut tile_ms: Vec<f64> = (0..51)
        .map(|_| {
            let t = Instant::now();
            let _ = compact_tile_field(&Reference(7), &TangentWarp, tile, 32, 6_371_000.0);
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    tile_ms.sort_by(f64::total_cmp);
    let tile_us = tile_ms[25] * 1e3;
    println!(
        "{workers} workers, {JOBS} empty jobs a round, median of {REPS} rounds: thread pool {old_us:.2} us per job, terrain share {new_us:.2} us per job (difference {:.2} us); median tile job {:.0} us; ratio difference / tile job {:.5}",
        new_us - old_us,
        tile_us,
        (new_us - old_us) / tile_us
    );
}
