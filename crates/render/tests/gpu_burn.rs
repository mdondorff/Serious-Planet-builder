//! BUDG-011: the GPU burn pass of the reserve test runs on every adapter and is calibrated with timestamp queries where the adapter has them (the discrete GPU);
//! on the software adapter it says it cannot be calibrated.

use planet_render::{AdapterPolicy, BurnPass, GpuContext};

// spec: BUDG-011
#[test]
fn the_burn_pass_runs_and_is_calibrated_to_its_milliseconds_where_timestamps_exist() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    let burn = BurnPass::new(&ctx);
    burn.submit(&ctx, 64);
    assert_eq!(burn.can_calibrate(), ctx.timestamps_supported());
    if !ctx.timestamps_supported() {
        assert_eq!(burn.measure_ms(&ctx, 64), None, "no timestamp queries, no time");
        assert_eq!(burn.calibrate(&ctx, 3.0, 0.05), None, "the software adapter cannot calibrate the burn: {}", ctx.adapter_log_line());
        return;
    }
    // The time grows with the iterations on every adapter that has timestamp queries.
    let (short, long) = (burn.measure_ms(&ctx, 2000).expect("timestamps"), burn.measure_ms(&ctx, 8000).expect("timestamps"));
    assert!(long > short * 1.5, "four times the iterations take {long:.3} ms against {short:.3} ms");
    // The calibration lands inside 5 % of 3.0 ms on the discrete GPU (3 attempts: the clock settles); a CPU adapter with timestamps (lavapipe) times a software kernel and
    // only has to land inside 25 %.
    let limit = if ctx.adapter_log_line().contains("DiscreteGpu") { 0.05 } else { 0.25 };
    let mut best = f64::MAX;
    for _ in 0..3 {
        if let Some(cal) = burn.calibrate(&ctx, 3.0, limit) {
            best = best.min(cal.error());
            if cal.error() <= limit {
                return;
            }
        }
    }
    panic!(
        "the 3.0 ms burn was not calibrated within {:.0} % (best error {:.1} %) on {}",
        limit * 100.0,
        best * 100.0,
        ctx.adapter_log_line()
    );
}
