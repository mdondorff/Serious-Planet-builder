//! The plan node of the reserve test (BUDG-011, CON-03): the GPU work a reserved share stands for, as a plan of a synthetic scene. The renderer executes it (`planet_render::BurnPass`);
//! the frame crate decides what it asks for. Only the plans of test scenes carry a `ReservePlan`; a production plan (`FramePlan`, `TerrainPlan`) has no field for one.

/// The GPU milliseconds one reserved share asks for each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurnNode {
    pub share: &'static str,
    pub gpu_ms: f64,
}

/// The burn nodes of a synthetic scene, in the order the scene encodes them before the terrain passes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReservePlan {
    pub burns: Vec<BurnNode>,
}

impl ReservePlan {
    /// A plan with a node for every share that asks for GPU time (`gpu_ms > 0`): `(share, gpu ms)` pairs.
    pub fn from_loads(loads: &[(&'static str, f64)]) -> ReservePlan {
        ReservePlan { burns: loads.iter().filter(|(_, ms)| *ms > 0.0).map(|&(share, gpu_ms)| BurnNode { share, gpu_ms }).collect() }
    }

    /// The GPU milliseconds of the whole plan per frame.
    pub fn total_gpu_ms(&self) -> f64 {
        self.burns.iter().map(|b| b.gpu_ms).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: BUDG-011
    #[test]
    fn only_shares_with_gpu_time_get_a_burn_node() {
        let plan = ReservePlan::from_loads(&[("sky", 3.0), ("physics", 0.5), ("game-logic", 0.0), ("networking", 0.0)]);
        assert_eq!(plan.burns.iter().map(|b| b.share).collect::<Vec<_>>(), vec!["sky", "physics"]);
        assert_eq!(plan.total_gpu_ms(), 3.5);
        assert!(ReservePlan::default().burns.is_empty());
    }
}
