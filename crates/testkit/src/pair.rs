//! The pair comparison of an edit shown as a preview and the same edit at rest (TEST-077): three frames of one pose, pure over RGBA8 images. The stale frame
//! is the unedited tiles, the preview frame the stale tiles with the REND-033 preview, the exact frame the refreshed tiles. The preview is within its bound
//! when it leaves at most a twentieth of the pixels the edit changes (the bound of `gpu_preview.rs`, REND-033).

use crate::Image;

/// The preview may leave at most `1 / BOUND_DEN` of the pixels the edit changes.
pub const BOUND_DEN: usize = 20;
/// An edit that changes this many pixels or fewer shows too little to judge.
pub const MIN_CHANGED: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairVerdict {
    Within,
    Over,
    /// The edit changed too few pixels in this view (at most [`MIN_CHANGED`]): nothing to judge, never a pass.
    TooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairReport {
    pub total: usize,
    /// Pixels of the stale frame more than `grey` levels from the exact frame (what the edit changes).
    pub changed_by_edit: usize,
    /// Pixels of the preview frame more than `grey` levels from the exact frame (what the preview leaves).
    pub left_after_preview: usize,
    /// Mean and largest of the per-pixel largest channel difference between the preview and the exact frame.
    pub mean_diff: f64,
    pub max_diff: u8,
    /// The bounding box `(x0, y0, x1, y1)` of the pixels left after the preview, inclusive.
    pub bbox: Option<(u32, u32, u32, u32)>,
    pub verdict: PairVerdict,
}

impl PairReport {
    /// One line for a log or a report.
    pub fn line(&self) -> String {
        let at = self.bbox.map_or(String::new(), |(x0, y0, x1, y1)| format!(", at x {x0}..{x1} y {y0}..{y1}"));
        format!(
            "{:?}: the edit changes {} of {} pixels, the preview leaves {}, mean {:.2} max {}{at}",
            self.verdict, self.changed_by_edit, self.total, self.left_after_preview, self.mean_diff, self.max_diff
        )
    }
}

fn channel_diff(a: [u8; 4], b: [u8; 4]) -> u8 {
    (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0)
}

/// Compare the three frames over `grey` levels. The images must have one size.
pub fn compare_pair(stale: &Image, preview: &Image, exact: &Image, grey: u8) -> Result<PairReport, String> {
    let dims = |i: &Image| (i.width, i.height);
    if dims(stale) != dims(exact) || dims(preview) != dims(exact) {
        return Err(format!("the frames differ in size: stale {:?}, preview {:?}, exact {:?}", dims(stale), dims(preview), dims(exact)));
    }
    let (mut changed, mut left, mut sum, mut max) = (0usize, 0usize, 0u64, 0u8);
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    for (k, ((s, p), e)) in stale.pixels().zip(preview.pixels()).zip(exact.pixels()).enumerate() {
        changed += usize::from(channel_diff(s, e) > grey);
        let d = channel_diff(p, e);
        sum += u64::from(d);
        max = max.max(d);
        if d > grey {
            left += 1;
            let (x, y) = (k as u32 % exact.width, k as u32 / exact.width);
            bbox = Some(bbox.map_or((x, y, x, y), |(x0, y0, x1, y1)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y))));
        }
    }
    let total = (exact.width * exact.height) as usize;
    let verdict = if changed <= MIN_CHANGED {
        PairVerdict::TooSmall
    } else if left * BOUND_DEN <= changed {
        PairVerdict::Within
    } else {
        PairVerdict::Over
    };
    Ok(PairReport {
        total,
        changed_by_edit: changed,
        left_after_preview: left,
        mean_diff: sum as f64 / total.max(1) as f64,
        max_diff: max,
        bbox,
        verdict,
    })
}

/// The cause class of a difference: the name of the first view (in the order given) whose pair is over the bound, none when no view is.
pub fn first_over<'a>(views: &[(&'a str, PairReport)]) -> Option<&'a str> {
    views.iter().find(|(_, r)| r.verdict == PairVerdict::Over).map(|(n, _)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 40 x 30 grey frame with a block of `edit` brighter pixels from the top left.
    fn frame(block: u32, level: u8) -> Image {
        let mut i = Image::filled(40, 30, [100, 100, 100, 255]);
        for y in 0..block {
            for x in 0..block {
                i.set_pixel(x, y, [level, level, level, 255]);
            }
        }
        i
    }

    // spec: TEST-077
    #[test]
    fn a_preview_equal_to_the_exact_frame_is_within_and_one_equal_to_the_stale_frame_is_over() {
        let stale = frame(0, 100);
        let exact = frame(10, 160); // the edit changes 100 pixels by 60 levels
        let within = compare_pair(&stale, &exact, &exact, 3).unwrap();
        assert_eq!((within.changed_by_edit, within.left_after_preview, within.verdict), (100, 0, PairVerdict::Within));
        assert_eq!(within.bbox, None);
        let over = compare_pair(&stale, &stale, &exact, 3).unwrap();
        assert_eq!((over.left_after_preview, over.max_diff, over.verdict), (100, 60, PairVerdict::Over));
        assert_eq!(over.bbox, Some((0, 0, 9, 9)));
        assert!((over.mean_diff - 5.0).abs() < 1e-12, "100 pixels off by 60 over 1200: {}", over.mean_diff);
        assert!(over.line().starts_with("Over: the edit changes 100 of 1200 pixels, the preview leaves 100"), "{}", over.line());
    }

    // spec: TEST-077
    #[test]
    fn the_bound_is_a_twentieth_of_the_changed_pixels_and_a_tiny_edit_is_too_small_never_within() {
        let stale = frame(0, 100);
        let exact = frame(20, 160); // 400 pixels change
        let mut preview = exact.clone();
        for x in 0..20 {
            preview.set_pixel(x, 0, [100, 100, 100, 255]); // 20 pixels left: exactly 400 / 20
        }
        assert_eq!(compare_pair(&stale, &preview, &exact, 3).unwrap().verdict, PairVerdict::Within);
        preview.set_pixel(0, 1, [100, 100, 100, 255]); // 21 left
        assert_eq!(compare_pair(&stale, &preview, &exact, 3).unwrap().verdict, PairVerdict::Over);
        let small = frame(7, 160); // 49 pixels
        assert_eq!(compare_pair(&stale, &stale, &small, 3).unwrap().verdict, PairVerdict::TooSmall);
        // A difference within the grey levels is not counted.
        assert_eq!(compare_pair(&stale, &stale, &frame(20, 102), 3).unwrap().verdict, PairVerdict::TooSmall);
    }

    // spec: TEST-077
    #[test]
    fn frames_of_different_sizes_are_refused_and_the_cause_is_the_first_view_over_the_bound() {
        let a = Image::filled(4, 4, [0, 0, 0, 255]);
        let b = Image::filled(5, 4, [0, 0, 0, 255]);
        assert!(compare_pair(&a, &a, &b, 3).unwrap_err().contains("differ in size"));
        let ok = PairReport {
            total: 1,
            changed_by_edit: 100,
            left_after_preview: 0,
            mean_diff: 0.0,
            max_diff: 0,
            bbox: None,
            verdict: PairVerdict::Within,
        };
        let bad = PairReport { verdict: PairVerdict::Over, left_after_preview: 50, ..ok };
        assert_eq!(first_over(&[("normals", ok), ("relief", bad), ("lit", bad)]), Some("relief"));
        assert_eq!(first_over(&[("normals", ok), ("lit", ok)]), None);
    }
}
