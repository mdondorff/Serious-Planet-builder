//! The producer of the level-1 preview (VIEW-068, A3 11.1): one thread of its own builds the resident base of a preview session once and answers the newest ask only, so a drag never waits for
//! a route and never queues a backlog of them. The window asks with the stamps of the definition and takes the answer as the plain data of [`planet_editor::preview_overlay`] (VIEW-067).
//! Nothing here touches a window or a GPU (CON-03); the preview is a non-authoritative field with its own id (CON-07).

use planet_core::Vec3;
use planet_editor::preview_overlay::{PreviewData, PreviewLine};
use planet_generators::bake::h0::NONE;
use planet_generators::bake::preview::{Preview, PreviewBase, PreviewEdit};
use planet_world_def::footprint::{Cap, FootprintItem};
use planet_world_def::{BodyId, WorldDefinition};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

/// The trunk threshold as a share of the base's largest discharge: the rivers that carry at least this much are drawn.
pub const TRUNK_SHARE: f64 = 2.0e-4;

/// What builds the resident base, on the worker's thread, at the first ask.
pub type BaseSource = Box<dyn FnOnce() -> PreviewBase + Send>;

/// A finished ask: its ticket and the preview's data, or why there is none.
pub type Answer = (u64, Result<PreviewData, String>);

/// The stamps of a definition as a preview's edit: every layer that holds a stamp, in the order the layers stack (`stamp_layers`), each layer's items in application order.
pub fn edit_of(def: &WorldDefinition, body: BodyId) -> PreviewEdit {
    let mut stamps: Vec<FootprintItem> = Vec::new();
    for layer in def.stamp_layers(body) {
        if let Ok(items) = def.footprint_items_cap(body, &layer, Cap { centre: Vec3::new(0.0, 0.0, 1.0), angle: core::f64::consts::PI }) {
            stamps.extend(items);
        }
    }
    PreviewEdit { stamps, climate: Vec::new() }
}

/// A preview as the plain data of the overlay: for every trunk cell with a receiver the line to it, its watershed, whether the edit changed its receiver or watershed, and its discharge.
pub fn preview_data(base: &PreviewBase, p: &Preview) -> PreviewData {
    let grid = base.grid();
    let base_routing = base.base_routing();
    let lines = p
        .overlay
        .trunk
        .iter()
        .filter_map(|&c| {
            let r = p.routing.receiver[c as usize];
            (r != NONE).then(|| PreviewLine {
                from: grid.direction(c),
                to: grid.direction(r),
                watershed: p.routing.outlet_of[c as usize],
                changed: p.routing.receiver[c as usize] != base_routing.receiver[c as usize]
                    || p.routing.outlet_of[c as usize] != base_routing.outlet_of[c as usize],
                weight: p.discharge[c as usize],
            })
        })
        .collect();
    PreviewData { id: p.id, lines, trunk_cells: p.overlay.trunk.len(), changed_cells: p.overlay.changed.len() }
}

/// A small synthetic base for tests: `n` cells a face edge, land where a smooth function of the direction is above the sea, a uniform climate.
#[cfg(test)]
pub fn test_source(n: usize) -> BaseSource {
    use planet_generators::bake::h0::GlobalGrid;
    use planet_generators::bake::preview::BaseFields;
    Box::new(move || {
        let grid = GlobalGrid::new(n, 6_371_000.0);
        let len = grid.len();
        let z0: Vec<f64> = (0..len as u32)
            .map(|c| {
                let d = grid.direction(c);
                500.0 * d.x + 300.0 * d.y * d.z + 200.0 * libm::sin(3.0 * d.z) + 40.0 * d.x * d.y
            })
            .collect();
        PreviewBase::new(grid, 6_371_000.0, 0.0, 1e-3, BaseFields { z0, p_mm: vec![800.0; len], t_c: vec![12.0; len] }, 0x1234)
    })
}

/// A thread that answers level-1 preview asks, the newest only.
pub struct PreviewWorker {
    tx: Sender<(u64, PreviewEdit)>,
    done: Arc<Mutex<Option<Answer>>>,
    busy: Arc<AtomicBool>,
}

