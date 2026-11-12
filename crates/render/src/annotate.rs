//! The scale bar and the pose label of the owner's review images (ADR 0118 point 5, owner via architect, 2026-11-12): every owner-facing review image shows how big things are and from where it was taken, so a look call is
//! not made on a picture of unknown scale. The bar and the label are drawn by the review tooling on the copy it writes under `target/review`; a golden is never annotated (a golden is compared pixel for pixel).
//!
//! The label is the pose: the altitude over the ground, the pitch, the vertical field of view and the metres per pixel at the centre of the picture. The metres per pixel follow from the distance along the view axis to the
//! ground (or the sea) at the centre: `2 d tan(fov / 2) / height_px`, exact for a surface square to the axis and the pixel's size across the axis otherwise (a picture of a ground that falls away has a larger scale towards
//! the horizon: the bar is for the centre). A view whose axis meets no ground (the sky, space) has no scale: the label says so and draws no bar.

use crate::RgbaFrame as Image;

#[cfg(test)]
fn pixel_at(img: &Image, x: u32, y: u32) -> [u8; 4] {
    let i = 4 * (y * img.width + x) as usize;
    [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2], img.rgba[i + 3]]
}

fn set_pixel_at(img: &mut Image, x: u32, y: u32, px: [u8; 4]) {
    let i = 4 * (y * img.width + x) as usize;
    img.rgba[i..i + 4].copy_from_slice(&px);
}

/// What the label says about a picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseLabel {
    /// The altitude of the eye over the ground or the sea below it, in metres.
    pub altitude_m: f64,
    /// Degrees below the horizon the view axis points.
    pub pitch_deg: f64,
    /// The vertical field of view in degrees.
    pub fov_deg: f64,
    /// The distance along the view axis to the ground at the centre of the picture, in metres; `None`: the axis meets none.
    pub centre_distance_m: Option<f64>,
}

impl PoseLabel {
    /// The metres a pixel spans at the centre of a picture `height_px` tall, if the axis meets the ground.
    pub fn metres_per_pixel(&self, height_px: u32) -> Option<f64> {
        let d = self.centre_distance_m?;
        if !(d.is_finite() && d > 0.0 && self.fov_deg > 0.0 && self.fov_deg < 180.0 && height_px > 0) {
            return None;
        }
        Some(2.0 * d * libm::tan(self.fov_deg.to_radians() / 2.0) / f64::from(height_px))
    }

    /// The text of the label for a picture `height_px` tall.
    pub fn text(&self, height_px: u32) -> String {
        let scale = match self.metres_per_pixel(height_px) {
            Some(m) => format!("{} m/px", number(m)),
            None => "no ground at centre".to_string(),
        };
        format!("alt {} m  pitch {} deg  fov {} deg  {scale}", number(self.altitude_m), number(self.pitch_deg), number(self.fov_deg))
    }
}

