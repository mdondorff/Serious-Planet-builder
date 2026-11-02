//! The spawn cost of the share's `Par` against the plan job it serves (BUDG-009; planner's rule: above 1 % a pooled version is considered). A diagnostic, not a gate
//! (`--run-ignored only --no-capture`): the median time of one `run` of eight trivial chunks on a share of 18 slots (what each fan-out of the plan job pays for its
//! scoped threads), and the median time of one cold plan job of the 200 m view on the reference field with the same `Par`. The reference field is cheap to read;
//! a plan over the production store takes longer, so the ratio printed is an upper bound for it.

use planet_core::cube::TangentWarp;
use planet_core::par::Par;
use planet_frame::{plan_terrain_par, scripted_views_field, LodParams, SelectionCache, TerrainRequest, TerrainView};
use planet_generators::height::{HeightField, Reference};
use planet_streaming::jobs::JobSystem;
use planet_streaming::Priority;
use std::time::Instant;

const R: f64 = 6_371_000.0;

// spec: BUDG-009
#[test]
#[ignore = "a diagnostic: prints the spawn cost of the share's Par against a plan job"]
fn the_spawn_cost_of_the_share_par_next_to_a_plan_job() {
    let system = JobSystem::new(18, Priority::Normal);
    let share = system.share("terrain", 18);
    let par = share.par();
    let mut run_us: Vec<f64> = (0..201)
        .map(|_| {
            let t = Instant::now();
            par.run(8, &|i| {
                std::hint::black_box(i);
            });
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    run_us.sort_by(f64::total_cmp);
    // The same fan-out on plain scoped threads: what the plan job paid before the job system (`impl Par for usize`).
    let mut plain_us: Vec<f64> = (0..201)
        .map(|_| {
            let t = Instant::now();
            18usize.run(8, &|i| {
                std::hint::black_box(i);
            });
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    plain_us.sort_by(f64::total_cmp);
    let field = Reference(1);
    let size = (1280u32, 720u32);
    let lod =
        LodParams { viewport_h_px: 720.0, tau_px: 6.0, min_level: 0, max_height_m: field.max_abs_height_m(), ..LodParams::earth_1080p() };
    let (_, camera) = scripted_views_field(R, 1280.0 / 720.0, &field).into_iter().find(|(n, _)| *n == "ground-200m").expect("a 200 m view");
    let req = TerrainRequest { camera, size, view: TerrainView::Level, lod, face_mapping: TangentWarp_id(), seed: 1 };
    let mut plan_us: Vec<f64> = (0..21)
        .map(|_| {
            let t = Instant::now();
            plan_terrain_par(&req, &field, &mut SelectionCache::new(), &par).expect("a plan");
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    plan_us.sort_by(f64::total_cmp);
    println!(
        "one fan-out of 8 trivial chunks: share Par {:.0} us, plain scoped threads (before) {:.0} us, median; cold plan job {:.0} us (reference field, cheap to read); fan-out / plan job {:.4}, added by the share (difference) / plan job {:.5}",
        run_us[100],
        plain_us[100],
        plan_us[10],
        run_us[100] / plan_us[10],
        (run_us[100] - plain_us[100]) / plan_us[10]
    );
}

#[allow(non_snake_case)]
fn TangentWarp_id() -> String {
    use planet_core::cube::FaceMapping;
    TangentWarp.id().to_string()
}
