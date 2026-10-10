//! Tier C: headless hello triangle on the selected adapter (software by default).

use planet_render::{hello_triangle, AdapterPolicy, GpuContext, HELLO_CLEAR, HELLO_FILL};
use planet_testkit::{assert_golden, Image, Tolerance};

const SIZE: u32 = 256;

// spec: TEST-004
#[test]
fn hello_triangle_renders_headless() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    eprintln!("adapter: {:?} {} ({:?})", ctx.info.backend, ctx.info.name, ctx.info.device_type);
    let frame = hello_triangle(&ctx, SIZE);
    let img = Image::new(frame.width, frame.height, frame.rgba);

    // Metrics that hold on every adapter, independent of any golden.
    assert_eq!(img.pixel(2, 2), HELLO_CLEAR, "corner must be the clear colour");
    assert_eq!(img.pixel(SIZE / 2, SIZE / 2), HELLO_FILL, "centre must be inside the triangle");
    let (clear, fill) = (img.count(HELLO_CLEAR), img.count(HELLO_FILL));
    assert_eq!(clear + fill, (SIZE * SIZE) as usize, "no antialiasing or stray colours expected: {clear} clear + {fill} fill");
    // Triangle area is 1.125 of 4.0 NDC units^2 = 28.125 % of the target.
    let expected = (SIZE * SIZE) as f64 * 0.28125;
    assert!((fill as f64 - expected).abs() < expected * 0.01, "filled area {fill} px, expected about {expected}");

    assert_golden(&ctx.adapter_key(), "hello_triangle", &img, Tolerance::Exact);
}
