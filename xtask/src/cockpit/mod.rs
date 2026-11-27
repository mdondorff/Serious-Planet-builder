//! `cargo xtask cockpit` (`BUILD-064`): offline pipeline DAG viewer, switch simulator, Global Authoring preview batch runner,
//! and version/probe ledger built purely on `std` (`CON-02`, `CON-06`, `CON-07`).

mod html;

use crate::util::{run, Res};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Reference world seed shared with `xtask/src/preview.rs`.
pub const DEFAULT_SEED: u64 = 14;
/// Default output folder relative to repository root.
pub const DEFAULT_OUT_DIR: &str = "target/review/cockpit";

const MAX_TXT_BYTES: usize = 64 * 1024;

pub const USAGE: &str = "usage: cargo xtask cockpit [-h|--help] [--seed N | --world FILE] [--erosion on|off [--channels on|off] | --commit DIR] [--pipeline-json FILE] [--run <world|drivers|stages|all>] [--compare-seed N] [--chain DIR [--face F]] [--km N] [--threads N] [--out-dir DIR]";

/// One CLI option in the living flag registry (`BUILD-064`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlagSpec {
    pub flags: &'static str,
    pub value: Option<&'static str>,
    pub summary: &'static str,
}

/// Authoritative option reference shared by `--help`, Tier-A sync tests, and the embedded HTML manual.
pub const FLAG_SPECS: &[FlagSpec] = &[
    FlagSpec { flags: "-h, --help", value: None, summary: "Print this living CLI reference and exit without taking the heavy lock." },
    FlagSpec { flags: "--seed", value: Some("N"), summary: "Resolve pipeline JSON and previews for seed N." },
    FlagSpec {
        flags: "--world",
        value: Some("FILE"),
        summary: "Resolve pipeline JSON and driver previews from a WorldDefinition TOML file.",
    },
    FlagSpec { flags: "--erosion", value: Some("on|off"), summary: "Enable or disable M9 erosion layers when building pipeline JSON." },
    FlagSpec {
        flags: "--channels",
        value: Some("on|off"),
        summary: "Enable driver channels alongside --erosion on (refused without --erosion on).",
    },
    FlagSpec {
        flags: "--commit",
        value: Some("DIR"),
        summary: "Resolve a K5-K7 commit directory (cpu-erosion-c1-v2); excludes --erosion/--channels.",
    },
    FlagSpec {
        flags: "--pipeline-json",
        value: Some("FILE"),
        summary: "Read an existing schema-v2 JSON instead of running `planet`; skips heavy lock when --run is unset.",
    },
    FlagSpec {
        flags: "--run",
        value: Some("<world|drivers|stages|all>"),
        summary: "Comma-separated or repeated preview/sheet generators to run before assembling HTML.",
    },
    FlagSpec {
        flags: "--compare-seed",
        value: Some("N"),
        summary: "Also run and index seed N alongside the primary target for comparison.",
    },
    FlagSpec {
        flags: "--chain",
        value: Some("DIR"),
        summary: "Index a river-graph chain directory (and render its stage/transect sheets with --run stages).",
    },
    FlagSpec { flags: "--face", value: Some("0..=5"), summary: "Cube face override passed to `transect_sheet` (requires --chain DIR)." },
    FlagSpec { flags: "--km", value: Some("N"), summary: "Kilometres per pixel forwarded to Global Authoring driver previews." },
    FlagSpec { flags: "--threads", value: Some("N"), summary: "Worker thread count forwarded to driver previews." },
    FlagSpec { flags: "--out-dir", value: Some("DIR"), summary: "Output folder for pipeline.json, previews/, and index.html." },
];

/// Targets accepted by `--run` (repeatable or comma-separated).
pub const RUN_TARGET_SPECS: &[(&str, &str)] = &[
    ("world", "Run `preview_world` (whole-planet continents, plates, relief classes, k, report.txt)."),
    ("drivers", "Run all 6 Global Authoring driver previews (`shelf`, `lithology`, `uplift`, `climate`, `arid`, `ice`)."),
    ("stages", "Run `stage_manifest` sheets and, with `--chain DIR`, `stage_sheet` + `transect_sheet`."),
    ("all", "Shortcut expanding to `world,drivers,stages`."),
];

/// Self-tested example workflows printed by `--help` and embedded inside the offline HTML help modal.
pub const EXAMPLE_INVOCATIONS: &[(&str, &[&str])] = &[
    ("Default seed 14 cockpit (re-indexes existing previews)", &[]),
    ("World TOML with M9 erosion and driver channels", &["--world", "worlds/earthlike.toml", "--erosion", "on", "--channels", "on"]),
    ("Run 6 driver previews side-by-side (seed 14 vs seed 7)", &["--seed", "14", "--compare-seed", "7", "--run", "drivers"]),
    ("Include river-graph chain sheets and face-4 transects", &["--seed", "15", "--chain", "chains/r1h", "--face", "4", "--run", "stages"]),
    ("Instant <100 ms offline HTML refresh from existing JSON", &["--pipeline-json", "target/review/cockpit/pipeline.json"]),
];

/// Formats the living CLI guide straight from code constants and option specs (`BUILD-064`).
pub fn help_text() -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "cargo xtask cockpit — Offline Pipeline DAG & Global Authoring Cockpit (BUILD-064)\n");
    let _ = writeln!(out, "{USAGE}\n");
    let _ = writeln!(out, "Defaults:");
    let _ = writeln!(out, "  seed:    {DEFAULT_SEED}");
    let _ = writeln!(out, "  out-dir: {DEFAULT_OUT_DIR}  (writes pipeline.json and index.html)\n");
    let _ = writeln!(out, "Options:");
    for spec in FLAG_SPECS {
        let lhs = match spec.value {
            Some(v) => format!("{} {v}", spec.flags),
            None => spec.flags.to_string(),
        };
        let _ = writeln!(out, "  {lhs:<38} {}", spec.summary);
    }
    let _ = writeln!(out, "\n--run targets:");
    for (name, desc) in RUN_TARGET_SPECS {
        let _ = writeln!(out, "  {name:<12} {desc}");
    }
    let _ = writeln!(out, "\nExamples (self-tested against parse_args):");
    for (desc, args) in EXAMPLE_INVOCATIONS {
        let suffix = if args.is_empty() { String::new() } else { format!(" {}", args.join(" ")) };
        let _ = writeln!(out, "  # {desc}");
        let _ = writeln!(out, "  cargo xtask cockpit{suffix}");
    }
    out
}

/// Preview groups executed by `--run`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunTargets {
    pub world: bool,
    pub drivers: bool,
    pub stages: bool,
}

impl RunTargets {
    pub fn any(self) -> bool {
        self.world || self.drivers || self.stages
    }
}

/// Parsed CLI options for `cargo xtask cockpit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opts {
    pub help: bool,
    pub seed: Option<u64>,
    pub world: Option<String>,
    pub erosion: Option<bool>,
    pub channels: Option<bool>,
    pub commit: Option<String>,
    pub pipeline_json: Option<String>,
    pub run: RunTargets,
    pub compare_seed: Option<u64>,
    pub chain: Option<String>,
    pub face: Option<u8>,
    pub km: Option<u32>,
    pub threads: Option<u32>,
    pub out_dir: String,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            help: false,
            seed: None,
            world: None,
            erosion: None,
            channels: None,
            commit: None,
            pipeline_json: None,
            run: RunTargets::default(),
            compare_seed: None,
            chain: None,
            face: None,
            km: None,
            threads: None,
            out_dir: DEFAULT_OUT_DIR.to_string(),
        }
    }
}

/// One planned child `cargo` invocation for `--run`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedCmd {
    pub label: String,
    pub cargo_args: Vec<String>,
}

/// Summary counts produced by [`assemble_cockpit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembleSummary {
    pub index_html: PathBuf,
    pub stage_count: usize,
    pub pin_count: usize,
    pub image_count: usize,
    pub text_count: usize,
    pub manifest_count: usize,
}

fn next_val<'a>(flag: &str, it: &mut std::slice::Iter<'a, String>) -> Result<&'a str, String> {
    it.next().map(String::as_str).filter(|v| !v.starts_with("--")).ok_or_else(|| format!("{flag} needs a value ({USAGE})"))
}

fn take_unique(slot: &mut Option<String>, flag: &str, it: &mut std::slice::Iter<'_, String>) -> Res {
    if slot.is_some() {
        return Err(format!("{flag} given twice ({USAGE})"));
    }
    *slot = Some(next_val(flag, it)?.to_string());
    Ok(())
}

fn parse_on_off(flag: &str, val: &str) -> Result<bool, String> {
    match val {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("{flag} expects 'on' or 'off', got '{other}'")),
    }
}