impl PreviewWorker {
    /// The thread ends when the worker is dropped.
    pub fn new(source: BaseSource) -> PreviewWorker {
        let (tx, rx) = channel::<(u64, PreviewEdit)>();
        let done: Arc<Mutex<Option<Answer>>> = Arc::new(Mutex::new(None));
        let busy = Arc::new(AtomicBool::new(false));
        let (slot, flag) = (Arc::clone(&done), Arc::clone(&busy));
        std::thread::Builder::new()
            .name("level-1 preview".into())
            .spawn(move || {
                let mut source = Some(source);
                let mut base: Option<(PreviewBase, f64)> = None;
                while let Ok(mut ask) = rx.recv() {
                    // The newest ask only: what queued up while the last one ran is stale.
                    while let Ok(newer) = rx.try_recv() {
                        ask = newer;
                    }
                    let (ticket, edit) = ask;
                    flag.store(true, Ordering::SeqCst);
                    if base.is_none() {
                        if let Some(make) = source.take() {
                            let b = make();
                            let top = b.base_discharge().iter().cloned().fold(0.0, f64::max);
                            base = Some((b, TRUNK_SHARE * top));
                        }
                    }
                    let answer = match &base {
                        Some((b, threshold)) => b.preview(&edit, *threshold).map(|p| preview_data(b, &p)),
                        None => Err("the preview base could not be built".to_string()),
                    };
                    *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some((ticket, answer));
                    flag.store(false, Ordering::SeqCst);
                }
            })
            .expect("the level-1 preview thread");
        PreviewWorker { tx, done, busy }
    }

    /// Asks for the preview of `edit`; never blocks. An ask the worker has not started when a newer one arrives is dropped.
    pub fn request(&self, ticket: u64, edit: PreviewEdit) {
        let _ = self.tx.send((ticket, edit));
    }

    /// The newest finished answer, once.
    pub fn poll(&self) -> Option<Answer> {
        self.done.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    /// Whether an ask is being answered now.
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_generators::bake::preview::ClimateRegion;

    fn wait(w: &PreviewWorker, ticket: u64) -> Answer {
        let started = std::time::Instant::now();
        loop {
            if let Some(a) = w.poll() {
                if a.0 >= ticket {
                    return a;
                }
            }
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "no answer for ticket {ticket} within 30 s");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn rain(dir: Vec3, factor: f64) -> PreviewEdit {
        PreviewEdit {
            stamps: Vec::new(),
            climate: vec![ClimateRegion { dir, reach_m: 2_000_000.0, rain_factor: factor, temp_offset_c: 0.0 }],
        }
    }

    // spec: VIEW-068
    #[test]
    fn the_worker_answers_the_newest_ask_and_an_invalid_edit_is_an_error_that_does_not_stop_it() {
        let w = PreviewWorker::new(test_source(12));
        assert!(w.poll().is_none(), "nothing asked, nothing answered, and no base is built yet");
        let d = Vec3::new(1.0, 0.0, 0.0);
        // Three asks in a row: the newest is answered (the older ones may or may not have run).
        w.request(1, rain(d, 2.0));
        w.request(2, rain(d, 3.0));
        w.request(3, rain(d, 4.0));
        let (ticket, answer) = wait(&w, 3);
        assert_eq!(ticket, 3, "the newest ask is the one answered last");
        let data = answer.expect("a valid edit gives a preview");
        assert!(data.trunk_cells > 0 && data.lines.len() <= data.trunk_cells);
        // An invalid edit is reported, and the worker still answers the next ask.
        w.request(4, rain(d, f64::NAN));
        let (ticket, answer) = wait(&w, 4);
        assert_eq!(ticket, 4);
        assert!(answer.is_err(), "a NaN rainfall factor is an error, not a panic");
        w.request(5, rain(d, 2.0));
        assert!(wait(&w, 5).1.is_ok(), "the worker is still there");
    }

    // spec: VIEW-068
    #[test]
    fn the_preview_data_has_a_line_for_every_trunk_cell_with_a_receiver_its_watershed_discharge_and_changed_flag() {
        let b = test_source(12)();
        let edit = rain(Vec3::new(0.0, 1.0, 0.0), 3.0);
        let p = b.preview(&edit, 1.0e12).unwrap();
        let data = preview_data(&b, &p);
        let with_receiver = p.overlay.trunk.iter().filter(|&&c| p.routing.receiver[c as usize] != NONE).count();
        assert_eq!(
            (data.lines.len(), data.trunk_cells, data.changed_cells, data.id),
            (with_receiver, p.overlay.trunk.len(), p.overlay.changed.len(), p.id)
        );
        assert!(data.lines.iter().all(|l| !l.changed), "a rainfall edit changes no receiver: no line is marked changed");
        let c = p.overlay.trunk.iter().copied().find(|&c| p.routing.receiver[c as usize] != NONE).expect("a trunk cell with a receiver");
        let l = &data.lines[0];
        assert_eq!((l.watershed, l.weight), (p.routing.outlet_of[c as usize], p.discharge[c as usize]));
        assert!(l.from == b.grid().direction(c) && l.to == b.grid().direction(p.routing.receiver[c as usize]));
    }

    // spec: VIEW-068
    #[test]
    fn a_definition_without_stamps_is_an_empty_edit() {
        let def = WorldDefinition::single_planet(6_371_000.0, 7);
        assert!(edit_of(&def, BodyId::PLANET).stamps.is_empty());
    }
}
