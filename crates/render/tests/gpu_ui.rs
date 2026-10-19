//! Tier C: the UI pass paints over the blitted frame and leaves every other pixel as the blit alone made it (VIEW-011). The window itself
//! needs a display; the compose path is the same blit and the same painter drawn into an offscreen target. No font is needed: the
//! panel is a filled rectangle.

use planet_render::{AdapterPolicy, GpuContext, RgbaFrame, UiFrame, UiPainter};

fn px(img: &RgbaFrame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
}

/// One egui pass painting a solid red rectangle of `rect` points at `ppp` pixels per point on a screen of `size_px`.
fn red_rect(size_px: (u32, u32), ppp: f32, rect: egui::Rect) -> UiFrame {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(ppp);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size_px.0 as f32 / ppp, size_px.1 as f32 / ppp))),
        ..Default::default()
    };
    let out = ctx.run_ui(input, |ui| {
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(255, 0, 0));
    });
    UiFrame {
        primitives: ctx.tessellate(out.shapes, out.pixels_per_point),
        textures_delta: out.textures_delta,
        pixels_per_point: out.pixels_per_point,
    }
}

// spec: VIEW-011
#[test]
fn the_ui_pass_paints_its_rectangle_and_leaves_every_pixel_outside_it_as_the_blit_made_it() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // A 64x64 gradient frame shown 1:1 in a 64x64 target.
    let rgba: Vec<u8> = (0..64u32 * 64).flat_map(|i| [(i % 64 * 4) as u8, (i / 64 * 4) as u8, 90, 255]).collect();
    let src = RgbaFrame { width: 64, height: 64, rgba };
    let mut painter = UiPainter::offscreen(&ctx);
    let plain = painter.compose(&ctx, &src, (64, 64), None);
    for ppp in [1.0f32, 2.0] {
        // The rectangle covers points (8, 8) to (24, 24): pixels (8 ppp .. 24 ppp).
        let mut ui = red_rect((64, 64), ppp, egui::Rect::from_min_max(egui::pos2(8.0, 8.0), egui::pos2(24.0, 24.0)));
        let with = painter.compose(&ctx, &src, (64, 64), Some(&mut ui));
        let (lo, hi) = ((8.0 * ppp) as u32, (24.0 * ppp) as u32);
        let mut outside = 0;
        for y in 0..64 {
            for x in 0..64 {
                // One pixel of margin for the edge's coverage.
                let inside = x > lo && x + 1 < hi && y > lo && y + 1 < hi;
                let near_edge = x + 1 >= lo && x <= hi && y + 1 >= lo && y <= hi && !inside;
                if inside {
                    assert_eq!(px(&with, x, y), [255, 0, 0, 255], "inside pixel ({x}, {y}) at ppp {ppp}");
                } else if !near_edge {
                    assert_eq!(px(&with, x, y), px(&plain, x, y), "outside pixel ({x}, {y}) at ppp {ppp} is the blit alone");
                    outside += 1;
                }
            }
        }
        assert!(outside > 64 * 64 / 2, "most of the frame was compared: {outside}");
    }
}