fn parse_u64_opt(flag: &str, val: &str) -> Result<u64, String> {
    val.parse::<u64>().map_err(|_| format!("{flag} needs a non-negative integer, got '{val}'"))
}

fn parse_nonzero_u32(flag: &str, val: &str) -> Result<u32, String> {
    let n = val.parse::<u32>().map_err(|_| format!("{flag} needs a positive integer, got '{val}'"))?;
    if n == 0 {
        return Err(format!("{flag} must be >= 1"));
    }
    Ok(n)
}

fn parse_run_targets(val: &str, run: &mut RunTargets) -> Res {
    let parts: Vec<&str> = val.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err("--run needs one or more targets: world, drivers, stages, all".into());
    }
    for part in parts {
        match part {
            "world" => run.world = true,
            "drivers" => run.drivers = true,
            "stages" => run.stages = true,
            "all" => {
                run.world = true;
                run.drivers = true;
                run.stages = true;
            }
            other => return Err(format!("unknown --run target '{other}' (expected world, drivers, stages, all)")),
        }
    }
    Ok(())
}

/// Parses and validates CLI flags for `cargo xtask cockpit`.
pub fn parse_args(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut seed_raw = None;
    let mut erosion_raw = None;
    let mut channels_raw = None;
    let mut compare_raw = None;
    let mut face_raw = None;
    let mut km_raw = None;
    let mut threads_raw = None;
    let mut out_dir_raw = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => o.help = true,
            "--seed" => take_unique(&mut seed_raw, "--seed", &mut it)?,
            "--world" => take_unique(&mut o.world, "--world", &mut it)?,
            "--erosion" => take_unique(&mut erosion_raw, "--erosion", &mut it)?,
            "--channels" => take_unique(&mut channels_raw, "--channels", &mut it)?,
            "--commit" => take_unique(&mut o.commit, "--commit", &mut it)?,
            "--pipeline-json" => take_unique(&mut o.pipeline_json, "--pipeline-json", &mut it)?,
            "--run" => parse_run_targets(next_val("--run", &mut it)?, &mut o.run)?,
            "--compare-seed" => take_unique(&mut compare_raw, "--compare-seed", &mut it)?,
            "--chain" => take_unique(&mut o.chain, "--chain", &mut it)?,
            "--face" => take_unique(&mut face_raw, "--face", &mut it)?,
            "--km" => take_unique(&mut km_raw, "--km", &mut it)?,
            "--threads" => take_unique(&mut threads_raw, "--threads", &mut it)?,
            "--out-dir" => take_unique(&mut out_dir_raw, "--out-dir", &mut it)?,
            other => return Err(format!("unknown option '{other}' ({USAGE})")),
        }
    }
    if o.help {
        return Ok(o);
    }

    if let Some(v) = seed_raw {
        o.seed = Some(parse_u64_opt("--seed", &v)?);
    }
    if let Some(v) = erosion_raw {
        o.erosion = Some(parse_on_off("--erosion", &v)?);
    }
    if let Some(v) = channels_raw {
        o.channels = Some(parse_on_off("--channels", &v)?);
    }
    if let Some(v) = compare_raw {
        o.compare_seed = Some(parse_u64_opt("--compare-seed", &v)?);
    }
    if let Some(v) = face_raw {
        let f = v.parse::<u8>().map_err(|_| format!("--face must be 0..=5, got '{v}'"))?;
        if f > 5 {
            return Err(format!("--face must be 0..=5, got '{v}'"));
        }
        o.face = Some(f);
    }
    if let Some(v) = km_raw {
        o.km = Some(parse_nonzero_u32("--km", &v)?);
    }
    if let Some(v) = threads_raw {
        o.threads = Some(parse_nonzero_u32("--threads", &v)?);
    }
    if let Some(v) = out_dir_raw {
        o.out_dir = v;
    }

    if o.seed.is_some() && o.world.is_some() {
        return Err("--world and --seed both name the definition: give one".into());
    }
    if o.channels == Some(true) && o.erosion != Some(true) {
        return Err("--channels on requires --erosion on".into());
    }
    if o.commit.is_some() && (o.erosion.is_some() || o.channels.is_some()) {
        return Err("--commit already carries the baked relief; drop --erosion and --channels".into());
    }
    if o.pipeline_json.is_some() && (o.world.is_some() || o.erosion.is_some() || o.channels.is_some() || o.commit.is_some()) {
        return Err("--pipeline-json reads an existing JSON file; drop --world, --erosion, --channels, and --commit".into());
    }
    if o.face.is_some() && o.chain.is_none() {
        return Err("--face applies to --chain DIR".into());
    }

    Ok(o)
}

/// True when `cockpit` must invoke child `cargo` commands (`planet generate --pipeline-json` or `--run`), false for `--help`, pure offline HTML assembly, or usage errors.
pub fn needs_heavy_slot(args: &[String]) -> bool {
    let Ok(o) = parse_args(args) else {
        return false;
    };
    !o.help && !(o.pipeline_json.is_some() && !o.run.any())
}

