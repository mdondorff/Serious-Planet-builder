# Starter: tooling and project setup

**Status:** v2 (2026-10-09). Frozen background document: once M0 is done this is history, not a requirement. Companion to the architecture report (Revision 4). The repository starter files described here (`CLAUDE.md`, `docs/constitution.md`, agents, skills, settings, registers) come as a ready-made bundle (`planet-starter.zip`); this doc explains them and the steps around them. Verify tool versions and commands at install time.

## 1. Order of operations

1. Install the tools (Section 2).
2. Create the GitHub repository and unpack the starter bundle into it (Section 3).
3. Initialise OpenSpec (Section 4).
4. Review `CLAUDE.md` and `docs/constitution.md` (Sections 5–6), adjust anything you disagree with, commit.
5. Start Claude Code in the repo and paste the kickoff prompt (Section 10).

## 2. Install on Rasierklinge (Windows native, no WSL)

| Tool | Why | Install (verify the ID with `winget search` first) |
|---|---|---|
| NVIDIA driver (current) | Real-GPU runs, Vulkan/D3D12 features | GeForce/Studio driver from NVIDIA |
| Git for Windows | Git, and Git Bash for Claude Code's Bash tool (without it Claude Code falls back to PowerShell) | `winget install Git.Git` |
| Visual Studio 2022 Build Tools, "Desktop development with C++" workload | MSVC linker and Windows SDK for the `x86_64-pc-windows-msvc` Rust toolchain | `winget install Microsoft.VisualStudio.2022.BuildTools`, then add the C++ workload in the installer |
| rustup + stable Rust (MSVC) | Compiler, Cargo; add `clippy`, `rustfmt`, `rust-src` | `winget install Rustlang.Rustup`, then `rustup component add clippy rustfmt rust-src` |
| VS Code | Editor | `winget install Microsoft.VisualStudioCode` |
| VS Code extensions | rust-analyzer, CodeLLDB (debugging), Even Better TOML, a WGSL extension, Claude Code extension | VS Code Extensions view |
| Claude Code | The agent | PowerShell: `irm https://claude.ai/install.ps1 \| iex`, then `claude doctor` |
| Node.js LTS (≥ 20.19) | Required by OpenSpec | `winget install OpenJS.NodeJS.LTS` |
| OpenSpec | Spec tooling (Section 4) | `npm install -g @fission-ai/openspec@latest` |
| GitHub CLI | PRs, CI status from the terminal | `winget install GitHub.cli`, then `gh auth login` |
| Cargo tools | Fast tests, snapshot tests, licence and dependency checks | `cargo install cargo-nextest cargo-insta cargo-deny` |
| RenderDoc, Tracy, PIX (Nsight Graphics optional) | Frame capture, profiling | Vendor downloads |

Machine settings:
- Create a **Dev Drive** and put the repo there (not in a OneDrive-synced folder). Add a Defender exclusion for the repo's `target/` directory, or rely on Dev Drive's performance mode.
- `git config --global core.autocrlf false` and `git config --global core.longpaths true`. Line endings are enforced by the bundle's `.gitattributes`.
- Laptop: for performance runs, connect the charger, use the performance power profile, and set the app to "High performance" GPU in Windows Graphics settings (architecture report §8, hybrid graphics).
- GitHub: enable branch protection on `main` (require CI to pass, disallow force pushes). This backs up the Git rules in `CLAUDE.md` for unattended runs.

## 3. Repository layout (from the bundle)

```
planet/                         (on the Dev Drive)
├─ CLAUDE.md                    agent operating manual (bundle)
├─ README.md                    short pointer (bundle)
├─ .gitattributes, .gitignore   (bundle)
├─ .claude/
│  ├─ settings.json             permissions for unattended runs (bundle, Section 7)
│  ├─ agents/                   verifier, spec-guardian, numerics-reviewer (bundle)
│  └─ skills/                   adr, spike, learning, bless, accept (bundle)
├─ openspec/                    created by `openspec init` (Section 4)
├─ docs/
│  ├─ constitution.md           tier 0 rules (bundle)
│  ├─ adr/                      decisions; README lists the decide-now ADRs (bundle)
│  ├─ status.md                 progress log and stop reports (bundle)
│  ├─ spikes.md                 spike register (bundle)
│  ├─ learnings.md              append-only findings (bundle)
│  ├─ milestones/               milestone reports (written by /accept)
│  └─ background/               frozen, read only when pointed to (bundle)
│     ├─ README.md
│     ├─ architecture-report.md
│     └─ starter.md             (this document)
├─ crates/                      created in M0
├─ xtask/                       created in M0; the only command surface
├─ tests/goldens/               per-adapter golden images
└─ .github/workflows/           created in M0: Windows (WARP) + Linux (lavapipe)
```

