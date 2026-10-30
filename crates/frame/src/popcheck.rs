//! The level-pop detector of a descent (TEST-071): pure over a trace of the drawn heights of fixed probes, no GPU. A probe whose drawn level
//! changes between two frames must not jump on screen; the morph (LOD-008) is what hides the switch.

/// One probe in one frame: the level of the node that draws it, the height that node draws there and the distance from the eye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub level: u8,
    pub height_m: f64,
    pub distance_m: f64,
}

/// A level change that jumped: where, between which levels and by how much.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pop {
    pub frame: usize,
    pub probe: usize,
    pub level_before: u8,
    pub level_after: u8,
    pub jump_m: f64,
    pub jump_px: f64,
}

/// The p99 of the jumps at level changes over which the verdict fails, in pixels.
pub const P99_LIMIT_PX: f64 = 0.5;
/// The largest jump at a level change over which the verdict fails, in pixels.
pub const MAX_LIMIT_PX: f64 = 2.0;
/// Fewer level changes than this prove nothing.
pub const MIN_CHANGES: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Pass { changes: usize, p99_px: f64, max_px: f64 },
    Fail { changes: usize, p99_px: f64, worst: Pop },
    NotExercised { changes: usize },
}

impl Verdict {
    /// One line for a log or a failure message.
    pub fn line(&self) -> String {
        match self {
            Verdict::Pass { changes, p99_px, max_px } => {
                format!("PASS: {changes} level changes, p99 {p99_px:.3} px, largest {max_px:.3} px")
            }
            Verdict::Fail { changes, p99_px, worst } => format!(
                "FAIL: {changes} level changes, p99 {p99_px:.3} px; worst at frame {} probe {} level {} -> {}: {:.3} m = {:.3} px",
                worst.frame, worst.probe, worst.level_before, worst.level_after, worst.jump_m, worst.jump_px
            ),
            Verdict::NotExercised { changes } => format!("NOT EXERCISED: {changes} level changes (at least {MIN_CHANGES} needed)"),
        }
    }
}

/// The jumps at the level changes of `trace[frame][probe]` (`None`: the probe is not covered that frame), in pixels at `focal_px`.
pub fn level_change_jumps(trace: &[Vec<Option<Sample>>], focal_px: f64) -> Vec<Pop> {
    let mut out = Vec::new();
    for (frame, pair) in trace.windows(2).enumerate() {
        for (probe, (a, b)) in pair[0].iter().zip(&pair[1]).enumerate() {
            if let (Some(a), Some(b)) = (a, b) {
                if a.level != b.level {
                    let jump_m = (b.height_m - a.height_m).abs();
                    out.push(Pop {
                        frame: frame + 1,
                        probe,
                        level_before: a.level,
                        level_after: b.level,
                        jump_m,
                        jump_px: jump_m * focal_px / b.distance_m,
                    });
                }
            }
        }
    }
    out
}

/// The verdict of a descent (TEST-071).
pub fn judge(trace: &[Vec<Option<Sample>>], focal_px: f64) -> Verdict {
    let jumps = level_change_jumps(trace, focal_px);
    let changes = jumps.len();
    if changes < MIN_CHANGES {
        return Verdict::NotExercised { changes };
    }
    let mut px: Vec<f64> = jumps.iter().map(|p| p.jump_px).collect();
    px.sort_by(f64::total_cmp);
    let p99_px = px[(px.len() * 99).div_ceil(100) - 1];
    let max_px = px[px.len() - 1];
    if p99_px > P99_LIMIT_PX || max_px > MAX_LIMIT_PX {
        let worst = jumps.iter().copied().max_by(|a, b| a.jump_px.total_cmp(&b.jump_px)).unwrap();
        return Verdict::Fail { changes, p99_px, worst };
    }
    Verdict::Pass { changes, p99_px, max_px }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(level: u8, height_m: f64) -> Option<Sample> {
        Some(Sample { level, height_m, distance_m: 1000.0 })
    }

    /// 12 probes whose level goes 3 -> 4 at frame 1, the height moving by `jump` there.
    fn trace(jump: f64) -> Vec<Vec<Option<Sample>>> {
        vec![vec![s(3, 10.0); 12], vec![s(4, 10.0 + jump); 12]]
    }

    // spec: TEST-071
    #[test]
    fn a_smooth_trace_passes_and_a_planted_jump_fails_naming_the_frame_probe_and_levels() {
        assert!(matches!(judge(&trace(0.0), 1000.0), Verdict::Pass { changes: 12, .. }));
        let mut bad = trace(0.0);
        bad[1][5] = s(4, 12.0); // 2 m at 1000 m with a 1000 px focal length: 2 px
        bad[1][7] = s(4, 13.0); // 3 px
        match judge(&bad, 1000.0) {
            Verdict::Fail { worst, .. } => {
                assert_eq!((worst.frame, worst.probe, worst.level_before, worst.level_after), (1, 7, 3, 4));
                assert!((worst.jump_m - 3.0).abs() < 1e-12 && (worst.jump_px - 3.0).abs() < 1e-12);
            }
            other => panic!("{other:?}"),
        }
    }

    // spec: TEST-071
    #[test]
    fn too_few_level_changes_prove_nothing() {
        let mut t = trace(0.0);
        for probe in t[1].iter_mut().take(3) {
            *probe = s(3, 10.0);
        }
        assert_eq!(judge(&t, 1000.0), Verdict::NotExercised { changes: 9 });
        let none = vec![vec![s(3, 1.0); 20]; 4];
        assert_eq!(judge(&none, 1000.0), Verdict::NotExercised { changes: 0 });
    }

    // spec: TEST-071
    #[test]
    fn an_uncovered_probe_and_a_jump_without_a_level_change_are_not_level_pops() {
        let mut t = trace(0.0);
        t[0][0] = None;
        t[1][1] = s(3, 99.0);
        let jumps = level_change_jumps(&t, 1000.0);
        assert_eq!(jumps.len(), 10, "the uncovered probe and the unchanged level are left out");
        assert!(jumps.iter().all(|p| p.jump_m == 0.0));
    }
}
