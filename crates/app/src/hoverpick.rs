//! The hover pick off the main thread (VIEW-059, CON-22): the cursor ray reads the height field along its whole length, so its first read of a bake
//! block nobody has touched decodes the block (30 to 120 ms), and a frame that waited for it was a stall. One thread of its own runs the pick against a
//! field it makes itself; the frame takes the last pick that finished and asks for the next, so the hover lags by a pick, never waits for one.

use planet_frame::pick::{PickLimits, Ray, SurfaceHit};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

/// What a pick needs to run: the ray, the limits and the tile level of the heights it reads; it makes the field it picks against.
pub type PickSource = Arc<dyn Fn(Ray, PickLimits, u8) -> Option<SurfaceHit> + Send + Sync>;

type Request = (Ray, PickLimits, u8);

/// A pick thread and the last hit it found.
pub struct HoverPicker {
    tx: Sender<(u64, Request)>,
    /// The last finished pick, with the epoch it started in.
    done: Arc<Mutex<Option<(u64, Option<SurfaceHit>)>>>,
    busy: Arc<AtomicBool>,
    /// Counts the clears: a pick that started before one is dropped.
    epoch: Arc<AtomicU64>,
    last: Option<SurfaceHit>,
}

impl HoverPicker {
    /// The thread ends when the picker is dropped.
    pub fn new(source: PickSource) -> HoverPicker {
        let (tx, rx) = channel::<(u64, Request)>();
        let done: Arc<Mutex<Option<(u64, Option<SurfaceHit>)>>> = Arc::new(Mutex::new(None));
        let busy = Arc::new(AtomicBool::new(false));
        let epoch = Arc::new(AtomicU64::new(0));
        let (slot, flag) = (Arc::clone(&done), Arc::clone(&busy));
        std::thread::Builder::new()
            .name("hover pick".into())
            .spawn(move || {
                while let Ok((started, (ray, limits, level))) = rx.recv() {
                    let hit = source(ray, limits, level);
                    *slot.lock().expect("hover pick slot") = Some((started, hit));
                    flag.store(false, Ordering::Release);
                }
            })
            .expect("the hover pick thread starts");
        HoverPicker { tx, done, busy, epoch, last: None }
    }

    /// One frame: the last pick that finished is the hit; when none is running, `request` (this frame's ray) starts the next.
    pub fn poll(&mut self, request: Request) -> Option<SurfaceHit> {
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
        (Ray { origin: Vec3::new(2.0, 0.0, 0.0), dir: Vec3::new(-1.0, 0.0, 0.0) }, PickLimits::new(1.0, 0.1), 5)
    }

    fn hit(distance_m: f64) -> SurfaceHit {
        SurfaceHit { point: Vec3::new(1.0, 0.0, 0.0), dir: Vec3::new(1.0, 0.0, 0.0), height_m: 0.0, distance_m }
    }

    // spec: VIEW-061
    #[test]
    fn a_pick_that_blocks_never_holds_the_frame_and_its_hit_arrives_on_a_later_poll() {
        let (release, wait) = sync_channel::<()>(0);
        let wait = Mutex::new(wait);
        let source: PickSource = Arc::new(move |_, _, level| {
            wait.lock().unwrap().recv_timeout(std::time::Duration::from_secs(2)).ok();
            Some(hit(f64::from(level)))
        });
        let mut picker = HoverPicker::new(source);
        let t = std::time::Instant::now();
        assert_eq!(picker.poll(request()), None, "nothing has finished");
        assert_eq!(picker.poll(request()), None, "the pick is still blocked and the frame did not wait");
        assert!(t.elapsed() < std::time::Duration::from_millis(500));
        release.send(()).unwrap();
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
        let source: PickSource = Arc::new(move |_, _, _| {
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
}
