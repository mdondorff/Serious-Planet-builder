//! What the pointer does on the planet (VIEW-019): hover, press, drag and release over the session. The window resolves a cursor into the
//! plain data of [`PointerView`] (the surface hit, the handles on the screen, the edits shown) and this module decides; nothing here
//! touches a window or a GPU, so every flow is a tier-A test.
//!
//! A press takes a handle of the selected edit when the cursor is within the handle's hit radius, else places a stamp when the stamp tool
//! is on and the cursor is on the surface, else selects the edit under the cursor, else clears the selection. While a handle is held the
//! pointer moves it; the release ends the drag as one command.

use crate::commands::Affected;
use crate::session::{height_delta_m, Session, SessionError, Tool};
use crate::shape::Handle;
use planet_frame::pick::SurfaceHit;
use planet_world_def::item_store::ItemStore;
use planet_world_def::ItemId;

/// What the window knows of the pointer at one moment.
#[derive(Clone, Copy, Debug)]
pub struct PointerView<'a> {
    /// The cursor in the pixels the handles are given in.
    pub cursor: [f64; 2],
    /// The surface under the cursor, if the ray meets one.
    pub hit: Option<SurfaceHit>,
    /// The edits the camera shows (VIEW-015): only these can be picked.
    pub shown: &'a [ItemId],
    /// The handles of the selected edit on the screen.
    pub handles: &'a [(Handle, [f64; 2])],
    /// Metres one pixel spans at the selected edit (`shape::metres_per_pixel`), for the height handle.
    pub metres_per_pixel: f64,
}

/// What a press did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    /// A handle of the selected edit is held.
    Handle(Handle),
    /// A stamp was placed (and selected).
    Placed(ItemId),
    /// An edit was selected.
    Selected(ItemId),
    /// The selection was cleared (the press was on nothing).
    Cleared,
    /// Nothing: the stamp tool was on and the cursor was off the planet.
    Nothing,
}

/// The pointer's state between events.
#[derive(Clone, Debug)]
pub struct Interaction {
    /// The radius in pixels around a handle in which a press takes it.
    pub handle_hit_px: f64,
    /// The cursor at the last event of a drag, for the height handle.
    last_cursor: Option<[f64; 2]>,
}

impl Interaction {
    pub fn new(handle_hit_px: f64) -> Interaction {
        Interaction { handle_hit_px, last_cursor: None }
    }

    /// The handle nearest to the cursor within the hit radius (ties to the earlier one in the list).
    pub fn handle_under(&self, v: &PointerView) -> Option<Handle> {
        let mut best: Option<(f64, Handle)> = None;
        for (h, p) in v.handles {
            let d = libm::hypot(p[0] - v.cursor[0], p[1] - v.cursor[1]);
            if d <= self.handle_hit_px && best.is_none_or(|b| d < b.0) {
                best = Some((d, *h));
            }
        }
        best.map(|b| b.1)
    }

    /// The pointer moved with no button down: the edit under it is the hovered one (none while a handle is held or over a handle).
    pub fn hover<S: ItemStore>(&mut self, s: &mut Session<S>, v: &PointerView) {
        if s.dragging().is_some() {
            return;
        }
        if self.handle_under(v).is_some() {
            s.set_hover(None);
            return;
        }
        let under = v.hit.and_then(|h| s.pick(&h, v.shown));
        s.set_hover(under);
    }

    /// The primary button went down.
    pub fn press<S: ItemStore>(&mut self, s: &mut Session<S>, v: &PointerView) -> Result<(Pressed, Vec<Affected>), SessionError> {
        if let Some(h) = self.handle_under(v) {
            s.begin_drag(h)?;
            self.last_cursor = Some(v.cursor);
            return Ok((Pressed::Handle(h), Vec::new()));
        }
        if let (Tool::Stamp(kind), Some(hit)) = (s.tool().clone(), v.hit) {
            let (id, affected) = s.place(&kind, hit.dir)?;
            return Ok((Pressed::Placed(id), affected));
        }
        if matches!(s.tool(), Tool::Stamp(_)) {
            return Ok((Pressed::Nothing, Vec::new()));
        }
        match v.hit.and_then(|h| s.pick(&h, v.shown)) {
            Some(id) => {
                s.select(id);
                Ok((Pressed::Selected(id), Vec::new()))
            }
            None => {
                s.deselect();
                Ok((Pressed::Cleared, Vec::new()))
            }
        }
    }

    /// The pointer moved with the button down: a held handle follows it. Without a surface hit a position or radius handle stays where
    /// it is.
    pub fn moved<S: ItemStore>(&mut self, s: &mut Session<S>, v: &PointerView) -> Result<Vec<Affected>, SessionError> {
        let Some(handle) = s.dragging() else { return Ok(Vec::new()) };
        let last = self.last_cursor.replace(v.cursor).unwrap_or(v.cursor);
        match handle {
            Handle::Move | Handle::Radius => match v.hit {
                Some(hit) => s.drag_surface(hit.dir),
                None => Ok(Vec::new()),
            },
            Handle::Height => {
                let dy = v.cursor[1] - last[1];
                if dy == 0.0 {
                    Ok(Vec::new())
                } else {
                    s.drag_height(height_delta_m(dy, v.metres_per_pixel))
                }
            }
        }
    }

    /// The button went up: a held handle is let go and the drag becomes one command. `None` when no handle was held.
    pub fn release<S: ItemStore>(&mut self, s: &mut Session<S>) -> Result<Option<Vec<Affected>>, SessionError> {
        self.last_cursor = None;
        if s.dragging().is_none() {
            return Ok(None);
        }
        s.end_drag().map(Some)
    }

    /// The drag was abandoned (Esc, the window lost the focus): the edit goes back.
    pub fn cancel<S: ItemStore>(&mut self, s: &mut Session<S>) -> Result<Option<Vec<Affected>>, SessionError> {
        self.last_cursor = None;
        if s.dragging().is_none() {
            return Ok(None);
        }
        s.cancel_drag().map(Some)
    }
}
