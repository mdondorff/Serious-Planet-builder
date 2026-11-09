//! The level-1 preview on the screen (VIEW-067, A3 11.1): the trunk rivers and the watersheds of the H0 re-route over an edit, as plain data for the egui layer, like the outlines of
//! [`crate::overlay`] (the camera's f64 projection; nothing here touches a window or a GPU). The preview is a non-authoritative field (CON-07): it is held apart from the session's items, replaced when a
//! newer preview is accepted, and removed when the edit is committed or undone.
//!
//! The data comes in as [`PreviewData`]: the lines of the trunk (a cell to its receiver, as directions) with the watershed each drains to, whether the edit changed the cell's routing, and a weight
//! (the discharge). The caller (the window) builds it from `planet_generators::bake::preview::Preview`; this crate does not depend on the generators.

use crate::relevance::ViewState;
use planet_core::Vec3;
use planet_frame::globe::{faces_camera, project};

/// The most lines drawn: the heaviest by weight, so a very dense trunk costs a bounded frame.
pub const MAX_LINES: usize = 20_000;

/// One trunk line: from a cell to its receiver.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewLine {
    pub from: Vec3,
    pub to: Vec3,
    /// The watershed (the outlet cell id) the line drains to.
    pub watershed: u32,
    /// Whether the edit changed this cell's receiver or watershed against the base.
    pub changed: bool,
    /// The discharge of the cell: the heavier lines are the bigger rivers.
    pub weight: f64,
}

/// A preview as the window hands it in.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewData {
    /// The preview's own id (CON-07).
    pub id: u64,
    pub lines: Vec<PreviewLine>,
    pub trunk_cells: usize,
    pub changed_cells: usize,
}

/// The accepted preview and the tickets of a drag: only the newest ticket's result is taken.
#[derive(Debug, Default)]
pub struct PreviewState {
    latest: u64,
    current: Option<PreviewData>,
}

impl PreviewState {
    /// A new edit asks for a preview: the ticket of its result.
    pub fn submit(&mut self) -> u64 {
        self.latest += 1;
        self.latest
    }

    /// A result arrives: it replaces the shown preview when its ticket is the newest, and is dropped otherwise. Returns whether it was taken.
    pub fn accept(&mut self, ticket: u64, data: PreviewData) -> bool {
        if ticket != self.latest {
            return false;
        }
        self.current = Some(data);
        true
    }

    /// The edit was committed or undone: the preview goes, and the results still on their way are stale.
    pub fn clear(&mut self) {
        self.latest += 1;
        self.current = None;
    }

    pub fn current(&self) -> Option<&PreviewData> {
        self.current.as_ref()
    }
}

/// One line on the screen, in window pixels (origin top-left).
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnLine {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub watershed: u32,
    pub changed: bool,
    pub weight: f64,
}

/// The preview as the egui layer draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewOverlay {
    pub id: u64,
    /// The text marking it as a preview, with its numbers.
    pub label: String,
    /// The visible lines, the heaviest first, at most the cap.
    pub lines: Vec<DrawnLine>,
    /// Visible lines beyond the cap, not drawn.
    pub dropped: usize,
}

/// The preview on the screen: the lines with both ends on the side of the planet the camera sees and inside the viewport, the heaviest `max_lines` of them (ties by their order in the data).
/// `ground_m` is the height of the ground above the sphere at a direction: the lines sit on the ground.
pub fn build(data: &PreviewData, view: &ViewState, ground_m: &dyn Fn(Vec3) -> f64, max_lines: usize) -> PreviewOverlay {
    let see = |p: Vec3| -> Option<[f64; 2]> {
        let d = p.normalized();
        faces_camera(view.camera.position.0, d, view.radius_m)
            .then(|| project(&view.camera, d * (view.radius_m + ground_m(d)), view.viewport))
            .flatten()
    };
    let mut visible: Vec<(usize, DrawnLine)> = data
        .lines
        .iter()
        .enumerate()
        .filter_map(|(k, l)| {
            let (a, b) = (see(l.from)?, see(l.to)?);
            Some((k, DrawnLine { a, b, watershed: l.watershed, changed: l.changed, weight: l.weight }))
        })
        .collect();
    visible.sort_by(|x, y| y.1.weight.total_cmp(&x.1.weight).then(x.0.cmp(&y.0)));
    let dropped = visible.len().saturating_sub(max_lines);
    visible.truncate(max_lines);
    PreviewOverlay {
        id: data.id,
        label: format!("preview (not committed): {} trunk cells, {} changed", data.trunk_cells, data.changed_cells),
        lines: visible.into_iter().map(|(_, l)| l).collect(),
        dropped,
    }
}