fn posix_str(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn clean_tag(raw: &str) -> String {
    let s: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    if s.is_empty() {
        "world".to_string()
    } else {
        s
    }
}

/// Builds the `cargo run` arguments for `planet generate --pipeline-json OUT`.
pub fn build_pipeline_json_cmd(o: &Opts, out_json: &Path) -> Vec<String> {
    let mut cmd = vec![
        "run".into(),
        "-q".into(),
        "-p".into(),
        "planet".into(),
        "--".into(),
        "generate".into(),
        "--pipeline-json".into(),
        posix_str(out_json),
    ];
    if let Some(w) = &o.world {
        cmd.push("--world".into());
        cmd.push(w.clone());
    } else {
        cmd.push("--seed".into());
        cmd.push(o.seed.unwrap_or(DEFAULT_SEED).to_string());
    }
    if let Some(commit) = &o.commit {
        cmd.push("--commit".into());
        cmd.push(commit.clone());
    } else {
        if let Some(er) = o.erosion {
            cmd.push("--erosion".into());
            cmd.push(if er { "on" } else { "off" }.into());
        }
        if let Some(ch) = o.channels {
            cmd.push("--channels".into());
            cmd.push(if ch { "on" } else { "off" }.into());
        }
    }
    cmd
}

/// Plans child `cargo` commands for `--run world|drivers|stages|all`.
pub fn build_run_plan(o: &Opts, primary_seed: u64, out_dir: &Path) -> Vec<PlannedCmd> {
    let mut plan = Vec::new();
    let previews_root = out_dir.join("previews");
    let world_tag = match &o.world {
        Some(w) => format!("world-{}", clean_tag(Path::new(w).file_stem().and_then(|s| s.to_str()).unwrap_or("custom"))),
        None => format!("seed-{primary_seed}"),
    };

    if o.run.world {
        let out_world = previews_root.join(format!("seed-{primary_seed}")).join("world");
        plan.push(PlannedCmd {
            label: format!("preview world (seed {primary_seed})"),
            cargo_args: preview_world_args(primary_seed, &out_world),
        });
        if let Some(cs) = o.compare_seed {
            if cs != primary_seed {
                let cmp_world = previews_root.join(format!("seed-{cs}")).join("world");
                plan.push(PlannedCmd {
                    label: format!("preview world (compare seed {cs})"),
                    cargo_args: preview_world_args(cs, &cmp_world),
                });
            }
        }
    }

    if o.run.drivers {
        const DRIVERS: [(&str, &str); 6] = [
            ("--shelf-preview", "shelf"),
            ("--lithology-preview", "lithology"),
            ("--uplift-preview", "uplift"),
            ("--climate-preview", "climate"),
            ("--arid-preview", "arid"),
            ("--ice-preview", "ice"),
        ];
        for (flag, slug) in DRIVERS {
            let dir = previews_root.join(&world_tag).join(slug);
            plan.push(PlannedCmd {
                label: format!("{slug} preview ({world_tag})"),
                cargo_args: driver_preview_args(flag, &dir, o.world.as_deref(), primary_seed, o.km, o.threads),
            });
            if let Some(cs) = o.compare_seed {
                if o.world.is_some() || cs != primary_seed {
                    let cmp_dir = previews_root.join(format!("seed-{cs}")).join(slug);
                    plan.push(PlannedCmd {
                        label: format!("{slug} preview (compare seed {cs})"),
                        cargo_args: driver_preview_args(flag, &cmp_dir, None, cs, o.km, o.threads),
                    });
                }
            }
        }
    }

    if o.run.stages {
        plan.push(PlannedCmd {
            label: "stage manifest sheets".into(),
            cargo_args: ["nextest", "run", "-p", "planet-generators", "--no-fail-fast", "stage_manifest"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        });
        if let Some(chain) = &o.chain {
            let chain_dir = previews_root.join("chain");
            plan.push(PlannedCmd {
                label: "chain stage sheet".into(),
                cargo_args: [
                    "run",
                    "--release",
                    "-q",
                    "-p",
                    "planet-cache",
                    "--example",
                    "stage_sheet",
                    "--",
                    chain.as_str(),
                    "--out",
                    &posix_str(&chain_dir.join("stage-sheet.png")),
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            });
            let mut tr = vec![
                "run".into(),
                "--release".into(),
                "-q".into(),
                "-p".into(),
                "planet-cache".into(),
                "--example".into(),
                "transect_sheet".into(),
                "--".into(),
                chain.clone(),
            ];
            if let Some(f) = o.face {
                tr.push("--face".into());
                tr.push(f.to_string());
            }
            tr.push("--out".into());
            tr.push(posix_str(&chain_dir.join("transects.png")));
            plan.push(PlannedCmd { label: "chain transect sheet".into(), cargo_args: tr });
        }
    }

    plan
}

fn preview_world_args(seed: u64, out_dir: &Path) -> Vec<String> {
    vec![
        "run".into(),
        "--release".into(),
        "-q".into(),
        "-p".into(),
        "planet-generators".into(),
        "--example".into(),
        "preview_world".into(),
        "--".into(),
        "--seed".into(),
        seed.to_string(),
        "--out".into(),
        posix_str(out_dir),
    ]
}

fn driver_preview_args(flag: &str, out_dir: &Path, world: Option<&str>, seed: u64, km: Option<u32>, threads: Option<u32>) -> Vec<String> {
    let mut cmd = vec![
        "run".into(),
        "--release".into(),
        "-q".into(),
        "-p".into(),
        "planet".into(),
        "--".into(),
        "generate".into(),
        flag.into(),
        posix_str(out_dir),
    ];
    if let Some(w) = world {
        cmd.push("--world".into());
        cmd.push(w.to_string());
    } else {
        cmd.push("--seed".into());
        cmd.push(seed.to_string());
    }
    if let Some(k) = km {
        cmd.push("--km".into());
        cmd.push(k.to_string());
    }
    if let Some(t) = threads {
        cmd.push("--threads".into());
        cmd.push(t.to_string());
    }
    cmd
}

/// Pure-`std` JSON value retaining exact object field ordering and raw numeric text.
#[derive(Clone, Debug, PartialEq)]
pub enum JsonVal {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(Vec<(String, JsonVal)>),
}

impl JsonVal {
    pub fn get(&self, key: &str) -> Option<&JsonVal> {
        let JsonVal::Obj(entries) = self else { return None };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonVal::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            JsonVal::Num(raw) => raw.parse::<u64>().ok(),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[JsonVal]> {
        match self {
            JsonVal::Arr(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    /// Serializes valid JSON while escaping `</` as `<\/` so inline `<script>` embedding cannot break HTML.
    pub fn to_safe_script_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out, true);
        out
    }

    fn write_json(&self, out: &mut String, escape_closing_tag: bool) {
        match self {
            JsonVal::Null => out.push_str("null"),
            JsonVal::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            JsonVal::Num(raw) => out.push_str(raw),
            JsonVal::Str(s) => write_json_str(out, s, escape_closing_tag),
            JsonVal::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_json(out, escape_closing_tag);
                }
                out.push(']');
            }
            JsonVal::Obj(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_str(out, k, escape_closing_tag);
                    out.push(':');
                    v.write_json(out, escape_closing_tag);
                }
                out.push('}');
            }
        }
    }
}

fn write_json_str(out: &mut String, s: &str, escape_closing_tag: bool) {
    out.push('"');
    let mut prev_lt = false;
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' if escape_closing_tag && prev_lt => out.push_str("\\/"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
        prev_lt = ch == '<';
    }
    out.push('"');
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { bytes: input.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    fn parse_root(&mut self) -> Result<JsonVal, String> {
        let v = self.parse_val()?;
        self.skip_ws();
        if self.pos != self.bytes.len() {
            return Err(format!("trailing JSON bytes at offset {}", self.pos));
        }
        Ok(v)
    }

    fn parse_val(&mut self) -> Result<JsonVal, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'n') => self.expect_lit(b"null").map(|_| JsonVal::Null),
            Some(b't') => self.expect_lit(b"true").map(|_| JsonVal::Bool(true)),
            Some(b'f') => self.expect_lit(b"false").map(|_| JsonVal::Bool(false)),
            Some(b'"') => self.parse_string().map(JsonVal::Str),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(other) => Err(format!("unexpected JSON token '{}' at byte {}", other as char, self.pos)),
            None => Err("unexpected end of JSON input".into()),
        }
    }

    fn expect_lit(&mut self, lit: &[u8]) -> Res {
        if self.bytes.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            Err(format!("invalid JSON literal at byte {}", self.pos))
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected '\"' at byte {}", self.pos));
        }
        self.pos += 1;
        let mut out = String::new();
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'"' {
                return Ok(out);
            }
            if b == b'\\' {
                let esc = self.peek().ok_or_else(|| "unterminated string escape".to_string())?;
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\x08'),
                    b'f' => out.push('\x0c'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let cp = self.parse_hex4()?;
                        if (0xD800..=0xDBFF).contains(&cp) && self.bytes.get(self.pos..self.pos + 2) == Some(b"\\u") {
                            self.pos += 2;
                            let lo = self.parse_hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                return Err("invalid UTF-16 surrogate pair in JSON".into());
                            }
                            let combined = 0x10000 + (((cp - 0xD800) << 10) | (lo - 0xDC00));
                            out.push(char::from_u32(combined).ok_or_else(|| "invalid Unicode scalar".to_string())?);
                        } else {
                            out.push(char::from_u32(cp).ok_or_else(|| "invalid Unicode scalar".to_string())?);
                        }
                    }
                    other => return Err(format!("unsupported JSON escape '\\{}'", other as char)),
                }
            } else if b < 0x80 {
                if b < 0x20 {
                    return Err(format!("unescaped control byte 0x{b:02x} in JSON string"));
                }
                out.push(b as char);
            } else {
                let start = self.pos - 1;
                let s = std::str::from_utf8(&self.bytes[start..]).unwrap_or_else(|e| {
                    let valid = e.valid_up_to();
                    std::str::from_utf8(&self.bytes[start..start + valid]).unwrap_or("")
                });
                let ch = s.chars().next().ok_or_else(|| "invalid UTF-8 in JSON string".to_string())?;
                out.push(ch);
                self.pos = start + ch.len_utf8();
            }
        }
        Err("unterminated JSON string".into())
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        let hex = self.bytes.get(self.pos..self.pos + 4).ok_or_else(|| "truncated \\u escape".to_string())?;
        let s = std::str::from_utf8(hex).map_err(|_| "invalid UTF-8 in \\u escape".to_string())?;
        let val = u32::from_str_radix(s, 16).map_err(|_| format!("invalid \\u hex '{s}'"))?;
        self.pos += 4;
        Ok(val)
    }

    fn parse_number(&mut self) -> Result<JsonVal, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(format!("invalid JSON number at byte {start}")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            let f_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == f_start {
                return Err(format!("missing fractional digits in JSON number at byte {start}"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let e_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == e_start {
                return Err(format!("missing exponent digits in JSON number at byte {start}"));
            }
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| "invalid UTF-8 number".to_string())?;
        raw.parse::<f64>().map_err(|e| format!("invalid JSON float '{raw}': {e}"))?;
        Ok(JsonVal::Num(raw.to_string()))
    }

    fn parse_array(&mut self) -> Result<JsonVal, String> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonVal::Arr(items));
        }
        loop {
            items.push(self.parse_val()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonVal::Arr(items));
                }
                _ => return Err(format!("expected ',' or ']' in array at byte {}", self.pos)),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonVal, String> {
        self.pos += 1;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonVal::Obj(entries));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("expected ':' after key '{key}' at byte {}", self.pos));
            }
            self.pos += 1;
            let val = self.parse_val()?;
            entries.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonVal::Obj(entries));
                }
                _ => {
                    let last_key = &entries.last().expect("entry pushed").0;
                    return Err(format!("expected ',' or '}}' after object field '{last_key}' at byte {}", self.pos));
                }
            }
        }
    }
}

/// Parses raw JSON text.
pub fn parse_json(raw: &str) -> Result<JsonVal, String> {
    JsonParser::new(raw).parse_root()
}