## 4. Spec tooling: OpenSpec over GitHub Spec Kit

**Recommendation: OpenSpec**, with the constitution, ADRs, spike register, learnings and status log kept as plain Markdown next to it. Confidence: medium-high. Switching cost later is low, because both tools are Markdown plus slash commands.

| Criterion (what this project needs) | OpenSpec | GitHub Spec Kit |
|---|---|---|
| Evergreen specs that change as implementation teaches us | **Strong fit.** `openspec/specs/` is the current truth per capability; each change carries deltas (e.g. "ADDED Requirements") that are merged into the specs when the change is archived | Organised around one spec folder per feature. Current state is spread across features; it has a guide for evolving existing specs, but it isn't the core model |
| Many subsystems evolving in parallel over a long project | Changes are small, independent folders; archive keeps history | Each feature runs the full specify → plan → tasks → implement flow |
| Process weight | Light: propose → apply → archive (optional explore and verify steps) | Heavier: constitution, specify, plan, tasks, implement, converge, plus quality gates and extensions |
| Quality gates | Fewer built in | Stronger built-in gates (clarification, checklists, consistency analysis, converge loop) |
| Constitution | Not a first-class artifact; we keep `docs/constitution.md` | First-class constitution |
| Claude Code support | Supported (tool id `claude`); generates skills under `.claude/skills/` and/or commands | Supported (`--integration claude`) |
| Windows | Node.js ≥ 20.19, npm | Python ≥ 3.11 + uv; PowerShell scripts by default on Windows |

Why OpenSpec wins here: the central requirement is that specs stay current while decisions are made late and learnings arrive during implementation. OpenSpec's "current specs + change deltas" model is that requirement. Spec Kit's strengths (constitution and phase gates) are covered by the project's own pieces: the constitution, ADRs, `spec-lint`, and the `spec-guardian` and `verifier` agents.

Initialise after unpacking the bundle: run `openspec init` in the repo and select Claude Code. OpenSpec may add its own instructions file or a managed block to `CLAUDE.md`; that's fine, but check afterwards that the bundle's content is still there. Then the first M0 session seeds:
- one spec per subsystem: `coordinates`, `world-definition`, `terrain-lod`, `streaming-cache`, `generator-pipeline`, `authored-layers`, `renderer`, `atmosphere-ocean`, `testing`, `build-tooling`;
- requirement headings with trace ID and tier, e.g. `### Requirement: COORD-003 Geo round-trip` with a line `Verify: A`;
- milestones as one or more changes each, e.g. `m1-coordinates`, `m1-walking-skeleton`.

## 5. CLAUDE.md (in the bundle)

What it contains and why:
- **Sources of truth and precedence** (Section 9), with the instruction to read `docs/background/` only when pointed at a specific section.
- **Commands**, all through `cargo xtask`; Claude creates missing xtask commands rather than scripts.
- **How to work:** OpenSpec change per piece of work, failing test first at the cheapest tier, reviewer agents, verifier before "done", archive, status line.
- **Rules that are easy to break** (FramePlan boundary, precision, determinism, actionable failures, no blessing, no weakened tests).
- **Git:** branch per change, PR via `gh`, self-merge only when CI is green on both OSes and the verifier passed; no force-push.
- **When to stop and report**, the list that makes long unattended runs safe (Section 10).

## 6. docs/constitution.md (in the bundle)

27 short rules (CON-01 … CON-27), each with a verification tier, distilled from report §3, §8, §10, §12, §16 and §17: layering and the FramePlan boundary, generator purity and cache keys, staged generation, authored layers, numerics and determinism, verification rules, performance gates on Rasierklinge, and process rules. Only the owner amends it; Claude proposes amendments as ADRs.

## 7. Agents, skills and permissions (in the bundle)

**Agents** (`.claude/agents/`; a new agents directory needs a Claude Code restart):

| Agent | Role | Tools |
|---|---|---|
| `verifier` | Runs the test tiers, inspects review images, reports pass/fail per requirement with evidence; never edits | Read, Glob, Grep, Bash |
| `spec-guardian` | Reviews a diff or plan against constitution, ADRs and specs; read-only | Read, Glob, Grep, Bash |
| `numerics-reviewer` | Precision and determinism review for core, generators, shaders; read-only | Read, Glob, Grep, Bash |

