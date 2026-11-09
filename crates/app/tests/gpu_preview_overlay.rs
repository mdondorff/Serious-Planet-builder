//! Tier C, VIEW-068: the level-1 preview overlay drawn through the egui path. A frame composed with the overlay differs from the same frame composed without it in the overlay's pixels only (its
//! lines and its label), and differs there; every other pixel is the blit alone. Not a golden: a pixel assertion, like `gpu_ui` (VIEW-011). The overlay is the one `draw_preview_overlay` paints
//! from the data of `planet_editor::preview_overlay` (VIEW-067).

use planet::panels::draw_preview_overlay;
use planet_editor::preview_overlay::{DrawnLine, PreviewOverlay};
use planet_render::{AdapterPolicy, GpuContext, RgbaFrame, UiFrame, UiPainter};

const SIZE: u32 = 256;

fn px(img: &RgbaFrame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
}

/// The overlay painted by `draw_preview_overlay` on a screen of `SIZE` pixels at `ppp` pixels per point, tessellated for the UI pass.
fn overlay_frame(overlay: &PreviewOverlay, ppp: f32) -> UiFrame {
    let ctx = egui::Context::default();
    ctx.set_fonts(planet::ui::fonts());
    ctx.set_pixels_per_point(ppp);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(SIZE as f32 / ppp, SIZE as f32 / ppp))),
        ..Default::default()
    };
    let out = ctx.run_ui(input, |ui| draw_preview_overlay(ui, overlay, |p| egui::pos2(p[0] as f32 / ppp, p[1] as f32 / ppp)));
    UiFrame {
        primitives: ctx.tessellate(out.shapes, out.pixels_per_point),
        textures_delta: out.textures_delta,
        pixels_per_point: out.pixels_per_point,
    }
}

// spec: VIEW-068
#[test]
fn the_preview_overlay_changes_the_pixels_of_its_lines_and_label_and_no_other_pixel() {
    let ctx = GpuContext::new(AdapterPolicy::from_env()).expect("adapter");
    // A gradient frame shown 1:1.
    let rgba: Vec<u8> = (0..SIZE * SIZE).flat_map(|i| [(i % SIZE) as u8, (i / SIZE) as u8, 90, 255]).collect();
    let src = RgbaFrame { width: SIZE, height: SIZE, rgba };
    let mut painter = UiPainter::offscreen(&ctx);
    let plain = painter.compose(&ctx, &src, (SIZE, SIZE), None);

    // Four long lines inside the frame, far from the label's corner (the label sits at the top left of the screen, 14 and 52 points in).
    let lines: Vec<DrawnLine> =
        [([30.0, 120.0], [220.0, 140.0]), ([40.0, 200.0], [210.0, 230.0]), ([128.0, 70.0], [128.0, 250.0]), ([220.0, 80.0], [60.0, 240.0])]
            .iter()
            .enumerate()
            .map(|(k, (a, b))| DrawnLine { a: *a, b: *b, watershed: k as u32, changed: k % 2 == 0, weight: 10.0 - k as f64 })
            .collect();
    let overlay = PreviewOverlay { id: 5, label: "preview".into(), lines: lines.clone(), dropped: 0 };

    for ppp in [1.0f32, 2.0] {
        let mut ui = overlay_frame(&overlay, ppp);
        let with = painter.compose(&ctx, &src, (SIZE, SIZE), Some(&mut ui));
        let (mut changed_on_lines, mut outside) = (0usize, 0usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                // The rectangle of a line grown by the stroke's width and the coverage of its edge; the label's rectangle at the top left.
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let near_line = lines.iter().any(|l| {
                    let (ax, ay, bx, by) = (l.a[0], l.a[1], l.b[0], l.b[1]);
                    let (dx, dy) = (bx - ax, by - ay);
                    let t = (((fx - ax) * dx + (fy - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                    let d = libm::hypot(fx - (ax + t * dx), fy - (ay + t * dy));
                    d <= 4.5 * f64::from(ppp)
                });
                let in_label =
                    (x as f64) < 120.0 * f64::from(ppp) && (y as f64) > 40.0 * f64::from(ppp) && (y as f64) < 80.0 * f64::from(ppp);
                if near_line || in_label {
                    if px(&with, x, y) != px(&plain, x, y) {
                        changed_on_lines += 1;
                    }
                } else {
                    assert_eq!(
                        px(&with, x, y),
                        px(&plain, x, y),
                        "pixel ({x}, {y}) at ppp {ppp} is outside the overlay's lines and label: the blit alone"
                    );
                    outside += 1;
                }
            }
        }
        assert!(changed_on_lines > 200, "the overlay is drawn: {changed_on_lines} pixels changed at ppp {ppp}");
        assert!(outside > (SIZE * SIZE) as usize / 2, "most of the frame was compared: {outside}");
    }

    // With the overlay off the frame is the frame without a preview: the same composition twice is the same pixels.
    let again = painter.compose(&ctx, &src, (SIZE, SIZE), None);
    assert_eq!(again.rgba, plain.rgba);
}