/// Validates schema v2 (`BUILD-063`) across either a `resolved_world_json` object or a standalone `to_canonical_json` object.
pub fn validate_and_normalize_pipeline(raw: &str) -> Result<JsonVal, String> {
    let val = parse_json(raw)?;
    let ver = val.get("schema_format_version").and_then(JsonVal::as_u64);
    if ver != Some(2) {
        return Err(format!("expected pipeline JSON schema_format_version == 2, got {ver:?}"));
    }
    let normalized = if val.get("catalog").is_some() {
        val
    } else if val.get("stages").is_some() {
        JsonVal::Obj(vec![
            ("schema_format_version".into(), JsonVal::Num("2".into())),
            ("world".into(), JsonVal::Null),
            ("active_stages".into(), JsonVal::Null),
            ("walked_chain".into(), JsonVal::Null),
            ("catalog".into(), val),
        ])
    } else {
        return Err("pipeline JSON must contain either 'catalog' or top-level 'stages'".into());
    };

    let cat = normalized.get("catalog").ok_or_else(|| "missing 'catalog' object".to_string())?;
    let stages = cat.get("stages").and_then(JsonVal::as_arr).ok_or_else(|| "catalog.stages must be an array".to_string())?;
    if stages.is_empty() {
        return Err("catalog.stages must not be empty".into());
    }
    for (idx, st) in stages.iter().enumerate() {
        for req in ["id", "title", "k_label", "scope"] {
            if st.get(req).and_then(JsonVal::as_str).is_none_or(str::is_empty) {
                return Err(format!("catalog.stages[{idx}] missing non-empty string field '{req}'"));
            }
        }
    }
    if cat.get("version_pins").and_then(JsonVal::as_arr).is_none_or(|a| a.is_empty()) {
        return Err("catalog.version_pins must be a non-empty array".into());
    }
    if cat.get("switch_matrix").and_then(JsonVal::as_arr).is_none_or(|a| a.is_empty()) {
        return Err("catalog.switch_matrix must be a non-empty array".into());
    }
    Ok(normalized)
}

/// One parsed line of `docs/tools/stages/manifest.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestRow {
    pub stage: String,
    pub field: String,
    pub kind_id: String,
    pub stage_id: Option<String>,
    pub cells: String,
    pub min: String,
    pub p5: String,
    pub p50: String,
    pub p95: String,
    pub max: String,
    pub hash: String,
    pub extras: String,
}

fn normalize_manifest_kind_id(stage: &str) -> String {
    if stage.starts_with("round-") {
        "round".into()
    } else if stage.starts_with("k7-") {
        "k7".into()
    } else {
        stage.to_string()
    }
}

/// Parses `docs/tools/stages/manifest.txt` into structured probe rows and attaches catalog `stage_id` links.
pub fn parse_stage_manifest(text: &str, catalog: &JsonVal) -> Vec<ManifestRow> {
    let mut hook_map: BTreeMap<(String, String), String> = BTreeMap::new();
    if let Some(stages) = catalog.get("stages").and_then(JsonVal::as_arr) {
        for st in stages {
            let Some(id) = st.get("id").and_then(JsonVal::as_str) else { continue };
            let Some(hooks) = st.get("probe_hooks").and_then(JsonVal::as_arr) else { continue };
            for hk in hooks {
                let Some(kind) = hk.get("kind_id").and_then(JsonVal::as_str) else { continue };
                let Some(fields) = hk.get("fields").and_then(JsonVal::as_arr) else { continue };
                for f in fields.iter().filter_map(JsonVal::as_str) {
                    hook_map.insert((kind.to_string(), f.to_string()), id.to_string());
                }
            }
        }
    }

    let mut rows = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        if cols.len() < 3 {
            continue;
        }
        let stage = cols[0].to_string();
        let field = cols[1].to_string();
        let kind_id = normalize_manifest_kind_id(&stage);
        let stage_id = hook_map.get(&(kind_id.clone(), field.clone())).cloned();

        let (mut cells, mut min, mut p5, mut p50, mut p95, mut max, mut hash) =
            (String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new());
        let mut extras = Vec::new();

        for col in &cols[2..] {
            if let Some((k, v)) = col.split_once(' ') {
                match k {
                    "cells" if cells.is_empty() => cells = v.trim().to_string(),
                    "min" => min = v.trim().to_string(),
                    "p5" => p5 = v.trim().to_string(),
                    "p50" => p50 = v.trim().to_string(),
                    "p95" => p95 = v.trim().to_string(),
                    "max" => max = v.trim().to_string(),
                    "hash" => hash = v.trim().to_string(),
                    _ => extras.push((*col).to_string()),
                }
            } else {
                extras.push((*col).to_string());
            }
        }

        rows.push(ManifestRow { stage, field, kind_id, stage_id, cells, min, p5, p50, p95, max, hash, extras: extras.join(" | ") });
    }
    rows
}

/// Reads width and height strictly from the 24-byte PNG `IHDR` header without decoding pixel data.
pub fn png_dimensions(path: &Path) -> Option<(u32, u32)> {
    let mut f = fs::File::open(path).ok()?;
    let mut buf = [0u8; 24];
    f.read_exact(&mut buf).ok()?;
    if &buf[0..8] != b"\x89PNG\r\n\x1a\n" || &buf[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let h = u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]);
    Some((w, h))
}

fn lexically_absolutize(base: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_absolute() { target.to_path_buf() } else { base.join(target) };
    let mut norm = PathBuf::new();
    for comp in joined.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                norm.pop();
            }
            Component::Prefix(p) => {
                let s = p.as_os_str().to_string_lossy();
                if let Some(stripped) = s.strip_prefix(r"\\?\") {
                    norm.push(stripped);
                } else {
                    norm.push(comp.as_os_str());
                }
            }
            other => norm.push(other.as_os_str()),
        }
    }
    norm
}

/// Computes a `/` separated path from directory `from_dir` to `to_file` relative to `base`.
pub fn relative_posix_path(base: &Path, from_dir: &Path, to_file: &Path) -> String {
    let a = lexically_absolutize(base, from_dir);
    let b = lexically_absolutize(base, to_file);
    let ac: Vec<Component<'_>> = a.components().collect();
    let bc: Vec<Component<'_>> = b.components().collect();

    let mut common = 0;
    while common < ac.len() && common < bc.len() {
        let eq = match (ac[common], bc[common]) {
            (Component::Prefix(pa), Component::Prefix(pb)) => {
                pa.as_os_str().to_string_lossy().eq_ignore_ascii_case(&pb.as_os_str().to_string_lossy())
            }
            (ca, cb) => ca == cb,
        };
        if !eq {
            break;
        }
        common += 1;
    }

    if common == 0 {
        return posix_str(&b);
    }

    let mut parts: Vec<String> = Vec::new();
    for c in &ac[common..] {
        if matches!(c, Component::Normal(_)) {
            parts.push("..".into());
        }
    }
    for c in &bc[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

struct DiscoveredImage {
    stage_id: String,
    variant: String,
    file_name: String,
    rel_path: String,
    width: u32,
    height: u32,
    bytes: u64,
}

struct DiscoveredText {
    stage_id: String,
    variant: String,
    file_name: String,
    rel_path: String,
    content: String,
    truncated: bool,
}

struct PreviewCatalogIndex {
    unique_file_stage: BTreeMap<String, String>,
    shared_file_candidates: BTreeMap<String, Vec<String>>,
    slug_to_stage: BTreeMap<String, String>,
}

fn build_preview_index(catalog: &JsonVal) -> PreviewCatalogIndex {
    let mut all_owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut slug_to_stage = BTreeMap::new();
    for (slug, id) in [
        ("world", "continents-and-plates"),
        ("continents-and-plates", "continents-and-plates"),
        ("shelf", "shelf-bathymetry"),
        ("shelf-bathymetry", "shelf-bathymetry"),
        ("lithology", "lithology"),
        ("uplift", "tectonic-uplift"),
        ("tectonic-uplift", "tectonic-uplift"),
        ("climate", "orographic-climate"),
        ("orographic-climate", "orographic-climate"),
        ("arid", "arid-class-rules"),
        ("arid-class-rules", "arid-class-rules"),
        ("ice", "ice-history"),
        ("ice-history", "ice-history"),
        ("stages", "k7-local-refinement"),
        ("chain", "k7-local-refinement"),
        ("k7-local-refinement", "k7-local-refinement"),
    ] {
        slug_to_stage.insert(slug.to_string(), id.to_string());
    }

    if let Some(stages) = catalog.get("stages").and_then(JsonVal::as_arr) {
        for st in stages {
            let Some(id) = st.get("id").and_then(JsonVal::as_str) else { continue };
            let Some(preview) = st.get("preview") else { continue };
            for arr_key in ["files", "optional_files"] {
                if let Some(files) = preview.get(arr_key).and_then(JsonVal::as_arr) {
                    for f in files.iter().filter_map(JsonVal::as_str) {
                        let base = Path::new(f).file_name().and_then(|s| s.to_str()).unwrap_or(f);
                        all_owners.entry(base.to_string()).or_default().push(id.to_string());
                    }
                }
            }
        }
    }

    let mut unique_file_stage = BTreeMap::new();
    let mut shared_file_candidates = BTreeMap::new();
    for (file, mut owners) in all_owners {
        owners.sort();
        owners.dedup();
        if owners.len() == 1 {
            unique_file_stage.insert(file, owners.remove(0));
        } else {
            shared_file_candidates.insert(file, owners);
        }
    }
    PreviewCatalogIndex { unique_file_stage, shared_file_candidates, slug_to_stage }
}

fn collect_files_rec(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_files_rec(&p, out);
        } else if p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("txt")) {
            out.insert(p);
        }
    }
}

