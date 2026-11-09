//! The stage probe (BUILD-046, GEN-161): every stage of the new chain (landmass, each round, H1, thermal, deposition, pit spreading, K7) can show its real output as a review sheet with a fixed colour scale
//! and a line of key numbers, at any scale, through the production path and no other. The stages call [`emit`] at their boundaries with a closure that builds a [`StageView`] of borrowed data; a sink
//! installed by the caller ([`install`], off by default) receives it. Nothing here writes to the data it is shown (the hooks only read, and a test compares every output bit with the probe on and off).
//!
//! Off, [`emit`] is one atomic load: the closure does not run, nothing is allocated, no surface is read. On, the [`SheetProbe`] downsamples a field into at most `max_bins` bins (a bounded buffer, so the
//! probe does not raise a stage's peak by more than `max_bins` times 12 bytes plus a 4,096-bin histogram, whatever the grid), writes `<run>/<stage>-<field>.png` and appends a line to `<run>/stages.txt`
//! with the cell count, minimum, 5th, 50th and 95th percentile (from the histogram) and maximum, a 64-bit hash of the data, the numbers the stage gave, and the probe's own cost in milliseconds and bytes.
//! The sheets are review aids, not goldens (ADR 0017 does not apply).

use super::Grid;
use crate::dump::Rgba;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// A stage of the new chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Landmass,
    /// A round of the round solver (0 is the first).
    Round(u32),
    H1,
    Thermal,
    Deposition,
    PitSpreading,
    K7,
}

impl Stage {
    /// The name in file names and in the numbers file.
    pub fn label(self) -> String {
        match self {
            Stage::Landmass => "landmass".into(),
            Stage::Round(k) => format!("round-{k:02}"),
            Stage::H1 => "h1".into(),
            Stage::Thermal => "thermal".into(),
            Stage::Deposition => "deposition".into(),
            Stage::PitSpreading => "pit-spreading".into(),
            Stage::K7 => "k7".into(),
        }
    }
}

/// What a field means, which fixes its colour scale: a sheet of the same kind has the same colours in every run, at every scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A height in metres above the sea: -2,000 to 4,000, blue under the sea, green to white over the land.
    Height,
    /// A drainage area in square metres on a log scale: 1e6 to 1e13, dark to yellow.
    Area,
    /// A change of height in metres: -100 to 100, blue to white to red.
    Delta,
    /// A flag (0 or 1) or a share: 0 to 1, black to white.
    Mask,
}

impl FieldKind {
    /// The scale in the units the colour reads: `(low, high, log10)`.
    fn scale(self) -> (f64, f64, bool) {
        match self {
            FieldKind::Height => (-2000.0, 4000.0, false),
            FieldKind::Area => (6.0, 13.0, true),
            FieldKind::Delta => (-100.0, 100.0, false),
            FieldKind::Mask => (0.0, 1.0, false),
        }
    }

    /// The value as the scale reads it (the logarithm of an area).
    fn read(self, v: f64) -> f64 {
        match self {
            FieldKind::Area => libm::log10(v.max(1.0)),
            _ => v,
        }
    }

    /// The colour of a value in the scale's units.
    pub fn colour(self, v: f64) -> [u8; 4] {
        let (lo, hi, _) = self.scale();
        let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
        let mix = |a: [f64; 3], b: [f64; 3], u: f64| -> [u8; 4] { [0, 1, 2].map(|i| (a[i] + (b[i] - a[i]) * u + 0.5) as u8).into_rgba() };
        match self {
            FieldKind::Height => {
                if v < 0.0 {
                    let u = (-v / 2000.0).clamp(0.0, 1.0);
                    mix([90.0, 150.0, 230.0], [10.0, 20.0, 90.0], u)
                } else {
                    let u = (v / 4000.0).clamp(0.0, 1.0);
                    if u < 0.5 {
                        mix([60.0, 150.0, 70.0], [150.0, 120.0, 70.0], u * 2.0)
                    } else {
                        mix([150.0, 120.0, 70.0], [250.0, 250.0, 250.0], (u - 0.5) * 2.0)
                    }
                }
            }
            FieldKind::Area => mix([40.0, 10.0, 80.0], [250.0, 230.0, 60.0], t),
            FieldKind::Delta => {
                if t < 0.5 {
                    mix([40.0, 90.0, 220.0], [250.0, 250.0, 250.0], t * 2.0)
                } else {
                    mix([250.0, 250.0, 250.0], [220.0, 50.0, 40.0], (t - 0.5) * 2.0)
                }
            }
            FieldKind::Mask => mix([0.0, 0.0, 0.0], [255.0, 255.0, 255.0], t),
        }
    }
}

