//! REND-029: the first wgpu error is kept and printed before the default panic.

use planet_render::{AdapterPolicy, GpuContext};

// spec: REND-029
#[test]
fn the_first_uncaptured_error_is_kept_and_still_panics() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    assert_eq!(ctx.first_error(), None);
    assert_eq!(ctx.device_lost(), None);
    let bad = |label: &'static str| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // No usage flags: a validation error at creation.
            ctx.test_create_invalid_buffer(label);
        }))
    };
    let panic = bad("first-bad").expect_err("an uncaptured error panics, as the default handler does");
    let text = panic.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(text.starts_with("wgpu error:"), "{text}");
    let first = ctx.first_error().expect("the error is kept");
    assert!(first.contains("usage") || first.contains("Usage"), "{first}");
    assert!(bad("second-bad").is_err());
    assert_eq!(ctx.first_error(), Some(first), "a later error does not replace the first");
}