Add later when the need recurs: `render-triage` (M2), `perf-analyst` (M4). Keep the set small; every agent description costs context.

**Skills** (`.claude/skills/`; procedures that become slash commands):

| Skill | Purpose | Who invokes |
|---|---|---|
| `/adr` | New ADR (always `proposed` when Claude writes it), linked from specs | Claude or owner |
| `/spike` | Open/close a time-boxed spike with success criteria and fallback | Claude or owner |
| `/learning` | Append a finding and the assumption it changes | Claude or owner |
| `/bless` | Review candidate goldens and promote approved ones | **Owner only** |
| `/accept` | Milestone acceptance and report; stops at go/no-go gates | Claude or owner |

**Permissions** (`.claude/settings.json`): starts sessions in `acceptEdits` mode; allows `cargo`, `rustup`, `git`, `openspec`, `nvidia-smi`, selected `gh` subcommands and documentation domains; denies force-push, `gh repo delete` and PowerShell `Remove-Item`. For fully unattended runs you can additionally use auto mode if your plan offers it; the deny rules and the stop conditions in `CLAUDE.md` still apply. Argument-level deny rules are not airtight (a reordered command can slip past), which is why branch protection on GitHub backs them up.

## 8. M0 plan (what the kickoff run executes)

1. Create the Cargo workspace and crate skeleton (report §10), with GPU-free crates compiling without wgpu; add `xtask` with `test fast`, `test`, `test gpu`, `spec-lint` (stubs are fine where the underlying tests don't exist yet).
2. Draft the decide-now ADRs listed in `docs/adr/README.md` as `proposed`, each citing its report section.
3. Seed `openspec/specs/` with the subsystem specs from Section 4, requirement IDs and tiers included; constitution rules are referenced, not copied.
4. Set up CI for Windows (WARP) and Linux (lavapipe) with a headless "hello triangle" golden test (candidate goldens wait for `/bless`).
5. Build the performance-harness skeleton (adapter check, logged clocks and power, warm-up, medians). Running it for real needs the owner's go (charger, power profile).
6. Run `/accept M0`, then open the first M1 changes: `m1-coordinates` and `m1-walking-skeleton`, and continue into M1.

## 9. Precedence and the background folder

1. `docs/constitution.md`
2. accepted ADRs (`docs/adr/`)
3. OpenSpec specs (`openspec/specs/`)
4. `docs/background/` (this doc, the architecture report): frozen, non-normative.

When sources conflict, the higher one wins and the lower one is corrected, except background docs, which are never corrected. ADRs and specs cite the report section they came from ("Source: background report §5"), so the report is read one section at a time, only when needed. `CLAUDE.md` lists the background folder under "read only when pointed to".

## 10. Kickoff prompt

Paste this into Claude Code in the repo after Sections 1–4:

> Read CLAUDE.md and docs/constitution.md. Execute the M0 plan in docs/background/starter.md §8, then continue into M1 via OpenSpec changes. Draft the decide-now ADRs as "proposed" and proceed on their leanings. Use the verifier agent before closing each task. Stop and write a stop report in docs/status.md if: a test can't be made deterministic, a hard-to-reverse decision isn't covered by an ADR or spec, golden images await blessing and further work depends on them, a performance measurement on Rasierklinge is needed, or you reach the M2 go/no-go gate. Record surprises with /learning.

What needs you after the run: accept or change the proposed ADRs (one batch), `/bless` the candidate goldens, confirm the machine for performance runs, and decide at the go/no-go gates. `docs/status.md` has a "Waiting for the owner" section that collects these.

## Sources

- Claude Code setup on Windows: https://code.claude.com/docs/en/setup
- Claude Code subagents: https://code.claude.com/docs/en/sub-agents
- Claude Code skills: https://code.claude.com/docs/en/skills
- Claude Code permissions: https://code.claude.com/docs/en/permissions
- OpenSpec: https://github.com/Fission-AI/OpenSpec and supported tools: https://github.com/Fission-AI/OpenSpec/blob/main/docs/supported-tools.md
- GitHub Spec Kit: https://github.com/github/spec-kit and installation: https://github.com/github/spec-kit/blob/main/docs/installation.md
- RTX 3080 Ti Laptop GPU specs: https://www.notebookcheck.net/NVIDIA-GeForce-RTX-3080-Ti-Laptop-GPU-GPU-Benchmarks-and-Specs.588451.0.html
