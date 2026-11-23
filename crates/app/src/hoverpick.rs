//! The hover pick off the main thread (VIEW-059, CON-22): the cursor ray reads the height field along its whole length, so its first read of a bake
//! block nobody has touched decodes the block (30 to 120 ms), and a frame that waited for it was a stall. One thread of its own runs the pick against a
//! field it makes itself; the frame takes the last pick that finished and asks for the next, so the hover lags by a pick, never waits for one.

use planet_editor::ruler::CursorReadout;
use planet_frame::pick::{PickLimits, Ray, SurfaceHit};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

/// Surface pick result plus off-thread local slope and coordinate readout (VIEW-059, VIEW-076).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverSample {
    pub hit: SurfaceHit,
    pub readout: CursorReadout,
}

/// Legacy hit-only pick callback.
pub type PickSource = Arc<dyn Fn(Ray, PickLimits) -> Option<SurfaceHit> + Send + Sync>;
/// Pick callback returning both surface hit and finite-difference local slope readout off the main thread.
pub type SampleSource = Arc<dyn Fn(Ray, PickLimits) -> Option<HoverSample> + Send + Sync>;

type Request = (Ray, PickLimits);
/// A finished pick: the epoch it started in and what it found.
type Finished = Option<(u64, Option<HoverSample>)>;

/// A pick thread and the last hit/sample it found.
pub struct HoverPicker {
    tx: Sender<(u64, Request)>,
    /// The last finished pick, with the epoch it started in.
    done: Arc<Mutex<Finished>>,
    busy: Arc<AtomicBool>,
    /// Counts the clears: a pick that started before one is dropped.
    epoch: Arc<AtomicU64>,
    last: Option<HoverSample>,
}

impl HoverPicker {
    /// Starts a hover picker from a hit-only callback (zero extra reads; slope reported as 0).
    pub fn new(source: PickSource) -> HoverPicker {
        Self::with_samples(Arc::new(move |ray, limits| {
            let hit = source(ray, limits)?;
            let point = planet_editor::polyline::GeoPoint::of_dir(hit.dir).canonical();
            let readout = CursorReadout { point, height_m: hit.height_m, gradient: 0.0, slope_deg: 0.0, over_limit: false };
            Some(HoverSample { hit, readout })
        }))
    }

    /// Starts a hover picker from a callback yielding full [`HoverSample`] data off the main thread.
    pub fn with_samples(source: SampleSource) -> HoverPicker {
        let (tx, rx) = channel::<(u64, Request)>();
        let done: Arc<Mutex<Finished>> = Arc::new(Mutex::new(None));
        let busy = Arc::new(AtomicBool::new(false));
        let epoch = Arc::new(AtomicU64::new(0));
        let (slot, flag) = (Arc::clone(&done), Arc::clone(&busy));
        std::thread::Builder::new()
            .name("hover pick".into())
            .spawn(move || {
                while let Ok((started, (ray, limits))) = rx.recv() {
                    let hit = source(ray, limits);
                    *slot.lock().expect("hover pick slot") = Some((started, hit));
                    flag.store(false, Ordering::Release);
                }
            })
            .expect("the hover pick thread starts");
        HoverPicker { tx, done, busy, epoch, last: None }
    }

    /// One frame: the last pick that finished is the sample; when none is running, `request` (this frame's ray) starts the next.
    pub fn poll_sample(&mut self, request: Request) -> Option<HoverSample> {
        if let Some((started, hit)) = self.done.lock().expect("hover pick slot").take() {
            if started == self.epoch.load(Ordering::Acquire) {
                self.last = hit;
            }
        }
        if !self.busy.swap(true, Ordering::AcqRel) && self.tx.send((self.epoch.load(Ordering::Acquire), request)).is_err() {
            self.busy.store(false, Ordering::Release);
        }
        self.last
    }

