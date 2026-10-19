//! Mouse look of the viewer's free camera (VIEW-010), GPU-free and window-free (CON-03): the grab state and the conversion of the
//! pointer's raw relative motion into the look delta that [`crate::freecam::FreeInput`] carries. The `winit` loop only turns its
//! events into calls of this type and applies the grab changes it asks for.
//!
//! The source is the raw relative motion of the pointer in device counts (`DeviceEvent::MouseMotion`), turned into radians as
//! `counts x sensitivity` once per frame and never scaled by the frame time (SPIKE-11: the turn per count is then independent of
//! the frame rate). There is no recentring of the cursor: 86 % of the recentres failed in the spike, and a locked or confined and
//! hidden pointer needs none.

/// Radians per device count (SPIKE-11: 0.0015, which the owner found smooth at normal and at 50 ms frames).
pub const SENSITIVITY_RAD_PER_COUNT: f64 = 0.0015;

/// How the pointer is held while the mouse looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grab {
    /// The pointer is locked in place and only the relative motion arrives.
    Locked,
    /// The pointer stays inside the window.
    Confined,
}

/// The grab modes to try, in order, at a click: `Locked` first, `Confined` when the platform refuses it.
pub const GRAB_ORDER: [Grab; 2] = [Grab::Locked, Grab::Confined];

/// A change of the pointer grab the window has to apply: capture (grab, cursor hidden) or release (cursor shown, no grab).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrabChange {
    Capture,
    Release,
}

/// The mouse-look state: whether the pointer is captured, the raw counts since the last frame, and the grab change not yet
/// applied by the window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MouseLook {
    captured: bool,
    counts: [f64; 2],
    change: Option<GrabChange>,
}