trait IntoRgba {
    fn into_rgba(self) -> [u8; 4];
}

impl IntoRgba for [u8; 3] {
    fn into_rgba(self) -> [u8; 4] {
        [self[0], self[1], self[2], 255]
    }
}

/// The values of a field, borrowed.
#[derive(Clone, Copy)]
pub enum FieldData<'a> {
    F64(&'a [f64]),
    F32(&'a [f32]),
    Bool(&'a [bool]),
    /// `a - b` evaluated cell by cell: a change against a reference, with no buffer of its own.
    Diff(&'a [f64], &'a [f64]),
}

impl FieldData<'_> {
    pub fn len(&self) -> usize {
        match self {
            FieldData::F64(v) => v.len(),
            FieldData::F32(v) => v.len(),
            FieldData::Bool(v) => v.len(),
            FieldData::Diff(a, _) => a.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn at(&self, p: usize) -> f64 {
        match self {
            FieldData::F64(v) => v[p],
            FieldData::F32(v) => f64::from(v[p]),
            FieldData::Bool(v) => f64::from(u8::from(v[p])),
            FieldData::Diff(a, b) => a[p] - b[p],
        }
    }

    /// The bits of the value, for the data hash.
    fn bits(&self, p: usize) -> u64 {
        match self {
            FieldData::F64(v) => v[p].to_bits(),
            FieldData::F32(v) => u64::from(v[p].to_bits()),
            FieldData::Bool(v) => u64::from(v[p]),
            FieldData::Diff(a, b) => (a[p] - b[p]).to_bits(),
        }
    }
}

/// One field of a stage's output.
#[derive(Clone, Copy)]
pub struct Field<'a> {
    pub name: &'static str,
    pub kind: FieldKind,
    pub data: FieldData<'a>,
    /// Cells where the field means something (the land, say); `None` is every cell of the grid.
    pub mask: Option<&'a [bool]>,
}

/// What a stage shows: its output on the grid, borrowed for the call, and the key numbers it computed.
pub struct StageView<'a> {
    pub stage: Stage,
    pub grid: &'a Grid,
    pub fields: Vec<Field<'a>>,
    pub numbers: Vec<(&'static str, f64)>,
}

impl<'a> StageView<'a> {
    pub fn new(stage: Stage, grid: &'a Grid) -> StageView<'a> {
        StageView { stage, grid, fields: Vec::new(), numbers: Vec::new() }
    }

    pub fn f64(mut self, name: &'static str, kind: FieldKind, data: &'a [f64]) -> Self {
        self.fields.push(Field { name, kind, data: FieldData::F64(data), mask: None });
        self
    }

    pub fn number(mut self, name: &'static str, v: f64) -> Self {
        self.numbers.push((name, v));
        self
    }
}

/// Receives the stages' views. `Send + Sync`: the rounds run on threads.
pub trait StageProbe: Send + Sync {
    fn stage(&self, view: &StageView<'_>);
}

/// How many sinks are installed (the process-wide one and the thread-local ones): `emit` looks no further when it is 0.
static INSTALLED: AtomicUsize = AtomicUsize::new(0);
static SINK: RwLock<Option<Arc<dyn StageProbe>>> = RwLock::new(None);

thread_local! {
    static LOCAL: std::cell::RefCell<Option<Arc<dyn StageProbe>>> = const { std::cell::RefCell::new(None) };
}

/// Installs the process-wide sink: from now on every [`emit`] of every thread reaches it. Off by default.
pub fn install(probe: Arc<dyn StageProbe>) {
    let mut g = SINK.write().unwrap_or_else(|e| e.into_inner());
    if g.replace(probe).is_none() {
        INSTALLED.fetch_add(1, Ordering::SeqCst);
    }
}

/// Removes the process-wide sink.
pub fn uninstall() {
    let mut g = SINK.write().unwrap_or_else(|e| e.into_inner());
    if g.take().is_some() {
        INSTALLED.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Installs a sink for the calling thread only (what the solvers emit from their calling thread reaches it, what other threads emit does not): tests that run beside others.
pub fn install_local(probe: Arc<dyn StageProbe>) {
    LOCAL.with(|l| {
        if l.borrow_mut().replace(probe).is_none() {
            INSTALLED.fetch_add(1, Ordering::SeqCst);
        }
    });
}

/// Removes the calling thread's sink.
pub fn uninstall_local() {
    LOCAL.with(|l| {
        if l.borrow_mut().take().is_some() {
            INSTALLED.fetch_sub(1, Ordering::SeqCst);
        }
    });
}

/// Whether any sink is installed.
pub fn enabled() -> bool {
    INSTALLED.load(Ordering::Relaxed) > 0
}

/// A stage boundary: with a sink installed, `make` (given the stage) builds the view of what the stage produced (it only borrows) and the sink receives it; without one, one atomic load and nothing else.
#[inline]
pub fn emit<'a>(stage: Stage, make: impl FnOnce(Stage) -> StageView<'a>) {
    if INSTALLED.load(Ordering::Relaxed) == 0 {
        return;
    }
    let local = LOCAL.with(|l| l.borrow().clone());
    let sink = local.or_else(|| SINK.read().unwrap_or_else(|e| e.into_inner()).clone());
    if let Some(s) = sink {
        s.stage(&make(stage));
    }
}

/// A 64-bit word mixed so that every bit of it reaches every bit of the result (a splitmix64 finaliser): the data hash is not linear in any bit of the data.
fn mix_word(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The histogram bins of the percentiles.
pub const HIST_BINS: usize = 4096;
/// The bytes a bin takes: the sum (8) and the count (4).
pub const BIN_BYTES: usize = 12;

/// Writes sheets and the numbers file of a run into `dir`.
pub struct SheetProbe {
    dir: PathBuf,
    max_bins: usize,
    /// Whether the numbers file is still to be created (the first line truncates it, later lines append).
    started: Mutex<bool>,
}

/// The percentile of a histogram: the value at the first bin whose cumulative count reaches `q` of the total, at the bin's centre.
fn percentile(hist: &[u32], total: u64, q: f64, lo: f64, hi: f64) -> f64 {
    let want = (q * total as f64).ceil().max(1.0) as u64;
    let mut seen = 0u64;
    for (i, &c) in hist.iter().enumerate() {
        seen += u64::from(c);
        if seen >= want {
            return lo + (hi - lo) * (i as f64 + 0.5) / hist.len() as f64;
        }
    }
    hi
}

impl SheetProbe {
    /// A probe writing into `dir` (made if missing), at most `max_bins` bins a sheet.
    pub fn new(dir: impl Into<PathBuf>, max_bins: usize) -> SheetProbe {
        SheetProbe { dir: dir.into(), max_bins: max_bins.max(4), started: Mutex::new(true) }
    }

    /// The numbers file: one line a field.
    pub fn log_path(&self) -> PathBuf {
        self.dir.join("stages.txt")
    }

    /// The bins of a sheet for a layout of `w` by `h`: the same aspect, at most `max_bins` of them.
    fn bins(&self, w: usize, h: usize) -> (usize, usize) {
        let scale = ((self.max_bins as f64) / (w as f64 * h as f64)).sqrt().min(1.0);
        let (bw, bh) = (((w as f64 * scale).floor() as usize).max(1), ((h as f64 * scale).floor() as usize).max(1));
        // An extreme aspect forces a side up to 1 bin: the other gives way, so the product never passes the bound.
        let bw = bw.min((self.max_bins / bh).max(1));
        (bw, bh.min((self.max_bins / bw).max(1)))
    }

    fn one(&self, view: &StageView<'_>, f: &Field<'_>) -> Result<(), String> {
        let started = std::time::Instant::now();
        let grid = view.grid;
        let n = grid.len();
        if f.data.len() != n {
            return Err(format!("{}: {} values for {n} cells", f.name, f.data.len()));
        }
        if let FieldData::Diff(_, b) = f.data {
            if b.len() != n {
                return Err(format!("{}: a reference of {} values for {n} cells", f.name, b.len()));
            }
        }
        if f.mask.is_some_and(|m| m.len() != n) {
            return Err(format!("{}: a mask of the wrong length for {n} cells", f.name));
        }
        let (w, h) = grid.layout();
        let (bw, bh) = self.bins(w, h);
        let (lo, hi, _) = f.kind.scale();
        let (mut sum, mut count) = (vec![0f64; bw * bh], vec![0u32; bw * bh]);
        let mut hist = vec![0u32; HIST_BINS];
        let (mut total, mut min, mut max, mut hash) = (0u64, f64::INFINITY, f64::NEG_INFINITY, 0xCBF2_9CE4_8422_2325u64);
        let (mut below, mut above) = (0u64, 0u64);
        for p in 0..n {
            hash = (hash ^ mix_word(f.data.bits(p))).wrapping_mul(0x0000_0100_0000_01B3);
            if !grid.is_active(p) || f.mask.is_some_and(|m| !m[p]) {
                continue;
            }
            let raw = f.data.at(p);
            if !raw.is_finite() {
                continue;
            }
            let v = f.kind.read(raw);
            let (x, y) = grid.xy(p);
            let b = (y * bh / h) * bw + x * bw / w;
            sum[b] += v;
            count[b] += 1;
            hist[(((v - lo) / (hi - lo)).clamp(0.0, 1.0) * HIST_BINS as f64).floor().min(HIST_BINS as f64 - 1.0) as usize] += 1;
            below += u64::from(v < lo);
            above += u64::from(v > hi);
            total += 1;
            min = min.min(v);
            max = max.max(v);
        }
        let mut data = vec![0u8; bw * bh * 4];
        for b in 0..bw * bh {
            let c = if count[b] > 0 { f.kind.colour(sum[b] / f64::from(count[b])) } else { [30, 30, 30, 255] };
            data[b * 4..b * 4 + 4].copy_from_slice(&c);
        }
        let name = format!("{}-{}", view.stage.label(), f.name);
        Rgba { width: bw as u32, height: bh as u32, data }
            .write_png(&self.dir.join(format!("{name}.png")))
            .map_err(|e| format!("{name}.png: {e}"))?;
        // A percentile is a value of the data: the scale's edge bins hold what is outside the scale, so it is held within the minimum and the maximum.
        let q = |q: f64| percentile(&hist, total, q, lo, hi).clamp(min, max);
        let unit = if f.kind == FieldKind::Area { " (log10 m2)" } else { "" };
        let mut line = if total == 0 {
            format!("{} | {}{unit} | cells 0 | none | hash {hash:016x}", view.stage.label(), f.name)
        } else {
            format!(
                "{} | {}{unit} | cells {total} | min {min:.6e} | p5 {:.6e} | p50 {:.6e} | p95 {:.6e} | max {max:.6e} | below_scale {below} | above_scale {above} | hash {hash:016x}",
                view.stage.label(),
                f.name,
                q(0.05),
                q(0.5),
                q(0.95)
            )
        };
        for (k, v) in &view.numbers {
            let _ = write!(line, " | {k} {v:.6e}");
        }
        let _ = write!(
            line,
            " | probe_ms {:.1} | probe_bytes {}",
            started.elapsed().as_secs_f64() * 1000.0,
            bw * bh * BIN_BYTES + HIST_BINS * 4 + bw * bh * 4 // the buffers of the probe; the PNG encoder's own state is a constant on top
        );
        // Appended, not kept: the probe holds no text of the run. The file is created fresh by the first line of this probe.
        let mut first = self.started.lock().unwrap_or_else(|e| e.into_inner());
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(!*first)
            .truncate(*first)
            .open(self.log_path())
            .map_err(|e| e.to_string())?;
        *first = false;
        std::io::Write::write_all(&mut file, format!("{line}\n").as_bytes()).map_err(|e| e.to_string())
    }
}

impl StageProbe for SheetProbe {
    fn stage(&self, view: &StageView<'_>) {
        for f in &view.fields {
            if let Err(e) = self.one(view, f) {
                eprintln!("stage probe: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("planet-probe-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    struct Counting(std::sync::atomic::AtomicUsize);
    impl StageProbe for Counting {
        fn stage(&self, _: &StageView<'_>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    // spec: GEN-161
    #[test]
    fn off_the_closure_does_not_run_on_it_does_and_the_view_only_borrows() {
        assert!(!enabled());
        let g = Grid::new(4, 4, 100.0);
        let z = vec![1.0; 16];
        let ran = std::cell::Cell::new(0);
        emit(Stage::Round(0), |s| {
            ran.set(ran.get() + 1);
            StageView::new(s, &g).f64("z", FieldKind::Height, &z)
        });
        assert_eq!(ran.get(), 0, "no sink: the view is not built, nothing is read");
        let c = Arc::new(Counting(0.into()));
        install(c.clone());
        emit(Stage::Round(0), |s| {
            ran.set(ran.get() + 1);
            StageView::new(s, &g).f64("z", FieldKind::Height, &z)
        });
        uninstall();
        assert_eq!((ran.get(), c.0.load(Ordering::SeqCst)), (1, 1));
        assert!(!enabled());
    }

    // spec: BUILD-046
    #[test]
    fn a_sheet_has_at_most_the_bins_the_buffer_allows_whatever_the_grid_and_its_cost_is_reported() {
        let g = Grid::new(900, 700, 100.0);
        let z: Vec<f64> = (0..g.len()).map(|p| (p % 900) as f64 * 4.0 - 500.0).collect();
        let d = dir("bins");
        let probe = SheetProbe::new(&d, 10_000);
        probe.stage(&StageView::new(Stage::Landmass, &g).f64("z", FieldKind::Height, &z).number("land", 1.0));
        let png = std::fs::read(d.join("landmass-z.png")).unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(png));
        let reader = dec.read_info().unwrap();
        let info = reader.info();
        assert!(info.width as usize * info.height as usize <= 10_000, "{}x{}", info.width, info.height);
        assert!(info.width > 8 && info.height > 6, "the aspect is kept: {}x{}", info.width, info.height);
        let log = std::fs::read_to_string(probe.log_path()).unwrap();
        assert!(log.starts_with("landmass | z | cells 630000 | min "), "{log}");
        assert!(log.contains("| land 1.000000e0 |") && log.contains("probe_ms") && log.contains("probe_bytes"), "{log}");
        let bytes: usize = log.split("probe_bytes ").nth(1).unwrap().trim().parse().unwrap();
        assert!(bytes <= 10_000 * 20 + HIST_BINS * 4, "the buffer is bounded by the bins, not the grid: {bytes}");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-046
    #[test]
    fn the_colour_of_a_value_is_fixed_by_the_kind_and_not_by_the_data_of_the_sheet() {
        for kind in [FieldKind::Height, FieldKind::Area, FieldKind::Delta, FieldKind::Mask] {
            let (lo, hi, _) = kind.scale();
            let v = lo + 0.3 * (hi - lo);
            assert_eq!(kind.colour(v), kind.colour(v), "{kind:?}");
            assert_ne!(kind.colour(lo), kind.colour(hi), "{kind:?} spans two colours");
            assert_eq!(kind.colour(lo - 1000.0), kind.colour(lo), "{kind:?} clamps below");
        }
        // Two sheets of different data: the same value is the same pixel.
        let g = Grid::new(8, 8, 100.0);
        let (a, mut b) = (vec![100.0; 64], vec![100.0; 64]);
        b[0] = 3500.0;
        let d = dir("colour");
        let probe = SheetProbe::new(&d, 64);
        probe.stage(&StageView::new(Stage::Round(1), &g).f64("a", FieldKind::Height, &a).f64("b", FieldKind::Height, &b));
        let load = |n: &str| {
            let dec = png::Decoder::new(std::io::Cursor::new(std::fs::read(d.join(format!("round-01-{n}.png"))).unwrap()));
            let mut r = dec.read_info().unwrap();
            let mut buf = vec![0; r.output_buffer_size().unwrap()];
            r.next_frame(&mut buf).unwrap();
            buf
        };
        let (pa, pb) = (load("a"), load("b"));
        assert_eq!(pa[4 * 9..4 * 9 + 4], pb[4 * 9..4 * 9 + 4], "a cell of 100 m is the same colour on both sheets");
        assert_ne!(pa[0..4], pb[0..4], "and the cell of 3,500 m is another");
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: BUILD-046
    #[test]
    fn the_percentiles_come_from_the_histogram_within_a_bin_and_the_hash_follows_every_bit() {
        let g = Grid::new(100, 100, 100.0);
        let z: Vec<f64> = (0..g.len()).map(|p| p as f64 * 0.3 - 500.0).collect();
        let d = dir("pct");
        let probe = SheetProbe::new(&d, 400);
        probe.stage(&StageView::new(Stage::Thermal, &g).f64("z", FieldKind::Height, &z));
        let log = std::fs::read_to_string(probe.log_path()).unwrap();
        let get = |k: &str| -> f64 { log.split(&format!("| {k} ")).nth(1).unwrap().split(' ').next().unwrap().parse().unwrap() };
        let bin = 6000.0 / HIST_BINS as f64;
        for (k, q) in [("p5", 0.05), ("p50", 0.5), ("p95", 0.95)] {
            let mut s = z.clone();
            s.sort_by(f64::total_cmp);
            let exact = s[((q * s.len() as f64).ceil() as usize - 1).min(s.len() - 1)];
            assert!((get(k) - exact).abs() <= 0.6 * bin, "{k}: {} against {exact}", get(k));
        }
        let hash_of = |v: &[f64]| {
            let p2 = SheetProbe::new(dir("pct2"), 400);
            p2.stage(&StageView::new(Stage::Thermal, &g).f64("z", FieldKind::Height, v));
            let l = std::fs::read_to_string(p2.log_path()).unwrap();
            l.split("| hash ").nth(1).unwrap().split(' ').next().unwrap().to_string()
        };
        let mut z2 = z.clone();
        z2[5000] = f64::from_bits(z2[5000].to_bits() ^ 1);
        assert_ne!(hash_of(&z), hash_of(&z2), "one bit of one cell changes the data hash");
        // The sign bit is no longer linear: two sign flips do not cancel, and doubling two values is another hash.
        let (a, b) = (vec![5.0; 10_000], vec![-5.0; 10_000]);
        assert_ne!(hash_of(&a), hash_of(&b));
        let (c, d2) = ([vec![5.0, 7.0, 1.0], vec![0.0; 9_997]].concat(), [vec![-5.0, -7.0, 1.0], vec![0.0; 9_997]].concat());
        assert_ne!(hash_of(&c), hash_of(&d2), "two sign flips do not cancel");
        assert_eq!(hash_of(&z), hash_of(&z), "and the hash is a function of the data");
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(dir("pct2"));
    }

    // spec: BUILD-046
    #[test]
    fn the_numbers_stay_inside_the_data_the_bound_holds_for_a_long_thin_grid_and_a_bad_length_or_a_nan_is_an_error_or_skipped() {
        let d = dir("edge");
        let probe = SheetProbe::new(&d, 2_000);
        let g = Grid::new(20_000, 3, 100.0);
        probe.stage(&StageView::new(Stage::H1, &g).f64("z", FieldKind::Height, &vec![-5000.0; g.len()]));
        let dec = png::Decoder::new(std::io::Cursor::new(std::fs::read(d.join("h1-z.png")).unwrap()));
        let r = dec.read_info().unwrap();
        assert!(r.info().width as usize * r.info().height as usize <= 2_000, "{}x{}", r.info().width, r.info().height);
        let log = std::fs::read_to_string(probe.log_path()).unwrap();
        assert!(
            log.contains("min -5.000000e3 | p5 -5.000000e3 | p50 -5.000000e3 | p95 -5.000000e3 | max -5.000000e3"),
            "a percentile is a value of the data: {log}"
        );
        assert!(log.contains("below_scale 60000"), "what is under the scale is counted: {log}");
        // Nothing counted: no numbers instead of an infinity.
        let g2 = Grid::new(4, 4, 100.0);
        let none = vec![f64::NAN; 16];
        probe.stage(&StageView::new(Stage::H1, &g2).f64("n", FieldKind::Area, &none));
        let log = std::fs::read_to_string(probe.log_path()).unwrap();
        assert!(log.contains("h1 | n (log10 m2) | cells 0 | none"), "a NaN area is skipped, and an empty field says none: {log}");
        // A reference or a mask of another length is an error and writes nothing.
        let short = vec![0.0; 3];
        assert!(probe
            .one(
                &StageView::new(Stage::H1, &g2),
                &Field { name: "d", kind: FieldKind::Delta, data: FieldData::Diff(&none, &short), mask: None }
            )
            .is_err());
        let mask = vec![true; 3];
        assert!(probe
            .one(
                &StageView::new(Stage::H1, &g2),
                &Field { name: "m", kind: FieldKind::Height, data: FieldData::F64(&none), mask: Some(&mask) }
            )
            .is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    // spec: GEN-161
    #[test]
    fn a_local_sink_reaches_the_threads_own_emits_only() {
        let c = Arc::new(Counting(0.into()));
        let g = Grid::new(4, 4, 100.0);
        install_local(c.clone());
        assert!(enabled());
        emit(Stage::K7, |s| StageView::new(s, &g));
        let other = std::thread::spawn({
            let g = Grid::new(4, 4, 100.0);
            move || emit(Stage::K7, |s| StageView::new(s, &g))
        });
        other.join().unwrap();
        uninstall_local();
        assert_eq!((c.0.load(Ordering::SeqCst), enabled()), (1, false), "only the installing thread's emit arrived");
    }
}
