//! The camera's ground off the main thread (VIEW-061, CON-22): the free camera reads the terrain height under itself every frame, and the
//! first read of a bake block nobody has decoded costs 20 ms on an idle machine and 70 to 320 ms while the tile streamer is busy (SPIKE-88). One
//! thread of its own reads the height at the camera's nadir; the frame takes the last answer that finished and asks for the next, so the ground
//! lags by at most the frame that asked, never waits for a read.

use planet_core::Vec3;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

/// The height in metres above the radius at a unit direction, read on the ground thread (it may decode bake blocks and pin them).
pub type GroundSource = Arc<dyn Fn(Vec3) -> f64 + Send + Sync>;

/// A finished read: the direction it was asked for and the height there.
pub type GroundAnswer = (Vec3, f64);

/// A ground thread and the last height it found.
pub struct CameraGround {
    tx: Sender<Vec3>,
    done: Arc<Mutex<Option<GroundAnswer>>>,
    busy: Arc<AtomicBool>,
    last: Option<GroundAnswer>,
}

impl CameraGround {
    /// The thread ends when the reader is dropped.
    pub fn new(source: GroundSource) -> CameraGround {
        let (tx, rx) = channel::<Vec3>();
        let done: Arc<Mutex<Option<GroundAnswer>>> = Arc::new(Mutex::new(None));
        let busy = Arc::new(AtomicBool::new(false));
        let (slot, flag) = (Arc::clone(&done), Arc::clone(&busy));
        std::thread::Builder::new()
            .name("camera ground".into())
            .spawn(move || {
                while let Ok(dir) = rx.recv() {
                    let h = source(dir);
                    *slot.lock().expect("camera ground slot") = Some((dir, h));
                    flag.store(false, Ordering::Release);
                }
            })
            .expect("the camera ground thread starts");
        CameraGround { tx, done, busy, last: None }
    }

    /// One frame: the last read that finished is the answer (`None` until the first finishes); when none is running, `dir` (this frame's
    /// nadir) starts the next. Never waits.
    pub fn poll(&mut self, dir: Vec3) -> Option<GroundAnswer> {
        if let Some(answer) = self.done.lock().expect("camera ground slot").take() {
            self.last = Some(answer);
        }
        if !self.busy.swap(true, Ordering::AcqRel) && self.tx.send(dir).is_err() {
            self.busy.store(false, Ordering::Release);
        }
        self.last
    }

    /// The last answer, without asking again.
    pub fn last(&self) -> Option<GroundAnswer> {
        self.last
    }
}

/// The answer's height when it may stand for the ground under a camera at `position` (planet-fixed, metres) on a planet of `radius_m`: the
/// answer's direction is within `max(1 km, half the camera's height above that ground)` of the camera's nadir along the surface, and the height
/// is finite. `None` (no answer yet, or a jump: a restart, a landing, a teleport) means the frame reads the ground itself (VIEW-061).
pub fn usable_ground(answer: Option<GroundAnswer>, position: Vec3, radius_m: f64) -> Option<f64> {
    let (dir, h) = answer?;
    let len = position.length();
    if !(h.is_finite() && len.is_finite() && len > 0.0) {
        return None;
    }
    let nadir = position * (1.0 / len);
    let apart_m = libm::atan2(dir.cross(nadir).length(), dir.dot(nadir)) * radius_m;
    let above_m = len - (radius_m + h);
    (apart_m <= (0.5 * above_m).max(1_000.0)).then_some(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;
    use std::time::{Duration, Instant};

    fn wait_for(ground: &mut CameraGround, dir: Vec3, want: Vec3) -> GroundAnswer {
        let start = Instant::now();
        loop {
            if let Some(a) = ground.poll(dir).filter(|a| a.0 == want) {
                return a;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "the read of {want:?} never arrived");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    // spec: VIEW-061
    #[test]
    fn the_ground_is_the_last_finished_read_at_most_one_request_behind() {
        let source: GroundSource = Arc::new(|d: Vec3| d.x * 1000.0);
        let mut ground = CameraGround::new(source);
        let (a, b) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.5, 0.0, 0.0));
        assert_eq!(ground.poll(a), None, "nothing has finished on the first frame");
        // The frame that asked for `a` is answered by a later frame with `a`'s height, never a stale one.
        assert_eq!(wait_for(&mut ground, b, a), (a, 1000.0));
        // `b` was asked on the first frame after `a` finished: the next answer is `b`, one request behind the frame.
        assert_eq!(wait_for(&mut ground, b, b), (b, 500.0));
    }

    // spec: VIEW-061
    #[test]
    fn an_answer_stands_for_the_ground_near_its_nadir_and_a_jump_reads_the_ground_again() {
        let r = 6_371_000.0;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let at = |along_m: f64, above_m: f64| {
            let a = along_m / r;
            Vec3::new(libm::sin(a), 0.0, libm::cos(a)) * (r + 500.0 + above_m)
        };
        let answer = Some((up, 500.0));
        // Near the ground: within 1 km of the answered nadir.
        assert_eq!(usable_ground(answer, at(900.0, 2.0), r), Some(500.0));
        assert_eq!(usable_ground(answer, at(1_100.0, 2.0), r), None, "a jump of 1.1 km at 2 m up reads again");
        // High up the tolerance grows with the height: half of it.
        assert_eq!(usable_ground(answer, at(40_000.0, 100_000.0), r), Some(500.0));
        assert_eq!(usable_ground(answer, at(60_000.0, 100_000.0), r), None);
        // No answer, or one that is not finite.
        assert_eq!(usable_ground(None, at(0.0, 2.0), r), None);
        assert_eq!(usable_ground(Some((up, f64::NAN)), at(0.0, 2.0), r), None);
    }

    // spec: VIEW-061
    #[test]
    fn a_slow_decode_never_holds_the_frame() {
        // The read blocks (a cold block decode under load) until it is released after the frames; a frame that waited would have timed it out.
        let (release, wait) = sync_channel::<()>(0);
        let wait = Mutex::new(wait);
        let gave_up = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&gave_up);
        let source: GroundSource = Arc::new(move |_| {
            if wait.lock().unwrap().recv_timeout(Duration::from_secs(10)).is_err() {
                flag.store(true, Ordering::SeqCst);
            }
            42.0
        });
        let mut ground = CameraGround::new(source);
        let d = Vec3::new(0.0, 0.0, 1.0);
        // An order, not a time (no wall-clock limit in tier A): ten frames return while the read is blocked.
        for _ in 0..10 {
            assert_eq!(ground.poll(d), None, "the read is still blocked and the frame did not wait");
        }
        release.send(()).unwrap();
        assert!(!gave_up.load(Ordering::SeqCst), "the read gave up before the frames returned: a frame waited for it");
        assert_eq!(wait_for(&mut ground, d, d).1, 42.0);
    }
}