/// The colour of a watershed as hue in degrees, from its id by an integer hash: the same watershed has the same hue in every frame and on every run.
pub fn watershed_hue(watershed: u32) -> f32 {
    let mut x = u64::from(watershed).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    ((x >> 40) % 360) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::PlanetFixed;
    use planet_frame::camera::Camera;
    use planet_frame::globe::Viewport;

    const R: f64 = 6_371_000.0;

    fn view_above(dir: Vec3, altitude_m: f64) -> ViewState {
        let camera = Camera::at_surface_point(
            PlanetFixed(dir * (R + altitude_m)),
            Vec3::new(0.0, 0.0, 1.0),
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_3,
            1.0,
            1.0,
        );
        ViewState { camera, viewport: Viewport { width_px: 1000.0, height_px: 1000.0 }, radius_m: R }
    }

    fn at(lat_deg: f64, lon_deg: f64) -> Vec3 {
        let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
        Vec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la))
    }

    fn line(from: Vec3, to: Vec3, weight: f64) -> PreviewLine {
        PreviewLine { from, to, watershed: 7, changed: false, weight }
    }

    fn data(lines: Vec<PreviewLine>) -> PreviewData {
        PreviewData { id: 42, trunk_cells: lines.len(), changed_cells: 1, lines }
    }

    // spec: VIEW-067
    #[test]
    fn a_line_is_projected_to_its_pixels_and_a_far_side_or_off_screen_line_is_not_drawn() {
        let c = at(10.0, 20.0);
        let view = view_above(c, 2_000.0);
        // Two points 400 m either side of the point below the camera along a parallel: symmetric about the screen centre.
        let a = at(10.0, 20.0 - 400.0 / R * (180.0 / std::f64::consts::PI) / libm::cos(10f64.to_radians()));
        let b = at(10.0, 20.0 + 400.0 / R * (180.0 / std::f64::consts::PI) / libm::cos(10f64.to_radians()));
        let far = at(-10.0, 200.0);
        let off = at(10.0, 25.0);
        let o = build(&data(vec![line(a, b, 3.0), line(a, far, 9.0), line(far, far, 9.0), line(a, off, 9.0)]), &view, &|_| 0.0, MAX_LINES);
        assert_eq!(o.lines.len(), 1, "only the near line: the others have an end on the far side or off the screen");
        let l = &o.lines[0];
        assert!((l.a[1] - 500.0).abs() < 1.0 && (l.b[1] - 500.0).abs() < 1.0, "on the horizontal midline: {l:?}");
        let half = 400.0 / 2000.0 * 500.0 / libm::tan(std::f64::consts::FRAC_PI_6);
        assert!(((l.b[0] - l.a[0]) / 2.0 - half).abs() < 0.02 * half, "400 m either side of the centre is {half} px: {l:?}");
        assert!(o.label.starts_with("preview (not committed)") && o.dropped == 0 && o.id == 42);
    }

    // spec: VIEW-067
    #[test]
    fn the_heaviest_lines_are_kept_up_to_the_cap_and_the_rest_are_counted() {
        let view = view_above(at(10.0, 20.0), 2_000.0);
        let lines: Vec<PreviewLine> = (0..10)
            .map(|k| {
                let d = 100.0 * k as f64 / R * (180.0 / std::f64::consts::PI);
                line(at(10.0, 20.0 + d), at(10.0, 20.0 + d + 0.0005), if k % 2 == 0 { 1.0 } else { 5.0 })
            })
            .collect();
        let o = build(&data(lines), &view, &|_| 0.0, 4);
        assert_eq!((o.lines.len(), o.dropped), (4, 6));
        assert!(o.lines.windows(2).all(|w| w[0].weight >= w[1].weight), "the heaviest first");
        assert!(o.lines.iter().all(|l| l.weight == 5.0), "the four heaviest are the odd ones");
        // The same data and view give the same pixels and the same order every time.
        assert_eq!(
            o,
            build(
                &data(
                    (0..10)
                        .map(|k| {
                            let d = 100.0 * k as f64 / R * (180.0 / std::f64::consts::PI);
                            line(at(10.0, 20.0 + d), at(10.0, 20.0 + d + 0.0005), if k % 2 == 0 { 1.0 } else { 5.0 })
                        })
                        .collect()
                ),
                &view,
                &|_| 0.0,
                4
            )
        );
    }

    // spec: VIEW-067
    #[test]
    fn a_newer_preview_replaces_the_shown_one_a_stale_result_is_dropped_and_commit_or_undo_removes_it() {
        let mut s = PreviewState::default();
        assert!(s.current().is_none());
        let (t1, t2) = (s.submit(), s.submit());
        let first = PreviewData { id: 1, ..data(vec![]) };
        let second = PreviewData { id: 2, ..data(vec![]) };
        assert!(!s.accept(t1, first.clone()), "the result of the first edit arrives after the second was asked for: dropped");
        assert!(s.current().is_none());
        assert!(s.accept(t2, second.clone()));
        assert_eq!(s.current().map(|p| p.id), Some(2));
        // A newer preview replaces it.
        let t3 = s.submit();
        assert!(s.accept(t3, first));
        assert_eq!(s.current().map(|p| p.id), Some(1), "replaced when the newer one is accepted");
        // The edit is committed (or undone): the preview is gone and a result still on its way is stale.
        let t4 = s.submit();
        s.clear();
        assert!(s.current().is_none());
        assert!(!s.accept(t4, second), "a result for an edit that was committed meanwhile is dropped");
        assert!(s.current().is_none());
    }

    // spec: VIEW-067
    #[test]
    fn a_watershed_has_one_hue_in_every_frame() {
        assert_eq!(watershed_hue(12345), watershed_hue(12345));
        let hues: std::collections::BTreeSet<u32> = (0..200u32).map(|w| watershed_hue(w) as u32).collect();
        assert!(hues.len() > 100, "200 watersheds spread over the hues: {}", hues.len());
        assert!((0..1000u32).all(|w| (0.0..360.0).contains(&watershed_hue(w))));
    }
}
