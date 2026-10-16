//! Tier C: the window's blit shows the renderer's frame in the window, scaled to fit with black bars (VIEW-006). The surface
//! itself needs a display; the blit pass is the same code drawn into an offscreen target.

use planet_render::window::blit_to_image;
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame};

fn px(img: &RgbaFrame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
}

// spec: VIEW-006
#[test]
fn the_blit_scales_the_frame_into_the_window_with_black_bars() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // A 4x2 frame of eight distinct colours into a 20x20 target: it fits 20x10, centred, bars of 5 rows above and below, and each texel is a 5x5 block
    // whose middle pixel samples the texel's centre exactly (the sampler filters linearly).
    let colours: Vec<[u8; 4]> = (0..8u8).map(|k| [30 * k + 20, 255 - 30 * k, 90 + 10 * k, 255]).collect();
    let src = RgbaFrame { width: 4, height: 2, rgba: colours.iter().flatten().copied().collect() };
    let out = blit_to_image(&ctx, &src, (20, 20));
    assert_eq!((out.width, out.height), (20, 20));
    for x in 0..20 {
        for y in (0..5).chain(15..20) {
            assert_eq!(px(&out, x, y), [0, 0, 0, 255], "bar pixel ({x}, {y})");
        }
    }
    // The middle of each source texel (a 5x5 block of the output) shows its colour, row 0 above row 1.
    for (k, c) in colours.iter().enumerate() {
        let (tx, ty) = ((k % 4) as u32, (k / 4) as u32);
        let got = px(&out, tx * 5 + 2, 5 + ty * 5 + 2);
        for ch in 0..3 {
            assert!((i32::from(got[ch]) - i32::from(c[ch])).abs() <= 1, "texel {k} channel {ch}: got {got:?}, want {c:?}");
        }
        assert_eq!(got[3], 255);
    }
}
