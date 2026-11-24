//! Spike only (spike/rg-prototype): the chain's budget field on threads with a floor of its own (lead #163 B3, spikes #170). Moved out of
//! `river_gate_graph` when the spike took main's gate files wholesale (lead #192 and 2026-11-24: the gate lives on main). Bit for bit main's
//! [`crate::river_gate_graph::budget_field`] at the contract's 10 m floor, on any thread count; `rg_chain --budget-floor` lowers the chain's own
//! floor here, while the gate's report keeps 10 m.

/// [`crate::river_gate_graph::budget_field`] on `threads` threads (spike, lead #163 B3): every block's relief and budget are computed alone, so the result is the one-thread result bit for bit.
pub fn budget_field_threads(s0: &[f64], w: usize, h: usize, block: usize, dx: f64, r_relief_m: f64, threads: usize) -> Vec<f64> {
    let (bw, bh) = (w.div_ceil(block), h.div_ceil(block));
    let par_map = |len: usize, f: &(dyn Fn(usize) -> f64 + Sync)| -> Vec<f64> {
        let mut out = vec![0f64; len];
        let t = threads.max(1).min(len.max(1));
        let chunk = len.div_ceil(t);
        std::thread::scope(|s| {
            for (k, part) in out.chunks_mut(chunk.max(1)).enumerate() {
                s.spawn(move || {
                    for (j, v) in part.iter_mut().enumerate() {
                        *v = f(k * chunk + j);
                    }
                });
            }
        });
        out
    };
    let relief: Vec<f64> = par_map(bw * bh, &|b| {
        let (cx, cy) = (((b % bw) * block + block / 2).min(w - 1), ((b / bw) * block + block / 2).min(h - 1));
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for y in cy.saturating_sub(5)..=(cy + 5).min(h - 1) {
            for x in cx.saturating_sub(5)..=(cx + 5).min(w - 1) {
                lo = lo.min(s0[y * w + x]);
                hi = hi.max(s0[y * w + x]);
            }
        }
        hi - lo
    });
    let rad = r_relief_m / (block as f64 * dx);
    let ri = rad.ceil() as i64;
    let offsets: Vec<(i64, i64)> =
        (-ri..=ri).flat_map(|y| (-ri..=ri).map(move |x| (x, y))).filter(|&(x, y)| libm::hypot(x as f64, y as f64) <= rad).collect();
    let budget: Vec<f64> = par_map(bw * bh, &|b| {
        let (bx, by) = ((b % bw) as i64, (b / bw) as i64);
        let mut v: Vec<f64> = offsets
            .iter()
            .filter_map(|&(ox, oy)| {
                let (x, y) = (bx + ox, by + oy);
                (x >= 0 && y >= 0 && x < bw as i64 && y < bh as i64).then(|| relief[y as usize * bw + x as usize])
            })
            .collect();
        let k = v.len() / 2;
        (0.5 * *v.select_nth_unstable_by(k, f64::total_cmp).1).clamp(budget_floor_m(), 50.0)
    });
    (0..w * h).map(|c| budget[(c / w / block) * bw + (c % w) / block]).collect()
}

/// spikes #170 (spike only): the budget field's minimum in millimetres; 10 m is the contract's floor (the gate keeps it), a chain may set a lower one with `--budget-floor`
pub static BUDGET_FLOOR_MM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);

fn budget_floor_m() -> f64 {
    BUDGET_FLOOR_MM.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
}