fn infer_stage_and_variant(path: &Path, file_name: &str, idx: &PreviewCatalogIndex, chain_dir: Option<&Path>) -> (String, String) {
    let parent_slug = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("");
    let parent_stage = idx.slug_to_stage.get(parent_slug).cloned().or_else(|| {
        if let Some(p) = path.parent() {
            if p.join("lithology.txt").is_file() {
                return Some("lithology".to_string());
            }
            if p.join("uplift.txt").is_file() {
                return Some("tectonic-uplift".to_string());
            }
        }
        None
    });

    let norm_path = posix_str(path);
    let in_chain =
        chain_dir.is_some_and(|c| lexically_absolutize(Path::new("."), path).starts_with(lexically_absolutize(Path::new("."), c)));

    let stage_id = if in_chain
        || norm_path.contains("/review/stages/")
        || norm_path.contains("/review/look/")
        || norm_path.contains("/stages/sheets/")
    {
        "k7-local-refinement".to_string()
    } else if let Some(candidates) = idx.shared_file_candidates.get(file_name) {
        parent_stage.clone().filter(|ps| candidates.contains(ps)).unwrap_or_else(|| candidates[0].clone())
    } else if let Some(unique) = idx.unique_file_stage.get(file_name) {
        unique.clone()
    } else if let Some(ps) = parent_stage {
        ps
    } else {
        "k7-local-refinement".to_string()
    };

    let variant = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .find(|s| s.starts_with("seed-") || s.starts_with("world-") || *s == "ci" || *s == "chain" || *s == "transects")
        .map(str::to_string)
        .unwrap_or_else(|| {
            if in_chain {
                "chain".into()
            } else if !parent_slug.is_empty() {
                parent_slug.into()
            } else {
                "default".into()
            }
        });

    (stage_id, variant)
}

fn discover_artifacts(root: &Path, out_dir: &Path, chain: Option<&str>, catalog: &JsonVal) -> (Vec<DiscoveredImage>, Vec<DiscoveredText>) {
    let idx = build_preview_index(catalog);
    let mut scan_roots = vec![
        out_dir.join("previews"),
        root.join("target/preview"),
        root.join("target/review/stages"),
        root.join("target/review/look"),
        root.join("docs/tools/stages/sheets"),
    ];
    let chain_path = chain.map(|c| lexically_absolutize(root, Path::new(c)));
    if let Some(cp) = &chain_path {
        scan_roots.push(cp.clone());
    }

    let mut seen_abs = BTreeSet::new();
    let mut files = BTreeSet::new();
    for sr in &scan_roots {
        collect_files_rec(sr, &mut files);
    }

    let mut images = Vec::new();
    let mut texts = Vec::new();
    for f in files {
        let abs = lexically_absolutize(root, &f);
        if !seen_abs.insert(abs.clone()) {
            continue;
        }
        let Some(file_name) = abs.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
        let (stage_id, variant) = infer_stage_and_variant(&abs, &file_name, &idx, chain_path.as_deref());
        let rel_path = relative_posix_path(root, out_dir, &abs);

        if abs.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
            let Some((width, height)) = png_dimensions(&abs) else { continue };
            let bytes = fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
            images.push(DiscoveredImage { stage_id, variant, file_name, rel_path, width, height, bytes });
        } else if abs.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")) {
            let Ok(raw_bytes) = fs::read(&abs) else { continue };
            let truncated = raw_bytes.len() > MAX_TXT_BYTES;
            let slice = if truncated { &raw_bytes[..MAX_TXT_BYTES] } else { &raw_bytes[..] };
            let content = String::from_utf8_lossy(slice).into_owned();
            texts.push(DiscoveredText { stage_id, variant, file_name, rel_path, content, truncated });
        }
    }

    (images, texts)
}

fn bundle_json_value(pipeline: &JsonVal, images: &[DiscoveredImage], texts: &[DiscoveredText], manifest_rows: &[ManifestRow]) -> JsonVal {
    let img_vals = images
        .iter()
        .map(|im| {
            JsonVal::Obj(vec![
                ("stage_id".into(), JsonVal::Str(im.stage_id.clone())),
                ("variant".into(), JsonVal::Str(im.variant.clone())),
                ("file_name".into(), JsonVal::Str(im.file_name.clone())),
                ("rel_path".into(), JsonVal::Str(im.rel_path.clone())),
                ("width".into(), JsonVal::Num(im.width.to_string())),
                ("height".into(), JsonVal::Num(im.height.to_string())),
                ("bytes".into(), JsonVal::Num(im.bytes.to_string())),
            ])
        })
        .collect();

    let txt_vals = texts
        .iter()
        .map(|tx| {
            JsonVal::Obj(vec![
                ("stage_id".into(), JsonVal::Str(tx.stage_id.clone())),
                ("variant".into(), JsonVal::Str(tx.variant.clone())),
                ("file_name".into(), JsonVal::Str(tx.file_name.clone())),
                ("rel_path".into(), JsonVal::Str(tx.rel_path.clone())),
                ("content".into(), JsonVal::Str(tx.content.clone())),
                ("truncated".into(), JsonVal::Bool(tx.truncated)),
            ])
        })
        .collect();

    let row_vals = manifest_rows
        .iter()
        .map(|r| {
            JsonVal::Obj(vec![
                ("stage".into(), JsonVal::Str(r.stage.clone())),
                ("field".into(), JsonVal::Str(r.field.clone())),
                ("kind_id".into(), JsonVal::Str(r.kind_id.clone())),
                (
                    "stage_id".into(),
                    match &r.stage_id {
                        Some(id) => JsonVal::Str(id.clone()),
                        None => JsonVal::Null,
                    },
                ),
                ("cells".into(), JsonVal::Str(r.cells.clone())),
                ("min".into(), JsonVal::Str(r.min.clone())),
                ("p5".into(), JsonVal::Str(r.p5.clone())),
                ("p50".into(), JsonVal::Str(r.p50.clone())),
                ("p95".into(), JsonVal::Str(r.p95.clone())),
                ("max".into(), JsonVal::Str(r.max.clone())),
                ("hash".into(), JsonVal::Str(r.hash.clone())),
                ("extras".into(), JsonVal::Str(r.extras.clone())),
            ])
        })
        .collect();

    JsonVal::Obj(vec![
        ("schema_format_version".into(), JsonVal::Num("2".into())),
        ("cli_help".into(), JsonVal::Str(help_text())),
        ("world".into(), pipeline.get("world").cloned().unwrap_or(JsonVal::Null)),
        ("active_stages".into(), pipeline.get("active_stages").cloned().unwrap_or(JsonVal::Null)),
        ("walked_chain".into(), pipeline.get("walked_chain").cloned().unwrap_or(JsonVal::Null)),
        ("catalog".into(), pipeline.get("catalog").cloned().unwrap_or(JsonVal::Null)),
        ("artifacts".into(), JsonVal::Obj(vec![("images".into(), JsonVal::Arr(img_vals)), ("texts".into(), JsonVal::Arr(txt_vals))])),
        ("manifest_rows".into(), JsonVal::Arr(row_vals)),
    ])
}

