//! Contact sheet of a flight (TEST-013): a few frames from orbit to 1 m, each labelled with its frame number and altitude,
//! so the whole dive reads at a glance without opening 240 PNGs. Pure CPU image code; the glyphs are a 3x5 pixel font
//! covering exactly the characters a label uses (digits, `.`, space, `k`, `m`).

use planet_render::RgbaFrame;

const GLYPH_W: u32 = 3;
const GLYPH_H: u32 = 5;
const GUTTER: u32 = 4;
const BACKGROUND: [u8; 4] = [24, 24, 28, 255];

/// Five rows of three bits (most significant bit is the left pixel).
fn glyph(c: char) -> Option<[u8; 5]> {
    Some(match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        'k' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'm' => [0b000, 0b111, 0b111, 0b101, 0b101],
        ' ' => [0; 5],
        _ => return None,
    })
}

/// Frame indices for a sheet of at most `count` frames: evenly spaced over `0..frames`, always including the first and
/// the last frame, without duplicates.
pub fn sheet_indices(frames: usize, count: usize) -> Vec<usize> {
    if frames == 0 || count == 0 {
        return vec![];
    }
    if count == 1 || frames == 1 {
        return vec![0];
    }
    let mut v: Vec<usize> = (0..count).map(|k| k * (frames - 1) / (count - 1)).collect();
    v.dedup();
    v
}

/// `"0060 3.0 km"`: the frame number and the altitude above the terrain, kilometres from 1 km up, otherwise metres.
pub fn label(frame: usize, altitude_m: f64) -> String {
    let alt = if altitude_m >= 1000.0 {
        let km = altitude_m / 1000.0;
        if km >= 100.0 {
            format!("{km:.0} km")
        } else {
            format!("{km:.1} km")
        }
    } else if altitude_m >= 10.0 {
        format!("{altitude_m:.0} m")
    } else {
        format!("{altitude_m:.1} m")
    };
    format!("{frame:04} {alt}")
}

fn put(img: &mut RgbaFrame, x: u32, y: u32, px: [u8; 4]) {
    if x < img.width && y < img.height {
        let i = ((y * img.width + x) * 4) as usize;
        img.rgba[i..i + 4].copy_from_slice(&px);
    }
}

