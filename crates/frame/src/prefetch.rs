//! The prefetch source (STRM-018, ADR 0031 point 2.3): where the camera will be a little later, from its recent
//! *translation* only, and the tiles that position needs which the current one does not.
//!
//! GPU-free (CON-03). Everything here is a pure function of the camera path: the sequence of `(time_ms, position)` samples on
//! the injected clock. No view direction, yaw or pitch rate exists in any signature, so a rotation cannot move the prediction
//! (a stationary position gives zero velocity exactly), and no thread count or timing enters (CON-14).
//!
//! - [`TravelEstimator`] keeps the samples of the last [`PrefetchConfig::window_ms`] and answers the velocity over that window.
//!   A jump too large to be travel (a teleport) clears the history, so nothing is extrapolated from the old direction.
//! - [`PrefetchConfig::predict`] extrapolates by [`PrefetchConfig::look_ahead_ms`], capped in distance and kept above the
//!   surface for a descent.
//! - [`prefetch_set`] is R of the predicted position minus R of the current one, ordered coarse before fine, nearer to the
//!   predicted position first. The worker computes it after R ([`crate::pipeline::PlanWorker`]); capping it by the free share
//!   is the managed loop's job ([`cap_to_free`]).

use crate::residency::ResidencySet;
use planet_core::{PlanetFixed, TileId, Vec3};
use planet_streaming::predicted_position;
use std::collections::VecDeque;

/// Tunable constants of the prefetch source. The defaults are conservative starting values; ADR 0031 owner question 2 (how far
/// ahead prefetch looks) is to be decided from the first dive and teleport measurements, by changing these numbers only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrefetchConfig {
    /// Seconds of travel to look ahead, in milliseconds on the injected clock.
    pub look_ahead_ms: u64,
    /// Samples older than this are forgotten; the velocity is the displacement over what remains.
    pub window_ms: u64,
    /// Below this speed (metres per second) the camera counts as stationary: no prefetch.
    pub min_speed_mps: f64,
    /// The predicted displacement never exceeds this many metres.
    pub max_distance_m: f64,
    /// The predicted point never descends below this fraction of the current altitude above the reference sphere (a dive
    /// extrapolated linearly would otherwise end under the surface).
    pub min_altitude_fraction: f64,
    /// A step between consecutive samples larger than `teleport_base_m + teleport_altitude_fraction * altitude` is a teleport,
    /// not travel: the history is cleared. (A dive moves under 1 % of its altitude per frame.)
    pub teleport_base_m: f64,
    pub teleport_altitude_fraction: f64,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            look_ahead_ms: 2000,
            window_ms: 400,
            min_speed_mps: 0.5,
            max_distance_m: 50_000.0,
            min_altitude_fraction: 0.5,
            teleport_base_m: 500.0,
            teleport_altitude_fraction: 0.1,
        }
    }
}

/// The camera path as the prefetch source sees it.
#[derive(Clone, Debug)]
pub struct TravelEstimator {
    config: PrefetchConfig,
    radius_m: f64,
    history: VecDeque<(u64, Vec3)>,
    teleports: u64,
}

impl TravelEstimator {
    /// `radius_m` is the reference sphere altitudes are measured from (the LOD parameters' `radius_m`).
    pub fn new(config: PrefetchConfig, radius_m: f64) -> Self {
        Self { config, radius_m, history: VecDeque::new(), teleports: 0 }
    }

    pub fn config(&self) -> &PrefetchConfig {
        &self.config
    }

    /// Jumps treated as teleports so far.
    pub fn teleports(&self) -> u64 {
        self.teleports
    }

    fn altitude(&self, p: Vec3) -> f64 {
        (p.length() - self.radius_m).max(1.0)
    }

    /// Record the camera position at `now_ms`. Returns whether this sample was a teleport (the history was cleared).
    pub fn observe(&mut self, now_ms: u64, position: PlanetFixed) -> bool {
        let p = position.0;
        let mut teleported = false;
        if let Some(&(t_last, p_last)) = self.history.back() {
            let jump = (p - p_last).length();
            if now_ms < t_last || jump > self.config.teleport_base_m + self.config.teleport_altitude_fraction * self.altitude(p_last) {
                self.history.clear();
                self.teleports += 1;
                teleported = true;
            }
        }
        // Same millisecond as the last sample: keep the newest position.
        if self.history.back().is_some_and(|&(t, _)| t == now_ms) {
            self.history.pop_back();
        }
        self.history.push_back((now_ms, p));
        while self.history.front().is_some_and(|&(t, _)| now_ms - t > self.config.window_ms) {
            self.history.pop_front();
        }
        teleported
    }

    /// The translation velocity (metres per second) over the window; `None` with fewer than two samples.
    pub fn velocity(&self) -> Option<Vec3> {
        let (&(t0, p0), &(t1, p1)) = (self.history.front()?, self.history.back()?);
        (t1 > t0).then(|| (p1 - p0) * (1000.0 / (t1 - t0) as f64))
    }

    /// The position the camera reaches `look_ahead_ms` from now at the current velocity, or `None` when it is (nearly)
    /// stationary or has just teleported.
    pub fn predict(&self) -> Option<PlanetFixed> {
        let (_, here) = *self.history.back()?;
        let v = self.velocity()?;
        let speed = v.length();
        if speed < self.config.min_speed_mps {
            return None;
        }
        let la = self.config.look_ahead_ms;
        let mut d = v * (la as f64 / 1000.0);
        if d.length() > self.config.max_distance_m {
            d = d * (self.config.max_distance_m / d.length());
        }
        // Keep the predicted altitude above a fraction of the current one.
        let h = self.altitude(here);
        let up = here.normalized();
        let down = -d.dot(up);
        let allowed = (1.0 - self.config.min_altitude_fraction) * h;
        if down > allowed {
            d = d * (allowed / down);
        }
        // `predicted_position` is the one extrapolation of the scheduler: displacement as velocity over one second.
        Some(PlanetFixed(predicted_position(here, d, 1000, 1000)))
    }
}

/// Prefetch set: the tiles of `ahead` (R at the predicted position) that `now` (R at the current position) does not hold,
/// as `(tile, distance to the predicted position)`, coarse before fine, then nearer first, then by tile id (a total order, a
/// pure function of the two sets).
pub fn prefetch_set(now: &ResidencySet, ahead: &ResidencySet) -> Vec<(TileId, f64)> {
    let mut out: Vec<(TileId, f64)> = ahead.tiles.iter().filter(|e| !now.contains(e.tile)).map(|e| (e.tile, e.distance_m)).collect();
    out.sort_by(|a, b| a.0.level().cmp(&b.0.level()).then(a.1.total_cmp(&b.1)).then(a.0.cmp(&b.0)));
    out
}

/// The prefetch wants the free share can take: every tile of `set` already resident (it costs nothing), then, in the set's
/// order, as many missing tiles as there are `free_slots`. A full share (no free slot) therefore asks for nothing new
/// (ADR 0031 point 2.3: prefetch never evicts).
pub fn cap_to_free(set: &[(TileId, f64)], free_slots: usize, is_resident: impl Fn(TileId) -> bool) -> Vec<(TileId, f64)> {
    let mut budget = free_slots;
    let mut out = Vec::new();
    for &(t, p) in set {
        if is_resident(t) {
            out.push((t, p));
        } else if budget > 0 {
            budget -= 1;
            out.push((t, p));
        }
    }
    out
}
