//! The core shares (BUDG-003, ADR 0092): ceilings on threads busy at once per subsystem, as fractions of the background capacity
//! `B = max(P − 2, 1)` of the machine's performance threads (BUDG-013). A pure function of the topology in integer arithmetic, so every
//! count the job system and the reserve test size from it is the same wherever it is read; no machine's count is a rule.

use crate::topology::CpuTopology;

/// Threads outside the shares: the frame's main and render threads (BUDG-003).
pub const OUTSIDE_SHARES: usize = 2;

/// One row of the fraction table: the share's name, its percentage of `B`, its minimum and its maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareRow {
    pub name: &'static str,
    pub pct: usize,
    pub min: usize,
    pub max: usize,
}

/// BUDG-003's fraction table. Terrain's 60 % is the owner's (ADR 0092); the other rows are provisional placeholders until a game-engine
/// integration measures them.
pub const TABLE: [ShareRow; 5] = [
    ShareRow { name: "terrain", pct: 60, min: 2, max: 10 },
    ShareRow { name: "physics", pct: 15, min: 1, max: 4 },
    ShareRow { name: "game-logic", pct: 15, min: 1, max: 4 },
    ShareRow { name: "sky", pct: 5, min: 1, max: 2 },
    ShareRow { name: "networking", pct: 5, min: 1, max: 1 },
];

/// The shares of one machine, in [`TABLE`]'s order, and the capacity they were cut from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shares {
    pub capacity: usize,
    pub terrain: usize,
    pub physics: usize,
    pub game_logic: usize,
    pub sky: usize,
    pub networking: usize,
}

impl Shares {
    /// The shares of `topology` (BUDG-003): `B = max(P − 2, 1)`; each share `clamp(floor(pct × B / 100), min, max)`; if they sum to more
    /// than `B`, terrain gives up threads down to its minimum; a machine whose minimums still exceed `B` runs at the minimums.
    pub fn for_topology(topology: CpuTopology) -> Shares {
        let capacity = topology.p.saturating_sub(OUTSIDE_SHARES).max(1);
        let n: Vec<usize> = TABLE.iter().map(|r| (r.pct * capacity / 100).clamp(r.min, r.max)).collect();
        let others: usize = n[1..].iter().sum();
        let terrain = n[0].min(capacity.saturating_sub(others)).max(TABLE[0].min);
        Shares { capacity, terrain, physics: n[1], game_logic: n[2], sky: n[3], networking: n[4] }
    }

    /// The shares of this machine: the detected topology (BUDG-013), or every usable thread as performance when the override is
    /// malformed (the error is the caller's to print).
    pub fn machine() -> Shares {
        Shares::for_topology(crate::topology::detect().unwrap_or(CpuTopology { p: crate::topology::available_threads(), e: 0 }))
    }

    /// The share named `name` in [`TABLE`] (`None` for another name).
    pub fn of(&self, name: &str) -> Option<usize> {
        match name {
            "terrain" => Some(self.terrain),
            "physics" => Some(self.physics),
            "game-logic" => Some(self.game_logic),
            "sky" => Some(self.sky),
            "networking" => Some(self.networking),
            _ => None,
        }
    }

    /// The sum of the shares (more than the capacity only on a machine below the minimum).
    pub fn sum(&self) -> usize {
        self.terrain + self.physics + self.game_logic + self.sky + self.networking
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shares(p: usize, e: usize) -> [usize; 6] {
        let s = Shares::for_topology(CpuTopology { p, e });
        [s.capacity, s.terrain, s.physics, s.game_logic, s.sky, s.networking]
    }

    // spec: BUDG-003
    #[test]
    fn the_shares_of_the_worked_examples_and_e_cores_change_nothing() {
        // Rasierklinge, 6 P-cores/12 threads + 8 E-cores: B = 10.
        assert_eq!(shares(12, 8), [10, 6, 1, 1, 1, 1]);
        assert_eq!(shares(12, 0), shares(12, 8), "E-cores are not counted");
        // An 8-core/16-thread desktop: B = 14.
        assert_eq!(shares(16, 0), [14, 8, 2, 2, 1, 1]);
        // A 4-core/8-thread laptop: B = 6, terrain 3 fitted to 2.
        assert_eq!(shares(8, 0), [6, 2, 1, 1, 1, 1]);
        // A 2-core/4-thread machine: B = 2, below the minimum: every share at its minimum, oversubscribed.
        let small = Shares::for_topology(CpuTopology { p: 4, e: 0 });
        assert_eq!(shares(4, 0), [2, 2, 1, 1, 1, 1]);
        assert_eq!(small.sum(), 6);
        // Very small and very large machines: the capacity is at least 1, terrain at most its maximum.
        assert_eq!(shares(1, 0)[0], 1);
        assert_eq!(shares(64, 0)[1], 10);
        assert_eq!(Shares::for_topology(CpuTopology { p: 12, e: 8 }).of("terrain"), Some(6));
        assert_eq!(Shares::for_topology(CpuTopology { p: 12, e: 8 }).of("gpu"), None);
    }

    // spec: BUDG-003
    #[test]
    fn the_shares_never_sum_above_the_capacity_unless_the_minimums_do() {
        let mins: usize = TABLE.iter().map(|r| r.min).sum();
        for p in 1..=128 {
            let s = Shares::for_topology(CpuTopology { p, e: 0 });
            if s.capacity >= mins {
                assert!(s.sum() <= s.capacity, "P = {p}: {s:?}");
            } else {
                assert_eq!(s.sum(), mins, "P = {p}: at the minimums: {s:?}");
            }
            assert!(s.terrain >= TABLE[0].min && s.terrain <= TABLE[0].max, "P = {p}: {s:?}");
        }
        assert_eq!(TABLE.iter().map(|r| r.pct).sum::<usize>(), 100);
    }
}