/// Assembles `<out_dir>/index.html` purely offline from an already-normalized pipeline JSON string and discovered previews.
pub fn assemble_cockpit(root: &Path, pipeline_json_raw: &str, out_dir: &Path, chain: Option<&str>) -> Res<AssembleSummary> {
    let pipeline = validate_and_normalize_pipeline(pipeline_json_raw)?;
    let catalog = pipeline.get("catalog").ok_or_else(|| "missing catalog object".to_string())?;
    let stage_count = catalog.get("stages").and_then(JsonVal::as_arr).map_or(0, |a| a.len());
    let pin_count = catalog.get("version_pins").and_then(JsonVal::as_arr).map_or(0, |a| a.len());

    let manifest_rows = match fs::read_to_string(root.join("docs/tools/stages/manifest.txt")) {
        Ok(txt) => parse_stage_manifest(&txt, catalog),
        Err(_) => Vec::new(),
    };
    let (images, texts) = discover_artifacts(root, out_dir, chain, catalog);

    let title = match pipeline.get("world").and_then(|w| w.get("seed")).and_then(JsonVal::as_u64) {
        Some(seed) => format!("Pipeline Cockpit — Seed {seed}"),
        None => "Pipeline Cockpit — Schema v2".to_string(),
    };
    let bundle = bundle_json_value(&pipeline, &images, &texts, &manifest_rows);
    let safe_json = bundle.to_safe_script_json();
    let page = html::render_html(&title, &safe_json);

    fs::create_dir_all(out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;
    let index_html = out_dir.join("index.html");
    fs::write(&index_html, page).map_err(|e| format!("writing {}: {e}", index_html.display()))?;

    Ok(AssembleSummary {
        index_html,
        stage_count,
        pin_count,
        image_count: images.len(),
        text_count: texts.len(),
        manifest_count: manifest_rows.len(),
    })
}

pub fn run_cmd(root: &Path, args: &[String]) -> Res {
    let o = parse_args(args)?;
    if o.help {
        print!("{}", help_text());
        return Ok(());
    }
    let out_dir = lexically_absolutize(root, Path::new(&o.out_dir));
    fs::create_dir_all(&out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;

    let pipeline_path = match &o.pipeline_json {
        Some(p) => lexically_absolutize(root, Path::new(p)),
        None => {
            let p = out_dir.join("pipeline.json");
            let cargo = build_pipeline_json_cmd(&o, &p);
            let refs: Vec<&str> = cargo.iter().map(String::as_str).collect();
            run(root, "cargo", &refs, &[])?;
            p
        }
    };

    let raw_pipeline = fs::read_to_string(&pipeline_path).map_err(|e| format!("reading {}: {e}", pipeline_path.display()))?;
    let normalized = validate_and_normalize_pipeline(&raw_pipeline)?;
    let primary_seed = normalized.get("world").and_then(|w| w.get("seed")).and_then(JsonVal::as_u64).or(o.seed).unwrap_or(DEFAULT_SEED);

    if o.run.any() {
        for step in build_run_plan(&o, primary_seed, &out_dir) {
            eprintln!("cockpit: running {}", step.label);
            let refs: Vec<&str> = step.cargo_args.iter().map(String::as_str).collect();
            run(root, "cargo", &refs, &[])?;
        }
    }

    let summary = assemble_cockpit(root, &raw_pipeline, &out_dir, o.chain.as_deref())?;
    let rel = relative_posix_path(root, root, &summary.index_html);
    println!(
        "cockpit: wrote {rel} ({} stages, {} pins, {} images, {} texts, {} manifest probes)",
        summary.stage_count, summary.pin_count, summary.image_count, summary.text_count, summary.manifest_count
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strips `//` and `/* ... */` comments outside string literals for lightweight Rust source inspection.
    fn strip_rust_comments(src: &str) -> String {
        let b = src.as_bytes();
        let mut out = String::with_capacity(src.len());
        let mut i = 0;
        let mut in_str = false;
        while i < b.len() {
            if !in_str && b.get(i..i + 2) == Some(b"//") {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            } else if !in_str && b.get(i..i + 2) == Some(b"/*") {
                i += 2;
                let mut depth = 1usize;
                while i + 1 < b.len() && depth > 0 {
                    if b.get(i..i + 2) == Some(b"/*") {
                        depth += 1;
                        i += 2;
                    } else if b.get(i..i + 2) == Some(b"*/") {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            } else {
                let ch = src[i..].chars().next().expect("valid UTF-8");
                if ch == '\\' && in_str && i + 1 < b.len() {
                    let next = src[i + 1..].chars().next().expect("valid UTF-8");
                    out.push(ch);
                    out.push(next);
                    i += 1 + next.len_utf8();
                    continue;
                }
                if ch == '"' {
                    in_str = !in_str;
                }
                out.push(ch);
                i += ch.len_utf8();
            }
        }
        out
    }

    /// Extracts variant names declared directly inside `enum <enum_name> { ... }`.
    fn extract_enum_variants(src: &str, enum_name: &str) -> Result<Vec<String>, String> {
        let clean = strip_rust_comments(src);
        let needle = format!("enum {enum_name}");
        let start = clean.find(&needle).ok_or_else(|| format!("enum {enum_name} not found"))?;
        let brace_rel = clean[start..].find('{').ok_or_else(|| format!("opening '{{' for enum {enum_name} not found"))?;
        let mut body_end = clean.len();
        let mut depth = 0usize;
        let body_open = start + brace_rel;
        for (offset, ch) in clean[body_open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        body_end = body_open + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &clean[body_open + 1..body_end];

        let mut variants = Vec::new();
        let mut b_depth = 0usize;
        let mut p_depth = 0usize;
        let mut s_depth = 0usize;
        let mut chunk = String::new();
        for ch in body.chars() {
            if b_depth > 0 || p_depth > 0 || s_depth > 0 {
                match ch {
                    '{' => b_depth += 1,
                    '}' => b_depth -= 1,
                    '(' => p_depth += 1,
                    ')' => p_depth -= 1,
                    '[' => s_depth += 1,
                    ']' => s_depth -= 1,
                    _ => {}
                }
                continue;
            }
            match ch {
                '{' => b_depth += 1,
                '(' => p_depth += 1,
                '[' => s_depth += 1,
                ',' => {
                    if let Some(v) = parse_variant_ident(&chunk) {
                        variants.push(v);
                    }
                    chunk.clear();
                }
                _ => chunk.push(ch),
            }
        }
        if let Some(v) = parse_variant_ident(&chunk) {
            variants.push(v);
        }
        Ok(variants)
    }

    fn parse_variant_ident(chunk: &str) -> Option<String> {
        let mut rest = chunk.trim();
        while rest.starts_with('#') {
            rest = rest.trim_start_matches('#').trim();
        }
        let ident: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if ident.is_empty() {
            None
        } else {
            Some(ident)
        }
    }

    /// Extracts `Stage::<Variant>` names inside `pub const ALL_KINDS`.
    fn extract_all_kinds_variants(src: &str) -> Result<Vec<String>, String> {
        let clean = strip_rust_comments(src);
        let pos = clean.find("const ALL_KINDS").ok_or_else(|| "const ALL_KINDS not found".to_string())?;
        let eq = clean[pos..].find('=').ok_or_else(|| "ALL_KINDS '=' not found".to_string())?;
        let open = clean[pos + eq..].find('[').ok_or_else(|| "ALL_KINDS '[' not found".to_string())?;
        let body_start = pos + eq + open + 1;
        let close = clean[body_start..].find("];").ok_or_else(|| "ALL_KINDS '];' not found".to_string())?;
        let body = &clean[body_start..body_start + close];

        let mut out = Vec::new();
        let mut rest = body;
        while let Some(idx) = rest.find("Stage::") {
            let after = &rest[idx + "Stage::".len()..];
            let ident: String = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            let n = ident.len();
            if !ident.is_empty() {
                out.push(ident);
            }
            rest = &after[n..];
        }
        Ok(out)
    }

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-cockpit-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn minimal_png_bytes(w: u32, h: u32) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(&13u32.to_be_bytes());
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]);
        v
    }

    fn sample_catalog_json(schema_ver: u32) -> String {
        format!(
            r#"{{
  "schema_format_version": {schema_ver},
  "default_height_field": "cpu-erosion-v1",
  "known_height_fields": ["cpu-erosion-v1", "cpu-erosion-c1-v2"],
  "layer_stack": ["base"],
  "turned_noise_default": true,
  "probe_stage_kinds": ["landmass", "k7"],
  "version_pins": [
    {{"key": "bake_kernel", "value": 3, "const_path": "planet_generators::bake::BAKE_KERNEL_VERSION", "spec_ids": ["GEN-161"]}}
  ],
  "stages": [
    {{
      "id": "shelf-bathymetry",
      "title": "Continental Shelf",
      "k_label": "K2",
      "scope": "global",
      "summary": "Coast shelf preview",
      "reads": [],
      "outputs": ["bathymetry.png"],
      "version_pins": [],
      "probe_hooks": [],
      "active_when": "always",
      "edit_preview_level": "L0",
      "spec_ids": ["GEN-118"],
      "preview": {{"command_template": "cargo run --release -p planet -- generate --shelf-preview {{out_dir}} --seed {{seed}}", "files": ["bathymetry.png", "shelf.txt"], "optional_files": []}}
    }},
    {{
      "id": "k5-landmass-and-window-cut",
      "title": "K5 Landmass",
      "k_label": "K5",
      "scope": "region",
      "summary": "Landmass extraction",
      "reads": [],
      "outputs": ["z0"],
      "version_pins": ["bake_kernel"],
      "probe_hooks": [{{"kind_id": "landmass", "fields": ["z"]}}],
      "active_when": "erosion_mode == k5-k7-commit",
      "edit_preview_level": "L2",
      "spec_ids": ["GEN-161"],
      "preview": null
    }},
    {{
      "id": "k7-local-refinement",
      "title": "K7 Refinement",
      "k_label": "K7",
      "scope": "tile",
      "summary": "Local tile refinement",
      "reads": ["k5-landmass-and-window-cut"],
      "outputs": ["docs/tools/stages/manifest.txt"],
      "version_pins": [],
      "probe_hooks": [{{"kind_id": "k7", "fields": ["z"]}}],
      "active_when": "erosion_mode == k5-k7-commit",
      "edit_preview_level": "L2",
      "spec_ids": ["BUILD-047"],
      "preview": {{"command_template": "cargo xtask stages", "files": ["docs/tools/stages/manifest.txt"], "optional_files": []}}
    }}
  ],
  "switch_matrix": [
    {{"height_field": "cpu-erosion-v1", "erosion_layer": false, "relief_lattice": false, "channels": false, "implementation_id": "cpu-erosion-v1", "version": 4, "needs_bake": true}}
  ]
}}"#
        )
    }

    // spec: BUILD-064
    #[test]
    fn cli_options_parse_and_invalid_or_clashing_flags_are_refused() {
        let def = parse_args(&[]).unwrap();
        assert_eq!(def.out_dir, DEFAULT_OUT_DIR);
        assert!(needs_heavy_slot(&[]));

        let offline = parse_args(&s(&["--pipeline-json", "target/review/stages/pipeline.json"])).unwrap();
        assert_eq!(offline.pipeline_json.as_deref(), Some("target/review/stages/pipeline.json"));
        assert!(!needs_heavy_slot(&s(&["--pipeline-json", "target/review/stages/pipeline.json"])));
        assert!(needs_heavy_slot(&s(&["--pipeline-json", "p.json", "--run", "drivers"])));

        let full = parse_args(&s(&[
            "--world",
            "w.toml",
            "--erosion",
            "on",
            "--channels",
            "on",
            "--run",
            "world,stages",
            "--compare-seed",
            "7",
            "--chain",
            "d/r1h",
            "--face",
            "3",
            "--km",
            "25",
            "--threads",
            "4",
            "--out-dir",
            "custom/out",
        ]))
        .unwrap();
        assert_eq!(full.world.as_deref(), Some("w.toml"));
        assert_eq!(full.erosion, Some(true));
        assert_eq!(full.channels, Some(true));
        assert_eq!(full.run, RunTargets { world: true, drivers: false, stages: true });
        assert_eq!(full.compare_seed, Some(7));
        assert_eq!(full.face, Some(3));
        assert_eq!(full.km, Some(25));
        assert_eq!(full.threads, Some(4));
        assert_eq!(full.out_dir, "custom/out");

        assert!(parse_args(&s(&["--unknown"])).unwrap_err().contains("unknown option"));
        assert!(parse_args(&s(&["--seed", "1", "--seed", "2"])).unwrap_err().contains("twice"));
        assert!(parse_args(&s(&["--seed", "1", "--world", "w.toml"])).unwrap_err().contains("give one"));
        assert!(parse_args(&s(&["--channels", "on"])).unwrap_err().contains("requires --erosion on"));
        assert!(parse_args(&s(&["--commit", "c", "--erosion", "on"])).unwrap_err().contains("--commit already carries"));
        assert!(parse_args(&s(&["--pipeline-json", "p.json", "--world", "w.toml"])).unwrap_err().contains("reads an existing JSON"));
        assert!(parse_args(&s(&["--face", "2"])).unwrap_err().contains("--chain DIR"));
        assert!(parse_args(&s(&["--chain", "d", "--face", "6"])).unwrap_err().contains("0..=5"));
        assert!(parse_args(&s(&["--km", "0"])).unwrap_err().contains(">= 1"));
        assert!(parse_args(&s(&["--run", "bogus"])).unwrap_err().contains("world, drivers, stages, all"));
        assert!(!needs_heavy_slot(&s(&["--bad-flag"])));
    }

    // spec: BUILD-064
    #[test]
    fn run_plan_builds_exact_world_driver_stage_and_compare_seed_commands() {
        let mut o = parse_args(&s(&[
            "--seed",
            "14",
            "--erosion",
            "on",
            "--channels",
            "on",
            "--run",
            "all",
            "--compare-seed",
            "7",
            "--chain",
            "chains/r1h",
            "--face",
            "4",
            "--km",
            "40",
            "--threads",
            "2",
        ]))
        .unwrap();
        let pj = build_pipeline_json_cmd(&o, Path::new("target/review/cockpit/pipeline.json"));
        assert_eq!(
            pj,
            s(&[
                "run",
                "-q",
                "-p",
                "planet",
                "--",
                "generate",
                "--pipeline-json",
                "target/review/cockpit/pipeline.json",
                "--seed",
                "14",
                "--erosion",
                "on",
                "--channels",
                "on"
            ])
        );

        let plan = build_run_plan(&o, 14, Path::new("target/review/cockpit"));
        // 2 world + 12 drivers (6 primary + 6 compare) + 3 stage commands (manifest + stage_sheet + transect_sheet) = 17
        assert_eq!(plan.len(), 17, "{plan:?}");
        assert!(plan[0].cargo_args.windows(2).any(|w| w == ["--seed", "14"]) && plan[0].cargo_args.contains(&"preview_world".into()));
        assert!(plan[1].cargo_args.windows(2).any(|w| w == ["--seed", "7"]) && plan[1].cargo_args.contains(&"preview_world".into()));
        assert!(plan[2].cargo_args.windows(2).any(|w| w == ["--shelf-preview", "target/review/cockpit/previews/seed-14/shelf"]));
        assert!(plan[2].cargo_args.windows(2).any(|w| w == ["--km", "40"]));
        assert!(plan[2].cargo_args.windows(2).any(|w| w == ["--threads", "2"]));
        assert!(plan[3].cargo_args.windows(2).any(|w| w == ["--shelf-preview", "target/review/cockpit/previews/seed-7/shelf"]));
        assert!(plan[14].cargo_args.contains(&"stage_manifest".into()));
        assert!(
            plan[15].cargo_args.contains(&"stage_sheet".into())
                && plan[15].cargo_args.windows(2).any(|w| w == ["--out", "target/review/cockpit/previews/chain/stage-sheet.png"])
        );
        assert!(plan[16].cargo_args.contains(&"transect_sheet".into()) && plan[16].cargo_args.windows(2).any(|w| w == ["--face", "4"]));

        o.world = Some("worlds/earthlike.toml".into());
        o.seed = None;
        o.erosion = None;
        o.channels = None;
        o.commit = Some("commits/g1f11".into());
        let pjc = build_pipeline_json_cmd(&o, Path::new("out/pipeline.json"));
        assert!(pjc.windows(2).any(|w| w == ["--world", "worlds/earthlike.toml"]));
        assert!(pjc.windows(2).any(|w| w == ["--commit", "commits/g1f11"]));
    }

    // spec: BUILD-064
    #[test]
    fn json_parser_and_validator_accept_resolved_and_catalog_shapes_and_refuse_bad_versions() {
        let cat = sample_catalog_json(2);
        let norm_cat = validate_and_normalize_pipeline(&cat).unwrap();
        assert!(norm_cat.get("world").is_some());
        assert_eq!(norm_cat.get("catalog").unwrap().get("stages").and_then(JsonVal::as_arr).unwrap().len(), 3);

        let resolved = format!(
            r#"{{
  "schema_format_version": 2,
  "world": {{"seed": 14, "base_node_hash": "0011223344556677", "definition_hash": "8899aabbccddeeff", "height_field": "cpu-erosion-v1", "erosion_mode": "none", "sea_level_m": 0.0}},
  "active_stages": ["shelf-bathymetry"],
  "walked_chain": {{"turned_noise": true, "has_driver_channels": false, "water_cull_level_m": null, "links_top_down": []}},
  "catalog": {cat}
}}"#
        );
        let norm_res = validate_and_normalize_pipeline(&resolved).unwrap();
        assert_eq!(norm_res.get("world").unwrap().get("seed").and_then(JsonVal::as_u64), Some(14));

        assert!(validate_and_normalize_pipeline(&sample_catalog_json(1)).unwrap_err().contains("schema_format_version == 2"));
        assert!(validate_and_normalize_pipeline("{\"schema_format_version\": 2}").unwrap_err().contains("catalog"));
        assert!(validate_and_normalize_pipeline("{bad json").is_err());

        let v = JsonVal::Obj(vec![("msg".into(), JsonVal::Str("hello </script> \u{2603} world".into()))]);
        let escaped = v.to_safe_script_json();
        assert!(!escaped.contains("</script>"), "{escaped}");
        assert!(escaped.contains("<\\/script>"), "{escaped}");
        assert_eq!(parse_json(&escaped).unwrap().get("msg").and_then(JsonVal::as_str), Some("hello </script> \u{2603} world"));
    }

    // spec: BUILD-064
    #[test]
    fn manifest_parser_png_header_and_offline_assembler_index_previews_into_self_contained_html() {
        let root = temp_workspace("assemble");
        fs::create_dir_all(root.join("docs/tools/stages")).unwrap();
        fs::write(
            root.join("docs/tools/stages/manifest.txt"),
            "landmass | z | cells 1408 | min -1.0e2 | p5 0.0 | p50 1.2e2 | p95 5.0e2 | max 9.0e2 | below_scale 0 | above_scale 0 | hash 26a251fd07de1f70 | pit_cells 29\nk7-f1-t3-4 | z | cells 4096 | min 4.0e2 | p5 4.1e2 | p50 4.2e2 | p95 4.3e2 | max 4.4e2 | below_scale 0 | above_scale 0 | hash 3a78b8893d8f388e\n",
        )
        .unwrap();

        let out_dir = root.join("target/review/cockpit");
        let s14_dir = out_dir.join("previews/seed-14/shelf");
        let s7_dir = out_dir.join("previews/seed-7/shelf");
        fs::create_dir_all(&s14_dir).unwrap();
        fs::create_dir_all(&s7_dir).unwrap();
        fs::write(s14_dir.join("bathymetry.png"), minimal_png_bytes(512, 256)).unwrap();
        fs::write(s14_dir.join("shelf.txt"), "shelf metrics seed 14 with </script> trap\n").unwrap();
        fs::write(s7_dir.join("bathymetry.png"), minimal_png_bytes(512, 256)).unwrap();
        fs::write(s7_dir.join("shelf.txt"), "shelf metrics seed 7\n").unwrap();

        let summary = assemble_cockpit(&root, &sample_catalog_json(2), &out_dir, None).unwrap();
        assert_eq!(summary.stage_count, 3);
        assert_eq!(summary.pin_count, 1);
        assert_eq!(summary.image_count, 2);
        assert_eq!(summary.text_count, 2);
        assert_eq!(summary.manifest_count, 2);

        let html = fs::read_to_string(&summary.index_html).unwrap();
        assert!(!html.contains("http://") && !html.contains("https://"), "must remain offline");
        let marker = r#"<script id="cockpit-bundle" type="application/json">"#;
        let start = html.find(marker).unwrap() + marker.len();
        let end = html[start..].find("</script>").unwrap() + start;
        let embedded = &html[start..end];
        let parsed = parse_json(embedded).unwrap();
        let imgs = parsed.get("artifacts").unwrap().get("images").and_then(JsonVal::as_arr).unwrap();
        assert_eq!(imgs.len(), 2);
        assert!(imgs.iter().any(|i| i.get("rel_path").and_then(JsonVal::as_str) == Some("previews/seed-14/shelf/bathymetry.png")));
        assert!(imgs
            .iter()
            .all(|i| i.get("width").and_then(JsonVal::as_u64) == Some(512) && i.get("height").and_then(JsonVal::as_u64) == Some(256)));
        assert!(imgs.iter().all(|i| i.get("stage_id").and_then(JsonVal::as_str) == Some("shelf-bathymetry")));

        let rows = parsed.get("manifest_rows").and_then(JsonVal::as_arr).unwrap();
        assert_eq!(rows[0].get("stage_id").and_then(JsonVal::as_str), Some("k5-landmass-and-window-cut"));
        assert_eq!(rows[1].get("stage_id").and_then(JsonVal::as_str), Some("k7-local-refinement"));

        let texts = parsed.get("artifacts").unwrap().get("texts").and_then(JsonVal::as_arr).unwrap();
        assert!(texts.iter().any(|t| t.get("content").and_then(JsonVal::as_str).unwrap().contains("</script>")));

        assert_eq!(
            relative_posix_path(&root, &out_dir, &root.join("target/preview/world-seed-14/k.png")),
            "../../preview/world-seed-14/k.png"
        );
        let _ = fs::remove_dir_all(&root);
    }

    // spec: BUILD-064, BUILD-067, TEST-150
    #[test]
    fn the_look_sheets_under_target_review_look_are_indexed_with_the_stage_sheets() {
        let root = temp_workspace("look");
        let look = root.join("target/review/look/r1h-switch");
        fs::create_dir_all(&look).unwrap();
        fs::write(look.join("river-az315.png"), minimal_png_bytes(1056, 442)).unwrap();
        fs::write(
            look.join("look.txt"),
            "look-numbers 1
run r1h-switch
",
        )
        .unwrap();
        let out_dir = root.join("target/review/cockpit");
        let summary = assemble_cockpit(&root, &sample_catalog_json(2), &out_dir, None).unwrap();
        assert_eq!((summary.image_count, summary.text_count), (1, 1));
        let html = fs::read_to_string(&summary.index_html).unwrap();
        let marker = r#"<script id="cockpit-bundle" type="application/json">"#;
        let start = html.find(marker).unwrap() + marker.len();
        let end = html[start..].find("</script>").unwrap() + start;
        let parsed = parse_json(&html[start..end]).unwrap();
        let imgs = parsed.get("artifacts").unwrap().get("images").and_then(JsonVal::as_arr).unwrap();
        assert_eq!(imgs[0].get("rel_path").and_then(JsonVal::as_str), Some("../look/r1h-switch/river-az315.png"));
        assert_eq!(imgs[0].get("stage_id").and_then(JsonVal::as_str), Some("k7-local-refinement"), "filed with the stage sheets");
        assert_eq!(imgs[0].get("variant").and_then(JsonVal::as_str), Some("r1h-switch"), "the run is the variant");
        let _ = fs::remove_dir_all(&root);
    }

    // spec: BUILD-063, BUILD-064
    #[test]
    fn probe_stage_all_kinds_covers_every_enum_variant_in_probe_rs() {
        let synthetic = r#"
            pub enum Sample {
                Alpha,
                // CommentedOut,
                /* BlockOut { a: u32 }, */
                #[allow(dead_code)]
                Beta(u32, u8),
                Gamma { x: i32, y: i32 },
            }
        "#;
        assert_eq!(extract_enum_variants(synthetic, "Sample").unwrap(), vec!["Alpha", "Beta", "Gamma"]);

        let probe_path = crate::util::repo_root().join("crates/generators/src/bake/probe.rs");
        let src = fs::read_to_string(&probe_path).expect("read probe.rs");
        let enum_variants = extract_enum_variants(&src, "Stage").expect("parse Stage enum");
        let all_kinds = extract_all_kinds_variants(&src).expect("parse Stage::ALL_KINDS");
        assert!(!enum_variants.is_empty());
        assert_eq!(enum_variants, all_kinds, "Stage::ALL_KINDS must cover every Stage enum variant in execution order");
    }

    // spec: BUILD-064
    #[test]
    fn help_text_and_flag_registry_stay_in_sync_with_parse_args() {
        let short = parse_args(&s(&["-h"])).unwrap();
        let long = parse_args(&s(&["--help"])).unwrap();
        assert!(short.help && long.help);
        assert!(!needs_heavy_slot(&s(&["-h"])));
        assert!(!needs_heavy_slot(&s(&["--help"])));

        let help = help_text();
        assert!(help.contains(&DEFAULT_SEED.to_string()));
        assert!(help.contains(DEFAULT_OUT_DIR));

        let mut documented_flags = BTreeSet::new();
        for spec in FLAG_SPECS {
            assert!(help.contains(spec.flags), "help missing flag spec: {}", spec.flags);
            for tok in spec.flags.split(',') {
                documented_flags.insert(tok.trim().to_string());
            }
        }
        for (target, _) in RUN_TARGET_SPECS {
            assert!(help.contains(target), "help missing --run target: {target}");
        }
        for (_, argv) in EXAMPLE_INVOCATIONS {
            let owned = s(argv);
            assert!(parse_args(&owned).is_ok(), "stale help example: {argv:?}");
            let expected_line =
                if argv.is_empty() { "cargo xtask cockpit\n".to_string() } else { format!("cargo xtask cockpit {}\n", argv.join(" ")) };
            assert!(help.contains(&expected_line), "help missing example: {expected_line}");
        }

        let mod_src = fs::read_to_string(crate::util::repo_root().join("xtask/src/cockpit/mod.rs")).expect("read cockpit/mod.rs");
        let match_start = mod_src.find("match arg.as_str() {").expect("match arg.as_str()");
        let match_end = mod_src[match_start..].find("other =>").expect("other => arm") + match_start;
        let mut parsed_flags = BTreeSet::new();
        for line in mod_src[match_start..match_end].lines().skip(1) {
            let Some((lhs, _)) = line.split_once("=>") else { continue };
            for raw in lhs.split('|') {
                let tok = raw.trim().trim_matches('"');
                if tok.starts_with('-') {
                    parsed_flags.insert(tok.to_string());
                }
            }
        }
        assert_eq!(parsed_flags, documented_flags, "FLAG_SPECS must stay 1-to-1 with parse_args flags");
    }
}