impl MouseLook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn captured(&self) -> bool {
        self.captured
    }

    fn set(&mut self, captured: bool) {
        if self.captured != captured {
            self.captured = captured;
            // A capture and a release in the same frame cancel; a release forgets the motion that is pending.
            self.change = match (self.change, captured) {
                (Some(GrabChange::Release), true) | (Some(GrabChange::Capture), false) => None,
                (_, true) => Some(GrabChange::Capture),
                (_, false) => Some(GrabChange::Release),
            };
        }
        if !captured {
            self.counts = [0.0; 2];
        }
    }

    /// A left click on the window while `free` (the free camera is on): capture the pointer. Nothing happens in the scripted flight.
    pub fn click(&mut self, free: bool) {
        if free {
            self.set(true);
        }
    }

    /// Esc: when the pointer is captured it is released and the key is used up (`true`); otherwise the key keeps its other meaning.
    pub fn escape(&mut self) -> bool {
        let was = self.captured;
        self.set(false);
        was
    }

    /// The window lost the focus: release, forget the pending motion. Regaining the focus does not capture again; a click does.
    pub fn focus_lost(&mut self) {
        self.set(false);
    }

    /// The free camera was left (C, restart): release.
    pub fn release(&mut self) {
        self.set(false);
    }

    /// Raw relative pointer motion in device counts (`dx` to the right, `dy` down). Ignored unless the pointer is captured, and a
    /// non-finite count is ignored.
    pub fn motion(&mut self, dx: f64, dy: f64) {
        if self.captured && dx.is_finite() && dy.is_finite() {
            self.counts[0] += dx;
            self.counts[1] += dy;
        }
    }

    /// The look change of this frame in radians, `[yaw to the right, pitch up]`: counts x sensitivity, not scaled by the frame time.
    /// The counts are used up.
    pub fn take_delta(&mut self) -> [f64; 2] {
        let c = std::mem::take(&mut self.counts);
        [c[0] * SENSITIVITY_RAD_PER_COUNT, -c[1] * SENSITIVITY_RAD_PER_COUNT]
    }

    /// The grab change the window has to apply now, once.
    pub fn take_grab_change(&mut self) -> Option<GrabChange> {
        self.change.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured() -> MouseLook {
        let mut m = MouseLook::new();
        m.click(true);
        m
    }

    // spec: VIEW-010
    #[test]
    fn a_click_in_the_free_camera_captures_and_the_window_is_asked_once() {
        let mut m = MouseLook::new();
        m.click(false);
        assert!(!m.captured() && m.take_grab_change().is_none(), "no capture in the scripted flight");
        m.click(true);
        assert!(m.captured());
        assert_eq!(m.take_grab_change(), Some(GrabChange::Capture));
        assert_eq!(m.take_grab_change(), None, "the change is reported once");
        m.click(true);
        assert_eq!(m.take_grab_change(), None, "a second click while captured changes nothing");
        assert_eq!(GRAB_ORDER, [Grab::Locked, Grab::Confined], "Locked is tried first, Confined is the fallback");
    }

    // spec: VIEW-010
    #[test]
    fn motion_is_counts_times_sensitivity_with_right_and_up_positive_and_is_used_up() {
        let mut m = captured();
        m.motion(100.0, 0.0);
        m.motion(0.0, -40.0);
        m.motion(-10.0, 0.0);
        let d = m.take_delta();
        assert!((d[0] - 90.0 * SENSITIVITY_RAD_PER_COUNT).abs() < 1e-15, "{d:?}");
        assert!((d[1] - 40.0 * SENSITIVITY_RAD_PER_COUNT).abs() < 1e-15, "moving the mouse up (dy < 0) looks up: {d:?}");
        assert_eq!(m.take_delta(), [0.0, 0.0], "the counts are used up");
        assert_eq!(SENSITIVITY_RAD_PER_COUNT, 0.0015);
    }

    // spec: VIEW-010
    #[test]
    fn motion_is_ignored_unless_captured_and_non_finite_counts_count_for_nothing() {
        let mut m = MouseLook::new();
        m.motion(500.0, 500.0);
        assert_eq!(m.take_delta(), [0.0, 0.0]);
        let mut m = captured();
        m.motion(f64::NAN, 3.0);
        m.motion(1.0, f64::INFINITY);
        assert_eq!(m.take_delta(), [0.0, 0.0]);
    }

    // spec: VIEW-010
    #[test]
    fn escape_releases_when_captured_and_is_otherwise_not_used_up() {
        let mut m = captured();
        m.take_grab_change();
        m.motion(10.0, 10.0);
        assert!(m.escape(), "Esc releases the mouse and does not quit");
        assert!(!m.captured());
        assert_eq!(m.take_grab_change(), Some(GrabChange::Release));
        assert_eq!(m.take_delta(), [0.0, 0.0], "the pending motion is forgotten");
        assert!(!m.escape(), "Esc with the mouse free is left to quit");
        assert_eq!(m.take_grab_change(), None);
    }

    // spec: VIEW-010
    #[test]
    fn focus_loss_and_leaving_the_free_camera_release_and_only_a_click_captures_again() {
        for release in [MouseLook::focus_lost as fn(&mut MouseLook), MouseLook::release] {
            let mut m = captured();
            m.take_grab_change();
            m.motion(5.0, 5.0);
            release(&mut m);
            assert!(!m.captured());
            assert_eq!(m.take_grab_change(), Some(GrabChange::Release));
            assert_eq!(m.take_delta(), [0.0, 0.0]);
            m.motion(5.0, 5.0);
            assert_eq!(m.take_delta(), [0.0, 0.0], "regaining the focus does not capture");
            m.click(true);
            assert_eq!(m.take_grab_change(), Some(GrabChange::Capture));
        }
        // A capture and a release between two reads of the change cancel out.
        let mut m = MouseLook::new();
        m.click(true);
        m.focus_lost();
        assert_eq!(m.take_grab_change(), None);
    }

    // spec: VIEW-010
    #[test]
    fn the_turn_does_not_depend_on_how_the_motion_is_cut_into_frames() {
        // 2000 counts as 1, 7 and 2000 frames: the same total angle (the delta never sees a frame time).
        let total = |frames: usize| {
            let mut m = captured();
            let mut yaw = 0.0;
            for i in 0..frames {
                // Spread the counts evenly (the remainder goes to the last frame).
                let share = if i + 1 == frames { 2000.0 - (2000 / frames * (frames - 1)) as f64 } else { (2000 / frames) as f64 };
                m.motion(share, 0.0);
                yaw += m.take_delta()[0];
            }
            yaw
        };
        for frames in [1, 7, 2000] {
            assert!((total(frames) - 2000.0 * SENSITIVITY_RAD_PER_COUNT).abs() < 1e-12, "{frames} frames");
        }
    }
}