    /// One frame: the last hit that finished (backwards-compatible wrapper around [`HoverPicker::poll_sample`]).
    pub fn poll(&mut self, request: Request) -> Option<SurfaceHit> {
        self.poll_sample(request).map(|s| s.hit)
    }

    /// The pointer is not over the world: no hit, and a pick that started before this is dropped when it finishes.
    pub fn clear(&mut self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        self.last = None;
        self.done.lock().expect("hover pick slot").take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet_core::Vec3;
    use std::sync::mpsc::sync_channel;

    fn request() -> Request {
        (Ray { origin: Vec3::new(2.0, 0.0, 0.0), dir: Vec3::new(-1.0, 0.0, 0.0) }, PickLimits::new(1.0, 0.1))
    }

    fn hit(distance_m: f64) -> SurfaceHit {
        SurfaceHit { point: Vec3::new(1.0, 0.0, 0.0), dir: Vec3::new(1.0, 0.0, 0.0), height_m: 0.0, distance_m }
    }

    // spec: VIEW-061
    #[test]
    fn a_pick_that_blocks_never_holds_the_frame_and_its_hit_arrives_on_a_later_poll() {
        // The pick blocks until it is released after the frames; had a frame waited for it, it would have given up first (an order, not
        // a time: the 10 s only ends a pick that is never released).
        let (release, wait) = channel::<()>();
        let wait = Mutex::new(wait);
        let gave_up = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&gave_up);
        let source: PickSource = Arc::new(move |_, _| {
            if wait.lock().unwrap().recv_timeout(std::time::Duration::from_secs(10)).is_err() {
                flag.store(true, Ordering::SeqCst);
            }
            Some(hit(5.0))
        });
        let mut picker = HoverPicker::new(source);
        assert_eq!(picker.poll(request()), None, "nothing has finished");
        assert_eq!(picker.poll(request()), None, "the pick is still blocked and the frame did not wait");
        release.send(()).unwrap();
        assert!(!gave_up.load(Ordering::SeqCst), "the pick gave up before the frames returned: a frame waited for it");
        let start = std::time::Instant::now();
        let got = loop {
            if let Some(h) = picker.poll(request()) {
                break h;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(5), "the finished pick never arrived");
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(got.distance_m, 5.0);
    }

    // spec: VIEW-061
    #[test]
    fn a_pick_that_started_before_a_clear_is_dropped_when_it_finishes() {
        let (release, wait) = sync_channel::<()>(0);
        let wait = Mutex::new(wait);
        let source: PickSource = Arc::new(move |_, _| {
            wait.lock().unwrap().recv_timeout(std::time::Duration::from_secs(2)).ok();
            Some(hit(1.0))
        });
        let mut picker = HoverPicker::new(source);
        assert_eq!(picker.poll(request()), None);
        picker.clear();
        release.send(()).unwrap();
        let start = std::time::Instant::now();
        while picker.busy.load(Ordering::Acquire) {
            assert!(start.elapsed() < std::time::Duration::from_secs(5), "the pick never finished");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(picker.poll(request()), None, "the pick that started before the clear is not the hit");
    }

    // spec: VIEW-076
    #[test]
    fn with_samples_delivers_surface_hit_and_slope_readout_off_thread() {
        let source: SampleSource = Arc::new(move |_, _| {
            let hit = hit(12.0);
            let readout = CursorReadout {
                point: planet_editor::polyline::GeoPoint::new(10.0, 20.0),
                height_m: 450.0,
                gradient: 1.25,
                slope_deg: 51.3,
                over_limit: true,
            };
            Some(HoverSample { hit, readout })
        });
        let mut picker = HoverPicker::with_samples(source);
        let start = std::time::Instant::now();
        let got = loop {
            if let Some(s) = picker.poll_sample(request()) {
                break s;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(5), "sample never arrived");
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(got.hit.distance_m, 12.0);
        assert!(got.readout.over_limit && got.readout.slope_deg > 50.0);
    }
}