/// Draws `text` with its top-left corner at (x, y) on a dark box, each font pixel `scale` pixels square. Characters
/// outside the font are drawn as blanks.
pub fn draw_text(img: &mut RgbaFrame, x: u32, y: u32, scale: u32, text: &str) {
    let advance = (GLYPH_W + 1) * scale;
    let (w, h) = (text.chars().count() as u32 * advance + scale, (GLYPH_H + 2) * scale);
    for dy in 0..h {
        for dx in 0..w {
            put(img, x + dx, y + dy, [0, 0, 0, 255]);
        }
    }
    for (i, c) in text.chars().enumerate() {
        let rows = glyph(c).unwrap_or([0; 5]);
        for (r, bits) in rows.iter().enumerate() {
            for col in 0..GLYPH_W {
                if bits >> (GLYPH_W - 1 - col) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = x + scale + i as u32 * advance + col * scale + sx;
                            put(img, px, y + scale + r as u32 * scale + sy, [255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
}

/// Box-filters `img` down by the integer factor `f` (at least 1).
fn shrink(img: &RgbaFrame, f: u32) -> RgbaFrame {
    let (w, h) = (img.width / f, img.height / f);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 4];
            for sy in 0..f {
                for sx in 0..f {
                    let i = (((y * f + sy) * img.width + x * f + sx) * 4) as usize;
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += u32::from(img.rgba[i + c]);
                    }
                }
            }
            rgba.extend(sum.map(|s| (s / (f * f)) as u8));
        }
    }
    RgbaFrame { width: w, height: h, rgba }
}

/// Lays the labelled frames out row by row in `columns` columns, each shrunk by the smallest integer factor that brings
/// it to at most `max_thumb_w` pixels wide. All frames must have the same size.
pub fn contact_sheet(frames: &[(RgbaFrame, String)], columns: usize, max_thumb_w: u32) -> RgbaFrame {
    assert!(!frames.is_empty() && columns > 0, "a contact sheet needs frames and columns");
    let (fw, fh) = (frames[0].0.width, frames[0].0.height);
    assert!(frames.iter().all(|(f, _)| (f.width, f.height) == (fw, fh)), "contact sheet frames must share one size");
    let f = fw.div_ceil(max_thumb_w.max(1)).max(1);
    let (tw, th) = (fw / f, fh / f);
    let columns = columns.min(frames.len()) as u32;
    let rows = (frames.len() as u32).div_ceil(columns);
    let (w, h) = (columns * tw + (columns + 1) * GUTTER, rows * th + (rows + 1) * GUTTER);
    let mut sheet = RgbaFrame { width: w, height: h, rgba: BACKGROUND.repeat((w * h) as usize) };
    let scale = (tw / 160).max(1);
    for (i, (frame, text)) in frames.iter().enumerate() {
        let (cx, cy) = (i as u32 % columns, i as u32 / columns);
        let (ox, oy) = (GUTTER + cx * (tw + GUTTER), GUTTER + cy * (th + GUTTER));
        let thumb = shrink(frame, f);
        for y in 0..th {
            for x in 0..tw {
                let s = ((y * thumb.width + x) * 4) as usize;
                put(&mut sheet, ox + x, oy + y, [thumb.rgba[s], thumb.rgba[s + 1], thumb.rgba[s + 2], thumb.rgba[s + 3]]);
            }
        }
        draw_text(&mut sheet, ox, oy, scale, text);
    }
    sheet
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> RgbaFrame {
        RgbaFrame { width: w, height: h, rgba: px.repeat((w * h) as usize) }
    }

    // spec: TEST-013
    #[test]
    fn sheet_indices_are_evenly_spaced_and_keep_both_ends() {
        assert_eq!(sheet_indices(240, 12).first(), Some(&0));
        assert_eq!(sheet_indices(240, 12).last(), Some(&239));
        assert_eq!(sheet_indices(240, 12).len(), 12);
        assert!(sheet_indices(240, 12).windows(2).all(|w| w[0] < w[1]));
        // Fewer frames than thumbnails: every frame once, no duplicates.
        assert_eq!(sheet_indices(10, 12), (0..10).collect::<Vec<_>>());
        assert_eq!(sheet_indices(0, 12), Vec::<usize>::new());
    }

    // spec: TEST-013
    #[test]
    fn labels_show_frame_and_altitude_in_a_readable_unit() {
        assert_eq!(label(0, 20_000_000.0), "0000 20000 km");
        assert_eq!(label(60, 3_009.5), "0060 3.0 km");
        assert_eq!(label(150, 214.2), "0150 214 m");
        assert_eq!(label(239, 1.0), "0239 1.0 m");
        assert!(label(7, 12.0).chars().all(|c| glyph(c).is_some()), "every label character has a glyph");
    }

    // spec: TEST-013
    #[test]
    fn a_sheet_places_shrunk_frames_in_a_grid_with_labels() {
        let frames: Vec<(RgbaFrame, String)> = (0..5).map(|i| (solid(640, 360, [10 * i as u8, 200, 50, 255]), label(i, 1.0))).collect();
        let sheet = contact_sheet(&frames, 4, 320);
        // 640 / 320 = factor 2: 320x180 thumbnails, 4 columns and 2 rows, with 4 px gutters.
        assert_eq!((sheet.width, sheet.height), (4 * 320 + 5 * GUTTER, 2 * 180 + 3 * GUTTER));
        let at = |x: u32, y: u32| {
            let i = ((y * sheet.width + x) * 4) as usize;
            [sheet.rgba[i], sheet.rgba[i + 1], sheet.rgba[i + 2], sheet.rgba[i + 3]]
        };
        // The bottom-right of each thumbnail shows its frame (label boxes sit at the top-left), the gutter the background.
        for i in 0..5u32 {
            let (cx, cy) = (i % 4, i / 4);
            let (x, y) = (GUTTER + cx * (320 + GUTTER) + 319, GUTTER + cy * (180 + GUTTER) + 179);
            assert_eq!(at(x, y), [10 * i as u8, 200, 50, 255], "thumbnail {i}");
        }
        assert_eq!(at(0, 0), BACKGROUND);
        // The label box is black with white glyph pixels.
        let first: Vec<[u8; 4]> =
            (GUTTER..GUTTER + 60).flat_map(|x| (GUTTER..GUTTER + 12).map(move |y| (x, y))).map(|(x, y)| at(x, y)).collect();
        assert!(first.contains(&[255, 255, 255, 255]) && first.contains(&[0, 0, 0, 255]));
    }

    // spec: TEST-013
    #[test]
    fn text_outside_the_font_is_blank_not_a_panic() {
        let mut img = solid(40, 20, [9, 9, 9, 255]);
        draw_text(&mut img, 30, 15, 2, "Zq?");
        assert_eq!(img.rgba.len(), 40 * 20 * 4);
    }
}