/// A number with three significant digits at most and no trailing zeros (`4.2`, `300`, `1200`, `0.35`).
fn number(v: f64) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    let a = v.abs();
    let places = if a >= 100.0 {
        0
    } else if a >= 10.0 {
        1
    } else if a >= 1.0 {
        2
    } else {
        3
    };
    let s = format!("{v:.places$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// The longest round length (1, 2 or 5 times a power of ten, in metres) that is at most `max_m`.
pub fn nice_length_m(max_m: f64) -> f64 {
    if !(max_m.is_finite() && max_m > 0.0) {
        return 0.0;
    }
    let p = libm::pow(10.0, libm::floor(libm::log10(max_m)));
    [5.0, 2.0, 1.0].into_iter().map(|m| m * p).find(|&l| l <= max_m * (1.0 + 1e-12)).unwrap_or(p)
}

/// `1 km`, `500 m`, `20 m`.
pub fn length_text(m: f64) -> String {
    if m >= 1000.0 {
        format!("{} km", number(m / 1000.0))
    } else {
        format!("{} m", number(m))
    }
}

/// The 5 x 7 glyph of `c` (seven rows, the low five bits of each, the left pixel the highest): the digits, the letters of the label and a few marks; anything else is a blank.
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        'a' => [0, 0, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
        'c' => [0, 0, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110],
        'd' => [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111],
        'e' => [0, 0, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
        'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
        'g' => [0, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        'h' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        'i' => [0b00100, 0, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
        'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'm' => [0, 0, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101],
        'n' => [0, 0, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        'o' => [0, 0, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'p' => [0, 0, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'r' => [0, 0, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
        's' => [0, 0, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
        't' => [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110],
        'u' => [0, 0, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101],
        'v' => [0, 0, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'x' => [0, 0, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        'y' => [0, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        _ => [0; 7],
    }
}

/// Blends `px` over the pixel at `(x, y)` if it is inside the picture.
fn put(img: &mut Image, x: i64, y: i64, px: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height {
        set_pixel_at(img, x as u32, y as u32, px);
    }
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// Draws `text` with its top left at `(x, y)`, each glyph pixel `s` pixels square, white with a black outline of one glyph pixel (readable on any picture). The width in pixels is returned.
pub fn draw_text(img: &mut Image, x: i64, y: i64, text: &str, s: u32) -> u32 {
    let s = i64::from(s.max(1));
    let advance = 6 * s;
    for pass in 0..2 {
        let mut cx = x;
        for c in text.chars() {
            let g = glyph(c.to_ascii_lowercase());
            for (row, bits) in g.iter().enumerate() {
                for col in 0..5 {
                    if bits >> (4 - col) & 1 == 0 {
                        continue;
                    }
                    let (px, py) = (cx + col * s, y + row as i64 * s);
                    if pass == 0 {
                        // The outline: the glyph pixel's square grown by `s` on every side.
                        for dy in -s..2 * s {
                            for dx in -s..2 * s {
                                put(img, px + dx, py + dy, BLACK);
                            }
                        }
                    } else {
                        for dy in 0..s {
                            for dx in 0..s {
                                put(img, px + dx, py + dy, WHITE);
                            }
                        }
                    }
                }
            }
            cx += advance;
        }
    }
    (text.chars().count() as i64 * advance) as u32
}

/// The scale of the text for a picture `height_px` tall: one glyph pixel per 180 pixels of height, at least one.
fn text_scale(height_px: u32) -> u32 {
    (height_px / 180).max(1)
}

/// Draws the pose label and, when the axis meets the ground, a scale bar on `img`, bottom left. The bar is the longest round length (1, 2 or 5 times a power of ten) that takes at most a quarter of the picture's width.
/// Returns the bar's length in metres (`None`: no bar).
pub fn annotate(img: &mut Image, pose: &PoseLabel) -> Option<f64> {
    // The text scale: by height, and no larger than the label fits the width at (a narrow picture gets small text rather than a cut-off label).
    let label = pose.text(img.height);
    let fit = (img.width.saturating_sub(16)) / (6 * label.chars().count() as u32).max(1);
    let s = text_scale(img.height).min(fit.max(1));
    let margin = i64::from(8 * s);
    let line = i64::from(10 * s);
    let mut y = i64::from(img.height) - margin - 7 * i64::from(s);
    let mut bar_m = None;
    if let Some(mpp) = pose.metres_per_pixel(img.height) {
        let max_m = mpp * f64::from(img.width) / 4.0;
        let len_m = nice_length_m(max_m);
        let len_px = (len_m / mpp).round() as i64;
        if len_m > 0.0 && len_px >= 2 {
            // The bar: a line of 2 glyph pixels with a tick of 6 at each end, in the outline's black under white.
            let (x0, yb) = (margin, y - 2 * i64::from(s));
            for pass in 0..2 {
                let (grow, colour) = if pass == 0 { (i64::from(s), BLACK) } else { (0, WHITE) };
                for dx in -grow..len_px + grow {
                    for dy in -grow..2 * i64::from(s) + grow {
                        put(img, x0 + dx, yb + dy, colour);
                    }
                }
                for tick_x in [x0, x0 + len_px - i64::from(s)] {
                    for dy in -4 * i64::from(s) - grow..2 * i64::from(s) + grow {
                        for dx in -grow..i64::from(s) + grow {
                            put(img, tick_x + dx, yb + dy, colour);
                        }
                    }
                }
            }
            draw_text(img, x0, yb - 6 * i64::from(s) - 7 * i64::from(s) - line / 2, &length_text(len_m), s);
            y = yb - 6 * i64::from(s) - 7 * i64::from(s) - line / 2 - line;
            bar_m = Some(len_m);
        }
    }
    draw_text(img, margin, y, &label, s);
    bar_m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(w: u32, h: u32, px: [u8; 4]) -> Image {
        Image { width: w, height: h, rgba: px.iter().copied().cycle().take((w * h * 4) as usize).collect() }
    }

    fn pose() -> PoseLabel {
        PoseLabel { altitude_m: 300.0, pitch_deg: 15.0, fov_deg: 60.0, centre_distance_m: Some(1159.0) }
    }

    // spec: TEST-126
    #[test]
    fn the_metres_per_pixel_follow_from_the_distance_the_field_of_view_and_the_height() {
        // 2 d tan(30 deg) / 720 px: a 1,159 m distance gives 1.86 m a pixel.
        let m = pose().metres_per_pixel(720).unwrap();
        assert!((m - 2.0 * 1159.0 * libm::tan(30f64.to_radians()) / 720.0).abs() < 1e-12 && (m - 1.8587).abs() < 1e-3, "{m}");
        assert_eq!(PoseLabel { centre_distance_m: None, ..pose() }.metres_per_pixel(720), None);
        assert_eq!(PoseLabel { fov_deg: 0.0, ..pose() }.metres_per_pixel(720), None);
        assert_eq!(pose().text(720), "alt 300 m  pitch 15 deg  fov 60 deg  1.86 m/px");
        assert!(PoseLabel { centre_distance_m: None, ..pose() }.text(720).ends_with("no ground at centre"));
    }

    // spec: TEST-126
    #[test]
    fn the_bar_is_a_round_length_under_a_quarter_of_the_width_and_named() {
        for (max, want) in
            [(1.0, 1.0), (1.9, 1.0), (2.0, 2.0), (4.9, 2.0), (5.0, 5.0), (9.99, 5.0), (130.0, 100.0), (2600.0, 2000.0), (0.35, 0.2)]
        {
            assert_eq!(nice_length_m(max), want, "{max}");
        }
        assert_eq!(
            (length_text(500.0), length_text(2000.0), length_text(20.0), length_text(1500.0)),
            ("500 m".into(), "2 km".into(), "20 m".into(), "1.5 km".into())
        );
        assert_eq!(nice_length_m(0.0), 0.0);
        assert_eq!(nice_length_m(f64::NAN), 0.0);
    }

    // spec: TEST-126
    #[test]
    fn annotating_draws_a_bar_of_the_stated_length_and_changes_only_the_bottom_left() {
        let (w, h) = (640u32, 360u32);
        let plain = filled(w, h, [120, 160, 90, 255]);
        let mut img = Image { width: plain.width, height: plain.height, rgba: plain.rgba.clone() };
        let p = pose();
        let bar_m = annotate(&mut img, &p).expect("a bar");
        let mpp = p.metres_per_pixel(h).unwrap();
        // The bar fits in a quarter of the width and is the longest round length that does (here: 2 m at 1.86 m a pixel is 1 px: the next round length down is 1 m: not drawn; 100 m would not fit): the length is round.
        let r = bar_m / mpp;
        assert!(r <= f64::from(w) / 4.0 + 1.0, "{r} px");
        // Pixels changed: only white, black and untouched; none in the top 60 %.
        let mut changed = 0usize;
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (pixel_at(&plain, x, y), pixel_at(&img, x, y));
                if a != b {
                    changed += 1;
                    assert!(b == [255, 255, 255, 255] || b == [0, 0, 0, 255], "({x}, {y}) {b:?}");
                    assert!(y > h * 6 / 10, "({x}, {y}) is outside the bottom of the picture");
                }
            }
        }
        assert!(changed > 200, "{changed} pixels drawn");
        // A picture whose axis meets no ground gets the label and no bar.
        let mut sky = Image { width: plain.width, height: plain.height, rgba: plain.rgba.clone() };
        assert_eq!(annotate(&mut sky, &PoseLabel { centre_distance_m: None, ..p }), None);
        assert!(sky.rgba != plain.rgba, "the label is still drawn");
        // The same picture annotated twice is the same picture (no randomness, no time).
        let mut again = Image { width: plain.width, height: plain.height, rgba: plain.rgba.clone() };
        annotate(&mut again, &p);
        assert_eq!(again.rgba, img.rgba);
    }

    // spec: TEST-126
    /// A sample picture for the eye (not a golden): a gradient ground with the bar and the label, in `target/review/annotate-sample.png`.
    #[test]
    fn a_sample_annotated_picture_is_written_for_the_eye() {
        let (w, h) = (1280u32, 720u32);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[(90 + 60 * x / w) as u8, (140 + 80 * y / h) as u8, 80, 255]);
            }
        }
        let mut img = Image { width: w, height: h, rgba };
        let bar = annotate(&mut img, &PoseLabel { altitude_m: 300.0, pitch_deg: 15.0, fov_deg: 60.0, centre_distance_m: Some(1159.0) });
        assert_eq!(bar, Some(500.0), "1.86 m a pixel: a quarter of 1280 px is 594 m, the longest round length under it 500 m (269 px)");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/review");
        img.write_png(&dir.join("annotate-sample.png")).unwrap();
    }
}
